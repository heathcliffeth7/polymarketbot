use super::*;

fn minimal_html(next_data_json: &str) -> String {
    format!(
        r#"<html><head></head><body><script id="__NEXT_DATA__" type="application/json" crossorigin="anonymous">{next_data_json}</script></body></html>"#
    )
}

fn test_snapshot(source: PriceToBeatSource) -> PolymarketPriceToBeatSnapshot {
    PolymarketPriceToBeatSnapshot {
        event_url: "https://polymarket.com/event/btc-updown-5m-1773232500".to_string(),
        asset: "btc".to_string(),
        timeframe: "5m".to_string(),
        price_to_beat: 69_279.93484689768,
        source,
        verified: source == PriceToBeatSource::Polymarket,
        source_latency_ms: Some(125),
        fetched_at: Utc::now(),
    }
}

#[test]
fn build_query_spec_for_five_minute_market() {
    let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");
    assert_eq!(spec.asset, "BTC");
    assert_eq!(spec.timeframe, "5m");
    assert_eq!(spec.query_timeframe, "fiveminute");
    assert_eq!(
        spec.start_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        "2026-03-11T12:35:00Z"
    );
    assert_eq!(
        spec.end_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        "2026-03-11T12:40:00Z"
    );
}

#[test]
fn build_query_spec_for_fifteen_minute_market() {
    let spec = build_price_to_beat_query_spec("btc-updown-15m-1773232200").expect("spec");
    assert_eq!(spec.asset, "BTC");
    assert_eq!(spec.timeframe, "15m");
    assert_eq!(spec.query_timeframe, "fifteen");
    assert_eq!(
        spec.start_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        "2026-03-11T12:30:00Z"
    );
    assert_eq!(
        spec.end_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        "2026-03-11T12:45:00Z"
    );
}

#[test]
fn parses_open_price_for_five_minute_market() {
    let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");
    let html = minimal_html(
        r#"{"props":{"pageProps":{"dehydratedState":{"queries":[{"queryKey":["crypto-prices","price","BTC","2026-03-11T12:35:00Z","fiveminute","2026-03-11T12:40:00Z"],"state":{"data":{"openPrice":69279.93484689768,"closePrice":null}}}]}}}}"#,
    );

    let open_price = parse_open_price_from_html(&html, &spec).expect("price");
    assert_eq!(open_price, 69_279.93484689768);
}

#[test]
fn parses_open_price_for_fifteen_minute_market() {
    let spec = build_price_to_beat_query_spec("btc-updown-15m-1773232200").expect("spec");
    let html = minimal_html(
        r#"{"props":{"pageProps":{"dehydratedState":{"queries":[{"queryKey":["crypto-prices","price","BTC","2026-03-11T12:30:00Z","fifteen","2026-03-11T12:45:00Z"],"state":{"data":{"openPrice":69421.75585678649,"closePrice":null}}}]}}}}"#,
    );

    let open_price = parse_open_price_from_html(&html, &spec).expect("price");
    assert_eq!(open_price, 69_421.75585678649);
}

#[test]
fn query_not_found_error_prefix_matches_actual_error() {
    let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");
    let html = minimal_html(r#"{"props":{"pageProps":{"dehydratedState":{"queries":[]}}}}"#);
    let err = parse_open_price_from_html(&html, &spec).unwrap_err();
    assert!(
        err.to_string().starts_with(QUERY_NOT_FOUND_ERROR_PREFIX),
        "error message does not start with expected prefix: {err}",
    );
}

#[test]
fn build_event_request_url_leaves_base_url_unchanged_without_cache_buster() {
    assert_eq!(
        build_event_request_url("https://polymarket.com/event/foo", None),
        "https://polymarket.com/event/foo"
    );
}

#[test]
fn build_event_request_url_appends_cache_buster_query() {
    assert_eq!(
        build_event_request_url("https://polymarket.com/event/foo", Some(123)),
        "https://polymarket.com/event/foo?ptb_ts=123"
    );
}

#[test]
fn build_event_request_url_respects_existing_query_string() {
    assert_eq!(
        build_event_request_url("https://polymarket.com/event/foo?lang=en", Some(123)),
        "https://polymarket.com/event/foo?lang=en&ptb_ts=123"
    );
}

#[test]
fn build_crypto_price_api_url_for_five_minute_market() {
    let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");

    let url = build_crypto_price_api_url(&spec).expect("url");

    assert_eq!(
        url,
        "https://polymarket.com/api/crypto/crypto-price?symbol=BTC&eventStartTime=2026-03-11T12%3A35%3A00Z&variant=fiveminute&endDate=2026-03-11T12%3A40%3A00Z"
    );
}

#[test]
fn build_crypto_price_api_url_for_fifteen_minute_market() {
    let spec = build_price_to_beat_query_spec("btc-updown-15m-1773232200").expect("spec");

    let url = build_crypto_price_api_url(&spec).expect("url");

    assert_eq!(
        url,
        "https://polymarket.com/api/crypto/crypto-price?symbol=BTC&eventStartTime=2026-03-11T12%3A30%3A00Z&variant=fifteen&endDate=2026-03-11T12%3A45%3A00Z"
    );
}

#[test]
fn builds_previous_query_spec_for_five_minute_market() {
    let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");

    let previous = build_previous_price_to_beat_query_spec(&spec).expect("previous spec");

    assert_eq!(previous.market_slug, "btc-updown-5m-1773232200");
    assert_eq!(
        previous.start_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        "2026-03-11T12:30:00Z"
    );
    assert_eq!(
        previous.end_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        "2026-03-11T12:35:00Z"
    );
}

#[test]
fn parses_open_price_from_crypto_price_api_response() {
    let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");
    let response_body = r#"{"openPrice":69279.93484689768,"closePrice":null,"completed":false,"incomplete":true,"cached":true}"#;

    let response = parse_crypto_price_api_response(response_body, &spec).expect("response");

    assert_eq!(response.sanitized_open_price(), Some(69_279.93484689768));
    assert_eq!(response.verified_close_price(), None);
}

#[test]
fn parses_null_open_price_from_crypto_price_api_response_as_pending() {
    let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");
    let response_body = r#"{"openPrice":null,"closePrice":null,"completed":false,"incomplete":true,"cached":false}"#;

    let response = parse_crypto_price_api_response(response_body, &spec).expect("pending");

    assert_eq!(response.sanitized_open_price(), None);
    assert_eq!(response.verified_close_price(), None);
}

#[test]
fn parses_verified_close_price_from_crypto_price_api_response() {
    let spec = build_price_to_beat_query_spec("btc-updown-5m-1773232500").expect("spec");
    let response_body = r#"{"openPrice":69279.93484689768,"closePrice":69282.125,"completed":true,"incomplete":false,"cached":true}"#;

    let response = parse_crypto_price_api_response(response_body, &spec).expect("response");

    assert_eq!(response.sanitized_open_price(), Some(69_279.93484689768));
    assert_eq!(response.verified_close_price(), Some(69_282.125));
}

#[test]
fn detects_query_pending_error_prefixes() {
    let err =
        anyhow!("price to beat query not found in polymarket page for btc-updown-5m-1773232500");
    assert!(is_price_to_beat_query_pending(&err));

    let retry_err = anyhow!(
        "price to beat query not found after {} retries for {}",
        QUERY_NOT_FOUND_RETRY_ATTEMPTS,
        "btc-updown-5m-1773232500"
    );
    assert!(is_price_to_beat_query_pending(&retry_err));

    let verification_err =
        anyhow!("price to beat awaiting previous window close for btc-updown-5m-1773232500");
    assert!(is_price_to_beat_query_pending(&verification_err));

    let rate_limited_err =
        anyhow!("price to beat rate limited for 30000ms on btc-updown-5m-1773232500");
    assert!(is_price_to_beat_query_pending(&rate_limited_err));

    let http_err = anyhow!("polymarket crypto-price api returned status 503: service unavailable");
    assert!(!is_price_to_beat_query_pending(&http_err));

    let other_err = anyhow!("__NEXT_DATA__ script tag not found in html");
    assert!(!is_price_to_beat_query_pending(&other_err));
}

#[test]
fn parses_retry_after_seconds_with_max_clamp() {
    assert_eq!(parse_retry_after_ms("30"), Some(30_000));
    assert_eq!(parse_retry_after_ms("0"), None);
    assert_eq!(parse_retry_after_ms("not-seconds"), None);
    assert_eq!(
        parse_retry_after_ms("999")
            .unwrap()
            .min(PTB_RATE_LIMIT_MAX_RETRY_AFTER_MS),
        PTB_RATE_LIMIT_MAX_RETRY_AFTER_MS
    );
}

#[test]
fn background_retry_backoff_is_exponential_and_capped() {
    assert_eq!(backoff_delay_ms(0), 2_000);
    assert_eq!(backoff_delay_ms(1), 4_000);
    assert_eq!(backoff_delay_ms(8), BG_FETCH_RETRY_MAX_DELAY_MS);
}

#[test]
fn seed_snapshot_inserts_chainlink_rtds_start_tick_when_cache_is_empty() {
    let service = PolymarketPriceToBeatService::new();

    let seeded =
        service.seed_snapshot("btc-updown-5m-1773232500", "btc", "5m", 69_200.0, Some(450));

    assert!(seeded);
    let snapshot = service
        .current_snapshot("btc-updown-5m-1773232500")
        .expect("snapshot");
    assert_eq!(snapshot.source, PriceToBeatSource::ChainlinkRtdsStartTick);
    assert!(snapshot.verified);
    assert_eq!(snapshot.price_to_beat, 69_200.0);
    assert_eq!(snapshot.source_latency_ms, Some(450));
}

#[test]
fn seed_snapshot_with_previous_close_source_is_verified_rtds_previous_close() {
    let service = PolymarketPriceToBeatService::new();

    let seeded = service.seed_snapshot_with_source(
        "btc-updown-5m-1773232500",
        "btc",
        "5m",
        69_282.125,
        PriceToBeatSource::ChainlinkRtdsPreviousClose,
        Some(0),
    );

    assert!(seeded);
    let snapshot = service
        .current_snapshot("btc-updown-5m-1773232500")
        .expect("snapshot");
    assert_eq!(snapshot.source, PriceToBeatSource::ChainlinkRtdsPreviousClose);
    assert_eq!(snapshot.status(), "rtds_previous_close");
    assert!(snapshot.verified);
    assert_eq!(snapshot.price_to_beat, 69_282.125);
    assert!(!snapshot.should_verify_with_http());
}

#[test]
fn seed_snapshot_does_not_overwrite_polymarket_value() {
    let service = PolymarketPriceToBeatService::new();
    service.cache.lock().insert(
        "btc-updown-5m-1773232500".to_string(),
        test_snapshot(PriceToBeatSource::Polymarket),
    );

    let seeded =
        service.seed_snapshot("btc-updown-5m-1773232500", "btc", "5m", 69_100.0, Some(900));

    assert!(!seeded);
    let snapshot = service
        .current_snapshot("btc-updown-5m-1773232500")
        .expect("snapshot");
    assert_eq!(snapshot.source, PriceToBeatSource::Polymarket);
    assert!(snapshot.verified);
    assert_eq!(snapshot.price_to_beat, 69_279.93484689768);
}

#[test]
fn seed_snapshot_does_not_overwrite_provisional_polymarket_value() {
    let service = PolymarketPriceToBeatService::new();
    service.cache.lock().insert(
        "btc-updown-5m-1773232500".to_string(),
        PolymarketPriceToBeatSnapshot {
            event_url: "https://polymarket.com/event/btc-updown-5m-1773232500".to_string(),
            asset: "btc".to_string(),
            timeframe: "5m".to_string(),
            price_to_beat: 69_100.0,
            source: PriceToBeatSource::Polymarket,
            verified: false,
            source_latency_ms: None,
            fetched_at: Utc::now(),
        },
    );

    let seeded =
        service.seed_snapshot("btc-updown-5m-1773232500", "btc", "5m", 69_200.0, Some(450));

    assert!(!seeded);
    let snapshot = service
        .current_snapshot("btc-updown-5m-1773232500")
        .expect("snapshot");
    assert_eq!(snapshot.source, PriceToBeatSource::Polymarket);
    assert!(!snapshot.verified);
    assert_eq!(snapshot.price_to_beat, 69_100.0);
    assert_eq!(snapshot.source_latency_ms, None);
}

#[tokio::test]
async fn fetch_snapshot_returns_seeded_cache_without_network_lookup() {
    let service = PolymarketPriceToBeatService::new();
    service.cache.lock().insert(
        "btc-updown-5m-1773232500".to_string(),
        test_snapshot(PriceToBeatSource::ChainlinkRtdsStartTick),
    );

    let snapshot = service
        .fetch_snapshot("btc-updown-5m-1773232500")
        .await
        .expect("seeded snapshot");

    assert_eq!(snapshot.source, PriceToBeatSource::ChainlinkRtdsStartTick);
    assert_eq!(snapshot.price_to_beat, 69_279.93484689768);
}

#[tokio::test]
async fn fetch_snapshot_once_uses_cached_previous_close_as_verified_snapshot() {
    let service = PolymarketPriceToBeatService::new();
    service.store_previous_close("btc-updown-5m-1773232200", 69_282.125);

    let snapshot = service
        .fetch_snapshot_once("btc-updown-5m-1773232500", None, true)
        .await
        .expect("snapshot");

    assert_eq!(snapshot.source, PriceToBeatSource::Polymarket);
    assert!(snapshot.verified);
    assert_eq!(snapshot.price_to_beat, 69_282.125);
    assert!(service
        .current_snapshot("btc-updown-5m-1773232500")
        .expect("cached current snapshot")
        .is_verified_polymarket());
}

#[test]
fn window_fetch_guard_dedups_until_drop() {
    let service = PolymarketPriceToBeatService::new();
    let request_url =
        "https://polymarket.com/api/crypto/crypto-price?symbol=BTC&eventStartTime=x".to_string();

    let first = WindowFetchGuard::acquire(&service, request_url.clone()).expect("first guard");

    assert!(WindowFetchGuard::acquire(&service, request_url.clone()).is_none());

    drop(first);

    assert!(WindowFetchGuard::acquire(&service, request_url).is_some());
}

#[tokio::test]
async fn fetch_snapshot_once_verify_only_pending_skips_provisional_fallback() {
    let service = PolymarketPriceToBeatService::new();
    let current_slug = "btc-updown-5m-1773232500";
    let spec = build_price_to_beat_query_spec(current_slug).expect("spec");
    let previous_spec = build_previous_price_to_beat_query_spec(&spec).expect("previous spec");
    let previous_url = build_crypto_price_api_url(&previous_spec).expect("previous url");
    let _previous_guard =
        WindowFetchGuard::acquire(&service, previous_url).expect("previous window guard");

    let err = service
        .fetch_snapshot_once(current_slug, None, true)
        .await
        .expect_err("pending");

    assert!(
        err.to_string()
            .starts_with(VERIFICATION_PENDING_ERROR_PREFIX),
        "unexpected error: {err}"
    );
    assert!(service.current_snapshot(current_slug).is_none());
}

#[test]
fn clear_price_to_beat_test_state_resets_round2_state() {
    let service = &POLYMARKET_PRICE_TO_BEAT_SERVICE;
    service
        .window_fetch_inflight
        .lock()
        .insert("https://polymarket.com/api/crypto/crypto-price?symbol=BTC".to_string());
    service
        .next_request_at_ms
        .store(123_456, std::sync::atomic::Ordering::Relaxed);

    clear_price_to_beat_test_state();

    assert!(service.window_fetch_inflight.lock().is_empty());
    assert_eq!(
        service
            .next_request_at_ms
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[tokio::test]
async fn try_cached_or_spawn_returns_ready_for_seeded_snapshot() {
    let service = Box::leak(Box::new(PolymarketPriceToBeatService::new()));
    service.cache.lock().insert(
        "btc-updown-5m-1773232500".to_string(),
        test_snapshot(PriceToBeatSource::ChainlinkRtdsStartTick),
    );

    let lookup = service.try_cached_or_spawn("btc-updown-5m-1773232500");

    match lookup {
        PriceToBeatLookup::Ready(snapshot) => {
            assert_eq!(snapshot.source, PriceToBeatSource::ChainlinkRtdsStartTick);
        }
        other => panic!("expected ready lookup, got {other:?}"),
    }
}

#[tokio::test]
async fn try_cached_or_spawn_returns_ready_for_provisional_snapshot() {
    let service = Box::leak(Box::new(PolymarketPriceToBeatService::new()));
    service.cache.lock().insert(
        "btc-updown-5m-1773232500".to_string(),
        PolymarketPriceToBeatSnapshot {
            event_url: "https://polymarket.com/event/btc-updown-5m-1773232500".to_string(),
            asset: "btc".to_string(),
            timeframe: "5m".to_string(),
            price_to_beat: 69_300.0,
            source: PriceToBeatSource::Polymarket,
            verified: false,
            source_latency_ms: None,
            fetched_at: Utc::now(),
        },
    );

    let lookup = service.try_cached_or_spawn("btc-updown-5m-1773232500");

    assert!(matches!(lookup, PriceToBeatLookup::Ready(_)));
}

#[tokio::test]
async fn try_cached_or_spawn_with_returns_ready_when_chainlink_seed_is_available() {
    let service = Box::leak(Box::new(PolymarketPriceToBeatService::new()));

    let lookup = service.try_cached_or_spawn_with("btc-updown-5m-1773232500", |_| {
        Ok(ChainlinkPriceTimestampSnapshot {
            price: 69_200.0,
            timestamp_ms: 1_773_232_500_000,
        })
    });

    match lookup {
        PriceToBeatLookup::Ready(snapshot) => {
            assert_eq!(snapshot.source, PriceToBeatSource::ChainlinkRtdsStartTick);
            assert_eq!(snapshot.price_to_beat, 69_200.0);
        }
        other => panic!("expected ready lookup, got {other:?}"),
    }
}

#[tokio::test]
async fn try_cached_or_spawn_with_keeps_existing_provisional_snapshot() {
    let service = Box::leak(Box::new(PolymarketPriceToBeatService::new()));
    service.cache.lock().insert(
        "btc-updown-5m-1773232500".to_string(),
        PolymarketPriceToBeatSnapshot {
            event_url: "https://polymarket.com/event/btc-updown-5m-1773232500".to_string(),
            asset: "btc".to_string(),
            timeframe: "5m".to_string(),
            price_to_beat: 69_100.0,
            source: PriceToBeatSource::Polymarket,
            verified: false,
            source_latency_ms: None,
            fetched_at: Utc::now(),
        },
    );

    let lookup = service.try_cached_or_spawn_with("btc-updown-5m-1773232500", |_| {
        Ok(ChainlinkPriceTimestampSnapshot {
            price: 69_200.0,
            timestamp_ms: 1_773_232_500_000,
        })
    });

    match lookup {
        PriceToBeatLookup::Ready(snapshot) => {
            assert_eq!(snapshot.source, PriceToBeatSource::Polymarket);
            assert_eq!(snapshot.price_to_beat, 69_100.0);
        }
        other => panic!("expected ready lookup, got {other:?}"),
    }
}

#[test]
fn try_cached_or_spawn_returns_terminal_failure_once() {
    let service = Box::leak(Box::new(PolymarketPriceToBeatService::new()));
    service.record_terminal_failure(
        "btc-updown-5m-1773232500",
        "__NEXT_DATA__ script tag not found in html".to_string(),
    );

    let first = service.try_cached_or_spawn("btc-updown-5m-1773232500");
    let second = service.take_terminal_failure("btc-updown-5m-1773232500");

    match first {
        PriceToBeatLookup::Unavailable(detail) => {
            assert_eq!(detail, "__NEXT_DATA__ script tag not found in html");
        }
        other => panic!("expected unavailable lookup, got {other:?}"),
    }
    assert!(second.is_none());
}
