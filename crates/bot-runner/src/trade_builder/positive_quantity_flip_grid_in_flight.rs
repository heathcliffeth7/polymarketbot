async fn positive_quantity_flip_grid_in_flight_buy_skip(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    market_slug: &str,
    state: &TradeBuilderPositiveQuantityFlipGridState,
) -> Result<Option<TradeFlowNodeExecution>> {
    let active_buys = repo
        .list_active_positive_quantity_flip_grid_buys(
            run.user_id,
            Some(run.definition_id),
            &node.key,
            market_slug,
        )
        .await?;
    let Some(details) = positive_quantity_flip_grid_in_flight_buy_details(&active_buys, state)
    else {
        return Ok(None);
    };
    Ok(Some(positive_quantity_flip_grid_output_skipped(
        node,
        market_slug,
        "positive_grid_buy_in_flight",
        details,
    )))
}

async fn positive_quantity_flip_grid_prepare_buy_order_submission(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    market_slug: &str,
    state: &TradeBuilderPositiveQuantityFlipGridState,
) -> Result<Result<PositiveQuantityFlipGridBuyExecutionLock, TradeFlowNodeExecution>> {
    if let Some(execution) =
        positive_quantity_flip_grid_in_flight_buy_skip(repo, run, node, market_slug, state).await?
    {
        return Ok(Err(execution));
    }
    let Some(lock) = repo
        .try_acquire_positive_quantity_flip_grid_buy_execution_lock(
            run.user_id,
            run.definition_id,
            &node.key,
            market_slug,
        )
        .await?
    else {
        return Ok(Err(positive_quantity_flip_grid_output_skipped(
            node,
            market_slug,
            "buy_execution_coalesced",
            json!({
                "market_slug": market_slug,
                "contention": true,
            }),
        )));
    };
    Ok(Ok(lock))
}

fn positive_quantity_flip_grid_in_flight_buy_details(
    active_buys: &[TradeBuilderPositiveQuantityFlipGridActiveBuy],
    state: &TradeBuilderPositiveQuantityFlipGridState,
) -> Option<Value> {
    let active = active_buys.first()?;
    let active_buy_notional_usdc: f64 = active_buys.iter().map(|order| order.size_usdc).sum();
    Some(json!({
        "active_order_id": active.order_id,
        "active_grid_side": active.grid_side,
        "active_outcome_label": active.outcome_label,
        "active_status": active.status,
        "active_created_at": active.created_at.to_rfc3339(),
        "active_order_count": active_buys.len(),
        "active_buy_notional_usdc": active_buy_notional_usdc,
        "projected_total_buy_cost_usdc": state.total_buy_cost + active_buy_notional_usdc,
    }))
}
