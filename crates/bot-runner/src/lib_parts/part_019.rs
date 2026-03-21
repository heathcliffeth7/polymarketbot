async fn execute_action_cancel_order(
    repo: &PostgresRepository,
    node: &TradeFlowNode,
    context: &Value,
) -> Result<TradeFlowNodeExecution> {
    let builder_order_id = resolve_flow_builder_order_id(node, context).ok_or_else(|| {
        anyhow::anyhow!("action.cancel_order requires builderOrderId or targetRef")
    })?;

    repo.set_trade_builder_order_status(builder_order_id, "canceled_requested", None)
        .await?;
    repo.append_trade_builder_order_event(
        builder_order_id,
        "flow_cancel_requested",
        &json!({ "node_key": node.key }),
    )
    .await?;

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "builder_order_id": builder_order_id,
            "canceled_requested": true
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

async fn execute_action_update_order(
    repo: &PostgresRepository,
    node: &TradeFlowNode,
    context: &Value,
) -> Result<TradeFlowNodeExecution> {
    let builder_order_id = resolve_flow_builder_order_id(node, context).ok_or_else(|| {
        anyhow::anyhow!("action.update_order requires builderOrderId or targetRef")
    })?;
    let min_price_distance_cent = node_config_f64(node, "minPriceDistanceCent");
    let max_triggers = node_config_i64(node, "maxTriggers").map(|v| v.clamp(1, 1000) as i32);

    repo.update_trade_builder_order_params(builder_order_id, min_price_distance_cent, max_triggers)
        .await?;
    repo.append_trade_builder_order_event(
        builder_order_id,
        "flow_updated",
        &json!({
            "node_key": node.key,
            "min_price_distance_cent": min_price_distance_cent,
            "max_triggers": max_triggers
        }),
    )
    .await?;

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "builder_order_id": builder_order_id,
            "updated": true
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

fn execute_action_set_state(
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let state_patch = node
        .config
        .get("statePatch")
        .cloned()
        .or_else(|| node.config.get("state").cloned())
        .unwrap_or_else(|| json!({}));
    let state_patch_obj = state_patch
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("action.set_state statePatch must be object"))?;

    let state = ensure_nested_object(context, "state");
    for (key, value) in state_patch_obj {
        state.insert(key.clone(), value.clone());
    }

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "state_patch": state_patch_obj
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

async fn execute_action_notify(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &Value,
) -> Result<TradeFlowNodeExecution> {
    let message =
        node_config_string(node, "message").unwrap_or_else(|| "trade flow notify".to_string());
    let channel = node_config_string(node, "channel").unwrap_or_else(|| "ui".to_string());
    let payload = json!({
        "node_key": node.key,
        "message": message,
        "channel": channel,
        "vars": context.get("vars").cloned().unwrap_or(Value::Null)
    });
    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "notify",
        &payload,
    )
    .await?;

    Ok(TradeFlowNodeExecution {
        output: payload,
        routes: vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

fn resolve_template_vars(template: &str, context: &Value) -> String {
    let mut result = template.to_string();
    if let Some(vars) = context.get("vars").and_then(|v| v.as_object()) {
        for (k, v) in vars {
            let placeholder = format!("{{{{vars.{}}}}}", k);
            let value_str = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            result = result.replace(&placeholder, &value_str);
        }
    }
    if let Some(state) = context.get("state").and_then(|v| v.as_object()) {
        for (k, v) in state {
            let placeholder = format!("{{{{state.{}}}}}", k);
            let value_str = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            result = result.replace(&placeholder, &value_str);
        }
    }
    result
}

async fn execute_action_telegram_notify(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &Value,
) -> Result<TradeFlowNodeExecution> {
    let telegram = load_user_telegram_config(repo, run.user_id).await?;
    let bot_token = resolve_telegram_bot_token(&telegram, node)?;
    let chat_id = resolve_telegram_chat_id(&telegram, node)?;
    let message_template = node_config_string(node, "message")
        .unwrap_or_else(|| "Trade flow notification".to_string());

    let message = resolve_template_vars(&message_template, context);
    let queued_at = step
        .input_json
        .as_ref()
        .and_then(|input| input.get("queuedAt").or_else(|| input.get("queued_at")))
        .and_then(Value::as_str)
        .map(str::to_string);
    let parsed_queued_at = queued_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc));
    let action_started_at = Utc::now();

    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let resp = TELEGRAM_HTTP_CLIENT
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": message,
            "parse_mode": "HTML",
        }))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;
    let action_finished_at = Utc::now();
    let latency_from_enqueue_ms = parsed_queued_at.map(|queued| {
        action_finished_at
            .signed_duration_since(queued)
            .num_milliseconds()
    });

    let (edge_type, output) = match resp {
        Ok(r) if r.status().is_success() => (
            "on_success".to_string(),
            json!({
                "node_key": node.key,
                "status": "sent",
                "chat_id": chat_id,
                "message": message,
                "queued_at": queued_at,
                "action_started_at": action_started_at,
                "sent_at": action_finished_at,
                "latency_from_enqueue_ms": latency_from_enqueue_ms,
            }),
        ),
        Ok(r) => {
            let status = r.status().as_u16();
            let body = r.text().await.unwrap_or_default();
            (
                "on_error".to_string(),
                json!({
                    "node_key": node.key,
                    "status": "error",
                    "http_status": status,
                    "error": body,
                    "queued_at": queued_at,
                    "action_started_at": action_started_at,
                    "sent_at": Value::Null,
                    "latency_from_enqueue_ms": latency_from_enqueue_ms,
                }),
            )
        }
        Err(e) => (
            "on_error".to_string(),
            json!({
                "node_key": node.key,
                "status": "error",
                "error": e.to_string(),
                "queued_at": queued_at,
                "action_started_at": action_started_at,
                "sent_at": Value::Null,
                "latency_from_enqueue_ms": latency_from_enqueue_ms,
            }),
        ),
    };

    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "telegram_notify",
        &output,
    )
    .await?;

    Ok(TradeFlowNodeExecution {
        output,
        routes: vec![TradeFlowRouteDecision {
            edge_type,
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

async fn fetch_trade_flow_market_price(
    ws: &ClobWsClient,
    client: Option<&dyn OrderExecutor>,
    market_slug: &str,
    token_id: Option<&str>,
    price_mode: WsPriceMode,
    trigger_condition: Option<&str>,
) -> Result<f64> {
    let token_id = token_id.filter(|v| !v.trim().is_empty());
    if let Some(token_id) = token_id {
        ws.ensure_market_stream(&[token_id.to_string()]).await?;
        if let Some(snapshot) = ws.get_market_snapshot(token_id).await {
            if let Some(resolved) =
                resolve_trigger_price_from_market_snapshot(&snapshot, price_mode, trigger_condition)
            {
                return Ok(resolved.price);
            }
        }
    }
    if let Some(client) = client {
        let token_id = token_id.ok_or_else(|| {
            anyhow::anyhow!(
                "trigger.market_price requires tokenId for REST fallback (marketSlug={market_slug})"
            )
        })?;
        let resolved =
            resolve_trigger_price_from_rest(client, token_id, price_mode, trigger_condition)
                .await?;
        if resolved.detail.source_detail == "rest_midpoint" {
            warn!(
                %market_slug,
                price_mode = price_mode.as_str(),
                "REST_FALLBACK_MIDPOINT_USED: WS/cache price unavailable, falling back to REST midpoint for {} mode",
                price_mode.as_str()
            );
        }
        return Ok(resolved.price);
    }

    Err(anyhow::anyhow!(
        "trigger.market_price fallback requires live order executor (set LIVE_TRADING_ENABLED=true or provide tokenId websocket price)"
    ))
}

fn node_config_string(node: &TradeFlowNode, key: &str) -> Option<String> {
    node.config
        .get(key)
        .and_then(|value| match value {
            Value::String(v) => Some(v.trim().to_string()),
            Value::Number(v) => Some(v.to_string()),
            Value::Bool(v) => Some(v.to_string()),
            _ => None,
        })
        .filter(|v| !v.is_empty())
}

fn node_config_f64(node: &TradeFlowNode, key: &str) -> Option<f64> {
    node.config.get(key).and_then(value_as_f64)
}

fn node_config_i64(node: &TradeFlowNode, key: &str) -> Option<i64> {
    node.config.get(key).and_then(value_as_i64)
}

fn node_config_bool(node: &TradeFlowNode, key: &str) -> Option<bool> {
    node.config.get(key).and_then(|value| match value {
        Value::Bool(v) => Some(*v),
        Value::Number(v) => v
            .as_i64()
            .map(|n| n != 0)
            .or_else(|| v.as_f64().map(|n| n != 0.0)),
        Value::String(v) => {
            let normalized = v.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "true" | "1" | "yes" | "y" | "on" => Some(true),
                "false" | "0" | "no" | "n" | "off" => Some(false),
                _ => None,
            }
        }
        _ => None,
    })
}

fn node_config_datetime(node: &TradeFlowNode, key: &str) -> Result<Option<DateTime<Utc>>> {
    let Some(value) = node.config.get(key) else {
        return Ok(None);
    };
    let Some(raw) = value.as_str() else {
        return Err(anyhow::anyhow!("{key} must be RFC3339 datetime string"));
    };
    let parsed = DateTime::parse_from_rfc3339(raw)
        .with_context(|| format!("invalid RFC3339 datetime for {key}"))?
        .with_timezone(&Utc);
    Ok(Some(parsed))
}

fn flow_context_string(context: &Value, key: &str) -> Option<String> {
    context
        .get("flowContext")
        .and_then(|v| v.get(key))
        .and_then(|v| match v {
            Value::String(v) => Some(v.trim().to_string()),
            Value::Number(v) => Some(v.to_string()),
            _ => None,
        })
        .filter(|v| !v.is_empty())
}

fn flow_context_f64(context: &Value, key: &str) -> Option<f64> {
    context
        .get("flowContext")
        .and_then(|v| v.get(key))
        .and_then(value_as_f64)
}

fn step_input_value<'a>(step: &'a TradeFlowRunStep, keys: &[&str]) -> Option<&'a Value> {
    let input = step.input_json.as_ref()?;
    keys.iter().find_map(|key| input.get(*key))
}

fn step_input_string(step: &TradeFlowRunStep, keys: &[&str]) -> Option<String> {
    step_input_value(step, keys)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn step_input_f64(step: &TradeFlowRunStep, keys: &[&str]) -> Option<f64> {
    step_input_value(step, keys).and_then(value_as_f64)
}

fn step_input_i64(step: &TradeFlowRunStep, keys: &[&str]) -> Option<i64> {
    step_input_value(step, keys).and_then(value_as_i64)
}

fn resolve_action_place_order_datetime(
    step: &TradeFlowRunStep,
    context: &Value,
    step_keys: &[&str],
    context_key: &str,
) -> Option<DateTime<Utc>> {
    step_input_string(step, step_keys)
        .or_else(|| flow_context_string(context, context_key))
        .and_then(|raw| DateTime::parse_from_rfc3339(&raw).ok())
        .map(|parsed| parsed.with_timezone(&Utc))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActionPlaceOrderStaleMarketRetry {
    stale_market_slug: String,
    current_market_slug: String,
}

fn resolve_action_place_order_stale_market_retry(
    node: &TradeFlowNode,
    context: &Value,
    step: &TradeFlowRunStep,
) -> Option<ActionPlaceOrderStaleMarketRetry> {
    if node_config_string(node, "marketSlug").is_some() {
        return None;
    }

    let stale_market_slug =
        step_input_string(step, &["market_slug", "marketSlug", "wsMarketSlug"])?;
    let current_market_slug = flow_context_string(context, "marketSlug")?;
    if stale_market_slug == current_market_slug {
        return None;
    }

    Some(ActionPlaceOrderStaleMarketRetry {
        stale_market_slug,
        current_market_slug,
    })
}

fn resolve_action_place_order_string(
    node: &TradeFlowNode,
    context: &Value,
    step: &TradeFlowRunStep,
    config_key: &str,
    context_key: &str,
    step_keys: &[&str],
) -> Option<String> {
    node_config_string(node, config_key)
        .or_else(|| step_input_string(step, step_keys))
        .or_else(|| flow_context_string(context, context_key))
}

fn resolve_action_place_order_exit_price(
    node: &TradeFlowNode,
    side: &str,
    enabled: bool,
    cent_key: &str,
    raw_key: &str,
    label: &str,
) -> Result<Option<f64>> {
    if !enabled {
        return Ok(None);
    }

    anyhow::ensure!(
        side == "buy",
        "action.place_order {label}Enabled is only valid for side=buy"
    );

    let cent = node_config_f64(node, cent_key);
    let raw = node_config_f64(node, raw_key);
    let price = cent.map(|value| value / 100.0).or(raw);
    anyhow::ensure!(
        price.is_some() && price.unwrap() > 0.0 && price.unwrap() <= 1.0,
        "action.place_order {label}Price must be in (0, 1] when {label}Enabled is true"
    );
    Ok(price)
}

fn resolve_action_place_order_reference_price(
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
) -> Option<f64> {
    step_input_f64(step, &["triggered_price", "price", "wsPrice"])
        .or_else(|| node_config_f64(node, "triggerPrice"))
        .or_else(|| node_config_f64(node, "triggerPriceCent").map(|value| value / 100.0))
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .map(clamp_probability)
}

fn resolve_action_place_order_max_price(
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
    context: &Value,
) -> Option<f64> {
    node_config_f64(node, "maxPriceCent")
        .map(|value| value / 100.0)
        .or_else(|| node_config_f64(node, "maxPrice"))
        .or_else(|| step_input_f64(step, &["max_price", "maxPrice"]))
        // Global fallback for flows that insert logic nodes between trigger and place_order.
        .or_else(|| flow_context_f64(context, "maxPrice"))
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .map(clamp_probability)
}

fn resolve_action_place_order_guard_trigger_price(step: &TradeFlowRunStep) -> Option<f64> {
    step_input_f64(step, &["trigger_price", "triggerPrice"])
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .map(clamp_probability)
}

fn resolve_action_place_order_underlying_protection(
    context: &Value,
    step: &TradeFlowRunStep,
) -> Option<Value> {
    flow_context_value(context, "underlyingProtection")
        .or_else(|| step_input_value(step, &["protection"]).cloned())
}

fn resolve_flow_source_trade_id(node: &TradeFlowNode, context: &Value) -> Option<i64> {
    node_config_i64(node, "sourceTradeId")
        .or_else(|| {
            context
                .get("flowContext")
                .and_then(|v| v.get("sourceTradeId"))
                .and_then(value_as_i64)
        })
        .filter(|value| *value > 0)
}

fn resolve_flow_builder_order_id(node: &TradeFlowNode, context: &Value) -> Option<i64> {
    if let Some(id) = node_config_i64(node, "builderOrderId") {
        return Some(id);
    }

    let target_ref = node_config_string(node, "targetRef")?;
    context
        .get("refs")
        .and_then(|v| v.get(&target_ref))
        .and_then(value_as_i64)
}

#[derive(Debug, Clone, Copy)]
struct ActionPlaceOrderSizing {
    size_usdc: f64,
    size_basis: &'static str,
    target_qty: Option<f64>,
    remaining_qty: Option<f64>,
    resolved_size_mode: &'static str,
    resolved_size_pct: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionPlaceOrderExistingOrderDecision {
    ReuseActive,
    RearmErrorSell,
    Ignore(&'static str),
}

fn resolve_action_place_order_existing_order_id(
    node: &TradeFlowNode,
    context: &Value,
) -> Option<i64> {
    let refs = context.get("refs")?;
    let ref_key = node_config_string(node, "refKey").unwrap_or_else(|| node.key.clone());
    refs.get(&node.key)
        .and_then(value_as_i64)
        .or_else(|| refs.get(&ref_key).and_then(value_as_i64))
}

fn find_upstream_market_price_trigger_key(
    node_key: &str,
    graph: &TradeFlowGraphRuntime,
) -> Option<String> {
    let mut incoming_by_target: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &graph.edges {
        incoming_by_target
            .entry(edge.target.as_str())
            .or_default()
            .push(edge.source.as_str());
    }

    let mut visited: HashSet<&str> = HashSet::new();
    let mut queue: VecDeque<&str> = VecDeque::from([node_key]);
    let mut found_key: Option<&str> = None;

    while let Some(current) = queue.pop_front() {
        if !visited.insert(current) {
            continue;
        }
        for source_key in incoming_by_target
            .get(current)
            .into_iter()
            .flatten()
            .copied()
        {
            let Some(source_node) = flow_node(graph, source_key) else {
                continue;
            };
            if source_node.node_type == "trigger.market_price" {
                if found_key.is_some_and(|existing| existing != source_key) {
                    return None;
                }
                found_key = Some(source_key);
            }
            queue.push_back(source_key);
        }
    }

    found_key.map(str::to_string)
}

fn classify_action_place_order_existing_order(
    order: &TradeBuilderOrder,
    side: &str,
    source_trade_id: i64,
    market_slug: &str,
    token_id: &str,
    kind: &str,
    execution_mode: &str,
) -> ActionPlaceOrderExistingOrderDecision {
    if order.trade_id != source_trade_id {
        return ActionPlaceOrderExistingOrderDecision::Ignore("source_trade_id_mismatch");
    }
    if order.market_slug != market_slug {
        return ActionPlaceOrderExistingOrderDecision::Ignore("market_slug_mismatch");
    }
    if order.token_id != token_id {
        return ActionPlaceOrderExistingOrderDecision::Ignore("token_id_mismatch");
    }
    if order.side != side {
        return ActionPlaceOrderExistingOrderDecision::Ignore("side_mismatch");
    }
    if order.kind != kind {
        return ActionPlaceOrderExistingOrderDecision::Ignore("kind_mismatch");
    }
    if order.execution_mode != execution_mode {
        return ActionPlaceOrderExistingOrderDecision::Ignore("execution_mode_mismatch");
    }
    if is_trade_builder_order_processable_status(&order.status) {
        return ActionPlaceOrderExistingOrderDecision::ReuseActive;
    }
    if order.status == "error" && trade_builder_should_retry_exit_sell(order) {
        return ActionPlaceOrderExistingOrderDecision::RearmErrorSell;
    }
    if matches!(
        order.status.as_str(),
        "filled" | "completed" | "canceled" | "expired"
    ) {
        return ActionPlaceOrderExistingOrderDecision::Ignore("terminal_status");
    }
    if order.status == "error" {
        return ActionPlaceOrderExistingOrderDecision::Ignore("error_status_not_reusable");
    }
    ActionPlaceOrderExistingOrderDecision::Ignore("inactive_status")
}

async fn load_action_place_order_sell_position(
    repo: &PostgresRepository,
    source_trade_id: i64,
    token_id: &str,
) -> Result<(f64, Option<f64>)> {
    let positions = repo.load_leg_positions(source_trade_id).await?;
    let mut position_qty = 0.0_f64;
    let mut fallback_price = None;

    for position in positions {
        if position.token_id != token_id {
            continue;
        }
        position_qty += position.qty.max(0.0);
        if fallback_price.is_none() {
            fallback_price = position
                .last_fill_price
                .filter(|value| value.is_finite() && *value > 0.0)
                .or_else(|| {
                    (position.avg_entry.is_finite() && position.avg_entry > 0.0)
                        .then_some(position.avg_entry)
                });
        }
    }

    anyhow::ensure!(
        position_qty > 0.0,
        "action.place_order sell requires an open position for the selected token"
    );

    Ok((position_qty, fallback_price))
}

async fn resolve_action_place_order_sell_sizing(
    repo: &PostgresRepository,
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
    source_trade_id: i64,
    token_id: &str,
    trigger_size_for_first_fire: Option<f64>,
    configured_size_usdc: Option<f64>,
    configured_size_pct: Option<f64>,
    use_pct_size: bool,
) -> Result<ActionPlaceOrderSizing> {
    let (position_qty, fallback_price) =
        load_action_place_order_sell_position(repo, source_trade_id, token_id).await?;
    let reference_price = resolve_action_place_order_reference_price(node, step)
        .or(fallback_price)
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "action.place_order sell requires a valid trigger/reference price to derive exit qty"
            )
        })?;

    let (requested_qty, resolved_size_mode, resolved_size_pct) = if use_pct_size {
        let size_pct = trigger_size_for_first_fire
            .or(configured_size_pct)
            .ok_or_else(|| {
                anyhow::anyhow!("action.place_order requires sizePct (0, 100] when sizeMode is pct")
            })?;
        anyhow::ensure!(
            size_pct > 0.0 && size_pct <= 100.0,
            "action.place_order sizePct must be in (0, 100]"
        );
        (position_qty * (size_pct / 100.0), "pct", Some(size_pct))
    } else {
        let requested_size_usdc = trigger_size_for_first_fire
            .or(configured_size_usdc)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "action.place_order requires sizeUsdc/targetNotionalUsdc > 0 (or sizePct in pct mode)"
                )
            })?;
        anyhow::ensure!(
            requested_size_usdc > 0.0,
            "action.place_order size must be > 0"
        );
        (requested_size_usdc / reference_price, "usdc", None)
    };

    let target_qty = round_trade_builder_share_qty(requested_qty.min(position_qty));
    anyhow::ensure!(
        target_qty > 0.0,
        "action.place_order sell resolved target qty must be > 0"
    );

    Ok(ActionPlaceOrderSizing {
        size_usdc: (target_qty * reference_price).max(0.0),
        size_basis: TRADE_BUILDER_SIZE_BASIS_SHARES,
        target_qty: Some(target_qty),
        remaining_qty: Some(target_qty),
        resolved_size_mode,
        resolved_size_pct,
    })
}
