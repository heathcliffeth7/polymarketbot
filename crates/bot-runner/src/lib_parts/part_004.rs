pub async fn run() -> Result<()> {
    init_tracing();
    let _process_lock = acquire_runner_process_lock()?;
    info!(
        lock_path = %_process_lock.path.display(),
        pid = std::process::id(),
        "BOT_RUNNER_PROCESS_LOCK_ACQUIRED"
    );

    let config_dir = current_config_dir();
    let database_url = env::var("DATABASE_URL").context(
        "DATABASE_URL is required, e.g. postgres://postgres:postgres@localhost:5432/dextrabot",
    )?;

    let cfg = AppConfig::load(&config_dir)?;
    trade_flow::guards::price_to_beat::run_price_to_beat_iv_gap_gate_startup_self_check()
        .context("price_to_beat iv gap gate startup self-check failed")?;

    let repo = PostgresRepository::new(&database_url).await?;
    let _runner_db_lock = repo
        .try_acquire_runner_singleton_lock(BOT_RUNNER_DB_LOCK_KEY)
        .await?
        .ok_or_else(|| anyhow::anyhow!("another bot-runner process is already active"))?;
    info!(
        lock_key = _runner_db_lock.lock_key(),
        "BOT_RUNNER_DB_LOCK_ACQUIRED"
    );
    let mode = match cfg.bot.mode {
        ExecutionMode::Paper => "paper",
        ExecutionMode::Live => "live",
    };

    let process_start_time = Utc::now();
    let mut config_snapshot = json!({
        "bot": {
            "mode": mode,
            "market_scope": cfg.bot.market_scope,
            "market_slug_override": cfg.bot.market_slug_override,
            "loop_interval_ms": cfg.bot.loop_interval_ms,
            "market_discovery_retry_interval_ms": cfg.bot.market_discovery_retry_interval_ms,
            "market_discovery_timeout_sec": cfg.bot.market_discovery_timeout_sec,
            "market_selection": cfg.bot.market_selection
        },
        "strategy": {
            "entry_price": cfg.strategy.entry_price,
            "tp_pct": cfg.strategy.tp_pct,
            "base_sl_pct": cfg.strategy.base_sl_pct,
            "aggressive_sl_pct": cfg.strategy.aggressive_sl_pct,
            "entry_window_sec": cfg.strategy.entry_window_sec,
            "max_hold_sec": cfg.strategy.max_hold_sec,
            "sl_renew_interval_ms": cfg.strategy.sl_renew_interval_ms,
            "flow_only": cfg.strategy.flow_only,
            "dual_side_enabled": cfg.strategy.dual_side_enabled,
            "total_notional_usdc": cfg.strategy.total_notional_usdc,
            "per_leg_initial_notional_usdc": cfg.strategy.per_leg_initial_notional_usdc,
            "dca_interval_sec": cfg.strategy.dca_interval_sec,
            "dca_step_pct": cfg.strategy.dca_step_pct,
            "max_dca_levels_per_leg": cfg.strategy.max_dca_levels_per_leg,
            "leg_tp_pct": cfg.strategy.leg_tp_pct,
            "basket_tp_usdc": cfg.strategy.basket_tp_usdc,
            "basket_sl_usdc": cfg.strategy.basket_sl_usdc,
            "force_flatten_sec_before_close": cfg.strategy.force_flatten_sec_before_close
        },
        "risk": {
            "max_daily_loss_usdc": cfg.risk.max_daily_loss_usdc,
            "max_consecutive_losses": cfg.risk.max_consecutive_losses,
            "max_notional_per_market_usdc": cfg.risk.max_notional_per_market_usdc,
            "max_open_orders": cfg.risk.max_open_orders,
            "max_stale_data_ms": cfg.risk.max_stale_data_ms,
            "kill_switch_mode": cfg.risk.kill_switch_mode,
            "manual_kill_switch_active": cfg.risk.manual_kill_switch_active
        },
        "execution": {
            "order_type": cfg.execution.order_type,
            "time_in_force": cfg.execution.time_in_force,
            "retry_count": cfg.execution.retry_count,
            "retry_backoff_ms": cfg.execution.retry_backoff_ms,
            "reconcile_interval_ms": cfg.execution.reconcile_interval_ms
        },
        "exchange": {
            "gamma_base_url": cfg.exchange.gamma_base_url,
            "clob_base_url": cfg.exchange.clob_base_url,
            "clob_ws_url": cfg.exchange.clob_ws_url,
            "chain_id": cfg.exchange.chain_id
        },
        "claim": {
            "enabled": cfg.claim.enabled,
            "rpc_url": cfg.claim.rpc_url,
            "rpc_url_env": cfg.claim.rpc_url_env,
            "data_api_base_url": cfg.claim.data_api_base_url,
            "user_address_inline_set": !cfg.claim.user_address.trim().is_empty(),
            "user_address_env": cfg.claim.user_address_env,
            "private_key_inline_set": !cfg.claim.private_key.trim().is_empty(),
            "private_key_env": cfg.claim.private_key_env,
            "chain_id": cfg.claim.chain_id,
            "ctf_contract_address": cfg.claim.ctf_contract_address,
            "collateral_token_address": cfg.claim.collateral_token_address,
            "discovery_interval_sec": cfg.claim.discovery_interval_sec,
            "positions_page_size": cfg.claim.positions_page_size,
            "positions_max_pages": cfg.claim.positions_max_pages,
            "process_batch_size": cfg.claim.process_batch_size,
            "max_attempts": cfg.claim.max_attempts,
            "retry_backoff_ms": cfg.claim.retry_backoff_ms
        }
    });
    let config_hash = bot_runtime_config_hash(&config_snapshot);
    set_bot_runtime_run_config_hash(&config_hash);
    let run_metadata = bot_run_start_metadata(process_start_time, config_hash.clone());
    if let Some(obj) = config_snapshot.as_object_mut() {
        obj.insert(
            "runtime".to_string(),
            json!({
                "package_version": &run_metadata.package_version,
                "git_sha": &run_metadata.git_sha,
                "build_time": &run_metadata.build_time,
                "process_start_time": run_metadata.process_start_time.to_rfc3339(),
                "config_hash": &run_metadata.config_hash,
            }),
        );
    }
    let run_id = repo
        .record_run_start(mode, env!("CARGO_PKG_VERSION"), &run_metadata)
        .await?;
    repo.store_config_snapshot(run_id, &config_hash, &config_snapshot)
        .await?;

    info!(run_id, mode, "BOT_STARTED");

    let mut standalone_auto_claim = match AutoClaimService::from_app_config(1, &cfg) {
        Ok(Some(service)) => {
            info!(run_id, "STANDALONE_AUTO_CLAIM_INITIALIZED");
            Some(service)
        }
        Ok(None) => {
            info!(run_id, "STANDALONE_AUTO_CLAIM_DISABLED");
            None
        }
        Err(err) => {
            warn!(run_id, error = %err, "STANDALONE_AUTO_CLAIM_INIT_FAILED");
            None
        }
    };

    let scopes = cfg.bot.resolve_scopes();

    if scopes.len() == 1 {
        loop {
            if let Some(ref mut claim_service) = standalone_auto_claim {
                if let Err(err) = claim_service.maybe_tick(&repo).await {
                    warn!(run_id, error = %err, "STANDALONE_AUTO_CLAIM_TICK_FAILED");
                }
            }
            let result = match cfg.bot.mode {
                ExecutionMode::Paper => run_paper_loop(run_id, &repo, &cfg).await,
                ExecutionMode::Live => run_live_loop(run_id, &repo, &cfg).await,
            };
            if let Err(e) = result {
                repo.record_run_stop(run_id, "error").await.ok();
                info!(run_id, "BOT_STOPPED");
                return Err(e);
            }
            info!(run_id, "MARKET_CYCLE_COMPLETE_SEARCHING_NEXT");
            // no sleep: discovery loop starts immediately, auto_claim continues uninterrupted
        }
    } else {
        let mut handles = Vec::new();
        for scope in scopes {
            let mut scope_cfg = cfg.clone();
            scope_cfg.bot.market_scope = scope.clone();
            let scope_repo = repo.clone();
            let scope_run_id = scope_repo
                .record_run_start(mode, env!("CARGO_PKG_VERSION"), &run_metadata)
                .await?;
            info!(
                run_id = scope_run_id,
                parent_run_id = run_id,
                scope = scope.as_str(),
                "SCOPE_STARTED"
            );
            let handle = tokio::spawn(async move {
                loop {
                    let result = match scope_cfg.bot.mode {
                        ExecutionMode::Paper => {
                            run_paper_loop(scope_run_id, &scope_repo, &scope_cfg).await
                        }
                        ExecutionMode::Live => {
                            run_live_loop(scope_run_id, &scope_repo, &scope_cfg).await
                        }
                    };
                    if let Err(e) = result {
                        error!(run_id = scope_run_id, error = %e, "SCOPE_TASK_FAILED");
                        let _ = scope_repo.record_run_stop(scope_run_id, "error").await;
                        break;
                    }
                    info!(
                        run_id = scope_run_id,
                        "MARKET_CYCLE_COMPLETE_SEARCHING_NEXT"
                    );
                    // no sleep: discovery loop starts immediately
                }
            });
            handles.push(handle);
        }
        for handle in handles {
            let _ = handle.await;
        }
        repo.record_run_stop(run_id, "all scopes complete").await?;
        info!(run_id, "BOT_STOPPED");
    }
    Ok(())
}

fn bot_run_start_metadata(
    process_start_time: DateTime<Utc>,
    config_hash: String,
) -> bot_infra::db::BotRunStartMetadata {
    bot_infra::db::BotRunStartMetadata {
        package_version: env!("CARGO_PKG_VERSION").to_string(),
        git_sha: bot_build_git_sha().to_string(),
        build_time: bot_build_time().to_string(),
        process_start_time,
        config_hash,
    }
}

static BOT_RUNTIME_RUN_CONFIG_HASH: LazyLock<StdMutex<Option<String>>> =
    LazyLock::new(|| StdMutex::new(None));

fn set_bot_runtime_run_config_hash(config_hash: &str) {
    if let Ok(mut value) = BOT_RUNTIME_RUN_CONFIG_HASH.lock() {
        *value = Some(config_hash.to_string());
    }
}

fn bot_runtime_run_config_hash() -> Option<String> {
    BOT_RUNTIME_RUN_CONFIG_HASH
        .lock()
        .ok()
        .and_then(|value| value.clone())
}

fn bot_build_git_sha() -> &'static str {
    option_env!("GIT_SHA")
        .or(option_env!("VERGEN_GIT_SHA"))
        .unwrap_or("unknown")
}

fn bot_build_time() -> &'static str {
    option_env!("BUILD_TIME")
        .or(option_env!("VERGEN_BUILD_TIMESTAMP"))
        .unwrap_or("unknown")
}

fn bot_runtime_config_hash(payload: &Value) -> String {
    use sha2::{Digest, Sha256};

    let raw = serde_json::to_vec(payload).unwrap_or_default();
    let digest = Sha256::digest(raw);
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

async fn run_paper_loop(run_id: i64, repo: &PostgresRepository, cfg: &AppConfig) -> Result<()> {
    if cfg.strategy.flow_only {
        return run_flow_only_loop(run_id, repo, cfg).await;
    }
    if cfg.strategy.dual_side_enabled {
        return run_paper_dual_loop(run_id, repo, cfg).await;
    }

    let cycle = MarketCycleId::from_now_rounded_5m(Utc::now());
    let market_id = repo.upsert_market_cycle(&cycle).await?;
    let market_slug = cycle.to_string();
    info!(run_id, market_id, market = %cycle, "MARKET_DISCOVERED");

    run_daily_pnl_startup_check(run_id, repo, cfg.risk.max_daily_loss_usdc).await?;

    let mut provider = MockMarketDataProvider::new();
    let strategy = PriceThresholdStrategy;
    let policy = DefaultRiskPolicy;
    let mut trade = create_runtime(repo, cfg, market_id, market_slug, &strategy).await?;
    transition(repo, &mut trade, TradeState::WaitingEntry, "cycle-start").await?;

    let limits = to_risk_limits(cfg);

    for iter in 0..12u32 {
        let tick = provider.next_tick(&trade.market_slug)?;
        let snapshot = provider.snapshot(&trade.market_slug)?;
        let now_ms = Utc::now().timestamp_millis();
        let merged = reconcile_tick_and_snapshot(tick.as_ref(), &snapshot, now_ms);

        if tick.is_none() {
            warn!(run_id, trade_id = trade.trade_id, iter, "WS_STALE");
        }

        match risk_gate(
            repo,
            run_id,
            cfg,
            &trade,
            &limits,
            merged.stale_data_ms,
            &policy,
        )
        .await?
        {
            RiskDecision::Halt => {
                halt_trade(repo, run_id, &mut trade, "risk-halt", None).await?;
                break;
            }
            RiskDecision::Block => {
                sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
                continue;
            }
            RiskDecision::Allow => {}
        }

        process_trade_step(
            repo,
            run_id,
            cfg,
            &mut trade,
            merged.chosen_price,
            true,
            &strategy,
        )
        .await?;

        if matches!(trade.state, TradeState::Settled | TradeState::Halted) {
            break;
        }

        sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
    }

    if can_transition(trade.state, TradeState::Settled).is_ok() {
        transition(repo, &mut trade, TradeState::Settled, "paper-loop-end").await?;
    }

    Ok(())
}
