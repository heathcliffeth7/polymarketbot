use serde_json::{json, Value};

const DEFAULT_ADAPTIVE_BASE_BPS: f64 = 1.5;
const DEFAULT_ADAPTIVE_MIN_BPS: f64 = 1.5;
const DEFAULT_ADAPTIVE_MAX_BPS: f64 = 2.75;
const DEFAULT_DISAGREEMENT_ADD_BPS: f64 = 0.25;
const DEFAULT_STRONG_DISAGREEMENT_ADD_BPS: f64 = 0.50;
const DEFAULT_SPREAD_ADD_BPS: f64 = 0.20;
const DEFAULT_WIDE_SPREAD_ADD_BPS: f64 = 0.40;
const DEFAULT_STALE_ADD_BPS: f64 = 0.20;
const DEFAULT_NOISE_ADD_BPS: f64 = 0.25;
const DISAGREEMENT_THRESHOLD: f64 = 0.12;
const STRONG_DISAGREEMENT_THRESHOLD: f64 = 0.18;
const SPREAD_THRESHOLD: f64 = 0.02;
const WIDE_SPREAD_THRESHOLD: f64 = 0.03;
const STALE_THRESHOLD_MS: i64 = 2_200;
const NOISE_RATIO_THRESHOLD: f64 = 1.35;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PriceToBeatIvMinExpectedMoveMode {
    Fixed,
    Adaptive,
}

impl PriceToBeatIvMinExpectedMoveMode {
    pub(crate) fn parse(raw: Option<&str>) -> Option<Self> {
        match raw
            .map(str::trim)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "fixed" => Some(Self::Fixed),
            "adaptive" => Some(Self::Adaptive),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Fixed => "fixed",
            Self::Adaptive => "adaptive",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvExpectedMoveFloorConfig {
    pub(crate) mode: PriceToBeatIvMinExpectedMoveMode,
    pub(crate) base_bps: f64,
    pub(crate) min_bps: f64,
    pub(crate) max_bps: f64,
    pub(crate) disagreement_add_bps: f64,
    pub(crate) strong_disagreement_add_bps: f64,
    pub(crate) spread_add_bps: f64,
    pub(crate) wide_spread_add_bps: f64,
    pub(crate) stale_add_bps: f64,
    pub(crate) noise_add_bps: f64,
}

impl Default for PriceToBeatIvExpectedMoveFloorConfig {
    fn default() -> Self {
        Self {
            mode: PriceToBeatIvMinExpectedMoveMode::Fixed,
            base_bps: DEFAULT_ADAPTIVE_BASE_BPS,
            min_bps: DEFAULT_ADAPTIVE_MIN_BPS,
            max_bps: DEFAULT_ADAPTIVE_MAX_BPS,
            disagreement_add_bps: DEFAULT_DISAGREEMENT_ADD_BPS,
            strong_disagreement_add_bps: DEFAULT_STRONG_DISAGREEMENT_ADD_BPS,
            spread_add_bps: DEFAULT_SPREAD_ADD_BPS,
            wide_spread_add_bps: DEFAULT_WIDE_SPREAD_ADD_BPS,
            stale_add_bps: DEFAULT_STALE_ADD_BPS,
            noise_add_bps: DEFAULT_NOISE_ADD_BPS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvExpectedMoveFloorInput {
    pub(crate) current_price: f64,
    pub(crate) spread: f64,
    pub(crate) source_staleness_ms: i64,
    pub(crate) sigma_fast: Option<f64>,
    pub(crate) sigma_eff: f64,
    pub(crate) disagreement_abs: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvExpectedMoveFloorEvaluation {
    pub(crate) mode: PriceToBeatIvMinExpectedMoveMode,
    pub(crate) base_bps: Option<f64>,
    pub(crate) effective_bps: Option<f64>,
    pub(crate) floor_usd: Option<f64>,
    pub(crate) adjustments: Vec<&'static str>,
}

impl PriceToBeatIvExpectedMoveFloorEvaluation {
    pub(crate) fn fixed() -> Self {
        Self {
            mode: PriceToBeatIvMinExpectedMoveMode::Fixed,
            base_bps: None,
            effective_bps: None,
            floor_usd: None,
            adjustments: Vec::new(),
        }
    }

    pub(crate) fn reason(&self) -> Option<String> {
        if self.adjustments.is_empty() {
            None
        } else {
            Some(self.adjustments.join("+"))
        }
    }

    pub(crate) fn append_to_json(&self, obj: &mut serde_json::Map<String, Value>) {
        obj.insert(
            "expected_move_floor_mode".to_string(),
            json!(self.mode.as_str()),
        );
        obj.insert(
            "expected_move_floor_bps_base".to_string(),
            json!(self.base_bps),
        );
        obj.insert(
            "expected_move_floor_bps_effective".to_string(),
            json!(self.effective_bps),
        );
        obj.insert("expected_move_floor_usd".to_string(), json!(self.floor_usd));
        obj.insert(
            "expected_move_floor_adjustments".to_string(),
            json!(self.adjustments),
        );
        obj.insert(
            "expected_move_floor_reason".to_string(),
            json!(self.reason()),
        );
    }
}

pub(crate) fn evaluate_expected_move_floor(
    config: &PriceToBeatIvExpectedMoveFloorConfig,
    input: PriceToBeatIvExpectedMoveFloorInput,
) -> PriceToBeatIvExpectedMoveFloorEvaluation {
    if config.mode != PriceToBeatIvMinExpectedMoveMode::Adaptive {
        return PriceToBeatIvExpectedMoveFloorEvaluation::fixed();
    }

    let mut bps = config.base_bps.max(0.0);
    let mut adjustments = Vec::new();
    if input.disagreement_abs.unwrap_or(0.0) >= STRONG_DISAGREEMENT_THRESHOLD {
        bps += config.strong_disagreement_add_bps.max(0.0);
        adjustments.push("disagreement_strong");
    } else if input.disagreement_abs.unwrap_or(0.0) >= DISAGREEMENT_THRESHOLD {
        bps += config.disagreement_add_bps.max(0.0);
        adjustments.push("disagreement");
    }
    if input.spread >= WIDE_SPREAD_THRESHOLD {
        bps += config.wide_spread_add_bps.max(0.0);
        adjustments.push("spread_wide");
    } else if input.spread >= SPREAD_THRESHOLD {
        bps += config.spread_add_bps.max(0.0);
        adjustments.push("spread");
    }
    if input.source_staleness_ms >= STALE_THRESHOLD_MS {
        bps += config.stale_add_bps.max(0.0);
        adjustments.push("stale");
    }
    if input
        .sigma_fast
        .filter(|fast| input.sigma_eff > 0.0 && *fast / input.sigma_eff >= NOISE_RATIO_THRESHOLD)
        .is_some()
    {
        bps += config.noise_add_bps.max(0.0);
        adjustments.push("noise");
    }

    let min_bps = config.min_bps.max(0.0);
    let max_bps = config.max_bps.max(min_bps);
    let effective_bps = bps.clamp(min_bps, max_bps);
    let floor_usd = (input.current_price.abs() * effective_bps / 10_000.0)
        .is_finite()
        .then_some(input.current_price.abs() * effective_bps / 10_000.0)
        .filter(|value| *value > 0.0);

    PriceToBeatIvExpectedMoveFloorEvaluation {
        mode: PriceToBeatIvMinExpectedMoveMode::Adaptive,
        base_bps: Some(config.base_bps),
        effective_bps: Some(effective_bps),
        floor_usd,
        adjustments,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_mode_does_not_add_floor() {
        let evaluation = evaluate_expected_move_floor(
            &PriceToBeatIvExpectedMoveFloorConfig::default(),
            PriceToBeatIvExpectedMoveFloorInput {
                current_price: 60_000.0,
                spread: 0.05,
                source_staleness_ms: 3_000,
                sigma_fast: Some(10.0),
                sigma_eff: 1.0,
                disagreement_abs: Some(0.5),
            },
        );

        assert_eq!(evaluation.mode, PriceToBeatIvMinExpectedMoveMode::Fixed);
        assert_eq!(evaluation.floor_usd, None);
    }

    #[test]
    fn adaptive_mode_applies_adjustments_and_clamps() {
        let mut config = PriceToBeatIvExpectedMoveFloorConfig::default();
        config.mode = PriceToBeatIvMinExpectedMoveMode::Adaptive;
        config.max_bps = 2.75;

        let evaluation = evaluate_expected_move_floor(
            &config,
            PriceToBeatIvExpectedMoveFloorInput {
                current_price: 60_000.0,
                spread: 0.03,
                source_staleness_ms: 2_200,
                sigma_fast: Some(2.0),
                sigma_eff: 1.0,
                disagreement_abs: Some(0.18),
            },
        );

        assert_eq!(evaluation.effective_bps, Some(2.75));
        assert_eq!(evaluation.floor_usd, Some(16.5));
        assert_eq!(
            evaluation.adjustments,
            vec!["disagreement_strong", "spread_wide", "stale", "noise"]
        );
    }
}
