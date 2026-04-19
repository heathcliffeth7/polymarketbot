const PAIR_LOCK_PRIMARY_GUARD_RETRY_DELAY_MS: i64 = 150;
const FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_WAITING_MARKET_SLUG: &str =
    "pair_lock_primary_waiting_market_slug";
const FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_WAITING_SIGNATURE: &str =
    "pair_lock_primary_waiting_signature";
const FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_MARKET_SLUG: &str =
    "pair_lock_primary_notification_market_slug";
const FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_REASON: &str =
    "pair_lock_primary_notification_reason";
const FLOW_NODE_STATE_PAIR_LOCK_PRIMARY_NOTIFICATION_SCOPE: &str =
    "pair_lock_primary_notification_scope";

#[derive(Debug, Clone)]
struct ActionPlaceOrderPairLockPrimaryCandidateEval {
    token_id: String,
    outcome_label: String,
    decision: &'static str,
    reason_code: String,
    quote: PairLockResolvedQuote,
    diagnostics: Value,
}

#[derive(Debug, Clone)]
struct ActionPlaceOrderPairLockPrimarySelection {
    token_id: String,
    outcome_label: String,
    selection_mode: &'static str,
    guard_reason: String,
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
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &Value,
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
    if decision == "passed" && node_config_bool(node, "priceToBeatGuardEnabled").unwrap_or(false) {
        let resolution =
            crate::trade_flow::guards::price_to_beat::resolve_action_place_order_price_to_beat_guard_resolution(
                node,
                context,
                market_slug,
            )?;
        let mut evaluation = crate::trade_flow::guards::price_to_beat::evaluate_price_to_beat_guard(
            market_slug,
            resolution.effective_mode,
            resolution.threshold_value,
            resolution.threshold_unit,
            outcome_label,
        )
        .await;
        resolution.apply_metadata(&mut evaluation);
        if resolution.effective_mode
            != crate::trade_flow::guards::price_to_beat::PriceToBeatMode::Manual
        {
            crate::trade_flow::guards::price_to_beat::apply_price_to_beat_risk_penalty(
                &mut evaluation,
                resolution.stop_loss_bump_usd,
            );
        }
        ptb_guard = evaluation.to_value();
        decision = pair_lock_primary_ptb_guard_decision(
            evaluation.passed,
            node_config_bool(node, "retryOnPriceToBeatGuardBlock").unwrap_or(true),
        );
        if decision != "passed" {
            reason_code = evaluation.reason_code.clone();
        }
    }

    Ok(ActionPlaceOrderPairLockPrimaryCandidateEval {
        token_id: token_id.to_string(),
        outcome_label: outcome_label.to_string(),
        decision,
        reason_code: reason_code.clone(),
        quote: quote.clone(),
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

async fn resolve_action_place_order_pair_lock_primary_selection(
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &Value,
    market_slug: &str,
    yes_token_id: Option<String>,
    no_token_id: Option<String>,
) -> Result<ActionPlaceOrderPairLockPrimarySelectionAttempt> {
    let (up_label, down_label) = pair_lock_primary_outcome_labels(market_slug);
    let yes_token_id = yes_token_id
        .ok_or_else(|| anyhow::anyhow!("pair_lock auto primary selection requires yesTokenId"))?;
    let no_token_id = no_token_id
        .ok_or_else(|| anyhow::anyhow!("pair_lock auto primary selection requires noTokenId"))?;

    let up_candidate = evaluate_action_place_order_pair_lock_primary_candidate(
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
    let down_candidate = evaluate_action_place_order_pair_lock_primary_candidate(
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
        return Ok(ActionPlaceOrderPairLockPrimarySelectionAttempt {
            selection: Some(ActionPlaceOrderPairLockPrimarySelection {
                token_id: selected.token_id.clone(),
                outcome_label: selected.outcome_label.clone(),
                selection_mode: "auto_guarded",
                guard_reason: selected.reason_code.clone(),
            }),
            waiting: false,
            failure_reason: None,
            yes_candidate: up_candidate,
            no_candidate: down_candidate,
            diagnostics,
        });
    }

    if passing.is_empty() && !waiting.is_empty() {
        return Ok(ActionPlaceOrderPairLockPrimarySelectionAttempt {
            selection: None,
            waiting: true,
            failure_reason: Some("pair_lock_primary_guard_waiting"),
            yes_candidate: up_candidate,
            no_candidate: down_candidate,
            diagnostics,
        });
    }

    let failure_reason = if passing.is_empty() {
        "pair_lock_no_primary_leg_passed"
    } else {
        "pair_lock_primary_leg_ambiguous"
    };
    Ok(ActionPlaceOrderPairLockPrimarySelectionAttempt {
        selection: None,
        waiting: false,
        failure_reason: Some(failure_reason),
        yes_candidate: up_candidate,
        no_candidate: down_candidate,
        diagnostics,
    })
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

        let selection = resolve_action_place_order_pair_lock_primary_selection(
            &ws,
            &executor,
            &pair_lock_test_run(),
            &step,
            &node,
            &json!({}),
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

        let selection = resolve_action_place_order_pair_lock_primary_selection(
            &ws,
            &executor,
            &pair_lock_test_run(),
            &step,
            &node,
            &json!({}),
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

        let selection = resolve_action_place_order_pair_lock_primary_selection(
            &ws,
            &executor,
            &pair_lock_test_run(),
            &step,
            &node,
            &json!({}),
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
