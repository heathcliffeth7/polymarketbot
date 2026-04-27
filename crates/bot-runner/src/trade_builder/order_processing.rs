static TRADE_BUILDER_ORDER_PROCESSING_GUARDS: LazyLock<parking_lot::Mutex<HashSet<i64>>> =
    LazyLock::new(|| parking_lot::Mutex::new(HashSet::new()));

struct TradeBuilderOrderProcessingGuard {
    order_id: i64,
}

impl Drop for TradeBuilderOrderProcessingGuard {
    fn drop(&mut self) {
        TRADE_BUILDER_ORDER_PROCESSING_GUARDS
            .lock()
            .remove(&self.order_id);
    }
}

fn try_acquire_trade_builder_order_processing_guard(
    order_id: i64,
) -> Option<TradeBuilderOrderProcessingGuard> {
    let mut guards = TRADE_BUILDER_ORDER_PROCESSING_GUARDS.lock();
    if !guards.insert(order_id) {
        return None;
    }
    Some(TradeBuilderOrderProcessingGuard { order_id })
}

fn trade_builder_order_age_ms(created_at: DateTime<Utc>) -> i64 {
    Utc::now()
        .signed_duration_since(created_at)
        .num_milliseconds()
        .max(0)
}

fn log_trade_builder_submit_trace(
    run_id: i64,
    builder_order_id: i64,
    path: &str,
    guard_acquired: bool,
    processing_ms: i64,
    order_age_ms: i64,
    status: &str,
    error: Option<&str>,
) {
    info!(
        run_id,
        builder_order_id,
        path,
        guard_acquired,
        processing_ms,
        order_age_ms,
        status,
        error = error.unwrap_or(""),
        "IMMEDIATE_SUBMIT_TRACE"
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TradeBuilderEligibilityWindowState {
    Wait,
    Expire,
    Allow,
}

fn classify_trade_builder_order_eligibility_window(
    order: &TradeBuilderOrder,
    now: DateTime<Utc>,
) -> TradeBuilderEligibilityWindowState {
    if order.active_exchange_order_id.is_some() {
        return TradeBuilderEligibilityWindowState::Allow;
    }
    if let Some(eligible_after_at) = order.eligible_after_at {
        if now < eligible_after_at {
            return TradeBuilderEligibilityWindowState::Wait;
        }
    }
    if let Some(eligible_before_at) = order.eligible_before_at {
        if now >= eligible_before_at {
            return TradeBuilderEligibilityWindowState::Expire;
        }
    }
    TradeBuilderEligibilityWindowState::Allow
}

async fn maybe_handle_trade_builder_order_eligibility_window(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    now: DateTime<Utc>,
) -> Result<bool> {
    match classify_trade_builder_order_eligibility_window(order, now) {
        TradeBuilderEligibilityWindowState::Wait => Ok(true),
        TradeBuilderEligibilityWindowState::Expire => {
            repo.set_trade_builder_order_status(order.id, "expired", Some("outside_cycle_window"))
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "expired",
                &json!({
                "reason_code": "outside_cycle_window",
                "reason_message": "Builder order window closed before the order could be submitted.",
                    "eligible_after_at": order.eligible_after_at.as_ref().map(|value| value.to_rfc3339()),
                    "eligible_before_at": order.eligible_before_at.as_ref().map(|value| value.to_rfc3339()),
                    "status_before": &order.status,
                    "status_after": "expired",
                }),
            )
            .await?;
            trade_builder_spawn_decision_log(
                repo,
                order,
                "ORDER_EXPIRED",
                json!({
                    "status_transition_id": format!("{}:outside_cycle_window", order.id),
                    "side": &order.side,
                    "kind": &order.kind,
                    "reason": "outside_cycle_window",
                    "time_alive_ms": Utc::now().signed_duration_since(order.created_at).num_milliseconds().max(0),
                    "eligible_after_at": order.eligible_after_at.as_ref().map(|value| value.to_rfc3339()),
                    "eligible_before_at": order.eligible_before_at.as_ref().map(|value| value.to_rfc3339()),
                }),
                TradeBuilderDecisionLogOptions {
                    idempotency_key: Some(format!("ORDER_EXPIRED:{}:outside_cycle_window", order.id)),
                    ..TradeBuilderDecisionLogOptions::default()
                },
            );
            maybe_send_order_not_filled_notification(
                repo,
                order,
                "outside_cycle_window",
                "Eligible penceresi kapandigi icin emir icra edilemeden expire oldu.",
            )
            .await;
            maybe_abort_trade_builder_pair_session_for_terminal_order(
                repo,
                order,
                "pair_counter_expired",
            )
            .await?;
            Ok(true)
        }
        TradeBuilderEligibilityWindowState::Allow => Ok(false),
    }
}

#[allow(clippy::too_many_arguments)]
async fn try_process_trade_builder_order(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    gamma: &GammaHttpClient,
    ws: &ClobWsClient,
    order: &TradeBuilderOrder,
) -> Result<bool> {
    let order_age_ms = trade_builder_order_age_ms(order.created_at);
    let Some(_guard) = try_acquire_trade_builder_order_processing_guard(order.id) else {
        log_trade_builder_submit_trace(
            run_id,
            order.id,
            "housekeeping",
            false,
            0,
            order_age_ms,
            &order.status,
            None,
        );
        return Ok(false);
    };
    let processing_started = Instant::now();
    let result = process_trade_builder_order(
        repo,
        run_id,
        cfg,
        limits,
        policy,
        client,
        gamma,
        ws,
        order,
        "housekeeping",
    )
    .await;
    let latest_status = repo
        .get_trade_builder_order(order.id)
        .await
        .ok()
        .flatten()
        .map(|latest| latest.status)
        .unwrap_or_else(|| order.status.clone());
    let processing_ms = processing_started.elapsed().as_millis() as i64;
    let error_text = result.as_ref().err().map(|err| err.to_string());
    log_trade_builder_submit_trace(
        run_id,
        order.id,
        "housekeeping",
        true,
        processing_ms,
        order_age_ms,
        &latest_status,
        error_text.as_deref(),
    );
    result?;
    Ok(true)
}

async fn try_immediate_submit_single_builder_order(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    ws: &ClobWsClient,
    client: SharedOrderExecutor,
    order_id: i64,
    path: &'static str,
) -> Result<()> {
    let policy = DefaultRiskPolicy;
    let limits = to_risk_limits(cfg);
    let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
    try_process_trade_builder_order_by_id(
        repo,
        run_id,
        cfg,
        &limits,
        &policy,
        &gamma,
        ws,
        client.as_ref(),
        order_id,
        path,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn try_process_trade_builder_order_by_id(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    gamma: &GammaHttpClient,
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    order_id: i64,
    path: &'static str,
) -> Result<()> {
    let Some(order) = repo.get_trade_builder_order(order_id).await? else {
        return Ok(());
    };
    let order_age_ms = trade_builder_order_age_ms(order.created_at);
    let Some(_guard) = try_acquire_trade_builder_order_processing_guard(order_id) else {
        log_trade_builder_submit_trace(
            run_id,
            order_id,
            path,
            false,
            0,
            order_age_ms,
            &order.status,
            None,
        );
        return Ok(());
    };
    let processing_started = Instant::now();
    let result = Box::pin(process_trade_builder_order(
        repo, run_id, cfg, limits, policy, client, gamma, ws, &order, path,
    ))
    .await;
    let latest_status = repo
        .get_trade_builder_order(order_id)
        .await
        .ok()
        .flatten()
        .map(|latest| latest.status)
        .unwrap_or_else(|| order.status.clone());
    let processing_ms = processing_started.elapsed().as_millis() as i64;
    let error_text = result.as_ref().err().map(|err| err.to_string());
    log_trade_builder_submit_trace(
        run_id,
        order_id,
        path,
        true,
        processing_ms,
        order_age_ms,
        &latest_status,
        error_text.as_deref(),
    );
    result
}

async fn process_trade_builder_order(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    gamma: &GammaHttpClient,
    ws: &ClobWsClient,
    order: &TradeBuilderOrder,
    submit_path: &'static str,
) -> Result<()> {
    let Some(mut order) = repo.get_trade_builder_order(order.id).await? else {
        return Ok(());
    };
    let retryable_error =
        order.status == "error" && trade_builder_should_retry_after_processing_error(&order);
    if !is_trade_builder_order_processable_status(&order.status) && !retryable_error {
        return Ok(());
    }

    if order.triggers_fired >= order.max_triggers && order.status != "completed" {
        repo.set_trade_builder_order_status(order.id, "completed", None)
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "max_trigger_reached",
            &json!({
                "triggers_fired": order.triggers_fired,
                "max_triggers": order.max_triggers
            }),
        )
        .await?;
        if let Ok(Some(unblocked_id)) = repo
            .unblock_next_trade_builder_order(order.trade_id, &order.token_id)
            .await
        {
            info!(
                builder_order_id = order.id,
                unblocked_order_id = unblocked_id,
                "TRADE_BUILDER_DCA_NEXT_LEVEL_UNBLOCKED"
            );
        }
        return Ok(());
    }

    let now = Utc::now();
    if let Some(expires_at) = order.expires_at {
        if expires_at <= now {
            if order.status != "expired" && order.status != "completed" {
                repo.clear_trade_builder_active_exchange_order(order.id, "expired")
                    .await?;
                repo.append_trade_builder_order_event(
                    order.id,
                    "expired",
                    &json!({ "expires_at": expires_at }),
                )
                .await?;
                trade_builder_spawn_decision_log(
                    repo,
                    &order,
                    "ORDER_EXPIRED",
                    json!({
                        "status_transition_id": format!("{}:ttl_expired", order.id),
                        "side": &order.side,
                        "kind": &order.kind,
                        "reason": "ttl_expired",
                        "expires_at": expires_at.to_rfc3339(),
                        "time_alive_ms": Utc::now().signed_duration_since(order.created_at).num_milliseconds().max(0),
                    }),
                    TradeBuilderDecisionLogOptions {
                        idempotency_key: Some(format!("ORDER_EXPIRED:{}:ttl_expired", order.id)),
                        ..TradeBuilderDecisionLogOptions::default()
                    },
                );
                maybe_send_order_not_filled_notification(
                    repo,
                    &order,
                    "ttl_expired",
                    "Sure asildi, emir icra edilemeden expire oldu.",
                )
                .await;
                maybe_abort_trade_builder_pair_session_for_terminal_order(
                    repo,
                    &order,
                    "pair_counter_ttl_expired",
                )
                .await?;
            }
            return Ok(());
        }
    }

    if maybe_handle_trade_builder_order_eligibility_window(repo, &order, now).await? {
        return Ok(());
    }

    if maybe_expire_trade_builder_stale_order(repo, gamma, &order).await? {
        maybe_abort_trade_builder_pair_session_for_terminal_order(
            repo,
            &order,
            "pair_counter_stale_market_cycle",
        )
        .await?;
        return Ok(());
    }

    if maybe_apply_trade_builder_pair_lock_runtime(repo, &mut order, now).await? {
        return Ok(());
    }

    let previous_price = order.last_seen_price;
    let runtime_price_fetch_started = Instant::now();
    let fresh_runtime_snapshot = trade_builder_runtime_snapshot_from_order(&order)
        .filter(|snapshot| trade_builder_runtime_snapshot_is_fresh(snapshot, now));
    let snapshot_age_ms = fresh_runtime_snapshot
        .as_ref()
        .map(|snapshot| trade_builder_runtime_snapshot_age_ms(snapshot, now));
    let use_book_aware_runtime_price = trade_builder_requires_book_aware_runtime_price(&order);
    let runtime_price_fetch = if let Some(snapshot) = fresh_runtime_snapshot.as_ref() {
        if let Some(runtime_price) = trade_builder_runtime_price_from_snapshot(snapshot) {
            TradeBuilderRuntimePriceFetch::Resolved(runtime_price)
        } else if use_book_aware_runtime_price {
            resolve_trade_builder_fast_runtime_price(ws, client, &order).await?
        } else {
            resolve_trade_builder_runtime_price(ws, client, &order).await?
        }
    } else if use_book_aware_runtime_price {
        resolve_trade_builder_fast_runtime_price(ws, client, &order).await?
    } else {
        resolve_trade_builder_runtime_price(ws, client, &order).await?
    };
    let runtime_price_fetch_ms = runtime_price_fetch_started.elapsed().as_millis() as i64;
    let runtime_price = match runtime_price_fetch {
        TradeBuilderRuntimePriceFetch::Resolved(runtime_price) => runtime_price,
        TradeBuilderRuntimePriceFetch::Retry { error_text } => {
            repo.append_trade_builder_order_event(
                order.id,
                "price_unavailable_retry",
                &json!({
                    "reason_code": "runtime_price_unavailable",
                    "reason_message": "Runtime price was unavailable and no fallback price was present.",
                    "status": &order.status,
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "previous_price": previous_price,
                    "working_price": order.working_price,
                    "error": error_text,
                }),
            )
            .await?;
            return Ok(());
        }
    };
    let mut runtime_price = runtime_price;
    if order.best_ask_floor_price.is_some() && runtime_price.best_ask.is_none() {
        match client.best_bid_ask(&order.token_id).await {
            Ok((_, Some(best_ask)))
                if best_ask.is_finite() && best_ask > 0.0 && best_ask <= 1.0 =>
            {
                runtime_price.best_ask = Some(clamp_probability(best_ask));
            }
            Ok(_) => {}
            Err(err) => {
                let mut warnings = Vec::new();
                if let Some(existing) = runtime_price.runtime_warning.take() {
                    warnings.push(existing);
                }
                warnings.push(format!("best_ask_backfill: {err}"));
                runtime_price.runtime_warning = trade_builder_runtime_warning(warnings);
            }
        }
    }
    let trigger_eval_price = trade_builder_trigger_eval_price_for_order(&order, &runtime_price);
    let execution_price = trade_builder_execution_price_for_order(&order, &runtime_price);
    let persisted_last_seen_price =
        trade_builder_last_seen_price_for_order(&order, trigger_eval_price, execution_price);
    let ptb_stop_loss_evaluation = trade_builder_evaluate_ptb_stop_loss(&order);
    if let Some(reference_price) = ptb_stop_loss_evaluation
        .as_ref()
        .and_then(|evaluation| trade_builder_ptb_reference_price_persist_candidate(&order, evaluation))
    {
        repo.set_trade_builder_order_ptb_reference_price(order.id, reference_price)
            .await?;
        order.ptb_reference_price = Some(reference_price);
    }
    let effective_trigger_current_price = ptb_stop_loss_evaluation
        .as_ref()
        .and_then(|evaluation| evaluation.current_chainlink_price)
        .unwrap_or(trigger_eval_price);
    if let Some(runtime_warning) = runtime_price.runtime_warning.as_deref() {
        repo.append_trade_builder_order_event(
            order.id,
            "runtime_price_fallback_used",
            &json!({
                "source": runtime_price.source,
                "current_price": execution_price,
                "trigger_eval_price": trigger_eval_price,
                "previous_price": previous_price,
                "working_price": order.working_price,
                "sl_trigger_price_mode": order.sl_trigger_price_mode.as_deref(),
                "runtime_warning": runtime_warning,
                "best_bid": runtime_price.best_bid,
                "best_ask": runtime_price.best_ask,
                "last_trade_price": runtime_price.last_trade_price,
            }),
        )
        .await?;
    }
    let sl_preemption =
        maybe_preempt_trade_builder_take_profit_for_stop_loss(repo, cfg, &mut order, &runtime_price)
            .await?;
    if sl_preemption.tp_preempted {
        let mut ready_sl_futures = sl_preemption
            .ready_sl_order_ids
            .iter()
            .copied()
            .map(|sl_order_id| async move {
                let result = try_process_trade_builder_order_by_id(
                    repo,
                    run_id,
                    cfg,
                    limits,
                    policy,
                    gamma,
                    ws,
                    client,
                    sl_order_id,
                    "tp_preempted_sl",
                )
                .await;
                (sl_order_id, result)
            })
            .collect::<FuturesUnordered<_>>();
        while let Some((sl_order_id, result)) = ready_sl_futures.next().await {
            if let Err(err) = result {
                warn!(
                    run_id,
                    builder_order_id = sl_order_id,
                    preempted_by_order_id = order.id,
                    error = %err,
                    "TP_PREEMPTED_SL_IMMEDIATE_SUBMIT_FAILED"
                );
            }
        }
    }
    if sl_preemption.tp_preempted && order.active_exchange_order_id.is_none() {
        repo.set_trade_builder_last_seen_price(order.id, persisted_last_seen_price)
            .await?;
        order.last_seen_price = Some(persisted_last_seen_price);
        return Ok(());
    }

    if let Some(exchange_order_id) = order.active_exchange_order_id.as_deref() {
        reconcile_trade_builder_open_order(
            repo,
            run_id,
            cfg,
            client,
            ws,
            &order,
            exchange_order_id,
            execution_price,
            runtime_price.best_bid,
            runtime_price.best_ask,
            runtime_price.last_trade_price,
        )
        .await?;
        repo.set_trade_builder_last_seen_price(order.id, persisted_last_seen_price)
            .await?;
        order.last_seen_price = Some(persisted_last_seen_price);
        return Ok(());
    }

    if order.status == "canceled_requested" {
        let cancel_reason = order.last_error.as_deref().unwrap_or("user_request");
        repo.set_trade_builder_order_status(order.id, "canceled", None)
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "canceled_without_open_order",
            &json!({ "reason": cancel_reason }),
        )
        .await?;
        return Ok(());
    }

    if trade_builder_is_stop_loss_child(&order) {
        if let Some(mode) = order.sl_trigger_price_mode.as_deref() {
            if sl_trigger_eval_price_for_mode(mode, &runtime_price).is_none() {
                repo.append_trade_builder_order_event(
                    order.id,
                    "selected_trigger_source_missing",
                    &json!({
                        "sl_trigger_price_mode": mode,
                        "best_bid": runtime_price.best_bid,
                        "last_trade_price": runtime_price.last_trade_price,
                        "status": &order.status,
                    }),
                )
                .await?;
                repo.set_trade_builder_last_seen_price(order.id, persisted_last_seen_price)
                    .await?;
                order.last_seen_price = Some(persisted_last_seen_price);
                return Ok(());
            }
        }
    }

    let trigger_evaluation = if let Some(evaluation) = ptb_stop_loss_evaluation.as_ref() {
        TradeBuilderTriggerEvaluation {
            should_trigger: evaluation.should_trigger,
            first_tick_threshold_used: evaluation.should_trigger && previous_price.is_none(),
        }
    } else {
        evaluate_trade_builder_order_trigger(&order, previous_price, trigger_eval_price)
    };
    if !trigger_evaluation.should_trigger {
        if order.kind == "conditional"
            && order.last_error.as_deref() == Some("composite_bid_confirmation_waiting")
        {
            repo.set_trade_builder_order_last_error(order.id, None)
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "composite_bid_confirmation_released",
                &json!({
                    "reason_code": "trigger_no_longer_valid",
                    "trigger_eval_price": trigger_eval_price,
                    "trigger_price": order.trigger_price,
                    "best_bid": runtime_price.best_bid,
                    "last_trade_price": runtime_price.last_trade_price,
                    "source": "housekeeping",
                }),
            )
            .await?;
            order.last_error = None;
        }
        if order.kind == "conditional" {
            info!(
                run_id,
                builder_order_id = order.id,
                market = %order.market_slug,
                token_id = %order.token_id,
                trigger_condition = ?order.trigger_condition,
                trigger_price = ?order.trigger_price,
                previous_price = ?previous_price,
                current_price = effective_trigger_current_price,
                execution_price,
                sl_trigger_price_mode = ?order.sl_trigger_price_mode,
                order_status = %order.status,
                reason_code = "trigger_not_crossed",
                "TRADE_BUILDER_TRIGGER_NOT_MET"
            );
        }
        if order.kind == "conditional" && order.status == "inventory_pending" {
            repo.set_trade_builder_order_status(order.id, "armed", None)
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "inventory_pending_released",
                &json!({
                    "reason_code": "trigger_recheck_failed",
                    "reason_message": "Exit trigger no longer valid while inventory was pending.",
                    "side": &order.side,
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "previous_price": previous_price,
                    "current_price": trigger_eval_price,
                    "execution_price": execution_price,
                    "trigger_eval_price": trigger_eval_price,
                    "sl_trigger_price_mode": order.sl_trigger_price_mode.as_deref(),
                    "best_bid": runtime_price.best_bid,
                    "last_trade_price": runtime_price.last_trade_price,
                    "status_before": &order.status,
                    "status_after": "armed"
                }),
            )
            .await?;
            return Ok(());
        }
        if order.kind == "conditional" && order.status == TRADE_BUILDER_GUARD_BLOCKED_STATUS {
            repo.set_trade_builder_order_status(order.id, "armed", None)
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "guard_waiting_released",
                &json!({
                    "reason_code": "trigger_recheck_failed",
                    "reason_message": "Guard wait ended because the trigger condition is no longer valid.",
                    "side": &order.side,
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "previous_price": previous_price,
                    "current_price": trigger_eval_price,
                    "execution_price": execution_price,
                    "trigger_eval_price": trigger_eval_price,
                    "sl_trigger_price_mode": order.sl_trigger_price_mode.as_deref(),
                    "best_bid": runtime_price.best_bid,
                    "last_trade_price": runtime_price.last_trade_price,
                    "status_before": &order.status,
                    "status_after": "armed"
                }),
            )
            .await?;
            return Ok(());
        }
        if order.kind == "conditional" && order.status == "pending" {
            let mut trigger_not_met_payload = json!({
                "reason_code": "trigger_not_crossed",
                "reason_message": "Trigger condition has not crossed yet.",
                "side": &order.side,
                "market_slug": &order.market_slug,
                "token_id": &order.token_id,
                "trigger_condition": order.trigger_condition.as_deref(),
                "trigger_price": order.trigger_price,
                "previous_price": previous_price,
                "current_price": effective_trigger_current_price,
                "execution_price": execution_price,
                "trigger_eval_price": trigger_eval_price,
                "sl_trigger_price_mode": order.sl_trigger_price_mode.as_deref(),
                "best_bid": runtime_price.best_bid,
                "last_trade_price": runtime_price.last_trade_price,
                "status_before": &order.status,
                "status_after": "armed"
            });
            if let (Some(payload), Some(evaluation)) = (
                trigger_not_met_payload.as_object_mut(),
                ptb_stop_loss_evaluation.as_ref(),
            ) {
                append_trade_builder_ptb_stop_loss_payload(payload, evaluation);
            }
            repo.set_trade_builder_order_status(order.id, "armed", None)
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "trigger_not_met",
                &trigger_not_met_payload,
            )
            .await?;
        }
        repo.set_trade_builder_last_seen_price(order.id, persisted_last_seen_price)
            .await?;
        order.last_seen_price = Some(persisted_last_seen_price);
        return Ok(());
    }

    if trigger_evaluation.first_tick_threshold_used {
        repo.append_trade_builder_order_event(
            order.id,
            "trigger_first_tick_threshold_used",
            &json!({
                "status": &order.status,
                "side": &order.side,
                "market_slug": &order.market_slug,
                "token_id": &order.token_id,
                "trigger_condition": order.trigger_condition.as_deref(),
                "trigger_price": order.trigger_price,
                "previous_price": previous_price,
                "current_price": effective_trigger_current_price,
                "execution_price": execution_price
            }),
        )
        .await?;
    }
    repo.set_trade_builder_last_seen_price(order.id, persisted_last_seen_price)
        .await?;
    order.last_seen_price = Some(persisted_last_seen_price);
    info!(
        run_id,
        builder_order_id = order.id,
        market = %order.market_slug,
        token_id = %order.token_id,
        trigger_condition = ?order.trigger_condition,
        trigger_price = ?order.trigger_price,
        previous_price = ?previous_price,
        current_price = effective_trigger_current_price,
        execution_price,
        sl_trigger_price_mode = ?order.sl_trigger_price_mode,
        order_status = %order.status,
        "TRADE_BUILDER_TRIGGER_CONDITION_MET"
    );
    if should_skip_trade_builder_composite_sl_bid_confirmation(&order, &runtime_price) {
        let already_waiting =
            order.last_error.as_deref() == Some("composite_bid_confirmation_waiting");
        if !already_waiting {
            repo.set_trade_builder_order_last_error(
                order.id,
                Some("composite_bid_confirmation_waiting"),
            )
            .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "composite_bid_confirmation_waiting",
                &json!({
                    "trigger_price": order.trigger_price,
                    "trigger_eval_price": trigger_eval_price,
                    "best_bid": runtime_price.best_bid,
                    "last_trade_price": runtime_price.last_trade_price,
                    "source": "housekeeping",
                }),
            )
            .await?;
            order.last_error = Some("composite_bid_confirmation_waiting".to_string());
        }
        info!(
            run_id,
            builder_order_id = order.id,
            market = %order.market_slug,
            best_bid = ?runtime_price.best_bid,
            trigger_price = ?order.trigger_price,
            trigger_eval_price,
            last_trade_price = ?runtime_price.last_trade_price,
            already_waiting,
            "SL_BID_CONFIRMATION_GUARD_WAITING"
        );
        return Ok(());
    }
    if order.last_error.as_deref() == Some("composite_bid_confirmation_waiting") {
        repo.set_trade_builder_order_last_error(order.id, None)
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "composite_bid_confirmation_confirmed",
            &json!({
                "trigger_price": order.trigger_price,
                "best_bid": runtime_price.best_bid,
                "last_trade_price": runtime_price.last_trade_price,
                "source": "housekeeping",
            }),
        )
        .await?;
        order.last_error = None;
        info!(
            run_id,
            builder_order_id = order.id,
            market = %order.market_slug,
            best_bid = ?runtime_price.best_bid,
            trigger_price = ?order.trigger_price,
            "SL_BID_CONFIRMATION_CONFIRMED"
        );
    }
    maybe_latch_trade_builder_stop_loss(
        repo,
        &mut order,
        effective_trigger_current_price,
        ptb_stop_loss_evaluation.as_ref(),
    )
    .await?;
    if let Some(evaluation) = ptb_stop_loss_evaluation
        .as_ref()
        .filter(|evaluation| evaluation.should_trigger)
    {
        trade_builder_spawn_ptb_stop_loss_triggered_log(
            repo,
            &order,
            evaluation,
            execution_price,
        );
    }
    if order.active_exchange_order_id.is_none() {
        let _ = maybe_dispatch_trade_builder_parallel_exit_batch(
            repo,
            run_id,
            ws,
            &order,
            &runtime_price,
            submit_path,
        )
        .await?;
    }
    let fee_rate_bps = resolve_trade_builder_order_fee_rate_bps(repo, client, &mut order).await?;

    let size_basis = normalize_trade_builder_size_basis(&order.size_basis);
    let (
        resolved_size_usdc,
        trigger_size_mode,
        trigger_size_value,
        exhausted_trigger_sizes,
        trigger_size_index,
    ) = if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        (
            order.size_usdc,
            None,
            None,
            false,
            order.triggers_fired.max(0) as usize,
        )
    } else {
        resolve_trade_builder_next_trigger_size_usdc(repo, &order).await?
    };
    if exhausted_trigger_sizes {
        repo.set_trade_builder_order_status(order.id, "completed", None)
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "trigger_size_exhausted",
            &json!({
                "triggers_fired": order.triggers_fired,
                "max_triggers": order.max_triggers,
                "next_trigger_index": trigger_size_index + 1
            }),
        )
        .await?;
        return Ok(());
    }

    submit_trade_builder_trigger_order(
        repo,
        run_id,
        cfg,
        limits,
        policy,
        client,
        ws,
        &mut order,
        execution_price,
        runtime_price.best_bid,
        runtime_price.best_ask,
        runtime_price.last_trade_price,
        fee_rate_bps,
        resolved_size_usdc,
        trigger_size_mode,
        trigger_size_value,
        trigger_size_index,
        &TradeBuilderSubmitAttemptContext {
            submit_path,
            runtime_price_fetch_ms,
            snapshot_age_ms,
        },
    )
    .await
}

fn should_skip_trade_builder_composite_sl_bid_confirmation(
    order: &TradeBuilderOrder,
    runtime_price: &TradeBuilderRuntimePrice,
) -> bool {
    if !trade_builder_is_stop_loss_child(order)
        || !matches!(
            order.sl_trigger_price_mode.as_deref(),
            Some("composite" | "composite_safe")
        )
    {
        return false;
    }
    let Some(trigger_price) = order.trigger_price else {
        return false;
    };
    match runtime_price.best_bid {
        Some(best_bid) => best_bid > trigger_price,
        None => true,
    }
}

#[cfg(test)]
mod eligibility_window_tests {
    use super::*;

    fn test_entry_order() -> TradeBuilderOrder {
        TradeBuilderOrder {
            id: 1,
            trade_id: 77,
            user_id: 1,
            kind: "immediate".to_string(),
            status: "pending".to_string(),
            market_slug: "btc-updown-5m-1773319200".to_string(),
            token_id: "tok-up".to_string(),
            outcome_label: "Up".to_string(),
            side: "buy".to_string(),
            execution_mode: "limit".to_string(),
            trigger_condition: None,
            trigger_price: None,
            max_price: None,
            size_basis: TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC.to_string(),
            size_usdc: 5.0,
            target_qty: None,
            min_price_distance_cent: 1.0,
            expires_at: None,
            eligible_after_at: None,
            eligible_before_at: None,
            max_triggers: 1,
            triggers_fired: 0,
            active_exchange_order_id: None,
            remaining_size: None,
            remaining_qty: None,
            working_price: None,
            last_seen_price: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            parent_order_id: None,
            origin_flow_definition_id: None,
            origin_flow_run_id: None,
            origin_flow_node_key: None,
            pair_session_id: None,
            pair_leg_role: None,
            tp_enabled: false,
            tp_price: None,
            tp_rules_json: Vec::new(),
            sl_enabled: false,
            sl_price: None,
            sl_rules_json: Vec::new(),
            time_exit_rules_json: Vec::new(),
            filled_qty: 0.0,
            fee_rate_bps: 0,
            trigger_latched: false,
            trigger_latched_reason: None,
            trigger_latched_at: None,
            submitted_dynamic_qty: None,
            submitted_dynamic_price: None,
            runtime_snapshot_json: None,
            fresh_submit_lease_until: None,
            guard_trigger_price: None,
            best_ask_floor_price: None,
            retry_on_trigger_guard_block: false,
            retry_on_execution_floor_guard_block: false,
            retry_on_max_price_block: false,
            ptb_stop_loss_gap_usd: None,
            ptb_reference_price: None,
            ptb_stop_loss_rules_json: Vec::new(),
            ptb_stop_loss_time_decay_mode: None,
            staged_sl_retry_only_dust: false,
            staged_sl_retry_dust_metric: None,
            staged_sl_retry_dust_value: None,
            staged_sl_reentry_use_sold_notional: false,
            staged_sl_reentry_only_after_all_stages: false,
            sl_trigger_price_mode: None,
            reenter_on_sl_hit: false,
            reentry_max_attempts: 0,
            reentry_trigger_node_key: None,
            notify_on_order_submitted: false,
            notify_on_fill: false,
            notify_on_order_not_filled: false,
            notify_on_trigger_guard_blocked: false,
            notify_on_execution_floor_blocked: false,
            notify_on_tp_hit: false,
            notify_on_sl_hit: false,
            notify_on_max_price_blocked: false,
            last_guard_notification_reason: None,
            exit_ladder_kind: None,
            exit_ladder_index: None,
            exit_ladder_size_pct: None,
        }
    }

    #[test]
    fn eligibility_window_waits_before_open() {
        let now = Utc::now();
        let mut order = test_entry_order();
        order.eligible_after_at = Some(now + ChronoDuration::seconds(30));
        order.eligible_before_at = Some(now + ChronoDuration::seconds(90));

        assert_eq!(
            classify_trade_builder_order_eligibility_window(&order, now),
            TradeBuilderEligibilityWindowState::Wait
        );
    }

    #[test]
    fn eligibility_window_expires_after_close_when_not_submitted() {
        let now = Utc::now();
        let mut order = test_entry_order();
        order.eligible_after_at = Some(now - ChronoDuration::seconds(90));
        order.eligible_before_at = Some(now - ChronoDuration::seconds(1));

        assert_eq!(
            classify_trade_builder_order_eligibility_window(&order, now),
            TradeBuilderEligibilityWindowState::Expire
        );
    }

    #[test]
    fn eligibility_window_allows_active_window() {
        let now = Utc::now();
        let mut order = test_entry_order();
        order.eligible_after_at = Some(now - ChronoDuration::seconds(10));
        order.eligible_before_at = Some(now + ChronoDuration::seconds(50));

        assert_eq!(
            classify_trade_builder_order_eligibility_window(&order, now),
            TradeBuilderEligibilityWindowState::Allow
        );
    }

    #[test]
    fn eligibility_window_does_not_block_submitted_order() {
        let now = Utc::now();
        let mut order = test_entry_order();
        order.eligible_after_at = Some(now + ChronoDuration::seconds(30));
        order.eligible_before_at = Some(now + ChronoDuration::seconds(90));
        order.active_exchange_order_id = Some("ex-1".to_string());

        assert_eq!(
            classify_trade_builder_order_eligibility_window(&order, now),
            TradeBuilderEligibilityWindowState::Allow
        );
    }
}
use futures_util::stream::{FuturesUnordered, StreamExt};
