fn action_place_order_target_qty(node: &TradeFlowNode) -> Option<f64> {
    node_config_f64(node, "targetQty")
        .or_else(|| node_config_f64(node, "target_qty"))
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn action_place_order_reference_price_for_share_sizing(reference_price: Option<f64>) -> f64 {
    reference_price
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .unwrap_or(0.5)
}

fn resolve_action_place_order_source_trade_seed_size_usdc(
    trigger_size_for_first_fire: Option<f64>,
    configured_size_usdc: Option<f64>,
    configured_target_qty: Option<f64>,
    use_share_size: bool,
    reference_price: Option<f64>,
) -> Result<f64> {
    if use_share_size {
        let target_qty = trigger_size_for_first_fire
            .or(configured_target_qty)
            .ok_or_else(|| anyhow::anyhow!("action.place_order requires targetQty > 0 when sizeMode is shares"))?;
        anyhow::ensure!(target_qty > 0.0, "action.place_order targetQty must be > 0");
        return Ok(target_qty * action_place_order_reference_price_for_share_sizing(reference_price));
    }

    let seed_size_usdc = trigger_size_for_first_fire
        .or(configured_size_usdc)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "action.place_order requires sizeUsdc/targetNotionalUsdc > 0 (or sizePct in pct mode)"
            )
        })?;
    anyhow::ensure!(seed_size_usdc > 0.0, "action.place_order size must be > 0");
    Ok(seed_size_usdc)
}

fn resolve_action_place_order_share_sizing(
    trigger_size_for_first_fire: Option<f64>,
    configured_target_qty: Option<f64>,
    reference_price: Option<f64>,
) -> Result<ActionPlaceOrderSizing> {
    let target_qty = trigger_size_for_first_fire
        .or(configured_target_qty)
        .ok_or_else(|| anyhow::anyhow!("action.place_order requires targetQty > 0 when sizeMode is shares"))?;
    anyhow::ensure!(target_qty > 0.0, "action.place_order targetQty must be > 0");
    let target_qty = round_trade_builder_share_qty(target_qty);
    let estimated_notional =
        target_qty * action_place_order_reference_price_for_share_sizing(reference_price);
    Ok(ActionPlaceOrderSizing {
        size_usdc: estimated_notional,
        size_basis: TRADE_BUILDER_SIZE_BASIS_SHARES,
        target_qty: Some(target_qty),
        remaining_qty: Some(target_qty),
        resolved_size_mode: "shares",
        resolved_size_pct: None,
    })
}

async fn resolve_action_place_order_buy_sizing(
    repo: &PostgresRepository,
    source_trade_id: i64,
    trigger_size_for_first_fire: Option<f64>,
    configured_size_usdc: Option<f64>,
    configured_size_pct: Option<f64>,
    configured_target_qty: Option<f64>,
    use_pct_size: bool,
    use_share_size: bool,
    reference_price: Option<f64>,
) -> Result<ActionPlaceOrderSizing> {
    if use_share_size {
        return resolve_action_place_order_share_sizing(
            trigger_size_for_first_fire,
            configured_target_qty,
            reference_price,
        );
    }

    if use_pct_size {
        let size_pct = trigger_size_for_first_fire
            .or(configured_size_pct)
            .ok_or_else(|| anyhow::anyhow!("action.place_order requires sizePct (0, 100] when sizeMode is pct"))?;
        anyhow::ensure!(size_pct > 0.0 && size_pct <= 100.0, "action.place_order sizePct must be in (0, 100]");
        let source_notional = repo.trade_notional_usdc(source_trade_id).await?.unwrap_or(0.0);
        anyhow::ensure!(source_notional > 0.0, "action.place_order sizePct requires source trade notional > 0");
        let resolved = source_notional * (size_pct / 100.0);
        anyhow::ensure!(resolved > 0.0, "action.place_order resolved size must be > 0");
        return Ok(ActionPlaceOrderSizing {
            size_usdc: resolved,
            size_basis: TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC,
            target_qty: None,
            remaining_qty: None,
            resolved_size_mode: "pct",
            resolved_size_pct: Some(size_pct),
        });
    }

    let resolved = trigger_size_for_first_fire
        .or(configured_size_usdc)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "action.place_order requires sizeUsdc/targetNotionalUsdc > 0 (or sizePct in pct mode)"
            )
        })?;
    anyhow::ensure!(resolved > 0.0, "action.place_order size must be > 0");
    Ok(ActionPlaceOrderSizing {
        size_usdc: resolved,
        size_basis: TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC,
        target_qty: None,
        remaining_qty: None,
        resolved_size_mode: "usdc",
        resolved_size_pct: None,
    })
}
