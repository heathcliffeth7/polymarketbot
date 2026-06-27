use super::super::iv_cex_open_gap::{CexOpenGapConsensus, PriceToBeatIvCexOpenGapEvaluation};
use serde_json::{json, Map, Value};

const DEFAULT_SHALLOW_RATIO: f64 = 0.50;
const DEFAULT_MODERATE_RATIO: f64 = 1.00;
const FRESH_SAME_SIDE_MAX_SECONDS: f64 = 3.0;
const FRESH_CHAINLINK_CEX_DIFF_Z: f64 = 1.0;

const GAP_FAIL_SHALLOW_REASON: &str = "blocked_gap_fail_shallow_cex_support";
const FRESH_SHALLOW_REASON: &str = "blocked_fresh_chainlink_gap_shallow_cex";
const SHALLOW_UNCONFIRMED_REASON: &str = "blocked_shallow_cex_unconfirmed_entry";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvCexMagnitudeConfig {
    pub(crate) enabled: bool,
    pub(crate) shallow_ratio: f64,
    pub(crate) moderate_ratio: f64,
}

impl Default for PriceToBeatIvCexMagnitudeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            shallow_ratio: DEFAULT_SHALLOW_RATIO,
            moderate_ratio: DEFAULT_MODERATE_RATIO,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvCexMagnitudeEvaluation {
    pub(crate) enabled: bool,
    pub(crate) ratio: Option<f64>,
    pub(crate) consensus: &'static str,
    pub(crate) direction_consensus: &'static str,
    pub(crate) clean_lane: bool,
    pub(crate) block_reason: Option<&'static str>,
    pub(crate) shallow_ratio: f64,
    pub(crate) moderate_ratio: f64,
    pub(crate) conservative_cex_gap: Option<f64>,
    pub(crate) required_gap_usd: Option<f64>,
    pub(crate) below_required: bool,
    pub(crate) same_side_age_seconds: Option<f64>,
    pub(crate) book_confirmation_missing: bool,
    pub(crate) chainlink_cex_diff_z: Option<f64>,
    pub(crate) eq77_gap_override_requested: bool,
    pub(crate) eq77_gap_override_effective: bool,
    pub(crate) eq77_gap_override_suppressed_by_cex_magnitude: bool,
    pub(crate) override_blocked_by: Option<&'static str>,
}

impl Default for PriceToBeatIvCexMagnitudeEvaluation {
    fn default() -> Self {
        let config = PriceToBeatIvCexMagnitudeConfig::default();
        Self {
            enabled: false,
            ratio: None,
            consensus: "unavailable",
            direction_consensus: CexOpenGapConsensus::Unavailable.as_str(),
            clean_lane: false,
            block_reason: None,
            shallow_ratio: config.shallow_ratio,
            moderate_ratio: config.moderate_ratio,
            conservative_cex_gap: None,
            required_gap_usd: None,
            below_required: false,
            same_side_age_seconds: None,
            book_confirmation_missing: true,
            chainlink_cex_diff_z: None,
            eq77_gap_override_requested: false,
            eq77_gap_override_effective: false,
            eq77_gap_override_suppressed_by_cex_magnitude: false,
            override_blocked_by: None,
        }
    }
}

impl PriceToBeatIvCexMagnitudeEvaluation {
    pub(crate) fn is_shallow(&self) -> bool {
        self.enabled && self.consensus == "shallow"
    }

    pub(crate) fn record_eq77_gap_override(&mut self, requested: bool) -> bool {
        let suppressed = requested && self.is_shallow();
        self.eq77_gap_override_requested = requested;
        self.eq77_gap_override_effective = requested && !suppressed;
        self.eq77_gap_override_suppressed_by_cex_magnitude = suppressed;
        self.override_blocked_by = suppressed.then_some("shallow_cex_magnitude");
        self.eq77_gap_override_effective
    }

    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert(
            "cex_magnitude_guard_enabled".to_string(),
            json!(self.enabled),
        );
        obj.insert("cex_magnitude_ratio".to_string(), json!(self.ratio));
        obj.insert("cex_magnitude_consensus".to_string(), json!(self.consensus));
        obj.insert(
            "cex_direction_consensus".to_string(),
            json!(self.direction_consensus),
        );
        obj.insert(
            "cex_magnitude_clean_lane".to_string(),
            json!(self.clean_lane),
        );
        obj.insert(
            "cex_magnitude_block_reason".to_string(),
            json!(self.block_reason),
        );
        obj.insert(
            "cex_magnitude_shallow_ratio".to_string(),
            json!(self.shallow_ratio),
        );
        obj.insert(
            "cex_magnitude_moderate_ratio".to_string(),
            json!(self.moderate_ratio),
        );
        obj.insert(
            "cex_magnitude_required_gap_usd".to_string(),
            json!(self.required_gap_usd),
        );
        obj.insert(
            "cex_magnitude_below_required".to_string(),
            json!(self.below_required),
        );
        obj.insert(
            "eq77_gap_override_requested".to_string(),
            json!(self.eq77_gap_override_requested),
        );
        obj.insert(
            "eq77_gap_override_effective".to_string(),
            json!(self.eq77_gap_override_effective),
        );
        obj.insert(
            "eq77_gap_override_suppressed_by_cex_magnitude".to_string(),
            json!(self.eq77_gap_override_suppressed_by_cex_magnitude),
        );
        obj.insert(
            "override_blocked_by".to_string(),
            json!(self.override_blocked_by),
        );
    }
}

pub(crate) struct PriceToBeatIvCexMagnitudeInput<'a> {
    pub(crate) config: PriceToBeatIvCexMagnitudeConfig,
    pub(crate) gap_strength: f64,
    pub(crate) required_gap_strength: f64,
    pub(crate) required_gap_usd_for_ratio: f64,
    pub(crate) same_side_age_seconds: Option<f64>,
    pub(crate) book_confirmation_available: bool,
    pub(crate) cex_open_gap: &'a PriceToBeatIvCexOpenGapEvaluation,
}

pub(crate) fn evaluate_price_to_beat_iv_cex_magnitude(
    input: PriceToBeatIvCexMagnitudeInput<'_>,
) -> PriceToBeatIvCexMagnitudeEvaluation {
    let shallow_ratio = input.config.shallow_ratio.max(0.0);
    let moderate_ratio = input.config.moderate_ratio.max(shallow_ratio);
    let ratio = input
        .cex_open_gap
        .conservative_cex_gap
        .filter(|gap| gap.is_finite())
        .zip(
            (input.required_gap_usd_for_ratio.is_finite()
                && input.required_gap_usd_for_ratio > 0.0)
                .then_some(input.required_gap_usd_for_ratio),
        )
        .map(|(gap, required)| gap / required);
    let consensus = ratio
        .map(|ratio| classify_magnitude(ratio, shallow_ratio, moderate_ratio))
        .unwrap_or("unavailable");
    let below_required = input.gap_strength < input.required_gap_strength;
    let book_confirmation_missing = !input.book_confirmation_available;
    let fresh_same_side = input
        .same_side_age_seconds
        .map(|age| age < FRESH_SAME_SIDE_MAX_SECONDS)
        .unwrap_or(false);
    let chainlink_cex_diff_z = input.cex_open_gap.chainlink_cex_diff_z;
    let chainlink_cex_diff_high = chainlink_cex_diff_z
        .map(|value| value >= FRESH_CHAINLINK_CEX_DIFF_Z)
        .unwrap_or(false);
    let shallow = consensus == "shallow";
    let block_reason = if input.config.enabled && shallow && below_required {
        Some(GAP_FAIL_SHALLOW_REASON)
    } else if input.config.enabled
        && shallow
        && fresh_same_side
        && book_confirmation_missing
        && chainlink_cex_diff_high
    {
        Some(FRESH_SHALLOW_REASON)
    } else if input.config.enabled && shallow && fresh_same_side && book_confirmation_missing {
        Some(SHALLOW_UNCONFIRMED_REASON)
    } else {
        None
    };

    PriceToBeatIvCexMagnitudeEvaluation {
        enabled: input.config.enabled,
        ratio,
        consensus,
        direction_consensus: input.cex_open_gap.consensus.as_str(),
        clean_lane: input.config.enabled && consensus == "strong",
        block_reason,
        shallow_ratio,
        moderate_ratio,
        conservative_cex_gap: input.cex_open_gap.conservative_cex_gap,
        required_gap_usd: (input.required_gap_usd_for_ratio.is_finite()
            && input.required_gap_usd_for_ratio > 0.0)
            .then_some(input.required_gap_usd_for_ratio),
        below_required,
        same_side_age_seconds: input.same_side_age_seconds,
        book_confirmation_missing,
        chainlink_cex_diff_z,
        eq77_gap_override_requested: false,
        eq77_gap_override_effective: false,
        eq77_gap_override_suppressed_by_cex_magnitude: false,
        override_blocked_by: None,
    }
}

fn classify_magnitude(ratio: f64, shallow_ratio: f64, moderate_ratio: f64) -> &'static str {
    if !ratio.is_finite() {
        "unavailable"
    } else if ratio >= moderate_ratio {
        "strong"
    } else if ratio >= shallow_ratio {
        "moderate"
    } else {
        "shallow"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cex(conservative_gap: f64, diff_z: f64) -> PriceToBeatIvCexOpenGapEvaluation {
        PriceToBeatIvCexOpenGapEvaluation {
            enabled: true,
            consensus: CexOpenGapConsensus::Strong,
            clean_lane: true,
            conservative_cex_gap: Some(conservative_gap),
            chainlink_cex_diff_z: Some(diff_z),
            ..Default::default()
        }
    }

    fn eval(
        cex: &PriceToBeatIvCexOpenGapEvaluation,
        gap_strength: f64,
        required_gap_strength: f64,
        required_gap_usd: f64,
        same_side_age_seconds: Option<f64>,
    ) -> PriceToBeatIvCexMagnitudeEvaluation {
        evaluate_price_to_beat_iv_cex_magnitude(PriceToBeatIvCexMagnitudeInput {
            config: PriceToBeatIvCexMagnitudeConfig {
                enabled: true,
                ..Default::default()
            },
            gap_strength,
            required_gap_strength,
            required_gap_usd_for_ratio: required_gap_usd,
            same_side_age_seconds,
            book_confirmation_available: false,
            cex_open_gap: cex,
        })
    }

    #[test]
    fn cex_magnitude_blocks_gap_fail_with_shallow_support() {
        let cex = cex(0.0550, 0.80);
        let mut evaluation = eval(&cex, 0.8635, 2.0, 0.1258, Some(10.0));

        assert_eq!(evaluation.consensus, "shallow");
        assert!((evaluation.ratio.expect("ratio") - 0.43720190779014306).abs() < 1e-12);
        assert_eq!(
            evaluation.block_reason,
            Some("blocked_gap_fail_shallow_cex_support")
        );
        assert!(!evaluation.record_eq77_gap_override(false));
        assert!(!evaluation.record_eq77_gap_override(true));
        assert!(!evaluation.eq77_gap_override_effective);
        assert!(evaluation.eq77_gap_override_suppressed_by_cex_magnitude);
        assert_eq!(
            evaluation.override_blocked_by,
            Some("shallow_cex_magnitude")
        );
    }

    #[test]
    fn cex_magnitude_blocks_fresh_chainlink_gap_with_shallow_support() {
        let cex = cex(0.015, 1.20);
        let evaluation = eval(&cex, 2.0203, 1.9, 0.0613, Some(0.0));

        assert_eq!(evaluation.consensus, "shallow");
        assert_eq!(
            evaluation.block_reason,
            Some("blocked_fresh_chainlink_gap_shallow_cex")
        );
    }

    #[test]
    fn cex_magnitude_allows_moderate_support_without_gap_fail() {
        let cex = cex(2.065, 0.72);
        let evaluation = eval(&cex, 2.10, 2.0, 3.079, Some(5.0));

        assert_eq!(evaluation.consensus, "moderate");
        assert_eq!(evaluation.block_reason, None);
        assert!(!evaluation.is_shallow());
    }

    #[test]
    fn cex_magnitude_keeps_eq77_override_for_moderate_gap_fail() {
        let cex = cex(0.075, 0.72);
        let mut evaluation = eval(&cex, 0.90, 2.0, 0.10, Some(5.0));

        assert_eq!(evaluation.consensus, "moderate");
        assert_eq!(evaluation.block_reason, None);
        assert!(evaluation.record_eq77_gap_override(true));
        assert!(evaluation.eq77_gap_override_effective);
        assert!(!evaluation.eq77_gap_override_suppressed_by_cex_magnitude);
        assert_eq!(evaluation.override_blocked_by, None);
    }
}
