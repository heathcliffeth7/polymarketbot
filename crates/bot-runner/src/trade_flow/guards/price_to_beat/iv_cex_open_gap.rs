use super::iv_mismatch_math::normal_cdf;
use crate::trade_flow::guards::cex_microstructure::{
    active_anchor_venue_for_asset, get_cex_venue_delta_snapshot, CexVenue, CexVenueDeltaSnapshot,
};
use serde_json::{json, Map, Value};

const DEFAULT_MIN_USD: f64 = 5.0;
const DEFAULT_MIN_Z: f64 = 0.25;
const DEFAULT_MAX_STALE_MS: i64 = 2_500;
const DEFAULT_DIFF_Z_BLOCK: f64 = 1.0;
const DEFAULT_MAX_DIFF_USD: f64 = 20.0;
const DEFAULT_MAX_DIFF_BPS: f64 = 3.5;
const DEFAULT_BOOK_MISMATCH_DISLOCATION: f64 = 0.20;
const NEUTRAL_EPSILON_USD: f64 = 0.000001;
const DEFAULT_DIVERGENCE_BLOCK_Z: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CexDecisionGapFallback {
    ChainlinkOnly,
    Block,
}

impl CexDecisionGapFallback {
    pub(crate) fn parse(s: Option<&str>) -> Option<Self> {
        match s? {
            "chainlink_only" => Some(Self::ChainlinkOnly),
            "block" => Some(Self::Block),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CexDecisionGap {
    pub(crate) effective_gap_usd: Option<f64>,
    pub(crate) source: &'static str,
    pub(crate) block_reason: Option<&'static str>,
}

impl CexDecisionGap {
    pub(crate) fn gap_strength(&self, expected_move_eff: f64) -> Option<f64> {
        if expected_move_eff <= 0.0 || !expected_move_eff.is_finite() {
            return None;
        }
        self.effective_gap_usd.map(|gap| gap / expected_move_eff)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CexOpenGapVenueState {
    Supporting,
    WeakPositive,
    Neutral,
    WeakAgainst,
    Against,
    Unavailable,
}

impl CexOpenGapVenueState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Supporting => "supporting",
            Self::WeakPositive => "weak_positive",
            Self::Neutral => "neutral",
            Self::WeakAgainst => "weak_against",
            Self::Against => "against",
            Self::Unavailable => "unavailable",
        }
    }

    fn usable(self) -> bool {
        !matches!(self, Self::Unavailable)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CexOpenGapConsensus {
    Strong,
    Mixed,
    Weak,
    Against,
    Partial,
    Unavailable,
}

impl CexOpenGapConsensus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Strong => "strong",
            Self::Mixed => "mixed",
            Self::Weak => "weak",
            Self::Against => "against",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvCexOpenGapConfig {
    pub(crate) enabled: bool,
    pub(crate) min_usd: f64,
    pub(crate) min_z: f64,
    pub(crate) max_stale_ms: i64,
    pub(crate) apply_negative_conservative_cap: bool,
    pub(crate) lag_guard_enabled: bool,
    pub(crate) diff_z_block: f64,
    pub(crate) max_diff_usd: f64,
    pub(crate) max_diff_bps: f64,
    pub(crate) book_mismatch_dislocation: f64,
    pub(crate) decision_gap_enabled: bool,
    pub(crate) decision_gap_fallback: CexDecisionGapFallback,
    pub(crate) divergence_hard_block_enabled: bool,
    pub(crate) divergence_block_z: f64,
    pub(crate) cex_lead_override_enabled: bool,
}

impl Default for PriceToBeatIvCexOpenGapConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_usd: DEFAULT_MIN_USD,
            min_z: DEFAULT_MIN_Z,
            max_stale_ms: DEFAULT_MAX_STALE_MS,
            apply_negative_conservative_cap: false,
            lag_guard_enabled: false,
            diff_z_block: DEFAULT_DIFF_Z_BLOCK,
            max_diff_usd: DEFAULT_MAX_DIFF_USD,
            max_diff_bps: DEFAULT_MAX_DIFF_BPS,
            book_mismatch_dislocation: DEFAULT_BOOK_MISMATCH_DISLOCATION,
            decision_gap_enabled: true,
            decision_gap_fallback: CexDecisionGapFallback::Block,
            divergence_hard_block_enabled: true,
            divergence_block_z: DEFAULT_DIVERGENCE_BLOCK_Z,
            cex_lead_override_enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvCexOpenGapVenueEvaluation {
    pub(crate) venue: &'static str,
    pub(crate) open_mid: Option<f64>,
    pub(crate) current_mid: Option<f64>,
    pub(crate) signed_gap: Option<f64>,
    pub(crate) gap_z: Option<f64>,
    pub(crate) state: CexOpenGapVenueState,
    pub(crate) open_source: Option<&'static str>,
    pub(crate) open_lag_ms: Option<i64>,
    pub(crate) book_staleness_ms: Option<i64>,
    pub(crate) error: Option<String>,
}

impl PriceToBeatIvCexOpenGapVenueEvaluation {
    fn unavailable(venue: &'static str, error: Option<String>) -> Self {
        Self {
            venue,
            open_mid: None,
            current_mid: None,
            signed_gap: None,
            gap_z: None,
            state: CexOpenGapVenueState::Unavailable,
            open_source: None,
            open_lag_ms: None,
            book_staleness_ms: None,
            error,
        }
    }

    fn from_snapshot(
        venue: &'static str,
        snapshot: CexVenueDeltaSnapshot,
        selected_side: &str,
        support_threshold_usd: f64,
        expected_move_eff: f64,
    ) -> Self {
        let signed_gap = if selected_side == "down" {
            -snapshot.delta_usd
        } else {
            snapshot.delta_usd
        };
        let gap_z = (expected_move_eff.is_finite() && expected_move_eff > 0.0)
            .then_some(signed_gap / expected_move_eff);
        Self {
            venue,
            open_mid: Some(snapshot.open_mid),
            current_mid: Some(snapshot.current_mid),
            signed_gap: Some(signed_gap),
            gap_z,
            state: classify_venue_state(signed_gap, support_threshold_usd),
            open_source: Some(snapshot.open_source),
            open_lag_ms: Some(snapshot.open_lag_ms),
            book_staleness_ms: Some(snapshot.book_staleness_ms),
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvCexOpenGapEvaluation {
    pub(crate) enabled: bool,
    pub(crate) support_threshold_usd: Option<f64>,
    pub(crate) chainlink_signed_gap: Option<f64>,
    pub(crate) binance: PriceToBeatIvCexOpenGapVenueEvaluation,
    pub(crate) anchor: PriceToBeatIvCexOpenGapVenueEvaluation,
    pub(crate) consensus: CexOpenGapConsensus,
    pub(crate) conservative_cex_gap: Option<f64>,
    pub(crate) effective_consensus_gap_usd: Option<f64>,
    pub(crate) q_final_before_cex_consensus: Option<f64>,
    pub(crate) q_final_after_cex_consensus: Option<f64>,
    pub(crate) cex_consensus_q_cap_applied: bool,
    pub(crate) q_consensus_cap: Option<f64>,
    pub(crate) negative_conservative_cap_skipped: bool,
    pub(crate) clean_lane: bool,
    pub(crate) chainlink_cex_diff_usd: Option<f64>,
    pub(crate) chainlink_cex_diff_z: Option<f64>,
    pub(crate) chainlink_cex_diff_bps: Option<f64>,
    pub(crate) lag_high: bool,
    pub(crate) block_reason: Option<&'static str>,
    pub(crate) chainlink_cex_book_mismatch_reason: Option<&'static str>,
    pub(crate) decision_gap_usd: Option<f64>,
    pub(crate) decision_gap_source: Option<&'static str>,
    pub(crate) decision_gap_strength: Option<f64>,
    pub(crate) divergence_block_reason: Option<&'static str>,
}

impl Default for PriceToBeatIvCexOpenGapEvaluation {
    fn default() -> Self {
        Self {
            enabled: false,
            support_threshold_usd: None,
            chainlink_signed_gap: None,
            binance: PriceToBeatIvCexOpenGapVenueEvaluation::unavailable("binance", None),
            anchor: PriceToBeatIvCexOpenGapVenueEvaluation::unavailable("anchor", None),
            consensus: CexOpenGapConsensus::Unavailable,
            conservative_cex_gap: None,
            effective_consensus_gap_usd: None,
            q_final_before_cex_consensus: None,
            q_final_after_cex_consensus: None,
            cex_consensus_q_cap_applied: false,
            q_consensus_cap: None,
            negative_conservative_cap_skipped: false,
            clean_lane: false,
            chainlink_cex_diff_usd: None,
            chainlink_cex_diff_z: None,
            chainlink_cex_diff_bps: None,
            lag_high: false,
            block_reason: None,
            chainlink_cex_book_mismatch_reason: None,
            decision_gap_usd: None,
            decision_gap_source: None,
            decision_gap_strength: None,
            divergence_block_reason: None,
        }
    }
}

impl PriceToBeatIvCexOpenGapEvaluation {
    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert("cex_open_gap_enabled".to_string(), json!(self.enabled));
        obj.insert(
            "cex_support_threshold_usd".to_string(),
            json!(self.support_threshold_usd),
        );
        obj.insert(
            "chainlink_signed_gap".to_string(),
            json!(self.chainlink_signed_gap),
        );
        append_venue(obj, "binance", &self.binance);
        append_venue(obj, self.anchor.venue, &self.anchor);
        append_venue(obj, "anchor", &self.anchor);
        obj.insert(
            "cex_open_gap_anchor_venue".to_string(),
            json!(self.anchor.venue),
        );
        obj.insert(
            "cex_open_gap_consensus".to_string(),
            json!(self.consensus.as_str()),
        );
        obj.insert(
            "cex_direction_consensus".to_string(),
            json!(self.consensus.as_str()),
        );
        obj.insert(
            "conservative_cex_gap".to_string(),
            json!(self.conservative_cex_gap),
        );
        obj.insert(
            "effective_consensus_gap_usd".to_string(),
            json!(self.effective_consensus_gap_usd),
        );
        obj.insert(
            "q_final_before_cex_consensus".to_string(),
            json!(self.q_final_before_cex_consensus),
        );
        obj.insert(
            "q_final_after_cex_consensus".to_string(),
            json!(self.q_final_after_cex_consensus),
        );
        obj.insert(
            "cex_consensus_q_cap_applied".to_string(),
            json!(self.cex_consensus_q_cap_applied),
        );
        obj.insert(
            "cex_consensus_q_cap".to_string(),
            json!(self.q_consensus_cap),
        );
        obj.insert(
            "cex_negative_conservative_cap_skipped".to_string(),
            json!(self.negative_conservative_cap_skipped),
        );
        obj.insert(
            "cex_open_gap_clean_lane".to_string(),
            json!(self.clean_lane),
        );
        obj.insert(
            "chainlink_cex_diff_usd".to_string(),
            json!(self.chainlink_cex_diff_usd),
        );
        obj.insert(
            "chainlink_cex_diff_z".to_string(),
            json!(self.chainlink_cex_diff_z),
        );
        obj.insert(
            "chainlink_cex_diff_bps".to_string(),
            json!(self.chainlink_cex_diff_bps),
        );
        obj.insert("cex_open_gap_lag_high".to_string(), json!(self.lag_high));
        obj.insert(
            "cex_open_gap_block_reason".to_string(),
            json!(self.block_reason),
        );
        obj.insert(
            "chainlink_cex_book_mismatch_reason".to_string(),
            json!(self.chainlink_cex_book_mismatch_reason),
        );
        obj.insert("decision_gap_usd".to_string(), json!(self.decision_gap_usd));
        obj.insert(
            "decision_gap_source".to_string(),
            json!(self.decision_gap_source),
        );
        obj.insert(
            "decision_gap_strength".to_string(),
            json!(self.decision_gap_strength),
        );
        obj.insert(
            "oracle_cex_divergence_block_reason".to_string(),
            json!(self.divergence_block_reason),
        );
    }
}

pub(crate) struct PriceToBeatIvCexOpenGapInput<'a> {
    pub(crate) config: PriceToBeatIvCexOpenGapConfig,
    pub(crate) market_slug: &'a str,
    pub(crate) asset: &'a str,
    pub(crate) selected_side: &'a str,
    pub(crate) current_price: f64,
    pub(crate) chainlink_signed_gap: f64,
    pub(crate) expected_move_eff: f64,
    pub(crate) q_final_before: f64,
}

pub(crate) fn evaluate_price_to_beat_iv_cex_open_gap(
    input: PriceToBeatIvCexOpenGapInput<'_>,
) -> PriceToBeatIvCexOpenGapEvaluation {
    let support_threshold_usd = support_threshold_usd(
        input.config.min_usd,
        input.config.min_z,
        input.expected_move_eff,
    );
    let mut evaluation = PriceToBeatIvCexOpenGapEvaluation {
        enabled: input.config.enabled,
        support_threshold_usd: Some(support_threshold_usd),
        chainlink_signed_gap: Some(input.chainlink_signed_gap),
        q_final_before_cex_consensus: Some(input.q_final_before),
        q_final_after_cex_consensus: Some(input.q_final_before),
        ..Default::default()
    };
    let needs_venues = input.config.enabled
        || input.config.decision_gap_enabled
        || input.config.divergence_hard_block_enabled;
    if !needs_venues {
        return evaluation;
    }

    let Some(window_start) = crate::MarketCycleId(input.market_slug.to_string()).start_time()
    else {
        evaluation.binance = PriceToBeatIvCexOpenGapVenueEvaluation::unavailable(
            "binance",
            Some("market_window_unavailable".to_string()),
        );
        let anchor = active_anchor_venue_for_asset(input.asset);
        evaluation.anchor = PriceToBeatIvCexOpenGapVenueEvaluation::unavailable(
            anchor.as_str(),
            Some("market_window_unavailable".to_string()),
        );
        return evaluation;
    };
    let window_start_ms = window_start.timestamp_millis();
    let anchor_venue = active_anchor_venue_for_asset(input.asset);
    evaluation.binance = load_venue(
        input.asset,
        CexVenue::Binance,
        window_start_ms,
        input.selected_side,
        support_threshold_usd,
        input.expected_move_eff,
        input.config.max_stale_ms,
    );
    evaluation.anchor = load_venue(
        input.asset,
        anchor_venue,
        window_start_ms,
        input.selected_side,
        support_threshold_usd,
        input.expected_move_eff,
        input.config.max_stale_ms,
    );
    evaluation.consensus = classify_consensus(evaluation.binance.state, evaluation.anchor.state);
    evaluation.conservative_cex_gap =
        conservative_gap(evaluation.binance.signed_gap, evaluation.anchor.signed_gap);
    if input.config.enabled {
        evaluation.clean_lane = matches!(evaluation.consensus, CexOpenGapConsensus::Strong);
        evaluation.block_reason = matches!(evaluation.consensus, CexOpenGapConsensus::Against)
            .then_some("blocked_cex_open_gap_both_against");
        apply_q_cap(&mut evaluation, &input);
    }
    apply_lag_metrics(&mut evaluation, &input);
    if input.config.divergence_hard_block_enabled {
        apply_divergence_hard_block(&mut evaluation, &input);
    }
    if input.config.decision_gap_enabled {
        let decision =
            compute_decision_gap_inner(&input.config, input.chainlink_signed_gap, &evaluation);
        evaluation.decision_gap_usd = decision.effective_gap_usd;
        evaluation.decision_gap_source = Some(decision.source);
        evaluation.decision_gap_strength = decision.gap_strength(input.expected_move_eff);
    }
    evaluation
}

fn load_venue(
    asset: &str,
    venue: CexVenue,
    window_start_ms: i64,
    selected_side: &str,
    support_threshold_usd: f64,
    expected_move_eff: f64,
    max_stale_ms: i64,
) -> PriceToBeatIvCexOpenGapVenueEvaluation {
    match get_cex_venue_delta_snapshot(
        asset,
        venue,
        window_start_ms,
        support_threshold_usd,
        max_stale_ms.max(0),
    ) {
        Ok(snapshot) => PriceToBeatIvCexOpenGapVenueEvaluation::from_snapshot(
            venue.as_str(),
            snapshot,
            selected_side,
            support_threshold_usd,
            expected_move_eff,
        ),
        Err(err) => PriceToBeatIvCexOpenGapVenueEvaluation::unavailable(
            venue.as_str(),
            Some(err.to_string()),
        ),
    }
}

fn apply_q_cap(
    evaluation: &mut PriceToBeatIvCexOpenGapEvaluation,
    input: &PriceToBeatIvCexOpenGapInput<'_>,
) {
    let Some(conservative_cex_gap) = evaluation.conservative_cex_gap else {
        return;
    };
    let cap_allowed = match evaluation.consensus {
        CexOpenGapConsensus::Strong | CexOpenGapConsensus::Weak => true,
        CexOpenGapConsensus::Mixed => {
            conservative_cex_gap >= 0.0
                || input.config.apply_negative_conservative_cap
                || (input.config.decision_gap_enabled && has_against_venue(evaluation))
        }
        CexOpenGapConsensus::Against
        | CexOpenGapConsensus::Partial
        | CexOpenGapConsensus::Unavailable => false,
    };
    evaluation.negative_conservative_cap_skipped =
        matches!(evaluation.consensus, CexOpenGapConsensus::Mixed)
            && conservative_cex_gap < 0.0
            && !cap_allowed;
    if !cap_allowed || input.expected_move_eff <= 0.0 || !input.expected_move_eff.is_finite() {
        return;
    }
    let effective_gap = input.chainlink_signed_gap.min(conservative_cex_gap);
    let q_cap = normal_cdf(effective_gap / input.expected_move_eff);
    if !q_cap.is_finite() {
        return;
    }
    let q_after = input.q_final_before.min(q_cap);
    evaluation.effective_consensus_gap_usd = Some(effective_gap);
    evaluation.q_consensus_cap = Some(q_cap);
    evaluation.q_final_after_cex_consensus = Some(q_after);
    evaluation.cex_consensus_q_cap_applied = q_after + 0.000000001 < input.q_final_before;
}

fn apply_lag_metrics(
    evaluation: &mut PriceToBeatIvCexOpenGapEvaluation,
    input: &PriceToBeatIvCexOpenGapInput<'_>,
) {
    let Some(conservative_cex_gap) = evaluation.conservative_cex_gap else {
        return;
    };
    let diff_usd = (input.chainlink_signed_gap - conservative_cex_gap).abs();
    evaluation.chainlink_cex_diff_usd = Some(diff_usd);
    if input.expected_move_eff.is_finite() && input.expected_move_eff > 0.0 {
        evaluation.chainlink_cex_diff_z = Some(diff_usd / input.expected_move_eff);
    }
    if input.current_price.is_finite() && input.current_price.abs() > 0.0 {
        evaluation.chainlink_cex_diff_bps = Some(diff_usd / input.current_price.abs() * 10_000.0);
    }
    evaluation.lag_high = evaluation
        .chainlink_cex_diff_z
        .map(|value| value >= input.config.diff_z_block)
        .unwrap_or(false)
        || diff_usd >= input.config.max_diff_usd
        || evaluation
            .chainlink_cex_diff_bps
            .map(|value| value >= input.config.max_diff_bps)
            .unwrap_or(false);
}

fn apply_divergence_hard_block(
    evaluation: &mut PriceToBeatIvCexOpenGapEvaluation,
    input: &PriceToBeatIvCexOpenGapInput<'_>,
) {
    let Some(diff_z) = evaluation.chainlink_cex_diff_z else {
        return;
    };
    if diff_z >= input.config.divergence_block_z
        && !cex_lead_override_applies(&input.config, evaluation, input.chainlink_signed_gap)
    {
        evaluation.divergence_block_reason = Some("blocked_oracle_cex_divergence");
    }
}

pub(crate) fn cex_lead_override_applies(
    config: &PriceToBeatIvCexOpenGapConfig,
    evaluation: &PriceToBeatIvCexOpenGapEvaluation,
    chainlink_signed_gap: f64,
) -> bool {
    let Some(conservative_cex_gap) = evaluation.conservative_cex_gap else {
        return false;
    };
    config.cex_lead_override_enabled
        && evaluation.consensus == CexOpenGapConsensus::Strong
        && chainlink_signed_gap.is_finite()
        && chainlink_signed_gap > 0.0
        && conservative_cex_gap.is_finite()
        && conservative_cex_gap >= chainlink_signed_gap
}

fn compute_decision_gap_inner(
    config: &PriceToBeatIvCexOpenGapConfig,
    chainlink_signed_gap: f64,
    evaluation: &PriceToBeatIvCexOpenGapEvaluation,
) -> CexDecisionGap {
    if !config.decision_gap_enabled {
        return CexDecisionGap {
            effective_gap_usd: Some(chainlink_signed_gap),
            source: "disabled",
            block_reason: None,
        };
    }
    match evaluation.consensus {
        CexOpenGapConsensus::Strong | CexOpenGapConsensus::Weak | CexOpenGapConsensus::Against => {
            if let Some(cex_gap) = evaluation.conservative_cex_gap {
                CexDecisionGap {
                    effective_gap_usd: Some(chainlink_signed_gap.min(cex_gap)),
                    source: "min_chainlink_cex",
                    block_reason: None,
                }
            } else {
                decision_gap_fallback(config, chainlink_signed_gap)
            }
        }
        CexOpenGapConsensus::Mixed => {
            if let Some(cex_gap) = evaluation.conservative_cex_gap {
                if cex_gap < 0.0 && !has_against_venue(evaluation) {
                    CexDecisionGap {
                        effective_gap_usd: Some(chainlink_signed_gap),
                        source: "chainlink_only_weak_mixed_cex",
                        block_reason: None,
                    }
                } else {
                    CexDecisionGap {
                        effective_gap_usd: Some(chainlink_signed_gap.min(cex_gap)),
                        source: "min_chainlink_cex",
                        block_reason: None,
                    }
                }
            } else {
                decision_gap_fallback(config, chainlink_signed_gap)
            }
        }
        CexOpenGapConsensus::Partial => {
            let single_gap = evaluation
                .binance
                .signed_gap
                .or(evaluation.anchor.signed_gap);
            if let Some(cex_gap) = single_gap {
                CexDecisionGap {
                    effective_gap_usd: Some(chainlink_signed_gap.min(cex_gap)),
                    source: "min_chainlink_single_venue",
                    block_reason: None,
                }
            } else {
                decision_gap_fallback(config, chainlink_signed_gap)
            }
        }
        CexOpenGapConsensus::Unavailable => decision_gap_fallback(config, chainlink_signed_gap),
    }
}

fn has_against_venue(evaluation: &PriceToBeatIvCexOpenGapEvaluation) -> bool {
    matches!(evaluation.binance.state, CexOpenGapVenueState::Against)
        || matches!(evaluation.anchor.state, CexOpenGapVenueState::Against)
}

fn decision_gap_fallback(
    config: &PriceToBeatIvCexOpenGapConfig,
    chainlink_signed_gap: f64,
) -> CexDecisionGap {
    match config.decision_gap_fallback {
        CexDecisionGapFallback::Block => CexDecisionGap {
            effective_gap_usd: None,
            source: "blocked",
            block_reason: Some("blocked_decision_gap_cex_unavailable"),
        },
        CexDecisionGapFallback::ChainlinkOnly => CexDecisionGap {
            effective_gap_usd: Some(chainlink_signed_gap),
            source: "chainlink_only",
            block_reason: None,
        },
    }
}

pub(crate) fn cex_open_gap_book_mismatch_reason(
    config: &PriceToBeatIvCexOpenGapConfig,
    evaluation: &PriceToBeatIvCexOpenGapEvaluation,
    model_book_dislocation: Option<f64>,
) -> Option<&'static str> {
    if config.enabled
        && config.lag_guard_enabled
        && evaluation.lag_high
        && model_book_dislocation
            .map(|value| value >= config.book_mismatch_dislocation)
            .unwrap_or(false)
    {
        Some("blocked_chainlink_cex_book_mismatch")
    } else {
        None
    }
}

fn support_threshold_usd(min_usd: f64, min_z: f64, expected_move_eff: f64) -> f64 {
    min_usd
        .max(expected_move_eff.max(0.0) * min_z.max(0.0))
        .max(0.0)
}

fn classify_venue_state(signed_gap: f64, support_threshold_usd: f64) -> CexOpenGapVenueState {
    if signed_gap >= support_threshold_usd {
        CexOpenGapVenueState::Supporting
    } else if signed_gap < -support_threshold_usd {
        CexOpenGapVenueState::Against
    } else if signed_gap.abs() <= NEUTRAL_EPSILON_USD {
        CexOpenGapVenueState::Neutral
    } else if signed_gap > 0.0 {
        CexOpenGapVenueState::WeakPositive
    } else {
        CexOpenGapVenueState::WeakAgainst
    }
}

fn classify_consensus(
    binance: CexOpenGapVenueState,
    anchor: CexOpenGapVenueState,
) -> CexOpenGapConsensus {
    let usable_count = [binance, anchor]
        .into_iter()
        .filter(|state| state.usable())
        .count();
    if usable_count == 0 {
        CexOpenGapConsensus::Unavailable
    } else if usable_count == 1 {
        CexOpenGapConsensus::Partial
    } else if matches!(binance, CexOpenGapVenueState::Against)
        && matches!(anchor, CexOpenGapVenueState::Against)
    {
        CexOpenGapConsensus::Against
    } else if matches!(binance, CexOpenGapVenueState::Supporting)
        && matches!(anchor, CexOpenGapVenueState::Supporting)
    {
        CexOpenGapConsensus::Strong
    } else if matches!(binance, CexOpenGapVenueState::WeakPositive)
        && matches!(anchor, CexOpenGapVenueState::WeakPositive)
    {
        CexOpenGapConsensus::Weak
    } else {
        CexOpenGapConsensus::Mixed
    }
}

fn conservative_gap(binance: Option<f64>, anchor: Option<f64>) -> Option<f64> {
    binance.zip(anchor).map(|(left, right)| left.min(right))
}

fn append_venue(
    obj: &mut Map<String, Value>,
    prefix: &str,
    venue: &PriceToBeatIvCexOpenGapVenueEvaluation,
) {
    obj.insert(format!("{prefix}_5m_open"), json!(venue.open_mid));
    obj.insert(format!("{prefix}_current_mid"), json!(venue.current_mid));
    obj.insert(format!("{prefix}_signed_gap"), json!(venue.signed_gap));
    obj.insert(format!("{prefix}_gap_z"), json!(venue.gap_z));
    obj.insert(format!("{prefix}_state"), json!(venue.state.as_str()));
    obj.insert(format!("{prefix}_open_source"), json!(venue.open_source));
    obj.insert(format!("{prefix}_open_lag_ms"), json!(venue.open_lag_ms));
    obj.insert(
        format!("{prefix}_book_staleness_ms"),
        json!(venue.book_staleness_ms),
    );
    obj.insert(format!("{prefix}_error"), json!(venue.error));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn venue(
        state: CexOpenGapVenueState,
        signed_gap: Option<f64>,
    ) -> PriceToBeatIvCexOpenGapVenueEvaluation {
        PriceToBeatIvCexOpenGapVenueEvaluation {
            venue: "test",
            open_mid: Some(100.0),
            current_mid: Some(100.0 + signed_gap.unwrap_or(0.0)),
            signed_gap,
            gap_z: signed_gap.map(|gap| gap / 10.0),
            state,
            open_source: Some("rest_open"),
            open_lag_ms: Some(0),
            book_staleness_ms: Some(10),
            error: None,
        }
    }

    #[test]
    fn classifies_venue_states_with_threshold() {
        assert_eq!(
            classify_venue_state(5.0, 5.0),
            CexOpenGapVenueState::Supporting
        );
        assert_eq!(
            classify_venue_state(2.0, 5.0),
            CexOpenGapVenueState::WeakPositive
        );
        assert_eq!(
            classify_venue_state(0.0, 5.0),
            CexOpenGapVenueState::Neutral
        );
        assert_eq!(
            classify_venue_state(-2.0, 5.0),
            CexOpenGapVenueState::WeakAgainst
        );
        assert_eq!(
            classify_venue_state(-5.0, 5.0),
            CexOpenGapVenueState::WeakAgainst
        );
        assert_eq!(
            classify_venue_state(-5.000001, 5.0),
            CexOpenGapVenueState::Against
        );
    }

    #[test]
    fn classifies_consensus_without_noise_hard_block() {
        assert_eq!(
            classify_consensus(
                CexOpenGapVenueState::Against,
                CexOpenGapVenueState::WeakAgainst
            ),
            CexOpenGapConsensus::Mixed
        );
        assert_eq!(
            classify_consensus(CexOpenGapVenueState::Against, CexOpenGapVenueState::Against),
            CexOpenGapConsensus::Against
        );
        assert_eq!(
            classify_consensus(
                CexOpenGapVenueState::WeakPositive,
                CexOpenGapVenueState::WeakPositive
            ),
            CexOpenGapConsensus::Weak
        );
    }

    #[test]
    fn caps_q_for_weak_consensus_and_records_before_after() {
        let config = PriceToBeatIvCexOpenGapConfig {
            enabled: true,
            ..Default::default()
        };
        let input = PriceToBeatIvCexOpenGapInput {
            config,
            market_slug: "btc-updown-5m-1",
            asset: "btc",
            selected_side: "up",
            current_price: 100_000.0,
            chainlink_signed_gap: 50.0,
            expected_move_eff: 20.0,
            q_final_before: 0.97,
        };
        let mut evaluation = PriceToBeatIvCexOpenGapEvaluation {
            enabled: true,
            binance: venue(CexOpenGapVenueState::WeakPositive, Some(18.0)),
            anchor: venue(CexOpenGapVenueState::WeakPositive, Some(12.0)),
            consensus: CexOpenGapConsensus::Weak,
            conservative_cex_gap: Some(12.0),
            q_final_before_cex_consensus: Some(0.97),
            q_final_after_cex_consensus: Some(0.97),
            ..Default::default()
        };

        apply_q_cap(&mut evaluation, &input);

        assert_eq!(evaluation.effective_consensus_gap_usd, Some(12.0));
        assert_eq!(evaluation.q_final_before_cex_consensus, Some(0.97));
        assert!(matches!(evaluation.q_final_after_cex_consensus, Some(q) if q < 0.97));
        assert!(evaluation.cex_consensus_q_cap_applied);
    }

    #[test]
    fn mixed_negative_cap_is_skipped_by_default() {
        let config = PriceToBeatIvCexOpenGapConfig {
            enabled: true,
            apply_negative_conservative_cap: false,
            decision_gap_enabled: false,
            ..Default::default()
        };
        let input = PriceToBeatIvCexOpenGapInput {
            config,
            market_slug: "btc-updown-5m-1",
            asset: "btc",
            selected_side: "up",
            current_price: 100_000.0,
            chainlink_signed_gap: 50.0,
            expected_move_eff: 20.0,
            q_final_before: 0.97,
        };
        let mut evaluation = PriceToBeatIvCexOpenGapEvaluation {
            enabled: true,
            consensus: CexOpenGapConsensus::Mixed,
            conservative_cex_gap: Some(-7.0),
            q_final_before_cex_consensus: Some(0.97),
            q_final_after_cex_consensus: Some(0.97),
            ..Default::default()
        };

        apply_q_cap(&mut evaluation, &input);

        assert_eq!(evaluation.q_final_after_cex_consensus, Some(0.97));
        assert!(evaluation.negative_conservative_cap_skipped);
        assert!(!evaluation.cex_consensus_q_cap_applied);
    }

    #[test]
    fn decision_gap_takes_min_of_chainlink_and_cex() {
        let config = PriceToBeatIvCexOpenGapConfig {
            decision_gap_enabled: true,
            ..Default::default()
        };
        let evaluation = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Weak,
            conservative_cex_gap: Some(5.51),
            ..Default::default()
        };
        let decision = compute_decision_gap_inner(&config, 90.58, &evaluation);
        assert_eq!(decision.effective_gap_usd, Some(5.51));
        assert_eq!(decision.source, "min_chainlink_cex");
        assert!(decision.block_reason.is_none());
        let strength = decision.gap_strength(19.60).unwrap();
        assert!((strength - 5.51 / 19.60).abs() < 0.0001);
    }

    #[test]
    fn decision_gap_negative_cex_yields_negative() {
        let config = PriceToBeatIvCexOpenGapConfig {
            decision_gap_enabled: true,
            ..Default::default()
        };
        let evaluation = PriceToBeatIvCexOpenGapEvaluation {
            binance: venue(CexOpenGapVenueState::Against, Some(-7.0)),
            anchor: venue(CexOpenGapVenueState::WeakPositive, Some(2.0)),
            consensus: CexOpenGapConsensus::Mixed,
            conservative_cex_gap: Some(-7.0),
            ..Default::default()
        };
        let decision = compute_decision_gap_inner(&config, 90.58, &evaluation);
        assert_eq!(decision.effective_gap_usd, Some(-7.0));
        assert!(decision.gap_strength(19.60).unwrap() < 0.0);
    }

    #[test]
    fn decision_gap_ignores_mixed_weak_against_noise() {
        let config = PriceToBeatIvCexOpenGapConfig {
            decision_gap_enabled: true,
            ..Default::default()
        };
        let evaluation = PriceToBeatIvCexOpenGapEvaluation {
            binance: venue(CexOpenGapVenueState::WeakPositive, Some(0.015)),
            anchor: venue(CexOpenGapVenueState::WeakAgainst, Some(-0.005)),
            consensus: CexOpenGapConsensus::Mixed,
            conservative_cex_gap: Some(-0.005),
            ..Default::default()
        };
        let decision = compute_decision_gap_inner(&config, 0.0156, &evaluation);
        assert_eq!(decision.effective_gap_usd, Some(0.0156));
        assert_eq!(decision.source, "chainlink_only_weak_mixed_cex");
        assert!(decision.block_reason.is_none());
    }

    #[test]
    fn decision_gap_ignores_mixed_weak_against_at_threshold() {
        let config = PriceToBeatIvCexOpenGapConfig {
            decision_gap_enabled: true,
            ..Default::default()
        };
        let evaluation = PriceToBeatIvCexOpenGapEvaluation {
            binance: venue(CexOpenGapVenueState::WeakPositive, Some(0.015)),
            anchor: venue(classify_venue_state(-0.005, 0.005), Some(-0.005)),
            consensus: CexOpenGapConsensus::Mixed,
            conservative_cex_gap: Some(-0.005),
            ..Default::default()
        };
        assert_eq!(evaluation.anchor.state, CexOpenGapVenueState::WeakAgainst);
        let decision = compute_decision_gap_inner(&config, 0.0156, &evaluation);
        assert_eq!(decision.effective_gap_usd, Some(0.0156));
        assert_eq!(decision.source, "chainlink_only_weak_mixed_cex");
    }

    #[test]
    fn decision_gap_ignores_all_weak_against_noise() {
        let config = PriceToBeatIvCexOpenGapConfig {
            decision_gap_enabled: true,
            ..Default::default()
        };
        let evaluation = PriceToBeatIvCexOpenGapEvaluation {
            binance: venue(CexOpenGapVenueState::WeakAgainst, Some(-0.004)),
            anchor: venue(CexOpenGapVenueState::WeakAgainst, Some(-0.005)),
            consensus: CexOpenGapConsensus::Mixed,
            conservative_cex_gap: Some(-0.005),
            ..Default::default()
        };
        let decision = compute_decision_gap_inner(&config, 0.0156, &evaluation);
        assert_eq!(decision.effective_gap_usd, Some(0.0156));
        assert_eq!(decision.source, "chainlink_only_weak_mixed_cex");
        assert!(decision.block_reason.is_none());
    }

    #[test]
    fn decision_gap_single_venue_partial() {
        let config = PriceToBeatIvCexOpenGapConfig {
            decision_gap_enabled: true,
            ..Default::default()
        };
        let evaluation = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Partial,
            conservative_cex_gap: None,
            binance: venue(CexOpenGapVenueState::Supporting, Some(30.0)),
            anchor: PriceToBeatIvCexOpenGapVenueEvaluation::unavailable("okx", None),
            ..Default::default()
        };
        let decision = compute_decision_gap_inner(&config, 112.18, &evaluation);
        assert_eq!(decision.effective_gap_usd, Some(30.0));
        assert_eq!(decision.source, "min_chainlink_single_venue");
    }

    #[test]
    fn decision_gap_blocks_when_cex_unavailable() {
        let config = PriceToBeatIvCexOpenGapConfig {
            decision_gap_enabled: true,
            decision_gap_fallback: CexDecisionGapFallback::Block,
            ..Default::default()
        };
        let evaluation = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Unavailable,
            conservative_cex_gap: None,
            ..Default::default()
        };
        let decision = compute_decision_gap_inner(&config, 90.0, &evaluation);
        assert!(decision.effective_gap_usd.is_none());
        assert_eq!(
            decision.block_reason,
            Some("blocked_decision_gap_cex_unavailable")
        );
    }

    #[test]
    fn decision_gap_disabled_passes_chainlink_through() {
        let config = PriceToBeatIvCexOpenGapConfig {
            decision_gap_enabled: false,
            ..Default::default()
        };
        let evaluation = PriceToBeatIvCexOpenGapEvaluation::default();
        let decision = compute_decision_gap_inner(&config, 90.58, &evaluation);
        assert_eq!(decision.effective_gap_usd, Some(90.58));
        assert_eq!(decision.source, "disabled");
    }

    #[test]
    fn mixed_negative_cap_applies_when_decision_gap_enabled() {
        let config = PriceToBeatIvCexOpenGapConfig {
            enabled: true,
            apply_negative_conservative_cap: false,
            decision_gap_enabled: true,
            ..Default::default()
        };
        let input = PriceToBeatIvCexOpenGapInput {
            config,
            market_slug: "btc-updown-5m-1",
            asset: "btc",
            selected_side: "up",
            current_price: 100_000.0,
            chainlink_signed_gap: 90.58,
            expected_move_eff: 19.60,
            q_final_before: 0.97,
        };
        let mut evaluation = PriceToBeatIvCexOpenGapEvaluation {
            enabled: true,
            binance: venue(CexOpenGapVenueState::Against, Some(-7.0)),
            anchor: venue(CexOpenGapVenueState::WeakPositive, Some(2.0)),
            consensus: CexOpenGapConsensus::Mixed,
            conservative_cex_gap: Some(-7.0),
            q_final_before_cex_consensus: Some(0.97),
            q_final_after_cex_consensus: Some(0.97),
            ..Default::default()
        };
        apply_q_cap(&mut evaluation, &input);
        assert!(evaluation.cex_consensus_q_cap_applied);
        assert!(matches!(evaluation.q_final_after_cex_consensus, Some(q) if q < 0.97));
    }

    #[test]
    fn mixed_negative_cap_skips_weak_against_noise_when_decision_gap_enabled() {
        let config = PriceToBeatIvCexOpenGapConfig {
            enabled: true,
            apply_negative_conservative_cap: false,
            decision_gap_enabled: true,
            ..Default::default()
        };
        let input = PriceToBeatIvCexOpenGapInput {
            config,
            market_slug: "sol-updown-5m-1",
            asset: "sol",
            selected_side: "up",
            current_price: 70.0,
            chainlink_signed_gap: 0.0156,
            expected_move_eff: 0.0728,
            q_final_before: 0.58,
        };
        let mut evaluation = PriceToBeatIvCexOpenGapEvaluation {
            enabled: true,
            binance: venue(CexOpenGapVenueState::WeakPositive, Some(0.015)),
            anchor: venue(CexOpenGapVenueState::WeakAgainst, Some(-0.005)),
            consensus: CexOpenGapConsensus::Mixed,
            conservative_cex_gap: Some(-0.005),
            q_final_before_cex_consensus: Some(0.58),
            q_final_after_cex_consensus: Some(0.58),
            ..Default::default()
        };
        apply_q_cap(&mut evaluation, &input);
        assert_eq!(evaluation.q_final_after_cex_consensus, Some(0.58));
        assert!(evaluation.negative_conservative_cap_skipped);
        assert!(!evaluation.cex_consensus_q_cap_applied);
    }

    #[test]
    fn divergence_hard_block_fires_at_threshold() {
        let config = PriceToBeatIvCexOpenGapConfig {
            divergence_hard_block_enabled: true,
            divergence_block_z: 2.0,
            ..Default::default()
        };
        let input = PriceToBeatIvCexOpenGapInput {
            config,
            market_slug: "btc-updown-5m-1",
            asset: "btc",
            selected_side: "up",
            current_price: 100_000.0,
            chainlink_signed_gap: 90.0,
            expected_move_eff: 20.0,
            q_final_before: 0.97,
        };
        let mut evaluation = PriceToBeatIvCexOpenGapEvaluation {
            chainlink_cex_diff_z: Some(2.5),
            ..Default::default()
        };
        apply_divergence_hard_block(&mut evaluation, &input);
        assert_eq!(
            evaluation.divergence_block_reason,
            Some("blocked_oracle_cex_divergence")
        );
    }

    #[test]
    fn divergence_hard_block_does_not_fire_below_threshold() {
        let config = PriceToBeatIvCexOpenGapConfig {
            divergence_hard_block_enabled: true,
            divergence_block_z: 2.0,
            ..Default::default()
        };
        let input = PriceToBeatIvCexOpenGapInput {
            config,
            market_slug: "btc-updown-5m-1",
            asset: "btc",
            selected_side: "up",
            current_price: 100_000.0,
            chainlink_signed_gap: 20.0,
            expected_move_eff: 20.0,
            q_final_before: 0.97,
        };
        let mut evaluation = PriceToBeatIvCexOpenGapEvaluation {
            chainlink_cex_diff_z: Some(1.5),
            ..Default::default()
        };
        apply_divergence_hard_block(&mut evaluation, &input);
        assert!(evaluation.divergence_block_reason.is_none());
    }

    #[test]
    fn cex_lead_override_suppresses_divergence_when_strong_cex_leads_chainlink() {
        let config = PriceToBeatIvCexOpenGapConfig {
            cex_lead_override_enabled: true,
            divergence_hard_block_enabled: true,
            divergence_block_z: 2.0,
            ..Default::default()
        };
        let input = PriceToBeatIvCexOpenGapInput {
            config,
            market_slug: "btc-updown-5m-1",
            asset: "btc",
            selected_side: "up",
            current_price: 100_000.0,
            chainlink_signed_gap: 46.8,
            expected_move_eff: 13.25,
            q_final_before: 0.97,
        };
        let mut evaluation = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Strong,
            conservative_cex_gap: Some(77.5),
            chainlink_cex_diff_z: Some(2.316),
            ..Default::default()
        };
        apply_divergence_hard_block(&mut evaluation, &input);
        assert!(evaluation.divergence_block_reason.is_none());
    }

    #[test]
    fn cex_lead_override_keeps_divergence_when_chainlink_leads_cex() {
        let config = PriceToBeatIvCexOpenGapConfig {
            cex_lead_override_enabled: true,
            divergence_hard_block_enabled: true,
            divergence_block_z: 2.0,
            ..Default::default()
        };
        let input = PriceToBeatIvCexOpenGapInput {
            config,
            market_slug: "btc-updown-5m-1",
            asset: "btc",
            selected_side: "up",
            current_price: 100_000.0,
            chainlink_signed_gap: 77.5,
            expected_move_eff: 13.25,
            q_final_before: 0.97,
        };
        let mut evaluation = PriceToBeatIvCexOpenGapEvaluation {
            consensus: CexOpenGapConsensus::Strong,
            conservative_cex_gap: Some(46.8),
            chainlink_cex_diff_z: Some(2.316),
            ..Default::default()
        };
        apply_divergence_hard_block(&mut evaluation, &input);
        assert_eq!(
            evaluation.divergence_block_reason,
            Some("blocked_oracle_cex_divergence")
        );
    }

    #[test]
    fn cex_lead_override_requires_strong_positive_cex_confirmation() {
        let config = PriceToBeatIvCexOpenGapConfig {
            cex_lead_override_enabled: true,
            divergence_hard_block_enabled: true,
            divergence_block_z: 2.0,
            ..Default::default()
        };
        for (consensus, chainlink_gap) in [
            (CexOpenGapConsensus::Weak, 46.8),
            (CexOpenGapConsensus::Mixed, 46.8),
            (CexOpenGapConsensus::Partial, 46.8),
            (CexOpenGapConsensus::Strong, 0.0),
        ] {
            let mut evaluation = PriceToBeatIvCexOpenGapEvaluation {
                consensus,
                conservative_cex_gap: Some(77.5),
                chainlink_cex_diff_z: Some(2.316),
                ..Default::default()
            };
            let input = PriceToBeatIvCexOpenGapInput {
                config,
                market_slug: "btc-updown-5m-1",
                asset: "btc",
                selected_side: "up",
                current_price: 100_000.0,
                chainlink_signed_gap: chainlink_gap,
                expected_move_eff: 13.25,
                q_final_before: 0.97,
            };
            apply_divergence_hard_block(&mut evaluation, &input);
            assert_eq!(
                evaluation.divergence_block_reason,
                Some("blocked_oracle_cex_divergence")
            );
        }
    }
}
