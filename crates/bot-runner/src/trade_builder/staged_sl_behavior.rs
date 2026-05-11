#[derive(Debug, Clone, Default, PartialEq)]
struct TradeBuilderStagedSlBehaviorConfig {
    reentry_only_after_all_stages: bool,
}

#[derive(Debug, Clone, Copy)]
struct ActionPlaceOrderNotificationAndRetryFlags {
    notify_on_order_submitted: bool,
    notify_on_fill: bool,
    notify_on_order_not_filled: bool,
    notify_on_trigger_guard_blocked: bool,
    notify_on_execution_floor_blocked: bool,
    retry_on_trigger_guard_block: bool,
    retry_on_execution_floor_guard_block: bool,
    retry_on_max_price_block: bool,
    notify_on_tp_hit: bool,
    notify_on_sl_hit: bool,
    notify_on_max_price_blocked: bool,
}

fn resolve_action_place_order_notification_and_retry_flags(
    node: &TradeFlowNode,
) -> ActionPlaceOrderNotificationAndRetryFlags {
    ActionPlaceOrderNotificationAndRetryFlags {
        notify_on_order_submitted: node_config_bool(node, "notifyOnOrderSubmitted")
            .unwrap_or(false),
        notify_on_fill: node_config_bool(node, "notifyOnOrderPlaced").unwrap_or(false),
        notify_on_order_not_filled: node_config_bool(node, "notifyOnOrderNotFilled")
            .unwrap_or(false),
        notify_on_trigger_guard_blocked: node_config_bool(node, "notifyOnTriggerPriceBlocked")
            .unwrap_or(false),
        notify_on_execution_floor_blocked: node_config_bool(node, "notifyOnExecutionFloorBlocked")
            .unwrap_or(false),
        retry_on_trigger_guard_block: node_config_bool(node, "retryOnTriggerPriceGuardBlock")
            .unwrap_or(false),
        retry_on_execution_floor_guard_block: node_config_bool(
            node,
            "retryOnExecutionFloorGuardBlock",
        )
        .unwrap_or(false),
        retry_on_max_price_block: node_config_bool(node, "retryOnMaxPriceBlock").unwrap_or(false),
        notify_on_tp_hit: node_config_bool(node, "notifyOnTpHit").unwrap_or(false),
        notify_on_sl_hit: node_config_bool(node, "notifyOnSlHit").unwrap_or(false),
        notify_on_max_price_blocked: node_config_bool(node, "notifyOnMaxPriceBlocked")
            .unwrap_or(false),
    }
}

fn resolve_action_place_order_staged_sl_behavior_config(
    node: &TradeFlowNode,
    side: &str,
    sl_rules: &[TradeBuilderPriceExitRule],
    ptb_stop_loss_rules: &[bot_infra::db::TradeBuilderPtbStopLossRule],
    reenter_on_sl_hit: bool,
) -> TradeBuilderStagedSlBehaviorConfig {
    if side != "buy"
        || (sl_rules.is_empty() && ptb_stop_loss_rules.is_empty())
        || !reenter_on_sl_hit
    {
        return TradeBuilderStagedSlBehaviorConfig::default();
    }

    TradeBuilderStagedSlBehaviorConfig {
        reentry_only_after_all_stages: node_config_bool(
            node,
            "stagedSlReentryOnlyAfterAllStages",
        )
        .unwrap_or(false),
    }
}

fn trade_builder_is_staged_stop_loss_child(order: &TradeBuilderOrder) -> bool {
    trade_builder_is_stop_loss_child(order)
        && matches!(
            order.exit_ladder_kind.as_deref(),
            Some(TRADE_BUILDER_EXIT_LADDER_KIND_SL | TRADE_BUILDER_EXIT_LADDER_KIND_PTB_SL)
        )
}

fn trade_builder_should_defer_reentry_until_all_staged_sl_complete(
    parent_order: &TradeBuilderOrder,
    stop_loss_order: &TradeBuilderOrder,
    siblings: &[TradeBuilderOrder],
) -> bool {
    parent_order.staged_sl_reentry_only_after_all_stages
        && trade_builder_is_staged_stop_loss_child(stop_loss_order)
        && siblings.iter().any(|sibling| {
            sibling.id != stop_loss_order.id
                && trade_builder_is_staged_stop_loss_child(sibling)
                && !trade_builder_is_terminal_status(&sibling.status)
        })
}

fn append_action_place_order_notification_and_retry_payload(
    payload: &mut serde_json::Map<String, Value>,
    flags: &ActionPlaceOrderNotificationAndRetryFlags,
) {
    payload.insert("notify_on_fill".to_string(), json!(flags.notify_on_fill));
    payload.insert(
        "notify_on_order_submitted".to_string(),
        json!(flags.notify_on_order_submitted),
    );
    payload.insert(
        "notify_on_order_not_filled".to_string(),
        json!(flags.notify_on_order_not_filled),
    );
    payload.insert(
        "notify_on_trigger_guard_blocked".to_string(),
        json!(flags.notify_on_trigger_guard_blocked),
    );
    payload.insert(
        "notify_on_execution_floor_blocked".to_string(),
        json!(flags.notify_on_execution_floor_blocked),
    );
    payload.insert(
        "retry_on_trigger_guard_block".to_string(),
        json!(flags.retry_on_trigger_guard_block),
    );
    payload.insert(
        "retry_on_execution_floor_guard_block".to_string(),
        json!(flags.retry_on_execution_floor_guard_block),
    );
    payload.insert(
        "retry_on_max_price_block".to_string(),
        json!(flags.retry_on_max_price_block),
    );
    payload.insert("notify_on_tp_hit".to_string(), json!(flags.notify_on_tp_hit));
    payload.insert("notify_on_sl_hit".to_string(), json!(flags.notify_on_sl_hit));
    payload.insert(
        "notify_on_max_price_blocked".to_string(),
        json!(flags.notify_on_max_price_blocked),
    );
}
