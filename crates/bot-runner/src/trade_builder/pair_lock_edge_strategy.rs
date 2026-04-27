const PAIR_LOCK_STRATEGY_LEGACY: &str = "legacy";
const PAIR_LOCK_STRATEGY_EDGE_PAIRLOCK_V1: &str = "edge_pairlock_v1";
const PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1: &str = "biased_hedge_v1";
const DEFAULT_PAIR_LOCK_DECISION_QTY: f64 = 5.0;
const DEFAULT_PAIR_LOCK_SINGLE_EDGE_THRESHOLD: f64 = 0.10;
const DEFAULT_PAIR_LOCK_COST_BUFFER: f64 = 0.005;
const PAIR_LOCK_EDGE_TAKER_FEE_RATE: f64 = 0.072;

#[derive(Debug, Clone, Copy)]
struct ActionPlaceOrderPairLockEdgeConfig {
    decision_qty: f64,
    single_edge_threshold: f64,
    cost_buffer: f64,
}

#[derive(Debug, Clone)]
struct PairLockEdgeCandidate {
    token_id: String,
    outcome_label: String,
    quote: PairLockResolvedQuote,
    ask: Option<f64>,
    fee: Option<f64>,
    cost: Option<f64>,
    q: Option<f64>,
    edge: Option<f64>,
    guard_decision: &'static str,
    guard_reason: String,
    diagnostics: Value,
}

#[derive(Debug)]
struct PairLockEdgeOrderResult {
    execution: TradeFlowNodeExecution,
    builder_order_id: Option<i64>,
    source_trade_id: Option<i64>,
}

#[derive(Debug, Clone)]
struct PairLockEdgePositionLock {
    position: TradeBuilderParentPosition,
    parent_order: TradeBuilderOrder,
    counter: PairLockEdgeCandidate,
    qty: f64,
    avg_cost: f64,
    total_cost: f64,
    margin: f64,
}

fn resolve_action_place_order_pair_lock_strategy(node: &TradeFlowNode) -> Result<&'static str> {
    match node_config_string(node, "pairLockStrategy")
        .unwrap_or_else(|| PAIR_LOCK_STRATEGY_LEGACY.to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "" | PAIR_LOCK_STRATEGY_LEGACY => Ok(PAIR_LOCK_STRATEGY_LEGACY),
        PAIR_LOCK_STRATEGY_EDGE_PAIRLOCK_V1 => Ok(PAIR_LOCK_STRATEGY_EDGE_PAIRLOCK_V1),
        PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1 => Ok(PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1),
        _ => anyhow::bail!("action.place_order pairLockStrategy must be legacy, edge_pairlock_v1, or biased_hedge_v1"),
    }
}

fn action_place_order_uses_edge_pairlock_strategy(node: &TradeFlowNode) -> bool {
    matches!(
        resolve_action_place_order_pair_lock_strategy(node),
        Ok(PAIR_LOCK_STRATEGY_EDGE_PAIRLOCK_V1)
    )
}

fn action_place_order_uses_biased_hedge_strategy(node: &TradeFlowNode) -> bool {
    matches!(
        resolve_action_place_order_pair_lock_strategy(node),
        Ok(PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1)
    )
}

fn resolve_action_place_order_pair_lock_edge_config(
    node: &TradeFlowNode,
) -> Result<ActionPlaceOrderPairLockEdgeConfig> {
    anyhow::ensure!(
        node_config_bool(node, "priceToBeatGuardEnabled").unwrap_or(false),
        "action.place_order edge_pairlock_v1 requires priceToBeatGuardEnabled=true"
    );
    let ptb_mode = node_config_string(node, "priceToBeatMode")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    anyhow::ensure!(
        ptb_mode == "iv_mismatch_edge",
        "action.place_order edge_pairlock_v1 requires priceToBeatMode=iv_mismatch_edge"
    );

    let decision_qty = node_config_f64(node, "pairLockDecisionQty")
        .unwrap_or(DEFAULT_PAIR_LOCK_DECISION_QTY);
    anyhow::ensure!(
        decision_qty.is_finite() && decision_qty > 0.0,
        "action.place_order pairLockDecisionQty must be > 0"
    );
    let single_edge_threshold = node_config_f64(node, "pairLockSingleEdgeThreshold")
        .unwrap_or(DEFAULT_PAIR_LOCK_SINGLE_EDGE_THRESHOLD);
    anyhow::ensure!(
        single_edge_threshold.is_finite() && single_edge_threshold >= 0.0,
        "action.place_order pairLockSingleEdgeThreshold must be >= 0"
    );
    let cost_buffer =
        node_config_f64(node, "pairLockCostBuffer").unwrap_or(DEFAULT_PAIR_LOCK_COST_BUFFER);
    anyhow::ensure!(
        cost_buffer.is_finite() && cost_buffer >= 0.0,
        "action.place_order pairLockCostBuffer must be >= 0"
    );

    Ok(ActionPlaceOrderPairLockEdgeConfig {
        decision_qty,
        single_edge_threshold,
        cost_buffer,
    })
}

fn pair_lock_edge_taker_fee(price: f64) -> f64 {
    PAIR_LOCK_EDGE_TAKER_FEE_RATE * price * (1.0 - price)
}

fn pair_lock_edge_valid_probability(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value <= 1.0
}

fn pair_lock_edge_iv_mismatch_payload(
    eval: &ActionPlaceOrderPairLockPrimaryCandidateEval,
) -> Option<&Value> {
    let guard = eval.diagnostics.get("price_to_beat_guard")?;
    if guard.get("threshold_mode").and_then(Value::as_str) != Some("iv_mismatch_edge") {
        return None;
    }
    guard.get("iv_mismatch_edge")
}

fn pair_lock_edge_q_from_iv_payload(payload: &Value) -> Option<f64> {
    payload
        .get("q_final")
        .and_then(value_as_f64)
        .or_else(|| payload.get("q").and_then(value_as_f64))
        .filter(|value| value.is_finite())
}

fn pair_lock_edge_candidate_from_eval(
    eval: ActionPlaceOrderPairLockPrimaryCandidateEval,
    cost_buffer: f64,
) -> PairLockEdgeCandidate {
    let iv_payload = pair_lock_edge_iv_mismatch_payload(&eval).cloned();
    let ask = eval
        .quote
        .best_ask
        .or_else(|| iv_payload.as_ref().and_then(|payload| payload.get("ask")).and_then(value_as_f64))
        .filter(|value| pair_lock_edge_valid_probability(*value));
    let fee = ask.map(pair_lock_edge_taker_fee);
    let cost = ask.zip(fee).map(|(ask, fee)| ask + fee + cost_buffer.max(0.0));
    let q = iv_payload.as_ref().and_then(pair_lock_edge_q_from_iv_payload);
    let edge = q.zip(cost).map(|(q, cost)| q - cost);
    let diagnostics = json!({
        "token_id": eval.token_id,
        "outcome_label": eval.outcome_label,
        "ask": ask,
        "fee": fee,
        "cost": cost,
        "q": q,
        "edge": edge,
        "cost_buffer": cost_buffer,
        "guard_decision": eval.decision,
        "guard_reason": eval.reason_code,
        "guard": eval.diagnostics,
    });

    PairLockEdgeCandidate {
        token_id: eval.token_id,
        outcome_label: eval.outcome_label,
        quote: eval.quote,
        ask,
        fee,
        cost,
        q,
        edge,
        guard_decision: eval.decision,
        guard_reason: eval.reason_code,
        diagnostics,
    }
}

fn pair_lock_edge_pair_total(
    up: &PairLockEdgeCandidate,
    down: &PairLockEdgeCandidate,
) -> Option<f64> {
    up.cost.zip(down.cost).map(|(up_cost, down_cost)| up_cost + down_cost)
}

fn pair_lock_edge_better_single<'a>(
    up: &'a PairLockEdgeCandidate,
    down: &'a PairLockEdgeCandidate,
    threshold: f64,
) -> Option<&'a PairLockEdgeCandidate> {
    [up, down]
        .into_iter()
        .filter(|candidate| candidate.edge.is_some_and(|edge| edge >= threshold))
        .max_by(|left, right| {
            left.edge
                .unwrap_or(f64::NEG_INFINITY)
                .partial_cmp(&right.edge.unwrap_or(f64::NEG_INFINITY))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn pair_lock_edge_lead_counter<'a>(
    up: &'a PairLockEdgeCandidate,
    down: &'a PairLockEdgeCandidate,
) -> (&'a PairLockEdgeCandidate, &'a PairLockEdgeCandidate) {
    let up_edge = up.edge.unwrap_or(f64::NEG_INFINITY);
    let down_edge = down.edge.unwrap_or(f64::NEG_INFINITY);
    if down_edge > up_edge {
        (down, up)
    } else {
        (up, down)
    }
}

fn pair_lock_edge_candidate_summary(candidate: &PairLockEdgeCandidate) -> Value {
    json!({
        "token_id": candidate.token_id,
        "outcome_label": candidate.outcome_label,
        "ask": candidate.ask,
        "fee": candidate.fee,
        "cost": candidate.cost,
        "q": candidate.q,
        "edge": candidate.edge,
        "guard_decision": candidate.guard_decision,
        "guard_reason": candidate.guard_reason,
    })
}

fn build_pair_lock_edge_diagnostics(
    edge_config: ActionPlaceOrderPairLockEdgeConfig,
    up: &PairLockEdgeCandidate,
    down: &PairLockEdgeCandidate,
    pair_lock: &ActionPlaceOrderPairLockConfig,
    token_resolution_payload: &Value,
) -> Value {
    let pair_total = pair_lock_edge_pair_total(up, down);
    let mut diagnostics = json!({
        "strategy": PAIR_LOCK_STRATEGY_EDGE_PAIRLOCK_V1,
        "decision_qty": edge_config.decision_qty,
        "single_edge_threshold": edge_config.single_edge_threshold,
        "cost_buffer": edge_config.cost_buffer,
        "pair_max_total": pair_lock.max_total_price,
        "pair_total": pair_total,
        "up": pair_lock_edge_candidate_summary(up),
        "down": pair_lock_edge_candidate_summary(down),
        "up_guard": up.diagnostics,
        "down_guard": down.diagnostics,
    });
    append_json_object_fields(&mut diagnostics, token_resolution_payload);
    diagnostics
}

fn configure_pair_lock_edge_share_child_node(
    node: &TradeFlowNode,
    qty: f64,
    ask: f64,
) -> TradeFlowNode {
    let mut config = node.config.as_object().cloned().unwrap_or_default();
    let size_usdc = qty * ask;
    config.insert("side".to_string(), json!("buy"));
    config.insert("kind".to_string(), json!("immediate"));
    config.insert("executionMode".to_string(), json!("market"));
    config.insert("sizeMode".to_string(), json!("usdc"));
    config.insert("sizeUsdc".to_string(), json!(size_usdc));
    config.insert("maxPriceCent".to_string(), json!(ask * 100.0));
    config.insert("priceToBeatGuardEnabled".to_string(), json!(false));
    config.insert("triggerPriceGuardEnabled".to_string(), json!(false));
    config.insert("executionFloorGuardEnabled".to_string(), json!(false));
    config.insert("retryOnPriceToBeatGuardBlock".to_string(), json!(false));
    config.insert("retryOnTriggerPriceGuardBlock".to_string(), json!(false));
    config.insert("retryOnExecutionFloorGuardBlock".to_string(), json!(false));
    config.insert("retryOnMaxPriceBlock".to_string(), json!(false));
    for key in [
        "triggerCondition",
        "triggerPrice",
        "triggerPriceCent",
        "guardTriggerPrice",
        "priceToBeatMode",
        "priceToBeatMaxDiff",
        "priceToBeatMaxDiffUnit",
        "pairLockStrategy",
        "pairLockDecisionQty",
        "pairLockSingleEdgeThreshold",
        "pairLockCostBuffer",
    ] {
        config.remove(key);
    }

    TradeFlowNode {
        key: node.key.clone(),
        node_type: node.node_type.clone(),
        config: Value::Object(config),
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_pair_lock_edge_share_buy_order(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    context: &mut Value,
    candidate: &PairLockEdgeCandidate,
    qty: f64,
    source_trade_id: Option<i64>,
) -> Result<PairLockEdgeOrderResult> {
    let ask = candidate
        .ask
        .ok_or_else(|| anyhow::anyhow!("edge_pairlock_v1 candidate ask unavailable"))?;
    let target_qty = round_trade_builder_share_qty(qty);
    anyhow::ensure!(
        target_qty > TRADE_BUILDER_PAIR_QTY_TOLERANCE,
        "edge_pairlock_v1 resolved qty is too small"
    );
    let share_size_usdc = target_qty * ask;
    let child_node = configure_pair_lock_edge_share_child_node(node, target_qty, ask);
    let child_step = clone_pair_lock_step_with_quote(step, &candidate.quote);
    if let Some(source_trade_id) = source_trade_id {
        set_flow_context(context, "sourceTradeId", json!(source_trade_id));
    }
    let mut execution = execute_action_place_order(
        repo,
        run_id,
        cfg,
        limits,
        policy,
        Some(client),
        run,
        &child_step,
        &child_node,
        graph,
        context,
    )
    .await?;
    let builder_order_id = execution
        .output
        .get("builder_order_id")
        .and_then(Value::as_i64);
    let source_trade_id = extract_source_trade_id(&execution);
    let Some(builder_order_id) = builder_order_id else {
        return Ok(PairLockEdgeOrderResult {
            execution,
            builder_order_id: None,
            source_trade_id,
        });
    };
    let order = repo
        .get_trade_builder_order(builder_order_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("edge_pairlock_v1 child order not found after create"))?;
    repo.update_trade_builder_order_sizing_and_state(
        builder_order_id,
        TRADE_BUILDER_SIZE_BASIS_SHARES,
        share_size_usdc,
        Some(target_qty),
        Some(share_size_usdc),
        Some(target_qty),
        &order.status,
        None,
        order.eligible_after_at,
        order.eligible_before_at,
        None,
        None,
        None,
    )
    .await?;
    repo.append_trade_builder_order_event(
        builder_order_id,
        "pair_lock_edge_share_sizing_applied",
        &json!({
            "strategy": PAIR_LOCK_STRATEGY_EDGE_PAIRLOCK_V1,
            "target_qty": target_qty,
            "reference_ask": ask,
            "size_usdc": share_size_usdc,
            "candidate": pair_lock_edge_candidate_summary(candidate),
        }),
    )
    .await?;

    if let Some(output) = execution.output.as_object_mut() {
        output.insert("pair_lock_strategy".to_string(), json!(PAIR_LOCK_STRATEGY_EDGE_PAIRLOCK_V1));
        output.insert("size_basis".to_string(), json!(TRADE_BUILDER_SIZE_BASIS_SHARES));
        output.insert("size_mode".to_string(), json!("shares"));
        output.insert("size_usdc".to_string(), json!(share_size_usdc));
        output.insert("target_qty".to_string(), json!(target_qty));
        output.insert("remaining_qty".to_string(), json!(target_qty));
        output.insert("reference_ask".to_string(), json!(ask));
    }

    Ok(PairLockEdgeOrderResult {
        execution,
        builder_order_id: Some(builder_order_id),
        source_trade_id,
    })
}

async fn create_pair_lock_edge_session(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    market_slug: &str,
    pair_lock: &ActionPlaceOrderPairLockConfig,
) -> Result<i64> {
    repo.create_trade_builder_pair_session(
        run.user_id,
        Some(run.definition_id),
        Some(run.id),
        Some(&node.key),
        market_slug,
        pair_lock.max_total_price * 100.0,
        0.0,
        0.0,
        pair_lock.orphan_grace_ms,
        pair_lock.ignore_stop_loss_after_locked,
        pair_lock.notify_on_pair_locked,
        pair_lock.notify_on_pair_unwind,
        false,
    )
    .await
}

fn pair_lock_edge_position_avg_cost(
    position: &TradeBuilderParentPosition,
    parent_order: &TradeBuilderOrder,
) -> Option<f64> {
    if position.baseline_qty > TRADE_BUILDER_PAIR_QTY_TOLERANCE && parent_order.size_usdc > 0.0 {
        let avg = parent_order.size_usdc / position.baseline_qty;
        if pair_lock_edge_valid_probability(avg) {
            return Some(avg);
        }
    }
    if let Some(target_qty) = parent_order.target_qty.filter(|value| *value > 0.0) {
        let avg = parent_order.size_usdc / target_qty;
        if pair_lock_edge_valid_probability(avg) {
            return Some(avg);
        }
    }
    position
        .last_fill_price
        .or(parent_order.submitted_dynamic_price)
        .or(parent_order.last_seen_price)
        .filter(|value| pair_lock_edge_valid_probability(*value))
}

async fn select_pair_lock_edge_position_lock(
    repo: &PostgresRepository,
    user_id: i64,
    market_slug: &str,
    up: &PairLockEdgeCandidate,
    down: &PairLockEdgeCandidate,
    max_total_price: f64,
) -> Result<Option<PairLockEdgePositionLock>> {
    let token_ids = vec![up.token_id.clone(), down.token_id.clone()];
    let positions = repo
        .list_open_trade_builder_parent_positions_for_market(user_id, market_slug, &token_ids)
        .await?;
    let mut best: Option<PairLockEdgePositionLock> = None;

    for position in positions {
        let Some(parent_order) = repo
            .get_trade_builder_order(position.parent_builder_order_id)
            .await?
        else {
            continue;
        };
        if parent_order.user_id != user_id
            || parent_order.market_slug != market_slug
            || parent_order.side != "buy"
            || parent_order.pair_session_id.is_some()
            || position.current_qty <= TRADE_BUILDER_PAIR_QTY_TOLERANCE
        {
            continue;
        }
        let counter = if position.token_id == up.token_id {
            down
        } else if position.token_id == down.token_id {
            up
        } else {
            continue;
        };
        let Some(counter_cost) = counter.cost else {
            continue;
        };
        let Some(avg_cost) = pair_lock_edge_position_avg_cost(&position, &parent_order) else {
            continue;
        };
        let total_cost = avg_cost + counter_cost;
        if total_cost > max_total_price {
            continue;
        }
        let qty = round_trade_builder_share_qty(position.current_qty);
        if qty <= TRADE_BUILDER_PAIR_QTY_TOLERANCE {
            continue;
        }
        let margin = max_total_price - total_cost;
        let next = PairLockEdgePositionLock {
            position,
            parent_order,
            counter: counter.clone(),
            qty,
            avg_cost,
            total_cost,
            margin,
        };
        let replace = best
            .as_ref()
            .map(|current| {
                margin > current.margin
                    || ((margin - current.margin).abs() <= f64::EPSILON && qty > current.qty)
            })
            .unwrap_or(true);
        if replace {
            best = Some(next);
        }
    }

    Ok(best)
}

fn pair_lock_edge_waiting_execution(
    node_key: &str,
    market_slug: &str,
    diagnostics: Value,
) -> TradeFlowNodeExecution {
    TradeFlowNodeExecution {
        output: json!({
            "node_key": node_key,
            "blocked": true,
            "retrying": true,
            "reason": "pair_lock_edge_no_decision",
            "market_slug": market_slug,
            "retry_delay_ms": PAIR_LOCK_PRIMARY_GUARD_RETRY_DELAY_MS,
            "pair_lock_strategy": PAIR_LOCK_STRATEGY_EDGE_PAIRLOCK_V1,
            "pair_lock_edge": diagnostics,
        }),
        routes: Vec::new(),
        repeat_at: Some(Utc::now() + ChronoDuration::milliseconds(PAIR_LOCK_PRIMARY_GUARD_RETRY_DELAY_MS)),
        repeat_idempotency_key: None,
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_action_place_order_pair_lock_edge_strategy(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    context: &mut Value,
    pair_lock: &ActionPlaceOrderPairLockConfig,
    trigger_node_key: &str,
) -> Result<TradeFlowNodeExecution> {
    let edge_config = resolve_action_place_order_pair_lock_edge_config(node)?;
    let market_slug = resolve_action_place_order_string(
        node,
        context,
        step,
        "marketSlug",
        "marketSlug",
        &["market_slug", "marketSlug", "wsMarketSlug"],
    )
    .ok_or_else(|| anyhow::anyhow!("pair_lock requires marketSlug"))?;
    let resolved_tokens =
        resolve_trade_builder_pair_lock_yes_no_tokens(cfg, &market_slug, trigger_node_key, context)
            .await?;
    let effective_market_slug = resolved_tokens
        .trigger_node_market_slug
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(market_slug.as_str());
    promote_trigger_node_auto_scope_context_to_flow_context(
        context,
        trigger_node_key,
        effective_market_slug,
    );
    let token_resolution_payload = pair_lock_token_resolution_payload(&resolved_tokens);
    let (up_label, down_label) = pair_lock_primary_outcome_labels(&market_slug);
    let ptb_runtime =
        crate::trade_flow::guards::price_to_beat::PriceToBeatGuardRuntimeContext::pair_lock_auto_primary(
            repo,
            run.user_id,
            cfg,
            Some(client),
        );
    let up_eval = evaluate_action_place_order_pair_lock_primary_candidate(
        Some(ptb_runtime),
        ws,
        client,
        run,
        step,
        node,
        context,
        &market_slug,
        &resolved_tokens.yes_token_id,
        up_label,
    )
    .await?;
    let down_eval = evaluate_action_place_order_pair_lock_primary_candidate(
        Some(ptb_runtime),
        ws,
        client,
        run,
        step,
        node,
        context,
        &market_slug,
        &resolved_tokens.no_token_id,
        down_label,
    )
    .await?;
    let up = pair_lock_edge_candidate_from_eval(up_eval, edge_config.cost_buffer);
    let down = pair_lock_edge_candidate_from_eval(down_eval, edge_config.cost_buffer);
    let diagnostics = build_pair_lock_edge_diagnostics(
        edge_config,
        &up,
        &down,
        pair_lock,
        &token_resolution_payload,
    );

    if let Some(lock) = select_pair_lock_edge_position_lock(
        repo,
        run.user_id,
        &market_slug,
        &up,
        &down,
        pair_lock.max_total_price,
    )
    .await?
    {
        let counter = ActionPlaceOrderPairResolvedCounterLeg {
            token_id: lock.counter.token_id.clone(),
            outcome_label: lock.counter.outcome_label.clone(),
        };
        let counter_node =
            build_pair_lock_counter_leg_node(node, &market_slug, &counter, pair_lock, trigger_node_key);
        let mut counter_context = context.clone();
        let counter_result = execute_pair_lock_edge_share_buy_order(
            repo,
            run_id,
            cfg,
            limits,
            policy,
            client,
            run,
            step,
            &counter_node,
            graph,
            &mut counter_context,
            &lock.counter,
            lock.qty,
            Some(lock.position.source_trade_id),
        )
        .await?;
        let Some(counter_order_id) = counter_result.builder_order_id else {
            return Ok(counter_result.execution);
        };
        let pair_session_id =
            create_pair_lock_edge_session(repo, run, node, &market_slug, pair_lock).await?;
        repo.attach_trade_builder_pair_session_orders(
            pair_session_id,
            lock.parent_order.id,
            counter_order_id,
        )
        .await?;
        repo.set_trade_builder_order_pair_session(
            lock.parent_order.id,
            Some(pair_session_id),
            Some(TRADE_BUILDER_PAIR_ROLE_LEAD_CANDIDATE),
        )
        .await?;
        repo.set_trade_builder_order_pair_session(
            counter_order_id,
            Some(pair_session_id),
            Some(TRADE_BUILDER_PAIR_ROLE_COUNTER_CANDIDATE),
        )
        .await?;
        repo.record_trade_builder_pair_session_fill(
            pair_session_id,
            TRADE_BUILDER_PAIR_ROLE_LEAD_CANDIDATE,
            lock.parent_order.id,
            lock.qty,
            0.0,
            lock.qty,
            lock.avg_cost,
            Utc::now(),
        )
        .await?;
        repo.append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "pair_lock_session_created",
            &json!({
                "node_key": node.key,
                "pair_lock_strategy": PAIR_LOCK_STRATEGY_EDGE_PAIRLOCK_V1,
                "pair_lock_edge_decision": "position_counter_lock",
                "pair_session_id": pair_session_id,
                "market_slug": market_slug,
                "primary_order_id": lock.parent_order.id,
                "counter_order_id": counter_order_id,
                "primary_source_trade_id": lock.position.source_trade_id,
                "counter_source_trade_id": counter_result.source_trade_id,
                "trigger_node_key": trigger_node_key,
                "pair_max_total_cent": pair_lock.max_total_price * 100.0,
                "target_qty": lock.qty,
                "position_avg_cost": lock.avg_cost,
                "counter_cost": lock.counter.cost,
                "total_cost": lock.total_cost,
                "edge_margin": lock.margin,
                "selection": diagnostics,
            }),
        )
        .await?;

        let ref_key = action_place_order_pair_lock_ref_key(node);
        bind_action_place_order_ref_bindings(context, node, &ref_key, lock.parent_order.id);
        set_flow_context(context, "sourceTradeId", json!(lock.position.source_trade_id));
        set_flow_var(context, &format!("{ref_key}_pair_session_id"), json!(pair_session_id));
        set_flow_var(
            context,
            &format!("{ref_key}_counter_builder_order_id"),
            json!(counter_order_id),
        );

        return Ok(TradeFlowNodeExecution {
            output: json!({
                "node_key": node.key,
                "builder_order_id": lock.parent_order.id,
                "counter_builder_order_id": counter_order_id,
                "pair_session_id": pair_session_id,
                "ref_key": ref_key,
                "market_slug": market_slug,
                "token_id": lock.parent_order.token_id,
                "counter_token_id": counter.token_id,
                "outcome_label": lock.parent_order.outcome_label,
                "counter_outcome_label": counter.outcome_label,
                "source_trade_id": lock.position.source_trade_id,
                "counter_source_trade_id": counter_result.source_trade_id,
                "trigger_node_key": trigger_node_key,
                "pair_lock_strategy": PAIR_LOCK_STRATEGY_EDGE_PAIRLOCK_V1,
                "pair_lock_edge_decision": "position_counter_lock",
                "pair_max_total_cent": pair_lock.max_total_price * 100.0,
                "target_qty": lock.qty,
                "position_avg_cost": lock.avg_cost,
                "counter_cost": lock.counter.cost,
                "total_cost": lock.total_cost,
                "pair_lock_edge": diagnostics,
            }),
            routes: Vec::new(),
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }

    if let Some(pair_total) = pair_lock_edge_pair_total(&up, &down) {
        if pair_total <= pair_lock.max_total_price {
            let (primary, counter_candidate) = pair_lock_edge_lead_counter(&up, &down);
            let counter = ActionPlaceOrderPairResolvedCounterLeg {
                token_id: counter_candidate.token_id.clone(),
                outcome_label: counter_candidate.outcome_label.clone(),
            };
            let primary_node = build_pair_lock_single_leg_node(
                node,
                &market_slug,
                &primary.token_id,
                &primary.outcome_label,
                trigger_node_key,
            );
            let counter_node = build_pair_lock_counter_leg_node(
                node,
                &market_slug,
                &counter,
                pair_lock,
                trigger_node_key,
            );
            let mut primary_context = context.clone();
            let primary_result = execute_pair_lock_edge_share_buy_order(
                repo,
                run_id,
                cfg,
                limits,
                policy,
                client,
                run,
                step,
                &primary_node,
                graph,
                &mut primary_context,
                primary,
                edge_config.decision_qty,
                None,
            )
            .await?;
            let Some(primary_order_id) = primary_result.builder_order_id else {
                return Ok(primary_result.execution);
            };
            let mut counter_context = context.clone();
            let counter_result = execute_pair_lock_edge_share_buy_order(
                repo,
                run_id,
                cfg,
                limits,
                policy,
                client,
                run,
                step,
                &counter_node,
                graph,
                &mut counter_context,
                counter_candidate,
                edge_config.decision_qty,
                None,
            )
            .await?;
            let Some(counter_order_id) = counter_result.builder_order_id else {
                cancel_pair_lock_order_if_created(
                    repo,
                    Some(primary_order_id),
                    "pair_lock_edge_counter_create_blocked",
                )
                .await;
                return Ok(counter_result.execution);
            };
            let pair_session_id =
                create_pair_lock_edge_session(repo, run, node, &market_slug, pair_lock).await?;
            repo.attach_trade_builder_pair_session_orders(
                pair_session_id,
                primary_order_id,
                counter_order_id,
            )
            .await?;
            repo.set_trade_builder_order_pair_session(
                primary_order_id,
                Some(pair_session_id),
                Some(TRADE_BUILDER_PAIR_ROLE_LEAD_CANDIDATE),
            )
            .await?;
            repo.set_trade_builder_order_pair_session(
                counter_order_id,
                Some(pair_session_id),
                Some(TRADE_BUILDER_PAIR_ROLE_COUNTER_CANDIDATE),
            )
            .await?;
            repo.append_trade_flow_event(
                Some(run.id),
                run.definition_id,
                Some(run.version_id),
                "pair_lock_session_created",
                &json!({
                    "node_key": node.key,
                    "pair_lock_strategy": PAIR_LOCK_STRATEGY_EDGE_PAIRLOCK_V1,
                    "pair_lock_edge_decision": "fresh_equal_pair",
                    "pair_session_id": pair_session_id,
                    "market_slug": market_slug,
                    "primary_order_id": primary_order_id,
                    "counter_order_id": counter_order_id,
                    "primary_source_trade_id": primary_result.source_trade_id,
                    "counter_source_trade_id": counter_result.source_trade_id,
                    "trigger_node_key": trigger_node_key,
                    "pair_max_total_cent": pair_lock.max_total_price * 100.0,
                    "pair_total": pair_total,
                    "target_qty": edge_config.decision_qty,
                    "selection": diagnostics,
                }),
            )
            .await?;

            let ref_key = action_place_order_pair_lock_ref_key(node);
            bind_action_place_order_ref_bindings(context, node, &ref_key, primary_order_id);
            if let Some(source_trade_id) = primary_result.source_trade_id {
                set_flow_context(context, "sourceTradeId", json!(source_trade_id));
            }
            set_flow_var(context, &format!("{ref_key}_pair_session_id"), json!(pair_session_id));
            set_flow_var(
                context,
                &format!("{ref_key}_counter_builder_order_id"),
                json!(counter_order_id),
            );
            if let Some(source_trade_id) = counter_result.source_trade_id {
                set_flow_var(
                    context,
                    &format!("{ref_key}_counter_source_trade_id"),
                    json!(source_trade_id),
                );
            }

            return Ok(TradeFlowNodeExecution {
                output: json!({
                    "node_key": node.key,
                    "builder_order_id": primary_order_id,
                    "counter_builder_order_id": counter_order_id,
                    "pair_session_id": pair_session_id,
                    "ref_key": ref_key,
                    "market_slug": market_slug,
                    "token_id": primary.token_id,
                    "counter_token_id": counter.token_id,
                    "outcome_label": primary.outcome_label,
                    "counter_outcome_label": counter.outcome_label,
                    "source_trade_id": primary_result.source_trade_id,
                    "counter_source_trade_id": counter_result.source_trade_id,
                    "trigger_node_key": trigger_node_key,
                    "pair_lock_strategy": PAIR_LOCK_STRATEGY_EDGE_PAIRLOCK_V1,
                    "pair_lock_edge_decision": "fresh_equal_pair",
                    "pair_max_total_cent": pair_lock.max_total_price * 100.0,
                    "pair_total": pair_total,
                    "target_qty": edge_config.decision_qty,
                    "pair_lock_edge": diagnostics,
                }),
                routes: Vec::new(),
                repeat_at: None,
                repeat_idempotency_key: None,
            });
        }
    }

    if let Some(selected) = pair_lock_edge_better_single(
        &up,
        &down,
        edge_config.single_edge_threshold,
    ) {
        let selected_node = build_pair_lock_single_leg_node(
            node,
            &market_slug,
            &selected.token_id,
            &selected.outcome_label,
            trigger_node_key,
        );
        let mut selected_result = execute_pair_lock_edge_share_buy_order(
            repo,
            run_id,
            cfg,
            limits,
            policy,
            client,
            run,
            step,
            &selected_node,
            graph,
            context,
            selected,
            edge_config.decision_qty,
            None,
        )
        .await?;
        if selected_result.builder_order_id.is_some() {
            repo.append_trade_flow_event(
                Some(run.id),
                run.definition_id,
                Some(run.version_id),
                "pair_lock_edge_single_order_created",
                &json!({
                    "node_key": node.key,
                    "pair_lock_strategy": PAIR_LOCK_STRATEGY_EDGE_PAIRLOCK_V1,
                    "pair_lock_edge_decision": "single_edge",
                    "builder_order_id": selected_result.builder_order_id,
                    "source_trade_id": selected_result.source_trade_id,
                    "market_slug": market_slug,
                    "token_id": selected.token_id,
                    "outcome_label": selected.outcome_label,
                    "target_qty": edge_config.decision_qty,
                    "single_edge_threshold": edge_config.single_edge_threshold,
                    "edge": selected.edge,
                    "selection": diagnostics,
                }),
            )
            .await?;
        }
        if let Some(output) = selected_result.execution.output.as_object_mut() {
            output.insert("pair_lock_edge_decision".to_string(), json!("single_edge"));
            output.insert("pair_lock_edge".to_string(), diagnostics);
        }
        return Ok(selected_result.execution);
    }

    Ok(pair_lock_edge_waiting_execution(
        &node.key,
        &market_slug,
        diagnostics,
    ))
}

#[cfg(test)]
mod pair_lock_edge_strategy_tests {
    use super::*;

    fn edge_test_quote(ask: f64) -> PairLockResolvedQuote {
        PairLockResolvedQuote {
            best_bid: Some((ask - 0.01).max(0.01)),
            best_ask: Some(ask),
            last_trade_price: Some(ask),
            current_price: ask,
            quote_source_kind: "test",
            quote_ws_state: "test",
            quote_event_ts: None,
            quote_snapshot_age_ms: None,
            quote_source_detail: "test".to_string(),
            quote_book_missing_fields: Vec::new(),
            quote_snapshot_used: json!({}),
        }
    }

    fn edge_test_eval(label: &str, ask: f64, q_final: f64) -> ActionPlaceOrderPairLockPrimaryCandidateEval {
        ActionPlaceOrderPairLockPrimaryCandidateEval {
            token_id: format!("tok-{}", label.to_ascii_lowercase()),
            outcome_label: label.to_string(),
            decision: "blocked",
            reason_code: "blocked_edge_below_threshold".to_string(),
            quote: edge_test_quote(ask),
            diagnostics: json!({
                "price_to_beat_guard": {
                    "threshold_mode": "iv_mismatch_edge",
                    "iv_mismatch_edge": {
                        "ask": ask,
                        "q": q_final - 0.01,
                        "q_final": q_final
                    }
                }
            }),
        }
    }

    #[test]
    fn edge_candidate_recomputes_cost_with_strategy_buffer() {
        let candidate =
            pair_lock_edge_candidate_from_eval(edge_test_eval("Up", 0.40, 0.60), 0.005);

        let expected_cost = 0.40 + PAIR_LOCK_EDGE_TAKER_FEE_RATE * 0.40 * 0.60 + 0.005;
        assert!((candidate.cost.unwrap() - expected_cost).abs() < 0.000001);
        assert!((candidate.edge.unwrap() - (0.60 - expected_cost)).abs() < 0.000001);
    }

    #[test]
    fn edge_strategy_decision_order_prefers_pair_total_then_single_edge() {
        let up = pair_lock_edge_candidate_from_eval(edge_test_eval("Up", 0.42, 0.80), 0.005);
        let down = pair_lock_edge_candidate_from_eval(edge_test_eval("Down", 0.45, 0.30), 0.005);

        assert!(pair_lock_edge_pair_total(&up, &down).unwrap() <= 0.95);
        assert_eq!(
            pair_lock_edge_better_single(&up, &down, 0.10)
                .unwrap()
                .outcome_label,
            "Up"
        );
    }
}
