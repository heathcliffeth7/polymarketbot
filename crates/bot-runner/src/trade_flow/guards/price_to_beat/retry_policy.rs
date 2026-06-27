use serde_json::{json, Value};

const CURRENT_PRICE_UNAVAILABLE_MIN_RETRY_DELAY_MS: i64 = 1_000;

pub(super) fn price_to_beat_guard_retry_delay_ms(node: &crate::TradeFlowNode) -> i64 {
    if crate::node_config_bool(node, "priceToBeatEarlyStaleSideEnabled").unwrap_or(false) {
        return crate::node_config_i64(node, "priceToBeatEarlyStaleRetryCooldownMs")
            .unwrap_or(500)
            .clamp(100, 60_000);
    }
    crate::PRICE_TO_BEAT_GUARD_RETRY_DELAY_MS
}

pub(super) fn price_to_beat_guard_retry_delay_ms_for_reason(
    node: &crate::TradeFlowNode,
    reason: &str,
) -> i64 {
    let delay_ms = price_to_beat_guard_retry_delay_ms(node);
    if reason == "current_price_unavailable" {
        return delay_ms.max(CURRENT_PRICE_UNAVAILABLE_MIN_RETRY_DELAY_MS);
    }
    delay_ms
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_node(config: Value) -> crate::TradeFlowNode {
        crate::TradeFlowNode {
            key: "action".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    #[test]
    fn current_price_unavailable_retry_delay_has_one_second_floor() {
        let node = test_node(json!({}));

        assert_eq!(
            price_to_beat_guard_retry_delay_ms_for_reason(&node, "current_price_unavailable"),
            1_000
        );
        assert_eq!(
            price_to_beat_guard_retry_delay_ms_for_reason(
                &node,
                "price_to_beat_gap_below_threshold"
            ),
            crate::PRICE_TO_BEAT_GUARD_RETRY_DELAY_MS
        );
    }

    #[test]
    fn default_price_to_beat_retry_delay_stays_150ms() {
        let node = test_node(json!({}));

        assert_eq!(price_to_beat_guard_retry_delay_ms(&node), 150);
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
