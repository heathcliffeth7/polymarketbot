const PAIR_LOCK_STRATEGY_ADAPTIVE_MAX_PRICE_V1: &str = "adaptive_max_price_v1";
const DEFAULT_ADAPTIVE_MAX_PRICE_MISS_COUNT: usize = 3;
const DEFAULT_ADAPTIVE_MAX_PRICE_REQUIRED_GOOD_MISS_COUNT: usize = 2;
const DEFAULT_ADAPTIVE_MAX_PRICE_RELAX_CREDIT_CENT: f64 = 2.0;
const DEFAULT_ADAPTIVE_MAX_PRICE_MAX_RELAX_CREDIT_CENT: f64 = 5.0;
const DEFAULT_ADAPTIVE_MAX_PRICE_HARD_CAP_CENT: f64 = 76.0;
const DEFAULT_ADAPTIVE_MAX_PRICE_EXTRA_BUFFER_CENT: f64 = 1.0;
const DEFAULT_ADAPTIVE_MAX_PRICE_PAIR_BUFFER_CENT: f64 = 1.0;
const DEFAULT_ADAPTIVE_MAX_PRICE_SIZE_MULTIPLIER: f64 = 0.5;
const DEFAULT_ADAPTIVE_MAX_PRICE_LATE_RELAX_CUTOFF_S: i64 = 210;
const DEFAULT_ADAPTIVE_MAX_PRICE_SL_COOLDOWN_MARKETS: usize = 3;
const FLOW_NODE_STATE_PAIR_LOCK_ADAPTIVE_MAX_PRICE_RELAXED_MARKET: &str =
    "pair_lock_adaptive_max_price_relaxed_market";
const FLOW_NODE_STATE_PAIR_LOCK_ADAPTIVE_MAX_PRICE_RELAXED_OUTCOME: &str =
    "pair_lock_adaptive_max_price_relaxed_outcome";

#[derive(Debug, Clone, Copy)]
struct PairLockAdaptiveMaxPriceConfig {
    miss_count: usize,
    required_good_miss_count: usize,
    relax_credit_cent: f64,
    max_relax_credit_cent: f64,
    hard_cap_cent: f64,
    extra_buffer_cent: f64,
    pair_buffer_cent: f64,
    size_multiplier: f64,
    late_relax_cutoff_s: i64,
    sl_cooldown_markets: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct PairLockAdaptiveMaxPriceHistory {
    resolved_good_miss_count: usize,
    resolved_good_block_count: usize,
    pending_miss_count: usize,
    unknown_miss_count: usize,
    bad_data_miss_count: usize,
    resolved_miss_count: usize,
    recent_sl: bool,
    cooldown_active: bool,
}

#[derive(Debug, Clone)]
struct PairLockAdaptiveMaxPriceDecisionInput<'a> {
    config: PairLockAdaptiveMaxPriceConfig,
    base_max_price: Option<f64>,
    ask: Option<f64>,
    estimated_avg_fill: Option<f64>,
    counter_estimated_avg_fill: Option<f64>,
    q_final: Option<f64>,
    dynamic_threshold: Option<f64>,
    pair_max_total_price: f64,
    base_size_usdc: f64,
    ptb_passed: bool,
    base_max_price_blocked: bool,
    depth_guard_pass: bool,
    counter_depth_ok: bool,
    book_reliability_ok: bool,
    volume_regime: &'a str,
    ptb_trend: &'a str,
    market_elapsed_s: Option<i64>,
    already_relaxed_current_market: bool,
    history: PairLockAdaptiveMaxPriceHistory,
}

#[derive(Debug, Clone)]
struct PairLockAdaptiveMaxPriceDecision {
    relax_applied: bool,
    decision: &'static str,
    reason: &'static str,
    effective_max_price: Option<f64>,
    effective_size_usdc: Option<f64>,
    diagnostics: Value,
}

#[derive(Debug, Clone)]
struct PairLockAdaptiveMaxPriceOverride {
    effective_max_price: f64,
    effective_size_usdc: f64,
    diagnostics: Value,
}

fn action_place_order_uses_adaptive_max_price_strategy(node: &TradeFlowNode) -> bool {
    matches!(
        resolve_action_place_order_pair_lock_strategy(node),
        Ok(PAIR_LOCK_STRATEGY_ADAPTIVE_MAX_PRICE_V1)
    )
}

fn resolve_pair_lock_adaptive_max_price_config(
    node: &TradeFlowNode,
) -> Result<PairLockAdaptiveMaxPriceConfig> {
    anyhow::ensure!(
        node_config_bool(node, "priceToBeatGuardEnabled").unwrap_or(false),
        "action.place_order adaptive_max_price_v1 requires priceToBeatGuardEnabled=true"
    );
    let ptb_mode = node_config_string(node, "priceToBeatMode")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    anyhow::ensure!(
        ptb_mode == "iv_mismatch_edge",
        "action.place_order adaptive_max_price_v1 requires priceToBeatMode=iv_mismatch_edge"
    );

    let config = PairLockAdaptiveMaxPriceConfig {
        miss_count: node_config_i64(node, "adaptiveMaxPriceMissCount")
            .filter(|value| *value > 0)
            .map(|value| value as usize)
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_MISS_COUNT),
        required_good_miss_count: node_config_i64(
            node,
            "adaptiveMaxPriceRequiredGoodMissCount",
        )
        .filter(|value| *value > 0)
        .map(|value| value as usize)
        .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_REQUIRED_GOOD_MISS_COUNT),
        relax_credit_cent: node_config_f64(node, "adaptiveMaxPriceRelaxCreditCent")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_RELAX_CREDIT_CENT),
        max_relax_credit_cent: node_config_f64(node, "adaptiveMaxPriceMaxRelaxCreditCent")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_MAX_RELAX_CREDIT_CENT),
        hard_cap_cent: node_config_f64(node, "adaptiveMaxPriceHardCapCent")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_HARD_CAP_CENT),
        extra_buffer_cent: node_config_f64(node, "adaptiveMaxPriceExtraBufferCent")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_EXTRA_BUFFER_CENT),
        pair_buffer_cent: node_config_f64(node, "adaptiveMaxPricePairBufferCent")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_PAIR_BUFFER_CENT),
        size_multiplier: node_config_f64(node, "adaptiveMaxPriceSizeMultiplier")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_SIZE_MULTIPLIER),
        late_relax_cutoff_s: node_config_i64(node, "adaptiveMaxPriceLateRelaxCutoffS")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_LATE_RELAX_CUTOFF_S),
        sl_cooldown_markets: node_config_i64(node, "adaptiveMaxPriceSlCooldownMarkets")
            .filter(|value| *value >= 0)
            .map(|value| value as usize)
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_SL_COOLDOWN_MARKETS),
    };
    anyhow::ensure!(
        config.required_good_miss_count <= config.miss_count,
        "action.place_order adaptiveMaxPriceRequiredGoodMissCount must be <= adaptiveMaxPriceMissCount"
    );
    anyhow::ensure!(
        config.relax_credit_cent.is_finite() && config.relax_credit_cent > 0.0,
        "action.place_order adaptiveMaxPriceRelaxCreditCent must be > 0"
    );
    anyhow::ensure!(
        config.max_relax_credit_cent.is_finite() && config.max_relax_credit_cent > 0.0,
        "action.place_order adaptiveMaxPriceMaxRelaxCreditCent must be > 0"
    );
    anyhow::ensure!(
        config.hard_cap_cent.is_finite()
            && config.hard_cap_cent > 0.0
            && config.hard_cap_cent < 100.0,
        "action.place_order adaptiveMaxPriceHardCapCent must be in (0, 100)"
    );
    anyhow::ensure!(
        config.extra_buffer_cent.is_finite() && config.extra_buffer_cent >= 1.0,
        "action.place_order adaptiveMaxPriceExtraBufferCent must be >= 1"
    );
    anyhow::ensure!(
        config.pair_buffer_cent.is_finite() && config.pair_buffer_cent >= 0.0,
        "action.place_order adaptiveMaxPricePairBufferCent must be >= 0"
    );
    anyhow::ensure!(
        config.size_multiplier.is_finite()
            && config.size_multiplier > 0.0
            && config.size_multiplier <= 1.0,
        "action.place_order adaptiveMaxPriceSizeMultiplier must be in (0, 1]"
    );
    anyhow::ensure!(
        config.late_relax_cutoff_s > 0,
        "action.place_order adaptiveMaxPriceLateRelaxCutoffS must be > 0"
    );
    Ok(config)
}

fn pair_lock_adaptive_max_price_current_market_relaxed(
    context: &Value,
    node_key: &str,
    market_slug: &str,
    outcome_label: &str,
) -> bool {
    flow_node_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_ADAPTIVE_MAX_PRICE_RELAXED_MARKET,
    )
    .as_deref()
        == Some(market_slug)
        && flow_node_state_string(
            context,
            node_key,
            FLOW_NODE_STATE_PAIR_LOCK_ADAPTIVE_MAX_PRICE_RELAXED_OUTCOME,
        )
        .as_deref()
            == Some(outcome_label)
}

fn mark_pair_lock_adaptive_max_price_relaxed(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
    outcome_label: &str,
) {
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_ADAPTIVE_MAX_PRICE_RELAXED_MARKET,
        json!(market_slug),
    );
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PAIR_LOCK_ADAPTIVE_MAX_PRICE_RELAXED_OUTCOME,
        json!(outcome_label),
    );
}

fn pair_lock_adaptive_cent_value(value: Option<f64>) -> Option<f64> {
    let value = value?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    Some(if value <= 1.0 { value * 100.0 } else { value })
}

fn pair_lock_adaptive_probability_value(value: Option<f64>) -> Option<f64> {
    let value = value?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    Some(if value > 1.0 { value / 100.0 } else { value })
        .filter(|value| *value > 0.0 && *value <= 1.0)
}

fn pair_lock_adaptive_iv_payload(
    candidate: &ActionPlaceOrderPairLockPrimaryCandidateEval,
) -> Option<&Value> {
    let guard = candidate.diagnostics.get("price_to_beat_guard")?;
    if guard.get("threshold_mode").and_then(Value::as_str) != Some("iv_mismatch_edge") {
        return None;
    }
    guard.get("iv_mismatch_edge")
}

fn pair_lock_adaptive_ptb_passed(
    candidate: &ActionPlaceOrderPairLockPrimaryCandidateEval,
) -> bool {
    candidate
        .diagnostics
        .get("price_to_beat_guard")
        .and_then(|guard| guard.get("passed"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && pair_lock_adaptive_iv_payload(candidate)
            .and_then(|payload| payload.get("passed"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

fn pair_lock_adaptive_depth_passed(
    candidate: &ActionPlaceOrderPairLockPrimaryCandidateEval,
) -> bool {
    pair_lock_adaptive_iv_payload(candidate)
        .and_then(|payload| payload.get("depth_guard_result"))
        .and_then(Value::as_str)
        == Some("pass")
}

fn pair_lock_adaptive_estimated_avg_fill(
    candidate: &ActionPlaceOrderPairLockPrimaryCandidateEval,
) -> Option<f64> {
    pair_lock_adaptive_iv_payload(candidate)
        .and_then(|payload| payload.get("estimated_avg_fill"))
        .and_then(value_as_f64)
        .and_then(|value| pair_lock_adaptive_probability_value(Some(value)))
}

fn pair_lock_adaptive_q_final(
    candidate: &ActionPlaceOrderPairLockPrimaryCandidateEval,
) -> Option<f64> {
    pair_lock_adaptive_iv_payload(candidate)
        .and_then(|payload| payload.get("q_final"))
        .and_then(value_as_f64)
        .or_else(|| {
            pair_lock_adaptive_iv_payload(candidate)
                .and_then(|payload| payload.get("q"))
                .and_then(value_as_f64)
        })
        .and_then(|value| pair_lock_adaptive_probability_value(Some(value)))
}

fn pair_lock_adaptive_dynamic_threshold(
    candidate: &ActionPlaceOrderPairLockPrimaryCandidateEval,
) -> Option<f64> {
    pair_lock_adaptive_iv_payload(candidate)
        .and_then(|payload| payload.get("dynamic_threshold"))
        .and_then(value_as_f64)
        .or_else(|| {
            pair_lock_adaptive_iv_payload(candidate)
                .and_then(|payload| payload.get("threshold"))
                .and_then(value_as_f64)
        })
        .and_then(|value| pair_lock_adaptive_probability_value(Some(value)))
}

fn pair_lock_adaptive_volume_regime(
    candidate: &ActionPlaceOrderPairLockPrimaryCandidateEval,
) -> &'static str {
    let Some(ratio) = pair_lock_adaptive_iv_payload(candidate)
        .and_then(|payload| payload.get("hourly_volume_ratio"))
        .and_then(value_as_f64)
        .filter(|value| value.is_finite() && *value >= 0.0)
    else {
        return "normal";
    };
    if ratio >= 4.0 {
        "extreme"
    } else if ratio >= 2.5 {
        "high"
    } else if ratio >= 1.5 {
        "elevated"
    } else {
        "normal"
    }
}

fn pair_lock_adaptive_ptb_trend(
    candidate: &ActionPlaceOrderPairLockPrimaryCandidateEval,
) -> &'static str {
    match pair_lock_adaptive_iv_payload(candidate)
        .and_then(|payload| payload.get("gap_velocity"))
        .and_then(value_as_f64)
    {
        Some(value) if value > 0.0 => "expanding",
        Some(value) if value < 0.0 => "collapsing",
        Some(_) => "flat",
        None => "unknown",
    }
}

fn pair_lock_adaptive_market_elapsed_s(market_slug: &str) -> Option<i64> {
    let scope = find_updown_scope_by_slug(market_slug)?;
    let start = MarketCycleId(market_slug.to_string()).start_time()?;
    let window = updown_scope_window_seconds(scope).max(1);
    Some(
        Utc::now()
            .signed_duration_since(start)
            .num_seconds()
            .clamp(0, window),
    )
}

fn pair_lock_adaptive_miss_classification(
    summary: &bot_infra::db::TradeFlowAutoTuneMarketSummaryRecord,
) -> &'static str {
    let adaptive: Option<&Value> = summary.metrics_json.get("adaptive_max_price");
    if let Some(classification) = adaptive
        .and_then(|value| value.get("resolved_classification"))
        .or_else(|| adaptive.and_then(|value| value.get("classification")))
        .and_then(Value::as_str)
    {
        return match classification {
            "good_miss" => "good_miss",
            "good_block" => "good_block",
            "pending_miss" => "pending_miss",
            "unknown_miss" => "unknown_miss",
            "bad_data_miss" => "bad_data_miss",
            _ => "unknown_miss",
        };
    }
    let pnl = adaptive
        .and_then(|value| value.get("shadow_hold_pnl_usdc"))
        .and_then(value_as_f64)
        .or_else(|| {
            adaptive
                .and_then(|value| value.get("shadow_tp_sl_pnl_usdc"))
                .and_then(value_as_f64)
        });
    match pnl {
        Some(value) if value > 0.0 => "good_miss",
        Some(_) => "good_block",
        None => "unknown_miss",
    }
}

fn pair_lock_adaptive_summary_outcome_matches(
    summary: &bot_infra::db::TradeFlowAutoTuneMarketSummaryRecord,
    outcome_label: &str,
) -> bool {
    let expected = normalize_pair_lock_binary_outcome(outcome_label);
    let observed = summary
        .metrics_json
        .get("adaptive_max_price")
        .and_then(|value| value.get("outcome_label"))
        .and_then(Value::as_str)
        .or_else(|| summary.metrics_json.get("outcome_label").and_then(Value::as_str))
        .and_then(normalize_pair_lock_binary_outcome);
    expected.is_some() && expected == observed
}

fn pair_lock_adaptive_summary_is_ptb_max_price_miss(
    summary: &bot_infra::db::TradeFlowAutoTuneMarketSummaryRecord,
) -> bool {
    if !summary.max_price_block || summary.ptb_block {
        return false;
    }
    summary
        .metrics_json
        .get("adaptive_max_price")
        .and_then(|value| value.get("ptb_pass"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || summary
            .metrics_json
            .get("adaptive_max_price")
            .and_then(|value| value.get("decision"))
            .and_then(Value::as_str)
            == Some("NO_RELAX")
}

fn build_pair_lock_adaptive_history(
    summaries: &[bot_infra::db::TradeFlowAutoTuneMarketSummaryRecord],
    outcome_label: &str,
    config: PairLockAdaptiveMaxPriceConfig,
) -> PairLockAdaptiveMaxPriceHistory {
    let mut history = PairLockAdaptiveMaxPriceHistory::default();
    history.recent_sl = summaries
        .iter()
        .filter(|summary| pair_lock_adaptive_summary_outcome_matches(summary, outcome_label))
        .take(config.sl_cooldown_markets)
        .any(|summary| summary.sl_hit);
    history.cooldown_active = history.recent_sl;

    for summary in summaries
        .iter()
        .filter(|summary| pair_lock_adaptive_summary_outcome_matches(summary, outcome_label))
        .filter(|summary| pair_lock_adaptive_summary_is_ptb_max_price_miss(summary))
    {
        match pair_lock_adaptive_miss_classification(summary) {
            "good_miss" => {
                history.resolved_good_miss_count += 1;
                history.resolved_miss_count += 1;
            }
            "good_block" => {
                history.resolved_good_block_count += 1;
                history.resolved_miss_count += 1;
            }
            "pending_miss" => history.pending_miss_count += 1,
            "bad_data_miss" => history.bad_data_miss_count += 1,
            _ => history.unknown_miss_count += 1,
        }
        if history.resolved_miss_count >= config.miss_count {
            break;
        }
    }
    history
}

fn pair_lock_adaptive_history_payload(history: &PairLockAdaptiveMaxPriceHistory) -> Value {
    json!({
        "resolved_good_miss_count": history.resolved_good_miss_count,
        "resolved_good_block_count": history.resolved_good_block_count,
        "pending_miss_count": history.pending_miss_count,
        "unknown_miss_count": history.unknown_miss_count,
        "bad_data_miss_count": history.bad_data_miss_count,
        "resolved_miss_count": history.resolved_miss_count,
        "recent_sl": history.recent_sl,
        "cooldown_active": history.cooldown_active,
    })
}

fn no_relax_decision(
    input: &PairLockAdaptiveMaxPriceDecisionInput<'_>,
    reason: &'static str,
    edge_price_cap: Option<f64>,
    pair_cap_price_limit: Option<f64>,
    effective_max_price: Option<f64>,
) -> PairLockAdaptiveMaxPriceDecision {
    PairLockAdaptiveMaxPriceDecision {
        relax_applied: false,
        decision: "NO_RELAX",
        reason,
        effective_max_price,
        effective_size_usdc: None,
        diagnostics: pair_lock_adaptive_diagnostics(
            input,
            "NO_RELAX",
            reason,
            false,
            edge_price_cap,
            pair_cap_price_limit,
            effective_max_price,
            None,
        ),
    }
}

fn pair_lock_adaptive_diagnostics(
    input: &PairLockAdaptiveMaxPriceDecisionInput<'_>,
    decision: &'static str,
    reason: &'static str,
    relax_applied: bool,
    edge_price_cap: Option<f64>,
    pair_cap_price_limit: Option<f64>,
    effective_max_price: Option<f64>,
    effective_size_usdc: Option<f64>,
) -> Value {
    let config = input.config;
    json!({
        "strategy": PAIR_LOCK_STRATEGY_ADAPTIVE_MAX_PRICE_V1,
        "evaluated": true,
        "base_max_price_cent": pair_lock_adaptive_cent_value(input.base_max_price),
        "ask_cent": pair_lock_adaptive_cent_value(input.ask),
        "estimated_avg_fill_cent": pair_lock_adaptive_cent_value(input.estimated_avg_fill),
        "counter_estimated_avg_fill_cent": pair_lock_adaptive_cent_value(input.counter_estimated_avg_fill),
        "q_final_cent": pair_lock_adaptive_cent_value(input.q_final),
        "dynamic_threshold_cent": pair_lock_adaptive_cent_value(input.dynamic_threshold),
        "extra_buffer_cent": config.extra_buffer_cent,
        "edge_price_cap_cent": edge_price_cap.map(|value| value * 100.0),
        "relax_credit_cent": config.relax_credit_cent.min(config.max_relax_credit_cent),
        "max_relax_credit_cent": config.max_relax_credit_cent,
        "hard_cap_cent": config.hard_cap_cent,
        "pair_max_total_cent": input.pair_max_total_price * 100.0,
        "pair_buffer_cent": config.pair_buffer_cent,
        "pair_cap_price_limit_cent": pair_cap_price_limit.map(|value| value * 100.0),
        "effective_max_price_cent": effective_max_price.map(|value| value * 100.0),
        "relax_applied": relax_applied,
        "base_size_usdc": input.base_size_usdc,
        "size_multiplier": config.size_multiplier,
        "effective_size_usdc": effective_size_usdc,
        "ptb_pass": input.ptb_passed,
        "base_max_price_block": input.base_max_price_blocked,
        "depth_guard_pass": input.depth_guard_pass,
        "counter_depth_ok": input.counter_depth_ok,
        "book_reliability_ok": input.book_reliability_ok,
        "volume_regime": input.volume_regime,
        "ptb_trend": input.ptb_trend,
        "market_elapsed_s": input.market_elapsed_s,
        "late_relax_cutoff_s": config.late_relax_cutoff_s,
        "already_relaxed_current_market": input.already_relaxed_current_market,
        "miss_count": config.miss_count,
        "required_good_miss_count": config.required_good_miss_count,
        "classification": if relax_applied { Value::Null } else { json!("pending_miss") },
        "history": pair_lock_adaptive_history_payload(&input.history),
        "resolved_good_miss_count": input.history.resolved_good_miss_count,
        "resolved_good_block_count": input.history.resolved_good_block_count,
        "pending_miss_count": input.history.pending_miss_count,
        "unknown_miss_count": input.history.unknown_miss_count,
        "decision": decision,
        "reason": reason,
    })
}

fn evaluate_pair_lock_adaptive_max_price_decision(
    input: PairLockAdaptiveMaxPriceDecisionInput<'_>,
) -> PairLockAdaptiveMaxPriceDecision {
    let base_max_price = match input.base_max_price {
        Some(value) if value.is_finite() && value > 0.0 => value,
        _ => return no_relax_decision(&input, "base_max_price_unavailable", None, None, None),
    };
    let ask = match input.ask {
        Some(value) if value.is_finite() && value > 0.0 => value,
        _ => return no_relax_decision(&input, "ask_unavailable", None, None, None),
    };
    let estimated_avg_fill = match input.estimated_avg_fill {
        Some(value) if value.is_finite() && value > 0.0 => value,
        _ => return no_relax_decision(&input, "estimated_avg_fill_unavailable", None, None, None),
    };
    let counter_estimated_avg_fill = match input.counter_estimated_avg_fill {
        Some(value) if value.is_finite() && value > 0.0 => value,
        _ => {
            return no_relax_decision(
                &input,
                "counter_estimated_avg_fill_unavailable",
                None,
                None,
                None,
            )
        }
    };
    let q_final = match input.q_final {
        Some(value) if value.is_finite() && value > 0.0 => value,
        _ => return no_relax_decision(&input, "q_final_unavailable", None, None, None),
    };
    let dynamic_threshold = match input.dynamic_threshold {
        Some(value) if value.is_finite() && value >= 0.0 => value,
        _ => return no_relax_decision(&input, "dynamic_threshold_unavailable", None, None, None),
    };
    let edge_price_cap = q_final - dynamic_threshold - input.config.extra_buffer_cent / 100.0;
    let pair_cap_price_limit =
        input.pair_max_total_price - counter_estimated_avg_fill - input.config.pair_buffer_cent / 100.0;
    let relaxed_cap = base_max_price
        + input
            .config
            .relax_credit_cent
            .min(input.config.max_relax_credit_cent)
            / 100.0;
    let hard_cap = input.config.hard_cap_cent / 100.0;
    let effective_max_price = relaxed_cap
        .min(edge_price_cap)
        .min(hard_cap)
        .min(pair_cap_price_limit);

    if !input.ptb_passed {
        return no_relax_decision(
            &input,
            "ptb_not_passed",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if !input.base_max_price_blocked {
        return no_relax_decision(
            &input,
            "base_max_price_not_blocked",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if ask <= base_max_price {
        return no_relax_decision(
            &input,
            "ask_not_above_base_max_price",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if input.history.resolved_miss_count < input.config.miss_count {
        return no_relax_decision(
            &input,
            "resolved_miss_history_insufficient",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if input.history.resolved_good_miss_count < input.config.required_good_miss_count {
        return no_relax_decision(
            &input,
            "resolved_good_miss_below_required",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if input.history.resolved_good_block_count > input.history.resolved_good_miss_count {
        return no_relax_decision(
            &input,
            "good_block_majority",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if !matches!(input.volume_regime, "normal" | "elevated") {
        return no_relax_decision(
            &input,
            "high_volume",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if input.ptb_trend != "expanding" {
        return no_relax_decision(
            &input,
            "ptb_not_expanding",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if input
        .market_elapsed_s
        .is_none_or(|elapsed| elapsed >= input.config.late_relax_cutoff_s)
    {
        return no_relax_decision(
            &input,
            "late_market",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if input.history.recent_sl || input.history.cooldown_active {
        return no_relax_decision(
            &input,
            "recent_sl_cooldown",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if input.already_relaxed_current_market {
        return no_relax_decision(
            &input,
            "market_already_relaxed",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if edge_price_cap < estimated_avg_fill {
        return no_relax_decision(
            &input,
            "edge_price_cap_below_estimated_fill",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if pair_cap_price_limit < estimated_avg_fill {
        return no_relax_decision(
            &input,
            "pair_cap_below_estimated_fill",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if estimated_avg_fill > effective_max_price {
        return no_relax_decision(
            &input,
            "estimated_avg_fill_above_effective_max_price",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if ask > effective_max_price {
        return no_relax_decision(
            &input,
            "ask_above_effective_max_price",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if effective_max_price <= base_max_price {
        return no_relax_decision(
            &input,
            "effective_max_price_not_above_base",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if !input.depth_guard_pass {
        return no_relax_decision(
            &input,
            "depth_guard_not_passed",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if !input.counter_depth_ok {
        return no_relax_decision(
            &input,
            "counter_depth_not_ok",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if !input.book_reliability_ok {
        return no_relax_decision(
            &input,
            "book_reliability_not_ok",
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }

    let effective_size_usdc = input.base_size_usdc * input.config.size_multiplier;
    PairLockAdaptiveMaxPriceDecision {
        relax_applied: true,
        decision: "RELAX_ALLOW",
        reason: "resolved_good_miss_history_normal_expanding",
        effective_max_price: Some(effective_max_price),
        effective_size_usdc: Some(effective_size_usdc),
        diagnostics: pair_lock_adaptive_diagnostics(
            &input,
            "RELAX_ALLOW",
            "resolved_good_miss_history_normal_expanding",
            true,
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
            Some(effective_size_usdc),
        ),
    }
}

async fn resolve_pair_lock_adaptive_max_price_decision_for_candidate(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &Value,
    market_slug: &str,
    pair_lock: &ActionPlaceOrderPairLockConfig,
    candidate: &ActionPlaceOrderPairLockPrimaryCandidateEval,
    counter: &ActionPlaceOrderPairLockPrimaryCandidateEval,
) -> Result<PairLockAdaptiveMaxPriceDecision> {
    let config = resolve_pair_lock_adaptive_max_price_config(node)?;
    let market_scope = find_updown_scope_by_slug(market_slug)
        .map(|scope| scope.scope.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let summaries = repo
        .list_trade_flow_auto_tune_market_summaries(
            run.definition_id,
            run.version_id,
            &node.key,
            &market_scope,
            (config.miss_count + config.sl_cooldown_markets + 8) as i64,
        )
        .await
        .unwrap_or_default();
    let history = build_pair_lock_adaptive_history(&summaries, &candidate.outcome_label, config);
    let base_max_price = candidate
        .diagnostics
        .pointer("/max_price_guard/details/max_price")
        .and_then(value_as_f64)
        .and_then(|value| pair_lock_adaptive_probability_value(Some(value)));
    let ask = candidate
        .quote
        .best_ask
        .or_else(|| {
            pair_lock_adaptive_iv_payload(candidate)
                .and_then(|payload| payload.get("ask"))
                .and_then(value_as_f64)
        })
        .and_then(|value| pair_lock_adaptive_probability_value(Some(value)));
    Ok(evaluate_pair_lock_adaptive_max_price_decision(
        PairLockAdaptiveMaxPriceDecisionInput {
            config,
            base_max_price,
            ask,
            estimated_avg_fill: pair_lock_adaptive_estimated_avg_fill(candidate),
            counter_estimated_avg_fill: pair_lock_adaptive_estimated_avg_fill(counter),
            q_final: pair_lock_adaptive_q_final(candidate),
            dynamic_threshold: pair_lock_adaptive_dynamic_threshold(candidate),
            pair_max_total_price: pair_lock.max_total_price,
            base_size_usdc: pair_lock.primary_leg_size_usdc,
            ptb_passed: pair_lock_adaptive_ptb_passed(candidate),
            base_max_price_blocked: candidate.reason_code == "above_max_price",
            depth_guard_pass: pair_lock_adaptive_depth_passed(candidate),
            counter_depth_ok: pair_lock_adaptive_depth_passed(counter),
            book_reliability_ok: candidate.quote.quote_book_missing_fields.is_empty()
                && counter.quote.quote_book_missing_fields.is_empty(),
            volume_regime: pair_lock_adaptive_volume_regime(candidate),
            ptb_trend: pair_lock_adaptive_ptb_trend(candidate),
            market_elapsed_s: pair_lock_adaptive_market_elapsed_s(market_slug),
            already_relaxed_current_market: pair_lock_adaptive_max_price_current_market_relaxed(
                context,
                &node.key,
                market_slug,
                &candidate.outcome_label,
            ),
            history,
        },
    ))
}

async fn maybe_apply_pair_lock_adaptive_max_price_candidate_override(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &Value,
    market_slug: &str,
    pair_lock: &ActionPlaceOrderPairLockConfig,
    candidate: &mut ActionPlaceOrderPairLockPrimaryCandidateEval,
    counter: &ActionPlaceOrderPairLockPrimaryCandidateEval,
) -> Result<()> {
    if !action_place_order_uses_adaptive_max_price_strategy(node) {
        return Ok(());
    }
    let decision = resolve_pair_lock_adaptive_max_price_decision_for_candidate(
        repo,
        run,
        node,
        context,
        market_slug,
        pair_lock,
        candidate,
        counter,
    )
    .await?;
    if let Some(obj) = candidate.diagnostics.as_object_mut() {
        obj.insert("adaptive_max_price".to_string(), decision.diagnostics.clone());
        obj.insert(
            "adaptive_max_price_decision".to_string(),
            json!(decision.decision),
        );
        obj.insert(
            "adaptive_max_price_reason".to_string(),
            json!(decision.reason),
        );
    }
    if decision.relax_applied {
        let override_result = PairLockAdaptiveMaxPriceOverride {
            effective_max_price: decision
                .effective_max_price
                .expect("relax_applied requires effective max price"),
            effective_size_usdc: decision
                .effective_size_usdc
                .expect("relax_applied requires effective size"),
            diagnostics: decision.diagnostics,
        };
        candidate.decision = "passed";
        candidate.reason_code = "adaptive_max_price_relaxed".to_string();
        if let Some(obj) = candidate.diagnostics.as_object_mut() {
            obj.insert("passed".to_string(), json!(true));
            obj.insert("decision".to_string(), json!("passed"));
            obj.insert(
                "reason_code".to_string(),
                json!("adaptive_max_price_relaxed"),
            );
            obj.insert(
                "adaptive_max_price".to_string(),
                override_result.diagnostics.clone(),
            );
        }
        candidate.adaptive_max_price_override = Some(override_result);
    }
    Ok(())
}

#[cfg(test)]
mod pair_lock_adaptive_max_price_tests {
    use super::*;

    fn default_config() -> PairLockAdaptiveMaxPriceConfig {
        PairLockAdaptiveMaxPriceConfig {
            miss_count: 3,
            required_good_miss_count: 2,
            relax_credit_cent: 2.0,
            max_relax_credit_cent: 5.0,
            hard_cap_cent: 76.0,
            extra_buffer_cent: 1.0,
            pair_buffer_cent: 1.0,
            size_multiplier: 0.5,
            late_relax_cutoff_s: 210,
            sl_cooldown_markets: 3,
        }
    }

    fn good_history() -> PairLockAdaptiveMaxPriceHistory {
        PairLockAdaptiveMaxPriceHistory {
            resolved_good_miss_count: 2,
            resolved_good_block_count: 1,
            resolved_miss_count: 3,
            ..Default::default()
        }
    }

    fn default_input() -> PairLockAdaptiveMaxPriceDecisionInput<'static> {
        PairLockAdaptiveMaxPriceDecisionInput {
            config: default_config(),
            base_max_price: Some(0.70),
            ask: Some(0.72),
            estimated_avg_fill: Some(0.72),
            counter_estimated_avg_fill: Some(0.23),
            q_final: Some(0.84),
            dynamic_threshold: Some(0.07),
            pair_max_total_price: 0.96,
            base_size_usdc: 5.0,
            ptb_passed: true,
            base_max_price_blocked: true,
            depth_guard_pass: true,
            counter_depth_ok: true,
            book_reliability_ok: true,
            volume_regime: "normal",
            ptb_trend: "expanding",
            market_elapsed_s: Some(144),
            already_relaxed_current_market: false,
            history: good_history(),
        }
    }

    fn adaptive_summary(
        outcome_label: &str,
        classification: &str,
        sl_hit: bool,
    ) -> bot_infra::db::TradeFlowAutoTuneMarketSummaryRecord {
        bot_infra::db::TradeFlowAutoTuneMarketSummaryRecord {
            id: 1,
            definition_id: 1,
            version_id: 1,
            flow_run_id: Some(1),
            node_key: "pair_buy".to_string(),
            market_scope: "eth_5m_updown".to_string(),
            market_slug: "eth-updown-5m-1".to_string(),
            window_start: None,
            window_end: None,
            completed_at: Utc::now(),
            trigger_passed: true,
            action_started: true,
            builder_order_created: false,
            order_submitted: false,
            order_filled: false,
            first_terminal_guard_scope: None,
            first_terminal_guard_code: None,
            first_terminal_guard_node: None,
            first_terminal_guard_at: None,
            last_guard_scope: None,
            last_guard_code: None,
            max_price_block: true,
            execution_floor_block: false,
            ptb_block: false,
            pair_total_block: false,
            counter_max_block: false,
            counter_floor_block: false,
            risk_block: false,
            data_problem_block: false,
            best_ask_at_block: Some(0.72),
            max_price_effective: Some(0.70),
            execution_floor_effective: None,
            pair_total_effective: Some(0.96),
            counter_price_effective: Some(0.23),
            iv_edge_margin: Some(0.07),
            binance_stale_ms: None,
            binance_same_direction: None,
            depth_ok: Some(true),
            floor_recovered_once: false,
            max_best_ask_after_block: None,
            tradable_seconds_count: Some(180),
            pair_session_id: None,
            pair_locked: false,
            locked_qty: None,
            unpaired_qty: None,
            locked_profit_per_share: None,
            orphan_detected: false,
            protective_unwind_triggered: false,
            sl_hit,
            tp_hit: false,
            realized_pnl_usdc: None,
            metrics_json: json!({
                "adaptive_max_price": {
                    "outcome_label": outcome_label,
                    "ptb_pass": true,
                    "resolved_classification": classification,
                }
            }),
        }
    }

    #[test]
    fn adaptive_max_price_allows_resolved_good_miss_normal_expanding() {
        let decision = evaluate_pair_lock_adaptive_max_price_decision(default_input());
        assert!(decision.relax_applied);
        assert_eq!(decision.decision, "RELAX_ALLOW");
        assert_eq!(decision.effective_max_price, Some(0.72));
        assert_eq!(decision.effective_size_usdc, Some(2.5));
    }

    #[test]
    fn adaptive_max_price_ignores_pending_and_unknown_misses() {
        let mut input = default_input();
        input.history = PairLockAdaptiveMaxPriceHistory {
            resolved_good_miss_count: 1,
            resolved_good_block_count: 0,
            pending_miss_count: 2,
            unknown_miss_count: 2,
            resolved_miss_count: 1,
            ..Default::default()
        };
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(!decision.relax_applied);
        assert_eq!(decision.reason, "resolved_miss_history_insufficient");
    }

    #[test]
    fn adaptive_max_price_blocks_late_market() {
        let mut input = default_input();
        input.market_elapsed_s = Some(210);
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(!decision.relax_applied);
        assert_eq!(decision.reason, "late_market");
    }

    #[test]
    fn adaptive_max_price_blocks_estimated_fill_above_effective_cap() {
        let mut input = default_input();
        input.estimated_avg_fill = Some(0.731);
        input.counter_estimated_avg_fill = Some(0.20);
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(!decision.relax_applied);
        assert_eq!(decision.reason, "estimated_avg_fill_above_effective_max_price");
    }

    #[test]
    fn adaptive_max_price_uses_counter_vwap_for_pair_cap() {
        let mut input = default_input();
        input.counter_estimated_avg_fill = Some(0.26);
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(!decision.relax_applied);
        assert_eq!(decision.reason, "pair_cap_below_estimated_fill");
    }

    #[test]
    fn adaptive_max_price_blocks_after_recent_sl_cooldown() {
        let mut input = default_input();
        input.history.recent_sl = true;
        input.history.cooldown_active = true;
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(!decision.relax_applied);
        assert_eq!(decision.reason, "recent_sl_cooldown");
    }

    #[test]
    fn adaptive_max_price_normalizes_probability_units_to_cent_payload() {
        let decision = evaluate_pair_lock_adaptive_max_price_decision(default_input());
        let payload = decision.diagnostics;
        assert_eq!(payload.get("q_final_cent").and_then(Value::as_f64), Some(84.0));
        let threshold = payload
            .get("dynamic_threshold_cent")
            .and_then(Value::as_f64)
            .expect("dynamic threshold");
        assert!((threshold - 7.0).abs() < 0.000001);
    }

    #[test]
    fn adaptive_max_price_blocks_collapsing_or_high_volume() {
        let mut collapsing = default_input();
        collapsing.ptb_trend = "collapsing";
        let collapsing_decision = evaluate_pair_lock_adaptive_max_price_decision(collapsing);
        assert_eq!(collapsing_decision.reason, "ptb_not_expanding");

        let mut high_volume = default_input();
        high_volume.volume_regime = "high";
        let high_volume_decision = evaluate_pair_lock_adaptive_max_price_decision(high_volume);
        assert_eq!(high_volume_decision.reason, "high_volume");
    }

    #[test]
    fn adaptive_max_price_skips_when_ask_does_not_need_override() {
        let mut input = default_input();
        input.ask = Some(0.70);
        let decision = evaluate_pair_lock_adaptive_max_price_decision(input);
        assert!(!decision.relax_applied);
        assert_eq!(decision.reason, "ask_not_above_base_max_price");
    }

    #[test]
    fn adaptive_max_price_history_is_side_specific() {
        let summaries = vec![
            adaptive_summary("Down", "good_block", false),
            adaptive_summary("Up", "good_miss", false),
            adaptive_summary("Up", "good_miss", false),
            adaptive_summary("Down", "good_block", false),
            adaptive_summary("Up", "good_block", false),
        ];

        let up_history = build_pair_lock_adaptive_history(&summaries, "UP", default_config());
        assert_eq!(up_history.resolved_good_miss_count, 2);
        assert_eq!(up_history.resolved_good_block_count, 1);

        let down_history = build_pair_lock_adaptive_history(&summaries, "DOWN", default_config());
        assert_eq!(down_history.resolved_good_miss_count, 0);
        assert_eq!(down_history.resolved_good_block_count, 2);
    }
}
