use super::iv_oracle_lag_book_lead::PriceToBeatIvOracleLagBookLeadEvaluation;
use super::iv_pump_shock::PriceToBeatIvPumpShockEvaluation;
use serde_json::{json, Map, Value};

const DEFAULT_GAP_MARGIN_EARLY: f64 = 0.10;
const DEFAULT_PUMP_SHOCK_RATIO: f64 = 1.25;
const DEFAULT_Q_MIN_CENT: f64 = 95.0;
const DEFAULT_CHEAP_TOKEN_CENT: f64 = 60.0;
const DEFAULT_DISLOCATION_CENT: f64 = 30.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvBorderlinePumpBookLeadConfig {
    pub(crate) enabled: bool,
    pub(crate) gap_margin_early: f64,
    pub(crate) pump_shock_ratio: f64,
    pub(crate) q_min_cent: f64,
    pub(crate) cheap_token_cent: f64,
    pub(crate) dislocation_cent: f64,
}

impl Default for PriceToBeatIvBorderlinePumpBookLeadConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            gap_margin_early: DEFAULT_GAP_MARGIN_EARLY,
            pump_shock_ratio: DEFAULT_PUMP_SHOCK_RATIO,
            q_min_cent: DEFAULT_Q_MIN_CENT,
            cheap_token_cent: DEFAULT_CHEAP_TOKEN_CENT,
            dislocation_cent: DEFAULT_DISLOCATION_CENT,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvBorderlinePumpBookLeadEvaluation {
    pub(crate) enabled: bool,
    pub(crate) borderline_gap: Option<bool>,
    pub(crate) gap_strength_margin: Option<f64>,
    pub(crate) gap_strength_margin_required: Option<f64>,
    pub(crate) pump_shock_ratio: Option<f64>,
    pub(crate) q_final_cent: Option<f64>,
    pub(crate) execution_ref_cent: Option<f64>,
    pub(crate) model_book_dislocation_cent: Option<f64>,
    pub(crate) execution_ref_status: &'static str,
    pub(crate) execution_ref_source: &'static str,
    pub(crate) action: &'static str,
    pub(crate) block_reason: Option<&'static str>,
}

impl Default for PriceToBeatIvBorderlinePumpBookLeadEvaluation {
    fn default() -> Self {
        Self {
            enabled: false,
            borderline_gap: None,
            gap_strength_margin: None,
            gap_strength_margin_required: None,
            pump_shock_ratio: None,
            q_final_cent: None,
            execution_ref_cent: None,
            model_book_dislocation_cent: None,
            execution_ref_status: "disabled",
            execution_ref_source: "disabled",
            action: "disabled",
            block_reason: None,
        }
    }
}

impl PriceToBeatIvBorderlinePumpBookLeadEvaluation {
    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert(
            "borderline_pump_book_lead_guard_enabled".to_string(),
            json!(self.enabled),
        );
        obj.insert("borderline_gap".to_string(), json!(self.borderline_gap));
        obj.insert(
            "borderline_gap_strength_margin".to_string(),
            json!(self.gap_strength_margin),
        );
        obj.insert(
            "borderline_gap_strength_margin_required".to_string(),
            json!(self.gap_strength_margin_required),
        );
        obj.insert(
            "borderline_pump_shock_ratio".to_string(),
            json!(self.pump_shock_ratio),
        );
        obj.insert(
            "borderline_q_final_cent".to_string(),
            json!(self.q_final_cent),
        );
        obj.insert(
            "borderline_execution_ref_cent".to_string(),
            json!(self.execution_ref_cent),
        );
        obj.insert(
            "borderline_model_book_dislocation_cent".to_string(),
            json!(self.model_book_dislocation_cent),
        );
        obj.insert(
            "borderline_execution_ref_status".to_string(),
            json!(self.execution_ref_status),
        );
        obj.insert(
            "borderline_execution_ref_source".to_string(),
            json!(self.execution_ref_source),
        );
        obj.insert(
            "borderline_pump_book_lead_action".to_string(),
            json!(self.action),
        );
        obj.insert(
            "borderline_pump_book_lead_block_reason".to_string(),
            json!(self.block_reason),
        );
    }
}

pub(crate) struct PriceToBeatIvBorderlinePumpBookLeadInput<'a> {
    pub(crate) config: PriceToBeatIvBorderlinePumpBookLeadConfig,
    pub(crate) seconds_left: f64,
    pub(crate) gap_strength: f64,
    pub(crate) required_gap_strength_raw: f64,
    pub(crate) q_final: f64,
    pub(crate) oracle_lag_book_lead: &'a PriceToBeatIvOracleLagBookLeadEvaluation,
    pub(crate) pump_shock: &'a PriceToBeatIvPumpShockEvaluation,
}

pub(crate) fn evaluate_price_to_beat_iv_borderline_pump_book_lead(
    input: PriceToBeatIvBorderlinePumpBookLeadInput<'_>,
) -> PriceToBeatIvBorderlinePumpBookLeadEvaluation {
    if !input.config.enabled {
        return PriceToBeatIvBorderlinePumpBookLeadEvaluation::default();
    }

    let gap_strength_margin = (input.gap_strength.is_finite()
        && input.required_gap_strength_raw.is_finite())
    .then_some(input.gap_strength - input.required_gap_strength_raw);
    let borderline_gap =
        gap_strength_margin.map(|margin| margin >= 0.0 && margin < input.config.gap_margin_early);
    let q_final_cent = price_to_cent(Some(input.q_final));
    let execution_ref_cent = price_to_cent(input.oracle_lag_book_lead.execution_ref);
    let model_book_dislocation_cent = price_to_cent(input.oracle_lag_book_lead.dislocation);
    let reference_reliable = matches!(
        input.oracle_lag_book_lead.reference_status,
        "reliable" | "best_ask_fallback"
    ) && execution_ref_cent.is_some();
    let block = input.seconds_left > 60.0
        && borderline_gap.unwrap_or(false)
        && matches!(
            input.pump_shock.gap_growth_ratio,
            Some(ratio) if ratio >= input.config.pump_shock_ratio
        )
        && matches!(q_final_cent, Some(value) if value >= input.config.q_min_cent)
        && matches!(execution_ref_cent, Some(value) if value <= input.config.cheap_token_cent)
        && matches!(
            model_book_dislocation_cent,
            Some(value) if value >= input.config.dislocation_cent
        )
        && reference_reliable;
    let action = if block {
        "BLOCK"
    } else if input.oracle_lag_book_lead.reference_status == "unavailable" {
        "UNAVAILABLE"
    } else if !reference_reliable {
        "UNRELIABLE"
    } else {
        "OBSERVE"
    };

    PriceToBeatIvBorderlinePumpBookLeadEvaluation {
        enabled: true,
        borderline_gap,
        gap_strength_margin,
        gap_strength_margin_required: Some(input.config.gap_margin_early),
        pump_shock_ratio: input.pump_shock.gap_growth_ratio,
        q_final_cent,
        execution_ref_cent,
        model_book_dislocation_cent,
        execution_ref_status: input.oracle_lag_book_lead.reference_status,
        execution_ref_source: input.oracle_lag_book_lead.execution_ref_source,
        action,
        block_reason: block.then_some("blocked_borderline_pump_book_lead"),
    }
}

fn price_to_cent(value: Option<f64>) -> Option<f64> {
    value
        .filter(|value| value.is_finite())
        .map(|value| value * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(
        gap_strength: f64,
        required_gap_strength_raw: f64,
        seconds_left: f64,
    ) -> PriceToBeatIvBorderlinePumpBookLeadEvaluation {
        let oracle_lag_book_lead = PriceToBeatIvOracleLagBookLeadEvaluation {
            enabled: true,
            suspicion: "HIGH",
            book_leading_oracle: true,
            q_final: Some(0.9607),
            execution_vwap: None,
            execution_ref: Some(0.54),
            execution_ref_source: "execution_best_ask_fallback",
            dislocation: Some(0.4207),
            action: "OBSERVE",
            block_reason: None,
            reference_status: "best_ask_fallback",
            reference_age_ms: Some(0),
            depth_coverage_ratio: None,
            ..PriceToBeatIvOracleLagBookLeadEvaluation::default()
        };
        let pump_shock = PriceToBeatIvPumpShockEvaluation {
            enabled: true,
            gap_growth_ratio: Some(1.30),
            action: "OBSERVE",
            block_reason: None,
            persistence_ok: None,
            hold_gap: None,
            buffer_retain: None,
            ..PriceToBeatIvPumpShockEvaluation::default()
        };

        evaluate_price_to_beat_iv_borderline_pump_book_lead(
            PriceToBeatIvBorderlinePumpBookLeadInput {
                config: PriceToBeatIvBorderlinePumpBookLeadConfig {
                    enabled: true,
                    ..Default::default()
                },
                seconds_left,
                gap_strength,
                required_gap_strength_raw,
                q_final: 0.9607,
                oracle_lag_book_lead: &oracle_lag_book_lead,
                pump_shock: &pump_shock,
            },
        )
    }

    #[test]
    fn borderline_pump_book_lead_blocks_reported_trade_shape() {
        let evaluation = eval(1.7594, 1.75, 90.0);

        assert_eq!(
            evaluation.block_reason,
            Some("blocked_borderline_pump_book_lead")
        );
        assert!(matches!(evaluation.q_final_cent, Some(value) if (value - 96.07).abs() < 0.000001));
        assert!(
            matches!(evaluation.execution_ref_cent, Some(value) if (value - 54.0).abs() < 0.000001)
        );
    }

    #[test]
    fn negative_gap_margin_is_not_borderline() {
        let evaluation = eval(1.60, 1.75, 90.0);

        assert_eq!(evaluation.borderline_gap, Some(false));
        assert_eq!(evaluation.block_reason, None);
    }

    #[test]
    fn exact_margin_boundary_does_not_block() {
        let evaluation = eval(1.85, 1.75, 90.0);

        assert_eq!(evaluation.borderline_gap, Some(false));
        assert_eq!(evaluation.block_reason, None);
    }

    #[test]
    fn seconds_boundary_is_strictly_greater_than_sixty() {
        assert_eq!(eval(1.7594, 1.75, 60.0).block_reason, None);
        assert_eq!(
            eval(1.7594, 1.75, 60.01).block_reason,
            Some("blocked_borderline_pump_book_lead")
        );
    }
}
