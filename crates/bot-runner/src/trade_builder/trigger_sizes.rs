async fn resolve_trade_builder_next_trigger_size_usdc(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
) -> Result<(f64, Option<String>, Option<f64>, bool, usize)> {
    let next_index = order.triggers_fired.max(0) as usize;
    let Some((size_mode, trigger_sizes)) =
        repo.load_trade_builder_order_trigger_plan(order.id).await?
    else {
        return Ok((order.size_usdc, None, None, false, next_index));
    };
    if trigger_sizes.is_empty() {
        return Ok((order.size_usdc, size_mode, None, false, next_index));
    }
    if next_index >= trigger_sizes.len() {
        return Ok((0.0, size_mode, None, true, next_index));
    }

    let trigger_size_value = trigger_sizes[next_index];
    let normalized_mode = size_mode.unwrap_or_else(|| "usdc".to_string());
    let resolved_size_usdc = if normalized_mode == "pct" {
        let source_notional = repo
            .trade_notional_usdc(order.trade_id)
            .await?
            .unwrap_or(0.0);
        anyhow::ensure!(
            source_notional > 0.0,
            "trade_builder pct trigger size requires source trade notional > 0"
        );
        source_notional * (trigger_size_value / 100.0)
    } else {
        trigger_size_value
    };
    anyhow::ensure!(
        resolved_size_usdc > 0.0 && resolved_size_usdc.is_finite(),
        "trade_builder resolved trigger size must be > 0"
    );

    Ok((
        resolved_size_usdc,
        Some(normalized_mode),
        Some(trigger_size_value),
        false,
        next_index,
    ))
}
