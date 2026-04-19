const PRICE_TO_BEAT_GUARD_RETRY_DELAY_MS: i64 = 150;

fn trade_builder_is_guard_blocked_for_reason(order: &TradeBuilderOrder, reason: &str) -> bool {
    order.status == TRADE_BUILDER_GUARD_BLOCKED_STATUS
        && order.last_error.as_deref() == Some(reason)
}

async fn transition_trade_builder_order_to_guard_waiting(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    reason: &str,
    event_type: &str,
    payload: &Value,
    remaining_size: Option<f64>,
    remaining_qty: Option<f64>,
    candidate_reason: Option<&str>,
    notification_type: Option<&str>,
    notification_message: Option<String>,
) -> Result<()> {
    let entered_waiting = !trade_builder_is_guard_blocked_for_reason(order, reason);
    repo.set_trade_builder_guard_blocked_state(order.id, reason, remaining_size, remaining_qty)
        .await?;
    if !entered_waiting {
        return Ok(());
    }

    repo.append_trade_builder_order_event(order.id, event_type, payload)
        .await?;

    if let (Some(candidate_reason), Some(notification_type), Some(message)) =
        (candidate_reason, notification_type, notification_message)
    {
        maybe_send_guard_transition_notification(
            repo,
            order,
            candidate_reason,
            true,
            notification_type,
            &message,
        )
        .await?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn maybe_handle_open_order_trigger_guard_cancel(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
    exchange_order_id: &str,
    order_info: &OrderInfo,
    normalized: &str,
    current_price: f64,
    best_ask: Option<f64>,
    desired_price: f64,
    remaining_size: Option<f64>,
    remaining_qty: Option<f64>,
) -> Result<bool> {
    let (trigger_guard_reference_price, trigger_guard_reference_source) =
        trade_builder_resolve_trigger_guard_reference_price(order, current_price, best_ask);
    if !trade_builder_price_below_guard_trigger(order, trigger_guard_reference_price) {
        return Ok(false);
    }

    let filled_size = normalize_trade_builder_terminal_fill_qty_candidate(order_info.filled_size)
        .unwrap_or_default();
    let execution_price = trade_builder_child_execution_price(
        order,
        order_info.price,
        order.working_price,
        Some(current_price),
    )
    .unwrap_or(current_price);
    match client.cancel(exchange_order_id).await {
        Ok(()) => {
            repo.mark_order_status(exchange_order_id, "canceled")
                .await?;
        }
        Err(err) => {
            let error_text = err.to_string();
            if cancel_error_indicates_terminal_match(&error_text) {
                let (
                    canonical_entry_qty,
                    canonical_entry_qty_source,
                    actual_fill_qty,
                    actual_fill_qty_source,
                ) = resolve_trade_builder_finalize_quantities(
                    repo,
                    client,
                    order,
                    exchange_order_id,
                    order_info,
                    None,
                )
                .await?;
                let terminal_price = trade_builder_child_execution_price(
                    order,
                    order_info.price,
                    order.working_price,
                    Some(current_price),
                )
                .unwrap_or(current_price);
                repo.mark_order_status(exchange_order_id, "filled").await?;
                repo.append_trade_builder_order_event(
                    order.id,
                    "trigger_price_canceled",
                    &json!({
                        "exchange_order_id": exchange_order_id,
                        "status_before": normalized,
                        "cancel_result": "terminal_match",
                        "canonical_entry_qty": canonical_entry_qty,
                        "canonical_entry_qty_source": canonical_entry_qty_source,
                        "actual_fill_qty": actual_fill_qty,
                        "actual_fill_qty_source": actual_fill_qty_source,
                        "execution_price": terminal_price,
                        "current_price": current_price,
                        "trigger_guard_reference_price": trigger_guard_reference_price,
                        "trigger_guard_reference_source": trigger_guard_reference_source,
                        "desired_price": desired_price,
                        "working_price": order.working_price,
                        "submitted_dynamic_price": order.submitted_dynamic_price,
                        "guard_trigger_price": order.guard_trigger_price,
                        "cancel_error": error_text
                    }),
                )
                .await?;
                finalize_builder_fill(
                    repo,
                    cfg,
                    ws,
                    order,
                    exchange_order_id,
                    canonical_entry_qty,
                    canonical_entry_qty_source,
                    actual_fill_qty,
                    terminal_price,
                    true,
                    actual_fill_qty_source,
                )
                .await?;
                return Ok(true);
            }
            return Err(err).context(format!(
                "failed to cancel builder order at trigger price guard: {exchange_order_id}"
            ));
        }
    }
    repo.append_trade_builder_order_event(
        order.id,
        "trigger_price_canceled",
        &json!({
            "exchange_order_id": exchange_order_id,
            "status_before": normalized,
            "actual_fill_qty": filled_size,
            "execution_price": execution_price,
            "current_price": current_price,
            "trigger_guard_reference_price": trigger_guard_reference_price,
            "trigger_guard_reference_source": trigger_guard_reference_source,
            "desired_price": desired_price,
            "working_price": order.working_price,
            "guard_trigger_price": order.guard_trigger_price
        }),
    )
    .await?;
    if filled_size > 0.0 {
        let (canonical_entry_qty, canonical_entry_qty_source) =
            trade_builder_canonical_entry_qty(order, Some(filled_size)).ok_or_else(|| {
                anyhow::anyhow!(
                    "builder order canonical fill qty unresolved for exchange_order_id={exchange_order_id}"
                )
            })?;
        finalize_builder_fill(
            repo,
            cfg,
            ws,
            order,
            exchange_order_id,
            canonical_entry_qty,
            canonical_entry_qty_source,
            Some(filled_size),
            execution_price,
            true,
            Some(TradeBuilderTerminalFillQtySource::OrderInfoFilledSize.as_str()),
        )
        .await?;
    } else if order.retry_on_trigger_guard_block {
        let candidate_reason =
            build_guard_notification_reason("trigger_price", "below_trigger_price_guard");
        let notification_message = order
            .notify_on_trigger_guard_blocked
            .then(|| {
                build_trigger_guard_waiting_notification_message(
                    order,
                    trigger_guard_reference_price,
                    trigger_guard_reference_source,
                )
            });
        transition_trade_builder_order_to_guard_waiting(
            repo,
            order,
            "below_trigger_price_guard",
            "trigger_price_waiting",
            &json!({
                "exchange_order_id": exchange_order_id,
                "status_before": normalized,
                "status_after": TRADE_BUILDER_GUARD_BLOCKED_STATUS,
                "current_price": current_price,
                "trigger_guard_reference_price": trigger_guard_reference_price,
                "trigger_guard_reference_source": trigger_guard_reference_source,
                "desired_price": desired_price,
                "working_price": order.working_price,
                "guard_trigger_price": order.guard_trigger_price,
            }),
            remaining_size,
            remaining_qty,
            Some(candidate_reason.as_str()),
            order
                .notify_on_trigger_guard_blocked
                .then_some("trigger_price_waiting"),
            notification_message,
        )
        .await?;
    } else {
        repo.clear_trade_builder_active_exchange_order(order.id, "canceled")
            .await?;
        repo.set_trade_builder_order_status(
            order.id,
            "canceled",
            Some("below_trigger_price_guard"),
        )
        .await?;
        let candidate_reason =
            build_guard_notification_reason("trigger_price", "below_trigger_price_guard");
        let message = build_trigger_guard_blocked_notification_message(
            order,
            trigger_guard_reference_price,
            trigger_guard_reference_source,
        );
        maybe_send_guard_transition_notification(
            repo,
            order,
            candidate_reason.as_str(),
            order.notify_on_trigger_guard_blocked,
            "trigger_price_blocked",
            &message,
        )
        .await?;
    }
    Ok(true)
}
