#[derive(Default)]
struct FlowAutoClaimRuntime {
    service: Option<AutoClaimService>,
    init_attempted: bool,
}

type SharedOrderExecutor = Arc<dyn OrderExecutor>;

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
        if !auto_claim.init_attempted {
            auto_claim.init_attempted = true;
            let cfg = match load_user_app_config_cached(repo, user_id, user_cfg_cache).await {
                Ok(cfg) => cfg,
                Err(err) => {
                    warn!(run_id, user_id, error = %err, "AUTO_CLAIM_USER_CONFIG_LOAD_FAILED");
                    continue;
                }
            };
            match AutoClaimService::from_app_config(&cfg) {
                Ok(service) => {
                    if service.is_none() {
                        warn!(
                            run_id,
                            user_id, "AUTO_CLAIM_FLOW_ENABLED_BUT_CLAIM_DISABLED"
                        );
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

