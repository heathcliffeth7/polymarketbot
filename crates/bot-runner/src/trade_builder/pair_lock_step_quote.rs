fn clone_pair_lock_step_with_quote(
    step: &TradeFlowRunStep,
    quote: &PairLockResolvedQuote,
) -> TradeFlowRunStep {
    let mut cloned = step.clone();
    let mut input_json = cloned
        .input_json
        .take()
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    input_json.insert("wsBestBid".to_string(), json!(quote.best_bid));
    input_json.insert("wsBestAsk".to_string(), json!(quote.best_ask));
    input_json.insert(
        "wsLastTradePrice".to_string(),
        json!(quote.last_trade_price),
    );
    input_json.insert(
        "wsPriceSource".to_string(),
        json!(quote.quote_source_kind),
    );
    cloned.input_json = Some(Value::Object(input_json));
    cloned
}

fn pair_lock_candidate_quote_for_token<'a>(
    token_id: &str,
    resolved_tokens: &PairLockResolvedTokenPair,
    quotes: &'a Option<(PairLockResolvedQuote, PairLockResolvedQuote)>,
) -> Option<&'a PairLockResolvedQuote> {
    let (yes_quote, no_quote) = quotes.as_ref()?;
    if token_id == resolved_tokens.yes_token_id {
        Some(yes_quote)
    } else if token_id == resolved_tokens.no_token_id {
        Some(no_quote)
    } else {
        None
    }
}

fn pair_lock_pre_dispatch_resolution(
    node: &TradeFlowNode,
    context: &Value,
    step: &TradeFlowRunStep,
    graph: &TradeFlowGraphRuntime,
) -> Result<(
    String,
    String,
    String,
    Option<String>,
    Option<String>,
)> {
    let side = node_config_string(node, "side")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "buy".to_string());
    let execution_mode = node_config_string(node, "executionMode")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "market".to_string());
    let trigger_node_key = resolve_pair_lock_direct_trigger_node_key(&node.key, graph)?;
    let explicit_primary_token_id = resolve_action_place_order_string(
        node,
        context,
        step,
        "tokenId",
        "tokenId",
        &["triggered_token_id", "tokenId"],
    );
    let explicit_primary_outcome_label = resolve_action_place_order_string(
        node,
        context,
        step,
        "outcomeLabel",
        "outcomeLabel",
        &["triggered_outcome_label", "outcomeLabel"],
    );
    Ok((
        side,
        execution_mode,
        trigger_node_key,
        explicit_primary_token_id,
        explicit_primary_outcome_label,
    ))
}

#[cfg(test)]
mod pair_lock_step_quote_tests {
    use super::*;

    #[test]
    fn clone_pair_lock_step_with_quote_injects_runtime_fields() {
        let step = TradeFlowRunStep {
            id: 1,
            run_id: 2,
            node_key: "pair_buy".to_string(),
            node_type: "action.place_order".to_string(),
            status: "queued".to_string(),
            attempt: 1,
            input_json: Some(json!({
                "existing": true,
                "wsBestAsk": 0.91
            })),
            output_json: None,
            error_text: None,
            started_at: None,
            ended_at: None,
            available_at: Utc::now(),
            parent_step_id: None,
            idempotency_key: None,
            created_at: Utc::now(),
        };
        let quote = PairLockResolvedQuote {
            best_bid: Some(0.41),
            best_ask: Some(0.43),
            last_trade_price: Some(0.42),
            current_price: 0.41,
            quote_source_kind: "ws_subscribe_once",
            quote_ws_state: "live_ws_subscribed_unseeded",
            quote_event_ts: Some(123),
            quote_snapshot_age_ms: Some(0),
            quote_source_detail: "ws_subscribe_once:book".to_string(),
            quote_book_missing_fields: Vec::new(),
            quote_snapshot_used: json!({}),
        };

        let cloned = clone_pair_lock_step_with_quote(&step, &quote);
        let input = cloned.input_json.expect("input json");
        assert_eq!(input.get("existing").and_then(Value::as_bool), Some(true));
        assert_eq!(input.get("wsBestBid").and_then(value_as_f64), Some(0.41));
        assert_eq!(input.get("wsBestAsk").and_then(value_as_f64), Some(0.43));
        assert_eq!(
            input.get("wsLastTradePrice").and_then(value_as_f64),
            Some(0.42)
        );
        assert_eq!(
            input.get("wsPriceSource").and_then(Value::as_str),
            Some("ws_subscribe_once")
        );
    }

    #[test]
    fn pair_lock_pre_dispatch_resolution_defaults_buy_market_and_resolves_trigger() {
        let graph = TradeFlowGraphRuntime {
            context: json!({}),
            nodes: vec![
                TradeFlowNode {
                    key: "trigger_pair".to_string(),
                    node_type: "trigger.market_price".to_string(),
                    config: json!({ "bindingMode": "pair_lock_only" }),
                },
                TradeFlowNode {
                    key: "pair_buy".to_string(),
                    node_type: "action.place_order".to_string(),
                    config: json!({ "mode": "pair_lock" }),
                },
            ],
            edges: vec![TradeFlowEdge {
                source: "trigger_pair".to_string(),
                target: "pair_buy".to_string(),
                edge_type: "default".to_string(),
                condition: None,
            }],
        };
        let node = TradeFlowNode {
            key: "pair_buy".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({ "mode": "pair_lock" }),
        };
        let step = TradeFlowRunStep {
            id: 1,
            run_id: 1,
            node_key: "pair_buy".to_string(),
            node_type: "action.place_order".to_string(),
            status: "queued".to_string(),
            attempt: 1,
            input_json: Some(json!({})),
            output_json: None,
            error_text: None,
            started_at: None,
            ended_at: None,
            available_at: Utc::now(),
            parent_step_id: None,
            idempotency_key: None,
            created_at: Utc::now(),
        };

        let (side, execution_mode, trigger_node_key, token_id, outcome_label) =
            pair_lock_pre_dispatch_resolution(&node, &json!({}), &step, &graph)
                .expect("resolution");
        assert_eq!(side, "buy");
        assert_eq!(execution_mode, "market");
        assert_eq!(trigger_node_key, "trigger_pair");
        assert!(token_id.is_none());
        assert!(outcome_label.is_none());
    }
}
