use serde_json::{json, Value};

pub(super) fn price_to_beat_guard_retry_delay_ms(node: &crate::TradeFlowNode) -> i64 {
    if crate::node_config_bool(node, "priceToBeatEarlyStaleSideEnabled").unwrap_or(false) {
        return crate::node_config_i64(node, "priceToBeatEarlyStaleRetryCooldownMs")
            .unwrap_or(500)
            .clamp(100, 60_000);
    }
    crate::PRICE_TO_BEAT_GUARD_RETRY_DELAY_MS
}

pub(super) fn early_stale_side_guard_retry_limit_reached(
    context: &mut Value,
    node: &crate::TradeFlowNode,
    market_slug: &str,
) -> bool {
    if !crate::node_config_bool(node, "priceToBeatEarlyStaleSideEnabled").unwrap_or(false) {
        return false;
    }
    let max_retries = crate::node_config_i64(node, "priceToBeatEarlyStaleMaxGuardRetriesPerMarket")
        .unwrap_or(40)
        .max(0);
    increment_early_stale_side_guard_retry_count(context, &node.key, market_slug) > max_retries
}

pub(super) fn clear_early_stale_side_guard_retry_count(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
) {
    let Some(mut state) = crate::flow_context_value(context, "earlyStaleSideGuardRetries")
        .and_then(|value| value.as_object().cloned())
    else {
        return;
    };
    state.remove(&early_stale_side_guard_retry_key(node_key, market_slug));
    crate::set_flow_context(
        context,
        "earlyStaleSideGuardRetries",
        if state.is_empty() {
            Value::Null
        } else {
            Value::Object(state)
        },
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn early_stale_side_retry_limit_execution(
    node: &crate::TradeFlowNode,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    side: &str,
    execution_mode: &str,
    evaluation_output: &Value,
) -> crate::TradeFlowNodeExecution {
    crate::TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "blocked": true,
            "reason": "price_to_beat_guard_blocked",
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "side": side,
            "execution_mode": execution_mode,
            "retrying": false,
            "retry_limit_reached": true,
            "price_to_beat_guard": evaluation_output,
        }),
        routes: vec![crate::TradeFlowRouteDecision {
            edge_type: "on_error".to_string(),
            available_at: crate::Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    }
}

fn early_stale_side_guard_retry_key(node_key: &str, market_slug: &str) -> String {
    format!("{node_key}:{market_slug}")
}

fn increment_early_stale_side_guard_retry_count(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
) -> i64 {
    let key = early_stale_side_guard_retry_key(node_key, market_slug);
    let mut state = crate::flow_context_value(context, "earlyStaleSideGuardRetries")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let attempts = state.get(&key).and_then(Value::as_i64).unwrap_or(0) + 1;
    state.insert(key, json!(attempts));
    crate::set_flow_context(context, "earlyStaleSideGuardRetries", Value::Object(state));
    attempts
}
