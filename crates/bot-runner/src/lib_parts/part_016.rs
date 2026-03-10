fn parse_position_drawdown_rules(node: &TradeFlowNode) -> Vec<PositionDrawdownRule> {
    let mut rules = Vec::new();
    if let Some(items) = node.config.get("lossRules").and_then(Value::as_array) {
        for (index, item) in items.iter().enumerate() {
            let Some(obj) = item.as_object() else {
                continue;
            };
            let Some(loss_pct) = obj.get("lossPct").and_then(value_as_f64) else {
                continue;
            };
            if !loss_pct.is_finite() || loss_pct <= 0.0 || loss_pct > 100.0 {
                continue;
            }
            let Some(direction) =
                PositionDrawdownDirection::parse(obj.get("direction").and_then(Value::as_str))
            else {
                continue;
            };
            let window_ms = obj
                .get("windowMs")
                .and_then(value_as_i64)
                .filter(|v| *v > 0);
            rules.push(PositionDrawdownRule {
                index,
                loss_pct,
                direction,
                window_ms,
            });
        }
    }

    // Backward-compatible single-rule fallback.
    if rules.is_empty() {
        if let Some(loss_pct) = node_config_f64(node, "lossPct") {
            if loss_pct.is_finite() && loss_pct > 0.0 && loss_pct <= 100.0 {
                rules.push(PositionDrawdownRule {
                    index: 0,
                    loss_pct,
                    direction: PositionDrawdownDirection::Down,
                    window_ms: node_config_i64(node, "windowMs").filter(|v| *v > 0),
                });
            }
        }
    }

    rules
}

fn has_deprecated_drawdown_window_sec(node: &TradeFlowNode) -> bool {
    if node.config.get("windowSec").is_some() {
        return true;
    }
    node.config
        .get("lossRules")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter().any(|item| {
                item.as_object()
                    .map(|obj| obj.contains_key("windowSec"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn parse_position_drawdown_samples(value: Option<&Value>) -> Vec<PositionDrawdownSample> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut samples = Vec::new();
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let Some(ts_ms) = obj.get("ts_ms").and_then(value_as_i64) else {
            continue;
        };
        let Some(price) = obj.get("price").and_then(value_as_f64) else {
            continue;
        };
        let loss_pct = obj
            .get("loss_pct")
            .and_then(value_as_f64)
            .unwrap_or_default();
        let gain_pct = obj
            .get("gain_pct")
            .and_then(value_as_f64)
            .unwrap_or_default();
        if !price.is_finite() || price < 0.0 {
            continue;
        }
        if !loss_pct.is_finite() || loss_pct < 0.0 {
            continue;
        }
        if !gain_pct.is_finite() || gain_pct < 0.0 {
            continue;
        }
        samples.push(PositionDrawdownSample {
            ts_ms,
            loss_pct: loss_pct.clamp(0.0, 100.0),
            gain_pct: gain_pct.clamp(0.0, 100.0),
            price: clamp_probability(price),
        });
    }
    samples.sort_by_key(|sample| sample.ts_ms);
    samples
}

async fn execute_trigger_position_drawdown(
    _repo: &PostgresRepository,
    ws: &ClobWsClient,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let source_trade_id = resolve_flow_source_trade_id(node, context);
    let market_slug = node_config_string(node, "marketSlug")
        .or_else(|| flow_context_string(context, "marketSlug"))
        .unwrap_or_default();
    let token_id = node_config_string(node, "tokenId")
        .or_else(|| flow_context_string(context, "tokenId"))
        .unwrap_or_default();
    let outcome_label = node_config_string(node, "outcomeLabel")
        .or_else(|| flow_context_string(context, "outcomeLabel"))
        .unwrap_or_default();
    let entry_price = node_config_f64(node, "entryPriceCent")
        .map(|value| value / 100.0)
        .or_else(|| node_config_f64(node, "entryPrice"));

    let interval_ms = node_config_i64(node, "minIntervalMs")
        .unwrap_or(250)
        .max(250) as i64;
    let now = Utc::now();
    let repeat_at = Some(now + ChronoDuration::milliseconds(interval_ms));

    let var_prefix = node_config_string(node, "varPrefix")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| node.key.clone());

    if has_deprecated_drawdown_window_sec(node) {
        let output = json!({
            "run_id": run.id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "reason": "deprecated_window_sec",
            "pass": false
        });
        return Ok(TradeFlowNodeExecution {
            output,
            routes: Vec::new(),
            repeat_at,
            repeat_idempotency_key: None,
        });
    }

    let rules = parse_position_drawdown_rules(node);
    if rules.is_empty() {
        let output = json!({
            "run_id": run.id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "reason": "missing_loss_rules",
            "pass": false
        });
        return Ok(TradeFlowNodeExecution {
            output,
            routes: Vec::new(),
            repeat_at,
            repeat_idempotency_key: None,
        });
    };
    let entry_price = match entry_price {
        Some(value) if value.is_finite() && value > 0.0 && value <= 1.0 => value,
        Some(value) => {
            let output = json!({
                "run_id": run.id,
                "node_key": node.key,
                "source_trade_id": source_trade_id,
                "market_slug": market_slug,
                "token_id": token_id,
                "outcome_label": outcome_label,
                "entry_price": value,
                "reason": "invalid_entry_price",
                "pass": false
            });
            return Ok(TradeFlowNodeExecution {
                output,
                routes: Vec::new(),
                repeat_at,
                repeat_idempotency_key: None,
            });
        }
        None => {
            let output = json!({
                "run_id": run.id,
                "node_key": node.key,
                "source_trade_id": source_trade_id,
                "market_slug": market_slug,
                "token_id": token_id,
                "outcome_label": outcome_label,
                "reason": "entry_price_missing",
                "pass": false
            });
            return Ok(TradeFlowNodeExecution {
                output,
                routes: Vec::new(),
                repeat_at,
                repeat_idempotency_key: None,
            });
        }
    };
    if market_slug.trim().is_empty() {
        let output = json!({
            "run_id": run.id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "entry_price": entry_price,
            "reason": "missing_market_slug",
            "pass": false
        });
        return Ok(TradeFlowNodeExecution {
            output,
            routes: Vec::new(),
            repeat_at,
            repeat_idempotency_key: None,
        });
    }
    if token_id.trim().is_empty() {
        let output = json!({
            "run_id": run.id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "market_slug": market_slug,
            "outcome_label": outcome_label,
            "entry_price": entry_price,
            "reason": "missing_token_id",
            "pass": false
        });
        return Ok(TradeFlowNodeExecution {
            output,
            routes: Vec::new(),
            repeat_at,
            repeat_idempotency_key: None,
        });
    }
    if outcome_label.trim().is_empty() {
        let output = json!({
            "run_id": run.id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "market_slug": market_slug,
            "token_id": token_id,
            "entry_price": entry_price,
            "reason": "missing_outcome_label",
            "pass": false
        });
        return Ok(TradeFlowNodeExecution {
            output,
            routes: Vec::new(),
            repeat_at,
            repeat_idempotency_key: None,
        });
    }

    let ws_sourced = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("triggerSource"))
        .and_then(Value::as_str)
        == Some("ws_market_price");
    let ws_token_id_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("tokenId"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let ws_price_from_step = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPrice"))
        .and_then(value_as_f64)
        .map(clamp_probability);
    let ws_prices_map: Option<&serde_json::Map<String, Value>> = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("wsPrices"))
        .and_then(Value::as_object);
    let step_ws_price_for_token = ws_prices_map
        .and_then(|map| map.get(&token_id))
        .and_then(value_as_f64)
        .map(clamp_probability);
    let step_token_matches = ws_token_id_from_step
        .as_deref()
        .map(|value| value == token_id.as_str())
        .unwrap_or(true);

    let mut current_price = None;
    let mut price_source = "unavailable";
    if let Some(price) = step_ws_price_for_token {
        current_price = Some(price);
        price_source = "step_ws_prices";
    } else if step_token_matches {
        if let Some(price) = ws_price_from_step {
            current_price = Some(price);
            price_source = "step_ws_price";
        }
    }
    if current_price.is_none() {
        if let Some(price) = fetch_price_from_market_ws(ws, &token_id).await {
            current_price = Some(clamp_probability(price));
            price_source = "ws_subscribe_once";
        }
    }
    let Some(current_price) = current_price else {
        let output = json!({
            "run_id": run.id,
            "node_key": node.key,
            "source_trade_id": source_trade_id,
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "entry_price": entry_price,
            "price_source": price_source,
            "reason": "current_price_unavailable",
            "pass": false
        });
        return Ok(TradeFlowNodeExecution {
            output,
            routes: Vec::new(),
            repeat_at,
            repeat_idempotency_key: None,
        });
    };

    let loss_pct_now = (((entry_price - current_price) / entry_price) * 100.0)
        .max(0.0)
        .clamp(0.0, 100.0);
    let gain_pct_now = (((current_price - entry_price) / entry_price) * 100.0)
        .max(0.0)
        .clamp(0.0, 100.0);
    let now_ms = now.timestamp_millis();

    let mut samples = parse_position_drawdown_samples(flow_node_state(
        context,
        &node.key,
        "drawdown_loss_samples",
    ));
    samples.push(PositionDrawdownSample {
        ts_ms: now_ms,
        loss_pct: loss_pct_now,
        gain_pct: gain_pct_now,
        price: current_price,
    });
    let max_window_ms = rules
        .iter()
        .filter_map(|rule| rule.window_ms)
        .max()
        .unwrap_or(0);
    if max_window_ms > 0 {
        let cutoff = now_ms.saturating_sub(max_window_ms);
        samples.retain(|sample| sample.ts_ms >= cutoff);
    } else if let Some(last) = samples.last().copied() {
        samples.clear();
        samples.push(last);
    }
    if samples.len() > 4000 {
        let overflow = samples.len() - 4000;
        samples.drain(0..overflow);
    }

    let combine_mode_raw = node_config_string(node, "combineMode")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let resolved_combine_mode = match combine_mode_raw.as_deref() {
        Some("and") => "and",
        Some("or") => "or",
        _ if rules.len() <= 1 => "single",
        _ => "or",
    };

    let mut rule_pass_flags = Vec::with_capacity(rules.len());
    let rule_outputs: Vec<Value> = rules
        .iter()
        .map(|rule| {
            let threshold_price = match rule.direction {
                PositionDrawdownDirection::Down => {
                    clamp_probability(entry_price * (1.0 - (rule.loss_pct / 100.0)))
                }
                PositionDrawdownDirection::Up => {
                    clamp_probability(entry_price * (1.0 + (rule.loss_pct / 100.0)))
                }
            };
            if let Some(window_ms) = rule.window_ms {
                let cutoff = now_ms.saturating_sub(window_ms);
                let window_samples: Vec<&PositionDrawdownSample> = samples
                    .iter()
                    .filter(|sample| sample.ts_ms >= cutoff)
                    .collect();
                let max_loss_pct = window_samples
                    .iter()
                    .map(|sample| (((entry_price - sample.price) / entry_price) * 100.0).max(0.0))
                    .map(|value| value.clamp(0.0, 100.0))
                    .fold(0.0_f64, f64::max);
                let max_gain_pct = window_samples
                    .iter()
                    .map(|sample| (((sample.price - entry_price) / entry_price) * 100.0).max(0.0))
                    .map(|value| value.clamp(0.0, 100.0))
                    .fold(0.0_f64, f64::max);
                let max_metric_pct = match rule.direction {
                    PositionDrawdownDirection::Down => max_loss_pct,
                    PositionDrawdownDirection::Up => max_gain_pct,
                };
                let pass = max_metric_pct >= rule.loss_pct;
                rule_pass_flags.push(pass);
                json!({
                    "index": rule.index,
                    "direction": rule.direction.as_str(),
                    "loss_pct": rule.loss_pct,
                    "window_ms": window_ms,
                    "threshold_price": threshold_price,
                    "max_loss_pct_in_window": max_loss_pct,
                    "max_gain_pct_in_window": max_gain_pct,
                    "metric_type": rule.direction.metric_type(),
                    "max_metric_pct_in_window": max_metric_pct,
                    "sample_count_in_window": window_samples.len(),
                    "pass": pass
                })
            } else {
                let metric_now = match rule.direction {
                    PositionDrawdownDirection::Down => loss_pct_now,
                    PositionDrawdownDirection::Up => gain_pct_now,
                };
                let pass = metric_now >= rule.loss_pct;
                rule_pass_flags.push(pass);
                json!({
                    "index": rule.index,
                    "direction": rule.direction.as_str(),
                    "loss_pct": rule.loss_pct,
                    "window_ms": Value::Null,
                    "threshold_price": threshold_price,
                    "max_loss_pct_in_window": loss_pct_now,
                    "max_gain_pct_in_window": gain_pct_now,
                    "metric_type": rule.direction.metric_type(),
                    "max_metric_pct_in_window": metric_now,
                    "sample_count_in_window": 1,
                    "pass": pass
                })
            }
        })
        .collect();

    let pass = match resolved_combine_mode {
        "and" => rule_pass_flags.iter().all(|value| *value),
        "or" => rule_pass_flags.iter().any(|value| *value),
        _ => rule_pass_flags.first().copied().unwrap_or(false),
    };

    set_flow_node_state(
        context,
        &node.key,
        "drawdown_loss_samples",
        json!(samples
            .iter()
            .map(|sample| json!({
                "ts_ms": sample.ts_ms,
                "loss_pct": sample.loss_pct,
                "gain_pct": sample.gain_pct,
                "price": sample.price
            }))
            .collect::<Vec<Value>>()),
    );
    set_flow_node_state(context, &node.key, "last_price", json!(current_price));
    set_flow_node_state(context, &node.key, "last_loss_pct", json!(loss_pct_now));
    set_flow_node_state(context, &node.key, "last_gain_pct", json!(gain_pct_now));
    set_flow_node_state(context, &node.key, "last_entry_price", json!(entry_price));
    set_flow_node_state(context, &node.key, "last_position_qty", json!(0.0));
    set_flow_node_state(context, &node.key, "last_pass", json!(pass));

    if let Some(trade_id) = source_trade_id {
        set_flow_var(context, &format!("{var_prefix}_trade_id"), json!(trade_id));
    }
    set_flow_var(
        context,
        &format!("{var_prefix}_market_slug"),
        json!(market_slug.clone()),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_token_id"),
        json!(token_id.clone()),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_outcome_label"),
        json!(outcome_label.clone()),
    );
    set_flow_var(context, &format!("{var_prefix}_position_qty"), json!(0.0));
    set_flow_var(
        context,
        &format!("{var_prefix}_entry_price"),
        json!(entry_price),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_current_price"),
        json!(current_price),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_loss_pct"),
        json!(loss_pct_now),
    );
    set_flow_var(
        context,
        &format!("{var_prefix}_gain_pct"),
        json!(gain_pct_now),
    );
    set_flow_var(context, &format!("{var_prefix}_pass"), json!(pass));

    let routes = if pass {
        vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at: now,
        }]
    } else {
        Vec::new()
    };

    let output = json!({
        "run_id": run.id,
        "node_key": node.key,
        "source_trade_id": source_trade_id,
        "market_slug": market_slug,
        "token_id": token_id,
        "outcome_label": outcome_label,
        "position_found": false,
        "position_qty": 0.0,
        "entry_price": entry_price,
        "entry_price_source": "manual_entry_price",
        "current_price": current_price,
        "loss_pct": loss_pct_now,
        "gain_pct": gain_pct_now,
        "loss_rules": rule_outputs,
        "combine_mode_input": combine_mode_raw,
        "combine_mode_resolved": resolved_combine_mode,
        "price_source": price_source,
        "ws_sourced": ws_sourced,
        "samples_tracked": samples.len(),
        "pass": pass
    });

    Ok(TradeFlowNodeExecution {
        output,
        routes,
        repeat_at,
        repeat_idempotency_key: None,
    })
}

fn execute_trigger_time_window(
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let start_at = node_config_datetime(node, "startAt")?;
    let end_at = node_config_datetime(node, "endAt")?;
    let now = Utc::now();

    let in_window = match (start_at, end_at) {
        (Some(start), Some(end)) => now >= start && now <= end,
        (Some(start), None) => now >= start,
        (None, Some(end)) => now <= end,
        (None, None) => true,
    };
    let var_key =
        node_config_string(node, "varKey").unwrap_or_else(|| "time_window_open".to_string());
    set_flow_var(context, &var_key, json!(in_window));

    let routes = if in_window {
        vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at: now,
        }]
    } else {
        Vec::new()
    };
    let interval_ms = node_config_i64(node, "minIntervalMs")
        .unwrap_or(5000)
        .max(250) as i64;

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "in_window": in_window,
            "start_at": start_at,
            "end_at": end_at
        }),
        routes,
        repeat_at: Some(now + ChronoDuration::milliseconds(interval_ms)),
        repeat_idempotency_key: None,
    })
}

fn execute_logic_if(node: &TradeFlowNode, context: &Value) -> Result<TradeFlowNodeExecution> {
    let expression = node
        .config
        .get("expression")
        .ok_or_else(|| anyhow::anyhow!("logic.if requires expression"))?;
    let eval_data = build_trade_flow_eval_data(context);
    let decision = evaluate_jsonlogic(expression, &eval_data);
    let branch = if value_truthy(&decision) {
        "on_true"
    } else {
        "on_false"
    };
    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "result": decision,
            "branch": branch
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: branch.to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

fn execute_logic_switch(node: &TradeFlowNode, context: &Value) -> Result<TradeFlowNodeExecution> {
    let expression = node
        .config
        .get("expression")
        .ok_or_else(|| anyhow::anyhow!("logic.switch requires expression"))?;
    let eval_data = build_trade_flow_eval_data(context);
    let switch_value = evaluate_jsonlogic(expression, &eval_data);

    let mut edge_type = "default".to_string();
    if let Some(cases) = node.config.get("cases").and_then(Value::as_array) {
        for case_item in cases {
            let Some(case_obj) = case_item.as_object() else {
                continue;
            };
            let Some(label) = case_obj
                .get("label")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
            else {
                continue;
            };
            let expected = case_obj.get("value").cloned().unwrap_or(Value::Null);
            if values_equal(&switch_value, &expected) {
                edge_type = format!("case:{label}");
                break;
            }
        }
    }

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "switch_value": switch_value,
            "edge_type": edge_type
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type,
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

fn execute_logic_delay(node: &TradeFlowNode) -> Result<TradeFlowNodeExecution> {
    let delay_ms = node_config_i64(node, "delayMs")
        .or_else(|| node_config_i64(node, "ms"))
        .unwrap_or(1000)
        .max(0) as i64;
    let available_at = Utc::now() + ChronoDuration::milliseconds(delay_ms);
    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "delay_ms": delay_ms
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at,
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

fn execute_logic_retry(
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
    context: &Value,
) -> Result<TradeFlowNodeExecution> {
    let max_attempts = node_config_i64(node, "maxAttempts").unwrap_or(3).max(1) as i32;
    let backoff_ms = node_config_i64(node, "backoffMs").unwrap_or(1000).max(0) as i64;
    let strategy = node_config_string(node, "strategy").unwrap_or_else(|| "fixed".to_string());

    let should_retry = if let Some(expression) = node.config.get("expression") {
        let eval_data = build_trade_flow_eval_data(context);
        value_truthy(&evaluate_jsonlogic(expression, &eval_data))
    } else {
        step.input_json
            .as_ref()
            .and_then(|input| input.get("error"))
            .is_some()
    };

    if should_retry && step.attempt < max_attempts {
        let multiplier = if strategy == "exponential" {
            2i64.pow((step.attempt.saturating_sub(1)) as u32)
        } else {
            1
        };
        let delay_ms = backoff_ms.saturating_mul(multiplier);
        let next_attempt = step.attempt + 1;
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "node_key": node.key,
                "should_retry": true,
                "attempt": step.attempt,
                "next_attempt": next_attempt,
                "max_attempts": max_attempts,
                "delay_ms": delay_ms
            }),
            routes: vec![TradeFlowRouteDecision {
                edge_type: "on_retry".to_string(),
                available_at: Utc::now() + ChronoDuration::milliseconds(delay_ms),
            }],
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }

    if should_retry && step.attempt >= max_attempts {
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "node_key": node.key,
                "should_retry": true,
                "attempt": step.attempt,
                "max_attempts": max_attempts,
                "attempts_exhausted": true
            }),
            routes: vec![TradeFlowRouteDecision {
                edge_type: "on_error".to_string(),
                available_at: Utc::now(),
            }],
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "should_retry": false,
            "attempt": step.attempt
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

