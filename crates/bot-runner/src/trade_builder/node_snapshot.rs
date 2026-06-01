use sha2::{Digest, Sha256};

fn trade_flow_node_public_json(node: &TradeFlowNode) -> Value {
    json!({
        "key": &node.key,
        "type": &node.node_type,
        "config": &node.config,
    })
}

fn trade_flow_node_config_hash(node: &TradeFlowNode) -> String {
    let raw = serde_json::to_vec(&node.config).unwrap_or_default();
    let digest = Sha256::digest(raw);
    let hex = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    format!("sha256:{hex}")
}

fn direct_upstream_node_snapshots(node: &TradeFlowNode, graph: &TradeFlowGraphRuntime) -> Value {
    let nodes_by_key = graph
        .nodes
        .iter()
        .map(|candidate| (candidate.key.as_str(), candidate))
        .collect::<std::collections::HashMap<_, _>>();
    let upstream = graph
        .edges
        .iter()
        .filter(|edge| edge.target == node.key)
        .filter_map(|edge| {
            let source = nodes_by_key.get(edge.source.as_str())?;
            Some(json!({
                "edge": {
                    "source": &edge.source,
                    "target": &edge.target,
                    "type": &edge.edge_type,
                    "condition": &edge.condition,
                },
                "node": trade_flow_node_public_json(source),
            }))
        })
        .collect::<Vec<_>>();
    Value::Array(upstream)
}

fn build_action_place_order_node_snapshot(
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    resolved_order_input: Value,
    config_version: Option<&str>,
) -> (String, Value) {
    let node_config_hash = trade_flow_node_config_hash(node);
    let snapshot = json!({
        "capture_source": "trade_flow_runtime",
        "captured_at": Utc::now().to_rfc3339(),
        "flow_run_id": run.id,
        "flow_definition_id": run.definition_id,
        "flow_version_id": run.version_id,
        "config_version": config_version,
        "node_key": &node.key,
        "node_type": &node.node_type,
        "node_config_hash": &node_config_hash,
        "action_node": trade_flow_node_public_json(node),
        "upstream_nodes": direct_upstream_node_snapshots(node, graph),
        "resolved_order_input": resolved_order_input,
    });
    (node_config_hash, snapshot)
}

async fn attach_action_place_order_node_snapshot(
    repo: &PostgresRepository,
    order_id: i64,
    root_order_id: i64,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    resolved_payload: &mut serde_json::Map<String, Value>,
    config_version: Option<String>,
) -> Result<()> {
    let (node_config_hash, snapshot) = build_action_place_order_node_snapshot(
        run,
        node,
        graph,
        Value::Object(resolved_payload.clone()),
        config_version.as_deref(),
    );
    repo.upsert_trade_builder_order_node_snapshot(
        &bot_infra::db::TradeBuilderOrderNodeSnapshotInput {
            order_id,
            root_order_id,
            flow_run_id: Some(run.id),
            flow_definition_id: Some(run.definition_id),
            flow_version_id: Some(run.version_id),
            node_key: node.key.clone(),
            node_type: node.node_type.clone(),
            node_config_hash,
            snapshot_json: snapshot.clone(),
            config_version: config_version.clone(),
        },
        config_version.as_deref(),
    )
    .await?;
    resolved_payload.insert("node_snapshot".to_string(), snapshot);
    Ok(())
}

#[cfg(test)]
mod trade_builder_node_snapshot_tests {
    use super::*;

    #[test]
    fn node_snapshot_captures_action_and_direct_upstream_nodes() {
        let action = TradeFlowNode {
            key: "entry".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({"side": "buy", "sizeUsdc": 5.0}),
        };
        let trigger = TradeFlowNode {
            key: "trigger".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({"asset": "BTC"}),
        };
        let graph = TradeFlowGraphRuntime {
            context: json!({}),
            nodes: vec![trigger, action.clone()],
            edges: vec![TradeFlowEdge {
                source: "trigger".to_string(),
                target: "entry".to_string(),
                edge_type: "on_success".to_string(),
                condition: Some(json!({"ok": true})),
            }],
        };
        let run = TradeFlowRun {
            id: 11,
            definition_id: 22,
            version_id: 33,
            user_id: 44,
            status: "running".to_string(),
            trigger_source: Some("test".to_string()),
            context_json: json!({}),
            started_at: Some(Utc::now()),
            ended_at: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let (hash, snapshot) =
            build_action_place_order_node_snapshot(&run, &action, &graph, json!({"market": "m"}), Some("v123"));

        assert!(hash.starts_with("sha256:"));
        assert_eq!(snapshot["action_node"]["key"], "entry");
        assert_eq!(snapshot["upstream_nodes"][0]["node"]["key"], "trigger");
        assert_eq!(snapshot["resolved_order_input"]["market"], "m");
        assert_eq!(snapshot["config_version"], "v123");
    }

    #[test]
    fn node_snapshot_handles_missing_config_version() {
        let action = TradeFlowNode {
            key: "entry".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({"side": "buy", "sizeUsdc": 5.0}),
        };
        let trigger = TradeFlowNode {
            key: "trigger".to_string(),
            node_type: "trigger.market_price".to_string(),
            config: json!({"asset": "BTC"}),
        };
        let graph = TradeFlowGraphRuntime {
            context: json!({}),
            nodes: vec![trigger, action.clone()],
            edges: vec![TradeFlowEdge {
                source: "trigger".to_string(),
                target: "entry".to_string(),
                edge_type: "on_success".to_string(),
                condition: Some(json!({"ok": true})),
            }],
        };
        let run = TradeFlowRun {
            id: 11,
            definition_id: 22,
            version_id: 33,
            user_id: 44,
            status: "running".to_string(),
            trigger_source: Some("test".to_string()),
            context_json: json!({}),
            started_at: Some(Utc::now()),
            ended_at: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let (_hash, snapshot) =
            build_action_place_order_node_snapshot(&run, &action, &graph, json!({"market": "m"}), None);

        assert!(snapshot["config_version"].is_null());
    }
}
