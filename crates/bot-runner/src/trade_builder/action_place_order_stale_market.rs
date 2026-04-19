fn build_action_place_order_stale_market_skipped_execution(
    node: &TradeFlowNode,
    stale_market_slug: &str,
    current_market_slug: &str,
    token_id: Option<&str>,
    outcome_label: Option<&str>,
    side: &str,
    execution_mode: &str,
) -> TradeFlowNodeExecution {
    TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "skipped": true,
            "reason": "stale_market_retry_skipped",
            "market_slug": current_market_slug,
            "stale_market_slug": stale_market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "side": side,
            "execution_mode": execution_mode,
        }),
        routes: Vec::new(),
        repeat_at: None,
        repeat_idempotency_key: None,
    }
}

async fn maybe_skip_stale_action_place_order_step(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    step: &TradeFlowRunStep,
    context: &mut Value,
    side: &str,
    execution_mode: &str,
    token_id: Option<&str>,
    outcome_label: Option<&str>,
    trigger_node_key: Option<&str>,
) -> Result<Option<TradeFlowNodeExecution>> {
    let Some(stale_market_retry) =
        resolve_action_place_order_stale_market_retry(node, context, step, graph)
    else {
        return Ok(None);
    };

    let stale_market_slug = stale_market_retry.stale_market_slug;
    let current_market_slug = stale_market_retry.current_market_slug;

    let effective_trigger_node_key = trigger_node_key
        .map(str::to_string)
        .or_else(|| find_upstream_market_price_trigger_key(&node.key, graph));
    if let Some(trigger_node_key) = effective_trigger_node_key.as_deref() {
        promote_trigger_node_auto_scope_context_to_flow_context(
            context,
            trigger_node_key,
            &current_market_slug,
        );
    } else {
        set_flow_context(context, "marketSlug", json!(current_market_slug.clone()));
    }
    set_flow_context(context, "priceToBeatGuard", Value::Null);
    set_flow_context(context, "priceToBeatGuardWaiting", Value::Null);
    set_flow_context(context, "priceToBeatGuardWaitingReason", Value::Null);
    set_flow_context(context, "lastGuardNotificationSeed", Value::Null);

    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "action_place_order_stale_market_skipped",
        &json!({
            "node_key": node.key,
            "node_type": node.node_type,
            "reason": "stale_market_retry_skipped",
            "stale_market_slug": stale_market_slug.clone(),
            "current_market_slug": current_market_slug.clone(),
            "token_id": token_id,
            "outcome_label": outcome_label,
            "side": side,
            "execution_mode": execution_mode,
        }),
    )
    .await?;

    Ok(Some(build_action_place_order_stale_market_skipped_execution(
        node,
        &stale_market_slug,
        &current_market_slug,
        token_id,
        outcome_label,
        side,
        execution_mode,
    )))
}

#[cfg(test)]
mod action_place_order_stale_market_tests {
    use super::*;

    #[test]
    fn stale_market_skipped_execution_preserves_skip_semantics() {
        let node = TradeFlowNode {
            key: "action_1".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({}),
        };
        let execution = build_action_place_order_stale_market_skipped_execution(
            &node,
            "btc-updown-5m-old",
            "btc-updown-5m-new",
            Some("tok-up"),
            Some("Up"),
            "buy",
            "market",
        );

        assert_eq!(
            execution.output.get("reason").and_then(Value::as_str),
            Some("stale_market_retry_skipped")
        );
        assert_eq!(
            execution
                .output
                .get("stale_market_slug")
                .and_then(Value::as_str),
            Some("btc-updown-5m-old")
        );
        assert_eq!(
            execution.output.get("market_slug").and_then(Value::as_str),
            Some("btc-updown-5m-new")
        );
        assert!(execution.repeat_at.is_none());
        assert!(execution.routes.is_empty());
    }
}
