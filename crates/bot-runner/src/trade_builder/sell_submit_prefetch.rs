#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderResolvedSellSubmitPrice {
    desired_price: f64,
    uncapped_desired_price: f64,
    source: &'static str,
    depth_levels_used: Option<usize>,
    visible_bid_qty: Option<f64>,
    requested_qty: Option<f64>,
}

fn normalize_trade_builder_visible_qty(value: Option<f64>) -> Option<f64> {
    let value = value?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    Some(round_trade_builder_share_qty(value))
}

fn trade_builder_order_book_visible_bid_qty(snapshot: &OrderBookSnapshot) -> Option<f64> {
    let visible_qty = snapshot
        .bids
        .iter()
        .map(|level| level.size)
        .filter(|size| size.is_finite() && *size > 0.0)
        .sum::<f64>();
    normalize_trade_builder_visible_qty(Some(visible_qty))
}

fn trade_builder_order_book_best_bid(snapshot: &OrderBookSnapshot) -> Option<f64> {
    snapshot
        .bids
        .iter()
        .filter(|level| {
            level.price.is_finite()
                && level.size.is_finite()
                && level.price > 0.0
                && level.size > 0.0
        })
        .map(|level| clamp_probability(level.price))
        .fold(None, |best, price| match best {
            Some(current) if current >= price => Some(current),
            _ => Some(price),
        })
}

fn trade_builder_order_book_sweep_sell_price(
    snapshot: &OrderBookSnapshot,
    requested_qty: f64,
) -> Option<(f64, usize)> {
    let requested_qty = normalize_trade_builder_visible_qty(Some(requested_qty))?;
    let mut cumulative_qty = 0.0;
    let mut depth_levels_used = 0usize;

    for level in snapshot.bids.iter().rev() {
        if !level.price.is_finite()
            || !level.size.is_finite()
            || level.price <= 0.0
            || level.size <= 0.0
        {
            continue;
        }
        cumulative_qty = round_trade_builder_share_qty(cumulative_qty + level.size);
        depth_levels_used += 1;
        if cumulative_qty + TRADE_BUILDER_EXIT_QTY_TOLERANCE >= requested_qty {
            return Some((clamp_probability(level.price), depth_levels_used));
        }
    }

    None
}

fn resolve_trade_builder_sell_submit_price_with_book(
    order: &TradeBuilderOrder,
    current_price: f64,
    runtime_best_bid: Option<f64>,
    runtime_last_trade_price: Option<f64>,
    requested_qty: Option<f64>,
    order_book: Option<&OrderBookSnapshot>,
) -> TradeBuilderResolvedSellSubmitPrice {
    let requested_qty = normalize_trade_builder_visible_qty(requested_qty);
    let visible_bid_qty = order_book.and_then(trade_builder_order_book_visible_bid_qty);

    if let (Some(snapshot), Some(requested_qty)) = (order_book, requested_qty) {
        if let Some((sweep_price, depth_levels_used)) =
            trade_builder_order_book_sweep_sell_price(snapshot, requested_qty)
        {
            return TradeBuilderResolvedSellSubmitPrice {
                desired_price: trade_builder_cap_exit_sell_price(order, sweep_price),
                uncapped_desired_price: sweep_price,
                source: "orderbook_depth",
                depth_levels_used: Some(depth_levels_used),
                visible_bid_qty,
                requested_qty: Some(requested_qty),
            };
        }
    }

    let (uncapped_desired_price, source) = runtime_best_bid
        .and_then(|value| normalize_trade_builder_reference_price(Some(value)))
        .map(|value| (clamp_probability(value), "best_bid_fallback"))
        .or_else(|| {
            runtime_last_trade_price
                .and_then(|value| normalize_trade_builder_reference_price(Some(value)))
                .map(|value| (clamp_probability(value), "last_trade_fallback"))
        })
        .unwrap_or((clamp_probability(current_price), "price_fallback"));

    TradeBuilderResolvedSellSubmitPrice {
        desired_price: trade_builder_cap_exit_sell_price(order, uncapped_desired_price),
        uncapped_desired_price,
        source,
        depth_levels_used: None,
        visible_bid_qty,
        requested_qty,
    }
}

async fn resolve_trade_builder_sell_submit_price(
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
    current_price: f64,
    runtime_best_bid: Option<f64>,
    runtime_last_trade_price: Option<f64>,
    requested_qty: Option<f64>,
) -> TradeBuilderResolvedSellSubmitPrice {
    let order_book = client.order_book(&order.token_id).await.ok().flatten();
    resolve_trade_builder_sell_submit_price_with_book(
        order,
        current_price,
        runtime_best_bid,
        runtime_last_trade_price,
        requested_qty,
        order_book.as_ref(),
    )
}

async fn prefetch_trade_builder_sell_submit_inputs(
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
    run_id: i64,
    current_price: f64,
    runtime_best_bid: Option<f64>,
    runtime_last_trade_price: Option<f64>,
    requested_qty: Option<f64>,
    size_basis: &str,
    exit_fast_quote: Option<&ExitFastSubmitQuote>,
) -> (Option<TradeBuilderResolvedSellSubmitPrice>, Option<f64>) {
    if order.side != "sell" {
        return (None, None);
    }

    if let Some(quote) = exit_fast_quote {
        return (
            Some(resolve_trade_builder_sell_submit_price_with_book(
                order,
                current_price,
                runtime_best_bid,
                runtime_last_trade_price,
                requested_qty,
                Some(&quote.order_book),
            )),
            None,
        );
    }

    let order_book_fut = client.order_book(&order.token_id);
    if size_basis != TRADE_BUILDER_SIZE_BASIS_SHARES {
        let order_book = order_book_fut.await.ok().flatten();
        return (
            Some(resolve_trade_builder_sell_submit_price_with_book(
                order,
                current_price,
                runtime_best_bid,
                runtime_last_trade_price,
                requested_qty,
                order_book.as_ref(),
            )),
            None,
        );
    }

    let (order_book_result, available_qty_result) =
        tokio::join!(order_book_fut, client.available_token_qty(&order.token_id));
    let order_book = order_book_result.ok().flatten();
    let prefetched_available_qty = match available_qty_result {
        Ok(quantity) => quantity,
        Err(err) => {
            warn!(
                run_id,
                builder_order_id = order.id,
                token_id = %order.token_id,
                error = %err,
                "TRADE_BUILDER_EXIT_INVENTORY_PREFETCH_FAILED"
            );
            None
        }
    };

    (
        Some(resolve_trade_builder_sell_submit_price_with_book(
            order,
            current_price,
            runtime_best_bid,
            runtime_last_trade_price,
            requested_qty,
            order_book.as_ref(),
        )),
        prefetched_available_qty,
    )
}
