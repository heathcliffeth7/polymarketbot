async fn maybe_schedule_trade_builder_stop_loss_reentry(
    repo: &PostgresRepository,
    parent_order: &TradeBuilderOrder,
    stop_loss_order: &TradeBuilderOrder,
) -> Result<()> {
    if !parent_order.reenter_on_sl_hit {
        return Ok(());
    }
    if parent_order.staged_sl_reentry_only_after_all_stages
        && trade_builder_is_staged_stop_loss_child(stop_loss_order)
    {
        let siblings = repo
            .list_trade_builder_child_orders_by_parent(parent_order.id, None)
            .await?;
        if trade_builder_should_defer_reentry_until_all_staged_sl_complete(
            parent_order,
            stop_loss_order,
            &siblings,
        ) {
            let pending_staged_sl_order_ids = siblings
                .iter()
                .filter(|sibling| {
                    sibling.id != stop_loss_order.id
                        && trade_builder_is_staged_stop_loss_child(sibling)
                        && !trade_builder_is_terminal_status(&sibling.status)
                })
                .map(|sibling| sibling.id)
                .collect::<Vec<_>>();
            repo.append_trade_builder_order_event(
                parent_order.id,
                "reentry_deferred_pending_staged_sl",
                &json!({
                    "sl_child_order_id": stop_loss_order.id,
                    "pending_staged_sl_order_ids": pending_staged_sl_order_ids,
                }),
            )
            .await?;
            return Ok(());
        }
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
    let pair_lock_session = if trade_builder_order_uses_pair_lock(parent_order)
        && trade_builder_pair_lock_is_candidate_order(parent_order)
    {
        let Some(pair_session_id) = parent_order.pair_session_id else {
            return Ok(());
        };
        let Some(session) = repo.get_trade_builder_pair_session(pair_session_id).await? else {
            return Ok(());
        };
        if !trade_builder_pair_lock_stop_loss_surface_active_from_session(&session, parent_order.id)
        {
            repo.append_trade_builder_order_event(
                parent_order.id,
                "reentry_skipped",
                &json!({
                    "reason": "pair_lock_stop_loss_inactive",
                    "sl_child_order_id": stop_loss_order.id,
                    "pair_session_id": pair_session_id,
                }),
            )
            .await?;
            return Ok(());
        }
        Some(session)
    } else {
        None
    };
    let action_node_key = pair_lock_session
        .as_ref()
        .and_then(|session| session.flow_node_key.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            parent_order
                .origin_flow_node_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
        });
    let Some(action_node_key) = action_node_key else {
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
    let Some(version) = repo.get_trade_flow_version(flow_run.version_id).await? else {
        repo.append_trade_builder_order_event(
            parent_order.id,
            "reentry_skipped",
            &json!({
                "reason": "flow_version_missing",
                "sl_child_order_id": stop_loss_order.id,
                "flow_run_id": run_id,
                "trigger_node_key": trigger_node_key,
            }),
        )
        .await?;
        return Ok(());
    };
    let graph = parse_trade_flow_graph(&version)?;
    let Some(action_node) = flow_node(&graph, action_node_key) else {
        repo.append_trade_builder_order_event(
            parent_order.id,
            "reentry_skipped",
            &json!({
                "reason": "action_node_missing",
                "sl_child_order_id": stop_loss_order.id,
                "flow_run_id": run_id,
                "action_node_key": action_node_key,
            }),
        )
        .await?;
        return Ok(());
    };
    let reentry_cooldown_sec = node_config_i64(action_node, "reentryCooldownSec")
        .unwrap_or(0)
        .max(0);
    let reentry_skip_current_window =
        node_config_bool(action_node, "reentrySkipCurrentWindow").unwrap_or(false);

    let max_attempts = i64::from(parent_order.reentry_max_attempts.max(0));
    let (attempts_used, stored_reentry_market_slug) =
        resolve_trade_flow_reentry_attempts_for_market(
            &flow_run.context_json,
            action_node_key,
            &parent_order.market_slug,
        );
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
                "current_market_slug": &parent_order.market_slug,
                "stored_reentry_market_slug": stored_reentry_market_slug,
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
        action_node_key,
        FLOW_NODE_STATE_REENTRY_MARKET_SLUG,
        json!(&parent_order.market_slug),
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
    let mut available_at = Utc::now() + ChronoDuration::seconds(reentry_cooldown_sec);
    let mut same_window_blocked_until = None;
    if reentry_skip_current_window {
        if let Some(scope) = find_updown_scope_by_slug(&parent_order.market_slug) {
            if let Some(window_start) = MarketCycleId(parent_order.market_slug.clone()).start_time()
            {
                let next_window_at =
                    window_start + ChronoDuration::seconds(updown_scope_window_seconds(scope) + 1);
                if next_window_at > available_at {
                    available_at = next_window_at;
                }
                same_window_blocked_until = Some(next_window_at);
            }
        }
    }
    let enqueued = repo
        .enqueue_trade_flow_step(
            run_id,
            trigger_node_key,
            "trigger.market_price",
            1,
            None,
            available_at,
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
            "current_market_slug": &parent_order.market_slug,
            "stored_reentry_market_slug": stored_reentry_market_slug,
            "idempotency_key": idempotency_key,
            "step_enqueued": enqueued.is_some(),
            "fast_path_cache_updated": cache_updated,
            "reentry_cooldown_sec": reentry_cooldown_sec,
            "reentry_skip_current_window": reentry_skip_current_window,
            "scheduled_for_at": available_at.to_rfc3339(),
            "same_window_blocked_until": same_window_blocked_until.map(|value| value.to_rfc3339()),
        }),
    )
    .await?;

    Ok(())
}

async fn maybe_record_action_place_order_max_price_relax_fill(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
) -> Result<()> {
    let Some(run_id) = order.origin_flow_run_id else {
        return Ok(());
    };
    let Some(action_node_key) = order
        .origin_flow_node_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };
    let Some(flow_run) = repo.get_trade_flow_run(run_id).await? else {
        return Ok(());
    };
    if flow_run.status != "running" {
        return Ok(());
    }

    let mut context = flow_run.context_json.clone();
    crate::trade_flow::guards::price_to_beat::note_max_price_relax_fill_market(
        &mut context,
        action_node_key,
        &order.market_slug,
    );
    repo.update_trade_flow_run_context(run_id, &context).await?;
    let cache_updated = replace_trade_flow_ws_fast_path_run_context(run_id, &context).await;
    repo.append_trade_builder_order_event(
        order.id,
        "max_price_relax_fill_recorded",
        &json!({
            "flow_run_id": run_id,
            "node_key": action_node_key,
            "market_slug": &order.market_slug,
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
    execution_price: f64,
    family: &str,
    rule_index: Option<usize>,
    rule: &TradeBuilderPriceExitRule,
    child_sizing: TradeBuilderExitChildSizing,
    ladder_metadata: Option<(&str, usize, f64)>,
) -> Result<Option<i64>> {
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
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            false,
            false,
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
        sync_armed_builder_order_to_cache(child_order).await;
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

async fn create_trade_builder_ptb_stop_loss_child_order(
    repo: &PostgresRepository,
    ws: &ClobWsClient,
    parent_order: &TradeBuilderOrder,
    execution_price: f64,
    child_sizing: TradeBuilderExitChildSizing,
    ptb_stop_loss_gap_usd: f64,
    ptb_reference_price: Option<f64>,
    ladder_metadata: Option<(usize, f64)>,
) -> Result<Option<i64>> {
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
    let exit_ladder_kind = ladder_metadata.map(|_| TRADE_BUILDER_EXIT_LADDER_KIND_PTB_SL);
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
            Some("cross_below"),
            None,
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
            Some(ptb_stop_loss_gap_usd),
            ptb_reference_price,
            None,
            parent_order.ptb_stop_loss_time_decay_mode.as_deref(),
            false,
            None,
            None,
            false,
            false,
            None,
            false,
            0,
            None,
            parent_order.notify_on_sl_hit,
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
            exit_ladder_kind,
            ladder_metadata.map(|(index, _)| index as i32),
            ladder_metadata.map(|(_, size_pct)| size_pct),
        )
        .await?;

    if let Ok(Some(mut child_order)) = repo.get_trade_builder_order(child_id).await {
        if let Some(snapshot) = ws.get_market_snapshot(&parent_order.token_id).await {
            if let Some(initial_last_seen_price) =
                trade_builder_last_seen_price_from_market_snapshot(&child_order, &snapshot)
            {
                repo.set_trade_builder_last_seen_price(child_id, initial_last_seen_price)
                    .await?;
                child_order.last_seen_price = Some(initial_last_seen_price);
            }
        }
        sync_armed_builder_order_to_cache(child_order).await;
    }

    repo.append_trade_builder_order_event(
        parent_order.id,
        "sl_sell_created",
        &json!({
            "child_order_id": child_id,
            "initial_status": "armed",
            "family": exit_ladder_kind.unwrap_or(TRADE_BUILDER_EXIT_LADDER_KIND_SL),
            "exit_mode": exit_mode,
            "sibling_policy": sibling_policy,
            "trigger_price": Value::Null,
            "size_basis": TRADE_BUILDER_SIZE_BASIS_SHARES,
            "size_pct": ladder_metadata.map(|(_, size_pct)| size_pct).unwrap_or(100.0),
            "target_qty": child_sizing.target_qty,
            "execution_price": execution_price,
            "ptb_stop_loss_gap_usd": ptb_stop_loss_gap_usd,
            "ptb_reference_price": ptb_reference_price,
        }),
    )
    .await?;

    Ok(Some(child_id))
}

async fn create_trade_builder_staged_price_exit_children(
    repo: &PostgresRepository,
    ws: &ClobWsClient,
    parent_order: &TradeBuilderOrder,
    canonical_entry_qty: f64,
    execution_price: f64,
    family: &str,
    rules: &[TradeBuilderPriceExitRule],
    order_min_size: Option<f64>,
) -> Result<Vec<i64>> {
    let plan = trade_builder_ladder_rule_target_plan(rules, canonical_entry_qty, order_min_size);
    if plan.targets.is_empty() {
        repo.append_trade_builder_order_event(
            parent_order.id,
            "no_executable_exit_child_due_to_min_size",
            &json!({
                "family": family,
                "canonical_entry_qty": canonical_entry_qty,
                "order_min_size": order_min_size,
                "rule_count": rules.len(),
            }),
        )
        .await?;
        return Ok(Vec::new());
    }

    for skipped_rule_index in &plan.skipped_keys {
        let requested_target_qty = plan
            .requested_targets
            .iter()
            .find_map(|(rule_index, qty)| (*rule_index == *skipped_rule_index).then_some(*qty));
        let rule = &rules[*skipped_rule_index];
        repo.append_trade_builder_order_event(
            parent_order.id,
            "staged_child_skipped_due_to_min_size",
            &json!({
                "family": family,
                "rule_index": skipped_rule_index,
                "trigger_price": trade_builder_child_rule_price(rule),
                "size_pct": rule.size_pct,
                "requested_target_qty": requested_target_qty,
                "canonical_entry_qty": canonical_entry_qty,
                "order_min_size": order_min_size,
            }),
        )
        .await?;
    }

    let consolidation_target = plan.consolidation_target;
    let mut created_order_ids = Vec::new();
    for (rule_index, target_qty) in &plan.targets {
        let rule = &rules[*rule_index];
        let child_sizing = TradeBuilderExitChildSizing {
            size_usdc: 0.0,
            target_qty: *target_qty,
            remaining_qty: *target_qty,
        };
        if let Some(child_id) = create_trade_builder_price_exit_child_order(
            repo,
            ws,
            parent_order,
            execution_price,
            family,
            Some(*rule_index),
            rule,
            child_sizing,
            Some((family, *rule_index, rule.size_pct)),
        )
        .await?
        {
            if consolidation_target == Some(*rule_index) {
                repo.append_trade_builder_order_event(
                    parent_order.id,
                    "staged_family_consolidated_due_to_min_size",
                    &json!({
                        "family": family,
                        "rule_index": rule_index,
                        "trigger_price": trade_builder_child_rule_price(rule),
                        "size_pct": rule.size_pct,
                        "target_qty": target_qty,
                        "canonical_entry_qty": canonical_entry_qty,
                        "order_min_size": order_min_size,
                        "skipped_rule_indices": &plan.skipped_keys,
                        "child_order_id": child_id,
                    }),
                )
                .await?;
            }
            created_order_ids.push(child_id);
        }
    }

    Ok(created_order_ids)
}

async fn finalize_builder_fill(
    repo: &PostgresRepository,
    cfg: &AppConfig,
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
    if order.side == "buy"
        && (order.tp_enabled
            || order.sl_enabled
            || order.ptb_stop_loss_gap_usd.is_some()
            || !order.ptb_stop_loss_rules_json.is_empty())
    {
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
        if let Err(err) = maybe_record_action_place_order_max_price_relax_fill(repo, order).await {
            warn!(
                builder_order_id = order.id,
                error = %err,
                "TRADE_BUILDER_MAX_PRICE_RELAX_FILL_RECORD_FAILED"
            );
        }
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
        maybe_handle_trade_builder_pair_lock_buy_fill(
            repo,
            order,
            canonical_entry_qty,
            execution_price,
        )
        .await?;
    }
    let stop_loss_surface_active = if order.side == "buy" {
        trade_builder_pair_lock_stop_loss_surface_active(repo, order).await?
    } else {
        false
    };

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

    let flow_created_payload = repo
        .load_trade_builder_order_flow_created_payload(order.id)
        .await?;
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
        let applied_fill_qty = trade_builder_is_child_exit_sell(order)
            .then_some(persisted_fill_qty.or(Some(canonical_entry_qty)))
            .flatten();
        if trade_builder_is_child_exit_sell(order) {
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
            if let Err(err) = trade_builder_sync_parent_exit_children(
                repo,
                cfg,
                parent_order,
                "child_exit_filled",
            )
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
                maybe_record_action_place_order_ptb_stop_loss_bump(repo, parent_order, order).await
            {
                warn!(
                    builder_order_id = order.id,
                    parent_builder_order_id = parent_order.id,
                    error = %err,
                    "TRADE_BUILDER_PTB_STOP_LOSS_BUMP_FAILED"
                );
            }
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
            if let Err(err) = maybe_finalize_trade_builder_pair_lock_after_lead_stop_loss_fill(
                repo,
                parent_order,
                order,
            )
            .await
            {
                warn!(
                    builder_order_id = order.id,
                    parent_builder_order_id = parent_order.id,
                    error = %err,
                    "TRADE_BUILDER_PAIR_LOCK_LEAD_STOP_LOSS_FINALIZE_FAILED"
                );
            }
        }
    }

    if trade_builder_pair_lock_is_unwind_order(order) {
        maybe_finalize_trade_builder_pair_lock_after_unwind_fill(repo, order).await?;
    }

    let mut stream_union_needs_refresh = false;
    if order.side == "buy" {
        let hard_tp_rule = trade_builder_hard_tp_rule(order);
        let hard_sl_rule = trade_builder_hard_sl_rule(order);
        let tp_rules = trade_builder_normalized_tp_rules(order);
        let sl_rules = trade_builder_normalized_sl_rules(order);
        let time_exit_rules = trade_builder_normalized_time_exit_rules(order);
        let order_min_size = resolve_trade_builder_order_min_size(cfg, order).await;

        if let Some(rule) = hard_tp_rule.as_ref() {
            if let Some(child_sizing) =
                trade_builder_ladder_child_qty(canonical_entry_qty, rule.size_pct)
            {
                if create_trade_builder_price_exit_child_order(
                    repo,
                    ws,
                    order,
                    execution_price,
                    TRADE_BUILDER_EXIT_LADDER_KIND_TP,
                    None,
                    rule,
                    child_sizing,
                    None,
                )
                .await?
                .is_some()
                {
                    stream_union_needs_refresh = true;
                }
            }
        }

        if !tp_rules.is_empty() {
            let created_tp_children = create_trade_builder_staged_price_exit_children(
                repo,
                ws,
                order,
                canonical_entry_qty,
                execution_price,
                TRADE_BUILDER_EXIT_LADDER_KIND_TP,
                &tp_rules,
                order_min_size,
            )
            .await?;
            if !created_tp_children.is_empty() {
                stream_union_needs_refresh = true;
            }
        }

        if stop_loss_surface_active {
            if let Some(rule) = hard_sl_rule.as_ref() {
                if let Some(child_sizing) =
                    trade_builder_ladder_child_qty(canonical_entry_qty, rule.size_pct)
                {
                    if create_trade_builder_price_exit_child_order(
                        repo,
                        ws,
                        order,
                        execution_price,
                        TRADE_BUILDER_EXIT_LADDER_KIND_SL,
                        None,
                        rule,
                        child_sizing,
                        None,
                    )
                    .await?
                    .is_some()
                    {
                        stream_union_needs_refresh = true;
                    }
                }
            }
        }

        if stop_loss_surface_active {
            if let Some(ptb_stop_loss_gap_usd) = order.ptb_stop_loss_gap_usd {
                if let Some(child_sizing) =
                    trade_builder_ladder_child_qty(canonical_entry_qty, 100.0)
                {
                    let _ = create_trade_builder_ptb_stop_loss_child_order(
                        repo,
                        ws,
                        order,
                        execution_price,
                        child_sizing,
                        ptb_stop_loss_gap_usd,
                        order
                            .ptb_reference_price
                            .or_else(|| trade_builder_cached_ptb_reference_price(&order.market_slug)),
                        None,
                    )
                    .await?;
                }
            }
        }

        if stop_loss_surface_active && !order.ptb_stop_loss_rules_json.is_empty() {
            let target_plan = trade_builder_ptb_stop_loss_target_plan(
                &order.ptb_stop_loss_rules_json,
                canonical_entry_qty,
                order_min_size,
            );
            for (rule_index, target_qty) in &target_plan.targets {
                let rule = &order.ptb_stop_loss_rules_json[*rule_index];
                let child_sizing = TradeBuilderExitChildSizing {
                    size_usdc: 0.0,
                    target_qty: *target_qty,
                    remaining_qty: *target_qty,
                };
                let _ = create_trade_builder_ptb_stop_loss_child_order(
                    repo,
                    ws,
                    order,
                    execution_price,
                    child_sizing,
                    rule.gap_usd,
                    order
                        .ptb_reference_price
                        .or_else(|| trade_builder_cached_ptb_reference_price(&order.market_slug)),
                    Some((*rule_index, rule.size_pct)),
                )
                .await?;
            }
        }

        if stop_loss_surface_active && !sl_rules.is_empty() {
            let created_sl_children = create_trade_builder_staged_price_exit_children(
                repo,
                ws,
                order,
                canonical_entry_qty,
                execution_price,
                TRADE_BUILDER_EXIT_LADDER_KIND_SL,
                &sl_rules,
                order_min_size,
            )
            .await?;
            if !created_sl_children.is_empty() {
                stream_union_needs_refresh = true;
            }
        }

        if let Err(err) =
            maybe_schedule_trade_builder_time_exit_rules(repo, order, &time_exit_rules).await
        {
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

    let snapshot_root_order_id = parent_order
        .as_ref()
        .map(|parent| parent.id)
        .unwrap_or(order.id);
    let snapshot_mark_price = order.last_seen_price.or(Some(execution_price));
    if let Err(err) = refresh_trade_builder_auto_scope_analysis_snapshot_for_root(
        repo,
        snapshot_root_order_id,
        snapshot_mark_price,
    )
    .await
    {
        warn!(
            builder_order_id = order.id,
            root_builder_order_id = snapshot_root_order_id,
            error = %err,
            "AUTO_SCOPE_ANALYSIS_REFRESH_FAILED"
        );
    }

    Ok(())
}
