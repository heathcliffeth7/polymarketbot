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
            matches!(condition, "cross_above" | "cross_below"),
            "action.place_order triggerCondition must be cross_above/cross_below"
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
    let max_price = resolve_action_place_order_max_price(context, step);

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
            Some(run.definition_id),
            Some(run.id),
            Some(&node.key),
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
                "remaining_qty": sizing.remaining_qty
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
                "rearmed_existing_order": true
            }),
            routes: Vec::new(),
            repeat_at: None,
            repeat_idempotency_key: None,
        });
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
            sizing.size_basis,
            sizing.size_usdc,
            sizing.target_qty,
            sizing.remaining_qty,
            min_price_distance_cent,
            expires_at,
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
        )
        .await?;
    repo.append_trade_builder_order_event(
        builder_order_id,
        "flow_created",
        &json!({
            "flow_run_id": run.id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "execution_mode": execution_mode,
            "order_type": clob_order_type_for_execution_mode(&execution_mode),
            "initial_status": if side == "sell" && kind == "immediate" { "triggered" } else { "pending" },
            "size_basis": sizing.size_basis,
            "size_mode": sizing.resolved_size_mode,
            "size_pct": sizing.resolved_size_pct,
            "size_usdc": sizing.size_usdc,
            "target_qty": sizing.target_qty,
            "remaining_qty": sizing.remaining_qty,
            "trigger_sizes": trigger_sizes,
            "max_price": max_price,
            "ignored_stale_existing_order": ignored_existing_order.is_some(),
            "ignored_existing_order_id": ignored_existing_order.as_ref().and_then(|(id, _)| *id),
            "ignored_existing_order_reason": ignored_existing_order.as_ref().map(|(_, reason)| *reason),
            "protection": protection_output.clone(),
            "tp_enabled": tp_enabled,
            "tp_price": tp_price,
            "sl_enabled": sl_enabled,
            "sl_price": sl_price
        }),
    )
    .await?;

    set_flow_ref(context, &ref_key, json!(builder_order_id));
    set_flow_ref(context, &node.key, json!(builder_order_id));

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "builder_order_id": builder_order_id,
            "ref_key": ref_key,
            "source_trade_id": source_trade_id,
            "kind": kind,
            "side": side,
            "execution_mode": execution_mode,
            "order_type": clob_order_type_for_execution_mode(&execution_mode),
            "market_slug": market_slug,
            "token_id": token_id,
            "max_price": max_price,
            "protection": protection_output,
            "size_basis": sizing.size_basis,
            "size_mode": sizing.resolved_size_mode,
            "size_pct": sizing.resolved_size_pct,
            "size_usdc": sizing.size_usdc,
            "target_qty": sizing.target_qty,
            "remaining_qty": sizing.remaining_qty,
            "tp_enabled": tp_enabled,
            "tp_price": tp_price,
            "sl_enabled": sl_enabled,
            "sl_price": sl_price
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}
