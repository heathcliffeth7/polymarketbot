const PAIR_LOCK_PRIMARY_GUARD_RETRY_DELAY_MS: i64 = 150;
const FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_WAITING_MARKET_SLUG: &str = "pair_lock_primary_waiting_market_slug";
const FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_WAITING_SIGNATURE: &str = "pair_lock_primary_waiting_signature";
const FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_MARKET_SLUG: &str = "pair_lock_primary_notification_market_slug";
const FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_REASON: &str = "pair_lock_primary_notification_reason";
const FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_SCOPE: &str = "pair_lock_primary_notification_scope";
#[derive(Debug, Clone)]
struct ActionPlaceOrderPairLockPrimaryCandidateEval {
    token_id: String,
    outcome_label: String,
    decision: &'static str,
    reason_code: String,
    quote: PairLockResolvedQuote,
    diagnostics: Value,
    adaptive_max_price_override: Option<PairLockAdaptiveMaxPriceOverride>,
    manual_adaptive_risk_override: Option<PairLockManualAdaptiveRiskOverride>,
}
#[derive(Debug, Clone)]
struct ActionPlaceOrderPairLockPrimarySelection {
    token_id: String,
    outcome_label: String,
    selection_mode: &'static str,
    guard_reason: String,
    adaptive_max_price_override: Option<PairLockAdaptiveMaxPriceOverride>,
    manual_adaptive_risk_override: Option<PairLockManualAdaptiveRiskOverride>,
}
#[derive(Debug, Clone)]
struct ActionPlaceOrderPairLockPrimarySelectionAttempt {
    selection: Option<ActionPlaceOrderPairLockPrimarySelection>,
    waiting: bool,
    failure_reason: Option<&'static str>,
    yes_candidate: ActionPlaceOrderPairLockPrimaryCandidateEval,
    no_candidate: ActionPlaceOrderPairLockPrimaryCandidateEval,
    diagnostics: Value,
}

#[derive(Debug, Clone)]
struct PairLockPrimaryNotificationReason {
    scope: &'static str,
    reason_code: String,
    decision: &'static str,
    candidate: Value,
    secondary_candidate: Option<Value>,
}

fn pair_lock_primary_outcome_labels(market_slug: &str) -> (&'static str, &'static str) {
    if market_slug.contains("-updown-") {
        ("Up", "Down")
    } else {
        ("Yes", "No")
    }
}

fn pair_lock_primary_waiting_signature(diagnostics: &Value) -> String {
    let yes = diagnostics
        .get("yes_candidate_guard")
        .and_then(Value::as_object)
        .map(|value| {
            format!(
                "{}:{}",
                value.get("decision").and_then(Value::as_str).unwrap_or("unknown"),
                value.get("reason_code").and_then(Value::as_str).unwrap_or("unknown")
            )
        })
        .unwrap_or_else(|| "missing:missing".to_string());
    let no = diagnostics
        .get("no_candidate_guard")
        .and_then(Value::as_object)
        .map(|value| {
            format!(
                "{}:{}",
                value.get("decision").and_then(Value::as_str).unwrap_or("unknown"),
                value.get("reason_code").and_then(Value::as_str).unwrap_or("unknown")
            )
        })
        .unwrap_or_else(|| "missing:missing".to_string());
    format!("yes={yes}|no={no}")
}

fn pair_lock_primary_notification_scope(reason_code: &str) -> Option<&'static str> {
    match reason_code {
        "price_to_beat_gap_below_threshold"
        | "price_to_beat_pending"
        | "price_to_beat_unavailable" => Some("price_to_beat"),
        "below_trigger_price_guard" => Some("trigger_price"),
        "below_best_ask_floor" | "best_ask_unavailable" | "pair_primary_best_ask_unavailable" => {
            Some("execution_floor")
        }
        "above_max_price" => Some("max_price"),
        _ => None,
    }
}

fn pair_lock_primary_notification_priority(scope: &str) -> i32 {
    match scope {
        "price_to_beat" => 0,
        "trigger_price" => 1,
        "execution_floor" => 2,
        "max_price" => 3,
        _ => 9,
    }
}

fn pair_lock_primary_notification_reason_from_candidate(
    candidate: &Value,
) -> Option<PairLockPrimaryNotificationReason> {
    let reason_code = candidate.get("reason_code").and_then(Value::as_str)?;
    let scope = pair_lock_primary_notification_scope(reason_code)?;
    let decision = candidate
        .get("decision")
        .and_then(Value::as_str)
        .map(|value| match value {
            "passed" => "passed",
            "blocked" => "blocked",
            _ => "waiting",
        })
        .unwrap_or("waiting");
    Some(PairLockPrimaryNotificationReason {
        scope,
        reason_code: reason_code.to_string(),
        decision,
        candidate: candidate.clone(),
        secondary_candidate: None,
    })
}

fn resolve_pair_lock_primary_notification_reason(
    diagnostics: &Value,
) -> Option<PairLockPrimaryNotificationReason> {
    let yes_candidate = diagnostics.get("yes_candidate_guard");
    let no_candidate = diagnostics.get("no_candidate_guard");
    let mut candidates = Vec::new();
    if let Some(candidate) = yes_candidate.and_then(pair_lock_primary_notification_reason_from_candidate)
    {
        candidates.push(candidate);
    }
    if let Some(candidate) = no_candidate.and_then(pair_lock_primary_notification_reason_from_candidate)
    {
        candidates.push(candidate);
    }
    let selected = candidates
        .iter()
        .enumerate()
        .min_by_key(|(_, candidate)| {
            (
                pair_lock_primary_notification_priority(candidate.scope),
                candidate.reason_code.clone(),
            )
        })
        .map(|(index, candidate)| (index, candidate.clone()))?;
    let secondary_candidate = candidates
        .into_iter()
        .enumerate()
        .find_map(|(index, candidate)| (index != selected.0).then_some(candidate.candidate));
    Some(PairLockPrimaryNotificationReason {
        secondary_candidate,
        ..selected.1
    })
}

fn pair_lock_primary_notify_flag(node: &TradeFlowNode, scope: &str) -> bool {
    match scope {
        "trigger_price" => node_config_bool(node, "notifyOnTriggerPriceBlocked").unwrap_or(false),
        "execution_floor" => {
            node_config_bool(node, "notifyOnExecutionFloorBlocked").unwrap_or(false)
        }
        "max_price" => node_config_bool(node, "notifyOnMaxPriceBlocked").unwrap_or(false),
        "price_to_beat" => true,
        _ => false,
    }
}

fn pair_lock_primary_notification_reason_changed(
    context: &Value,
    node_key: &str,
    market_slug: &str,
    candidate_reason: &str,
) -> bool {
    let previous_market = flow_node_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_MARKET_SLUG,
    );
    let previous_reason = flow_node_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_REASON,
    );
    previous_market.as_deref() != Some(market_slug)
        || previous_reason.as_deref() != Some(candidate_reason)
}

fn set_pair_lock_primary_notification_state(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
    scope: &str,
    candidate_reason: &str,
) {
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_MARKET_SLUG,
        json!(market_slug),
    );
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_REASON,
        json!(candidate_reason),
    );
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_SCOPE,
        json!(scope),
    );
}

fn clear_pair_lock_primary_notification_state(context: &mut Value, node_key: &str) {
    remove_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_MARKET_SLUG,
    );
    remove_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_REASON,
    );
    remove_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_SCOPE,
    );
}

async fn maybe_send_pair_lock_primary_guard_notification(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    market_slug: &str,
    diagnostics: &Value,
    context: &mut Value,
) -> Result<()> {
    let Some(reason) = resolve_pair_lock_primary_notification_reason(diagnostics) else {
        return Ok(());
    };
    let candidate_reason = build_guard_notification_reason(reason.scope, &reason.reason_code);
    if !pair_lock_primary_notification_reason_changed(
        context,
        &node.key,
        market_slug,
        &candidate_reason,
    ) {
        return Ok(());
    }
    if !pair_lock_primary_notify_flag(node, reason.scope) {
        return Ok(());
    }

    let message = match reason.scope {
        "trigger_price" => build_pair_lock_primary_trigger_guard_notification_message(
            market_slug,
            &reason.candidate,
            reason
                .candidate
                .pointer("/trigger_price_guard/details/guard_trigger_price")
                .and_then(value_as_f64),
            reason.decision == "waiting",
            reason.secondary_candidate.as_ref(),
        ),
        "execution_floor" => build_pair_lock_primary_execution_floor_notification_message(
            market_slug,
            &reason.candidate,
            reason
                .candidate
                .pointer("/execution_floor_guard/details/best_ask_floor_price")
                .and_then(value_as_f64),
            reason.decision == "waiting",
            reason.secondary_candidate.as_ref(),
        ),
        "max_price" => build_pair_lock_primary_max_price_notification_message(
            market_slug,
            &reason.candidate,
            reason
                .candidate
                .pointer("/max_price_guard/details/max_price")
                .and_then(value_as_f64),
            reason.decision == "waiting",
            reason.secondary_candidate.as_ref(),
        ),
        "price_to_beat" => build_pair_lock_primary_price_to_beat_notification_message(
            market_slug,
            &reason.candidate,
            reason.decision == "waiting",
            reason.secondary_candidate.as_ref(),
        ),
        _ => return Ok(()),
    };
    let notification_type = if reason.decision == "waiting" {
        "pair_lock_primary_guard_waiting"
    } else {
        "pair_lock_primary_guard_blocked"
    };
    if send_trade_flow_notification(repo, run, &node.key, notification_type, &message).await {
        set_pair_lock_primary_notification_state(
            context,
            &node.key,
            market_slug,
            reason.scope,
            &candidate_reason,
        );
    }
    Ok(())
}

async fn maybe_send_pair_lock_primary_guard_recovered_notification(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node_key: &str,
    market_slug: &str,
    context: &Value,
) {
    let previous_market = flow_node_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_MARKET_SLUG,
    );
    let previous_reason = flow_node_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_REASON,
    );
    let previous_scope = flow_node_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_SCOPE,
    );
    if previous_market.as_deref() != Some(market_slug) {
        return;
    }
    let Some(previous_scope) = previous_scope else {
        return;
    };
    let previous_reason_code = previous_reason
        .as_deref()
        .and_then(|reason| reason.split_once(':').map(|(_, code)| code))
        .unwrap_or("unknown");
    let message = build_pair_lock_primary_guard_recovered_notification_message(
        market_slug,
        &previous_scope,
        previous_reason_code,
    );
    let _ = send_trade_flow_notification(
        repo,
        run,
        node_key,
        "pair_lock_primary_guard_recovered",
        &message,
    )
    .await;
}

fn pair_lock_primary_ptb_guard_decision(
    passed: bool,
    retry_on_price_to_beat_guard_block: bool,
) -> &'static str {
    if passed {
        "passed"
    } else if retry_on_price_to_beat_guard_block {
        "waiting"
    } else {
        "blocked"
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PairLockPrimaryPtbEvaluationLogSnapshot {
    flow_run_id: i64,
    node_key: String,
    market_slug: String,
    outcome_label: String,
    ptb_passed: bool,
    ptb_reason_code: String,
    directional_gap: Option<f64>,
    threshold_usd: f64,
    current_price: Option<f64>,
    price_to_beat: Option<f64>,
}

fn pair_lock_primary_should_log_ptb_skip(node: &TradeFlowNode, decision: &str) -> bool {
    node_config_bool(node, "priceToBeatGuardEnabled").unwrap_or(false) && decision != "passed"
}

fn pair_lock_primary_ptb_evaluation_log_snapshot(
    flow_run_id: i64,
    node_key: &str,
    market_slug: &str,
    outcome_label: &str,
    evaluation: &crate::trade_flow::guards::price_to_beat::PriceToBeatGuardEvaluation,
) -> PairLockPrimaryPtbEvaluationLogSnapshot {
    PairLockPrimaryPtbEvaluationLogSnapshot {
        flow_run_id,
        node_key: node_key.to_string(),
        market_slug: market_slug.to_string(),
        outcome_label: outcome_label.to_string(),
        ptb_passed: evaluation.passed,
        ptb_reason_code: evaluation.reason_code.clone(),
        directional_gap: evaluation.directional_gap,
        threshold_usd: evaluation.threshold_usd,
        current_price: evaluation.current_price,
        price_to_beat: evaluation.price_to_beat,
    }
}

fn pair_lock_primary_waiting_signature_changed(
    context: &Value,
    node_key: &str,
    market_slug: &str,
    diagnostics: &Value,
) -> bool {
    let previous_market = flow_node_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_WAITING_MARKET_SLUG,
    );
    let previous_signature = flow_node_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_WAITING_SIGNATURE,
    );
    let next_signature = pair_lock_primary_waiting_signature(diagnostics);
    previous_market.as_deref() != Some(market_slug)
        || previous_signature.as_deref() != Some(next_signature.as_str())
}

fn set_pair_lock_primary_waiting_state(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
    diagnostics: &Value,
) {
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_WAITING_MARKET_SLUG,
        json!(market_slug),
    );
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_WAITING_SIGNATURE,
        json!(pair_lock_primary_waiting_signature(diagnostics)),
    );
}

fn clear_pair_lock_primary_waiting_state(context: &mut Value, node_key: &str) {
    remove_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_WAITING_MARKET_SLUG,
    );
    remove_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_WAITING_SIGNATURE,
    );
}

fn build_pair_lock_primary_waiting_execution(
    node_key: &str,
    market_slug: &str,
    diagnostics: &Value,
) -> TradeFlowNodeExecution {
    TradeFlowNodeExecution {
        output: json!({
            "node_key": node_key,
            "blocked": true,
            "retrying": true,
            "reason": "pair_lock_primary_guard_waiting",
            "market_slug": market_slug,
            "retry_delay_ms": PAIR_LOCK_PRIMARY_GUARD_RETRY_DELAY_MS,
            "resolved_yes_token_id": diagnostics.get("resolved_yes_token_id").cloned().unwrap_or(Value::Null),
            "resolved_no_token_id": diagnostics.get("resolved_no_token_id").cloned().unwrap_or(Value::Null),
            "trigger_node_market_slug": diagnostics.get("trigger_node_market_slug").cloned().unwrap_or(Value::Null),
            "primary_selection": diagnostics,
            "yes_candidate_guard": diagnostics.get("yes_candidate_guard").cloned().unwrap_or(Value::Null),
            "no_candidate_guard": diagnostics.get("no_candidate_guard").cloned().unwrap_or(Value::Null),
        }),
        routes: Vec::new(),
        repeat_at: Some(Utc::now() + ChronoDuration::milliseconds(PAIR_LOCK_PRIMARY_GUARD_RETRY_DELAY_MS)),
        repeat_idempotency_key: None,
    }
}

async fn maybe_emit_pair_lock_primary_waiting_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node_key: &str,
    market_slug: &str,
    diagnostics: &Value,
    token_resolution_payload: &Value,
    context: &Value,
) -> Result<()> {
    if !pair_lock_primary_waiting_signature_changed(context, node_key, market_slug, diagnostics) {
        return Ok(());
    }
    let mut payload = json!({
        "node_key": node_key,
        "market_slug": market_slug,
        "reason": "pair_lock_primary_guard_waiting",
        "selection_mode": "auto_guarded",
        "selection": diagnostics,
    });
    append_json_object_fields(&mut payload, token_resolution_payload);
    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "pair_lock_primary_leg_waiting",
        &payload,
    )
    .await
}

async fn maybe_emit_pair_lock_primary_recovered_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node_key: &str,
    market_slug: &str,
    selection_mode: &str,
    selected_primary_token_id: &str,
    selected_primary_outcome_label: &str,
    selected_primary_guard_reason: &str,
    diagnostics: &Value,
    token_resolution_payload: &Value,
    context: &Value,
) -> Result<()> {
    let previous_market = flow_node_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_WAITING_MARKET_SLUG,
    );
    let previous_signature = flow_node_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_WAITING_SIGNATURE,
    );
    if previous_market.as_deref() != Some(market_slug) || previous_signature.is_none() {
        return Ok(());
    }
    let mut payload = json!({
        "node_key": node_key,
        "market_slug": market_slug,
        "selection_mode": selection_mode,
        "selected_primary_token_id": selected_primary_token_id,
        "selected_primary_outcome_label": selected_primary_outcome_label,
        "selected_primary_guard_reason": selected_primary_guard_reason,
        "selection": diagnostics,
    });
    append_json_object_fields(&mut payload, token_resolution_payload);
    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "pair_lock_primary_leg_recovered",
        &payload,
    )
    .await
}

async fn evaluate_action_place_order_pair_lock_primary_candidate(
    ptb_runtime: Option<crate::trade_flow::guards::price_to_beat::PriceToBeatGuardRuntimeContext<'_>>,
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &mut Value,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
) -> Result<ActionPlaceOrderPairLockPrimaryCandidateEval> {
    let current_price_hint = resolve_action_place_order_reference_price(node, step);
    let quote = resolve_pair_lock_action_candidate_quote(
        ws,
        client,
        step,
        token_id,
        outcome_label,
        current_price_hint,
    )
    .await;
    let best_bid = quote.best_bid;
    let best_ask = quote.best_ask;
    let last_trade_price = quote.last_trade_price;
    let current_price = quote.current_price;

    let min_price_distance_cent = node_config_f64(node, "minPriceDistanceCent").unwrap_or(1.0);
    anyhow::ensure!(
        min_price_distance_cent > 0.0,
        "action.place_order minPriceDistanceCent must be > 0"
    );
    let base_max_price = resolve_action_place_order_max_price(node, step, context);
    let trigger_price_guard_enabled =
        node_config_bool(node, "triggerPriceGuardEnabled").unwrap_or(false);
    let base_guard_trigger_price = if trigger_price_guard_enabled {
        resolve_action_place_order_guard_trigger_price(step)
    } else {
        None
    };
    let reentry_guard_resolution = resolve_action_place_order_reentry_guard_resolution(
        node,
        context,
        base_guard_trigger_price,
        base_max_price,
    )?;
    let execution_floor_guard_enabled =
        node_config_bool(node, "executionFloorGuardEnabled").unwrap_or(false);
    let best_ask_floor_price = if execution_floor_guard_enabled {
        resolve_action_place_order_execution_floor_price(node, step)
    } else {
        None
    };
    let desired_price = best_ask.unwrap_or(current_price);
    let guard_eval = evaluate_trade_builder_buy_guards(
        node_config_string(node, "executionMode")
            .as_deref()
            .unwrap_or("market"),
        Some("lead_candidate"),
        current_price,
        best_ask,
        desired_price,
        reentry_guard_resolution.effective_guard_trigger_price,
        reentry_guard_resolution.effective_max_price,
        best_ask_floor_price,
        node_config_bool(node, "retryOnTriggerPriceGuardBlock").unwrap_or(false),
        node_config_bool(node, "retryOnExecutionFloorGuardBlock").unwrap_or(false),
        node_config_bool(node, "retryOnMaxPriceBlock").unwrap_or(false),
    );

    let mut ptb_guard = Value::Null;
    let mut decision = guard_eval.effective_decision;
    let mut reason_code = guard_eval.effective_reason_code.to_string();
    let adaptive_max_price_probe =
        action_place_order_uses_adaptive_max_price_strategy(node)
            && guard_eval.max_price_blocked
            && !guard_eval.trigger_price_guard_blocked
            && guard_eval.execution_floor_reason.is_none()
            && guard_eval.pair_lock_market_waiting_reason.is_none();
    if pair_lock_primary_should_log_ptb_skip(node, decision) && !adaptive_max_price_probe {
        tracing::debug!(
            message = "PAIR_LOCK_PRIMARY_PTB_SKIPPED_BY_PRE_GUARD",
            flow_run_id = run.id,
            node_key = %node.key,
            market_slug = %market_slug,
            outcome_label = %outcome_label,
            pre_guard_decision = %decision,
            pre_guard_reason_code = %reason_code,
            best_ask = ?best_ask,
            current_price,
            effective_max_price = ?reentry_guard_resolution.effective_max_price,
            best_ask_floor_price = ?best_ask_floor_price,
        );
    }
    if (decision == "passed" || adaptive_max_price_probe)
        && node_config_bool(node, "priceToBeatGuardEnabled").unwrap_or(false)
    {
        let evaluation =
            crate::trade_flow::guards::price_to_beat::evaluate_action_place_order_price_to_beat_guard_state(
                ptb_runtime,
                context,
                node,
                run.id,
                Some(run.definition_id),
                market_slug,
                outcome_label,
                Some(crate::trade_flow::guards::price_to_beat::PriceToBeatSignalFormulaMarketInput {
                    best_bid,
                    best_ask,
                }),
            )
            .await?;
        let ptb_log_snapshot = pair_lock_primary_ptb_evaluation_log_snapshot(
            run.id,
            &node.key,
            market_slug,
            outcome_label,
            &evaluation,
        );
        tracing::debug!(
            message = "PAIR_LOCK_PRIMARY_PTB_EVALUATED",
            flow_run_id = ptb_log_snapshot.flow_run_id,
            node_key = %ptb_log_snapshot.node_key,
            market_slug = %ptb_log_snapshot.market_slug,
            outcome_label = %ptb_log_snapshot.outcome_label,
            ptb_passed = ptb_log_snapshot.ptb_passed,
            ptb_reason_code = %ptb_log_snapshot.ptb_reason_code,
            directional_gap = ?ptb_log_snapshot.directional_gap,
            threshold_usd = ptb_log_snapshot.threshold_usd,
            current_price = ?ptb_log_snapshot.current_price,
            price_to_beat = ?ptb_log_snapshot.price_to_beat,
        );
        ptb_guard = evaluation.to_value();
        if !adaptive_max_price_probe {
            decision = pair_lock_primary_ptb_guard_decision(
                evaluation.passed,
                node_config_bool(node, "retryOnPriceToBeatGuardBlock").unwrap_or(true),
            );
            if decision != "passed" {
                reason_code = evaluation.reason_code.clone();
            }
        }
    }

    Ok(ActionPlaceOrderPairLockPrimaryCandidateEval {
        token_id: token_id.to_string(),
        outcome_label: outcome_label.to_string(),
        decision,
        reason_code: reason_code.clone(),
        quote: quote.clone(),
        adaptive_max_price_override: None,
        manual_adaptive_risk_override: None,
        diagnostics: json!({
            "token_id": token_id,
            "outcome_label": outcome_label,
            "passed": decision == "passed",
            "decision": decision,
            "reason_code": reason_code,
            "current_price": current_price,
            "best_bid": best_bid,
            "best_ask": best_ask,
            "last_trade_price": last_trade_price,
            "desired_price": desired_price,
            "quote_source_kind": quote.quote_source_kind,
            "quote_ws_state": quote.quote_ws_state,
            "quote_event_ts": quote.quote_event_ts,
            "quote_snapshot_age_ms": quote.quote_snapshot_age_ms,
            "quote_source_detail": quote.quote_source_detail,
            "quote_book_missing_fields": quote.quote_book_missing_fields,
            "quote_snapshot_used": quote.quote_snapshot_used,
            "reentry_guard_resolution": {
                "generation": reentry_guard_resolution.generation,
                "band_active": reentry_guard_resolution.band_active,
                "effective_guard_trigger_price": reentry_guard_resolution.effective_guard_trigger_price,
                "effective_max_price": reentry_guard_resolution.effective_max_price,
            },
            "trigger_price_guard": guard_eval.trigger_price_guard_payload,
            "execution_floor_guard": guard_eval.execution_floor_payload,
            "max_price_guard": guard_eval.max_price_payload,
            "price_to_beat_guard": ptb_guard,
            "flow_run_id": run.id,
        }),
    })
}

fn resolve_action_place_order_pair_lock_primary_selection_attempt(
    up_candidate: ActionPlaceOrderPairLockPrimaryCandidateEval,
    down_candidate: ActionPlaceOrderPairLockPrimaryCandidateEval,
) -> ActionPlaceOrderPairLockPrimarySelectionAttempt {
    let passing = [&up_candidate, &down_candidate]
        .into_iter()
        .filter(|candidate| candidate.decision == "passed")
        .collect::<Vec<_>>();
    let waiting = [&up_candidate, &down_candidate]
        .into_iter()
        .filter(|candidate| candidate.decision == "waiting")
        .collect::<Vec<_>>();
    let diagnostics = json!({
        "selection_mode": "auto_guarded",
        "yes_candidate_guard": up_candidate.diagnostics.clone(),
        "no_candidate_guard": down_candidate.diagnostics.clone(),
    });

    if passing.len() == 1 {
        let selected = passing[0];
        return ActionPlaceOrderPairLockPrimarySelectionAttempt {
            selection: Some(ActionPlaceOrderPairLockPrimarySelection {
                token_id: selected.token_id.clone(),
                outcome_label: selected.outcome_label.clone(),
                selection_mode: "auto_guarded",
                guard_reason: selected.reason_code.clone(),
                adaptive_max_price_override: selected.adaptive_max_price_override.clone(),
                manual_adaptive_risk_override: selected.manual_adaptive_risk_override.clone(),
            }),
            waiting: false,
            failure_reason: None,
            yes_candidate: up_candidate,
            no_candidate: down_candidate,
            diagnostics,
        };
    }
    if passing.len() == 2 {
        let edge_a = pair_lock_primary_iv_edge(passing[0]);
        let edge_b = pair_lock_primary_iv_edge(passing[1]);
        if let (Some(edge_a), Some(edge_b)) = (edge_a, edge_b) {
            if (edge_a - edge_b).abs() > f64::EPSILON {
                let selected = if edge_a > edge_b { passing[0] } else { passing[1] };
                return ActionPlaceOrderPairLockPrimarySelectionAttempt {
                    selection: Some(ActionPlaceOrderPairLockPrimarySelection {
                        token_id: selected.token_id.clone(),
                        outcome_label: selected.outcome_label.clone(),
                        selection_mode: "auto_guarded_iv_mismatch_edge",
                        guard_reason: "selected_edge_passed".to_string(),
                        adaptive_max_price_override: selected.adaptive_max_price_override.clone(),
                        manual_adaptive_risk_override: selected.manual_adaptive_risk_override.clone(),
                    }),
                    waiting: false,
                    failure_reason: None,
                    yes_candidate: up_candidate,
                    no_candidate: down_candidate,
                    diagnostics,
                };
            }
        }
    }
    let failure_reason = if passing.is_empty() && !waiting.is_empty() {
        Some("pair_lock_primary_guard_waiting")
    } else if passing.is_empty() {
        Some("pair_lock_no_primary_leg_passed")
    } else {
        Some("pair_lock_primary_leg_ambiguous")
    };
    ActionPlaceOrderPairLockPrimarySelectionAttempt {
        selection: None,
        waiting: failure_reason == Some("pair_lock_primary_guard_waiting"),
        failure_reason,
        yes_candidate: up_candidate,
        no_candidate: down_candidate,
        diagnostics,
    }
}

fn pair_lock_primary_iv_edge(candidate: &ActionPlaceOrderPairLockPrimaryCandidateEval) -> Option<f64> {
    let guard = candidate.diagnostics.get("price_to_beat_guard")?;
    (guard.get("threshold_mode").and_then(Value::as_str) == Some("iv_mismatch_edge"))
        .then(|| guard.get("iv_mismatch_edge")?.get("edge").and_then(Value::as_f64))?
}

async fn resolve_action_place_order_pair_lock_primary_selection(
    ptb_runtime: Option<crate::trade_flow::guards::price_to_beat::PriceToBeatGuardRuntimeContext<'_>>,
    adaptive_max_price_runtime: Option<(&PostgresRepository, &ActionPlaceOrderPairLockConfig)>,
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &mut Value,
    market_slug: &str,
    yes_token_id: Option<String>,
    no_token_id: Option<String>,
) -> Result<ActionPlaceOrderPairLockPrimarySelectionAttempt> {
    let (up_label, down_label) = pair_lock_primary_outcome_labels(market_slug);
    let yes_token_id = yes_token_id
        .ok_or_else(|| anyhow::anyhow!("pair_lock auto primary selection requires yesTokenId"))?;
    let no_token_id = no_token_id
        .ok_or_else(|| anyhow::anyhow!("pair_lock auto primary selection requires noTokenId"))?;

    let mut up_candidate = evaluate_action_place_order_pair_lock_primary_candidate(
        ptb_runtime,
        ws,
        client,
        run,
        step,
        node,
        context,
        market_slug,
        &yes_token_id,
        up_label,
    )
    .await?;
    let mut down_candidate = evaluate_action_place_order_pair_lock_primary_candidate(
        ptb_runtime,
        ws,
        client,
        run,
        step,
        node,
        context,
        market_slug,
        &no_token_id,
        down_label,
    )
    .await?;
    if let Some((repo, pair_lock)) = adaptive_max_price_runtime {
        maybe_apply_pair_lock_adaptive_max_price_candidate_override(
            repo,
            run,
            step,
            node,
            context,
            market_slug,
            pair_lock,
            &mut up_candidate,
            &down_candidate,
        )
        .await?;
        maybe_apply_pair_lock_adaptive_max_price_candidate_override(
            repo,
            run,
            step,
            node,
            context,
            market_slug,
            pair_lock,
            &mut down_candidate,
            &up_candidate,
        )
        .await?;
        maybe_apply_pair_lock_manual_adaptive_risk_candidate_override(
            repo,
            run,
            step,
            node,
            context,
            market_slug,
            pair_lock,
            &mut up_candidate,
            &down_candidate,
        )
        .await?;
        maybe_apply_pair_lock_manual_adaptive_risk_candidate_override(
            repo,
            run,
            step,
            node,
            context,
            market_slug,
            pair_lock,
            &mut down_candidate,
            &up_candidate,
        )
        .await?;
    }
    let selection_attempt =
        resolve_action_place_order_pair_lock_primary_selection_attempt(up_candidate, down_candidate);
    if let Some(runtime) = ptb_runtime {
        let selected = selection_attempt.selection.as_ref();
        crate::trade_flow::guards::price_to_beat::maybe_emit_pair_lock_primary_iv_mismatch_edge_decision_event(
            runtime.repo, run, context, node, market_slug,
            selected.map(|selection| selection.selection_mode).unwrap_or("auto_guarded"),
            selected.map(|selection| selection.token_id.as_str()),
            selected.map(|selection| selection.outcome_label.as_str()),
            selected.map(|selection| selection.guard_reason.as_str()),
            selection_attempt.failure_reason, &selection_attempt.diagnostics,
        )
        .await?;
    }
    Ok(selection_attempt)
}

#[cfg(test)]
mod pair_lock_auto_primary_tests {
    use super::*;
    use async_trait::async_trait;
    use bot_infra::exchange::{FillInfo, OrderAck, OrderInfo, PlaceOrderRequest, PriceHistoryPoint, PriceSnapshot};

    struct FakeExecutor {
        quotes: HashMap<String, (Option<f64>, Option<f64>, Option<f64>)>,
    }

    #[async_trait]
    impl OrderExecutor for FakeExecutor {
        async fn midpoint(&self, market: &str) -> Result<PriceSnapshot> {
            Ok(PriceSnapshot {
                market: market.to_string(),
                price: 0.5,
            })
        }

        async fn best_bid_ask(&self, token_id: &str) -> Result<(Option<f64>, Option<f64>)> {
            let (best_bid, best_ask, _) = self.quotes.get(token_id).cloned().unwrap_or((None, None, None));
            Ok((best_bid, best_ask))
        }

        async fn last_trade_price(&self, token_id: &str) -> Result<Option<f64>> {
            Ok(self.quotes.get(token_id).and_then(|(_, _, last_trade)| *last_trade))
        }

        async fn price_history(
            &self,
            _token_id: &str,
            _start_ts: i64,
            _end_ts: i64,
            _fidelity: i64,
        ) -> Result<Vec<PriceHistoryPoint>> {
            Ok(Vec::new())
        }

        async fn fee_rate_bps(&self, _token_id: &str) -> Result<Option<u64>> {
            Ok(Some(0))
        }

        async fn place(&self, _req: &PlaceOrderRequest) -> Result<OrderAck> {
            anyhow::bail!("not used in test")
        }

        async fn cancel(&self, _exchange_order_id: &str) -> Result<()> {
            anyhow::bail!("not used in test")
        }

        async fn status(&self, _exchange_order_id: &str) -> Result<OrderInfo> {
            anyhow::bail!("not used in test")
        }

        async fn list_open(&self, _market: Option<&str>) -> Result<Vec<OrderInfo>> {
            Ok(Vec::new())
        }

        async fn list_fills(&self, _next_cursor: Option<&str>) -> Result<Vec<FillInfo>> {
            Ok(Vec::new())
        }

        async fn available_token_qty(&self, _token_id: &str) -> Result<Option<f64>> {
            Ok(None)
        }
    }

    fn pair_lock_test_run() -> TradeFlowRun {
        TradeFlowRun {
            id: 77,
            definition_id: 88,
            version_id: 99,
            user_id: 1,
            status: "running".to_string(),
            trigger_source: Some("test".to_string()),
            context_json: json!({}),
            started_at: Some(Utc::now()),
            ended_at: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn pair_lock_auto_primary_selects_up_when_only_up_passes_max_price() {
        let executor = FakeExecutor {
            quotes: HashMap::from([
                ("yes".to_string(), (Some(0.68), Some(0.69), Some(0.69))),
                ("no".to_string(), (Some(0.71), Some(0.72), Some(0.72))),
            ]),
        };
        let ws = ClobWsClient::new("wss://example.com/ws".to_string());
        let node = TradeFlowNode {
            key: "pair_buy".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({
                "executionMode": "market",
                "maxPriceCent": 70,
                "minPriceDistanceCent": 1,
            }),
        };
        let step = TradeFlowRunStep {
            id: 1,
            run_id: 77,
            node_key: "pair_buy".to_string(),
            node_type: "action.place_order".to_string(),
            status: "queued".to_string(),
            attempt: 1,
            input_json: Some(json!({})),
            output_json: None,
            error_text: None,
            started_at: None,
            ended_at: None,
            available_at: Utc::now(),
            parent_step_id: None,
            idempotency_key: None,
            created_at: Utc::now(),
        };
        let mut context = json!({});

        let selection = resolve_action_place_order_pair_lock_primary_selection(
            None,
            None,
            &ws,
            &executor,
            &pair_lock_test_run(),
            &step,
            &node,
            &mut context,
            "btc-updown-5m-1",
            Some("yes".to_string()),
            Some("no".to_string()),
        )
        .await
        .expect("selection");

        assert_eq!(selection.yes_candidate.quote.best_ask, Some(0.69));
        assert_eq!(selection.no_candidate.quote.best_ask, Some(0.72));
        let selection = selection.selection.expect("primary selection");
        assert_eq!(selection.token_id, "yes");
        assert_eq!(selection.outcome_label, "Up");
        assert_eq!(selection.selection_mode, "auto_guarded");
    }

    #[tokio::test]
    async fn pair_lock_auto_primary_selects_down_when_only_down_passes_max_price() {
        let executor = FakeExecutor {
            quotes: HashMap::from([
                ("yes".to_string(), (Some(0.73), Some(0.74), Some(0.74))),
                ("no".to_string(), (Some(0.68), Some(0.69), Some(0.69))),
            ]),
        };
        let ws = ClobWsClient::new("wss://example.com/ws".to_string());
        let node = TradeFlowNode {
            key: "pair_buy".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({
                "executionMode": "market",
                "maxPriceCent": 70,
                "minPriceDistanceCent": 1,
            }),
        };
        let step = TradeFlowRunStep {
            id: 1,
            run_id: 77,
            node_key: "pair_buy".to_string(),
            node_type: "action.place_order".to_string(),
            status: "queued".to_string(),
            attempt: 1,
            input_json: Some(json!({})),
            output_json: None,
            error_text: None,
            started_at: None,
            ended_at: None,
            available_at: Utc::now(),
            parent_step_id: None,
            idempotency_key: None,
            created_at: Utc::now(),
        };
        let mut context = json!({});

        let selection = resolve_action_place_order_pair_lock_primary_selection(
            None,
            None,
            &ws,
            &executor,
            &pair_lock_test_run(),
            &step,
            &node,
            &mut context,
            "btc-updown-5m-1",
            Some("yes".to_string()),
            Some("no".to_string()),
        )
        .await
        .expect("selection");

        assert_eq!(selection.yes_candidate.quote.best_ask, Some(0.74));
        assert_eq!(selection.no_candidate.quote.best_ask, Some(0.69));
        let selection = selection.selection.expect("primary selection");
        assert_eq!(selection.token_id, "no");
        assert_eq!(selection.outcome_label, "Down");
    }

    #[tokio::test]
    async fn pair_lock_auto_primary_returns_waiting_when_retryable_guards_block_all_candidates() {
        let executor = FakeExecutor {
            quotes: HashMap::from([
                ("yes".to_string(), (Some(0.84), Some(0.85), Some(0.85))),
                ("no".to_string(), (Some(0.68), Some(0.69), Some(0.69))),
            ]),
        };
        let ws = ClobWsClient::new("wss://example.com/ws".to_string());
        let node = TradeFlowNode {
            key: "pair_buy".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({
                "executionMode": "market",
                "maxPriceCent": 70,
                "minPriceDistanceCent": 1,
                "retryOnMaxPriceBlock": true,
                "executionFloorGuardEnabled": true,
                "executionFloorPriceCent": 80
            }),
        };
        let step = TradeFlowRunStep {
            id: 1,
            run_id: 77,
            node_key: "pair_buy".to_string(),
            node_type: "action.place_order".to_string(),
            status: "queued".to_string(),
            attempt: 1,
            input_json: Some(json!({})),
            output_json: None,
            error_text: None,
            started_at: None,
            ended_at: None,
            available_at: Utc::now(),
            parent_step_id: None,
            idempotency_key: None,
            created_at: Utc::now(),
        };
        let mut context = json!({});

        let selection = resolve_action_place_order_pair_lock_primary_selection(
            None,
            None,
            &ws,
            &executor,
            &pair_lock_test_run(),
            &step,
            &node,
            &mut context,
            "btc-updown-5m-1",
            Some("yes".to_string()),
            Some("no".to_string()),
        )
        .await
        .expect("selection");

        assert!(selection.selection.is_none());
        assert!(selection.waiting);
        assert_eq!(
            selection.failure_reason,
            Some("pair_lock_primary_guard_waiting")
        );
    }

    #[test]
    fn build_pair_lock_primary_waiting_execution_repeats_same_node() {
        let execution = build_pair_lock_primary_waiting_execution(
            "pair_buy",
            "btc-updown-5m-1",
            &json!({
                "selection_mode": "auto_guarded",
                "resolved_yes_token_id": "yes-token",
                "resolved_no_token_id": "no-token",
                "trigger_node_market_slug": "btc-updown-5m-1",
                "yes_candidate_guard": {"decision": "waiting", "reason_code": "above_max_price"},
                "no_candidate_guard": {"decision": "blocked", "reason_code": "below_best_ask_floor"}
            }),
        );

        assert_eq!(
            execution.output.get("reason").and_then(Value::as_str),
            Some("pair_lock_primary_guard_waiting")
        );
        assert_eq!(
            execution
                .output
                .get("resolved_yes_token_id")
                .and_then(Value::as_str),
            Some("yes-token")
        );
        assert!(execution.repeat_at.is_some());
        assert!(execution.routes.is_empty());
    }

    #[test]
    fn pair_lock_primary_ptb_guard_decision_maps_retryable_failures_to_waiting() {
        assert_eq!(pair_lock_primary_ptb_guard_decision(true, true), "passed");
        assert_eq!(pair_lock_primary_ptb_guard_decision(false, true), "waiting");
        assert_eq!(pair_lock_primary_ptb_guard_decision(false, false), "blocked");
    }

    #[test]
    fn pair_lock_primary_logs_ptb_skip_only_when_pre_guard_blocks_with_ptb_enabled() {
        let node = TradeFlowNode {
            key: "pair_buy".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({
                "priceToBeatGuardEnabled": true
            }),
        };
        let node_without_ptb = TradeFlowNode {
            key: "pair_buy".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({}),
        };

        assert!(pair_lock_primary_should_log_ptb_skip(&node, "waiting"));
        assert!(pair_lock_primary_should_log_ptb_skip(&node, "blocked"));
        assert!(!pair_lock_primary_should_log_ptb_skip(&node, "passed"));
        assert!(!pair_lock_primary_should_log_ptb_skip(&node_without_ptb, "waiting"));
    }

    #[test]
    fn pair_lock_primary_ptb_evaluation_log_snapshot_captures_key_fields() {
        let evaluation = crate::trade_flow::guards::price_to_beat::PriceToBeatGuardEvaluation {
            passed: false,
            reason_code: "price_to_beat_gap_below_threshold".to_string(),
            reason_detail: None,
            normalized_outcome_label: Some("down".to_string()),
            direction: Some("down".to_string()),
            market_slug: "eth-updown-5m-1".to_string(),
            event_url: String::new(),
            timeframe: Some("5m".to_string()),
            asset: Some("eth".to_string()),
            price_to_beat: Some(2305.9),
            price_to_beat_status: None,
            price_to_beat_source: None,
            price_to_beat_source_latency_ms: None,
            current_price: Some(2304.55),
            current_price_source: "chainlink_live_data_ws",
            directional_gap: Some(1.35),
            gap_abs: Some(1.35),
            threshold_mode: "manual".to_string(),
            configured_threshold_mode: Some("manual".to_string()),
            base_threshold_value: None,
            base_threshold_unit: None,
            base_threshold_usd: None,
            current_effective_ptb_usd: Some(2.5),
            threshold_value: 250.0,
            threshold_unit: "cent".to_string(),
            threshold_usd: 2.5,
            stop_loss_bump_count: 0,
            stop_loss_bump_applied_count: 0,
            stop_loss_bump_amount: None,
            stop_loss_bump_max_value: None,
            stop_loss_bump_unit: None,
            stop_loss_bump_raw_usd: 0.0,
            stop_loss_bump_usd: 0.0,
            stop_loss_bump_capped: false,
            stop_loss_bump_max_reached: false,
            stop_loss_bump_current_market_excluded: false,
            stop_loss_bump_increment_usd: 0.0,
            reentry_generation: 0,
            reentry_override_active: false,
            reentry_override_value: None,
            reentry_override_unit: None,
            max_price_relax: None,
            auto_threshold_usd: None,
            lookback_windows_used: None,
            current_windows_used: None,
            avg_up_excursion_usd: None,
            avg_down_excursion_usd: None,
            lookback_market_slugs: None,
            lookback_window_snapshots: None,
            baseline_pct: None,
            current_pct: None,
            vol_factor: None,
            threshold_pct: None,
            base_pct: None,
            floor_usd: None,
            ceiling_usd: None,
            threshold_was_clamped: None,
            signal_formula: None,
            iv_mismatch_edge: None,
        };

        let snapshot = pair_lock_primary_ptb_evaluation_log_snapshot(
            715,
            "action_xy5g02",
            "eth-updown-5m-1776677100",
            "Down",
            &evaluation,
        );

        assert_eq!(snapshot.flow_run_id, 715);
        assert_eq!(snapshot.node_key, "action_xy5g02");
        assert_eq!(snapshot.market_slug, "eth-updown-5m-1776677100");
        assert_eq!(snapshot.outcome_label, "Down");
        assert!(!snapshot.ptb_passed);
        assert_eq!(snapshot.ptb_reason_code, "price_to_beat_gap_below_threshold");
        assert_eq!(snapshot.directional_gap, Some(1.35));
        assert_eq!(snapshot.threshold_usd, 2.5);
        assert_eq!(snapshot.current_price, Some(2304.55));
        assert_eq!(snapshot.price_to_beat, Some(2305.9));
    }

    #[test]
    fn pair_lock_primary_selection_prefers_relaxed_ptb_candidate() {
        let quote = PairLockResolvedQuote {
            best_bid: Some(0.69),
            best_ask: Some(0.70),
            last_trade_price: Some(0.70),
            current_price: 0.70,
            quote_source_kind: "test",
            quote_ws_state: "live_ws_not_subscribed",
            quote_event_ts: None,
            quote_snapshot_age_ms: None,
            quote_source_detail: "test".to_string(),
            quote_book_missing_fields: Vec::new(),
            quote_snapshot_used: Value::Null,
        };
        let selection = resolve_action_place_order_pair_lock_primary_selection_attempt(
            ActionPlaceOrderPairLockPrimaryCandidateEval {
                token_id: "yes".to_string(),
                outcome_label: "Up".to_string(),
                decision: "passed",
                reason_code: "passed".to_string(),
                quote: quote.clone(),
                diagnostics: json!({
                    "decision": "passed",
                    "reason_code": "passed",
                    "outcome_label": "Up",
                    "price_to_beat_guard": {
                        "passed": true,
                        "reason_code": "passed",
                        "max_price_relax": { "max_price_relax_applied": true }
                    }
                }),
                adaptive_max_price_override: None,
                manual_adaptive_risk_override: None,
            },
            ActionPlaceOrderPairLockPrimaryCandidateEval {
                token_id: "no".to_string(),
                outcome_label: "Down".to_string(),
                decision: "waiting",
                reason_code: "price_to_beat_gap_below_threshold".to_string(),
                quote,
                diagnostics: json!({
                    "decision": "waiting",
                    "reason_code": "price_to_beat_gap_below_threshold",
                    "outcome_label": "Down",
                    "price_to_beat_guard": {
                        "passed": false,
                        "reason_code": "price_to_beat_gap_below_threshold"
                    }
                }),
                adaptive_max_price_override: None,
                manual_adaptive_risk_override: None,
            },
        );

        assert!(!selection.waiting);
        assert_eq!(
            selection.selection.as_ref().map(|value| value.token_id.as_str()),
            Some("yes")
        );
        assert_eq!(selection.failure_reason, None);
    }

    #[test]
    fn pair_lock_primary_notification_reason_prefers_ptb_then_execution_floor_then_max_price() {
        let diagnostics = json!({
            "selection_mode": "auto_guarded",
            "yes_candidate_guard": {
                "decision": "waiting",
                "reason_code": "below_best_ask_floor",
                "outcome_label": "Up"
            },
            "no_candidate_guard": {
                "decision": "waiting",
                "reason_code": "price_to_beat_gap_below_threshold",
                "outcome_label": "Down"
            }
        });

        let reason =
            resolve_pair_lock_primary_notification_reason(&diagnostics).expect("reason");
        assert_eq!(reason.scope, "price_to_beat");
        assert_eq!(reason.reason_code, "price_to_beat_gap_below_threshold");
        assert_eq!(
            reason
                .secondary_candidate
                .as_ref()
                .and_then(|value| value.get("reason_code"))
                .and_then(Value::as_str),
            Some("below_best_ask_floor")
        );
    }

    #[test]
    fn pair_lock_primary_notification_reason_maps_execution_floor_before_max_price() {
        let diagnostics = json!({
            "selection_mode": "auto_guarded",
            "yes_candidate_guard": {
                "decision": "waiting",
                "reason_code": "below_best_ask_floor",
                "outcome_label": "Up"
            },
            "no_candidate_guard": {
                "decision": "waiting",
                "reason_code": "above_max_price",
                "outcome_label": "Down"
            }
        });

        let reason =
            resolve_pair_lock_primary_notification_reason(&diagnostics).expect("reason");
        assert_eq!(reason.scope, "execution_floor");
        assert_eq!(reason.reason_code, "below_best_ask_floor");
    }
}
