const ACTION_PLACE_ORDER_INTERNAL_PAIR_LOCK_CHILD_ROLE_KEY: &str = "internalPairLockChildRole";
const ACTION_PLACE_ORDER_INTERNAL_INITIAL_STATUS_KEY: &str = "internalInitialStatus";
const ACTION_PLACE_ORDER_INTERNAL_BLOCKED_STATUS: &str = "blocked";
const ACTION_PLACE_ORDER_INTERNAL_REVENGE_FLIP_STOP_LOSS_SELL: &str =
    "revenge_flip_stop_loss_sell";
const ACTION_PLACE_ORDER_REVENGE_FLIP_STOP_LOSS_SELL_KEY: &str = "revengeFlipStopLossSell";

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

fn action_place_order_is_revenge_flip_stop_loss_sell(
    node: &TradeFlowNode,
    internal_mode: Option<&str>,
) -> bool {
    internal_mode == Some(ACTION_PLACE_ORDER_INTERNAL_REVENGE_FLIP_STOP_LOSS_SELL)
        || node_config_bool(node, ACTION_PLACE_ORDER_REVENGE_FLIP_STOP_LOSS_SELL_KEY)
            .unwrap_or(false)
}

fn resolve_action_place_order_revenge_flip_parent_builder_order_id(
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
) -> Option<i64> {
    node_config_i64(node, "parentBuilderOrderId")
        .or_else(|| node_config_i64(node, "parent_builder_order_id"))
        .or_else(|| step_input_i64(step, &["parentBuilderOrderId", "parent_builder_order_id"]))
        .filter(|value| *value > 0)
}

async fn maybe_mark_action_place_order_revenge_flip_stop_loss(
    repo: &PostgresRepository,
    builder_order_id: i64,
    enabled: bool,
    parent_builder_order_id: Option<i64>,
) -> Result<()> {
    if !enabled {
        return Ok(());
    }
    repo.set_trade_builder_order_trigger_latched(builder_order_id, true, Some("stop_loss"))
        .await?;
    repo.append_trade_builder_order_event(
        builder_order_id,
        "revenge_flip_stop_loss_latched",
        &json!({
            "revenge_flip_stop_loss_sell": true,
            "trigger_latched": true,
            "trigger_latched_reason": "stop_loss",
            "parent_builder_order_id": parent_builder_order_id,
        }),
    )
    .await?;
    Ok(())
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

    #[test]
    fn detects_revenge_flip_stop_loss_sell_marker() {
        assert!(action_place_order_is_revenge_flip_stop_loss_sell(
            &node(json!({"revengeFlipStopLossSell": true})),
            None
        ));
        assert!(action_place_order_is_revenge_flip_stop_loss_sell(
            &node(json!({})),
            Some("revenge_flip_stop_loss_sell")
        ));
    }
}
