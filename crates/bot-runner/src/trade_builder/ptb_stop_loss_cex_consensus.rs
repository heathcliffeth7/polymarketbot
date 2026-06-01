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
    let bybit = load_cex_consensus_delta(
        scope.asset,
        crate::trade_flow::guards::cex_microstructure::CexVenue::Bybit,
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

    let bybit_snapshot = bybit
        .snapshot
        .as_ref()
        .map(|snapshot| cex_consensus_decorated_delta(snapshot, direction, threshold_gap_usd, "lead"));
    let binance_snapshot = binance.snapshot.as_ref().map(|snapshot| {
        cex_consensus_decorated_delta(snapshot, direction, threshold_gap_usd, "confirm_candidate")
    });
    let coinbase_snapshot = coinbase.snapshot.as_ref().map(|snapshot| {
        cex_consensus_decorated_delta(snapshot, direction, threshold_gap_usd, "confirm_candidate")
    });

    let bybit_hit = bybit_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.threshold_hit)
        .unwrap_or(false);
    let confirming_venues = [
        ("binance", binance_snapshot.as_ref()),
        ("coinbase", coinbase_snapshot.as_ref()),
    ]
    .into_iter()
    .filter_map(|(venue, snapshot)| {
        snapshot
            .and_then(|snapshot| snapshot.threshold_hit)
            .unwrap_or(false)
            .then_some(venue)
    })
    .collect::<Vec<_>>();
    let should_trigger = bybit_hit && !confirming_venues.is_empty();
    let current_price = bybit_snapshot
        .as_ref()
        .map(|snapshot| ptb_reference_price + snapshot.delta_usd);
    let directional_gap = bybit_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.directional_gap);
    let metadata = cex_consensus_metadata(
        market_slug,
        scope.asset,
        scope.timeframe,
        window_start_ms,
        bybit_snapshot.as_ref(),
        binance_snapshot.as_ref(),
        coinbase_snapshot.as_ref(),
        &bybit,
        &binance,
        &coinbase,
        &confirming_venues,
    );
    let (error_code, error_detail) = cex_consensus_error(&bybit, bybit_hit, &confirming_venues);

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
    bybit_snapshot: Option<&crate::trade_flow::guards::cex_microstructure::CexVenueDeltaSnapshot>,
    binance_snapshot: Option<&crate::trade_flow::guards::cex_microstructure::CexVenueDeltaSnapshot>,
    coinbase_snapshot: Option<&crate::trade_flow::guards::cex_microstructure::CexVenueDeltaSnapshot>,
    bybit: &TradeBuilderCexConsensusDelta,
    binance: &TradeBuilderCexConsensusDelta,
    coinbase: &TradeBuilderCexConsensusDelta,
    confirming_venues: &[&str],
) -> Value {
    serde_json::json!({
        "consensus_mode": "bybit_plus_one",
        "feed_study_mode": "bybit_plus_one",
        "market_slug": market_slug,
        "asset": asset,
        "timeframe": timeframe,
        "window_start_ms": window_start_ms,
        "confirming_venues": confirming_venues,
        "venue_deltas": {
            "bybit": cex_consensus_delta_value(bybit_snapshot, bybit),
            "binance": cex_consensus_delta_value(binance_snapshot, binance),
            "coinbase": cex_consensus_delta_value(coinbase_snapshot, coinbase),
        }
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
    bybit: &TradeBuilderCexConsensusDelta,
    bybit_hit: bool,
    confirming_venues: &[&str],
) -> (Option<&'static str>, Option<String>) {
    if bybit.snapshot.is_none() {
        return (
            Some("cex_consensus_bybit_unavailable"),
            bybit.error.clone(),
        );
    }
    if bybit_hit && confirming_venues.is_empty() {
        return (
            Some("cex_consensus_unconfirmed"),
            Some("bybit threshold hit but binance/coinbase did not confirm".to_string()),
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
    use super::*;
    use super::trade_builder_ptb_stop_loss_tests::{
        current_5m_market_slug, lock_ptb_stop_loss_test_state, seed_ptb_stop_loss_current_price,
        test_ptb_stop_loss_order,
    };
    use crate::trade_flow::guards::cex_microstructure::{
        clear_cex_microstructure_test_state, seed_cex_book_test_sample, seed_cex_open_test_sample,
        CexBookSample, CexVenue,
    };
    use chrono::Utc;

    fn sample(venue: CexVenue, timestamp_ms: i64, mid: f64, source: &'static str) -> CexBookSample {
        CexBookSample {
            venue,
            asset: "btc".to_string(),
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
        seed_cex_open_test_sample(sample(CexVenue::Bybit, start_ms, 100.0, "rest_open"));
        seed_cex_book_test_sample(sample(CexVenue::Bybit, now_ms, 90.0, "ticker"));
        seed_cex_book_test_sample(sample(CexVenue::Binance, now_ms, 90.0, "bookTicker"));

        let evaluation = trade_builder_evaluate_ptb_stop_loss(&consensus_order(&market_slug))
            .expect("ptb eval");

        assert_eq!(evaluation.current_price_source, "cex_consensus_bybit_plus_one");
        assert_eq!(
            evaluation.source_evaluations[1].error_code,
            Some("cex_consensus_unconfirmed")
        );
        assert!(!evaluation.should_trigger);
        let metadata = evaluation.source_evaluations[1]
            .metadata
            .as_ref()
            .expect("cex metadata");
        assert!(metadata["venue_deltas"]["binance"]["error"]
            .as_str()
            .unwrap_or_default()
            .contains("window open book missing"));
    }

    #[tokio::test]
    async fn ptb_stop_loss_cex_consensus_triggers_with_binance_rest_open_confirm() {
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("btc", 120.0);
        let (market_slug, start_ms) = current_5m_market_slug("btc");
        let now_ms = Utc::now().timestamp_millis();
        seed_cex_open_test_sample(sample(CexVenue::Bybit, start_ms, 100.0, "rest_open"));
        seed_cex_book_test_sample(sample(CexVenue::Bybit, now_ms, 90.0, "ticker"));
        seed_cex_open_test_sample(sample(CexVenue::Binance, start_ms, 200.0, "rest_open"));
        seed_cex_book_test_sample(sample(CexVenue::Binance, now_ms, 190.0, "bookTicker"));

        let evaluation = trade_builder_evaluate_ptb_stop_loss(&consensus_order(&market_slug))
            .expect("ptb eval");

        assert_eq!(evaluation.current_price_source, "cex_consensus_bybit_plus_one");
        assert!(evaluation.should_trigger);
        let metadata = evaluation.source_evaluations[1]
            .metadata
            .as_ref()
            .expect("cex metadata");
        assert_eq!(metadata["confirming_venues"], serde_json::json!(["binance"]));
        assert_eq!(
            metadata["venue_deltas"]["binance"]["open_source"],
            serde_json::json!("rest_kline_open")
        );
        assert_eq!(metadata["venue_deltas"]["binance"]["open_lag_ms"], serde_json::json!(0));
    }

    #[tokio::test]
    async fn ptb_stop_loss_cex_consensus_chainlink_triggers_when_binance_unavailable() {
        let _guard = lock_ptb_stop_loss_test_state();
        clear_cex_microstructure_test_state();
        seed_ptb_stop_loss_current_price("btc", 90.0);
        let (market_slug, start_ms) = current_5m_market_slug("btc");
        let now_ms = Utc::now().timestamp_millis();
        seed_cex_open_test_sample(sample(CexVenue::Bybit, start_ms, 100.0, "rest_open"));
        seed_cex_book_test_sample(sample(CexVenue::Bybit, now_ms, 90.0, "ticker"));
        seed_cex_book_test_sample(sample(CexVenue::Binance, now_ms, 90.0, "bookTicker"));

        let evaluation = trade_builder_evaluate_ptb_stop_loss(&consensus_order(&market_slug))
            .expect("ptb eval");

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
        seed_cex_book_test_sample(sample(CexVenue::Bybit, start_ms - 22, 73_891.15, "ticker"));
        seed_cex_open_test_sample(sample(CexVenue::Bybit, start_ms, 73_895.20, "rest_open"));
        seed_cex_book_test_sample(sample(CexVenue::Bybit, now_ms, 73_892.35, "ticker"));
        seed_cex_open_test_sample(sample(CexVenue::Binance, start_ms, 73_897.98, "rest_open"));
        seed_cex_book_test_sample(sample(CexVenue::Binance, now_ms, 73_898.005, "bookTicker"));
        let order = consensus_order_for(&market_slug, "Down", 1.0, 73_799.02121662606);

        let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

        assert_eq!(evaluation.current_price_source, "cex_consensus_bybit_plus_one");
        assert!(!evaluation.should_trigger);
        let cex_evaluation = &evaluation.source_evaluations[1];
        assert!(!cex_evaluation.should_trigger);
        assert_eq!(cex_evaluation.error_code, None);
        let metadata = cex_evaluation.metadata.as_ref().expect("cex metadata");
        assert_eq!(metadata["confirming_venues"], serde_json::json!(["binance"]));
        assert_eq!(
            metadata["venue_deltas"]["bybit"]["open_source"],
            serde_json::json!("rest_kline_open")
        );
        assert_eq!(
            metadata["venue_deltas"]["bybit"]["threshold_hit"],
            serde_json::json!(false)
        );
        assert_f64_close(
            metadata["venue_deltas"]["bybit"]["open_mid"]
                .as_f64()
                .expect("bybit open mid"),
            73_895.20,
        );
        assert_f64_close(
            metadata["venue_deltas"]["bybit"]["delta_usd"]
                .as_f64()
                .expect("bybit delta"),
            -2.85,
        );
    }
}
