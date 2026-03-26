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

async fn maybe_schedule_trade_builder_time_exit_rules(
    repo: &PostgresRepository,
    parent_order: &TradeBuilderOrder,
    rules: &[TradeBuilderTimeExitRule],
) -> Result<()> {
    if rules.is_empty() {
        return Ok(());
    }

    let Some(run_id) = parent_order.origin_flow_run_id else {
        repo.append_trade_builder_order_event(
            parent_order.id,
            "time_exit_schedule_skipped",
            &json!({
                "reason": "missing_origin_flow_run",
                "rules": rules,
            }),
        )
        .await?;
        return Ok(());
    };
    let Some(node_key) = parent_order
        .origin_flow_node_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        repo.append_trade_builder_order_event(
            parent_order.id,
            "time_exit_schedule_skipped",
            &json!({
                "reason": "missing_origin_flow_node_key",
                "flow_run_id": run_id,
                "rules": rules,
            }),
        )
        .await?;
        return Ok(());
    };

    for (rule_index, rule) in rules.iter().enumerate() {
        let available_at = Utc::now() + ChronoDuration::minutes(rule.elapsed_minutes as i64);
        let idempotency_key = format!("time_exit:{run_id}:{}:{rule_index}", parent_order.id);
        let input = json!({
            "internalMode": "time_exit",
            "parentBuilderOrderId": parent_order.id,
            "sourceTradeId": parent_order.trade_id,
            "marketSlug": parent_order.market_slug,
            "tokenId": parent_order.token_id,
            "outcomeLabel": parent_order.outcome_label,
            "remainingPct": rule.remaining_pct,
            "elapsedMinutes": rule.elapsed_minutes,
            "ruleIndex": rule_index,
        });
        let enqueued = repo
            .enqueue_trade_flow_step(
                run_id,
                node_key,
                "action.place_order",
                1,
                Some(&input),
                available_at,
                None,
                Some(&idempotency_key),
            )
            .await?;
        schedule_enqueued_flow_process_notify(enqueued, available_at);
        repo.append_trade_builder_order_event(
            parent_order.id,
            if enqueued.is_some() {
                "time_exit_scheduled"
            } else {
                "time_exit_duplicate_ignored"
            },
            &json!({
                "flow_run_id": run_id,
                "node_key": node_key,
                "rule_index": rule_index,
                "elapsed_minutes": rule.elapsed_minutes,
                "remaining_pct": rule.remaining_pct,
                "available_at": available_at.to_rfc3339(),
                "idempotency_key": idempotency_key,
                "step_enqueued": enqueued.is_some(),
            }),
        )
        .await?;
    }

    Ok(())
}

async fn create_trade_builder_price_exit_child_order(
    repo: &PostgresRepository,
    ws: &ClobWsClient,
    parent_order: &TradeBuilderOrder,
    canonical_entry_qty: f64,
    execution_price: f64,
    family: &str,
    rule_index: Option<usize>,
    rule: &TradeBuilderPriceExitRule,
    ladder_metadata: Option<(&str, usize, f64)>,
) -> Result<Option<i64>> {
    let Some(child_sizing) = trade_builder_ladder_child_qty(canonical_entry_qty, rule.size_pct) else {
        return Ok(None);
    };
    let child_size_usdc = (child_sizing.target_qty * execution_price).max(0.0);
    let exit_mode = if ladder_metadata.is_some() {
        TRADE_BUILDER_EXIT_MODE_STAGED
    } else {
        TRADE_BUILDER_EXIT_MODE_HARD
    };
    let sibling_policy = if ladder_metadata.is_some() {
        TRADE_BUILDER_EXIT_SIBLING_POLICY_RESIZE_REMAINING
    } else {
        TRADE_BUILDER_EXIT_SIBLING_POLICY_CANCEL_ALL
    };
    let (trigger_condition, notify_on_hit) = if family == TRADE_BUILDER_EXIT_LADDER_KIND_TP {
        (Some("cross_above"), parent_order.notify_on_tp_hit)
    } else {
        (Some("cross_below"), parent_order.notify_on_sl_hit)
    };
    let child_id = repo
        .create_trade_builder_order_with_exit_ladders(
            parent_order.trade_id,
            "conditional",
            "armed",
            &parent_order.market_slug,
            &parent_order.token_id,
            &parent_order.outcome_label,
            "sell",
            "market",
            trigger_condition,
            Some(trade_builder_child_rule_price(rule)),
            None,
            None,
            None,
            TRADE_BUILDER_SIZE_BASIS_SHARES,
            child_size_usdc,
            Some(child_sizing.target_qty),
            Some(child_sizing.remaining_qty),
            parent_order.min_price_distance_cent,
            parent_order.expires_at,
            None,
            None,
            1,
            Some(parent_order.id),
            false,
            None,
            None,
            false,
            None,
            None,
            None,
            parent_order.fee_rate_bps,
            None,
            None,
            None,
            if family == TRADE_BUILDER_EXIT_LADDER_KIND_SL {
                parent_order.sl_trigger_price_mode.as_deref()
            } else {
                None
            },
            false,
            0,
            None,
            notify_on_hit,
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
            ladder_metadata.map(|(ladder_family, _, _)| ladder_family),
            ladder_metadata.map(|(_, index, _)| index as i32),
            ladder_metadata.map(|(_, _, size_pct)| size_pct),
        )
        .await?;

    if let Some(mut child_order) = repo.get_trade_builder_order(child_id).await? {
        if let Some(snapshot) = ws.get_market_snapshot(&child_order.token_id).await {
            if let Some(initial_last_seen_price) =
                trade_builder_last_seen_price_from_market_snapshot(&child_order, &snapshot)
            {
                repo.set_trade_builder_last_seen_price(child_id, initial_last_seen_price)
                    .await?;
                child_order.last_seen_price = Some(initial_last_seen_price);
            }
        }
        insert_into_armed_builder_order_cache(child_order).await;
    }

    repo.append_trade_builder_order_event(
        parent_order.id,
        if family == TRADE_BUILDER_EXIT_LADDER_KIND_TP {
            "tp_sell_created"
        } else {
            "sl_sell_created"
        },
        &json!({
            "child_order_id": child_id,
            "initial_status": "armed",
            "family": family,
            "exit_mode": exit_mode,
            "sibling_policy": sibling_policy,
            "rule_index": rule_index,
            "trigger_price": trade_builder_child_rule_price(rule),
            "size_basis": TRADE_BUILDER_SIZE_BASIS_SHARES,
            "size_pct": rule.size_pct,
            "target_qty": child_sizing.target_qty,
            "canonical_entry_qty": canonical_entry_qty,
            "execution_price": execution_price,
            "sl_trigger_price_mode": if family == TRADE_BUILDER_EXIT_LADDER_KIND_SL {
                parent_order.sl_trigger_price_mode.as_deref()
            } else {
                None
            },
        }),
    )
    .await?;

    Ok(Some(child_id))
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
        let _ = ensure_trade_builder_parent_position_from_parent_fill(
            repo,
            order,
            canonical_entry_qty,
            execution_price,
            actual_fill_qty_source,
        )
        .await?;
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

    let flow_created_payload = repo.load_trade_builder_order_flow_created_payload(order.id).await?;
    let is_window_end_auto_sell = flow_created_payload
        .as_ref()
        .and_then(|payload| payload.get("internal_mode"))
        .and_then(Value::as_str)
        == Some("window_end_auto_sell");
    if should_request_trade_builder_oco_cancel(order, "filled") {
        let canceled_sibling_ids =
            request_trade_builder_oco_cancel_for_siblings(repo, order, "child_exit_filled").await?;
        if is_window_end_auto_sell && !canceled_sibling_ids.is_empty() {
            if let Some(parent_order_id) = order.parent_order_id {
                repo.append_trade_builder_order_event(
                    parent_order_id,
                    "window_end_auto_sell_child_exits_canceled",
                    &json!({
                        "window_end_builder_order_id": order.id,
                        "sibling_order_ids": canceled_sibling_ids,
                    }),
                )
                .await?;
            }
        }
    }
    let parent_order = if order.parent_order_id.is_some() {
        repo.get_trade_builder_order(order.parent_order_id.unwrap_or_default())
            .await?
    } else {
        None
    };
    if let Some(parent_order) = parent_order.as_ref() {
        if trade_builder_is_child_exit_sell(order) {
            let applied_fill_qty = persisted_fill_qty.or(Some(canonical_entry_qty));
            let _ = apply_trade_builder_parent_position_child_fill(
                repo,
                parent_order,
                applied_fill_qty,
                execution_price,
                actual_fill_qty_source,
            )
            .await?;
        }
        if trade_builder_order_has_exit_ladders(parent_order)
            && !should_request_trade_builder_oco_cancel(order, "filled")
        {
            if let Err(err) =
                trade_builder_sync_parent_exit_children(repo, parent_order, "child_exit_filled")
                    .await
            {
                warn!(
                    builder_order_id = order.id,
                    parent_builder_order_id = parent_order.id,
                    error = %err,
                    "TRADE_BUILDER_LADDER_SYNC_FAILED"
                );
            }
        }
        if order.side == "sell" && order.trigger_condition.as_deref() == Some("cross_below") {
            if let Err(err) =
                maybe_schedule_trade_builder_stop_loss_reentry(repo, parent_order, order).await
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
    if order.side == "buy" {
        let hard_tp_rule = trade_builder_hard_tp_rule(order);
        let hard_sl_rule = trade_builder_hard_sl_rule(order);
        let tp_rules = trade_builder_normalized_tp_rules(order);
        let sl_rules = trade_builder_normalized_sl_rules(order);
        let time_exit_rules = trade_builder_normalized_time_exit_rules(order);

        if let Some(rule) = hard_tp_rule.as_ref() {
            if create_trade_builder_price_exit_child_order(
                repo,
                ws,
                order,
                canonical_entry_qty,
                execution_price,
                TRADE_BUILDER_EXIT_LADDER_KIND_TP,
                None,
                rule,
                None,
            )
            .await?
            .is_some()
            {
                stream_union_needs_refresh = true;
            }
        }

        for (rule_index, rule) in tp_rules.iter().enumerate() {
            if create_trade_builder_price_exit_child_order(
                repo,
                ws,
                order,
                canonical_entry_qty,
                execution_price,
                TRADE_BUILDER_EXIT_LADDER_KIND_TP,
                Some(rule_index),
                rule,
                Some((
                    TRADE_BUILDER_EXIT_LADDER_KIND_TP,
                    rule_index,
                    rule.size_pct,
                )),
            )
            .await?
            .is_some()
            {
                stream_union_needs_refresh = true;
            }
        }

        if let Some(rule) = hard_sl_rule.as_ref() {
            if create_trade_builder_price_exit_child_order(
                repo,
                ws,
                order,
                canonical_entry_qty,
                execution_price,
                TRADE_BUILDER_EXIT_LADDER_KIND_SL,
                None,
                rule,
                None,
            )
            .await?
            .is_some()
            {
                stream_union_needs_refresh = true;
            }
        }

        for (rule_index, rule) in sl_rules.iter().enumerate() {
            if create_trade_builder_price_exit_child_order(
                repo,
                ws,
                order,
                canonical_entry_qty,
                execution_price,
                TRADE_BUILDER_EXIT_LADDER_KIND_SL,
                Some(rule_index),
                rule,
                Some((
                    TRADE_BUILDER_EXIT_LADDER_KIND_SL,
                    rule_index,
                    rule.size_pct,
                )),
            )
            .await?
            .is_some()
            {
                stream_union_needs_refresh = true;
            }
        }

        if let Err(err) = maybe_schedule_trade_builder_time_exit_rules(repo, order, &time_exit_rules).await {
            warn!(
                builder_order_id = order.id,
                error = %err,
                "TRADE_BUILDER_TIME_EXIT_SCHEDULE_FAILED"
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
