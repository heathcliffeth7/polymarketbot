fn resolve_action_place_order_guard_trigger_price(step: &TradeFlowRunStep) -> Option<f64> {
    step_input_f64(step, &["trigger_price", "triggerPrice"])
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .map(clamp_probability)
}

fn resolve_action_place_order_execution_floor_price(
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
) -> Option<f64> {
    node_config_f64(node, "executionFloorPriceCent")
        .map(|value| value / 100.0)
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .map(clamp_probability)
        .or_else(|| resolve_action_place_order_guard_trigger_price(step))
}
