async fn append_trade_builder_terminal_fill_qty_event(
    repo: &PostgresRepository,
    order_id: i64,
    exchange_order_id: &str,
    event_type: &str,
    candidates: TradeBuilderTerminalFillQtyCandidates,
    resolution: Option<TradeBuilderResolvedTerminalFillQty>,
    sync_recent_fills_error: Option<&str>,
) -> Result<()> {
    let payload = json!({
        "exchange_order_id": exchange_order_id,
        "resolved_qty": resolution.map(|value| value.qty),
        "source": resolution.map(|value| value.source.as_str()),
        "order_info_filled_size": candidates.order_info_filled_size,
        "synced_db_fill_qty": candidates.synced_db_fill_qty,
        "order_info_size": candidates.order_info_size,
        "stored_order_size": candidates.stored_order_size,
        "sync_recent_fills_error": sync_recent_fills_error,
    });
    repo.append_trade_builder_order_event(order_id, event_type, &payload)
        .await?;
    Ok(())
}

async fn resolve_trade_builder_terminal_fill_qty(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
    exchange_order_id: &str,
    order_info: &OrderInfo,
) -> Result<TradeBuilderResolvedTerminalFillQty> {
    let mut candidates = TradeBuilderTerminalFillQtyCandidates {
        order_info_filled_size: order_info.filled_size,
        synced_db_fill_qty: None,
        order_info_size: order_info.size,
        stored_order_size: repo
            .order_size_by_exchange_order_id(exchange_order_id)
            .await?,
    };
    if let Some(resolution) = select_trade_builder_terminal_fill_qty(candidates) {
        let event_type =
            if resolution.source == TradeBuilderTerminalFillQtySource::OrderInfoFilledSize {
                "filled_qty_resolved"
            } else {
                "filled_qty_fallback_used"
            };
        append_trade_builder_terminal_fill_qty_event(
            repo,
            order.id,
            exchange_order_id,
            event_type,
            candidates,
            Some(resolution),
            None,
        )
        .await?;
        return Ok(resolution);
    }

    let sync_recent_fills_error = match sync_recent_trade_builder_fills(repo, client).await {
        Ok(_) => None,
        Err(err) => Some(err.to_string()),
    };
    candidates.synced_db_fill_qty = Some(
        repo.aggregate_fill_qty_by_exchange_order_id(exchange_order_id)
            .await?,
    );

    append_trade_builder_terminal_fill_qty_event(
        repo,
        order.id,
        exchange_order_id,
        "filled_qty_unresolved",
        candidates,
        None,
        sync_recent_fills_error.as_deref(),
    )
    .await?;
    Err(anyhow::anyhow!(
        "builder order terminal fill qty unresolved for exchange_order_id={exchange_order_id}"
    ))
}

async fn resolve_trade_builder_parent_buy_actual_fill_qty(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
    exchange_order_id: &str,
    order_info: &OrderInfo,
) -> Result<Option<TradeBuilderResolvedTerminalFillQty>> {
    match resolve_trade_builder_terminal_fill_qty(
        repo,
        client,
        order,
        exchange_order_id,
        order_info,
    )
    .await
    {
        Ok(resolution)
            if resolution.source != TradeBuilderTerminalFillQtySource::StoredOrderSize =>
        {
            Ok(Some(resolution))
        }
        Ok(resolution) if trade_builder_submitted_dynamic_qty(order).is_some() => {
            repo.append_trade_builder_order_event(
                order.id,
                "actual_fill_qty_unresolved",
                &json!({
                    "exchange_order_id": exchange_order_id,
                    "reason": "stored_order_size_not_treated_as_actual_fill",
                    "ignored_qty": resolution.qty,
                    "ignored_source": resolution.source.as_str(),
                    "submitted_dynamic_qty": trade_builder_submitted_dynamic_qty(order),
                    "submitted_dynamic_price": trade_builder_submitted_dynamic_price(order),
                }),
            )
            .await?;
            Ok(None)
        }
        Ok(resolution) => Ok(Some(resolution)),
        Err(err) if trade_builder_submitted_dynamic_qty(order).is_some() => {
            repo.append_trade_builder_order_event(
                order.id,
                "actual_fill_qty_unresolved",
                &json!({
                    "exchange_order_id": exchange_order_id,
                    "reason": "actual_fill_resolution_failed",
                    "error": err.to_string(),
                    "order_info_filled_size": order_info.filled_size,
                    "order_info_size": order_info.size,
                    "submitted_dynamic_qty": trade_builder_submitted_dynamic_qty(order),
                    "submitted_dynamic_price": trade_builder_submitted_dynamic_price(order),
                }),
            )
            .await?;
            Ok(None)
        }
        Err(err) => Err(err),
    }
}

async fn resolve_trade_builder_finalize_quantities(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
    exchange_order_id: &str,
    order_info: &OrderInfo,
    fallback_qty: Option<f64>,
) -> Result<(f64, &'static str, Option<f64>, Option<&'static str>)> {
    let actual_fill_resolution = if trade_builder_should_track_buy_inventory_observation(order) {
        resolve_trade_builder_parent_buy_actual_fill_qty(
            repo,
            client,
            order,
            exchange_order_id,
            order_info,
        )
        .await?
    } else {
        Some(
            resolve_trade_builder_terminal_fill_qty(
                repo,
                client,
                order,
                exchange_order_id,
                order_info,
            )
            .await?,
        )
    };

    let actual_fill_qty = actual_fill_resolution.map(|resolution| resolution.qty);
    let actual_fill_qty_source =
        actual_fill_resolution.map(|resolution| resolution.source.as_str());
    let (canonical_entry_qty, canonical_entry_qty_source) = trade_builder_canonical_entry_qty(
        order,
        actual_fill_qty.or(fallback_qty),
    )
    .ok_or_else(|| {
        anyhow::anyhow!(
            "builder order canonical fill qty unresolved for exchange_order_id={exchange_order_id}"
        )
    })?;

    Ok((
        canonical_entry_qty,
        canonical_entry_qty_source,
        actual_fill_qty,
        actual_fill_qty_source,
    ))
}
