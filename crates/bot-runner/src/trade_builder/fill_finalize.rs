async fn maybe_schedule_trade_builder_stop_loss_reentry(
    repo: &PostgresRepository,
    parent_order: &TradeBuilderOrder,
    stop_loss_order: &TradeBuilderOrder,
) -> Result<()> {
    if !parent_order.reenter_on_sl_hit {
        return Ok(());
    }

    let Some(run_id) = parent_order.origin_flow_run_id else {
        repo.append_trade_builder_order_event(
            parent_order.id,
            "reentry_skipped",
            &json!({
                "reason": "missing_origin_flow_run",
                "sl_child_order_id": stop_loss_order.id,
            }),
        )
        .await?;
        return Ok(());
    };
    let Some(action_node_key) = parent_order
        .origin_flow_node_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        repo.append_trade_builder_order_event(
            parent_order.id,
            "reentry_skipped",
            &json!({
                "reason": "missing_origin_flow_node_key",
                "sl_child_order_id": stop_loss_order.id,
                "flow_run_id": run_id,
            }),
        )
        .await?;
        return Ok(());
    };
    let Some(trigger_node_key) = parent_order
        .reentry_trigger_node_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        repo.append_trade_builder_order_event(
            parent_order.id,
            "reentry_skipped",
            &json!({
                "reason": "missing_reentry_trigger_node_key",
                "sl_child_order_id": stop_loss_order.id,
                "flow_run_id": run_id,
            }),
        )
        .await?;
        return Ok(());
    };

    let Some(flow_run) = repo.get_trade_flow_run(run_id).await? else {
        repo.append_trade_builder_order_event(
            parent_order.id,
            "reentry_skipped",
            &json!({
                "reason": "flow_run_missing",
                "sl_child_order_id": stop_loss_order.id,
                "flow_run_id": run_id,
                "trigger_node_key": trigger_node_key,
            }),
        )
        .await?;
        return Ok(());
    };
    if flow_run.status != "running" {
        repo.append_trade_builder_order_event(
            parent_order.id,
            "reentry_skipped",
            &json!({
                "reason": "flow_run_not_running",
                "sl_child_order_id": stop_loss_order.id,
                "flow_run_id": run_id,
                "run_status": flow_run.status,
                "trigger_node_key": trigger_node_key,
            }),
        )
        .await?;
        return Ok(());
    }

    let max_attempts = i64::from(parent_order.reentry_max_attempts.max(0));
    let attempts_used = flow_node_reentry_attempts_used(&flow_run.context_json, action_node_key);
    if attempts_used >= max_attempts {
        repo.append_trade_builder_order_event(
            parent_order.id,
            "reentry_limit_reached",
            &json!({
                "sl_child_order_id": stop_loss_order.id,
                "flow_run_id": run_id,
                "trigger_node_key": trigger_node_key,
                "attempts_used": attempts_used,
                "max_attempts": max_attempts,
            }),
        )
        .await?;
        return Ok(());
    }

    let next_generation = attempts_used + 1;
    let mut context = flow_run.context_json.clone();
    set_flow_node_state(
        &mut context,
        action_node_key,
        FLOW_NODE_STATE_REENTRY_ATTEMPTS_USED,
        json!(next_generation),
    );
    set_flow_node_state(
        &mut context,
        trigger_node_key,
        FLOW_NODE_STATE_REENTRY_GENERATION,
        json!(next_generation),
    );
    clear_trade_flow_market_price_once_state(&mut context, trigger_node_key);
    clear_trade_flow_market_price_ws_runtime_state(&mut context, trigger_node_key);
    repo.update_trade_flow_run_context(run_id, &context).await?;
    let cache_updated = replace_trade_flow_ws_fast_path_run_context(run_id, &context).await;

    let idempotency_key = format!("reentry:{run_id}:{trigger_node_key}:gen{next_generation}");
    let enqueued = repo
        .enqueue_trade_flow_step(
            run_id,
            trigger_node_key,
            "trigger.market_price",
            1,
            None,
            Utc::now(),
            None,
            Some(&idempotency_key),
        )
        .await?;
    if enqueued.is_some() {
        FLOW_PROCESS_NOTIFY.notify_one();
    }

    repo.append_trade_builder_order_event(
        parent_order.id,
        if enqueued.is_some() {
            "reentry_scheduled"
        } else {
            "reentry_duplicate_ignored"
        },
        &json!({
            "sl_child_order_id": stop_loss_order.id,
            "flow_run_id": run_id,
            "trigger_node_key": trigger_node_key,
            "generation": next_generation,
            "attempts_used": next_generation,
            "max_attempts": max_attempts,
            "idempotency_key": idempotency_key,
            "step_enqueued": enqueued.is_some(),
            "fast_path_cache_updated": cache_updated,
        }),
    )
    .await?;

    Ok(())
}

async fn finalize_builder_fill(
    repo: &PostgresRepository,
    ws: &ClobWsClient,
    order: &TradeBuilderOrder,
    exchange_order_id: &str,
    canonical_entry_qty: f64,
    canonical_entry_qty_source: &str,
    actual_fill_qty: Option<f64>,
    execution_price: f64,
    force_terminal: bool,
    actual_fill_qty_source: Option<&str>,
) -> Result<()> {
    let canonical_entry_qty = round_trade_builder_share_qty(canonical_entry_qty);
    let actual_fill_qty = normalize_trade_builder_terminal_fill_qty_candidate(actual_fill_qty);
    let persisted_fill_qty = if canonical_entry_qty_source == "cumulative_fill_qty" {
        Some(canonical_entry_qty)
    } else {
        actual_fill_qty
    };
    if order.side == "buy" && (order.tp_enabled || order.sl_enabled) {
        anyhow::ensure!(
            canonical_entry_qty > 0.0,
            "builder buy fill qty must be > 0 before creating exit children"
        );
    }
    repo.increment_trade_builder_trigger_count(order.id).await?;
    if let Some(actual_fill_qty) = persisted_fill_qty {
        repo.set_trade_builder_order_filled_qty(order.id, actual_fill_qty)
            .await?;
    }
    if order.side == "buy" {
        maybe_record_trade_builder_buy_fill_observation(
            repo,
            order,
            exchange_order_id,
            actual_fill_qty,
            execution_price,
            actual_fill_qty_source,
            force_terminal,
        )
        .await;
        if canonical_entry_qty_source == "submitted_dynamic_qty" {
            repo.append_trade_builder_order_event(
                order.id,
                "dynamic_qty_used_for_children",
                &json!({
                    "exchange_order_id": exchange_order_id,
                    "canonical_entry_qty": canonical_entry_qty,
                    "canonical_entry_qty_source": canonical_entry_qty_source,
                    "actual_fill_qty": actual_fill_qty,
                    "actual_fill_qty_source": actual_fill_qty_source,
                    "execution_price": execution_price,
                }),
            )
            .await?;
            match actual_fill_qty {
                Some(actual_fill_qty)
                    if (actual_fill_qty - canonical_entry_qty).abs()
                        >= TRADE_BUILDER_EXIT_QTY_TOLERANCE =>
                {
                    repo.append_trade_builder_order_event(
                        order.id,
                        "dynamic_vs_actual_fill_mismatch",
                        &json!({
                            "exchange_order_id": exchange_order_id,
                            "canonical_entry_qty": canonical_entry_qty,
                            "actual_fill_qty": actual_fill_qty,
                            "actual_fill_qty_source": actual_fill_qty_source,
                            "qty_delta": round_trade_builder_signed_qty(
                                canonical_entry_qty - actual_fill_qty
                            ),
                        }),
                    )
                    .await?;
                }
                None => {
                    repo.append_trade_builder_order_event(
                        order.id,
                        "actual_fill_qty_unresolved",
                        &json!({
                            "exchange_order_id": exchange_order_id,
                            "canonical_entry_qty": canonical_entry_qty,
                            "canonical_entry_qty_source": canonical_entry_qty_source,
                            "submitted_dynamic_price": trade_builder_submitted_dynamic_price(order),
                        }),
                    )
                    .await?;
                }
                _ => {}
            }
        }
    }

    let next_trigger_count = order.triggers_fired + 1;
    let reached_limit = next_trigger_count >= order.max_triggers;
    let next_status = if force_terminal || order.kind == "immediate" || reached_limit {
        "completed"
    } else {
        "armed"
    };

    repo.clear_trade_builder_active_exchange_order(order.id, next_status)
        .await?;
    repo.append_trade_builder_order_event(
        order.id,
        "filled",
        &json!({
            "exchange_order_id": exchange_order_id,
            "canonical_entry_qty": canonical_entry_qty,
            "canonical_entry_qty_source": canonical_entry_qty_source,
            "actual_fill_qty": actual_fill_qty,
            "actual_fill_qty_source": actual_fill_qty_source,
            "execution_price": execution_price,
            "triggers_fired": next_trigger_count,
            "max_triggers": order.max_triggers,
            "next_status": next_status
        }),
    )
    .await?;

    if let Some((notification_type, message)) =
        build_trade_builder_fill_notification(order, execution_price, canonical_entry_qty)
    {
        send_trade_builder_notification(repo, order, notification_type, &message).await;
    }

    if should_request_trade_builder_oco_cancel(order, "filled") {
        request_trade_builder_oco_cancel_for_siblings(repo, order, "child_exit_filled").await?;
    }
    if order.side == "sell"
        && order.trigger_condition.as_deref() == Some("cross_below")
        && order.parent_order_id.is_some()
    {
        if let Some(parent_order) = repo
            .get_trade_builder_order(order.parent_order_id.unwrap_or_default())
            .await?
        {
            if let Err(err) =
                maybe_schedule_trade_builder_stop_loss_reentry(repo, &parent_order, order).await
            {
                warn!(
                    builder_order_id = order.id,
                    parent_builder_order_id = parent_order.id,
                    error = %err,
                    "TRADE_BUILDER_REENTRY_SCHEDULE_FAILED"
                );
            }
        }
    }

    let mut stream_union_needs_refresh = false;
    if order.side == "buy" && order.tp_enabled {
        if let Some(tp_price) = order.tp_price {
            let tp_sizing = trade_builder_exit_child_sizing(canonical_entry_qty, execution_price);
            let tp_sell_id = repo
                .create_trade_builder_order(
                    order.trade_id,
                    "conditional",
                    "armed",
                    &order.market_slug,
                    &order.token_id,
                    &order.outcome_label,
                    "sell",
                    "market",
                    Some("cross_above"),
                    Some(tp_price),
                    None,
                    None,
                    None,
                    TRADE_BUILDER_SIZE_BASIS_SHARES,
                    tp_sizing.size_usdc,
                    Some(tp_sizing.target_qty),
                    Some(tp_sizing.remaining_qty),
                    order.min_price_distance_cent,
                    order.expires_at,
                    None,
                    None,
                    1,
                    Some(order.id),
                    false,
                    None,
                    false,
                    None,
                    order.fee_rate_bps,
                    None,
                    None,
                    None,
                    None,
                    false,
                    0,
                    None,
                    order.notify_on_tp_hit,
                    false,
                    false,
                    false,
                    false,
                    false,
                    false,
                    None,
                    false,
                    false,
                    false,
                )
                .await?;
            if let Some(mut child_order) = repo.get_trade_builder_order(tp_sell_id).await? {
                if let Some(snapshot) = ws.get_market_snapshot(&child_order.token_id).await {
                    if let Some(initial_last_seen_price) =
                        trade_builder_last_seen_price_from_market_snapshot(&child_order, &snapshot)
                    {
                        repo.set_trade_builder_last_seen_price(tp_sell_id, initial_last_seen_price)
                            .await?;
                        child_order.last_seen_price = Some(initial_last_seen_price);
                    }
                }
                insert_into_armed_builder_order_cache(child_order).await;
                stream_union_needs_refresh = true;
            }
            repo.append_trade_builder_order_event(
                order.id,
                "tp_sell_created",
                &json!({
                    "child_order_id": tp_sell_id,
                    "initial_status": "armed",
                    "tp_price": tp_price,
                    "tp_execution_mode": "market_ioc",
                    "size_basis": TRADE_BUILDER_SIZE_BASIS_SHARES,
                    "target_qty": tp_sizing.target_qty,
                    "canonical_entry_qty": canonical_entry_qty,
                    "actual_fill_qty": actual_fill_qty,
                    "execution_price": execution_price,
                }),
            )
            .await?;
            info!(
                builder_order_id = order.id,
                tp_sell_order_id = tp_sell_id,
                tp_price,
                "TRADE_BUILDER_TP_SELL_CREATED"
            );
        }
    }
    if order.side == "buy" && order.sl_enabled {
        if let Some(sl_price) = order.sl_price {
            let sl_sizing = trade_builder_exit_child_sizing(canonical_entry_qty, execution_price);
            let sl_sell_id = repo
                .create_trade_builder_order(
                    order.trade_id,
                    "conditional",
                    "armed",
                    &order.market_slug,
                    &order.token_id,
                    &order.outcome_label,
                    "sell",
                    "market",
                    Some("cross_below"),
                    Some(sl_price),
                    None,
                    None,
                    None,
                    TRADE_BUILDER_SIZE_BASIS_SHARES,
                    sl_sizing.size_usdc,
                    Some(sl_sizing.target_qty),
                    Some(sl_sizing.remaining_qty),
                    order.min_price_distance_cent,
                    order.expires_at,
                    None,
                    None,
                    1,
                    Some(order.id),
                    false,
                    None,
                    false,
                    None,
                    order.fee_rate_bps,
                    None,
                    None,
                    None,
                    order.sl_trigger_price_mode.as_deref(),
                    false,
                    0,
                    None,
                    order.notify_on_sl_hit,
                    false,
                    false,
                    false,
                    false,
                    false,
                    false,
                    None,
                    false,
                    false,
                    false,
                )
                .await?;
            if let Some(mut child_order) = repo.get_trade_builder_order(sl_sell_id).await? {
                if let Some(snapshot) = ws.get_market_snapshot(&child_order.token_id).await {
                    if let Some(initial_last_seen_price) =
                        trade_builder_last_seen_price_from_market_snapshot(&child_order, &snapshot)
                    {
                        repo.set_trade_builder_last_seen_price(sl_sell_id, initial_last_seen_price)
                            .await?;
                        child_order.last_seen_price = Some(initial_last_seen_price);
                    }
                }
                insert_into_armed_builder_order_cache(child_order).await;
                stream_union_needs_refresh = true;
            }
            repo.append_trade_builder_order_event(
                order.id,
                "sl_sell_created",
                &json!({
                    "child_order_id": sl_sell_id,
                    "initial_status": "armed",
                    "sl_price": sl_price,
                    "sl_execution_mode": "market_ioc",
                    "size_basis": TRADE_BUILDER_SIZE_BASIS_SHARES,
                    "target_qty": sl_sizing.target_qty,
                    "canonical_entry_qty": canonical_entry_qty,
                    "actual_fill_qty": actual_fill_qty,
                    "execution_price": execution_price,
                    "sl_trigger_price_mode": order.sl_trigger_price_mode,
                }),
            )
            .await?;
            info!(
                builder_order_id = order.id,
                sl_sell_order_id = sl_sell_id,
                sl_price,
                "TRADE_BUILDER_SL_SELL_CREATED"
            );
        }
    }
    if stream_union_needs_refresh {
        if let Err(err) = ensure_fast_path_market_stream_union(ws).await {
            warn!(
                builder_order_id = order.id,
                error = %err,
                "ARMED_ORDER_WS_STREAM_UNION_CHILD_INSERT_FAILED"
            );
        }
    }

    if let Ok(Some(unblocked_id)) = repo
        .unblock_next_trade_builder_order(order.trade_id, &order.token_id)
        .await
    {
        info!(
            builder_order_id = order.id,
            unblocked_order_id = unblocked_id,
            trade_id = order.trade_id,
            "TRADE_BUILDER_DCA_NEXT_LEVEL_UNBLOCKED"
        );
    }

    Ok(())
}
