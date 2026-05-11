use super::*;

fn test_graph(trigger_binding_mode: &str, extra_incoming: bool) -> TradeFlowGraphRuntime {
    let mut edges = vec![TradeFlowEdge {
        source: "trigger_pair".to_string(),
        target: "pair_buy".to_string(),
        edge_type: "default".to_string(),
        condition: None,
    }];
    let mut nodes = vec![
        TradeFlowNode {
            key: "trigger_pair".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "bindingMode": trigger_binding_mode,
            }),
        },
        TradeFlowNode {
            key: "pair_buy".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({
                "mode": "pair_lock",
            }),
        },
    ];
    if extra_incoming {
        nodes.push(TradeFlowNode {
            key: "trigger_extra".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({
                "bindingMode": trigger_binding_mode,
            }),
        });
        edges.push(TradeFlowEdge {
            source: "trigger_extra".to_string(),
            target: "pair_buy".to_string(),
            edge_type: "default".to_string(),
            condition: None,
        });
    }
    TradeFlowGraphRuntime {
        context: json!({}),
        nodes,
        edges,
    }
}

#[test]
fn resolve_pair_lock_direct_trigger_node_key_accepts_pair_lock_only_parent() {
    let graph = test_graph("pair_lock_only", false);
    let trigger_key =
        resolve_pair_lock_direct_trigger_node_key("pair_buy", &graph).expect("pair lock trigger");
    assert_eq!(trigger_key, "trigger_pair");
}

#[test]
fn resolve_pair_lock_direct_trigger_node_key_rejects_standard_parent() {
    let graph = test_graph("standard", false);
    let err = resolve_pair_lock_direct_trigger_node_key("pair_buy", &graph)
        .expect_err("standard binding should fail");
    assert!(err.to_string().contains("bindingMode=pair_lock_only"));
}

#[test]
fn resolve_pair_lock_direct_trigger_node_key_rejects_multiple_direct_parents() {
    let graph = test_graph("pair_lock_only", true);
    let err = resolve_pair_lock_direct_trigger_node_key("pair_buy", &graph)
        .expect_err("multiple direct parents should fail");
    assert!(err
        .to_string()
        .contains("exactly one direct upstream trigger.market_price"));
}
