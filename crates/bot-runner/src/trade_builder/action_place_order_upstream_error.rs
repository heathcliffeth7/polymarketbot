fn maybe_block_action_place_order_upstream_error(
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
) -> Option<TradeFlowNodeExecution> {
    let input = step.input_json.as_ref()?;
    let upstream_error = input.get("error").and_then(Value::as_str)?.trim();
    if upstream_error.is_empty() {
        return None;
    }

    Some(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "blocked": true,
            "skipped": true,
            "reason": "upstream_trigger_error",
            "upstream_error": upstream_error,
            "upstream_node_key": input.get("node_key").and_then(Value::as_str),
            "upstream_node_type": input.get("node_type").and_then(Value::as_str),
            "side": node_config_string(node, "side"),
            "execution_mode": node_config_string(node, "executionMode"),
        }),
        routes: Vec::new(),
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

#[cfg(test)]
mod action_place_order_upstream_error_tests {
    use super::*;

    #[test]
    fn upstream_error_blocks_action_before_order_work() {
        let node = TradeFlowNode {
            key: "action_1".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({
                "side": "buy",
                "executionMode": "market"
            }),
        };
        let step = TradeFlowRunStep {
            id: 1,
            run_id: 42,
            node_key: "action_1".to_string(),
            node_type: "action.place_order".to_string(),
            status: "queued".to_string(),
            attempt: 1,
            input_json: Some(json!({
                "error": "trigger.market_price auto_scope unsupported marketScope=hype_5m_updown",
                "node_key": "trigger_hype_eq77_up",
                "node_type": "trigger.market_price"
            })),
            output_json: None,
            error_text: None,
            started_at: None,
            ended_at: None,
            available_at: Utc::now(),
            parent_step_id: Some(99),
            idempotency_key: None,
            created_at: Utc::now(),
        };

        let execution =
            maybe_block_action_place_order_upstream_error(&node, &step).expect("blocked");

        assert_eq!(
            execution.output.get("reason").and_then(Value::as_str),
            Some("upstream_trigger_error")
        );
        assert_eq!(
            execution.output.get("upstream_node_key").and_then(Value::as_str),
            Some("trigger_hype_eq77_up")
        );
        assert!(execution.routes.is_empty());
    }
}
