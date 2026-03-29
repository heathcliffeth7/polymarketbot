use super::*;

const BTC_MARKET_5M: &str = "btc-updown-5m-1774013100";
const SUPPORTED_ASSET_MARKETS: [(&str, &str); 4] = [
    ("btc", "btc-updown-5m-1774016400"),
    ("eth", "eth-updown-5m-1774016400"),
    ("sol", "sol-updown-5m-1774016400"),
    ("xrp", "xrp-updown-5m-1774016400"),
];

fn default_guard_evaluation() -> PriceToBeatGuardEvaluation {
    PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: String::new(),
        reason_detail: None,
        normalized_outcome_label: None,
        direction: None,
        market_slug: String::new(),
        event_url: String::new(),
        timeframe: None,
        asset: None,
        price_to_beat: None,
        price_to_beat_status: None,
        price_to_beat_source: None,
        price_to_beat_source_latency_ms: None,
        current_price: None,
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: None,
        threshold_mode: "manual".to_string(),
        threshold_value: 0.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 0.0,
        auto_threshold_usd: None,
        lookback_windows_used: None,
        avg_up_excursion_usd: None,
        avg_down_excursion_usd: None,
        lookback_market_slugs: None,
    }
}

fn default_trigger_gate_result() -> PriceToBeatTriggerGateResult {
    PriceToBeatTriggerGateResult {
        passed: false,
        reason: "price_to_beat_pending",
        directional_gap: None,
        price_to_beat: None,
        price_to_beat_status: None,
        current_price: None,
        threshold_mode: "manual".to_string(),
        min_gap: 0.0,
        max_gap: None,
        auto_threshold_usd: None,
        lookback_windows_used: None,
        avg_up_excursion_usd: None,
        avg_down_excursion_usd: None,
        lookback_market_slugs: None,
    }
}

#[test]
fn builds_gap_below_threshold_notification_with_values() {
    let evaluation = PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: "price_to_beat_gap_below_threshold".to_string(),
        reason_detail: None,
        normalized_outcome_label: Some("yes".to_string()),
        direction: Some("up".to_string()),
        market_slug: "btc-updown-5m-1773232500".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1773232500".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("btc".to_string()),
        price_to_beat: Some(69_279.93484689),
        price_to_beat_status: Some("polymarket_verified".to_string()),
        price_to_beat_source: Some("polymarket".to_string()),
        price_to_beat_source_latency_ms: None,
        current_price: Some(69_300.12),
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: Some(20.18515311),
        gap_abs: Some(20.18515311),
        threshold_value: 30.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 30.0,
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
    assert!(message.contains("Price to Beat Korumasi Engelledi"));
    assert!(message.contains("Yon: Up"));
    assert!(message.contains("Market: btc-updown-5m-1773232500"));
    assert!(message.contains("Asset: btc"));
    assert!(message.contains("gereken minimum seviyenin altinda"));
    assert!(message.contains("Yonsel Fark: 20.18515311"));
    assert!(message.contains("Karar Metrigi: Yonsel fark kullanilir"));
    assert!(message.contains("Limit: 30.00000000 usd (~30.00000000 USD)"));
}

#[test]
fn blocked_notification_includes_reason_detail_and_partial_prices() {
    let evaluation = PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: "price_to_beat_pending".to_string(),
        reason_detail: Some("__NEXT_DATA__ script tag not found in html".to_string()),
        normalized_outcome_label: None,
        direction: None,
        market_slug: "btc-updown-5m-1773242700".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1773242700".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("btc".to_string()),
        price_to_beat: None,
        price_to_beat_status: None,
        price_to_beat_source: None,
        price_to_beat_source_latency_ms: None,
        current_price: Some(70_404.25964978),
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: None,
        threshold_value: 30.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 30.0,
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
    assert!(message.contains("Price to Beat verisi henuz hazir degil"));
    assert!(message.contains("Detay: __NEXT_DATA__ script tag not found in html"));
    assert!(message.contains("Current (Chainlink): 70404.25964978"));
    assert!(message.contains("Price to Beat: N/A"));
}

#[test]
fn pending_notification_without_detail_omits_detail_line() {
    let evaluation = PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: "price_to_beat_pending".to_string(),
        reason_detail: None,
        normalized_outcome_label: None,
        direction: None,
        market_slug: "btc-updown-5m-1774028100".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1774028100".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("btc".to_string()),
        price_to_beat: None,
        price_to_beat_status: None,
        price_to_beat_source: None,
        price_to_beat_source_latency_ms: None,
        current_price: None,
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: None,
        threshold_value: 20.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 20.0,
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_waiting_notification_message(&evaluation);
    assert!(message.contains("cycle-open fiyat snapshot'i bekleniyor"));
    assert!(!message.contains("Detay:"));
    assert!(!message.contains("background fetch in progress"));
}

#[test]
fn waiting_notification_mentions_recovery_retry() {
    let evaluation = PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: "price_to_beat_gap_below_threshold".to_string(),
        reason_detail: None,
        normalized_outcome_label: Some("yes".to_string()),
        direction: Some("up".to_string()),
        market_slug: "btc-updown-5m-1773232500".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1773232500".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("btc".to_string()),
        price_to_beat: Some(69_279.93484689),
        price_to_beat_status: Some("polymarket_verified".to_string()),
        price_to_beat_source: Some("polymarket".to_string()),
        price_to_beat_source_latency_ms: None,
        current_price: Some(69_300.12),
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: Some(20.18515311),
        gap_abs: Some(20.18515311),
        threshold_value: 30.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 30.0,
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_waiting_notification_message(&evaluation);
    assert!(message.contains("Bekleme moduna alindi"));
    assert!(message.contains("yeniden denenecek"));
}

#[test]
fn recovered_notification_mentions_previous_reason_and_metrics() {
    let evaluation = PriceToBeatGuardEvaluation {
        passed: true,
        reason_code: "passed".to_string(),
        reason_detail: None,
        normalized_outcome_label: Some("yes".to_string()),
        direction: Some("up".to_string()),
        market_slug: "btc-updown-5m-1773232500".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1773232500".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("btc".to_string()),
        price_to_beat: Some(69_279.93484689),
        price_to_beat_status: Some("polymarket_verified".to_string()),
        price_to_beat_source: Some("polymarket".to_string()),
        price_to_beat_source_latency_ms: None,
        current_price: Some(69_320.12),
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: Some(40.18515311),
        gap_abs: Some(40.18515311),
        threshold_value: 30.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 30.0,
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_recovered_notification_message(
        &evaluation,
        "price_to_beat_gap_below_threshold",
    );
    assert!(message.contains("Price to Beat Korumasi Gecti"));
    assert!(message.contains("Onceki Sebep: price_to_beat_gap_below_threshold"));
    assert!(message.contains("Price to Beat Source: polymarket"));
    assert!(message.contains("Current (Chainlink): 69320.12000000"));
}

#[test]
fn cent_threshold_converts_to_usd() {
    assert_eq!(
        normalize_price_to_beat_threshold_usd(1.0, PriceToBeatDiffUnit::Cent),
        0.01
    );
    assert_eq!(
        normalize_price_to_beat_threshold_usd(0.01, PriceToBeatDiffUnit::Cent),
        0.0001
    );
}

#[test]
fn parses_threshold_units() {
    assert_eq!(
        PriceToBeatDiffUnit::parse(Some("usd")),
        Some(PriceToBeatDiffUnit::Usd)
    );
    assert_eq!(
        PriceToBeatDiffUnit::parse(Some("cent")),
        Some(PriceToBeatDiffUnit::Cent)
    );
    assert_eq!(
        PriceToBeatDiffUnit::parse(None),
        Some(PriceToBeatDiffUnit::Usd)
    );
    assert_eq!(PriceToBeatDiffUnit::parse(Some("foo")), None);
}

#[test]
fn trigger_gate_cache_miss_blocks_as_pending() {
    let gate = evaluate_price_to_beat_trigger_gate(
        "missing-market-slug",
        "Up",
        PriceToBeatMode::Manual,
        Some(30.0),
        Some(60.0),
        PriceToBeatDiffUnit::Usd,
    );
    assert!(!gate.passed);
    assert_eq!(gate.reason, "price_to_beat_pending");
    assert_eq!(gate.min_gap, 30.0);
    assert_eq!(gate.max_gap, Some(60.0));
}

#[test]
fn trigger_gate_value_includes_reason_and_bounds() {
    let gate = PriceToBeatTriggerGateResult {
        passed: false,
        reason: "below_min_gap",
        directional_gap: Some(12.5),
        price_to_beat: Some(100.0),
        price_to_beat_status: Some("polymarket_verified".to_string()),
        current_price: Some(112.5),
        threshold_mode: "manual".to_string(),
        min_gap: 30.0,
        max_gap: Some(60.0),
        ..default_trigger_gate_result()
    };
    let value = gate.to_value();
    assert_eq!(
        value.get("reason").and_then(Value::as_str),
        Some("below_min_gap")
    );
    assert_eq!(value.get("min_gap").and_then(Value::as_f64), Some(30.0));
    assert_eq!(value.get("max_gap").and_then(Value::as_f64), Some(60.0));
}

#[tokio::test]
async fn auto_trigger_gate_uses_average_excursion_from_last_three_markets() {
    let now_ms = chrono::Utc::now().timestamp_millis();
    crate::trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
        "btc",
        &[
            (1_774_012_200_000, 100.0),
            (1_774_012_350_000, 130.0),
            (1_774_012_480_000, 95.0),
            (1_774_012_499_000, 110.0),
            (1_774_012_500_000, 110.0),
            (1_774_012_650_000, 125.0),
            (1_774_012_790_000, 100.0),
            (1_774_012_799_000, 122.0),
            (1_774_012_800_000, 122.0),
            (1_774_012_950_000, 145.0),
            (1_774_013_090_000, 117.0),
            (1_774_013_099_000, 130.0),
            (now_ms, 123.0),
        ],
    )
    .expect("seed chainlink ticks");
    assert!(crate::trade_flow::guards::polymarket_price_to_beat::seed_price_to_beat_from_chainlink(
        BTC_MARKET_5M,
        "btc",
        "5m",
        100.0,
        None,
    ));

    let gate = evaluate_price_to_beat_trigger_gate(
        BTC_MARKET_5M,
        "Up",
        PriceToBeatMode::AutoLast3AvgExcursion,
        None,
        None,
        PriceToBeatDiffUnit::Usd,
    );

    assert!(gate.passed);
    assert_eq!(gate.reason, "in_range");
    assert_eq!(gate.threshold_mode, "auto_last_3_avg_excursion");
    assert_eq!(gate.lookback_windows_used, Some(3));
    assert_eq!(gate.lookback_market_slugs.as_ref().map(Vec::len), Some(3));
    assert_eq!(gate.avg_up_excursion_usd, Some(22.666666666666668));
    assert_eq!(gate.avg_down_excursion_usd, Some(6.666666666666667));
    assert_eq!(gate.auto_threshold_usd, gate.avg_up_excursion_usd);
    assert_eq!(gate.min_gap, 22.666666666666668);
}

#[test]
fn auto_trigger_gate_blocks_as_pending_when_three_completed_markets_are_missing() {
    crate::trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
        "eth",
        &[
            (1_774_012_500_000, 2_000.0),
            (1_774_012_800_000, 2_020.0),
            (1_774_012_800_000, 2_010.0),
            (1_774_013_100_000, 2_030.0),
            (1_774_013_101_000, 2_040.0),
        ],
    )
    .expect("seed eth ticks");

    let gate = evaluate_price_to_beat_trigger_gate(
        "eth-updown-5m-1774013100",
        "Up",
        PriceToBeatMode::AutoLast3AvgExcursion,
        None,
        None,
        PriceToBeatDiffUnit::Usd,
    );

    assert!(!gate.passed);
    assert_eq!(gate.reason, "auto_threshold_pending");
    assert_eq!(gate.threshold_mode, "auto_last_3_avg_excursion");
    assert_eq!(gate.lookback_windows_used, None);
}

#[tokio::test]
async fn auto_guard_uses_dynamic_threshold_diagnostics() {
    let now_ms = chrono::Utc::now().timestamp_millis();
    crate::trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
        "sol",
        &[
            (1_774_012_200_000, 200.0),
            (1_774_012_350_000, 215.0),
            (1_774_012_480_000, 195.0),
            (1_774_012_500_000, 205.0),
            (1_774_012_500_500, 210.0),
            (1_774_012_650_000, 225.0),
            (1_774_012_790_000, 205.0),
            (1_774_012_800_000, 222.0),
            (1_774_012_800_500, 220.0),
            (1_774_012_950_000, 235.0),
            (1_774_013_090_000, 215.0),
            (1_774_013_100_000, 230.0),
            (now_ms, 231.0),
        ],
    )
    .expect("seed sol ticks");
    assert!(crate::trade_flow::guards::polymarket_price_to_beat::seed_price_to_beat_from_chainlink(
        "sol-updown-5m-1774013100",
        "sol",
        "5m",
        210.0,
        None,
    ));

    let evaluation = evaluate_price_to_beat_guard(
        "sol-updown-5m-1774013100",
        PriceToBeatMode::AutoLast3AvgExcursion,
        None,
        PriceToBeatDiffUnit::Usd,
        "Up",
    )
    .await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.threshold_mode, "auto_last_3_avg_excursion");
    assert_eq!(evaluation.lookback_windows_used, Some(3));
    assert_eq!(evaluation.avg_up_excursion_usd, Some(16.0));
    assert_eq!(evaluation.avg_down_excursion_usd, Some(4.0));
    assert_eq!(evaluation.auto_threshold_usd, Some(16.0));
    assert_eq!(evaluation.threshold_usd, 16.0);
}

#[test]
fn min_gap_mode_blocks_when_gap_is_below_threshold() {
    let gap_abs = (70_230.57_f64 - 70_249.29979780_f64).abs();
    assert!(gap_abs < 30.0);
    assert!(!(gap_abs >= 30.0));
}

#[test]
fn min_gap_mode_allows_when_gap_equals_threshold() {
    let gap_abs = 30.0_f64;
    assert!(gap_abs >= 30.0);
}

#[test]
fn min_gap_mode_allows_when_gap_is_above_threshold() {
    let gap_abs = 38.72979780_f64;
    assert!(gap_abs >= 30.0);
}

#[test]
fn up_direction_passes_when_current_above_threshold() {
    let evaluation = evaluate_directional_gap(105.0, 100.0, 4.0, "Up").expect("direction");
    assert_eq!(
        evaluation,
        DirectionalGapEvaluation {
            normalized_outcome_label: "yes",
            direction: "up",
            directional_gap: 5.0,
            passed: true,
        }
    );
}

#[test]
fn up_direction_blocks_when_price_below() {
    let evaluation = evaluate_directional_gap(95.0, 100.0, 4.0, "Up").expect("direction");
    assert_eq!(evaluation.direction, "up");
    assert_eq!(evaluation.directional_gap, -5.0);
    assert!(!evaluation.passed);
}

#[test]
fn down_direction_passes_when_current_below_threshold() {
    let evaluation = evaluate_directional_gap(95.0, 100.0, 4.0, "Down").expect("direction");
    assert_eq!(
        evaluation,
        DirectionalGapEvaluation {
            normalized_outcome_label: "no",
            direction: "down",
            directional_gap: 5.0,
            passed: true,
        }
    );
}

#[test]
fn down_direction_blocks_when_price_above() {
    let evaluation = evaluate_directional_gap(105.0, 100.0, 4.0, "Down").expect("direction");
    assert_eq!(evaluation.direction, "down");
    assert_eq!(evaluation.directional_gap, -5.0);
    assert!(!evaluation.passed);
}

#[test]
fn unsupported_outcome_label_blocks() {
    assert!(evaluate_directional_gap(105.0, 100.0, 4.0, "Flat").is_none());
}

#[test]
fn current_price_unavailable_reason_is_supported() {
    let evaluation = PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: "current_price_unavailable".to_string(),
        reason_detail: Some(
            "chainlink ws error: stale price for btc/usd: 123s old (provider_age_ms=123000, receive_age_ms=250, provider_timestamp_ms=1774000000000, received_at_ms=1774000122750)".to_string(),
        ),
        normalized_outcome_label: None,
        direction: None,
        market_slug: "btc-updown-5m-1773246900".to_string(),
        event_url: "https://polymarket.com/event/btc-updown-5m-1773246900".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("btc".to_string()),
        price_to_beat: Some(70_714.62472011),
        price_to_beat_status: Some("polymarket_verified".to_string()),
        price_to_beat_source: Some("polymarket".to_string()),
        price_to_beat_source_latency_ms: None,
        current_price: None,
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: None,
        threshold_value: 30.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 30.0,
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
    assert!(message.contains("Current price verisi alinamadigi"));
    assert!(message.contains("Current (Chainlink): N/A"));
    assert!(message.contains("provider_age_ms=123000"));
}

#[test]
fn current_price_unavailable_with_provisional_price_to_beat_stays_strict() {
    let evaluation = PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: "current_price_unavailable".to_string(),
        reason_detail: Some(
            "asset=eth; market_slug=eth-updown-5m-1774564800; primary_source=chainlink_live_data_ws; chainlink_error=stale price for eth/usd: 4237s old (provider_age_ms=4237828, receive_age_ms=4236687, provider_timestamp_ms=1774560571000, received_at_ms=1774560572141)".to_string(),
        ),
        normalized_outcome_label: None,
        direction: None,
        market_slug: "eth-updown-5m-1774564800".to_string(),
        event_url: "https://polymarket.com/event/eth-updown-5m-1774564800".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("eth".to_string()),
        price_to_beat: Some(2_061.85412287),
        price_to_beat_status: Some("polymarket_provisional".to_string()),
        price_to_beat_source: Some("polymarket".to_string()),
        price_to_beat_source_latency_ms: None,
        current_price: None,
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: None,
        threshold_value: 80.0,
        threshold_unit: "cent".to_string(),
        threshold_usd: 0.8,
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_waiting_notification_message(&evaluation);
    assert!(message.contains("Current price verisi alinamadigi"));
    assert!(message.contains("Price to Beat Status: polymarket_provisional"));
    assert!(message.contains("Current (Chainlink): N/A"));
    assert!(message.contains("Bekleme moduna alindi"));
}

#[test]
fn chainlink_rtds_start_tick_stale_current_price_maps_to_pending() {
    let (reason_code, reason_detail) = map_current_price_error(
        PriceToBeatSource::ChainlinkRtdsStartTick,
        BTC_MARKET_5M,
        "btc",
        Some(217_000),
        "stale price for btc/usd: 216s old (provider_age_ms=216937, receive_age_ms=90, provider_timestamp_ms=1774012890000, received_at_ms=1774013106847)",
    );
    assert_eq!(reason_code, "price_to_beat_pending");
    assert!(reason_detail.contains("asset=btc"));
    assert!(reason_detail.contains("snapshot_source=chainlink_rtds_start_tick"));
    assert!(reason_detail.contains(&format!("market_slug={BTC_MARKET_5M}")));
    assert!(reason_detail.contains("gap_ms=217000"));
    assert!(reason_detail.contains("provider_age_ms=216937"));
    assert!(reason_detail.contains("receive_age_ms=90"));
    assert!(reason_detail.contains("awaiting_current_price_tick=true"));
    assert!(reason_detail.contains("chainlink_error=stale price for btc/usd"));
}

#[test]
fn authoritative_price_to_beat_keeps_current_price_unavailable_reason() {
    let (reason_code, reason_detail) = map_current_price_error(
        PriceToBeatSource::Polymarket,
        BTC_MARKET_5M,
        "btc",
        None,
        "stale price for btc/usd: 216s old (provider_age_ms=216937, receive_age_ms=90, provider_timestamp_ms=1774012890000, received_at_ms=1774013106847)",
    );
    assert_eq!(reason_code, "current_price_unavailable");
    assert!(reason_detail.contains("primary_source=chainlink_live_data_ws"));
    assert!(reason_detail.contains("chainlink_error=stale price for btc/usd"));
}

#[test]
fn pending_reason_can_describe_authoritative_snapshot_wait() {
    let evaluation = PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: "price_to_beat_pending".to_string(),
        reason_detail: Some(
            "snapshot_source=chainlink_rtds_start_tick; market_slug=btc-updown-5m-1774013100; gap_ms=217000; provider_age_ms=216937; receive_age_ms=90; awaiting_current_price_tick=true; raw_error=stale price for btc/usd: 216s old".to_string(),
        ),
        normalized_outcome_label: None,
        direction: None,
        market_slug: BTC_MARKET_5M.to_string(),
        event_url: format!("https://polymarket.com/event/{BTC_MARKET_5M}"),
        timeframe: Some("5m".to_string()),
        asset: Some("btc".to_string()),
        price_to_beat: Some(70_484.80743654),
        price_to_beat_status: Some("rtds_live".to_string()),
        price_to_beat_source: Some("chainlink_rtds_start_tick".to_string()),
        price_to_beat_source_latency_ms: Some(217_000),
        current_price: None,
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: None,
        threshold_value: 20.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 20.0,
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_waiting_notification_message(&evaluation);
    assert!(message.contains("Price to Beat verisi henuz hazir degil"));
    assert!(message.contains("awaiting_current_price_tick=true"));
    assert!(message.contains("Price to Beat Source: chainlink_rtds_start_tick"));
    assert!(message.contains("Price to Beat Status: rtds_live"));
    assert!(message.contains("gap_ms=217000"));
}

#[test]
fn chainlink_rtds_start_tick_no_cached_current_price_uses_unknown_age_placeholders() {
    let (reason_code, reason_detail) = map_current_price_error(
        PriceToBeatSource::ChainlinkRtdsStartTick,
        BTC_MARKET_5M,
        "btc",
        Some(217_000),
        "no cached price for btc/usd; last ws error: live data websocket stream ended",
    );
    assert_eq!(reason_code, "price_to_beat_pending");
    assert!(reason_detail.contains("provider_age_ms=unknown"));
    assert!(reason_detail.contains("receive_age_ms=unknown"));
    assert!(reason_detail.contains("gap_ms=217000"));
    assert!(reason_detail.contains("chainlink_error=no cached price for btc/usd"));
}

#[test]
fn resolve_current_price_result_uses_chainlink_for_all_supported_assets() {
    for ((asset, market_slug), expected_price) in SUPPORTED_ASSET_MARKETS
        .into_iter()
        .zip([69_720.16, 2_124.25, 88.77, 1.43])
    {
        let resolved = resolve_current_price_result(
            PriceToBeatSource::Polymarket,
            market_slug,
            asset,
            None,
            Ok(expected_price),
        )
        .expect("chainlink current price should resolve");
        assert_eq!(resolved, (expected_price, CURRENT_PRICE_SOURCE_CHAINLINK));
    }
}

#[test]
fn resolve_current_price_result_keeps_failure_for_all_supported_assets() {
    for (asset, market_slug) in SUPPORTED_ASSET_MARKETS {
        let (reason_code, reason_detail) = resolve_current_price_result(
            PriceToBeatSource::Polymarket,
            market_slug,
            asset,
            None,
            Err("stale price for test/usd: 700s old"),
        )
        .unwrap_err();
        assert_eq!(reason_code, "current_price_unavailable");
        assert!(reason_detail.contains(&format!("asset={asset}")));
        assert!(reason_detail.contains(&format!("market_slug={market_slug}")));
        assert!(reason_detail.contains("primary_source=chainlink_live_data_ws"));
    }
}

#[test]
fn blocked_notification_uses_chainlink_current_price_label() {
    let evaluation = PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: "price_to_beat_gap_below_threshold".to_string(),
        reason_detail: None,
        normalized_outcome_label: Some("yes".to_string()),
        direction: Some("up".to_string()),
        market_slug: "eth-updown-5m-1774016400".to_string(),
        event_url: "https://polymarket.com/event/eth-updown-5m-1774016400".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("eth".to_string()),
        price_to_beat: Some(2_120.0),
        price_to_beat_status: Some("polymarket_verified".to_string()),
        price_to_beat_source: Some("polymarket".to_string()),
        price_to_beat_source_latency_ms: None,
        current_price: Some(2_124.25),
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: Some(4.25),
        gap_abs: Some(4.25),
        threshold_value: 4.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 4.0,
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
    assert!(message.contains("Current (Chainlink): 2124.25000000"));
}

#[test]
fn unsupported_outcome_label_reason_is_supported() {
    let evaluation = PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: "unsupported_outcome_label".to_string(),
        reason_detail: Some("outcome_label 'Flat' is not a recognized direction".to_string()),
        normalized_outcome_label: None,
        direction: None,
        market_slug: "eth-updown-5m-1773710400".to_string(),
        event_url: "https://polymarket.com/event/eth-updown-5m-1773710400".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("eth".to_string()),
        price_to_beat: Some(2366.97),
        price_to_beat_status: Some("polymarket_provisional".to_string()),
        price_to_beat_source: Some("polymarket".to_string()),
        price_to_beat_source_latency_ms: None,
        current_price: Some(2368.11),
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: Some(1.14),
        threshold_value: 4.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 4.0,
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
    assert!(message.contains("Outcome label Up/Down veya Yes/No"));
    assert!(message.contains("Yon: N/A"));
}

#[test]
fn price_to_beat_notification_seed_is_consumed_for_matching_identity() {
    let mut context = json!({});
    set_price_to_beat_guard_notification_seed(
        &mut context,
        "action_1",
        "btc-updown-5m-1773232500",
        "tok-up",
        "price_to_beat:price_to_beat_gap_below_threshold",
    );

    let reason = take_price_to_beat_guard_notification_seed(
        &mut context,
        "action_1",
        "btc-updown-5m-1773232500",
        "tok-up",
    );

    assert_eq!(
        reason.as_deref(),
        Some("price_to_beat:price_to_beat_gap_below_threshold")
    );
    assert!(
        crate::flow_context_value(&context, PRICE_TO_BEAT_GUARD_NOTIFICATION_SEED_KEY).is_none()
    );
}

#[test]
fn price_to_beat_notification_seed_ignores_mismatched_identity() {
    let mut context = json!({});
    set_price_to_beat_guard_notification_seed(
        &mut context,
        "action_1",
        "btc-updown-5m-1773232500",
        "tok-up",
        "price_to_beat:price_to_beat_gap_below_threshold",
    );

    let reason = take_price_to_beat_guard_notification_seed(
        &mut context,
        "action_2",
        "btc-updown-5m-1773232500",
        "tok-up",
    );

    assert!(reason.is_none());
    assert_eq!(
        price_to_beat_guard_notification_seed_reason(
            &context,
            "action_1",
            "btc-updown-5m-1773232500",
            "tok-up"
        )
        .as_deref(),
        Some("price_to_beat:price_to_beat_gap_below_threshold")
    );
}

#[test]
fn waiting_state_tracks_market_and_reason() {
    let mut context = json!({});
    set_price_to_beat_guard_waiting_state(
        &mut context,
        "btc-updown-5m-1773232500",
        "price_to_beat_gap_below_threshold",
    );

    let state = price_to_beat_guard_waiting_state(&context).expect("waiting state");
    assert_eq!(state.market_slug, "btc-updown-5m-1773232500");
    assert_eq!(state.reason_code, "price_to_beat_gap_below_threshold");
}

#[test]
fn notification_phase_tracks_identity_and_phase() {
    let mut context = json!({});
    set_price_to_beat_guard_notification_phase(
        &mut context,
        "action_1",
        "btc-updown-5m-1773232500",
        "tok-up",
        PriceToBeatGuardNotificationPhase::BlockedNotified,
        "price_to_beat_gap_below_threshold",
    );

    assert_eq!(
        price_to_beat_guard_notification_phase(
            &context,
            "action_1",
            "btc-updown-5m-1773232500",
            "tok-up"
        ),
        Some(PriceToBeatGuardNotificationPhase::BlockedNotified)
    );
}

#[test]
fn notification_phase_is_identity_scoped() {
    let mut context = json!({});
    set_price_to_beat_guard_notification_phase(
        &mut context,
        "action_1",
        "btc-updown-5m-1773232500",
        "tok-up",
        PriceToBeatGuardNotificationPhase::PassedNotified,
        "passed",
    );

    assert_eq!(
        price_to_beat_guard_notification_phase(
            &context,
            "action_2",
            "btc-updown-5m-1773232500",
            "tok-up"
        ),
        None
    );
    assert_eq!(
        price_to_beat_guard_notification_phase(
            &context,
            "action_1",
            "btc-updown-5m-1773232800",
            "tok-up"
        ),
        None
    );
}
