async fn run_live_dual_loop(run_id: i64, repo: &PostgresRepository, cfg: &AppConfig) -> Result<()> {
    let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
    let live_enabled = env::var("LIVE_TRADING_ENABLED").ok().as_deref() == Some("true");
    anyhow::ensure!(
        live_enabled,
        "dual_side strategy requires LIVE_TRADING_ENABLED=true"
    );

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
    let (snapshot_tx, snapshot_rx) =
        tokio::sync::mpsc::unbounded_channel::<MarketSecondSnapshotTick>();
    let (volume_tx, volume_rx) =
        tokio::sync::mpsc::unbounded_channel::<MarketTradeVolumeTick>();
    tokio::spawn(run_market_second_snapshot_recorder(
        repo.clone(),
        client.clone(),
        snapshot_rx,
    ));
    tokio::spawn(run_market_trade_volume_recorder(repo.clone(), volume_rx));
    ws.set_tick_callback(build_market_second_snapshot_callback(snapshot_tx))
        .await;
    ws.set_trade_callback(build_market_trade_volume_callback(volume_tx))
        .await;
    let mut auto_claim_runtimes: HashMap<i64, FlowAutoClaimRuntime> = HashMap::new();
    let mut flow_runtime_caches = FlowRuntimeCaches::default();
    let override_slug = configured_market_override_slug(cfg)?;

    let mut waiting_event_emitted = false;
    let selected = loop {
        if let Err(e) = process_trade_builder_orders(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_BUILDER_PROCESS_FAILED");
        }
        if let Err(e) = process_trade_builder_workflows(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_BUILDER_WORKFLOW_PROCESS_FAILED");
        }
        if let Err(e) = dca::process_trade_flow_dual_dca_jobs(repo, run_id, cfg, &client, &ws).await
        {
            warn!(run_id, error = %e, "TRADE_FLOW_DUAL_DCA_PROCESS_FAILED");
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

        match discover_live_market_once(cfg, &gamma, true, override_slug.as_deref()).await {
            Ok(Some(selected)) => {
                info!(
                    run_id,
                    scope = %cfg.bot.market_scope,
                    selection = %cfg.bot.market_selection,
                    override_slug = ?override_slug,
                    market = %selected.slug,
                    selection_reason = selected.selection_reason.as_str(),
                    market_start_at = ?selected.starts_at,
                    market_end_at = ?selected.ends_at,
                    now_utc = %Utc::now(),
                    "MARKET_DISCOVERY_FOUND_NON_BLOCKING"
                );
                record_market_discovery_event(
                    repo,
                    run_id,
                    "allow",
                    MarketDiscoveryState::Ready,
                    &cfg.bot.market_scope,
                    Some(&selected.slug),
                    "market_discovery_ready",
                    "Market selected successfully.",
                )
                .await;
                break selected;
            }
            Ok(None) => {
                if !waiting_event_emitted {
                    waiting_event_emitted = true;
                    let waiting_message = if let Some(forced_slug) = override_slug.as_ref() {
                        format!(
                            "Override market not active or missing YES/NO token IDs: {forced_slug}. Retrying while trade-flow continues."
                        )
                    } else {
                        "No active market with YES/NO token IDs. Retrying while trade-flow continues."
                            .to_string()
                    };
                    info!(
                        run_id,
                        scope = %cfg.bot.market_scope,
                        selection = %cfg.bot.market_selection,
                        override_slug = ?override_slug,
                        "MARKET_DISCOVERY_WAITING_NON_BLOCKING"
                    );
                    record_market_discovery_event(
                        repo,
                        run_id,
                        "block",
                        MarketDiscoveryState::WaitingForMarket,
                        &cfg.bot.market_scope,
                        None,
                        "market_missing_token_ids",
                        &waiting_message,
                    )
                    .await;
                }
            }
            Err(err) => {
                warn!(
                    run_id,
                    scope = %cfg.bot.market_scope,
                    error = %err,
                    "MARKET_DISCOVERY_FETCH_FAILED_NON_BLOCKING"
                );
                if !waiting_event_emitted {
                    waiting_event_emitted = true;
                    record_market_discovery_event(
                        repo,
                        run_id,
                        "block",
                        MarketDiscoveryState::WaitingForMarket,
                        &cfg.bot.market_scope,
                        None,
                        "market_discovery_fetch_failed",
                        "Failed to fetch market list. Retrying while trade-flow continues.",
                    )
                    .await;
                }
            }
        }

        sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
    };

    let selected_reason = selected.selection_reason.as_str();
    let selected_start_at = selected.starts_at.as_ref().cloned();
    let selected_end_at = selected.ends_at.as_ref().cloned();
    let market_slug = selected.slug.clone();
    let yes_token_id = selected
        .yes_token_id
        .clone()
        .context("missing YES token id for selected market")?;
    let no_token_id = selected
        .no_token_id
        .clone()
        .context("missing NO token id for selected market")?;

    let cycle = MarketCycleId(market_slug.clone());
    let cycle_ends_at = selected_end_at.as_ref().cloned().unwrap_or_else(|| {
        cycle.start_time().unwrap_or_else(Utc::now) + ChronoDuration::seconds(300)
    });
    let market_id = repo.upsert_market_cycle(&cycle).await?;
    info!(
        run_id,
        market_id,
        market = %market_slug,
        selection_reason = selected_reason,
        market_start_at = ?selected_start_at,
        market_end_at = ?selected_end_at,
        yes_token_id,
        no_token_id,
        "LIVE_DUAL_MARKET_DISCOVERED"
    );

    let mut user_ws_events: Vec<WsEvent> = Vec::new();
    let market_ws_ids = vec![yes_token_id.clone(), no_token_id.clone()];
    match ws.subscribe_once(WsChannel::Market, &market_ws_ids).await {
        Ok(messages) => {
            info!(run_id, market = %market_slug, ws_messages = messages.len(), "WS_MARKET_CONNECT_OK")
        }
        Err(e) => warn!(run_id, market = %market_slug, error = %e, "WS_MARKET_CONNECT_FAILED"),
    }
    match ws
        .subscribe_once(WsChannel::User, &[market_slug.clone()])
        .await
    {
        Ok(messages) => {
            info!(run_id, market = %market_slug, ws_messages = messages.len(), "WS_USER_CONNECT_OK");
            user_ws_events = messages;
        }
        Err(e) => warn!(run_id, market = %market_slug, error = %e, "WS_USER_CONNECT_FAILED"),
    }

    let mut basket = create_dual_runtime(
        repo,
        cfg,
        market_id,
        market_slug.clone(),
        yes_token_id,
        no_token_id,
        selected.maker_base_fee,
        cycle_ends_at,
    )
    .await?;
    transition_dual(
        repo,
        &mut basket,
        TradeState::WaitingEntry,
        "live-dual-cycle-start",
    )
    .await?;

    let strategy = SymmetricDualDcaStrategy;
    let policy = DefaultRiskPolicy;
    let limits = to_risk_limits(cfg);
    let mut order_meta: HashMap<String, OrderMeta> = HashMap::new();
    let mut seen_fill_ids: HashSet<String> = HashSet::new();
    let mut seen_buy_fill_order_ids: HashSet<String> = HashSet::new();
    let level_notional =
        cfg.strategy.per_leg_initial_notional_usdc / cfg.strategy.max_dca_levels_per_leg as f64;

    let initial_yes = match client.midpoint(&basket.yes_leg.token_id).await {
        Ok(snapshot) => clamp_probability(snapshot.price),
        Err(err) => {
            let fallback = 0.5;
            warn!(
                run_id,
                trade_id = basket.trade_id,
                market = %basket.market_slug,
                error = %err,
                fallback_yes = fallback,
                "LIVE_DUAL_MIDPOINT_FAILED_USING_FALLBACK"
            );
            fallback
        }
    };
    let initial_no = clamp_probability(1.0 - initial_yes);

    transition_dual(
        repo,
        &mut basket,
        TradeState::EntryPlaced,
        "live-dual-initial-entry",
    )
    .await?;
    let yes_size = calc_level_size(level_notional, initial_yes);
    let no_size = calc_level_size(level_notional, initial_no);
    let mut entry_submit_failed = false;
    if let Err(err) = place_live_leg_order(
        repo,
        basket.trade_id,
        &basket.market_slug,
        &mut basket.yes_leg,
        "buy",
        "entry",
        initial_yes,
        yes_size,
        basket.maker_base_fee,
        &client,
        &mut order_meta,
    )
    .await
    {
        entry_submit_failed = true;
        let classification = classify_clob_error(&err);
        warn!(
            run_id,
            trade_id = basket.trade_id,
            market = %basket.market_slug,
            leg = "yes",
            reason_code = classification.reason_code,
            reason_message = classification.reason_message,
            error = %err,
            "LIVE_DUAL_ENTRY_ORDER_FAILED"
        );
    }
    if let Err(err) = place_live_leg_order(
        repo,
        basket.trade_id,
        &basket.market_slug,
        &mut basket.no_leg,
        "buy",
        "entry",
        initial_no,
        no_size,
        basket.maker_base_fee,
        &client,
        &mut order_meta,
    )
    .await
    {
        entry_submit_failed = true;
        let classification = classify_clob_error(&err);
        warn!(
            run_id,
            trade_id = basket.trade_id,
            market = %basket.market_slug,
            leg = "no",
            reason_code = classification.reason_code,
            reason_message = classification.reason_message,
            error = %err,
            "LIVE_DUAL_ENTRY_ORDER_FAILED"
        );
    }
    if entry_submit_failed {
        warn!(
            run_id,
            trade_id = basket.trade_id,
            market = %basket.market_slug,
            "LIVE_DUAL_ENTRY_SUBMIT_FAILED_CONTINUING"
        );
        // If no orders were placed at all (both legs rejected), settle the empty trade
        // and wait until the market window closes before returning.
        // Without this, the scope task immediately starts a new run_live_dual_loop()
        // creating hundreds of empty trades per market window.
        if order_meta.is_empty() {
            persist_leg_snapshots(repo, &basket).await?;
            if can_transition(basket.state, TradeState::ExitFilled).is_ok() {
                transition_dual(
                    repo,
                    &mut basket,
                    TradeState::ExitFilled,
                    "entry-failed-no-fill",
                )
                .await?;
            }
            if can_transition(basket.state, TradeState::Settled).is_ok() {
                transition_dual(
                    repo,
                    &mut basket,
                    TradeState::Settled,
                    "entry-failed-settle",
                )
                .await?;
            }
            repo.close_trade(basket.trade_id, 0.5, 0.0).await?;
            let remaining = basket.cycle_ends_at.signed_duration_since(Utc::now());
            if remaining > ChronoDuration::zero() {
                let wait_secs = remaining.num_seconds().min(300) as u64;
                info!(
                    run_id,
                    trade_id = basket.trade_id,
                    market = %basket.market_slug,
                    wait_secs,
                    "LIVE_DUAL_ENTRY_FAILED_WAITING_WINDOW_END"
                );
                sleep(Duration::from_secs(wait_secs)).await;
            }
            return Ok(());
        }
    } else {
        transition_dual(
            repo,
            &mut basket,
            TradeState::EntryFilled,
            "live-dual-entry-submitted",
        )
        .await?;
        transition_dual(
            repo,
            &mut basket,
            TradeState::TpPlaced,
            "live-dual-tp-active",
        )
        .await?;
    }
    let mut last_yes_price: Option<f64> = None;
    let mut flow_runtime_caches = FlowRuntimeCaches::default();

    for iter in 0..120u32 {
        if let Err(e) = process_trade_builder_orders(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_BUILDER_PROCESS_FAILED");
        }
        if let Err(e) = process_trade_builder_workflows(repo, run_id, cfg, &client, &ws).await {
            warn!(run_id, error = %e, "TRADE_BUILDER_WORKFLOW_PROCESS_FAILED");
        }
        if let Err(e) = dca::process_trade_flow_dual_dca_jobs(repo, run_id, cfg, &client, &ws).await
        {
            warn!(run_id, error = %e, "TRADE_FLOW_DUAL_DCA_PROCESS_FAILED");
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

        let yes_price = match client.midpoint(&basket.yes_leg.token_id).await {
            Ok(snapshot) => clamp_probability(snapshot.price),
            Err(err) => {
                let fallback_yes = clamp_probability(last_yes_price.unwrap_or(0.5));
                warn!(
                    run_id,
                    trade_id = basket.trade_id,
                    market = %basket.market_slug,
                    error = %err,
                    fallback_yes,
                    "LIVE_DUAL_MIDPOINT_FAILED_USING_PREVIOUS_PRICE"
                );
                fallback_yes
            }
        };
        let no_price = clamp_probability(1.0 - yes_price);

        if let Err(err) = process_live_dual_fills(
            repo,
            basket.trade_id,
            &client,
            &mut basket,
            &order_meta,
            &mut seen_fill_ids,
            &mut seen_buy_fill_order_ids,
        )
        .await
        {
            let classification = classify_clob_error(&err);
            warn!(
                run_id,
                trade_id = basket.trade_id,
                market = %basket.market_slug,
                reason_code = classification.reason_code,
                reason_message = classification.reason_message,
                error = %err,
                "LIVE_DUAL_FILL_RECONCILE_FAILED_CONTINUING"
            );
        }

        let user_applied = apply_user_stream_events(repo, basket.trade_id, &user_ws_events).await?;
        if user_applied > 0 {
            info!(
                run_id,
                trade_id = basket.trade_id,
                user_applied,
                "LIVE_DUAL_USER_EVENTS_APPLIED"
            );
        }

        let risk = risk_gate_dual(
            repo,
            run_id,
            cfg,
            None,
            basket.trade_id,
            &limits,
            0,
            &policy,
        )
        .await?;
        match risk {
            RiskDecision::Halt => {
                let mut trade = basket_to_trade_runtime(&basket);
                halt_trade(repo, run_id, &mut trade, "risk-halt", Some(&client)).await?;
                basket.state = trade.state;
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
        let (pressure_score, bid_ask_imbalance, sell_ratio, pressure_triggered) =
            compute_pressure_score(last_yes_price, yes_price);
        let drop_sell_reason =
            detect_drop_sell_reason(repo, basket.trade_id, &basket, yes_price, no_price).await?;
        let pressure_reason = if pressure_triggered {
            Some("pressure_exit")
        } else {
            None
        };
        let custom_reason = drop_sell_reason.or(pressure_reason);

        repo.upsert_pressure_snapshot(
            basket.trade_id,
            pressure_score,
            Some(bid_ask_imbalance),
            Some(sell_ratio),
            Some(yes_price),
            Some(no_price),
            custom_reason,
            custom_reason.is_some(),
        )
        .await?;
        last_yes_price = Some(yes_price);

        let basket_flatten = strategy.should_flatten_basket(
            basket_pnl,
            cfg.strategy.basket_tp_usdc,
            cfg.strategy.basket_sl_usdc,
        );

        if force_flatten || basket_flatten || custom_reason.is_some() {
            let reason = if let Some(reason) = custom_reason {
                reason
            } else if force_flatten {
                "force_flatten"
            } else if basket_pnl <= cfg.strategy.basket_sl_usdc {
                "basket_sl"
            } else {
                "basket_tp"
            };
            let flatten_key = format!("trade:{}:flatten:{reason}", basket.trade_id);
            if repo.try_record_idempotency_key(&flatten_key).await? {
                if reason == "basket_sl" && basket.state == TradeState::TpPlaced {
                    transition_dual(repo, &mut basket, TradeState::SlArmed, "basket-sl").await?;
                }
                if basket.yes_leg.qty > 0.0 {
                    let qty = basket.yes_leg.qty;
                    if let Err(err) = place_live_leg_order(
                        repo,
                        basket.trade_id,
                        &basket.market_slug,
                        &mut basket.yes_leg,
                        "sell",
                        "basket_exit",
                        yes_price,
                        qty,
                        basket.maker_base_fee,
                        &client,
                        &mut order_meta,
                    )
                    .await
                    {
                        let classification = classify_clob_error(&err);
                        warn!(
                            run_id,
                            trade_id = basket.trade_id,
                            market = %basket.market_slug,
                            leg = "yes",
                            reason_code = classification.reason_code,
                            reason_message = classification.reason_message,
                            error = %err,
                            "LIVE_DUAL_EXIT_ORDER_FAILED_CONTINUING"
                        );
                    }
                }
                if basket.no_leg.qty > 0.0 {
                    let qty = basket.no_leg.qty;
                    if let Err(err) = place_live_leg_order(
                        repo,
                        basket.trade_id,
                        &basket.market_slug,
                        &mut basket.no_leg,
                        "sell",
                        "basket_exit",
                        no_price,
                        qty,
                        basket.maker_base_fee,
                        &client,
                        &mut order_meta,
                    )
                    .await
                    {
                        let classification = classify_clob_error(&err);
                        warn!(
                            run_id,
                            trade_id = basket.trade_id,
                            market = %basket.market_slug,
                            leg = "no",
                            reason_code = classification.reason_code,
                            reason_message = classification.reason_message,
                            error = %err,
                            "LIVE_DUAL_EXIT_ORDER_FAILED_CONTINUING"
                        );
                    }
                }
            }
        } else {
            if let Err(err) = maybe_live_leg_tp(
                repo,
                cfg,
                &strategy,
                &mut basket,
                &mut order_meta,
                &client,
                yes_price,
                no_price,
            )
            .await
            {
                let classification = classify_clob_error(&err);
                warn!(
                    run_id,
                    trade_id = basket.trade_id,
                    market = %basket.market_slug,
                    reason_code = classification.reason_code,
                    reason_message = classification.reason_message,
                    error = %err,
                    "LIVE_DUAL_TP_CHECK_FAILED_CONTINUING"
                );
            }

            if let Err(err) = maybe_live_leg_dca(
                repo,
                cfg,
                &strategy,
                basket.trade_id,
                &basket.market_slug,
                &mut basket.yes_leg,
                yes_price,
                level_notional,
                basket.maker_base_fee,
                &client,
                &mut order_meta,
                now,
            )
            .await
            {
                let classification = classify_clob_error(&err);
                warn!(
                    run_id,
                    trade_id = basket.trade_id,
                    market = %basket.market_slug,
                    leg = "yes",
                    reason_code = classification.reason_code,
                    reason_message = classification.reason_message,
                    error = %err,
                    "LIVE_DUAL_DCA_CHECK_FAILED_CONTINUING"
                );
            }
            if let Err(err) = maybe_live_leg_dca(
                repo,
                cfg,
                &strategy,
                basket.trade_id,
                &basket.market_slug,
                &mut basket.no_leg,
                no_price,
                level_notional,
                basket.maker_base_fee,
                &client,
                &mut order_meta,
                now,
            )
            .await
            {
                let classification = classify_clob_error(&err);
                warn!(
                    run_id,
                    trade_id = basket.trade_id,
                    market = %basket.market_slug,
                    leg = "no",
                    reason_code = classification.reason_code,
                    reason_message = classification.reason_message,
                    error = %err,
                    "LIVE_DUAL_DCA_CHECK_FAILED_CONTINUING"
                );
            }
        }

        if let Err(err) = process_live_dual_fills(
            repo,
            basket.trade_id,
            &client,
            &mut basket,
            &order_meta,
            &mut seen_fill_ids,
            &mut seen_buy_fill_order_ids,
        )
        .await
        {
            let classification = classify_clob_error(&err);
            warn!(
                run_id,
                trade_id = basket.trade_id,
                market = %basket.market_slug,
                reason_code = classification.reason_code,
                reason_message = classification.reason_message,
                error = %err,
                "LIVE_DUAL_FILL_RECONCILE_FINAL_FAILED_CONTINUING"
            );
        }
        persist_leg_snapshots(repo, &basket).await?;

        if basket.yes_leg.qty <= 0.0 && basket.no_leg.qty <= 0.0 {
            if can_transition(basket.state, TradeState::ExitFilled).is_ok() {
                transition_dual(
                    repo,
                    &mut basket,
                    TradeState::ExitFilled,
                    "live-dual-exit-filled",
                )
                .await?;
            }
            if can_transition(basket.state, TradeState::Settled).is_ok() {
                transition_dual(repo, &mut basket, TradeState::Settled, "live-dual-settled")
                    .await?;
            }
            let pnl = basket_unrealized_pnl(&basket, yes_price, no_price);
            repo.close_trade(basket.trade_id, 0.5, pnl).await?;
            info!(
                run_id,
                trade_id = basket.trade_id,
                pnl,
                iter,
                "LIVE_DUAL_SETTLED"
            );
            break;
        }

        sleep(Duration::from_millis(cfg.bot.loop_interval_ms)).await;
    }

    Ok(())
}
