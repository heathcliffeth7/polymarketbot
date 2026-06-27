use super::{
    TradeBuilderPtbStopLossSourceEvaluation, evaluate_cex_consensus_ptb_stop_loss,
    trade_builder_ptb_directional_gap, trade_builder_ptb_stop_loss_source_evaluation,
    trade_builder_ptb_stop_loss_source_reason_code,
};
use crate::trade_flow::guards::cex_microstructure::{
    active_spot_venues_for_asset, CexVenue, CexVenueDeltaSnapshot, get_cex_venue_delta_snapshot,
};
use crate::trade_flow::guards::price_to_beat::PriceToBeatCurrentPriceSource;
use crate::{MarketCycleId, find_updown_scope_by_slug};
use serde_json::{Value, json};

const CEX_MEDIAN_FAST_MAX_STALE_MS: i64 = 750;
#[derive(Debug, Clone)]
struct CexMedianFastLeg {
    snapshot: Option<CexVenueDeltaSnapshot>,
    error: Option<String>,
}

pub(super) fn cex_median_fast_source_evaluations(
    market_slug: &str,
    asset: &str,
    direction: &str,
    threshold_gap_usd: f64,
    base_threshold_gap_usd: f64,
    ptb_reference_price: f64,
) -> Vec<TradeBuilderPtbStopLossSourceEvaluation> {
    vec![
        evaluate_cex_median_fast_ptb_stop_loss(
            market_slug,
            direction,
            threshold_gap_usd,
            base_threshold_gap_usd,
            ptb_reference_price,
        ),
        trade_builder_ptb_stop_loss_source_evaluation(
            PriceToBeatCurrentPriceSource::Chainlink,
            market_slug,
            asset,
            direction,
            threshold_gap_usd,
            ptb_reference_price,
        ),
        evaluate_cex_consensus_ptb_stop_loss(
            market_slug,
            direction,
            threshold_gap_usd,
            ptb_reference_price,
        ),
    ]
}

fn evaluate_cex_median_fast_ptb_stop_loss(
    market_slug: &str,
    direction: &str,
    threshold_gap_usd: f64,
    base_threshold_gap_usd: f64,
    ptb_reference_price: f64,
) -> TradeBuilderPtbStopLossSourceEvaluation {
    let source = PriceToBeatCurrentPriceSource::CexMedianFast;
    let Some(scope) = find_updown_scope_by_slug(market_slug) else {
        return unavailable(source, "cex_median_fast_unsupported_market", None, None);
    };
    let Some(window_start) = MarketCycleId(market_slug.to_string()).start_time() else {
        return unavailable(source, "cex_median_fast_missing_window_start", None, None);
    };
    let window_start_ms = window_start.timestamp_millis();
    let legs = active_spot_venues_for_asset(scope.asset)
        .into_iter()
        .map(|venue| {
            (
                venue,
                load_cex_median_fast_leg(scope.asset, venue, window_start_ms),
            )
        })
        .collect::<Vec<_>>();
    let valid = legs
        .iter()
        .filter_map(|(_, leg)| leg.snapshot.as_ref())
        .collect::<Vec<_>>();
    let sanity_band_usd = 3.0 * base_threshold_gap_usd.abs();
    let selected = select_cex_median_fast_delta(&valid, direction, sanity_band_usd);
    let metadata = cex_median_fast_metadata(
        market_slug,
        scope.asset,
        scope.timeframe,
        window_start_ms,
        sanity_band_usd,
        &legs,
        &selected,
    );
    let Some((selected_delta_usd, mode)) = selected.delta_mode else {
        return unavailable(
            source,
            selected.error_code,
            selected.error_detail,
            Some(metadata),
        );
    };
    let current_price = ptb_reference_price + selected_delta_usd;
    let directional_gap =
        trade_builder_ptb_directional_gap(direction, ptb_reference_price, current_price);
    let should_trigger = directional_gap <= threshold_gap_usd;
    TradeBuilderPtbStopLossSourceEvaluation {
        config_source: source.as_config_str(),
        current_price_source: source.current_price_source_label(),
        current_price: Some(current_price),
        directional_gap: Some(directional_gap),
        reason_code: trade_builder_ptb_stop_loss_source_reason_code(source, should_trigger),
        should_trigger,
        error_code: None,
        error_detail: None,
        metadata: Some(metadata_with_selection(
            metadata,
            mode,
            selected_delta_usd,
            current_price,
        )),
    }
}

fn load_cex_median_fast_leg(
    asset: &str,
    venue: CexVenue,
    window_start_ms: i64,
) -> CexMedianFastLeg {
    match get_cex_venue_delta_snapshot(
        asset,
        venue,
        window_start_ms,
        0.0,
        CEX_MEDIAN_FAST_MAX_STALE_MS,
    ) {
        Ok(snapshot) if snapshot.book_staleness_ms < CEX_MEDIAN_FAST_MAX_STALE_MS => {
            CexMedianFastLeg {
                snapshot: Some(snapshot),
                error: None,
            }
        }
        Ok(snapshot) => CexMedianFastLeg {
            snapshot: None,
            error: Some(format!(
                "{} book stale: age_ms={} max_ms={}",
                venue.as_str(),
                snapshot.book_staleness_ms,
                CEX_MEDIAN_FAST_MAX_STALE_MS
            )),
        },
        Err(err) => CexMedianFastLeg {
            snapshot: None,
            error: Some(err.to_string()),
        },
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CexMedianFastSelection {
    delta_mode: Option<(f64, &'static str)>,
    error_code: &'static str,
    error_detail: Option<String>,
}

fn select_cex_median_fast_delta(
    valid: &[&CexVenueDeltaSnapshot],
    direction: &str,
    sanity_band_usd: f64,
) -> CexMedianFastSelection {
    match valid.len() {
        3 => {
            let mut deltas = valid
                .iter()
                .map(|snapshot| snapshot.delta_usd)
                .collect::<Vec<_>>();
            deltas.sort_by(f64::total_cmp);
            CexMedianFastSelection {
                delta_mode: Some((deltas[1], "median")),
                error_code: "cex_median_fast_available",
                error_detail: None,
            }
        }
        2 => {
            let left = valid[0].delta_usd;
            let right = valid[1].delta_usd;
            if (left - right).abs() > sanity_band_usd {
                return CexMedianFastSelection {
                    delta_mode: None,
                    error_code: "cex_median_fast_sanity_band_exceeded",
                    error_detail: Some(format!(
                        "two-leg delta diff exceeded sanity band: diff_usd={} sanity_band_usd={}",
                        (left - right).abs(),
                        sanity_band_usd
                    )),
                };
            }
            let selected = if direction == "up" {
                left.min(right)
            } else {
                left.max(right)
            };
            CexMedianFastSelection {
                delta_mode: Some((selected, "conservative")),
                error_code: "cex_median_fast_available",
                error_detail: None,
            }
        }
        _ => CexMedianFastSelection {
            delta_mode: None,
            error_code: "cex_median_fast_insufficient_venues",
            error_detail: Some(format!("valid_venue_count={}", valid.len())),
        },
    }
}

fn cex_median_fast_metadata(
    market_slug: &str,
    asset: &str,
    timeframe: &str,
    window_start_ms: i64,
    sanity_band_usd: f64,
    legs: &[(CexVenue, CexMedianFastLeg)],
    selected: &CexMedianFastSelection,
) -> Value {
    json!({
        "mode": selected.delta_mode.map(|(_, mode)| mode).unwrap_or("fallback"),
        "fallback_reason": selected.delta_mode.is_none().then_some(selected.error_code),
        "market_slug": market_slug,
        "asset": asset,
        "timeframe": timeframe,
        "window_start_ms": window_start_ms,
        "sanity_band_usd": sanity_band_usd,
        "venue_deltas": legs.iter().map(|(venue, leg)| {
            (
                venue.as_str().to_string(),
                leg.snapshot
                    .as_ref()
                    .map(CexVenueDeltaSnapshot::to_value)
                    .unwrap_or_else(|| json!({ "error": leg.error })),
            )
        }).collect::<serde_json::Map<String, Value>>(),
    })
}

fn metadata_with_selection(
    mut metadata: Value,
    mode: &'static str,
    selected_delta_usd: f64,
    effective_price: f64,
) -> Value {
    if let Some(obj) = metadata.as_object_mut() {
        obj.insert("mode".to_string(), json!(mode));
        obj.insert("selected_delta_usd".to_string(), json!(selected_delta_usd));
        obj.insert("effective_price".to_string(), json!(effective_price));
    }
    metadata
}

fn unavailable(
    source: PriceToBeatCurrentPriceSource,
    error_code: &'static str,
    error_detail: Option<String>,
    metadata: Option<Value>,
) -> TradeBuilderPtbStopLossSourceEvaluation {
    TradeBuilderPtbStopLossSourceEvaluation {
        config_source: source.as_config_str(),
        current_price_source: source.current_price_source_label(),
        current_price: None,
        directional_gap: None,
        reason_code: "threshold_not_met",
        should_trigger: false,
        error_code: Some(error_code),
        error_detail,
        metadata,
    }
}

#[cfg(test)]
mod tests {
    use super::super::trade_builder_ptb_stop_loss_tests::{
        current_5m_market_slug, lock_ptb_stop_loss_test_state, seed_ptb_stop_loss_current_price,
        test_ptb_stop_loss_order,
    };
    use super::*;
    use crate::trade_flow::guards::cex_microstructure::{
        CexBookSample, clear_cex_microstructure_test_state, lock_cex_microstructure_test_state,
        seed_cex_book_test_sample, seed_cex_open_test_sample,
    };
    use chrono::Utc;

    fn sample(venue: CexVenue, timestamp_ms: i64, mid: f64) -> CexBookSample {
        sample_for_asset("btc", venue, timestamp_ms, mid)
    }

    fn sample_for_asset(
        asset: &str,
        venue: CexVenue,
        timestamp_ms: i64,
        mid: f64,
    ) -> CexBookSample {
        CexBookSample {
            venue,
            asset: asset.to_string(),
            timestamp_ms,
            bid: mid - 0.5,
            ask: mid + 0.5,
            bid_size: Some(1.0),
            ask_size: Some(1.0),
            source: "rest_open",
        }
    }

    fn book(venue: CexVenue, timestamp_ms: i64, mid: f64) -> CexBookSample {
        book_for_asset("btc", venue, timestamp_ms, mid)
    }

    fn book_for_asset(
        asset: &str,
        venue: CexVenue,
        timestamp_ms: i64,
        mid: f64,
    ) -> CexBookSample {
        CexBookSample {
            source: "ticker",
            ..sample_for_asset(asset, venue, timestamp_ms, mid)
        }
    }

    fn seed_leg(venue: CexVenue, start_ms: i64, open_mid: f64, current_mid: f64) {
        seed_cex_open_test_sample(sample(venue, start_ms, open_mid));
        seed_cex_book_test_sample(book(venue, Utc::now().timestamp_millis(), current_mid));
    }

    fn seed_leg_for_asset(
        asset: &str,
        venue: CexVenue,
        start_ms: i64,
        open_mid: f64,
        current_mid: f64,
    ) {
        seed_cex_open_test_sample(sample_for_asset(asset, venue, start_ms, open_mid));
        seed_cex_book_test_sample(book_for_asset(
            asset,
            venue,
            Utc::now().timestamp_millis(),
            current_mid,
        ));
    }

    fn median_order(market_slug: &str, outcome_label: &str) -> super::super::TradeBuilderOrder {
        let mut order = test_ptb_stop_loss_order(market_slug, outcome_label, -5.0, Some(100.0));
        order.ptb_current_price_source = "cex_median_fast".to_string();
        order
    }

    #[tokio::test]
    async fn cex_median_fast_is_basis_independent() {
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("btc", 120.0);
        let (market_slug, start_ms) = current_5m_market_slug("btc");
        seed_leg(CexVenue::Binance, start_ms, 100.0, 94.0);
        seed_leg(CexVenue::Okx, start_ms, 200.0, 194.0);
        seed_leg(CexVenue::Coinbase, start_ms, 300.0, 294.0);

        let evaluation =
            super::super::trade_builder_evaluate_ptb_stop_loss(&median_order(&market_slug, "Up"))
                .expect("ptb eval");

        assert!(evaluation.should_trigger);
        assert_eq!(evaluation.current_price_source, "cex_median_fast_delta");
        assert_eq!(evaluation.current_price, Some(94.0));
    }

    #[tokio::test]
    async fn cex_median_fast_uses_median_with_three_venues() {
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("btc", 120.0);
        let (market_slug, start_ms) = current_5m_market_slug("btc");
        seed_leg(CexVenue::Binance, start_ms, 100.0, 70.0);
        seed_leg(CexVenue::Okx, start_ms, 100.0, 94.0);
        seed_leg(CexVenue::Coinbase, start_ms, 100.0, 95.0);

        let evaluation =
            super::super::trade_builder_evaluate_ptb_stop_loss(&median_order(&market_slug, "Up"))
                .expect("ptb eval");

        assert_eq!(evaluation.directional_gap, Some(-6.0));
    }

    #[tokio::test]
    async fn sol_cex_median_fast_uses_gateio_active_leg() {
        let _cex_guard = lock_cex_microstructure_test_state();
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("sol", 120.0);
        let (market_slug, start_ms) = current_5m_market_slug("sol");
        seed_leg_for_asset("sol", CexVenue::Binance, start_ms, 100.0, 70.0);
        seed_leg_for_asset("sol", CexVenue::Gateio, start_ms, 100.0, 94.0);
        seed_leg_for_asset("sol", CexVenue::Coinbase, start_ms, 100.0, 95.0);

        let evaluation =
            super::super::trade_builder_evaluate_ptb_stop_loss(&median_order(&market_slug, "Up"))
                .expect("ptb eval");

        assert_eq!(evaluation.source_evaluations[0].directional_gap, Some(-6.0));
        let metadata = evaluation.source_evaluations[0]
            .metadata
            .as_ref()
            .expect("cex median metadata");
        assert_eq!(
            metadata["venue_deltas"]["gateio"]["venue"],
            serde_json::json!("gateio")
        );
    }

    #[tokio::test]
    async fn cex_median_fast_two_venues_choose_adverse_direction() {
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("btc", 120.0);
        let (market_slug, start_ms) = current_5m_market_slug("btc");
        seed_leg(CexVenue::Binance, start_ms, 100.0, 96.0);
        seed_leg(CexVenue::Okx, start_ms, 100.0, 94.0);

        let up =
            super::super::trade_builder_evaluate_ptb_stop_loss(&median_order(&market_slug, "Up"))
                .expect("up eval");
        let down =
            super::super::trade_builder_evaluate_ptb_stop_loss(&median_order(&market_slug, "Down"))
                .expect("down eval");

        assert_eq!(up.directional_gap, Some(-6.0));
        assert_eq!(down.source_evaluations[0].directional_gap, Some(4.0));
        assert!(!down.source_evaluations[0].should_trigger);
    }

    #[tokio::test]
    async fn cex_median_fast_two_venue_sanity_falls_back_to_confirmed_backstop() {
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("btc", 90.0);
        let (market_slug, start_ms) = current_5m_market_slug("btc");
        seed_leg(CexVenue::Binance, start_ms, 100.0, 50.0);
        seed_leg(CexVenue::Okx, start_ms, 100.0, 99.0);

        let evaluation =
            super::super::trade_builder_evaluate_ptb_stop_loss(&median_order(&market_slug, "Up"))
                .expect("ptb eval");

        assert!(evaluation.should_trigger);
        assert_eq!(evaluation.current_price_source, "chainlink_live_data_ws");
        assert_eq!(
            evaluation.source_evaluations[0].error_code,
            Some("cex_median_fast_sanity_band_exceeded")
        );
    }

    #[tokio::test]
    async fn cex_median_fast_down_triggers_on_price_rise_not_drop() {
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("btc", 120.0);
        let (market_slug, start_ms) = current_5m_market_slug("btc");
        seed_leg(CexVenue::Binance, start_ms, 100.0, 106.0);
        seed_leg(CexVenue::Okx, start_ms, 100.0, 106.0);
        seed_leg(CexVenue::Coinbase, start_ms, 100.0, 106.0);

        let rise =
            super::super::trade_builder_evaluate_ptb_stop_loss(&median_order(&market_slug, "Down"))
                .expect("rise eval");
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("btc", 120.0);
        seed_leg(CexVenue::Binance, start_ms, 100.0, 94.0);
        seed_leg(CexVenue::Okx, start_ms, 100.0, 94.0);
        seed_leg(CexVenue::Coinbase, start_ms, 100.0, 94.0);
        let drop =
            super::super::trade_builder_evaluate_ptb_stop_loss(&median_order(&market_slug, "Down"))
                .expect("drop eval");

        assert!(rise.should_trigger);
        assert!(!drop.source_evaluations[0].should_trigger);
    }
}
