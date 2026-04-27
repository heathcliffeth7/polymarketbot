#[allow(clippy::too_many_arguments)]
async fn execute_action_place_order(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: Option<&dyn OrderExecutor>,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let internal_mode = step_input_string(step, &["internalMode", "internal_mode"])
        .map(|value| value.trim().to_ascii_lowercase());
    let is_internal_time_exit = internal_mode.as_deref() == Some("time_exit");
    let is_window_end_auto_sell = step
        .input_json
        .as_ref()
        .and_then(|value| value.get("windowEndAutoSell"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let effective_internal_mode = internal_mode
        .clone()
        .or_else(|| is_window_end_auto_sell.then_some("window_end_auto_sell".to_string()));
    let is_special_internal_sell = is_internal_time_exit || is_window_end_auto_sell;
    let side = if is_special_internal_sell {
        "sell".to_string()
    } else {
        node_config_string(node, "side")
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("action.place_order requires side (buy or sell)"))?
    };
    anyhow::ensure!(
        matches!(side.as_str(), "buy" | "sell"),
        "action.place_order side must be buy or sell"
    );
    let execution_mode = if is_special_internal_sell {
        "market".to_string()
    } else {
        node_config_string(node, "executionMode")
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("action.place_order requires executionMode (market or limit)")
            })?
    };
    anyhow::ensure!(
        matches!(execution_mode.as_str(), "market" | "limit"),
        "action.place_order executionMode must be market or limit"
    );
    let market_slug = resolve_action_place_order_string(
        node,
        context,
        step,
        "marketSlug",
        "marketSlug",
        &["market_slug", "marketSlug", "wsMarketSlug"],
    )
    .ok_or_else(|| anyhow::anyhow!("action.place_order requires marketSlug"))?;
    let token_id = resolve_action_place_order_string(
        node,
        context,
        step,
        "tokenId",
        "tokenId",
        &["triggered_token_id", "tokenId"],
    )
    .ok_or_else(|| anyhow::anyhow!("action.place_order requires tokenId"))?;
    let outcome_label = resolve_action_place_order_string(
        node,
        context,
        step,
        "outcomeLabel",
        "outcomeLabel",
        &["triggered_outcome_label", "outcomeLabel"],
    )
    .unwrap_or_else(|| token_id.clone());
    if let Some(stale_execution) = maybe_skip_stale_action_place_order_step(
        repo,
        run,
        node,
        graph,
        step,
        context,
        &side,
        &execution_mode,
        Some(token_id.as_str()),
        Some(outcome_label.as_str()),
        None,
    )
    .await?
    {
        return Ok(stale_execution);
    }
    set_flow_context(context, "marketSlug", json!(market_slug.clone()));
    set_flow_context(context, "tokenId", json!(token_id.clone()));
    set_flow_context(context, "outcomeLabel", json!(outcome_label.clone()));
    let mut protection_output = Value::Null;
    if side == "buy" {
        if let Some(raw_protection) =
            resolve_action_place_order_underlying_protection(context, step)
        {
            if let Some(protection_config) =
                parse_underlying_protection_config(Some(raw_protection.clone()))
            {
                let poly_delta_10s_cent = raw_protection
                    .get("poly_delta_10s_cent")
                    .and_then(value_as_f64);
                let resolved_market_asset = flow_context_string(context, "marketAsset")
                    .or_else(|| {
                        step_input_string(step, &["market_asset", "marketAsset"])
                            .map(|value| value.trim().to_ascii_lowercase())
                    })
                    .or_else(|| {
                        find_updown_scope_by_slug(&market_slug).map(|scope| scope.asset.to_string())
                    });
                let resolved_direction =
                    resolve_underlying_direction_label(&outcome_label).map(str::to_string);
                let protection = if let Some(ref market_asset) = resolved_market_asset {
                    if market_asset != &protection_config.asset {
                        UnderlyingProtectionEvaluation {
                            mode: protection_config.mode.clone(),
                            preset: protection_config.preset.clone(),
                            asset: protection_config.asset.clone(),
                            direction: protection_config.direction.clone(),
                            reference_feed: "coinbase_spot".to_string(),
                            reference_symbol: protection_config.reference_symbol.clone(),
                            passed: false,
                            reason_code: "asset_mismatch".to_string(),
                            reason_detail: Some(format!(
                                "expected_asset={} current_asset={market_asset}",
                                protection_config.asset
                            )),
                            cycle_open_price: None,
                            current_price: None,
                            delta_10s_pct: None,
                            delta_30s_pct: None,
                            poly_delta_10s_cent,
                            divergence_blocked: false,
                        }
                    } else if let Some(ref current_direction) = resolved_direction {
                        if current_direction != &protection_config.direction {
                            UnderlyingProtectionEvaluation {
                                mode: protection_config.mode.clone(),
                                preset: protection_config.preset.clone(),
                                asset: protection_config.asset.clone(),
                                direction: protection_config.direction.clone(),
                                reference_feed: "coinbase_spot".to_string(),
                                reference_symbol: protection_config.reference_symbol.clone(),
                                passed: false,
                                reason_code: "direction_mismatch".to_string(),
                                reason_detail: Some(format!(
                                    "expected_direction={} current_direction={current_direction}",
                                    protection_config.direction
                                )),
                                cycle_open_price: None,
                                current_price: None,
                                delta_10s_pct: None,
                                delta_30s_pct: None,
                                poly_delta_10s_cent,
                                divergence_blocked: false,
                            }
                        } else {
                            evaluate_underlying_protection(
                                &protection_config,
                                &market_slug,
                                poly_delta_10s_cent,
                            )
                            .await
                        }
                    } else {
                        evaluate_underlying_protection(
                            &protection_config,
                            &market_slug,
                            poly_delta_10s_cent,
                        )
                        .await
                    }
                } else if let Some(ref current_direction) = resolved_direction {
                    if current_direction != &protection_config.direction {
                        UnderlyingProtectionEvaluation {
                            mode: protection_config.mode.clone(),
                            preset: protection_config.preset.clone(),
                            asset: protection_config.asset.clone(),
                            direction: protection_config.direction.clone(),
                            reference_feed: "coinbase_spot".to_string(),
                            reference_symbol: protection_config.reference_symbol.clone(),
                            passed: false,
                            reason_code: "direction_mismatch".to_string(),
                            reason_detail: Some(format!(
                                "expected_direction={} current_direction={current_direction}",
                                protection_config.direction
                            )),
                            cycle_open_price: None,
                            current_price: None,
                            delta_10s_pct: None,
                            delta_30s_pct: None,
                            poly_delta_10s_cent,
                            divergence_blocked: false,
                        }
                    } else {
                        evaluate_underlying_protection(
                            &protection_config,
                            &market_slug,
                            poly_delta_10s_cent,
                        )
                        .await
                    }
                } else {
                    evaluate_underlying_protection(
                        &protection_config,
                        &market_slug,
                        poly_delta_10s_cent,
                    )
                    .await
                };
                protection_output = protection.to_value();
                set_flow_context(context, "underlyingProtection", protection_output.clone());
                if !protection.passed {
                    repo.append_trade_flow_event(
                        Some(run.id),
                        run.definition_id,
                        Some(run.version_id),
                        "pre_order_protection_blocked",
                        &json!({
                            "node_key": node.key,
                            "node_type": node.node_type,
                            "market_slug": market_slug,
                            "token_id": token_id,
                            "outcome_label": outcome_label,
                            "side": side,
                            "execution_mode": execution_mode,
                            "protection": protection_output.clone()
                        }),
                    )
                    .await?;
                    return Ok(TradeFlowNodeExecution {
                        output: json!({
                            "node_key": node.key,
                            "blocked": true,
                            "reason": "underlying_protection_blocked",
                            "market_slug": market_slug,
                            "token_id": token_id,
                            "outcome_label": outcome_label,
                            "side": side,
                            "execution_mode": execution_mode,
                            "protection": protection_output
                        }),
                        routes: vec![TradeFlowRouteDecision {
                            edge_type: "on_error".to_string(),
                            available_at: Utc::now(),
                        }],
                        repeat_at: None,
                        repeat_idempotency_key: None,
                    });
                }
            }
        }
    }
    let price_to_beat_signal_market = Some(trade_flow::guards::price_to_beat::PriceToBeatSignalFormulaMarketInput {
            best_bid: step_input_f64(step, &["wsBestBid", "ws_best_bid"]),
            best_ask: step_input_f64(step, &["wsBestAsk", "ws_best_ask"]),
    });
    if let Some(blocked_execution) =
        trade_flow::guards::maybe_block_action_place_order_price_to_beat_guard(
            repo,
            cfg,
            client,
            run,
            node,
            context,
            &market_slug,
            &token_id,
            &outcome_label,
            &side,
            &execution_mode,
            price_to_beat_signal_market,
        )
        .await?
    {
        return Ok(blocked_execution);
    }
    let mut source_trade_id = resolve_flow_source_trade_id(node, context).or_else(|| {
        step_input_i64(step, &["sourceTradeId", "source_trade_id"]).filter(|value| *value > 0)
    });
    if let Some(resolved_source_trade_id) = source_trade_id {
        set_flow_context(context, "sourceTradeId", json!(resolved_source_trade_id));
    }
    let size_mode = if is_special_internal_sell {
        Some("pct".to_string())
    } else {
        node_config_string(node, "sizeMode")
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
    };
    if let Some(mode) = size_mode.as_deref() {
        anyhow::ensure!(
            matches!(mode, "usdc" | "pct" | "shares"),
            "action.place_order sizeMode must be usdc, pct, or shares"
        );
    }
    let max_triggers = node_config_i64(node, "maxTriggers")
        .unwrap_or(1)
        .clamp(1, 20) as i32;
    let trigger_sizes = if let Some(raw_values) = node.config.get("triggerSizes") {
        let values = raw_values
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("action.place_order triggerSizes must be an array"))?;
        let mut out = Vec::with_capacity(values.len());
        for value in values {
            let parsed = value_as_f64(value).ok_or_else(|| {
                anyhow::anyhow!("action.place_order triggerSizes entries must be numeric")
            })?;
            anyhow::ensure!(
                parsed > 0.0 && parsed.is_finite(),
                "action.place_order triggerSizes entries must be > 0"
            );
            out.push(parsed);
        }
        anyhow::ensure!(
            out.len() <= max_triggers as usize,
            "action.place_order triggerSizes length cannot exceed maxTriggers"
        );
        out
    } else {
        Vec::new()
    };
    let trigger_size_for_first_fire = trigger_sizes.first().copied();
    let selected_entry_timing_profile = (!is_special_internal_sell)
        .then(|| resolve_action_place_order_selected_entry_timing_profile_value(step, context))
        .flatten();
    let selected_entry_size_usdc = (!is_internal_time_exit)
        .then(|| resolve_action_place_order_selected_entry_size_usdc(step, context)).flatten();
    let configured_size_usdc = if is_internal_time_exit {
        None
    } else {
        node_config_f64(node, "sizeUsdc")
            .or_else(|| node_config_f64(node, "targetNotionalUsdc"))
            .or(selected_entry_size_usdc)
    };
    let configured_target_qty = (!is_internal_time_exit)
        .then(|| action_place_order_target_qty(node))
        .flatten();
    let configured_size_pct = if is_internal_time_exit {
        step_input_f64(step, &["remainingPct", "remaining_pct"])
    } else if is_window_end_auto_sell {
        Some(100.0)
    } else {
        node_config_f64(node, "sizePct").or_else(|| node_config_f64(node, "sizePercent"))
    };
    let use_share_size = matches!(size_mode.as_deref(), Some("shares"));
    let use_pct_size = if trigger_size_for_first_fire.is_some() {
        if let Some(mode) = size_mode.as_deref() {
            mode == "pct"
        } else {
            configured_size_usdc.is_none() && configured_size_pct.is_some()
        }
    } else {
        is_special_internal_sell
            || matches!(size_mode.as_deref(), Some("pct"))
            || (configured_size_usdc.is_none() && configured_size_pct.is_some())
    };
    if !trigger_sizes.is_empty() && use_pct_size {
        let trigger_pct_total: f64 = trigger_sizes.iter().sum();
        anyhow::ensure!(
            trigger_pct_total <= 100.000001,
            "action.place_order pct triggerSizes total must be <= 100"
        );
    }
    let reference_price = resolve_action_place_order_reference_price(node, step);
    if source_trade_id.is_none() {
        anyhow::ensure!(
            side == "buy",
            "action.place_order side=sell requires sourceTradeId or an explicit open-position context"
        );
        anyhow::ensure!(
            !use_pct_size,
            "action.place_order sizePct requires sourceTradeId when sizeMode is pct"
        );
        let seed_size_usdc = resolve_action_place_order_source_trade_seed_size_usdc(
            trigger_size_for_first_fire,
            configured_size_usdc,
            configured_target_qty,
            use_share_size,
            reference_price,
        )?;
        let ensured_source_trade_id = repo
            .ensure_manual_builder_source_trade(
                run.user_id,
                &market_slug,
                &token_id,
                &outcome_label,
                reference_price.unwrap_or(0.5),
                seed_size_usdc,
            )
            .await?;
        info!(
            flow_run_id = run.id,
            node_key = %node.key,
            source_trade_id = ensured_source_trade_id,
            side = %side,
            market_slug = %market_slug,
            token_id = %token_id,
            "TRADE_FLOW_PLACE_ORDER_SOURCE_TRADE_AUTO_RESOLVED"
        );
        set_flow_context(context, "sourceTradeId", json!(ensured_source_trade_id));
        source_trade_id = Some(ensured_source_trade_id);
    }
    let source_trade_id = source_trade_id
        .ok_or_else(|| anyhow::anyhow!("action.place_order requires sourceTradeId"))?;
    if is_internal_time_exit {
        if load_action_place_order_sell_position(repo, source_trade_id, &token_id)
            .await
            .is_err()
        {
            if let Some(parent_builder_order_id) =
                step_input_i64(step, &["parentBuilderOrderId", "parent_builder_order_id"])
            {
                repo.append_trade_builder_order_event(
                    parent_builder_order_id,
                    "time_exit_skipped_closed",
                    &json!({
                        "node_key": node.key,
                        "source_trade_id": source_trade_id,
                        "market_slug": market_slug,
                        "token_id": token_id,
                        "outcome_label": outcome_label,
                    }),
                )
                .await?;
            }
            let skipped_output = json!({
                "node_key": node.key,
                "skipped": true,
                "reason": "time_exit_skipped_closed",
                "internal_mode": "time_exit",
                "source_trade_id": source_trade_id,
                "market_slug": market_slug,
                "token_id": token_id,
                "outcome_label": outcome_label,
            });
            return Ok(TradeFlowNodeExecution {
                output: skipped_output,
                routes: Vec::new(),
                repeat_at: None,
                repeat_idempotency_key: None,
            });
        }
    }
    let window_end_parent_builder_order_id = if is_window_end_auto_sell {
        Some(
            step_input_i64(step, &["parentBuilderOrderId", "parent_builder_order_id"])
                .filter(|value| *value > 0)
                .ok_or_else(|| {
                    anyhow::anyhow!("windowEndAutoSell requires parentBuilderOrderId")
                })?,
        )
    } else {
        None
    };
    let ref_key = node_config_string(node, "refKey").unwrap_or_else(|| node.key.clone());
    let trigger_condition = if is_special_internal_sell {
        None
    } else {
        node_config_string(node, "triggerCondition")
    };
    let trigger_price = if is_special_internal_sell {
        None
    } else {
        node_config_f64(node, "triggerPrice")
            .or_else(|| node_config_f64(node, "triggerPriceCent").map(|v| v / 100.0))
    };
    if let Some(condition) = trigger_condition.as_deref() {
        anyhow::ensure!(
            matches!(
                condition,
                "cross_above" | "cross_below" | "level_above" | "level_below"
            ),
            "action.place_order triggerCondition must be cross_above/cross_below/level_above/level_below"
        );
    }
    let mut kind = if is_special_internal_sell {
        "immediate".to_string()
    } else {
        node_config_string(node, "kind").unwrap_or_else(|| {
            if trigger_condition.is_some() && trigger_price.is_some() {
                "conditional".to_string()
            } else {
                "immediate".to_string()
            }
        })
    };
    if kind != "conditional" && kind != "immediate" {
        kind = "immediate".to_string();
    }
    if let Some(blocked_execution) = maybe_block_action_place_order_buy_fill_lock(
        repo, run, node, context, &side, &market_slug, &token_id, &outcome_label,
        &execution_mode, source_trade_id,
    ).await?
    {
        return Ok(blocked_execution);
    }
    let expires_at = node_config_datetime(node, "expiresAt")?;
    let tp_rules = if is_special_internal_sell {
        Vec::new()
    } else {
        parse_trade_builder_price_exit_rules(
            node.config.get("tpRules"),
            TRADE_BUILDER_EXIT_LADDER_KIND_TP,
        )?
    };
    let sl_rules = if is_special_internal_sell {
        Vec::new()
    } else {
        parse_trade_builder_price_exit_rules(
            node.config.get("slRules"),
            TRADE_BUILDER_EXIT_LADDER_KIND_SL,
        )?
    };
    let ptb_stop_loss = if is_special_internal_sell {
        None
    } else {
        resolve_action_place_order_ptb_stop_loss_config(node, &side, &market_slug)?
    };
    let ptb_stop_loss_rules = ptb_stop_loss
        .as_ref()
        .map(|config| config.staged_rules.clone())
        .unwrap_or_default();
    let time_exit_rules = if is_special_internal_sell {
        Vec::new()
    } else {
        parse_trade_builder_time_exit_rules(node.config.get("timeExitRules"))?
    };
    let mut ignored_existing_order: Option<(
        Option<i64>,
        &'static str,
        Option<ActionPlaceOrderExistingRefScope>,
    )> = None;
    let existing_order_ref = if is_special_internal_sell {
        None
    } else {
        resolve_action_place_order_existing_order_ref(node, context)
    };
    let existing_order_id = existing_order_ref.map(|(existing_order_id, _)| existing_order_id);
    let existing_order_ref_scope = existing_order_ref.map(|(_, scope)| scope);
    let mut existing_order = if let Some(existing_order_id) = existing_order_id {
        match repo.get_trade_builder_order(existing_order_id).await? {
            Some(order) => Some(order),
            None => {
                ignored_existing_order = Some((
                    Some(existing_order_id),
                    "missing_existing_order",
                    existing_order_ref_scope,
                ));
                clear_action_place_order_ref_bindings(context, node, &ref_key);
                repo.append_trade_flow_event(
                    Some(run.id),
                    run.definition_id,
                    Some(run.version_id),
                    "place_order_existing_ref_ignored",
                    &json!({
                        "node_key": node.key,
                        "node_type": node.node_type,
                        "ref_key": ref_key,
                        "reason": "missing_existing_order",
                        "existing_order_id": existing_order_id,
                        "existing_ref_scope": existing_order_ref_scope.map(|scope| scope.as_str()),
                        "expected_market_slug": market_slug,
                        "expected_token_id": token_id,
                        "expected_source_trade_id": source_trade_id,
                        "expected_side": side,
                        "expected_kind": kind,
                        "expected_execution_mode": execution_mode
                    }),
                )
                .await?;
                None
            }
        }
    } else {
        None
    };
    if let Some(existing_order_snapshot) = existing_order.clone() {
        match classify_action_place_order_existing_order(
            &existing_order_snapshot,
            &side,
            source_trade_id,
            &market_slug,
            &token_id,
            &kind,
            &execution_mode,
        ) {
            ActionPlaceOrderExistingOrderDecision::ReuseActive => {
                bind_action_place_order_ref_bindings(
                    context,
                    node,
                    &ref_key,
                    existing_order_snapshot.id,
                );
                return Ok(TradeFlowNodeExecution {
                    output: json!({
                        "node_key": node.key,
                        "builder_order_id": existing_order_snapshot.id,
                        "ref_key": ref_key,
                        "source_trade_id": existing_order_snapshot.trade_id,
                        "kind": &existing_order_snapshot.kind,
                        "side": &existing_order_snapshot.side,
                        "status": &existing_order_snapshot.status,
                        "execution_mode": &existing_order_snapshot.execution_mode,
                        "market_slug": &existing_order_snapshot.market_slug,
                        "token_id": &existing_order_snapshot.token_id,
                        "size_basis": &existing_order_snapshot.size_basis,
                        "size_usdc": existing_order_snapshot.size_usdc,
                        "target_qty": existing_order_snapshot.target_qty,
                        "remaining_qty": existing_order_snapshot.remaining_qty,
                        "existing_ref_scope": existing_order_ref_scope.map(|scope| scope.as_str()),
                        "should_inline_submit": false,
                        "reused_existing_order": true
                    }),
                    routes: Vec::new(),
                    repeat_at: None,
                    repeat_idempotency_key: None,
                });
            }
            ActionPlaceOrderExistingOrderDecision::RearmErrorSell => {
                bind_action_place_order_ref_bindings(
                    context,
                    node,
                    &ref_key,
                    existing_order_snapshot.id,
                );
            }
            ActionPlaceOrderExistingOrderDecision::Ignore(reason) => {
                ignored_existing_order = Some((
                    Some(existing_order_snapshot.id),
                    reason,
                    existing_order_ref_scope,
                ));
                clear_action_place_order_ref_bindings(context, node, &ref_key);
                repo.append_trade_flow_event(
                    Some(run.id),
                    run.definition_id,
                    Some(run.version_id),
                    "place_order_existing_ref_ignored",
                    &json!({
                        "node_key": node.key,
                        "node_type": node.node_type,
                        "ref_key": ref_key,
                        "reason": reason,
                        "existing_order_id": existing_order_snapshot.id,
                        "existing_ref_scope": existing_order_ref_scope.map(|scope| scope.as_str()),
                        "existing_status": existing_order_snapshot.status,
                        "existing_market_slug": existing_order_snapshot.market_slug,
                        "existing_token_id": existing_order_snapshot.token_id,
                        "existing_source_trade_id": existing_order_snapshot.trade_id,
                        "existing_side": existing_order_snapshot.side,
                        "existing_kind": existing_order_snapshot.kind,
                        "existing_execution_mode": existing_order_snapshot.execution_mode,
                        "expected_market_slug": market_slug,
                        "expected_token_id": token_id,
                        "expected_source_trade_id": source_trade_id,
                        "expected_side": side,
                        "expected_kind": kind,
                        "expected_execution_mode": execution_mode
                    }),
                )
                .await?;
                existing_order = None;
            }
        }
    }
    let min_price_distance_cent = node_config_f64(node, "minPriceDistanceCent").unwrap_or(1.0);
    anyhow::ensure!(
        min_price_distance_cent > 0.0,
        "action.place_order minPriceDistanceCent must be > 0"
    );
    let should_inline_submit = kind == "immediate";
    let base_max_price = resolve_action_place_order_max_price(node, step, context);
    let trigger_price_guard_enabled =
        node_config_bool(node, "triggerPriceGuardEnabled").unwrap_or(false);
    let base_guard_trigger_price = if trigger_price_guard_enabled && side == "buy" {
        Some(resolve_action_place_order_guard_trigger_price(step).ok_or_else(|| {
            anyhow::anyhow!(
                "action.place_order triggerPriceGuardEnabled=true but no trigger_price found in step inputs"
            )
        })?)
    } else {
        None
    };
    let reentry_guard_resolution = resolve_action_place_order_reentry_guard_resolution(
        node,
        context,
        base_guard_trigger_price,
        base_max_price,
    )?;
    let max_price = reentry_guard_resolution.effective_max_price;
    let guard_trigger_price = reentry_guard_resolution.effective_guard_trigger_price;
    let execution_floor_guard_enabled =
        node_config_bool(node, "executionFloorGuardEnabled").unwrap_or(false);
    let best_ask_floor_price = if execution_floor_guard_enabled && side == "buy" {
        Some(resolve_action_place_order_execution_floor_price(node, step).ok_or_else(|| {
            anyhow::anyhow!("action.place_order executionFloorGuardEnabled=true but neither executionFloorPriceCent nor trigger_price found")
        })?)
    } else {
        None
    };
    let (
        runtime_snapshot,
        runtime_snapshot_json,
        fresh_submit_lease_until,
        prefetched_fee_rate_bps,
    ) = prepare_trade_builder_runtime_snapshot_state(
        cfg,
        client,
        step,
        &market_slug,
        &token_id,
        should_inline_submit,
        &side,
        reference_price,
        best_ask_floor_price,
    )
    .await;
    let cycle_window_open_at = resolve_action_place_order_datetime(
        step,
        context,
        &["cycleWindowOpenAt", "cycle_window_open_at"],
        "cycleWindowOpenAt",
    );
    let cycle_window_end_at = resolve_action_place_order_datetime(
        step,
        context,
        &["cycleWindowEndAt", "cycle_window_end_at"],
        "cycleWindowEndAt",
    );
    let (eligible_after_at, eligible_before_at) = if side == "buy" {
        match (cycle_window_open_at, cycle_window_end_at) {
            (Some(open_at), Some(end_at)) if open_at < end_at => (Some(open_at), Some(end_at)),
            _ => (None, None),
        }
    } else {
        (None, None)
    };
    let flags = resolve_action_place_order_notification_and_retry_flags(node);
    let exit_config = resolve_action_place_order_exit_config(
        node,
        graph,
        &side,
        &kind,
        is_internal_time_exit,
        &tp_rules,
        &sl_rules,
        ptb_stop_loss.as_ref(),
        &ptb_stop_loss_rules,
    )?;
    let tp_enabled = exit_config.tp_enabled;
    let tp_price = exit_config.tp_price;
    let sl_enabled = exit_config.sl_enabled;
    let sl_price = exit_config.sl_price;
    let sl_trigger_price_mode = exit_config.sl_trigger_price_mode.as_deref();
    let reenter_on_sl_hit = exit_config.reenter_on_sl_hit;
    let reentry_max_attempts = exit_config.reentry_max_attempts;
    let reentry_trigger_node_key = exit_config.reentry_trigger_node_key;
    let staged_sl_behavior = exit_config.staged_sl_behavior;
    let ptb_stop_loss_gap_usd = exit_config.ptb_stop_loss_gap_usd;
    let ptb_reference_price = exit_config.ptb_reference_price;
    let ptb_stop_loss_time_decay_mode = exit_config.ptb_stop_loss_time_decay_mode.as_deref();
    let sizing = if side == "sell" {
        if let Some(parent_builder_order_id) = window_end_parent_builder_order_id {
            let Some(sizing) = resolve_action_place_order_window_end_auto_sell_sizing(
                repo,
                node,
                step,
                parent_builder_order_id,
                source_trade_id,
                &token_id,
            )
            .await?
            else {
                repo.append_trade_builder_order_event(
                    parent_builder_order_id,
                    "window_end_auto_sell_skipped_closed",
                    &json!({
                        "node_key": node.key,
                        "source_trade_id": source_trade_id,
                        "market_slug": market_slug,
                        "token_id": token_id,
                        "outcome_label": outcome_label,
                    }),
                )
                .await?;
                return Ok(TradeFlowNodeExecution {
                    output: json!({
                        "node_key": node.key,
                        "skipped": true,
                        "reason": "window_end_auto_sell_skipped_closed",
                        "internal_mode": effective_internal_mode,
                        "source_trade_id": source_trade_id,
                        "market_slug": market_slug,
                        "token_id": token_id,
                        "outcome_label": outcome_label,
                        "parent_builder_order_id": parent_builder_order_id,
                    }),
                    routes: Vec::new(),
                    repeat_at: None,
                    repeat_idempotency_key: None,
                });
            };
            sizing
        } else {
            resolve_action_place_order_sell_sizing(
                repo,
                node,
                step,
                source_trade_id,
                &token_id,
                trigger_size_for_first_fire,
                configured_size_usdc,
                configured_size_pct,
                use_pct_size,
            )
            .await?
        }
    } else {
        resolve_action_place_order_buy_sizing(
            repo,
            source_trade_id,
            trigger_size_for_first_fire,
            configured_size_usdc,
            configured_size_pct,
            configured_target_qty,
            use_pct_size,
            use_share_size,
            reference_price,
        )
        .await?
    };
    let persisted_trigger_price = if side == "buy"
        && kind == "immediate"
        && execution_mode == "market"
        && sizing.size_basis == TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC
    {
        reference_price.or(trigger_price)
    } else {
        trigger_price
    };

    let risk = risk_gate_manual_order(
        repo,
        run_id,
        cfg,
        Some(run.user_id),
        source_trade_id,
        sizing.size_usdc,
        limits,
        policy,
    )
    .await?;
    if !matches!(risk, RiskDecision::Allow) {
        let output = json!({
            "node_key": node.key,
            "blocked": true,
            "blocked_by": "risk_policy",
            "risk_decision": format!("{risk:?}"),
            "source_trade_id": source_trade_id,
            "ignored_stale_existing_order": ignored_existing_order.is_some(),
            "ignored_existing_order_id": ignored_existing_order.as_ref().and_then(|(id, _, _)| *id),
            "ignored_existing_order_reason": ignored_existing_order.as_ref().map(|(_, reason, _)| *reason),
            "ignored_existing_order_scope": ignored_existing_order
                .as_ref()
                .and_then(|(_, _, scope)| scope.map(|scope| scope.as_str()))
        });
        return Ok(TradeFlowNodeExecution {
            output,
            routes: vec![TradeFlowRouteDecision {
                edge_type: "on_error".to_string(),
                available_at: Utc::now(),
            }],
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }

    let price_to_beat_guard_notification_seed =
        trade_flow::guards::take_price_to_beat_guard_notification_seed(
            context,
            &node.key,
            &market_slug,
            &token_id,
        );
    let price_to_beat_guard_snapshot =
        flow_context_value(context, "priceToBeatGuard").unwrap_or(Value::Null);
    if let Some(existing_order) = existing_order.as_ref() {
        repo.update_trade_builder_order_sizing_and_state(
            existing_order.id,
            sizing.size_basis,
            sizing.size_usdc,
            sizing.target_qty,
            Some(sizing.size_usdc),
            sizing.remaining_qty,
            "triggered",
            None,
            eligible_after_at.clone(),
            eligible_before_at.clone(),
            Some(run.definition_id),
            Some(run.id),
            Some(&node.key),
        )
        .await?;
        if let Some(reason) = price_to_beat_guard_notification_seed.as_deref() {
            repo.update_trade_builder_guard_notification_reason(existing_order.id, Some(reason))
                .await?;
        }
        persist_trade_builder_runtime_snapshot_state(
            repo,
            existing_order.id,
            prefetched_fee_rate_bps,
            runtime_snapshot_json.as_ref(),
            fresh_submit_lease_until,
        )
        .await?;
        repo.set_trade_builder_order_notification_flags(
            existing_order.id,
            flags.notify_on_order_submitted,
            flags.notify_on_fill,
            flags.notify_on_order_not_filled,
            flags.notify_on_trigger_guard_blocked,
            flags.notify_on_execution_floor_blocked,
            flags.notify_on_tp_hit,
            flags.notify_on_sl_hit,
            flags.notify_on_max_price_blocked,
        )
        .await?;
        repo.set_trade_builder_order_guard_retry_flags(
            existing_order.id,
            flags.retry_on_trigger_guard_block,
            flags.retry_on_execution_floor_guard_block,
            flags.retry_on_max_price_block,
        )
        .await?;
        repo.set_trade_builder_order_staged_sl_behavior(
            existing_order.id,
            staged_sl_behavior.reentry_only_after_all_stages,
        )
        .await?;
        repo.set_trade_builder_order_ptb_stop_loss(
            existing_order.id,
            ptb_stop_loss_gap_usd,
            ptb_reference_price.or(existing_order.ptb_reference_price),
            (!ptb_stop_loss_rules.is_empty()).then_some(ptb_stop_loss_rules.as_slice()),
            ptb_stop_loss_time_decay_mode,
        )
        .await?;
        let mut flow_rearmed_payload = json!({
            "flow_run_id": run.id,
            "node_key": node.key,
            "previous_status": &existing_order.status,
            "previous_size_basis": &existing_order.size_basis,
            "next_status": "triggered",
            "size_basis": sizing.size_basis,
            "size_mode": sizing.resolved_size_mode,
            "size_pct": sizing.resolved_size_pct,
            "size_usdc": sizing.size_usdc,
            "target_qty": sizing.target_qty,
            "remaining_qty": sizing.remaining_qty,
            "eligible_after_at": eligible_after_at.as_ref().map(|value| value.to_rfc3339()),
            "eligible_before_at": eligible_before_at.as_ref().map(|value| value.to_rfc3339()),
            "notify_on_order_submitted": flags.notify_on_order_submitted,
            "notify_on_fill": flags.notify_on_fill,
            "notify_on_order_not_filled": flags.notify_on_order_not_filled,
            "notify_on_trigger_guard_blocked": flags.notify_on_trigger_guard_blocked,
            "notify_on_execution_floor_blocked": flags.notify_on_execution_floor_blocked,
            "retry_on_trigger_guard_block": flags.retry_on_trigger_guard_block,
            "retry_on_execution_floor_guard_block": flags.retry_on_execution_floor_guard_block,
            "retry_on_max_price_block": flags.retry_on_max_price_block,
            "notify_on_tp_hit": flags.notify_on_tp_hit,
            "notify_on_sl_hit": flags.notify_on_sl_hit,
            "ptb_stop_loss_gap_usd": ptb_stop_loss_gap_usd,
            "ptb_reference_price": ptb_reference_price.or(existing_order.ptb_reference_price),
            "ptb_stop_loss_rules": serde_json::to_value(&ptb_stop_loss_rules)?,
            "last_guard_notification_reason": price_to_beat_guard_notification_seed.clone(),
            "price_to_beat_guard": price_to_beat_guard_snapshot.clone(),
        });
        append_trade_builder_runtime_snapshot_payload(
            flow_rearmed_payload
                .as_object_mut()
                .expect("flow_rearmed payload"),
            runtime_snapshot.as_ref(),
            fresh_submit_lease_until,
        );
        attach_action_place_order_node_snapshot(
            repo,
            existing_order.id,
            existing_order.parent_order_id.unwrap_or(existing_order.id),
            run,
            node,
            graph,
            flow_rearmed_payload
                .as_object_mut()
                .expect("flow_rearmed payload"),
        )
        .await?;
        repo.append_trade_builder_order_event(
            existing_order.id,
            "flow_rearmed",
            &flow_rearmed_payload,
        )
        .await?;
        if !is_internal_time_exit {
            bind_action_place_order_ref_bindings(context, node, &ref_key, existing_order.id);
        }
        let mut output = json!({
            "node_key": node.key,
            "builder_order_id": existing_order.id,
            "ref_key": ref_key,
            "source_trade_id": source_trade_id,
            "kind": &existing_order.kind,
            "side": side,
            "status": "triggered",
            "execution_mode": execution_mode,
            "market_slug": market_slug,
            "token_id": token_id,
            "size_basis": sizing.size_basis,
            "size_mode": sizing.resolved_size_mode,
            "size_pct": sizing.resolved_size_pct,
            "size_usdc": sizing.size_usdc,
            "target_qty": sizing.target_qty,
            "remaining_qty": sizing.remaining_qty,
            "notify_on_order_submitted": flags.notify_on_order_submitted,
            "notify_on_fill": flags.notify_on_fill,
            "notify_on_order_not_filled": flags.notify_on_order_not_filled,
            "notify_on_trigger_guard_blocked": flags.notify_on_trigger_guard_blocked,
            "notify_on_execution_floor_blocked": flags.notify_on_execution_floor_blocked,
            "retry_on_trigger_guard_block": flags.retry_on_trigger_guard_block,
            "retry_on_execution_floor_guard_block": flags.retry_on_execution_floor_guard_block,
            "retry_on_max_price_block": flags.retry_on_max_price_block,
            "notify_on_tp_hit": flags.notify_on_tp_hit,
            "notify_on_sl_hit": flags.notify_on_sl_hit,
            "ptb_stop_loss_gap_usd": ptb_stop_loss_gap_usd,
            "ptb_reference_price": ptb_reference_price.or(existing_order.ptb_reference_price),
            "last_guard_notification_reason": price_to_beat_guard_notification_seed.clone(),
            "price_to_beat_guard": price_to_beat_guard_snapshot.clone(),
            "should_inline_submit": should_inline_submit,
            "rearmed_existing_order": true
        });
        append_trade_builder_runtime_snapshot_payload(
            output.as_object_mut().expect("rearmed output"),
            runtime_snapshot.as_ref(),
            fresh_submit_lease_until,
        );
        return Ok(TradeFlowNodeExecution {
            output,
            routes: Vec::new(),
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }

    if let Err(err) = append_parallel_flow_overlap_observed(
        repo,
        run,
        &node.key,
        &market_slug,
        &token_id,
        &outcome_label,
        &side,
        source_trade_id,
    )
    .await
    {
        warn!(
            flow_run_id = run.id,
            node_key = %node.key,
            market_slug = %market_slug,
            token_id = %token_id,
            side = %side,
            error = %err,
            "PARALLEL_FLOW_OVERLAP_CHECK_FAILED"
        );
    }

    let parent_builder_order_id = if is_internal_time_exit || is_window_end_auto_sell {
        window_end_parent_builder_order_id.or_else(|| {
            step_input_i64(step, &["parentBuilderOrderId", "parent_builder_order_id"])
                .filter(|value| *value > 0)
        })
    } else {
        None
    };
    let builder_order_id = repo
        .create_trade_builder_order_with_exit_ladders(
            source_trade_id,
            &kind,
            if side == "sell" && kind == "immediate" {
                "triggered"
            } else {
                "pending"
            },
            &market_slug,
            &token_id,
            &outcome_label,
            &side,
            &execution_mode,
            trigger_condition.as_deref(),
            persisted_trigger_price,
            max_price,
            guard_trigger_price,
            best_ask_floor_price,
            sizing.size_basis,
            sizing.size_usdc,
            sizing.target_qty,
            sizing.remaining_qty,
            min_price_distance_cent,
            expires_at,
            eligible_after_at.clone(),
            eligible_before_at.clone(),
            max_triggers,
            parent_builder_order_id,
            tp_enabled,
            tp_price,
            (!tp_rules.is_empty()).then_some(tp_rules.as_slice()),
            sl_enabled,
            sl_price,
            (!sl_rules.is_empty()).then_some(sl_rules.as_slice()),
            (!time_exit_rules.is_empty()).then_some(time_exit_rules.as_slice()),
            prefetched_fee_rate_bps.unwrap_or_default() as i64,
            Some(run.definition_id),
            Some(run.id),
            Some(&node.key),
            ptb_stop_loss_gap_usd,
            ptb_reference_price,
            (!ptb_stop_loss_rules.is_empty()).then_some(ptb_stop_loss_rules.as_slice()),
            ptb_stop_loss_time_decay_mode,
            false,
            None,
            None,
            false,
            staged_sl_behavior.reentry_only_after_all_stages,
            sl_trigger_price_mode,
            reenter_on_sl_hit,
            reentry_max_attempts,
            reentry_trigger_node_key.as_deref(),
            flags.notify_on_order_submitted,
            flags.notify_on_fill,
            flags.notify_on_order_not_filled,
            flags.notify_on_trigger_guard_blocked,
            flags.notify_on_execution_floor_blocked,
            flags.notify_on_tp_hit,
            flags.notify_on_sl_hit,
            flags.notify_on_max_price_blocked,
            price_to_beat_guard_notification_seed.as_deref(),
            flags.retry_on_trigger_guard_block,
            flags.retry_on_execution_floor_guard_block,
            flags.retry_on_max_price_block,
            None,
            None,
            None,
        )
        .await?;
    persist_trade_builder_runtime_snapshot_state(
        repo,
        builder_order_id,
        prefetched_fee_rate_bps,
        runtime_snapshot_json.as_ref(),
        fresh_submit_lease_until,
    )
    .await?;
    let initial_status = if side == "sell" && kind == "immediate" {
        "triggered"
    } else {
        "pending"
    };
    let buy_fill_lock = (!is_special_internal_sell)
        .then(|| resolve_action_place_order_buy_fill_lock_config(node, &side))
        .transpose()?
        .flatten()
        .map(|config| config.to_value())
        .unwrap_or(Value::Null);
    let selected_entry_timing_profile_value = selected_entry_timing_profile.clone().unwrap_or(Value::Null);
    let mut flow_created_payload = serde_json::Map::new();
    flow_created_payload.insert("flow_run_id".to_string(), json!(run.id));
    flow_created_payload.insert("node_key".to_string(), json!(node.key));
    flow_created_payload.insert("source_trade_id".to_string(), json!(source_trade_id));
    flow_created_payload.insert("execution_mode".to_string(), json!(execution_mode));
    flow_created_payload.insert(
        "order_type".to_string(),
        json!(clob_order_type_for_execution_mode(&execution_mode)),
    );
    flow_created_payload.insert("initial_status".to_string(), json!(initial_status));
    flow_created_payload.insert("size_basis".to_string(), json!(sizing.size_basis));
    flow_created_payload.insert("size_mode".to_string(), json!(sizing.resolved_size_mode));
    flow_created_payload.insert("size_pct".to_string(), json!(sizing.resolved_size_pct));
    flow_created_payload.insert("size_usdc".to_string(), json!(sizing.size_usdc));
    flow_created_payload.insert("target_qty".to_string(), json!(sizing.target_qty));
    flow_created_payload.insert("remaining_qty".to_string(), json!(sizing.remaining_qty));
    flow_created_payload.insert(
        "eligible_after_at".to_string(),
        json!(eligible_after_at.as_ref().map(|value| value.to_rfc3339())),
    );
    flow_created_payload.insert(
        "eligible_before_at".to_string(),
        json!(eligible_before_at.as_ref().map(|value| value.to_rfc3339())),
    );
    flow_created_payload.insert("trigger_sizes".to_string(), json!(trigger_sizes));
    flow_created_payload.insert("selected_entry_timing_profile".to_string(), selected_entry_timing_profile_value.clone());
    flow_created_payload.insert("buy_fill_lock".to_string(), buy_fill_lock.clone());
    flow_created_payload.insert("max_price".to_string(), json!(max_price));
    flow_created_payload.insert(
        "price_to_beat_guard".to_string(),
        price_to_beat_guard_snapshot.clone(),
    );
    flow_created_payload.insert(
        "guard_trigger_price".to_string(),
        json!(guard_trigger_price),
    );
    flow_created_payload.insert(
        "reentry_band".to_string(),
        json!({"generation": reentry_guard_resolution.generation, "band_active": reentry_guard_resolution.band_active, "configured_min_price": reentry_guard_resolution.configured_min_price, "configured_max_price": reentry_guard_resolution.configured_max_price, "effective_guard_trigger_price": guard_trigger_price, "effective_max_price": max_price}),
    );
    flow_created_payload.insert(
        "trigger_price_guard_enabled".to_string(),
        json!(trigger_price_guard_enabled),
    );
    flow_created_payload.insert(
        "best_ask_floor_price".to_string(),
        json!(best_ask_floor_price),
    );
    flow_created_payload.insert(
        "execution_floor_guard_enabled".to_string(),
        json!(execution_floor_guard_enabled),
    );
    flow_created_payload.insert(
        "ignored_stale_existing_order".to_string(),
        json!(ignored_existing_order.is_some()),
    );
    flow_created_payload.insert(
        "ignored_existing_order_id".to_string(),
        json!(ignored_existing_order.as_ref().and_then(|(id, _, _)| *id)),
    );
    flow_created_payload.insert(
        "ignored_existing_order_reason".to_string(),
        json!(ignored_existing_order
            .as_ref()
            .map(|(_, reason, _)| *reason)),
    );
    flow_created_payload.insert(
        "ignored_existing_order_scope".to_string(),
        json!(ignored_existing_order
            .as_ref()
            .and_then(|(_, _, scope)| scope.map(|scope| scope.as_str()))),
    );
    flow_created_payload.insert("protection".to_string(), protection_output.clone());
    flow_created_payload.insert(
        "internal_mode".to_string(),
        json!(effective_internal_mode.clone()),
    );
    flow_created_payload.insert("tp_enabled".to_string(), json!(tp_enabled));
    flow_created_payload.insert("tp_price".to_string(), json!(tp_price));
    flow_created_payload.insert("sl_enabled".to_string(), json!(sl_enabled));
    flow_created_payload.insert("sl_price".to_string(), json!(sl_price));
    flow_created_payload.insert("tp_rules".to_string(), serde_json::to_value(&tp_rules)?);
    flow_created_payload.insert("sl_rules".to_string(), serde_json::to_value(&sl_rules)?);
    flow_created_payload.insert(
        "time_exit_rules".to_string(),
        serde_json::to_value(&time_exit_rules)?,
    );
    flow_created_payload.insert(
        "sl_trigger_price_mode".to_string(),
        json!(sl_trigger_price_mode),
    );
    flow_created_payload.insert("reenter_on_sl_hit".to_string(), json!(reenter_on_sl_hit));
    flow_created_payload.insert(
        "reentry_max_attempts".to_string(),
        json!(reentry_max_attempts),
    );
    flow_created_payload.insert(
        "reentry_trigger_node_key".to_string(),
        json!(reentry_trigger_node_key.as_deref()),
    );
    flow_created_payload.insert(
        "ptb_stop_loss_gap_usd".to_string(),
        json!(ptb_stop_loss_gap_usd),
    );
    flow_created_payload.insert(
        "ptb_reference_price".to_string(),
        json!(ptb_reference_price),
    );
    flow_created_payload.insert(
        "ptb_stop_loss_rules".to_string(),
        serde_json::to_value(&ptb_stop_loss_rules)?,
    );
    flow_created_payload.insert(
        "ptb_stop_loss_time_decay_mode".to_string(),
        json!(ptb_stop_loss_time_decay_mode),
    );
    flow_created_payload.insert(
        "staged_sl_reentry_only_after_all_stages".to_string(),
        json!(staged_sl_behavior.reentry_only_after_all_stages),
    );
    append_action_place_order_notification_and_retry_payload(&mut flow_created_payload, &flags);
    flow_created_payload.insert(
        "last_guard_notification_reason".to_string(),
        json!(price_to_beat_guard_notification_seed.clone()),
    );
    flow_created_payload.insert(
        "should_inline_submit".to_string(),
        json!(should_inline_submit),
    );
    append_trade_builder_runtime_snapshot_payload(
        &mut flow_created_payload,
        runtime_snapshot.as_ref(),
        fresh_submit_lease_until,
    );
    attach_action_place_order_node_snapshot(
        repo,
        builder_order_id,
        parent_builder_order_id.unwrap_or(builder_order_id),
        run,
        node,
        graph,
        &mut flow_created_payload,
    )
    .await?;
    repo.append_trade_builder_order_event(
        builder_order_id,
        "flow_created",
        &Value::Object(flow_created_payload),
    )
    .await?;

    if !is_special_internal_sell {
        bind_action_place_order_ref_bindings(context, node, &ref_key, builder_order_id);
    }
    if is_window_end_auto_sell {
        if let Some(parent_builder_order_id) = parent_builder_order_id {
            repo.append_trade_builder_order_event(
                parent_builder_order_id,
                "window_end_auto_sell_submitted",
                &json!({
                    "node_key": node.key,
                    "builder_order_id": builder_order_id,
                    "source_trade_id": source_trade_id,
                    "size_basis": sizing.size_basis,
                    "target_qty": sizing.target_qty,
                    "remaining_qty": sizing.remaining_qty,
                }),
            )
            .await?;
        }
        if let Some(parent_builder_order_id) = parent_builder_order_id {
            let siblings = repo
                .list_trade_builder_child_orders_by_parent(parent_builder_order_id, None)
                .await?
                .into_iter()
                .filter(|sibling| sibling.id != builder_order_id)
                .map(|sibling| sibling.id)
                .collect::<Vec<_>>();
            repo.append_trade_builder_order_event(
                parent_builder_order_id,
                "window_end_auto_sell_child_exits_preserved",
                &json!({
                    "node_key": node.key,
                    "builder_order_id": builder_order_id,
                    "sibling_order_ids": siblings,
                }),
            )
            .await?;
        }
    }

    let mut output = serde_json::Map::new();
    output.insert("node_key".to_string(), json!(node.key));
    output.insert("builder_order_id".to_string(), json!(builder_order_id));
    output.insert("ref_key".to_string(), json!(ref_key));
    output.insert("source_trade_id".to_string(), json!(source_trade_id));
    output.insert("kind".to_string(), json!(kind));
    output.insert("side".to_string(), json!(side));
    output.insert("execution_mode".to_string(), json!(execution_mode));
    output.insert(
        "order_type".to_string(),
        json!(clob_order_type_for_execution_mode(&execution_mode)),
    );
    output.insert("market_slug".to_string(), json!(market_slug));
    output.insert("token_id".to_string(), json!(token_id));
    output.insert("selected_entry_timing_profile".to_string(), selected_entry_timing_profile_value);
    output.insert("buy_fill_lock".to_string(), buy_fill_lock);
    output.insert("max_price".to_string(), json!(max_price));
    output.insert(
        "price_to_beat_guard".to_string(),
        price_to_beat_guard_snapshot.clone(),
    );
    output.insert(
        "guard_trigger_price".to_string(),
        json!(guard_trigger_price),
    );
    output.insert(
        "reentry_band".to_string(),
        json!({"generation": reentry_guard_resolution.generation, "band_active": reentry_guard_resolution.band_active, "configured_min_price": reentry_guard_resolution.configured_min_price, "configured_max_price": reentry_guard_resolution.configured_max_price, "effective_guard_trigger_price": guard_trigger_price, "effective_max_price": max_price}),
    );
    output.insert(
        "best_ask_floor_price".to_string(),
        json!(best_ask_floor_price),
    );
    output.insert("protection".to_string(), protection_output);
    output.insert("size_basis".to_string(), json!(sizing.size_basis));
    output.insert("size_mode".to_string(), json!(sizing.resolved_size_mode));
    output.insert("size_pct".to_string(), json!(sizing.resolved_size_pct));
    output.insert("size_usdc".to_string(), json!(sizing.size_usdc));
    output.insert("target_qty".to_string(), json!(sizing.target_qty));
    output.insert("remaining_qty".to_string(), json!(sizing.remaining_qty));
    output.insert("tp_enabled".to_string(), json!(tp_enabled));
    output.insert("tp_price".to_string(), json!(tp_price));
    output.insert("sl_enabled".to_string(), json!(sl_enabled));
    output.insert("sl_price".to_string(), json!(sl_price));
    output.insert(
        "internal_mode".to_string(),
        json!(effective_internal_mode.clone()),
    );
    output.insert(
        "parent_builder_order_id".to_string(),
        json!(parent_builder_order_id),
    );
    output.insert("tp_rules".to_string(), serde_json::to_value(&tp_rules)?);
    output.insert("sl_rules".to_string(), serde_json::to_value(&sl_rules)?);
    output.insert(
        "time_exit_rules".to_string(),
        serde_json::to_value(&time_exit_rules)?,
    );
    output.insert(
        "execution_floor_guard_enabled".to_string(),
        json!(execution_floor_guard_enabled),
    );
    output.insert(
        "sl_trigger_price_mode".to_string(),
        json!(sl_trigger_price_mode),
    );
    output.insert("reenter_on_sl_hit".to_string(), json!(reenter_on_sl_hit));
    output.insert(
        "reentry_max_attempts".to_string(),
        json!(reentry_max_attempts),
    );
    output.insert(
        "reentry_trigger_node_key".to_string(),
        json!(reentry_trigger_node_key.as_deref()),
    );
    output.insert(
        "ptb_stop_loss_gap_usd".to_string(),
        json!(ptb_stop_loss_gap_usd),
    );
    output.insert(
        "ptb_reference_price".to_string(),
        json!(ptb_reference_price),
    );
    output.insert(
        "ptb_stop_loss_rules".to_string(),
        serde_json::to_value(&ptb_stop_loss_rules)?,
    );
    output.insert(
        "ptb_stop_loss_time_decay_mode".to_string(),
        json!(ptb_stop_loss_time_decay_mode),
    );
    output.insert(
        "staged_sl_reentry_only_after_all_stages".to_string(),
        json!(staged_sl_behavior.reentry_only_after_all_stages),
    );
    append_action_place_order_notification_and_retry_payload(&mut output, &flags);
    output.insert(
        "last_guard_notification_reason".to_string(),
        json!(price_to_beat_guard_notification_seed.clone()),
    );
    output.insert(
        "should_inline_submit".to_string(),
        json!(should_inline_submit),
    );
    append_trade_builder_runtime_snapshot_payload(
        &mut output,
        runtime_snapshot.as_ref(),
        fresh_submit_lease_until,
    );

    Ok(TradeFlowNodeExecution {
        output: Value::Object(output),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}
