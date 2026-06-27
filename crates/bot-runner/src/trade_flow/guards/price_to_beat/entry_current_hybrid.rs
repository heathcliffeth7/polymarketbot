use super::current_price::{
    CURRENT_PRICE_SOURCE_CEX_CONSENSUS, CURRENT_PRICE_SOURCE_CHAINLINK,
    CURRENT_PRICE_SOURCE_CHAINLINK_CEX_CONSENSUS, resolve_price_to_beat_current_price_snapshot,
};
use super::iv_chainlink_stale_strong_gap_exception::ChainlinkStaleStrongGapRuntimeContext;
use super::*;
use crate::trade_flow::guards::cex_microstructure::{
    CexCurrentPriceSnapshot, CexMicrostructureSnapshotConfig, CexVenue,
    get_cex_current_price_snapshot,
};
use crate::trade_flow::guards::polymarket_price_to_beat::PolymarketPriceToBeatSnapshot;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(test)]
const BYBIT_MICRO_STALE_RETRY_TIMEOUT_MS: u64 = 150;
#[cfg(not(test))]
const BYBIT_MICRO_STALE_RETRY_TIMEOUT_MS: u64 = 500;
const BYBIT_MICRO_STALE_RETRY_STEP_MS: u64 = 25;
const BYBIT_MICRO_STALE_RETRY_MAX_OVER_MS: i64 = 750;
const CLEAN_PAIR_MAX_DIFF_USD: f64 = 20.0;
const CLEAN_PAIR_MAX_DIFF_BPS: f64 = 3.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CexEntryConsensusMode {
    #[default]
    BinanceCoinbase,
    AssetAutoPlusOneOrCleanPair,
    BybitPlusOne,
    BybitPlusOneOrCleanPair,
    OkxPlusOne,
    OkxPlusOneOrCleanPair,
    GatePlusOne,
    GatePlusOneOrCleanPair,
}

impl CexEntryConsensusMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::BinanceCoinbase => "binance_coinbase",
            Self::AssetAutoPlusOneOrCleanPair => "asset_auto_plus_one_or_clean_pair",
            Self::BybitPlusOne => "bybit_plus_one",
            Self::BybitPlusOneOrCleanPair => "bybit_plus_one_or_clean_pair",
            Self::OkxPlusOne => "okx_plus_one",
            Self::OkxPlusOneOrCleanPair => "okx_plus_one_or_clean_pair",
            Self::GatePlusOne => "gate_plus_one",
            Self::GatePlusOneOrCleanPair => "gate_plus_one_or_clean_pair",
        }
    }

    fn parse_known(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "" | "binance_coinbase" | "open_gap_binance_coinbase" => Some(Self::BinanceCoinbase),
            "asset_auto_plus_one_or_clean_pair" | "asset_auto" => {
                Some(Self::AssetAutoPlusOneOrCleanPair)
            }
            "bybit_plus_one" => Some(Self::BybitPlusOne),
            "bybit_plus_one_or_clean_pair" => Some(Self::BybitPlusOneOrCleanPair),
            "okx_plus_one" => Some(Self::OkxPlusOne),
            "okx_plus_one_or_clean_pair" => Some(Self::OkxPlusOneOrCleanPair),
            "open_gap_okx_plus_one" => Some(Self::OkxPlusOne),
            "open_gap_okx_plus_one_or_clean_pair" => Some(Self::OkxPlusOneOrCleanPair),
            "gate_plus_one" => Some(Self::GatePlusOne),
            "gate_plus_one_or_clean_pair" => Some(Self::GatePlusOneOrCleanPair),
            _ => None,
        }
    }

    pub(super) fn anchor_venue(self) -> CexVenue {
        match self {
            Self::BinanceCoinbase => CexVenue::Coinbase,
            Self::AssetAutoPlusOneOrCleanPair => CexVenue::Okx,
            Self::BybitPlusOne | Self::BybitPlusOneOrCleanPair => CexVenue::Bybit,
            Self::OkxPlusOne | Self::OkxPlusOneOrCleanPair => CexVenue::Okx,
            Self::GatePlusOne | Self::GatePlusOneOrCleanPair => CexVenue::Gateio,
        }
    }

    pub(super) fn allows_clean_pair(self) -> bool {
        matches!(
            self,
            Self::BinanceCoinbase
                | Self::AssetAutoPlusOneOrCleanPair
                | Self::BybitPlusOneOrCleanPair
                | Self::OkxPlusOneOrCleanPair
                | Self::GatePlusOneOrCleanPair
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CexEntryConsensusBasis {
    OwnOpenGap,
    CurrentPrice,
}

impl CexEntryConsensusBasis {
    pub(crate) fn parse(raw: Option<&str>) -> Self {
        match raw
            .map(str::trim)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "current_price" => Self::CurrentPrice,
            _ => Self::OwnOpenGap,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::OwnOpenGap => "own_open_gap",
            Self::CurrentPrice => "current_price",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct CexEntryConsensusModeConfig {
    pub(crate) mode: CexEntryConsensusMode,
    pub(crate) raw: Option<String>,
    pub(crate) parse_warning: bool,
}

impl CexEntryConsensusModeConfig {
    pub(crate) fn parse(raw: Option<&str>) -> Self {
        let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
            return Self::default();
        };
        match CexEntryConsensusMode::parse_known(raw) {
            Some(mode) => Self {
                mode,
                raw: Some(raw.to_string()),
                parse_warning: false,
            },
            None => Self {
                mode: CexEntryConsensusMode::default(),
                raw: Some(raw.to_string()),
                parse_warning: true,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CexEntryOpenGapConfig {
    pub(crate) threshold_usd: f64,
    pub(crate) threshold_usd_explicit: bool,
    pub(crate) min_venues: usize,
    pub(crate) allow_clean_pair_without_anchor: bool,
    pub(crate) ratio_min: f64,
    pub(crate) spread_floor_usd: f64,
    pub(crate) spread_expected_move_mult: f64,
    pub(crate) chainlink_sanity_check: bool,
    pub(crate) max_stale_ms: i64,
}

impl Default for CexEntryOpenGapConfig {
    fn default() -> Self {
        Self {
            threshold_usd: 0.30,
            threshold_usd_explicit: false,
            min_venues: 2,
            allow_clean_pair_without_anchor: true,
            ratio_min: 0.25,
            spread_floor_usd: 0.20,
            spread_expected_move_mult: 0.75,
            chainlink_sanity_check: true,
            max_stale_ms: 2_500,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CexEntryConsensusConfig {
    pub(crate) basis: CexEntryConsensusBasis,
    pub(crate) mode: CexEntryConsensusModeConfig,
    pub(crate) open_gap: CexEntryOpenGapConfig,
}

impl Default for CexEntryConsensusConfig {
    fn default() -> Self {
        Self {
            basis: CexEntryConsensusBasis::OwnOpenGap,
            mode: CexEntryConsensusModeConfig::parse(Some("binance_coinbase")),
            open_gap: CexEntryOpenGapConfig::default(),
        }
    }
}

impl CexEntryConsensusConfig {
    #[cfg(test)]
    pub(crate) fn current_price(mode: CexEntryConsensusModeConfig) -> Self {
        Self {
            basis: CexEntryConsensusBasis::CurrentPrice,
            mode,
            open_gap: CexEntryOpenGapConfig::default(),
        }
    }

    pub(crate) fn from_node(node: &crate::TradeFlowNode) -> Self {
        let mut open_gap = CexEntryOpenGapConfig::default();
        if let Some(threshold_usd) =
            positive_f64_option(crate::node_config_f64(node, "cexEntryOpenGapThresholdUsd"))
        {
            open_gap.threshold_usd = threshold_usd;
            open_gap.threshold_usd_explicit = true;
        }
        open_gap.min_venues = crate::node_config_f64(node, "cexEntryOpenGapMinVenues")
            .filter(|value| value.is_finite() && *value >= 2.0 && *value <= 3.0)
            .map(|value| value.round() as usize)
            .unwrap_or(open_gap.min_venues);
        open_gap.allow_clean_pair_without_anchor =
            crate::node_config_bool(node, "cexEntryOpenGapAllowCleanPairWithoutAnchor")
                .unwrap_or(open_gap.allow_clean_pair_without_anchor);
        open_gap.ratio_min = positive_f64(
            crate::node_config_f64(node, "cexEntryOpenGapRatioMin"),
            open_gap.ratio_min,
        );
        open_gap.spread_floor_usd = non_negative_f64(
            crate::node_config_f64(node, "cexEntryOpenGapSpreadFloorUsd"),
            open_gap.spread_floor_usd,
        );
        open_gap.spread_expected_move_mult = positive_f64(
            crate::node_config_f64(node, "cexEntryOpenGapSpreadExpectedMoveMult"),
            open_gap.spread_expected_move_mult,
        );
        open_gap.chainlink_sanity_check =
            crate::node_config_bool(node, "cexEntryChainlinkSanityCheck")
                .unwrap_or(open_gap.chainlink_sanity_check);

        Self {
            basis: CexEntryConsensusBasis::parse(
                crate::node_config_string(node, "cexEntryConsensusBasis").as_deref(),
            ),
            mode: CexEntryConsensusModeConfig::parse(
                crate::node_config_string(node, "cexEntryConsensusMode").as_deref(),
            ),
            open_gap,
        }
    }
}

fn positive_f64(value: Option<f64>, fallback: f64) -> f64 {
    positive_f64_option(value).unwrap_or(fallback)
}

fn positive_f64_option(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value > 0.0)
}

fn non_negative_f64(value: Option<f64>, fallback: f64) -> f64 {
    value
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(fallback)
}

#[derive(Debug, Clone)]
pub(super) struct EntryCurrentSourceDecision {
    pub(super) passed: bool,
    pub(super) reason_code: String,
    pub(super) reason_detail: Option<String>,
    pub(super) normalized_outcome_label: Option<String>,
    pub(super) direction: Option<String>,
    pub(super) current_price: Option<f64>,
    pub(super) current_price_source: &'static str,
    pub(super) directional_gap: Option<f64>,
    pub(super) gap_abs: Option<f64>,
    pub(super) signal_formula: Option<Value>,
    pub(super) iv_mismatch_edge: Option<Value>,
    pub(super) debug: Value,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_chainlink_cex_consensus_guard_evaluation(
    market_slug: &str,
    outcome_label: &str,
    snapshot: PolymarketPriceToBeatSnapshot,
    resolved_threshold_value: f64,
    resolved_threshold_unit: PriceToBeatDiffUnit,
    threshold_usd: f64,
    mode: PriceToBeatMode,
    cex_entry_consensus_config: CexEntryConsensusConfig,
    signal_config: Option<PriceToBeatSignalFormulaConfig>,
    iv_mismatch_config: Option<PriceToBeatIvMismatchEdgeConfig>,
    auto_threshold_usd: Option<f64>,
    lookback_windows_used: Option<usize>,
    current_windows_used: Option<usize>,
    avg_up_excursion_usd: Option<f64>,
    avg_down_excursion_usd: Option<f64>,
    lookback_market_slugs: Option<Vec<String>>,
    lookback_window_snapshots: Option<Vec<Value>>,
    baseline_pct: Option<f64>,
    current_pct: Option<f64>,
    vol_factor: Option<f64>,
    threshold_pct: Option<f64>,
    base_pct: Option<f64>,
    floor_usd: Option<f64>,
    ceiling_usd: Option<f64>,
    threshold_was_clamped: Option<bool>,
) -> PriceToBeatGuardEvaluation {
    let decision = evaluate_chainlink_cex_consensus_entry_current_source(
        market_slug,
        outcome_label,
        &snapshot,
        resolved_threshold_value,
        resolved_threshold_unit,
        threshold_usd,
        mode,
        cex_entry_consensus_config,
        signal_config,
        iv_mismatch_config,
    );
    let snapshot_status = snapshot.status().to_string();
    let snapshot_source = snapshot.source.as_str().to_string();
    PriceToBeatGuardEvaluation {
        passed: decision.passed,
        reason_code: decision.reason_code,
        reason_detail: decision.reason_detail,
        normalized_outcome_label: decision.normalized_outcome_label,
        direction: decision.direction,
        market_slug: market_slug.to_string(),
        event_url: snapshot.event_url,
        timeframe: Some(snapshot.timeframe),
        asset: Some(snapshot.asset),
        price_to_beat: Some(snapshot.price_to_beat),
        price_to_beat_status: Some(snapshot_status),
        price_to_beat_source: Some(snapshot_source),
        price_to_beat_source_latency_ms: snapshot.source_latency_ms,
        current_price: decision.current_price,
        current_price_source: decision.current_price_source,
        directional_gap: decision.directional_gap,
        gap_abs: decision.gap_abs,
        threshold_mode: mode.as_str().to_string(),
        configured_threshold_mode: None,
        base_threshold_value: None,
        base_threshold_unit: None,
        base_threshold_usd: None,
        current_effective_ptb_usd: None,
        threshold_value: resolved_threshold_value,
        threshold_unit: resolved_threshold_unit.as_str().to_string(),
        threshold_usd,
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
        auto_threshold_usd,
        lookback_windows_used,
        current_windows_used,
        avg_up_excursion_usd,
        avg_down_excursion_usd,
        lookback_market_slugs,
        lookback_window_snapshots,
        baseline_pct,
        current_pct,
        vol_factor,
        threshold_pct,
        base_pct,
        floor_usd,
        ceiling_usd,
        threshold_was_clamped,
        signal_formula: decision.signal_formula,
        iv_mismatch_edge: decision.iv_mismatch_edge,
        early_stale_side: None,
        cex_direction_guard: None,
        entry_current_source_debug: Some(decision.debug),
    }
}

#[derive(Debug, Clone)]
pub(super) struct EntryCurrentCandidate {
    pub(super) name: &'static str,
    pub(super) current_price_source: &'static str,
    pub(super) current_price: Option<f64>,
    pub(super) source_reason_code: String,
    pub(super) source_reason_detail: Option<String>,
    pub(super) cex_context: Option<ChainlinkStaleStrongGapRuntimeContext>,
    pub(super) debug: Value,
}

#[derive(Debug, Clone)]
struct EvaluatedEntryCurrentCandidate {
    name: &'static str,
    current_price_source: &'static str,
    passed: bool,
    reason_code: String,
    reason_detail: Option<String>,
    normalized_outcome_label: Option<String>,
    direction: Option<String>,
    current_price: Option<f64>,
    directional_gap: Option<f64>,
    gap_abs: Option<f64>,
    signal_formula: Option<Value>,
    iv_mismatch_edge: Option<Value>,
    debug: Value,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn evaluate_chainlink_cex_consensus_entry_current_source(
    market_slug: &str,
    outcome_label: &str,
    snapshot: &PolymarketPriceToBeatSnapshot,
    resolved_threshold_value: f64,
    resolved_threshold_unit: PriceToBeatDiffUnit,
    threshold_usd: f64,
    mode: PriceToBeatMode,
    cex_entry_consensus_config: CexEntryConsensusConfig,
    signal_config: Option<PriceToBeatSignalFormulaConfig>,
    iv_mismatch_config: Option<PriceToBeatIvMismatchEdgeConfig>,
) -> EntryCurrentSourceDecision {
    if cex_entry_consensus_config.basis == CexEntryConsensusBasis::OwnOpenGap {
        return evaluate_own_open_gap_entry_current_source(
            market_slug,
            outcome_label,
            snapshot,
            resolved_threshold_value,
            resolved_threshold_unit,
            threshold_usd,
            mode,
            cex_entry_consensus_config,
            signal_config,
            iv_mismatch_config,
        );
    }

    let cex_candidate = cex_consensus_candidate(
        market_slug,
        outcome_label,
        snapshot,
        threshold_usd,
        &cex_entry_consensus_config.mode,
    );
    let cex_context = cex_candidate.cex_context.clone();
    let candidates = vec![
        chainlink_candidate(market_slug, snapshot, cex_context),
        cex_candidate,
    ];
    let evaluated = candidates
        .into_iter()
        .map(|candidate| {
            evaluate_candidate(
                candidate,
                market_slug,
                outcome_label,
                snapshot,
                resolved_threshold_value,
                resolved_threshold_unit,
                threshold_usd,
                mode,
                signal_config,
                iv_mismatch_config.clone(),
            )
        })
        .collect::<Vec<_>>();

    let chainlink = evaluated
        .iter()
        .find(|candidate| candidate.name == "chainlink");
    let cex_consensus = evaluated
        .iter()
        .find(|candidate| candidate.name == "cex_consensus_bybit_plus_one");
    let selected = chainlink
        .filter(|candidate| candidate.passed)
        .or_else(|| cex_consensus.filter(|candidate| candidate.passed))
        .or_else(|| cex_consensus.filter(|candidate| candidate.current_price.is_some()))
        .or_else(|| chainlink.filter(|candidate| candidate.current_price.is_some()))
        .or(chainlink)
        .or_else(|| evaluated.first())
        .expect("entry hybrid candidates");

    EntryCurrentSourceDecision {
        passed: selected.passed,
        reason_code: selected.reason_code.clone(),
        reason_detail: selected.reason_detail.clone(),
        normalized_outcome_label: selected.normalized_outcome_label.clone(),
        direction: selected.direction.clone(),
        current_price: selected.current_price,
        current_price_source: selected.current_price_source,
        directional_gap: selected.directional_gap,
        gap_abs: selected.gap_abs,
        signal_formula: selected.signal_formula.clone(),
        iv_mismatch_edge: selected.iv_mismatch_edge.clone(),
        debug: json!({
            "entry_current_source_evaluations": evaluated
                .iter()
                .map(|candidate| candidate.debug.clone())
                .collect::<Vec<_>>(),
            "selected_entry_current_source": selected.current_price_source,
            "hybrid_mode": "or",
            "cex_entry_consensus_result": {
                "basis": cex_entry_consensus_config.basis.as_str(),
                "input_mode": cex_entry_consensus_config
                    .mode
                    .raw
                    .as_deref()
                    .unwrap_or_else(|| cex_entry_consensus_config.mode.mode.as_str()),
                "resolved_mode": cex_entry_consensus_config.mode.mode.as_str(),
                "mode": cex_entry_consensus_config.mode.mode.as_str(),
            },
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_own_open_gap_entry_current_source(
    market_slug: &str,
    outcome_label: &str,
    snapshot: &PolymarketPriceToBeatSnapshot,
    resolved_threshold_value: f64,
    resolved_threshold_unit: PriceToBeatDiffUnit,
    threshold_usd: f64,
    mode: PriceToBeatMode,
    cex_entry_consensus_config: CexEntryConsensusConfig,
    signal_config: Option<PriceToBeatSignalFormulaConfig>,
    iv_mismatch_config: Option<PriceToBeatIvMismatchEdgeConfig>,
) -> EntryCurrentSourceDecision {
    let chainlink = chainlink_candidate(market_slug, snapshot, None);
    let expected_move_eff = own_open_gap_expected_move_eff(
        market_slug,
        outcome_label,
        snapshot,
        mode,
        chainlink.current_price,
        signal_config.clone(),
        iv_mismatch_config.clone(),
    );
    let cex_candidate = super::own_open_gap_consensus::own_open_gap_candidate(
        market_slug,
        outcome_label,
        snapshot,
        &cex_entry_consensus_config,
        &chainlink,
        expected_move_eff,
    );
    let evaluated = vec![evaluate_candidate(
        cex_candidate,
        market_slug,
        outcome_label,
        snapshot,
        resolved_threshold_value,
        resolved_threshold_unit,
        threshold_usd,
        mode,
        signal_config,
        iv_mismatch_config,
    )];
    let selected = evaluated.first().expect("own open gap candidate");

    EntryCurrentSourceDecision {
        passed: selected.passed,
        reason_code: selected.reason_code.clone(),
        reason_detail: selected.reason_detail.clone(),
        normalized_outcome_label: selected.normalized_outcome_label.clone(),
        direction: selected.direction.clone(),
        current_price: selected.current_price,
        current_price_source: selected.current_price_source,
        directional_gap: selected.directional_gap,
        gap_abs: selected.gap_abs,
        signal_formula: selected.signal_formula.clone(),
        iv_mismatch_edge: selected.iv_mismatch_edge.clone(),
        debug: json!({
            "entry_current_source_evaluations": evaluated
                .iter()
                .map(|candidate| candidate.debug.clone())
                .collect::<Vec<_>>(),
            "selected_entry_current_source": selected.current_price_source,
            "hybrid_mode": "own_open_gap_only",
            "cex_entry_consensus_result": selected.debug.get("cex_entry_consensus_result").cloned(),
        }),
    }
}

fn own_open_gap_expected_move_eff(
    market_slug: &str,
    outcome_label: &str,
    snapshot: &PolymarketPriceToBeatSnapshot,
    mode: PriceToBeatMode,
    chainlink_current_price: Option<f64>,
    signal_config: Option<PriceToBeatSignalFormulaConfig>,
    iv_mismatch_config: Option<PriceToBeatIvMismatchEdgeConfig>,
) -> Option<f64> {
    if mode != PriceToBeatMode::IvMismatchEdge {
        return None;
    }
    let current_price =
        chainlink_current_price.filter(|value| value.is_finite() && *value > 0.0)?;
    let market_input =
        signal_config
            .map(|config| config.market)
            .unwrap_or(PriceToBeatSignalFormulaMarketInput {
                best_bid: None,
                best_ask: None,
            });
    let config = iv_mismatch_config
        .unwrap_or_else(|| PriceToBeatIvMismatchEdgeConfig::crypto_defaults(market_input));
    evaluate_price_to_beat_iv_mismatch_edge(
        market_slug,
        outcome_label,
        &snapshot.asset,
        current_price,
        snapshot.price_to_beat,
        config,
    )
    .expected_move_eff
    .filter(|value| value.is_finite() && *value > 0.0)
}

fn chainlink_candidate(
    market_slug: &str,
    snapshot: &PolymarketPriceToBeatSnapshot,
    cex_context: Option<ChainlinkStaleStrongGapRuntimeContext>,
) -> EntryCurrentCandidate {
    match resolve_price_to_beat_current_price_snapshot(
        PriceToBeatCurrentPriceSource::Chainlink,
        snapshot.source,
        market_slug,
        &snapshot.asset,
        snapshot.source_latency_ms,
    ) {
        Ok((current_price, current_price_source)) => EntryCurrentCandidate {
            name: "chainlink",
            current_price_source,
            current_price: Some(current_price),
            source_reason_code: "source_available".to_string(),
            source_reason_detail: None,
            cex_context,
            debug: json!({
                "source": "chainlink",
                "current_price_source": current_price_source,
                "current_price": current_price,
                "source_available": true,
            }),
        },
        Err((reason_code, reason_detail)) => EntryCurrentCandidate {
            name: "chainlink",
            current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
            current_price: None,
            source_reason_code: reason_code.to_string(),
            source_reason_detail: Some(reason_detail.clone()),
            cex_context,
            debug: json!({
                "source": "chainlink",
                "current_price_source": CURRENT_PRICE_SOURCE_CHAINLINK,
                "source_available": false,
                "reason_code": reason_code,
                "reason_detail": reason_detail,
            }),
        },
    }
}

fn cex_consensus_candidate(
    market_slug: &str,
    outcome_label: &str,
    snapshot: &PolymarketPriceToBeatSnapshot,
    threshold_usd: f64,
    consensus_mode: &CexEntryConsensusModeConfig,
) -> EntryCurrentCandidate {
    let anchor_venue = current_price_anchor_venue(consensus_mode.mode, &snapshot.asset);
    let anchor_name = anchor_venue.as_str();
    let anchor = cex_venue_leg(
        anchor_venue,
        &snapshot.asset,
        snapshot.price_to_beat,
        threshold_usd,
        outcome_label,
    );
    let binance = if anchor_venue == CexVenue::Hyperliquid {
        skipped_cex_venue_leg(CexVenue::Binance, "not_required_for_asset_auto")
    } else {
        cex_venue_leg(
            CexVenue::Binance,
            &snapshot.asset,
            snapshot.price_to_beat,
            threshold_usd,
            outcome_label,
        )
    };
    let coinbase = cex_venue_leg(
        CexVenue::Coinbase,
        &snapshot.asset,
        snapshot.price_to_beat,
        threshold_usd,
        outcome_label,
    );
    let confirming_venues = [(&binance, "binance"), (&coinbase, "coinbase")]
        .into_iter()
        .filter(|(_, venue)| *venue != anchor_name)
        .filter(|(_, venue)| anchor_venue != CexVenue::Hyperliquid || *venue == "coinbase")
        .filter_map(|(leg, venue)| leg.threshold_hit.then_some(venue))
        .collect::<Vec<_>>();
    let anchor_plus_one = anchor.threshold_hit && !confirming_venues.is_empty();
    let clean_pair_enabled =
        consensus_mode.mode.allows_clean_pair() && anchor_venue != CexVenue::Hyperliquid;
    let clean_pair_candidate =
        clean_pair_enabled && binance.threshold_hit && coinbase.threshold_hit;
    let clean_pair_pass_selection = clean_pair_selection(outcome_label, &binance, &coinbase)
        .filter(|selection| clean_pair_candidate && selection.within_limits);
    let clean_pair_dislocated = clean_pair_candidate
        && clean_pair_selection(outcome_label, &binance, &coinbase)
            .map(|selection| !selection.within_limits)
            .unwrap_or(false);
    let cex_passed = anchor_plus_one || clean_pair_pass_selection.is_some();
    let confirmed_via = if anchor_plus_one {
        Some(anchor_plus_one_label(consensus_mode.mode, anchor_venue))
    } else if clean_pair_pass_selection.is_some() {
        Some("clean_pair")
    } else {
        None
    };
    let (current_price, current_price_venue) = if anchor_plus_one {
        (
            anchor.current_price,
            anchor.current_price.map(|_| anchor_name),
        )
    } else if let Some(selection) = clean_pair_pass_selection.as_ref() {
        (
            Some(selection.selected_current),
            Some("clean_pair_conservative"),
        )
    } else {
        (
            anchor.current_price,
            anchor.current_price.map(|_| anchor_name),
        )
    };
    let cex_context = ChainlinkStaleStrongGapRuntimeContext {
        cex_confirmed: cex_passed,
        anchor_venue: Some(anchor_name.to_string()),
        anchor_hit: anchor.threshold_hit,
        bybit_hit: anchor_venue == CexVenue::Bybit && anchor.threshold_hit,
        secondary_confirmed: !confirming_venues.is_empty(),
        secondary_sources: confirming_venues
            .iter()
            .map(|venue| (*venue).to_string())
            .collect(),
        cex_clean: Some(cex_passed),
        cex_direction: Some(if cex_passed { "clean" } else { "unknown" }.to_string()),
    };
    let reason_code = if cex_passed {
        "source_available"
    } else if clean_pair_dislocated {
        "cex_consensus_clean_pair_dislocated"
    } else if anchor.current_price.is_none() {
        unavailable_reason(consensus_mode.mode, anchor_venue)
    } else if anchor.directional_gap.is_none() {
        "unsupported_outcome_label"
    } else if !anchor.threshold_hit {
        below_threshold_reason(consensus_mode.mode, anchor_venue)
    } else {
        "cex_consensus_unconfirmed"
    };
    let reason_detail = (!cex_passed).then(|| {
        format!(
            "market_slug={market_slug}; cex_entry_consensus mode={} requires {} threshold hit plus {}{}",
            consensus_mode.mode.as_str(),
            anchor_display(consensus_mode.mode, anchor_venue),
            secondary_confirmation_display(anchor_venue),
            if clean_pair_enabled {
                ", or clean binance+coinbase pair confirmation"
            } else {
                ""
            }
        )
    });
    let clean_pair_debug = clean_pair_selection(outcome_label, &binance, &coinbase);

    EntryCurrentCandidate {
        name: "cex_consensus_bybit_plus_one",
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK_CEX_CONSENSUS,
        current_price,
        source_reason_code: reason_code.to_string(),
        source_reason_detail: reason_detail,
        cex_context: Some(cex_context),
        debug: json!({
            "source": "cex_consensus_bybit_plus_one",
            "current_price_source": CURRENT_PRICE_SOURCE_CHAINLINK_CEX_CONSENSUS,
            "runtime_source": CURRENT_PRICE_SOURCE_CEX_CONSENSUS,
            "source_available": current_price.is_some(),
            "confirmed": cex_passed,
            "confirmed_via": confirmed_via,
            "anchor_venue": anchor_name,
            "anchor_hit": anchor.threshold_hit,
            "confirming_venues": confirming_venues,
            "cex_entry_consensus_mode": consensus_mode.mode.as_str(),
            "cex_entry_consensus_mode_parse_warning": consensus_mode.parse_warning,
            "cex_entry_consensus_mode_raw": consensus_mode.raw,
            "current_price_venue": current_price_venue,
            "current_price_sources": clean_pair_pass_selection
                .as_ref()
                .map(|_| vec!["binance", "coinbase"])
                .unwrap_or_else(Vec::new),
            "selected_current": current_price,
            "binance_current": binance.current_price,
            "coinbase_current": coinbase.current_price,
            "clean_pair_candidate": clean_pair_candidate,
            "clean_pair_fresh": binance.current_price.is_some() && coinbase.current_price.is_some(),
            "clean_pair_diff_usd": clean_pair_debug.as_ref().map(|selection| selection.diff_usd),
            "clean_pair_diff_bps": clean_pair_debug.as_ref().map(|selection| selection.diff_bps),
            "clean_pair_max_diff_usd": CLEAN_PAIR_MAX_DIFF_USD,
            "clean_pair_max_diff_bps": CLEAN_PAIR_MAX_DIFF_BPS,
            "reason_code": reason_code,
            "anchor": anchor.debug,
            "bybit": if anchor_venue == CexVenue::Bybit { anchor.debug.clone() } else { Value::Null },
            "okx": if anchor_venue == CexVenue::Okx { anchor.debug.clone() } else { Value::Null },
            "gateio": if anchor_venue == CexVenue::Gateio { anchor.debug.clone() } else { Value::Null },
            "hyperliquid": if anchor_venue == CexVenue::Hyperliquid { anchor.debug.clone() } else { Value::Null },
            "binance": binance.debug,
            "coinbase": coinbase.debug,
        }),
    }
}

fn current_price_anchor_venue(mode: CexEntryConsensusMode, asset: &str) -> CexVenue {
    if mode != CexEntryConsensusMode::AssetAutoPlusOneOrCleanPair {
        return mode.anchor_venue();
    }
    match asset.trim().to_ascii_lowercase().as_str() {
        "sol" => CexVenue::Gateio,
        "hype" | "hyperliquid" => CexVenue::Hyperliquid,
        _ => CexVenue::Okx,
    }
}

fn anchor_plus_one_label(mode: CexEntryConsensusMode, anchor_venue: CexVenue) -> &'static str {
    if mode == CexEntryConsensusMode::BinanceCoinbase {
        return "binance_coinbase";
    }
    match anchor_venue {
        CexVenue::Okx => "okx_plus_one",
        CexVenue::Gateio => "gate_plus_one",
        CexVenue::Hyperliquid => "hyperliquid_plus_one",
        CexVenue::Coinbase => "coinbase_plus_one",
        CexVenue::Binance => "binance_plus_one",
        CexVenue::Bybit => "bybit_plus_one",
    }
}

fn unavailable_reason(mode: CexEntryConsensusMode, anchor_venue: CexVenue) -> &'static str {
    if mode == CexEntryConsensusMode::BinanceCoinbase {
        return "cex_consensus_binance_coinbase_unavailable";
    }
    match anchor_venue {
        CexVenue::Okx => "cex_consensus_okx_unavailable",
        CexVenue::Gateio => "cex_consensus_gateio_unavailable",
        CexVenue::Hyperliquid => "cex_consensus_hyperliquid_unavailable",
        CexVenue::Coinbase => "cex_consensus_coinbase_unavailable",
        CexVenue::Binance => "cex_consensus_binance_unavailable",
        CexVenue::Bybit => "cex_consensus_bybit_unavailable",
    }
}

fn below_threshold_reason(mode: CexEntryConsensusMode, anchor_venue: CexVenue) -> &'static str {
    if mode == CexEntryConsensusMode::BinanceCoinbase {
        return "cex_consensus_binance_coinbase_below_threshold";
    }
    match anchor_venue {
        CexVenue::Okx => "cex_consensus_okx_below_threshold",
        CexVenue::Gateio => "cex_consensus_gateio_below_threshold",
        CexVenue::Hyperliquid => "cex_consensus_hyperliquid_below_threshold",
        CexVenue::Coinbase => "cex_consensus_coinbase_below_threshold",
        CexVenue::Binance => "cex_consensus_binance_below_threshold",
        CexVenue::Bybit => "cex_consensus_bybit_below_threshold",
    }
}

fn anchor_display(mode: CexEntryConsensusMode, anchor_venue: CexVenue) -> &'static str {
    if mode == CexEntryConsensusMode::BinanceCoinbase {
        return "binance+coinbase";
    }
    anchor_venue.as_str()
}

fn secondary_confirmation_display(anchor_venue: CexVenue) -> &'static str {
    if anchor_venue == CexVenue::Hyperliquid {
        "coinbase confirmation"
    } else {
        "binance or coinbase confirmation"
    }
}

#[derive(Debug, Clone, Copy)]
struct CleanPairSelection {
    selected_current: f64,
    diff_usd: f64,
    diff_bps: f64,
    within_limits: bool,
}

fn clean_pair_selection(
    outcome_label: &str,
    binance: &CexEntryLeg,
    coinbase: &CexEntryLeg,
) -> Option<CleanPairSelection> {
    let binance_price = binance.current_price?;
    let coinbase_price = coinbase.current_price?;
    let (_, direction) = normalize_outcome_direction(outcome_label)?;
    let selected_current = if direction == "up" {
        binance_price.min(coinbase_price)
    } else {
        binance_price.max(coinbase_price)
    };
    let diff_usd = (binance_price - coinbase_price).abs();
    let diff_bps = if selected_current.abs() > f64::EPSILON {
        diff_usd / selected_current.abs() * 10_000.0
    } else {
        f64::INFINITY
    };
    Some(CleanPairSelection {
        selected_current,
        diff_usd,
        diff_bps,
        within_limits: diff_usd < CLEAN_PAIR_MAX_DIFF_USD && diff_bps < CLEAN_PAIR_MAX_DIFF_BPS,
    })
}

#[derive(Debug, Clone)]
struct CexEntryLeg {
    current_price: Option<f64>,
    directional_gap: Option<f64>,
    threshold_hit: bool,
    debug: Value,
}

fn skipped_cex_venue_leg(venue: CexVenue, reason: &'static str) -> CexEntryLeg {
    CexEntryLeg {
        current_price: None,
        directional_gap: None,
        threshold_hit: false,
        debug: json!({
            "venue": venue.as_str(),
            "current_price": Value::Null,
            "directional_gap": Value::Null,
            "threshold_hit": false,
            "skipped": true,
            "skip_reason": reason,
        }),
    }
}

fn cex_venue_leg(
    venue: CexVenue,
    asset: &str,
    price_to_beat: f64,
    threshold_usd: f64,
    outcome_label: &str,
) -> CexEntryLeg {
    let config = CexMicrostructureSnapshotConfig::default();
    match get_cex_current_price_snapshot_with_bybit_retry(asset, venue, &config) {
        Ok((snapshot, retry_debug)) => {
            let directional =
                evaluate_directional_gap(snapshot.mid, price_to_beat, threshold_usd, outcome_label);
            let directional_gap = directional.map(|value| value.directional_gap);
            let threshold_hit = directional.map(|value| value.passed).unwrap_or(false);
            CexEntryLeg {
                current_price: Some(snapshot.mid),
                directional_gap,
                threshold_hit,
                debug: json!({
                    "venue": venue.as_str(),
                    "current_price": snapshot.mid,
                    "bid": snapshot.bid,
                    "ask": snapshot.ask,
                    "book_staleness_ms": snapshot.book_staleness_ms,
                    "ticker_staleness_ms": snapshot.ticker_staleness_ms,
                    "directional_gap": directional_gap,
                    "threshold_hit": threshold_hit,
                    "bybit_micro_stale_retry": retry_debug,
                }),
            }
        }
        Err((err, retry_debug)) => CexEntryLeg {
            current_price: None,
            directional_gap: None,
            threshold_hit: false,
            debug: json!({
                "venue": venue.as_str(),
                "current_price": Value::Null,
                "directional_gap": Value::Null,
                "threshold_hit": false,
                "error": err,
                "bybit_micro_stale_retry": retry_debug,
            }),
        },
    }
}

fn get_cex_current_price_snapshot_with_bybit_retry(
    asset: &str,
    venue: CexVenue,
    config: &CexMicrostructureSnapshotConfig,
) -> std::result::Result<(CexCurrentPriceSnapshot, Option<Value>), (String, Option<Value>)> {
    let initial_error = match get_cex_current_price_snapshot(asset, venue, config) {
        Ok(snapshot) => return Ok((snapshot, None)),
        Err(error) => error.to_string(),
    };
    let Some(initial_stale) = bybit_micro_stale_retry_details(venue, &initial_error) else {
        return Err((initial_error, None));
    };

    tracing::warn!(
        asset,
        initial_age_ms = initial_stale.age_ms,
        max_ms = initial_stale.max_ms,
        retry_timeout_ms = BYBIT_MICRO_STALE_RETRY_TIMEOUT_MS,
        "CEX_ENTRY_BYBIT_MICRO_STALE_RETRY_REQUESTED"
    );
    let started_at = Instant::now();
    let deadline = started_at + Duration::from_millis(BYBIT_MICRO_STALE_RETRY_TIMEOUT_MS);
    let mut final_error = initial_error;
    while Instant::now() < deadline {
        let now = Instant::now();
        let sleep_ms = BYBIT_MICRO_STALE_RETRY_STEP_MS
            .min(deadline.saturating_duration_since(now).as_millis() as u64);
        if sleep_ms == 0 {
            break;
        }
        thread::sleep(Duration::from_millis(sleep_ms));
        match get_cex_current_price_snapshot(asset, venue, config) {
            Ok(snapshot) => {
                let waited_ms = started_at.elapsed().as_millis() as u64;
                tracing::info!(
                    asset,
                    waited_ms,
                    final_age_ms = snapshot.book_staleness_ms,
                    "CEX_ENTRY_BYBIT_MICRO_STALE_RETRY_READY"
                );
                return Ok((
                    snapshot,
                    Some(json!({
                        "requested": true,
                        "initial_age_ms": initial_stale.age_ms,
                        "max_ms": initial_stale.max_ms,
                        "retry_waited_ms": waited_ms,
                        "result": "ready",
                    })),
                ));
            }
            Err(error) => {
                final_error = error.to_string();
                if bybit_micro_stale_retry_details(venue, &final_error).is_none() {
                    let waited_ms = started_at.elapsed().as_millis() as u64;
                    return Err((
                        final_error.clone(),
                        Some(json!({
                            "requested": true,
                            "initial_age_ms": initial_stale.age_ms,
                            "max_ms": initial_stale.max_ms,
                            "retry_waited_ms": waited_ms,
                            "result": "non_retry_error",
                            "final_error": final_error,
                        })),
                    ));
                }
            }
        }
    }

    let waited_ms = started_at.elapsed().as_millis() as u64;
    tracing::warn!(
        asset,
        waited_ms,
        final_error,
        "CEX_ENTRY_BYBIT_MICRO_STALE_RETRY_EXHAUSTED"
    );
    Err((
        final_error.clone(),
        Some(json!({
            "requested": true,
            "initial_age_ms": initial_stale.age_ms,
            "max_ms": initial_stale.max_ms,
            "retry_waited_ms": waited_ms,
            "result": "timeout",
            "final_error": final_error,
        })),
    ))
}

#[derive(Debug, Clone, Copy)]
struct BybitMicroStaleRetryDetails {
    age_ms: i64,
    max_ms: i64,
}

fn bybit_micro_stale_retry_details(
    venue: CexVenue,
    error: &str,
) -> Option<BybitMicroStaleRetryDetails> {
    if venue != CexVenue::Bybit
        || !(error.contains("bybit book stale") || error.contains("bybit ticker stale"))
    {
        return None;
    }
    let age_ms = parse_named_i64(error, "age_ms")?;
    let max_ms = parse_named_i64(error, "max_ms")?;
    (age_ms > max_ms && age_ms - max_ms <= BYBIT_MICRO_STALE_RETRY_MAX_OVER_MS)
        .then_some(BybitMicroStaleRetryDetails { age_ms, max_ms })
}

fn parse_named_i64(text: &str, key: &str) -> Option<i64> {
    text.split(key)
        .nth(1)?
        .strip_prefix('=')?
        .split(|ch: char| !ch.is_ascii_digit() && ch != '-')
        .next()?
        .parse()
        .ok()
}

#[allow(clippy::too_many_arguments)]
fn evaluate_candidate(
    candidate: EntryCurrentCandidate,
    market_slug: &str,
    outcome_label: &str,
    snapshot: &PolymarketPriceToBeatSnapshot,
    resolved_threshold_value: f64,
    resolved_threshold_unit: PriceToBeatDiffUnit,
    threshold_usd: f64,
    mode: PriceToBeatMode,
    signal_config: Option<PriceToBeatSignalFormulaConfig>,
    iv_mismatch_config: Option<PriceToBeatIvMismatchEdgeConfig>,
) -> EvaluatedEntryCurrentCandidate {
    let Some(current_price) = candidate.current_price else {
        let mut debug = candidate.debug;
        merge_debug(
            &mut debug,
            false,
            &candidate.source_reason_code,
            candidate.source_reason_detail.as_deref(),
            None,
            None,
            None,
            None,
        );
        return EvaluatedEntryCurrentCandidate {
            name: candidate.name,
            current_price_source: candidate.current_price_source,
            passed: false,
            reason_code: candidate.source_reason_code,
            reason_detail: candidate.source_reason_detail,
            normalized_outcome_label: None,
            direction: None,
            current_price: None,
            directional_gap: None,
            gap_abs: None,
            signal_formula: None,
            iv_mismatch_edge: None,
            debug,
        };
    };

    let gap_abs = (current_price - snapshot.price_to_beat).abs();
    let Some(directional) = evaluate_directional_gap(
        current_price,
        snapshot.price_to_beat,
        threshold_usd,
        outcome_label,
    ) else {
        let mut debug = candidate.debug;
        merge_debug(
            &mut debug,
            false,
            "unsupported_outcome_label",
            Some("outcome_label is not a recognized direction"),
            Some(current_price),
            None,
            Some(gap_abs),
            None,
        );
        return EvaluatedEntryCurrentCandidate {
            name: candidate.name,
            current_price_source: candidate.current_price_source,
            passed: false,
            reason_code: "unsupported_outcome_label".to_string(),
            reason_detail: Some(format!(
                "outcome_label '{outcome_label}' is not a recognized direction"
            )),
            normalized_outcome_label: None,
            direction: None,
            current_price: Some(current_price),
            directional_gap: None,
            gap_abs: Some(gap_abs),
            signal_formula: None,
            iv_mismatch_edge: None,
            debug,
        };
    };

    let signal_evaluation = if mode == PriceToBeatMode::SignalFormula {
        let config = signal_config.unwrap_or_else(|| {
            PriceToBeatSignalFormulaConfig::taker(PriceToBeatSignalFormulaMarketInput {
                best_bid: None,
                best_ask: None,
            })
        });
        Some(evaluate_price_to_beat_signal_formula(
            market_slug,
            outcome_label,
            &snapshot.asset,
            current_price,
            snapshot.price_to_beat,
            config,
        ))
    } else {
        None
    };
    let iv_mismatch_evaluation = if mode == PriceToBeatMode::IvMismatchEdge {
        let market_input = signal_config.map(|config| config.market).unwrap_or(
            PriceToBeatSignalFormulaMarketInput {
                best_bid: None,
                best_ask: None,
            },
        );
        let mut config = iv_mismatch_config
            .unwrap_or_else(|| PriceToBeatIvMismatchEdgeConfig::crypto_defaults(market_input));
        config.chainlink_stale_strong_gap_context = candidate.cex_context.clone();
        Some(evaluate_price_to_beat_iv_mismatch_edge(
            market_slug,
            outcome_label,
            &snapshot.asset,
            current_price,
            snapshot.price_to_beat,
            config,
        ))
    } else {
        None
    };
    let model_passed = signal_evaluation
        .as_ref()
        .map(|evaluation| evaluation.passed)
        .or_else(|| {
            iv_mismatch_evaluation
                .as_ref()
                .map(|evaluation| evaluation.passed)
        })
        .unwrap_or(directional.passed);
    let source_passed = candidate.source_reason_code == "source_available";
    let passed = source_passed && model_passed;
    let reason_code = if !source_passed {
        candidate.source_reason_code.clone()
    } else {
        signal_evaluation
            .as_ref()
            .map(|evaluation| {
                if evaluation.passed {
                    "passed".to_string()
                } else {
                    format!("signal_formula_{}", evaluation.reason)
                }
            })
            .or_else(|| {
                iv_mismatch_evaluation
                    .as_ref()
                    .map(|evaluation| evaluation.reason.to_string())
            })
            .unwrap_or_else(|| {
                if passed {
                    "passed".to_string()
                } else {
                    "price_to_beat_gap_below_threshold".to_string()
                }
            })
    };
    let reason_detail = if !source_passed {
        candidate.source_reason_detail.clone()
    } else {
        signal_evaluation
            .as_ref()
            .and_then(|evaluation| {
                (!evaluation.passed).then(|| {
                    format!(
                        "signal formula reason={} q_side={:?} cost={:?} edge={:?} edge_threshold={:.8}",
                        evaluation.reason,
                        evaluation.q_side,
                        evaluation.cost,
                        evaluation.edge,
                        evaluation.edge_threshold,
                    )
                })
            })
            .or_else(|| {
                iv_mismatch_evaluation.as_ref().and_then(|evaluation| {
                    (!evaluation.passed).then(|| {
                        format!(
                            "iv mismatch edge reason={} q={:?} cost={:?} edge={:?} threshold={:?} gap_strength={:?} required_gap_strength={:?}",
                            evaluation.reason,
                            evaluation.q,
                            evaluation.cost,
                            evaluation.edge,
                            evaluation.threshold,
                            evaluation.gap_strength,
                            evaluation.required_gap_strength,
                        )
                    })
                })
            })
            .or_else(|| {
                (!passed).then(|| {
                    format!(
                        "directional gap {:.8} (direction={}) is below threshold {:.8} {} (~{:.8} usd)",
                        directional.directional_gap,
                        directional.direction,
                        resolved_threshold_value,
                        resolved_threshold_unit.as_str(),
                        threshold_usd
                    )
                })
            })
    };
    let signal_value = signal_evaluation.map(|evaluation| evaluation.to_value());
    let iv_value = iv_mismatch_evaluation.map(|evaluation| evaluation.to_value());
    let mut debug = candidate.debug;
    merge_debug(
        &mut debug,
        passed,
        &reason_code,
        reason_detail.as_deref(),
        Some(current_price),
        Some(directional.directional_gap),
        Some(gap_abs),
        Some(directional.passed),
    );
    if let Some(value) = signal_value.as_ref() {
        insert_debug_field(&mut debug, "signal_formula", value.clone());
    }
    if let Some(value) = iv_value.as_ref() {
        insert_debug_field(&mut debug, "iv_mismatch_edge", value.clone());
    }

    EvaluatedEntryCurrentCandidate {
        name: candidate.name,
        current_price_source: candidate.current_price_source,
        passed,
        reason_code,
        reason_detail,
        normalized_outcome_label: Some(directional.normalized_outcome_label.to_string()),
        direction: Some(directional.direction.to_string()),
        current_price: Some(current_price),
        directional_gap: Some(directional.directional_gap),
        gap_abs: Some(gap_abs),
        signal_formula: signal_value,
        iv_mismatch_edge: iv_value,
        debug,
    }
}

fn merge_debug(
    debug: &mut Value,
    passed: bool,
    reason_code: &str,
    reason_detail: Option<&str>,
    current_price: Option<f64>,
    directional_gap: Option<f64>,
    gap_abs: Option<f64>,
    threshold_hit: Option<bool>,
) {
    if let Some(object) = debug.as_object_mut() {
        object.insert("passed".to_string(), json!(passed));
        object.insert("reason_code".to_string(), json!(reason_code));
        object.insert("reason_detail".to_string(), json!(reason_detail));
        object.insert("current_price".to_string(), json!(current_price));
        object.insert("directional_gap".to_string(), json!(directional_gap));
        object.insert("gap_abs".to_string(), json!(gap_abs));
        object.insert("threshold_hit".to_string(), json!(threshold_hit));
    }
}

fn insert_debug_field(debug: &mut Value, key: &str, value: Value) {
    if let Some(object) = debug.as_object_mut() {
        object.insert(key.to_string(), value);
    }
}
