use super::*;

fn book(venue: CexVenue, ts: i64, bid: f64, ask: f64) -> CexBookSample {
    book_with_source(venue, ts, bid, ask, "ticker")
}

fn book_with_source(
    venue: CexVenue,
    ts: i64,
    bid: f64,
    ask: f64,
    source: &'static str,
) -> CexBookSample {
    CexBookSample {
        venue,
        asset: "btc".to_string(),
        timestamp_ms: ts,
        bid,
        ask,
        bid_size: Some(1.0),
        ask_size: Some(1.0),
        source,
    }
}

fn trade(venue: CexVenue, ts: i64, price: f64, side: TakerSide) -> CexTradeSample {
    CexTradeSample {
        venue,
        asset: "btc".to_string(),
        timestamp_ms: ts,
        price,
        size: 1.0,
        taker_side: side,
    }
}

#[test]
fn non_improving_level2_partial_is_rejected() {
    let previous = book_with_source(CexVenue::Coinbase, 1_000, 66.89, 66.90, "level2");
    // Derin seviyedeki bid update'i best'i bozmamali
    let deep_bid = CexBookSample {
        bid: 58.46,
        ask: 0.0,
        bid_size: Some(2.0),
        ask_size: None,
        ..book_with_source(CexVenue::Coinbase, 2_000, 58.46, 0.0, "level2")
    };
    assert!(non_improving_partial_level2_update(
        Some(&previous),
        &deep_bid
    ));
    // Best'i iyilestiren bid kabul edilmeli
    let better_bid = CexBookSample {
        bid_size: Some(2.0),
        ask_size: None,
        ..book_with_source(CexVenue::Coinbase, 2_000, 66.895, 0.0, "level2")
    };
    assert!(!non_improving_partial_level2_update(
        Some(&previous),
        &better_bid
    ));
    // qty=0 silme update'i reddedilmeli
    let removal = CexBookSample {
        bid_size: Some(0.0),
        ask_size: None,
        ..book_with_source(CexVenue::Coinbase, 2_000, 66.90, 0.0, "level2")
    };
    assert!(non_improving_partial_level2_update(
        Some(&previous),
        &removal
    ));
    // Best'i iyilestiren ask kabul edilmeli
    let better_ask = CexBookSample {
        bid_size: None,
        ask_size: Some(2.0),
        ..book_with_source(CexVenue::Coinbase, 2_000, 0.0, 66.895, "level2")
    };
    assert!(!non_improving_partial_level2_update(
        Some(&previous),
        &better_ask
    ));
}

#[test]
fn cex_snapshot_requires_matching_consensus_side() {
    let _guard = lock_cex_microstructure_test_state();
    clear_cex_microstructure_test_state();
    seed_cex_book_test_sample(book(CexVenue::Binance, 20_000, 67_519.0, 67_521.0));
    seed_cex_book_test_sample(book(CexVenue::Coinbase, 20_000, 67_518.0, 67_522.0));
    for venue in [CexVenue::Binance, CexVenue::Coinbase] {
        seed_cex_trade_test_sample(trade(venue, 6_000, 67_500.0, TakerSide::Buy));
        seed_cex_trade_test_sample(trade(venue, 20_000, 67_520.0, TakerSide::Buy));
    }

    let snapshot = SERVICE
        .snapshot("btc", 20_100, &CexMicrostructureSnapshotConfig::default())
        .expect("snapshot");

    assert_eq!(snapshot.consensus_side, Some("up"));
    assert!(snapshot.normalized_source_skew_usd.abs() <= 0.001);
}

#[test]
fn current_price_snapshot_allows_fresh_book_without_ticker() {
    let _guard = lock_cex_microstructure_test_state();
    clear_cex_microstructure_test_state();
    seed_cex_book_test_sample(book_with_source(
        CexVenue::Binance,
        20_000,
        67_519.0,
        67_521.0,
        "depth5",
    ));

    let snapshot = SERVICE
        .current_price_snapshot(
            "btc",
            CexVenue::Binance,
            20_100,
            &CexMicrostructureSnapshotConfig::default(),
        )
        .expect("current price snapshot");

    assert_eq!(snapshot.mid, 67_520.0);
    assert_eq!(snapshot.book_staleness_ms, 100);
    assert_eq!(snapshot.ticker_staleness_ms, 100);
}

#[test]
fn current_price_snapshot_allows_fresh_book_with_stale_ticker_debug() {
    let _guard = lock_cex_microstructure_test_state();
    clear_cex_microstructure_test_state();
    seed_cex_book_test_sample(book(CexVenue::Binance, 10_000, 67_500.0, 67_502.0));
    seed_cex_book_test_sample(book_with_source(
        CexVenue::Binance,
        20_000,
        67_519.0,
        67_521.0,
        "depth5",
    ));

    let snapshot = SERVICE
        .current_price_snapshot(
            "btc",
            CexVenue::Binance,
            20_100,
            &CexMicrostructureSnapshotConfig::default(),
        )
        .expect("current price snapshot");

    assert_eq!(snapshot.mid, 67_520.0);
    assert_eq!(snapshot.book_staleness_ms, 100);
    assert_eq!(snapshot.ticker_staleness_ms, 10_100);
}

#[test]
fn source_snapshot_still_requires_ticker_for_full_consensus() {
    let state = VenueState {
        latest_book: Some(book_with_source(
            CexVenue::Binance,
            20_000,
            67_519.0,
            67_521.0,
            "depth5",
        )),
        trades: std::collections::VecDeque::from([trade(
            CexVenue::Binance,
            20_000,
            67_520.0,
            TakerSide::Buy,
        )]),
        ..VenueState::default()
    };

    let error = source_snapshot(
        CexVenue::Binance,
        &state,
        20_100,
        &CexMicrostructureSnapshotConfig::default(),
        "binance",
    )
    .expect_err("full source snapshot must still require ticker");

    assert!(error.to_string().contains("binance ticker missing"));
}

#[test]
fn unseeded_partial_level2_update_does_not_create_book() {
    let _guard = lock_cex_microstructure_test_state();
    clear_cex_microstructure_test_state();
    seed_cex_book_test_sample(CexBookSample {
        venue: CexVenue::Coinbase,
        asset: "btc".to_string(),
        timestamp_ms: 20_000,
        bid: 67_519.0,
        ask: 67_519.0,
        bid_size: Some(1.0),
        ask_size: None,
        source: "level2",
    });

    let error = SERVICE
        .current_price_snapshot(
            "btc",
            CexVenue::Coinbase,
            20_100,
            &CexMicrostructureSnapshotConfig::default(),
        )
        .expect_err("partial l2 update must not seed a current book");

    assert!(error.to_string().contains("coinbase book missing"));
}

#[test]
fn coinbase_error_summary_detects_top_level_and_event_errors() {
    let top_level = serde_json::json!({
        "type": "error",
        "channel": "subscriptions",
        "message": "bad subscription"
    });
    let event_level = serde_json::json!({
        "channel": "subscriptions",
        "events": [{
            "type": "error",
            "message": "unknown product"
        }]
    });

    assert!(coinbase_error_summary(&top_level)
        .expect("top level error")
        .contains("bad subscription"));
    assert!(coinbase_error_summary(&event_level)
        .expect("event error")
        .contains("unknown product"));
}

#[tokio::test]
async fn schedule_window_open_backfill_skips_when_rest_open_already_pinned() {
    let _guard = lock_cex_microstructure_test_state();
    clear_cex_microstructure_test_state();
    let window_start_ms = 1780291200000_i64;
    seed_cex_open_test_sample(CexBookSample {
        venue: CexVenue::Bybit,
        asset: "btc".to_string(),
        timestamp_ms: window_start_ms,
        bid: 73_460.4,
        ask: 73_460.4,
        bid_size: None,
        ask_size: None,
        source: "rest_open",
    });
    seed_cex_book_test_sample(book(
        CexVenue::Bybit,
        window_start_ms + 30_000,
        73_500.0,
        73_502.0,
    ));

    SERVICE.schedule_window_open_backfill("btc".to_string(), CexVenue::Bybit, window_start_ms);
    tokio::time::sleep(Duration::from_millis(50)).await;

    let key = format!("btc:bybit:{window_start_ms}");
    assert!(!SERVICE.open_backfill_inflight.lock().contains(&key));
}

#[test]
fn venue_delta_snapshot_rejects_bybit_pre_window_ws_book_without_rest_open() {
    let _guard = lock_cex_microstructure_test_state();
    clear_cex_microstructure_test_state();
    seed_cex_book_test_sample(book(CexVenue::Bybit, 10_000, 99.0, 101.0));
    seed_cex_book_test_sample(book(CexVenue::Bybit, 20_000, 109.0, 111.0));

    let error = SERVICE
        .venue_delta_snapshot("btc", CexVenue::Bybit, 10_000, 20_100, 1.0, 500)
        .expect_err("bybit ws book must not become window open");

    assert!(error.to_string().contains("window open book missing"));
}

#[test]
fn venue_delta_snapshot_rejects_late_first_book_as_window_open() {
    let _guard = lock_cex_microstructure_test_state();
    clear_cex_microstructure_test_state();
    seed_cex_book_test_sample(book(CexVenue::Bybit, 190_000, 99.0, 101.0));
    seed_cex_book_test_sample(book(CexVenue::Bybit, 200_000, 89.0, 91.0));

    let error = SERVICE
        .venue_delta_snapshot("btc", CexVenue::Bybit, 0, 200_100, 1.0, 500)
        .expect_err("late service-start book must not become window open");

    assert!(error.to_string().contains("window open book missing"));
}

#[test]
fn venue_delta_snapshot_rejects_binance_late_book_as_window_open() {
    let _guard = lock_cex_microstructure_test_state();
    clear_cex_microstructure_test_state();
    seed_cex_book_test_sample(book(CexVenue::Binance, 41_201, 73_838.97, 73_839.02));

    let error = SERVICE
        .venue_delta_snapshot("btc", CexVenue::Binance, 0, 41_301, 1.0, 500)
        .expect_err("binance late book must not become window open");

    assert!(error.to_string().contains("window open book missing"));
}

#[test]
fn venue_delta_snapshot_rejects_binance_pre_window_ws_book_without_rest_open() {
    let _guard = lock_cex_microstructure_test_state();
    clear_cex_microstructure_test_state();
    seed_cex_book_test_sample(book(CexVenue::Binance, 9_800, 73_821.98, 73_822.00));
    seed_cex_book_test_sample(book(CexVenue::Binance, 20_000, 73_838.97, 73_839.02));

    let error = SERVICE
        .venue_delta_snapshot("btc", CexVenue::Binance, 10_000, 20_100, 1.0, 500)
        .expect_err("binance ws book must not become window open");

    assert!(error.to_string().contains("window open book missing"));
}

#[test]
fn venue_delta_snapshot_prefers_pinned_rest_open() {
    let _guard = lock_cex_microstructure_test_state();
    clear_cex_microstructure_test_state();
    seed_cex_open_test_sample(CexBookSample {
        venue: CexVenue::Bybit,
        asset: "btc".to_string(),
        timestamp_ms: 0,
        bid: 100.0,
        ask: 100.0,
        bid_size: None,
        ask_size: None,
        source: "rest_open",
    });
    seed_cex_book_test_sample(book(CexVenue::Bybit, 190_000, 89.0, 91.0));

    let snapshot = SERVICE
        .venue_delta_snapshot("btc", CexVenue::Bybit, 0, 190_100, 1.0, 500)
        .expect("delta snapshot");

    assert_eq!(snapshot.open_mid, 100.0);
    assert_eq!(snapshot.current_mid, 90.0);
    assert_eq!(snapshot.delta_usd, -10.0);
    assert_eq!(snapshot.open_source, "rest_kline_open");
}

#[test]
fn venue_delta_snapshot_uses_coinbase_pinned_rest_open() {
    let _guard = lock_cex_microstructure_test_state();
    clear_cex_microstructure_test_state();
    seed_cex_open_test_sample(CexBookSample {
        venue: CexVenue::Coinbase,
        asset: "btc".to_string(),
        timestamp_ms: 0,
        bid: 100.0,
        ask: 100.0,
        bid_size: None,
        ask_size: None,
        source: "rest_open",
    });
    seed_cex_book_test_sample(book(CexVenue::Coinbase, 190_000, 110.0, 112.0));

    let snapshot = SERVICE
        .venue_delta_snapshot("btc", CexVenue::Coinbase, 0, 190_100, 1.0, 500)
        .expect("delta snapshot");

    assert_eq!(snapshot.open_mid, 100.0);
    assert_eq!(snapshot.current_mid, 111.0);
    assert_eq!(snapshot.delta_usd, 11.0);
    assert_eq!(snapshot.open_lag_ms, 0);
    assert_eq!(snapshot.open_source, "rest_kline_open");
}

#[test]
fn venue_delta_snapshot_rejects_coinbase_pre_window_ws_book_without_rest_open() {
    let _guard = lock_cex_microstructure_test_state();
    clear_cex_microstructure_test_state();
    seed_cex_book_test_sample(book(CexVenue::Coinbase, 9_800, 99.0, 101.0));
    seed_cex_book_test_sample(book(CexVenue::Coinbase, 20_000, 109.0, 111.0));

    let error = SERVICE
        .venue_delta_snapshot("btc", CexVenue::Coinbase, 10_000, 20_100, 1.0, 500)
        .expect_err("coinbase ws book must not become window open");

    assert!(error.to_string().contains("window open book missing"));
}

#[test]
fn venue_delta_snapshot_uses_binance_pinned_rest_open() {
    let _guard = lock_cex_microstructure_test_state();
    clear_cex_microstructure_test_state();
    seed_cex_open_test_sample(CexBookSample {
        venue: CexVenue::Binance,
        asset: "btc".to_string(),
        timestamp_ms: 0,
        bid: 73_821.99,
        ask: 73_821.99,
        bid_size: None,
        ask_size: None,
        source: "rest_open",
    });
    seed_cex_book_test_sample(book(CexVenue::Binance, 41_201, 73_838.97, 73_839.02));

    let snapshot = SERVICE
        .venue_delta_snapshot("btc", CexVenue::Binance, 0, 41_301, 1.0, 500)
        .expect("delta snapshot");

    assert_eq!(snapshot.open_mid, 73_821.99);
    assert_eq!(snapshot.current_mid, 73_838.995);
    assert!((snapshot.delta_usd - 17.005).abs() < 0.000_000_1);
    assert_eq!(snapshot.open_lag_ms, 0);
    assert_eq!(snapshot.open_source, "rest_kline_open");
}
