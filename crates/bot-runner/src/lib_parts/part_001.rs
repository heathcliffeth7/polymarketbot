const AUTO_CLAIM_INIT_RETRY_COOLDOWN_SECS: u64 = 60;

#[derive(Default)]
struct FlowAutoClaimRuntime {
    service: Option<AutoClaimService>,
    last_init_failure_at: Option<Instant>,
}

type SharedOrderExecutor = Arc<dyn OrderExecutor>;

#[derive(Debug, Clone)]
struct FlowStepProcessingContext {
    id: String,
    started_at: DateTime<Utc>,
}

impl FlowStepProcessingContext {
    fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            started_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct FlowStepProcessingStats {
    processing_run_id: String,
    claim_passes: u8,
    claimed_step_count: usize,
    last_pass_claimed_steps: usize,
    ptb_retry_blocked_count: u64,
    ptb_retry_created_count: u64,
    ptb_retry_same_run_excluded_count: i64,
    ready_remaining_total_count: i64,
    ready_remaining_ptb_retry_count: i64,
    runnable_non_retry_ready_count: i64,
    clob_book_fetch_hit_count: u64,
    clob_book_fetch_pass_cache_hit_count: u64,
    clob_book_fetch_process_ttl_hit_count: u64,
    clob_book_fetch_miss_count: u64,
    clob_book_fetch_error_count: u64,
    unique_book_tokens_fetched: usize,
    coalesced_event_suppressed_count: u64,
}

#[derive(Default)]
struct FlowRuntimeCaches {
    user_cfg: HashMap<i64, AppConfig>,
    user_executor: HashMap<i64, SharedOrderExecutor>,
    last_used: HashMap<i64, Instant>,
    last_step_processing_stats: FlowStepProcessingStats,
    last_stale_running_step_recovery_at: Option<Instant>,
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
    _definitions: &[TradeFlowDefinitionRuntime],
    user_cfg_cache: &mut HashMap<i64, AppConfig>,
    auto_claim_runtimes: &mut HashMap<i64, FlowAutoClaimRuntime>,
) {
    let enabled_user_ids = match repo.list_auto_claim_candidate_user_ids().await {
        Ok(user_ids) => user_ids,
        Err(err) => {
            warn!(
                run_id,
                error = %err,
                "AUTO_CLAIM_CANDIDATE_USERS_LOAD_FAILED"
            );
            return;
        }
    };
    let enabled_user_ids_set = enabled_user_ids.iter().copied().collect::<HashSet<_>>();
    auto_claim_runtimes.retain(|user_id, _| enabled_user_ids_set.contains(user_id));

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
                        warn!(run_id, user_id, "AUTO_CLAIM_DISABLED_FOR_USER");
                        auto_claim.last_init_failure_at = Some(Instant::now());
                    }
                    auto_claim.service = service;
                }
                Err(err) => {
                    warn!(run_id, user_id, error = %err, "AUTO_CLAIM_CONFIG_INVALID");
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
