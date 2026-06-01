const ACTION_PLACE_ORDER_MODE_POSITIVE_QUANTITY_FLIP_GRID_V1: &str =
    "positive_quantity_flip_grid_v1";
const ACTION_PLACE_ORDER_MODE_POSITIVE_FLIP_PAIRLOCK_COMPRESSION_V1: &str =
    "positive_flip_pairlock_compression_v1";
use bot_infra::db::{
    TradeBuilderPositiveQuantityFlipGridFillInput, TradeBuilderPositiveQuantityFlipGridState,
};
const POSITIVE_QUANTITY_FLIP_GRID_BINDING_MODE: &str = "positive_quantity_flip_grid_only";
const POSITIVE_QUANTITY_FLIP_GRID_CONFIG_KEY: &str = "positiveQuantityFlipGrid";
const POSITIVE_QUANTITY_FLIP_GRID_ORDER_MARKER_KEY: &str = "positiveQuantityFlipGridOrder";
const POSITIVE_QUANTITY_FLIP_GRID_ROOT_NODE_KEY: &str = "positiveQuantityFlipGridRootNodeKey";
const POSITIVE_QUANTITY_FLIP_GRID_SIDE_KEY: &str = "positiveQuantityFlipGridSide";
const POSITIVE_QUANTITY_FLIP_GRID_INTENT_KEY: &str = "positiveQuantityFlipGridIntent";
const POSITIVE_QUANTITY_FLIP_GRID_MIN_MARKETABLE_BUY_USDC: f64 = 1.01;
const POSITIVE_QUANTITY_FLIP_GRID_DEFAULT_PROFIT_TARGET_USDC: f64 = 1.0;
const POSITIVE_QUANTITY_FLIP_GRID_DEFAULT_SIZING_BUFFER_CENT: f64 = 3.0;
const POSITIVE_QUANTITY_FLIP_GRID_DEFAULT_PARTIAL_MIN_LOSS_REDUCTION_USDC: f64 = 0.10;
const POSITIVE_QUANTITY_FLIP_GRID_DEFAULT_PARTIAL_BALANCE_RESERVE_USDC: f64 = 1.0;
static POSITIVE_QUANTITY_FLIP_GRID_EXECUTION_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> =
    std::sync::OnceLock::new();

#[derive(Debug, Clone)]
struct PositiveQuantityFlipGridQuote {
    grid_side: &'static str,
    token_id: String,
    outcome_label: String,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    quote_snapshot: Value,
}

#[derive(Debug, Clone)]
struct PositiveQuantityFlipGridBuyCandidate {
    quote: PositiveQuantityFlipGridQuote,
    ask_price: f64,
    effective_ask_price: f64,
    sizing_ask_price: f64,
    worst_price: f64,
    required_buy_usdc: f64,
    actual_buy_usdc: f64,
    target_qty: f64,
    projected_side_qty: f64,
    projected_net_cost: f64,
    projected_pnl_at_exit: f64,
    pre_pnl_at_exit: f64,
    preferred_band_rank: i32,
    depth_result: Value,
    rescue_buy: bool,
    partial_recovery: bool,
    partial_recovery_details: Option<Value>,
}

fn action_place_order_uses_positive_quantity_flip_grid(node: &TradeFlowNode) -> bool {
    action_place_order_positive_grid_mode(node).is_some()
}

fn action_place_order_positive_grid_mode(node: &TradeFlowNode) -> Option<&'static str> {
    match node_config_string(node, "mode")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        ACTION_PLACE_ORDER_MODE_POSITIVE_QUANTITY_FLIP_GRID_V1 => {
            Some(ACTION_PLACE_ORDER_MODE_POSITIVE_QUANTITY_FLIP_GRID_V1)
        }
        ACTION_PLACE_ORDER_MODE_POSITIVE_FLIP_PAIRLOCK_COMPRESSION_V1 => {
            Some(ACTION_PLACE_ORDER_MODE_POSITIVE_FLIP_PAIRLOCK_COMPRESSION_V1)
        }
        _ => None,
    }
}

fn action_place_order_positive_grid_mode_or_default(node: &TradeFlowNode) -> &'static str {
    action_place_order_positive_grid_mode(node)
        .unwrap_or(ACTION_PLACE_ORDER_MODE_POSITIVE_QUANTITY_FLIP_GRID_V1)
}

fn positive_quantity_flip_grid_execution_lock() -> &'static tokio::sync::Mutex<()> {
    POSITIVE_QUANTITY_FLIP_GRID_EXECUTION_LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}


fn positive_quantity_flip_grid_required_buy_usdc(
    net_cost: f64,
    min_profit: f64,
    exit_price: f64,
    current_side_qty: f64,
    ask_price: f64,
) -> Option<f64> {
    let denominator = exit_price / ask_price - 1.0;
    if !denominator.is_finite() || denominator <= 0.0 {
        return None;
    }
    let required = (net_cost + min_profit - exit_price * current_side_qty) / denominator;
    Some(required.max(0.0))
}

fn positive_quantity_flip_grid_round_up_cent(value: f64) -> f64 {
    if !value.is_finite() || value <= 0.0 {
        return 0.0;
    }
    (value * 100.0).ceil() / 100.0
}

fn positive_quantity_flip_grid_round_up_share_qty(value: f64) -> f64 {
    if !value.is_finite() || value <= 0.0 {
        return 0.0;
    }
    round_trade_builder_share_qty((value * 100.0).ceil() / 100.0)
}

fn positive_quantity_flip_grid_side_qty(
    state: &TradeBuilderPositiveQuantityFlipGridState,
    grid_side: &str,
) -> f64 {
    if grid_side == "down" {
        state.down_qty
    } else {
        state.up_qty
    }
}

fn positive_quantity_flip_grid_pnl_at_exit(side_qty: f64, exit_price: f64, net_cost: f64) -> f64 {
    side_qty * exit_price - net_cost
}

fn positive_quantity_flip_grid_remaining_sec(market_slug: &str) -> Option<i64> {
    live_gap_collector_remaining_sec(market_slug, Utc::now())
}

fn positive_quantity_flip_grid_binding_trigger_key(
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
) -> Result<String> {
    let trigger_key = find_upstream_market_price_trigger_key(&node.key, graph).ok_or_else(|| {
        anyhow::anyhow!(
            "action.place_order positive_quantity_flip_grid_v1 requires upstream trigger.market_price bindingMode=positive_quantity_flip_grid_only"
        )
    })?;
    let trigger_node = flow_node(graph, &trigger_key).ok_or_else(|| {
        anyhow::anyhow!("positive_quantity_flip_grid_v1 upstream trigger node not found")
    })?;
    let binding_mode = node_config_string(trigger_node, "bindingMode")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    anyhow::ensure!(
        binding_mode == POSITIVE_QUANTITY_FLIP_GRID_BINDING_MODE,
        "action.place_order positive_quantity_flip_grid_v1 requires upstream trigger.market_price bindingMode=positive_quantity_flip_grid_only"
    );
    Ok(trigger_key)
}

fn positive_quantity_flip_grid_market_slug(
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
    context: &Value,
) -> Result<String> {
    step_input_string(step, &["marketSlug", "market_slug", "wsMarketSlug"])
        .or_else(|| flow_context_string(context, "marketSlug"))
        .or_else(|| node_config_string(node, "marketSlug"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("positive_quantity_flip_grid_v1 requires marketSlug"))
}

fn positive_quantity_flip_grid_quote_from_resolved(
    grid_side: &'static str,
    token_id: String,
    outcome_label: String,
    quote: PairLockResolvedQuote,
) -> PositiveQuantityFlipGridQuote {
    PositiveQuantityFlipGridQuote {
        grid_side,
        token_id,
        outcome_label,
        best_bid: quote.best_bid,
        best_ask: quote.best_ask,
        quote_snapshot: quote.quote_snapshot_used,
    }
}

#[cfg(test)]
fn positive_quantity_flip_grid_buy_candidate(
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
    quote: PositiveQuantityFlipGridQuote,
    completion_only: bool,
    order_book: Option<&OrderBookSnapshot>,
) -> Option<PositiveQuantityFlipGridBuyCandidate> {
    positive_quantity_flip_grid_evaluate_buy_candidate(
        config,
        state,
        quote,
        completion_only,
        order_book,
    )
    .candidate
}

fn positive_quantity_flip_grid_exit_side(
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
    quotes: &[PositiveQuantityFlipGridQuote],
) -> Option<(&'static str, f64, f64, f64)> {
    quotes
        .iter()
        .filter_map(|quote| {
            let bid = quote.best_bid?;
            if bid + 0.000001 < config.sell_bid_min {
                return None;
            }
            let side_qty = positive_quantity_flip_grid_side_qty(state, quote.grid_side);
            if side_qty <= 0.000001 {
                return None;
            }
            let pnl = side_qty * bid - state.net_cost;
            let min_profit = if config.pairlock_compression_enabled {
                config.min_direct_profit_usdc
            } else {
                config.min_sell_net_profit_usdc
            };
            (pnl + 0.000001 >= min_profit).then_some((
                quote.grid_side,
                bid,
                pnl,
                side_qty,
            ))
        })
        .max_by(|left, right| left.2.total_cmp(&right.2))
}

async fn positive_quantity_flip_grid_parent_has_active_sell(
    repo: &PostgresRepository,
    parent_builder_order_id: i64,
) -> Result<bool> {
    Ok(repo
        .list_trade_builder_child_orders_by_parent(parent_builder_order_id, None)
        .await?
        .into_iter()
        .any(|order| {
            order.side == "sell" && is_trade_builder_order_processable_status(&order.status)
        }))
}

#[allow(clippy::too_many_arguments)]
async fn positive_quantity_flip_grid_execute_sell_side(
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
    config: &PositiveQuantityFlipGridConfig,
    market_slug: &str,
    grid_side: &str,
    bid_price: f64,
) -> Result<Vec<Value>> {
    let positions = repo
        .list_open_positive_quantity_flip_grid_positions(
            run.user_id,
            Some(run.definition_id),
            &node.key,
            market_slug,
            grid_side,
        )
        .await?;
    let mut outputs = Vec::new();
    for (index, position) in positions.iter().enumerate() {
        if positive_quantity_flip_grid_parent_has_active_sell(
            repo,
            position.parent_builder_order_id,
        )
        .await?
        {
            outputs.push(json!({
                "parent_builder_order_id": position.parent_builder_order_id,
                "skipped": true,
                "reason": "active_sell_exists",
            }));
            continue;
        }
        let sell_node = positive_quantity_flip_grid_sell_node(
            node,
            config,
            position,
            Utc::now().timestamp_millis() + index as i64,
        );
        let sell_step = positive_quantity_flip_grid_sell_step(step, position, bid_price);
        let execution = execute_action_place_order(
            repo, run_id, cfg, limits, policy, client, run, &sell_step, &sell_node, graph, context,
        )
        .await?;
        outputs.push(execution.output);
    }
    Ok(outputs)
}

fn positive_quantity_flip_grid_marker_config(payload: &Value) -> Option<&Value> {
    payload
        .pointer("/node_snapshot/action_node/config")
        .filter(|config| {
            config
                .get(POSITIVE_QUANTITY_FLIP_GRID_ORDER_MARKER_KEY)
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
}

fn positive_quantity_flip_grid_marker_string(config: &Value, key: &str) -> Option<String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn positive_quantity_flip_grid_resolve_fill_parent_builder_order_id(
    order_id: i64,
    parent_order_id: Option<i64>,
    order_side: &str,
    parent_order: Option<i64>,
) -> Option<i64> {
    parent_order_id
        .or(parent_order)
        .or_else(|| (order_side == "buy").then_some(order_id))
}

async fn maybe_record_positive_quantity_flip_grid_fill(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    parent_order: Option<&TradeBuilderOrder>,
    flow_created_payload: Option<&Value>,
    fill_qty: f64,
    execution_price: f64,
) -> Result<()> {
    let Some(marker_config) =
        flow_created_payload.and_then(positive_quantity_flip_grid_marker_config)
    else {
        return Ok(());
    };
    let root_node_key = positive_quantity_flip_grid_marker_string(
        marker_config,
        POSITIVE_QUANTITY_FLIP_GRID_ROOT_NODE_KEY,
    )
    .unwrap_or_else(|| {
        order
            .origin_flow_node_key
            .clone()
            .unwrap_or_else(|| order.market_slug.clone())
    });
    let grid_side = positive_quantity_flip_grid_marker_string(
        marker_config,
        POSITIVE_QUANTITY_FLIP_GRID_SIDE_KEY,
    )
    .unwrap_or_else(|| {
        if normalize_pair_lock_binary_outcome(&order.outcome_label) == Some("no") {
            "down".to_string()
        } else {
            "up".to_string()
        }
    });
    if grid_side != "up" && grid_side != "down" {
        return Ok(());
    }
    let parent_builder_order_id =
        positive_quantity_flip_grid_resolve_fill_parent_builder_order_id(
            order.id,
            order.parent_order_id,
            &order.side,
            parent_order.map(|parent| parent.id),
        );
    repo.record_positive_quantity_flip_grid_fill(
        &TradeBuilderPositiveQuantityFlipGridFillInput {
            user_id: order.user_id,
            flow_definition_id: order.origin_flow_definition_id,
            flow_run_id: order.origin_flow_run_id,
            root_flow_node_key: root_node_key,
            market_slug: order.market_slug.clone(),
            token_id: order.token_id.clone(),
            outcome_label: order.outcome_label.clone(),
            grid_side,
            order_side: order.side.clone(),
            builder_order_id: order.id,
            parent_builder_order_id,
            quantity: fill_qty.max(0.0),
            execution_price: clamp_probability(execution_price.max(0.0)),
            notional_usdc: (fill_qty.max(0.0) * execution_price.max(0.0)).max(0.0),
            payload_json: json!({
                "builder_order_id": order.id,
                "parent_builder_order_id": parent_builder_order_id,
                "intent": positive_quantity_flip_grid_marker_string(marker_config, POSITIVE_QUANTITY_FLIP_GRID_INTENT_KEY),
                "flow_created": flow_created_payload,
            }),
        },
    )
    .await?;
    repo.append_trade_builder_order_event(
        order.id,
        "positive_quantity_flip_grid_fill_recorded",
        &json!({
            "grid_side": positive_quantity_flip_grid_marker_string(marker_config, POSITIVE_QUANTITY_FLIP_GRID_SIDE_KEY),
            "order_side": order.side,
            "quantity": fill_qty,
            "execution_price": execution_price,
        }),
    )
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn execute_action_place_order_positive_quantity_flip_grid(
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
    let action_mode = action_place_order_positive_grid_mode_or_default(node);
    let config = resolve_positive_quantity_flip_grid_config(node)?;
    let trigger_key = positive_quantity_flip_grid_binding_trigger_key(node, graph)?;
    let market_slug = positive_quantity_flip_grid_market_slug(node, step, context)?;
    let _execution_guard = positive_quantity_flip_grid_execution_lock().lock().await;
    let Some(remaining_sec) = positive_quantity_flip_grid_remaining_sec(&market_slug) else {
        return Ok(positive_quantity_flip_grid_output_skipped(
            node,
            &market_slug,
            "unsupported_market_window",
            json!({}),
        ));
    };
    let Some(client) = client else {
        return Ok(positive_quantity_flip_grid_output_skipped(
            node,
            &market_slug,
            "order_executor_unavailable",
            json!({}),
        ));
    };
    let resolved_tokens =
        resolve_pair_lock_trigger_scoped_token_pair(cfg, &market_slug, &trigger_key, context)
            .await?;
    let (up_label, down_label) = pair_lock_monitor_outcome_labels(Some(&market_slug));
    let up_quote = resolve_pair_lock_action_candidate_quote(
        ws,
        client,
        step,
        &resolved_tokens.yes_token_id,
        up_label,
        None,
    )
    .await;
    let down_quote = resolve_pair_lock_action_candidate_quote(
        ws,
        client,
        step,
        &resolved_tokens.no_token_id,
        down_label,
        None,
    )
    .await;
    let quotes = vec![
        positive_quantity_flip_grid_quote_from_resolved(
            "up",
            resolved_tokens.yes_token_id.clone(),
            up_label.to_string(),
            up_quote,
        ),
        positive_quantity_flip_grid_quote_from_resolved(
            "down",
            resolved_tokens.no_token_id.clone(),
            down_label.to_string(),
            down_quote,
        ),
    ];
    let state = repo
        .load_positive_quantity_flip_grid_state(
            run.user_id,
            Some(run.definition_id),
            &node.key,
            &market_slug,
        )
        .await?;

    if config.pairlock_compression_enabled {
        if let Some(execution) = positive_flip_pairlock_try_basket_exit(
            repo,
            run_id,
            cfg,
            limits,
            policy,
            client,
            run,
            step,
            node,
            graph,
            context,
            &config,
            &state,
            &market_slug,
            &quotes,
        )
        .await?
        {
            return Ok(execution);
        }
        if let Some(execution) = positive_flip_pairlock_try_compression(
            repo,
            run_id,
            cfg,
            limits,
            policy,
            client,
            run,
            step,
            node,
            graph,
            context,
            &config,
            &state,
            &market_slug,
            &quotes,
        )
        .await?
        {
            return Ok(execution);
        }
    }

    if let Some((exit_side, bid_price, projected_profit, open_side_qty)) =
        positive_quantity_flip_grid_exit_side(&config, &state, &quotes)
    {
        let sell_outputs = positive_quantity_flip_grid_execute_sell_side(
            repo,
            run_id,
            cfg,
            limits,
            policy,
            Some(client),
            run,
            step,
            node,
            graph,
            context,
            &config,
            &market_slug,
            exit_side,
            bid_price,
        )
        .await?;
        return Ok(TradeFlowNodeExecution {
            output: json!({
                "node_key": node.key,
                "mode": action_mode,
                "market_slug": market_slug,
                "decision": "sell_near_certain",
                "exit_side": exit_side,
                "bid_price": bid_price,
                "open_side_qty": open_side_qty,
                "net_cost": state.net_cost,
                "projected_profit_usdc": projected_profit,
                "state": {
                    "up_qty": state.up_qty,
                    "down_qty": state.down_qty,
                    "total_buy_cost": state.total_buy_cost,
                    "total_sell_revenue": state.total_sell_revenue,
                    "total_merge_return": state.total_merge_return,
                    "net_cost": state.net_cost,
                    "buy_count": state.buy_count,
                },
                "sell_outputs": sell_outputs,
            }),
            routes: vec![TradeFlowRouteDecision {
                edge_type: "on_success".to_string(),
                available_at: Utc::now(),
            }],
            repeat_at: None,
            repeat_idempotency_key: None,
        });
    }

    if remaining_sec < config.no_new_buy_under_sec {
        return Ok(positive_quantity_flip_grid_output_skipped(
            node,
            &market_slug,
            "no_new_buy_under_sec",
            json!({ "remaining_sec": remaining_sec }),
        ));
    }
    if state.buy_count >= config.max_open_grid_buys_per_market {
        return Ok(positive_quantity_flip_grid_output_skipped(
            node,
            &market_slug,
            "max_open_grid_buys_per_market",
            json!({ "buy_count": state.buy_count }),
        ));
    }
    let completion_only = remaining_sec < config.new_grid_buy_end_remaining_sec;
    if let Some(max_total_spent_per_market_usdc) = config.max_total_spent_per_market_usdc {
        if state.total_buy_cost >= max_total_spent_per_market_usdc
            && !(config.partial_recovery_enabled
                && config.partial_recovery_ignore_market_budget
                && completion_only)
        {
            return Ok(positive_quantity_flip_grid_output_skipped(
                node,
                &market_slug,
                "max_total_spent_per_market",
                json!({ "total_buy_cost": state.total_buy_cost }),
            ));
        }
    }
    let (active_market_count, current_market_active) = repo
        .positive_quantity_flip_grid_open_market_usage(
            run.user_id,
            Some(run.definition_id),
            &node.key,
            &market_slug,
        )
        .await?;
    if !current_market_active && active_market_count >= config.max_active_markets {
        return Ok(positive_quantity_flip_grid_output_skipped(
            node,
            &market_slug,
            "max_active_markets",
            json!({
                "active_market_count": active_market_count,
                "max_active_markets": config.max_active_markets,
            }),
        ));
    }

    if let Some(details) = positive_quantity_flip_grid_cycle_window_skip_for_market(
        &config,
        &market_slug,
        remaining_sec,
    ) {
        return Ok(positive_quantity_flip_grid_output_skipped(
            node,
            &market_slug,
            "cycle_window",
            details,
        ));
    }
    if config.cycle_window_mode.is_none() && remaining_sec > config.new_grid_buy_start_remaining_sec
    {
        return Ok(positive_quantity_flip_grid_output_skipped(
            node,
            &market_slug,
            "before_grid_buy_window",
            json!({ "remaining_sec": remaining_sec }),
        ));
    }
    if remaining_sec < config.positive_completion_buy_end_remaining_sec {
        return Ok(positive_quantity_flip_grid_output_skipped(
            node,
            &market_slug,
            "after_completion_buy_window",
            json!({ "remaining_sec": remaining_sec }),
        ));
    }
    if positive_flip_pairlock_buys_blocked_after_profit_lock(&config, &state) {
        return Ok(positive_quantity_flip_grid_output_skipped(
            node,
            &market_slug,
            "pairlock_profit_locked_stop_buys",
            json!({
                "total_merge_return": state.total_merge_return,
                "up_qty": state.up_qty,
                "down_qty": state.down_qty,
            }),
        ));
    }

    let selection = positive_quantity_flip_grid_select_buy_candidate(
        &config,
        &state,
        step,
        &market_slug,
        &quotes,
        completion_only,
        client,
    )
    .await?;
    let Some(candidate) = selection.candidate else {
        return Ok(positive_quantity_flip_grid_output_skipped(
            node,
            &market_slug,
            "no_buy_candidate",
            json!({
                "remaining_sec": remaining_sec,
                "completion_only": completion_only,
                "quotes": selection.guard_reports,
            }),
        ));
    };

    let buy_lock = match positive_quantity_flip_grid_prepare_buy_order_submission(
        repo,
        run,
        node,
        &market_slug,
        &state,
    )
    .await?
    {
        Ok(lock) => lock,
        Err(skip) => return Ok(skip),
    };

    let buy_node = positive_quantity_flip_grid_buy_node(
        node,
        &config,
        &market_slug,
        &candidate,
        Utc::now().timestamp_millis(),
        positive_flip_pairlock_positive_buy_intent(&config, &state),
        None,
    );
    let buy_step = positive_quantity_flip_grid_step_with_price(
        step,
        &market_slug,
        &candidate.quote.token_id,
        &candidate.quote.outcome_label,
        candidate.sizing_ask_price,
    );
    clear_action_place_order_ref_bindings(
        context,
        &buy_node,
        &action_place_order_ref_key(&buy_node),
    );
    let execution = execute_action_place_order(
        repo,
        run_id,
        cfg,
        limits,
        policy,
        Some(client),
        run,
        &buy_step,
        &buy_node,
        graph,
        context,
    )
    .await?;
    buy_lock.release().await;

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "mode": action_mode,
            "market_slug": market_slug,
            "decision": "buy",
            "quantity_sizing_mode": config.quantity_sizing_mode.as_str(),
            "grid_side": candidate.quote.grid_side,
            "ask_price": candidate.ask_price,
            "effective_ask_price": candidate.effective_ask_price,
            "sizing_ask_price": candidate.sizing_ask_price,
            "sizing_price_buffer_cent": config.sizing_price_buffer_cent,
            "worst_price": candidate.worst_price,
            "required_buy_usdc": candidate.required_buy_usdc,
            "actual_buy_usdc": candidate.actual_buy_usdc,
            "target_qty": candidate.target_qty,
            "projected_side_qty": candidate.projected_side_qty,
            "projected_net_cost": candidate.projected_net_cost,
            "projected_pnl_at_exit": candidate.projected_pnl_at_exit,
            "pre_pnl_at_exit": candidate.pre_pnl_at_exit,
            "rescue_buy": candidate.rescue_buy,
            "partial_recovery": candidate.partial_recovery,
            "partial_recovery_details": candidate.partial_recovery_details,
            "depth": candidate.depth_result,
            "remaining_sec": remaining_sec,
            "completion_only": completion_only,
            "child_output": execution.output,
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}
