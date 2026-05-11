#[allow(clippy::too_many_arguments)]
async fn process_trade_flow_dual_dca_job(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    gamma: &GammaHttpClient,
    job: &TradeFlowDualDcaJob,
) -> Result<()> {
    // --- Phase 0: Pre-checks ---
    let run = repo.get_trade_flow_run(job.flow_run_id).await?;
    let Some(run) = run else {
        cancel_dual_dca_active_orders(repo, client, job, None, "flow_run_missing").await?;
        repo.set_trade_flow_dual_dca_job_status(job.id, "canceled", Some("flow_run_missing"))
            .await?;
        repo.append_trade_flow_dual_dca_event(
            job.id,
            None,
            "job_canceled",
            &json!({ "reason": "flow_run_missing" }),
        )
        .await?;
        return Ok(());
    };
    if run.status != "running" {
        cancel_dual_dca_active_orders(repo, client, job, None, "flow_run_not_running").await?;
        repo.set_trade_flow_dual_dca_job_status(job.id, "completed", Some("flow_run_not_running"))
            .await?;
        repo.append_trade_flow_dual_dca_event(
            job.id,
            None,
            "job_completed",
            &json!({ "reason": "flow_run_not_running", "run_status": run.status }),
        )
        .await?;
        return Ok(());
    }
    if job.consecutive_errors >= FLOW_DUAL_DCA_MAX_CONSECUTIVE_ERRORS {
        cancel_dual_dca_active_orders(repo, client, job, None, "max_consecutive_errors").await?;
        repo.set_trade_flow_dual_dca_job_status(
            job.id,
            "paused",
            Some("max_consecutive_errors_reached"),
        )
        .await?;
        repo.append_trade_flow_dual_dca_event(
            job.id,
            None,
            "job_paused",
            &json!({
                "reason": "max_consecutive_errors_reached",
                "consecutive_errors": job.consecutive_errors,
                "last_error": job.last_error
            }),
        )
        .await?;
        warn!(
            run_id,
            dual_dca_job_id = job.id,
            consecutive_errors = job.consecutive_errors,
            "TRADE_FLOW_DUAL_DCA_JOB_PAUSED_MAX_ERRORS"
        );
        return Ok(());
    }

    // --- Phase 1: Market discovery ---
    let scope_def = find_updown_scope_by_asset_timeframe(&job.market_asset, &job.market_timeframe)
        .ok_or_else(|| anyhow::anyhow!("dual_dca job has unsupported market asset/timeframe"))?;
    let mut markets = match list_markets_for_scope(gamma, scope_def.scope).await {
        Ok(m) => m,
        Err(err) => {
            let next_check = Utc::now() + ChronoDuration::seconds(FLOW_DUAL_DCA_RETRY_SECONDS);
            repo.schedule_trade_flow_dual_dca_job_check(job.id, next_check, None)
                .await?;
            repo.append_trade_flow_dual_dca_event(
                job.id,
                None,
                "not_buy_decision",
                &json!({
                    "reason_code": "market_discovery_fetch_failed",
                    "error": err.to_string(),
                    "next_check_at": next_check
                }),
            )
            .await?;
            return Ok(());
        }
    };
    markets.retain(|m| m.yes_token_id.is_some() && m.no_token_id.is_some());
    if markets.is_empty() {
        let next_check = Utc::now() + ChronoDuration::seconds(FLOW_DUAL_DCA_RETRY_SECONDS);
        repo.update_trade_flow_dual_dca_job_market_state(job.id, None, None, None, next_check, 0)
            .await?;
        repo.append_trade_flow_dual_dca_event(
            job.id,
            None,
            "not_buy_decision",
            &json!({ "reason_code": "market_missing_token_ids", "next_check_at": next_check }),
        )
        .await?;
        return Ok(());
    }

    let now = Utc::now();
    let selected = select_preferred_live_market(markets, now)
        .ok_or_else(|| anyhow::anyhow!("no active market for dual_dca"))?;
    let market_slug = selected.slug.clone();
    let market_started_at = selected.starts_at;
    let market_ends_at = selected.ends_at;
    let market_selection_reason = selected.selection_reason.as_str();
    let maker_base_fee = selected.maker_base_fee;

    info!(
        run_id,
        dual_dca_job_id = job.id,
        market = %market_slug,
        selection_reason = market_selection_reason,
        "TRADE_FLOW_DUAL_DCA_MARKET_SELECTED"
    );

    let source_trade_id = job
        .source_trade_id
        .ok_or_else(|| anyhow::anyhow!("dual_dca job missing source_trade_id"))?;
    let yes_token_id = selected
        .yes_token_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("selected market missing yes token id"))?;
    let no_token_id = selected
        .no_token_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("selected market missing no token id"))?;
    let (yes_price, no_price) =
        resolve_dual_dca_outcome_prices(ws, client, &market_slug, &yes_token_id, &no_token_id)
            .await?;
    let active_next_check = dual_dca_active_next_check(now, market_ends_at);
    let rollover_next_check =
        dual_dca_rollover_next_check(now, market_ends_at, &job.market_timeframe);

    // --- Phase 2: TP/SL check ---
    if let Some((unrealized_pnl_usdc, token_breakdown)) = evaluate_dual_dca_unrealized_pnl_usdc(
        repo,
        source_trade_id,
        &yes_token_id,
        &no_token_id,
        yes_price,
        no_price,
    )
    .await?
    {
        let tp_hit = job.tp_profit_pct > 0.0 && unrealized_pnl_usdc >= job.tp_profit_pct;
        let sl_hit = job.sl_loss_pct > 0.0 && unrealized_pnl_usdc <= -job.sl_loss_pct;
        if tp_hit || sl_hit {
            let reason = if tp_hit {
                "tp_profit_hit"
            } else {
                "sl_loss_hit"
            };
            cancel_dual_dca_active_orders(repo, client, job, Some(&market_slug), reason).await?;
            repo.update_trade_flow_dual_dca_job_market_state(
                job.id,
                Some(&market_slug),
                market_started_at,
                market_ends_at,
                rollover_next_check,
                0,
            )
            .await?;
            repo.append_trade_flow_dual_dca_event(
                job.id,
                None,
                "risk_guard_triggered",
                &json!({
                    "market_slug": market_slug,
                    "reason": reason,
                    "unrealized_pnl_usdc": unrealized_pnl_usdc,
                    "token_breakdown": token_breakdown,
                    "next_check_at": rollover_next_check
                }),
            )
            .await?;
            return Ok(());
        }
    }

    // --- Phase 3: Cutoff check ---
    if let Some(ends_at) = market_ends_at {
        let cutoff_at = ends_at - ChronoDuration::minutes(job.cutoff_min.max(0) as i64);
        if now >= cutoff_at {
            cancel_dual_dca_active_orders(repo, client, job, Some(&market_slug), "cutoff_window")
                .await?;
            repo.update_trade_flow_dual_dca_job_market_state(
                job.id,
                Some(&market_slug),
                market_started_at,
                Some(ends_at),
                rollover_next_check,
                0,
            )
            .await?;
            repo.append_trade_flow_dual_dca_event(
                job.id,
                None,
                "market_cutoff_window",
                &json!({
                    "market_slug": market_slug,
                    "cutoff_min": job.cutoff_min,
                    "market_ends_at": ends_at,
                    "next_check_at": rollover_next_check
                }),
            )
            .await?;
            return Ok(());
        }
    }

    // --- Phase 4: Initialize legs if new market ---
    let is_new_market = job
        .last_market_slug
        .as_ref()
        .map(|prev| prev != &market_slug)
        .unwrap_or(true);

    let outcomes = resolve_dual_dca_outcomes(
        &job.side_mode,
        &yes_token_id,
        yes_price,
        &no_token_id,
        no_price,
    );

    if is_new_market {
        cancel_dual_dca_active_orders(repo, client, job, None, "market_change").await?;
        let mut created_count = 0i32;

        for (outcome_label, token_id, outcome_price) in &outcomes {
            let base_reference_price = if let Some(bp) = job.base_price_usdc {
                bp
            } else {
                *outcome_price
            };
            let level_reference_price = clamp_probability(base_reference_price);

            for level in 0..job.dca_levels {
                let level_f64 = level as f64;
                let step_distance =
                    dual_dca_level_step_distance(job.near_step, job.step_mult, level);
                let trigger_price = if level == 0 && job.base_price_usdc.is_none() {
                    None
                } else {
                    Some(clamp_probability(level_reference_price - step_distance))
                };
                let trigger_condition: Option<&str> = trigger_price.map(|_| "cross_below");

                let size_multiplier = job.size_mult.powf(level_f64);
                let (size_usdc, _planned_shares) = if job.base_sizing == "usdc" {
                    let base_usdc = job.base_usdc.unwrap_or(0.0);
                    (base_usdc * size_multiplier, None::<f64>)
                } else {
                    let base_shares = job.base_shares.unwrap_or(0.0);
                    let shares = base_shares * size_multiplier;
                    let price_for_notional = trigger_price.unwrap_or(level_reference_price);
                    (shares * price_for_notional, Some(shares))
                };
                if !size_usdc.is_finite() || size_usdc < FLOW_DUAL_DCA_MIN_ORDER_USDC {
                    continue;
                }

                // Risk gate
                let risk = risk_gate_manual_order(
                    repo,
                    run_id,
                    cfg,
                    Some(run.user_id),
                    source_trade_id,
                    size_usdc,
                    limits,
                    policy,
                )
                .await?;
                if !matches!(risk, RiskDecision::Allow) {
                    repo.append_trade_flow_dual_dca_event(
                        job.id,
                        None,
                        "level_blocked_by_risk",
                        &json!({
                            "outcome_label": outcome_label,
                            "level_index": level,
                            "size_usdc": size_usdc,
                            "decision": format!("{risk:?}")
                        }),
                    )
                    .await?;
                    continue;
                }

                // Create leg as pending (no builder order)
                let leg = repo
                    .upsert_trade_flow_dual_dca_leg(
                        job.id,
                        &market_slug,
                        token_id,
                        outcome_label,
                        "buy",
                        level,
                        trigger_condition,
                        trigger_price,
                        size_usdc,
                        Some(level_reference_price),
                        None,
                        "pending",
                    )
                    .await?;

                repo.append_trade_flow_dual_dca_event(
                    job.id,
                    Some(leg.id),
                    "leg_created",
                    &json!({
                        "market_slug": market_slug,
                        "outcome_label": outcome_label,
                        "level_index": level,
                        "trigger_price": trigger_price,
                        "size_usdc": size_usdc,
                        "reference_price": level_reference_price
                    }),
                )
                .await?;
                created_count += 1;
            }
        }

        repo.update_trade_flow_dual_dca_job_market_state(
            job.id,
            Some(&market_slug),
            market_started_at,
            market_ends_at,
            active_next_check,
            created_count,
        )
        .await?;
        repo.append_trade_flow_dual_dca_event(
            job.id,
            None,
            "market_cycle_initialized",
            &json!({
                "market_slug": market_slug,
                "orders_created": created_count,
                "next_check_at": active_next_check
            }),
        )
        .await?;
    }

    // --- Phase 5: Check fills on active orders ---
    let active_legs = repo
        .list_dual_dca_legs_with_active_orders(job.id, &market_slug)
        .await?;
    for leg in &active_legs {
        if let Some(exchange_oid) = leg.active_exchange_order_id.as_deref() {
            if let Ok(order_info) = client.status(exchange_oid).await {
                let status = normalize_exchange_status(&order_info.status);
                match status {
                    "filled" => {
                        let fill_price = order_info.price.unwrap_or(0.0);
                        let fill_size = order_info
                            .filled_size
                            .unwrap_or(order_info.size.unwrap_or(0.0));
                        repo.set_dual_dca_leg_filled(leg.id, fill_price, fill_size)
                            .await?;
                        repo.append_trade_flow_dual_dca_event(
                            job.id,
                            Some(leg.id),
                            "leg_filled",
                            &json!({
                                "market_slug": market_slug,
                                "outcome_label": leg.outcome_label,
                                "level_index": leg.level_index,
                                "exchange_order_id": exchange_oid,
                                "filled_price": fill_price,
                                "filled_size": fill_size
                            }),
                        )
                        .await?;
                        info!(
                            run_id,
                            dual_dca_job_id = job.id,
                            leg_id = leg.id,
                            outcome = %leg.outcome_label,
                            level = leg.level_index,
                            filled_price = fill_price,
                            filled_size = fill_size,
                            "TRADE_FLOW_DUAL_DCA_LEG_FILLED"
                        );
                    }
                    "canceled" | "expired" | "rejected" => {
                        repo.reset_dual_dca_leg_to_pending(leg.id).await?;
                        repo.append_trade_flow_dual_dca_event(
                            job.id,
                            Some(leg.id),
                            "leg_order_canceled_retry",
                            &json!({
                                "exchange_order_id": exchange_oid,
                                "exchange_status": status,
                                "action": "reset_to_pending"
                            }),
                        )
                        .await?;
                    }
                    _ => {} // still open, wait
                }
            }
        }
    }

    // Sync fills from exchange for PnL accuracy
    if let Err(err) = sync_recent_trade_builder_fills(repo, client).await {
        warn!(run_id, error = %err, "DUAL_DCA_FILL_SYNC_ERROR");
    }

    // --- Phase 6: Check triggers and place orders ---
    for (outcome_label, token_id, current_price) in &outcomes {
        let Some(leg) = repo
            .next_pending_dual_dca_leg(job.id, &market_slug, outcome_label)
            .await?
        else {
            continue; // all levels filled or no pending legs
        };

        // Level 0 (no trigger): fire immediately. Level 1+: check trigger.
        let should_fire = dual_dca_trigger_crossed_below_strict(*current_price, leg.trigger_price);
        if !should_fire {
            continue;
        }

        // Risk gate
        let risk = risk_gate_manual_order(
            repo,
            run_id,
            cfg,
            Some(run.user_id),
            source_trade_id,
            leg.size_usdc,
            limits,
            policy,
        )
        .await?;
        if !matches!(risk, RiskDecision::Allow) {
            info!(
                run_id,
                dual_dca_job_id = job.id,
                outcome = %outcome_label,
                level = leg.level_index,
                "TRADE_FLOW_DUAL_DCA_LEVEL_RISK_BLOCKED"
            );
            continue;
        }

        // Compute order price and size
        let desired_price =
            aggressive_price_for_side("buy", *current_price, job.min_price_distance_cent);
        let size = calc_level_size(leg.size_usdc, desired_price);
        if size <= 0.0 {
            continue;
        }

        let client_order_id = format!("dca-{}", Uuid::new_v4());
        let fee_rate = if maker_base_fee > 0 {
            maker_base_fee
        } else {
            1000
        };
        let req = PlaceOrderRequest {
            market: market_slug.clone(),
            token_id: Some(token_id.clone()),
            side: "buy".to_string(),
            price: desired_price,
            size,
            intent: "dca_direct".to_string(),
            order_type: "GTC".to_string(),
            client_order_id: client_order_id.clone(),
            leg_side: None,
            fee_rate_bps: fee_rate,
            neg_risk: false,
        };

        let ack = client.place(&req).await?;
        let exchange_order_id = ack
            .exchange_order_id
            .clone()
            .unwrap_or_else(|| ack.client_order_id.clone());
        let normalized_status = normalize_exchange_status(&ack.status);

        // Record in orders table for PnL tracking
        let raw = json!({
            "dca_job_id": job.id,
            "leg_id": leg.id,
            "client_order_id": ack.client_order_id,
            "exchange_order_id": exchange_order_id,
            "status": ack.status,
            "normalized_status": normalized_status,
            "trigger_price": leg.trigger_price,
            "current_price": current_price,
            "execution_price": desired_price,
            "size": size,
            "reject_reason": ack.reject_reason,
            "exchange_ts": ack.exchange_ts
        });
        repo.upsert_order_by_exchange_id(
            source_trade_id,
            &exchange_order_id,
            Some(&client_order_id),
            "dca_direct",
            "buy",
            desired_price,
            size,
            normalized_status,
            ack.exchange_ts,
            ack.reject_reason.as_deref(),
            &raw,
        )
        .await?;

        // Update leg status
        if normalized_status == "filled" {
            repo.set_dual_dca_leg_submitted(leg.id, &exchange_order_id, &client_order_id)
                .await?;
            repo.set_dual_dca_leg_filled(leg.id, desired_price, size)
                .await?;
            repo.append_trade_flow_dual_dca_event(
                job.id,
                Some(leg.id),
                "leg_filled_immediately",
                &json!({
                    "market_slug": market_slug,
                    "outcome_label": outcome_label,
                    "level_index": leg.level_index,
                    "exchange_order_id": exchange_order_id,
                    "filled_price": desired_price,
                    "filled_size": size
                }),
            )
            .await?;
            info!(
                run_id,
                dual_dca_job_id = job.id,
                leg_id = leg.id,
                outcome = %outcome_label,
                level = leg.level_index,
                price = desired_price,
                size,
                "TRADE_FLOW_DUAL_DCA_DIRECT_ORDER_FILLED_IMMEDIATELY"
            );
        } else if normalized_status == "rejected" {
            repo.append_trade_flow_dual_dca_event(
                job.id,
                Some(leg.id),
                "leg_order_rejected",
                &json!({
                    "exchange_order_id": exchange_order_id,
                    "reject_reason": ack.reject_reason,
                    "status": ack.status
                }),
            )
            .await?;
            warn!(
                run_id,
                dual_dca_job_id = job.id,
                leg_id = leg.id,
                outcome = %outcome_label,
                level = leg.level_index,
                reject_reason = ?ack.reject_reason,
                "TRADE_FLOW_DUAL_DCA_DIRECT_ORDER_REJECTED"
            );
        } else {
            // Order is open on the book
            repo.set_dual_dca_leg_submitted(leg.id, &exchange_order_id, &client_order_id)
                .await?;
            repo.append_trade_flow_dual_dca_event(
                job.id,
                Some(leg.id),
                "leg_order_placed",
                &json!({
                    "market_slug": market_slug,
                    "outcome_label": outcome_label,
                    "level_index": leg.level_index,
                    "exchange_order_id": exchange_order_id,
                    "desired_price": desired_price,
                    "size": size,
                    "current_price": current_price,
                    "trigger_price": leg.trigger_price
                }),
            )
            .await?;
            info!(
                run_id,
                dual_dca_job_id = job.id,
                leg_id = leg.id,
                outcome = %outcome_label,
                level = leg.level_index,
                price = desired_price,
                size,
                exchange_order_id = %exchange_order_id,
                "TRADE_FLOW_DUAL_DCA_DIRECT_ORDER_PLACED"
            );
        }
    }

    // --- Phase 7: Schedule next check ---
    repo.update_trade_flow_dual_dca_job_market_state(
        job.id,
        Some(&market_slug),
        market_started_at,
        market_ends_at,
        active_next_check,
        0,
    )
    .await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Cancel active CLOB orders for DCA legs
// ---------------------------------------------------------------------------
