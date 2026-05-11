#[derive(Debug, Clone, PartialEq)]
struct TradeBuilderNoFillReasonSummary {
    scope: String,
    reason_code: String,
    decision: Option<String>,
    payload: Value,
    source_event: Option<String>,
}

fn no_fill_summary(
    scope: &str,
    reason_code: &str,
    decision: Option<&str>,
    payload: Value,
    source_event: Option<&str>,
) -> TradeBuilderNoFillReasonSummary {
    TradeBuilderNoFillReasonSummary {
        scope: scope.to_string(),
        reason_code: reason_code.to_string(),
        decision: decision.map(str::to_string),
        payload,
        source_event: source_event.map(str::to_string),
    }
}

fn no_fill_scope_label(scope: &str) -> &'static str {
    match scope {
        "price_to_beat" => "Price to Beat",
        "max_price" => "Max Fiyat",
        "execution_floor" => "Execution Floor",
        "trigger_price" => "Tetik Fiyat",
        "runtime_price" => "Runtime Fiyat",
        "trigger_condition" => "Trigger Condition",
        "action_failed" => "Action Failed",
        _ => "Bilinmeyen Engel",
    }
}

fn no_fill_payload_details(payload: &Value) -> &Value {
    payload.get("details").unwrap_or(payload)
}

fn no_fill_optional_f64(payload: &Value, path: &[&str]) -> Option<f64> {
    let mut value = payload;
    for key in path {
        value = value.get(*key)?;
    }
    value_as_f64(value)
}

fn no_fill_optional_str<'a>(payload: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut value = payload;
    for key in path {
        value = value.get(*key)?;
    }
    value.as_str()
}

fn no_fill_format_price(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.4}"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn no_fill_format_precise(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.8}"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn parse_namespaced_guard_reason(reason: &str) -> Option<TradeBuilderNoFillReasonSummary> {
    let (scope, reason_code) = reason.split_once(':')?;
    if scope.trim().is_empty() || reason_code.trim().is_empty() {
        return None;
    }
    Some(no_fill_summary(
        scope.trim(),
        reason_code.trim(),
        None,
        Value::Null,
        Some("last_guard_notification_reason"),
    ))
}

fn guard_eval_payload_for_scope(payload: &Value, scope: &str) -> Value {
    let key = match scope {
        "trigger_price" => "trigger_price_guard",
        "execution_floor" => "execution_floor_guard",
        "max_price" => "max_price_guard",
        _ => return payload.clone(),
    };
    payload.get(key).cloned().unwrap_or_else(|| payload.clone())
}

fn no_fill_summary_from_guard_evaluated(
    payload: &Value,
    source_event: &str,
) -> Option<TradeBuilderNoFillReasonSummary> {
    let decision = payload
        .get("effective_decision")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if decision == "passed" {
        return None;
    }
    let scope = payload
        .get("effective_guard_scope")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())?;
    let reason_code = payload
        .get("effective_reason_code")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    Some(no_fill_summary(
        scope,
        reason_code,
        Some(decision),
        guard_eval_payload_for_scope(payload, scope),
        Some(source_event),
    ))
}

fn no_fill_summary_from_price_to_beat_payload(
    payload: &Value,
    source_event: &str,
) -> Option<TradeBuilderNoFillReasonSummary> {
    let guard = payload
        .get("price_to_beat_guard")
        .filter(|value| !value.is_null())?;
    if guard.get("passed").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let reason_code = guard
        .get("reason_code")
        .and_then(Value::as_str)
        .or_else(|| guard.get("reason").and_then(Value::as_str))
        .unwrap_or("price_to_beat_guard_blocked");
    let decision = guard.get("decision").and_then(Value::as_str).or_else(|| {
        if guard.get("passed").and_then(Value::as_bool) == Some(false) {
            Some("blocked")
        } else {
            None
        }
    });
    Some(no_fill_summary(
        "price_to_beat",
        reason_code,
        decision,
        guard.clone(),
        Some(source_event),
    ))
}

fn no_fill_is_blocking_guard(decision: Option<&str>, reason_code: Option<&str>) -> bool {
    let decision = decision.map(str::trim).filter(|value| !value.is_empty());
    let reason_code = reason_code.map(str::trim).filter(|value| !value.is_empty());
    if matches!(decision, Some("passed" | "not_configured")) {
        return false;
    }
    if matches!(reason_code, Some("passed" | "not_configured")) {
        return false;
    }
    decision.is_some() || reason_code.is_some()
}

fn no_fill_pair_lock_scope_from_reason(reason_code: &str) -> Option<&'static str> {
    match reason_code {
        "above_max_price" => Some("max_price"),
        "below_best_ask_floor" | "best_ask_unavailable" => Some("execution_floor"),
        "below_trigger_price_guard" => Some("trigger_price"),
        "price_to_beat_guard_blocked" | "price_to_beat_gap_below_threshold" => {
            Some("price_to_beat")
        }
        _ => None,
    }
}

fn no_fill_summary_from_pair_lock_candidate_guard(
    candidate: &Value,
    source_event: &str,
) -> Option<TradeBuilderNoFillReasonSummary> {
    for (scope, key) in [
        ("price_to_beat", "price_to_beat_guard"),
        ("max_price", "max_price_guard"),
        ("execution_floor", "execution_floor_guard"),
        ("trigger_price", "trigger_price_guard"),
    ] {
        let Some(guard) = candidate.get(key).filter(|value| !value.is_null()) else {
            continue;
        };
        let decision = guard
            .get("decision")
            .and_then(Value::as_str)
            .or_else(|| candidate.get("decision").and_then(Value::as_str));
        let reason_code = guard
            .get("reason_code")
            .and_then(Value::as_str)
            .or_else(|| candidate.get("reason_code").and_then(Value::as_str));
        if !no_fill_is_blocking_guard(decision, reason_code) {
            continue;
        }
        return Some(no_fill_summary(
            scope,
            reason_code.unwrap_or("unknown"),
            decision,
            guard.clone(),
            Some(source_event),
        ));
    }

    let reason_code = candidate.get("reason_code").and_then(Value::as_str)?;
    let scope = no_fill_pair_lock_scope_from_reason(reason_code)?;
    Some(no_fill_summary(
        scope,
        reason_code,
        candidate.get("decision").and_then(Value::as_str),
        candidate.clone(),
        Some(source_event),
    ))
}

fn no_fill_outcome_matches(candidate_outcome: &str, target_outcome: &str) -> bool {
    if candidate_outcome.eq_ignore_ascii_case(target_outcome) {
        return true;
    }
    let candidate_outcome = candidate_outcome.trim().to_ascii_lowercase();
    let target_outcome = target_outcome.trim().to_ascii_lowercase();
    matches!(
        (candidate_outcome.as_str(), target_outcome.as_str()),
        ("up" | "yes", "up" | "yes") | ("down" | "no", "down" | "no")
    )
}

fn no_fill_candidate_matches_target(
    candidate: &Value,
    token_id: &str,
    outcome_label: &str,
) -> bool {
    if candidate.get("token_id").and_then(Value::as_str) == Some(token_id) {
        return true;
    }
    candidate
        .get("outcome_label")
        .and_then(Value::as_str)
        .map(|candidate_outcome| no_fill_outcome_matches(candidate_outcome, outcome_label))
        .unwrap_or(false)
}

fn no_fill_value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn no_fill_pair_lock_candidate_for_target<'a>(
    output: &'a Value,
    token_id: &str,
    outcome_label: &str,
) -> Option<&'a Value> {
    for path in [
        &["no_candidate_guard"][..],
        &["yes_candidate_guard"][..],
        &["primary_selection", "no_candidate_guard"][..],
        &["primary_selection", "yes_candidate_guard"][..],
    ] {
        let Some(value) = no_fill_value_at_path(output, path) else {
            continue;
        };
        if no_fill_candidate_matches_target(value, token_id, outcome_label) {
            return Some(value);
        }
    }
    None
}

fn no_fill_summary_from_action_place_order_output(
    output: &Value,
    token_id: &str,
    outcome_label: &str,
) -> Option<TradeBuilderNoFillReasonSummary> {
    const SOURCE_EVENT: &str = "action_place_order_output";
    if let Some(candidate) =
        no_fill_pair_lock_candidate_for_target(output, token_id, outcome_label)
    {
        return no_fill_summary_from_pair_lock_candidate_guard(candidate, SOURCE_EVENT);
    }
    no_fill_summary_from_price_to_beat_payload(output, SOURCE_EVENT)
        .or_else(|| no_fill_summary_from_guard_evaluated(output, SOURCE_EVENT))
}

fn no_fill_summary_from_simple_order_event(
    event_type: &str,
    payload: &Value,
) -> Option<TradeBuilderNoFillReasonSummary> {
    let (scope, fallback_reason) = match event_type {
        "trigger_price_blocked" | "trigger_price_waiting" => {
            ("trigger_price", "below_trigger_price_guard")
        }
        "execution_floor_blocked" | "execution_floor_waiting" => {
            ("execution_floor", "execution_floor_blocked")
        }
        "max_price_blocked" | "max_price_waiting" => ("max_price", "above_max_price"),
        "price_unavailable_retry" => ("runtime_price", "runtime_price_unavailable"),
        _ => return None,
    };
    let reason_code = payload
        .get("reason_code")
        .and_then(Value::as_str)
        .unwrap_or(fallback_reason);
    Some(no_fill_summary(
        scope,
        reason_code,
        None,
        payload.clone(),
        Some(event_type),
    ))
}

fn build_order_not_filled_guard_summary(
    order: &TradeBuilderOrder,
    events: &[TradeBuilderOrderEventRecord],
) -> Option<TradeBuilderNoFillReasonSummary> {
    for event in events.iter().rev() {
        let summary = match event.event_type.as_str() {
            "guard_evaluated" => {
                no_fill_summary_from_guard_evaluated(&event.payload_json, event.event_type.as_str())
            }
            "flow_created" | "flow_rearmed" => no_fill_summary_from_price_to_beat_payload(
                &event.payload_json,
                event.event_type.as_str(),
            ),
            event_type => no_fill_summary_from_simple_order_event(event_type, &event.payload_json),
        };
        if summary.is_some() {
            return summary;
        }
    }
    order
        .last_guard_notification_reason
        .as_deref()
        .and_then(parse_namespaced_guard_reason)
}

fn no_fill_summary_from_trade_flow_event(
    event: &TradeFlowEventRecord,
) -> Option<TradeBuilderNoFillReasonSummary> {
    let payload = &event.payload_json;
    match event.event_type.as_str() {
        "pre_order_price_to_beat_blocked" => {
            no_fill_summary_from_price_to_beat_payload(payload, event.event_type.as_str())
        }
        "trigger_cycle_window_price_to_beat_gate_blocked"
        | "trigger_ws_price_to_beat_gate_blocked" => {
            let guard = payload
                .get("price_to_beat_trigger_gate")
                .filter(|value| !value.is_null())?;
            let reason_code = guard
                .get("reason")
                .and_then(Value::as_str)
                .or_else(|| guard.get("reason_code").and_then(Value::as_str))
                .unwrap_or("price_to_beat_trigger_gate_blocked");
            Some(no_fill_summary(
                "price_to_beat",
                reason_code,
                Some("blocked"),
                guard.clone(),
                Some(event.event_type.as_str()),
            ))
        }
        "trigger_cycle_window_above_max" => Some(no_fill_summary(
            "max_price",
            "above_max_price",
            Some("blocked"),
            payload.clone(),
            Some(event.event_type.as_str()),
        )),
        "trigger_cycle_window_condition_not_met" => {
            let reason_code = payload
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("condition_not_met");
            let scope = if reason_code == "price_unavailable" {
                "runtime_price"
            } else {
                "trigger_condition"
            };
            Some(no_fill_summary(
                scope,
                reason_code,
                Some("blocked"),
                payload.clone(),
                Some(event.event_type.as_str()),
            ))
        }
        _ => None,
    }
}

fn trade_flow_event_matches_market_node(
    event: &TradeFlowEventRecord,
    node_key: &str,
    market_slug: &str,
    token_id: &str,
) -> bool {
    let payload = &event.payload_json;
    let event_node_key = payload.get("node_key").and_then(Value::as_str);
    if event_node_key != Some(node_key) {
        return false;
    }
    let event_market_slug = payload
        .get("market_slug")
        .and_then(Value::as_str)
        .or_else(|| payload.get("resolved_market_slug").and_then(Value::as_str));
    if event_market_slug != Some(market_slug) {
        return false;
    }
    let event_token_id = payload
        .get("token_id")
        .and_then(Value::as_str)
        .or_else(|| payload.get("dirty_token_id").and_then(Value::as_str));
    event_token_id == Some(token_id)
}

fn latest_trade_flow_no_fill_summary(
    events: &[TradeFlowEventRecord],
    node_key: &str,
    market_slug: &str,
    token_id: &str,
) -> Option<TradeBuilderNoFillReasonSummary> {
    events
        .iter()
        .filter(|event| {
            trade_flow_event_matches_market_node(event, node_key, market_slug, token_id)
        })
        .find_map(no_fill_summary_from_trade_flow_event)
}

fn append_price_to_beat_no_fill_lines(lines: &mut Vec<String>, payload: &Value) {
    let guard = payload.get("price_to_beat_guard").unwrap_or(payload);
    if let Some(detail) = guard.get("reason_detail").and_then(Value::as_str) {
        lines.push(format!("Detay: {detail}"));
    }
    lines.push(format!(
        "Price to Beat: {}",
        no_fill_format_precise(no_fill_optional_f64(guard, &["price_to_beat"]))
    ));
    lines.push(format!(
        "Current: {}",
        no_fill_format_precise(no_fill_optional_f64(guard, &["current_price"]))
    ));
    lines.push(format!(
        "Yonsel Fark: {}",
        no_fill_format_precise(no_fill_optional_f64(guard, &["directional_gap"]))
    ));
    let limit = no_fill_optional_f64(guard, &["threshold_usd"])
        .or_else(|| no_fill_optional_f64(guard, &["min_gap"]));
    lines.push(format!("Limit: {}", no_fill_format_precise(limit)));
}

fn append_max_price_no_fill_lines(lines: &mut Vec<String>, payload: &Value) {
    let details = no_fill_payload_details(payload);
    lines.push(format!(
        "Referans: {}",
        no_fill_format_price(
            no_fill_optional_f64(details, &["reference_price"])
                .or_else(|| no_fill_optional_f64(payload, &["price"]))
        )
    ));
    if let Some(source) = no_fill_optional_str(details, &["reference_price_source"])
        .or_else(|| no_fill_optional_str(payload, &["price_mode"]))
    {
        lines.push(format!("Referans Kaynagi: {source}"));
    }
    lines.push(format!(
        "Max: {}",
        no_fill_format_price(
            no_fill_optional_f64(details, &["max_price"])
                .or_else(|| no_fill_optional_f64(payload, &["max_price"]))
        )
    ));
    lines.push(format!(
        "Desired: {}",
        no_fill_format_price(no_fill_optional_f64(details, &["desired_price"]))
    ));
}

fn append_execution_floor_no_fill_lines(lines: &mut Vec<String>, payload: &Value) {
    let details = no_fill_payload_details(payload);
    lines.push(format!(
        "Best Ask: {}",
        no_fill_format_price(
            no_fill_optional_f64(details, &["best_ask"])
                .or_else(|| no_fill_optional_f64(payload, &["best_ask"]))
        )
    ));
    lines.push(format!(
        "Floor: {}",
        no_fill_format_price(
            no_fill_optional_f64(details, &["best_ask_floor_price"])
                .or_else(|| no_fill_optional_f64(payload, &["best_ask_floor_price"]))
        )
    ));
}

fn append_trigger_price_no_fill_lines(lines: &mut Vec<String>, payload: &Value) {
    let details = no_fill_payload_details(payload);
    lines.push(format!(
        "Referans: {}",
        no_fill_format_price(
            no_fill_optional_f64(details, &["trigger_guard_reference_price"])
                .or_else(|| no_fill_optional_f64(payload, &["trigger_guard_reference_price"]))
                .or_else(|| no_fill_optional_f64(payload, &["price"]))
        )
    ));
    if let Some(source) = no_fill_optional_str(details, &["trigger_guard_reference_source"])
        .or_else(|| no_fill_optional_str(payload, &["trigger_guard_reference_source"]))
    {
        lines.push(format!("Referans Kaynagi: {source}"));
    }
    lines.push(format!(
        "Guard: {}",
        no_fill_format_price(
            no_fill_optional_f64(details, &["guard_trigger_price"])
                .or_else(|| no_fill_optional_f64(payload, &["guard_trigger_price"]))
                .or_else(|| no_fill_optional_f64(payload, &["trigger_price"]))
        )
    ));
}

fn build_no_fill_guard_summary_block(summary: &TradeBuilderNoFillReasonSummary) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Son Engel: {}",
        no_fill_scope_label(&summary.scope)
    ));
    lines.push(format!("Engel Kodu: {}", summary.reason_code));
    if let Some(decision) = summary.decision.as_deref() {
        lines.push(format!("Karar: {decision}"));
    }
    match summary.scope.as_str() {
        "price_to_beat" => append_price_to_beat_no_fill_lines(&mut lines, &summary.payload),
        "max_price" => append_max_price_no_fill_lines(&mut lines, &summary.payload),
        "execution_floor" => append_execution_floor_no_fill_lines(&mut lines, &summary.payload),
        "trigger_price" => append_trigger_price_no_fill_lines(&mut lines, &summary.payload),
        "runtime_price" => {
            if let Some(reason) = summary.payload.get("reason").and_then(Value::as_str) {
                lines.push(format!("Detay: {reason}"));
            }
        }
        "trigger_condition" => {
            lines.push(format!(
                "Fiyat: {}",
                no_fill_format_price(no_fill_optional_f64(&summary.payload, &["price"]))
            ));
            lines.push(format!(
                "Trigger: {}",
                no_fill_format_price(no_fill_optional_f64(&summary.payload, &["trigger_price"]))
            ));
        }
        _ => {}
    }
    if let Some(source_event) = summary.source_event.as_deref() {
        lines.push(format!("Kaynak Event: {source_event}"));
    }
    format!("\n{}", lines.join("\n"))
}

fn build_no_fill_missing_guard_block() -> &'static str {
    "\nSon Engel: Kayit Yok\nDetay: Guard sebebi kaydedilmedi; window kapanmadan submit tamamlanamadi."
}
