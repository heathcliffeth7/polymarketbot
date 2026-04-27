const BIASED_HEDGE_DEFAULT_PRIMARY_MIN_EDGE: f64 = 0.08;
const BIASED_HEDGE_DEFAULT_PRIMARY_MIN_FINAL_Q: f64 = 0.72;
const BIASED_HEDGE_DEFAULT_HEDGE_MIN_PRICE_CENT: f64 = 3.0;
const BIASED_HEDGE_DEFAULT_HEDGE_MAX_PRICE_CENT: f64 = 25.0;
const BIASED_HEDGE_DEFAULT_HIGH_PRICE_CENT: f64 = 70.0;
const BIASED_HEDGE_DEFAULT_HIGH_PRICE_MIN_FINAL_Q: f64 = 0.82;
const BIASED_HEDGE_DEFAULT_HIGH_PRICE_MIN_EDGE: f64 = 0.10;
const BIASED_HEDGE_DEFAULT_MIN_DOMINANT_SHARE: f64 = 0.75;
const BIASED_HEDGE_DEFAULT_MAX_HEDGE_SPEND_RATIO: f64 = 0.25;
const BIASED_HEDGE_DEFAULT_ENTRY_START_SEC: i64 = 30;
const BIASED_HEDGE_DEFAULT_DISABLE_NEW_PRIMARY_AFTER_SEC: i64 = 180;
const BIASED_HEDGE_DEFAULT_DISABLE_ANY_BUY_AFTER_SEC: i64 = 240;
const BIASED_HEDGE_MONITOR_INTERVAL_SEC: i64 = 10;

#[derive(Debug, Clone)]
struct ActionPlaceOrderBiasedHedgeConfig {
    primary_budget_usdc: f64,
    hedge_budget_usdc: f64,
    min_dominant_share: f64,
    max_hedge_spend_ratio: f64,
    primary_min_edge: f64,
    primary_min_final_q: f64,
    hedge_max_price: f64,
    hedge_min_price: f64,
    hedge_only_if_primary_filled: bool,
    disable_new_primary_after_sec: i64,
    disable_any_buy_after_sec: i64,
    max_side_switch_count: i64,
    high_price: f64,
    high_price_min_final_q: f64,
    high_price_min_edge: f64,
    max_paired_effective_cost: Option<f64>,
    stop: ActionPlaceOrderBiasedHedgeStopConfig,
}

#[derive(Debug, Clone)]
struct ActionPlaceOrderBiasedHedgeStopConfig {
    bias_invalidation_enabled: bool,
    min_q_final_to_hold: f64,
    min_edge_to_hold: f64,
    exit_pct_on_invalidation: f64,
    ptb_stop_loss_enabled: bool,
    ptb_stop_loss_gap_usd: Option<f64>,
    ptb_stop_loss_time_decay_mode: Option<String>,
    time_exit_rules: Vec<ActionPlaceOrderBiasedHedgeTimeExitRule>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActionPlaceOrderBiasedHedgeTimeExitRule {
    elapsed_sec: i64,
    remaining_pct: f64,
}

#[derive(Debug, Clone, Copy)]
struct BiasedHedgeMarketTiming {
    elapsed_sec: Option<i64>,
    remaining_sec: Option<i64>,
    cycle_window_active: bool,
}

fn biased_hedge_object<'a>(node: &'a TradeFlowNode, key: &str) -> Option<&'a Value> {
    node.config.get(key).filter(|value| value.is_object())
}

fn biased_hedge_f64(raw: Option<&Value>, key: &str) -> Option<f64> {
    raw.and_then(|value| value.get(key)).and_then(value_as_f64)
}

fn biased_hedge_bool(raw: Option<&Value>, key: &str) -> Option<bool> {
    raw.and_then(|value| value.get(key)).and_then(Value::as_bool)
}

fn biased_hedge_i64(raw: Option<&Value>, key: &str) -> Option<i64> {
    raw.and_then(|value| value.get(key)).and_then(Value::as_i64)
}

fn resolve_action_place_order_biased_hedge_config(
    node: &TradeFlowNode,
) -> Result<Option<ActionPlaceOrderBiasedHedgeConfig>> {
    if !action_place_order_uses_biased_hedge_strategy(node) {
        return Ok(None);
    }
    let biased = biased_hedge_object(node, "biasedHedge");
    let stop = biased_hedge_object(node, "biasedHedgeStop");
    let primary_budget_usdc = biased_hedge_f64(biased, "primaryBudgetUsdc")
        .or_else(|| node_config_f64(node, "sizeUsdc"))
        .ok_or_else(|| anyhow::anyhow!("biased_hedge_v1 requires biasedHedge.primaryBudgetUsdc > 0"))?;
    let hedge_budget_usdc = biased_hedge_f64(biased, "hedgeBudgetUsdc")
        .ok_or_else(|| anyhow::anyhow!("biased_hedge_v1 requires biasedHedge.hedgeBudgetUsdc > 0"))?;
    let min_dominant_share =
        biased_hedge_f64(biased, "minDominantShare").unwrap_or(BIASED_HEDGE_DEFAULT_MIN_DOMINANT_SHARE);
    let max_hedge_spend_ratio = biased_hedge_f64(biased, "maxHedgeSpendRatio")
        .unwrap_or(BIASED_HEDGE_DEFAULT_MAX_HEDGE_SPEND_RATIO);
    let primary_min_edge =
        biased_hedge_f64(biased, "primaryMinEdge").unwrap_or(BIASED_HEDGE_DEFAULT_PRIMARY_MIN_EDGE);
    let primary_min_final_q = biased_hedge_f64(biased, "primaryMinFinalQ")
        .unwrap_or(BIASED_HEDGE_DEFAULT_PRIMARY_MIN_FINAL_Q);
    let hedge_max_price =
        biased_hedge_f64(biased, "hedgeMaxPriceCent").unwrap_or(BIASED_HEDGE_DEFAULT_HEDGE_MAX_PRICE_CENT) / 100.0;
    let hedge_min_price =
        biased_hedge_f64(biased, "hedgeMinPriceCent").unwrap_or(BIASED_HEDGE_DEFAULT_HEDGE_MIN_PRICE_CENT) / 100.0;
    let high_price =
        biased_hedge_f64(biased, "highPriceCent").unwrap_or(BIASED_HEDGE_DEFAULT_HIGH_PRICE_CENT) / 100.0;
    let high_price_min_final_q = biased_hedge_f64(biased, "highPriceMinFinalQ")
        .unwrap_or(BIASED_HEDGE_DEFAULT_HIGH_PRICE_MIN_FINAL_Q);
    let high_price_min_edge =
        biased_hedge_f64(biased, "highPriceMinEdge").unwrap_or(BIASED_HEDGE_DEFAULT_HIGH_PRICE_MIN_EDGE);
    let disable_new_primary_after_sec = biased_hedge_i64(biased, "disableNewPrimaryAfterSec")
        .unwrap_or(BIASED_HEDGE_DEFAULT_DISABLE_NEW_PRIMARY_AFTER_SEC);
    let disable_any_buy_after_sec = biased_hedge_i64(biased, "disableAnyBuyAfterSec")
        .unwrap_or(BIASED_HEDGE_DEFAULT_DISABLE_ANY_BUY_AFTER_SEC);
    let max_side_switch_count = biased_hedge_i64(biased, "maxSideSwitchCount").unwrap_or(0);
    let hedge_only_if_primary_filled =
        biased_hedge_bool(biased, "hedgeOnlyIfPrimaryFilled").unwrap_or(true);
    let max_paired_effective_cost =
        node_config_f64(node, "biasedHedgeMaxPairedEffectiveCostCent").map(|value| value / 100.0);

    let stop = stop.ok_or_else(|| anyhow::anyhow!("biased_hedge_v1 requires biasedHedgeStop"))?;
    let time_exit_rules: Vec<ActionPlaceOrderBiasedHedgeTimeExitRule> = serde_json::from_value(
        stop.get("timeExitRules")
            .cloned()
            .unwrap_or_else(|| json!([])),
    )
    .context("biasedHedgeStop.timeExitRules must be an array")?;
    let stop_config = ActionPlaceOrderBiasedHedgeStopConfig {
        bias_invalidation_enabled:
            biased_hedge_bool(Some(stop), "biasInvalidationEnabled").unwrap_or(true),
        min_q_final_to_hold: biased_hedge_f64(Some(stop), "minQFinalToHold").unwrap_or(0.55),
        min_edge_to_hold: biased_hedge_f64(Some(stop), "minEdgeToHold").unwrap_or(0.0),
        exit_pct_on_invalidation:
            biased_hedge_f64(Some(stop), "exitPctOnInvalidation").unwrap_or(100.0),
        ptb_stop_loss_enabled:
            biased_hedge_bool(Some(stop), "ptbStopLossEnabled").unwrap_or(true),
        ptb_stop_loss_gap_usd: biased_hedge_f64(Some(stop), "ptbStopLossGapUsd"),
        ptb_stop_loss_time_decay_mode: stop
            .get("ptbStopLossTimeDecayMode")
            .and_then(Value::as_str)
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| matches!(value.as_str(), "none" | "tighten" | "relax")),
        time_exit_rules,
    };

    anyhow::ensure!(primary_budget_usdc > 0.0, "biasedHedge.primaryBudgetUsdc must be > 0");
    anyhow::ensure!(hedge_budget_usdc > 0.0, "biasedHedge.hedgeBudgetUsdc must be > 0");
    anyhow::ensure!(
        hedge_budget_usdc < primary_budget_usdc,
        "biasedHedge.hedgeBudgetUsdc must be less than primaryBudgetUsdc"
    );
    anyhow::ensure!(min_dominant_share >= 0.70 && min_dominant_share < 1.0, "biasedHedge.minDominantShare must be in [0.70, 1)");
    anyhow::ensure!(max_hedge_spend_ratio > 0.0 && max_hedge_spend_ratio <= 0.35, "biasedHedge.maxHedgeSpendRatio must be in (0, 0.35]");
    anyhow::ensure!(
        max_hedge_spend_ratio <= (1.0 - min_dominant_share) / min_dominant_share + 0.000000001,
        "biasedHedge.maxHedgeSpendRatio cannot break minDominantShare"
    );
    anyhow::ensure!(hedge_only_if_primary_filled, "biasedHedge.hedgeOnlyIfPrimaryFilled must be true");
    anyhow::ensure!(hedge_min_price > 0.0 && hedge_min_price < hedge_max_price, "biasedHedge hedge price range is invalid");
    anyhow::ensure!(high_price_min_final_q >= 0.78, "biasedHedge.highPriceMinFinalQ must be >= 0.78");
    anyhow::ensure!(max_side_switch_count <= 1, "biasedHedge.maxSideSwitchCount must be <= 1");
    anyhow::ensure!(disable_any_buy_after_sec >= disable_new_primary_after_sec, "biasedHedge.disableAnyBuyAfterSec must be >= disableNewPrimaryAfterSec");
    anyhow::ensure!(disable_any_buy_after_sec <= 240, "biasedHedge.disableAnyBuyAfterSec must be <= 240");
    anyhow::ensure!(stop_config.bias_invalidation_enabled, "biasedHedgeStop.biasInvalidationEnabled must be true");
    anyhow::ensure!(stop_config.exit_pct_on_invalidation > 0.0, "biasedHedgeStop.exitPctOnInvalidation must be > 0");
    anyhow::ensure!(!stop_config.time_exit_rules.is_empty(), "biasedHedgeStop.timeExitRules cannot be empty");
    for (index, rule) in stop_config.time_exit_rules.iter().enumerate() {
        anyhow::ensure!(rule.elapsed_sec > 0, "biasedHedgeStop.timeExitRules[{index}].elapsedSec must be > 0");
        anyhow::ensure!(
            rule.remaining_pct.is_finite() && rule.remaining_pct >= 0.0 && rule.remaining_pct <= 100.0,
            "biasedHedgeStop.timeExitRules[{index}].remainingPct must be in [0, 100]"
        );
    }

    Ok(Some(ActionPlaceOrderBiasedHedgeConfig {
        primary_budget_usdc,
        hedge_budget_usdc,
        min_dominant_share,
        max_hedge_spend_ratio,
        primary_min_edge,
        primary_min_final_q,
        hedge_max_price,
        hedge_min_price,
        hedge_only_if_primary_filled,
        disable_new_primary_after_sec,
        disable_any_buy_after_sec,
        max_side_switch_count,
        high_price,
        high_price_min_final_q,
        high_price_min_edge,
        max_paired_effective_cost,
        stop: stop_config,
    }))
}

fn biased_hedge_market_timing(market_slug: &str, now: DateTime<Utc>) -> BiasedHedgeMarketTiming {
    let Some(scope) = find_updown_scope_by_slug(market_slug) else {
        return BiasedHedgeMarketTiming {
            elapsed_sec: None,
            remaining_sec: None,
            cycle_window_active: false,
        };
    };
    let Some(start) = MarketCycleId(market_slug.to_string()).start_time() else {
        return BiasedHedgeMarketTiming {
            elapsed_sec: None,
            remaining_sec: None,
            cycle_window_active: false,
        };
    };
    let window_seconds = updown_scope_window_seconds(scope).max(1);
    let elapsed_sec = now.signed_duration_since(start).num_seconds().max(0);
    let remaining_sec = (window_seconds - elapsed_sec).max(0);
    BiasedHedgeMarketTiming {
        elapsed_sec: Some(elapsed_sec),
        remaining_sec: Some(remaining_sec),
        cycle_window_active: elapsed_sec <= window_seconds,
    }
}

fn biased_hedge_timing_payload(timing: BiasedHedgeMarketTiming) -> Value {
    json!({
        "elapsed_sec": timing.elapsed_sec,
        "remaining_sec": timing.remaining_sec,
        "cycle_window_active": timing.cycle_window_active,
    })
}

fn biased_hedge_has_explicit_iv_time_rules(node: &TradeFlowNode) -> bool {
    node.config
        .get("priceToBeatIvTimeRules")
        .and_then(Value::as_array)
        .is_some_and(|rules| !rules.is_empty())
}

fn biased_hedge_default_iv_time_rule(
    node: &TradeFlowNode,
    config: &ActionPlaceOrderBiasedHedgeConfig,
    market_slug: &str,
) -> Option<Value> {
    let scope = find_updown_scope_by_slug(market_slug)?;
    let window_seconds = updown_scope_window_seconds(scope).max(1);
    let entry_start_sec = BIASED_HEDGE_DEFAULT_ENTRY_START_SEC
        .max(0)
        .min(window_seconds.saturating_sub(1));
    let entry_end_sec = config
        .disable_new_primary_after_sec
        .max(entry_start_sec + 1)
        .min(window_seconds);
    let start_remaining_sec = window_seconds.saturating_sub(entry_start_sec);
    let end_remaining_sec = window_seconds.saturating_sub(entry_end_sec);
    (start_remaining_sec > end_remaining_sec).then(|| {
        let biased = biased_hedge_object(node, "biasedHedge");
        let max_price_cent = biased_hedge_f64(biased, "maxPriceCent")
            .or_else(|| node_config_f64(node, "maxPriceCent"))
            .unwrap_or(75.0);
        json!({
            "startRemainingSec": start_remaining_sec,
            "endRemainingSec": end_remaining_sec,
            "maxPriceCent": max_price_cent,
            "minEdge": config.primary_min_edge,
            "minGapStrength": 0,
        })
    })
}

fn apply_biased_hedge_early_iv_time_rule(
    node: &mut TradeFlowNode,
    config: &ActionPlaceOrderBiasedHedgeConfig,
    market_slug: &str,
) {
    if biased_hedge_has_explicit_iv_time_rules(node) {
        return;
    }
    let Some(rule) = biased_hedge_default_iv_time_rule(node, config, market_slug) else {
        return;
    };
    if let Some(map) = node.config.as_object_mut() {
        map.insert("priceToBeatIvTimeRules".to_string(), json!([rule]));
    }
}

fn biased_hedge_iv_payload(candidate: &PairLockEdgeCandidate) -> Option<&Value> {
    candidate
        .diagnostics
        .get("guard")
        .and_then(|guard| guard.get("price_to_beat_guard"))
        .and_then(|guard| guard.get("iv_mismatch_edge"))
}

fn biased_hedge_selected_time_rule(candidate: &PairLockEdgeCandidate) -> Option<Value> {
    biased_hedge_iv_payload(candidate)
        .and_then(|payload| payload.get("selected_time_rule"))
        .cloned()
}

fn biased_hedge_depth_ok(candidate: &PairLockEdgeCandidate) -> bool {
    biased_hedge_iv_payload(candidate)
        .and_then(|payload| payload.get("depth_guard_result"))
        .and_then(Value::as_str)
        .is_some_and(|value| value == "pass")
}

fn biased_hedge_binance_same_direction(candidate: &PairLockEdgeCandidate) -> bool {
    biased_hedge_iv_payload(candidate)
        .and_then(|payload| payload.get("binance_same_direction"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn biased_hedge_candidate_summary(candidate: &PairLockEdgeCandidate) -> Value {
    let iv_payload = biased_hedge_iv_payload(candidate);
    json!({
        "token_id": candidate.token_id,
        "outcome_label": candidate.outcome_label,
        "ask": candidate.ask,
        "fee": candidate.fee,
        "cost": candidate.cost,
        "q_final": candidate.q,
        "edge_adjusted": candidate.edge,
        "gap_strength": iv_payload.and_then(|payload| payload.get("gap_strength")).and_then(value_as_f64),
        "required_gap_strength": iv_payload.and_then(|payload| payload.get("required_gap_strength")).and_then(value_as_f64),
        "binance_same_direction": iv_payload.and_then(|payload| payload.get("binance_same_direction")).and_then(Value::as_bool),
        "depth_guard_result": iv_payload.and_then(|payload| payload.get("depth_guard_result")).and_then(Value::as_str),
        "selected_iv_time_rule": biased_hedge_selected_time_rule(candidate),
        "guard_decision": candidate.guard_decision,
        "guard_reason": candidate.guard_reason,
    })
}

fn biased_hedge_primary_block_reason(
    candidate: &PairLockEdgeCandidate,
    config: &ActionPlaceOrderBiasedHedgeConfig,
) -> Option<&'static str> {
    if candidate.guard_decision != "passed" {
        return match candidate.guard_reason.as_str() {
            "below_best_ask_floor" | "best_ask_unavailable" | "pair_primary_best_ask_unavailable" => {
                Some("primary_floor_fail")
            }
            "blocked_insufficient_depth" | "blocked_depth_guard_unavailable" => {
                Some("primary_depth_fail")
            }
            "above_max_price" => Some("high_price_confidence_fail"),
            _ => Some("no_primary_edge"),
        };
    }
    let Some(q_final) = candidate.q else {
        return Some("no_primary_edge");
    };
    let Some(edge) = candidate.edge else {
        return Some("no_primary_edge");
    };
    if q_final < config.primary_min_final_q || edge < config.primary_min_edge {
        return Some("no_primary_edge");
    }
    let Some(ask) = candidate.ask else {
        return Some("primary_depth_fail");
    };
    if ask >= config.high_price {
        if q_final < config.high_price_min_final_q
            || edge < config.high_price_min_edge
            || !biased_hedge_binance_same_direction(candidate)
            || !biased_hedge_depth_ok(candidate)
        {
            return Some("high_price_confidence_fail");
        }
    }
    None
}

fn biased_hedge_select_primary<'a>(
    up: &'a PairLockEdgeCandidate,
    down: &'a PairLockEdgeCandidate,
    config: &ActionPlaceOrderBiasedHedgeConfig,
) -> Option<&'a PairLockEdgeCandidate> {
    [up, down]
        .into_iter()
        .filter(|candidate| biased_hedge_primary_block_reason(candidate, config).is_none())
        .max_by(|left, right| {
            left.edge
                .unwrap_or(f64::NEG_INFINITY)
                .partial_cmp(&right.edge.unwrap_or(f64::NEG_INFINITY))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn configure_biased_hedge_primary_node(
    node: &TradeFlowNode,
    market_slug: &str,
    primary: &PairLockEdgeCandidate,
    trigger_node_key: &str,
    config: &ActionPlaceOrderBiasedHedgeConfig,
) -> TradeFlowNode {
    let mut child =
        build_pair_lock_single_leg_node(node, market_slug, &primary.token_id, &primary.outcome_label, trigger_node_key);
    let mut map = child.config.as_object().cloned().unwrap_or_default();
    map.insert("sizeMode".to_string(), json!("usdc"));
    map.insert("sizeUsdc".to_string(), json!(config.primary_budget_usdc));
    map.insert("kind".to_string(), json!("immediate"));
    map.insert("executionMode".to_string(), json!("market"));
    map.insert("tpEnabled".to_string(), json!(false));
    map.insert("priceToBeatGuardEnabled".to_string(), json!(false));
    map.insert("triggerPriceGuardEnabled".to_string(), json!(false));
    map.insert("executionFloorGuardEnabled".to_string(), json!(false));
    map.insert("retryOnPriceToBeatGuardBlock".to_string(), json!(false));
    map.insert("retryOnTriggerPriceGuardBlock".to_string(), json!(false));
    map.insert("retryOnExecutionFloorGuardBlock".to_string(), json!(false));
    map.insert("retryOnMaxPriceBlock".to_string(), json!(false));
    if let Some(ask) = primary.ask {
        map.insert("maxPriceCent".to_string(), json!((ask * 100.0).min(99.0)));
    }
    if config.stop.ptb_stop_loss_enabled {
        map.insert("ptbStopLossEnabled".to_string(), json!(true));
        if let Some(gap) = config.stop.ptb_stop_loss_gap_usd {
            map.insert("ptbStopLossGapUsd".to_string(), json!(gap));
        }
        if let Some(mode) = config.stop.ptb_stop_loss_time_decay_mode.as_deref() {
            map.insert("ptbStopLossTimeDecayMode".to_string(), json!(mode));
        }
    }
    child.config = Value::Object(map);
    child
}

fn configure_biased_hedge_counter_node(
    node: &TradeFlowNode,
    market_slug: &str,
    counter: &ActionPlaceOrderPairResolvedCounterLeg,
    pair_lock: &ActionPlaceOrderPairLockConfig,
    trigger_node_key: &str,
    config: &ActionPlaceOrderBiasedHedgeConfig,
) -> TradeFlowNode {
    let mut child = build_pair_lock_counter_leg_node(node, market_slug, counter, pair_lock, trigger_node_key);
    let mut map = child.config.as_object().cloned().unwrap_or_default();
    map.insert("sizeMode".to_string(), json!("usdc"));
    map.insert("sizeUsdc".to_string(), json!(config.hedge_budget_usdc));
    map.insert("maxPriceCent".to_string(), json!(config.hedge_max_price * 100.0));
    map.insert("priceToBeatGuardEnabled".to_string(), json!(false));
    map.insert("executionFloorGuardEnabled".to_string(), json!(false));
    map.insert("retryOnMaxPriceBlock".to_string(), json!(true));
    map.insert("retryOnPriceToBeatGuardBlock".to_string(), json!(true));
    map.insert("retryOnExecutionFloorGuardBlock".to_string(), json!(true));
    map.insert("tpEnabled".to_string(), json!(false));
    map.insert("slEnabled".to_string(), json!(false));
    child.config = Value::Object(map);
    child
}

fn biased_hedge_waiting_execution(
    node_key: &str,
    market_slug: &str,
    reason: &str,
    diagnostics: Value,
) -> TradeFlowNodeExecution {
    TradeFlowNodeExecution {
        output: json!({
            "node_key": node_key,
            "blocked": true,
            "retrying": true,
            "reason": reason,
            "market_slug": market_slug,
            "retry_delay_ms": PAIR_LOCK_PRIMARY_GUARD_RETRY_DELAY_MS,
            "pair_lock_strategy": PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1,
            "biased_hedge": diagnostics,
        }),
        routes: Vec::new(),
        repeat_at: Some(Utc::now() + ChronoDuration::milliseconds(PAIR_LOCK_PRIMARY_GUARD_RETRY_DELAY_MS)),
        repeat_idempotency_key: None,
    }
}

async fn enqueue_biased_hedge_monitor(
    repo: &PostgresRepository,
    run_id: i64,
    node_key: &str,
    session_id: i64,
    primary_order_id: i64,
    available_at: DateTime<Utc>,
) -> Result<()> {
    let tick = available_at.timestamp() / BIASED_HEDGE_MONITOR_INTERVAL_SEC;
    let idempotency_key = format!("biased_hedge_monitor:{run_id}:{session_id}:{tick}");
    let input = json!({
        "internalMode": "biased_hedge_monitor",
        "pairSessionId": session_id,
        "primaryBuilderOrderId": primary_order_id,
    });
    let enqueued = repo
        .enqueue_trade_flow_step(
            run_id,
            node_key,
            "action.place_order",
            1,
            Some(&input),
            available_at,
            None,
            Some(&idempotency_key),
        )
        .await?;
    schedule_enqueued_flow_process_notify(enqueued, available_at);
    Ok(())
}

async fn enqueue_biased_hedge_time_exits(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node_key: &str,
    session: &TradeBuilderPairSession,
    config: &ActionPlaceOrderBiasedHedgeConfig,
) -> Result<()> {
    let Some(primary_order_id) = session.primary_order_id else {
        return Ok(());
    };
    let Some(window_start) = MarketCycleId(session.market_slug.clone()).start_time() else {
        return Ok(());
    };
    for (index, rule) in config.stop.time_exit_rules.iter().enumerate() {
        let available_at = (window_start + ChronoDuration::seconds(rule.elapsed_sec)).max(Utc::now());
        let idempotency_key = format!("biased_hedge_time_exit:{}:{primary_order_id}:{index}", run.id);
        let input = json!({
            "internalMode": "biased_hedge_time_exit",
            "pairSessionId": session.id,
            "parentBuilderOrderId": primary_order_id,
            "targetRemainingPct": rule.remaining_pct,
            "ruleIndex": index,
            "elapsedSec": rule.elapsed_sec,
        });
        let enqueued = repo
            .enqueue_trade_flow_step(
                run.id,
                node_key,
                "action.place_order",
                1,
                Some(&input),
                available_at,
                None,
                Some(&idempotency_key),
            )
            .await?;
        schedule_enqueued_flow_process_notify(enqueued, available_at);
        repo.append_trade_builder_order_event(
            primary_order_id,
            if enqueued.is_some() {
                "biased_hedge_time_exit_scheduled"
            } else {
                "biased_hedge_time_exit_duplicate_ignored"
            },
            &json!({
                "pair_session_id": session.id,
                "rule_index": index,
                "elapsed_sec": rule.elapsed_sec,
                "target_remaining_pct": rule.remaining_pct,
                "available_at": available_at.to_rfc3339(),
                "idempotency_key": idempotency_key,
                "step_enqueued": enqueued.is_some(),
            }),
        )
        .await?;
    }
    Ok(())
}

fn biased_hedge_notional(fill_qty: Option<f64>, avg_fill_price: Option<f64>) -> f64 {
    fill_qty.unwrap_or_default().max(0.0) * avg_fill_price.unwrap_or_default().max(0.0)
}

fn biased_hedge_dominant_share(session: &TradeBuilderPairSession) -> Option<f64> {
    let primary = biased_hedge_notional(session.primary_fill_qty, session.primary_avg_fill_price);
    let hedge = biased_hedge_notional(session.counter_fill_qty, session.counter_avg_fill_price);
    let total = primary + hedge;
    (total > 0.0).then_some(primary / total)
}

fn biased_hedge_clamped_hedge_notional(
    primary_notional: f64,
    config: &ActionPlaceOrderBiasedHedgeConfig,
) -> f64 {
    if primary_notional <= 0.0 {
        return 0.0;
    }
    let max_allowed_by_dominance =
        primary_notional * (1.0 - config.min_dominant_share) / config.min_dominant_share;
    config
        .hedge_budget_usdc
        .min(config.primary_budget_usdc * config.max_hedge_spend_ratio)
        .min(max_allowed_by_dominance)
        .max(0.0)
}

fn biased_hedge_bias_invalidated(
    q_final: Option<f64>,
    edge: Option<f64>,
    depth_ok: bool,
    binance_same_direction: bool,
    config: &ActionPlaceOrderBiasedHedgeConfig,
) -> bool {
    q_final.is_none_or(|value| value < config.stop.min_q_final_to_hold)
        || edge.is_none_or(|value| value < config.stop.min_edge_to_hold)
        || !depth_ok
        || !binance_same_direction
}

async fn maybe_schedule_biased_hedge_after_primary_fill(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node_key: &str,
    session: &TradeBuilderPairSession,
    config: &ActionPlaceOrderBiasedHedgeConfig,
) -> Result<()> {
    let Some(primary_order_id) = session.primary_order_id else {
        return Ok(());
    };
    enqueue_biased_hedge_monitor(
        repo,
        run.id,
        node_key,
        session.id,
        primary_order_id,
        Utc::now() + ChronoDuration::seconds(BIASED_HEDGE_MONITOR_INTERVAL_SEC),
    )
    .await?;
    enqueue_biased_hedge_time_exits(repo, run, node_key, session, config).await
}

async fn maybe_handle_biased_hedge_pair_fill(
    repo: &PostgresRepository,
    session: &TradeBuilderPairSession,
    order: &TradeBuilderOrder,
) -> Result<bool> {
    let Some(node) = resolve_trade_builder_pair_lock_node(repo, session).await? else {
        return Ok(false);
    };
    if !action_place_order_uses_biased_hedge_strategy(&node) {
        return Ok(false);
    }
    let Some(config) = resolve_action_place_order_biased_hedge_config(&node)? else {
        return Ok(false);
    };
    let Some(run_id) = session.flow_run_id else {
        return Ok(true);
    };
    let Some(run) = repo.get_trade_flow_run(run_id).await? else {
        return Ok(true);
    };
    let fresh_session = repo
        .get_trade_builder_pair_session(session.id)
        .await?
        .unwrap_or_else(|| session.clone());
    let primary_notional =
        biased_hedge_notional(fresh_session.primary_fill_qty, fresh_session.primary_avg_fill_price);
    let hedge_notional =
        biased_hedge_notional(fresh_session.counter_fill_qty, fresh_session.counter_avg_fill_price);
    let dominant_side_share = biased_hedge_dominant_share(&fresh_session);
    let phase = if hedge_notional > 0.0 {
        "BIASED_HEDGED"
    } else if primary_notional > 0.0 {
        "PRIMARY_FILLED"
    } else {
        "PRIMARY_SUBMITTED"
    };
    repo.append_trade_builder_order_event(
        order.id,
        "biased_hedge_fill_observed",
        &json!({
            "strategy": PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1,
            "phase": phase,
            "pair_session_id": fresh_session.id,
            "primary_filled_notional_usdc": primary_notional,
            "hedge_filled_notional_usdc": hedge_notional,
            "total_filled_notional_usdc": primary_notional + hedge_notional,
            "dominant_side_share": dominant_side_share,
            "primary_trade_id": fresh_session.primary_order_id,
            "hedge_trade_id": fresh_session.counter_order_id,
            "primary_fill_price": fresh_session.primary_avg_fill_price,
            "hedge_fill_price": fresh_session.counter_avg_fill_price,
            "primary_filled_qty": fresh_session.primary_fill_qty,
            "hedge_filled_qty": fresh_session.counter_fill_qty,
        }),
    )
    .await?;
    if order.pair_leg_role.as_deref() == Some(TRADE_BUILDER_PAIR_ROLE_LEAD_CANDIDATE) {
        if let Some(node_key) = fresh_session.flow_node_key.as_deref() {
            maybe_schedule_biased_hedge_after_primary_fill(repo, &run, node_key, &fresh_session, &config)
                .await?;
        }
    }
    if primary_notional > 0.0 && hedge_notional > 0.0 {
        repo.update_trade_builder_pair_session_state(
            fresh_session.id,
            TRADE_BUILDER_PAIR_STATUS_LOCKED,
            fresh_session.locked_qty,
            fresh_session.projected_net_profit_usdc,
            None,
        )
        .await?;
    }
    Ok(true)
}

async fn maybe_prepare_biased_hedge_counter_runtime(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    session: &TradeBuilderPairSession,
    config: &ActionPlaceOrderBiasedHedgeConfig,
    now: DateTime<Utc>,
) -> Result<bool> {
    if session.counter_order_id != Some(order.id) {
        return Ok(false);
    }
    if session.status != TRADE_BUILDER_PAIR_STATUS_WORKING {
        return Ok(false);
    }
    let timing = biased_hedge_market_timing(&order.market_slug, now);
    if timing
        .elapsed_sec
        .is_some_and(|elapsed| elapsed > config.disable_any_buy_after_sec)
    {
        repo.set_trade_builder_order_status(order.id, "canceled", Some("biased_hedge_late_buy_block"))
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "biased_hedge_counter_blocked",
            &json!({
                "strategy": PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1,
                "phase": "HEDGE_ELIGIBLE",
                "decision": "block",
                "block_reason": "late_chase_block",
                "pair_session_id": session.id,
                "timing": biased_hedge_timing_payload(timing),
            }),
        )
        .await?;
        return Ok(true);
    }
    if session.lead_order_id.is_none() {
        repo.append_trade_builder_order_event(
            order.id,
            "biased_hedge_counter_waiting_primary_fill",
            &json!({
                "strategy": PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1,
                "phase": "PRIMARY_SUBMITTED",
                "decision": "hold",
                "block_reason": "hedge_before_primary_fill",
                "pair_session_id": session.id,
                "timing": biased_hedge_timing_payload(timing),
            }),
        )
        .await?;
        return Ok(true);
    }
    if pair_lock_auto_counter_has_started_submit_lifecycle(order) {
        return Ok(false);
    }
    let primary_notional = biased_hedge_notional(session.primary_fill_qty, session.primary_avg_fill_price);
    if primary_notional <= 0.0 {
        return Ok(true);
    }
    let max_allowed_by_dominance =
        primary_notional * (1.0 - config.min_dominant_share) / config.min_dominant_share;
    let hedge_notional = biased_hedge_clamped_hedge_notional(primary_notional, config);
    let projected_share = primary_notional / (primary_notional + hedge_notional.max(0.0));
    if hedge_notional <= 0.01 || projected_share + 0.000000001 < config.min_dominant_share {
        repo.set_trade_builder_order_status(order.id, "canceled", Some("biased_hedge_dominance_break"))
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "biased_hedge_counter_blocked",
            &json!({
                "strategy": PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1,
                "phase": "HEDGE_ELIGIBLE",
                "decision": "block",
                "block_reason": "dominance_break",
                "pair_session_id": session.id,
                "primary_filled_notional_usdc": primary_notional,
                "hedge_filled_notional_usdc": 0.0,
                "total_filled_notional_usdc": primary_notional,
                "dominant_side_share": 1.0,
                "max_allowed_hedge_usdc": max_allowed_by_dominance,
                "clamped_hedge_usdc": hedge_notional,
                "hedge_only_if_primary_filled": config.hedge_only_if_primary_filled,
                "max_side_switch_count": config.max_side_switch_count,
            }),
        )
        .await?;
        return Ok(true);
    }

    let target_qty = order
        .max_price
        .filter(|price| *price > 0.0)
        .map(|price| round_trade_builder_share_qty(hedge_notional / price));
    let sizing_changed = (order.size_usdc - hedge_notional).abs() >= 0.000001
        || order.remaining_size != Some(hedge_notional);
    if sizing_changed {
        repo.update_trade_builder_order_sizing_and_state(
            order.id,
            TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC,
            hedge_notional,
            target_qty,
            Some(hedge_notional),
            target_qty,
            "pending",
            None,
            order.eligible_after_at,
            order.eligible_before_at,
            None,
            None,
            None,
        )
        .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "biased_hedge_counter_clamped",
            &json!({
                "strategy": PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1,
                "phase": "HEDGE_ELIGIBLE",
                "decision": "hedge_buy",
                "pair_session_id": session.id,
                "primary_filled_notional_usdc": primary_notional,
                "clamped_hedge_usdc": hedge_notional,
                "max_allowed_hedge_usdc": max_allowed_by_dominance,
                "dominant_side_share": projected_share,
                "hedge_min_price": config.hedge_min_price,
                "hedge_max_price": config.hedge_max_price,
                "hedge_only_if_primary_filled": config.hedge_only_if_primary_filled,
                "max_side_switch_count": config.max_side_switch_count,
                "timing": biased_hedge_timing_payload(timing),
            }),
        )
        .await?;
        return Ok(true);
    }
    Ok(false)
}

#[allow(clippy::too_many_arguments)]
async fn execute_action_place_order_pair_lock_biased_hedge_strategy(
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
    let internal_mode = step_input_string(step, &["internalMode", "internal_mode"])
        .map(|value| value.trim().to_ascii_lowercase());
    match internal_mode.as_deref() {
        Some("biased_hedge_monitor") => {
            return execute_biased_hedge_monitor(repo, cfg, client, ws, run, step, node, context).await;
        }
        Some("biased_hedge_time_exit") => {
            return execute_biased_hedge_time_exit(
                repo, run_id, cfg, limits, policy, client, run, step, node, graph, context,
            )
            .await;
        }
        _ => {}
    }

    let config = resolve_action_place_order_biased_hedge_config(node)?
        .ok_or_else(|| anyhow::anyhow!("biased_hedge_v1 config missing"))?;
    let market_slug = resolve_action_place_order_string(
        node,
        context,
        step,
        "marketSlug",
        "marketSlug",
        &["market_slug", "marketSlug", "wsMarketSlug"],
    )
    .ok_or_else(|| anyhow::anyhow!("pair_lock requires marketSlug"))?;
    let timing = biased_hedge_market_timing(&market_slug, Utc::now());
    if timing
        .elapsed_sec
        .is_some_and(|elapsed| elapsed > config.disable_new_primary_after_sec)
    {
        let diagnostics = json!({
            "strategy": PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1,
            "phase": "NO_POSITION",
            "decision": "block",
            "block_reason": "outside_entry_window",
            "timing": biased_hedge_timing_payload(timing),
        });
        repo.append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "biased_hedge_decision",
            &diagnostics,
        )
        .await?;
        return Ok(biased_hedge_waiting_execution(
            &node.key,
            &market_slug,
            "biased_hedge_late_primary_block",
            diagnostics,
        ));
    }

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
    let mut eval_node = node.clone();
    if let Some(map) = eval_node.config.as_object_mut() {
        map.insert("sizeUsdc".to_string(), json!(config.primary_budget_usdc));
    }
    apply_biased_hedge_early_iv_time_rule(&mut eval_node, &config, &market_slug);
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
        &eval_node,
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
        &eval_node,
        context,
        &market_slug,
        &resolved_tokens.no_token_id,
        down_label,
    )
    .await?;
    let up = pair_lock_edge_candidate_from_eval(up_eval, 0.0);
    let down = pair_lock_edge_candidate_from_eval(down_eval, 0.0);
    let selected = biased_hedge_select_primary(&up, &down, &config);
    let Some(primary) = selected else {
        let diagnostics = json!({
            "strategy": PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1,
            "phase": "PRIMARY_CANDIDATE",
            "decision": "block",
            "block_reason": "no_primary_edge",
            "timing": biased_hedge_timing_payload(timing),
            "up": biased_hedge_candidate_summary(&up),
            "down": biased_hedge_candidate_summary(&down),
            "token_resolution": token_resolution_payload,
        });
        repo.append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "biased_hedge_decision",
            &diagnostics,
        )
        .await?;
        return Ok(biased_hedge_waiting_execution(
            &node.key,
            &market_slug,
            "biased_hedge_no_primary_edge",
            diagnostics,
        ));
    };
    let counter = if primary.token_id == resolved_tokens.yes_token_id {
        ActionPlaceOrderPairResolvedCounterLeg {
            token_id: resolved_tokens.no_token_id.clone(),
            outcome_label: down_label.to_string(),
        }
    } else {
        ActionPlaceOrderPairResolvedCounterLeg {
            token_id: resolved_tokens.yes_token_id.clone(),
            outcome_label: up_label.to_string(),
        }
    };
    let primary_node =
        configure_biased_hedge_primary_node(node, &market_slug, primary, trigger_node_key, &config);
    let counter_node = configure_biased_hedge_counter_node(
        node,
        &market_slug,
        &counter,
        pair_lock,
        trigger_node_key,
        &config,
    );
    let primary_step = clone_pair_lock_step_with_quote(step, &primary.quote);
    let counter_candidate = if counter.token_id == up.token_id { &up } else { &down };
    let counter_step = clone_pair_lock_step_with_quote(step, &counter_candidate.quote);

    let mut primary_context = context.clone();
    let primary_execution = execute_action_place_order(
        repo,
        run_id,
        cfg,
        limits,
        policy,
        Some(client),
        run,
        &primary_step,
        &primary_node,
        graph,
        &mut primary_context,
    )
    .await?;
    let primary_order_id = extract_builder_order_id(&primary_execution)?;
    let primary_source_trade_id = extract_source_trade_id(&primary_execution);
    let mut counter_context = context.clone();
    let counter_execution = match execute_action_place_order(
        repo,
        run_id,
        cfg,
        limits,
        policy,
        Some(client),
        run,
        &counter_step,
        &counter_node,
        graph,
        &mut counter_context,
    )
    .await
    {
        Ok(execution) => execution,
        Err(err) => {
            cancel_pair_lock_order_if_created(repo, Some(primary_order_id), "biased_hedge_counter_create_failed").await;
            return Err(err);
        }
    };
    let counter_order_id = extract_builder_order_id(&counter_execution)?;
    let counter_source_trade_id = extract_source_trade_id(&counter_execution);
    let pair_session_id = repo
        .create_trade_builder_pair_session(
            run.user_id,
            Some(run.definition_id),
            Some(run.id),
            Some(&node.key),
            &market_slug,
            config
                .max_paired_effective_cost
                .unwrap_or(pair_lock.max_total_price)
                * 100.0,
            0.0,
            0.0,
            pair_lock.orphan_grace_ms,
            pair_lock.ignore_stop_loss_after_locked,
            pair_lock.notify_on_pair_locked,
            pair_lock.notify_on_pair_unwind,
            false,
        )
        .await?;
    repo.attach_trade_builder_pair_session_orders(pair_session_id, primary_order_id, counter_order_id)
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
    let diagnostics = json!({
        "strategy": PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1,
        "phase": "PRIMARY_SUBMITTED",
        "decision": "primary_buy",
        "pair_session_id": pair_session_id,
        "market_slug": market_slug,
        "selected_primary_outcome": primary.outcome_label,
        "primary_trade_id": primary_order_id,
        "hedge_trade_id": counter_order_id,
        "primary_source_trade_id": primary_source_trade_id,
        "hedge_source_trade_id": counter_source_trade_id,
        "primary_spend_usdc": config.primary_budget_usdc,
        "hedge_spend_usdc": config.hedge_budget_usdc,
        "hedge_min_price": config.hedge_min_price,
        "hedge_max_price": config.hedge_max_price,
        "q_final": primary.q,
        "edge_adjusted": primary.edge,
        "cost": primary.cost,
        "gap_strength": biased_hedge_iv_payload(primary).and_then(|payload| payload.get("gap_strength")).and_then(value_as_f64),
        "side_switch_count": 0,
        "max_side_switch_count": config.max_side_switch_count,
        "hedge_only_if_primary_filled": config.hedge_only_if_primary_filled,
        "dominant_side_share": Value::Null,
        "block_reason": Value::Null,
        "timing": biased_hedge_timing_payload(timing),
        "selected_iv_time_rule": biased_hedge_selected_time_rule(primary),
        "up": biased_hedge_candidate_summary(&up),
        "down": biased_hedge_candidate_summary(&down),
        "token_resolution": token_resolution_payload,
    });
    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "biased_hedge_decision",
        &diagnostics,
    )
    .await?;
    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "pair_lock_session_created",
        &diagnostics,
    )
    .await?;

    let ref_key = action_place_order_pair_lock_ref_key(node);
    bind_action_place_order_ref_bindings(context, node, &ref_key, primary_order_id);
    if let Some(source_trade_id) = primary_source_trade_id {
        set_flow_context(context, "sourceTradeId", json!(source_trade_id));
    }
    set_flow_var(context, &format!("{ref_key}_pair_session_id"), json!(pair_session_id));
    set_flow_var(
        context,
        &format!("{ref_key}_counter_builder_order_id"),
        json!(counter_order_id),
    );
    if let Some(source_trade_id) = counter_source_trade_id {
        set_flow_var(
            context,
            &format!("{ref_key}_counter_source_trade_id"),
            json!(source_trade_id),
        );
    }

    Ok(TradeFlowNodeExecution {
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
            "source_trade_id": primary_source_trade_id,
            "counter_source_trade_id": counter_source_trade_id,
            "trigger_node_key": trigger_node_key,
            "pair_lock_strategy": PAIR_LOCK_STRATEGY_BIASED_HEDGE_V1,
            "biased_hedge_decision": "primary_buy",
            "biased_hedge": diagnostics,
        }),
        routes: Vec::new(),
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}
