async fn expire_trade_builder_order_for_stale_market(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    stale_market: &TradeBuilderStaleRollingMarket,
    expired_by_order_id: Option<i64>,
) -> Result<()> {
    repo.set_trade_builder_order_status(order.id, "expired", Some("stale_market_cycle"))
        .await?;
    repo.append_trade_builder_order_event(
        order.id,
        "expired",
        &json!({
            "reason_code": "stale_market_cycle",
            "reason_message": "Rolling market cycle advanced before the builder order could be submitted.",
            "status_before": &order.status,
            "status_after": "expired",
            "side": &order.side,
            "kind": &order.kind,
            "original_market_slug": &order.market_slug,
            "current_live_market_slug": &stale_market.current_live_market_slug,
            "original_token_id": &order.token_id,
            "detected_scope": stale_market.detected_scope,
            "detected_asset": stale_market.detected_asset,
            "detected_timeframe": stale_market.detected_timeframe,
            "current_live_selection_reason": stale_market.current_live_selection_reason.as_str(),
            "expired_by_order_id": expired_by_order_id,
        }),
    )
    .await?;
    maybe_send_order_not_filled_notification(
        repo,
        order,
        "stale_market_cycle",
        "Market cycle degisti, emir icra edilemeden expire oldu.",
    )
    .await;
    Ok(())
}

async fn maybe_expire_trade_builder_stale_order(
    repo: &PostgresRepository,
    gamma: &GammaHttpClient,
    order: &TradeBuilderOrder,
) -> Result<bool> {
    if order.active_exchange_order_id.is_some() {
        return Ok(false);
    }

    let Some(stale_market) = resolve_trade_builder_stale_market(gamma, &order.market_slug).await?
    else {
        return Ok(false);
    };

    expire_trade_builder_order_for_stale_market(repo, order, &stale_market, None).await?;

    if let Some(parent_order_id) = order.parent_order_id {
        let siblings = repo
            .list_trade_builder_child_orders_by_parent(parent_order_id, Some(order.id))
            .await?;
        for sibling in siblings {
            if sibling.market_slug != order.market_slug
                || sibling.side != "sell"
                || sibling.active_exchange_order_id.is_some()
                || !is_trade_builder_order_processable_status(&sibling.status)
            {
                continue;
            }
            expire_trade_builder_order_for_stale_market(
                repo,
                &sibling,
                &stale_market,
                Some(order.id),
            )
            .await?;
        }
    }

    Ok(true)
}
