use super::iv_cex_open_gap::CexOpenGapConsensus;
use serde_json::{json, Map, Value};

const DEFAULT_GAP_GROWTH_RATIO: f64 = 1.25;
const DEFAULT_HARD_RATIO: f64 = 1.50;
const DEFAULT_MIN_HOLD_MS: i64 = 3_000;
const DEFAULT_MIN_BUFFER_RETAIN: f64 = 0.80;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvPumpShockConfig {
    pub(crate) enabled: bool,
    pub(crate) gap_growth_ratio: f64,
    pub(crate) hard_ratio: f64,
    pub(crate) min_hold_ms: i64,
    pub(crate) min_buffer_retain: f64,
}

impl Default for PriceToBeatIvPumpShockConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            gap_growth_ratio: DEFAULT_GAP_GROWTH_RATIO,
            hard_ratio: DEFAULT_HARD_RATIO,
            min_hold_ms: DEFAULT_MIN_HOLD_MS,
            min_buffer_retain: DEFAULT_MIN_BUFFER_RETAIN,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvPumpShockEvaluation {
    pub(crate) enabled: bool,
    pub(crate) gap_growth_ratio: Option<f64>,
    pub(crate) action: &'static str,
    pub(crate) block_reason: Option<&'static str>,
    pub(crate) persistence_ok: Option<bool>,
    pub(crate) persistence_status: &'static str,
    pub(crate) hold_gap: Option<f64>,
    pub(crate) buffer_retain: Option<f64>,
    pub(crate) cex_consensus: Option<CexOpenGapConsensus>,
    pub(crate) execution_ref_reliable: Option<bool>,
    pub(crate) token_price_confirming: Option<bool>,
    pub(crate) book_dislocation_improving: Option<bool>,
}

impl Default for PriceToBeatIvPumpShockEvaluation {
    fn default() -> Self {
        Self {
            enabled: false,
            gap_growth_ratio: None,
            action: "disabled",
            block_reason: None,
            persistence_ok: None,
            persistence_status: "disabled",
            hold_gap: None,
            buffer_retain: None,
            cex_consensus: None,
            execution_ref_reliable: None,
            token_price_confirming: None,
            book_dislocation_improving: None,
        }
    }
}

impl PriceToBeatIvPumpShockEvaluation {
    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert("pump_shock_guard_enabled".to_string(), json!(self.enabled));
        obj.insert(
            "pump_shock_gap_growth_ratio".to_string(),
            json!(self.gap_growth_ratio),
        );
        obj.insert("pump_shock_action".to_string(), json!(self.action));
        obj.insert(
            "pump_shock_block_reason".to_string(),
            json!(self.block_reason),
        );
        obj.insert(
            "pump_shock_persistence_ok".to_string(),
            json!(self.persistence_ok),
        );
        obj.insert(
            "pump_shock_persistence_status".to_string(),
            json!(self.persistence_status),
        );
        obj.insert("pump_shock_hold_gap".to_string(), json!(self.hold_gap));
        obj.insert(
            "pump_shock_buffer_retain".to_string(),
            json!(self.buffer_retain),
        );
        obj.insert(
            "pump_shock_cex_consensus".to_string(),
            json!(self.cex_consensus.map(CexOpenGapConsensus::as_str)),
        );
        obj.insert(
            "pump_shock_execution_ref_reliable".to_string(),
            json!(self.execution_ref_reliable),
        );
        obj.insert(
            "pump_shock_token_price_confirming".to_string(),
            json!(self.token_price_confirming),
        );
        obj.insert(
            "pump_shock_book_dislocation_improving".to_string(),
            json!(self.book_dislocation_improving),
        );
    }
}

pub(crate) struct PriceToBeatIvPumpShockInput {
    pub(crate) config: PriceToBeatIvPumpShockConfig,
    pub(crate) seconds_left: f64,
    pub(crate) x_now: f64,
    pub(crate) x_prev: Option<f64>,
    pub(crate) expected_move_eff: f64,
    pub(crate) same_side_gap_at_hold: Option<f64>,
    pub(crate) model_book_dislocation: Option<f64>,
    pub(crate) dislocation_red: f64,
    pub(crate) cex_consensus: Option<CexOpenGapConsensus>,
    pub(crate) execution_ref_reliable: Option<bool>,
    pub(crate) token_price_confirming: Option<bool>,
    pub(crate) book_dislocation_improving: Option<bool>,
}

pub(crate) fn evaluate_price_to_beat_iv_pump_shock(
    input: PriceToBeatIvPumpShockInput,
) -> PriceToBeatIvPumpShockEvaluation {
    if !input.config.enabled {
        return PriceToBeatIvPumpShockEvaluation::default();
    }

    let gap_growth_ratio = input.x_prev.and_then(|x_prev| {
        let numerator = input.x_now - x_prev.max(0.0);
        (input.expected_move_eff.is_finite() && input.expected_move_eff > 0.0)
            .then_some(numerator / input.expected_move_eff)
    });
    let buffer_retain = input.same_side_gap_at_hold.and_then(|hold_gap| {
        (hold_gap.is_finite() && hold_gap > 0.0 && input.x_now.is_finite())
            .then_some(input.x_now / hold_gap)
    });
    let raw_persistence_ok = input
        .same_side_gap_at_hold
        .map(|hold_gap| hold_gap > 0.0)
        .zip(buffer_retain)
        .map(|(same_side, retain)| same_side && retain >= input.config.min_buffer_retain);
    let (persistence_ok, persistence_status) = consensus_persistence_status(
        raw_persistence_ok,
        input.cex_consensus,
        input.execution_ref_reliable,
        input.token_price_confirming,
        input.book_dislocation_improving,
    );

    let high_dislocation = input
        .model_book_dislocation
        .map(|value| value >= input.dislocation_red)
        .unwrap_or(false);
    let block_reason = if input.seconds_left > 60.0 {
        if matches!(gap_growth_ratio, Some(ratio) if ratio >= input.config.hard_ratio)
            && high_dislocation
        {
            if input
                .cex_consensus
                .map(|consensus| consensus != CexOpenGapConsensus::Strong)
                .unwrap_or(false)
            {
                Some("blocked_pump_shock_cex_book_mismatch")
            } else {
                Some("blocked_pump_shock_book_lead")
            }
        } else if matches!(gap_growth_ratio, Some(ratio) if ratio >= input.config.gap_growth_ratio)
            && !persistence_ok.unwrap_or(false)
        {
            Some("wait_pump_shock_persistence")
        } else {
            None
        }
    } else {
        None
    };
    let action = match block_reason {
        Some("blocked_pump_shock_book_lead") => "BLOCK",
        Some("blocked_pump_shock_cex_book_mismatch") => "BLOCK",
        Some("wait_pump_shock_persistence") => "WAIT",
        Some(_) => "BLOCK",
        None => "OBSERVE",
    };

    PriceToBeatIvPumpShockEvaluation {
        enabled: true,
        gap_growth_ratio,
        action,
        block_reason,
        persistence_ok,
        persistence_status,
        hold_gap: input.same_side_gap_at_hold,
        buffer_retain,
        cex_consensus: input.cex_consensus,
        execution_ref_reliable: input.execution_ref_reliable,
        token_price_confirming: input.token_price_confirming,
        book_dislocation_improving: input.book_dislocation_improving,
    }
}

fn consensus_persistence_status(
    raw_persistence_ok: Option<bool>,
    cex_consensus: Option<CexOpenGapConsensus>,
    execution_ref_reliable: Option<bool>,
    token_price_confirming: Option<bool>,
    book_dislocation_improving: Option<bool>,
) -> (Option<bool>, &'static str) {
    let Some(raw_ok) = raw_persistence_ok else {
        return (None, "hold_gap_unavailable");
    };
    if !raw_ok {
        return (Some(false), "chainlink_buffer_not_retained");
    }
    let Some(consensus) = cex_consensus else {
        return (Some(true), "legacy_pass");
    };
    if !execution_ref_reliable.unwrap_or(false) {
        return (Some(false), "execution_ref_unreliable");
    }
    match consensus {
        CexOpenGapConsensus::Strong => (Some(true), "strong_consensus_pass"),
        CexOpenGapConsensus::Mixed => {
            if book_dislocation_improving.unwrap_or(false)
                || token_price_confirming.unwrap_or(false)
            {
                (Some(true), "mixed_confirmed_pass")
            } else {
                (Some(false), "mixed_unconfirmed")
            }
        }
        CexOpenGapConsensus::Weak => (Some(false), "weak_consensus_no_pass"),
        CexOpenGapConsensus::Against => (Some(false), "against_consensus_no_pass"),
        CexOpenGapConsensus::Partial => (Some(false), "partial_consensus_no_pass"),
        CexOpenGapConsensus::Unavailable => (Some(false), "unavailable_consensus_no_pass"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_ratio_with_high_dislocation_blocks() {
        let eval = evaluate_price_to_beat_iv_pump_shock(PriceToBeatIvPumpShockInput {
            config: PriceToBeatIvPumpShockConfig {
                enabled: true,
                ..Default::default()
            },
            seconds_left: 111.0,
            x_now: 50.0,
            x_prev: Some(0.0),
            expected_move_eff: 29.5,
            same_side_gap_at_hold: None,
            model_book_dislocation: Some(0.31),
            dislocation_red: 0.25,
            cex_consensus: None,
            execution_ref_reliable: None,
            token_price_confirming: None,
            book_dislocation_improving: None,
        });

        assert!(matches!(eval.gap_growth_ratio, Some(ratio) if ratio > 1.69));
        assert_eq!(eval.block_reason, Some("blocked_pump_shock_book_lead"));
    }

    #[test]
    fn growth_ratio_with_persistence_observes() {
        let eval = evaluate_price_to_beat_iv_pump_shock(PriceToBeatIvPumpShockInput {
            config: PriceToBeatIvPumpShockConfig {
                enabled: true,
                ..Default::default()
            },
            seconds_left: 90.0,
            x_now: 39.0,
            x_prev: Some(0.0),
            expected_move_eff: 30.0,
            same_side_gap_at_hold: Some(35.0),
            model_book_dislocation: Some(0.10),
            dislocation_red: 0.25,
            cex_consensus: None,
            execution_ref_reliable: None,
            token_price_confirming: None,
            book_dislocation_improving: None,
        });

        assert_eq!(eval.block_reason, None);
        assert_eq!(eval.action, "OBSERVE");
        assert_eq!(eval.persistence_ok, Some(true));
    }

    #[test]
    fn partial_consensus_does_not_pass_persistence() {
        let eval = evaluate_price_to_beat_iv_pump_shock(PriceToBeatIvPumpShockInput {
            config: PriceToBeatIvPumpShockConfig {
                enabled: true,
                ..Default::default()
            },
            seconds_left: 90.0,
            x_now: 39.0,
            x_prev: Some(0.0),
            expected_move_eff: 30.0,
            same_side_gap_at_hold: Some(35.0),
            model_book_dislocation: Some(0.10),
            dislocation_red: 0.25,
            cex_consensus: Some(CexOpenGapConsensus::Partial),
            execution_ref_reliable: Some(true),
            token_price_confirming: Some(true),
            book_dislocation_improving: Some(true),
        });

        assert_eq!(eval.persistence_ok, Some(false));
        assert_eq!(eval.persistence_status, "partial_consensus_no_pass");
    }
}
