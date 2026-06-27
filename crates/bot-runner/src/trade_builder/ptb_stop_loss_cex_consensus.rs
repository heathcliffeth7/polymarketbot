#[derive(Debug, Clone)]
struct TradeBuilderCexConsensusDelta {
    snapshot: Option<crate::trade_flow::guards::cex_microstructure::CexVenueDeltaSnapshot>,
    error: Option<String>,
}

fn evaluate_cex_consensus_ptb_stop_loss(
    market_slug: &str,
    direction: &str,
    threshold_gap_usd: f64,
    ptb_reference_price: f64,
) -> TradeBuilderPtbStopLossSourceEvaluation {
    let source = PriceToBeatCurrentPriceSource::CexConsensus;
    let Some(scope) = find_updown_scope_by_slug(market_slug) else {
        return cex_consensus_unavailable_source(
            source,
            "cex_consensus_unsupported_market",
            Some("market slug is not a supported updown scope".to_string()),
            None,
        );
    };
    let Some(window_start) = MarketCycleId(market_slug.to_string()).start_time() else {
        return cex_consensus_unavailable_source(
            source,
            "cex_consensus_missing_window_start",
            None,
            None,
        );
    };

    let config =
        crate::trade_flow::guards::price_to_beat::CexDirectionGuardConfig::consensus_stop_loss_defaults(
        );
    let window_start_ms = window_start.timestamp_millis();
    let anchor_venue =
        crate::trade_flow::guards::cex_microstructure::active_anchor_venue_for_asset(scope.asset);
    let anchor_name = anchor_venue.as_str();
    let anchor = load_cex_consensus_delta(
        scope.asset,
        anchor_venue,
        window_start_ms,
        config.min_move_usd,
        config.max_book_stale_ms,
    );
    let binance = load_cex_consensus_delta(
        scope.asset,
        crate::trade_flow::guards::cex_microstructure::CexVenue::Binance,
        window_start_ms,
        config.min_move_usd,
        config.max_book_stale_ms,
    );
    let coinbase = load_cex_consensus_delta(
        scope.asset,
        crate::trade_flow::guards::cex_microstructure::CexVenue::Coinbase,
        window_start_ms,
        config.min_move_usd,
        config.max_book_stale_ms,
    );

    let anchor_snapshot = anchor.snapshot.as_ref().map(|snapshot| {
        cex_consensus_decorated_delta(snapshot, direction, threshold_gap_usd, "lead")
    });
    let binance_snapshot = binance.snapshot.as_ref().map(|snapshot| {
        cex_consensus_decorated_delta(snapshot, direction, threshold_gap_usd, "confirm_candidate")
    });
    let coinbase_snapshot = coinbase.snapshot.as_ref().map(|snapshot| {
        cex_consensus_decorated_delta(snapshot, direction, threshold_gap_usd, "confirm_candidate")
    });

    let anchor_hit = anchor_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.threshold_hit)
        .unwrap_or(false);
    let confirming_venues = [
        ("binance", binance_snapshot.as_ref()),
        ("coinbase", coinbase_snapshot.as_ref()),
    ]
    .into_iter()
    .filter(|(venue, _)| *venue != anchor_name)
    .filter_map(|(venue, snapshot)| {
        snapshot
            .and_then(|snapshot| snapshot.threshold_hit)
            .unwrap_or(false)
            .then_some(venue)
    })
    .collect::<Vec<_>>();
    let should_trigger = anchor_hit && !confirming_venues.is_empty();
    let current_price = anchor_snapshot
        .as_ref()
        .map(|snapshot| ptb_reference_price + snapshot.delta_usd);
    let directional_gap = anchor_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.directional_gap);
    let metadata = cex_consensus_metadata(
        market_slug,
        scope.asset,
        scope.timeframe,
        window_start_ms,
        anchor_name,
        anchor_snapshot.as_ref(),
        binance_snapshot.as_ref(),
        coinbase_snapshot.as_ref(),
        &anchor,
        &binance,
        &coinbase,
        &confirming_venues,
    );
    let (error_code, error_detail) =
        cex_consensus_error(anchor_name, &anchor, anchor_hit, &confirming_venues);

    TradeBuilderPtbStopLossSourceEvaluation {
        config_source: source.as_config_str(),
        current_price_source: source.current_price_source_label(),
        current_price,
        directional_gap,
        reason_code: trade_builder_ptb_stop_loss_source_reason_code(source, should_trigger),
        should_trigger,
        error_code,
        error_detail,
        metadata: Some(metadata),
    }
}

fn trade_builder_ptb_stop_loss_final_should_trigger(
    current_price_source: PriceToBeatCurrentPriceSource,
    selected_source: &TradeBuilderPtbStopLossSourceEvaluation,
    source_evaluations: &[TradeBuilderPtbStopLossSourceEvaluation],
    threshold_gap_usd: f64,
) -> bool {
    if current_price_source == PriceToBeatCurrentPriceSource::CexMedianFast {
        return source_evaluations.iter().any(|evaluation| {
            evaluation.config_source == "cex_median_fast"
                && evaluation.current_price.is_some()
                && evaluation.should_trigger
        }) || confirmed_cex_backstop_should_trigger(source_evaluations, threshold_gap_usd);
    }
    if current_price_source != PriceToBeatCurrentPriceSource::ChainlinkCexConsensusConfirmed {
        return selected_source.should_trigger;
    }
    confirmed_cex_backstop_should_trigger(source_evaluations, threshold_gap_usd)
}

fn confirmed_cex_backstop_should_trigger(
    source_evaluations: &[TradeBuilderPtbStopLossSourceEvaluation],
    threshold_gap_usd: f64,
) -> bool {
    let chainlink = source_evaluations
        .iter()
        .find(|evaluation| evaluation.config_source == "chainlink");
    if chainlink
        .is_some_and(|evaluation| evaluation.current_price.is_some() && evaluation.should_trigger)
    {
        return true;
    }
    if chainlink.is_some_and(|evaluation| evaluation.current_price.is_some()) {
        return false;
    }
    source_evaluations
        .iter()
        .find(|evaluation| evaluation.config_source == "cex_consensus")
        .is_some_and(|evaluation| {
            evaluation.current_price.is_some()
                && evaluation.should_trigger
                && evaluation
                    .directional_gap
                    .is_some_and(|gap| confirmed_cex_fallback_overshot(gap, threshold_gap_usd))
        })
}

fn trade_builder_ptb_stop_loss_select_source(
    source_evaluations: &[TradeBuilderPtbStopLossSourceEvaluation],
) -> Option<&TradeBuilderPtbStopLossSourceEvaluation> {
    source_evaluations
        .iter()
        .find(|evaluation| {
            evaluation.config_source == "cex_median_fast"
                && evaluation.current_price.is_some()
                && evaluation.should_trigger
        })
        .or_else(|| {
            source_evaluations.iter().find(|evaluation| {
                evaluation.config_source == "chainlink"
                    && evaluation.current_price.is_some()
                    && evaluation.should_trigger
            })
        })
        .or_else(|| {
            let has_triggering_source = source_evaluations
                .iter()
                .any(|evaluation| evaluation.current_price.is_some() && evaluation.should_trigger);
            source_evaluations
                .iter()
                .filter(|evaluation| evaluation.current_price.is_some())
                .filter(|evaluation| !has_triggering_source || evaluation.should_trigger)
                .min_by(|left, right| {
                    left.directional_gap
                        .unwrap_or(f64::INFINITY)
                        .total_cmp(&right.directional_gap.unwrap_or(f64::INFINITY))
                })
        })
}

fn trade_builder_ptb_stop_loss_final_reason_code(
    current_price_source: PriceToBeatCurrentPriceSource,
    source_evaluations: &[TradeBuilderPtbStopLossSourceEvaluation],
    should_trigger: bool,
) -> &'static str {
    if should_trigger {
        return "ptb_gap_threshold_hit";
    }
    if matches!(
        current_price_source,
        PriceToBeatCurrentPriceSource::ChainlinkCexConsensusConfirmed
            | PriceToBeatCurrentPriceSource::CexMedianFast
    ) {
        let chainlink = source_evaluations
            .iter()
            .find(|evaluation| evaluation.config_source == "chainlink");
        let cex_triggered = source_evaluations.iter().any(|evaluation| {
            evaluation.config_source == "cex_consensus"
                && evaluation.current_price.is_some()
                && evaluation.should_trigger
        });
        if cex_triggered
            && chainlink
                .and_then(|evaluation| evaluation.directional_gap)
                .is_some_and(|gap| gap > 0.0)
        {
            return "cex_trigger_chainlink_conflict";
        }
        if cex_triggered {
            return "cex_trigger_chainlink_unconfirmed";
        }
    }
    "ptb_gap_threshold_not_met"
}

fn confirmed_cex_fallback_overshot(directional_gap: f64, threshold_gap_usd: f64) -> bool {
    if threshold_gap_usd < 0.0 {
        directional_gap <= threshold_gap_usd * 1.25
    } else {
        directional_gap <= threshold_gap_usd
    }
}

fn load_cex_consensus_delta(
    asset: &str,
    venue: crate::trade_flow::guards::cex_microstructure::CexVenue,
    window_start_ms: i64,
    min_move_usd: f64,
    max_book_stale_ms: i64,
) -> TradeBuilderCexConsensusDelta {
    match crate::trade_flow::guards::cex_microstructure::get_cex_venue_delta_snapshot(
        asset,
        venue,
        window_start_ms,
        min_move_usd,
        max_book_stale_ms,
    ) {
        Ok(snapshot) => TradeBuilderCexConsensusDelta {
            snapshot: Some(snapshot),
            error: None,
        },
        Err(err) => TradeBuilderCexConsensusDelta {
            snapshot: None,
            error: Some(err.to_string()),
        },
    }
}

fn cex_consensus_decorated_delta(
    snapshot: &crate::trade_flow::guards::cex_microstructure::CexVenueDeltaSnapshot,
    direction: &str,
    threshold_gap_usd: f64,
    role: &'static str,
) -> crate::trade_flow::guards::cex_microstructure::CexVenueDeltaSnapshot {
    let mut snapshot = snapshot.clone();
    let directional_gap = if direction == "up" {
        snapshot.delta_usd
    } else {
        -snapshot.delta_usd
    };
    snapshot.role = Some(role);
    snapshot.directional_gap = Some(directional_gap);
    snapshot.threshold_hit = Some(directional_gap <= threshold_gap_usd);
    snapshot
}

#[allow(clippy::too_many_arguments)]
fn cex_consensus_metadata(
    market_slug: &str,
    asset: &str,
    timeframe: &str,
    window_start_ms: i64,
    anchor_name: &str,
    anchor_snapshot: Option<&crate::trade_flow::guards::cex_microstructure::CexVenueDeltaSnapshot>,
    binance_snapshot: Option<&crate::trade_flow::guards::cex_microstructure::CexVenueDeltaSnapshot>,
    coinbase_snapshot: Option<
        &crate::trade_flow::guards::cex_microstructure::CexVenueDeltaSnapshot,
    >,
    anchor: &TradeBuilderCexConsensusDelta,
    binance: &TradeBuilderCexConsensusDelta,
    coinbase: &TradeBuilderCexConsensusDelta,
    confirming_venues: &[&str],
) -> Value {
    let mode = format!("{anchor_name}_plus_one");
    let mut venue_deltas = serde_json::Map::new();
    venue_deltas.insert(
        "anchor".to_string(),
        cex_consensus_delta_value(anchor_snapshot, anchor),
    );
    venue_deltas.insert(
        anchor_name.to_string(),
        cex_consensus_delta_value(anchor_snapshot, anchor),
    );
    venue_deltas.insert(
        "binance".to_string(),
        cex_consensus_delta_value(binance_snapshot, binance),
    );
    venue_deltas.insert(
        "coinbase".to_string(),
        cex_consensus_delta_value(coinbase_snapshot, coinbase),
    );
    serde_json::json!({
        "consensus_mode": mode,
        "feed_study_mode": mode,
        "anchor_venue": anchor_name,
        "market_slug": market_slug,
        "asset": asset,
        "timeframe": timeframe,
        "window_start_ms": window_start_ms,
        "confirming_venues": confirming_venues,
        "venue_deltas": venue_deltas,
    })
}

fn cex_consensus_delta_value(
    snapshot: Option<&crate::trade_flow::guards::cex_microstructure::CexVenueDeltaSnapshot>,
    delta: &TradeBuilderCexConsensusDelta,
) -> Value {
    snapshot
        .map(crate::trade_flow::guards::cex_microstructure::CexVenueDeltaSnapshot::to_value)
        .unwrap_or_else(|| serde_json::json!({ "error": delta.error }))
}

fn cex_consensus_error(
    anchor_name: &'static str,
    anchor: &TradeBuilderCexConsensusDelta,
    anchor_hit: bool,
    confirming_venues: &[&str],
) -> (Option<&'static str>, Option<String>) {
    if anchor.snapshot.is_none() {
        let code = match anchor_name {
            "gateio" => "cex_consensus_gateio_unavailable",
            "okx" => "cex_consensus_okx_unavailable",
            _ => "cex_consensus_anchor_unavailable",
        };
        return (Some(code), anchor.error.clone());
    }
    if anchor_hit && confirming_venues.is_empty() {
        return (
            Some("cex_consensus_unconfirmed"),
            Some(format!(
                "{anchor_name} threshold hit but binance/coinbase did not confirm"
            )),
        );
    }
    (None, None)
}

fn cex_consensus_unavailable_source(
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
mod cex_consensus_tests {
    use super::trade_builder_ptb_stop_loss_tests::{
        current_5m_market_slug, lock_ptb_stop_loss_test_state, seed_ptb_stop_loss_current_price,
        test_ptb_stop_loss_order,
    };
    use super::*;
    use crate::trade_flow::guards::cex_microstructure::{
        CexBookSample, CexVenue, clear_cex_microstructure_test_state,
        lock_cex_microstructure_test_state, seed_cex_book_test_sample, seed_cex_open_test_sample,
    };
    use chrono::Utc;

    fn sample(venue: CexVenue, timestamp_ms: i64, mid: f64, source: &'static str) -> CexBookSample {
        sample_for("btc", venue, timestamp_ms, mid, source)
    }

    fn sample_for(
        asset: &str,
        venue: CexVenue,
        timestamp_ms: i64,
        mid: f64,
        source: &'static str,
    ) -> CexBookSample {
        CexBookSample {
            venue,
            asset: asset.to_string(),
            timestamp_ms,
            bid: mid - 0.5,
            ask: mid + 0.5,
            bid_size: Some(1.0),
            ask_size: Some(1.0),
            source,
        }
    }

    fn consensus_order(market_slug: &str) -> TradeBuilderOrder {
        consensus_order_for(market_slug, "Up", -5.0, 100.0)
    }

    fn confirmed_order(market_slug: &str) -> TradeBuilderOrder {
        let mut order = consensus_order(market_slug);
        order.ptb_current_price_source = "chainlink_cex_consensus_confirmed".to_string();
        order
    }

    fn consensus_order_for(
        market_slug: &str,
        outcome_label: &str,
        gap_usd: f64,
        reference_price: f64,
    ) -> TradeBuilderOrder {
        let mut order =
            test_ptb_stop_loss_order(market_slug, outcome_label, gap_usd, Some(reference_price));
        order.ptb_current_price_source = "cex_consensus".to_string();
        order
    }

    fn assert_f64_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 0.000001,
            "expected {expected}, got {actual}"
        );
    }

    #[tokio::test]
    async fn ptb_stop_loss_cex_consensus_ignores_binance_late_ws_only_confirm() {
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("btc", 120.0);
        let (market_slug, start_ms) = current_5m_market_slug("btc");
        let now_ms = Utc::now().timestamp_millis();
        seed_cex_open_test_sample(sample(CexVenue::Okx, start_ms, 100.0, "rest_open"));
        seed_cex_book_test_sample(sample(CexVenue::Okx, now_ms, 90.0, "ticker"));
        seed_cex_book_test_sample(sample(CexVenue::Binance, now_ms, 90.0, "bookTicker"));

        let evaluation =
            trade_builder_evaluate_ptb_stop_loss(&consensus_order(&market_slug)).expect("ptb eval");

        assert_eq!(
            evaluation.current_price_source,
            "cex_consensus_bybit_plus_one"
        );
        assert_eq!(
            evaluation.source_evaluations[1].error_code,
            Some("cex_consensus_unconfirmed")
        );
        assert!(!evaluation.should_trigger);
        let metadata = evaluation.source_evaluations[1]
            .metadata
            .as_ref()
            .expect("cex metadata");
        assert!(
            metadata["venue_deltas"]["binance"]["error"]
                .as_str()
                .unwrap_or_default()
                .contains("window open book missing")
        );
    }

    #[tokio::test]
    async fn ptb_stop_loss_cex_consensus_triggers_with_binance_rest_open_confirm() {
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("btc", 120.0);
        let (market_slug, start_ms) = current_5m_market_slug("btc");
        let now_ms = Utc::now().timestamp_millis();
        seed_cex_open_test_sample(sample(CexVenue::Okx, start_ms, 100.0, "rest_open"));
        seed_cex_book_test_sample(sample(CexVenue::Okx, now_ms, 90.0, "ticker"));
        seed_cex_open_test_sample(sample(CexVenue::Binance, start_ms, 200.0, "rest_open"));
        seed_cex_book_test_sample(sample(CexVenue::Binance, now_ms, 190.0, "bookTicker"));

        let evaluation =
            trade_builder_evaluate_ptb_stop_loss(&consensus_order(&market_slug)).expect("ptb eval");

        assert_eq!(
            evaluation.current_price_source,
            "cex_consensus_bybit_plus_one"
        );
        assert!(evaluation.should_trigger);
        let metadata = evaluation.source_evaluations[1]
            .metadata
            .as_ref()
            .expect("cex metadata");
        assert_eq!(
            metadata["confirming_venues"],
            serde_json::json!(["binance"])
        );
        assert_eq!(
            metadata["venue_deltas"]["binance"]["open_source"],
            serde_json::json!("rest_kline_open")
        );
        assert_eq!(
            metadata["venue_deltas"]["binance"]["open_lag_ms"],
            serde_json::json!(0)
        );
    }

    #[tokio::test]
    async fn sol_ptb_stop_loss_cex_consensus_uses_gateio_anchor() {
        let _cex_guard = lock_cex_microstructure_test_state();
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("sol", 120.0);
        let (market_slug, start_ms) = current_5m_market_slug("sol");
        let now_ms = Utc::now().timestamp_millis();
        seed_cex_open_test_sample(sample_for("sol", CexVenue::Gateio, start_ms, 100.0, "rest_open"));
        seed_cex_book_test_sample(sample_for("sol", CexVenue::Gateio, now_ms, 90.0, "book_ticker"));
        seed_cex_open_test_sample(sample_for("sol", CexVenue::Binance, start_ms, 200.0, "rest_open"));
        seed_cex_book_test_sample(sample_for("sol", CexVenue::Binance, now_ms, 190.0, "bookTicker"));

        let evaluation =
            trade_builder_evaluate_ptb_stop_loss(&consensus_order(&market_slug)).expect("ptb eval");

        assert!(evaluation.should_trigger);
        let metadata = evaluation.source_evaluations[1]
            .metadata
            .as_ref()
            .expect("cex metadata");
        assert_eq!(metadata["anchor_venue"], serde_json::json!("gateio"));
        assert_eq!(
            metadata["venue_deltas"]["gateio"]["threshold_hit"],
            serde_json::json!(true)
        );
        assert_eq!(
            metadata["confirming_venues"],
            serde_json::json!(["binance"])
        );
    }

    #[tokio::test]
    async fn ptb_stop_loss_confirmed_blocks_cex_trigger_when_chainlink_positive() {
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("btc", 120.0);
        let (market_slug, start_ms) = current_5m_market_slug("btc");
        let now_ms = Utc::now().timestamp_millis();
        seed_cex_open_test_sample(sample(CexVenue::Okx, start_ms, 100.0, "rest_open"));
        seed_cex_book_test_sample(sample(CexVenue::Okx, now_ms, 90.0, "ticker"));
        seed_cex_open_test_sample(sample(CexVenue::Binance, start_ms, 200.0, "rest_open"));
        seed_cex_book_test_sample(sample(CexVenue::Binance, now_ms, 190.0, "bookTicker"));

        let evaluation =
            trade_builder_evaluate_ptb_stop_loss(&confirmed_order(&market_slug)).expect("ptb eval");

        assert!(!evaluation.should_trigger);
        assert_eq!(evaluation.reason_code, "cex_trigger_chainlink_conflict");
        assert_eq!(evaluation.current_chainlink_price, Some(120.0));
        assert!(evaluation.source_evaluations[1].should_trigger);
    }

    #[tokio::test]
    async fn ptb_stop_loss_confirmed_allows_chainlink_trigger() {
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("btc", 90.0);
        let (market_slug, start_ms) = current_5m_market_slug("btc");
        let now_ms = Utc::now().timestamp_millis();
        seed_cex_open_test_sample(sample(CexVenue::Okx, start_ms, 100.0, "rest_open"));
        seed_cex_book_test_sample(sample(CexVenue::Okx, now_ms, 101.0, "ticker"));

        let evaluation =
            trade_builder_evaluate_ptb_stop_loss(&confirmed_order(&market_slug)).expect("ptb eval");

        assert!(evaluation.should_trigger);
        assert_eq!(evaluation.reason_code, "ptb_gap_threshold_hit");
        assert_eq!(evaluation.current_price_source, "chainlink_live_data_ws");
    }

    #[tokio::test]
    async fn ptb_stop_loss_confirmed_allows_cex_fallback_when_chainlink_unavailable_and_overshot() {
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        let (market_slug, start_ms) = current_5m_market_slug("xrp");
        let now_ms = Utc::now().timestamp_millis();
        seed_cex_open_test_sample(sample_for(
            "xrp",
            CexVenue::Okx,
            start_ms,
            100.0,
            "rest_open",
        ));
        seed_cex_book_test_sample(sample_for("xrp", CexVenue::Okx, now_ms, 90.0, "ticker"));
        seed_cex_open_test_sample(sample_for(
            "xrp",
            CexVenue::Binance,
            start_ms,
            200.0,
            "rest_open",
        ));
        seed_cex_book_test_sample(sample_for(
            "xrp",
            CexVenue::Binance,
            now_ms,
            190.0,
            "bookTicker",
        ));

        let evaluation =
            trade_builder_evaluate_ptb_stop_loss(&confirmed_order(&market_slug)).expect("ptb eval");

        assert!(evaluation.should_trigger);
        assert_eq!(evaluation.current_chainlink_price, None);
        assert_eq!(
            evaluation.current_price_source,
            "cex_consensus_bybit_plus_one"
        );
    }

    #[tokio::test]
    async fn ptb_stop_loss_cex_consensus_chainlink_triggers_when_binance_unavailable() {
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("btc", 90.0);
        let (market_slug, start_ms) = current_5m_market_slug("btc");
        let now_ms = Utc::now().timestamp_millis();
        seed_cex_open_test_sample(sample(CexVenue::Okx, start_ms, 100.0, "rest_open"));
        seed_cex_book_test_sample(sample(CexVenue::Okx, now_ms, 90.0, "ticker"));
        seed_cex_book_test_sample(sample(CexVenue::Binance, now_ms, 90.0, "bookTicker"));

        let evaluation =
            trade_builder_evaluate_ptb_stop_loss(&consensus_order(&market_slug)).expect("ptb eval");

        assert_eq!(evaluation.current_price_source, "chainlink_live_data_ws");
        assert!(evaluation.should_trigger);
        assert_eq!(
            evaluation.source_evaluations[1].error_code,
            Some("cex_consensus_unconfirmed")
        );
    }

    #[tokio::test]
    async fn ptb_stop_loss_cex_consensus_uses_bybit_rest_open_not_ws_pre_open_for_lead() {
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("btc", 73_790.0);
        let (market_slug, start_ms) = current_5m_market_slug("btc");
        let now_ms = Utc::now().timestamp_millis();
        seed_cex_book_test_sample(sample(CexVenue::Okx, start_ms - 22, 73_891.15, "ticker"));
        seed_cex_open_test_sample(sample(CexVenue::Okx, start_ms, 73_895.20, "rest_open"));
        seed_cex_book_test_sample(sample(CexVenue::Okx, now_ms, 73_892.35, "ticker"));
        seed_cex_open_test_sample(sample(CexVenue::Binance, start_ms, 73_897.98, "rest_open"));
        seed_cex_book_test_sample(sample(CexVenue::Binance, now_ms, 73_898.005, "bookTicker"));
        let order = consensus_order_for(&market_slug, "Down", 1.0, 73_799.02121662606);

        let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

        assert_eq!(
            evaluation.current_price_source,
            "cex_consensus_bybit_plus_one"
        );
        assert!(!evaluation.should_trigger);
        let cex_evaluation = &evaluation.source_evaluations[1];
        assert!(!cex_evaluation.should_trigger);
        assert_eq!(cex_evaluation.error_code, None);
        let metadata = cex_evaluation.metadata.as_ref().expect("cex metadata");
        assert_eq!(
            metadata["confirming_venues"],
            serde_json::json!(["binance"])
        );
        assert_eq!(
            metadata["venue_deltas"]["okx"]["open_source"],
            serde_json::json!("rest_kline_open")
        );
        assert_eq!(
            metadata["venue_deltas"]["okx"]["threshold_hit"],
            serde_json::json!(false)
        );
        assert_f64_close(
            metadata["venue_deltas"]["okx"]["open_mid"]
                .as_f64()
                .expect("okx open mid"),
            73_895.20,
        );
        assert_f64_close(
            metadata["venue_deltas"]["okx"]["delta_usd"]
                .as_f64()
                .expect("okx delta"),
            -2.85,
        );
    }
}
