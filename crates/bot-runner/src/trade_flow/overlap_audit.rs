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
    let cross_flow_peers = repo
        .list_running_trade_flow_market_peers(run.user_id, market_slug, run.id)
        .await?
        .into_iter()
        .filter(|peer| peer.definition_id != run.definition_id)
        .collect::<Vec<_>>();
    let intra_flow_peers = repo
        .list_active_trade_flow_run_market_orders(run.id, market_slug)
        .await?
        .into_iter()
        .filter(|peer| peer.origin_flow_node_key.as_deref() != Some(node_key))
        .collect::<Vec<_>>();

    if cross_flow_peers.is_empty() && intra_flow_peers.is_empty() {
        return Ok(());
    }

    let current_flow_name = repo
        .get_trade_flow_definition(run.definition_id)
        .await?
        .map(|definition| definition.name)
        .unwrap_or_else(|| "?".to_string());
    let overlap_type = match (cross_flow_peers.is_empty(), intra_flow_peers.is_empty()) {
        (false, false) => "both",
        (false, true) => "cross_flow",
        (true, false) => "intra_flow",
        (true, true) => unreachable!("empty overlap sets return early"),
    };

    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "parallel_flow_overlap_observed",
        &json!({
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "side": side,
            "overlap_type": overlap_type,
            "current": {
                "definition_id": run.definition_id,
                "flow_name": current_flow_name,
                "run_id": run.id,
                "node_key": node_key,
                "source_trade_id": source_trade_id,
            },
            "cross_flow_peers": cross_flow_peers.into_iter().map(|peer| {
                json!({
                    "definition_id": peer.definition_id,
                    "flow_name": peer.definition_name,
                    "run_id": peer.run_id,
                    "source_trade_id": peer.source_trade_id,
                })
            }).collect::<Vec<_>>(),
            "intra_flow_peers": intra_flow_peers.into_iter().map(|peer| {
                json!({
                    "builder_order_id": peer.builder_order_id,
                    "node_key": peer.origin_flow_node_key,
                    "source_trade_id": peer.source_trade_id,
                })
            }).collect::<Vec<_>>(),
        }),
    )
    .await?;

    Ok(())
}
