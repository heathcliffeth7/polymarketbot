struct NoOrderMarketTimelineContext<'a> {
    node_key: &'a str,
    market_slug: &'a str,
    token_id: &'a str,
    outcome_label: &'a str,
    window_end_at: DateTime<Utc>,
    summary: &'a TradeBuilderNoFillReasonSummary,
    events: &'a [TradeFlowEventRecord],
    action_steps: &'a [TradeFlowRunStep],
}

fn no_order_parse_rfc3339(value: Option<&str>) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value?).ok().map(|value| value.with_timezone(&Utc))
}

fn no_order_json_i64(value: Option<i64>) -> Value {
    value.map(Value::from).unwrap_or(Value::Null)
}

fn no_order_timeline_ms_since(
    value: Option<DateTime<Utc>>,
    baseline: Option<DateTime<Utc>>,
) -> Option<i64> {
    Some(value?.signed_duration_since(baseline?).num_milliseconds())
}

fn no_order_timeline_time(value: Option<DateTime<Utc>>) -> Value {
    value
        .map(|value| json!(value.to_rfc3339()))
        .unwrap_or(Value::Null)
}

fn no_order_payload_str<'a>(payload: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| payload.get(*key)?.as_str())
}

fn no_order_payload_i64(payload: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| payload.get(*key)?.as_i64())
}

fn no_order_event_payload_matches_node(payload: &Value, node_key: &str) -> bool {
    no_order_payload_str(payload, &["node_key"]) == Some(node_key)
}

fn no_order_event_payload_matches_market(payload: &Value, market_slug: &str) -> bool {
    no_order_payload_str(
        payload,
        &["market_slug", "resolved_market_slug", "new_market_slug"],
    ) == Some(market_slug)
}

fn no_order_event_payload_matches_token(payload: &Value, token_id: &str) -> bool {
    no_order_payload_str(payload, &["token_id", "dirty_token_id"]) == Some(token_id)
}

fn no_order_expected_market_start(
    rotation_event: Option<&TradeFlowEventRecord>,
    market_slug: &str,
) -> Option<DateTime<Utc>> {
    rotation_event
        .and_then(|event| {
            no_order_parse_rfc3339(
                event
                    .payload_json
                    .get("expected_market_start")
                    .and_then(Value::as_str),
            )
        })
        .or_else(|| trade_builder_second_snapshot_window(market_slug).map(|(start, _)| start))
}

fn no_order_rotation_event<'a>(
    events: &'a [TradeFlowEventRecord],
    node_key: &str,
    market_slug: &str,
) -> Option<&'a TradeFlowEventRecord> {
    events
        .iter()
        .filter(|event| event.event_type == "trigger_auto_scope_market_rotated")
        .filter(|event| no_order_event_payload_matches_node(&event.payload_json, node_key))
        .filter(|event| {
            event
                .payload_json
                .get("new_market_slug")
                .and_then(Value::as_str)
                == Some(market_slug)
        })
        .min_by_key(|event| event.created_at)
}

fn no_order_first_trigger_event<'a>(
    events: &'a [TradeFlowEventRecord],
    node_key: &str,
    market_slug: &str,
    token_id: &str,
) -> Option<&'a TradeFlowEventRecord> {
    events
        .iter()
        .filter(|event| {
            matches!(
                event.event_type.as_str(),
                "trigger_ws_price_enqueued" | "trigger_cycle_window_condition_met"
            )
        })
        .filter(|event| no_order_event_payload_matches_node(&event.payload_json, node_key))
        .filter(|event| no_order_event_payload_matches_market(&event.payload_json, market_slug))
        .filter(|event| no_order_event_payload_matches_token(&event.payload_json, token_id))
        .min_by_key(|event| event.created_at)
}

fn no_order_first_action_step(action_steps: &[TradeFlowRunStep]) -> Option<&TradeFlowRunStep> {
    action_steps.iter().min_by_key(|step| no_order_action_start_time(step))
}

fn no_order_action_start_time(step: &TradeFlowRunStep) -> DateTime<Utc> {
    step.started_at.unwrap_or(step.available_at)
}

fn no_order_step_ptb_guard<'a>(
    step: &'a TradeFlowRunStep,
    token_id: &str,
    outcome_label: &str,
) -> Option<&'a Value> {
    let output = step.output_json.as_ref()?;
    no_fill_pair_lock_candidate_for_target(output, token_id, outcome_label)?
        .get("price_to_beat_guard")
        .filter(|guard| !guard.is_null())
}

fn no_order_first_ptb_guard<'a>(
    action_steps: &'a [TradeFlowRunStep],
    token_id: &str,
    outcome_label: &str,
) -> Option<(&'a TradeFlowRunStep, &'a Value)> {
    action_steps
        .iter()
        .filter_map(|step| Some((step, no_order_step_ptb_guard(step, token_id, outcome_label)?)))
        .min_by_key(|(step, _)| no_order_step_time(step))
}

fn no_order_first_action_blocker<'a>(
    action_steps: &'a [TradeFlowRunStep],
    token_id: &str,
    outcome_label: &str,
) -> Option<(&'a TradeFlowRunStep, TradeBuilderNoFillReasonSummary)> {
    action_steps
        .iter()
        .filter_map(|step| {
            let summary = no_fill_summary_from_action_place_order_output(
                step.output_json.as_ref()?,
                token_id,
                outcome_label,
            )?;
            Some((step, summary))
        })
        .min_by_key(|(step, _)| no_order_step_time(step))
}

fn no_order_current_ptb_cache_payload(
    market_slug: &str,
    expected_market_start: Option<DateTime<Utc>>,
) -> Value {
    let Some(snapshot) =
        crate::trade_flow::guards::polymarket_price_to_beat::get_price_to_beat_cached(market_slug)
    else {
        return json!({
            "ptb_cache_status": "unavailable",
            "ptb_cache_source": Value::Null,
            "ptb_cache_fetched_at": Value::Null,
            "ptb_cache_lag_ms": Value::Null,
            "ptb_cache_source_latency_ms": Value::Null,
            "ptb_cache_price_to_beat": Value::Null,
        });
    };
    json!({
        "ptb_cache_status": snapshot.status(),
        "ptb_cache_source": snapshot.source.as_str(),
        "ptb_cache_fetched_at": snapshot.fetched_at.to_rfc3339(),
        "ptb_cache_lag_ms": no_order_json_i64(no_order_timeline_ms_since(
            Some(snapshot.fetched_at),
            expected_market_start,
        )),
        "ptb_cache_source_latency_ms": no_order_json_i64(snapshot.source_latency_ms),
        "ptb_cache_price_to_beat": no_order_json_f64(Some(snapshot.price_to_beat)),
    })
}

fn no_order_primary_reason_label(scope: &str, reason_code: &str) -> &'static str {
    match (scope, reason_code) {
        ("price_to_beat", "price_to_beat_gap_below_threshold") => "ptb_gap_below_threshold",
        ("price_to_beat", "price_to_beat_pending") => "ptb_pending",
        ("max_price", "above_max_price") => "above_max_price",
        ("execution_floor", "below_best_ask_floor") => "below_best_ask_floor",
        ("action_failed", _) => "action_failed",
        ("trigger_condition", _) => "trigger_condition_not_met",
        _ => "guard_blocked",
    }
}

fn build_no_order_market_timeline_payload(input: NoOrderMarketTimelineContext<'_>) -> Value {
    let rotation = no_order_rotation_event(input.events, input.node_key, input.market_slug);
    let expected_market_start =
        no_order_expected_market_start(rotation, input.market_slug);
    let rotation_payload = rotation.map(|event| &event.payload_json);
    let rotation_detected_at = rotation_payload
        .and_then(|payload| {
            no_order_parse_rfc3339(payload.get("rotation_detected_at").and_then(Value::as_str))
        })
        .or_else(|| rotation.map(|event| event.created_at));
    let first_trigger =
        no_order_first_trigger_event(input.events, input.node_key, input.market_slug, input.token_id);
    let first_trigger_at = first_trigger
        .and_then(|event| {
            no_order_parse_rfc3339(event.payload_json.get("queued_at").and_then(Value::as_str))
        })
        .or_else(|| first_trigger.map(|event| event.created_at));
    let first_action = no_order_first_action_step(input.action_steps);
    let first_action_at = first_action.map(no_order_action_start_time);
    let first_ptb_guard = no_order_first_ptb_guard(
        input.action_steps,
        input.token_id,
        input.outcome_label,
    );
    let first_blocker = no_order_first_action_blocker(
        input.action_steps,
        input.token_id,
        input.outcome_label,
    );
    let first_ptb_guard_at = first_ptb_guard.map(|(step, _)| no_order_step_time(step));
    let first_ptb_guard_payload = first_ptb_guard.map(|(_, guard)| guard);

    let mut payload = json!({
        "market_timeline_status": "ready",
        "market_start_at": no_order_timeline_time(expected_market_start),
        "window_end_at": input.window_end_at.to_rfc3339(),
        "market_rotation_status": if rotation.is_some() { "rotated" } else { "not_recorded" },
        "rotation_detected_at": no_order_timeline_time(rotation_detected_at),
        "rotation_lag_ms": no_order_json_i64(
            rotation_payload
                .and_then(|payload| no_order_payload_i64(payload, &["rotation_lag_ms"]))
                .or_else(|| no_order_timeline_ms_since(rotation_detected_at, expected_market_start)),
        ),
        "market_selection_reason": rotation_payload
            .and_then(|payload| no_order_payload_str(payload, &["selection_reason"]))
            .unwrap_or("unknown"),
        "first_trigger_event": first_trigger
            .map(|event| event.event_type.as_str())
            .unwrap_or("not_recorded"),
        "first_trigger_at": no_order_timeline_time(first_trigger_at),
        "first_trigger_lag_ms": no_order_json_i64(no_order_timeline_ms_since(
            first_trigger_at,
            expected_market_start,
        )),
        "first_action_at": no_order_timeline_time(first_action_at),
        "first_action_lag_ms": no_order_json_i64(no_order_timeline_ms_since(
            first_action_at,
            expected_market_start,
        )),
        "first_action_status": first_action.map(|step| step.status.as_str()).unwrap_or("not_recorded"),
        "first_action_blocker_scope": first_blocker
            .as_ref()
            .map(|(_, summary)| summary.scope.as_str())
            .unwrap_or("not_recorded"),
        "first_action_blocker_code": first_blocker
            .as_ref()
            .map(|(_, summary)| summary.reason_code.as_str())
            .unwrap_or("not_recorded"),
        "first_action_blocker_decision": first_blocker
            .as_ref()
            .and_then(|(_, summary)| summary.decision.as_deref())
            .unwrap_or("not_recorded"),
        "first_ptb_guard_at": no_order_timeline_time(first_ptb_guard_at),
        "first_ptb_guard_lag_ms": no_order_json_i64(no_order_timeline_ms_since(
            first_ptb_guard_at,
            expected_market_start,
        )),
        "first_ptb_guard_reason_code": first_ptb_guard_payload
            .and_then(|guard| guard.get("reason_code").and_then(Value::as_str))
            .unwrap_or("not_recorded"),
        "first_ptb_guard_source": first_ptb_guard_payload
            .and_then(|guard| guard.get("price_to_beat_source").and_then(Value::as_str))
            .unwrap_or("not_recorded"),
        "first_ptb_guard_status": first_ptb_guard_payload
            .and_then(|guard| guard.get("price_to_beat_status").and_then(Value::as_str))
            .unwrap_or("not_recorded"),
        "first_ptb_guard_price_to_beat": no_order_json_f64(first_ptb_guard_payload
            .and_then(|guard| no_fill_optional_f64(guard, &["price_to_beat"]))),
        "first_ptb_guard_current_price": no_order_json_f64(first_ptb_guard_payload
            .and_then(|guard| no_fill_optional_f64(guard, &["current_price"]))),
        "first_ptb_guard_directional_gap": no_order_json_f64(first_ptb_guard_payload
            .and_then(|guard| no_fill_optional_f64(guard, &["directional_gap"]))),
        "first_ptb_guard_threshold_usd": no_order_json_f64(first_ptb_guard_payload
            .and_then(|guard| no_fill_optional_f64(guard, &["threshold_usd"]))),
        "first_ptb_guard_source_latency_ms": no_order_json_i64(first_ptb_guard_payload
            .and_then(|guard| guard.get("price_to_beat_source_latency_ms").and_then(Value::as_i64))),
        "no_order_primary_reason": no_order_primary_reason_label(
            &input.summary.scope,
            &input.summary.reason_code,
        ),
    });
    if let Some(obj) = payload.as_object_mut() {
        if let Some(cache_obj) =
            no_order_current_ptb_cache_payload(input.market_slug, expected_market_start).as_object()
        {
            for (key, value) in cache_obj {
                obj.insert(key.clone(), value.clone());
            }
        }
    }
    payload
}

fn no_order_format_ms(value: Option<i64>) -> String {
    value
        .map(|value| format!("{value} ms"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn append_no_order_market_timeline_lines(lines: &mut Vec<String>, diagnosis: &Value) {
    if no_order_diag_str(diagnosis, "market_timeline_status").is_none() {
        return;
    }
    lines.push(String::new());
    lines.push("Market / PTB Timeline".to_string());
    lines.push(format!(
        "Market start: {}",
        no_order_diag_str(diagnosis, "market_start_at").unwrap_or("N/A")
    ));
    lines.push(format!(
        "Rotation: {} | lag {} | selection {}",
        no_order_diag_str(diagnosis, "market_rotation_status").unwrap_or("N/A"),
        no_order_format_ms(diagnosis.get("rotation_lag_ms").and_then(Value::as_i64)),
        no_order_diag_str(diagnosis, "market_selection_reason").unwrap_or("N/A")
    ));
    lines.push(format!(
        "Ilk trigger: {} | lag {}",
        no_order_diag_str(diagnosis, "first_trigger_event").unwrap_or("N/A"),
        no_order_format_ms(diagnosis.get("first_trigger_lag_ms").and_then(Value::as_i64))
    ));
    lines.push(format!(
        "Ilk action: {} | lag {}",
        no_order_diag_str(diagnosis, "first_action_status").unwrap_or("N/A"),
        no_order_format_ms(diagnosis.get("first_action_lag_ms").and_then(Value::as_i64))
    ));
    lines.push(format!(
        "Ilk blocker: {}:{} ({})",
        no_order_diag_str(diagnosis, "first_action_blocker_scope").unwrap_or("N/A"),
        no_order_diag_str(diagnosis, "first_action_blocker_code").unwrap_or("N/A"),
        no_order_diag_str(diagnosis, "first_action_blocker_decision").unwrap_or("N/A")
    ));
    lines.push(format!(
        "Ilk PTB guard: {} | source={} status={} | lag {}",
        no_order_diag_str(diagnosis, "first_ptb_guard_reason_code").unwrap_or("N/A"),
        no_order_diag_str(diagnosis, "first_ptb_guard_source").unwrap_or("N/A"),
        no_order_diag_str(diagnosis, "first_ptb_guard_status").unwrap_or("N/A"),
        no_order_format_ms(diagnosis.get("first_ptb_guard_lag_ms").and_then(Value::as_i64))
    ));
    lines.push(format!(
        "Ilk PTB gap/limit: {} / {}",
        no_fill_format_precise(no_order_diag_f64(diagnosis, "first_ptb_guard_directional_gap")),
        no_fill_format_precise(no_order_diag_f64(diagnosis, "first_ptb_guard_threshold_usd"))
    ));
    lines.push(format!(
        "PTB cache: source={} status={} | fetched lag {} | source latency {}",
        no_order_diag_str(diagnosis, "ptb_cache_source").unwrap_or("N/A"),
        no_order_diag_str(diagnosis, "ptb_cache_status").unwrap_or("N/A"),
        no_order_format_ms(diagnosis.get("ptb_cache_lag_ms").and_then(Value::as_i64)),
        no_order_format_ms(
            diagnosis
                .get("ptb_cache_source_latency_ms")
                .and_then(Value::as_i64)
        )
    ));
    lines.push(format!(
        "Primary reason: {}",
        no_order_diag_str(diagnosis, "no_order_primary_reason").unwrap_or("guard_blocked")
    ));
}
