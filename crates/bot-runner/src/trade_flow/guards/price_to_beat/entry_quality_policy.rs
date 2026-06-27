use super::PriceToBeatGuardEvaluation;
use serde_json::{json, Value};

const DEFAULT_HIGH_RISK_UNDER_SEC: f64 = 30.0;
const DEFAULT_HIGH_RISK_ASK_CENT: f64 = 85.0;
pub(crate) const DEFAULT_ENTRY_QUALITY_CHAINLINK_MAX_AGE_MS: i64 = 3_000;
const DEFAULT_HIGH_ASK_MAX_SPREAD_CENT: f64 = 2.0;
const DEFAULT_MAX_SPREAD_CENT: f64 = 3.0;
const DEFAULT_NEUTRAL_EDGE_PENALTY: f64 = 0.03;
const DEFAULT_NEUTRAL_GAP_STRENGTH_PENALTY: f64 = 0.25;
const DEFAULT_STALE_EDGE_PENALTY: f64 = 0.03;
const DEFAULT_STALE_GAP_STRENGTH_PENALTY: f64 = 0.25;
const REASON_CHAINLINK_PROVIDER_STALE_ENTRY_QUALITY: &str =
    "chainlink_provider_stale_entry_quality";
const REASON_CHAINLINK_PROVIDER_STALE_GLOBAL: &str = "chainlink_provider_stale_global";

#[derive(Debug, Clone, Copy)]
struct EntryQualityPolicy {
    high_risk_under_sec: f64,
    high_risk_ask: f64,
    chainlink_max_age_ms: i64,
    high_ask_max_spread: f64,
    max_spread: f64,
    neutral_edge_penalty: f64,
    neutral_gap_strength_penalty: f64,
    stale_edge_penalty: f64,
    stale_gap_strength_penalty: f64,
}

impl EntryQualityPolicy {
    fn from_node(node: &crate::TradeFlowNode) -> Self {
        Self {
            high_risk_under_sec: node_f64(
                node,
                "priceToBeatIvEntryQualityHighRiskUnderSec",
                DEFAULT_HIGH_RISK_UNDER_SEC,
            )
            .max(0.0),
            high_risk_ask: node_cent_probability(
                node,
                "priceToBeatIvEntryQualityHighRiskAskCent",
                DEFAULT_HIGH_RISK_ASK_CENT,
            ),
            chainlink_max_age_ms: entry_quality_chainlink_max_age_ms(node),
            high_ask_max_spread: node_cent_probability(
                node,
                "priceToBeatIvEntryQualityHighPriceMaxSpreadCent",
                DEFAULT_HIGH_ASK_MAX_SPREAD_CENT,
            ),
            max_spread: node_cent_probability(
                node,
                "priceToBeatIvEntryQualityMaxSpreadCent",
                DEFAULT_MAX_SPREAD_CENT,
            ),
            neutral_edge_penalty: node_f64(
                node,
                "priceToBeatIvEntryQualityNeutralEdgePenalty",
                DEFAULT_NEUTRAL_EDGE_PENALTY,
            )
            .max(0.0),
            neutral_gap_strength_penalty: node_f64(
                node,
                "priceToBeatIvEntryQualityNeutralGapStrengthPenalty",
                DEFAULT_NEUTRAL_GAP_STRENGTH_PENALTY,
            )
            .max(0.0),
            stale_edge_penalty: node_f64(
                node,
                "priceToBeatIvEntryQualityStaleEdgePenalty",
                DEFAULT_STALE_EDGE_PENALTY,
            )
            .max(0.0),
            stale_gap_strength_penalty: node_f64(
                node,
                "priceToBeatIvEntryQualityStaleGapStrengthPenalty",
                DEFAULT_STALE_GAP_STRENGTH_PENALTY,
            )
            .max(0.0),
        }
    }
}

pub(crate) fn entry_quality_chainlink_max_age_ms(node: &crate::TradeFlowNode) -> i64 {
    crate::node_config_i64(node, "priceToBeatIvEntryQualityChainlinkMaxAgeMs")
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_ENTRY_QUALITY_CHAINLINK_MAX_AGE_MS)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CexEntryStatus {
    Disabled,
    Aligned,
    Opposite,
    Neutral,
    Unconfirmed,
    Stale,
    Error,
    NotEvaluated,
}

impl CexEntryStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Aligned => "aligned",
            Self::Opposite => "opposite",
            Self::Neutral => "neutral",
            Self::Unconfirmed => "unconfirmed",
            Self::Stale => "stale",
            Self::Error => "error",
            Self::NotEvaluated => "not_evaluated",
        }
    }

    fn is_neutral_like(&self) -> bool {
        matches!(self, Self::Neutral | Self::Unconfirmed)
    }

    fn is_stale_like(&self) -> bool {
        matches!(self, Self::Stale | Self::Error)
    }
}

#[derive(Debug, Clone)]
struct EntryQualityState {
    seconds_left: Option<f64>,
    ask: Option<f64>,
    spread: Option<f64>,
    chainlink_age_ms: Option<i64>,
    chainlink_receive_age_ms: Option<i64>,
    chainlink_global_limit_ms: Option<i64>,
    edge: Option<f64>,
    required_edge: Option<f64>,
    adjusted_margin: Option<f64>,
    gap_strength: Option<f64>,
    required_gap_strength: Option<f64>,
    gap_strength_margin: Option<f64>,
    matched_rule: Option<String>,
    cex_status: CexEntryStatus,
    cex_reason_code: Option<String>,
    chainlink_stale_exception_passed: bool,
}

impl EntryQualityState {
    fn from_evaluation(evaluation: &PriceToBeatGuardEvaluation) -> Self {
        let iv = evaluation.iv_mismatch_edge.as_ref();
        let cex = evaluation.cex_direction_guard.as_ref();
        let expected_move_eff = iv.and_then(expected_move_eff_for_gap_strength);
        let gap_strength = iv
            .and_then(|value| json_f64(value, "gap_strength"))
            .filter(|value| *value > f64::EPSILON)
            .or_else(|| {
                evaluation
                    .directional_gap
                    .zip(expected_move_eff)
                    .map(|(gap, expected_move)| (gap / expected_move.max(f64::EPSILON)).max(0.0))
            });
        let required_gap_strength = iv.and_then(|value| {
            json_f64(value, "required_gap_strength")
                .filter(|value| *value > f64::EPSILON)
                .or_else(|| selected_time_rule_number(value, "min_gap_strength"))
        });
        let gap_strength_margin = iv
            .and_then(|value| json_f64(value, "gap_strength_margin"))
            .or_else(|| {
                gap_strength
                    .zip(required_gap_strength)
                    .map(|(gap, required)| gap - required)
            });
        Self {
            seconds_left: iv.and_then(|value| json_f64(value, "seconds_left")),
            ask: iv.and_then(|value| {
                json_f64(value, "selected_ask").or_else(|| json_f64(value, "ask"))
            }),
            spread: iv.and_then(|value| json_f64(value, "spread")),
            chainlink_age_ms: iv.and_then(|value| json_i64(value, "chainlink_staleness_ms")),
            chainlink_receive_age_ms: iv
                .and_then(|value| json_i64(value, "last_symbol_received_age_ms"))
                .or_else(|| iv.and_then(|value| json_i64(value, "chainlink_ws_receipt_age_ms"))),
            chainlink_global_limit_ms: iv
                .and_then(|value| json_i64(value, "chainlink_stale_ms_effective"))
                .or_else(|| {
                    iv.and_then(|value| json_i64(value, "chainlink_normal_stale_limit_ms"))
                }),
            chainlink_stale_exception_passed: iv
                .and_then(|value| json_bool(value, "chainlink_stale_strong_gap_exception_passed"))
                .unwrap_or(false),
            edge: iv
                .and_then(|value| json_f64(value, "edge_adj").or_else(|| json_f64(value, "edge"))),
            required_edge: iv.and_then(|value| {
                json_f64(value, "dynamic_threshold").or_else(|| json_f64(value, "threshold"))
            }),
            adjusted_margin: iv.and_then(|value| json_f64(value, "adjusted_margin")),
            gap_strength,
            required_gap_strength,
            gap_strength_margin,
            matched_rule: iv.and_then(matched_rule_label),
            cex_status: cex_entry_status(cex),
            cex_reason_code: cex
                .and_then(|value| json_string(value, "reason_code"))
                .or_else(|| {
                    (!evaluation.passed).then(|| "skipped_price_to_beat_not_passed".to_string())
                }),
        }
    }

    fn high_risk_entry(&self, policy: &EntryQualityPolicy) -> bool {
        self.seconds_left
            .map(|value| value <= policy.high_risk_under_sec)
            .unwrap_or(false)
            && self
                .ask
                .map(|value| value >= policy.high_risk_ask)
                .unwrap_or(false)
    }

    fn late_entry(&self, policy: &EntryQualityPolicy) -> bool {
        self.seconds_left
            .map(|value| value <= policy.high_risk_under_sec)
            .unwrap_or(false)
    }

    fn high_ask(&self, policy: &EntryQualityPolicy) -> bool {
        self.ask
            .map(|value| value >= policy.high_risk_ask)
            .unwrap_or(false)
    }

    fn chainlink_stale(&self, policy: &EntryQualityPolicy) -> bool {
        self.chainlink_age_ms
            .map(|value| value > policy.chainlink_max_age_ms)
            .unwrap_or(false)
    }
}

pub(crate) fn apply_action_place_order_entry_quality_policy(
    node: &crate::TradeFlowNode,
    evaluation: &mut PriceToBeatGuardEvaluation,
) {
    if !crate::node_config_bool(node, "priceToBeatIvEntryQualityPolicy").unwrap_or(false) {
        return;
    }
    if evaluation.iv_mismatch_edge.is_none() {
        return;
    }

    let policy = EntryQualityPolicy::from_node(node);
    let state = EntryQualityState::from_evaluation(evaluation);
    let mut reason = normalized_existing_reason(evaluation);
    let mut detail = reason.map(|reason| format!("entry quality normalized reason={reason}"));
    let mut stale_policy_action: Option<&'static str> = None;
    let mut stale_penalty_applied = false;

    if reason.is_none() {
        if state.chainlink_stale(&policy) {
            if state.late_entry(&policy) {
                if state.chainlink_stale_exception_passed {
                    stale_policy_action = Some("skip_suppressed_apply_stale_penalty");
                    stale_penalty_applied = true;
                    if let Some(penalty_reason) = penalty_block_reason(
                        &state,
                        policy.stale_edge_penalty,
                        policy.stale_gap_strength_penalty,
                    ) {
                        reason = Some(penalty_reason);
                        detail = Some(format!(
                            "chainlink stale exception suppressed hard skip; stale penalty failed; edge_penalty={}; gap_strength_penalty={}",
                            policy.stale_edge_penalty, policy.stale_gap_strength_penalty
                        ));
                    }
                } else {
                    stale_policy_action = Some("skip");
                    reason = Some(REASON_CHAINLINK_PROVIDER_STALE_ENTRY_QUALITY);
                    detail = Some(format!(
                        "chainlink_provider_age_ms={} exceeds entry_quality_max_age_ms={} inside late entry window",
                        state.chainlink_age_ms.unwrap_or_default(),
                        policy.chainlink_max_age_ms
                    ));
                }
            } else if let Some(penalty_reason) = penalty_block_reason(
                &state,
                policy.stale_edge_penalty,
                policy.stale_gap_strength_penalty,
            ) {
                reason = Some(penalty_reason);
                detail = Some(format!(
                    "chainlink stale penalty failed; edge_penalty={}; gap_strength_penalty={}",
                    policy.stale_edge_penalty, policy.stale_gap_strength_penalty
                ));
            }
        }
    }

    if reason.is_none() {
        if let (Some(ask), Some(spread)) = (state.ask, state.spread) {
            let max_spread = if ask >= policy.high_risk_ask {
                policy.high_ask_max_spread
            } else {
                policy.max_spread
            };
            if spread > max_spread {
                reason = Some("entry_spread_too_wide");
                detail = Some(format!(
                    "spread={spread:.6} exceeds max_spread={max_spread:.6}"
                ));
            }
        }
    }

    if reason.is_none() {
        match state.cex_status {
            CexEntryStatus::Opposite => {
                reason = Some("cex_direction_opposite");
                detail = Some("CEX consensus is opposite selected outcome".to_string());
            }
            ref status if status.is_stale_like() => {
                if state.late_entry(&policy) || state.high_ask(&policy) {
                    reason = Some("cex_direction_stale");
                    detail = Some(format!(
                        "CEX status={} in late/high-price entry zone",
                        status.as_str()
                    ));
                } else if let Some(penalty_reason) = penalty_block_reason(
                    &state,
                    policy.stale_edge_penalty,
                    policy.stale_gap_strength_penalty,
                ) {
                    reason = Some(penalty_reason);
                    detail = Some(format!(
                        "CEX stale penalty failed; edge_penalty={}; gap_strength_penalty={}",
                        policy.stale_edge_penalty, policy.stale_gap_strength_penalty
                    ));
                }
            }
            ref status if status.is_neutral_like() && state.high_risk_entry(&policy) => {
                if let Some(penalty_reason) = penalty_block_reason(
                    &state,
                    policy.neutral_edge_penalty,
                    policy.neutral_gap_strength_penalty,
                ) {
                    reason = Some(penalty_reason);
                    detail = Some(format!(
                        "CEX {} penalty failed; edge_penalty={}; gap_strength_penalty={}",
                        status.as_str(),
                        policy.neutral_edge_penalty,
                        policy.neutral_gap_strength_penalty
                    ));
                }
            }
            _ => {}
        }
    }

    if let Some(reason) = reason {
        evaluation.passed = false;
        evaluation.reason_code = reason.to_string();
        evaluation.reason_detail = detail;
    }
    attach_entry_quality_debug(
        evaluation,
        &state,
        &policy,
        reason,
        stale_policy_action,
        stale_penalty_applied,
    );
}

fn normalized_existing_reason(evaluation: &PriceToBeatGuardEvaluation) -> Option<&'static str> {
    if evaluation.passed {
        return None;
    }
    match evaluation.reason_code.as_str() {
        "blocked_rtds_stale" | REASON_CHAINLINK_PROVIDER_STALE_GLOBAL => {
            Some(REASON_CHAINLINK_PROVIDER_STALE_GLOBAL)
        }
        REASON_CHAINLINK_PROVIDER_STALE_ENTRY_QUALITY => {
            Some(REASON_CHAINLINK_PROVIDER_STALE_ENTRY_QUALITY)
        }
        "blocked_spread_wide" => Some("entry_spread_too_wide"),
        "blocked_gap_strength_below_threshold" => Some("iv_gap_strength_below_threshold"),
        "blocked_edge_below_threshold" | "blocked_thin_adjusted_margin" => {
            Some("iv_edge_below_threshold")
        }
        "cex_direction_guard_opposite" => Some("cex_direction_opposite"),
        "cex_direction_guard_unavailable"
        | "cex_direction_guard_unsupported_outcome"
        | "cex_direction_guard_unsupported_market"
        | "cex_direction_guard_missing_window_start" => Some("cex_direction_stale"),
        "blocked_chainlink_stale_age_unavailable" => {
            Some("blocked_chainlink_stale_age_unavailable")
        }
        "blocked_chainlink_stale_age_too_high" => Some("blocked_chainlink_stale_age_too_high"),
        "blocked_chainlink_stale_gap_unavailable" => {
            Some("blocked_chainlink_stale_gap_unavailable")
        }
        "blocked_chainlink_stale_weak_gap" => Some("blocked_chainlink_stale_weak_gap"),
        "blocked_chainlink_stale_no_cex_confirmation" => {
            Some("blocked_chainlink_stale_no_cex_confirmation")
        }
        "blocked_chainlink_stale_no_bybit_hit" => Some("blocked_chainlink_stale_no_bybit_hit"),
        "blocked_chainlink_stale_no_secondary_confirmation" => {
            Some("blocked_chainlink_stale_no_secondary_confirmation")
        }
        "blocked_chainlink_stale_cex_not_clean" => Some("blocked_chainlink_stale_cex_not_clean"),
        _ => None,
    }
}

fn penalty_block_reason(
    state: &EntryQualityState,
    edge_penalty: f64,
    gap_strength_penalty: f64,
) -> Option<&'static str> {
    if edge_penalty > 0.0
        && state
            .adjusted_margin
            .map(|margin| margin < edge_penalty)
            .unwrap_or(false)
    {
        return Some("iv_edge_below_threshold");
    }
    if gap_strength_penalty > 0.0
        && state
            .gap_strength_margin
            .map(|margin| margin < gap_strength_penalty)
            .unwrap_or(false)
    {
        return Some("iv_gap_strength_below_threshold");
    }
    None
}

fn attach_entry_quality_debug(
    evaluation: &mut PriceToBeatGuardEvaluation,
    state: &EntryQualityState,
    policy: &EntryQualityPolicy,
    blocking_reason: Option<&'static str>,
    stale_policy_action: Option<&'static str>,
    stale_penalty_applied: bool,
) {
    let reason = blocking_reason.unwrap_or(evaluation.reason_code.as_str());
    let chainlink_stale_kind = chainlink_stale_kind_for_reason(reason);
    let legacy_reason_code = chainlink_stale_kind.and_then(|kind| match kind {
        "provider_stale_global" => Some("blocked_rtds_stale"),
        "provider_stale_entry_quality" => Some("chainlink_stale"),
        _ => None,
    });
    let mut debug = json!({
        "ptb_gate": {
            "passed": ptb_gate_passed(evaluation),
            "gapUsd": evaluation.directional_gap,
            "requiredGapUsd": evaluation.threshold_usd,
        },
        "iv_edge": {
            "passed": !matches!(reason, "iv_edge_below_threshold" | "iv_gap_strength_below_threshold"),
            "edge": state.edge,
            "requiredEdge": state.required_edge,
            "adjustedMargin": state.adjusted_margin,
            "gapStrength": state.gap_strength,
            "requiredGapStrength": state.required_gap_strength,
            "gapStrengthMargin": state.gap_strength_margin,
            "matchedRule": state.matched_rule.as_deref(),
        },
        "cex_direction_guard": {
            "enabled": state.cex_status != CexEntryStatus::Disabled,
            "mode": "bybit_plus_one",
            "status": state.cex_status.as_str(),
            "blocking": matches!(reason, "cex_direction_opposite" | "cex_direction_stale"),
            "reasonCode": state.cex_reason_code.as_deref(),
        },
        "source": {
            "ptbCurrentPriceSource": evaluation.current_price_source,
            "chainlinkAgeMs": state.chainlink_age_ms,
        },
        "chainlink_stale_policy": {
            "late_entry": state.late_entry(policy),
            "chainlink_age_ms": state.chainlink_age_ms,
            "provider_age_ms": state.chainlink_age_ms,
            "receive_age_ms": state.chainlink_receive_age_ms,
            "global_limit_ms": state.chainlink_global_limit_ms,
            "entry_quality_limit_ms": policy.chainlink_max_age_ms,
            "normal_late_skip_threshold_ms": policy.chainlink_max_age_ms,
            "chainlink_stale_kind": chainlink_stale_kind,
            "legacy_reason_code": legacy_reason_code,
            "chainlink_stale_exception_passed": state.chainlink_stale_exception_passed,
            "action": stale_policy_action,
            "stale_penalty_applied": stale_penalty_applied,
        },
        "decision": if evaluation.passed { "allow" } else { "skip" },
        "reason": reason,
    });
    if let Some(Value::Object(obj)) = evaluation.iv_mismatch_edge.as_mut() {
        let preserved_eq77_fields = obj
            .get("entry_quality_debug")
            .and_then(Value::as_object)
            .map(preserved_eq77_entry_quality_debug_fields)
            .unwrap_or_default();
        if let Some(next_debug) = debug.as_object_mut() {
            for (key, value) in preserved_eq77_fields {
                next_debug.insert(key, value);
            }
        }
        obj.insert("entry_quality_debug".to_string(), debug);
    }
}

fn chainlink_stale_kind_for_reason(reason: &str) -> Option<&'static str> {
    match reason {
        REASON_CHAINLINK_PROVIDER_STALE_GLOBAL => Some("provider_stale_global"),
        REASON_CHAINLINK_PROVIDER_STALE_ENTRY_QUALITY => Some("provider_stale_entry_quality"),
        _ => None,
    }
}

fn preserved_eq77_entry_quality_debug_fields(
    existing: &serde_json::Map<String, Value>,
) -> Vec<(String, Value)> {
    [
        "allowed",
        "primary_reason",
        "all_reasons",
        "expected_move_raw",
        "expected_move_eff",
        "expected_move_floor_applied",
        "required_gap_strength",
        "gap_strength_required",
        "gap_strength_required_with_margin",
        "gap_strength",
        "gap_strength_hard_floor",
        "gap_strength_deficit",
        "gap_strength_soft_low_ratio",
        "gap_soft_low_risk_points",
        "gap_strength_soft_low",
        "eq77_lite_profile",
        "buffer_5s_ago",
        "buffer_10s_ago",
        "same_side_history_5s",
        "same_side_history_10s",
        "buffer_retain_5s",
        "buffer_retain_10s",
        "spike_ratio",
        "spike_retrace_usd",
        "premium_ev_pass",
        "premium_ev_required_price",
        "premium_ev_margin_cent",
        "premium_price_allowed",
        "effective_max_buy_price",
        "fair_probability",
        "fee_buffer",
        "min_edge",
        "entry_quality_price_source",
        "history_price_source",
        "entry_action",
        "hard_block",
        "deferred",
        "signal_recheck_required",
        "risk_cap_price_cent",
        "ask_over_cap_cent",
        "risk_score",
        "cap_haircut_cent",
        "risk_level",
        "lane",
        "size_multiplier",
        "risk_components",
        "cap_components",
    ]
    .into_iter()
    .filter_map(|key| {
        existing
            .get(key)
            .map(|value| (key.to_string(), value.clone()))
    })
    .collect()
}

fn ptb_gate_passed(evaluation: &PriceToBeatGuardEvaluation) -> Option<bool> {
    evaluation
        .directional_gap
        .map(|gap| gap >= evaluation.threshold_usd)
}

fn cex_entry_status(cex: Option<&Value>) -> CexEntryStatus {
    let Some(cex) = cex else {
        return CexEntryStatus::NotEvaluated;
    };
    if json_bool(cex, "enabled") == Some(false) {
        return CexEntryStatus::Disabled;
    }
    match json_string(cex, "reason_code").as_deref() {
        Some("cex_direction_guard_passed") => CexEntryStatus::Aligned,
        Some("cex_direction_guard_opposite") => CexEntryStatus::Opposite,
        Some("cex_direction_guard_neutral") => CexEntryStatus::Neutral,
        Some("cex_direction_guard_unconfirmed") => CexEntryStatus::Unconfirmed,
        Some("cex_direction_guard_unavailable") => CexEntryStatus::Stale,
        Some("cex_direction_guard_unsupported_outcome")
        | Some("cex_direction_guard_unsupported_market")
        | Some("cex_direction_guard_missing_window_start") => CexEntryStatus::Error,
        Some(_) => CexEntryStatus::Error,
        None => CexEntryStatus::NotEvaluated,
    }
}

fn matched_rule_label(iv: &Value) -> Option<String> {
    let rule = iv.get("selected_time_rule")?;
    let start = json_f64(rule, "start_remaining_secs")?;
    let end = json_f64(rule, "end_remaining_secs")?;
    Some(format!("{start:.0}-{end:.0}"))
}

fn expected_move_eff_for_gap_strength(iv: &Value) -> Option<f64> {
    json_f64(iv, "expected_move_eff")
        .or_else(|| json_f64(iv, "expected_move"))
        .or_else(|| selected_time_rule_number(iv, "min_expected_move_usd"))
        .filter(|value| *value > f64::EPSILON)
}

fn selected_time_rule_number(iv: &Value, key: &str) -> Option<f64> {
    iv.get("selected_time_rule")
        .and_then(|rule| json_f64(rule, key))
        .filter(|value| *value > f64::EPSILON)
}

fn node_f64(node: &crate::TradeFlowNode, key: &str, fallback: f64) -> f64 {
    crate::node_config_f64(node, key)
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
}

fn node_cent_probability(node: &crate::TradeFlowNode, key: &str, fallback_cent: f64) -> f64 {
    (node_f64(node, key, fallback_cent) / 100.0).clamp(0.0, 1.0)
}

fn json_f64(value: &Value, key: &str) -> Option<f64> {
    value
        .get(key)
        .and_then(crate::value_as_f64)
        .filter(|value| value.is_finite())
}

fn json_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(crate::value_as_i64)
}

fn json_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn json_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_node() -> crate::TradeFlowNode {
        test_node_with_config(json!({
            "priceToBeatIvEntryQualityPolicy": true,
        }))
    }

    fn test_node_with_config(config: Value) -> crate::TradeFlowNode {
        crate::TradeFlowNode {
            key: "action_1".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    fn evaluation(iv: Value, cex: Option<Value>) -> PriceToBeatGuardEvaluation {
        PriceToBeatGuardEvaluation {
            passed: true,
            reason_code: "selected_edge_passed".to_string(),
            reason_detail: None,
            normalized_outcome_label: Some("Up".to_string()),
            direction: Some("up".to_string()),
            market_slug: "btc-updown-5m-1774013100".to_string(),
            event_url: String::new(),
            timeframe: Some("5m".to_string()),
            asset: Some("btc".to_string()),
            price_to_beat: Some(100.0),
            price_to_beat_status: Some("ok".to_string()),
            price_to_beat_source: Some("chainlink".to_string()),
            price_to_beat_source_latency_ms: Some(1),
            current_price: Some(110.0),
            current_price_source: "chainlink",
            directional_gap: Some(10.0),
            gap_abs: Some(10.0),
            threshold_mode: "iv_mismatch_edge".to_string(),
            configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
            base_threshold_value: Some(10.0),
            base_threshold_unit: Some("usd".to_string()),
            base_threshold_usd: Some(10.0),
            current_effective_ptb_usd: Some(10.0),
            threshold_value: 10.0,
            threshold_unit: "usd".to_string(),
            threshold_usd: 10.0,
            stop_loss_bump_count: 0,
            stop_loss_bump_applied_count: 0,
            stop_loss_bump_amount: None,
            stop_loss_bump_max_value: None,
            stop_loss_bump_unit: None,
            stop_loss_bump_raw_usd: 0.0,
            stop_loss_bump_usd: 0.0,
            stop_loss_bump_capped: false,
            stop_loss_bump_max_reached: false,
            stop_loss_bump_current_market_excluded: false,
            stop_loss_bump_increment_usd: 0.0,
            reentry_generation: 0,
            reentry_override_active: false,
            reentry_override_value: None,
            reentry_override_unit: None,
            max_price_relax: None,
            auto_threshold_usd: None,
            lookback_windows_used: None,
            current_windows_used: None,
            avg_up_excursion_usd: None,
            avg_down_excursion_usd: None,
            lookback_market_slugs: None,
            lookback_window_snapshots: None,
            baseline_pct: None,
            current_pct: None,
            vol_factor: None,
            threshold_pct: None,
            base_pct: None,
            floor_usd: None,
            ceiling_usd: None,
            threshold_was_clamped: None,
            signal_formula: None,
            iv_mismatch_edge: Some(iv),
            early_stale_side: None,
            cex_direction_guard: cex,
            entry_current_source_debug: None,
        }
    }

    fn base_iv(overrides: Value) -> Value {
        let mut value = json!({
            "seconds_left": 25.0,
            "ask": 0.86,
            "spread": 0.01,
            "chainlink_staleness_ms": 500,
            "edge_adj": 0.16,
            "dynamic_threshold": 0.08,
            "adjusted_margin": 0.08,
            "gap_strength": 1.0,
            "required_gap_strength": 0.75,
            "gap_strength_margin": 0.25,
            "selected_time_rule": {
                "start_remaining_secs": 30.0,
                "end_remaining_secs": 15.0,
            }
        });
        if let (Some(base), Some(extra)) = (value.as_object_mut(), overrides.as_object()) {
            for (key, override_value) in extra {
                base.insert(key.clone(), override_value.clone());
            }
        }
        value
    }

    #[test]
    fn ptb_pass_iv_pass_cex_neutral_allows_when_penalty_margin_exists() {
        let mut evaluation = evaluation(
            base_iv(json!({ "adjusted_margin": 0.08, "gap_strength_margin": 0.25 })),
            Some(json!({
                "enabled": true,
                "reason_code": "cex_direction_guard_neutral",
            })),
        );

        apply_action_place_order_entry_quality_policy(&test_node(), &mut evaluation);

        assert!(evaluation.passed);
        assert_eq!(evaluation.reason_code, "selected_edge_passed");
        assert_eq!(
            evaluation
                .iv_mismatch_edge
                .as_ref()
                .and_then(|value| value.pointer("/entry_quality_debug/decision"))
                .and_then(Value::as_str),
            Some("allow")
        );
    }

    #[test]
    fn policy_debug_preserves_eq77_risk_cap_fields() {
        let mut evaluation = evaluation(
            base_iv(json!({
                "entry_quality_debug": {
                    "allowed": false,
                    "primary_reason": "blocked_price_above_effective_max",
                    "entry_action": "wait_for_price",
                    "hard_block": false,
                    "deferred": true,
                    "signal_recheck_required": true,
                    "risk_cap_price_cent": 70.0,
                    "ask_over_cap_cent": 5.0,
                    "risk_score": 48.0,
                    "cap_haircut_cent": 6.0,
                    "risk_level": "high",
                    "lane": "high",
                    "size_multiplier": 0.5,
                    "risk_components": [
                        {"name": "impulse_ratio_strong", "risk_points": 18.0, "haircut_cent": 2.0}
                    ],
                    "cap_components": [
                        {"name": "risk_cap", "cap_cent": 70.0}
                    ]
                }
            })),
            None,
        );

        apply_action_place_order_entry_quality_policy(&test_node(), &mut evaluation);

        let debug = evaluation
            .iv_mismatch_edge
            .as_ref()
            .and_then(|value| value.get("entry_quality_debug"))
            .expect("entry quality debug");
        assert_eq!(
            debug.pointer("/decision").and_then(Value::as_str),
            Some("allow")
        );
        assert_eq!(
            debug.pointer("/entry_action").and_then(Value::as_str),
            Some("wait_for_price")
        );
        assert_eq!(
            debug.pointer("/risk_score").and_then(crate::value_as_f64),
            Some(48.0)
        );
        assert_eq!(
            debug
                .pointer("/risk_components/0/name")
                .and_then(Value::as_str),
            Some("impulse_ratio_strong")
        );
    }

    #[test]
    fn cex_stale_late_entry_blocks() {
        let mut evaluation = evaluation(
            base_iv(json!({ "seconds_left": 30.0, "ask": 0.70 })),
            Some(json!({
                "enabled": true,
                "reason_code": "cex_direction_guard_unavailable",
            })),
        );

        apply_action_place_order_entry_quality_policy(&test_node(), &mut evaluation);

        assert!(!evaluation.passed);
        assert_eq!(evaluation.reason_code, "cex_direction_stale");
    }

    #[test]
    fn debug_marks_cex_direction_guard_skipped_when_ptb_already_failed() {
        let mut evaluation = evaluation(base_iv(json!({})), None);
        evaluation.passed = false;
        evaluation.reason_code = "cex_consensus_bybit_below_threshold".to_string();

        apply_action_place_order_entry_quality_policy(&test_node(), &mut evaluation);

        assert_eq!(
            evaluation
                .iv_mismatch_edge
                .as_ref()
                .and_then(|value| value.pointer("/entry_quality_debug/cex_direction_guard/status"))
                .and_then(Value::as_str),
            Some("not_evaluated")
        );
        assert_eq!(
            evaluation
                .iv_mismatch_edge
                .as_ref()
                .and_then(|value| {
                    value.pointer("/entry_quality_debug/cex_direction_guard/reasonCode")
                })
                .and_then(Value::as_str),
            Some("skipped_price_to_beat_not_passed")
        );
    }

    #[test]
    fn chainlink_provider_stale_entry_quality_late_entry_blocks() {
        let mut evaluation = evaluation(
            base_iv(json!({
                "chainlink_staleness_ms": 3_001,
                "chainlink_stale_ms_effective": 3_500,
                "last_symbol_received_age_ms": 300
            })),
            None,
        );
        let node = test_node_with_config(json!({
            "priceToBeatIvEntryQualityPolicy": true,
            "priceToBeatIvEntryQualityChainlinkMaxAgeMs": 2_500,
        }));

        apply_action_place_order_entry_quality_policy(&node, &mut evaluation);

        assert!(!evaluation.passed);
        assert_eq!(
            evaluation.reason_code,
            REASON_CHAINLINK_PROVIDER_STALE_ENTRY_QUALITY
        );
        let debug = evaluation
            .iv_mismatch_edge
            .as_ref()
            .and_then(|value| value.get("entry_quality_debug"))
            .expect("entry quality debug");
        assert_eq!(
            debug
                .pointer("/chainlink_stale_policy/chainlink_stale_kind")
                .and_then(Value::as_str),
            Some("provider_stale_entry_quality")
        );
        assert_eq!(
            debug
                .pointer("/chainlink_stale_policy/legacy_reason_code")
                .and_then(Value::as_str),
            Some("chainlink_stale")
        );
        assert_eq!(
            debug
                .pointer("/chainlink_stale_policy/provider_age_ms")
                .and_then(crate::value_as_i64),
            Some(3_001)
        );
        assert_eq!(
            debug
                .pointer("/chainlink_stale_policy/receive_age_ms")
                .and_then(crate::value_as_i64),
            Some(300)
        );
        assert_eq!(
            debug
                .pointer("/chainlink_stale_policy/global_limit_ms")
                .and_then(crate::value_as_i64),
            Some(3_500)
        );
    }

    #[test]
    fn chainlink_provider_stale_entry_quality_non_late_applies_penalty_path() {
        let mut evaluation = evaluation(
            base_iv(json!({
                "seconds_left": 35.0,
                "chainlink_staleness_ms": 3_001,
                "chainlink_stale_ms_effective": 3_500
            })),
            None,
        );
        let node = test_node_with_config(json!({
            "priceToBeatIvEntryQualityPolicy": true,
            "priceToBeatIvEntryQualityChainlinkMaxAgeMs": 2_500,
        }));

        apply_action_place_order_entry_quality_policy(&node, &mut evaluation);

        assert!(evaluation.passed, "{evaluation:?}");
        assert_eq!(evaluation.reason_code, "selected_edge_passed");
    }

    #[test]
    fn chainlink_stale_late_entry_uses_configured_max_age() {
        let mut evaluation = evaluation(base_iv(json!({ "chainlink_staleness_ms": 3_200 })), None);
        let node = test_node_with_config(json!({
            "priceToBeatIvEntryQualityPolicy": true,
            "priceToBeatIvEntryQualityChainlinkMaxAgeMs": 3_500,
        }));

        apply_action_place_order_entry_quality_policy(&node, &mut evaluation);

        assert!(evaluation.passed, "{evaluation:?}");
        assert_eq!(evaluation.reason_code, "selected_edge_passed");
        let debug = evaluation
            .iv_mismatch_edge
            .as_ref()
            .and_then(|value| value.get("entry_quality_debug"))
            .expect("entry quality debug");
        assert_eq!(
            debug
                .pointer("/chainlink_stale_policy/normal_late_skip_threshold_ms")
                .and_then(crate::value_as_i64),
            Some(3_500)
        );
        assert_eq!(
            debug
                .pointer("/chainlink_stale_policy/action")
                .and_then(Value::as_str),
            None
        );
    }

    #[test]
    fn chainlink_stale_exception_suppresses_late_entry_hard_skip() {
        let mut evaluation = evaluation(
            base_iv(json!({
                "chainlink_staleness_ms": 3_015,
                "chainlink_stale_strong_gap_exception_passed": true,
                "gap_strength": 10.0,
                "adjusted_margin": 0.08,
                "gap_strength_margin": 9.25
            })),
            None,
        );

        apply_action_place_order_entry_quality_policy(&test_node(), &mut evaluation);

        assert!(evaluation.passed);
        assert_eq!(evaluation.reason_code, "selected_edge_passed");
        let debug = evaluation
            .iv_mismatch_edge
            .as_ref()
            .and_then(|value| value.get("entry_quality_debug"))
            .expect("entry quality debug");
        assert_eq!(
            debug
                .pointer("/chainlink_stale_policy/action")
                .and_then(Value::as_str),
            Some("skip_suppressed_apply_stale_penalty")
        );
        assert_eq!(
            debug
                .pointer("/chainlink_stale_policy/stale_penalty_applied")
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn chainlink_stale_exception_can_still_fail_stale_penalty() {
        let mut evaluation = evaluation(
            base_iv(json!({
                "chainlink_staleness_ms": 3_015,
                "chainlink_stale_strong_gap_exception_passed": true,
                "gap_strength": 10.0,
                "adjusted_margin": 0.01,
                "gap_strength_margin": 9.25
            })),
            None,
        );

        apply_action_place_order_entry_quality_policy(&test_node(), &mut evaluation);

        assert!(!evaluation.passed);
        assert_eq!(evaluation.reason_code, "iv_edge_below_threshold");
    }

    #[test]
    fn high_ask_spread_blocks_at_two_cent() {
        let mut evaluation = evaluation(base_iv(json!({ "ask": 0.86, "spread": 0.021 })), None);

        apply_action_place_order_entry_quality_policy(&test_node(), &mut evaluation);

        assert!(!evaluation.passed);
        assert_eq!(evaluation.reason_code, "entry_spread_too_wide");
    }

    #[test]
    fn policy_debug_derives_gap_metrics_from_selected_time_rule() {
        let mut evaluation = evaluation(
            base_iv(json!({
                "ask": 0.67,
                "spread": 0.04,
                "gap_strength": 0.0,
                "required_gap_strength": 0.0,
                "gap_strength_margin": null,
                "expected_move_eff": null,
                "selected_time_rule": {
                    "start_remaining_secs": 25.0,
                    "end_remaining_secs": 10.0,
                    "min_gap_strength": 1.35,
                    "min_expected_move_usd": 0.00005
                }
            })),
            None,
        );
        evaluation.directional_gap = Some(0.00025);

        apply_action_place_order_entry_quality_policy(&test_node(), &mut evaluation);

        let debug = evaluation
            .iv_mismatch_edge
            .as_ref()
            .and_then(|value| value.pointer("/entry_quality_debug/iv_edge"))
            .expect("iv edge debug");
        assert_eq!(
            debug.get("gapStrength").and_then(crate::value_as_f64),
            Some(5.0)
        );
        assert_eq!(
            debug
                .get("requiredGapStrength")
                .and_then(crate::value_as_f64),
            Some(1.35)
        );
        assert_eq!(
            debug.get("gapStrengthMargin").and_then(crate::value_as_f64),
            Some(3.65)
        );
        assert_eq!(
            debug.get("matchedRule").and_then(Value::as_str),
            Some("25-10")
        );
    }
}
