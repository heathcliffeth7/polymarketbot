const AUTO_CLAIM_INIT_RETRY_COOLDOWN_SECS: u64 = 60;

#[derive(Default)]
struct FlowAutoClaimRuntime {
    service: Option<AutoClaimService>,
    last_init_failure_at: Option<Instant>,
}

type SharedOrderExecutor = Arc<dyn OrderExecutor>;

#[derive(Default)]
struct FlowRuntimeCaches {
    user_cfg: HashMap<i64, AppConfig>,
    user_executor: HashMap<i64, SharedOrderExecutor>,
    last_used: HashMap<i64, Instant>,
}

impl FlowRuntimeCaches {
    fn touch(&mut self, user_id: i64) {
        self.last_used.insert(user_id, Instant::now());
    }

    fn prune_stale(&mut self) {
        let now = Instant::now();
        self.last_used.retain(|user_id, last_used| {
            let keep = now.duration_since(*last_used).as_secs() <= FLOW_RUNTIME_CACHE_TTL_SECS;
            if !keep {
                self.user_cfg.remove(user_id);
                self.user_executor.remove(user_id);
            }
            keep
        });
    }
}

async fn maybe_tick_flow_auto_claims(
    repo: &PostgresRepository,
    run_id: i64,
    definitions: &[TradeFlowDefinitionRuntime],
    user_cfg_cache: &mut HashMap<i64, AppConfig>,
    auto_claim_runtimes: &mut HashMap<i64, FlowAutoClaimRuntime>,
) {
    let mut enabled_user_ids = HashSet::new();

    for definition in definitions {
        let Some(version_id) = definition.published_version_id else {
            continue;
        };
        let version = match repo.get_trade_flow_version(version_id).await {
            Ok(Some(version)) => version,
            Ok(None) => continue,
            Err(err) => {
                warn!(
                    run_id,
                    definition_id = definition.id,
                    user_id = definition.user_id,
                    error = %err,
                    "AUTO_CLAIM_FLOW_VERSION_LOAD_FAILED"
                );
                continue;
            }
        };
        let graph = match parse_trade_flow_graph(&version) {
            Ok(graph) => graph,
            Err(err) => {
                warn!(
                    run_id,
                    definition_id = definition.id,
                    user_id = definition.user_id,
                    error = %err,
                    "AUTO_CLAIM_FLOW_GRAPH_PARSE_FAILED"
                );
                continue;
            }
        };
        if graph
            .context
            .get("autoClaimEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            enabled_user_ids.insert(definition.user_id);
        }
    }

    for user_id in enabled_user_ids {
        let auto_claim = auto_claim_runtimes.entry(user_id).or_default();

        if auto_claim.service.is_none() {
            if let Some(last_failure) = auto_claim.last_init_failure_at {
                if last_failure.elapsed()
                    < std::time::Duration::from_secs(AUTO_CLAIM_INIT_RETRY_COOLDOWN_SECS)
                {
                    continue;
                }
            }

            let cfg = match load_user_app_config_cached(repo, user_id, user_cfg_cache).await {
                Ok(cfg) => cfg,
                Err(err) => {
                    warn!(run_id, user_id, error = %err, "AUTO_CLAIM_USER_CONFIG_LOAD_FAILED");
                    auto_claim.last_init_failure_at = Some(Instant::now());
                    continue;
                }
            };
            match AutoClaimService::from_app_config(user_id, &cfg) {
                Ok(service) => {
                    if service.is_none() {
                        warn!(
                            run_id,
                            user_id, "AUTO_CLAIM_FLOW_ENABLED_BUT_CLAIM_DISABLED"
                        );
                        auto_claim.last_init_failure_at = Some(Instant::now());
                    }
                    auto_claim.service = service;
                }
                Err(err) => {
                    warn!(
                        run_id,
                        user_id,
                        error = %err,
                        "AUTO_CLAIM_FLOW_ENABLED_BUT_CONFIG_INVALID"
                    );
                    auto_claim.last_init_failure_at = Some(Instant::now());
                    continue;
                }
            }
        }

        if let Some(service) = auto_claim.service.as_mut() {
            if let Err(err) = service.maybe_tick(repo).await {
                warn!(run_id, user_id, error = %err, "AUTO_CLAIM_TICK_FAILED");
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct UpdownScopeDef {
    scope: &'static str,
    asset: &'static str,
    timeframe: &'static str,
    slug_prefix: &'static str,
}
