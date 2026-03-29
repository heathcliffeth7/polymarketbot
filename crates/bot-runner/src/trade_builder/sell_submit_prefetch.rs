async fn prefetch_trade_builder_sell_submit_inputs(
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
    run_id: i64,
    current_price: f64,
    runtime_best_bid: Option<f64>,
    runtime_last_trade_price: Option<f64>,
    requested_qty: Option<f64>,
    size_basis: &str,
) -> (Option<TradeBuilderResolvedSellSubmitPrice>, Option<f64>) {
    if order.side != "sell" {
        return (None, None);
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
