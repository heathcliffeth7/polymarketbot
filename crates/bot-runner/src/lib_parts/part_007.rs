async fn run_live_loop(run_id: i64, repo: &PostgresRepository, cfg: &AppConfig) -> Result<()> {
    if cfg.strategy.flow_only {
        return run_flow_only_loop(run_id, repo, cfg).await;
    }
    if cfg.strategy.dual_side_enabled {
        return run_live_dual_loop(run_id, repo, cfg).await;
    }

    let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
    let selected = discover_live_market(run_id, repo, cfg, &gamma, false).await?;
    let selected_reason = selected.selection_reason.as_str();
    let selected_start_at = selected.starts_at.as_ref().cloned();
    let selected_end_at = selected.ends_at.as_ref().cloned();
    let cycle_slug = selected.slug;
    let entry_token_id = selected
        .yes_token_id
        .clone()
        .or(selected.no_token_id.clone());

    let cycle = MarketCycleId(cycle_slug.clone());
    let market_id = repo.upsert_market_cycle(&cycle).await?;
    info!(
        run_id,
        market_id,
        market = %cycle_slug,
        selection_reason = selected_reason,
        market_start_at = ?selected_start_at,
        market_end_at = ?selected_end_at,
        "LIVE_MARKET_DISCOVERED"
    );

    let ws = ClobWsClient::new(cfg.exchange.clob_ws_url.clone());
    let mut user_ws_events: Vec<WsEvent> = Vec::new();
    let market_ws_ids = if let Some(token_id) = entry_token_id.as_ref() {
        vec![token_id.clone()]
    } else {
        vec![cycle_slug.clone()]
    };
    match ws.subscribe_once(WsChannel::Market, &market_ws_ids).await {
        Ok(messages) => {
            info!(run_id, market = %cycle_slug, ws_messages = messages.len(), "WS_CONNECT_OK")
        }
        Err(e) => {
            warn!(run_id, market = %cycle_slug, error = %e, "WS_CONNECT_FAILED_USING_REST_FALLBACK")
        }
    }

    let live_enabled = env::var("LIVE_TRADING_ENABLED").ok().as_deref() == Some("true");
    let strategy = PriceThresholdStrategy;
    let policy = DefaultRiskPolicy;
    let mut clob_client = None;
    if live_enabled {
        let (creds, credential_source) = resolve_api_credentials_with_source(cfg)?;
        log_resolved_api_credentials(run_id, &creds, credential_source);
        let private_key = cfg
            .exchange
            .resolve_signer_private_key()
            .context("CLOB signer private key")?;
        let wallet = private_key
            .parse::<LocalWallet>()
            .context("parse signer private key")?
            .with_chain_id(cfg.exchange.chain_id);
        let (exchange_address, neg_risk_exchange_address) = resolve_clob_exchange_addresses(cfg)?;
        let gnosis_safe: Option<Address> = cfg
            .exchange
            .resolve_gnosis_safe_address()
            .map(|s| s.parse::<Address>().context("parse gnosis_safe_address"))
            .transpose()?;
        let client = ClobHttpClient::from_credentials(
            cfg.exchange.clob_base_url.clone(),
            Some(cfg.claim.data_api_base_url.clone()),
            cfg.claim.positions_page_size,
            cfg.claim.positions_max_pages,
            creds.clone(),
            wallet,
            exchange_address,
            neg_risk_exchange_address,
            cfg.exchange.chain_id,
            gnosis_safe,
        );
        run_clob_auth_preflight(run_id, repo, &client, &creds, credential_source).await;
        clob_client = Some(client);
        match ws
            .subscribe_once(WsChannel::User, &[cycle_slug.clone()])
            .await
        {
            Ok(messages) => {
                info!(run_id, market = %cycle_slug, ws_messages = messages.len(), "WS_USER_CONNECT_OK");
                user_ws_events = messages;
            }
            Err(e) => {
                warn!(run_id, market = %cycle_slug, error = %e, "WS_USER_CONNECT_FAILED_USING_REST_RECONCILE")
            }
        }
    }
    let mut auto_claim_runtimes: HashMap<i64, FlowAutoClaimRuntime> = HashMap::new();
    let mut flow_runtime_caches = FlowRuntimeCaches::default();

    run_daily_pnl_startup_check(run_id, repo, cfg.risk.max_daily_loss_usdc).await?;
    if let Some(client) = clob_client.as_ref() {
        run_balance_preflight(run_id, repo, client, cfg.risk.min_balance_usdc).await;
    }

    let mut trade = create_runtime(repo, cfg, market_id, cycle_slug.clone(), &strategy).await?;
    transition(
        repo,
        &mut trade,
        TradeState::WaitingEntry,
        "live-cycle-start",
    )
    .await?;
    let limits = to_risk_limits(cfg);

    let mut reconcile_errors = 0u32;
    for iter in 0..20u32 {
        let price = if let Some(client) = clob_client.as_ref() {
            let midpoint_key = entry_token_id
                .as_deref()
                .unwrap_or(trade.market_slug.as_str());
            match client.midpoint(midpoint_key).await {
                Ok(s) => s.price,
                Err(e) => {
                    warn!(run_id, trade_id = trade.trade_id, error = %e, "REST_SNAPSHOT_FAILED");
                    trade.entry_price
                }
            }
        } else {
            // Dry live mode: no API keys, evaluate state machine with deterministic price.
            if iter % 4 == 0 {
                trade.tp_price
            } else {
                trade.entry_price
            }
        };

        if live_enabled {
            if let Some(client) = clob_client.as_ref() {
                if let Err(e) = process_trade_builder_orders(repo, run_id, cfg, client, &ws).await {
                    warn!(run_id, error = %e, "TRADE_BUILDER_PROCESS_FAILED");
                }
                if let Err(e) =
                    process_trade_builder_workflows(repo, run_id, cfg, client, &ws).await
                {
                    warn!(run_id, error = %e, "TRADE_BUILDER_WORKFLOW_PROCESS_FAILED");
                }
                if let Err(e) =
                    dca::process_trade_flow_dual_dca_jobs(repo, run_id, cfg, client, &ws).await
                {
                    warn!(run_id, error = %e, "TRADE_FLOW_DUAL_DCA_PROCESS_FAILED");
                }
            }
        }
        if let Err(e) = process_trade_flows(
            repo,
            run_id,
            cfg,
            clob_client
                .as_ref()
                .map(|client| client as &dyn OrderExecutor),
            &ws,
            &mut flow_runtime_caches,
            &mut auto_claim_runtimes,
        )
        .await
        {
            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED");
        }

        match risk_gate(repo, run_id, cfg, &trade, &limits, 0, &policy).await? {
            RiskDecision::Halt => {
                halt_trade(
                    repo,
                    run_id,
                    &mut trade,
                    "risk-halt",
                    clob_client
                        .as_ref()
                        .map(|client| client as &dyn OrderExecutor),
                )
                .await?;
                break;
            }
            RiskDecision::Block => {
                sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
                continue;
            }
            RiskDecision::Allow => {}
        }

        if live_enabled
            && matches!(trade.state, TradeState::WaitingEntry)
            && strategy.entry_signal(price, trade.entry_price)
        {
            if let Some(client) = clob_client.as_ref() {
                transition(
                    repo,
                    &mut trade,
                    TradeState::EntryPlaced,
                    "entry-threshold-hit",
                )
                .await?;
                let client_order_id = Uuid::new_v4().to_string();
                let req = PlaceOrderRequest {
                    market: trade.market_slug.clone(),
                    token_id: entry_token_id.clone(),
                    side: "buy".to_string(),
                    price: trade.entry_price,
                    size: trade.position_size,
                    intent: "entry".to_string(),
                    order_type: "GTC".to_string(),
                    client_order_id: client_order_id.clone(),
                    leg_side: None,
                    fee_rate_bps: 1000,
                    neg_risk: false,
                };

                let ack = client.place(&req).await?;
                let exchange_order_id = ack
                    .exchange_order_id
                    .as_deref()
                    .unwrap_or(&ack.client_order_id);
                let raw = json!({
                    "client_order_id": ack.client_order_id,
                    "exchange_order_id": ack.exchange_order_id,
                    "status": ack.status,
                    "reject_reason": ack.reject_reason,
                    "raw_status": ack.raw_status,
                    "exchange_ts": ack.exchange_ts
                });
                repo.upsert_order_by_exchange_id(
                    trade.trade_id,
                    exchange_order_id,
                    Some(&ack.client_order_id),
                    "entry",
                    "buy",
                    trade.entry_price,
                    trade.position_size,
                    &ack.status,
                    ack.exchange_ts,
                    ack.reject_reason.as_deref(),
                    &raw,
                )
                .await?;
                info!(run_id, trade_id = trade.trade_id, client_order_id = %ack.client_order_id, "LIVE_ENTRY_ACK");
            }
        }

        if live_enabled {
            if let Some(client) = clob_client.as_ref() {
                match reconcile_live(repo, run_id, &mut trade, client, &user_ws_events).await? {
                    ReconcileOutcome::Ok => {
                        reconcile_errors = 0;
                    }
                    ReconcileOutcome::Warning => {
                        reconcile_errors = 0;
                    }
                    ReconcileOutcome::Error => {
                        reconcile_errors += 1;
                        if reconcile_errors >= 3 {
                            halt_trade(
                                repo,
                                run_id,
                                &mut trade,
                                "reconcile-error-threshold",
                                Some(client),
                            )
                            .await?;
                            warn!(
                                run_id,
                                trade_id = trade.trade_id,
                                "RECONCILE_ERROR_THRESHOLD_HALTED"
                            );
                            break;
                        }
                        sleep(Duration::from_millis(cfg.execution.retry_backoff_ms)).await;
                        continue;
                    }
                }
            }
        } else {
            process_trade_step(repo, run_id, cfg, &mut trade, price, true, &strategy).await?;
        }

        if matches!(trade.state, TradeState::Settled | TradeState::Halted) {
            break;
        }

        sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
    }

    if can_transition(trade.state, TradeState::Settled).is_ok() {
        transition(repo, &mut trade, TradeState::Settled, "live-loop-end").await?;
    }

    Ok(())
}

async fn run_paper_dual_loop(
    run_id: i64,
    repo: &PostgresRepository,
    cfg: &AppConfig,
) -> Result<()> {
    let cycle = MarketCycleId::from_now_rounded_5m(Utc::now());
    let market_id = repo.upsert_market_cycle(&cycle).await?;
    let market_slug = cycle.to_string();
    let cycle_ends_at = cycle.start_time().unwrap_or_else(Utc::now) + ChronoDuration::seconds(300);
    info!(run_id, market_id, market = %market_slug, "PAPER_DUAL_MARKET_DISCOVERED");

    run_daily_pnl_startup_check(run_id, repo, cfg.risk.max_daily_loss_usdc).await?;

    let strategy = SymmetricDualDcaStrategy;
    let policy = DefaultRiskPolicy;
    let limits = to_risk_limits(cfg);

    let mut basket = create_dual_runtime(
        repo,
        cfg,
        market_id,
        market_slug.clone(),
        "paper-yes".to_string(),
        "paper-no".to_string(),
        0,
        cycle_ends_at,
    )
    .await?;

    transition_dual(
        repo,
        &mut basket,
        TradeState::WaitingEntry,
        "paper-dual-cycle-start",
    )
    .await?;
    transition_dual(
        repo,
        &mut basket,
        TradeState::EntryPlaced,
        "paper-dual-initial-entry",
    )
    .await?;

    let mut provider = MockMarketDataProvider::new();
    let level_notional =
        cfg.strategy.per_leg_initial_notional_usdc / cfg.strategy.max_dca_levels_per_leg as f64;
    let start_yes = clamp_probability(cfg.strategy.entry_price);
    let start_no = clamp_probability(1.0 - start_yes);
    let start_yes_size = calc_level_size(level_notional, start_yes);
    let start_no_size = calc_level_size(level_notional, start_no);

    record_paper_leg_fill(
        repo,
        basket.trade_id,
        &mut basket.yes_leg,
        "entry",
        "buy",
        start_yes,
        start_yes_size,
    )
    .await?;
    record_paper_leg_fill(
        repo,
        basket.trade_id,
        &mut basket.no_leg,
        "entry",
        "buy",
        start_no,
        start_no_size,
    )
    .await?;

    transition_dual(
        repo,
        &mut basket,
        TradeState::EntryFilled,
        "paper-dual-entry-filled",
    )
    .await?;
    transition_dual(
        repo,
        &mut basket,
        TradeState::TpPlaced,
        "paper-dual-tp-active",
    )
    .await?;

    for iter in 0..40u32 {
        let tick = provider.next_tick(&basket.market_slug)?;
        let snapshot = provider.snapshot(&basket.market_slug)?;
        let now_ms = Utc::now().timestamp_millis();
        let merged = reconcile_tick_and_snapshot(tick.as_ref(), &snapshot, now_ms);
        let yes_price = clamp_probability(merged.chosen_price);
        let no_price = clamp_probability(1.0 - yes_price);

        match risk_gate_dual(
            repo,
            run_id,
            cfg,
            None,
            basket.trade_id,
            &limits,
            merged.stale_data_ms,
            &policy,
        )
        .await?
        {
            RiskDecision::Halt => {
                transition_dual(repo, &mut basket, TradeState::Halted, "risk-halt").await?;
                break;
            }
            RiskDecision::Block => {
                sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
                continue;
            }
            RiskDecision::Allow => {}
        }

        let now = Utc::now();
        let force_flatten_at = basket.cycle_ends_at
            - ChronoDuration::seconds(cfg.strategy.force_flatten_sec_before_close as i64);
        let basket_pnl = basket_unrealized_pnl(&basket, yes_price, no_price);
        let force_flatten = now >= force_flatten_at;
        let basket_flatten = strategy.should_flatten_basket(
            basket_pnl,
            cfg.strategy.basket_tp_usdc,
            cfg.strategy.basket_sl_usdc,
        );

        if force_flatten || basket_flatten {
            if force_flatten {
                info!(
                    run_id,
                    trade_id = basket.trade_id,
                    "PAPER_DUAL_FORCE_FLATTEN"
                );
            }

            if basket_pnl <= cfg.strategy.basket_sl_usdc && basket.state == TradeState::TpPlaced {
                transition_dual(repo, &mut basket, TradeState::SlArmed, "paper-basket-sl").await?;
            }

            if basket.yes_leg.qty > 0.0 {
                let qty = basket.yes_leg.qty;
                record_paper_leg_fill(
                    repo,
                    basket.trade_id,
                    &mut basket.yes_leg,
                    "basket_exit",
                    "sell",
                    yes_price,
                    qty,
                )
                .await?;
            }
            if basket.no_leg.qty > 0.0 {
                let qty = basket.no_leg.qty;
                record_paper_leg_fill(
                    repo,
                    basket.trade_id,
                    &mut basket.no_leg,
                    "basket_exit",
                    "sell",
                    no_price,
                    qty,
                )
                .await?;
            }
        } else {
            if should_leg_take_profit(
                &strategy,
                &basket.yes_leg,
                yes_price,
                cfg.strategy.leg_tp_pct,
            ) {
                let qty = basket.yes_leg.qty;
                if qty > 0.0 {
                    record_paper_leg_fill(
                        repo,
                        basket.trade_id,
                        &mut basket.yes_leg,
                        "leg_tp",
                        "sell",
                        yes_price,
                        qty,
                    )
                    .await?;
                }
            }
            if should_leg_take_profit(&strategy, &basket.no_leg, no_price, cfg.strategy.leg_tp_pct)
            {
                let qty = basket.no_leg.qty;
                if qty > 0.0 {
                    record_paper_leg_fill(
                        repo,
                        basket.trade_id,
                        &mut basket.no_leg,
                        "leg_tp",
                        "sell",
                        no_price,
                        qty,
                    )
                    .await?;
                }
            }

            maybe_paper_dca(
                repo,
                cfg,
                &strategy,
                basket.trade_id,
                &mut basket.yes_leg,
                yes_price,
                level_notional,
                now,
            )
            .await?;
            maybe_paper_dca(
                repo,
                cfg,
                &strategy,
                basket.trade_id,
                &mut basket.no_leg,
                no_price,
                level_notional,
                now,
            )
            .await?;
        }

        persist_leg_snapshots(repo, &basket).await?;

        if basket.yes_leg.qty <= 0.0 && basket.no_leg.qty <= 0.0 {
            if can_transition(basket.state, TradeState::ExitFilled).is_ok() {
                transition_dual(
                    repo,
                    &mut basket,
                    TradeState::ExitFilled,
                    "paper-dual-exit-filled",
                )
                .await?;
            }
            if can_transition(basket.state, TradeState::Settled).is_ok() {
                transition_dual(repo, &mut basket, TradeState::Settled, "paper-dual-settled")
                    .await?;
            }
            repo.close_trade(basket.trade_id, 0.5, basket_pnl).await?;
            info!(
                run_id,
                trade_id = basket.trade_id,
                pnl = basket_pnl,
                iter,
                "PAPER_DUAL_SETTLED"
            );
            break;
        }

        sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
    }

    Ok(())
}

fn effective_boundary_refresh_delay(
    raw_boundary_refresh_delay: Option<Duration>,
    next_boundary_refresh_retry_at: Option<Instant>,
    now: Instant,
) -> Option<Duration> {
    match raw_boundary_refresh_delay {
        Some(delay) if delay.is_zero() => Some(
            next_boundary_refresh_retry_at
                .map(|retry_at| retry_at.saturating_duration_since(now))
                .unwrap_or(Duration::ZERO),
        ),
        delay => delay,
    }
}

fn boundary_refresh_retry_is_due(
    next_boundary_refresh_retry_at: Option<Instant>,
    now: Instant,
) -> bool {
    next_boundary_refresh_retry_at
        .map(|retry_at| retry_at <= now)
        .unwrap_or(true)
}

async fn attempt_flow_boundary_refresh(
    repo: &PostgresRepository,
    run_id: i64,
    ws: &ClobWsClient,
    user_cfg_cache: &mut HashMap<i64, AppConfig>,
    reason: &'static str,
    next_boundary_refresh_retry_at: &mut Option<Instant>,
) -> bool {
    info!(run_id, reason, "TRADE_FLOW_BOUNDARY_REFRESH_TRIGGERED");
    match refresh_trade_flow_ws_fast_path_for_boundary(repo, run_id, ws, user_cfg_cache).await {
        Ok(()) => {
            *next_boundary_refresh_retry_at = None;
            true
        }
        Err(err) => {
            warn!(
                run_id,
                error = %err,
                reason,
                "TRADE_FLOW_BOUNDARY_REFRESH_FAILED"
            );
            let retry_after_ms = FLOW_BOUNDARY_REFRESH_RETRY_MS;
            *next_boundary_refresh_retry_at =
                Some(Instant::now() + Duration::from_millis(retry_after_ms));
            info!(
                run_id,
                reason, retry_after_ms, "TRADE_FLOW_BOUNDARY_REFRESH_RETRY_SCHEDULED"
            );
            false
        }
    }
}

async fn run_flow_only_loop(run_id: i64, repo: &PostgresRepository, cfg: &AppConfig) -> Result<()> {
    let live_enabled = env::var("LIVE_TRADING_ENABLED").ok().as_deref() == Some("true");
    anyhow::ensure!(
        live_enabled,
        "flow_only mode requires LIVE_TRADING_ENABLED=true"
    );

    // CLOB client init (flows need it for order placement)
    let (creds, credential_source) = resolve_api_credentials_with_source(cfg)?;
    log_resolved_api_credentials(run_id, &creds, credential_source);
    let private_key = cfg
        .exchange
        .resolve_signer_private_key()
        .context("CLOB signer private key")?;
    let wallet = private_key
        .parse::<LocalWallet>()
        .context("parse signer private key")?
        .with_chain_id(cfg.exchange.chain_id);
    let (exchange_address, neg_risk_exchange_address) = resolve_clob_exchange_addresses(cfg)?;
    let gnosis_safe: Option<Address> = cfg
        .exchange
        .resolve_gnosis_safe_address()
        .map(|s| s.parse::<Address>().context("parse gnosis_safe_address"))
        .transpose()?;
    let client = ClobHttpClient::from_credentials(
        cfg.exchange.clob_base_url.clone(),
        Some(cfg.claim.data_api_base_url.clone()),
        cfg.claim.positions_page_size,
        cfg.claim.positions_max_pages,
        creds.clone(),
        wallet,
        exchange_address,
        neg_risk_exchange_address,
        cfg.exchange.chain_id,
        gnosis_safe,
    );
    run_clob_auth_preflight(run_id, repo, &client, &creds, credential_source).await;
    run_daily_pnl_startup_check(run_id, repo, cfg.risk.max_daily_loss_usdc).await?;
    run_balance_preflight(run_id, repo, &client, cfg.risk.min_balance_usdc).await;
    let ws = ClobWsClient::new(cfg.exchange.clob_ws_url.clone());
    let (tick_trigger_tx, mut tick_trigger_rx) =
        tokio::sync::mpsc::unbounded_channel::<TickTrigger>();
    let (snapshot_tx, snapshot_rx) =
        tokio::sync::mpsc::unbounded_channel::<MarketSecondSnapshotTick>();
    tokio::spawn(run_market_second_snapshot_recorder(
        repo.clone(),
        client.clone(),
        snapshot_rx,
    ));
    ws.set_tick_callback(build_combined_market_tick_callback(vec![
        build_tick_trigger_callback(tick_trigger_tx),
        build_market_second_snapshot_callback(snapshot_tx),
    ]))
    .await;
    let mut auto_claim_runtimes: HashMap<i64, FlowAutoClaimRuntime> = HashMap::new();
    let mut flow_runtime_caches = FlowRuntimeCaches::default();

    info!(run_id, "FLOW_ONLY_LOOP_STARTED");

    // Infinite loop — only processes canvas/flow systems, no automatic trades.
    // `trigger.market_price` gets a WS-driven fast-path; heavier housekeeping
    // work stays on a slower cadence.
    let mut loop_count: u64 = 0;
    let housekeeping_interval = Duration::from_millis(FLOW_HOUSEKEEPING_INTERVAL_MS);
    let fast_path_debounce = Duration::from_millis(FLOW_WS_FAST_PATH_DEBOUNCE_MS);
    const FLOW_HOUSEKEEPING_SLOW_THRESHOLD_MS: u128 = 500;
    let mut next_housekeeping_at = Instant::now();
    let mut next_boundary_refresh_retry_at: Option<Instant> = None;
    loop {
        let now = Instant::now();
        if now >= next_housekeeping_at {
            loop_count += 1;
            let housekeeping_started = Instant::now();
            flow_runtime_caches.prune_stale();
            if loop_count % 20 == 1 {
                info!(run_id, loop_count, "FLOW_LOOP_TICK_PRE_FLOWS");
            }
            if let Err(e) = process_trade_flows(
                repo,
                run_id,
                cfg,
                Some(&client),
                &ws,
                &mut flow_runtime_caches,
                &mut auto_claim_runtimes,
            )
            .await
            {
                warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED");
            }
            if loop_count % 20 == 1 {
                info!(run_id, loop_count, "FLOW_LOOP_TICK_POST_FLOWS");
            }
            if let Err(e) = process_trade_builder_orders(repo, run_id, cfg, &client, &ws).await {
                warn!(run_id, error = %e, "TRADE_BUILDER_PROCESS_FAILED");
            }
            if let Err(e) = process_trade_builder_workflows(repo, run_id, cfg, &client, &ws).await {
                warn!(run_id, error = %e, "TRADE_BUILDER_WORKFLOW_PROCESS_FAILED");
            }
            if let Err(e) =
                dca::process_trade_flow_dual_dca_jobs(repo, run_id, cfg, &client, &ws).await
            {
                warn!(run_id, error = %e, "TRADE_FLOW_DUAL_DCA_PROCESS_FAILED");
            }
            let housekeeping_elapsed_ms = housekeeping_started.elapsed().as_millis();
            if housekeeping_elapsed_ms >= FLOW_HOUSEKEEPING_SLOW_THRESHOLD_MS {
                warn!(
                    run_id,
                    loop_count, housekeeping_elapsed_ms, "FLOW_HOUSEKEEPING_SLOW"
                );
            } else if loop_count % 20 == 1 {
                info!(
                    run_id,
                    loop_count, housekeeping_elapsed_ms, "FLOW_HOUSEKEEPING_ELAPSED"
                );
            }
            next_housekeeping_at = Instant::now() + housekeeping_interval;
            continue;
        }

        let remaining = next_housekeeping_at.saturating_duration_since(now);
        let timer_delay = trade_flow_next_trigger_market_price_timer_delay().await;
        let raw_boundary_refresh_delay = trade_flow_next_auto_scope_boundary_refresh_delay().await;
        if !matches!(raw_boundary_refresh_delay, Some(delay) if delay.is_zero()) {
            next_boundary_refresh_retry_at = None;
        }
        let boundary_refresh_delay = effective_boundary_refresh_delay(
            raw_boundary_refresh_delay,
            next_boundary_refresh_retry_at,
            now,
        );
        if matches!(boundary_refresh_delay, Some(delay) if delay.is_zero()) {
            let _ = attempt_flow_boundary_refresh(
                repo,
                run_id,
                &ws,
                &mut flow_runtime_caches.user_cfg,
                "immediate_due",
                &mut next_boundary_refresh_retry_at,
            )
            .await;
            continue;
        }
        if matches!(timer_delay, Some(delay) if delay.is_zero()) {
            if let Err(e) =
                process_trade_flow_trigger_market_price_timers(repo, run_id, &ws, Some(&client))
                    .await
            {
                warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED_TIMER_WAKE");
            }
            if let Err(e) = process_trade_flow_ready_steps(
                repo,
                run_id,
                Some(&client),
                &ws,
                &mut flow_runtime_caches,
            )
            .await
            {
                warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED_TIMER_READY");
            }
            continue;
        }

        if let Some(delay) = timer_delay {
            if let Some(boundary_delay) = boundary_refresh_delay {
                tokio::select! {
                    Some(trigger) = tick_trigger_rx.recv() => {
                        handle_tick_trigger(repo, run_id, &ws, trigger).await;
                    }
                    _ = tokio::time::sleep(remaining) => {}
                    _ = tokio::time::sleep(delay) => {
                        if let Err(e) = process_trade_flow_trigger_market_price_timers(
                            repo,
                            run_id,
                            &ws,
                            Some(&client),
                        ).await {
                            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED_TIMER_WAKE");
                        }
                        if let Err(e) = process_trade_flow_ready_steps(
                            repo,
                            run_id,
                            Some(&client),
                            &ws,
                            &mut flow_runtime_caches,
                        ).await {
                            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED_TIMER_READY");
                        }
                    }
                    _ = tokio::time::sleep(boundary_delay) => {
                        let _ = attempt_flow_boundary_refresh(
                            repo,
                            run_id,
                            &ws,
                            &mut flow_runtime_caches.user_cfg,
                            "sleep_wake",
                            &mut next_boundary_refresh_retry_at,
                        )
                        .await;
                    }
                    _ = ws.wait_for_market_update() => {
                        tokio::time::sleep(fast_path_debounce).await;
                        if trade_flow_ws_fast_path_cache_requires_refresh_now().await {
                            if boundary_refresh_retry_is_due(
                                next_boundary_refresh_retry_at,
                                Instant::now(),
                            ) {
                                let _ = attempt_flow_boundary_refresh(
                                    repo,
                                    run_id,
                                    &ws,
                                    &mut flow_runtime_caches.user_cfg,
                                    "ws_stale",
                                    &mut next_boundary_refresh_retry_at,
                                )
                                .await;
                            }
                            continue;
                        }
                        let dirty_token_ids = ws.take_dirty_market_tokens().await;
                        ws.clear_dirty_market_tokens(&dirty_token_ids).await;
                        if let Err(e) = process_trade_flow_ws_fast_path(
                            repo,
                            run_id,
                            Some(&client),
                            &ws,
                            &mut flow_runtime_caches,
                            &dirty_token_ids,
                        ).await {
                            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED_FAST_WAKE");
                        }
                        if let Err(e) = evaluate_armed_builder_orders_for_dirty_tokens(
                            repo,
                            run_id,
                            &ws,
                            &dirty_token_ids,
                        )
                        .await
                        {
                            warn!(run_id, error = %e, "ARMED_ORDER_WS_EVAL_FAILED");
                        }
                        if let Err(e) = evaluate_guard_blocked_buy_orders_for_dirty_tokens(
                            repo,
                            run_id,
                            &ws,
                            &dirty_token_ids,
                        )
                        .await
                        {
                            warn!(run_id, error = %e, "GUARD_BLOCKED_BUY_WS_EVAL_FAILED");
                        }
                    }
                    _ = wait_for_trade_builder_ptb_stop_loss_dirty_update() => {
                        if let Err(e) = process_trade_builder_ptb_fast_path_dirty_context(
                            repo,
                            run_id,
                            &ws,
                        )
                        .await
                        {
                            warn!(run_id, error = %e, "ARMED_ORDER_PTB_DIRTY_EVAL_FAILED");
                        }
                    }
                    _ = FLOW_PROCESS_NOTIFY.notified() => {
                        if let Err(e) = process_trade_flow_ready_steps(
                            repo,
                            run_id,
                            Some(&client),
                            &ws,
                            &mut flow_runtime_caches,
                        ).await {
                            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED_NOTIFY_WAKE");
                        }
                    }
                }
            } else {
                tokio::select! {
                    Some(trigger) = tick_trigger_rx.recv() => {
                        handle_tick_trigger(repo, run_id, &ws, trigger).await;
                    }
                    _ = tokio::time::sleep(remaining) => {}
                    _ = tokio::time::sleep(delay) => {
                        if let Err(e) = process_trade_flow_trigger_market_price_timers(
                            repo,
                            run_id,
                            &ws,
                            Some(&client),
                        ).await {
                            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED_TIMER_WAKE");
                        }
                        if let Err(e) = process_trade_flow_ready_steps(
                            repo,
                            run_id,
                            Some(&client),
                            &ws,
                            &mut flow_runtime_caches,
                        ).await {
                            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED_TIMER_READY");
                        }
                    }
                    _ = ws.wait_for_market_update() => {
                        tokio::time::sleep(fast_path_debounce).await;
                        if trade_flow_ws_fast_path_cache_requires_refresh_now().await {
                            if boundary_refresh_retry_is_due(
                                next_boundary_refresh_retry_at,
                                Instant::now(),
                            ) {
                                let _ = attempt_flow_boundary_refresh(
                                    repo,
                                    run_id,
                                    &ws,
                                    &mut flow_runtime_caches.user_cfg,
                                    "ws_stale",
                                    &mut next_boundary_refresh_retry_at,
                                )
                                .await;
                            }
                            continue;
                        }
                        let dirty_token_ids = ws.take_dirty_market_tokens().await;
                        ws.clear_dirty_market_tokens(&dirty_token_ids).await;
                        if let Err(e) = process_trade_flow_ws_fast_path(
                            repo,
                            run_id,
                            Some(&client),
                            &ws,
                            &mut flow_runtime_caches,
                            &dirty_token_ids,
                        ).await {
                            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED_FAST_WAKE");
                        }
                        if let Err(e) = evaluate_armed_builder_orders_for_dirty_tokens(
                            repo,
                            run_id,
                            &ws,
                            &dirty_token_ids,
                        )
                        .await
                        {
                            warn!(run_id, error = %e, "ARMED_ORDER_WS_EVAL_FAILED");
                        }
                        if let Err(e) = evaluate_guard_blocked_buy_orders_for_dirty_tokens(
                            repo,
                            run_id,
                            &ws,
                            &dirty_token_ids,
                        )
                        .await
                        {
                            warn!(run_id, error = %e, "GUARD_BLOCKED_BUY_WS_EVAL_FAILED");
                        }
                    }
                    _ = wait_for_trade_builder_ptb_stop_loss_dirty_update() => {
                        if let Err(e) = process_trade_builder_ptb_fast_path_dirty_context(
                            repo,
                            run_id,
                            &ws,
                        )
                        .await
                        {
                            warn!(run_id, error = %e, "ARMED_ORDER_PTB_DIRTY_EVAL_FAILED");
                        }
                    }
                    _ = FLOW_PROCESS_NOTIFY.notified() => {
                        if let Err(e) = process_trade_flow_ready_steps(
                            repo,
                            run_id,
                            Some(&client),
                            &ws,
                            &mut flow_runtime_caches,
                        ).await {
                            warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED_NOTIFY_WAKE");
                        }
                    }
                }
            }
        } else if let Some(boundary_delay) = boundary_refresh_delay {
            tokio::select! {
                Some(trigger) = tick_trigger_rx.recv() => {
                    handle_tick_trigger(repo, run_id, &ws, trigger).await;
                }
                _ = tokio::time::sleep(remaining) => {}
                _ = tokio::time::sleep(boundary_delay) => {
                    let _ = attempt_flow_boundary_refresh(
                        repo,
                        run_id,
                        &ws,
                        &mut flow_runtime_caches.user_cfg,
                        "sleep_wake",
                        &mut next_boundary_refresh_retry_at,
                    )
                    .await;
                }
                _ = ws.wait_for_market_update() => {
                    tokio::time::sleep(fast_path_debounce).await;
                    if trade_flow_ws_fast_path_cache_requires_refresh_now().await {
                        if boundary_refresh_retry_is_due(
                            next_boundary_refresh_retry_at,
                            Instant::now(),
                        ) {
                            let _ = attempt_flow_boundary_refresh(
                                repo,
                                run_id,
                                &ws,
                                &mut flow_runtime_caches.user_cfg,
                                "ws_stale",
                                &mut next_boundary_refresh_retry_at,
                            )
                            .await;
                        }
                        continue;
                    }
                    let dirty_token_ids = ws.take_dirty_market_tokens().await;
                    ws.clear_dirty_market_tokens(&dirty_token_ids).await;
                    if let Err(e) = process_trade_flow_ws_fast_path(
                        repo,
                        run_id,
                        Some(&client),
                        &ws,
                        &mut flow_runtime_caches,
                        &dirty_token_ids,
                    ).await {
                        warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED_FAST_WAKE");
                    }
                    if let Err(e) = evaluate_armed_builder_orders_for_dirty_tokens(
                        repo,
                        run_id,
                        &ws,
                        &dirty_token_ids,
                    )
                    .await
                    {
                        warn!(run_id, error = %e, "ARMED_ORDER_WS_EVAL_FAILED");
                    }
                    if let Err(e) = evaluate_guard_blocked_buy_orders_for_dirty_tokens(
                        repo,
                        run_id,
                        &ws,
                        &dirty_token_ids,
                    )
                    .await
                    {
                        warn!(run_id, error = %e, "GUARD_BLOCKED_BUY_WS_EVAL_FAILED");
                    }
                }
                _ = wait_for_trade_builder_ptb_stop_loss_dirty_update() => {
                    if let Err(e) = process_trade_builder_ptb_fast_path_dirty_context(
                        repo,
                        run_id,
                        &ws,
                    )
                    .await
                    {
                        warn!(run_id, error = %e, "ARMED_ORDER_PTB_DIRTY_EVAL_FAILED");
                    }
                }
                _ = FLOW_PROCESS_NOTIFY.notified() => {
                    if let Err(e) = process_trade_flow_ready_steps(
                        repo,
                        run_id,
                        Some(&client),
                        &ws,
                        &mut flow_runtime_caches,
                    ).await {
                        warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED_NOTIFY_WAKE");
                    }
                }
            }
        } else {
            tokio::select! {
                Some(trigger) = tick_trigger_rx.recv() => {
                    handle_tick_trigger(repo, run_id, &ws, trigger).await;
                }
                _ = tokio::time::sleep(remaining) => {}
                _ = ws.wait_for_market_update() => {
                    tokio::time::sleep(fast_path_debounce).await;
                    if trade_flow_ws_fast_path_cache_requires_refresh_now().await {
                        if boundary_refresh_retry_is_due(
                            next_boundary_refresh_retry_at,
                            Instant::now(),
                        ) {
                            let _ = attempt_flow_boundary_refresh(
                                repo,
                                run_id,
                                &ws,
                                &mut flow_runtime_caches.user_cfg,
                                "ws_stale",
                                &mut next_boundary_refresh_retry_at,
                            )
                            .await;
                        }
                        continue;
                    }
                    let dirty_token_ids = ws.take_dirty_market_tokens().await;
                    ws.clear_dirty_market_tokens(&dirty_token_ids).await;
                    if let Err(e) = process_trade_flow_ws_fast_path(
                        repo,
                        run_id,
                        Some(&client),
                        &ws,
                        &mut flow_runtime_caches,
                        &dirty_token_ids,
                    ).await {
                        warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED_FAST_WAKE");
                    }
                    if let Err(e) = evaluate_armed_builder_orders_for_dirty_tokens(
                        repo,
                        run_id,
                        &ws,
                        &dirty_token_ids,
                    )
                    .await
                    {
                        warn!(run_id, error = %e, "ARMED_ORDER_WS_EVAL_FAILED");
                    }
                    if let Err(e) = evaluate_guard_blocked_buy_orders_for_dirty_tokens(
                        repo,
                        run_id,
                        &ws,
                        &dirty_token_ids,
                    )
                    .await
                    {
                        warn!(run_id, error = %e, "GUARD_BLOCKED_BUY_WS_EVAL_FAILED");
                    }
                }
                _ = wait_for_trade_builder_ptb_stop_loss_dirty_update() => {
                    if let Err(e) = process_trade_builder_ptb_fast_path_dirty_context(
                        repo,
                        run_id,
                        &ws,
                    )
                    .await
                    {
                        warn!(run_id, error = %e, "ARMED_ORDER_PTB_DIRTY_EVAL_FAILED");
                    }
                }
                _ = FLOW_PROCESS_NOTIFY.notified() => {
                    if let Err(e) = process_trade_flow_ready_steps(
                        repo,
                        run_id,
                        Some(&client),
                        &ws,
                        &mut flow_runtime_caches,
                    ).await {
                        warn!(run_id, error = %e, "TRADE_FLOW_PROCESS_FAILED_NOTIFY_WAKE");
                    }
                }
            }
        }
    }
}

async fn wait_for_trade_builder_ptb_stop_loss_dirty_update() {
    tokio::select! {
        _ = crate::trade_flow::guards::chainlink_price::wait_for_chainlink_dirty_asset_update() => {}
        _ = crate::trade_flow::guards::polymarket_price_to_beat::wait_for_price_to_beat_dirty_market_update() => {}
    }
}

async fn process_trade_builder_ptb_fast_path_dirty_context(
    repo: &PostgresRepository,
    run_id: i64,
    ws: &ClobWsClient,
) -> Result<()> {
    let dirty_assets = crate::trade_flow::guards::chainlink_price::take_chainlink_dirty_assets();
    crate::trade_flow::guards::chainlink_price::clear_chainlink_dirty_assets(&dirty_assets);
    let dirty_market_slugs =
        crate::trade_flow::guards::polymarket_price_to_beat::take_price_to_beat_dirty_market_slugs();
    crate::trade_flow::guards::polymarket_price_to_beat::clear_price_to_beat_dirty_market_slugs(
        &dirty_market_slugs,
    );
    evaluate_armed_builder_ptb_orders_for_dirty_context(
        repo,
        run_id,
        ws,
        &dirty_assets,
        &dirty_market_slugs,
    )
    .await
}

async fn handle_tick_trigger(
    repo: &PostgresRepository,
    run_id: i64,
    ws: &ClobWsClient,
    trigger: TickTrigger,
) {
    let Some(order) =
        take_armed_builder_order_from_cache(&trigger.token_id, trigger.order_id).await
    else {
        return;
    };

    info!(
        run_id,
        builder_order_id = order.id,
        user_id = order.user_id,
        token_id = order.token_id,
        trigger_kind = trigger.trigger_kind,
        trigger_price = trigger.trigger_price,
        tick_price = trigger.tick_price,
        "TICK_TRIGGER_FIRED"
    );

    spawn_armed_order_immediate_processing(repo, run_id, ws, order.id, order.user_id, None);
}
