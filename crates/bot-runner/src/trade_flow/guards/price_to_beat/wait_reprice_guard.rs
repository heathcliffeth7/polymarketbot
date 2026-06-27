use super::PriceToBeatGuardEvaluation;
use serde_json::{json, Value};

const REASON_WAIT_SIGNAL_EXPIRED: &str = "blocked_wait_signal_expired";
const REASON_INITIAL_ASK_TOO_FAR: &str = "blocked_initial_ask_too_far_above_cap";
const REASON_FALLING_INTO_CAP: &str = "blocked_falling_into_cap_after_wait";
const REASON_LATE_EXPENSIVE_ENTRY: &str = "blocked_late_expensive_entry";
const REASON_RECHECK_LOW_QUALITY_EDGE: &str = "recheck_low_quality_edge";
const REASON_RECHECK_MEDIUM_CHOP_EXPENSIVE_ENTRY: &str = "recheck_medium_chop_expensive_entry";
const REASON_LOW_QUALITY_EDGE_RECHECK_FAILED: &str = "blocked_low_quality_edge_recheck_failed";

#[derive(Debug, Clone, Copy)]
pub(super) struct PriceToBeatWaitRepriceGuardConfig {
    enabled: bool,
    max_age_ms_early: i64,
    max_age_ms_mid: i64,
    max_age_ms_late: i64,
    initial_ask_max_over_cap_cent: f64,
    falling_into_cap_enabled: bool,
    falling_drop_cent_early: f64,
    falling_drop_cent_mid: f64,
    falling_drop_cent_late: f64,
    late_expensive_enabled: bool,
    late_expensive_seconds: f64,
    late_expensive_vwap_cent: f64,
    late_expensive_min_q_cent: f64,
    late_expensive_min_gap_strength_extra: f64,
    low_quality_edge_recheck_enabled: bool,
    low_quality_gap_margin: f64,
    low_quality_adjusted_margin_cent: f64,
}

impl PriceToBeatWaitRepriceGuardConfig {
    pub(super) fn from_node(node: &crate::TradeFlowNode) -> Self {
        Self {
            enabled: node_config_bool(node, "priceToBeatIvWaitRepriceGuardEnabled")
                .unwrap_or(false),
            max_age_ms_early: node_config_i64(node, "priceToBeatIvWaitMaxAgeMsEarly")
                .filter(|value| *value > 0)
                .unwrap_or(8_000),
            max_age_ms_mid: node_config_i64(node, "priceToBeatIvWaitMaxAgeMsMid")
                .filter(|value| *value > 0)
                .unwrap_or(5_000),
            max_age_ms_late: node_config_i64(node, "priceToBeatIvWaitMaxAgeMsLate")
                .filter(|value| *value > 0)
                .unwrap_or(3_000),
            initial_ask_max_over_cap_cent: non_negative_f64(
                node,
                "priceToBeatIvWaitInitialAskMaxOverCapCent",
            )
            .unwrap_or(10.0),
            falling_into_cap_enabled: node_config_bool(
                node,
                "priceToBeatIvFallingIntoCapGuardEnabled",
            )
            .unwrap_or(true),
            falling_drop_cent_early: non_negative_f64(
                node,
                "priceToBeatIvFallingIntoCapDropCentEarly",
            )
            .unwrap_or(15.0),
            falling_drop_cent_mid: non_negative_f64(node, "priceToBeatIvFallingIntoCapDropCentMid")
                .unwrap_or(12.0),
            falling_drop_cent_late: non_negative_f64(
                node,
                "priceToBeatIvFallingIntoCapDropCentLate",
            )
            .unwrap_or(8.0),
            late_expensive_enabled: node_config_bool(
                node,
                "priceToBeatIvLateExpensiveEntryGuardEnabled",
            )
            .unwrap_or(true),
            late_expensive_seconds: non_negative_f64(node, "priceToBeatIvLateExpensiveSeconds")
                .unwrap_or(45.0),
            late_expensive_vwap_cent: non_negative_f64(node, "priceToBeatIvLateExpensiveVwapCent")
                .unwrap_or(70.0),
            late_expensive_min_q_cent: non_negative_f64(node, "priceToBeatIvLateExpensiveMinQCent")
                .unwrap_or(92.0),
            late_expensive_min_gap_strength_extra: non_negative_f64(
                node,
                "priceToBeatIvLateExpensiveMinGapStrengthExtra",
            )
            .unwrap_or(0.50),
            low_quality_edge_recheck_enabled: node_config_bool(
                node,
                "priceToBeatIvLowQualityEdgeRecheckEnabled",
            )
            .unwrap_or(false),
            low_quality_gap_margin: non_negative_f64(node, "priceToBeatIvLowQualityGapMargin")
                .unwrap_or(0.10),
            low_quality_adjusted_margin_cent: non_negative_f64(
                node,
                "priceToBeatIvLowQualityEdgeMarginCent",
            )
            .unwrap_or(5.0),
        }
    }

    fn max_age_ms(self, seconds_left: Option<f64>) -> i64 {
        match seconds_left {
            Some(seconds) if seconds > 60.0 => self.max_age_ms_early,
            Some(seconds) if seconds > 25.0 => self.max_age_ms_mid,
            _ => self.max_age_ms_late,
        }
    }

    fn falling_drop_threshold(self, seconds_left: Option<f64>) -> f64 {
        match seconds_left {
            Some(seconds) if seconds > 60.0 => self.falling_drop_cent_early,
            Some(seconds) if seconds > self.late_expensive_seconds => self.falling_drop_cent_mid,
            _ => self.falling_drop_cent_late,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct PriceToBeatWaitRepriceSnapshot {
    pub(super) execution_ask_cent: Option<f64>,
    pub(super) gap_strength: Option<f64>,
    pub(super) q_final_cent: Option<f64>,
}

pub(super) fn wait_reprice_snapshot_from_evaluation(
    evaluation: &PriceToBeatGuardEvaluation,
) -> PriceToBeatWaitRepriceSnapshot {
    let iv = evaluation.iv_mismatch_edge.as_ref();
    PriceToBeatWaitRepriceSnapshot {
        execution_ask_cent: iv.and_then(current_execution_ask_cent),
        gap_strength: iv.and_then(|value| f64_field(value, "gap_strength")),
        q_final_cent: iv
            .and_then(|value| f64_field(value, "q_final"))
            .map(|value| value * 100.0),
    }
}

pub(super) fn maybe_apply_wait_reprice_guard(
    node: &crate::TradeFlowNode,
    context: &Value,
    evaluation: &mut PriceToBeatGuardEvaluation,
    now_ms: i64,
) {
    let config = PriceToBeatWaitRepriceGuardConfig::from_node(node);
    if (!config.enabled && !config.low_quality_edge_recheck_enabled)
        || evaluation.configured_threshold_mode.as_deref() != Some("iv_mismatch_edge")
    {
        return;
    }

    let Some(iv) = evaluation.iv_mismatch_edge.as_ref() else {
        return;
    };
    let seconds_left = f64_field(iv, "seconds_left");
    let current_execution_ask_cent = current_execution_ask_cent(iv);
    let cap_cent = f64_field(iv, "time_rule_max_price_cent")
        .or_else(|| f64_field(iv, "time_rule_max_price").map(|value| value * 100.0))
        .or_else(|| f64_field(iv, "effective_max_price").map(|value| value * 100.0));
    let gap_strength = meaningful_gap_metric(iv, "gap_strength");
    let required_gap_strength = meaningful_gap_metric(iv, "required_gap_strength");
    let adjusted_margin_cent = f64_field(iv, "adjusted_margin").map(|value| value * 100.0);
    let q_final_cent = f64_field(iv, "q_final").map(|value| value * 100.0);
    let previous_wait = super::notification_state::price_to_beat_guard_waiting_state(context)
        .filter(|state| state.market_slug == evaluation.market_slug);
    let previous_time_rule_wait = previous_wait
        .as_ref()
        .filter(|state| state.reason_code == "blocked_time_rule_max_price");
    let previous_low_quality_wait = previous_wait.as_ref().filter(|state| {
        state.reason_code == REASON_RECHECK_LOW_QUALITY_EDGE
            || state.reason_code == REASON_RECHECK_MEDIUM_CHOP_EXPENSIVE_ENTRY
    });

    let initial_too_far = config.enabled
        && evaluation.reason_code == "blocked_time_rule_max_price"
        && previous_time_rule_wait.is_none()
        && current_execution_ask_cent
            .zip(cap_cent)
            .is_some_and(|(ask, cap)| ask - cap >= config.initial_ask_max_over_cap_cent);

    let wait_age_ms = previous_time_rule_wait
        .as_ref()
        .and_then(|state| state.started_at_ms)
        .map(|started| (now_ms - started).max(0));
    let wait_expired = wait_age_ms
        .map(|age| age > config.max_age_ms(seconds_left))
        .unwrap_or(false);

    let max_wait_ask_cent = previous_time_rule_wait
        .as_ref()
        .and_then(|state| state.max_execution_ask_cent)
        .or_else(|| {
            previous_time_rule_wait
                .as_ref()
                .and_then(|state| state.initial_execution_ask_cent)
        });
    let wait_price_drop_cent = max_wait_ask_cent
        .zip(current_execution_ask_cent)
        .map(|(max_ask, current_ask)| (max_ask - current_ask).max(0.0));
    let fell_into_cap = config.enabled
        && config.falling_into_cap_enabled
        && previous_time_rule_wait
            .as_ref()
            .and_then(|state| state.initial_execution_ask_cent)
            .zip(current_execution_ask_cent)
            .zip(cap_cent)
            .is_some_and(|((initial_ask, current_ask), cap)| {
                initial_ask > cap
                    && current_ask <= cap
                    && wait_price_drop_cent.unwrap_or(0.0)
                        >= config.falling_drop_threshold(seconds_left)
            });

    let late_expensive_entry = config.enabled
        && config.late_expensive_enabled
        && previous_time_rule_wait.is_some()
        && seconds_left.is_some_and(|seconds| seconds < config.late_expensive_seconds)
        && current_execution_ask_cent.is_some_and(|ask| ask >= config.late_expensive_vwap_cent)
        && (q_final_cent
            .map(|q| q < config.late_expensive_min_q_cent)
            .unwrap_or(true)
            || gap_strength
                .zip(required_gap_strength)
                .map(|(gap, required)| {
                    gap < required + config.late_expensive_min_gap_strength_extra
                })
                .unwrap_or(true));
    let low_quality_edge = config.low_quality_edge_recheck_enabled
        && evaluation.passed
        && evaluation.reason_code == "selected_edge_passed"
        && gap_strength
            .zip(required_gap_strength)
            .map(|(gap, required)| gap - required < config.low_quality_gap_margin)
            .unwrap_or(false)
        && adjusted_margin_cent
            .map(|margin| margin < config.low_quality_adjusted_margin_cent)
            .unwrap_or(false)
        && low_quality_dirty_signal(iv);
    let medium_chop_expensive_entry = config.low_quality_edge_recheck_enabled
        && evaluation.passed
        && evaluation.reason_code == "selected_edge_passed"
        && current_execution_ask_cent
            .map(|ask| ask >= config.late_expensive_vwap_cent)
            .unwrap_or(false)
        && str_field(iv, "ptb_movement_mode")
            .map(|mode| mode != "clean_trend")
            .unwrap_or(false)
        && f64_field(iv, "ptb_chop_gap_strength_penalty")
            .map(|penalty| penalty > 0.0)
            .unwrap_or(false)
        && gap_strength
            .zip(required_gap_strength)
            .map(|(gap, required)| gap - required < config.low_quality_gap_margin)
            .unwrap_or(false)
        && bool_field(iv, "book_confirmation_missing").unwrap_or(false);
    let low_quality_reason = if low_quality_edge || medium_chop_expensive_entry {
        Some(if previous_low_quality_wait.is_some() {
            REASON_LOW_QUALITY_EDGE_RECHECK_FAILED
        } else if medium_chop_expensive_entry {
            REASON_RECHECK_MEDIUM_CHOP_EXPENSIVE_ENTRY
        } else {
            REASON_RECHECK_LOW_QUALITY_EDGE
        })
    } else {
        None
    };

    let reason = if fell_into_cap {
        Some(REASON_FALLING_INTO_CAP)
    } else if initial_too_far {
        Some(REASON_INITIAL_ASK_TOO_FAR)
    } else if wait_expired {
        Some(REASON_WAIT_SIGNAL_EXPIRED)
    } else if late_expensive_entry {
        Some(REASON_LATE_EXPENSIVE_ENTRY)
    } else {
        low_quality_reason
    };

    append_wait_reprice_debug(
        evaluation,
        json!({
            "enabled": config.enabled,
            "blocked": reason.is_some(),
            "reason": reason,
            "low_quality_edge_recheck_enabled": config.low_quality_edge_recheck_enabled,
            "low_quality_edge": low_quality_edge,
            "medium_chop_expensive_entry": medium_chop_expensive_entry,
            "low_quality_gap_margin": gap_strength
                .zip(required_gap_strength)
                .map(|(gap, required)| gap - required),
            "low_quality_gap_margin_threshold": config.low_quality_gap_margin,
            "low_quality_adjusted_margin_cent": adjusted_margin_cent,
            "low_quality_adjusted_margin_threshold_cent": config.low_quality_adjusted_margin_cent,
            "low_quality_dirty_signal": low_quality_dirty_signal(iv),
            "low_quality_recheck_failed": reason == Some(REASON_LOW_QUALITY_EDGE_RECHECK_FAILED),
            "wait_for_price_age_ms": wait_age_ms,
            "wait_max_age_ms": config.max_age_ms(seconds_left),
            "wait_initial_execution_ask_cent": previous_time_rule_wait
                .as_ref()
                .and_then(|state| state.initial_execution_ask_cent)
                .or(current_execution_ask_cent),
            "wait_max_execution_ask_cent": max_wait_ask_cent.or(current_execution_ask_cent),
            "wait_current_execution_ask_cent": current_execution_ask_cent,
            "wait_price_drop_cent": wait_price_drop_cent,
            "wait_initial_gap_strength": previous_time_rule_wait
                .as_ref()
                .and_then(|state| state.initial_gap_strength)
                .or(gap_strength),
            "wait_current_gap_strength": gap_strength,
            "wait_initial_q_final_cent": previous_time_rule_wait
                .as_ref()
                .and_then(|state| state.initial_q_final_cent)
                .or(q_final_cent),
            "wait_current_q_final_cent": q_final_cent,
            "fell_into_cap": fell_into_cap,
            "late_expensive_entry": late_expensive_entry,
            "initial_ask_too_far_above_cap": initial_too_far,
            "wait_signal_expired": wait_expired,
            "time_rule_max_price_cent": cap_cent,
            "late_expensive_seconds": config.late_expensive_seconds,
            "late_expensive_vwap_cent": config.late_expensive_vwap_cent,
            "late_expensive_min_q_cent": config.late_expensive_min_q_cent,
            "late_expensive_min_gap_strength_extra": config.late_expensive_min_gap_strength_extra,
        }),
    );

    if let Some(reason) = reason {
        evaluation.passed = false;
        evaluation.reason_code = reason.to_string();
        evaluation.reason_detail = Some(wait_reprice_reason_detail(reason).to_string());
        if let Some(Value::Object(iv)) = evaluation.iv_mismatch_edge.as_mut() {
            iv.insert("passed".to_string(), json!(false));
            iv.insert("decision_reason".to_string(), json!(reason));
            prepend_all_reason(iv, reason);
        }
    }
}

pub(super) fn wait_reprice_reason_disables_retry(reason: &str) -> bool {
    matches!(
        reason,
        "blocked_no_matching_time_rule"
            | REASON_WAIT_SIGNAL_EXPIRED
            | REASON_INITIAL_ASK_TOO_FAR
            | REASON_FALLING_INTO_CAP
            | REASON_LATE_EXPENSIVE_ENTRY
            | REASON_LOW_QUALITY_EDGE_RECHECK_FAILED
    )
}

fn wait_reprice_reason_detail(reason: &str) -> &'static str {
    match reason {
        REASON_RECHECK_LOW_QUALITY_EDGE => {
            "EQ77 low quality edge requested a recheck before submitting the fixed-size order."
        }
        REASON_RECHECK_MEDIUM_CHOP_EXPENSIVE_ENTRY => {
            "EQ77 medium-chop expensive entry requested a recheck before submitting the fixed-size order."
        }
        REASON_LOW_QUALITY_EDGE_RECHECK_FAILED => {
            "EQ77 low quality edge stayed weak after recheck, so the fixed-size order was blocked."
        }
        _ => "EQ77 wait reprice guard blocked a stale or falling-into-cap max-price wait signal.",
    }
}

fn append_wait_reprice_debug(evaluation: &mut PriceToBeatGuardEvaluation, debug: Value) {
    if let Some(Value::Object(iv)) = evaluation.iv_mismatch_edge.as_mut() {
        iv.insert("wait_reprice_guard".to_string(), debug);
    }
}

fn meaningful_gap_metric(iv: &Value, key: &str) -> Option<f64> {
    let value = f64_field(iv, key)?;
    if value.abs() > f64::EPSILON || iv.get("selected_time_rule").is_some() {
        Some(value)
    } else {
        None
    }
}

fn prepend_all_reason(iv: &mut serde_json::Map<String, Value>, reason: &str) {
    let mut reasons = iv
        .get("all_reasons")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !reasons.iter().any(|value| value.as_str() == Some(reason)) {
        reasons.insert(0, json!(reason));
    }
    iv.insert("all_reasons".to_string(), Value::Array(reasons));
}

fn current_execution_ask_cent(iv: &Value) -> Option<f64> {
    f64_field(iv, "execution_vwap_cent")
        .or_else(|| f64_field(iv, "execution_best_ask_cent"))
        .or_else(|| f64_field(iv, "model_ask_cent"))
        .or_else(|| f64_field(iv, "ask").map(|value| value * 100.0))
}

fn low_quality_dirty_signal(iv: &Value) -> bool {
    str_field(iv, "ptb_movement_mode")
        .map(|mode| mode != "clean_trend")
        .unwrap_or(false)
        || str_field(iv, "cex_open_gap_consensus")
            .map(|consensus| !matches!(consensus, "strong" | "disabled" | "unavailable" | ""))
            .unwrap_or(false)
        || str_field(iv, "binance_veto_status")
            .map(|status| {
                status.contains("stale")
                    || status.contains("missing")
                    || status.contains("unavailable")
                    || status.contains("fail")
                    || status.contains("error")
            })
            .unwrap_or(false)
        || f64_field(iv, "binance_staleness_ms").is_some_and(|value| value > 1_500.0)
        || f64_field(iv, "chainlink_staleness_ms").is_some_and(|value| value > 1_500.0)
}

fn str_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn f64_field(value: &Value, key: &str) -> Option<f64> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
}

fn bool_field(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn non_negative_f64(node: &crate::TradeFlowNode, key: &str) -> Option<f64> {
    node_config_f64(node, key).filter(|value| value.is_finite() && *value >= 0.0)
}

fn node_config_bool(node: &crate::TradeFlowNode, key: &str) -> Option<bool> {
    crate::node_config_bool(node, key)
}

fn node_config_f64(node: &crate::TradeFlowNode, key: &str) -> Option<f64> {
    crate::node_config_f64(node, key)
}

fn node_config_i64(node: &crate::TradeFlowNode, key: &str) -> Option<i64> {
    crate::node_config_i64(node, key)
}
