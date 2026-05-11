fn action_place_order_ptb_stop_loss_bump_mode_as_str(
    mode: ActionPlaceOrderPtbStopLossBumpMode,
) -> &'static str {
    match mode {
        ActionPlaceOrderPtbStopLossBumpMode::Fixed => "fixed",
        ActionPlaceOrderPtbStopLossBumpMode::LossTable => "loss_table",
    }
}

fn action_place_order_ptb_stop_loss_bump_scope_mode_as_str(
    mode: ActionPlaceOrderPtbStopLossBumpScopeMode,
) -> &'static str {
    match mode {
        ActionPlaceOrderPtbStopLossBumpScopeMode::Global => "global",
        ActionPlaceOrderPtbStopLossBumpScopeMode::PerScope => "per_scope",
    }
}

fn action_place_order_ptb_stop_loss_bump_config_snapshot(
    config: &ActionPlaceOrderPtbStopLossBumpConfig,
    decay_step_usd: f64,
    bump_increment_usd: Option<f64>,
) -> Value {
    json!({
        "enabled": true,
        "mode": action_place_order_ptb_stop_loss_bump_mode_as_str(config.mode),
        "amount": config.amount,
        "unit": config.unit.as_str(),
        "max_value": config.max_value,
        "decay_windows": config.decay_windows,
        "decay_step_usd": decay_step_usd,
        "bump_increment_usd": bump_increment_usd,
        "scope_mode": action_place_order_ptb_stop_loss_bump_scope_mode_as_str(config.scope_mode),
        "loss_rules": config.loss_rules.iter().map(|rule| {
            json!({
                "loss_usd": rule.loss_usd,
                "bump_value": rule.bump_value,
                "bump_usd": rule.bump_usd,
            })
        }).collect::<Vec<_>>(),
    })
}

async fn maybe_resolve_action_place_order_ptb_stop_loss_bump_loss_metrics(
    repo: &PostgresRepository,
    parent_order: &TradeBuilderOrder,
    stop_loss_order: &TradeBuilderOrder,
) -> Result<Option<ActionPlaceOrderPtbStopLossBumpLossMetrics>> {
    let order_ids = [parent_order.id, stop_loss_order.id];
    let order_events = repo
        .list_trade_builder_order_events_for_orders(&order_ids)
        .await?;
    let mut events_by_order_id =
        std::collections::HashMap::<i64, Vec<TradeBuilderOrderEventRecord>>::new();
    for event in order_events {
        events_by_order_id
            .entry(event.builder_order_id)
            .or_default()
            .push(event);
    }

    let mut exchange_order_ids = order_ids
        .iter()
        .filter_map(|order_id| events_by_order_id.get(order_id).map(Vec::as_slice))
        .flat_map(trade_builder_analysis_extract_exchange_order_ids)
        .collect::<Vec<_>>();
    exchange_order_ids.sort();
    exchange_order_ids.dedup();
    let fill_summaries = repo
        .list_trade_builder_fill_summaries_by_exchange_order_ids(&exchange_order_ids)
        .await?;
    let fill_summaries_by_exchange_id = fill_summaries
        .into_iter()
        .map(|summary| (summary.exchange_order_id.clone(), summary))
        .collect::<std::collections::HashMap<_, _>>();

    let buy_metrics = trade_builder_analysis_resolve_order_metrics(
        parent_order.id,
        &events_by_order_id,
        &fill_summaries_by_exchange_id,
    );
    let sell_metrics = trade_builder_analysis_resolve_order_metrics(
        stop_loss_order.id,
        &events_by_order_id,
        &fill_summaries_by_exchange_id,
    );
    if buy_metrics.qty <= 0.0 || sell_metrics.qty <= 0.0 {
        return Ok(None);
    }

    let sell_qty = round_trade_builder_share_qty(sell_metrics.qty);
    if !sell_qty.is_finite() || sell_qty <= 0.0 {
        return Ok(None);
    }

    let cost_basis_per_share =
        (buy_metrics.notional_usdc + buy_metrics.fee_usdc) / buy_metrics.qty.max(0.0000001);
    let realized_loss_usd = ((sell_qty * cost_basis_per_share)
        - (sell_metrics.notional_usdc - sell_metrics.fee_usdc))
        .max(0.0);

    Ok(Some(ActionPlaceOrderPtbStopLossBumpLossMetrics {
        realized_loss_usd,
        sell_qty,
        sell_notional_usdc: sell_metrics.notional_usdc,
        sell_fee_usdc: sell_metrics.fee_usdc,
        cost_basis_per_share,
    }))
}
