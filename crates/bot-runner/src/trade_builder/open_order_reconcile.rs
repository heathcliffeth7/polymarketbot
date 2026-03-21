async fn reconcile_trade_builder_open_order(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    order: &TradeBuilderOrder,
    exchange_order_id: &str,
    current_price: f64,
) -> Result<()> {
    let mut order = order.clone();
    if order.status == "canceled_requested" {
        let cancel_reason = order.last_error.as_deref().unwrap_or("user_request");
        client.cancel(exchange_order_id).await?;
        repo.mark_order_status(exchange_order_id, "canceled").await?;
        repo.clear_trade_builder_active_exchange_order(order.id, "canceled").await?;
        repo.append_trade_builder_order_event(
            order.id,
            "cancel_requested",
            &json!({
                "exchange_order_id": exchange_order_id,
                "reason": cancel_reason
            }),
        )
        .await?;
        return Ok(());
    }

    let order_info = client.status(exchange_order_id).await?;
    let normalized = normalize_exchange_status(&order_info.status);
    repo.mark_order_status(exchange_order_id, normalized).await?;

    if normalized == "filled" {
        let (
            canonical_entry_qty,
            canonical_entry_qty_source,
            actual_fill_qty,
            actual_fill_qty_source,
        ) = resolve_trade_builder_finalize_quantities(
            repo,
            client,
            &order,
            exchange_order_id,
            &order_info,
            None,
        )
        .await?;
        let price = trade_builder_child_execution_price(
            &order,
            order_info.price,
            order.working_price,
            Some(current_price),
        )
        .unwrap_or(current_price);
        finalize_builder_fill(
            repo,
            ws,
            &order,
            exchange_order_id,
            canonical_entry_qty,
            canonical_entry_qty_source,
            actual_fill_qty,
            price,
            false,
            actual_fill_qty_source,
        )
        .await?;
        return Ok(());
    }

    if matches!(normalized, "canceled" | "rejected" | "expired") {
        let next_status =
            if order.kind == "conditional" && order.triggers_fired < order.max_triggers {
                "armed"
            } else {
                "completed"
            };
        if next_status == "armed" {
            repo.clear_trade_builder_active_exchange_order_preserve_sizing(order.id, next_status).await?;
        } else {
            repo.clear_trade_builder_active_exchange_order(order.id, next_status).await?;
        }
        repo.append_trade_builder_order_event(
            order.id,
            "terminal_exchange_status",
            &json!({
                "exchange_order_id": exchange_order_id,
                "status": normalized
            }),
        )
        .await?;
        return Ok(());
    }

    let size_basis = normalize_trade_builder_size_basis(&order.size_basis);
    let (mut remaining_usdc, mut remaining_qty) =
        estimate_remaining_trade_builder_sizing(&order, &order_info, current_price);
    if trade_builder_is_take_profit_child(&order) {
        if let Some(remaining_qty) = remaining_qty {
            let resized_sibling_ids = sync_trade_builder_pending_sibling_exit_qty(
                repo,
                &order,
                remaining_qty,
                "tp_partial_fill",
            )
            .await?;
            if !resized_sibling_ids.is_empty() {
                repo.append_trade_builder_order_event(
                    order.id,
                    "sibling_resized_after_partial_fill",
                    &json!({
                        "exchange_order_id": exchange_order_id,
                        "status": normalized,
                        "remaining_qty": remaining_qty,
                        "resized_sibling_order_ids": resized_sibling_ids,
                    }),
                )
                .await?;
            }
        }
    }
    let uncapped_desired_price =
        aggressive_price_for_side(&order.side, current_price, order.min_price_distance_cent);
    let desired_price = trade_builder_cap_exit_sell_price(&order, uncapped_desired_price);
    if (desired_price - uncapped_desired_price).abs() >= 0.000001 {
        repo.append_trade_builder_order_event(
            order.id,
            "exit_price_capped",
            &json!({
                "current_price": current_price,
                "uncapped_desired_price": uncapped_desired_price,
                "capped_desired_price": desired_price,
                "price_floor": trade_builder_exit_sell_price_floor(&order),
                "trigger_price": order.trigger_price,
                "exchange_order_id": exchange_order_id,
            }),
        )
        .await?;
    }
    let fee_rate_bps = resolve_trade_builder_order_fee_rate_bps(repo, client, &mut order).await?;
    let price_distance = min_price_distance_to_probability(order.min_price_distance_cent);
    let should_reprice = order.working_price.map_or(true, |working_price| {
        (working_price - desired_price).abs() >= price_distance
    });

    if !should_reprice {
        repo.set_trade_builder_order_working_state(
            order.id,
            Some(exchange_order_id),
            order.working_price,
            remaining_usdc,
            remaining_qty,
            normalized,
        )
        .await?;
        return Ok(());
    }

    let requested_qty = if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        remaining_qty
    } else {
        None
    };
    let optimistic_exit_submit = size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
        && trade_builder_should_use_optimistic_exit_submit(&order);
    let optimistic_exit_stage =
        optimistic_exit_submit.then(|| trade_builder_current_exit_submit_stage(&order));
    let mut available_qty = None;
    let mut size = if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        remaining_qty.unwrap_or_default()
    } else {
        calc_level_size(remaining_usdc.unwrap_or_default(), desired_price)
    };
    if size <= 0.0 {
        repo.clear_trade_builder_active_exchange_order(order.id, "completed").await?;
        return Ok(());
    }
    if maybe_handle_open_order_trigger_guard_cancel(
        repo,
        ws,
        client,
        &order,
        exchange_order_id,
        &order_info,
        normalized,
        current_price,
        desired_price,
        remaining_usdc,
        remaining_qty,
    )
    .await?
    {
        return Ok(());
    }

    if trade_builder_price_exceeds_max_price(&order, desired_price) {
        let filled_size =
            normalize_trade_builder_terminal_fill_qty_candidate(order_info.filled_size)
                .unwrap_or_default();
        let execution_price = trade_builder_child_execution_price(
            &order,
            order_info.price,
            order.working_price,
            Some(current_price),
        )
        .unwrap_or(current_price);
        match client.cancel(exchange_order_id).await {
            Ok(()) => {
                repo.mark_order_status(exchange_order_id, "canceled").await?;
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
                        &order,
                        exchange_order_id,
                        &order_info,
                        None,
                    )
                    .await?;
                    let terminal_price = trade_builder_child_execution_price(
                        &order,
                        order_info.price,
                        order.working_price,
                        Some(current_price),
                    )
                    .unwrap_or(current_price);
                    repo.mark_order_status(exchange_order_id, "filled").await?;
                    repo.append_trade_builder_order_event(
                        order.id,
                        "max_price_canceled",
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
                            "desired_price": desired_price,
                            "working_price": order.working_price,
                            "submitted_dynamic_price": order.submitted_dynamic_price,
                            "max_price": order.max_price,
                            "cancel_error": error_text
                        }),
                    )
                    .await?;
                    finalize_builder_fill(
                        repo,
                        ws,
                        &order,
                        exchange_order_id,
                        canonical_entry_qty,
                        canonical_entry_qty_source,
                        actual_fill_qty,
                        terminal_price,
                        true,
                        actual_fill_qty_source,
                    )
                    .await?;
                    return Ok(());
                }
                return Err(err).context(format!(
                    "failed to cancel builder order at max price guard: {exchange_order_id}"
                ));
            }
        }
        repo.append_trade_builder_order_event(
            order.id,
            "max_price_canceled",
            &json!({
                "exchange_order_id": exchange_order_id,
                "status_before": normalized,
                "actual_fill_qty": filled_size,
                "execution_price": execution_price,
                "current_price": current_price,
                "desired_price": desired_price,
                "working_price": order.working_price,
                "max_price": order.max_price
            }),
        )
        .await?;
        if filled_size > 0.0 {
            let (canonical_entry_qty, canonical_entry_qty_source) =
                trade_builder_canonical_entry_qty(&order, Some(filled_size)).ok_or_else(|| {
                    anyhow::anyhow!(
                        "builder order canonical fill qty unresolved for exchange_order_id={exchange_order_id}"
                    )
                })?;
            finalize_builder_fill(
                repo,
                ws,
                &order,
                exchange_order_id,
                canonical_entry_qty,
                canonical_entry_qty_source,
                Some(filled_size),
                execution_price,
                true,
                Some(TradeBuilderTerminalFillQtySource::OrderInfoFilledSize.as_str()),
            )
            .await?;
        } else if order.retry_on_max_price_block {
            repo.clear_trade_builder_active_exchange_order(order.id, "canceled").await?;
            let candidate_reason = build_guard_notification_reason("max_price", "above_max_price");
            let notification_message = order.notify_on_max_price_blocked.then(|| {
                build_max_price_waiting_notification_message(
                    &order,
                    current_price,
                    desired_price,
                    "desired_price",
                )
            });
            transition_trade_builder_order_to_guard_waiting(
                repo,
                &order,
                "above_max_price",
                "max_price_waiting",
                &json!({
                    "exchange_order_id": exchange_order_id,
                    "status_before": normalized,
                    "status_after": TRADE_BUILDER_GUARD_BLOCKED_STATUS,
                    "current_price": current_price,
                    "desired_price": desired_price,
                    "working_price": order.working_price,
                    "max_price": order.max_price,
                }),
                remaining_usdc,
                remaining_qty,
                Some(candidate_reason.as_str()),
                order.notify_on_max_price_blocked.then_some("max_price_waiting"),
                notification_message,
            )
            .await?;
        } else {
            repo.clear_trade_builder_active_exchange_order(order.id, "canceled").await?;
            repo.set_trade_builder_order_status(order.id, "canceled", Some("above_max_price")).await?;
            let candidate_reason = build_guard_notification_reason("max_price", "above_max_price");
            if let Some((notification_type, message)) =
                build_max_price_blocked_notification(
                    &order,
                    current_price,
                    desired_price,
                    "desired_price",
                )
            {
                maybe_send_guard_transition_notification(
                    repo,
                    &order,
                    candidate_reason.as_str(),
                    true,
                    notification_type,
                    &message,
                )
                .await?;
            }
        }
        return Ok(());
    }

    if optimistic_exit_submit
        && optimistic_exit_stage == Some(TradeBuilderExitSubmitStage::DynamicGross)
    {
        if let Some(estimated) = trade_builder_estimated_visible_exit_qty(&order, size) {
            if estimated.submit_qty < size {
                let original_qty = size;
                size = estimated.submit_qty;
                repo.append_trade_builder_order_event(
                    order.id,
                    "dynamic_gross_fee_adjusted",
                    &json!({
                        "submit_kind": "reprice",
                        "original_qty": original_qty,
                        "adjusted_qty": estimated.submit_qty,
                        "estimated_fee_qty": estimated.estimated_fee_qty,
                        "execution_price": estimated.execution_price,
                        "fee_rate_bps": estimated.fee_rate_bps,
                        "buffer_qty": trade_builder_exit_qty_buffer(order.target_qty.unwrap_or(original_qty)),
                    }),
                )
                .await?;
            }
        }
    }

    let mut submit_partial_visible_inventory = false;
    if order.side == "sell"
        && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
        && !optimistic_exit_submit
    {
        match client.available_token_qty(&order.token_id).await {
            Ok(quantity) => {
                available_qty = quantity;
            }
            Err(err) => {
                warn!(
                    run_id,
                    builder_order_id = order.id,
                    token_id = %order.token_id,
                    error = %err,
                    "TRADE_BUILDER_EXIT_INVENTORY_CHECK_FAILED"
                );
            }
        }
        let Some(inventory_resolution) =
            resolve_trade_builder_exit_inventory(&order, size, available_qty)
        else {
            mark_trade_builder_inventory_pending(
                repo,
                &order,
                "exit inventory not yet available",
                current_price,
                size,
                available_qty,
            )
            .await?;
            return Ok(());
        };
        if let (Some(visible), Some(local_fallback_qty)) = (
            inventory_resolution.visible_qty,
            inventory_resolution.local_fallback_qty,
        ) {
            if (visible - local_fallback_qty).abs() >= 0.02 {
                repo.append_trade_builder_order_event(
                    order.id,
                    "inventory_source_mismatch",
                    &json!({
                        "visible_qty": visible,
                        "local_fallback_qty": local_fallback_qty,
                        "requested_qty": size
                    }),
                )
                .await?;
            }
        }
        if inventory_resolution.local_fallback_qty.is_some()
            && inventory_resolution.visible_qty.unwrap_or_default() <= 0.0
        {
            repo.append_trade_builder_order_event(
                order.id,
                "local_inventory_fallback_used",
                &json!({
                    "requested_qty": size,
                    "submit_qty": inventory_resolution.submit_qty,
                    "visible_qty": inventory_resolution.visible_qty,
                    "estimated_fee_qty": inventory_resolution.local_fallback_fee_qty,
                    "entry_price": inventory_resolution.local_fallback_entry_price,
                    "fee_rate_bps": inventory_resolution.local_fallback_fee_rate_bps
                }),
            )
            .await?;
        }
        submit_partial_visible_inventory = inventory_resolution.submit_partial_visible_inventory;
        size = inventory_resolution.submit_qty;
    } else if order.side == "sell"
        && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
        && optimistic_exit_stage == Some(TradeBuilderExitSubmitStage::VisibleInventory)
    {
        match client.available_token_qty(&order.token_id).await {
            Ok(quantity) => {
                available_qty = quantity;
            }
            Err(err) => {
                warn!(
                    run_id,
                    builder_order_id = order.id,
                    token_id = %order.token_id,
                    error = %err,
                    "TRADE_BUILDER_EXIT_INVENTORY_CHECK_FAILED"
                );
            }
        }
        let Some(visible_inventory_resolution) =
            resolve_trade_builder_visible_inventory_submit(size, available_qty)
        else {
            schedule_trade_builder_exit_sell_retry(
                repo,
                &order,
                "reprice_retry_scheduled",
                "exit inventory not yet available",
                current_price,
                desired_price,
                requested_qty,
                available_qty,
                Some(size),
                None,
                optimistic_exit_stage,
                optimistic_exit_stage,
            )
            .await?;
            return Ok(());
        };
        submit_partial_visible_inventory =
            visible_inventory_resolution.submit_partial_visible_inventory;
        size = visible_inventory_resolution.submit_qty;
    }

    match client.cancel(exchange_order_id).await {
        Ok(()) => {
            let mut cancel_recorded = false;
            if trade_builder_should_track_buy_inventory_observation(&order) {
                match reconcile_trade_builder_post_cancel_buy_fill(
                    repo,
                    client,
                    ws,
                    &mut order,
                    exchange_order_id,
                    &order_info,
                    normalized,
                    current_price,
                    remaining_usdc,
                    desired_price,
                )
                .await?
                {
                    TradeBuilderPostCancelBuyOutcome::Finalized => return Ok(()),
                    TradeBuilderPostCancelBuyOutcome::RepriceRemainder {
                        remaining_usdc: next_remaining_usdc,
                        remaining_qty: next_remaining_qty,
                        size: next_size,
                    } => {
                        cancel_recorded = true;
                        remaining_usdc = next_remaining_usdc;
                        remaining_qty = next_remaining_qty;
                        size = next_size;
                    }
                    TradeBuilderPostCancelBuyOutcome::NoFill => {}
                }
            }
            if !cancel_recorded {
                repo.mark_order_status(exchange_order_id, "canceled").await?;
            }
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
                    &order,
                    exchange_order_id,
                    &order_info,
                    None,
                )
                .await?;
                let price = trade_builder_child_execution_price(
                    &order,
                    order_info.price,
                    order.working_price,
                    Some(current_price),
                )
                .unwrap_or(current_price);
                repo.mark_order_status(exchange_order_id, "filled").await?;
                repo.append_trade_builder_order_event(
                    order.id,
                    "replace_cancel_terminal_match",
                    &json!({
                        "exchange_order_id": exchange_order_id,
                        "status": order_info.status,
                        "normalized_status": normalized,
                        "canonical_entry_qty": canonical_entry_qty,
                        "canonical_entry_qty_source": canonical_entry_qty_source,
                        "actual_fill_qty": actual_fill_qty,
                        "actual_fill_qty_source": actual_fill_qty_source,
                        "execution_price": price,
                        "cancel_error": error_text
                    }),
                )
                .await?;
                finalize_builder_fill(
                    repo,
                    ws,
                    &order,
                    exchange_order_id,
                    canonical_entry_qty,
                    canonical_entry_qty_source,
                    actual_fill_qty,
                    price,
                    false,
                    actual_fill_qty_source,
                )
                .await?;
                return Ok(());
            }
            return Err(err).context(format!(
                "failed to cancel builder order before reprice: {exchange_order_id}"
            ));
        }
    }

    let market_spec = resolve_trade_builder_market_spec(cfg, &order.market_slug, &order.token_id).await;
    let replace_req = PlaceOrderRequest {
        market: order.market_slug.clone(),
        token_id: Some(order.token_id.clone()),
        side: order.side.clone(),
        price: desired_price,
        size,
        intent: "manual_reprice".to_string(),
        order_type: clob_order_type_for_execution_mode(normalize_trade_builder_execution_mode(
            &order.execution_mode,
        ))
        .to_string(),
        client_order_id: format!("tb-reprice-{}", Uuid::new_v4()),
        leg_side: None,
        fee_rate_bps,
        neg_risk: market_spec.is_some_and(|spec| spec.neg_risk),
    };

    maybe_record_trade_builder_buy_inventory_baseline(
        repo,
        run_id,
        client,
        &order,
        desired_price,
        fee_rate_bps,
    )
    .await;
    if optimistic_exit_submit {
        repo.append_trade_builder_order_event(
            order.id,
            "optimistic_exit_submit_used",
            &json!({
                "submit_kind": "reprice",
                "attempt_stage": optimistic_exit_stage.map(TradeBuilderExitSubmitStage::as_str),
                "status_before": &order.status,
                "requested_qty": requested_qty,
                "attempted_qty": size,
                "current_price": current_price,
                "desired_price": desired_price,
                "size_basis": size_basis,
                "available_qty": available_qty,
                "precheck_skipped": optimistic_exit_stage != Some(TradeBuilderExitSubmitStage::VisibleInventory),
            }),
        )
        .await?;
    }

    let ack = match client.place(&replace_req).await {
        Ok(ack) => ack,
        Err(err) => {
            let error_text = err.to_string();
            if trade_builder_error_is_fatal_exchange_rejection(&error_text) {
                repo.set_trade_builder_order_status(order.id, "error", Some(&error_text)).await?;
                repo.append_trade_builder_order_event(
                    order.id,
                    "fatal_exchange_rejection",
                    &json!({
                        "error": error_text,
                        "context": "reprice_submit",
                        "side": &order.side,
                        "market_slug": &order.market_slug,
                        "token_id": &order.token_id,
                        "neg_risk": replace_req.neg_risk,
                        "order_price_min_tick_size": market_spec.and_then(|spec| spec.order_price_min_tick_size),
                        "order_min_size": market_spec.and_then(|spec| spec.order_min_size),
                    }),
                )
                .await?;
                warn!(
                    builder_order_id = order.id,
                    market = %order.market_slug,
                    error = %error_text,
                    neg_risk = replace_req.neg_risk,
                    "TRADE_BUILDER_FATAL_EXCHANGE_REJECTION_REPRICE"
                );
                maybe_send_trade_builder_system_alert(
                    repo,
                    &order,
                    "fatal_exchange_rejection",
                    &error_text,
                )
                .await;
                return Ok(());
            }
            if order.side == "sell"
                && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
                && trade_builder_error_indicates_balance_or_allowance(&error_text)
            {
                if optimistic_exit_submit {
                    let current_attempt_stage =
                        optimistic_exit_stage.unwrap_or(TradeBuilderExitSubmitStage::DynamicGross);
                    let next_attempt_stage =
                        trade_builder_next_optimistic_exit_stage_after_balance_reject(
                            current_attempt_stage,
                        );
                    repo.append_trade_builder_order_event(
                        order.id,
                        "optimistic_exit_balance_rejected",
                        &json!({
                            "submit_kind": "reprice",
                            "reason": error_text,
                            "attempt_stage": current_attempt_stage.as_str(),
                            "next_attempt_stage": next_attempt_stage.as_str(),
                            "status_before": &order.status,
                            "current_price": current_price,
                            "desired_price": desired_price,
                            "requested_qty": requested_qty,
                            "attempted_qty": size,
                            "available_qty": available_qty,
                        }),
                    )
                    .await?;
                    schedule_trade_builder_exit_sell_retry(
                        repo,
                        &order,
                        "reprice_retry_scheduled",
                        &error_text,
                        current_price,
                        desired_price,
                        requested_qty,
                        available_qty,
                        Some(size),
                        None,
                        Some(current_attempt_stage),
                        Some(next_attempt_stage),
                    )
                    .await?;
                    return Ok(());
                }
                if trade_builder_stop_loss_latched(&order) {
                    schedule_trade_builder_exit_sell_retry(
                        repo,
                        &order,
                        "reprice_retry_scheduled",
                        &error_text,
                        current_price,
                        desired_price,
                        requested_qty,
                        available_qty,
                        Some(size),
                        None,
                        None,
                        None,
                    )
                    .await?;
                    return Ok(());
                }
                let rechecked_qty = match client.available_token_qty(&order.token_id).await {
                    Ok(quantity) => quantity,
                    Err(recheck_err) => {
                        warn!(
                            run_id,
                            builder_order_id = order.id,
                            token_id = %order.token_id,
                            error = %recheck_err,
                            "TRADE_BUILDER_EXIT_INVENTORY_RECHECK_FAILED"
                        );
                        None
                    }
                };
                available_qty = rechecked_qty;
                if rechecked_qty
                    .and_then(|qty| clamp_trade_builder_visible_share_qty(size, Some(qty)))
                    .is_some()
                {
                    mark_trade_builder_inventory_pending(
                        repo,
                        &order,
                        "exchange rejected repriced sell before inventory synced",
                        current_price,
                        size,
                        rechecked_qty,
                    )
                    .await?;
                    return Ok(());
                }
            }
            if trade_builder_should_retry_exit_sell(&order) {
                schedule_trade_builder_exit_sell_retry(
                    repo,
                    &order,
                    "reprice_retry_scheduled",
                    &error_text,
                    current_price,
                    desired_price,
                    requested_qty,
                    available_qty,
                    Some(size),
                    None,
                    None,
                    None,
                )
                .await?;
                return Ok(());
            }
            return Err(err);
        }
    };
    let new_exchange_order_id = ack
        .exchange_order_id
        .clone()
        .unwrap_or_else(|| ack.client_order_id.clone());
    let normalized_status = normalize_exchange_status(&ack.status);
    let raw = json!({
        "prev_exchange_order_id": exchange_order_id,
        "new_exchange_order_id": new_exchange_order_id,
        "status": ack.status,
        "normalized_status": normalized_status,
        "execution_mode": normalize_trade_builder_execution_mode(&order.execution_mode),
        "order_type": clob_order_type_for_execution_mode(normalize_trade_builder_execution_mode(&order.execution_mode)),
        "target_price": desired_price,
        "max_price": order.max_price,
        "remaining_usdc": if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES { Some((size * desired_price).max(0.0)) } else { remaining_usdc },
        "remaining_qty": if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES { Some(size) } else { remaining_qty },
        "size_basis": size_basis,
        "size": size,
        "requested_qty": requested_qty,
        "clamped_qty": if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES { Some(size) } else { None },
        "available_qty": available_qty,
        "fee_rate_bps": fee_rate_bps,
        "partial_visible_inventory_submit": submit_partial_visible_inventory
    });

    repo.upsert_order_by_exchange_id(
        order.trade_id,
        &new_exchange_order_id,
        Some(&ack.client_order_id),
        "manual_reprice",
        &order.side,
        desired_price,
        size,
        normalized_status,
        ack.exchange_ts,
        ack.reject_reason.as_deref(),
        &raw,
    )
    .await?;
    repo.set_trade_builder_order_working_state(
        order.id,
        Some(&new_exchange_order_id),
        Some(desired_price),
        if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
            Some((size * desired_price).max(0.0))
        } else {
            remaining_usdc
        },
        if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
            Some(size)
        } else {
            remaining_qty
        },
        normalized_status,
    )
    .await?;
    maybe_persist_trade_builder_submitted_dynamic(repo, run_id, &mut order, size, desired_price)
        .await;
    if submit_partial_visible_inventory {
        repo.append_trade_builder_order_event(
            order.id,
            "partial_visible_inventory_submit",
            &json!({
                "requested_qty": requested_qty,
                "available_qty": available_qty,
                "submitted_qty": size,
                "residual_qty_ignored": requested_qty.map(|qty| (qty - size).max(0.0)),
            }),
        )
        .await?;
    }
    repo.append_trade_builder_order_event(order.id, "reprice", &raw)
        .await?;
    maybe_record_trade_builder_buy_submit_observation(
        repo,
        run_id,
        &order,
        &new_exchange_order_id,
        size,
        desired_price,
        fee_rate_bps,
        normalized_status,
        raw.clone(),
    )
    .await;
    info!(
        run_id,
        builder_order_id = order.id,
        old_exchange_order_id = exchange_order_id,
        new_exchange_order_id = %new_exchange_order_id,
        "TRADE_BUILDER_ORDER_REPRICED"
    );

    if normalized_status == "filled" {
        let (
            canonical_entry_qty,
            canonical_entry_qty_source,
            actual_fill_qty,
            actual_fill_qty_source,
        ) = if trade_builder_should_track_buy_inventory_observation(&order) {
            let (canonical_entry_qty, canonical_entry_qty_source) =
                    trade_builder_canonical_entry_qty(&order, Some(size)).ok_or_else(|| {
                        anyhow::anyhow!(
                            "builder order canonical fill qty unresolved for exchange_order_id={new_exchange_order_id}"
                        )
                    })?;
            let actual_fill_qty = if order.filled_qty > 0.0 {
                Some(size)
            } else {
                None
            };
            let actual_fill_qty_source = actual_fill_qty.map(|_| "submitted_order_size");
            (
                canonical_entry_qty,
                canonical_entry_qty_source,
                actual_fill_qty,
                actual_fill_qty_source,
            )
        } else {
            (
                size,
                "actual_fill_qty",
                Some(size),
                Some("submitted_order_size"),
            )
        };
        finalize_builder_fill(
            repo,
            ws,
            &order,
            &new_exchange_order_id,
            canonical_entry_qty,
            canonical_entry_qty_source,
            actual_fill_qty,
            desired_price,
            false,
            actual_fill_qty_source,
        )
        .await?;
    }

    Ok(())
}
