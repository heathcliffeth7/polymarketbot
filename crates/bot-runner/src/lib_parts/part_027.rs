pub(crate) fn normalize_exchange_status(status: &str) -> &'static str {
    let normalized = status.to_lowercase();
    if normalized.contains("partial") {
        return "partially_filled";
    }
    if normalized.contains("match") {
        return "filled";
    }
    if normalized.contains("fill") {
        return "filled";
    }
    if normalized.contains("cancel") {
        return "canceled";
    }
    if normalized.contains("reject") {
        return "rejected";
    }
    if normalized.contains("expir") {
        return "expired";
    }
    if normalized.contains("open") || normalized.contains("live") || normalized.contains("book") {
        return "open";
    }
    "open"
}

pub(crate) fn min_price_distance_to_probability(min_price_distance_cent: f64) -> f64 {
    (min_price_distance_cent / 100.0).max(0.0001)
}

pub(crate) fn normalize_trade_builder_execution_mode(raw: &str) -> &'static str {
    if raw.trim().eq_ignore_ascii_case("market") {
        return "market";
    }
    "market"
}

pub(crate) fn clob_order_type_for_execution_mode(mode: &str) -> &'static str {
    if mode.eq_ignore_ascii_case("market") {
        return "IOC";
    }
    "GTC"
}

pub(crate) fn aggressive_price_for_side(
    side: &str,
    current_price: f64,
    min_price_distance_cent: f64,
) -> f64 {
    let distance = min_price_distance_to_probability(min_price_distance_cent);
    if side == "sell" {
        return clamp_probability(current_price - distance);
    }
    clamp_probability(current_price + distance)
}

pub(crate) async fn risk_gate_manual_order(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    user_id: Option<i64>,
    trade_id: i64,
    proposed_notional_usdc: f64,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
) -> Result<RiskDecision> {
    let (open_orders, daily_pnl, consec_losses) = if let Some(user_id) = user_id {
        (
            repo.open_order_count_for_user(user_id).await?,
            repo.daily_realized_pnl_for_user(user_id).await?,
            repo.consecutive_losses_for_user(user_id, cfg.risk.max_consecutive_losses as i64)
                .await?,
        )
    } else {
        (
            repo.open_order_count().await?,
            repo.daily_realized_pnl().await?,
            repo.consecutive_losses(cfg.risk.max_consecutive_losses as i64)
                .await?,
        )
    };

    let risk = policy.evaluate(
        limits,
        &RiskInput {
            proposed_notional_usdc,
            open_orders,
            stale_data_ms: 0,
            daily_realized_pnl_usdc: daily_pnl,
            consecutive_losses: consec_losses,
            manual_kill_switch_active: cfg.risk.manual_kill_switch_active,
        },
    );

    let decision_text = format!("{:?}", risk.decision).to_lowercase();
    if matches!(risk.decision, RiskDecision::Allow) {
        let repo = repo.clone();
        let reason = risk.reason.to_string();
        tokio::spawn(async move {
            if let Err(err) = repo
                .record_risk_event(
                    Some(trade_id),
                    "risk_check_manual_order",
                    &decision_text,
                    &reason,
                )
                .await
            {
                warn!(
                    trade_id,
                    error = %err,
                    "RISK_MANUAL_ORDER_ALLOW_EVENT_DEFERRED_WRITE_FAILED"
                );
            }
        });
    } else {
        StateRepository::record_risk_event(
            repo,
            Some(trade_id),
            "risk_check_manual_order",
            &decision_text,
            risk.reason,
        )
        .await?;
        warn!(
            run_id,
            trade_id,
            reason = risk.reason,
            decision = ?risk.decision,
            "RISK_MANUAL_ORDER_BLOCKED"
        );
    }

    Ok(risk.decision)
}

async fn risk_gate_dual(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    user_id: Option<i64>,
    trade_id: i64,
    limits: &RiskLimits,
    stale_data_ms: u64,
    policy: &impl RiskPolicy,
) -> Result<RiskDecision> {
    let (open_orders, daily_pnl, consec_losses) = if let Some(user_id) = user_id {
        (
            repo.open_order_count_for_user(user_id).await?,
            repo.daily_realized_pnl_for_user(user_id).await?,
            repo.consecutive_losses_for_user(user_id, cfg.risk.max_consecutive_losses as i64)
                .await?,
        )
    } else {
        (
            repo.open_order_count().await?,
            repo.daily_realized_pnl().await?,
            repo.consecutive_losses(cfg.risk.max_consecutive_losses as i64)
                .await?,
        )
    };

    let risk = policy.evaluate(
        limits,
        &RiskInput {
            proposed_notional_usdc: cfg.strategy.total_notional_usdc,
            open_orders,
            stale_data_ms,
            daily_realized_pnl_usdc: daily_pnl,
            consecutive_losses: consec_losses,
            manual_kill_switch_active: cfg.risk.manual_kill_switch_active,
        },
    );

    StateRepository::record_risk_event(
        repo,
        Some(trade_id),
        "risk_check_dual",
        &format!("{:?}", risk.decision).to_lowercase(),
        risk.reason,
    )
    .await?;

    match risk.decision {
        RiskDecision::Halt => {
            warn!(run_id, trade_id, reason = risk.reason, "RISK_HALT_DUAL");
            Ok(RiskDecision::Halt)
        }
        RiskDecision::Block => {
            warn!(run_id, trade_id, reason = risk.reason, "RISK_BLOCK_DUAL");
            Ok(RiskDecision::Block)
        }
        RiskDecision::Allow => Ok(RiskDecision::Allow),
    }
}

async fn process_trade_step(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    trade: &mut TradeRuntime,
    current_price: f64,
    auto_fill: bool,
    strategy: &impl Strategy,
) -> Result<()> {
    if trade.state == TradeState::WaitingEntry
        && strategy.entry_signal(current_price, trade.entry_price)
    {
        transition(repo, trade, TradeState::EntryPlaced, "entry-threshold-hit").await?;
        let entry_status = if auto_fill { "filled" } else { "open" };
        let entry_order_id = repo
            .append_order_event(
                trade.trade_id,
                "entry",
                "buy",
                trade.entry_price,
                trade.position_size,
                entry_status,
            )
            .await?;

        if auto_fill {
            repo.append_fill_event(entry_order_id, trade.entry_price, trade.position_size, 0.0)
                .await?;
            transition(repo, trade, TradeState::EntryFilled, "entry-filled").await?;
            transition(repo, trade, TradeState::TpPlaced, "tp-placed").await?;
            info!(
                run_id,
                trade_id = trade.trade_id,
                tp_price = trade.tp_price,
                "ENTRY_FILLED"
            );
        }
    }

    if trade.state == TradeState::TpPlaced {
        if current_price >= trade.tp_price {
            let key = format!("trade:{}:tp:{}", trade.trade_id, trade.tp_price);
            if repo.try_record_idempotency_key(&key).await? {
                let exit_order_id = repo
                    .append_order_event(
                        trade.trade_id,
                        "tp",
                        "sell",
                        trade.tp_price,
                        trade.position_size,
                        "filled",
                    )
                    .await?;
                repo.append_fill_event(exit_order_id, trade.tp_price, trade.position_size, 0.0)
                    .await?;
                transition(repo, trade, TradeState::ExitFilled, "tp-hit").await?;
                let pnl = (trade.tp_price - trade.entry_price) * trade.position_size;
                repo.close_trade(trade.trade_id, trade.tp_price, pnl)
                    .await?;
                info!(run_id, trade_id = trade.trade_id, pnl, "TRADE_COMPLETED");
            }
            return Ok(());
        }

        let aggressive_sl =
            strategy.aggressive_stop_price(trade.entry_price, cfg.strategy.aggressive_sl_pct);
        if current_price <= aggressive_sl {
            let key = format!("trade:{}:sl:{:.4}", trade.trade_id, aggressive_sl);
            if repo.try_record_idempotency_key(&key).await? {
                transition(repo, trade, TradeState::SlArmed, "aggressive-sl-armed").await?;
                let sl_order_id = repo
                    .append_order_event(
                        trade.trade_id,
                        "sl",
                        "sell",
                        aggressive_sl,
                        trade.position_size,
                        "filled",
                    )
                    .await?;
                repo.append_fill_event(sl_order_id, aggressive_sl, trade.position_size, 0.0)
                    .await?;
                transition(repo, trade, TradeState::ExitFilled, "sl-hit").await?;
                let pnl = (aggressive_sl - trade.entry_price) * trade.position_size;
                repo.close_trade(trade.trade_id, aggressive_sl, pnl).await?;
                warn!(
                    run_id,
                    trade_id = trade.trade_id,
                    pnl,
                    "TRADE_COMPLETED_WITH_SL"
                );
            }
        }
    }

    Ok(())
}

async fn reconcile_live(
    repo: &PostgresRepository,
    run_id: i64,
    trade: &mut TradeRuntime,
    client: &dyn OrderExecutor,
    user_events: &[WsEvent],
) -> Result<ReconcileOutcome> {
    let open_orders = match client.list_open(Some(&trade.market_slug)).await {
        Ok(v) => v,
        Err(e) => {
            record_reconcile_error(
                repo,
                run_id,
                &trade.market_slug,
                ReconcileErrorKind::Network,
                &e.to_string(),
            )
            .await?;
            return Ok(ReconcileOutcome::Error);
        }
    };
    let fills = match client.list_fills(None).await {
        Ok(v) => v,
        Err(e) => {
            record_reconcile_error(
                repo,
                run_id,
                &trade.market_slug,
                ReconcileErrorKind::Network,
                &e.to_string(),
            )
            .await?;
            return Ok(ReconcileOutcome::Error);
        }
    };

    apply_order_reconcile(repo, trade.trade_id, &open_orders).await?;
    let (applied_fills, duplicate_fills, skipped_fills) =
        apply_fill_reconcile(repo, trade.trade_id, &fills).await?;
    let user_applied = apply_user_stream_events(repo, trade.trade_id, user_events).await?;

    if matches!(
        trade.state,
        TradeState::EntryPlaced | TradeState::EntryPartiallyFilled
    ) && (applied_fills > 0 || user_applied > 0)
    {
        transition(
            repo,
            trade,
            TradeState::EntryFilled,
            "reconcile-entry-filled",
        )
        .await?;
        transition(repo, trade, TradeState::TpPlaced, "reconcile-tp-placed").await?;
    }

    let status = if skipped_fills > 0 { "warning" } else { "ok" };
    repo.record_reconcile_run(
        run_id,
        &trade.market_slug,
        status,
        &json!({
            "open_orders_count": open_orders.len(),
            "fills_count": fills.len(),
            "applied_fills": applied_fills,
            "duplicate_fills": duplicate_fills,
            "skipped_fills": skipped_fills,
            "user_events_applied": user_applied
        })
        .to_string(),
    )
    .await?;

    Ok(if skipped_fills > 0 {
        ReconcileOutcome::Warning
    } else {
        ReconcileOutcome::Ok
    })
}

async fn apply_order_reconcile(
    repo: &PostgresRepository,
    trade_id: i64,
    open_orders: &[OrderInfo],
) -> Result<()> {
    for order in open_orders {
        if order.order_id.is_empty() {
            continue;
        }
        let raw = json!({
            "order_id": order.order_id,
            "client_order_id": order.client_order_id,
            "status": order.status,
            "price": order.price,
            "size": order.size,
            "filled_size": order.filled_size
        });
        repo.upsert_order_by_exchange_id(
            trade_id,
            &order.order_id,
            order.client_order_id.as_deref(),
            "reconcile",
            "unknown",
            order.price.unwrap_or_default(),
            order.size.unwrap_or_default(),
            &order.status,
            None,
            None,
            &raw,
        )
        .await?;
    }
    Ok(())
}

async fn apply_fill_reconcile(
    repo: &PostgresRepository,
    trade_id: i64,
    fills: &[FillInfo],
) -> Result<(usize, usize, usize)> {
    let mut applied = 0usize;
    let mut duplicates = 0usize;
    let mut skipped = 0usize;

    for fill in fills {
        if fill.fill_id.is_empty() || fill.order_id.is_empty() {
            skipped += 1;
            continue;
        }
        let key = format!("fill:{}", fill.fill_id);
        if !repo.try_record_idempotency_key(&key).await? {
            duplicates += 1;
            continue;
        }
        let internal_order_id = if let Some(id) = repo
            .internal_order_id_by_exchange_order_id(&fill.order_id)
            .await?
        {
            id
        } else {
            let raw_order = json!({"from_fill": true, "exchange_order_id": fill.order_id, "trade_id": trade_id});
            repo.upsert_order_by_exchange_id(
                trade_id,
                &fill.order_id,
                None,
                "reconcile",
                "unknown",
                fill.price,
                fill.size,
                "filled",
                fill.ts,
                None,
                &raw_order,
            )
            .await?
        };
        let raw_fill = json!({
            "fill_id": fill.fill_id,
            "order_id": fill.order_id,
            "price": fill.price,
            "size": fill.size,
            "fee": fill.fee,
            "timestamp": fill.ts
        });
        repo.upsert_fill_by_exchange_fill_id(
            internal_order_id,
            &fill.fill_id,
            fill.price,
            fill.size,
            fill.fee.unwrap_or_default(),
            fill.ts,
            &raw_fill,
        )
        .await?;
        applied += 1;
    }
    Ok((applied, duplicates, skipped))
}

async fn apply_user_stream_events(
    repo: &PostgresRepository,
    trade_id: i64,
    events: &[WsEvent],
) -> Result<usize> {
    let mut applied = 0usize;
    for event in events {
        if !matches!(event.channel, WsChannel::User) {
            continue;
        }
        if !matches!(event.event_type, WsEventType::Fill | WsEventType::Order) {
            continue;
        }
        let key = format!(
            "ws:{}:{:?}",
            event
                .fill_id
                .as_deref()
                .or(event.order_id.as_deref())
                .unwrap_or("na"),
            event.event_type
        );
        if !repo.try_record_idempotency_key(&key).await? {
            continue;
        }
        repo.record_risk_event(
            Some(trade_id),
            "user_stream_event",
            "allow",
            &event.payload.to_string(),
        )
        .await?;
        applied += 1;
    }
    Ok(applied)
}

async fn record_reconcile_error(
    repo: &PostgresRepository,
    run_id: i64,
    market_slug: &str,
    kind: ReconcileErrorKind,
    details: &str,
) -> Result<()> {
    repo.record_reconcile_run(
        run_id,
        market_slug,
        "error",
        &json!({
            "kind": format!("{kind:?}"),
            "details": details
        })
        .to_string(),
    )
    .await
}

async fn halt_trade(
    repo: &PostgresRepository,
    run_id: i64,
    trade: &mut TradeRuntime,
    reason: &str,
    order_executor: Option<&dyn OrderExecutor>,
) -> Result<()> {
    if let Some(executor) = order_executor {
        enforce_halt_open_order_safety(repo, run_id, trade, executor).await?;
    }
    transition(repo, trade, TradeState::Halted, reason).await
}

async fn enforce_halt_open_order_safety(
    repo: &PostgresRepository,
    run_id: i64,
    trade: &TradeRuntime,
    order_executor: &dyn OrderExecutor,
) -> Result<()> {
    let db_open_order_ids: BTreeSet<String> = repo
        .open_exchange_order_ids_for_trade(trade.trade_id)
        .await?
        .into_iter()
        .filter(|id| !id.is_empty())
        .collect();
    let mut open_order_ids: BTreeSet<String> = BTreeSet::new();

    match order_executor.list_open(Some(&trade.market_slug)).await {
        Ok(open_orders) => {
            for order in open_orders {
                if !order.order_id.is_empty() {
                    open_order_ids.insert(order.order_id);
                }
            }
            if open_order_ids.is_empty() {
                open_order_ids.extend(db_open_order_ids.iter().cloned());
            }
        }
        Err(e) => {
            record_reconcile_error(
                repo,
                run_id,
                &trade.market_slug,
                ReconcileErrorKind::Network,
                &format!("halt-cancel list_open failed: {e}"),
            )
            .await?;
            open_order_ids.extend(db_open_order_ids.iter().cloned());
        }
    }

    let mut canceled = 0usize;
    let mut failed = 0usize;
    for exchange_order_id in &open_order_ids {
        match order_executor.cancel(exchange_order_id).await {
            Ok(()) => {
                repo.mark_order_status(exchange_order_id, "canceled")
                    .await?;
                repo.record_risk_event(
                    Some(trade.trade_id),
                    "halt_cancel_order",
                    "allow",
                    &format!("exchange_order_id={exchange_order_id}"),
                )
                .await?;
                canceled += 1;
            }
            Err(e) => {
                record_reconcile_error(
                    repo,
                    run_id,
                    &trade.market_slug,
                    ReconcileErrorKind::Network,
                    &format!("halt-cancel failed order_id={exchange_order_id} err={e}"),
                )
                .await?;
                repo.record_risk_event(
                    Some(trade.trade_id),
                    "halt_cancel_order",
                    "block",
                    &format!("exchange_order_id={exchange_order_id} err={e}"),
                )
                .await?;
                failed += 1;
            }
        }
    }

    repo.record_reconcile_run(
        run_id,
        &trade.market_slug,
        if failed > 0 { "error" } else { "ok" },
        &json!({
            "reason": "halt_open_order_safety",
            "trade_id": trade.trade_id,
            "candidates": open_order_ids.len(),
            "canceled": canceled,
            "failed": failed
        })
        .to_string(),
    )
    .await?;

    info!(
        run_id,
        trade_id = trade.trade_id,
        candidates = open_order_ids.len(),
        canceled,
        failed,
        "HALT_OPEN_ORDER_SAFETY_COMPLETED"
    );
    Ok(())
}

async fn create_runtime(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    market_id: i64,
    market_slug: String,
    strategy: &impl Strategy,
) -> Result<TradeRuntime> {
    let trade_id = repo
        .create_trade_stub(
            market_id,
            cfg.strategy.entry_price,
            cfg.risk.max_notional_per_market_usdc,
        )
        .await?;

    let size =
        (cfg.risk.max_notional_per_market_usdc / cfg.strategy.entry_price * 100.0).round() / 100.0;
    Ok(TradeRuntime {
        trade_id,
        user_id: None,
        market_slug,
        entry_price: cfg.strategy.entry_price,
        tp_price: strategy.take_profit_price(cfg.strategy.entry_price, cfg.strategy.tp_pct),
        position_size: size,
        state: TradeState::Idle,
    })
}

pub(crate) fn to_risk_limits(cfg: &AppConfig) -> RiskLimits {
    RiskLimits {
        max_daily_loss_usdc: cfg.risk.max_daily_loss_usdc,
        max_consecutive_losses: cfg.risk.max_consecutive_losses,
        max_notional_per_market_usdc: cfg.risk.max_notional_per_market_usdc,
        max_open_orders: cfg.risk.max_open_orders,
        max_stale_data_ms: cfg.risk.max_stale_data_ms,
        kill_switch_mode: cfg.risk.kill_switch_mode,
    }
}

async fn risk_gate(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    trade: &TradeRuntime,
    limits: &RiskLimits,
    stale_data_ms: u64,
    policy: &impl RiskPolicy,
) -> Result<RiskDecision> {
    let (open_orders, daily_pnl, consec_losses) = if let Some(user_id) = trade.user_id {
        (
            repo.open_order_count_for_user(user_id).await?,
            repo.daily_realized_pnl_for_user(user_id).await?,
            repo.consecutive_losses_for_user(user_id, cfg.risk.max_consecutive_losses as i64)
                .await?,
        )
    } else {
        (
            repo.open_order_count().await?,
            repo.daily_realized_pnl().await?,
            repo.consecutive_losses(cfg.risk.max_consecutive_losses as i64)
                .await?,
        )
    };

    let risk = policy.evaluate(
        limits,
        &RiskInput {
            proposed_notional_usdc: cfg.risk.max_notional_per_market_usdc,
            open_orders,
            stale_data_ms,
            daily_realized_pnl_usdc: daily_pnl,
            consecutive_losses: consec_losses,
            manual_kill_switch_active: cfg.risk.manual_kill_switch_active,
        },
    );

    StateRepository::record_risk_event(
        repo,
        Some(trade.trade_id),
        "risk_check",
        &format!("{:?}", risk.decision).to_lowercase(),
        risk.reason,
    )
    .await?;

    match risk.decision {
        RiskDecision::Halt => {
            warn!(
                run_id,
                trade_id = trade.trade_id,
                reason = risk.reason,
                "RISK_HALT"
            );
            return Ok(RiskDecision::Halt);
        }
        RiskDecision::Block => {
            warn!(
                run_id,
                trade_id = trade.trade_id,
                reason = risk.reason,
                "RISK_BLOCK"
            );
            return Ok(RiskDecision::Block);
        }
        RiskDecision::Allow => {}
    }

    Ok(RiskDecision::Allow)
}

async fn transition(
    repo: &PostgresRepository,
    trade: &mut TradeRuntime,
    to: TradeState,
    reason: &str,
) -> Result<()> {
    let from = trade.state;
    StateRepository::transition_trade_state(repo, trade.trade_id, from, to, reason).await?;
    trade.state = to;
    Ok(())
}
