#[allow(clippy::too_many_arguments)]
async fn execute_action_place_order(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let side = node_config_string(node, "side")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("action.place_order requires side (buy or sell)"))?;
    anyhow::ensure!(
        matches!(side.as_str(), "buy" | "sell"),
        "action.place_order side must be buy or sell"
    );
    let execution_mode = node_config_string(node, "executionMode")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("action.place_order requires executionMode (market or limit)")
        })?;
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
    if let Some(stale_market_retry) =
        resolve_action_place_order_stale_market_retry(node, context, step)
    {
        let stale_market_slug = stale_market_retry.stale_market_slug;
        let current_market_slug = stale_market_retry.current_market_slug;
        set_flow_context(context, "priceToBeatGuard", Value::Null);
        set_flow_context(context, "priceToBeatGuardWaiting", Value::Null);
        set_flow_context(context, "priceToBeatGuardWaitingReason", Value::Null);
        set_flow_context(context, "lastGuardNotificationSeed", Value::Null);
        repo.append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "action_place_order_stale_market_skipped",
            &json!({
                "node_key": node.key,
                "node_type": node.node_type,
                "reason": "stale_market_retry_skipped",
                "stale_market_slug": stale_market_slug.clone(),
                "current_market_slug": current_market_slug.clone(),
                "token_id": token_id.clone(),
                "outcome_label": outcome_label.clone(),
                "side": side.clone(),
                "execution_mode": execution_mode.clone(),
            }),
        )
        .await?;
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "node_key": node.key,
                "skipped": true,
                "reason": "stale_market_retry_skipped",
                "market_slug": current_market_slug,
                "stale_market_slug": stale_market_slug,
                "token_id": token_id,
                "outcome_label": outcome_label,
                "side": side,
                "execution_mode": execution_mode,
            }),
            routes: vec![],
            repeat_at: None,
            repeat_idempotency_key: None,
        });
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
    if let Some(blocked_execution) =
        trade_flow::guards::maybe_block_action_place_order_price_to_beat_guard(
            repo,
            run,
            node,
            context,
            &market_slug,
            &token_id,
            &outcome_label,
            &side,
            &execution_mode,
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
    let size_mode = node_config_string(node, "sizeMode")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    if let Some(mode) = size_mode.as_deref() {
        anyhow::ensure!(
            matches!(mode, "usdc" | "pct"),
            "action.place_order sizeMode must be usdc or pct"
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
    let configured_size_usdc =
        node_config_f64(node, "sizeUsdc").or_else(|| node_config_f64(node, "targetNotionalUsdc"));
    let configured_size_pct =
        node_config_f64(node, "sizePct").or_else(|| node_config_f64(node, "sizePercent"));
    let use_pct_size = if trigger_size_for_first_fire.is_some() {
        if let Some(mode) = size_mode.as_deref() {
            mode == "pct"
        } else {
            configured_size_usdc.is_none() && configured_size_pct.is_some()
        }
    } else {
        matches!(size_mode.as_deref(), Some("pct"))
            || (configured_size_usdc.is_none() && configured_size_pct.is_some())
    };
    if !trigger_sizes.is_empty() && use_pct_size {
        let trigger_pct_total: f64 = trigger_sizes.iter().sum();
        anyhow::ensure!(
            trigger_pct_total <= 100.000001,
            "action.place_order pct triggerSizes total must be <= 100"
        );
    }
    if source_trade_id.is_none() {
        anyhow::ensure!(
            side == "buy",
            "action.place_order side=sell requires sourceTradeId or an explicit open-position context"
        );
        anyhow::ensure!(
            !use_pct_size,
            "action.place_order sizePct requires sourceTradeId when sizeMode is pct"
        );
        let seed_size_usdc = trigger_size_for_first_fire
            .or(configured_size_usdc)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "action.place_order requires sizeUsdc/targetNotionalUsdc > 0 (or sizePct in pct mode)"
                )
            })?;
        anyhow::ensure!(seed_size_usdc > 0.0, "action.place_order size must be > 0");
        let reference_price = resolve_action_place_order_reference_price(node, step).unwrap_or(0.5);
        let ensured_source_trade_id = repo
            .ensure_manual_builder_source_trade(
                run.user_id,
                &market_slug,
                &token_id,
                &outcome_label,
                reference_price,
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
    let ref_key = node_config_string(node, "refKey").unwrap_or_else(|| node.key.clone());
    let trigger_condition = node_config_string(node, "triggerCondition");
    let trigger_price = node_config_f64(node, "triggerPrice")
        .or_else(|| node_config_f64(node, "triggerPriceCent").map(|v| v / 100.0));
    if let Some(condition) = trigger_condition.as_deref() {
        anyhow::ensure!(
            matches!(
                condition,
                "cross_above" | "cross_below" | "level_above" | "level_below"
            ),
            "action.place_order triggerCondition must be cross_above/cross_below/level_above/level_below"
        );
    }
    let mut kind = node_config_string(node, "kind").unwrap_or_else(|| {
        if trigger_condition.is_some() && trigger_price.is_some() {
            "conditional".to_string()
        } else {
            "immediate".to_string()
        }
    });
    if kind != "conditional" && kind != "immediate" {
        kind = "immediate".to_string();
    }
    let expires_at = node_config_datetime(node, "expiresAt")?;
    let mut ignored_existing_order: Option<(Option<i64>, &'static str)> = None;
    let existing_order_id = resolve_action_place_order_existing_order_id(node, context);
    let mut existing_order = if let Some(existing_order_id) = existing_order_id {
        match repo.get_trade_builder_order(existing_order_id).await? {
            Some(order) => Some(order),
            None => {
                ignored_existing_order = Some((Some(existing_order_id), "missing_existing_order"));
                set_flow_ref(context, &ref_key, Value::Null);
                set_flow_ref(context, &node.key, Value::Null);
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
                set_flow_ref(context, &ref_key, json!(existing_order_snapshot.id));
                set_flow_ref(context, &node.key, json!(existing_order_snapshot.id));
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
                        "should_inline_submit": false,
                        "reused_existing_order": true
                    }),
                    routes: Vec::new(),
                    repeat_at: None,
                    repeat_idempotency_key: None,
                });
            }
            ActionPlaceOrderExistingOrderDecision::RearmErrorSell => {
                set_flow_ref(context, &ref_key, json!(existing_order_snapshot.id));
                set_flow_ref(context, &node.key, json!(existing_order_snapshot.id));
            }
            ActionPlaceOrderExistingOrderDecision::Ignore(reason) => {
                ignored_existing_order = Some((Some(existing_order_snapshot.id), reason));
                set_flow_ref(context, &ref_key, Value::Null);
                set_flow_ref(context, &node.key, Value::Null);
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
    let max_price = resolve_action_place_order_max_price(node, step, context);
    let trigger_price_guard_enabled =
        node_config_bool(node, "triggerPriceGuardEnabled").unwrap_or(false);
    let guard_trigger_price = if trigger_price_guard_enabled && side == "buy" {
        Some(resolve_action_place_order_guard_trigger_price(step).ok_or_else(|| {
            anyhow::anyhow!(
                "action.place_order triggerPriceGuardEnabled=true but no trigger_price found in step inputs"
            )
        })?)
    } else {
        None
    };
    let execution_floor_guard_enabled =
        node_config_bool(node, "executionFloorGuardEnabled").unwrap_or(false);
    let best_ask_floor_price = if execution_floor_guard_enabled && side == "buy" {
        Some(resolve_action_place_order_guard_trigger_price(step).ok_or_else(|| {
            anyhow::anyhow!(
                "action.place_order executionFloorGuardEnabled=true but no trigger_price found in step inputs"
            )
        })?)
    } else {
        None
    };
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
    let notify_on_fill = node_config_bool(node, "notifyOnOrderPlaced").unwrap_or(false);
    let notify_on_order_not_filled =
        node_config_bool(node, "notifyOnOrderNotFilled").unwrap_or(false);
    let notify_on_trigger_guard_blocked =
        node_config_bool(node, "notifyOnTriggerPriceBlocked").unwrap_or(false);
    let notify_on_execution_floor_blocked =
        node_config_bool(node, "notifyOnExecutionFloorBlocked").unwrap_or(false);
    let retry_on_trigger_guard_block =
        node_config_bool(node, "retryOnTriggerPriceGuardBlock").unwrap_or(false);
    let retry_on_execution_floor_guard_block =
        node_config_bool(node, "retryOnExecutionFloorGuardBlock").unwrap_or(false);
    let retry_on_max_price_block = node_config_bool(node, "retryOnMaxPriceBlock").unwrap_or(false);
    let notify_on_tp_hit = node_config_bool(node, "notifyOnTpHit").unwrap_or(false);
    let notify_on_sl_hit = node_config_bool(node, "notifyOnSlHit").unwrap_or(false);
    let notify_on_max_price_blocked =
        node_config_bool(node, "notifyOnMaxPriceBlocked").unwrap_or(false);

    let tp_enabled = node_config_bool(node, "tpEnabled").unwrap_or(false);
    let tp_price = resolve_action_place_order_exit_price(
        node,
        &side,
        tp_enabled,
        "tpPriceCent",
        "tpPrice",
        "tp",
    )?;
    let sl_enabled = node_config_bool(node, "slEnabled").unwrap_or(false);
    let sl_price = resolve_action_place_order_exit_price(
        node,
        &side,
        sl_enabled,
        "slPriceCent",
        "slPrice",
        "sl",
    )?;
    let sl_trigger_price_mode: Option<&str> = if sl_enabled {
        let raw = node_config_string(node, "slTriggerPriceMode");
        let mode = match raw.as_deref() {
            Some("best_bid") => "best_bid",
            Some("composite") => "composite",
            Some("composite_safe") => "composite_safe",
            Some("composite_fast") => "composite_fast",
            Some("last_trade") => "last_trade",
            Some(other) => {
                anyhow::bail!(
                    "action.place_order slTriggerPriceMode must be best_bid|composite|composite_safe|composite_fast|last_trade, got: {other}"
                )
            }
            None => "best_bid",
        };
        Some(mode)
    } else {
        None
    };
    let reenter_on_sl_hit =
        node_config_bool(node, "reenterOnSlHit").unwrap_or(false) && sl_enabled && side == "buy";
    let reentry_max_attempts = if reenter_on_sl_hit {
        node_config_i64(node, "reentryMaxAttempts")
            .unwrap_or(1)
            .clamp(1, 10) as i32
    } else {
        0
    };
    let reentry_trigger_node_key = if reenter_on_sl_hit {
        anyhow::ensure!(
            kind == "immediate",
            "action.place_order reenterOnSlHit is only supported for immediate buy nodes"
        );
        let trigger_key = find_upstream_market_price_trigger_key(&node.key, graph).ok_or_else(
            || anyhow::anyhow!("action.place_order reenterOnSlHit requires exactly one upstream trigger.market_price"),
        )?;
        let trigger_node = flow_node(graph, &trigger_key).ok_or_else(|| {
            anyhow::anyhow!(
                "action.place_order upstream trigger.market_price missing for re-entry: {trigger_key}"
            )
        })?;
        anyhow::ensure!(
            node_repeat_mode(trigger_node) == "once",
            "action.place_order reenterOnSlHit requires upstream trigger.market_price repeatMode=once"
        );
        Some(trigger_key)
    } else {
        None
    };
    if let (Some(tp_price), Some(sl_price)) = (tp_price, sl_price) {
        anyhow::ensure!(
            sl_price < tp_price,
            "action.place_order requires slPrice < tpPrice when both stop loss and take profit are enabled"
        );
    }
    let sizing = if side == "sell" {
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
    } else if use_pct_size {
        let size_pct = trigger_size_for_first_fire
            .or(configured_size_pct)
            .ok_or_else(|| {
                anyhow::anyhow!("action.place_order requires sizePct (0, 100] when sizeMode is pct")
            })?;
        anyhow::ensure!(
            size_pct > 0.0 && size_pct <= 100.0,
            "action.place_order sizePct must be in (0, 100]"
        );
        let source_notional = repo
            .trade_notional_usdc(source_trade_id)
            .await?
            .unwrap_or(0.0);
        anyhow::ensure!(
            source_notional > 0.0,
            "action.place_order sizePct requires source trade notional > 0"
        );
        let resolved = source_notional * (size_pct / 100.0);
        anyhow::ensure!(
            resolved > 0.0,
            "action.place_order resolved size must be > 0"
        );
        ActionPlaceOrderSizing {
            size_usdc: resolved,
            size_basis: TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC,
            target_qty: None,
            remaining_qty: None,
            resolved_size_mode: "pct",
            resolved_size_pct: Some(size_pct),
        }
    } else {
        let resolved = trigger_size_for_first_fire
            .or(configured_size_usdc)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "action.place_order requires sizeUsdc/targetNotionalUsdc > 0 (or sizePct in pct mode)"
                )
            })?;
        anyhow::ensure!(resolved > 0.0, "action.place_order size must be > 0");
        ActionPlaceOrderSizing {
            size_usdc: resolved,
            size_basis: TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC,
            target_qty: None,
            remaining_qty: None,
            resolved_size_mode: "usdc",
            resolved_size_pct: None,
        }
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
            "risk_decision": format!("{risk:?}"),
            "source_trade_id": source_trade_id,
            "ignored_stale_existing_order": ignored_existing_order.is_some(),
            "ignored_existing_order_id": ignored_existing_order.as_ref().and_then(|(id, _)| *id),
            "ignored_existing_order_reason": ignored_existing_order.as_ref().map(|(_, reason)| *reason)
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
        repo.set_trade_builder_order_notification_flags(
            existing_order.id,
            notify_on_fill,
            notify_on_order_not_filled,
            notify_on_trigger_guard_blocked,
            notify_on_execution_floor_blocked,
            notify_on_tp_hit,
            notify_on_sl_hit,
            notify_on_max_price_blocked,
        )
        .await?;
        repo.set_trade_builder_order_guard_retry_flags(
            existing_order.id,
            retry_on_trigger_guard_block,
            retry_on_execution_floor_guard_block,
            retry_on_max_price_block,
        )
        .await?;
        repo.set_trade_builder_order_guard_retry_flags(
            existing_order.id,
            retry_on_trigger_guard_block,
            retry_on_execution_floor_guard_block,
            retry_on_max_price_block,
        )
        .await?;
        repo.append_trade_builder_order_event(
            existing_order.id,
            "flow_rearmed",
            &json!({
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
                "notify_on_fill": notify_on_fill,
                "notify_on_order_not_filled": notify_on_order_not_filled,
                "notify_on_trigger_guard_blocked": notify_on_trigger_guard_blocked,
                "notify_on_execution_floor_blocked": notify_on_execution_floor_blocked,
                "retry_on_trigger_guard_block": retry_on_trigger_guard_block,
                "retry_on_execution_floor_guard_block": retry_on_execution_floor_guard_block,
                "retry_on_max_price_block": retry_on_max_price_block,
                "notify_on_tp_hit": notify_on_tp_hit,
                "notify_on_sl_hit": notify_on_sl_hit,
                "last_guard_notification_reason": price_to_beat_guard_notification_seed.clone(),
                "price_to_beat_guard": price_to_beat_guard_snapshot.clone(),
            }),
        )
        .await?;
        set_flow_ref(context, &ref_key, json!(existing_order.id));
        set_flow_ref(context, &node.key, json!(existing_order.id));
        return Ok(TradeFlowNodeExecution {
            output: json!({
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
                "notify_on_fill": notify_on_fill,
                "notify_on_order_not_filled": notify_on_order_not_filled,
                "notify_on_trigger_guard_blocked": notify_on_trigger_guard_blocked,
                "notify_on_execution_floor_blocked": notify_on_execution_floor_blocked,
                "retry_on_trigger_guard_block": retry_on_trigger_guard_block,
                "retry_on_execution_floor_guard_block": retry_on_execution_floor_guard_block,
                "retry_on_max_price_block": retry_on_max_price_block,
                "notify_on_tp_hit": notify_on_tp_hit,
                "notify_on_sl_hit": notify_on_sl_hit,
                "last_guard_notification_reason": price_to_beat_guard_notification_seed.clone(),
                "price_to_beat_guard": price_to_beat_guard_snapshot.clone(),
                "should_inline_submit": should_inline_submit,
                "rearmed_existing_order": true
            }),
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

    let builder_order_id = repo
        .create_trade_builder_order(
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
            trigger_price,
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
            None,
            tp_enabled,
            tp_price,
            sl_enabled,
            sl_price,
            0,
            Some(run.definition_id),
            Some(run.id),
            Some(&node.key),
            sl_trigger_price_mode,
            reenter_on_sl_hit,
            reentry_max_attempts,
            reentry_trigger_node_key.as_deref(),
            notify_on_fill,
            notify_on_order_not_filled,
            notify_on_trigger_guard_blocked,
            notify_on_execution_floor_blocked,
            notify_on_tp_hit,
            notify_on_sl_hit,
            notify_on_max_price_blocked,
            price_to_beat_guard_notification_seed.as_deref(),
            retry_on_trigger_guard_block,
            retry_on_execution_floor_guard_block,
            retry_on_max_price_block,
        )
        .await?;
    let initial_status = if side == "sell" && kind == "immediate" {
        "triggered"
    } else {
        "pending"
    };
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
        json!(ignored_existing_order.as_ref().and_then(|(id, _)| *id)),
    );
    flow_created_payload.insert(
        "ignored_existing_order_reason".to_string(),
        json!(ignored_existing_order.as_ref().map(|(_, reason)| *reason)),
    );
    flow_created_payload.insert("protection".to_string(), protection_output.clone());
    flow_created_payload.insert("tp_enabled".to_string(), json!(tp_enabled));
    flow_created_payload.insert("tp_price".to_string(), json!(tp_price));
    flow_created_payload.insert("sl_enabled".to_string(), json!(sl_enabled));
    flow_created_payload.insert("sl_price".to_string(), json!(sl_price));
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
    flow_created_payload.insert("notify_on_fill".to_string(), json!(notify_on_fill));
    flow_created_payload.insert(
        "notify_on_order_not_filled".to_string(),
        json!(notify_on_order_not_filled),
    );
    flow_created_payload.insert(
        "notify_on_trigger_guard_blocked".to_string(),
        json!(notify_on_trigger_guard_blocked),
    );
    flow_created_payload.insert(
        "notify_on_execution_floor_blocked".to_string(),
        json!(notify_on_execution_floor_blocked),
    );
    flow_created_payload.insert(
        "retry_on_trigger_guard_block".to_string(),
        json!(retry_on_trigger_guard_block),
    );
    flow_created_payload.insert(
        "retry_on_execution_floor_guard_block".to_string(),
        json!(retry_on_execution_floor_guard_block),
    );
    flow_created_payload.insert(
        "retry_on_max_price_block".to_string(),
        json!(retry_on_max_price_block),
    );
    flow_created_payload.insert("notify_on_tp_hit".to_string(), json!(notify_on_tp_hit));
    flow_created_payload.insert("notify_on_sl_hit".to_string(), json!(notify_on_sl_hit));
    flow_created_payload.insert(
        "notify_on_max_price_blocked".to_string(),
        json!(notify_on_max_price_blocked),
    );
    flow_created_payload.insert(
        "last_guard_notification_reason".to_string(),
        json!(price_to_beat_guard_notification_seed.clone()),
    );
    flow_created_payload.insert(
        "should_inline_submit".to_string(),
        json!(should_inline_submit),
    );
    repo.append_trade_builder_order_event(
        builder_order_id,
        "flow_created",
        &Value::Object(flow_created_payload),
    )
    .await?;

    set_flow_ref(context, &ref_key, json!(builder_order_id));
    set_flow_ref(context, &node.key, json!(builder_order_id));

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
    output.insert("notify_on_fill".to_string(), json!(notify_on_fill));
    output.insert(
        "notify_on_order_not_filled".to_string(),
        json!(notify_on_order_not_filled),
    );
    output.insert(
        "notify_on_trigger_guard_blocked".to_string(),
        json!(notify_on_trigger_guard_blocked),
    );
    output.insert(
        "notify_on_execution_floor_blocked".to_string(),
        json!(notify_on_execution_floor_blocked),
    );
    output.insert(
        "retry_on_trigger_guard_block".to_string(),
        json!(retry_on_trigger_guard_block),
    );
    output.insert(
        "retry_on_execution_floor_guard_block".to_string(),
        json!(retry_on_execution_floor_guard_block),
    );
    output.insert(
        "retry_on_max_price_block".to_string(),
        json!(retry_on_max_price_block),
    );
    output.insert("notify_on_tp_hit".to_string(), json!(notify_on_tp_hit));
    output.insert("notify_on_sl_hit".to_string(), json!(notify_on_sl_hit));
    output.insert(
        "last_guard_notification_reason".to_string(),
        json!(price_to_beat_guard_notification_seed.clone()),
    );
    output.insert(
        "should_inline_submit".to_string(),
        json!(should_inline_submit),
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
