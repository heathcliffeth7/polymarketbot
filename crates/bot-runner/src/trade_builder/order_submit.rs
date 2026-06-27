async fn append_trade_builder_guard_diagnostics_event(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    current_price: f64,
    desired_price: f64,
    best_ask: Option<f64>,
    trigger_price_guard: Value,
    execution_floor_guard: Value,
    max_price_guard: Value,
    effective_guard_scope: Option<&str>,
    effective_decision: &str,
    effective_reason_code: &str,
) -> Result<()> {
    let order_event_payload = json!({
        "market_slug": &order.market_slug,
        "token_id": &order.token_id,
        "outcome_label": &order.outcome_label,
        "status_before": &order.status,
        "current_price": current_price,
        "desired_price": desired_price,
        "best_ask": best_ask,
        "trigger_price_guard": trigger_price_guard.clone(),
        "execution_floor_guard": execution_floor_guard.clone(),
        "max_price_guard": max_price_guard.clone(),
        "effective_guard_scope": effective_guard_scope,
        "effective_decision": effective_decision,
        "effective_reason_code": effective_reason_code,
    });
    repo.append_trade_builder_order_event(order.id, "guard_evaluated", &order_event_payload)
        .await?;
    trade_builder_spawn_entry_evaluated_decision_log(
        repo,
        order,
        current_price,
        desired_price,
        best_ask,
        trigger_price_guard,
        execution_floor_guard,
        max_price_guard,
        effective_guard_scope,
        effective_decision,
        effective_reason_code,
    );
    Ok(())
}
#[allow(clippy::too_many_arguments)]
async fn submit_trade_builder_trigger_order(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    order: &mut TradeBuilderOrder,
    current_price: f64,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
    fee_rate_bps: u64,
    resolved_size_usdc: f64,
    trigger_size_mode: Option<String>,
    trigger_size_value: Option<f64>,
    trigger_size_index: usize,
    submit_context: &TradeBuilderSubmitAttemptContext,
    exit_fast_quote: Option<&ExitFastSubmitQuote>,
) -> Result<()> {
    let mut submit_started_at = Utc::now();
    let mut deferred_submit_events = DeferredTradeBuilderSubmitEvents::default();
    let cached_market_spec = trade_builder_cached_market_spec_for_order(order, submit_started_at);
    let exit_fast_quote = exit_fast_quote.filter(|_| cached_market_spec.is_some());
    let size_basis = normalize_trade_builder_size_basis(&order.size_basis);
    let submit_price_requested_qty = if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        trade_builder_share_request_qty(order)
    } else {
        None
    };
    let market_buy_execution_price =
        trade_builder_market_buy_execution_price(order, current_price, best_ask);
    let (mut sell_submit_price, prefetched_available_qty) = prefetch_trade_builder_sell_submit_inputs(
        client,
        order,
        run_id,
        current_price,
        best_bid,
        last_trade_price,
        submit_price_requested_qty,
        size_basis,
        exit_fast_quote,
    )
    .await;
    let mut fast_stop_loss_initial_submit_payload: Option<Value> = None;
    match prepare_trade_builder_fast_stop_loss_initial_submit(
        repo,
        client,
        ws,
        order,
        submit_started_at,
        current_price,
        best_bid,
        last_trade_price,
        submit_price_requested_qty,
        size_basis,
        exit_fast_quote,
    )
    .await?
    {
        TradeBuilderFastStopLossInitialSubmitOutcome::NotEligible => {}
        TradeBuilderFastStopLossInitialSubmitOutcome::ScheduledRetry => return Ok(()),
        TradeBuilderFastStopLossInitialSubmitOutcome::Resolved {
            sell_submit_price: resolution,
            payload,
        } => {
            sell_submit_price = Some(resolution);
            fast_stop_loss_initial_submit_payload = Some(payload);
        }
    }
    let mut desired_price = sell_submit_price
        .map(|resolution| resolution.desired_price)
        .or_else(|| market_buy_execution_price.map(|resolution| resolution.price))
        .unwrap_or_else(|| trade_builder_submit_desired_price(order, current_price));
    desired_price = trade_builder_clamp_buy_limit_price(order, desired_price);
    let uncapped_desired_price = sell_submit_price
        .map(|resolution| resolution.uncapped_desired_price)
        .or_else(|| market_buy_execution_price.map(|resolution| resolution.price))
        .unwrap_or_else(|| {
            aggressive_price_for_side(&order.side, current_price, order.min_price_distance_cent)
        });
    if market_buy_execution_price.is_none()
        && fast_stop_loss_initial_submit_payload.is_none()
        && (desired_price - uncapped_desired_price).abs() >= 0.000001
    {
        repo.append_trade_builder_order_event(
            order.id,
            "exit_price_capped",
            &json!({
                "current_price": current_price,
                "uncapped_desired_price": uncapped_desired_price,
                "capped_desired_price": desired_price,
                "price_floor": trade_builder_exit_sell_price_floor(order),
                "trigger_price": order.trigger_price,
            }),
        )
        .await?;
    }

    let (remaining_usdc, remaining_qty, size, proposed_notional_usdc) =
        if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
            let qty = trade_builder_share_request_qty(order).ok_or_else(|| {
                anyhow::anyhow!("share-basis builder order requires target_qty or remaining_qty")
            })?;
            let remaining_usdc = (qty * desired_price).max(0.0);
            (Some(remaining_usdc), Some(qty), qty, remaining_usdc)
        } else {
            let remaining_usdc = order.remaining_size.unwrap_or(resolved_size_usdc);
            let size = calc_level_size(remaining_usdc, desired_price);
            (Some(remaining_usdc), None, size, resolved_size_usdc)
        };
    anyhow::ensure!(size > 0.0, "computed builder order size is zero");

    let guard_eval_started = Instant::now();
    let force_pair_counter_guard_waiting =
        trade_builder_pair_lock_counter_forces_guard_waiting(repo, order).await?;
    let retry_on_trigger_guard_block =
        order.retry_on_trigger_guard_block || force_pair_counter_guard_waiting;
    let retry_on_execution_floor_guard_block =
        order.retry_on_execution_floor_guard_block || force_pair_counter_guard_waiting;
    let retry_on_max_price_block =
        order.retry_on_max_price_block || force_pair_counter_guard_waiting;
    let buy_guard_eval = evaluate_trade_builder_buy_guards(
        &order.execution_mode,
        order.pair_leg_role.as_deref(),
        current_price,
        best_ask,
        desired_price,
        order.guard_trigger_price,
        order.max_price,
        order.best_ask_floor_price,
        retry_on_trigger_guard_block,
        retry_on_execution_floor_guard_block,
        retry_on_max_price_block,
    );
    let trigger_guard_reference_price = buy_guard_eval.trigger_guard_reference_price;
    let trigger_guard_reference_source = buy_guard_eval.trigger_guard_reference_source;
    let trigger_price_guard_blocked = buy_guard_eval.trigger_price_guard_blocked;
    let trigger_price_guard_payload = buy_guard_eval.trigger_price_guard_payload.clone();
    let execution_floor_reason = buy_guard_eval.execution_floor_reason;
    let pair_lock_market_waiting_reason = buy_guard_eval.pair_lock_market_waiting_reason;
    let execution_floor_payload = buy_guard_eval.execution_floor_payload.clone();
    let max_price_reference = buy_guard_eval.max_price_reference;
    let max_price_reference_source = buy_guard_eval.max_price_reference_source;
    let max_price_blocked = buy_guard_eval.max_price_blocked;
    let max_price_payload = buy_guard_eval.max_price_payload.clone();

    if let Some(reason_code) = pair_lock_market_waiting_reason {
        let candidate_reason = build_guard_notification_reason("max_price", reason_code);
        append_trade_builder_guard_diagnostics_event(
            repo,
            order,
            current_price,
            desired_price,
            best_ask,
            trigger_price_guard_payload.clone(),
            execution_floor_payload.clone(),
            max_price_payload.clone(),
            Some("max_price"),
            "waiting",
            reason_code,
        )
        .await?;
        transition_trade_builder_order_to_guard_waiting(
            repo,
            order,
            reason_code,
            "max_price_waiting",
            &json!({
                "reason_code": reason_code,
                "reason_message": "Pair lock market buy is waiting for best ask before max price evaluation.",
                "market_slug": &order.market_slug,
                "token_id": &order.token_id,
                "trigger_condition": order.trigger_condition.as_deref(),
                "trigger_price": order.trigger_price,
                "max_price": order.max_price,
                "current_price": current_price,
                "desired_price": desired_price,
                "reference_price": Value::Null,
                "reference_price_source": "best_ask_unavailable",
                "status_before": &order.status,
                "status_after": TRADE_BUILDER_GUARD_BLOCKED_STATUS
            }),
            remaining_usdc,
            remaining_qty,
            Some(candidate_reason.as_str()),
            order
                .notify_on_max_price_blocked
                .then_some("max_price_waiting"),
            order.notify_on_max_price_blocked.then(|| {
                build_max_price_waiting_notification_message(
                    order,
                    current_price,
                    desired_price,
                    "best_ask_unavailable",
                    Some(reason_code),
                )
            }),
        )
        .await?;
        return Ok(());
    }

    if trigger_price_guard_blocked {
        let candidate_reason =
            build_guard_notification_reason("trigger_price", "below_trigger_price_guard");
        append_trade_builder_guard_diagnostics_event(
            repo,
            order,
            current_price,
            desired_price,
            best_ask,
            trigger_price_guard_payload.clone(),
            execution_floor_payload.clone(),
            max_price_payload.clone(),
            Some("trigger_price"),
            if retry_on_trigger_guard_block {
                "waiting"
            } else {
                "blocked"
            },
            "below_trigger_price_guard",
        )
        .await?;
        if retry_on_trigger_guard_block {
            let notification_message = order.notify_on_trigger_guard_blocked.then(|| {
                build_trigger_guard_waiting_notification_message(
                    order,
                    trigger_guard_reference_price,
                    trigger_guard_reference_source,
                )
            });
            transition_trade_builder_order_to_guard_waiting(
                repo,
                order,
                "below_trigger_price_guard",
                "trigger_price_waiting",
                &json!({
                    "reason_code": "below_trigger_price_guard",
                    "reason_message": "Current price is below the trigger price guard floor. Order is waiting for recovery.",
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "guard_trigger_price": order.guard_trigger_price,
                    "current_price": current_price,
                    "trigger_guard_reference_price": trigger_guard_reference_price,
                    "trigger_guard_reference_source": trigger_guard_reference_source,
                    "desired_price": desired_price,
                    "status_before": &order.status,
                    "status_after": TRADE_BUILDER_GUARD_BLOCKED_STATUS
                }),
                remaining_usdc,
                remaining_qty,
                Some(candidate_reason.as_str()),
                order
                    .notify_on_trigger_guard_blocked
                    .then_some("trigger_price_waiting"),
                notification_message,
            )
            .await?;
        } else {
            repo.set_trade_builder_order_status(
                order.id,
                "canceled",
                Some("below_trigger_price_guard"),
            )
            .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "trigger_price_blocked",
                &json!({
                    "reason_code": "below_trigger_price_guard",
                    "reason_message": "Current price is below the trigger price guard floor.",
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "guard_trigger_price": order.guard_trigger_price,
                    "current_price": current_price,
                    "trigger_guard_reference_price": trigger_guard_reference_price,
                    "trigger_guard_reference_source": trigger_guard_reference_source,
                    "desired_price": desired_price,
                    "status_before": &order.status,
                    "status_after": "canceled"
                }),
            )
            .await?;
            let message = build_trigger_guard_blocked_notification_message(
                order,
                trigger_guard_reference_price,
                trigger_guard_reference_source,
            );
            maybe_send_guard_transition_notification(
                repo,
                order,
                candidate_reason.as_str(),
                order.notify_on_trigger_guard_blocked,
                "trigger_price_blocked",
                &message,
            )
            .await?;
        }
        warn!(
            run_id,
            builder_order_id = order.id,
            market = %order.market_slug,
            token_id = %order.token_id,
            current_price,
            trigger_guard_reference_price,
            trigger_guard_reference_source,
            desired_price,
            guard_trigger_price = ?order.guard_trigger_price,
            waiting = retry_on_trigger_guard_block,
            reason_code = "below_trigger_price_guard",
            "TRADE_BUILDER_ORDER_TRIGGER_PRICE_BLOCKED"
        );
        return Ok(());
    }

    if let Some(reason_code) = execution_floor_reason {
        let candidate_reason = build_guard_notification_reason("execution_floor", reason_code);
        let should_wait = trade_builder_execution_floor_should_wait(order, reason_code)
            || force_pair_counter_guard_waiting;
        append_trade_builder_guard_diagnostics_event(
            repo,
            order,
            current_price,
            desired_price,
            best_ask,
            trigger_price_guard_payload.clone(),
            execution_floor_payload.clone(),
            max_price_payload.clone(),
            Some("execution_floor"),
            if should_wait { "waiting" } else { "blocked" },
            reason_code,
        )
        .await?;
        let reason_message = match reason_code {
            "best_ask_unavailable" => {
                "Best ask is unavailable, so the execution floor guard blocked the buy order."
            }
            "below_best_ask_floor" => "Best ask is below the configured execution floor.",
            _ => "Execution floor guard blocked the buy order.",
        };
        if should_wait {
            let notification_message = order
                .notify_on_execution_floor_blocked
                .then(|| build_execution_floor_waiting_notification_message(order, best_ask));
            transition_trade_builder_order_to_guard_waiting(
                repo,
                order,
                reason_code,
                "execution_floor_waiting",
                &json!({
                    "reason_code": reason_code,
                    "reason_message": "Execution floor guard moved the order into waiting mode.",
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "best_ask_floor_price": order.best_ask_floor_price,
                    "best_ask": best_ask,
                    "current_price": current_price,
                    "desired_price": desired_price,
                    "status_before": &order.status,
                    "status_after": TRADE_BUILDER_GUARD_BLOCKED_STATUS
                }),
                remaining_usdc,
                remaining_qty,
                Some(candidate_reason.as_str()),
                order
                    .notify_on_execution_floor_blocked
                    .then_some("execution_floor_waiting"),
                notification_message,
            )
            .await?;
        } else {
            repo.set_trade_builder_order_status(order.id, "canceled", Some(reason_code))
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "execution_floor_blocked",
                &json!({
                    "reason_code": reason_code,
                    "reason_message": reason_message,
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "best_ask_floor_price": order.best_ask_floor_price,
                    "best_ask": best_ask,
                    "current_price": current_price,
                    "desired_price": desired_price,
                    "status_before": &order.status,
                    "status_after": "canceled"
                }),
            )
            .await?;
            let message = build_execution_floor_blocked_notification_message(order, best_ask);
            maybe_send_guard_transition_notification(
                repo,
                order,
                candidate_reason.as_str(),
                order.notify_on_execution_floor_blocked,
                "execution_floor_blocked",
                &message,
            )
            .await?;
            maybe_abort_trade_builder_pair_session_for_terminal_order(
                repo,
                order,
                "pair_counter_execution_floor_blocked",
            )
            .await?;
        }
        warn!(
            run_id,
            builder_order_id = order.id,
            market = %order.market_slug,
            token_id = %order.token_id,
            current_price,
            best_ask,
            desired_price,
            best_ask_floor_price = ?order.best_ask_floor_price,
            waiting = should_wait,
            reason_code,
            "TRADE_BUILDER_ORDER_EXECUTION_FLOOR_BLOCKED"
        );
        return Ok(());
    }

    if max_price_blocked {
        let candidate_reason = build_guard_notification_reason("max_price", "above_max_price");
        append_trade_builder_guard_diagnostics_event(
            repo,
            order,
            current_price,
            desired_price,
            best_ask,
            trigger_price_guard_payload,
            execution_floor_payload,
            max_price_payload,
            Some("max_price"),
            if retry_on_max_price_block {
                "waiting"
            } else {
                "blocked"
            },
            "above_max_price",
        )
        .await?;
        record_adaptive_low_gap_above_max_or_warn(repo, order, submit_started_at.timestamp_millis()).await;
        if retry_on_max_price_block {
            let notification_message = order.notify_on_max_price_blocked.then(|| {
                build_max_price_waiting_notification_message(
                    order,
                    current_price,
                    max_price_reference,
                    max_price_reference_source,
                    Some("above_max_price"),
                )
            });
            transition_trade_builder_order_to_guard_waiting(
                repo,
                order,
                "above_max_price",
                "max_price_waiting",
                &json!({
                    "reason_code": "above_max_price",
                    "reason_message": "Max price guard moved the order into waiting mode.",
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "max_price": order.max_price,
                    "current_price": current_price,
                    "desired_price": desired_price,
                    "reference_price": max_price_reference,
                    "reference_price_source": max_price_reference_source,
                    "status_before": &order.status,
                    "status_after": TRADE_BUILDER_GUARD_BLOCKED_STATUS
                }),
                remaining_usdc,
                remaining_qty,
                Some(candidate_reason.as_str()),
                order
                    .notify_on_max_price_blocked
                    .then_some("max_price_waiting"),
                notification_message,
            )
            .await?;
        } else {
            repo.set_trade_builder_order_status(order.id, "canceled", Some("above_max_price"))
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "max_price_blocked",
                &json!({
                    "reason_code": "above_max_price",
                    "reason_message": "Reference price would exceed the configured max price.",
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "max_price": order.max_price,
                    "current_price": current_price,
                    "desired_price": desired_price,
                    "reference_price": max_price_reference,
                    "reference_price_source": max_price_reference_source,
                    "status_before": &order.status,
                    "status_after": "canceled"
                }),
            )
            .await?;
            if let Some((notification_type, message)) = build_max_price_blocked_notification(
                order,
                current_price,
                max_price_reference,
                max_price_reference_source,
            ) {
                maybe_send_guard_transition_notification(
                    repo,
                    order,
                    candidate_reason.as_str(),
                    true,
                    notification_type,
                    &message,
                )
                .await?;
            }
            maybe_abort_trade_builder_pair_session_for_terminal_order(
                repo,
                order,
                "pair_counter_above_max_price",
            )
            .await?;
        }
        warn!(
            run_id,
            builder_order_id = order.id,
            market = %order.market_slug,
            token_id = %order.token_id,
            current_price,
            desired_price,
            reference_price = max_price_reference,
            reference_price_source = max_price_reference_source,
            max_price = ?order.max_price,
            waiting = retry_on_max_price_block,
            reason_code = "above_max_price",
            "TRADE_BUILDER_ORDER_MAX_PRICE_BLOCKED"
        );
        return Ok(());
    }

    if maybe_block_live_gap_collector_submit_revalidation(
        repo, client, order, remaining_usdc, remaining_qty,
    )
    .await?
    {
        return Ok(());
    }
    deferred_submit_events.defer_guard_passed(
        current_price,
        desired_price,
        best_ask,
        trigger_price_guard_payload,
        execution_floor_payload,
        max_price_payload,
    );
    let guard_eval_ms = guard_eval_started.elapsed().as_millis() as i64;

    let risk = risk_gate_manual_order(
        repo,
        run_id,
        cfg,
        Some(order.user_id),
        order.trade_id,
        proposed_notional_usdc,
        limits,
        policy,
    )
    .await?;
    if !matches!(risk, RiskDecision::Allow) {
        deferred_submit_events.flush(repo, order).await?;
        repo.set_trade_builder_order_status(order.id, "blocked", Some("risk_block"))
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "blocked_by_risk",
            &json!({
                "reason_code": "risk_blocked",
                "reason_message": "Order blocked by risk policy.",
                "decision": format!("{risk:?}"),
                "market_slug": &order.market_slug,
                "token_id": &order.token_id,
                "trigger_condition": order.trigger_condition.as_deref(),
                "trigger_price": order.trigger_price,
                "current_price": current_price
            }),
        )
        .await?;
        warn!(
            run_id,
            builder_order_id = order.id,
            market = %order.market_slug,
            token_id = %order.token_id,
            reason_code = "risk_blocked",
            decision = %format!("{risk:?}"),
            current_price,
            "TRADE_BUILDER_ORDER_BLOCKED"
        );
        return Ok(());
    }

    let requested_share_qty = if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        Some(size)
    } else {
        None
    };
    let optimistic_exit_submit = size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
        && trade_builder_should_use_optimistic_exit_submit(order);
    let optimistic_exit_stage =
        optimistic_exit_submit.then(|| trade_builder_current_exit_submit_stage(order));
    let mut available_qty = prefetched_available_qty;
    let mut submit_partial_visible_inventory = false;
    let mut submit_size = size;
    let mut submit_remaining_usdc = remaining_usdc;
    let mut submit_remaining_qty = remaining_qty;

    if optimistic_exit_submit
        && optimistic_exit_stage == Some(TradeBuilderExitSubmitStage::DynamicGross)
    {
        if let Some(estimated) = trade_builder_estimated_visible_exit_qty(order, size) {
            if estimated.submit_qty < submit_size {
                submit_size = estimated.submit_qty;
                submit_remaining_qty = Some(estimated.submit_qty);
                submit_remaining_usdc = Some((estimated.submit_qty * desired_price).max(0.0));
                deferred_submit_events.defer_order_event(
                    "dynamic_gross_fee_adjusted",
                    json!({
                        "submit_kind": "submit",
                        "original_qty": size,
                        "adjusted_qty": estimated.submit_qty,
                        "estimated_fee_qty": estimated.estimated_fee_qty,
                        "execution_price": estimated.execution_price,
                        "fee_rate_bps": estimated.fee_rate_bps,
                        "buffer_qty": trade_builder_exit_qty_buffer(order.target_qty.unwrap_or(size)),
                    }),
                );
            }
        }
    }

    if order.side == "sell"
        && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
        && !optimistic_exit_submit
    {
        if available_qty.is_none() {
            match client.available_token_qty(&order.token_id).await {
                Ok(quantity) => {
                    available_qty = quantity;
                }
                Err(err) => {
                    warn!(
                        run_id,
                        builder_order_id = order.id,
                        token_id = %order.token_id,
                        error = %err,
                        "TRADE_BUILDER_EXIT_INVENTORY_CHECK_FAILED"
                    );
                }
            }
        }
        let Some(inventory_resolution) =
            resolve_trade_builder_exit_inventory(order, size, available_qty)
        else {
            let reason = "exit inventory not yet available";
            mark_trade_builder_inventory_pending(
                repo,
                order,
                reason,
                current_price,
                size,
                available_qty,
            )
            .await?;
            deferred_submit_events.flush(repo, order).await?;
            return Ok(());
        };
        if let (Some(visible), Some(local_fallback_qty)) = (
            inventory_resolution.visible_qty,
            inventory_resolution.local_fallback_qty,
        ) {
            if (visible - local_fallback_qty).abs() >= 0.02 {
                repo.append_trade_builder_order_event(
                    order.id,
                    "inventory_source_mismatch",
                    &json!({
                        "visible_qty": visible,
                        "local_fallback_qty": local_fallback_qty,
                        "requested_qty": size
                    }),
                )
                .await?;
            }
        }
        if inventory_resolution.local_fallback_qty.is_some()
            && inventory_resolution.visible_qty.unwrap_or_default() <= 0.0
        {
            repo.append_trade_builder_order_event(
                order.id,
                "local_inventory_fallback_used",
                &json!({
                    "requested_qty": size,
                    "submit_qty": inventory_resolution.submit_qty,
                    "visible_qty": inventory_resolution.visible_qty,
                    "estimated_fee_qty": inventory_resolution.local_fallback_fee_qty,
                    "entry_price": inventory_resolution.local_fallback_entry_price,
                    "fee_rate_bps": inventory_resolution.local_fallback_fee_rate_bps
                }),
            )
            .await?;
        }
        submit_partial_visible_inventory = inventory_resolution.submit_partial_visible_inventory;
        submit_size = inventory_resolution.submit_qty;
        submit_remaining_qty = Some(inventory_resolution.submit_qty);
        submit_remaining_usdc = Some((inventory_resolution.submit_qty * desired_price).max(0.0));
    } else if order.side == "sell"
        && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
        && optimistic_exit_stage == Some(TradeBuilderExitSubmitStage::VisibleInventory)
    {
        if available_qty.is_none() {
            match client.available_token_qty(&order.token_id).await {
                Ok(quantity) => {
                    available_qty = quantity;
                }
                Err(err) => {
                    warn!(
                        run_id,
                        builder_order_id = order.id,
                        token_id = %order.token_id,
                        error = %err,
                        "TRADE_BUILDER_EXIT_INVENTORY_CHECK_FAILED"
                    );
                }
            }
        }
        let Some(visible_inventory_resolution) =
            resolve_trade_builder_visible_inventory_submit(size, available_qty)
        else {
            schedule_trade_builder_exit_sell_retry(
                repo,
                order,
                "submit_retry_scheduled",
                "exit inventory not yet available",
                current_price,
                desired_price,
                requested_share_qty,
                available_qty,
                Some(size),
                None,
                optimistic_exit_stage,
                optimistic_exit_stage,
            )
            .await?;
            deferred_submit_events.flush(repo, order).await?;
            return Ok(());
        };
        submit_partial_visible_inventory =
            visible_inventory_resolution.submit_partial_visible_inventory;
        submit_size = visible_inventory_resolution.submit_qty;
        submit_remaining_qty = Some(visible_inventory_resolution.submit_qty);
        submit_remaining_usdc =
            Some((visible_inventory_resolution.submit_qty * desired_price).max(0.0));
    }

    let intent = if order.kind == "immediate" {
        "manual_immediate"
    } else {
        "manual_trigger"
    };
    if order.side == "sell" && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        if let Some(clamped_qty) = clamp_trade_builder_visible_share_qty(submit_size, available_qty)
        {
            if clamped_qty < submit_size {
                submit_partial_visible_inventory = true;
                submit_size = clamped_qty;
                submit_remaining_qty = Some(clamped_qty);
                submit_remaining_usdc = Some((clamped_qty * desired_price).max(0.0));
            }
        }
    }
    let normalized_execution_mode = normalize_trade_builder_execution_mode(&order.execution_mode);
    let order_type =
        resolve_trade_builder_submit_order_type(repo, order, normalized_execution_mode).await?;
    let post_only = resolve_trade_builder_submit_post_only(repo, order, order_type).await?;
    let mut client_order_id = format!("tb-{}", Uuid::new_v4());
    let market_spec = match cached_market_spec {
        Some(spec) => Some(spec),
        None => {
            resolve_trade_builder_market_spec_with_client(
                client,
                cfg,
                &order.market_slug,
                &order.token_id,
            )
            .await
        }
    };
    if maybe_apply_trade_builder_pre_submit_book_recheck(
        repo,
        client,
        ws,
        order,
        current_price,
        best_ask,
        order_type,
        size_basis,
        retry_on_trigger_guard_block,
        retry_on_execution_floor_guard_block,
        retry_on_max_price_block,
        &mut desired_price,
        &mut submit_size,
        &mut submit_remaining_usdc,
        &mut submit_remaining_qty,
        &mut deferred_submit_events,
    )
    .await?
    {
        return Ok(());
    }
    desired_price = trade_builder_clamp_buy_limit_price(order, desired_price);
    let min_notional_cap_usdc = order
        .size_usdc
        .max(submit_remaining_usdc.unwrap_or(0.0))
        .max((submit_size * desired_price).max(0.0));
    if let Some(top_up) = trade_builder_marketable_buy_min_notional_top_up(
        &order.side,
        order_type,
        size_basis,
        desired_price,
        submit_size,
        min_notional_cap_usdc,
    ) {
        if top_up.blocked_by_cap {
            let reason = "marketable_buy_min_notional_exceeds_cap";
            repo.set_trade_builder_order_status(order.id, "error", Some(reason))
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                reason,
                &json!({
                    "submit_kind": "submit",
                    "status_before": &order.status,
                    "current_price": current_price,
                    "desired_price": desired_price,
                    "order_type": order_type,
                    "size_basis": size_basis,
                    "original_qty": top_up.original_qty,
                    "adjusted_qty": top_up.adjusted_qty,
                    "original_notional_usdc": top_up.original_notional_usdc,
                    "adjusted_notional_usdc": top_up.adjusted_notional_usdc,
                    "min_notional_usdc": top_up.min_notional_usdc,
                    "cap_usdc": top_up.cap_usdc,
                    "cap_tolerance_usdc": top_up.cap_tolerance_usdc,
                }),
            )
            .await?;
            return Ok(());
        }
        submit_size = top_up.adjusted_qty;
        submit_remaining_qty = Some(top_up.adjusted_qty);
        submit_remaining_usdc = Some(top_up.adjusted_notional_usdc);
        deferred_submit_events.defer_order_event(
            "marketable_buy_min_notional_top_up",
            json!({
                "submit_kind": "submit",
                "status_before": &order.status,
                "current_price": current_price,
                "desired_price": desired_price,
                "order_type": order_type,
                "size_basis": size_basis,
                "original_qty": top_up.original_qty,
                "adjusted_qty": top_up.adjusted_qty,
                "original_notional_usdc": top_up.original_notional_usdc,
                "adjusted_notional_usdc": top_up.adjusted_notional_usdc,
                "min_notional_usdc": top_up.min_notional_usdc,
                "cap_usdc": top_up.cap_usdc,
                "cap_tolerance_usdc": top_up.cap_tolerance_usdc,
            }),
        );
    }
    if maybe_handle_trade_builder_share_submit_below_market_min(
        repo,
        order,
        "submit_deferred_below_market_min",
        "submit",
        current_price,
        desired_price,
        requested_share_qty,
        submit_size,
        available_qty,
        trade_builder_market_min_size(market_spec),
        optimistic_exit_stage,
    )
    .await?
    {
        deferred_submit_events.flush(repo, order).await?;
        return Ok(());
    }

    let req = PlaceOrderRequest {
        market: order.market_slug.clone(),
        token_id: Some(order.token_id.clone()),
        side: order.side.clone(),
        price: desired_price,
        size: submit_size,
        intent: intent.to_string(),
        order_type: order_type.to_string(),
        post_only,
        client_order_id: client_order_id.clone(),
        leg_side: None,
        fee_rate_bps,
        neg_risk: market_spec.is_some_and(|spec| spec.neg_risk),
    };

    if optimistic_exit_submit {
        deferred_submit_events.defer_order_event(
            "optimistic_exit_submit_used",
            json!({
                "submit_kind": "submit",
                "attempt_stage": optimistic_exit_stage.map(TradeBuilderExitSubmitStage::as_str),
                "status_before": &order.status,
                "requested_qty": requested_share_qty,
                "attempted_qty": submit_size,
                "current_price": current_price,
                "desired_price": desired_price,
                "size_basis": size_basis,
                "available_qty": available_qty,
                "precheck_skipped": optimistic_exit_stage != Some(TradeBuilderExitSubmitStage::VisibleInventory),
                "submit_price_source": sell_submit_price.map(|resolution| resolution.source),
                "submit_price_depth_levels_used": sell_submit_price.and_then(|resolution| resolution.depth_levels_used),
                "submit_price_visible_bid_qty": sell_submit_price.and_then(|resolution| resolution.visible_bid_qty),
                "submit_price_requested_qty": sell_submit_price.and_then(|resolution| resolution.requested_qty),
            }),
        );
    }

    let mut fast_stop_loss_retry_payload: Option<Value> = None;
    let mut submit_finished_at_override: Option<DateTime<Utc>> = None;
    let ack = match client.place(&req).await {
        Ok(ack) => ack,
        Err(err) => {
            let mut error_text = err.to_string();
            trade_builder_spawn_decision_log(
                repo,
                order,
                "ORDER_ERROR",
                json!({
                    "submit_attempt_no": order.triggers_fired + 1,
                    "submit_status": "error",
                    "error": &error_text,
                    "side": &order.side,
                    "kind": &order.kind,
                    "intended_price": desired_price,
                    "intended_qty": submit_size,
                    "book_at_submit": {
                        "best_bid": best_bid,
                        "best_ask": best_ask,
                        "estimated_avg_fill": desired_price
                    }
                }),
                TradeBuilderDecisionLogOptions {
                    idempotency_key: Some(format!(
                        "ORDER_ERROR:{}:{}:{}",
                        order.id,
                        order.triggers_fired + 1,
                        error_text.chars().take(48).collect::<String>()
                    )),
                    ..TradeBuilderDecisionLogOptions::default()
                },
            );
            let fast_retry_success = match try_trade_builder_fast_stop_loss_retry(
                client,
                ws,
                order,
                &req,
                order_type,
                size_basis,
                &error_text,
                current_price,
                best_bid,
                last_trade_price,
                requested_share_qty,
            )
            .await
            {
                TradeBuilderFastStopLossRetryOutcome::Success(success) => Some(success),
                TradeBuilderFastStopLossRetryOutcome::Exhausted(failure) => {
                    deferred_submit_events.flush(repo, order).await?;
                    append_trade_builder_fast_stop_loss_retry_attempt_events(
                        repo,
                        order.id,
                        &failure.attempt_events,
                    )
                    .await?;
                    if let Some(last_error_text) = failure.last_error_text {
                        error_text = last_error_text;
                    }
                    None
                }
                TradeBuilderFastStopLossRetryOutcome::NotEligible => {
                    deferred_submit_events.flush(repo, order).await?;
                    None
                }
            };
            if let Some(success) = fast_retry_success {
                deferred_submit_events.flush(repo, order).await?;
                append_trade_builder_fast_stop_loss_retry_attempt_events(
                    repo,
                    order.id,
                    &success.attempt_events,
                )
                .await?;
                desired_price = success.desired_price;
                sell_submit_price = Some(success.sell_submit_price);
                client_order_id = success.client_order_id;
                submit_started_at = success.submit_started_at;
                submit_finished_at_override = Some(success.submit_finished_at);
                fast_stop_loss_retry_payload = Some(success.attempt_payload);
                if let Some(qty) = submit_remaining_qty {
                    submit_remaining_usdc = Some((qty * desired_price).max(0.0));
                }
                success.ack
            } else {
                if trade_builder_error_is_fatal_exchange_rejection(&error_text) {
                    repo.set_trade_builder_order_status(order.id, "error", Some(&error_text))
                        .await?;
                    repo.append_trade_builder_order_event(
                        order.id,
                        "fatal_exchange_rejection",
                        &json!({
                            "error": error_text,
                            "status_before": &order.status,
                            "side": &order.side,
                            "market_slug": &order.market_slug,
                            "token_id": &order.token_id,
                            "attempted_qty": submit_size,
                            "desired_price": desired_price,
                            "neg_risk": req.neg_risk,
                            "order_price_min_tick_size": market_spec.and_then(|spec| spec.order_price_min_tick_size),
                            "order_min_size": market_spec.and_then(|spec| spec.order_min_size),
                            "submit_price_source": sell_submit_price.map(|resolution| resolution.source),
                            "submit_price_depth_levels_used": sell_submit_price.and_then(|resolution| resolution.depth_levels_used),
                            "submit_price_visible_bid_qty": sell_submit_price.and_then(|resolution| resolution.visible_bid_qty),
                            "submit_price_requested_qty": sell_submit_price.and_then(|resolution| resolution.requested_qty),
                        }),
                    )
                    .await?;
                    warn!(
                        run_id,
                        builder_order_id = order.id,
                        market = %order.market_slug,
                        error = %error_text,
                        neg_risk = req.neg_risk,
                        "TRADE_BUILDER_FATAL_EXCHANGE_REJECTION"
                    );
                    maybe_send_trade_builder_system_alert(
                        repo,
                        order,
                        "fatal_exchange_rejection",
                        &error_text,
                    )
                    .await;
                    return Ok(());
                }
                if order.side == "sell"
                    && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
                    && trade_builder_error_indicates_balance_or_allowance(&error_text)
                {
                    if optimistic_exit_submit {
                        let current_attempt_stage =
                            optimistic_exit_stage.unwrap_or(TradeBuilderExitSubmitStage::DynamicGross);
                        let next_attempt_stage =
                            trade_builder_next_optimistic_exit_stage_after_balance_reject(
                                current_attempt_stage,
                            );
                        repo.append_trade_builder_order_event(
                            order.id,
                            "optimistic_exit_balance_rejected",
                            &json!({
                                "reason": error_text,
                                "attempt_stage": current_attempt_stage.as_str(),
                                "next_attempt_stage": next_attempt_stage.as_str(),
                                "status_before": &order.status,
                                "current_price": current_price,
                                "desired_price": desired_price,
                                "requested_qty": requested_share_qty,
                                "attempted_qty": submit_size,
                                "available_qty": available_qty,
                            }),
                        )
                        .await?;
                        schedule_trade_builder_exit_sell_retry(
                            repo,
                            order,
                            "submit_retry_scheduled",
                            &error_text,
                            current_price,
                            desired_price,
                            requested_share_qty,
                            available_qty,
                            Some(submit_size),
                            None,
                            Some(current_attempt_stage),
                            Some(next_attempt_stage),
                        )
                        .await?;
                        return Ok(());
                    }
                    if trade_builder_stop_loss_latched(order) {
                        schedule_trade_builder_exit_sell_retry(
                            repo,
                            order,
                            "submit_retry_scheduled",
                            &error_text,
                            current_price,
                            desired_price,
                            requested_share_qty,
                            available_qty,
                            Some(submit_size),
                            None,
                            None,
                            None,
                        )
                        .await?;
                        return Ok(());
                    }
                    let rechecked_qty = match client.available_token_qty(&order.token_id).await {
                        Ok(quantity) => quantity,
                        Err(recheck_err) => {
                            warn!(
                                run_id,
                                builder_order_id = order.id,
                                token_id = %order.token_id,
                                error = %recheck_err,
                                "TRADE_BUILDER_EXIT_INVENTORY_RECHECK_FAILED"
                            );
                            None
                        }
                    };
                    available_qty = rechecked_qty;
                    if rechecked_qty
                        .and_then(|qty| clamp_trade_builder_visible_share_qty(size, Some(qty)))
                        .is_some()
                    {
                        mark_trade_builder_inventory_pending(
                            repo,
                            order,
                            "exchange rejected sell before inventory synced",
                            current_price,
                            size,
                            rechecked_qty,
                        )
                        .await?;
                        return Ok(());
                    }
                }
                if trade_builder_should_retry_exit_sell(order) {
                    schedule_trade_builder_exit_sell_retry(
                        repo,
                        order,
                        "submit_retry_scheduled",
                        &error_text,
                        current_price,
                        desired_price,
                        requested_share_qty,
                        available_qty,
                        Some(submit_size),
                        None,
                        None,
                        None,
                    )
                    .await?;
                    return Ok(());
                }
                return Err(err);
            }
        }
    };
    let submit_finished_at = submit_finished_at_override.unwrap_or_else(Utc::now);
    deferred_submit_events.flush(repo, order).await?;

    let exchange_order_id = ack
        .exchange_order_id
        .clone()
        .unwrap_or_else(|| ack.client_order_id.clone());
    let normalized_status = normalize_exchange_status(&ack.status);
    let mut raw = json!({
        "builder_order_id": order.id,
        "client_order_id": ack.client_order_id,
        "exchange_order_id": exchange_order_id,
        "status": ack.status,
        "normalized_status": normalized_status,
        "trigger_price": order.trigger_price,
        "max_price": order.max_price,
        "guard_trigger_price": order.guard_trigger_price,
        "best_ask_floor_price": order.best_ask_floor_price,
        "current_price": current_price,
        "best_ask": best_ask,
        "execution_price": desired_price,
        "execution_price_source": market_buy_execution_price
            .map(|resolution| resolution.source)
            .unwrap_or_else(|| sell_submit_price.map(|resolution| resolution.source).unwrap_or("runtime_price")),
        "trigger_reference_price": market_buy_execution_price
            .and_then(|resolution| resolution.trigger_reference_price),
        "submit_price_source": sell_submit_price.map(|resolution| resolution.source),
        "submit_price_depth_levels_used": sell_submit_price.and_then(|resolution| resolution.depth_levels_used),
        "submit_price_visible_bid_qty": sell_submit_price.and_then(|resolution| resolution.visible_bid_qty),
        "submit_price_requested_qty": sell_submit_price.and_then(|resolution| resolution.requested_qty),
        "execution_mode": normalized_execution_mode,
        "order_type": order_type,
        "allow_partial_fill": action_place_order_allow_partial_fill(order_type),
        "size_basis": size_basis,
        "size": submit_size,
        "requested_qty": requested_share_qty,
        "clamped_qty": submit_remaining_qty,
        "partial_visible_inventory_submit": submit_partial_visible_inventory,
        "target_qty": order.target_qty,
        "remaining_qty": submit_remaining_qty,
        "size_mode": trigger_size_mode,
        "trigger_size_value": trigger_size_value,
        "trigger_size_index": trigger_size_index + 1,
        "resolved_size_usdc": resolved_size_usdc,
        "remaining_usdc": submit_remaining_usdc,
        "available_qty": available_qty,
        "fee_rate_bps": fee_rate_bps,
        "reject_reason": ack.reject_reason,
        "raw_status": ack.raw_status,
        "exchange_ts": ack.exchange_ts
    });
    raw.as_object_mut().expect("submitted payload").insert(
        "exit_fast_quote_used".to_string(),
        json!(exit_fast_quote.is_some()),
    );
    raw.as_object_mut().expect("submitted payload").insert(
        "exit_fast_quote_age_ms".to_string(),
        json!(exit_fast_quote.map(|quote| exit_fast_submit_quote_age_ms(quote, submit_started_at))),
    );
    raw.as_object_mut().expect("submitted payload").insert(
        "exit_fast_quote_market_updated_at_ms".to_string(),
        json!(exit_fast_quote.map(|quote| quote.market_updated_at_ms)),
    );
    if let Some(payload) = fast_stop_loss_initial_submit_payload {
        append_trade_builder_fast_stop_loss_initial_submit_payload(
            raw.as_object_mut().expect("submitted payload"),
            payload,
        );
    }
    if let Some(payload) = fast_stop_loss_retry_payload {
        let raw_payload = raw.as_object_mut().expect("submitted payload");
        raw_payload.insert("fast_sl_retry".to_string(), payload.clone());
        for key in [
            "fast_sl_retry_attempt",
            "fresh_quote_age_ms",
            "retry_price",
            "retry_reason",
        ] {
            if let Some(value) = payload.get(key) {
                raw_payload.insert(key.to_string(), value.clone());
            }
        }
    }
    append_trade_builder_submit_telemetry(
        raw.as_object_mut().expect("submitted payload"),
        submit_context,
        &TradeBuilderSubmitTiming {
            submit_started_at,
            submit_finished_at,
            guard_eval_ms,
        },
        Some(&ack),
    );
    let submit_flow_payload = load_trade_builder_latest_flow_payload(repo, order.id).await?;
    trade_builder_append_submitted_telemetry(
        raw.as_object_mut().expect("submitted payload"),
        order,
        submit_flow_payload.as_ref(),
        submit_size,
        desired_price,
    );

    repo.upsert_order_by_exchange_id(
        order.trade_id,
        &exchange_order_id,
        Some(&client_order_id),
        intent,
        &order.side,
        desired_price,
        submit_size,
        normalized_status,
        ack.exchange_ts,
        ack.reject_reason.as_deref(),
        &raw,
    )
    .await?;
    repo.set_trade_builder_order_working_state(
        order.id,
        Some(&exchange_order_id),
        Some(desired_price),
        submit_remaining_usdc,
        submit_remaining_qty,
        normalized_status,
    )
    .await?;
    maybe_persist_trade_builder_submitted_dynamic(repo, run_id, order, submit_size, desired_price)
        .await;
    if submit_partial_visible_inventory {
        repo.append_trade_builder_order_event(
            order.id,
            "partial_visible_inventory_submit",
            &json!({
                "requested_qty": requested_share_qty,
                "available_qty": available_qty,
                "submitted_qty": submit_size,
                "residual_qty_ignored": requested_share_qty.map(|qty| (qty - submit_size).max(0.0)),
            }),
        )
        .await?;
    }
    repo.append_trade_builder_order_event(order.id, "submitted", &raw)
        .await?;
    maybe_record_trade_builder_buy_inventory_baseline(
        repo,
        run_id,
        client,
        order,
        desired_price,
        fee_rate_bps,
    )
    .await;
    trade_builder_spawn_decision_log(
        repo,
        order,
        "ORDER_SUBMITTED",
        json!({
            "submit_attempt_no": order.triggers_fired + 1,
            "submit_status": normalized_status,
            "exchange_order_id": &exchange_order_id,
            "client_order_id": &client_order_id,
            "intended_price": desired_price,
            "intended_qty": submit_size,
            "book_at_submit": {
                "best_bid": best_bid,
                "best_ask": best_ask,
                "estimated_avg_fill": desired_price
            },
            "raw_submitted_payload": raw.clone(),
        }),
        TradeBuilderDecisionLogOptions {
            idempotency_key: Some(format!(
                "ORDER_SUBMITTED:{}:{}",
                order.id,
                order.triggers_fired + 1
            )),
            exchange_order_id: Some(exchange_order_id.clone()),
            ..TradeBuilderDecisionLogOptions::default()
        },
    );
    maybe_send_trade_builder_submitted_notification(
        repo,
        order,
        &raw,
        submit_flow_payload.as_ref(),
    )
    .await?;
    maybe_record_trade_builder_buy_submit_observation(
        repo,
        run_id,
        order,
        &exchange_order_id,
        submit_size,
        desired_price,
        fee_rate_bps,
        normalized_status,
        raw.clone(),
    )
    .await;

    if normalized_status == "filled" {
        let fill_backfill =
            match backfill_trade_builder_fills_for_order(repo, client, order.id, &exchange_order_id)
                .await
            {
                Ok(outcome) => outcome,
                Err(err) => {
                    warn!(
                        builder_order_id = order.id,
                        exchange_order_id = %exchange_order_id,
                        error = %err,
                        "TRADE_BUILDER_IMMEDIATE_FILL_BACKFILL_FAILED"
                    );
                    TradeBuilderFillBackfillOutcome::default()
                }
            };
        let (
            canonical_entry_qty,
            canonical_entry_qty_source,
            actual_fill_qty,
            actual_fill_qty_source,
        ) = resolve_trade_builder_immediate_fill_quantities(order, submit_size, &fill_backfill)?;
        finalize_builder_fill(
            repo,
            cfg,
            ws,
            order,
            &exchange_order_id,
            canonical_entry_qty,
            canonical_entry_qty_source,
            actual_fill_qty,
            desired_price,
            false,
            actual_fill_qty_source,
        )
        .await?;
    }

    Ok(())
}
