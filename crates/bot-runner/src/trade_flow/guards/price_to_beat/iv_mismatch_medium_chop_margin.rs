use serde_json::{json, Map, Value};

const DEFAULT_MEDIUM_CHOP_MIN_ADJ_MARGIN: f64 = 0.045;
const DEFAULT_MEDIUM_CHOP_HIGH_PRICE_MIN_ADJ_MARGIN: f64 = 0.050;
const DEFAULT_MEDIUM_CHOP_HIGH_PRICE_REF: f64 = 0.82;
const DEFAULT_MEDIUM_CHOP_BINANCE_FAIL_OPEN_MARGIN_ADD: f64 = 0.005;
const DEFAULT_MEDIUM_CHOP_STALE_MS: i64 = 1_500;
const DEFAULT_MEDIUM_CHOP_STALE_MARGIN_ADD: f64 = 0.005;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvMediumChopMarginConfig {
    pub(crate) min_adj_margin: f64,
    pub(crate) high_price_min_adj_margin: f64,
    pub(crate) high_price_ref: f64,
    pub(crate) binance_fail_open_margin_add: f64,
    pub(crate) stale_ms: i64,
    pub(crate) stale_margin_add: f64,
}

impl Default for PriceToBeatIvMediumChopMarginConfig {
    fn default() -> Self {
        Self {
            min_adj_margin: DEFAULT_MEDIUM_CHOP_MIN_ADJ_MARGIN,
            high_price_min_adj_margin: DEFAULT_MEDIUM_CHOP_HIGH_PRICE_MIN_ADJ_MARGIN,
            high_price_ref: DEFAULT_MEDIUM_CHOP_HIGH_PRICE_REF,
            binance_fail_open_margin_add: DEFAULT_MEDIUM_CHOP_BINANCE_FAIL_OPEN_MARGIN_ADD,
            stale_ms: DEFAULT_MEDIUM_CHOP_STALE_MS,
            stale_margin_add: DEFAULT_MEDIUM_CHOP_STALE_MARGIN_ADD,
        }
    }
}

pub(crate) struct PriceToBeatIvMediumChopMarginInput<'a> {
    pub(crate) config: &'a PriceToBeatIvMediumChopMarginConfig,
    pub(crate) movement_mode: &'static str,
    pub(crate) decision_ref: Option<f64>,
    pub(crate) adjusted_margin: f64,
    pub(crate) binance_fail_open: bool,
    pub(crate) chainlink_staleness_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvMediumChopMarginEvaluation {
    pub(crate) enabled: bool,
    pub(crate) movement_mode: &'static str,
    pub(crate) decision_ref: Option<f64>,
    pub(crate) adjusted_margin: Option<f64>,
    pub(crate) required_margin: Option<f64>,
    pub(crate) base_margin: Option<f64>,
    pub(crate) high_price_margin_add: f64,
    pub(crate) binance_fail_open_margin_add: f64,
    pub(crate) stale_margin_add: f64,
    pub(crate) result: &'static str,
}

impl PriceToBeatIvMediumChopMarginEvaluation {
    pub(crate) fn off(movement_mode: &'static str) -> Self {
        Self {
            enabled: false,
            movement_mode,
            decision_ref: None,
            adjusted_margin: None,
            required_margin: None,
            base_margin: None,
            high_price_margin_add: 0.0,
            binance_fail_open_margin_add: 0.0,
            stale_margin_add: 0.0,
            result: "off",
        }
    }

    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert(
            "medium_chop_margin_guard_enabled".to_string(),
            json!(self.enabled),
        );
        obj.insert(
            "medium_chop_margin_mode".to_string(),
            json!(self.movement_mode),
        );
        obj.insert(
            "medium_chop_margin_decision_ref_cent".to_string(),
            json!(self.decision_ref.map(|value| value * 100.0)),
        );
        obj.insert(
            "medium_chop_margin_adjusted_margin".to_string(),
            json!(self.adjusted_margin),
        );
        obj.insert(
            "medium_chop_margin_required_margin".to_string(),
            json!(self.required_margin),
        );
        obj.insert(
            "medium_chop_margin_base".to_string(),
            json!(self.base_margin),
        );
        obj.insert(
            "medium_chop_margin_high_price_add".to_string(),
            json!(self.high_price_margin_add),
        );
        obj.insert(
            "medium_chop_margin_binance_fail_open_add".to_string(),
            json!(self.binance_fail_open_margin_add),
        );
        obj.insert(
            "medium_chop_margin_stale_add".to_string(),
            json!(self.stale_margin_add),
        );
        obj.insert("medium_chop_margin_result".to_string(), json!(self.result));
    }
}

pub(crate) fn evaluate_price_to_beat_iv_medium_chop_margin(
    input: PriceToBeatIvMediumChopMarginInput<'_>,
) -> PriceToBeatIvMediumChopMarginEvaluation {
    if input.movement_mode != "medium_chop" {
        return PriceToBeatIvMediumChopMarginEvaluation::off(input.movement_mode);
    }

    let base_margin = sane_margin(input.config.min_adj_margin);
    let high_price_ref = input.config.high_price_ref.clamp(0.0, 1.0);
    let high_price_margin_add = if input
        .decision_ref
        .filter(|value| value.is_finite())
        .map(|value| value >= high_price_ref)
        .unwrap_or(false)
    {
        (sane_margin(input.config.high_price_min_adj_margin) - base_margin).max(0.0)
    } else {
        0.0
    };
    let binance_fail_open_margin_add = if input.binance_fail_open {
        sane_margin(input.config.binance_fail_open_margin_add)
    } else {
        0.0
    };
    let stale_margin_add = if input.chainlink_staleness_ms > input.config.stale_ms.max(0) {
        sane_margin(input.config.stale_margin_add)
    } else {
        0.0
    };
    let required_margin =
        base_margin + high_price_margin_add + binance_fail_open_margin_add + stale_margin_add;
    let result = if input.adjusted_margin < required_margin {
        "blocked_medium_chop_adj_margin"
    } else {
        "pass"
    };

    PriceToBeatIvMediumChopMarginEvaluation {
        enabled: true,
        movement_mode: input.movement_mode,
        decision_ref: input.decision_ref,
        adjusted_margin: Some(input.adjusted_margin),
        required_margin: Some(required_margin),
        base_margin: Some(base_margin),
        high_price_margin_add,
        binance_fail_open_margin_add,
        stale_margin_add,
        result,
    }
}

fn sane_margin(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_low_adjusted_margin_with_default_components() {
        let config = PriceToBeatIvMediumChopMarginConfig::default();
        let evaluation =
            evaluate_price_to_beat_iv_medium_chop_margin(PriceToBeatIvMediumChopMarginInput {
                config: &config,
                movement_mode: "medium_chop",
                decision_ref: Some(0.83),
                adjusted_margin: 0.0400,
                binance_fail_open: true,
                chainlink_staleness_ms: 1_756,
            });

        assert_eq!(evaluation.result, "blocked_medium_chop_adj_margin");
        assert_eq!(evaluation.base_margin, Some(0.045));
        assert!((evaluation.high_price_margin_add - 0.005).abs() < 0.000_001);
        assert!((evaluation.binance_fail_open_margin_add - 0.005).abs() < 0.000_001);
        assert!((evaluation.stale_margin_add - 0.005).abs() < 0.000_001);
        assert!((evaluation.required_margin.unwrap() - 0.0600).abs() < 0.000_001);
    }

    #[test]
    fn passes_medium_chop_when_margin_is_sufficient() {
        let config = PriceToBeatIvMediumChopMarginConfig::default();
        let evaluation =
            evaluate_price_to_beat_iv_medium_chop_margin(PriceToBeatIvMediumChopMarginInput {
                config: &config,
                movement_mode: "medium_chop",
                decision_ref: Some(0.83),
                adjusted_margin: 0.0810,
                binance_fail_open: true,
                chainlink_staleness_ms: 1_756,
            });

        assert_eq!(evaluation.result, "pass");
        assert!((evaluation.required_margin.unwrap() - 0.0600).abs() < 0.000_001);
    }

    #[test]
    fn ignores_non_medium_chop_modes() {
        let config = PriceToBeatIvMediumChopMarginConfig::default();
        let evaluation =
            evaluate_price_to_beat_iv_medium_chop_margin(PriceToBeatIvMediumChopMarginInput {
                config: &config,
                movement_mode: "clean_trend",
                decision_ref: Some(0.83),
                adjusted_margin: 0.0,
                binance_fail_open: true,
                chainlink_staleness_ms: 1_756,
            });

        assert!(!evaluation.enabled);
        assert_eq!(evaluation.result, "off");
        assert_eq!(evaluation.required_margin, None);
    }
}
