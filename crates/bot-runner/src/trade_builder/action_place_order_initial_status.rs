const ACTION_PLACE_ORDER_INTERNAL_PAIR_LOCK_CHILD_ROLE_KEY: &str = "internalPairLockChildRole";
const ACTION_PLACE_ORDER_INTERNAL_INITIAL_STATUS_KEY: &str = "internalInitialStatus";
const ACTION_PLACE_ORDER_INTERNAL_BLOCKED_STATUS: &str = "blocked";

fn action_place_order_uses_blocked_internal_initial_status(node: &TradeFlowNode) -> bool {
    node_config_string(node, ACTION_PLACE_ORDER_INTERNAL_PAIR_LOCK_CHILD_ROLE_KEY).as_deref()
        == Some(TRADE_BUILDER_PAIR_ROLE_COUNTER_CANDIDATE)
        && node_config_string(node, ACTION_PLACE_ORDER_INTERNAL_INITIAL_STATUS_KEY).as_deref()
            == Some(ACTION_PLACE_ORDER_INTERNAL_BLOCKED_STATUS)
}

fn resolve_action_place_order_initial_status(
    node: &TradeFlowNode,
    side: &str,
    kind: &str,
) -> &'static str {
    if side == "sell" && kind == "immediate" {
        "triggered"
    } else if action_place_order_uses_blocked_internal_initial_status(node) {
        ACTION_PLACE_ORDER_INTERNAL_BLOCKED_STATUS
    } else {
        "pending"
    }
}

fn action_place_order_should_inline_submit(kind: &str, initial_status: &str) -> bool {
    kind == "immediate" && initial_status != ACTION_PLACE_ORDER_INTERNAL_BLOCKED_STATUS
}

#[cfg(test)]
mod action_place_order_initial_status_tests {
    use super::*;

    fn node(config: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "order".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    #[test]
    fn defaults_to_pending_for_normal_buy_orders() {
        assert_eq!(
            resolve_action_place_order_initial_status(&node(json!({})), "buy", "immediate"),
            "pending"
        );
    }

    #[test]
    fn blocks_internal_pair_lock_counter_until_session_attach() {
        let mut config = serde_json::Map::new();
        config.insert(
            ACTION_PLACE_ORDER_INTERNAL_PAIR_LOCK_CHILD_ROLE_KEY.to_string(),
            json!(TRADE_BUILDER_PAIR_ROLE_COUNTER_CANDIDATE),
        );
        config.insert(
            ACTION_PLACE_ORDER_INTERNAL_INITIAL_STATUS_KEY.to_string(),
            json!(ACTION_PLACE_ORDER_INTERNAL_BLOCKED_STATUS),
        );
        let node = node(Value::Object(config));

        let initial_status = resolve_action_place_order_initial_status(&node, "buy", "immediate");

        assert_eq!(initial_status, ACTION_PLACE_ORDER_INTERNAL_BLOCKED_STATUS);
        assert!(!action_place_order_should_inline_submit(
            "immediate",
            initial_status
        ));
    }

    #[test]
    fn keeps_immediate_sell_orders_triggered() {
        assert_eq!(
            resolve_action_place_order_initial_status(&node(json!({})), "sell", "immediate"),
            "triggered"
        );
    }
}
