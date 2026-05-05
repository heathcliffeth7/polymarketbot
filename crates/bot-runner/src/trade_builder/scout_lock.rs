fn action_place_order_early_stale_scout_lock_enabled(
    node: &TradeFlowNode,
    side: &str,
) -> bool {
    side == "buy"
        && node_config_bool(node, "priceToBeatEarlyStaleSideEnabled").unwrap_or(false)
        && node_config_bool(node, "priceToBeatEarlyStaleOneScoutPerMarket").unwrap_or(true)
}

fn action_place_order_early_stale_scout_lock_group(node: &TradeFlowNode) -> String {
    let scope = node_config_string(node, "priceToBeatEarlyStaleLockScope")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "condition_id".to_string());
    format!("early_stale_side:{scope}")
}

#[cfg(test)]
mod action_place_order_scout_lock_tests {
    use super::*;

    fn node(config: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "scout".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    #[test]
    fn early_stale_scout_lock_defaults_to_condition_scope() {
        let node = node(json!({"priceToBeatEarlyStaleSideEnabled": true}));

        assert!(action_place_order_early_stale_scout_lock_enabled(&node, "buy"));
        assert_eq!(
            action_place_order_early_stale_scout_lock_group(&node),
            "early_stale_side:condition_id"
        );
    }

    #[test]
    fn early_stale_scout_lock_can_be_disabled() {
        let node = node(json!({
            "priceToBeatEarlyStaleSideEnabled": true,
            "priceToBeatEarlyStaleOneScoutPerMarket": false
        }));

        assert!(!action_place_order_early_stale_scout_lock_enabled(&node, "buy"));
    }
}
