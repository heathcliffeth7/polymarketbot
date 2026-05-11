use super::*;

#[test]
fn parse_trade_flow_graph_preserves_edge_condition() {
    let condition = json!({
        "==": [{ "var": "triggered_outcome_label" }, "Down"]
    });
    let version = TradeFlowVersionRuntime {
        id: 1,
        definition_id: 1,
        version_no: 1,
        status: "draft".to_string(),
        graph_json: json!({
            "context": {},
            "nodes": [
                { "key": "trigger_market", "type": "trigger.market_price", "config": {} },
                { "key": "action_down", "type": "action.place_order", "config": {} }
            ],
            "edges": [
                {
                    "key": "edge_down",
                    "source": "trigger_market",
                    "target": "action_down",
                    "type": "default",
                    "condition": condition
                }
            ]
        }),
        published_at: None,
        created_at: Utc::now(),
    };

    let graph = parse_trade_flow_graph(&version).expect("graph should parse");

    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].condition, Some(condition));
}

#[test]
fn build_trade_flow_route_eval_data_prefers_output_fields() {
    let context = json!({
        "flowContext": {
            "triggered_outcome_label": "Up",
            "tokenId": "tok-up"
        },
        "vars": {
            "price": 0.11
        },
        "state": {
            "mode": "armed"
        },
        "refs": {
            "tradeId": 42
        },
        "nodeState": {
            "trigger_market": {
                "previous_price_tok-up": 0.10
            }
        }
    });
    let output = json!({
        "triggered_outcome_label": "Down",
        "price": 0.42
    });

    let eval_data = build_trade_flow_route_eval_data(&context, &output);

    assert_eq!(
        eval_data.get("triggered_outcome_label"),
        Some(&json!("Down"))
    );
    assert_eq!(eval_data.get("price"), Some(&json!(0.42)));
    assert_eq!(
        eval_data.pointer("/output/triggered_outcome_label"),
        Some(&json!("Down"))
    );
    assert_eq!(
        eval_data.pointer("/flowContext/tokenId"),
        Some(&json!("tok-up"))
    );
    assert_eq!(eval_data.pointer("/refs/tradeId"), Some(&json!(42)));
}

#[test]
fn resolve_trade_flow_targets_filters_edges_by_condition() {
    let graph = TradeFlowGraphRuntime {
        context: json!({}),
        nodes: vec![
            TradeFlowNode {
                key: "trigger_market".to_string(),
                node_type: "trigger.market_price".to_string(),
                config: json!({}),
            },
            TradeFlowNode {
                key: "action_up".to_string(),
                node_type: "action.place_order".to_string(),
                config: json!({}),
            },
            TradeFlowNode {
                key: "action_down".to_string(),
                node_type: "action.place_order".to_string(),
                config: json!({}),
            },
        ],
        edges: vec![
            TradeFlowEdge {
                source: "trigger_market".to_string(),
                target: "action_up".to_string(),
                edge_type: "default".to_string(),
                condition: Some(json!({
                    "==": [{ "var": "triggered_outcome_label" }, "Up"]
                })),
            },
            TradeFlowEdge {
                source: "trigger_market".to_string(),
                target: "action_down".to_string(),
                edge_type: "default".to_string(),
                condition: Some(json!({
                    "==": [{ "var": "triggered_outcome_label" }, "Down"]
                })),
            },
        ],
    };

    let targets = resolve_trade_flow_targets(
        &graph,
        "trigger_market",
        "default",
        &json!({ "triggered_outcome_label": "Down" }),
    );

    assert_eq!(targets, vec!["action_down".to_string()]);
}

#[test]
fn resolve_trade_flow_targets_keeps_conditionless_edges() {
    let graph = TradeFlowGraphRuntime {
        context: json!({}),
        nodes: vec![
            TradeFlowNode {
                key: "trigger_market".to_string(),
                node_type: "trigger.market_price".to_string(),
                config: json!({}),
            },
            TradeFlowNode {
                key: "action_any".to_string(),
                node_type: "action.place_order".to_string(),
                config: json!({}),
            },
            TradeFlowNode {
                key: "action_up".to_string(),
                node_type: "action.place_order".to_string(),
                config: json!({}),
            },
        ],
        edges: vec![
            TradeFlowEdge {
                source: "trigger_market".to_string(),
                target: "action_any".to_string(),
                edge_type: "default".to_string(),
                condition: None,
            },
            TradeFlowEdge {
                source: "trigger_market".to_string(),
                target: "action_up".to_string(),
                edge_type: "default".to_string(),
                condition: Some(json!({
                    "==": [{ "var": "triggered_outcome_label" }, "Up"]
                })),
            },
        ],
    };

    let targets = resolve_trade_flow_targets(
        &graph,
        "trigger_market",
        "default",
        &json!({ "triggered_outcome_label": "Down" }),
    );

    assert_eq!(targets, vec!["action_any".to_string()]);
}
