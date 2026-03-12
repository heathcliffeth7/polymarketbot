fn trade_builder_detected_cancel_fill_qty(
    api_filled_size: Option<f64>,
    db_fill_qty: f64,
) -> Option<f64> {
    normalize_trade_builder_terminal_fill_qty_candidate(api_filled_size)
        .or_else(|| normalize_trade_builder_terminal_fill_qty_candidate(Some(db_fill_qty)))
}

fn trade_builder_detected_cancel_fill_notional(
    db_fill_notional: f64,
    detected_fill_qty: Option<f64>,
    post_cancel_price: Option<f64>,
    working_price: Option<f64>,
    current_price: f64,
) -> f64 {
    if db_fill_notional > 0.0 {
        return db_fill_notional;
    }

    let fallback_price = post_cancel_price.or(working_price).unwrap_or(current_price);
    detected_fill_qty.unwrap_or(0.0) * fallback_price
}

fn trade_builder_cancel_fill_detection_source(api_filled_size: Option<f64>) -> &'static str {
    if api_filled_size.is_some() {
        TradeBuilderTerminalFillQtySource::OrderInfoFilledSize.as_str()
    } else {
        "db_aggregate"
    }
}

fn trade_builder_cancel_fill_is_full(
    effective_status: &str,
    order_size: Option<f64>,
    fill_qty: f64,
) -> bool {
    effective_status == "filled"
        || order_size.is_some_and(|size| {
            (fill_qty - round_trade_builder_share_qty(size)).abs() < 0.02
        })
}

fn trade_builder_remaining_usdc_after_partial_fill(
    remaining_usdc: Option<f64>,
    order_remaining_size: Option<f64>,
    size_usdc: f64,
    detected_fill_notional: f64,
) -> f64 {
    let old_remaining = remaining_usdc.unwrap_or(order_remaining_size.unwrap_or(size_usdc));
    (old_remaining - detected_fill_notional).max(0.0)
}

enum TradeBuilderPostCancelBuyOutcome {
    NoFill,
    Finalized,
    RepriceRemainder {
        remaining_usdc: Option<f64>,
        remaining_qty: Option<f64>,
        size: f64,
    },
}

#[allow(clippy::too_many_arguments)]
async fn reconcile_trade_builder_post_cancel_buy_fill(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    order: &mut TradeBuilderOrder,
    exchange_order_id: &str,
    order_info: &OrderInfo,
    normalized: &str,
    current_price: f64,
    remaining_usdc: Option<f64>,
    desired_price: f64,
) -> Result<TradeBuilderPostCancelBuyOutcome> {
    let post_cancel_info = client.status(exchange_order_id).await;
    let api_filled_size = post_cancel_info
        .as_ref()
        .ok()
        .and_then(|info| normalize_trade_builder_terminal_fill_qty_candidate(info.filled_size));
    let _ = sync_recent_trade_builder_fills(repo, client).await;
    let (db_fill_qty, db_fill_notional) = repo
        .aggregate_fill_metrics_by_exchange_order_id(exchange_order_id)
        .await
        .unwrap_or((0.0, 0.0));
    let detected_fill_qty = trade_builder_detected_cancel_fill_qty(api_filled_size, db_fill_qty);
    let detection_source = trade_builder_cancel_fill_detection_source(api_filled_size);
    let detected_fill_notional = trade_builder_detected_cancel_fill_notional(
        db_fill_notional,
        detected_fill_qty,
        post_cancel_info.as_ref().ok().and_then(|info| info.price),
        order.working_price,
        current_price,
    );

    let Some(fill_qty) = detected_fill_qty else {
        return Ok(TradeBuilderPostCancelBuyOutcome::NoFill);
    };

    let effective_info = post_cancel_info.as_ref().ok().unwrap_or(order_info);
    let effective_status = post_cancel_info
        .as_ref()
        .ok()
        .map(|info| normalize_exchange_status(&info.status))
        .unwrap_or("canceled");
    if trade_builder_cancel_fill_is_full(effective_status, order_info.size, fill_qty) {
        let (canonical_entry_qty, canonical_entry_qty_source) =
            trade_builder_canonical_entry_qty(order, Some(fill_qty)).ok_or_else(|| {
                anyhow::anyhow!(
                    "builder order canonical fill qty unresolved for exchange_order_id={exchange_order_id}"
                )
            })?;
        let price = trade_builder_child_execution_price(
            order,
            effective_info.price,
            order.working_price,
            Some(current_price),
        )
        .unwrap_or(current_price);
        repo.mark_order_status(exchange_order_id, "filled").await?;
        repo.append_trade_builder_order_event(
            order.id,
            "reprice_cancel_full_fill_detected",
            &json!({
                "exchange_order_id": exchange_order_id,
                "pre_cancel_status": normalized,
                "post_cancel_filled_size": fill_qty,
                "filled_notional_usdc": detected_fill_notional,
                "detection_source": detection_source,
                "canonical_entry_qty": canonical_entry_qty,
                "canonical_entry_qty_source": canonical_entry_qty_source,
                "execution_price": price,
            }),
        )
        .await?;
        finalize_builder_fill(
            repo,
            ws,
            order,
            exchange_order_id,
            canonical_entry_qty,
            canonical_entry_qty_source,
            Some(fill_qty),
            price,
            false,
            Some(detection_source),
        )
        .await?;
        return Ok(TradeBuilderPostCancelBuyOutcome::Finalized);
    }

    let execution_price = effective_info
        .price
        .or(order.working_price)
        .unwrap_or(current_price);
    let cumulative_filled_qty =
        trade_builder_cumulative_fill_qty(order, Some(fill_qty)).unwrap_or(fill_qty);
    let old_remaining = remaining_usdc.unwrap_or(order.remaining_size.unwrap_or(order.size_usdc));
    let new_remaining_usdc = trade_builder_remaining_usdc_after_partial_fill(
        remaining_usdc,
        order.remaining_size,
        order.size_usdc,
        detected_fill_notional,
    );

    repo.mark_order_status(exchange_order_id, "canceled").await?;
    repo.set_trade_builder_order_filled_qty(order.id, cumulative_filled_qty)
        .await?;
    repo.append_trade_builder_order_event(
        order.id,
        "reprice_partial_fill_detected",
        &json!({
            "exchange_order_id": exchange_order_id,
            "detected_fill_qty": fill_qty,
            "detected_fill_notional_usdc": detected_fill_notional,
            "cumulative_filled_qty": cumulative_filled_qty,
            "old_remaining_usdc": old_remaining,
            "new_remaining_usdc": new_remaining_usdc,
            "execution_price": execution_price,
            "detection_source": detection_source,
        }),
    )
    .await?;
    let size = calc_level_size(new_remaining_usdc, desired_price);
    if size <= 0.0 {
        let (canonical_entry_qty, canonical_entry_qty_source) =
            trade_builder_canonical_entry_qty(order, Some(fill_qty)).ok_or_else(|| {
                anyhow::anyhow!(
                    "builder order canonical fill qty unresolved for exchange_order_id={exchange_order_id}"
                )
            })?;
        finalize_builder_fill(
            repo,
            ws,
            order,
            exchange_order_id,
            canonical_entry_qty,
            canonical_entry_qty_source,
            Some(fill_qty),
            execution_price,
            false,
            Some(detection_source),
        )
        .await?;
        return Ok(TradeBuilderPostCancelBuyOutcome::Finalized);
    }

    order.filled_qty = cumulative_filled_qty;
    Ok(TradeBuilderPostCancelBuyOutcome::RepriceRemainder {
        remaining_usdc: Some(new_remaining_usdc),
        remaining_qty: None,
        size,
    })
}
