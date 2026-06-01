fn positive_quantity_flip_grid_output_skipped(
    node: &TradeFlowNode,
    market_slug: &str,
    reason: &str,
    extra: Value,
) -> TradeFlowNodeExecution {
    TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "mode": action_place_order_positive_grid_mode_or_default(node),
            "market_slug": market_slug,
            "skipped": true,
            "reason": reason,
            "details": extra,
        }),
        routes: Vec::new(),
        repeat_at: None,
        repeat_idempotency_key: None,
    }
}
