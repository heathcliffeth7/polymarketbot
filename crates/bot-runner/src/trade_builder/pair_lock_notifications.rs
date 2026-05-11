async fn maybe_send_trade_builder_pair_notification(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    notification_type: &str,
    message: &str,
    enabled: bool,
) {
    if enabled {
        let _ = send_trade_builder_notification(repo, order, notification_type, message).await;
    }
}

fn build_trade_builder_pair_locked_message(
    session: &TradeBuilderPairSession,
    projected_net_profit_usdc: f64,
) -> String {
    format!(
        "Pair Lock Basarili\nMarket: {}\nLocked Qty: {:.2}\nProj. Net Kar: {:.4} USDC\nMax Total: {:.2}c",
        session.market_slug,
        session.locked_qty.unwrap_or_default(),
        projected_net_profit_usdc,
        session.pair_target_total_cent
    )
}

fn build_trade_builder_pair_unwind_message(
    session: &TradeBuilderPairSession,
    reason: &str,
) -> String {
    format!(
        "Pair Lock Unwind\nMarket: {}\nSebep: {}\nMax Total: {:.2}c",
        session.market_slug, reason, session.pair_target_total_cent
    )
}
