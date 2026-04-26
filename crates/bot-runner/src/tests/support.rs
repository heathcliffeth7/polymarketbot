use super::*;

pub(super) fn runtime_graph(
    nodes: Vec<(&str, &str)>,
    edges: Vec<(&str, &str)>,
) -> TradeFlowGraphRuntime {
    TradeFlowGraphRuntime {
        context: json!({}),
        nodes: nodes
            .into_iter()
            .map(|(key, node_type)| TradeFlowNode {
                key: key.to_string(),
                node_type: node_type.to_string(),
                config: json!({}),
            })
            .collect(),
        edges: edges
            .into_iter()
            .map(|(source, target)| TradeFlowEdge {
                source: source.to_string(),
                target: target.to_string(),
                edge_type: "default".to_string(),
                condition: None,
            })
            .collect(),
    }
}

pub(super) fn drawdown_node(config: Value) -> TradeFlowNode {
    TradeFlowNode {
        key: "drawdown_test".to_string(),
        node_type: "trigger.position_drawdown".to_string(),
        config,
    }
}

pub(super) fn make_leg(levels_filled: u32, last_fill_price: Option<f64>) -> DualLegRuntime {
    DualLegRuntime {
        side: LegSide::Yes,
        token_id: "tok".to_string(),
        qty: 10.0,
        avg_entry: 0.50,
        levels_filled,
        last_fill_price,
        last_dca_at: None,
    }
}

pub(super) fn test_step(input_json: Value) -> TradeFlowRunStep {
    TradeFlowRunStep {
        id: 1,
        run_id: 42,
        node_key: "action_1".to_string(),
        node_type: "action.place_order".to_string(),
        status: "queued".to_string(),
        attempt: 1,
        input_json: Some(input_json),
        output_json: None,
        error_text: None,
        started_at: None,
        ended_at: None,
        available_at: Utc::now(),
        parent_step_id: None,
        idempotency_key: None,
        created_at: Utc::now(),
    }
}

pub(super) fn test_node(config: Value) -> TradeFlowNode {
    TradeFlowNode {
        key: "action_1".to_string(),
        node_type: "action.place_order".to_string(),
        config,
    }
}

pub(super) fn test_builder_order(side: &str, parent_order_id: Option<i64>) -> TradeBuilderOrder {
    TradeBuilderOrder {
        id: 1,
        trade_id: 77,
        user_id: 1,
        kind: "conditional".to_string(),
        status: "pending".to_string(),
        market_slug: "btc-updown-5m-1".to_string(),
        token_id: "tok-up".to_string(),
        outcome_label: "Up".to_string(),
        side: side.to_string(),
        execution_mode: "market".to_string(),
        trigger_condition: Some("cross_above".to_string()),
        trigger_price: Some(0.8),
        max_price: None,
        size_basis: TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC.to_string(),
        size_usdc: 5.0,
        target_qty: None,
        min_price_distance_cent: 1.0,
        expires_at: None,
        eligible_after_at: None,
        eligible_before_at: None,
        max_triggers: 1,
        triggers_fired: 0,
        active_exchange_order_id: None,
        remaining_size: None,
        remaining_qty: None,
        working_price: None,
        last_seen_price: None,
        last_error: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        parent_order_id,
        origin_flow_definition_id: None,
        origin_flow_run_id: None,
        origin_flow_node_key: None,
        pair_session_id: None,
        pair_leg_role: None,
        tp_enabled: false,
        tp_price: None,
        tp_rules_json: Vec::new(),
        sl_enabled: false,
        sl_price: None,
        sl_rules_json: Vec::new(),
        time_exit_rules_json: Vec::new(),
        filled_qty: 0.0,
        fee_rate_bps: 0,
        trigger_latched: false,
        trigger_latched_reason: None,
        trigger_latched_at: None,
        submitted_dynamic_qty: None,
        submitted_dynamic_price: None,
        runtime_snapshot_json: None,
        fresh_submit_lease_until: None,
        guard_trigger_price: None,
        best_ask_floor_price: None,
        retry_on_trigger_guard_block: false,
        retry_on_execution_floor_guard_block: false,
        retry_on_max_price_block: false,
        ptb_stop_loss_gap_usd: None,
        ptb_reference_price: None,
        ptb_stop_loss_rules_json: Vec::new(),
        ptb_stop_loss_time_decay_mode: None,
        staged_sl_retry_only_dust: false,
        staged_sl_retry_dust_metric: None,
        staged_sl_retry_dust_value: None,
        staged_sl_reentry_use_sold_notional: false,
        staged_sl_reentry_only_after_all_stages: false,
        sl_trigger_price_mode: None,
        reenter_on_sl_hit: false,
        reentry_max_attempts: 0,
        reentry_trigger_node_key: None,
        notify_on_order_submitted: false,
        notify_on_fill: false,
        notify_on_order_not_filled: false,
        notify_on_trigger_guard_blocked: false,
        notify_on_execution_floor_blocked: false,
        notify_on_tp_hit: false,
        notify_on_sl_hit: false,
        notify_on_max_price_blocked: false,
        last_guard_notification_reason: None,
        exit_ladder_kind: None,
        exit_ladder_index: None,
        exit_ladder_size_pct: None,
    }
}

pub(super) fn telegram_node(config: serde_json::Value) -> TradeFlowNode {
    TradeFlowNode {
        key: "telegram_1".to_string(),
        node_type: "action.telegram_notify".to_string(),
        config,
    }
}
