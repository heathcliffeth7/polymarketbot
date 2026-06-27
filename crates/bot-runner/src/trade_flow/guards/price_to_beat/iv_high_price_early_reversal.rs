use super::iv_cex_open_gap::CexOpenGapConsensus;
use serde_json::{json, Map, Value};

const DEFAULT_REF_THRESHOLD: f64 = 0.77;
const DEFAULT_REMAINING_SEC: f64 = 120.0;
const DEFAULT_MAX_STALE_MS: i64 = 2_000;
const DEFAULT_STALE_GAP_ADD: f64 = 0.30;
const DEFAULT_BINANCE_MISSING_GAP_ADD: f64 = 0.35;
const DEFAULT_Q_EXTREME: f64 = 0.985;
const DEFAULT_Q_EXTREME_MIN_GAP_STRENGTH: f64 = 3.50;
const DEFAULT_Q_EXTREME_MAX_STALE_MS: i64 = 1_500;

pub(crate) const HIGH_PRICE_EARLY_Q_SATURATION_REASON: &str =
    "blocked_high_price_early_q_saturation";
pub(crate) const HIGH_PRICE_EARLY_REVERSAL_GAP_REASON: &str =
    "blocked_high_price_early_reversal_gap";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvHighPriceEarlyReversalConfig {
    pub(crate) enabled: bool,
    pub(crate) ref_threshold: f64,
    pub(crate) remaining_sec: f64,
    pub(crate) max_stale_ms: i64,
    pub(crate) stale_gap_add: f64,
    pub(crate) binance_missing_gap_add: f64,
    pub(crate) q_extreme: f64,
    pub(crate) q_extreme_min_gap_strength: f64,
    pub(crate) q_extreme_max_stale_ms: i64,
    pub(crate) q_extreme_require_binance_q: bool,
    pub(crate) q_extreme_require_clean_strong_cex: bool,
}

impl Default for PriceToBeatIvHighPriceEarlyReversalConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ref_threshold: DEFAULT_REF_THRESHOLD,
            remaining_sec: DEFAULT_REMAINING_SEC,
            max_stale_ms: DEFAULT_MAX_STALE_MS,
            stale_gap_add: DEFAULT_STALE_GAP_ADD,
            binance_missing_gap_add: DEFAULT_BINANCE_MISSING_GAP_ADD,
            q_extreme: DEFAULT_Q_EXTREME,
            q_extreme_min_gap_strength: DEFAULT_Q_EXTREME_MIN_GAP_STRENGTH,
            q_extreme_max_stale_ms: DEFAULT_Q_EXTREME_MAX_STALE_MS,
            q_extreme_require_binance_q: true,
            q_extreme_require_clean_strong_cex: true,
        }
    }
}

pub(crate) struct PriceToBeatIvHighPriceEarlyReversalInput<'a> {
    pub(crate) config: &'a PriceToBeatIvHighPriceEarlyReversalConfig,
    pub(crate) decision_ref: Option<f64>,
    pub(crate) seconds_left: Option<f64>,
    pub(crate) q_final: Option<f64>,
    pub(crate) q_binance: Option<f64>,
    pub(crate) binance_fail_open: bool,
    pub(crate) chainlink_staleness_ms: Option<i64>,
    pub(crate) gap_strength: Option<f64>,
    pub(crate) base_required_gap_strength: f64,
    pub(crate) cex_consensus: Option<CexOpenGapConsensus>,
    pub(crate) cex_clean_lane: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvHighPriceEarlyReversalEvaluation {
    pub(crate) enabled: bool,
    pub(crate) applies: bool,
    pub(crate) result: &'static str,
    pub(crate) reasons: Vec<&'static str>,
    pub(crate) block_reason: Option<&'static str>,
    pub(crate) decision_ref: Option<f64>,
    pub(crate) seconds_left: Option<f64>,
    pub(crate) q_final: Option<f64>,
    pub(crate) q_extreme_active: bool,
    pub(crate) q_binance_available: bool,
    pub(crate) chainlink_staleness_ms: Option<i64>,
    pub(crate) cex_consensus: Option<&'static str>,
    pub(crate) cex_clean: Option<bool>,
    pub(crate) base_required_gap_strength: Option<f64>,
    pub(crate) stale_gap_add_applied: f64,
    pub(crate) binance_missing_gap_add_applied: f64,
    pub(crate) effective_required_gap_strength: Option<f64>,
    pub(crate) q_extreme_requirements_met: Option<bool>,
}

impl PriceToBeatIvHighPriceEarlyReversalEvaluation {
    pub(crate) fn off() -> Self {
        Self {
            enabled: false,
            applies: false,
            result: "off",
            reasons: Vec::new(),
            block_reason: None,
            decision_ref: None,
            seconds_left: None,
            q_final: None,
            q_extreme_active: false,
            q_binance_available: false,
            chainlink_staleness_ms: None,
            cex_consensus: None,
            cex_clean: None,
            base_required_gap_strength: None,
            stale_gap_add_applied: 0.0,
            binance_missing_gap_add_applied: 0.0,
            effective_required_gap_strength: None,
            q_extreme_requirements_met: None,
        }
    }

    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert(
            "high_price_early_guard_enabled".to_string(),
            json!(self.enabled),
        );
        obj.insert("high_price_early_applies".to_string(), json!(self.applies));
        obj.insert(
            "high_price_early_guard_result".to_string(),
            json!(self.result),
        );
        obj.insert(
            "high_price_early_guard_reasons".to_string(),
            json!(self.reasons),
        );
        obj.insert(
            "high_price_early_decision_ref_cent".to_string(),
            json!(self.decision_ref.map(|value| value * 100.0)),
        );
        obj.insert(
            "high_price_early_seconds_left".to_string(),
            json!(self.seconds_left),
        );
        obj.insert("high_price_early_q_final".to_string(), json!(self.q_final));
        obj.insert(
            "high_price_early_base_required_gap_strength".to_string(),
            json!(self.base_required_gap_strength),
        );
        obj.insert(
            "high_price_early_stale_gap_add_applied".to_string(),
            json!(self.stale_gap_add_applied),
        );
        obj.insert(
            "high_price_early_binance_missing_gap_add_applied".to_string(),
            json!(self.binance_missing_gap_add_applied),
        );
        obj.insert(
            "high_price_early_effective_required_gap_strength".to_string(),
            json!(self.effective_required_gap_strength),
        );
        obj.insert(
            "high_price_early_q_extreme".to_string(),
            json!(self.q_extreme_active),
        );
        obj.insert(
            "high_price_early_q_binance_available".to_string(),
            json!(self.q_binance_available),
        );
        obj.insert(
            "high_price_early_chainlink_staleness_ms".to_string(),
            json!(self.chainlink_staleness_ms),
        );
        obj.insert(
            "high_price_early_cex_consensus".to_string(),
            json!(self.cex_consensus),
        );
        obj.insert(
            "high_price_early_cex_clean".to_string(),
            json!(self.cex_clean),
        );
        obj.insert(
            "high_price_early_q_extreme_requirements_met".to_string(),
            json!(self.q_extreme_requirements_met),
        );
    }
}

pub(crate) fn evaluate_price_to_beat_iv_high_price_early_reversal(
    input: PriceToBeatIvHighPriceEarlyReversalInput<'_>,
) -> PriceToBeatIvHighPriceEarlyReversalEvaluation {
    if !input.config.enabled {
        return PriceToBeatIvHighPriceEarlyReversalEvaluation::off();
    }

    let mut evaluation = PriceToBeatIvHighPriceEarlyReversalEvaluation {
        enabled: true,
        decision_ref: input.decision_ref,
        seconds_left: input.seconds_left,
        q_final: input.q_final,
        q_binance_available: input.q_binance.is_some(),
        chainlink_staleness_ms: input.chainlink_staleness_ms,
        cex_consensus: input.cex_consensus.map(CexOpenGapConsensus::as_str),
        cex_clean: input.cex_clean_lane,
        base_required_gap_strength: Some(input.base_required_gap_strength),
        effective_required_gap_strength: Some(input.base_required_gap_strength),
        ..PriceToBeatIvHighPriceEarlyReversalEvaluation::off()
    };

    let applies = input
        .decision_ref
        .filter(|value| value.is_finite())
        .map(|value| value >= input.config.ref_threshold.clamp(0.0, 1.0))
        .unwrap_or(false)
        && input
            .seconds_left
            .filter(|value| value.is_finite())
            .map(|value| value >= input.config.remaining_sec.max(0.0))
            .unwrap_or(false);

    if !applies {
        evaluation.result = "not_applicable";
        return evaluation;
    }
    evaluation.applies = true;

    let stale_gap_add = if input
        .chainlink_staleness_ms
        .map(|value| value > input.config.max_stale_ms.max(0))
        .unwrap_or(false)
    {
        input.config.stale_gap_add.max(0.0)
    } else {
        0.0
    };
    let binance_missing_gap_add = if input.binance_fail_open || input.q_binance.is_none() {
        input.config.binance_missing_gap_add.max(0.0)
    } else {
        0.0
    };
    evaluation.stale_gap_add_applied = stale_gap_add;
    evaluation.binance_missing_gap_add_applied = binance_missing_gap_add;

    let with_adds = input.base_required_gap_strength + stale_gap_add + binance_missing_gap_add;
    let q_extreme_active = input
        .q_final
        .filter(|value| value.is_finite())
        .map(|value| value >= input.config.q_extreme.clamp(0.0, 1.0))
        .unwrap_or(false);
    evaluation.q_extreme_active = q_extreme_active;
    let q_extreme_floor = input.config.q_extreme_min_gap_strength.max(0.0);
    let effective_required_gap_strength = if q_extreme_active {
        with_adds.max(q_extreme_floor)
    } else {
        with_adds
    };
    evaluation.effective_required_gap_strength = Some(effective_required_gap_strength);

    if q_extreme_active {
        evaluation.reasons.push("q_extreme");
        if input.config.q_extreme_require_binance_q && input.q_binance.is_none() {
            evaluation.reasons.push("q_binance_missing");
            if input.binance_fail_open {
                evaluation.reasons.push("binance_fail_open_unavailable");
            }
        }
        match input.chainlink_staleness_ms {
            Some(staleness_ms) if staleness_ms > input.config.q_extreme_max_stale_ms.max(0) => {
                evaluation.reasons.push("stale_above_q_extreme_max");
            }
            None => evaluation.reasons.push("stale_unknown"),
            _ => {}
        }
        if input.config.q_extreme_require_clean_strong_cex {
            if input.cex_consensus != Some(CexOpenGapConsensus::Strong) {
                evaluation.reasons.push("cex_not_strong");
            }
            if input.cex_clean_lane != Some(true) {
                evaluation.reasons.push("cex_not_clean");
            }
        }
        match input.gap_strength {
            Some(gap_strength) if gap_strength < q_extreme_floor => {
                evaluation.reasons.push("gap_below_q_extreme_min");
            }
            None => evaluation.reasons.push("gap_strength_unknown"),
            _ => {}
        }

        let requirements_met = evaluation.reasons.len() == 1;
        evaluation.q_extreme_requirements_met = Some(requirements_met);
        if !requirements_met {
            evaluation.result = HIGH_PRICE_EARLY_Q_SATURATION_REASON;
            evaluation.block_reason = Some(HIGH_PRICE_EARLY_Q_SATURATION_REASON);
            return evaluation;
        }
    }

    evaluation.result = "pass";
    evaluation
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> PriceToBeatIvHighPriceEarlyReversalConfig {
        PriceToBeatIvHighPriceEarlyReversalConfig {
            enabled: true,
            ..PriceToBeatIvHighPriceEarlyReversalConfig::default()
        }
    }

    fn input<'a>(
        config: &'a PriceToBeatIvHighPriceEarlyReversalConfig,
    ) -> PriceToBeatIvHighPriceEarlyReversalInput<'a> {
        PriceToBeatIvHighPriceEarlyReversalInput {
            config,
            decision_ref: Some(0.78),
            seconds_left: Some(161.81),
            q_final: Some(0.999),
            q_binance: None,
            binance_fail_open: true,
            chainlink_staleness_ms: Some(2_190),
            gap_strength: Some(3.1287),
            base_required_gap_strength: 2.50,
            cex_consensus: Some(CexOpenGapConsensus::Strong),
            cex_clean_lane: Some(true),
        }
    }

    #[test]
    fn high_price_early_blocks_eth_q_saturation_example() {
        let config = config();
        let evaluation = evaluate_price_to_beat_iv_high_price_early_reversal(input(&config));

        assert!(evaluation.applies);
        assert_eq!(evaluation.effective_required_gap_strength, Some(3.50));
        assert_eq!(
            evaluation.block_reason,
            Some(HIGH_PRICE_EARLY_Q_SATURATION_REASON)
        );
        assert!(evaluation.reasons.contains(&"q_extreme"));
        assert!(evaluation.reasons.contains(&"q_binance_missing"));
        assert!(evaluation
            .reasons
            .contains(&"binance_fail_open_unavailable"));
        assert!(evaluation.reasons.contains(&"stale_above_q_extreme_max"));
        assert!(evaluation.reasons.contains(&"gap_below_q_extreme_min"));
    }

    #[test]
    fn high_price_early_non_extreme_q_uses_additive_gap_only() {
        let config = config();
        let evaluation = evaluate_price_to_beat_iv_high_price_early_reversal(
            PriceToBeatIvHighPriceEarlyReversalInput {
                q_final: Some(0.96),
                gap_strength: Some(3.0),
                ..input(&config)
            },
        );

        assert!(evaluation.applies);
        assert!(!evaluation.q_extreme_active);
        assert_eq!(evaluation.effective_required_gap_strength, Some(3.15));
        assert_eq!(evaluation.result, "pass");
        assert_eq!(evaluation.block_reason, None);
    }

    #[test]
    fn high_price_early_extreme_q_passes_with_clean_confirmation() {
        let config = config();
        let evaluation = evaluate_price_to_beat_iv_high_price_early_reversal(
            PriceToBeatIvHighPriceEarlyReversalInput {
                q_binance: Some(0.99),
                binance_fail_open: false,
                chainlink_staleness_ms: Some(1_200),
                gap_strength: Some(3.70),
                ..input(&config)
            },
        );

        assert!(evaluation.applies);
        assert!(evaluation.q_extreme_active);
        assert_eq!(evaluation.q_extreme_requirements_met, Some(true));
        assert_eq!(evaluation.result, "pass");
        assert_eq!(evaluation.block_reason, None);
    }

    #[test]
    fn high_price_early_stays_off_below_ref_or_time() {
        let config = config();
        let below_ref = evaluate_price_to_beat_iv_high_price_early_reversal(
            PriceToBeatIvHighPriceEarlyReversalInput {
                decision_ref: Some(0.76),
                ..input(&config)
            },
        );
        let late = evaluate_price_to_beat_iv_high_price_early_reversal(
            PriceToBeatIvHighPriceEarlyReversalInput {
                seconds_left: Some(119.0),
                ..input(&config)
            },
        );

        assert_eq!(below_ref.result, "not_applicable");
        assert!(!below_ref.applies);
        assert_eq!(late.result, "not_applicable");
        assert!(!late.applies);
    }
}
