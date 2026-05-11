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
const DEFAULT_ADAPTIVE_MAX_PRICE_LATE_RISK_ENABLED: bool = true;
const DEFAULT_ADAPTIVE_MAX_PRICE_LATE_RISK_AFTER_S: i64 = 210;
const DEFAULT_ADAPTIVE_MAX_PRICE_LATE_EXTRA_BUFFER_CENT: f64 = 1.0;
const DEFAULT_ADAPTIVE_MAX_PRICE_LATE_SIZE_MULTIPLIER: f64 = 0.35;
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
    window_start_sec: Option<i64>,
    window_end_sec: Option<i64>,
    legacy_late_relax_cutoff_s: Option<i64>,
    late_risk_enabled: bool,
    late_risk_after_sec: i64,
    late_extra_buffer_cent: f64,
    late_size_multiplier: f64,
    sl_cooldown_markets: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PairLockAdaptiveMaxPriceTiming {
    configured_window_start_sec: i64,
    configured_window_end_sec: i64,
    cycle_window_start_sec: Option<i64>,
    cycle_window_end_sec: Option<i64>,
    effective_window_start_sec: i64,
    effective_window_end_sec: i64,
    market_elapsed_s: Option<i64>,
    in_adaptive_window: bool,
    late_risk_enabled: bool,
    late_risk_after_sec: i64,
    late_risk_active: bool,
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
    cycle_window_start_sec: Option<i64>,
    cycle_window_end_sec: Option<i64>,
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
    resolve_pair_lock_adaptive_max_price_notify_config(node)?;
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
        window_start_sec: node_config_i64(node, "adaptiveMaxPriceWindowStartSec"),
        window_end_sec: node_config_i64(node, "adaptiveMaxPriceWindowEndSec"),
        legacy_late_relax_cutoff_s: node_config_i64(node, "adaptiveMaxPriceLateRelaxCutoffS"),
        late_risk_enabled: node_config_bool(node, "adaptiveMaxPriceLateRiskEnabled")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_LATE_RISK_ENABLED),
        late_risk_after_sec: node_config_i64(node, "adaptiveMaxPriceLateRiskAfterSec")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_LATE_RISK_AFTER_S),
        late_extra_buffer_cent: node_config_f64(node, "adaptiveMaxPriceLateExtraBufferCent")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_LATE_EXTRA_BUFFER_CENT),
        late_size_multiplier: node_config_f64(node, "adaptiveMaxPriceLateSizeMultiplier")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_LATE_SIZE_MULTIPLIER),
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
    if let Some(start) = config.window_start_sec {
        anyhow::ensure!(
            (0..=300).contains(&start),
            "action.place_order adaptiveMaxPriceWindowStartSec must be in [0, 300]"
        );
    }
    if let Some(end) = config.window_end_sec {
        anyhow::ensure!(
            (0..=300).contains(&end),
            "action.place_order adaptiveMaxPriceWindowEndSec must be in [0, 300]"
        );
    }
    if let (Some(start), Some(end)) = (config.window_start_sec, config.window_end_sec) {
        anyhow::ensure!(
            start < end,
            "action.place_order adaptiveMaxPriceWindowStartSec must be < adaptiveMaxPriceWindowEndSec"
        );
    }
    if let Some(cutoff) = config.legacy_late_relax_cutoff_s {
        anyhow::ensure!(
            (1..=300).contains(&cutoff),
            "action.place_order adaptiveMaxPriceLateRelaxCutoffS must be in [1, 300]"
        );
    }
    anyhow::ensure!(
        (0..=300).contains(&config.late_risk_after_sec),
        "action.place_order adaptiveMaxPriceLateRiskAfterSec must be in [0, 300]"
    );
    anyhow::ensure!(
        config.late_extra_buffer_cent.is_finite() && config.late_extra_buffer_cent >= 0.0,
        "action.place_order adaptiveMaxPriceLateExtraBufferCent must be >= 0"
    );
    anyhow::ensure!(
        config.late_size_multiplier.is_finite()
            && config.late_size_multiplier > 0.0
            && config.late_size_multiplier <= 1.0,
        "action.place_order adaptiveMaxPriceLateSizeMultiplier must be in (0, 1]"
    );
    Ok(config)
}

fn pair_lock_adaptive_step_i64(step: &TradeFlowRunStep, key: &str) -> Option<i64> {
    step.input_json
        .as_ref()
        .and_then(|input| input.get(key))
        .and_then(value_as_i64)
}

fn pair_lock_adaptive_cycle_window_from_step(
    step: &TradeFlowRunStep,
) -> (Option<i64>, Option<i64>) {
    (
        pair_lock_adaptive_step_i64(step, "cycleWindowStartSec")
            .or_else(|| pair_lock_adaptive_step_i64(step, "cycle_window_start_sec")),
        pair_lock_adaptive_step_i64(step, "cycleWindowEndSec")
            .or_else(|| pair_lock_adaptive_step_i64(step, "cycle_window_end_sec")),
    )
}

fn pair_lock_adaptive_timing(
    input: &PairLockAdaptiveMaxPriceDecisionInput<'_>,
) -> PairLockAdaptiveMaxPriceTiming {
    let configured_window_start_sec = input
        .config
        .window_start_sec
        .or(input.cycle_window_start_sec)
        .unwrap_or(0);
    let configured_window_end_sec = input
        .config
        .window_end_sec
        .or(input.cycle_window_end_sec)
        .or(input.config.legacy_late_relax_cutoff_s)
        .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_LATE_RELAX_CUTOFF_S);
    let effective_window_start_sec = input
        .cycle_window_start_sec
        .map(|cycle_start| configured_window_start_sec.max(cycle_start))
        .unwrap_or(configured_window_start_sec);
    let effective_window_end_sec = input
        .cycle_window_end_sec
        .map(|cycle_end| configured_window_end_sec.min(cycle_end))
        .unwrap_or(configured_window_end_sec);
    let in_adaptive_window = input
        .market_elapsed_s
        .is_some_and(|elapsed| {
            elapsed >= effective_window_start_sec && elapsed <= effective_window_end_sec
        });
    let late_risk_active = input.config.late_risk_enabled
        && in_adaptive_window
        && input
            .market_elapsed_s
            .is_some_and(|elapsed| elapsed >= input.config.late_risk_after_sec);

    PairLockAdaptiveMaxPriceTiming {
        configured_window_start_sec,
        configured_window_end_sec,
        cycle_window_start_sec: input.cycle_window_start_sec,
        cycle_window_end_sec: input.cycle_window_end_sec,
        effective_window_start_sec,
        effective_window_end_sec,
        market_elapsed_s: input.market_elapsed_s,
        in_adaptive_window,
        late_risk_enabled: input.config.late_risk_enabled,
        late_risk_after_sec: input.config.late_risk_after_sec,
        late_risk_active,
    }
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
    pair_lock_adaptive_miss_classification_from_metrics(&summary.metrics_json)
}

fn pair_lock_adaptive_miss_classification_from_metrics(metrics_json: &Value) -> &'static str {
    let adaptive: Option<&Value> = metrics_json.get("adaptive_max_price");
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
    let timing = pair_lock_adaptive_timing(input);
    let applied_extra_buffer_cent = if timing.late_risk_active {
        config.extra_buffer_cent + config.late_extra_buffer_cent
    } else {
        config.extra_buffer_cent
    };
    let applied_size_multiplier = if timing.late_risk_active {
        config.size_multiplier.min(config.late_size_multiplier)
    } else {
        config.size_multiplier
    };
    let mut payload = json!({
        "strategy": PAIR_LOCK_STRATEGY_ADAPTIVE_MAX_PRICE_V1,
        "evaluated": true,
        "base_max_price_cent": pair_lock_adaptive_cent_value(input.base_max_price),
        "ask_cent": pair_lock_adaptive_cent_value(input.ask),
        "estimated_avg_fill_cent": pair_lock_adaptive_cent_value(input.estimated_avg_fill),
        "counter_estimated_avg_fill_cent": pair_lock_adaptive_cent_value(input.counter_estimated_avg_fill),
        "q_final_cent": pair_lock_adaptive_cent_value(input.q_final),
        "dynamic_threshold_cent": pair_lock_adaptive_cent_value(input.dynamic_threshold),
        "extra_buffer_cent": config.extra_buffer_cent,
        "applied_extra_buffer_cent": applied_extra_buffer_cent,
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
        "applied_size_multiplier": applied_size_multiplier,
        "effective_size_usdc": effective_size_usdc,
        "ptb_pass": input.ptb_passed,
        "base_max_price_block": input.base_max_price_blocked,
        "depth_guard_pass": input.depth_guard_pass,
        "counter_depth_ok": input.counter_depth_ok,
        "book_reliability_ok": input.book_reliability_ok,
    });
    append_json_object_fields(&mut payload, &json!({
        "volume_regime": input.volume_regime,
        "ptb_trend": input.ptb_trend,
        "timing": {
            "market_elapsed_s": timing.market_elapsed_s,
            "configured_window": {
                "start_sec": timing.configured_window_start_sec,
                "end_sec": timing.configured_window_end_sec,
            },
            "cycle_window": {
                "start_sec": timing.cycle_window_start_sec,
                "end_sec": timing.cycle_window_end_sec,
            },
            "effective_window": {
                "start_sec": timing.effective_window_start_sec,
                "end_sec": timing.effective_window_end_sec,
            },
            "in_adaptive_window": timing.in_adaptive_window,
            "late_risk_enabled": timing.late_risk_enabled,
            "late_risk_after_s": timing.late_risk_after_sec,
            "late_risk_active": timing.late_risk_active,
            "late_extra_buffer_cent": config.late_extra_buffer_cent,
            "late_size_multiplier": config.late_size_multiplier,
        },
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
    }));
    payload
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
    let timing = pair_lock_adaptive_timing(&input);
    let applied_extra_buffer_cent = if timing.late_risk_active {
        input.config.extra_buffer_cent + input.config.late_extra_buffer_cent
    } else {
        input.config.extra_buffer_cent
    };
    let applied_size_multiplier = if timing.late_risk_active {
        input.config.size_multiplier.min(input.config.late_size_multiplier)
    } else {
        input.config.size_multiplier
    };
    let edge_price_cap = q_final - dynamic_threshold - applied_extra_buffer_cent / 100.0;
    let base_edge_price_cap = q_final - dynamic_threshold - input.config.extra_buffer_cent / 100.0;
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
    let base_effective_max_price = relaxed_cap
        .min(base_edge_price_cap)
        .min(hard_cap)
        .min(pair_cap_price_limit);
    let late_risk_caused_price_block = timing.late_risk_active
        && base_edge_price_cap >= estimated_avg_fill
        && base_effective_max_price > base_max_price
        && estimated_avg_fill <= base_effective_max_price
        && ask <= base_effective_max_price;

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
    if !timing.in_adaptive_window {
        return no_relax_decision(
            &input,
            "outside_adaptive_window",
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
            if late_risk_caused_price_block {
                "late_risk_block"
            } else {
                "edge_price_cap_below_estimated_fill"
            },
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
            if late_risk_caused_price_block {
                "late_risk_block"
            } else {
                "estimated_avg_fill_above_effective_max_price"
            },
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if ask > effective_max_price {
        return no_relax_decision(
            &input,
            if late_risk_caused_price_block {
                "late_risk_block"
            } else {
                "ask_above_effective_max_price"
            },
            Some(edge_price_cap),
            Some(pair_cap_price_limit),
            Some(effective_max_price),
        );
    }
    if effective_max_price <= base_max_price {
        return no_relax_decision(
            &input,
            if late_risk_caused_price_block {
                "late_risk_block"
            } else {
                "effective_max_price_not_above_base"
            },
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

    let effective_size_usdc = input.base_size_usdc * applied_size_multiplier;
    let reason = if timing.late_risk_active {
        "late_risk_size_reduced"
    } else {
        "resolved_good_miss_history_normal_expanding"
    };
    PairLockAdaptiveMaxPriceDecision {
        relax_applied: true,
        decision: "RELAX_ALLOW",
        reason,
        effective_max_price: Some(effective_max_price),
        effective_size_usdc: Some(effective_size_usdc),
        diagnostics: pair_lock_adaptive_diagnostics(
            &input,
            "RELAX_ALLOW",
            reason,
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
    step: &TradeFlowRunStep,
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
    let (cycle_window_start_sec, cycle_window_end_sec) =
        pair_lock_adaptive_cycle_window_from_step(step);
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
            cycle_window_start_sec,
            cycle_window_end_sec,
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
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &mut Value,
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
        step,
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
    if !decision.relax_applied {
        maybe_notify_pair_lock_adaptive_no_relax(
            repo,
            run,
            node,
            context,
            market_slug,
            &candidate.outcome_label,
            &decision.diagnostics,
        )
        .await?;
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
