use super::iv_cex_open_gap::{CexOpenGapConsensus, PriceToBeatIvCexOpenGapEvaluation};
use super::iv_execution_vwap::PriceToBeatIvExecutionVwapEvaluation;
use serde_json::{json, Map, Value};

const DEFAULT_BOOK_MAX_EXECUTION_REF: f64 = 0.70;
const DEFAULT_RAW_BOOK_DISLOCATION: f64 = 0.20;
const DEFAULT_MIXED_CEX_MAX_SECONDS: f64 = 60.0;
const DEFAULT_LATE_EXPENSIVE_SECONDS: f64 = 45.0;
const DEFAULT_LATE_EXPENSIVE_MIN_VWAP: f64 = 0.70;
const DEFAULT_NO_BOOK_MAX_SECONDS: f64 = 45.0;
const BOOK_MISMATCH_BLOCK_REASON: &str = "blocked_gap_fail_chainlink_cex_book_mismatch";
const MIXED_CEX_GAP_FAIL_BLOCK_REASON: &str = "blocked_chainlink_cex_mixed_gap_fail";
const LATE_EXPENSIVE_MIXED_CEX_BLOCK_REASON: &str = "blocked_late_expensive_mixed_cex";
const LAG_NO_BOOK_BLOCK_REASON: &str = "blocked_chainlink_cex_lag_no_book_confirmation";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvGapFailCexBookGuardConfig {
    pub(crate) enabled: bool,
    pub(crate) book_max_execution_ref: f64,
    pub(crate) raw_book_dislocation: f64,
    pub(crate) mixed_cex_guard_enabled: bool,
    pub(crate) mixed_cex_max_seconds: f64,
    pub(crate) late_expensive_mixed_cex_guard_enabled: bool,
    pub(crate) late_expensive_mixed_cex_seconds: f64,
    pub(crate) late_expensive_mixed_cex_min_vwap: f64,
    pub(crate) late_expensive_mixed_cex_require_gap_fail_or_lag_high: bool,
    pub(crate) chainlink_cex_lag_no_book_guard_enabled: bool,
    pub(crate) chainlink_cex_lag_no_book_max_seconds: f64,
    pub(crate) chainlink_cex_lag_no_book_require_non_strong_cex: bool,
}

impl Default for PriceToBeatIvGapFailCexBookGuardConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            book_max_execution_ref: DEFAULT_BOOK_MAX_EXECUTION_REF,
            raw_book_dislocation: DEFAULT_RAW_BOOK_DISLOCATION,
            mixed_cex_guard_enabled: false,
            mixed_cex_max_seconds: DEFAULT_MIXED_CEX_MAX_SECONDS,
            late_expensive_mixed_cex_guard_enabled: false,
            late_expensive_mixed_cex_seconds: DEFAULT_LATE_EXPENSIVE_SECONDS,
            late_expensive_mixed_cex_min_vwap: DEFAULT_LATE_EXPENSIVE_MIN_VWAP,
            late_expensive_mixed_cex_require_gap_fail_or_lag_high: true,
            chainlink_cex_lag_no_book_guard_enabled: false,
            chainlink_cex_lag_no_book_max_seconds: DEFAULT_NO_BOOK_MAX_SECONDS,
            chainlink_cex_lag_no_book_require_non_strong_cex: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvGapFailCexBookGuardEvaluation {
    pub(crate) enabled: bool,
    pub(crate) below_required: bool,
    pub(crate) cex_lag_high: bool,
    pub(crate) execution_ref: Option<f64>,
    pub(crate) execution_ref_source: Option<&'static str>,
    pub(crate) book_ref_low: bool,
    pub(crate) raw_book_dislocation: Option<f64>,
    pub(crate) raw_book_dislocation_high: bool,
    pub(crate) book_max_execution_ref: f64,
    pub(crate) raw_book_dislocation_threshold: f64,
    pub(crate) mixed_cex_guard_enabled: bool,
    pub(crate) mixed_cex_max_seconds: f64,
    pub(crate) mixed_cex_triggered: bool,
    pub(crate) late_expensive_mixed_cex_guard_enabled: bool,
    pub(crate) late_expensive_entry: bool,
    pub(crate) late_expensive_seconds: f64,
    pub(crate) late_expensive_min_vwap: f64,
    pub(crate) late_expensive_mixed_cex_triggered: bool,
    pub(crate) chainlink_cex_lag_no_book_guard_enabled: bool,
    pub(crate) chainlink_cex_lag_no_book_max_seconds: f64,
    pub(crate) book_confirmation_available: bool,
    pub(crate) book_confirmation_missing: bool,
    pub(crate) chainlink_cex_lag_no_book_triggered: bool,
    pub(crate) seconds_left: f64,
    pub(crate) gap_strength: f64,
    pub(crate) required_gap_strength: f64,
    pub(crate) cex_consensus: CexOpenGapConsensus,
    pub(crate) action: &'static str,
    pub(crate) block_reason: Option<&'static str>,
    pub(crate) all_reasons: Vec<&'static str>,
}

impl Default for PriceToBeatIvGapFailCexBookGuardEvaluation {
    fn default() -> Self {
        let config = PriceToBeatIvGapFailCexBookGuardConfig::default();
        Self {
            enabled: false,
            below_required: false,
            cex_lag_high: false,
            execution_ref: None,
            execution_ref_source: None,
            book_ref_low: false,
            raw_book_dislocation: None,
            raw_book_dislocation_high: false,
            book_max_execution_ref: config.book_max_execution_ref,
            raw_book_dislocation_threshold: config.raw_book_dislocation,
            mixed_cex_guard_enabled: false,
            mixed_cex_max_seconds: config.mixed_cex_max_seconds,
            mixed_cex_triggered: false,
            late_expensive_mixed_cex_guard_enabled: false,
            late_expensive_entry: false,
            late_expensive_seconds: config.late_expensive_mixed_cex_seconds,
            late_expensive_min_vwap: config.late_expensive_mixed_cex_min_vwap,
            late_expensive_mixed_cex_triggered: false,
            chainlink_cex_lag_no_book_guard_enabled: false,
            chainlink_cex_lag_no_book_max_seconds: config.chainlink_cex_lag_no_book_max_seconds,
            book_confirmation_available: false,
            book_confirmation_missing: true,
            chainlink_cex_lag_no_book_triggered: false,
            seconds_left: 0.0,
            gap_strength: 0.0,
            required_gap_strength: 0.0,
            cex_consensus: CexOpenGapConsensus::Unavailable,
            action: "off",
            block_reason: None,
            all_reasons: Vec::new(),
        }
    }
}

impl PriceToBeatIvGapFailCexBookGuardEvaluation {
    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert(
            "gap_fail_cex_book_guard_enabled".to_string(),
            json!(self.enabled),
        );
        obj.insert("gap_fail_cex_book_action".to_string(), json!(self.action));
        obj.insert(
            "gap_fail_cex_book_block_reason".to_string(),
            json!(self.block_reason),
        );
        obj.insert(
            "gap_fail_below_required".to_string(),
            json!(self.below_required),
        );
        obj.insert(
            "gap_fail_cex_lag_high".to_string(),
            json!(self.cex_lag_high),
        );
        obj.insert(
            "gap_fail_execution_ref_cent".to_string(),
            json!(self.execution_ref.map(|value| value * 100.0)),
        );
        obj.insert(
            "gap_fail_execution_ref_source".to_string(),
            json!(self.execution_ref_source),
        );
        obj.insert(
            "gap_fail_book_ref_low".to_string(),
            json!(self.book_ref_low),
        );
        obj.insert(
            "gap_fail_raw_book_dislocation_cent".to_string(),
            json!(self.raw_book_dislocation.map(|value| value * 100.0)),
        );
        obj.insert(
            "gap_fail_raw_book_dislocation_high".to_string(),
            json!(self.raw_book_dislocation_high),
        );
        obj.insert(
            "gap_fail_book_max_execution_ref_cent".to_string(),
            json!(self.book_max_execution_ref * 100.0),
        );
        obj.insert(
            "gap_fail_raw_book_dislocation_threshold_cent".to_string(),
            json!(self.raw_book_dislocation_threshold * 100.0),
        );
        obj.insert(
            "gap_fail_mixed_cex_guard_enabled".to_string(),
            json!(self.mixed_cex_guard_enabled),
        );
        obj.insert(
            "gap_fail_mixed_cex_max_seconds".to_string(),
            json!(self.mixed_cex_max_seconds),
        );
        obj.insert(
            "gap_fail_mixed_cex_triggered".to_string(),
            json!(self.mixed_cex_triggered),
        );
        obj.insert(
            "late_expensive_mixed_cex_guard_enabled".to_string(),
            json!(self.late_expensive_mixed_cex_guard_enabled),
        );
        obj.insert(
            "late_expensive_entry".to_string(),
            json!(self.late_expensive_entry),
        );
        obj.insert(
            "late_expensive_seconds".to_string(),
            json!(self.late_expensive_seconds),
        );
        obj.insert(
            "late_expensive_min_vwap_cent".to_string(),
            json!(self.late_expensive_min_vwap * 100.0),
        );
        obj.insert(
            "late_expensive_mixed_cex_triggered".to_string(),
            json!(self.late_expensive_mixed_cex_triggered),
        );
        obj.insert(
            "chainlink_cex_lag_no_book_guard_enabled".to_string(),
            json!(self.chainlink_cex_lag_no_book_guard_enabled),
        );
        obj.insert(
            "chainlink_cex_lag_no_book_max_seconds".to_string(),
            json!(self.chainlink_cex_lag_no_book_max_seconds),
        );
        obj.insert(
            "book_confirmation_available".to_string(),
            json!(self.book_confirmation_available),
        );
        obj.insert(
            "book_confirmation_missing".to_string(),
            json!(self.book_confirmation_missing),
        );
        obj.insert(
            "chainlink_cex_lag_no_book_triggered".to_string(),
            json!(self.chainlink_cex_lag_no_book_triggered),
        );
        obj.insert("gap_fail".to_string(), json!(self.below_required));
        obj.insert("seconds_left".to_string(), json!(self.seconds_left));
        obj.insert("gap_strength".to_string(), json!(self.gap_strength));
        obj.insert(
            "required_gap_strength".to_string(),
            json!(self.required_gap_strength),
        );
        obj.insert(
            "cex_consensus".to_string(),
            json!(self.cex_consensus.as_str()),
        );
        obj.insert("lag_high".to_string(), json!(self.cex_lag_high));
        obj.insert(
            "gap_fail_cex_book_reasons".to_string(),
            json!(self.all_reasons),
        );
    }
}

pub(crate) struct PriceToBeatIvGapFailCexBookGuardInput<'a> {
    pub(crate) config: PriceToBeatIvGapFailCexBookGuardConfig,
    pub(crate) seconds_left: f64,
    pub(crate) gap_strength: f64,
    pub(crate) required_gap_strength: f64,
    pub(crate) q_raw: f64,
    pub(crate) book_confirmation_available: bool,
    pub(crate) cex_open_gap: &'a PriceToBeatIvCexOpenGapEvaluation,
    pub(crate) execution_vwap: &'a PriceToBeatIvExecutionVwapEvaluation,
}

pub(crate) fn evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
    input: PriceToBeatIvGapFailCexBookGuardInput<'_>,
) -> PriceToBeatIvGapFailCexBookGuardEvaluation {
    let (execution_ref_source, execution_ref) = execution_ref(input.execution_vwap);
    let below_required = input.gap_strength < input.required_gap_strength;
    let cex_lag_high = input.cex_open_gap.lag_high;
    let book_ref_low = execution_ref
        .map(|value| value <= input.config.book_max_execution_ref)
        .unwrap_or(false);
    let raw_book_dislocation = execution_ref.map(|reference| input.q_raw - reference);
    let raw_book_dislocation_high = raw_book_dislocation
        .map(|value| value >= input.config.raw_book_dislocation)
        .unwrap_or(false);
    let book_mismatch_triggered = input.config.enabled
        && below_required
        && cex_lag_high
        && book_ref_low
        && raw_book_dislocation_high;
    let consensus = input.cex_open_gap.consensus;
    let cex_against = consensus == CexOpenGapConsensus::Against;
    let non_strong_cex = consensus != CexOpenGapConsensus::Strong;
    let mixed_cex_triggered = input.config.mixed_cex_guard_enabled
        && input.seconds_left <= input.config.mixed_cex_max_seconds.max(0.0)
        && below_required
        && cex_lag_high
        && non_strong_cex
        && !cex_against;
    let execution_ref_for_late = input
        .execution_vwap
        .execution_vwap
        .or(input.execution_vwap.execution_best_ask)
        .or(execution_ref);
    let late_expensive_entry = input.seconds_left < input.config.late_expensive_mixed_cex_seconds
        && execution_ref_for_late
            .map(|value| value >= input.config.late_expensive_mixed_cex_min_vwap)
            .unwrap_or(false);
    let late_expensive_mixed_cex_triggered = input.config.late_expensive_mixed_cex_guard_enabled
        && late_expensive_entry
        && non_strong_cex
        && !cex_against
        && (!input
            .config
            .late_expensive_mixed_cex_require_gap_fail_or_lag_high
            || below_required
            || cex_lag_high);
    let book_confirmation_available = input.book_confirmation_available;
    let book_confirmation_missing = !book_confirmation_available;
    let no_book_cex_ok = if input
        .config
        .chainlink_cex_lag_no_book_require_non_strong_cex
    {
        non_strong_cex && !cex_against
    } else {
        !cex_against
    };
    let chainlink_cex_lag_no_book_triggered = input.config.chainlink_cex_lag_no_book_guard_enabled
        && input.seconds_left <= input.config.chainlink_cex_lag_no_book_max_seconds.max(0.0)
        && cex_lag_high
        && no_book_cex_ok
        && book_confirmation_missing
        && (below_required || late_expensive_entry);
    let mut all_reasons = Vec::new();
    if mixed_cex_triggered {
        all_reasons.push(MIXED_CEX_GAP_FAIL_BLOCK_REASON);
    }
    if late_expensive_mixed_cex_triggered {
        all_reasons.push(LATE_EXPENSIVE_MIXED_CEX_BLOCK_REASON);
    }
    if chainlink_cex_lag_no_book_triggered {
        all_reasons.push(LAG_NO_BOOK_BLOCK_REASON);
    }
    if book_mismatch_triggered {
        all_reasons.push(BOOK_MISMATCH_BLOCK_REASON);
    }
    let block_reason = all_reasons.first().copied();
    let any_guard_enabled = input.config.enabled
        || input.config.mixed_cex_guard_enabled
        || input.config.late_expensive_mixed_cex_guard_enabled
        || input.config.chainlink_cex_lag_no_book_guard_enabled;

    PriceToBeatIvGapFailCexBookGuardEvaluation {
        enabled: any_guard_enabled,
        below_required,
        cex_lag_high,
        execution_ref,
        execution_ref_source,
        book_ref_low,
        raw_book_dislocation,
        raw_book_dislocation_high,
        book_max_execution_ref: input.config.book_max_execution_ref,
        raw_book_dislocation_threshold: input.config.raw_book_dislocation,
        mixed_cex_guard_enabled: input.config.mixed_cex_guard_enabled,
        mixed_cex_max_seconds: input.config.mixed_cex_max_seconds,
        mixed_cex_triggered,
        late_expensive_mixed_cex_guard_enabled: input.config.late_expensive_mixed_cex_guard_enabled,
        late_expensive_entry,
        late_expensive_seconds: input.config.late_expensive_mixed_cex_seconds,
        late_expensive_min_vwap: input.config.late_expensive_mixed_cex_min_vwap,
        late_expensive_mixed_cex_triggered,
        chainlink_cex_lag_no_book_guard_enabled: input
            .config
            .chainlink_cex_lag_no_book_guard_enabled,
        chainlink_cex_lag_no_book_max_seconds: input.config.chainlink_cex_lag_no_book_max_seconds,
        book_confirmation_available,
        book_confirmation_missing,
        chainlink_cex_lag_no_book_triggered,
        seconds_left: input.seconds_left,
        gap_strength: input.gap_strength,
        required_gap_strength: input.required_gap_strength,
        cex_consensus: consensus,
        action: if block_reason.is_some() {
            "block"
        } else if any_guard_enabled {
            "pass"
        } else {
            "off"
        },
        block_reason,
        all_reasons,
    }
}

fn execution_ref(
    execution_vwap: &PriceToBeatIvExecutionVwapEvaluation,
) -> (Option<&'static str>, Option<f64>) {
    if let Some(value) = execution_vwap.execution_best_ask {
        return (Some("execution_best_ask"), Some(value));
    }
    if let Some(value) = execution_vwap.execution_vwap {
        return (Some("execution_vwap"), Some(value));
    }
    if let Some(value) = execution_vwap.model_ask {
        return (Some("model_ask"), Some(value));
    }
    (None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_btc_gap_fail_with_chainlink_cex_and_book_mismatch() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            lag_high: true,
            chainlink_cex_diff_usd: Some(27.40),
            chainlink_cex_diff_bps: Some(4.43),
            chainlink_cex_diff_z: Some(0.98),
            ..Default::default()
        };
        let execution_vwap = PriceToBeatIvExecutionVwapEvaluation {
            execution_best_ask: Some(0.60),
            execution_vwap: Some(0.53),
            model_ask: Some(0.60),
            ..Default::default()
        };

        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    enabled: true,
                    ..Default::default()
                },
                seconds_left: 52.0,
                gap_strength: 1.5869,
                required_gap_strength: 2.0,
                q_raw: 0.9437,
                book_confirmation_available: true,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &execution_vwap,
            },
        );

        assert_eq!(
            evaluation.block_reason,
            Some("blocked_gap_fail_chainlink_cex_book_mismatch")
        );
        assert_eq!(evaluation.action, "block");
        assert_eq!(evaluation.execution_ref, Some(0.60));
        assert!(evaluation.raw_book_dislocation_high);
    }

    #[test]
    fn preserves_clean_soft_low_gap_without_cex_or_book_mismatch() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            lag_high: false,
            ..Default::default()
        };
        let execution_vwap = PriceToBeatIvExecutionVwapEvaluation {
            execution_best_ask: Some(0.74),
            model_ask: Some(0.74),
            ..Default::default()
        };

        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    enabled: true,
                    ..Default::default()
                },
                seconds_left: 52.0,
                gap_strength: 1.95,
                required_gap_strength: 2.0,
                q_raw: 0.78,
                book_confirmation_available: true,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &execution_vwap,
            },
        );

        assert_eq!(evaluation.block_reason, None);
        assert_eq!(evaluation.action, "pass");
    }

    #[test]
    fn blocks_eth_gap_fail_at_twenty_cent_raw_book_dislocation() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            lag_high: true,
            chainlink_cex_diff_usd: Some(0.653),
            chainlink_cex_diff_bps: Some(3.89),
            chainlink_cex_diff_z: Some(0.271),
            ..Default::default()
        };
        let execution_vwap = PriceToBeatIvExecutionVwapEvaluation {
            execution_best_ask: Some(0.63),
            execution_vwap: Some(0.63),
            model_ask: Some(0.65),
            ..Default::default()
        };

        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    enabled: true,
                    ..Default::default()
                },
                seconds_left: 52.0,
                gap_strength: 1.041,
                required_gap_strength: 2.0,
                q_raw: 0.8511,
                book_confirmation_available: true,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &execution_vwap,
            },
        );

        assert_eq!(
            evaluation.block_reason,
            Some("blocked_gap_fail_chainlink_cex_book_mismatch")
        );
        assert_eq!(evaluation.action, "block");
        assert!(
            (evaluation.raw_book_dislocation.unwrap_or_default() - 0.2211).abs() < f64::EPSILON
        );
        assert!(evaluation.raw_book_dislocation_high);
    }

    #[test]
    fn mixed_cex_gap_fail_blocks_lag_high_non_strong() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Mixed,
            lag_high: true,
            ..Default::default()
        };
        let execution_vwap = PriceToBeatIvExecutionVwapEvaluation {
            execution_best_ask: Some(0.75),
            execution_vwap: Some(0.73),
            ..Default::default()
        };

        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    mixed_cex_guard_enabled: true,
                    mixed_cex_max_seconds: 120.0,
                    ..Default::default()
                },
                seconds_left: 40.39,
                gap_strength: 1.3122,
                required_gap_strength: 1.55,
                q_raw: 0.9053,
                book_confirmation_available: false,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &execution_vwap,
            },
        );

        assert_eq!(
            evaluation.block_reason,
            Some("blocked_chainlink_cex_mixed_gap_fail")
        );
        assert_eq!(evaluation.action, "block");
    }

    #[test]
    fn mixed_cex_gap_fail_does_not_block_strong_consensus() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Strong,
            lag_high: true,
            ..Default::default()
        };
        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    mixed_cex_guard_enabled: true,
                    mixed_cex_max_seconds: 120.0,
                    ..Default::default()
                },
                seconds_left: 40.0,
                gap_strength: 1.3,
                required_gap_strength: 1.55,
                q_raw: 0.90,
                book_confirmation_available: false,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &PriceToBeatIvExecutionVwapEvaluation::default(),
            },
        );

        assert_eq!(evaluation.block_reason, None);
        assert_eq!(evaluation.action, "pass");
    }

    #[test]
    fn mixed_cex_gap_fail_does_not_block_without_lag_high() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Mixed,
            lag_high: false,
            ..Default::default()
        };
        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    mixed_cex_guard_enabled: true,
                    mixed_cex_max_seconds: 120.0,
                    ..Default::default()
                },
                seconds_left: 40.0,
                gap_strength: 1.3,
                required_gap_strength: 1.55,
                q_raw: 0.90,
                book_confirmation_available: false,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &PriceToBeatIvExecutionVwapEvaluation::default(),
            },
        );

        assert_eq!(evaluation.block_reason, None);
    }

    #[test]
    fn mixed_cex_gap_fail_does_not_block_when_gap_passes() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Mixed,
            lag_high: true,
            ..Default::default()
        };
        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    mixed_cex_guard_enabled: true,
                    mixed_cex_max_seconds: 120.0,
                    ..Default::default()
                },
                seconds_left: 40.0,
                gap_strength: 1.55,
                required_gap_strength: 1.55,
                q_raw: 0.90,
                book_confirmation_available: false,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &PriceToBeatIvExecutionVwapEvaluation::default(),
            },
        );

        assert_eq!(evaluation.block_reason, None);
    }

    #[test]
    fn mixed_cex_gap_fail_does_not_block_after_window() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Mixed,
            lag_high: true,
            ..Default::default()
        };
        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    mixed_cex_guard_enabled: true,
                    mixed_cex_max_seconds: 120.0,
                    ..Default::default()
                },
                seconds_left: 121.0,
                gap_strength: 1.3,
                required_gap_strength: 1.55,
                q_raw: 0.90,
                book_confirmation_available: false,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &PriceToBeatIvExecutionVwapEvaluation::default(),
            },
        );

        assert_eq!(evaluation.block_reason, None);
    }

    #[test]
    fn mixed_cex_gap_fail_leaves_against_consensus_to_cex_open_gap() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Against,
            lag_high: true,
            ..Default::default()
        };
        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    mixed_cex_guard_enabled: true,
                    mixed_cex_max_seconds: 120.0,
                    ..Default::default()
                },
                seconds_left: 40.0,
                gap_strength: 1.3,
                required_gap_strength: 1.55,
                q_raw: 0.90,
                book_confirmation_available: false,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &PriceToBeatIvExecutionVwapEvaluation::default(),
            },
        );

        assert_eq!(evaluation.block_reason, None);
    }

    #[test]
    fn late_expensive_mixed_cex_blocks_when_gap_fail_or_lag_high() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Mixed,
            lag_high: false,
            ..Default::default()
        };
        let execution_vwap = PriceToBeatIvExecutionVwapEvaluation {
            execution_vwap: Some(0.73),
            ..Default::default()
        };
        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    late_expensive_mixed_cex_guard_enabled: true,
                    late_expensive_mixed_cex_seconds: 45.0,
                    late_expensive_mixed_cex_min_vwap: 0.70,
                    late_expensive_mixed_cex_require_gap_fail_or_lag_high: true,
                    ..Default::default()
                },
                seconds_left: 40.0,
                gap_strength: 1.3,
                required_gap_strength: 1.55,
                q_raw: 0.90,
                book_confirmation_available: false,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &execution_vwap,
            },
        );

        assert_eq!(
            evaluation.block_reason,
            Some("blocked_late_expensive_mixed_cex")
        );
    }

    #[test]
    fn late_expensive_mixed_cex_does_not_block_after_window() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Mixed,
            lag_high: true,
            ..Default::default()
        };
        let execution_vwap = PriceToBeatIvExecutionVwapEvaluation {
            execution_vwap: Some(0.73),
            ..Default::default()
        };
        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    late_expensive_mixed_cex_guard_enabled: true,
                    late_expensive_mixed_cex_seconds: 45.0,
                    late_expensive_mixed_cex_min_vwap: 0.70,
                    ..Default::default()
                },
                seconds_left: 46.0,
                gap_strength: 1.3,
                required_gap_strength: 1.55,
                q_raw: 0.90,
                book_confirmation_available: false,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &execution_vwap,
            },
        );

        assert_eq!(evaluation.block_reason, None);
    }

    #[test]
    fn late_expensive_mixed_cex_does_not_block_below_vwap_threshold() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Mixed,
            lag_high: true,
            ..Default::default()
        };
        let execution_vwap = PriceToBeatIvExecutionVwapEvaluation {
            execution_vwap: Some(0.69),
            ..Default::default()
        };
        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    late_expensive_mixed_cex_guard_enabled: true,
                    late_expensive_mixed_cex_seconds: 45.0,
                    late_expensive_mixed_cex_min_vwap: 0.70,
                    ..Default::default()
                },
                seconds_left: 40.0,
                gap_strength: 1.3,
                required_gap_strength: 1.55,
                q_raw: 0.90,
                book_confirmation_available: false,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &execution_vwap,
            },
        );

        assert_eq!(evaluation.block_reason, None);
    }

    #[test]
    fn late_expensive_mixed_cex_does_not_block_strong_consensus() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Strong,
            lag_high: true,
            ..Default::default()
        };
        let execution_vwap = PriceToBeatIvExecutionVwapEvaluation {
            execution_vwap: Some(0.73),
            ..Default::default()
        };
        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    late_expensive_mixed_cex_guard_enabled: true,
                    late_expensive_mixed_cex_seconds: 45.0,
                    late_expensive_mixed_cex_min_vwap: 0.70,
                    ..Default::default()
                },
                seconds_left: 40.0,
                gap_strength: 1.3,
                required_gap_strength: 1.55,
                q_raw: 0.90,
                book_confirmation_available: false,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &execution_vwap,
            },
        );

        assert_eq!(evaluation.block_reason, None);
    }

    #[test]
    fn late_expensive_mixed_cex_respects_require_gap_fail_or_lag_high() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Mixed,
            lag_high: false,
            ..Default::default()
        };
        let execution_vwap = PriceToBeatIvExecutionVwapEvaluation {
            execution_vwap: Some(0.73),
            ..Default::default()
        };
        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    late_expensive_mixed_cex_guard_enabled: true,
                    late_expensive_mixed_cex_seconds: 45.0,
                    late_expensive_mixed_cex_min_vwap: 0.70,
                    late_expensive_mixed_cex_require_gap_fail_or_lag_high: true,
                    ..Default::default()
                },
                seconds_left: 40.0,
                gap_strength: 1.55,
                required_gap_strength: 1.55,
                q_raw: 0.90,
                book_confirmation_available: false,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &execution_vwap,
            },
        );

        assert_eq!(evaluation.block_reason, None);
    }

    #[test]
    fn chainlink_cex_lag_no_book_blocks_when_confirmation_missing() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Mixed,
            lag_high: true,
            ..Default::default()
        };
        let execution_vwap = PriceToBeatIvExecutionVwapEvaluation {
            execution_vwap: Some(0.73),
            ..Default::default()
        };
        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    chainlink_cex_lag_no_book_guard_enabled: true,
                    chainlink_cex_lag_no_book_max_seconds: 45.0,
                    chainlink_cex_lag_no_book_require_non_strong_cex: true,
                    ..Default::default()
                },
                seconds_left: 40.0,
                gap_strength: 1.3,
                required_gap_strength: 1.55,
                q_raw: 0.90,
                book_confirmation_available: false,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &execution_vwap,
            },
        );

        assert_eq!(
            evaluation.block_reason,
            Some("blocked_chainlink_cex_lag_no_book_confirmation")
        );
    }

    #[test]
    fn chainlink_cex_lag_no_book_does_not_block_with_confirmation() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Mixed,
            lag_high: true,
            ..Default::default()
        };
        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    chainlink_cex_lag_no_book_guard_enabled: true,
                    chainlink_cex_lag_no_book_max_seconds: 45.0,
                    ..Default::default()
                },
                seconds_left: 40.0,
                gap_strength: 1.3,
                required_gap_strength: 1.55,
                q_raw: 0.90,
                book_confirmation_available: true,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &PriceToBeatIvExecutionVwapEvaluation::default(),
            },
        );

        assert_eq!(evaluation.block_reason, None);
    }

    #[test]
    fn combined_rules_keep_primary_reason_order_and_all_reasons() {
        let cex_open_gap = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Mixed,
            lag_high: true,
            ..Default::default()
        };
        let execution_vwap = PriceToBeatIvExecutionVwapEvaluation {
            execution_vwap: Some(0.73),
            execution_best_ask: Some(0.75),
            ..Default::default()
        };
        let evaluation = evaluate_price_to_beat_iv_gap_fail_cex_book_guard(
            PriceToBeatIvGapFailCexBookGuardInput {
                config: PriceToBeatIvGapFailCexBookGuardConfig {
                    mixed_cex_guard_enabled: true,
                    mixed_cex_max_seconds: 120.0,
                    late_expensive_mixed_cex_guard_enabled: true,
                    late_expensive_mixed_cex_seconds: 45.0,
                    late_expensive_mixed_cex_min_vwap: 0.70,
                    chainlink_cex_lag_no_book_guard_enabled: true,
                    chainlink_cex_lag_no_book_max_seconds: 45.0,
                    ..Default::default()
                },
                seconds_left: 40.39,
                gap_strength: 1.3122,
                required_gap_strength: 1.55,
                q_raw: 0.9053,
                book_confirmation_available: false,
                cex_open_gap: &cex_open_gap,
                execution_vwap: &execution_vwap,
            },
        );

        assert_eq!(
            evaluation.block_reason,
            Some("blocked_chainlink_cex_mixed_gap_fail")
        );
        assert_eq!(
            evaluation.all_reasons,
            vec![
                "blocked_chainlink_cex_mixed_gap_fail",
                "blocked_late_expensive_mixed_cex",
                "blocked_chainlink_cex_lag_no_book_confirmation"
            ]
        );
    }
}
