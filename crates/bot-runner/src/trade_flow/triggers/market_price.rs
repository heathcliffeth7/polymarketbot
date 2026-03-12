async fn execute_trigger_market_price(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    client: Option<&dyn OrderExecutor>,
    ws: &ClobWsClient,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let var_key = node_config_string(node, "varKey").unwrap_or_else(|| node.key.clone());
    let interval_ms = node_config_i64(node, "minIntervalMs")
        .or_else(|| node_config_i64(node, "pollIntervalMs"))
        .unwrap_or(10000)
        .max(250) as i64;
    let repeat_mode = node_repeat_mode(node);
    let once_mode = repeat_mode == "once";
    let once_scope_market = is_trade_flow_market_price_once_scope_market(node);
    let auto_scope_mode = node_market_mode(node) == "auto_scope";
    let price_mode = WsPriceMode::parse(
        node.config.get("priceMode").and_then(|v| v.as_str()),
        WsPriceMode::Composite,
    );
    // --- Early WS-sourced detection for auto_scope guard ---
    let ws_sourced = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("triggerSource"))
        .and_then(|v| v.as_str())
        == Some("ws_market_price");
    let ws_market_slug_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| {
            input
                .get("wsMarketSlug")
                .or_else(|| input.get("marketSlug"))
        })
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string);

    // Skip WS-sourced steps for expired auto_scope markets
    if ws_sourced && auto_scope_mode {
        if let Some(ref ws_slug) = ws_market_slug_from_step {
            if is_auto_scope_market_expired(ws_slug, 30) {
                let output = json!({
                    "run_id": run.id,
                    "node_key": node.key,
                    "pass": false,
                    "reason": "market_expired",
                    "ws_market_slug": ws_slug
                });
                return Ok(TradeFlowNodeExecution {
                    output,
                    routes: Vec::new(),
                    repeat_at: None,
                    repeat_idempotency_key: None,
                });
            }
        }
    }

    let mut market_slug = node_config_string(node, "marketSlug")
        .or_else(|| flow_context_string(context, "marketSlug"))
        .unwrap_or_default();
    if auto_scope_mode && !(ws_sourced && ws_market_slug_from_step.is_some()) {
        match sync_trigger_market_auto_scope_context(cfg, node, context).await? {
            Some(selected) => {
                market_slug = selected.slug;
            }
            None => {
                let output = json!({
                    "run_id": run.id,
                    "node_key": node.key,
                    "pass": false,
                    "once_mode": once_mode,
                    "once_scope": if once_scope_market { "market" } else { "run" },
                    "market_mode": "auto_scope",
                    "market_scope": node_config_string(node, "marketScope")
                        .or_else(|| flow_context_string(context, "marketScope")),
                    "error": "market_not_found"
                });
                let repeat_at = if once_mode {
                    None
                } else {
                    Some(Utc::now() + ChronoDuration::milliseconds(interval_ms))
                };
                return Ok(TradeFlowNodeExecution {
                    output,
                    routes: Vec::new(),
                    repeat_at,
                    repeat_idempotency_key: None,
                });
            }
        }
    }
    // For WS-sourced auto_scope steps, use the step's market slug directly
    // instead of re-resolving from Gamma API (which may return a newer market)
    if ws_sourced && auto_scope_mode {
        if let Some(ref ws_slug) = ws_market_slug_from_step {
            market_slug = ws_slug.clone();
        }
    }
    if market_slug.trim().is_empty() {
        return Err(anyhow::anyhow!("trigger.market_price requires marketSlug"));
    }
    set_flow_context(context, "marketSlug", json!(market_slug.clone()));
    let trigger_protection_mode = normalize_trigger_protection_mode(
        node.config.get("protectionMode").and_then(Value::as_str),
    );
    if trigger_protection_mode == TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM
        && node_market_mode(node) == "auto_scope"
    {
        set_flow_context(context, "underlyingProtection", Value::Null);
        if let Some(asset) = resolve_auto_scope_underlying_asset(node, context, Some(&market_slug))
        {
            if let Err(err) = UNDERLYING_REFERENCE_SERVICE.prime(&asset).await {
                debug!(
                    flow_run_id = run.id,
                    node_key = %node.key,
                    asset = %asset,
                    error = %err,
                    "TRIGGER_UNDERLYING_PRIME_FAILED"
                );
            }
        }
    }
    sync_trade_flow_market_price_once_scope_state(
        context,
        &node.key,
        once_scope_market,
        Some(market_slug.as_str()),
    );

    // --- WS-sourced step data (ws_sourced already computed above) ---
    let trigger_source = if ws_sourced {
        Some("ws_market_price")
    } else {
        None
    };
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
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    let ws_price_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPrice"))
        .and_then(value_as_f64)
        .map(clamp_probability);
    let ws_previous_price_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPreviousPrice"))
        .and_then(value_as_f64)
        .map(clamp_probability);
    let ws_previous_price_present = step.input_json.as_ref().is_some_and(|input| input.get("wsPreviousPrice").is_some());
    let ws_boundary_open_source = step.input_json.as_ref().and_then(|input| input.get("windowBoundaryOpen")).and_then(Value::as_bool).unwrap_or(false);
    // Prevalidated WS paths (for example cross_confirmed) must bypass strict cross re-check.
    let ws_evaluation_mode_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsEvaluationMode"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let ws_price_mode_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPriceMode"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let ws_price_source_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPriceSource"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let ws_price_source_detail_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPriceSourceDetail"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let ws_best_bid_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsBestBid"))
        .and_then(value_as_f64);
    let ws_best_ask_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsBestAsk"))
        .and_then(value_as_f64);
    let ws_last_trade_price_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsLastTradePrice"))
        .and_then(value_as_f64);
    let ws_snapshot_age_ms_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsSnapshotAgeMs"))
        .and_then(value_as_i64);
    let ws_site_display_mode_decision_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsSiteDisplayModeDecision"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let queued_at_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("queuedAt").or_else(|| input.get("queued_at")))
        .and_then(Value::as_str);
    let mut ws_cross_confirmed_short_circuit_applied = false;
    // ws_market_slug_from_step already computed above
    if let Some(ws_market_slug) = ws_market_slug_from_step.as_deref() {
        if should_accept_ws_market_slug_override(node, &market_slug) {
            market_slug = ws_market_slug.to_string();
            set_flow_context(context, "marketSlug", json!(market_slug.clone()));
            sync_trade_flow_market_price_once_scope_state(
                context,
                &node.key,
                once_scope_market,
                Some(market_slug.as_str()),
            );
        }
    }
    if once_mode
        && trade_flow_market_price_once_fired_for_scope(
            context,
            &node.key,
            once_scope_market,
            Some(market_slug.as_str()),
        )
    {
        if !flow_node_state_truthy(context, &node.key, FLOW_NODE_STATE_ONCE_BLOCK_LOGGED) {
            set_flow_node_state(
                context,
                &node.key,
                FLOW_NODE_STATE_ONCE_BLOCK_LOGGED,
                json!(true),
            );
            record_trigger_once_blocked_event(
                repo,
                run,
                node,
                "execute_once_fired_guard",
                once_scope_market,
                &market_slug,
                trigger_source,
                None,
                None,
            )
            .await;
        }
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "run_id": run.id,
                "node_key": node.key,
                "market_slug": market_slug,
                "pass": false,
                "once_mode": true,
                "once_scope": if once_scope_market { "market" } else { "run" },
                "once_fired": true,
                "once_blocked": true,
                "trigger_source": trigger_source
            }),
            routes: Vec::new(),
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }

    // --- Multi-outcome conditions (outcomeConditions array) ---
    let outcome_conditions = node
        .config
        .get("outcomeConditions")
        .and_then(|v| v.as_array())
        .cloned();

    let mut triggered_token_id = String::new();
    let mut triggered_outcome_label = String::new();
    let mut triggered_condition = String::new();
    let mut triggered_trigger_price: Option<f64> = None;
    let mut triggered_price: Option<f64> = None;
    let mut triggered_max_price: Option<f64> = None;
    let mut current_price: Option<f64> = None;
    let mut triggered_previous_price: Option<f64> = None;
    let mut effective_previous_price: Option<f64> = None;
    let mut triggered_poly_delta_10s_cent: Option<f64> = None;
    let mut trigger_evaluation_mode: &'static str = "not_evaluated";
    let mut ws_hard_ignore_reason: Option<String> = None;
    let mut ws_soft_ignore_reason: Option<String> = None;
    let mut pass: bool;

    if ws_sourced {
        match ws_market_slug_from_step.as_deref() {
            Some(ws_market_slug) if ws_market_slug == market_slug => {}
            Some(ws_market_slug) => {
                ws_hard_ignore_reason = Some(format!(
                    "ws_market_slug_mismatch:{ws_market_slug}!={market_slug}"
                ));
            }
            None => {
                ws_hard_ignore_reason = Some("ws_market_slug_missing".to_string());
            }
        }
    }
    let ws_first_tick_threshold_override = should_allow_ws_first_tick_threshold_override(
        ws_sourced,
        &node.node_type,
        node_market_mode(node) == "auto_scope",
        ws_evaluation_mode_from_step,
        ws_boundary_open_source,
        ws_hard_ignore_reason.as_deref(),
    );

    if should_apply_ws_cross_confirmed_short_circuit(
        ws_sourced,
        ws_evaluation_mode_from_step,
        ws_hard_ignore_reason.as_deref(),
    ) {
        let conf_token_id = ws_token_id_from_step.clone().unwrap_or_default();
        let conf_price = ws_price_from_step;
        let conf_prev = ws_previous_price_from_step;

        let (conf_condition, conf_trigger_price, conf_outcome_label) =
            if let Some(ref conditions) = outcome_conditions {
                conditions
                    .iter()
                    .find_map(|cond| {
                        let mut tid = cond
                            .get("tokenId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        let ol = cond
                            .get("outcomeLabel")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        if node_market_mode(node) == "auto_scope" && !ol.is_empty() {
                            tid = resolve_token_id_for_outcome_label(&ol, context).unwrap_or(tid);
                        }
                        if !conf_token_id.is_empty() && tid != conf_token_id {
                            return None;
                        }
                        let tc = cond
                            .get("triggerCondition")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        let tp = cond
                            .get("triggerPriceCent")
                            .and_then(value_as_f64)
                            .map(|v| v / 100.0)
                            .or_else(|| cond.get("triggerPrice").and_then(value_as_f64));
                        Some((tc, tp, ol))
                    })
                    .unwrap_or_default()
            } else {
                let tc = node_config_string(node, "triggerCondition").unwrap_or_default();
                let tp = node_config_f64(node, "triggerPrice")
                    .or_else(|| node_config_f64(node, "triggerPriceCent").map(|v| v / 100.0));
                (tc, tp, String::new())
            };

        triggered_token_id = conf_token_id;
        triggered_outcome_label = conf_outcome_label;
        triggered_condition = conf_condition;
        triggered_trigger_price = conf_trigger_price;
        triggered_price = conf_price;
        current_price = conf_price;
        triggered_previous_price = conf_prev;
        effective_previous_price = conf_prev;
        trigger_evaluation_mode = "cross_confirmed";
        pass = true;
        ws_cross_confirmed_short_circuit_applied = true;
        if let Some(price) = conf_price {
            if !triggered_token_id.is_empty() {
                triggered_poly_delta_10s_cent = record_trigger_price_sample(
                    context,
                    &node.key,
                    &triggered_token_id,
                    price,
                    Utc::now(),
                );
            }
        }

        record_trigger_ws_cross_confirmed_applied_event(
            repo,
            run,
            node,
            &market_slug,
            price_mode,
            ws_market_slug_from_step.clone(),
            ws_token_id_from_step.clone(),
            ws_price_from_step,
            ws_previous_price_from_step,
            ws_evaluation_mode_from_step,
            ws_price_mode_from_step,
            ws_price_source_from_step,
            once_mode,
            once_scope_market,
        )
        .await;

        if let Some(price) = conf_price {
            if !triggered_token_id.is_empty() {
                let per_token_key = format!("previous_price_{}", triggered_token_id);
                set_flow_node_state(context, &node.key, &per_token_key, json!(price));
            }
        }
    } else if let Some(ref conditions) = outcome_conditions {
        // Multi-outcome: OR logic
        pass = false;
        let mut last_eval_mode = "not_evaluated";
        for cond in conditions {
            let mut cond_token_id = cond
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
            if node_market_mode(node) == "auto_scope" && !cond_outcome_label.is_empty() {
                cond_token_id = resolve_token_id_for_outcome_label(&cond_outcome_label, context)
                    .unwrap_or(cond_token_id);
            }
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
            let step_ws_price = ws_prices_map
                .and_then(|m| m.get(&cond_token_id))
                .and_then(value_as_f64)
                .map(clamp_probability);
            if ws_sourced && step_ws_price.is_none() {
                if ws_soft_ignore_reason.is_none() {
                    ws_soft_ignore_reason =
                        Some(format!("ws_price_missing_for_token:{cond_token_id}"));
                }
                continue;
            }
            let cur_result = if let Some(sp) = step_ws_price {
                Ok(sp)
            } else {
                fetch_trade_flow_market_price(
                    ws,
                    client,
                    &market_slug,
                    Some(cond_token_id.as_str()),
                    price_mode,
                    Some(cond_trigger_condition.as_str()),
                )
                .await
            };
            let cur = match cur_result {
                Ok(price) => price,
                Err(err) => {
                    if repeat_mode == "once" {
                        return Ok(TradeFlowNodeExecution {
                            output: json!({
                                "run_id": run.id,
                                "node_key": node.key,
                                "market_slug": market_slug,
                                "error": err.to_string(),
                                "retry": false
                            }),
                            routes: Vec::new(),
                            repeat_at: None,
                            repeat_idempotency_key: None,
                        });
                    }
                    let retry_ms = interval_ms.max(5000);
                    let repeat_at = Utc::now() + ChronoDuration::milliseconds(retry_ms);
                    return Ok(TradeFlowNodeExecution {
                        output: json!({
                            "run_id": run.id,
                            "node_key": node.key,
                            "market_slug": market_slug,
                            "error": err.to_string(),
                            "retry": true,
                            "retry_at": repeat_at.to_rfc3339()
                        }),
                        routes: Vec::new(),
                        repeat_at: Some(repeat_at),
                        repeat_idempotency_key: None,
                    });
                }
            };
            // Store per-token previous price
            set_flow_node_state(context, &node.key, &prev_state_key, json!(cur));
            let poly_delta_10s_cent =
                record_trigger_price_sample(context, &node.key, &cond_token_id, cur, Utc::now());
            let allow_first_tick = !once_mode || ws_first_tick_threshold_override;
            let (pass_this, eval_mode) = evaluate_trigger_market_price_condition(
                prev,
                cur,
                tp,
                &cond_trigger_condition,
                allow_first_tick,
                cond_max_price,
            );
            last_eval_mode = eval_mode;
            if ws_sourced && !pass_this && ws_soft_ignore_reason.is_none() {
                ws_soft_ignore_reason = Some(format!(
                    "ws_condition_not_met:{cond_token_id}:{cond_trigger_condition}:{tp:.6}"
                ));
            }
            if pass_this && !pass {
                pass = true;
                triggered_token_id = cond_token_id;
                triggered_outcome_label = cond_outcome_label;
                triggered_condition = cond_trigger_condition;
                triggered_trigger_price = Some(tp);
                triggered_price = Some(cur);
                triggered_max_price = cond_max_price;
                current_price = Some(cur);
                triggered_previous_price = prev;
                trigger_evaluation_mode = eval_mode;
                triggered_poly_delta_10s_cent = poly_delta_10s_cent;
            }
        }
        if !pass {
            trigger_evaluation_mode = last_eval_mode;
        }
    } else {
        // Legacy single-token path
        let token_id = node_config_string(node, "tokenId")
            .or_else(|| flow_context_string(context, "tokenId"))
            .or_else(|| {
                if node_market_mode(node) != "auto_scope" {
                    return None;
                }
                let outcome = node_config_string(node, "outcomeLabel")
                    .or_else(|| flow_context_string(context, "outcomeLabel"))?;
                resolve_token_id_for_outcome_label(&outcome, context)
            });
        let legacy_outcome_label = node_config_string(node, "outcomeLabel")
            .or_else(|| flow_context_string(context, "outcomeLabel"))
            .unwrap_or_default();
        let trigger_condition = node_config_string(node, "triggerCondition");
        let cur_result = if let Some(sp) = ws_price_from_step {
            Ok(Some(sp))
        } else if ws_sourced {
            if ws_soft_ignore_reason.is_none() {
                ws_soft_ignore_reason = Some("ws_price_missing".to_string());
            }
            Ok(None)
        } else {
            fetch_trade_flow_market_price(
                ws,
                client,
                &market_slug,
                token_id.as_deref(),
                price_mode,
                trigger_condition.as_deref(),
            )
                .await
                .map(Some)
        };
        let cur = match cur_result {
            Ok(price) => price,
            Err(err) => {
                if repeat_mode == "once" {
                    return Ok(TradeFlowNodeExecution {
                        output: json!({
                            "run_id": run.id,
                            "node_key": node.key,
                            "market_slug": market_slug,
                            "error": err.to_string(),
                            "retry": false
                        }),
                        routes: Vec::new(),
                        repeat_at: None,
                        repeat_idempotency_key: None,
                    });
                }
                let retry_ms = interval_ms.max(5000);
                let repeat_at = Utc::now() + ChronoDuration::milliseconds(retry_ms);
                return Ok(TradeFlowNodeExecution {
                    output: json!({
                        "run_id": run.id,
                        "node_key": node.key,
                        "market_slug": market_slug,
                        "error": err.to_string(),
                        "retry": true,
                        "retry_at": repeat_at.to_rfc3339()
                    }),
                    routes: Vec::new(),
                    repeat_at: Some(repeat_at),
                    repeat_idempotency_key: None,
                });
            }
        };
        current_price = cur;
        if let Some(cur_price) = cur {
            set_flow_var(context, &var_key, json!(cur_price));
            set_flow_node_state(context, &node.key, "last_price", json!(cur_price));
        }

        let trigger_price = node_config_f64(node, "triggerPrice")
            .or_else(|| node_config_f64(node, "triggerPriceCent").map(|v| v / 100.0));
        // PRCE-01: Use per-token key to avoid cross-token price contamination.
        // Returns None when token_id is empty/missing — safe because resolve_ws_previous_price
        // handles None state_previous_price correctly (falls through to ws payload or returns None).
        let state_previous_price = token_id
            .as_deref()
            .filter(|v| !v.is_empty())
            .map(|tid| format!("previous_price_{}", tid))
            .and_then(|key| flow_node_state(context, &node.key, &key))
            .and_then(value_as_f64);
        let expected_token_id = token_id.as_deref().filter(|value| !value.is_empty());
        let previous_price = resolve_ws_previous_price(
            ws_sourced,
            state_previous_price,
            expected_token_id.unwrap_or_default(),
            ws_token_id_from_step.as_deref(),
            ws_previous_price_from_step,
            ws_previous_price_present,
            ws_previous_prices_map,
        );
        effective_previous_price = previous_price;
        let allow_first_tick = !once_mode || ws_first_tick_threshold_override;
        let legacy_max_price = node_config_f64(node, "maxPrice")
            .or_else(|| node_config_f64(node, "maxPriceCent").map(|v| v / 100.0))
            .filter(|v| *v > 0.0 && *v <= 1.0);
        let legacy_poly_delta_10s_cent =
            match (cur, token_id.as_deref().filter(|value| !value.is_empty())) {
                (Some(cur_price), Some(tid)) => {
                    record_trigger_price_sample(context, &node.key, tid, cur_price, Utc::now())
                }
                _ => None,
            };
        triggered_max_price = legacy_max_price;
        pass = if let Some(cur_price) = cur {
            match (trigger_condition.as_deref(), trigger_price) {
                (Some("cross_above"), Some(tp)) => {
                    let (matched, eval_mode) = evaluate_trigger_market_price_condition(
                        previous_price,
                        cur_price,
                        tp,
                        "cross_above",
                        allow_first_tick,
                        legacy_max_price,
                    );
                    trigger_evaluation_mode = eval_mode;
                    matched
                }
                (Some("cross_below"), Some(tp)) => {
                    let (matched, eval_mode) = evaluate_trigger_market_price_condition(
                        previous_price,
                        cur_price,
                        tp,
                        "cross_below",
                        allow_first_tick,
                        legacy_max_price,
                    );
                    trigger_evaluation_mode = eval_mode;
                    matched
                }
                _ => {
                    trigger_evaluation_mode = "unsupported_condition";
                    false
                }
            }
        } else {
            trigger_evaluation_mode = "ws_missing_price";
            false
        };
        if let Some(cur_price) = cur {
            // PRCE-01: Write only per-token key. Bare "previous_price" write removed —
            // nothing reads it after the per-token read-side fix.
            if let Some(ref tid) = token_id {
                if !tid.is_empty() {
                    let per_token_key = format!("previous_price_{}", tid);
                    set_flow_node_state(context, &node.key, &per_token_key, json!(cur_price));
                }
            }
        }
        if ws_sourced {
            if let Some(expected_token_id) = token_id.as_deref() {
                if expected_token_id.is_empty() {
                    if ws_hard_ignore_reason.is_none() {
                        ws_hard_ignore_reason = Some("ws_expected_token_missing".to_string());
                    }
                } else if ws_token_id_from_step.as_deref() != Some(expected_token_id) {
                    if ws_hard_ignore_reason.is_none() {
                        ws_hard_ignore_reason = Some(format!(
                            "ws_token_mismatch:{}!={expected_token_id}",
                            ws_token_id_from_step.as_deref().unwrap_or("missing")
                        ));
                    }
                }
            }
            if !pass && ws_soft_ignore_reason.is_none() {
                ws_soft_ignore_reason = Some("ws_condition_not_met".to_string());
            }
        }
        triggered_token_id = token_id.unwrap_or_default();
        triggered_outcome_label = legacy_outcome_label;
        triggered_condition = trigger_condition.unwrap_or_default();
        triggered_trigger_price = trigger_price;
        triggered_price = current_price;
        triggered_previous_price = previous_price;
        if pass {
            triggered_poly_delta_10s_cent = legacy_poly_delta_10s_cent;
        }
    }

    let ws_ignore_reason = if ws_sourced {
        if let Some(reason) = ws_hard_ignore_reason.clone() {
            pass = false;
            Some(reason)
        } else if pass {
            None
        } else {
            ws_soft_ignore_reason.clone()
        }
    } else {
        None
    };
    if let Some(reason) = ws_ignore_reason.as_deref() {
        record_trigger_ws_price_ignored_event(
            repo,
            run,
            node,
            reason,
            price_mode,
            trigger_source,
            &market_slug,
            ws_market_slug_from_step.clone(),
            ws_token_id_from_step.clone(),
            &triggered_token_id,
            ws_price_from_step,
            ws_previous_price_from_step,
            ws_price_mode_from_step,
            ws_price_source_from_step,
            effective_previous_price,
        )
        .await;
    }
    if is_ws_cross_confirmed_unexpected_fail(
        ws_sourced,
        ws_evaluation_mode_from_step,
        pass,
        ws_hard_ignore_reason.as_deref(),
    ) {
        record_trigger_ws_cross_confirmed_unexpected_fail_event(
            repo,
            run,
            node,
            &market_slug,
            price_mode,
            ws_market_slug_from_step.clone(),
            ws_token_id_from_step.clone(),
            ws_price_from_step,
            ws_previous_price_from_step,
            ws_evaluation_mode_from_step,
            ws_price_mode_from_step,
            ws_price_source_from_step,
            trigger_evaluation_mode,
            ws_ignore_reason.clone(),
            effective_previous_price,
            ws_cross_confirmed_short_circuit_applied,
        )
        .await;
    }
    let cycle_window_mode = node
        .config
        .get("cycleWindowMode")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| value == "first" || value == "last");
    let cycle_window_secs = node_config_i64(node, "cycleWindowSecs").filter(|value| *value > 0);
    let (cycle_window_open_at, cycle_window_end_at) =
        match (cycle_window_mode.as_deref(), cycle_window_secs) {
            (Some(mode), Some(window_secs)) => resolve_cycle_window_absolute_bounds(
                &market_slug,
                mode,
                window_secs,
            )
            .map_or((None, None), |(open_at, end_at)| (Some(open_at), Some(end_at))),
            _ => (None, None),
        };

    apply_trigger_market_price_context_updates(
        context,
        node,
        &var_key,
        current_price,
        &triggered_token_id,
        &triggered_outcome_label,
        &triggered_condition,
        triggered_trigger_price,
        triggered_price,
        triggered_max_price,
        cycle_window_mode.as_deref(),
        cycle_window_secs,
        cycle_window_open_at.clone(),
        cycle_window_end_at.clone(),
        pass,
    );
    let mut protection_output = Value::Null;
    if pass {
        if let Some(protection_config) = build_underlying_protection_config(
            node,
            context,
            &market_slug,
            &triggered_outcome_label,
        ) {
            let protection = evaluate_underlying_protection(
                &protection_config,
                &market_slug,
                triggered_poly_delta_10s_cent,
            )
            .await;
            protection_output = protection.to_value();
            set_flow_context(context, "underlyingProtection", protection_output.clone());
            record_trigger_protection_event(
                repo,
                run,
                node,
                protection.passed,
                &market_slug,
                &triggered_token_id,
                &triggered_outcome_label,
                &triggered_condition,
                triggered_price,
                triggered_max_price,
                triggered_poly_delta_10s_cent,
                &protection_output,
            )
            .await;
            if !protection.passed {
                pass = false;
            }
        }
    }
    if once_mode && pass {
        let once_fire_key = trade_flow_market_price_once_idempotency_key(
            run.id,
            &node.key,
            once_scope_market,
            Some(market_slug.as_str()),
        );
        if repo.try_record_idempotency_key(&once_fire_key).await? {
            let fired_at = Utc::now();
            let cycle_window_diagnostics = cycle_window_followup_diagnostics_from_context(
                context,
                &node.key,
                &triggered_token_id,
                fired_at,
            );
            mark_trade_flow_market_price_once_fired(
                context,
                &node.key,
                fired_at,
                once_scope_market.then_some(market_slug.as_str()),
            );
            record_trigger_once_fired_event(
                repo,
                run,
                node,
                &market_slug,
                price_mode,
                &triggered_token_id,
                &triggered_outcome_label,
                &triggered_condition,
                triggered_price,
                triggered_max_price,
                triggered_previous_price,
                triggered_poly_delta_10s_cent,
                &protection_output,
                trigger_evaluation_mode,
                current_price,
                ws_sourced,
                ws_price_mode_from_step,
                ws_price_source_from_step,
                once_scope_market,
                fired_at,
                &once_fire_key,
                cycle_window_diagnostics.as_ref(),
            )
            .await;
        } else {
            let already_block_logged =
                flow_node_state_truthy(context, &node.key, FLOW_NODE_STATE_ONCE_BLOCK_LOGGED);
            if !trade_flow_market_price_once_fired_for_scope(
                context,
                &node.key,
                once_scope_market,
                Some(market_slug.as_str()),
            ) {
                mark_trade_flow_market_price_once_fired(
                    context,
                    &node.key,
                    Utc::now(),
                    once_scope_market.then_some(market_slug.as_str()),
                );
            }
            set_flow_node_state(
                context,
                &node.key,
                FLOW_NODE_STATE_ONCE_BLOCK_LOGGED,
                json!(true),
            );
            pass = false;

            if !already_block_logged {
                record_trigger_once_blocked_event(
                    repo,
                    run,
                    node,
                    "global_once_idempotency",
                    once_scope_market,
                    &market_slug,
                    trigger_source,
                    Some(ws_sourced),
                    Some(&once_fire_key),
                )
                .await;
            }
        }
    }

    Ok(finish_trigger_market_price_execution(
        run, node, context, &market_slug, price_mode, &triggered_token_id,
        &triggered_outcome_label, &triggered_condition, triggered_trigger_price,
        triggered_price, triggered_max_price, &protection_output, triggered_previous_price,
        ws_previous_price_from_step, effective_previous_price, trigger_evaluation_mode,
        ws_evaluation_mode_from_step, ws_price_mode_from_step, ws_price_source_from_step,
        ws_price_source_detail_from_step, ws_best_bid_from_step, ws_best_ask_from_step,
        ws_last_trade_price_from_step, ws_snapshot_age_ms_from_step,
        ws_site_display_mode_decision_from_step, ws_cross_confirmed_short_circuit_applied,
        current_price, pass, &var_key, &outcome_conditions, ws_sourced, ws_ignore_reason.clone(),
        once_mode, once_scope_market, queued_at_from_step, cycle_window_mode.as_deref(),
        cycle_window_secs, cycle_window_open_at, cycle_window_end_at, interval_ms,
    ))
}
