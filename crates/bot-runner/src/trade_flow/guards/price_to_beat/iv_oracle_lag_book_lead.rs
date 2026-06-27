use super::iv_cex_open_gap::CexOpenGapConsensus;
use super::iv_execution_vwap::PriceToBeatIvExecutionVwapEvaluation;
use serde_json::{json, Map, Value};

const DEFAULT_EARLY_SECONDS: f64 = 60.0;
const DEFAULT_Q_HIGH: f64 = 0.95;
const DEFAULT_CHEAP_TOKEN: f64 = 0.70;
const DEFAULT_Q_EXTREME: f64 = 0.98;
const DEFAULT_CHEAP_TOKEN_EXTREME: f64 = 0.72;
const DEFAULT_Q_CONSENSUS_MISMATCH: f64 = 0.95;
const DEFAULT_CHEAP_TOKEN_CONSENSUS_MISMATCH: f64 = 0.65;
const DEFAULT_DISLOCATION_WARN: f64 = 0.12;
const DEFAULT_DISLOCATION_HIGH: f64 = 0.20;
const DEFAULT_DISLOCATION_RED: f64 = 0.25;
const DEFAULT_DISLOCATION_CONSENSUS_MISMATCH: f64 = 0.30;
const DEFAULT_MAX_BOOK_AGE_MS: i64 = 1_500;
const DEFAULT_MIN_DEPTH_COVERAGE: f64 = 0.95;
const DEFAULT_BEST_ASK_FALLBACK_MAX_SPREAD: f64 = 0.02;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvOracleLagBookLeadConfig {
    pub(crate) enabled: bool,
    pub(crate) early_seconds: f64,
    pub(crate) q_high: f64,
    pub(crate) cheap_token: f64,
    pub(crate) q_extreme: f64,
    pub(crate) cheap_token_extreme: f64,
    pub(crate) q_consensus_mismatch: f64,
    pub(crate) cheap_token_consensus_mismatch: f64,
    pub(crate) dislocation_consensus_mismatch: f64,
    pub(crate) dislocation_warn: f64,
    pub(crate) dislocation_high: f64,
    pub(crate) dislocation_red: f64,
    pub(crate) max_book_age_ms: i64,
    pub(crate) min_depth_coverage: f64,
    pub(crate) use_best_ask_fallback: bool,
    pub(crate) best_ask_fallback_max_spread: f64,
}

impl Default for PriceToBeatIvOracleLagBookLeadConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            early_seconds: DEFAULT_EARLY_SECONDS,
            q_high: DEFAULT_Q_HIGH,
            cheap_token: DEFAULT_CHEAP_TOKEN,
            q_extreme: DEFAULT_Q_EXTREME,
            cheap_token_extreme: DEFAULT_CHEAP_TOKEN_EXTREME,
            q_consensus_mismatch: DEFAULT_Q_CONSENSUS_MISMATCH,
            cheap_token_consensus_mismatch: DEFAULT_CHEAP_TOKEN_CONSENSUS_MISMATCH,
            dislocation_consensus_mismatch: DEFAULT_DISLOCATION_CONSENSUS_MISMATCH,
            dislocation_warn: DEFAULT_DISLOCATION_WARN,
            dislocation_high: DEFAULT_DISLOCATION_HIGH,
            dislocation_red: DEFAULT_DISLOCATION_RED,
            max_book_age_ms: DEFAULT_MAX_BOOK_AGE_MS,
            min_depth_coverage: DEFAULT_MIN_DEPTH_COVERAGE,
            use_best_ask_fallback: false,
            best_ask_fallback_max_spread: DEFAULT_BEST_ASK_FALLBACK_MAX_SPREAD,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvOracleLagBookLeadEvaluation {
    pub(crate) enabled: bool,
    pub(crate) suspicion: &'static str,
    pub(crate) book_leading_oracle: bool,
    pub(crate) q_final: Option<f64>,
    pub(crate) execution_vwap: Option<f64>,
    pub(crate) execution_ref: Option<f64>,
    pub(crate) execution_ref_source: &'static str,
    pub(crate) dislocation: Option<f64>,
    pub(crate) action: &'static str,
    pub(crate) block_reason: Option<&'static str>,
    pub(crate) reference_status: &'static str,
    pub(crate) reference_age_ms: Option<i64>,
    pub(crate) depth_coverage_ratio: Option<f64>,
    pub(crate) cex_consensus: Option<CexOpenGapConsensus>,
}

impl Default for PriceToBeatIvOracleLagBookLeadEvaluation {
    fn default() -> Self {
        Self {
            enabled: false,
            suspicion: "disabled",
            book_leading_oracle: false,
            q_final: None,
            execution_vwap: None,
            execution_ref: None,
            execution_ref_source: "disabled",
            dislocation: None,
            action: "disabled",
            block_reason: None,
            reference_status: "disabled",
            reference_age_ms: None,
            depth_coverage_ratio: None,
            cex_consensus: None,
        }
    }
}

impl PriceToBeatIvOracleLagBookLeadEvaluation {
    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert(
            "oracle_lag_book_lead_guard_enabled".to_string(),
            json!(self.enabled),
        );
        obj.insert("oracle_lag_suspicion".to_string(), json!(self.suspicion));
        obj.insert(
            "book_leading_oracle".to_string(),
            json!(self.book_leading_oracle),
        );
        obj.insert(
            "q_final_cent".to_string(),
            json!(price_to_cent(self.q_final)),
        );
        obj.insert(
            "model_book_dislocation_cent".to_string(),
            json!(price_to_cent(self.dislocation)),
        );
        obj.insert(
            "execution_ref_cent".to_string(),
            json!(price_to_cent(self.execution_ref)),
        );
        obj.insert(
            "execution_ref_source".to_string(),
            json!(self.execution_ref_source),
        );
        obj.insert("oracle_lag_action".to_string(), json!(self.action));
        obj.insert(
            "oracle_lag_block_reason".to_string(),
            json!(self.block_reason),
        );
        obj.insert(
            "oracle_lag_book_reference_status".to_string(),
            json!(self.reference_status),
        );
        obj.insert(
            "oracle_lag_book_reference_age_ms".to_string(),
            json!(self.reference_age_ms),
        );
        obj.insert(
            "oracle_lag_book_depth_coverage_ratio".to_string(),
            json!(self.depth_coverage_ratio),
        );
        obj.insert(
            "oracle_lag_cex_consensus".to_string(),
            json!(self.cex_consensus.map(CexOpenGapConsensus::as_str)),
        );
    }
}

pub(crate) struct PriceToBeatIvOracleLagBookLeadInput<'a> {
    pub(crate) config: PriceToBeatIvOracleLagBookLeadConfig,
    pub(crate) seconds_left: f64,
    pub(crate) q_final: f64,
    pub(crate) execution_vwap: &'a PriceToBeatIvExecutionVwapEvaluation,
    pub(crate) spread: Option<f64>,
    pub(crate) cex_consensus: Option<CexOpenGapConsensus>,
    pub(crate) cex_lead_override_applies: bool,
}

pub(crate) fn evaluate_price_to_beat_iv_oracle_lag_book_lead(
    input: PriceToBeatIvOracleLagBookLeadInput<'_>,
) -> PriceToBeatIvOracleLagBookLeadEvaluation {
    if !input.config.enabled {
        return PriceToBeatIvOracleLagBookLeadEvaluation::default();
    }

    let reference_age_ms = Some(0);
    let execution_vwap = input.execution_vwap.execution_vwap;
    let depth_coverage_ratio = input.execution_vwap.depth_coverage_ratio;
    let vwap_reliable = execution_vwap.is_some()
        && book_reference_reliable(&input.config, input.execution_vwap, reference_age_ms);
    let best_ask_fallback = input
        .config
        .use_best_ask_fallback
        .then_some(input.execution_vwap.execution_best_ask)
        .flatten()
        .filter(|ask| ask.is_finite() && *ask > 0.0 && *ask < 1.0)
        .filter(|_| best_ask_fallback_spread_ok(&input.config, input.spread));
    let (execution_ref, execution_ref_source, reference_status) = if vwap_reliable {
        (execution_vwap, "execution_vwap", "reliable")
    } else if let Some(best_ask) = best_ask_fallback {
        (
            Some(best_ask),
            "execution_best_ask_fallback",
            "best_ask_fallback",
        )
    } else if execution_vwap.is_some() {
        (execution_vwap, "execution_vwap", "unreliable")
    } else {
        (None, "unavailable", "unavailable")
    };
    let dislocation = execution_ref.map(|reference| input.q_final - reference);
    let suspicion = oracle_lag_suspicion(&input.config, dislocation);
    let book_leading_oracle = dislocation
        .map(|value| value >= input.config.dislocation_warn)
        .unwrap_or(false);
    let extreme = matches!(reference_status, "reliable" | "best_ask_fallback")
        && input.seconds_left > input.config.early_seconds
        && input.q_final >= input.config.q_extreme
        && execution_ref
            .map(|value| value <= input.config.cheap_token_extreme)
            .unwrap_or(false)
        && dislocation
            .map(|value| value >= input.config.dislocation_red)
            .unwrap_or(false);
    let consensus_mismatch = matches!(reference_status, "reliable" | "best_ask_fallback")
        && input.seconds_left > input.config.early_seconds
        && input.q_final >= input.config.q_consensus_mismatch
        && execution_ref
            .map(|value| value <= input.config.cheap_token_consensus_mismatch)
            .unwrap_or(false)
        && dislocation
            .map(|value| value >= input.config.dislocation_consensus_mismatch)
            .unwrap_or(false)
        && input
            .cex_consensus
            .map(|consensus| consensus != CexOpenGapConsensus::Strong)
            .unwrap_or(false);
    let late_high_book_lead = input.seconds_left <= input.config.early_seconds
        && input.q_final >= input.config.q_high
        && execution_ref
            .map(|value| value <= input.config.cheap_token)
            .unwrap_or(false)
        && dislocation
            .map(|value| value >= input.config.dislocation_high)
            .unwrap_or(false)
        && !input.cex_lead_override_applies;
    let block_reason = if extreme {
        Some("blocked_oracle_lag_book_lead")
    } else if consensus_mismatch {
        Some("blocked_oracle_lag_consensus_mismatch")
    } else if late_high_book_lead {
        Some("blocked_oracle_lag_late_high_book_lead")
    } else {
        None
    };
    let action = if block_reason.is_some() {
        "BLOCK"
    } else if reference_status == "unavailable" {
        "UNAVAILABLE"
    } else if reference_status == "unreliable" {
        "UNRELIABLE"
    } else {
        "OBSERVE"
    };

    PriceToBeatIvOracleLagBookLeadEvaluation {
        enabled: true,
        suspicion,
        book_leading_oracle,
        q_final: Some(input.q_final),
        execution_vwap,
        execution_ref,
        execution_ref_source,
        dislocation,
        action,
        block_reason,
        reference_status,
        reference_age_ms,
        depth_coverage_ratio,
        cex_consensus: input.cex_consensus,
    }
}

fn best_ask_fallback_spread_ok(
    config: &PriceToBeatIvOracleLagBookLeadConfig,
    spread: Option<f64>,
) -> bool {
    spread
        .map(|value| {
            value.is_finite() && value >= 0.0 && value <= config.best_ask_fallback_max_spread
        })
        .unwrap_or(false)
}

fn book_reference_reliable(
    config: &PriceToBeatIvOracleLagBookLeadConfig,
    execution_vwap: &PriceToBeatIvExecutionVwapEvaluation,
    reference_age_ms: Option<i64>,
) -> bool {
    execution_vwap.book_depth_ok.unwrap_or(false)
        && reference_age_ms
            .map(|age| age <= config.max_book_age_ms)
            .unwrap_or(false)
        && execution_vwap
            .depth_coverage_ratio
            .map(|coverage| coverage + 0.000001 >= config.min_depth_coverage)
            .unwrap_or(false)
}

fn oracle_lag_suspicion(
    config: &PriceToBeatIvOracleLagBookLeadConfig,
    dislocation: Option<f64>,
) -> &'static str {
    let Some(dislocation) = dislocation else {
        return "unavailable";
    };
    if dislocation >= config.dislocation_red {
        "HIGH"
    } else if dislocation >= config.dislocation_high {
        "HIGH"
    } else if dislocation >= config.dislocation_warn {
        "MEDIUM"
    } else {
        "LOW"
    }
}

fn price_to_cent(value: Option<f64>) -> Option<f64> {
    value
        .map(|value| value * 100.0)
        .filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn execution_vwap(
        vwap: Option<f64>,
        coverage: Option<f64>,
    ) -> PriceToBeatIvExecutionVwapEvaluation {
        PriceToBeatIvExecutionVwapEvaluation {
            enabled: true,
            execution_vwap: vwap,
            execution_best_ask: vwap,
            depth_coverage_ratio: coverage,
            book_depth_ok: Some(vwap.is_some()),
            ..PriceToBeatIvExecutionVwapEvaluation::default()
        }
    }

    #[test]
    fn extreme_reliable_book_blocks_entry() {
        let eval =
            evaluate_price_to_beat_iv_oracle_lag_book_lead(PriceToBeatIvOracleLagBookLeadInput {
                config: PriceToBeatIvOracleLagBookLeadConfig {
                    enabled: true,
                    ..Default::default()
                },
                seconds_left: 114.0,
                q_final: 0.9956,
                execution_vwap: &execution_vwap(Some(0.64), Some(1.0)),
                spread: Some(0.01),
                cex_consensus: None,
                cex_lead_override_applies: false,
            });
        assert_eq!(eval.block_reason, Some("blocked_oracle_lag_book_lead"));
        assert_eq!(eval.action, "BLOCK");
    }

    #[test]
    fn seconds_boundary_at_sixty_blocks_late_high_book_lead() {
        let eval =
            evaluate_price_to_beat_iv_oracle_lag_book_lead(PriceToBeatIvOracleLagBookLeadInput {
                config: PriceToBeatIvOracleLagBookLeadConfig {
                    enabled: true,
                    ..Default::default()
                },
                seconds_left: 60.0,
                q_final: 0.95,
                execution_vwap: &execution_vwap(Some(0.70), Some(1.0)),
                spread: Some(0.01),
                cex_consensus: None,
                cex_lead_override_applies: false,
            });
        assert_eq!(
            eval.block_reason,
            Some("blocked_oracle_lag_late_high_book_lead")
        );
        assert_eq!(eval.action, "BLOCK");
        assert_eq!(eval.suspicion, "HIGH");
    }

    #[test]
    fn late_high_book_lead_blocks_entry() {
        let eval =
            evaluate_price_to_beat_iv_oracle_lag_book_lead(PriceToBeatIvOracleLagBookLeadInput {
                config: PriceToBeatIvOracleLagBookLeadConfig {
                    enabled: true,
                    q_high: 0.70,
                    cheap_token: 0.70,
                    dislocation_high: 0.20,
                    early_seconds: 60.0,
                    ..Default::default()
                },
                seconds_left: 12.0,
                q_final: 0.7307,
                execution_vwap: &execution_vwap(Some(0.38), Some(1.0)),
                spread: Some(0.01),
                cex_consensus: None,
                cex_lead_override_applies: false,
            });

        assert_eq!(
            eval.block_reason,
            Some("blocked_oracle_lag_late_high_book_lead")
        );
        assert_eq!(eval.action, "BLOCK");
    }

    #[test]
    fn cex_lead_override_bypasses_late_high_book_lead_only() {
        let eval =
            evaluate_price_to_beat_iv_oracle_lag_book_lead(PriceToBeatIvOracleLagBookLeadInput {
                config: PriceToBeatIvOracleLagBookLeadConfig {
                    enabled: true,
                    q_high: 0.70,
                    cheap_token: 0.70,
                    dislocation_high: 0.20,
                    early_seconds: 120.0,
                    use_best_ask_fallback: true,
                    ..Default::default()
                },
                seconds_left: 39.775,
                q_final: 0.999_997,
                execution_vwap: &PriceToBeatIvExecutionVwapEvaluation {
                    enabled: true,
                    execution_best_ask: Some(0.68),
                    ..Default::default()
                },
                spread: Some(0.01),
                cex_consensus: Some(CexOpenGapConsensus::Strong),
                cex_lead_override_applies: true,
            });

        assert_eq!(eval.block_reason, None);
        assert_eq!(eval.action, "OBSERVE");
        assert!(eval.book_leading_oracle);
    }

    #[test]
    fn late_high_book_lead_does_not_block_below_q_threshold() {
        let eval =
            evaluate_price_to_beat_iv_oracle_lag_book_lead(PriceToBeatIvOracleLagBookLeadInput {
                config: PriceToBeatIvOracleLagBookLeadConfig {
                    enabled: true,
                    q_high: 0.70,
                    cheap_token: 0.70,
                    dislocation_high: 0.20,
                    early_seconds: 60.0,
                    ..Default::default()
                },
                seconds_left: 12.0,
                q_final: 0.6999,
                execution_vwap: &execution_vwap(Some(0.38), Some(1.0)),
                spread: Some(0.01),
                cex_consensus: None,
                cex_lead_override_applies: false,
            });

        assert_eq!(eval.block_reason, None);
        assert_eq!(eval.action, "OBSERVE");
    }

    #[test]
    fn late_high_book_lead_does_not_block_before_late_window() {
        let eval =
            evaluate_price_to_beat_iv_oracle_lag_book_lead(PriceToBeatIvOracleLagBookLeadInput {
                config: PriceToBeatIvOracleLagBookLeadConfig {
                    enabled: true,
                    q_high: 0.70,
                    cheap_token: 0.70,
                    dislocation_high: 0.20,
                    early_seconds: 60.0,
                    ..Default::default()
                },
                seconds_left: 61.0,
                q_final: 0.7307,
                execution_vwap: &execution_vwap(Some(0.38), Some(1.0)),
                spread: Some(0.01),
                cex_consensus: None,
                cex_lead_override_applies: false,
            });

        assert_eq!(eval.block_reason, None);
        assert_eq!(eval.action, "OBSERVE");
    }

    #[test]
    fn unreliable_book_does_not_hard_block() {
        let eval =
            evaluate_price_to_beat_iv_oracle_lag_book_lead(PriceToBeatIvOracleLagBookLeadInput {
                config: PriceToBeatIvOracleLagBookLeadConfig {
                    enabled: true,
                    ..Default::default()
                },
                seconds_left: 114.0,
                q_final: 0.9956,
                execution_vwap: &execution_vwap(Some(0.64), Some(0.50)),
                spread: Some(0.01),
                cex_consensus: None,
                cex_lead_override_applies: false,
            });
        assert_eq!(eval.reference_status, "unreliable");
        assert_eq!(eval.block_reason, None);
    }

    #[test]
    fn best_ask_fallback_blocks_extreme_lag_when_vwap_unavailable() {
        let book = PriceToBeatIvExecutionVwapEvaluation {
            enabled: true,
            execution_best_ask: Some(0.68),
            execution_vwap: None,
            ..PriceToBeatIvExecutionVwapEvaluation::default()
        };
        let eval =
            evaluate_price_to_beat_iv_oracle_lag_book_lead(PriceToBeatIvOracleLagBookLeadInput {
                config: PriceToBeatIvOracleLagBookLeadConfig {
                    enabled: true,
                    use_best_ask_fallback: true,
                    q_extreme: 0.98,
                    cheap_token_extreme: 0.72,
                    dislocation_red: 0.25,
                    ..Default::default()
                },
                seconds_left: 111.0,
                q_final: 0.9893,
                execution_vwap: &book,
                spread: Some(0.01),
                cex_consensus: None,
                cex_lead_override_applies: false,
            });

        assert_eq!(eval.reference_status, "best_ask_fallback");
        assert_eq!(eval.execution_ref, Some(0.68));
        assert_eq!(eval.block_reason, Some("blocked_oracle_lag_book_lead"));
    }

    #[test]
    fn cex_lead_override_does_not_bypass_extreme_or_consensus_mismatch() {
        let extreme =
            evaluate_price_to_beat_iv_oracle_lag_book_lead(PriceToBeatIvOracleLagBookLeadInput {
                config: PriceToBeatIvOracleLagBookLeadConfig {
                    enabled: true,
                    q_extreme: 0.98,
                    cheap_token_extreme: 0.72,
                    dislocation_red: 0.25,
                    use_best_ask_fallback: true,
                    ..Default::default()
                },
                seconds_left: 121.0,
                q_final: 0.99,
                execution_vwap: &execution_vwap(Some(0.68), Some(1.0)),
                spread: Some(0.01),
                cex_consensus: Some(CexOpenGapConsensus::Strong),
                cex_lead_override_applies: true,
            });
        assert_eq!(extreme.block_reason, Some("blocked_oracle_lag_book_lead"));

        let mismatch =
            evaluate_price_to_beat_iv_oracle_lag_book_lead(PriceToBeatIvOracleLagBookLeadInput {
                config: PriceToBeatIvOracleLagBookLeadConfig {
                    enabled: true,
                    q_consensus_mismatch: 0.95,
                    cheap_token_consensus_mismatch: 0.65,
                    dislocation_consensus_mismatch: 0.30,
                    use_best_ask_fallback: true,
                    ..Default::default()
                },
                seconds_left: 121.0,
                q_final: 0.96,
                execution_vwap: &execution_vwap(Some(0.64), Some(1.0)),
                spread: Some(0.01),
                cex_consensus: Some(CexOpenGapConsensus::Weak),
                cex_lead_override_applies: true,
            });
        assert_eq!(
            mismatch.block_reason,
            Some("blocked_oracle_lag_consensus_mismatch")
        );
    }
}
