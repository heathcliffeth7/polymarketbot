fn build_parallel_flow_overlap_payload(
    run: &TradeFlowRun,
    current_flow_name: &str,
    node_key: &str,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    side: &str,
    source_trade_id: i64,
    intra_flow_peers: Vec<ActiveTradeFlowRunOrderPeer>,
) -> Value {
    json!({
        "market_slug": market_slug,
        "token_id": token_id,
        "outcome_label": outcome_label,
        "side": side,
        "overlap_type": "intra_flow",
        "current": {
            "definition_id": run.definition_id,
            "flow_name": current_flow_name,
            "run_id": run.id,
            "node_key": node_key,
            "source_trade_id": source_trade_id,
        },
        "cross_flow_peers": Vec::<Value>::new(),
        "intra_flow_peers": intra_flow_peers.into_iter().map(|peer| {
            json!({
                "builder_order_id": peer.builder_order_id,
                "node_key": peer.origin_flow_node_key,
                "source_trade_id": peer.source_trade_id,
            })
        }).collect::<Vec<_>>(),
    })
}

async fn append_parallel_flow_overlap_observed(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node_key: &str,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    side: &str,
    source_trade_id: i64,
) -> Result<()> {
    let intra_flow_peers = repo
        .list_active_trade_flow_run_market_orders(run.id, market_slug)
        .await?
        .into_iter()
        .filter(|peer| peer.origin_flow_node_key.as_deref() != Some(node_key))
        .collect::<Vec<_>>();

    if intra_flow_peers.is_empty() {
        return Ok(());
    }

    let current_flow_name = repo
        .get_trade_flow_definition(run.definition_id)
        .await?
        .map(|definition| definition.name)
        .unwrap_or_else(|| "?".to_string());

    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "parallel_flow_overlap_observed",
        &build_parallel_flow_overlap_payload(
            run,
            &current_flow_name,
            node_key,
            market_slug,
            token_id,
            outcome_label,
            side,
            source_trade_id,
            intra_flow_peers,
        ),
    )
    .await?;

    Ok(())
}

#[cfg(test)]
mod overlap_audit_tests {
    use super::*;

    #[test]
    fn overlap_payload_is_intra_flow_only() {
        let run = TradeFlowRun {
            id: 42,
            definition_id: 7,
            version_id: 9,
            status: "running".to_string(),
            trigger_source: Some("publish_start".to_string()),
            context_json: json!({}),
            started_at: None,
            ended_at: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            user_id: 1,
        };

        let payload = build_parallel_flow_overlap_payload(
            &run,
            "mallorca21",
            "action_buy_1",
            "lal-elc-mal-2026-03-21",
            "tok-mallorca",
            "RCD Mallorca",
            "buy",
            107909,
            vec![ActiveTradeFlowRunOrderPeer {
                builder_order_id: 8528,
                source_trade_id: 107909,
                origin_flow_node_key: Some("action_buy_2".to_string()),
            }],
        );

        assert_eq!(payload.get("overlap_type"), Some(&json!("intra_flow")));
        assert_eq!(payload.get("cross_flow_peers"), Some(&json!([])));
        assert_eq!(
            payload.pointer("/intra_flow_peers/0/builder_order_id"),
            Some(&json!(8528))
        );
    }
}
