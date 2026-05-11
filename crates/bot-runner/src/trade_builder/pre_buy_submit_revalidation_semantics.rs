const LIVE_GAP_SUBMIT_REVALIDATION_NOTIFICATION_STATE_KEY: &str =
    "liveGapSubmitRevalidationNotificationState";
const LIVE_GAP_SUBMIT_REVALIDATION_TRIGGER_FLOOR_BREACH: &str = "floor_breach";
const LIVE_GAP_SUBMIT_REVALIDATION_TRIGGER_CANDIDATE_STALE: &str = "candidate_stale";
const LIVE_GAP_SUBMIT_REVALIDATION_TRIGGER_SUBMIT_BEFORE_CLOB: &str = "submit_before_clob";
const LIVE_GAP_SUBMIT_REVALIDATION_FRESH_PASS: &str = "PASS";
const LIVE_GAP_SUBMIT_REVALIDATION_BLOCK_RETRY: &str = "BLOCK_RETRY";
const LIVE_GAP_SUBMIT_REVALIDATION_BLOCK_TERMINAL: &str = "BLOCK_TERMINAL";
const LIVE_GAP_SUBMIT_REVALIDATION_CLOB_ALLOWED: &str = "CLOB_SUBMIT_ALLOWED";
const LIVE_GAP_SUBMIT_REVALIDATION_CLOB_NOT_SUBMITTED: &str = "CLOB_NOT_SUBMITTED";
const LIVE_GAP_SUBMIT_REVALIDATION_DECISION_REASON_PASS: &str = "fresh_revalidation_passed";
const LIVE_GAP_SUBMIT_REVALIDATION_LATE_HIGH_REMAINING_SEC: i64 = 30;
const LIVE_GAP_SUBMIT_REVALIDATION_LATE_HIGH_PRICE: f64 = 0.85;

fn live_gap_submit_revalidation_triggers(
    candidate_stale: bool,
    floor_invalidated: bool,
) -> Vec<&'static str> {
    let mut triggers = Vec::new();
    if floor_invalidated {
        triggers.push(LIVE_GAP_SUBMIT_REVALIDATION_TRIGGER_FLOOR_BREACH);
    }
    if candidate_stale {
        triggers.push(LIVE_GAP_SUBMIT_REVALIDATION_TRIGGER_CANDIDATE_STALE);
    }
    triggers.push(LIVE_GAP_SUBMIT_REVALIDATION_TRIGGER_SUBMIT_BEFORE_CLOB);
    triggers
}

fn live_gap_submit_revalidation_candidate_reuse_decision(
    candidate_stale: bool,
    floor_invalidated: bool,
) -> &'static str {
    if floor_invalidated {
        "reuse_denied_floor_breach"
    } else if candidate_stale {
        "reuse_denied_revalidation_required"
    } else {
        "reuse_allowed"
    }
}

fn live_gap_submit_revalidation_fresh_decision(
    decision: &LiveGapSubmitRevalidationDecision,
) -> &'static str {
    if decision.passed {
        LIVE_GAP_SUBMIT_REVALIDATION_FRESH_PASS
    } else if decision.terminal {
        LIVE_GAP_SUBMIT_REVALIDATION_BLOCK_TERMINAL
    } else {
        LIVE_GAP_SUBMIT_REVALIDATION_BLOCK_RETRY
    }
}

fn live_gap_submit_revalidation_decision_reason(
    decision: &LiveGapSubmitRevalidationDecision,
) -> &'static str {
    if decision.passed {
        LIVE_GAP_SUBMIT_REVALIDATION_DECISION_REASON_PASS
    } else {
        decision.reason_code
    }
}

fn live_gap_submit_revalidation_clob_decision(
    decision: &LiveGapSubmitRevalidationDecision,
) -> &'static str {
    if decision.passed {
        LIVE_GAP_SUBMIT_REVALIDATION_CLOB_ALLOWED
    } else {
        LIVE_GAP_SUBMIT_REVALIDATION_CLOB_NOT_SUBMITTED
    }
}

fn live_gap_submit_revalidation_payload_f64(payload: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| payload.get(*key).and_then(value_as_f64))
}

fn live_gap_submit_revalidation_payload_i64(payload: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|key| payload.get(*key).and_then(value_as_i64))
}

fn live_gap_submit_revalidation_fresh_snapshot_age_ms(payload: &Value, now_ms: i64) -> Option<i64> {
    if let Some(staleness) = live_gap_submit_revalidation_payload_i64(
        payload,
        &["fresh_snapshot_age_ms", "binance_staleness_ms"],
    ) {
        return Some(staleness.max(0));
    }
    live_gap_submit_revalidation_payload_i64(payload, &["current_price_ts_ms", "binance_price_ts"])
        .map(|ts_ms| (now_ms - ts_ms).max(0))
}

fn live_gap_submit_revalidation_late_high_price_risk(payload: &Value) -> Option<Value> {
    let remaining_sec =
        live_gap_submit_revalidation_payload_i64(payload, &["remaining_sec", "candidate_remaining_sec"])?;
    let effective_fill = live_gap_submit_revalidation_payload_f64(
        payload,
        &[
            "effective_fill",
            "effective_fill_price",
            "candidate_effective_fill",
            "best_ask",
        ],
    )?;
    (remaining_sec <= LIVE_GAP_SUBMIT_REVALIDATION_LATE_HIGH_REMAINING_SEC
        && effective_fill >= LIVE_GAP_SUBMIT_REVALIDATION_LATE_HIGH_PRICE)
        .then(|| {
            json!({
                "enabled": true,
                "mode": "notify_only",
                "remaining_sec": remaining_sec,
                "effective_fill": effective_fill,
                "threshold_remaining_sec": LIVE_GAP_SUBMIT_REVALIDATION_LATE_HIGH_REMAINING_SEC,
                "threshold_price": LIVE_GAP_SUBMIT_REVALIDATION_LATE_HIGH_PRICE
            })
        })
}

fn copy_live_gap_submit_revalidation_notification_state(source: &Value, target: &mut Value) {
    if let Some(state) =
        flow_context_value(source, LIVE_GAP_SUBMIT_REVALIDATION_NOTIFICATION_STATE_KEY)
    {
        set_flow_context(
            target,
            LIVE_GAP_SUBMIT_REVALIDATION_NOTIFICATION_STATE_KEY,
            state,
        );
    }
}

fn annotate_live_gap_submit_revalidation_payload(
    decision: &mut LiveGapSubmitRevalidationDecision,
    previous_metadata: &Value,
    candidate_age_ms: i64,
    candidate_reuse_max_ms: i64,
    candidate_stale: bool,
    floor_invalidated: bool,
    now_ms: i64,
) {
    copy_live_gap_submit_revalidation_notification_state(previous_metadata, &mut decision.payload);
    let triggers = live_gap_submit_revalidation_triggers(candidate_stale, floor_invalidated);
    let trigger = triggers
        .first()
        .copied()
        .unwrap_or(LIVE_GAP_SUBMIT_REVALIDATION_TRIGGER_SUBMIT_BEFORE_CLOB);
    let fresh_revalidation_decision = live_gap_submit_revalidation_fresh_decision(decision);
    let decision_reason = live_gap_submit_revalidation_decision_reason(decision);
    let clob_submit_decision = live_gap_submit_revalidation_clob_decision(decision);
    let fresh_snapshot_age_ms =
        live_gap_submit_revalidation_fresh_snapshot_age_ms(&decision.payload, now_ms);
    let late_high_price_risk =
        live_gap_submit_revalidation_late_high_price_risk(&decision.payload);

    if let Some(obj) = decision.payload.as_object_mut() {
        obj.insert(
            "revalidation_trigger".to_string(),
            json!(trigger),
        );
        obj.insert("revalidation_triggers".to_string(), json!(triggers));
        obj.insert(
            "candidate_reuse_decision".to_string(),
            json!(live_gap_submit_revalidation_candidate_reuse_decision(
                candidate_stale,
                floor_invalidated,
            )),
        );
        obj.insert("original_candidate_age_ms".to_string(), json!(candidate_age_ms));
        obj.insert(
            "candidate_reuse_max_ms".to_string(),
            json!(candidate_reuse_max_ms),
        );
        obj.insert("candidate_age_ms".to_string(), json!(candidate_age_ms));
        obj.insert("candidate_max_age_ms".to_string(), json!(candidate_reuse_max_ms));
        obj.insert("candidate_stale".to_string(), json!(candidate_stale));
        obj.insert(
            "candidate_floor_invalidated".to_string(),
            json!(floor_invalidated),
        );
        obj.insert("fresh_revalidation_ts_ms".to_string(), json!(now_ms));
        obj.insert(
            "fresh_snapshot_age_ms".to_string(),
            json!(fresh_snapshot_age_ms),
        );
        obj.insert(
            "fresh_revalidation_decision".to_string(),
            json!(fresh_revalidation_decision),
        );
        obj.insert("decision_reason".to_string(), json!(decision_reason));
        obj.insert(
            "clob_submit_decision".to_string(),
            json!(clob_submit_decision),
        );
        if decision.passed {
            obj.insert("fresh_guard_reason".to_string(), json!(decision.reason_code));
        }
        if let Some(risk) = late_high_price_risk {
            obj.insert("late_high_price_risk".to_string(), risk);
        }
    }
}

fn live_gap_submit_revalidation_notification_identity(
    order: &TradeBuilderOrder,
    payload: &Value,
) -> String {
    let market_slug = payload
        .get("market_slug")
        .and_then(Value::as_str)
        .unwrap_or(order.market_slug.as_str());
    let token_id = payload
        .get("token_id")
        .and_then(Value::as_str)
        .unwrap_or(order.token_id.as_str());
    let outcome_label = payload
        .get("outcome_label")
        .and_then(Value::as_str)
        .unwrap_or(order.outcome_label.as_str());
    format!(
        "{}:{market_slug}:{token_id}:{}",
        order.trade_id,
        outcome_label.trim().to_ascii_lowercase()
    )
}

fn live_gap_submit_revalidation_notification_signature(payload: &Value) -> String {
    let trigger = payload
        .get("revalidation_trigger")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let decision = payload
        .get("fresh_revalidation_decision")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let reason = payload
        .get("decision_reason")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    format!("{trigger}:{decision}:{reason}")
}

fn live_gap_submit_revalidation_current_notification_signature(
    context: &Value,
    identity: &str,
) -> Option<String> {
    flow_context_value(context, LIVE_GAP_SUBMIT_REVALIDATION_NOTIFICATION_STATE_KEY)?
        .get(identity)?
        .get("signature")?
        .as_str()
        .map(str::to_string)
}

fn should_notify_live_gap_submit_revalidation_state_change(
    previous_signature: Option<String>,
    payload: &Value,
    mode: &str,
) -> bool {
    let mode = mode.trim().to_ascii_lowercase();
    if pre_buy_collapse_guard_notification_mode_is_off(&mode) {
        return false;
    }
    if mode == "all" {
        return true;
    }
    if payload
        .get("fresh_revalidation_decision")
        .and_then(Value::as_str)
        == Some(LIVE_GAP_SUBMIT_REVALIDATION_BLOCK_TERMINAL)
    {
        return true;
    }
    previous_signature
        .as_deref()
        .is_none_or(|previous| previous != live_gap_submit_revalidation_notification_signature(payload))
}

fn remember_live_gap_submit_revalidation_notification_state(
    context: &mut Value,
    order: &TradeBuilderOrder,
    mode: &str,
) -> bool {
    let identity = live_gap_submit_revalidation_notification_identity(order, context);
    let signature = live_gap_submit_revalidation_notification_signature(context);
    let previous =
        live_gap_submit_revalidation_current_notification_signature(context, &identity);
    let should_notify =
        should_notify_live_gap_submit_revalidation_state_change(previous, context, mode);
    let mut state = flow_context_value(context, LIVE_GAP_SUBMIT_REVALIDATION_NOTIFICATION_STATE_KEY)
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    state.insert(
        identity,
        json!({
            "signature": signature,
            "sourceTradeId": order.trade_id,
            "marketSlug": context.get("market_slug").and_then(Value::as_str).unwrap_or(order.market_slug.as_str()),
            "tokenId": context.get("token_id").and_then(Value::as_str).unwrap_or(order.token_id.as_str()),
            "outcomeLabel": context.get("outcome_label").and_then(Value::as_str).unwrap_or(order.outcome_label.as_str()),
            "revalidationTrigger": context.get("revalidation_trigger").and_then(Value::as_str),
            "freshRevalidationDecision": context.get("fresh_revalidation_decision").and_then(Value::as_str),
            "decisionReason": context.get("decision_reason").and_then(Value::as_str),
            "updatedAtMs": Utc::now().timestamp_millis(),
        }),
    );
    set_flow_context(
        context,
        LIVE_GAP_SUBMIT_REVALIDATION_NOTIFICATION_STATE_KEY,
        Value::Object(state),
    );
    should_notify
}
