use super::current_price::CURRENT_PRICE_SOURCE_CHAINLINK_CEX_CONSENSUS;
use super::entry_current_hybrid::{CexEntryConsensusConfig, EntryCurrentCandidate};
use super::iv_chainlink_stale_strong_gap_exception::ChainlinkStaleStrongGapRuntimeContext;
use super::*;
use crate::trade_flow::guards::cex_microstructure::{
    active_anchor_venue_for_asset, active_spot_venues_for_asset, get_cex_venue_delta_snapshot,
    CexVenue, CexVenueDeltaSnapshot,
};
use crate::trade_flow::guards::polymarket_price_to_beat::PolymarketPriceToBeatSnapshot;
use serde_json::{json, Value};

const SOURCE_NAME: &str = "cex_consensus_own_open_gap";
const FIVE_MINUTES_MS: i64 = 300_000;

#[derive(Debug, Clone)]
struct VenueOpenGapLeg {
    venue: CexVenue,
    own_5m_open: Option<f64>,
    current_mid: Option<f64>,
    open_timestamp_ms: Option<i64>,
    current_timestamp_ms: Option<i64>,
    gap: Option<f64>,
    opposite_gap: Option<f64>,
    pass: bool,
    opposite_pass: bool,
    stale: bool,
    error: Option<String>,
}

#[derive(Debug, Clone)]
struct PairQuality {
    venues: [CexVenue; 2],
    gap_spread: f64,
    gap_ratio: f64,
    gap_spread_limit: f64,
}

#[derive(Debug, Clone)]
struct PairFailReason {
    venues: [CexVenue; 2],
    reason: &'static str,
}

pub(super) fn own_open_gap_candidate(
    market_slug: &str,
    outcome_label: &str,
    snapshot: &PolymarketPriceToBeatSnapshot,
    config: &CexEntryConsensusConfig,
    chainlink: &EntryCurrentCandidate,
    expected_move_eff: Option<f64>,
) -> EntryCurrentCandidate {
    let threshold_usd = resolve_threshold_usd(
        &snapshot.asset,
        config.open_gap.threshold_usd,
        config.open_gap.threshold_usd_explicit,
    );
    let effective_expected_move = expected_move_eff
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(threshold_usd);
    let active_venues = entry_active_spot_venues_for_asset(&snapshot.asset);
    let anchor_venue = entry_active_anchor_venue_for_asset(&snapshot.asset);
    let Some((_, direction)) = normalize_outcome_direction(outcome_label) else {
        return blocked_candidate(
            config,
            "unsupported_outcome_label",
            Some(format!(
                "outcome_label '{outcome_label}' is not a recognized direction"
            )),
            None,
            Vec::new(),
            Vec::new(),
            None,
            effective_expected_move,
            None,
            anchor_venue,
            &active_venues,
        );
    };
    let Some(window_start_ms) = crate::MarketCycleId(market_slug.to_string()).start_time() else {
        return blocked_candidate(
            config,
            "open_gap_missing_open",
            Some(format!(
                "market_slug={market_slug}; cannot resolve market window start"
            )),
            None,
            Vec::new(),
            Vec::new(),
            None,
            effective_expected_move,
            None,
            anchor_venue,
            &active_venues,
        );
    };

    let legs = active_venues
        .iter()
        .copied()
        .map(|venue| {
            load_leg(
                venue,
                &snapshot.asset,
                window_start_ms.timestamp_millis(),
                direction,
                threshold_usd,
                config.open_gap.max_stale_ms,
            )
        })
        .collect::<Vec<_>>();
    let available_count = legs.iter().filter(|leg| leg.is_available()).count();
    let required_venues = config.open_gap.min_venues.min(active_venues.len());
    if let Some(opposite) = legs.iter().find(|leg| leg.opposite_pass) {
        return blocked_candidate(
            config,
            "open_gap_opposite_venue_detected",
            Some(format!(
                "venue={} opposite_gap={:.8} threshold_usd={:.8}",
                opposite.venue.as_str(),
                opposite.opposite_gap.unwrap_or_default(),
                threshold_usd
            )),
            None,
            legs,
            Vec::new(),
            None,
            effective_expected_move,
            None,
            anchor_venue,
            &active_venues,
        );
    }
    let gap_spread_limit = config
        .open_gap
        .spread_floor_usd
        .max(effective_expected_move * config.open_gap.spread_expected_move_mult);
    if available_count < required_venues {
        return blocked_candidate(
            config,
            "open_gap_insufficient_venues",
            Some(format!(
                "available_venues={available_count}; required={}",
                required_venues
            )),
            None,
            legs,
            Vec::new(),
            None,
            effective_expected_move,
            Some(gap_spread_limit),
            anchor_venue,
            &active_venues,
        );
    }

    let mut quality_fail_reasons = Vec::new();
    let anchor_pairs = anchor_candidate_pairs(anchor_venue);
    let selected_anchor = best_clean_pair(
        &anchor_pairs,
        &legs,
        config.open_gap.ratio_min,
        gap_spread_limit,
        &mut quality_fail_reasons,
    );
    let selected = selected_anchor.or_else(|| {
        config.open_gap.allow_clean_pair_without_anchor.then(|| {
            let fallback_pairs = clean_pair_fallback_pairs(anchor_venue);
            best_clean_pair(
                &fallback_pairs,
                &legs,
                config.open_gap.ratio_min,
                gap_spread_limit,
                &mut quality_fail_reasons,
            )
        })?
    });
    let Some(selected) = selected else {
        return blocked_candidate(
            config,
            "open_gap_no_clean_pair",
            Some("no venue pair passed own-open-gap quality checks".to_string()),
            None,
            legs,
            quality_fail_reasons,
            None,
            effective_expected_move,
            Some(gap_spread_limit),
            anchor_venue,
            &active_venues,
        );
    };

    if config.open_gap.chainlink_sanity_check {
        if let Some((reason, detail)) = chainlink_sanity_failure(
            chainlink,
            snapshot.price_to_beat,
            threshold_usd,
            selected.min_gap(&legs),
            direction,
            gap_spread_limit,
        ) {
            return blocked_candidate(
                config,
                "open_gap_chainlink_sanity_fail",
                Some(detail),
                Some(json!({ "reason": reason })),
                legs,
                quality_fail_reasons,
                Some(selected),
                effective_expected_move,
                Some(gap_spread_limit),
                anchor_venue,
                &active_venues,
            );
        }
    }

    let selected_current = selected_current(&selected.venues, &legs, direction);
    let global_gap_dislocation_warn = same_direction_dislocation_warn(
        &selected.venues,
        &legs,
        config.open_gap.ratio_min,
    );
    let reason_code = if global_gap_dislocation_warn {
        "open_gap_anchor_pair_pass_with_dislocation_warning"
    } else if selected.venues.contains(&anchor_venue) {
        "open_gap_anchor_pair_pass"
    } else {
        "open_gap_clean_pair_pass"
    };
    let cex_context = ChainlinkStaleStrongGapRuntimeContext {
        cex_confirmed: true,
        anchor_venue: Some(anchor_venue.as_str().to_string()),
        anchor_hit: leg_for(&legs, anchor_venue)
            .map(|leg| leg.pass)
            .unwrap_or(false),
        bybit_hit: false,
        secondary_confirmed: true,
        secondary_sources: selected
            .venues
            .iter()
            .map(|venue| venue.as_str().to_string())
            .collect(),
        cex_clean: Some(true),
        cex_direction: Some("clean".to_string()),
    };
    EntryCurrentCandidate {
        name: SOURCE_NAME,
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK_CEX_CONSENSUS,
        current_price: selected_current,
        source_reason_code: "source_available".to_string(),
        source_reason_detail: None,
        cex_context: Some(cex_context),
        debug: candidate_debug(
            config,
            reason_code,
            None,
            selected_current,
            legs,
            quality_fail_reasons,
            Some(selected),
            None,
            global_gap_dislocation_warn,
            effective_expected_move,
            Some(gap_spread_limit),
            anchor_venue,
            &active_venues,
        ),
    }
}

fn anchor_candidate_pairs(anchor_venue: CexVenue) -> Vec<[CexVenue; 2]> {
    [CexVenue::Binance, CexVenue::Coinbase]
        .into_iter()
        .filter(|venue| *venue != anchor_venue)
        .map(|venue| [anchor_venue, venue])
        .collect()
}

fn clean_pair_fallback_pairs(anchor_venue: CexVenue) -> Vec<[CexVenue; 2]> {
    let non_anchor = [CexVenue::Binance, CexVenue::Coinbase]
        .into_iter()
        .filter(|venue| *venue != anchor_venue)
        .collect::<Vec<_>>();
    if non_anchor.len() == 2 {
        vec![[non_anchor[0], non_anchor[1]]]
    } else {
        Vec::new()
    }
}

fn entry_active_anchor_venue_for_asset(asset: &str) -> CexVenue {
    match asset.trim().to_ascii_lowercase().as_str() {
        "btc" | "eth" | "sol" => CexVenue::Coinbase,
        _ => active_anchor_venue_for_asset(asset),
    }
}

fn entry_active_spot_venues_for_asset(asset: &str) -> Vec<CexVenue> {
    match asset.trim().to_ascii_lowercase().as_str() {
        "btc" | "eth" | "sol" => vec![CexVenue::Binance, CexVenue::Coinbase],
        _ => active_spot_venues_for_asset(asset).to_vec(),
    }
}

fn load_leg(
    venue: CexVenue,
    asset: &str,
    window_start_ms: i64,
    direction: &str,
    threshold_usd: f64,
    max_stale_ms: i64,
) -> VenueOpenGapLeg {
    match get_cex_venue_delta_snapshot(asset, venue, window_start_ms, threshold_usd, max_stale_ms) {
        Ok(snapshot) => leg_from_snapshot(snapshot, window_start_ms, direction, threshold_usd),
        Err(error) => {
            let err = error.to_string();
            VenueOpenGapLeg {
                venue,
                own_5m_open: None,
                current_mid: None,
                open_timestamp_ms: None,
                current_timestamp_ms: None,
                gap: None,
                opposite_gap: None,
                pass: false,
                opposite_pass: false,
                stale: err.contains("stale"),
                error: Some(classify_venue_error(&err).to_string()),
            }
        }
    }
}

fn leg_from_snapshot(
    snapshot: CexVenueDeltaSnapshot,
    window_start_ms: i64,
    direction: &str,
    threshold_usd: f64,
) -> VenueOpenGapLeg {
    if bucket_5m(snapshot.open_timestamp_ms) != bucket_5m(window_start_ms) {
        return VenueOpenGapLeg {
            venue: snapshot.venue,
            own_5m_open: Some(snapshot.open_mid),
            current_mid: Some(snapshot.current_mid),
            open_timestamp_ms: Some(snapshot.open_timestamp_ms),
            current_timestamp_ms: Some(snapshot.current_timestamp_ms),
            gap: None,
            opposite_gap: None,
            pass: false,
            opposite_pass: false,
            stale: true,
            error: Some("open_gap_stale_open_bucket".to_string()),
        };
    }
    let gap = if direction == "down" {
        -snapshot.delta_usd
    } else {
        snapshot.delta_usd
    };
    let opposite_gap = (-gap).max(0.0);
    VenueOpenGapLeg {
        venue: snapshot.venue,
        own_5m_open: Some(snapshot.open_mid),
        current_mid: Some(snapshot.current_mid),
        open_timestamp_ms: Some(snapshot.open_timestamp_ms),
        current_timestamp_ms: Some(snapshot.current_timestamp_ms),
        gap: Some(gap),
        opposite_gap: Some(opposite_gap),
        pass: gap >= threshold_usd,
        opposite_pass: opposite_gap >= threshold_usd,
        stale: false,
        error: None,
    }
}

fn best_clean_pair(
    pairs: &[[CexVenue; 2]],
    legs: &[VenueOpenGapLeg],
    ratio_min: f64,
    gap_spread_limit: f64,
    fail_reasons: &mut Vec<PairFailReason>,
) -> Option<PairQuality> {
    pairs
        .iter()
        .filter_map(|venues| {
            let quality = pair_quality(*venues, legs, ratio_min, gap_spread_limit, fail_reasons)?;
            Some(quality)
        })
        .min_by(|a, b| {
            a.gap_spread
                .total_cmp(&b.gap_spread)
                .then_with(|| b.gap_ratio.total_cmp(&a.gap_ratio))
        })
}

fn pair_quality(
    venues: [CexVenue; 2],
    legs: &[VenueOpenGapLeg],
    ratio_min: f64,
    gap_spread_limit: f64,
    fail_reasons: &mut Vec<PairFailReason>,
) -> Option<PairQuality> {
    let a = leg_for(legs, venues[0])?;
    let b = leg_for(legs, venues[1])?;
    if !(a.pass && b.pass) {
        return None;
    }
    let gap_a = a.gap?;
    let gap_b = b.gap?;
    let min_gap = gap_a.min(gap_b);
    let max_gap = gap_a.max(gap_b);
    let gap_spread = (gap_a - gap_b).abs();
    let gap_ratio = if max_gap > f64::EPSILON {
        min_gap / max_gap
    } else {
        0.0
    };
    if gap_ratio <= ratio_min {
        fail_reasons.push(PairFailReason {
            venues,
            reason: "open_gap_ratio_too_low",
        });
        return None;
    }
    Some(PairQuality {
        venues,
        gap_spread,
        gap_ratio,
        gap_spread_limit,
    })
}

fn chainlink_sanity_failure(
    chainlink: &EntryCurrentCandidate,
    price_to_beat: f64,
    threshold_usd: f64,
    selected_min_gap: Option<f64>,
    direction: &str,
    gap_spread_limit: f64,
) -> Option<(&'static str, String)> {
    let current = match chainlink.current_price {
        Some(current) => current,
        None => {
            let reason = chainlink.source_reason_code.as_str();
            let detail = chainlink.source_reason_detail.clone().unwrap_or_default();
            let sanity_reason = if reason.contains("stale") || detail.contains("stale") {
                "stale"
            } else {
                "unavailable"
            };
            return Some((sanity_reason, detail));
        }
    };
    let chainlink_gap = if direction == "down" {
        price_to_beat - current
    } else {
        current - price_to_beat
    };
    let opposite_gap = (-chainlink_gap).max(0.0);
    if opposite_gap >= threshold_usd {
        return Some((
            "opposite_direction",
            format!("chainlink opposite_gap={opposite_gap:.8} threshold_usd={threshold_usd:.8}"),
        ));
    }
    if chainlink_gap >= threshold_usd {
        if let Some(selected_min_gap) = selected_min_gap {
            if (chainlink_gap - selected_min_gap).abs() > gap_spread_limit {
                return Some((
                    "deviation_too_high",
                    format!(
                        "chainlink_gap={chainlink_gap:.8} selected_min_gap={selected_min_gap:.8} gap_spread_limit={gap_spread_limit:.8}"
                    ),
                ));
            }
        }
    }
    None
}

fn blocked_candidate(
    config: &CexEntryConsensusConfig,
    reason_code: &'static str,
    reason_detail: Option<String>,
    chainlink_sanity: Option<Value>,
    legs: Vec<VenueOpenGapLeg>,
    quality_fail_reasons: Vec<PairFailReason>,
    selected: Option<PairQuality>,
    effective_expected_move: f64,
    gap_spread_limit: Option<f64>,
    anchor_venue: CexVenue,
    active_venues: &[CexVenue],
) -> EntryCurrentCandidate {
    EntryCurrentCandidate {
        name: SOURCE_NAME,
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK_CEX_CONSENSUS,
        current_price: selected
            .as_ref()
            .and_then(|selection| selected_current(&selection.venues, &legs, "up")),
        source_reason_code: reason_code.to_string(),
        source_reason_detail: reason_detail.clone(),
        cex_context: None,
        debug: candidate_debug(
            config,
            reason_code,
            reason_detail,
            None,
            legs,
            quality_fail_reasons,
            selected,
            chainlink_sanity,
            false,
            effective_expected_move,
            gap_spread_limit,
            anchor_venue,
            active_venues,
        ),
    }
}

fn candidate_debug(
    config: &CexEntryConsensusConfig,
    reason_code: &str,
    reason_detail: Option<String>,
    selected_current: Option<f64>,
    legs: Vec<VenueOpenGapLeg>,
    quality_fail_reasons: Vec<PairFailReason>,
    selected: Option<PairQuality>,
    chainlink_sanity: Option<Value>,
    global_gap_dislocation_warn: bool,
    effective_expected_move: f64,
    gap_spread_limit: Option<f64>,
    anchor_venue: CexVenue,
    active_venues: &[CexVenue],
) -> Value {
    let selected_venues = selected
        .as_ref()
        .map(|pair| {
            pair.venues
                .iter()
                .map(|venue| venue.as_str())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let input_mode = config
        .mode
        .raw
        .as_deref()
        .unwrap_or_else(|| config.mode.mode.as_str());
    let active_venues_json = active_venues
        .iter()
        .map(|venue| venue.as_str())
        .collect::<Vec<_>>();
    let result = json!({
        "basis": config.basis.as_str(),
        "input_mode": input_mode,
        "resolved_mode": resolved_open_gap_mode(config),
        "resolved_open_gap_label": resolved_open_gap_label(config, anchor_venue),
        "anchor_venue": anchor_venue.as_str(),
        "active_venues": active_venues_json,
        "mode": config.mode.mode.as_str(),
        "reason": reason_code,
        "selected_venues": selected_venues,
        "gap_spread": selected.as_ref().map(|pair| pair.gap_spread),
        "gap_ratio": selected.as_ref().map(|pair| pair.gap_ratio),
        "gap_spread_limit": selected
            .as_ref()
            .map(|pair| pair.gap_spread_limit)
            .or(gap_spread_limit),
        "effective_expected_move": effective_expected_move,
        "quality_fail_reasons": quality_fail_reasons
            .iter()
            .map(|fail| json!({
                "pair": fail.venues.iter().map(|venue| venue.as_str()).collect::<Vec<_>>(),
                "reason": fail.reason,
            }))
            .collect::<Vec<_>>(),
        "global_gap_dislocation_warn": global_gap_dislocation_warn,
        "chainlink_sanity": chainlink_sanity.unwrap_or_else(|| json!({ "reason": "chainlink_sanity_ok" })),
        "venues": legs.iter().map(venue_debug).collect::<Vec<_>>(),
    });
    json!({
        "source": SOURCE_NAME,
        "current_price_source": CURRENT_PRICE_SOURCE_CHAINLINK_CEX_CONSENSUS,
        "runtime_source": "own_open_gap",
        "source_available": selected_current.is_some(),
        "selected_current": selected_current,
        "reason_code": reason_code,
        "reason_detail": reason_detail,
        "cex_entry_consensus_result": result,
    })
}

fn venue_debug(leg: &VenueOpenGapLeg) -> Value {
    json!({
        "venue": leg.venue.as_str(),
        "own_5m_open": leg.own_5m_open,
        "current_mid": leg.current_mid,
        "open_timestamp_ms": leg.open_timestamp_ms,
        "current_timestamp_ms": leg.current_timestamp_ms,
        "gap": leg.gap,
        "opposite_gap": leg.opposite_gap.unwrap_or(0.0),
        "pass": leg.pass,
        "opposite_pass": leg.opposite_pass,
        "stale": leg.stale,
        "error": leg.error,
    })
}

fn selected_current(
    venues: &[CexVenue; 2],
    legs: &[VenueOpenGapLeg],
    direction: &str,
) -> Option<f64> {
    let a = leg_for(legs, venues[0])?.current_mid?;
    let b = leg_for(legs, venues[1])?.current_mid?;
    Some(if direction == "down" {
        a.max(b)
    } else {
        a.min(b)
    })
}

fn same_direction_dislocation_warn(
    selected_venues: &[CexVenue; 2],
    legs: &[VenueOpenGapLeg],
    ratio_min: f64,
) -> bool {
    let selected_gaps = selected_venues
        .iter()
        .filter_map(|venue| leg_for(legs, *venue).and_then(|leg| leg.gap))
        .collect::<Vec<_>>();
    let Some(base_min) = selected_gaps.iter().copied().reduce(f64::min) else {
        return false;
    };
    let Some(base_max) = selected_gaps.iter().copied().reduce(f64::max) else {
        return false;
    };
    legs.iter()
        .filter(|leg| leg.pass && !selected_venues.contains(&leg.venue))
        .any(|leg| {
            let Some(gap) = leg.gap else {
                return false;
            };
            let min_gap = base_min.min(gap);
            let max_gap = base_max.max(gap);
            let ratio = if max_gap > f64::EPSILON {
                min_gap / max_gap
            } else {
                0.0
            };
            ratio <= ratio_min
        })
}

fn leg_for(legs: &[VenueOpenGapLeg], venue: CexVenue) -> Option<&VenueOpenGapLeg> {
    legs.iter().find(|leg| leg.venue == venue)
}

impl PairQuality {
    fn min_gap(&self, legs: &[VenueOpenGapLeg]) -> Option<f64> {
        let a = leg_for(legs, self.venues[0])?.gap?;
        let b = leg_for(legs, self.venues[1])?.gap?;
        Some(a.min(b))
    }
}

fn classify_venue_error(error: &str) -> &'static str {
    let lower = error.to_ascii_lowercase();
    if lower.contains("open") {
        "open_gap_missing_open"
    } else if lower.contains("stale") {
        "open_gap_stale_current_mid"
    } else {
        "open_gap_missing_current_mid"
    }
}

fn resolve_threshold_usd(asset: &str, configured_threshold_usd: f64, explicit: bool) -> f64 {
    if explicit {
        return configured_threshold_usd;
    }
    asset_default_threshold_usd(asset).unwrap_or(configured_threshold_usd)
}

fn asset_default_threshold_usd(asset: &str) -> Option<f64> {
    match asset.trim().to_ascii_lowercase().as_str() {
        "btc" => Some(5.0),
        "eth" => Some(0.175),
        "sol" => Some(0.0075),
        _ => None,
    }
}

fn bucket_5m(ts_ms: i64) -> i64 {
    ts_ms / FIVE_MINUTES_MS
}

fn resolved_open_gap_mode(config: &CexEntryConsensusConfig) -> &'static str {
    match config.mode.mode {
        super::entry_current_hybrid::CexEntryConsensusMode::BinanceCoinbase => {
            "open_gap_binance_coinbase"
        }
        super::entry_current_hybrid::CexEntryConsensusMode::OkxPlusOne
        | super::entry_current_hybrid::CexEntryConsensusMode::BybitPlusOne
        | super::entry_current_hybrid::CexEntryConsensusMode::GatePlusOne => {
            "open_gap_anchor_plus_one"
        }
        _ => "open_gap_anchor_plus_one_or_clean_pair",
    }
}

fn resolved_open_gap_label(config: &CexEntryConsensusConfig, anchor_venue: CexVenue) -> String {
    match config.mode.mode {
        super::entry_current_hybrid::CexEntryConsensusMode::BinanceCoinbase => {
            "open_gap_binance_coinbase".to_string()
        }
        super::entry_current_hybrid::CexEntryConsensusMode::OkxPlusOne
        | super::entry_current_hybrid::CexEntryConsensusMode::BybitPlusOne
        | super::entry_current_hybrid::CexEntryConsensusMode::GatePlusOne => {
            format!("open_gap_{}_plus_one", anchor_venue.as_str())
        }
        _ => format!("open_gap_{}_plus_one_or_clean_pair", anchor_venue.as_str()),
    }
}

impl VenueOpenGapLeg {
    fn is_available(&self) -> bool {
        !self.stale && self.error.is_none() && self.current_mid.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade_flow::guards::cex_microstructure::{
        clear_cex_microstructure_test_state, lock_cex_microstructure_test_state,
        seed_cex_book_test_sample, seed_cex_open_test_sample_for_window, CexBookSample,
    };
    use crate::trade_flow::guards::polymarket_price_to_beat::PriceToBeatSource;

    const MARKET: &str = "btc-updown-5m-1774013100";
    const SOL_MARKET: &str = "sol-updown-5m-1774013100";

    fn snapshot() -> PolymarketPriceToBeatSnapshot {
        snapshot_for_asset("btc", MARKET)
    }

    fn snapshot_for_asset(asset: &str, market_slug: &str) -> PolymarketPriceToBeatSnapshot {
        PolymarketPriceToBeatSnapshot {
            event_url: format!("https://polymarket.com/event/{market_slug}"),
            asset: asset.to_string(),
            timeframe: "5m".to_string(),
            price_to_beat: 100.0,
            source: PriceToBeatSource::ChainlinkRtdsStartTick,
            verified: true,
            source_latency_ms: None,
            fetched_at: chrono::Utc::now(),
        }
    }

    fn chainlink(current_price: Option<f64>) -> EntryCurrentCandidate {
        EntryCurrentCandidate {
            name: "chainlink",
            current_price_source: "chainlink_live_data_ws",
            current_price,
            source_reason_code: if current_price.is_some() {
                "source_available"
            } else {
                "chainlink_unavailable"
            }
            .to_string(),
            source_reason_detail: current_price
                .is_none()
                .then(|| "missing chainlink".to_string()),
            cex_context: None,
            debug: json!({}),
        }
    }

    fn config(chainlink_sanity_check: bool) -> CexEntryConsensusConfig {
        let mut config = CexEntryConsensusConfig::default();
        config.open_gap.threshold_usd_explicit = true;
        config.open_gap.chainlink_sanity_check = chainlink_sanity_check;
        config.open_gap.ratio_min = 0.25;
        config.open_gap.spread_floor_usd = 0.20;
        config.open_gap.spread_expected_move_mult = 0.75;
        config
    }

    #[test]
    fn open_gap_threshold_defaults_are_asset_specific() {
        let default_config = CexEntryConsensusConfig::default();
        assert_close(
            resolve_threshold_usd(
                "btc",
                default_config.open_gap.threshold_usd,
                default_config.open_gap.threshold_usd_explicit,
            ),
            5.0,
        );
        assert_close(
            resolve_threshold_usd(
                "eth",
                default_config.open_gap.threshold_usd,
                default_config.open_gap.threshold_usd_explicit,
            ),
            0.175,
        );
        assert_close(
            resolve_threshold_usd(
                "sol",
                default_config.open_gap.threshold_usd,
                default_config.open_gap.threshold_usd_explicit,
            ),
            0.0075,
        );
        assert_close(
            resolve_threshold_usd(
                "xrp",
                default_config.open_gap.threshold_usd,
                default_config.open_gap.threshold_usd_explicit,
            ),
            0.30,
        );

        let mut explicit_config = CexEntryConsensusConfig::default();
        explicit_config.open_gap.threshold_usd = 0.30;
        explicit_config.open_gap.threshold_usd_explicit = true;
        assert_close(
            resolve_threshold_usd(
                "btc",
                explicit_config.open_gap.threshold_usd,
                explicit_config.open_gap.threshold_usd_explicit,
            ),
            0.30,
        );
    }

    fn seed_gap(venue: CexVenue, open_mid: f64, current_mid: f64) {
        seed_gap_for_asset("btc", venue, open_mid, current_mid);
    }

    fn seed_gap_for_asset(asset: &str, venue: CexVenue, open_mid: f64, current_mid: f64) {
        seed_gap_for_asset_with_open_timestamp(
            asset,
            venue,
            open_mid,
            current_mid,
            window_start_ms(),
        );
    }

    fn seed_gap_with_open_timestamp(
        venue: CexVenue,
        open_mid: f64,
        current_mid: f64,
        open_timestamp_ms: i64,
    ) {
        seed_gap_for_asset_with_open_timestamp(
            "btc",
            venue,
            open_mid,
            current_mid,
            open_timestamp_ms,
        );
    }

    fn seed_gap_for_asset_with_open_timestamp(
        asset: &str,
        venue: CexVenue,
        open_mid: f64,
        current_mid: f64,
        open_timestamp_ms: i64,
    ) {
        let start_ms = window_start_ms();
        seed_cex_open_test_sample_for_window(
            start_ms,
            CexBookSample {
                venue,
                asset: asset.to_string(),
                timestamp_ms: open_timestamp_ms,
                bid: open_mid - 0.5,
                ask: open_mid + 0.5,
                bid_size: Some(1.0),
                ask_size: Some(1.0),
                source: "rest_open",
            },
        );
        seed_cex_book_test_sample(CexBookSample {
            venue,
            asset: asset.to_string(),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            bid: current_mid - 0.5,
            ask: current_mid + 0.5,
            bid_size: Some(1.0),
            ask_size: Some(1.0),
            source: "bookTicker",
        });
    }

    fn window_start_ms() -> i64 {
        crate::MarketCycleId(MARKET.to_string())
            .start_time()
            .expect("market start")
            .timestamp_millis()
    }

    fn result(candidate: &EntryCurrentCandidate) -> &Value {
        candidate
            .debug
            .get("cex_entry_consensus_result")
            .expect("cex consensus result")
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.000001,
            "actual={actual} expected={expected}"
        );
    }

    fn string_array(value: Option<&Value>) -> Vec<&str> {
        value
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
            .unwrap_or_default()
    }

    fn venue_result<'a>(candidate: &'a EntryCurrentCandidate, venue_name: &str) -> &'a Value {
        result(candidate)
            .get("venues")
            .and_then(Value::as_array)
            .and_then(|venues| {
                venues
                    .iter()
                    .find(|venue| venue.get("venue").and_then(Value::as_str) == Some(venue_name))
            })
            .expect("venue debug")
    }

    #[test]
    fn expected_move_eff_reports_spread_limit_without_blocking_clean_ratio() {
        let _cex_guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_gap(CexVenue::Binance, 100.0, 99.15);
        seed_gap(CexVenue::Coinbase, 100.0, 99.60);

        let pass = own_open_gap_candidate(
            MARKET,
            "Down",
            &snapshot(),
            &config(false),
            &chainlink(None),
            Some(1.0),
        );
        assert_eq!(pass.source_reason_code, "source_available");
        assert_eq!(
            result(&pass).get("reason").and_then(Value::as_str),
            Some("open_gap_anchor_pair_pass")
        );
        assert_close(
            result(&pass)
                .get("gap_spread_limit")
                .and_then(Value::as_f64)
                .expect("gap_spread_limit"),
            0.75,
        );
        assert_close(
            result(&pass)
                .get("effective_expected_move")
                .and_then(Value::as_f64)
                .expect("effective_expected_move"),
            1.0,
        );

        let fallback = own_open_gap_candidate(
            MARKET,
            "Down",
            &snapshot(),
            &config(false),
            &chainlink(None),
            None,
        );
        assert_eq!(fallback.source_reason_code, "source_available");
        assert_close(
            result(&fallback)
                .get("gap_spread_limit")
                .and_then(Value::as_f64)
                .expect("gap_spread_limit"),
            0.225,
        );
        assert_close(
            result(&fallback)
                .get("effective_expected_move")
                .and_then(Value::as_f64)
                .expect("effective_expected_move"),
            0.30,
        );
    }

    #[test]
    fn clean_pair_ignores_large_usd_spread_when_ratio_is_strong() {
        let _cex_guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_gap(CexVenue::Binance, 1000.0, 847.735);
        seed_gap(CexVenue::Coinbase, 1000.0, 852.060);
        let mut config = config(false);
        config.open_gap.ratio_min = 0.25;

        let candidate = own_open_gap_candidate(
            MARKET,
            "Down",
            &snapshot(),
            &config,
            &chainlink(None),
            Some(1.0),
        );

        assert_eq!(candidate.source_reason_code, "source_available");
        assert_close(
            result(&candidate)
                .get("gap_spread")
                .and_then(Value::as_f64)
                .expect("gap_spread"),
            4.325,
        );
        assert_close(
            result(&candidate)
                .get("gap_ratio")
                .and_then(Value::as_f64)
                .expect("gap_ratio"),
            147.94 / 152.265,
        );
    }

    #[test]
    fn clean_pair_blocks_ratio_at_or_below_minimum() {
        let _cex_guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_gap(CexVenue::Binance, 100.0, 80.0);
        seed_gap(CexVenue::Coinbase, 100.0, 95.0);
        let mut config = config(false);
        config.open_gap.ratio_min = 0.25;

        let candidate = own_open_gap_candidate(
            MARKET,
            "Down",
            &snapshot(),
            &config,
            &chainlink(None),
            Some(1.0),
        );

        assert_eq!(candidate.source_reason_code, "open_gap_no_clean_pair");
        assert_eq!(
            result(&candidate)
                .get("quality_fail_reasons")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("reason"))
                .and_then(Value::as_str),
            Some("open_gap_ratio_too_low")
        );
    }

    #[test]
    fn clean_pair_allows_ratio_above_minimum() {
        let _cex_guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_gap(CexVenue::Binance, 100.0, 80.0);
        seed_gap(CexVenue::Coinbase, 100.0, 94.0);
        let mut config = config(false);
        config.open_gap.ratio_min = 0.25;

        let candidate = own_open_gap_candidate(
            MARKET,
            "Down",
            &snapshot(),
            &config,
            &chainlink(None),
            Some(1.0),
        );

        assert_eq!(candidate.source_reason_code, "source_available");
        assert_close(
            result(&candidate)
                .get("gap_ratio")
                .and_then(Value::as_f64)
                .expect("gap_ratio"),
            0.30,
        );
    }

    #[test]
    fn binance_coinbase_pair_is_selected_for_active_open_gap() {
        let _cex_guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_gap(CexVenue::Okx, 100.0, 99.50);
        seed_gap(CexVenue::Binance, 100.0, 99.20);
        seed_gap(CexVenue::Coinbase, 100.0, 99.19);

        let candidate = own_open_gap_candidate(
            MARKET,
            "Down",
            &snapshot(),
            &config(false),
            &chainlink(None),
            Some(1.0),
        );

        assert_eq!(candidate.source_reason_code, "source_available");
        assert_eq!(
            result(&candidate)
                .get("selected_venues")
                .and_then(Value::as_array)
                .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>()),
            Some(vec!["coinbase", "binance"])
        );
    }

    #[test]
    fn inactive_stale_okx_is_excluded_without_blocking_binance_coinbase_pair() {
        let _cex_guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_gap_with_open_timestamp(
            CexVenue::Okx,
            100.0,
            99.50,
            window_start_ms() - FIVE_MINUTES_MS,
        );
        seed_gap(CexVenue::Binance, 100.0, 99.50);
        seed_gap(CexVenue::Coinbase, 100.0, 99.48);

        let candidate = own_open_gap_candidate(
            MARKET,
            "Down",
            &snapshot(),
            &config(false),
            &chainlink(None),
            Some(1.0),
        );

        assert_eq!(candidate.source_reason_code, "source_available");
        assert_eq!(
            result(&candidate).get("reason").and_then(Value::as_str),
            Some("open_gap_anchor_pair_pass")
        );
        assert!(result(&candidate)
            .get("venues")
            .and_then(Value::as_array)
            .is_some_and(|venues| venues
                .iter()
                .all(|venue| { venue.get("venue").and_then(Value::as_str) != Some("okx") })));
    }

    #[test]
    fn chainlink_sanity_blocks_opposite_when_enabled_but_not_when_disabled() {
        let _cex_guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_gap(CexVenue::Binance, 100.0, 99.48);
        seed_gap(CexVenue::Coinbase, 100.0, 99.50);

        let enabled = own_open_gap_candidate(
            MARKET,
            "Down",
            &snapshot(),
            &config(true),
            &chainlink(Some(100.60)),
            Some(1.0),
        );
        assert_eq!(enabled.source_reason_code, "open_gap_chainlink_sanity_fail");

        let disabled = own_open_gap_candidate(
            MARKET,
            "Down",
            &snapshot(),
            &config(false),
            &chainlink(None),
            Some(1.0),
        );
        assert_eq!(disabled.source_reason_code, "source_available");
    }

    #[test]
    fn sol_uses_binance_coinbase_and_ignores_gateio_okx() {
        let _cex_guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_gap_for_asset("sol", CexVenue::Gateio, 100.0, 90.0);
        seed_gap_for_asset("sol", CexVenue::Okx, 100.0, 90.0);
        seed_gap_for_asset("sol", CexVenue::Binance, 100.0, 100.50);
        seed_gap_for_asset("sol", CexVenue::Coinbase, 100.0, 100.55);

        let candidate = own_open_gap_candidate(
            SOL_MARKET,
            "Up",
            &snapshot_for_asset("sol", SOL_MARKET),
            &config(false),
            &chainlink(None),
            Some(1.0),
        );

        assert_eq!(candidate.source_reason_code, "source_available");
        assert_eq!(
            result(&candidate).get("reason").and_then(Value::as_str),
            Some("open_gap_anchor_pair_pass")
        );
        assert_eq!(
            result(&candidate)
                .get("anchor_venue")
                .and_then(Value::as_str),
            Some("coinbase")
        );
        assert_eq!(
            string_array(result(&candidate).get("active_venues")),
            vec!["binance", "coinbase"]
        );
        assert_eq!(
            string_array(result(&candidate).get("selected_venues")),
            vec!["coinbase", "binance"]
        );
        assert_eq!(
            result(&candidate)
                .get("resolved_mode")
                .and_then(Value::as_str),
            Some("open_gap_binance_coinbase")
        );
        assert_eq!(
            result(&candidate)
                .get("resolved_open_gap_label")
                .and_then(Value::as_str),
            Some("open_gap_binance_coinbase")
        );
        assert!(result(&candidate)
            .get("venues")
            .and_then(Value::as_array)
            .is_some_and(|venues| venues.iter().all(|venue| {
                !matches!(
                    venue.get("venue").and_then(Value::as_str),
                    Some("okx" | "gateio")
                )
            })));
    }

    #[test]
    fn sol_coinbase_gap_uses_coinbase_own_open() {
        let _cex_guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_gap_for_asset("sol", CexVenue::Coinbase, 100.0, 101.0);
        seed_gap_for_asset("sol", CexVenue::Binance, 500.0, 501.10);
        seed_gap_for_asset("sol", CexVenue::Okx, 900.0, 850.0);

        let candidate = own_open_gap_candidate(
            SOL_MARKET,
            "Up",
            &snapshot_for_asset("sol", SOL_MARKET),
            &config(false),
            &chainlink(None),
            Some(1.0),
        );

        assert_eq!(candidate.source_reason_code, "source_available");
        let coinbase = venue_result(&candidate, "coinbase");
        assert_close(
            coinbase
                .get("own_5m_open")
                .and_then(Value::as_f64)
                .expect("coinbase open"),
            100.0,
        );
        assert_close(
            coinbase
                .get("current_mid")
                .and_then(Value::as_f64)
                .expect("coinbase current"),
            101.0,
        );
        assert_close(
            coinbase
                .get("gap")
                .and_then(Value::as_f64)
                .expect("coinbase gap"),
            1.0,
        );
    }

    #[test]
    fn sol_inactive_okx_gateio_opposite_does_not_veto_but_coinbase_opposite_does() {
        let _cex_guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_gap_for_asset("sol", CexVenue::Gateio, 100.0, 99.0);
        seed_gap_for_asset("sol", CexVenue::Okx, 100.0, 99.0);
        seed_gap_for_asset("sol", CexVenue::Binance, 100.0, 100.50);
        seed_gap_for_asset("sol", CexVenue::Coinbase, 100.0, 100.55);

        let okx_ignored = own_open_gap_candidate(
            SOL_MARKET,
            "Up",
            &snapshot_for_asset("sol", SOL_MARKET),
            &config(false),
            &chainlink(None),
            Some(1.0),
        );
        assert_eq!(okx_ignored.source_reason_code, "source_available");

        clear_cex_microstructure_test_state();
        seed_gap_for_asset("sol", CexVenue::Binance, 100.0, 100.50);
        seed_gap_for_asset("sol", CexVenue::Coinbase, 100.0, 99.0);

        let coinbase_veto = own_open_gap_candidate(
            SOL_MARKET,
            "Up",
            &snapshot_for_asset("sol", SOL_MARKET),
            &config(false),
            &chainlink(None),
            Some(1.0),
        );
        assert_eq!(
            coinbase_veto.source_reason_code,
            "open_gap_opposite_venue_detected"
        );
        assert!(coinbase_veto
            .source_reason_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("venue=coinbase")));
    }

    #[test]
    fn sol_binance_coinbase_pair_does_not_depend_on_legacy_clean_pair_fallback() {
        let _cex_guard = lock_cex_microstructure_test_state();
        clear_cex_microstructure_test_state();
        seed_gap_for_asset_with_open_timestamp(
            "sol",
            CexVenue::Gateio,
            100.0,
            100.55,
            window_start_ms() - FIVE_MINUTES_MS,
        );
        seed_gap_for_asset("sol", CexVenue::Binance, 100.0, 100.55);
        seed_gap_for_asset("sol", CexVenue::Coinbase, 100.0, 100.52);

        let enabled = own_open_gap_candidate(
            SOL_MARKET,
            "Up",
            &snapshot_for_asset("sol", SOL_MARKET),
            &config(false),
            &chainlink(None),
            Some(1.0),
        );
        assert_eq!(enabled.source_reason_code, "source_available");
        assert_eq!(
            result(&enabled).get("reason").and_then(Value::as_str),
            Some("open_gap_anchor_pair_pass")
        );
        assert_eq!(
            string_array(result(&enabled).get("selected_venues")),
            vec!["coinbase", "binance"]
        );

        let mut disabled_config = config(false);
        disabled_config.open_gap.allow_clean_pair_without_anchor = false;
        let disabled = own_open_gap_candidate(
            SOL_MARKET,
            "Up",
            &snapshot_for_asset("sol", SOL_MARKET),
            &disabled_config,
            &chainlink(None),
            Some(1.0),
        );
        assert_eq!(disabled.source_reason_code, "source_available");
    }
}
