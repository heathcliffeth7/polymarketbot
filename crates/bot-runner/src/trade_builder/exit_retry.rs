async fn mark_trade_builder_inventory_pending(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    reason: &str,
    current_price: f64,
    required_qty: f64,
    available_qty: Option<f64>,
) -> Result<()> {
    if order.active_exchange_order_id.is_some() {
        repo.clear_trade_builder_active_exchange_order_preserve_sizing(
            order.id,
            "inventory_pending",
        )
        .await?;
    } else {
        repo.set_trade_builder_order_status(order.id, "inventory_pending", Some(reason))
            .await?;
    }
    repo.set_trade_builder_order_status(order.id, "inventory_pending", Some(reason))
        .await?;

    repo.append_trade_builder_order_event(
        order.id,
        "exit_inventory_pending",
        &json!({
            "reason": reason,
            "side": order.side,
            "size_basis": order.size_basis,
            "trigger_condition": order.trigger_condition,
            "trigger_price": order.trigger_price,
            "current_price": current_price,
            "required_qty": required_qty,
            "available_qty": available_qty,
        }),
    )
    .await?;

    info!(
        builder_order_id = order.id,
        token_id = %order.token_id,
        required_qty,
        available_qty,
        current_price,
        reason = reason,
        "TRADE_BUILDER_EXIT_INVENTORY_PENDING"
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn schedule_trade_builder_exit_sell_retry(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    event_type: &str,
    error_text: &str,
    current_price: f64,
    desired_price: f64,
    requested_qty: Option<f64>,
    available_qty: Option<f64>,
    attempted_qty: Option<f64>,
    next_remaining_qty_override: Option<f64>,
    attempt_stage: Option<TradeBuilderExitSubmitStage>,
    next_stage: Option<TradeBuilderExitSubmitStage>,
) -> Result<()> {
    let preserve_visible_inventory_stage_qty = next_remaining_qty_override.is_none()
        && next_stage == Some(TradeBuilderExitSubmitStage::VisibleInventory)
        && attempt_stage != Some(TradeBuilderExitSubmitStage::VisibleInventory);
    let next_remaining_qty = next_remaining_qty_override
        .or(if preserve_visible_inventory_stage_qty {
            attempted_qty
        } else {
            None
        })
        .or_else(|| attempted_qty.and_then(trade_builder_next_retry_share_qty))
        .or(order.remaining_qty);
    let next_remaining_size = next_remaining_qty.map(|qty| (qty * desired_price).max(0.0));
    let stored_error_text = trade_builder_retry_error_text(error_text, next_stage);
    let qty_changed =
        attempted_qty
            .zip(next_remaining_qty)
            .is_some_and(|(previous_qty, next_qty)| {
                trade_builder_retry_qty_is_lower(previous_qty, next_qty)
            });

    repo.set_trade_builder_order_retry_state(
        order.id,
        "triggered",
        Some(&stored_error_text),
        next_remaining_size,
        next_remaining_qty,
    )
    .await?;
    if next_remaining_qty_override.is_none() && !preserve_visible_inventory_stage_qty {
        if let (Some(previous_qty), Some(shaved_qty)) = (attempted_qty, next_remaining_qty) {
            if trade_builder_retry_qty_is_lower(previous_qty, shaved_qty) {
                repo.append_trade_builder_order_event(
                    order.id,
                    "local_inventory_retry_shaved",
                    &json!({
                        "previous_qty": previous_qty,
                        "next_qty": shaved_qty,
                        "buffer_qty": TRADE_BUILDER_LOCAL_EXIT_QTY_BUFFER,
                        "reason": error_text,
                    }),
                )
                .await?;
            }
        }
    }
    repo.append_trade_builder_order_event(
        order.id,
        event_type,
        &json!({
            "reason": error_text,
            "status_before": &order.status,
            "status_after": "triggered",
            "active_exchange_order_id": order.active_exchange_order_id,
            "current_price": current_price,
            "desired_price": desired_price,
            "requested_qty": requested_qty,
            "attempted_qty": attempted_qty,
            "available_qty": available_qty,
            "next_remaining_qty_override": next_remaining_qty_override,
            "attempt_stage": attempt_stage.map(TradeBuilderExitSubmitStage::as_str),
            "next_attempt_stage": next_stage.map(TradeBuilderExitSubmitStage::as_str),
            "qty_changed": qty_changed,
            "qty_change_strategy": if next_remaining_qty_override.is_some() {
                "override"
            } else if preserve_visible_inventory_stage_qty {
                "preserve_for_visible_inventory_stage"
            } else if qty_changed {
                "auto_shave"
            } else {
                "unchanged"
            },
            "size_basis": &order.size_basis,
            "target_qty": order.target_qty,
            "remaining_qty": next_remaining_qty
        }),
    )
    .await?;
    info!(
        builder_order_id = order.id,
        token_id = %order.token_id,
        reason = error_text,
        current_price,
        desired_price,
        "TRADE_BUILDER_EXIT_SELL_RETRY_SCHEDULED"
    );
    Ok(())
}

async fn resolve_trade_builder_order_fee_rate_bps(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    order: &mut TradeBuilderOrder,
) -> Result<u64> {
    let default_fee_rate_bps = trade_builder_fee_rate_bps_or_default(order.fee_rate_bps);
    match client.fee_rate_bps(&order.token_id).await {
        Ok(Some(fee_rate_bps)) if fee_rate_bps > 0 => {
            if order.fee_rate_bps != fee_rate_bps as i64 {
                repo.set_trade_builder_order_fee_rate_bps(order.id, fee_rate_bps as i64)
                    .await?;
                order.fee_rate_bps = fee_rate_bps as i64;
            }
            Ok(fee_rate_bps)
        }
        Ok(_) => {
            if order.fee_rate_bps <= 0 {
                repo.set_trade_builder_order_fee_rate_bps(order.id, default_fee_rate_bps as i64)
                    .await?;
                order.fee_rate_bps = default_fee_rate_bps as i64;
                repo.append_trade_builder_order_event(
                    order.id,
                    "fee_rate_fallback",
                    &json!({
                        "reason": "fee_rate_lookup_empty",
                        "token_id": order.token_id,
                        "fallback_fee_rate_bps": default_fee_rate_bps
                    }),
                )
                .await?;
            }
            Ok(default_fee_rate_bps)
        }
        Err(err) => {
            warn!(
                builder_order_id = order.id,
                token_id = %order.token_id,
                error = %err,
                "TRADE_BUILDER_FEE_RATE_LOOKUP_FAILED"
            );
            if order.fee_rate_bps <= 0 {
                repo.set_trade_builder_order_fee_rate_bps(order.id, default_fee_rate_bps as i64)
                    .await?;
                order.fee_rate_bps = default_fee_rate_bps as i64;
                repo.append_trade_builder_order_event(
                    order.id,
                    "fee_rate_fallback",
                    &json!({
                        "reason": "fee_rate_lookup_failed",
                        "token_id": order.token_id,
                        "error": err.to_string(),
                        "fallback_fee_rate_bps": default_fee_rate_bps
                    }),
                )
                .await?;
            }
            Ok(default_fee_rate_bps)
        }
    }
}
