#[derive(Debug, Clone, PartialEq)]
struct TradeBuilderExitFillPositionSummary {
    sold_this_fill_qty: f64,
    target_qty: Option<f64>,
    remaining_qty: Option<f64>,
    remaining_qty_source: &'static str,
    remaining_qty_estimated: bool,
    mark_price: Option<f64>,
    mark_price_source: &'static str,
    remaining_mark_value: Option<f64>,
    remaining_max_loss: Option<f64>,
    remaining_if_win: Option<f64>,
    closed: bool,
}

fn trade_builder_exit_fill_position_summary(
    order: &TradeBuilderOrder,
    sold_this_fill_qty: Option<f64>,
    execution_price: f64,
    updated_parent_position: Option<&TradeBuilderParentPosition>,
) -> Option<TradeBuilderExitFillPositionSummary> {
    if !trade_builder_is_child_exit_sell(order) {
        return None;
    }
    let sold_this_fill_qty = normalize_trade_builder_terminal_fill_qty_candidate(sold_this_fill_qty)
        .unwrap_or(0.0);
    let target_qty = order
        .target_qty
        .or(order.remaining_qty)
        .filter(|qty| qty.is_finite())
        .map(|qty| round_trade_builder_share_qty(qty.max(0.0)));
    let (remaining_qty, remaining_qty_source, remaining_qty_estimated) =
        if let Some(position) = updated_parent_position {
            (
                Some(round_trade_builder_share_qty(position.current_qty.max(0.0))),
                "trade_builder_parent_positions",
                false,
            )
        } else {
            (
                target_qty.map(|target| round_trade_builder_share_qty((target - sold_this_fill_qty).max(0.0))),
                "target_qty_minus_fill_estimate",
                true,
            )
        };
    let closed = remaining_qty.is_some_and(|qty| qty <= TRADE_BUILDER_EXIT_QTY_TOLERANCE);
    let mark_price = execution_price.is_finite().then_some(execution_price);
    let remaining_mark_value = remaining_qty
        .zip(mark_price)
        .map(|(qty, price)| if closed { 0.0 } else { qty * price });
    let remaining_if_win = remaining_qty.map(|qty| if closed { 0.0 } else { qty });
    Some(TradeBuilderExitFillPositionSummary {
        sold_this_fill_qty,
        target_qty,
        remaining_qty,
        remaining_qty_source,
        remaining_qty_estimated,
        mark_price,
        mark_price_source: "last_exit_fill_price_estimate",
        remaining_mark_value,
        remaining_max_loss: remaining_mark_value,
        remaining_if_win,
        closed,
    })
}
