async fn maybe_resize_trade_builder_pending_exit_order(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    next_remaining_qty: f64,
    triggered_by_order_id: i64,
    reason: &str,
) -> Result<bool> {
    if trade_builder_is_terminal_status(&order.status)
        || order.active_exchange_order_id.is_some()
        || normalize_trade_builder_size_basis(&order.size_basis) != TRADE_BUILDER_SIZE_BASIS_SHARES
    {
        return Ok(false);
    }

    let Some(current_remaining_qty) = trade_builder_share_remaining_qty(order) else {
        return Ok(false);
    };
    let next_qty = round_trade_builder_share_qty(next_remaining_qty.min(current_remaining_qty));
    if current_remaining_qty - next_qty < TRADE_BUILDER_EXIT_QTY_TOLERANCE {
        return Ok(false);
    }

    let next_size_usdc = trade_builder_scaled_size_usdc(order, next_qty);
    repo.update_trade_builder_order_sizing_and_state(
        order.id,
        &order.size_basis,
        next_size_usdc,
        Some(next_qty),
        Some(next_size_usdc),
        Some(next_qty),
        &order.status,
        order.last_error.as_deref(),
        order.eligible_after_at,
        order.eligible_before_at,
        None,
        None,
        None,
    )
    .await?;
    repo.append_trade_builder_order_event(
        order.id,
        "sibling_resized_after_partial_fill",
        &json!({
            "triggered_by_order_id": triggered_by_order_id,
            "reason": reason,
            "status_after": &order.status,
            "previous_target_qty": order.target_qty,
            "previous_remaining_qty": order.remaining_qty,
            "next_target_qty": next_qty,
            "next_remaining_qty": next_qty,
        }),
    )
    .await?;
    Ok(true)
}

async fn request_trade_builder_oco_cancel_for_siblings(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    reason: &str,
) -> Result<Vec<i64>> {
    let Some(parent_order_id) = order.parent_order_id else {
        return Ok(Vec::new());
    };
    if order.side != "sell" {
        return Ok(Vec::new());
    }

    let siblings = repo
        .list_trade_builder_child_orders_by_parent(parent_order_id, Some(order.id))
        .await?;
    let mut sibling_order_ids = Vec::new();
    for sibling in siblings {
        if matches!(
            sibling.status.as_str(),
            "completed" | "canceled" | "expired" | "filled" | "canceled_requested"
        ) {
            continue;
        }
        if trade_builder_stop_loss_latched(&sibling) && reason != "stop_loss_latched" {
            continue;
        }

        repo.set_trade_builder_order_status(
            sibling.id,
            "canceled_requested",
            Some("oco_sibling_triggered"),
        )
        .await?;
        repo.append_trade_builder_order_event(
            sibling.id,
            "oco_cancel_requested",
            &json!({
                "parent_order_id": parent_order_id,
                "triggered_by_order_id": order.id,
                "reason": reason,
                "status_before": sibling.status,
                "status_after": "canceled_requested",
                "active_exchange_order_id": sibling.active_exchange_order_id
            }),
        )
        .await?;
        sibling_order_ids.push(sibling.id);
    }

    if !sibling_order_ids.is_empty() {
        repo.append_trade_builder_order_event(
            order.id,
            "oco_siblings_cancel_requested",
            &json!({
                "parent_order_id": parent_order_id,
                "sibling_order_ids": sibling_order_ids,
                "reason": reason
            }),
        )
        .await?;
    }

    Ok(sibling_order_ids)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderStopLossPreemptionDecision {
    current_price: f64,
    ready_for_inline_submit: bool,
}

#[derive(Debug, Default, Clone, PartialEq)]
struct TradeBuilderStopLossPreemption {
    tp_preempted: bool,
    ready_sl_order_ids: Vec<i64>,
}

fn evaluate_trade_builder_preempted_stop_loss(
    sibling: &TradeBuilderOrder,
    runtime_price: &TradeBuilderRuntimePrice,
) -> Option<TradeBuilderStopLossPreemptionDecision> {
    if !trade_builder_is_stop_loss_child(sibling)
        || trade_builder_is_terminal_status(&sibling.status)
    {
        return None;
    }

    if let Some(mode) = sibling.sl_trigger_price_mode.as_deref() {
        sl_trigger_eval_price_for_mode(mode, runtime_price)?;
    }

    let current_price = trade_builder_trigger_eval_price_for_order(sibling, runtime_price);
    let evaluation =
        evaluate_trade_builder_order_trigger(sibling, sibling.last_seen_price, current_price);
    if !evaluation.should_trigger
        || should_skip_trade_builder_composite_sl_bid_confirmation(sibling, runtime_price)
    {
        return None;
    }

    Some(TradeBuilderStopLossPreemptionDecision {
        current_price,
        ready_for_inline_submit: sibling.active_exchange_order_id.is_none()
            && sibling.status != "canceled_requested",
    })
}

async fn maybe_preempt_trade_builder_take_profit_for_stop_loss(
    repo: &PostgresRepository,
    order: &mut TradeBuilderOrder,
    runtime_price: &TradeBuilderRuntimePrice,
) -> Result<TradeBuilderStopLossPreemption> {
    let mut preemption = TradeBuilderStopLossPreemption::default();
    if !trade_builder_is_take_profit_child(order) {
        return Ok(preemption);
    }

    let Some(parent_order_id) = order.parent_order_id else {
        return Ok(preemption);
    };
    let siblings = repo
        .list_trade_builder_child_orders_by_parent(parent_order_id, Some(order.id))
        .await?;
    let remaining_qty = trade_builder_share_remaining_qty(order);
    let mut stop_loss_sibling_ids = Vec::new();
    let mut stop_loss_current_price = None;

    for sibling in siblings
        .iter()
        .filter(|sibling| trade_builder_is_stop_loss_child(sibling))
    {
        let Some(decision) = evaluate_trade_builder_preempted_stop_loss(sibling, runtime_price)
        else {
            if let Some(mode) = sibling.sl_trigger_price_mode.as_deref() {
                if sl_trigger_eval_price_for_mode(mode, runtime_price).is_none() {
                    repo.append_trade_builder_order_event(
                        sibling.id,
                        "selected_trigger_source_missing",
                        &json!({
                            "sl_trigger_price_mode": mode,
                            "best_bid": runtime_price.best_bid,
                            "last_trade_price": runtime_price.last_trade_price,
                            "context": "tp_preemption_check",
                            "status": &sibling.status,
                        }),
                    )
                    .await?;
                }
            }
            continue;
        };

        stop_loss_sibling_ids.push(sibling.id);
        stop_loss_current_price.get_or_insert(decision.current_price);
        if decision.ready_for_inline_submit {
            preemption.ready_sl_order_ids.push(sibling.id);
        }
        if !trade_builder_stop_loss_latched(sibling) {
            repo.set_trade_builder_order_trigger_latched(sibling.id, true, Some("stop_loss"))
                .await?;
            repo.append_trade_builder_order_event(
                sibling.id,
                "sl_latched",
                &json!({
                    "reason": "stop_loss",
                    "priority_source": "tp_guard",
                    "trigger_price": sibling.trigger_price,
                    "current_price": decision.current_price,
                    "sl_trigger_price_mode": &sibling.sl_trigger_price_mode,
                    "family": trade_builder_exit_family(sibling),
                    "exit_mode": trade_builder_exit_mode(sibling),
                    "sibling_policy": trade_builder_exit_sibling_policy(sibling),
                    "best_bid": runtime_price.best_bid,
                    "last_trade_price": runtime_price.last_trade_price,
                    "status_before": &sibling.status
                }),
            )
            .await?;
        }
        if let Some(remaining_qty) = remaining_qty {
            let _ = maybe_resize_trade_builder_pending_exit_order(
                repo,
                sibling,
                remaining_qty,
                order.id,
                "sl_priority_preempted",
            )
            .await?;
        }
        let event_type = if trade_builder_is_hard_exit_child(sibling) {
            "sl_became_sole_exit"
        } else {
            "sl_staged_exit_ready"
        };
        repo.append_trade_builder_order_event(
            sibling.id,
            event_type,
            &json!({
                "preempted_tp_order_id": order.id,
                "current_price": decision.current_price,
                "remaining_qty": remaining_qty,
                "family": trade_builder_exit_family(sibling),
                "exit_mode": trade_builder_exit_mode(sibling),
                "sibling_policy": trade_builder_exit_sibling_policy(sibling),
            }),
        )
        .await?;
    }

    if stop_loss_sibling_ids.is_empty() {
        return Ok(preemption);
    }

    let hard_stop_loss_sibling_ids = siblings
        .iter()
        .filter(|sibling| {
            stop_loss_sibling_ids.contains(&sibling.id) && trade_builder_is_hard_exit_child(sibling)
        })
        .map(|sibling| sibling.id)
        .collect::<Vec<_>>();
    if hard_stop_loss_sibling_ids.is_empty() {
        repo.append_trade_builder_order_event(
            order.id,
            "tp_deferred_by_staged_sl",
            &json!({
                "stop_loss_sibling_ids": stop_loss_sibling_ids,
                "current_price": stop_loss_current_price
                    .unwrap_or_else(|| trade_builder_trigger_eval_price_for_order(order, runtime_price)),
                "status_before": &order.status,
                "remaining_qty": remaining_qty,
                "family": trade_builder_exit_family(order),
                "exit_mode": trade_builder_exit_mode(order),
                "sibling_policy": trade_builder_exit_sibling_policy(order),
            }),
        )
        .await?;
        preemption.tp_preempted = true;
        return Ok(preemption);
    }

    for sibling in siblings.iter().filter(|sibling| {
        trade_builder_is_take_profit_child(sibling)
            && !trade_builder_is_terminal_status(&sibling.status)
    }) {
        let status_after = if sibling.active_exchange_order_id.is_some() {
            "canceled_requested"
        } else {
            "canceled"
        };
        repo.set_trade_builder_order_status(
            sibling.id,
            status_after,
            Some("sl_priority_preempted"),
        )
        .await?;
        repo.append_trade_builder_order_event(
            sibling.id,
            if sibling.active_exchange_order_id.is_some() {
                "sl_preempted_live_tp"
            } else {
                "tp_blocked_by_sl_priority"
            },
            &json!({
                "stop_loss_sibling_ids": &stop_loss_sibling_ids,
                "current_price": stop_loss_current_price,
                "status_before": &sibling.status,
                "status_after": status_after,
                "remaining_qty": trade_builder_share_remaining_qty(sibling),
                "active_exchange_order_id": sibling.active_exchange_order_id,
            }),
        )
        .await?;
    }

    let status_before = order.status.clone();
    let status_after = if order.active_exchange_order_id.is_some() {
        "canceled_requested"
    } else {
        "canceled"
    };
    let event_type = if order.active_exchange_order_id.is_some() {
        "sl_preempted_live_tp"
    } else {
        "tp_blocked_by_sl_priority"
    };
    repo.set_trade_builder_order_status(order.id, status_after, Some("sl_priority_preempted"))
        .await?;
    repo.append_trade_builder_order_event(
        order.id,
        event_type,
        &json!({
            "stop_loss_sibling_ids": stop_loss_sibling_ids,
            "current_price": stop_loss_current_price
                .unwrap_or_else(|| trade_builder_trigger_eval_price_for_order(order, runtime_price)),
            "status_before": status_before,
            "status_after": status_after,
            "remaining_qty": remaining_qty,
            "active_exchange_order_id": order.active_exchange_order_id,
        }),
    )
    .await?;
    order.status = status_after.to_string();
    order.last_error = Some("sl_priority_preempted".to_string());

    preemption.tp_preempted = true;
    Ok(preemption)
}

async fn maybe_latch_trade_builder_stop_loss(
    repo: &PostgresRepository,
    order: &mut TradeBuilderOrder,
    current_price: f64,
) -> Result<()> {
    if !trade_builder_is_stop_loss_child(order) || trade_builder_stop_loss_latched(order) {
        return Ok(());
    }

    repo.set_trade_builder_order_trigger_latched(order.id, true, Some("stop_loss"))
        .await?;
    order.trigger_latched = true;
    order.trigger_latched_reason = Some("stop_loss".to_string());
    repo.append_trade_builder_order_event(
        order.id,
        "sl_latched",
        &json!({
            "reason": "stop_loss",
            "trigger_price": order.trigger_price,
            "current_price": current_price,
            "sl_trigger_price_mode": &order.sl_trigger_price_mode,
            "family": trade_builder_exit_family(order),
            "exit_mode": trade_builder_exit_mode(order),
            "sibling_policy": trade_builder_exit_sibling_policy(order),
            "status_before": &order.status
        }),
    )
    .await?;
    if !trade_builder_is_hard_exit_child(order) {
        return Ok(());
    }
    let sibling_order_ids =
        request_trade_builder_oco_cancel_for_siblings(repo, order, "stop_loss_latched").await?;
    if !sibling_order_ids.is_empty() {
        repo.append_trade_builder_order_event(
            order.id,
            "tp_preempted_by_sl",
            &json!({
                "sibling_order_ids": sibling_order_ids,
                "current_price": current_price,
                "trigger_price": order.trigger_price,
                "family": trade_builder_exit_family(order),
                "exit_mode": trade_builder_exit_mode(order),
                "sibling_policy": trade_builder_exit_sibling_policy(order),
            }),
        )
        .await?;
    }

    Ok(())
}
