
async fn execute_trigger_sell_progress(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let source_trade_id = resolve_flow_source_trade_id(node, context)
        .ok_or_else(|| anyhow::anyhow!("trigger.sell_progress requires sourceTradeId"))?;
    let (filled_notional_usdc, _) = repo.aggregate_trade_fills(source_trade_id).await?;
    let target_notional_usdc = repo
        .trade_notional_usdc(source_trade_id)
        .await?
        .unwrap_or(0.0);

    let progress = if target_notional_usdc > 0.0 {
        ((filled_notional_usdc / target_notional_usdc) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    let var_key =
        node_config_string(node, "varKey").unwrap_or_else(|| "sell_progress_pct".to_string());
    set_flow_var(context, &var_key, json!(progress));
    set_flow_node_state(context, &node.key, "last_progress_pct", json!(progress));

    let min_progress = node_config_f64(node, "minProgressPct");
    let pass = min_progress.map(|v| progress >= v).unwrap_or(true);
    let interval_ms = node_config_i64(node, "minIntervalMs")
        .unwrap_or(1500)
        .max(250) as i64;

    let routes = if pass {
        vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at: Utc::now(),
        }]
    } else {
        Vec::new()
    };

    let output = json!({
        "run_id": run.id,
        "node_key": node.key,
        "source_trade_id": source_trade_id,
        "filled_notional_usdc": filled_notional_usdc,
        "target_notional_usdc": target_notional_usdc,
        "progress_pct": progress,
        "pass": pass
    });
    Ok(TradeFlowNodeExecution {
        output,
        routes,
        repeat_at: Some(Utc::now() + ChronoDuration::milliseconds(interval_ms)),
        repeat_idempotency_key: None,
    })
}

async fn execute_trigger_open_positions(
    repo: &PostgresRepository,
    client: Option<&dyn OrderExecutor>,
    ws: &ClobWsClient,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let source_trade_id = resolve_flow_source_trade_id(node, context)
        .ok_or_else(|| anyhow::anyhow!("trigger.open_positions requires sourceTradeId"))?;
    let positions = repo.load_leg_positions(source_trade_id).await?;

    let mut qty_yes = 0.0_f64;
    let mut qty_no = 0.0_f64;
    for leg in &positions {
        match leg.leg_side {
            LegSide::Yes => qty_yes += leg.qty,
            LegSide::No => qty_no += leg.qty,
        }
    }
    let qty_total = qty_yes + qty_no;

    let min_position_qty = node_config_f64(node, "minPositionQty")
        .unwrap_or(0.0)
        .max(0.0);
    let exists = qty_total > 0.0;
    let qty_pass = qty_total >= min_position_qty;
    let interval_ms = node_config_i64(node, "minIntervalMs")
        .unwrap_or(3000)
        .max(250) as i64;

    let var_prefix = node_config_string(node, "varPrefix")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| node.key.clone());

    let market_slug = node_config_string(node, "marketSlug")
        .or_else(|| flow_context_string(context, "marketSlug"))
        .unwrap_or_default();

    // --- Multi-outcome conditions (outcomeConditions array) ---
    let outcome_conditions = node
        .config
        .get("outcomeConditions")
        .and_then(|v| v.as_array())
        .cloned();

    let ws_price_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPrice"))
        .and_then(value_as_f64)
        .map(clamp_probability);
    // Multi-outcome ws prices: input may carry wsPrices: { "tokenId": price, ... }
    let ws_prices_map: Option<&serde_json::Map<String, Value>> = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPrices"))
        .and_then(|v| v.as_object());
    let ws_previous_prices_map: Option<&serde_json::Map<String, Value>> = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPreviousPrices"))
        .and_then(|v| v.as_object());
    let ws_token_id_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("tokenId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let ws_previous_price_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPreviousPrice"))
        .and_then(value_as_f64)
        .map(clamp_probability);
    let ws_previous_price_present = step
        .input_json
        .as_ref()
        .map(|input| input.get("wsPreviousPrice").is_some())
        .unwrap_or(false);
    let ws_sourced = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("triggerSource"))
        .and_then(Value::as_str)
        == Some("ws_market_price");

    // Shared mutable state for triggered outcome
    let mut triggered_token_id = String::new();
    let mut triggered_outcome_label = String::new();
    let mut triggered_condition = String::new();
    let mut triggered_price: Option<f64> = None;
    let mut triggered_max_price: Option<f64> = None;
    let mut current_price: Option<f64> = None;
    let mut effective_previous_price: Option<f64> = None;
    let mut price_pass = false;
    let mut websocket_price_mode = false;

    if let Some(ref conditions) = outcome_conditions {
        // Multi-outcome path: OR logic
        anyhow::ensure!(
            !market_slug.is_empty(),
            "trigger.open_positions with outcomeConditions requires marketSlug"
        );
        for cond in conditions {
            let cond_token_id = cond
                .get("tokenId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let cond_outcome_label = cond
                .get("outcomeLabel")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let cond_trigger_condition = cond
                .get("triggerCondition")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let cond_trigger_price = cond
                .get("triggerPriceCent")
                .and_then(value_as_f64)
                .map(|v| v / 100.0)
                .or_else(|| cond.get("triggerPrice").and_then(value_as_f64));
            let cond_max_price = cond
                .get("maxPriceCent")
                .and_then(value_as_f64)
                .map(|v| v / 100.0)
                .or_else(|| cond.get("maxPrice").and_then(value_as_f64))
                .filter(|v| *v > 0.0 && *v <= 1.0);
            if cond_token_id.is_empty() || cond_trigger_condition.is_empty() {
                continue;
            }
            let tp = match cond_trigger_price {
                Some(v) => v,
                None => continue,
            };
            let is_ws_mode = matches!(
                cond_trigger_condition.as_str(),
                "cross_above" | "cross_below"
            );
            if is_ws_mode {
                websocket_price_mode = true;
            }
            let prev_state_key = format!("previous_price_{}", cond_token_id);
            let state_prev =
                flow_node_state(context, &node.key, &prev_state_key).and_then(value_as_f64);
            let prev = resolve_ws_previous_price(
                ws_sourced,
                state_prev,
                cond_token_id.as_str(),
                ws_token_id_from_step.as_deref(),
                ws_previous_price_from_step,
                ws_previous_price_present,
                ws_previous_prices_map,
            );
            effective_previous_price = prev;
            // Get current price for this token
            let step_ws_price = ws_prices_map
                .and_then(|m| m.get(&cond_token_id))
                .and_then(value_as_f64)
                .map(clamp_probability);
            let cur = if let Some(sp) = step_ws_price {
                Some(sp)
            } else if is_ws_mode {
                fetch_price_from_market_ws(ws, &cond_token_id)
                    .await
                    .map(clamp_probability)
            } else {
                Some(
                    fetch_trade_flow_market_price(
                        ws,
                        client,
                        &market_slug,
                        Some(cond_token_id.as_str()),
                        WsPriceMode::Raw,
                    )
                    .await?,
                )
            };
            // Store previous_price per token
            if let Some(p) = cur {
                set_flow_node_state(context, &node.key, &prev_state_key, json!(p));
            }
            let pass_this = match cond_trigger_condition.as_str() {
                "cross_above" => cur
                    .map(|v| {
                        crossed_above_strict(prev, v, tp)
                            && cond_max_price.map_or(true, |mp| v <= mp)
                    })
                    .unwrap_or(false),
                "cross_below" => cur
                    .map(|v| {
                        crossed_below_strict(prev, v, tp)
                            && cond_max_price.map_or(true, |mp| v <= mp)
                    })
                    .unwrap_or(false),
                _ => false,
            };
            if pass_this && !price_pass {
                price_pass = true;
                triggered_token_id = cond_token_id.clone();
                triggered_outcome_label = cond_outcome_label;
                triggered_condition = cond_trigger_condition;
                triggered_price = cur;
                triggered_max_price = cond_max_price;
                current_price = cur;
            }
        }
    } else {
        // Legacy single-token path (backward compatibility)
        let token_id = node_config_string(node, "tokenId")
            .or_else(|| flow_context_string(context, "tokenId"))
            .unwrap_or_default();
        let outcome_label = node_config_string(node, "outcomeLabel")
            .or_else(|| flow_context_string(context, "outcomeLabel"))
            .unwrap_or_default();
        let trigger_condition = node_config_string(node, "triggerCondition");
        let trigger_price = node_config_f64(node, "triggerPrice")
            .or_else(|| node_config_f64(node, "triggerPriceCent").map(|v| v / 100.0));
        let legacy_max_price = node_config_f64(node, "maxPrice")
            .or_else(|| node_config_f64(node, "maxPriceCent").map(|v| v / 100.0))
            .filter(|v| *v > 0.0 && *v <= 1.0);
        websocket_price_mode = matches!(
            trigger_condition.as_deref(),
            Some("cross_above" | "cross_below")
        ) && trigger_price.is_some();
        if trigger_condition.is_some() {
            anyhow::ensure!(
                !market_slug.is_empty(),
                "trigger.open_positions with triggerCondition requires marketSlug"
            );
            anyhow::ensure!(
                !token_id.is_empty(),
                "trigger.open_positions with triggerCondition requires tokenId"
            );
        }
        // PRCE-01: Use per-token key to avoid cross-token price contamination
        let state_previous_price = if !token_id.is_empty() {
            let key = format!("previous_price_{}", token_id);
            flow_node_state(context, &node.key, &key).and_then(value_as_f64)
        } else {
            None
        };
        let token_id_ref = if token_id.is_empty() {
            None
        } else {
            Some(token_id.as_str())
        };
        let previous_price = resolve_ws_previous_price(
            ws_sourced,
            state_previous_price,
            token_id_ref.unwrap_or_default(),
            ws_token_id_from_step.as_deref(),
            ws_previous_price_from_step,
            ws_previous_price_present,
            ws_previous_prices_map,
        );
        effective_previous_price = previous_price;
        let (cur, pass_single) = match (trigger_condition.as_deref(), trigger_price) {
            (Some("cross_above"), Some(tp)) => {
                let cur_val = if let Some(step_price) = ws_price_from_step {
                    Some(step_price)
                } else if websocket_price_mode {
                    if let Some(token) = token_id_ref {
                        fetch_price_from_market_ws(ws, token)
                            .await
                            .map(clamp_probability)
                    } else {
                        None
                    }
                } else {
                    Some(
                        fetch_trade_flow_market_price(
                            ws,
                            client,
                            &market_slug,
                            token_id_ref,
                            WsPriceMode::Raw,
                        )
                        .await?,
                    )
                };
                let p = cur_val
                    .map(|value| {
                        crossed_above_strict(previous_price, value, tp)
                            && legacy_max_price.map_or(true, |mp| value <= mp)
                    })
                    .unwrap_or(false);
                (cur_val, p)
            }
            (Some("cross_below"), Some(tp)) => {
                let cur_val = if let Some(step_price) = ws_price_from_step {
                    Some(step_price)
                } else if websocket_price_mode {
                    if let Some(token) = token_id_ref {
                        fetch_price_from_market_ws(ws, token)
                            .await
                            .map(clamp_probability)
                    } else {
                        None
                    }
                } else {
                    Some(
                        fetch_trade_flow_market_price(
                            ws,
                            client,
                            &market_slug,
                            token_id_ref,
                            WsPriceMode::Raw,
                        )
                        .await?,
                    )
                };
                let p = cur_val
                    .map(|value| {
                        crossed_below_strict(previous_price, value, tp)
                            && legacy_max_price.map_or(true, |mp| value <= mp)
                    })
                    .unwrap_or(false);
                (cur_val, p)
            }
            (Some(other), Some(_)) => {
                return Err(anyhow::anyhow!(
                    "trigger.open_positions triggerCondition must be cross_above/cross_below (got {other})"
                ));
            }
            (Some(_), None) => {
                return Err(anyhow::anyhow!(
                    "trigger.open_positions triggerCondition requires triggerPrice or triggerPriceCent"
                ));
            }
            _ => (None, true),
        };
        current_price = cur;
        price_pass = pass_single;
        triggered_token_id = token_id;
        triggered_outcome_label = outcome_label;
        triggered_condition = trigger_condition.unwrap_or_default();
        triggered_price = cur;
        triggered_max_price = legacy_max_price;
        if let Some(p) = cur {
            // PRCE-01: Write only per-token key. Bare "previous_price" write removed —
            // nothing reads it after the per-token read-side fix.
            if !triggered_token_id.is_empty() {
                let per_token_key = format!("previous_price_{}", triggered_token_id);
                set_flow_node_state(context, &node.key, &per_token_key, json!(p));
            }
        }
    }

    let pass = exists && qty_pass && price_pass;
    if let Some(price) = current_price {
        set_flow_node_state(context, &node.key, "last_price", json!(price));
        set_flow_var(context, &format!("{var_prefix}_price"), json!(price));
    }

    set_flow_var(context, &format!("{var_prefix}_exists"), json!(exists));
    set_flow_var(
        context,
        &format!("{var_prefix}_qty_total"),
        json!(qty_total),
    );
    set_flow_var(context, &format!("{var_prefix}_qty_yes"), json!(qty_yes));
    set_flow_var(context, &format!("{var_prefix}_qty_no"), json!(qty_no));
    set_flow_var(
        context,
        &format!("{var_prefix}_trade_id"),
        json!(source_trade_id),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_market_slug"),
        json!(market_slug),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_token_id"),
        json!(triggered_token_id),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_outcome_label"),
        json!(triggered_outcome_label),
    );
    if !triggered_condition.is_empty() {
        set_flow_var(
            context,
            &format!("{var_prefix}_triggered_condition"),
            json!(triggered_condition),
        );
    }
    if let Some(tp) = triggered_price {
        set_flow_var(context, &format!("{var_prefix}_triggered_price"), json!(tp));
    }
    if let Some(max_price) = triggered_max_price {
        set_flow_var(
            context,
            &format!("{var_prefix}_max_price"),
            json!(max_price),
        );
    }
    if pass {
        if let Some(max_price) = triggered_max_price {
            set_flow_context(context, "maxPrice", json!(max_price));
        } else {
            set_flow_context(context, "maxPrice", Value::Null);
        }
    }
    set_flow_node_state(
        context,
        &node.key,
        "last_position_qty_total",
        json!(qty_total),
    );

    let routes = if pass {
        vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at: Utc::now(),
        }]
    } else {
        Vec::new()
    };

    let output = json!({
        "run_id": run.id,
        "node_key": node.key,
        "source_trade_id": source_trade_id,
        "positions_count": positions.len(),
        "qty_yes": qty_yes,
        "qty_no": qty_no,
        "qty_total": qty_total,
        "min_position_qty": min_position_qty,
        "qty_pass": qty_pass,
        "triggered_condition": triggered_condition,
        "triggered_token_id": triggered_token_id,
        "triggered_outcome_label": triggered_outcome_label,
        "triggered_price": triggered_price,
        "max_price": triggered_max_price,
        "maxPrice": triggered_max_price,
        "websocket_price_mode": websocket_price_mode,
        "ws_sourced": ws_sourced,
        "ws_previous_price": ws_previous_price_from_step,
        "effective_previous_price": effective_previous_price,
        "price": current_price,
        "price_pass": price_pass,
        "exists": exists,
        "pass": pass,
        "multi_outcome": outcome_conditions.is_some()
    });
    let repeat_at = if websocket_price_mode {
        None
    } else {
        Some(Utc::now() + ChronoDuration::milliseconds(interval_ms))
    };

    Ok(TradeFlowNodeExecution {
        output,
        routes,
        repeat_at,
        repeat_idempotency_key: None,
    })
}

