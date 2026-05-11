fn parse_rfc3339_utc(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .map(|value| value.with_timezone(&Utc))
        .ok()
}

pub(crate) fn dual_dca_timeframe_duration(timeframe: &str) -> ChronoDuration {
    match timeframe.trim().to_ascii_lowercase().as_str() {
        "15m" => ChronoDuration::minutes(15),
        _ => ChronoDuration::minutes(5),
    }
}

async fn process_trade_builder_workflows(
    repo: &PostgresRepository,
    run_id: i64,
    _cfg: &AppConfig,
    _client: &dyn OrderExecutor,
    ws: &ClobWsClient,
) -> Result<()> {
    let workflows = repo
        .list_trade_builder_workflows_for_processing(WORKFLOW_PROCESS_LIMIT)
        .await?;
    if workflows.is_empty() {
        return Ok(());
    }

    let policy = DefaultRiskPolicy;
    let mut user_cfg_cache: HashMap<i64, AppConfig> = HashMap::new();
    let mut user_executor_cache: HashMap<i64, SharedOrderExecutor> = HashMap::new();
    let mut synced_user_ids: HashSet<i64> = HashSet::new();

    for workflow in workflows {
        let result = async {
            let user_cfg =
                load_user_app_config_cached(repo, workflow.user_id, &mut user_cfg_cache).await?;
            let client = load_user_order_executor_cached(
                repo,
                workflow.user_id,
                &mut user_cfg_cache,
                &mut user_executor_cache,
            )
            .await?;
            if synced_user_ids.insert(workflow.user_id) {
                sync_recent_trade_builder_fills(repo, client.as_ref()).await?;
            }
            let limits = to_risk_limits(&user_cfg);
            process_trade_builder_workflow(
                repo,
                run_id,
                &user_cfg,
                &limits,
                &policy,
                client.as_ref(),
                ws,
                &workflow,
            )
            .await
        }
        .await;
        if let Err(err) = result {
            let err_text = format!("{err:#}");
            let _ = repo
                .set_trade_builder_workflow_status(workflow.id, "error", Some(&err_text))
                .await;
            let _ = repo
                .append_trade_builder_workflow_event(
                    workflow.id,
                    None,
                    "processing_error",
                    &json!({ "error": err_text }),
                )
                .await;
            warn!(
                run_id,
                workflow_id = workflow.id,
                error = %err_text,
                "TRADE_BUILDER_WORKFLOW_ERROR"
            );
        }
        if trade_flow_ws_fast_path_cache_requires_refresh_now().await {
            if let Err(e) = refresh_trade_flow_ws_fast_path_for_boundary(
                repo, run_id, ws, &mut user_cfg_cache,
            ).await {
                warn!(run_id, error = %e, "TRADE_FLOW_BOUNDARY_REFRESH_FAILED");
            }
        }
    }

    for user_id in synced_user_ids {
        let Some(client) = user_executor_cache.get(&user_id) else {
            continue;
        };
        if let Err(err) = sync_recent_trade_builder_fills(repo, client.as_ref()).await {
            let err_text = format!("{err:#}");
            warn!(
                run_id,
                user_id,
                error = %err_text,
                "TRADE_BUILDER_FILL_SYNC_ERROR"
            );
        }
    }

    Ok(())
}

async fn process_trade_builder_workflow(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    workflow: &TradeBuilderWorkflow,
) -> Result<()> {
    if let Some(expires_at) = workflow.expires_at {
        if expires_at <= Utc::now() {
            if workflow.status != "expired" {
                repo.set_trade_builder_workflow_status(workflow.id, "expired", None)
                    .await?;
                repo.append_trade_builder_workflow_event(
                    workflow.id,
                    None,
                    "expired",
                    &json!({ "expires_at": expires_at }),
                )
                .await?;
            }
            return Ok(());
        }
    }

    let legs = repo.load_trade_builder_workflow_legs(workflow.id).await?;
    let mut sell_leg = legs
        .iter()
        .find(|leg| leg.leg_type == "sell")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("workflow missing sell leg"))?;
    let mut buy_leg = legs
        .iter()
        .find(|leg| leg.leg_type == "buy")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("workflow missing buy leg"))?;

    let _ = ensure_sell_leg_order(repo, workflow, &sell_leg).await?;
    refresh_workflow_leg_fill_metrics(repo, &mut sell_leg).await?;
    refresh_workflow_leg_fill_metrics(repo, &mut buy_leg).await?;

    let sell_progress_pct = workflow_leg_progress_pct(&sell_leg);

    if workflow.status == "armed" && sell_progress_pct > 0.0 {
        repo.set_trade_builder_workflow_status(workflow.id, "running", None)
            .await?;
    }

    let mut buy_child_transitioned = false;
    if let Some(active_buy_order_id) = buy_leg.builder_order_id {
        if let Some(active_buy_order) = repo.get_trade_builder_order(active_buy_order_id).await? {
            if active_buy_order.status == "completed" {
                repo.set_trade_builder_workflow_leg_builder_order(buy_leg.id, None, "armed")
                    .await?;
                repo.append_trade_builder_workflow_event(
                    workflow.id,
                    Some(buy_leg.id),
                    "buy_child_completed",
                    &json!({
                        "builder_order_id": active_buy_order.id,
                        "filled_notional_usdc": buy_leg.filled_notional_usdc,
                        "filled_qty": buy_leg.filled_qty
                    }),
                )
                .await?;
                buy_leg.builder_order_id = None;
                buy_child_transitioned = true;
            } else if matches!(
                active_buy_order.status.as_str(),
                "canceled" | "expired" | "blocked" | "error"
            ) {
                repo.set_trade_builder_workflow_leg_builder_order(buy_leg.id, None, "armed")
                    .await?;
                repo.append_trade_builder_workflow_event(
                    workflow.id,
                    Some(buy_leg.id),
                    "buy_child_terminal",
                    &json!({
                        "builder_order_id": active_buy_order.id,
                        "status": active_buy_order.status
                    }),
                )
                .await?;
                buy_leg.builder_order_id = None;
                buy_child_transitioned = true;
            }
        } else {
            repo.set_trade_builder_workflow_leg_builder_order(buy_leg.id, None, "armed")
                .await?;
            buy_leg.builder_order_id = None;
            buy_child_transitioned = true;
        }
    }

    refresh_workflow_leg_fill_metrics(repo, &mut buy_leg).await?;
    if buy_child_transitioned {
        return Ok(());
    }

    let threshold_ok = sell_progress_pct >= workflow.buy_start_after_sell_progress_pct;
    let (price_ok, workflow_current_price, workflow_previous_price) =
        evaluate_workflow_buy_price_condition(repo, ws, client, &buy_leg).await?;
    let should_buy = workflow_should_activate_buy(workflow, threshold_ok, price_ok);

    let desired_buy_notional = (buy_leg.target_notional_usdc * (sell_progress_pct / 100.0))
        .clamp(0.0, buy_leg.target_notional_usdc.max(0.0));
    let mut delta_notional = (desired_buy_notional - buy_leg.filled_notional_usdc).max(0.0);

    if !should_buy {
        let (reason_code, reason_message) = if !threshold_ok && !price_ok {
            (
                "workflow_condition_not_met",
                "Sell progress and price condition are both not met.",
            )
        } else if !threshold_ok {
            (
                "workflow_waiting_sell_progress",
                "Waiting for required sell progress threshold.",
            )
        } else {
            (
                "workflow_waiting_price_condition",
                "Waiting for price trigger condition.",
            )
        };
        info!(
            run_id,
            workflow_id = workflow.id,
            buy_leg_id = buy_leg.id,
            reason_code,
            sell_progress_pct,
            sell_progress_threshold = workflow.buy_start_after_sell_progress_pct,
            price_condition_passed = price_ok,
            current_price = ?workflow_current_price,
            previous_price = ?workflow_previous_price,
            trigger_condition = ?buy_leg.trigger_condition,
            trigger_price = ?buy_leg.trigger_price,
            "TRADE_BUILDER_WORKFLOW_NOT_BUY_DECISION"
        );
        repo.set_trade_builder_workflow_leg_status(buy_leg.id, "waiting_sell_progress")
            .await?;
        repo.append_trade_builder_workflow_event(
            workflow.id,
            Some(buy_leg.id),
            "not_buy_decision",
            &json!({
                "reason_code": reason_code,
                "reason_message": reason_message,
                "mode": workflow.buy_trigger_mode,
                "sell_progress_pct": sell_progress_pct,
                "sell_progress_threshold_pct": workflow.buy_start_after_sell_progress_pct,
                "threshold_ok": threshold_ok,
                "price_ok": price_ok,
                "market_slug": &buy_leg.market_slug,
                "token_id": &buy_leg.token_id,
                "trigger_condition": buy_leg.trigger_condition.as_deref(),
                "trigger_price": buy_leg.trigger_price,
                "previous_price": workflow_previous_price,
                "current_price": workflow_current_price
            }),
        )
        .await?;
        delta_notional = 0.0;
    }

    let can_place_buy_child = buy_leg.builder_order_id.is_none();
    if should_buy
        && can_place_buy_child
        && delta_notional >= WORKFLOW_MIN_BUY_INCREMENT_USDC
        && buy_leg.filled_notional_usdc < buy_leg.target_notional_usdc
    {
        let risk = risk_gate_manual_order(
            repo,
            run_id,
            cfg,
            Some(workflow.user_id),
            workflow.source_trade_id,
            delta_notional,
            limits,
            policy,
        )
        .await?;

        if matches!(risk, RiskDecision::Allow) {
            let buy_order_id = repo
                .create_trade_builder_order(
                    workflow.source_trade_id,
                    "immediate",
                    "pending",
                    &buy_leg.market_slug,
                    &buy_leg.token_id,
                    &buy_leg.outcome_label,
                    &buy_leg.side,
                    "limit",
                    None,
                    None,
                    None,
                    None,
                    None,
                    TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC,
                    delta_notional,
                    None,
                    None,
                    buy_leg.min_price_distance_cent,
                    workflow.expires_at,
                    None,
                    None,
                    1,
                    None,
                    false,
                    None,
                    false,
                    None,
                    0,
                    None,
                    None,
                    None,
                    None,
                    false,
                    0,
                    None,
                    None,
                    false,
                    false,
                    false,
                    false,
                    false,
                    false,
                    false,
                    false,
                    None,
                    false,
                    false,
                    false,
                )
                .await?;
            repo.set_trade_builder_workflow_leg_builder_order(
                buy_leg.id,
                Some(buy_order_id),
                "open",
            )
            .await?;
            repo.set_trade_builder_workflow_status(workflow.id, "running", None)
                .await?;
            repo.append_trade_builder_workflow_event(
                workflow.id,
                Some(buy_leg.id),
                "buy_child_created",
                &json!({
                    "builder_order_id": buy_order_id,
                    "size_usdc": delta_notional,
                    "sell_progress_pct": sell_progress_pct,
                    "mode": workflow.buy_trigger_mode
                }),
            )
            .await?;
        } else {
            repo.set_trade_builder_workflow_leg_status(buy_leg.id, "blocked")
                .await?;
            repo.append_trade_builder_workflow_event(
                workflow.id,
                Some(buy_leg.id),
                "buy_blocked_by_risk",
                &json!({
                    "reason_code": "risk_blocked",
                    "reason_message": "Buy leg order blocked by risk policy.",
                    "decision": format!("{risk:?}"),
                    "size_usdc": delta_notional,
                    "sell_progress_pct": sell_progress_pct
                }),
            )
            .await?;
            warn!(
                run_id,
                workflow_id = workflow.id,
                buy_leg_id = buy_leg.id,
                reason_code = "risk_blocked",
                decision = %format!("{risk:?}"),
                size_usdc = delta_notional,
                "TRADE_BUILDER_WORKFLOW_NOT_BUY_DECISION"
            );
        }
    }

    let is_sell_done = sell_progress_pct >= 99.999;
    let no_active_buy_order = buy_leg.builder_order_id.is_none();
    let is_buy_target_done = buy_leg.filled_notional_usdc + 0.0001 >= buy_leg.target_notional_usdc;

    if is_sell_done && no_active_buy_order && is_buy_target_done {
        repo.set_trade_builder_workflow_status(workflow.id, "completed", None)
            .await?;
        repo.set_trade_builder_workflow_leg_status(sell_leg.id, "completed")
            .await?;
        repo.set_trade_builder_workflow_leg_status(buy_leg.id, "completed")
            .await?;
        repo.append_trade_builder_workflow_event(
            workflow.id,
            None,
            "completed",
            &json!({
                "sell_progress_pct": sell_progress_pct,
                "buy_filled_usdc": buy_leg.filled_notional_usdc,
                "buy_target_usdc": buy_leg.target_notional_usdc
            }),
        )
        .await?;
    }

    Ok(())
}

async fn ensure_sell_leg_order(
    repo: &PostgresRepository,
    workflow: &TradeBuilderWorkflow,
    sell_leg: &TradeBuilderWorkflowLeg,
) -> Result<Option<TradeBuilderOrder>> {
    if let Some(builder_order_id) = sell_leg.builder_order_id {
        if let Some(existing) = repo.get_trade_builder_order(builder_order_id).await? {
            return Ok(Some(existing));
        }
        repo.set_trade_builder_workflow_leg_builder_order(sell_leg.id, None, "pending")
            .await?;
    }

    anyhow::ensure!(
        sell_leg.target_notional_usdc > 0.0,
        "sell leg target_notional_usdc must be > 0"
    );

    let kind = if sell_leg.trigger_condition.is_some() && sell_leg.trigger_price.is_some() {
        "conditional"
    } else {
        "immediate"
    };

    let sell_order_id = repo
        .create_trade_builder_order(
            workflow.source_trade_id,
            kind,
            "pending",
            &sell_leg.market_slug,
            &sell_leg.token_id,
            &sell_leg.outcome_label,
            &sell_leg.side,
            "limit",
            sell_leg.trigger_condition.as_deref(),
            sell_leg.trigger_price,
            None,
            None,
            None,
            TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC,
            sell_leg.target_notional_usdc,
            None,
            None,
            sell_leg.min_price_distance_cent,
            workflow.expires_at,
            None,
            None,
            1,
            None,
            false,
            None,
            false,
            None,
            0,
            None,
            None,
            None,
            None,
            false,
            0,
            None,
            None,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            None,
            false,
            false,
            false,
        )
        .await?;
    repo.set_trade_builder_workflow_leg_builder_order(sell_leg.id, Some(sell_order_id), "open")
        .await?;
    repo.append_trade_builder_workflow_event(
        workflow.id,
        Some(sell_leg.id),
        "sell_child_created",
        &json!({
            "builder_order_id": sell_order_id,
            "kind": kind,
            "target_notional_usdc": sell_leg.target_notional_usdc
        }),
    )
    .await?;

    repo.get_trade_builder_order(sell_order_id).await
}

fn workflow_leg_progress_pct(leg: &TradeBuilderWorkflowLeg) -> f64 {
    if leg.target_notional_usdc <= 0.0 {
        return 0.0;
    }

    ((leg.filled_notional_usdc.max(0.0) / leg.target_notional_usdc) * 100.0).clamp(0.0, 100.0)
}

fn workflow_should_activate_buy(
    workflow: &TradeBuilderWorkflow,
    threshold_ok: bool,
    price_ok: bool,
) -> bool {
    match workflow.buy_trigger_mode.as_str() {
        "sell_progress_only" => threshold_ok,
        "price_only" => price_ok,
        "sell_progress_and_price" => threshold_ok && price_ok,
        _ => threshold_ok && price_ok,
    }
}

async fn evaluate_workflow_buy_price_condition(
    repo: &PostgresRepository,
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    buy_leg: &TradeBuilderWorkflowLeg,
) -> Result<(bool, Option<f64>, Option<f64>)> {
    let Some(trigger_price) = buy_leg.trigger_price else {
        return Ok((true, None, buy_leg.last_seen_price));
    };
    let Some(trigger_condition) = buy_leg.trigger_condition.as_deref() else {
        return Ok((true, None, buy_leg.last_seen_price));
    };

    let current_price =
        fetch_current_token_price_for_leg(ws, client, &buy_leg.market_slug, &buy_leg.token_id)
            .await?;
    repo.set_trade_builder_workflow_leg_last_seen_price(buy_leg.id, current_price)
        .await?;

    let previous_price = buy_leg.last_seen_price;
    let pass = match trigger_condition {
        "cross_above" => crossed_above_strict(previous_price, current_price, trigger_price),
        "cross_below" => crossed_below_strict(previous_price, current_price, trigger_price),
        _ => true,
    };
    Ok((pass, Some(current_price), previous_price))
}

async fn fetch_current_token_price_for_leg(
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    _market_slug: &str,
    token_id: &str,
) -> Result<f64> {
    if let Some(ws_price) = fetch_price_from_market_ws(ws, token_id).await {
        return Ok(clamp_probability(ws_price));
    }
    let fallback = client.midpoint(token_id).await?;
    Ok(clamp_probability(fallback.price))
}

async fn refresh_workflow_leg_fill_metrics(
    repo: &PostgresRepository,
    leg: &mut TradeBuilderWorkflowLeg,
) -> Result<()> {
    let (filled_notional_usdc, filled_qty) = repo
        .aggregate_trade_builder_workflow_leg_fills(leg.id)
        .await?;

    if (leg.filled_notional_usdc - filled_notional_usdc).abs() > 0.0001
        || (leg.filled_qty - filled_qty).abs() > 0.000001
    {
        repo.set_trade_builder_workflow_leg_filled_metrics(
            leg.id,
            filled_notional_usdc,
            filled_qty,
        )
        .await?;
    }

    leg.filled_notional_usdc = filled_notional_usdc;
    leg.filled_qty = filled_qty;
    Ok(())
}

pub(crate) async fn sync_recent_trade_builder_fills(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
) -> Result<usize> {
    let fills = client.list_fills(None).await?;
    let mut synced = 0usize;

    for fill in fills {
        if fill.fill_id.is_empty() || fill.order_id.is_empty() {
            continue;
        }
        let Some(internal_order_id) = repo
            .internal_order_id_by_exchange_order_id(&fill.order_id)
            .await?
        else {
            continue;
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

        synced = synced.saturating_add(1);
    }

    Ok(synced)
}

fn is_trade_builder_order_processable_status(status: &str) -> bool {
    matches!(
        status,
        "pending"
            | "armed"
            | "triggered"
            | "open"
            | "partially_filled"
            | "canceled_requested"
            | "guard_blocked"
            | "inventory_pending"
    )
}

fn should_request_trade_builder_oco_cancel(
    order: &TradeBuilderOrder,
    normalized_status: &str,
) -> bool {
    trade_builder_is_child_exit_sell(order)
        && order.exit_ladder_kind.is_none()
        && matches!(normalized_status, "open" | "partially_filled" | "filled")
}

fn trade_builder_fee_rate_bps_or_default(raw: i64) -> u64 {
    if raw > 0 {
        raw as u64
    } else {
        DEFAULT_TRADE_BUILDER_FEE_RATE_BPS
    }
}

fn trade_builder_is_stop_loss_child(order: &TradeBuilderOrder) -> bool {
    order.parent_order_id.is_some()
        && order.side == "sell"
        && order.kind == "conditional"
        && matches!(order.trigger_condition.as_deref(), Some("cross_below"))
}

fn trade_builder_stop_loss_latched(order: &TradeBuilderOrder) -> bool {
    order.trigger_latched && order.trigger_latched_reason.as_deref() == Some("stop_loss")
}

fn estimate_trade_builder_buy_fee_shares(
    execution_price: f64,
    gross_qty: f64,
    fee_rate_bps: u64,
) -> f64 {
    if execution_price <= 0.0 || gross_qty <= 0.0 || fee_rate_bps == 0 {
        return 0.0;
    }

    // Fee-enabled crypto markets expose fee_rate_bps via the CLOB API. For buy fills,
    // the fee is collected in shares, so estimate the fee in quote terms via the
    // documented fee curve and convert it back to shares.
    let fee_curve_rate = fee_rate_bps as f64 / 4000.0;
    let curve_input = (execution_price * (1.0 - execution_price)).clamp(0.0, 1.0);
    let fee_quote = gross_qty * fee_curve_rate * curve_input.powi(2);
    (fee_quote / execution_price).max(0.0)
}
