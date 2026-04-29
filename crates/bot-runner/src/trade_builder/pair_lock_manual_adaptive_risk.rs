const PAIR_LOCK_STRATEGY_MANUAL_ADAPTIVE_RISK_V1: &str = "manual_adaptive_risk_v1";
const DEFAULT_MANUAL_ADAPTIVE_VOLUME_NORMAL_LT: f64 = 1.5;
const DEFAULT_MANUAL_ADAPTIVE_VOLUME_ELEVATED_LT: f64 = 2.5;
const DEFAULT_MANUAL_ADAPTIVE_VOLUME_HIGH_LT: f64 = 4.0;
const DEFAULT_MANUAL_ADAPTIVE_TREND_DELTA_USD: f64 = 0.05;
const DEFAULT_MANUAL_ADAPTIVE_NORMAL_FLAT_MAX_PRICE_SUB_CENT: f64 = 2.0;
const DEFAULT_MANUAL_ADAPTIVE_NORMAL_FLAT_SIZE_MULTIPLIER: f64 = 0.8;
const DEFAULT_MANUAL_ADAPTIVE_NORMAL_FLAT_PTB_GAP_ADD_CENT: f64 = 5.0;
const DEFAULT_MANUAL_ADAPTIVE_NORMAL_COLLAPSING_MAX_PRICE_CENT: f64 = 62.0;
const DEFAULT_MANUAL_ADAPTIVE_NORMAL_COLLAPSING_SIZE_MULTIPLIER: f64 = 0.4;
const DEFAULT_MANUAL_ADAPTIVE_NORMAL_COLLAPSING_PTB_GAP_ADD_CENT: f64 = 15.0;
const DEFAULT_MANUAL_ADAPTIVE_ELEVATED_MAX_PRICE_CENT: f64 = 66.0;
const DEFAULT_MANUAL_ADAPTIVE_ELEVATED_SIZE_MULTIPLIER: f64 = 0.6;
const DEFAULT_MANUAL_ADAPTIVE_ELEVATED_PTB_GAP_ADD_CENT: f64 = 10.0;
const DEFAULT_MANUAL_ADAPTIVE_HIGH_MAX_PRICE_CENT: f64 = 58.0;
const DEFAULT_MANUAL_ADAPTIVE_HIGH_SIZE_MULTIPLIER: f64 = 0.3;
const DEFAULT_MANUAL_ADAPTIVE_HIGH_PTB_GAP_ADD_CENT: f64 = 25.0;
const DEFAULT_MANUAL_ADAPTIVE_AFTER_SL_MAX_PRICE_SUB_CENT: f64 = 5.0;
const DEFAULT_MANUAL_ADAPTIVE_AFTER_SL_PTB_GAP_ADD_CENT: f64 = 15.0;
const DEFAULT_MANUAL_ADAPTIVE_SL_COOLDOWN_MARKETS: usize = 3;
const DEFAULT_MANUAL_ADAPTIVE_PAIR_BUFFER_CENT: f64 = 1.0;
const FLOW_NODE_STATE_MANUAL_ADAPTIVE_LAST_GAP_PREFIX: &str =
    "manual_adaptive_risk_last_gap";
const FLOW_NODE_STATE_MANUAL_ADAPTIVE_COOLDOWN_SCOPE_SIDE: &str =
    "manual_adaptive_risk_cooldown_scope_side";
const FLOW_NODE_STATE_MANUAL_ADAPTIVE_COOLDOWN_REMAINING: &str =
    "manual_adaptive_risk_cooldown_remaining";
const FLOW_NODE_STATE_MANUAL_ADAPTIVE_COOLDOWN_MARKET: &str =
    "manual_adaptive_risk_cooldown_market";

#[derive(Debug, Clone, Copy)]
struct PairLockManualAdaptiveRiskConfig {
    window_start_sec: Option<i64>,
    window_end_sec: Option<i64>,
    volume_normal_lt: f64,
    volume_elevated_lt: f64,
    volume_high_lt: f64,
    trend_delta_usd: f64,
    normal_flat_max_price_sub_cent: f64,
    normal_flat_size_multiplier: f64,
    normal_flat_ptb_gap_add_cent: f64,
    normal_collapsing_max_price_cent: f64,
    normal_collapsing_size_multiplier: f64,
    normal_collapsing_ptb_gap_add_cent: f64,
    elevated_max_price_cent: f64,
    elevated_size_multiplier: f64,
    elevated_ptb_gap_add_cent: f64,
    high_max_price_cent: f64,
    high_size_multiplier: f64,
    high_ptb_gap_add_cent: f64,
    after_sl_max_price_sub_cent: f64,
    after_sl_ptb_gap_add_cent: f64,
    sl_cooldown_markets: usize,
    pair_buffer_cent: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PairLockManualAdaptiveTiming {
    configured_window_start_sec: i64,
    configured_window_end_sec: i64,
    cycle_window_start_sec: Option<i64>,
    cycle_window_end_sec: Option<i64>,
    effective_window_start_sec: i64,
    effective_window_end_sec: i64,
    market_elapsed_s: Option<i64>,
    in_window: bool,
}

#[derive(Debug, Clone)]
struct PairLockManualAdaptiveVolumeContext {
    regime: &'static str,
    ratio: Option<f64>,
    recent_notional_30s: Option<f64>,
    baseline_notional_30s: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct PairLockManualAdaptiveCooldown {
    active: bool,
    remaining_before: usize,
    remaining_after: usize,
}

#[derive(Debug, Clone)]
struct PairLockManualAdaptiveDecisionInput {
    config: PairLockManualAdaptiveRiskConfig,
    base_max_price: Option<f64>,
    base_size_usdc: f64,
    base_reenter_on_sl_hit: bool,
    base_counter_max_price: Option<f64>,
    counter_floor_price: Option<f64>,
    ask: Option<f64>,
    primary_estimated_avg_fill: Option<f64>,
    counter_estimated_avg_fill: Option<f64>,
    pair_max_total_price: f64,
    base_decision_passed: bool,
    base_reason_code: String,
    ptb_passed: bool,
    ptb_directional_gap: Option<f64>,
    ptb_threshold_usd: Option<f64>,
    ptb_threshold_value: Option<f64>,
    ptb_threshold_unit: Option<String>,
    volume: PairLockManualAdaptiveVolumeContext,
    ptb_trend: &'static str,
    timing: PairLockManualAdaptiveTiming,
    cooldown: PairLockManualAdaptiveCooldown,
}

#[derive(Debug, Clone)]
struct PairLockManualAdaptiveRiskDecision {
    applied: bool,
    decision: &'static str,
    reason: &'static str,
    effective_max_price: Option<f64>,
    effective_size_usdc: Option<f64>,
    effective_ptb_threshold_value: Option<f64>,
    effective_ptb_threshold_unit: Option<String>,
    counter_max_price: Option<f64>,
    diagnostics: Value,
}

#[derive(Debug, Clone)]
struct PairLockManualAdaptiveRiskOverride {
    effective_max_price: f64,
    effective_size_usdc: f64,
    effective_ptb_threshold_value: Option<f64>,
    effective_ptb_threshold_unit: Option<String>,
    counter_max_price: Option<f64>,
    diagnostics: Value,
}

fn action_place_order_uses_manual_adaptive_risk_strategy(node: &TradeFlowNode) -> bool {
    matches!(
        resolve_action_place_order_pair_lock_strategy(node),
        Ok(PAIR_LOCK_STRATEGY_MANUAL_ADAPTIVE_RISK_V1)
    )
}

fn manual_adaptive_node_f64(node: &TradeFlowNode, key: &str, fallback: f64) -> f64 {
    node_config_f64(node, key).unwrap_or(fallback)
}

fn resolve_pair_lock_manual_adaptive_risk_config(
    node: &TradeFlowNode,
) -> Result<PairLockManualAdaptiveRiskConfig> {
    resolve_pair_lock_manual_adaptive_notify_config(node)?;
    anyhow::ensure!(
        node_config_bool(node, "priceToBeatGuardEnabled").unwrap_or(false),
        "action.place_order manual_adaptive_risk_v1 requires priceToBeatGuardEnabled=true"
    );
    let ptb_mode = node_config_string(node, "priceToBeatMode")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    anyhow::ensure!(
        ptb_mode == "manual",
        "action.place_order manual_adaptive_risk_v1 requires priceToBeatMode=manual"
    );
    let sl_cooldown_markets = node_config_i64(node, "manualAdaptiveSlCooldownMarkets");
    anyhow::ensure!(
        sl_cooldown_markets.is_none_or(|value| value >= 0),
        "action.place_order manualAdaptiveSlCooldownMarkets must be >= 0"
    );
    let config = PairLockManualAdaptiveRiskConfig {
        window_start_sec: node_config_i64(node, "manualAdaptiveWindowStartSec"),
        window_end_sec: node_config_i64(node, "manualAdaptiveWindowEndSec"),
        volume_normal_lt: manual_adaptive_node_f64(
            node,
            "manualAdaptiveVolumeNormalLt",
            DEFAULT_MANUAL_ADAPTIVE_VOLUME_NORMAL_LT,
        ),
        volume_elevated_lt: manual_adaptive_node_f64(
            node,
            "manualAdaptiveVolumeElevatedLt",
            DEFAULT_MANUAL_ADAPTIVE_VOLUME_ELEVATED_LT,
        ),
        volume_high_lt: manual_adaptive_node_f64(
            node,
            "manualAdaptiveVolumeHighLt",
            DEFAULT_MANUAL_ADAPTIVE_VOLUME_HIGH_LT,
        ),
        trend_delta_usd: manual_adaptive_node_f64(
            node,
            "manualAdaptiveTrendDeltaUsd",
            DEFAULT_MANUAL_ADAPTIVE_TREND_DELTA_USD,
        ),
        normal_flat_max_price_sub_cent: manual_adaptive_node_f64(
            node,
            "manualAdaptiveNormalFlatMaxPriceSubCent",
            DEFAULT_MANUAL_ADAPTIVE_NORMAL_FLAT_MAX_PRICE_SUB_CENT,
        ),
        normal_flat_size_multiplier: manual_adaptive_node_f64(
            node,
            "manualAdaptiveNormalFlatSizeMultiplier",
            DEFAULT_MANUAL_ADAPTIVE_NORMAL_FLAT_SIZE_MULTIPLIER,
        ),
        normal_flat_ptb_gap_add_cent: manual_adaptive_node_f64(
            node,
            "manualAdaptiveNormalFlatPtbGapAddCent",
            DEFAULT_MANUAL_ADAPTIVE_NORMAL_FLAT_PTB_GAP_ADD_CENT,
        ),
        normal_collapsing_max_price_cent: manual_adaptive_node_f64(
            node,
            "manualAdaptiveNormalCollapsingMaxPriceCent",
            DEFAULT_MANUAL_ADAPTIVE_NORMAL_COLLAPSING_MAX_PRICE_CENT,
        ),
        normal_collapsing_size_multiplier: manual_adaptive_node_f64(
            node,
            "manualAdaptiveNormalCollapsingSizeMultiplier",
            DEFAULT_MANUAL_ADAPTIVE_NORMAL_COLLAPSING_SIZE_MULTIPLIER,
        ),
        normal_collapsing_ptb_gap_add_cent: manual_adaptive_node_f64(
            node,
            "manualAdaptiveNormalCollapsingPtbGapAddCent",
            DEFAULT_MANUAL_ADAPTIVE_NORMAL_COLLAPSING_PTB_GAP_ADD_CENT,
        ),
        elevated_max_price_cent: manual_adaptive_node_f64(
            node,
            "manualAdaptiveElevatedMaxPriceCent",
            DEFAULT_MANUAL_ADAPTIVE_ELEVATED_MAX_PRICE_CENT,
        ),
        elevated_size_multiplier: manual_adaptive_node_f64(
            node,
            "manualAdaptiveElevatedSizeMultiplier",
            DEFAULT_MANUAL_ADAPTIVE_ELEVATED_SIZE_MULTIPLIER,
        ),
        elevated_ptb_gap_add_cent: manual_adaptive_node_f64(
            node,
            "manualAdaptiveElevatedPtbGapAddCent",
            DEFAULT_MANUAL_ADAPTIVE_ELEVATED_PTB_GAP_ADD_CENT,
        ),
        high_max_price_cent: manual_adaptive_node_f64(
            node,
            "manualAdaptiveHighMaxPriceCent",
            DEFAULT_MANUAL_ADAPTIVE_HIGH_MAX_PRICE_CENT,
        ),
        high_size_multiplier: manual_adaptive_node_f64(
            node,
            "manualAdaptiveHighSizeMultiplier",
            DEFAULT_MANUAL_ADAPTIVE_HIGH_SIZE_MULTIPLIER,
        ),
        high_ptb_gap_add_cent: manual_adaptive_node_f64(
            node,
            "manualAdaptiveHighPtbGapAddCent",
            DEFAULT_MANUAL_ADAPTIVE_HIGH_PTB_GAP_ADD_CENT,
        ),
        after_sl_max_price_sub_cent: manual_adaptive_node_f64(
            node,
            "manualAdaptiveAfterSlMaxPriceSubCent",
            DEFAULT_MANUAL_ADAPTIVE_AFTER_SL_MAX_PRICE_SUB_CENT,
        ),
        after_sl_ptb_gap_add_cent: manual_adaptive_node_f64(
            node,
            "manualAdaptiveAfterSlPtbGapAddCent",
            DEFAULT_MANUAL_ADAPTIVE_AFTER_SL_PTB_GAP_ADD_CENT,
        ),
        sl_cooldown_markets: sl_cooldown_markets
            .map(|value| value as usize)
            .unwrap_or(DEFAULT_MANUAL_ADAPTIVE_SL_COOLDOWN_MARKETS),
        pair_buffer_cent: manual_adaptive_node_f64(
            node,
            "manualAdaptivePairBufferCent",
            DEFAULT_MANUAL_ADAPTIVE_PAIR_BUFFER_CENT,
        ),
    };
    validate_pair_lock_manual_adaptive_risk_config(config)?;
    Ok(config)
}

fn validate_pair_lock_manual_adaptive_risk_config(
    config: PairLockManualAdaptiveRiskConfig,
) -> Result<()> {
    if let Some(start) = config.window_start_sec {
        anyhow::ensure!(
            (0..=300).contains(&start),
            "action.place_order manualAdaptiveWindowStartSec must be in [0, 300]"
        );
    }
    if let Some(end) = config.window_end_sec {
        anyhow::ensure!(
            (0..=300).contains(&end),
            "action.place_order manualAdaptiveWindowEndSec must be in [0, 300]"
        );
    }
    if let (Some(start), Some(end)) = (config.window_start_sec, config.window_end_sec) {
        anyhow::ensure!(
            start < end,
            "action.place_order manualAdaptiveWindowStartSec must be < manualAdaptiveWindowEndSec"
        );
    }
    anyhow::ensure!(
        config.volume_normal_lt.is_finite()
            && config.volume_elevated_lt.is_finite()
            && config.volume_high_lt.is_finite()
            && config.volume_normal_lt > 0.0
            && config.volume_normal_lt < config.volume_elevated_lt
            && config.volume_elevated_lt < config.volume_high_lt,
        "action.place_order manual adaptive volume thresholds must be positive and ascending"
    );
    anyhow::ensure!(
        config.trend_delta_usd.is_finite() && config.trend_delta_usd > 0.0,
        "action.place_order manualAdaptiveTrendDeltaUsd must be > 0"
    );
    for (name, value) in [
        ("manualAdaptiveNormalFlatSizeMultiplier", config.normal_flat_size_multiplier),
        (
            "manualAdaptiveNormalCollapsingSizeMultiplier",
            config.normal_collapsing_size_multiplier,
        ),
        ("manualAdaptiveElevatedSizeMultiplier", config.elevated_size_multiplier),
        ("manualAdaptiveHighSizeMultiplier", config.high_size_multiplier),
    ] {
        anyhow::ensure!(
            value.is_finite() && value > 0.0 && value <= 1.0,
            "action.place_order {name} must be in (0, 1]"
        );
    }
    for (name, value) in [
        (
            "manualAdaptiveNormalCollapsingMaxPriceCent",
            config.normal_collapsing_max_price_cent,
        ),
        ("manualAdaptiveElevatedMaxPriceCent", config.elevated_max_price_cent),
        ("manualAdaptiveHighMaxPriceCent", config.high_max_price_cent),
    ] {
        anyhow::ensure!(
            value.is_finite() && value > 0.0 && value < 100.0,
            "action.place_order {name} must be in (0, 100)"
        );
    }
    for (name, value) in [
        (
            "manualAdaptiveNormalFlatMaxPriceSubCent",
            config.normal_flat_max_price_sub_cent,
        ),
        (
            "manualAdaptiveNormalFlatPtbGapAddCent",
            config.normal_flat_ptb_gap_add_cent,
        ),
        (
            "manualAdaptiveNormalCollapsingPtbGapAddCent",
            config.normal_collapsing_ptb_gap_add_cent,
        ),
        ("manualAdaptiveElevatedPtbGapAddCent", config.elevated_ptb_gap_add_cent),
        ("manualAdaptiveHighPtbGapAddCent", config.high_ptb_gap_add_cent),
        (
            "manualAdaptiveAfterSlMaxPriceSubCent",
            config.after_sl_max_price_sub_cent,
        ),
        (
            "manualAdaptiveAfterSlPtbGapAddCent",
            config.after_sl_ptb_gap_add_cent,
        ),
        ("manualAdaptivePairBufferCent", config.pair_buffer_cent),
    ] {
        anyhow::ensure!(
            value.is_finite() && value >= 0.0,
            "action.place_order {name} must be >= 0"
        );
    }
    Ok(())
}

fn pair_lock_manual_adaptive_step_i64(step: &TradeFlowRunStep, key: &str) -> Option<i64> {
    step.input_json
        .as_ref()
        .and_then(|input| input.get(key))
        .and_then(value_as_i64)
}

fn pair_lock_manual_adaptive_cycle_window_from_step(
    step: &TradeFlowRunStep,
) -> (Option<i64>, Option<i64>) {
    (
        pair_lock_manual_adaptive_step_i64(step, "cycleWindowStartSec")
            .or_else(|| pair_lock_manual_adaptive_step_i64(step, "cycle_window_start_sec")),
        pair_lock_manual_adaptive_step_i64(step, "cycleWindowEndSec")
            .or_else(|| pair_lock_manual_adaptive_step_i64(step, "cycle_window_end_sec")),
    )
}

fn pair_lock_manual_adaptive_timing(
    config: PairLockManualAdaptiveRiskConfig,
    step: &TradeFlowRunStep,
    market_slug: &str,
) -> PairLockManualAdaptiveTiming {
    let (cycle_window_start_sec, cycle_window_end_sec) =
        pair_lock_manual_adaptive_cycle_window_from_step(step);
    let configured_window_start_sec = config
        .window_start_sec
        .or(cycle_window_start_sec)
        .unwrap_or(0);
    let configured_window_end_sec = config
        .window_end_sec
        .or(cycle_window_end_sec)
        .unwrap_or(300);
    let effective_window_start_sec = cycle_window_start_sec
        .map(|cycle_start| configured_window_start_sec.max(cycle_start))
        .unwrap_or(configured_window_start_sec);
    let effective_window_end_sec = cycle_window_end_sec
        .map(|cycle_end| configured_window_end_sec.min(cycle_end))
        .unwrap_or(configured_window_end_sec);
    let market_elapsed_s = pair_lock_adaptive_market_elapsed_s(market_slug);
    let in_window = market_elapsed_s.is_some_and(|elapsed| {
        elapsed >= effective_window_start_sec && elapsed <= effective_window_end_sec
    });
    PairLockManualAdaptiveTiming {
        configured_window_start_sec,
        configured_window_end_sec,
        cycle_window_start_sec,
        cycle_window_end_sec,
        effective_window_start_sec,
        effective_window_end_sec,
        market_elapsed_s,
        in_window,
    }
}

fn pair_lock_manual_adaptive_volume_regime(
    ratio: Option<f64>,
    config: PairLockManualAdaptiveRiskConfig,
) -> &'static str {
    let Some(ratio) = ratio.filter(|value| value.is_finite() && *value >= 0.0) else {
        return "normal";
    };
    if ratio < config.volume_normal_lt {
        "normal"
    } else if ratio < config.volume_elevated_lt {
        "elevated"
    } else if ratio < config.volume_high_lt {
        "high"
    } else {
        "extreme"
    }
}

async fn resolve_pair_lock_manual_adaptive_volume_context(
    repo: &PostgresRepository,
    market_slug: &str,
    config: PairLockManualAdaptiveRiskConfig,
) -> PairLockManualAdaptiveVolumeContext {
    let now = Utc::now();
    let summary = repo.market_trade_volume_summary(market_slug, now).await.ok();
    let baseline = if let Some(scope) = find_updown_scope_by_slug(market_slug) {
        repo.market_trade_volume_bucket_median(
            scope.asset,
            30.0,
            0.0,
            DECISION_LOG_VOLUME_LOOKBACK_DAYS,
            DECISION_LOG_VOLUME_WINDOW_SEC,
            market_slug,
            now,
        )
        .await
        .ok()
    } else {
        None
    };
    let baseline_notional_30s = baseline
        .filter(|median| {
            median.sample_count >= DECISION_LOG_VOLUME_BASELINE_MIN_SAMPLES
                && median.median_volume_usdc.is_finite()
                && median.median_volume_usdc > 0.0
        })
        .map(|median| median.median_volume_usdc);
    let recent_notional_30s = summary.as_ref().map(|summary| summary.volume_30s);
    let ratio = recent_notional_30s.zip(baseline_notional_30s).map(|(recent, baseline)| {
        if baseline > 0.0 {
            recent / baseline
        } else {
            0.0
        }
    });
    PairLockManualAdaptiveVolumeContext {
        regime: pair_lock_manual_adaptive_volume_regime(ratio, config),
        ratio,
        recent_notional_30s,
        baseline_notional_30s,
    }
}

fn pair_lock_manual_adaptive_state_side(outcome_label: &str) -> String {
    normalize_pair_lock_binary_outcome(outcome_label)
        .unwrap_or(outcome_label)
        .to_ascii_lowercase()
}

fn pair_lock_manual_adaptive_scope_side(market_slug: &str, outcome_label: &str) -> String {
    let scope = find_updown_scope_by_slug(market_slug)
        .map(|scope| scope.scope.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    format!(
        "{}:{}",
        scope,
        pair_lock_manual_adaptive_state_side(outcome_label).to_ascii_uppercase()
    )
}

fn pair_lock_manual_adaptive_gap_state_key(
    outcome_label: &str,
    suffix: &str,
) -> String {
    format!(
        "{}_{}_{}",
        FLOW_NODE_STATE_MANUAL_ADAPTIVE_LAST_GAP_PREFIX,
        pair_lock_manual_adaptive_state_side(outcome_label),
        suffix
    )
}

fn pair_lock_manual_adaptive_ptb_trend(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
    outcome_label: &str,
    directional_gap: Option<f64>,
    trend_delta_usd: f64,
) -> &'static str {
    let Some(current_gap) = directional_gap.filter(|value| value.is_finite()) else {
        return "flat";
    };
    let market_key = pair_lock_manual_adaptive_gap_state_key(outcome_label, "market");
    let gap_key = pair_lock_manual_adaptive_gap_state_key(outcome_label, "value");
    let previous_market = flow_node_state_string(context, node_key, &market_key);
    let previous_gap = flow_node_state(context, node_key, &gap_key).and_then(value_as_f64);
    set_flow_node_state(context, node_key, &market_key, json!(market_slug));
    set_flow_node_state(context, node_key, &gap_key, json!(current_gap));
    if previous_market.as_deref() != Some(market_slug) {
        return "flat";
    }
    match previous_gap.map(|previous| current_gap - previous) {
        Some(delta) if delta >= trend_delta_usd => "expanding",
        Some(delta) if delta <= -trend_delta_usd => "collapsing",
        _ => "flat",
    }
}

fn pair_lock_manual_adaptive_cooldown_state(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
    scope_side: &str,
) -> PairLockManualAdaptiveCooldown {
    let stored_scope = flow_node_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_MANUAL_ADAPTIVE_COOLDOWN_SCOPE_SIDE,
    );
    if stored_scope.as_deref() != Some(scope_side) {
        return PairLockManualAdaptiveCooldown {
            active: false,
            remaining_before: 0,
            remaining_after: 0,
        };
    }
    let remaining_before = flow_node_state_i64(
        context,
        node_key,
        FLOW_NODE_STATE_MANUAL_ADAPTIVE_COOLDOWN_REMAINING,
    )
    .unwrap_or(0)
    .max(0) as usize;
    if remaining_before == 0 {
        return PairLockManualAdaptiveCooldown {
            active: false,
            remaining_before: 0,
            remaining_after: 0,
        };
    }
    let previous_market = flow_node_state_string(
        context,
        node_key,
        FLOW_NODE_STATE_MANUAL_ADAPTIVE_COOLDOWN_MARKET,
    );
    let remaining_after = if previous_market.as_deref() == Some(market_slug) {
        remaining_before
    } else {
        remaining_before.saturating_sub(1)
    };
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_MANUAL_ADAPTIVE_COOLDOWN_MARKET,
        json!(market_slug),
    );
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_MANUAL_ADAPTIVE_COOLDOWN_REMAINING,
        json!(remaining_after as i64),
    );
    PairLockManualAdaptiveCooldown {
        active: true,
        remaining_before,
        remaining_after,
    }
}

fn mark_pair_lock_manual_adaptive_sl_cooldown(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
    outcome_label: &str,
    cooldown_markets: usize,
) {
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_MANUAL_ADAPTIVE_COOLDOWN_SCOPE_SIDE,
        json!(pair_lock_manual_adaptive_scope_side(market_slug, outcome_label)),
    );
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_MANUAL_ADAPTIVE_COOLDOWN_MARKET,
        json!(market_slug),
    );
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_MANUAL_ADAPTIVE_COOLDOWN_REMAINING,
        json!(cooldown_markets as i64),
    );
}

fn pair_lock_manual_adaptive_cent(value: Option<f64>) -> Option<f64> {
    let value = value?;
    if value.is_finite() && value > 0.0 {
        Some(pair_lock_manual_adaptive_round_payload_number(
            if value <= 1.0 { value * 100.0 } else { value },
        ))
    } else {
        None
    }
}

fn pair_lock_manual_adaptive_round_payload_number(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn pair_lock_manual_adaptive_probability(value: Option<f64>) -> Option<f64> {
    let value = value?;
    if value.is_finite() && value > 0.0 {
        Some(if value > 1.0 { value / 100.0 } else { value })
    } else {
        None
    }
    .filter(|value| *value > 0.0 && *value < 1.0)
}

fn pair_lock_manual_adaptive_effective_ptb_value(
    base_value: Option<f64>,
    base_unit: Option<&str>,
    add_cent: f64,
) -> (Option<f64>, Option<String>) {
    let value = base_value.filter(|value| value.is_finite() && *value > 0.0);
    let unit = base_unit.unwrap_or("usd").trim().to_ascii_lowercase();
    let add_value = if unit == "cent" {
        add_cent
    } else {
        add_cent / 100.0
    };
    (
        value.map(|value| value + add_value.max(0.0)),
        Some(if unit == "cent" { "cent" } else { "usd" }.to_string()),
    )
}

fn pair_lock_manual_adaptive_diagnostics(
    input: &PairLockManualAdaptiveDecisionInput,
    decision: &'static str,
    reason: &'static str,
    applied: bool,
    effective_max_price: Option<f64>,
    effective_size_usdc: Option<f64>,
    effective_ptb_threshold_value: Option<f64>,
    effective_ptb_threshold_unit: Option<&str>,
    counter_max_price: Option<f64>,
    ptb_gap_add_cent: f64,
) -> Value {
    let base_counter_max_cent = pair_lock_manual_adaptive_cent(input.base_counter_max_price);
    let effective_counter_max_cent = pair_lock_manual_adaptive_cent(counter_max_price);
    let counter_delta_cent = base_counter_max_cent
        .zip(effective_counter_max_cent)
        .map(|(base, effective)| pair_lock_manual_adaptive_round_payload_number((base - effective).max(0.0)));
    let counter_cap_applied = counter_delta_cent.is_some_and(|delta| delta > 0.0);
    json!({
        "enabled": true,
        "strategy": PAIR_LOCK_STRATEGY_MANUAL_ADAPTIVE_RISK_V1,
        "decision": decision,
        "reason": reason,
        "applied": applied,
        "volume_regime": input.volume.regime,
        "volume_ratio": input.volume.ratio,
        "recent_notional_30s": input.volume.recent_notional_30s,
        "baseline_notional_30s": input.volume.baseline_notional_30s,
        "ptb_trend": input.ptb_trend,
        "recent_sl": input.cooldown.active,
        "cooldown": {
            "active": input.cooldown.active,
            "remaining_before": input.cooldown.remaining_before,
            "remaining_after": input.cooldown.remaining_after,
            "configured_markets": input.config.sl_cooldown_markets,
        },
        "timing": {
            "market_elapsed_s": input.timing.market_elapsed_s,
            "configured_window_start_s": input.timing.configured_window_start_sec,
            "configured_window_end_s": input.timing.configured_window_end_sec,
            "cycle_window_start_s": input.timing.cycle_window_start_sec,
            "cycle_window_end_s": input.timing.cycle_window_end_sec,
            "effective_window_start_s": input.timing.effective_window_start_sec,
            "effective_window_end_s": input.timing.effective_window_end_sec,
            "in_window": input.timing.in_window,
        },
        "base": {
            "max_price_cent": pair_lock_manual_adaptive_cent(input.base_max_price),
            "size_usdc": input.base_size_usdc,
            "reenter_on_sl_hit": input.base_reenter_on_sl_hit,
            "ptb_threshold_value": input.ptb_threshold_value,
            "ptb_threshold_unit": input.ptb_threshold_unit,
            "ptb_threshold_usd": input.ptb_threshold_usd,
            "decision_passed": input.base_decision_passed,
            "reason_code": input.base_reason_code,
        },
        "effective": {
            "max_price_cent": pair_lock_manual_adaptive_cent(effective_max_price),
            "size_usdc": effective_size_usdc,
            "ptb_threshold_value": effective_ptb_threshold_value,
            "ptb_threshold_unit": effective_ptb_threshold_unit,
            "ptb_gap_add_cent": ptb_gap_add_cent,
            "counter_max_price_cent": pair_lock_manual_adaptive_cent(counter_max_price),
            "reentry_allowed": input.base_reenter_on_sl_hit && !applied,
            "reenter_on_sl_hit": input.base_reenter_on_sl_hit && !applied,
        },
        "market": {
            "ask_cent": pair_lock_manual_adaptive_cent(input.ask),
            "primary_estimated_avg_fill_cent": pair_lock_manual_adaptive_cent(input.primary_estimated_avg_fill),
            "counter_estimated_avg_fill_cent": pair_lock_manual_adaptive_cent(input.counter_estimated_avg_fill),
            "pair_max_total_cent": pair_lock_manual_adaptive_cent(Some(input.pair_max_total_price)),
            "pair_buffer_cent": input.config.pair_buffer_cent,
            "ptb_passed": input.ptb_passed,
            "directional_gap": input.ptb_directional_gap,
        },
        "counter_dynamic_cap": {
            "applied": counter_cap_applied,
            "base_counter_max_cent": base_counter_max_cent,
            "effective_counter_max_cent": effective_counter_max_cent,
            "primary_estimated_avg_fill_cent": pair_lock_manual_adaptive_cent(input.primary_estimated_avg_fill),
            "counter_estimated_avg_fill_cent": pair_lock_manual_adaptive_cent(input.counter_estimated_avg_fill),
            "counter_floor_cent": pair_lock_manual_adaptive_cent(input.counter_floor_price),
            "pair_max_total_cent": pair_lock_manual_adaptive_cent(Some(input.pair_max_total_price)),
            "pair_buffer_cent": input.config.pair_buffer_cent,
            "delta_cent": counter_delta_cent,
        }
    })
}

fn pair_lock_manual_adaptive_block(
    input: &PairLockManualAdaptiveDecisionInput,
    reason: &'static str,
    effective_max_price: Option<f64>,
    effective_size_usdc: Option<f64>,
    effective_ptb_threshold_value: Option<f64>,
    effective_ptb_threshold_unit: Option<&str>,
    counter_max_price: Option<f64>,
    ptb_gap_add_cent: f64,
) -> PairLockManualAdaptiveRiskDecision {
    PairLockManualAdaptiveRiskDecision {
        applied: true,
        decision: "BLOCK",
        reason,
        effective_max_price,
        effective_size_usdc,
        effective_ptb_threshold_value,
        effective_ptb_threshold_unit: effective_ptb_threshold_unit.map(str::to_string),
        counter_max_price,
        diagnostics: pair_lock_manual_adaptive_diagnostics(
            input,
            "BLOCK",
            reason,
            true,
            effective_max_price,
            effective_size_usdc,
            effective_ptb_threshold_value,
            effective_ptb_threshold_unit,
            counter_max_price,
            ptb_gap_add_cent,
        ),
    }
}

fn pair_lock_manual_adaptive_noop(
    input: &PairLockManualAdaptiveDecisionInput,
    reason: &'static str,
) -> PairLockManualAdaptiveRiskDecision {
    PairLockManualAdaptiveRiskDecision {
        applied: false,
        decision: "BASE",
        reason,
        effective_max_price: input.base_max_price,
        effective_size_usdc: Some(input.base_size_usdc),
        effective_ptb_threshold_value: input.ptb_threshold_value,
        effective_ptb_threshold_unit: input.ptb_threshold_unit.clone(),
        counter_max_price: None,
        diagnostics: pair_lock_manual_adaptive_diagnostics(
            input,
            "BASE",
            reason,
            false,
            input.base_max_price,
            Some(input.base_size_usdc),
            input.ptb_threshold_value,
            input.ptb_threshold_unit.as_deref(),
            None,
            0.0,
        ),
    }
}

fn evaluate_pair_lock_manual_adaptive_risk_decision(
    input: PairLockManualAdaptiveDecisionInput,
) -> PairLockManualAdaptiveRiskDecision {
    if !input.timing.in_window {
        return pair_lock_manual_adaptive_noop(&input, "outside_manual_adaptive_window");
    }
    if !input.base_decision_passed {
        return pair_lock_manual_adaptive_noop(&input, "base_candidate_not_passed");
    }
    let base_max_price = match input.base_max_price {
        Some(value) if value.is_finite() && value > 0.0 => value,
        _ => return pair_lock_manual_adaptive_noop(&input, "base_max_price_unavailable"),
    };
    let mut max_price_cent = base_max_price * 100.0;
    let mut size_multiplier = 1.0;
    let mut ptb_gap_add_cent = 0.0;
    let mut reason = "base_normal_expanding";
    let mut block_reason: Option<&'static str> = None;

    match (input.volume.regime, input.ptb_trend) {
        ("extreme", _) => block_reason = Some("extreme_volume"),
        ("high", "collapsing") => block_reason = Some("high_volume_gap_collapsing"),
        ("elevated", "collapsing") => block_reason = Some("elevated_volume_gap_collapsing"),
        ("normal", "collapsing") => {
            max_price_cent = max_price_cent.min(input.config.normal_collapsing_max_price_cent);
            size_multiplier = input.config.normal_collapsing_size_multiplier;
            ptb_gap_add_cent = input.config.normal_collapsing_ptb_gap_add_cent;
            reason = "normal_collapsing_strict";
        }
        ("normal", "flat") => {
            max_price_cent =
                (max_price_cent - input.config.normal_flat_max_price_sub_cent).max(1.0);
            size_multiplier = input.config.normal_flat_size_multiplier;
            ptb_gap_add_cent = input.config.normal_flat_ptb_gap_add_cent;
            reason = "normal_flat_strict";
        }
        ("elevated", _) => {
            max_price_cent = max_price_cent.min(input.config.elevated_max_price_cent);
            size_multiplier = input.config.elevated_size_multiplier;
            ptb_gap_add_cent = input.config.elevated_ptb_gap_add_cent;
            reason = "elevated_volume_strict";
        }
        ("high", _) => {
            max_price_cent = max_price_cent.min(input.config.high_max_price_cent);
            size_multiplier = input.config.high_size_multiplier;
            ptb_gap_add_cent = input.config.high_ptb_gap_add_cent;
            reason = "high_volume_strict";
        }
        _ => {}
    }

    if input.cooldown.active {
        max_price_cent = (max_price_cent - input.config.after_sl_max_price_sub_cent).max(1.0);
        ptb_gap_add_cent += input.config.after_sl_ptb_gap_add_cent;
        size_multiplier = size_multiplier.min(input.config.high_size_multiplier);
        reason = "recent_sl_strict_mode";
    }

    let effective_max_price = (max_price_cent / 100.0).min(base_max_price);
    let effective_size_usdc = (input.base_size_usdc * size_multiplier).max(0.0);
    let (effective_ptb_threshold_value, effective_ptb_threshold_unit) =
        pair_lock_manual_adaptive_effective_ptb_value(
            input.ptb_threshold_value,
            input.ptb_threshold_unit.as_deref(),
            ptb_gap_add_cent,
        );
    let strict_ptb_threshold_usd = input
        .ptb_threshold_usd
        .map(|threshold| threshold + ptb_gap_add_cent / 100.0);
    let primary_estimated_avg_fill = input
        .primary_estimated_avg_fill
        .or(input.ask)
        .filter(|value| value.is_finite() && *value > 0.0);
    let counter_estimated_avg_fill = input
        .counter_estimated_avg_fill
        .filter(|value| value.is_finite() && *value > 0.0);
    let counter_max_price = primary_estimated_avg_fill.map(|primary_fill| {
        (input.pair_max_total_price - primary_fill - input.config.pair_buffer_cent / 100.0)
            .max(0.0)
    });

    if let Some(block_reason) = block_reason {
        return pair_lock_manual_adaptive_block(
            &input,
            block_reason,
            Some(effective_max_price),
            Some(0.0),
            effective_ptb_threshold_value,
            effective_ptb_threshold_unit.as_deref(),
            counter_max_price,
            ptb_gap_add_cent,
        );
    }
    if let Some(fill) = primary_estimated_avg_fill {
        if fill > effective_max_price {
            return pair_lock_manual_adaptive_block(
                &input,
                "manual_adaptive_max_price_block",
                Some(effective_max_price),
                Some(effective_size_usdc),
                effective_ptb_threshold_value,
                effective_ptb_threshold_unit.as_deref(),
                counter_max_price,
                ptb_gap_add_cent,
            );
        }
    }
    if let (Some(gap), Some(threshold)) = (input.ptb_directional_gap, strict_ptb_threshold_usd) {
        if gap < threshold {
            return pair_lock_manual_adaptive_block(
                &input,
                "manual_adaptive_ptb_gap_below_strict_threshold",
                Some(effective_max_price),
                Some(effective_size_usdc),
                effective_ptb_threshold_value,
                effective_ptb_threshold_unit.as_deref(),
                counter_max_price,
                ptb_gap_add_cent,
            );
        }
    }
    if let (Some(primary), Some(counter)) = (primary_estimated_avg_fill, counter_estimated_avg_fill)
    {
        if primary + counter + input.config.pair_buffer_cent / 100.0 > input.pair_max_total_price {
            return pair_lock_manual_adaptive_block(
                &input,
                "manual_adaptive_pair_cap_block",
                Some(effective_max_price),
                Some(effective_size_usdc),
                effective_ptb_threshold_value,
                effective_ptb_threshold_unit.as_deref(),
                counter_max_price,
                ptb_gap_add_cent,
            );
        }
    }

    let applied = reason != "base_normal_expanding";
    PairLockManualAdaptiveRiskDecision {
        applied,
        decision: if applied { "ALLOW_STRICT" } else { "BASE" },
        reason,
        effective_max_price: Some(effective_max_price),
        effective_size_usdc: Some(effective_size_usdc),
        effective_ptb_threshold_value,
        effective_ptb_threshold_unit: effective_ptb_threshold_unit.clone(),
        counter_max_price,
        diagnostics: pair_lock_manual_adaptive_diagnostics(
            &input,
            if applied { "ALLOW_STRICT" } else { "BASE" },
            reason,
            applied,
            Some(effective_max_price),
            Some(effective_size_usdc),
            effective_ptb_threshold_value,
            effective_ptb_threshold_unit.as_deref(),
            counter_max_price,
            ptb_gap_add_cent,
        ),
    }
}

async fn resolve_pair_lock_manual_adaptive_risk_decision_for_candidate(
    repo: &PostgresRepository,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    context: &mut Value,
    market_slug: &str,
    pair_lock: &ActionPlaceOrderPairLockConfig,
    candidate: &ActionPlaceOrderPairLockPrimaryCandidateEval,
    counter: &ActionPlaceOrderPairLockPrimaryCandidateEval,
) -> Result<PairLockManualAdaptiveRiskDecision> {
    let config = resolve_pair_lock_manual_adaptive_risk_config(node)?;
    let ptb_guard = candidate
        .diagnostics
        .get("price_to_beat_guard")
        .unwrap_or(&Value::Null);
    let directional_gap = ptb_guard.get("directional_gap").and_then(value_as_f64);
    let ptb_trend = pair_lock_manual_adaptive_ptb_trend(
        context,
        &node.key,
        market_slug,
        &candidate.outcome_label,
        directional_gap,
        config.trend_delta_usd,
    );
    let scope_side =
        pair_lock_manual_adaptive_scope_side(market_slug, &candidate.outcome_label);
    let cooldown =
        pair_lock_manual_adaptive_cooldown_state(context, &node.key, market_slug, &scope_side);
    let timing = pair_lock_manual_adaptive_timing(config, step, market_slug);
    let volume =
        resolve_pair_lock_manual_adaptive_volume_context(repo, market_slug, config).await;
    let base_max_price = candidate
        .diagnostics
        .pointer("/max_price_guard/details/max_price")
        .and_then(value_as_f64)
        .and_then(|value| pair_lock_manual_adaptive_probability(Some(value)))
        .or_else(|| {
            node_config_f64(node, "maxPriceCent")
                .and_then(|value| pair_lock_manual_adaptive_probability(Some(value)))
        });
    let ask = candidate
        .quote
        .best_ask
        .or(Some(candidate.quote.current_price))
        .and_then(|value| pair_lock_manual_adaptive_probability(Some(value)));
    let counter_estimated_avg_fill = counter
        .quote
        .best_ask
        .or(Some(counter.quote.current_price))
        .and_then(|value| pair_lock_manual_adaptive_probability(Some(value)));
    let base_counter_max_price = node_config_f64(node, "counterLegMaxPriceCent")
        .and_then(|value| pair_lock_manual_adaptive_probability(Some(value)));
    let counter_floor_price = node_config_f64(node, "counterLegExecutionFloorPriceCent")
        .and_then(|value| pair_lock_manual_adaptive_probability(Some(value)));
    Ok(evaluate_pair_lock_manual_adaptive_risk_decision(
        PairLockManualAdaptiveDecisionInput {
            config,
            base_max_price,
            base_size_usdc: pair_lock.primary_leg_size_usdc,
            base_reenter_on_sl_hit: node_config_bool(node, "reenterOnSlHit").unwrap_or(false),
            base_counter_max_price,
            counter_floor_price,
            ask,
            primary_estimated_avg_fill: ask,
            counter_estimated_avg_fill,
            pair_max_total_price: pair_lock.max_total_price,
            base_decision_passed: candidate.decision == "passed",
            base_reason_code: candidate.reason_code.clone(),
            ptb_passed: ptb_guard
                .get("passed")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            ptb_directional_gap: directional_gap,
            ptb_threshold_usd: ptb_guard.get("threshold_usd").and_then(value_as_f64),
            ptb_threshold_value: ptb_guard.get("threshold_value").and_then(value_as_f64),
            ptb_threshold_unit: ptb_guard
                .get("threshold_unit")
                .and_then(Value::as_str)
                .map(str::to_string),
            volume,
            ptb_trend,
            timing,
            cooldown,
        },
    ))
}

async fn maybe_apply_pair_lock_manual_adaptive_risk_candidate_override(
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
    if !action_place_order_uses_manual_adaptive_risk_strategy(node) {
        return Ok(());
    }
    let decision = resolve_pair_lock_manual_adaptive_risk_decision_for_candidate(
        repo, step, node, context, market_slug, pair_lock, candidate, counter,
    )
    .await?;
    if let Some(obj) = candidate.diagnostics.as_object_mut() {
        obj.insert(
            "manual_adaptive_risk".to_string(),
            decision.diagnostics.clone(),
        );
        obj.insert(
            "manual_adaptive_risk_decision".to_string(),
            json!(decision.decision),
        );
        obj.insert(
            "manual_adaptive_risk_reason".to_string(),
            json!(decision.reason),
        );
    }
    maybe_notify_pair_lock_manual_adaptive_risk_decision(
        repo,
        run,
        node,
        context,
        market_slug,
        &candidate.outcome_label,
        &decision.diagnostics,
    )
    .await?;
    if decision.decision == "BLOCK" {
        candidate.decision = "blocked";
        candidate.reason_code = decision.reason.to_string();
        if let Some(obj) = candidate.diagnostics.as_object_mut() {
            obj.insert("passed".to_string(), json!(false));
            obj.insert("decision".to_string(), json!("blocked"));
            obj.insert("reason_code".to_string(), json!(decision.reason));
        }
        return Ok(());
    }
    if decision.applied {
        let override_result = PairLockManualAdaptiveRiskOverride {
            effective_max_price: decision
                .effective_max_price
                .expect("manual adaptive applied requires effective max price"),
            effective_size_usdc: decision
                .effective_size_usdc
                .expect("manual adaptive applied requires effective size"),
            effective_ptb_threshold_value: decision.effective_ptb_threshold_value,
            effective_ptb_threshold_unit: decision.effective_ptb_threshold_unit,
            counter_max_price: decision.counter_max_price,
            diagnostics: decision.diagnostics,
        };
        candidate.manual_adaptive_risk_override = Some(override_result);
    }
    Ok(())
}
