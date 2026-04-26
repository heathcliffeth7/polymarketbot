#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionPlaceOrderPairLockSizingMode {
    Manual,
    AutoRemainingBudget,
}

fn resolve_action_place_order_pair_lock_sizing_mode(
    node: &TradeFlowNode,
) -> ActionPlaceOrderPairLockSizingMode {
    match node_config_string(node, "pairSizingMode")
        .unwrap_or_else(|| "manual".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "auto_remaining_budget" => ActionPlaceOrderPairLockSizingMode::AutoRemainingBudget,
        _ => ActionPlaceOrderPairLockSizingMode::Manual,
    }
}

fn resolve_action_place_order_pair_lock_primary_budget_usdc(
    node: &TradeFlowNode,
) -> Option<f64> {
    node_config_f64(node, "sizeUsdc")
        .or_else(|| node_config_f64(node, "targetNotionalUsdc"))
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn trade_builder_pair_lock_initial_counter_budget(
    total_budget_usdc: f64,
    primary_budget_usdc: f64,
) -> Option<f64> {
    let remaining_budget_usdc = total_budget_usdc - primary_budget_usdc;
    (remaining_budget_usdc.is_finite() && remaining_budget_usdc > 0.0)
        .then_some(remaining_budget_usdc)
}

fn trade_builder_pair_lock_actual_primary_spend(
    session: &TradeBuilderPairSession,
) -> Option<f64> {
    Some(session.primary_fill_qty? * session.primary_avg_fill_price?)
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn trade_builder_pair_lock_remaining_budget_usdc(
    total_budget_usdc: f64,
    session: &TradeBuilderPairSession,
) -> Option<f64> {
    let actual_primary_spend = trade_builder_pair_lock_actual_primary_spend(session)?;
    let remaining_budget_usdc = total_budget_usdc - actual_primary_spend;
    (remaining_budget_usdc.is_finite() && remaining_budget_usdc > 0.0)
        .then_some(remaining_budget_usdc)
}

#[cfg(test)]
mod pair_lock_budget_tests {
    use super::*;

    #[test]
    fn pair_lock_budget_remaining_budget_uses_actual_primary_spend() {
        let session = TradeBuilderPairSession {
            id: 1,
            user_id: 1,
            flow_definition_id: None,
            flow_run_id: None,
            flow_node_key: None,
            market_slug: "btc-updown-5m-1".to_string(),
            status: "working".to_string(),
            pair_target_total_cent: 90.0,
            min_net_profit_usdc: 0.0,
            profit_safety_buffer_usdc: 0.0,
            orphan_grace_ms: 1500,
            ignore_stop_loss_after_locked: false,
            notify_on_pair_locked: false,
            notify_on_pair_unwind: false,
            notify_on_pair_no_edge: false,
            primary_order_id: Some(11),
            counter_order_id: Some(12),
            lead_order_id: Some(11),
            primary_fill_qty: Some(6.0),
            primary_fill_fee_qty: Some(0.0),
            primary_net_qty: Some(6.0),
            primary_avg_fill_price: Some(0.70),
            counter_fill_qty: None,
            counter_fill_fee_qty: None,
            counter_net_qty: None,
            counter_avg_fill_price: None,
            lead_filled_at: None,
            locked_qty: None,
            projected_net_profit_usdc: None,
            last_error: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert_eq!(
            trade_builder_pair_lock_remaining_budget_usdc(14.0, &session),
            Some(9.8)
        );
    }

    #[test]
    fn pair_lock_budget_initial_counter_budget_requires_positive_remainder() {
        assert_eq!(
            trade_builder_pair_lock_initial_counter_budget(14.0, 5.0),
            Some(9.0)
        );
        assert_eq!(trade_builder_pair_lock_initial_counter_budget(5.0, 5.0), None);
    }
}
