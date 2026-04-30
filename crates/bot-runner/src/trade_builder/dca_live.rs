#[derive(Debug, Clone)]
struct DcaLiveSelectedOutcome {
    market_slug: String,
    outcome_label: String,
    token_id: String,
}

fn dca_live_direct_trigger_key(
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
) -> Result<Option<String>> {
    let incoming = graph
        .edges
        .iter()
        .filter(|edge| edge.target == node.key)
        .collect::<Vec<_>>();
    if incoming.is_empty() {
        return Ok(None);
    }
    anyhow::ensure!(
        incoming.len() == 1,
        "action.place_order dca_live_v1 requires exactly one direct upstream trigger.market_price when trigger-bound"
    );
    let trigger_key = incoming[0].source.as_str();
    let trigger = flow_node(graph, trigger_key)
        .ok_or_else(|| anyhow::anyhow!("dca_live_v1 upstream trigger node not found"))?;
    anyhow::ensure!(
        trigger.node_type == "trigger.market_price",
        "action.place_order dca_live_v1 only supports a direct upstream trigger.market_price"
    );
    let binding_mode = node_config_string(trigger, "bindingMode")
        .unwrap_or_else(|| "standard".to_string())
        .trim()
        .to_ascii_lowercase();
    anyhow::ensure!(
        binding_mode == "dca_live_only",
        "action.place_order dca_live_v1 requires upstream trigger.market_price bindingMode=dca_live_only"
    );
    Ok(Some(trigger_key.to_string()))
}

fn dca_live_parse_context_time(context: &Value, key: &str) -> Option<DateTime<Utc>> {
    let raw = flow_context_string(context, key)?;
    DateTime::parse_from_rfc3339(&raw)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn dca_live_outside_binding_window(context: &Value) -> Option<Value> {
    let mode = flow_context_string(context, "cycleWindowMode")?;
    if !mode.trim().eq_ignore_ascii_case("custom_range") {
        return None;
    }
    let open_at = dca_live_parse_context_time(context, "cycleWindowOpenAt")?;
    let end_at = dca_live_parse_context_time(context, "cycleWindowEndAt")?;
    let now = Utc::now();
    if now >= open_at && now < end_at {
        return None;
    }
    Some(json!({
        "blocked": true,
        "reason": "outside_dca_binding_window",
        "cycle_window_mode": mode,
        "cycle_window_open_at": open_at.to_rfc3339(),
        "cycle_window_end_at": end_at.to_rfc3339(),
        "evaluated_at": now.to_rfc3339(),
    }))
}

fn dca_live_blocked_execution(node: &TradeFlowNode, mut payload: Value) -> TradeFlowNodeExecution {
    if let Some(object) = payload.as_object_mut() {
        object.insert("node_key".to_string(), json!(node.key));
        object.insert("mode".to_string(), json!("dca_live_v1"));
    }
    TradeFlowNodeExecution {
        output: payload,
        routes: Vec::new(),
        repeat_at: None,
        repeat_idempotency_key: None,
    }
}

fn dca_live_selected_outcome_from_value(
    value: &Value,
    context: &Value,
) -> Option<DcaLiveSelectedOutcome> {
    let object = value.as_object()?;
    let market_slug = object
        .get("slug")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| flow_context_string(context, "marketSlug"))?;
    let outcome_label = object
        .get("outcomeLabel")
        .or_else(|| object.get("outcome"))
        .or_else(|| object.get("label"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)?;
    let token_id = object
        .get("tokenId")
        .or_else(|| object.get("token_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)?;
    Some(DcaLiveSelectedOutcome {
        market_slug,
        outcome_label,
        token_id,
    })
}

fn dca_live_selected_outcomes(
    node: &TradeFlowNode,
    context: &Value,
) -> Vec<DcaLiveSelectedOutcome> {
    let mut outcomes = node
        .config
        .get("selectedOutcomes")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| dca_live_selected_outcome_from_value(row, context))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !outcomes.is_empty() {
        return outcomes;
    }

    let market_slug = node_config_string(node, "manualSlug")
        .or_else(|| node_config_string(node, "marketSlug"))
        .or_else(|| flow_context_string(context, "marketSlug"));
    let token_id = node_config_string(node, "tokenId").or_else(|| flow_context_string(context, "tokenId"));
    let outcome_label = node_config_string(node, "outcomeLabel")
        .or_else(|| flow_context_string(context, "outcomeLabel"));
    if let (Some(market_slug), Some(token_id), Some(outcome_label)) =
        (market_slug, token_id, outcome_label)
    {
        outcomes.push(DcaLiveSelectedOutcome {
            market_slug,
            outcome_label,
            token_id,
        });
    }
    outcomes
}

fn dca_live_initial_shares(node: &TradeFlowNode) -> Option<f64> {
    node_config_f64(node, "initialOrderShares")
        .or_else(|| node_config_f64(node, "firstDcaShares"))
        .or_else(|| node_config_f64(node, "targetQty"))
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn dca_live_seed_usdc(node: &TradeFlowNode, shares: f64) -> Option<f64> {
    node_config_f64(node, "sizeUsdc")
        .or_else(|| node_config_f64(node, "targetNotionalUsdc"))
        .or_else(|| {
            let max_price = node_config_f64(node, "dcaEntryMaxPriceCent")
                .or_else(|| node_config_f64(node, "maxPriceCent"))
                .unwrap_or(100.0)
                / 100.0;
            (max_price.is_finite() && max_price > 0.0).then_some(shares * max_price)
        })
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn build_dca_live_single_leg_node(
    node: &TradeFlowNode,
    outcome: &DcaLiveSelectedOutcome,
    shares: f64,
) -> TradeFlowNode {
    let mut config = node
        .config
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);
    config.insert("mode".to_string(), json!("single"));
    config.insert("side".to_string(), json!("buy"));
    config.insert("marketSlug".to_string(), json!(outcome.market_slug));
    config.insert("tokenId".to_string(), json!(outcome.token_id));
    config.insert("outcomeLabel".to_string(), json!(outcome.outcome_label));
    config.insert("sizeMode".to_string(), json!("shares"));
    config.insert("targetQty".to_string(), json!(shares));
    if !config.contains_key("executionMode") {
        config.insert("executionMode".to_string(), json!("limit"));
    }
    if let Some(max_price_cent) = node_config_f64(node, "dcaEntryMaxPriceCent")
        .or_else(|| node_config_f64(node, "maxPriceCent"))
    {
        config.insert("maxPriceCent".to_string(), json!(max_price_cent));
    }
    TradeFlowNode {
        key: node.key.clone(),
        node_type: node.node_type.clone(),
        config: Value::Object(config),
    }
}

fn build_dca_live_pair_node(
    node: &TradeFlowNode,
    context: &Value,
    shares: f64,
) -> Result<TradeFlowNode> {
    let mut config = node
        .config
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);
    config.insert("mode".to_string(), json!("pair_lock"));
    let target_pair_cost_cent = node_config_f64(node, "targetPairCostCent")
        .or_else(|| node_config_f64(node, "pairMaxTotalCent"))
        .unwrap_or(97.0);
    config.insert("pairMaxTotalCent".to_string(), json!(target_pair_cost_cent));
    if !config.contains_key("sizeUsdc") && !config.contains_key("targetNotionalUsdc") {
        let seed_usdc = dca_live_seed_usdc(node, shares)
            .ok_or_else(|| anyhow::anyhow!("dca_live_v1 pair mode requires seed size"))?;
        config.insert("sizeUsdc".to_string(), json!(seed_usdc));
    }
    if !config.contains_key("executionMode") {
        config.insert("executionMode".to_string(), json!("limit"));
    }
    if let Some(market_slug) = node_config_string(node, "manualSlug")
        .or_else(|| node_config_string(node, "marketSlug"))
        .or_else(|| flow_context_string(context, "marketSlug"))
    {
        config.insert("marketSlug".to_string(), json!(market_slug));
    }
    if let Some(first) = dca_live_selected_outcomes(node, context).into_iter().next() {
        config.insert("tokenId".to_string(), json!(first.token_id));
        config.insert("outcomeLabel".to_string(), json!(first.outcome_label));
        config.insert("marketSlug".to_string(), json!(first.market_slug));
    }
    Ok(TradeFlowNode {
        key: node.key.clone(),
        node_type: node.node_type.clone(),
        config: Value::Object(config),
    })
}

fn dca_live_pair_graph(
    graph: &TradeFlowGraphRuntime,
    trigger_key: &str,
) -> TradeFlowGraphRuntime {
    let nodes = graph
        .nodes
        .iter()
        .map(|candidate| {
            if candidate.key != trigger_key {
                return candidate.clone();
            }
            let mut config = candidate
                .config
                .as_object()
                .cloned()
                .unwrap_or_else(serde_json::Map::new);
            config.insert("bindingMode".to_string(), json!("pair_lock_only"));
            TradeFlowNode {
                key: candidate.key.clone(),
                node_type: candidate.node_type.clone(),
                config: Value::Object(config),
            }
        })
        .collect();
    TradeFlowGraphRuntime {
        context: graph.context.clone(),
        nodes,
        edges: graph.edges.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_action_place_order_dca_live(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: Option<&dyn OrderExecutor>,
    ws: &ClobWsClient,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let trigger_key = dca_live_direct_trigger_key(node, graph)?;
    if let Some(blocked) = dca_live_outside_binding_window(context) {
        return Ok(dca_live_blocked_execution(node, blocked));
    }

    let side = node_config_string(node, "side")
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_else(|| "buy".to_string());
    anyhow::ensure!(side == "buy", "action.place_order dca_live_v1 only supports side=buy");
    let shares = dca_live_initial_shares(node)
        .ok_or_else(|| anyhow::anyhow!("action.place_order dca_live_v1 requires initialOrderShares, firstDcaShares, or targetQty > 0"))?;
    let side_mode = node_config_string(node, "sideMode")
        .or_else(|| node_config_string(node, "dcaSideMode"))
        .unwrap_or_else(|| "one_sided".to_string())
        .trim()
        .to_ascii_lowercase();

    if side_mode == "two_sided_pair" {
        let trigger_key = trigger_key.ok_or_else(|| {
            anyhow::anyhow!(
                "action.place_order dca_live_v1 two_sided_pair requires direct trigger.market_price bindingMode=dca_live_only"
            )
        })?;
        let pair_node = build_dca_live_pair_node(node, context, shares)?;
        let pair_graph = dca_live_pair_graph(graph, &trigger_key);
        let mut pair_context = context.clone();
        let execution = execute_action_place_order_pair_lock(
            repo,
            run_id,
            cfg,
            limits,
            policy,
            client,
            ws,
            run,
            step,
            &pair_node,
            &pair_graph,
            &mut pair_context,
        )
        .await?;
        *context = pair_context;
        return Ok(execution);
    }

    let outcomes = dca_live_selected_outcomes(node, context);
    anyhow::ensure!(
        !outcomes.is_empty(),
        "action.place_order dca_live_v1 requires selectedOutcomes or marketSlug/tokenId/outcomeLabel"
    );
    if side_mode == "one_sided" {
        anyhow::ensure!(
            outcomes.len() == 1,
            "action.place_order dca_live_v1 one_sided requires exactly one selected outcome"
        );
    } else if side_mode == "multi_outcome_basket" {
        anyhow::ensure!(
            outcomes.len() >= 2,
            "action.place_order dca_live_v1 multi_outcome_basket requires at least two selected outcomes"
        );
    } else {
        anyhow::bail!("action.place_order dca_live_v1 sideMode must be one_sided, two_sided_pair, or multi_outcome_basket");
    }

    let max_orders = node_config_i64(node, "maxOpenOrdersAllSlugs")
        .unwrap_or(outcomes.len() as i64)
        .max(1) as usize;
    let mut executions = Vec::new();
    for outcome in outcomes.iter().take(max_orders) {
        let single_node = build_dca_live_single_leg_node(node, outcome, shares);
        let mut single_context = context.clone();
        let execution = execute_action_place_order(
            repo,
            run_id,
            cfg,
            limits,
            policy,
            client,
            run,
            step,
            &single_node,
            graph,
            &mut single_context,
        )
        .await?;
        *context = single_context;
        executions.push(json!({
            "market_slug": outcome.market_slug,
            "token_id": outcome.token_id,
            "outcome_label": outcome.outcome_label,
            "output": execution.output,
        }));
    }

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "mode": "dca_live_v1",
            "side_mode": side_mode,
            "selected_outcome_count": outcomes.len(),
            "submitted_count": executions.len(),
            "executions": executions,
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}
