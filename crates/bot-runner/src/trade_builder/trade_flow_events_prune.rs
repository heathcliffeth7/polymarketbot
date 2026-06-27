async fn run_trade_flow_events_prune_task(repo: PostgresRepository) {
    loop {
        prune_trade_flow_events_once(&repo).await;
        tokio::time::sleep(Duration::from_secs(TRADE_FLOW_EVENTS_PRUNE_INTERVAL_SECS)).await;
    }
}

async fn prune_trade_flow_events_once(repo: &PostgresRepository) {
    match repo
        .delete_old_trade_flow_events(TRADE_FLOW_EVENTS_RETENTION_DAYS)
        .await
    {
        Ok(rows) => info!(rows, "TRADE_FLOW_EVENTS_PRUNED"),
        Err(err) => warn!(error = %err, "TRADE_FLOW_EVENTS_PRUNE_FAILED"),
    }
}
