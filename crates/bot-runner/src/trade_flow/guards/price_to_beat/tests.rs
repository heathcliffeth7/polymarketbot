use super::*;

const BTC_MARKET_5M: &str = "btc-updown-5m-1774013100";
const SUPPORTED_ASSET_MARKETS: [(&str, &str); 4] = [
    ("btc", "btc-updown-5m-1774016400"),
    ("eth", "eth-updown-5m-1774016400"),
    ("sol", "sol-updown-5m-1774016400"),
    ("xrp", "xrp-updown-5m-1774016400"),
];

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
        price_to_beat_source: Some("polymarket".to_string()),
        price_to_beat_source_latency_ms: None,
        current_price: Some(69_300.12),
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: Some(20.18515311),
        gap_abs: Some(20.18515311),
        threshold_value: 30.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 30.0,
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
        price_to_beat_source: None,
        price_to_beat_source_latency_ms: None,
        current_price: Some(70_404.25964978),
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: None,
        threshold_value: 30.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 30.0,
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
        price_to_beat_source: None,
        price_to_beat_source_latency_ms: None,
        current_price: None,
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: None,
        threshold_value: 20.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 20.0,
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
        price_to_beat_source: Some("polymarket".to_string()),
        price_to_beat_source_latency_ms: None,
        current_price: Some(69_300.12),
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: Some(20.18515311),
        gap_abs: Some(20.18515311),
        threshold_value: 30.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 30.0,
    };

    let message = build_price_to_beat_guard_waiting_notification_message(&evaluation);
    assert!(message.contains("Bekleme moduna alindi"));
    assert!(message.contains("yeniden denenecek"));
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
        30.0,
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
        current_price: Some(112.5),
        min_gap: 30.0,
        max_gap: Some(60.0),
    };
    let value = gate.to_value();
    assert_eq!(
        value.get("reason").and_then(Value::as_str),
        Some("below_min_gap")
    );
    assert_eq!(value.get("min_gap").and_then(Value::as_f64), Some(30.0));
    assert_eq!(value.get("max_gap").and_then(Value::as_f64), Some(60.0));
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
        price_to_beat_source: Some("polymarket".to_string()),
        price_to_beat_source_latency_ms: None,
        current_price: None,
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: None,
        threshold_value: 30.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 30.0,
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
    assert!(message.contains("Current price verisi alinamadigi"));
    assert!(message.contains("Current (Chainlink): N/A"));
    assert!(message.contains("provider_age_ms=123000"));
}

#[test]
fn chainlink_carryover_stale_current_price_maps_to_pending() {
    let (reason_code, reason_detail) = map_current_price_error(
        PriceToBeatSource::ChainlinkCarryover,
        BTC_MARKET_5M,
        "btc",
        Some(217_000),
        "stale price for btc/usd: 216s old (provider_age_ms=216937, receive_age_ms=90, provider_timestamp_ms=1774012890000, received_at_ms=1774013106847)",
        Some("coinbase timeout"),
    );
    assert_eq!(reason_code, "price_to_beat_pending");
    assert!(reason_detail.contains("asset=btc"));
    assert!(reason_detail.contains("snapshot_source=chainlink_carryover"));
    assert!(reason_detail.contains(&format!("market_slug={BTC_MARKET_5M}")));
    assert!(reason_detail.contains("gap_ms=217000"));
    assert!(reason_detail.contains("provider_age_ms=216937"));
    assert!(reason_detail.contains("receive_age_ms=90"));
    assert!(reason_detail.contains("awaiting_authoritative_polymarket_snapshot=true"));
    assert!(reason_detail.contains("fallback_error=coinbase timeout"));
}

#[test]
fn authoritative_price_to_beat_keeps_current_price_unavailable_reason() {
    let (reason_code, reason_detail) = map_current_price_error(
        PriceToBeatSource::Polymarket,
        BTC_MARKET_5M,
        "btc",
        None,
        "stale price for btc/usd: 216s old (provider_age_ms=216937, receive_age_ms=90, provider_timestamp_ms=1774012890000, received_at_ms=1774013106847)",
        Some("coinbase timeout"),
    );
    assert_eq!(reason_code, "current_price_unavailable");
    assert!(reason_detail.contains("primary_source=chainlink_live_data_ws"));
    assert!(reason_detail.contains("fallback_source=coinbase_spot"));
    assert!(reason_detail.contains("chainlink_error=stale price for btc/usd"));
    assert!(reason_detail.contains("fallback_error=coinbase timeout"));
}

#[test]
fn pending_reason_can_describe_authoritative_snapshot_wait() {
    let evaluation = PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: "price_to_beat_pending".to_string(),
        reason_detail: Some(
            "snapshot_source=chainlink_carryover; market_slug=btc-updown-5m-1774013100; gap_ms=217000; provider_age_ms=216937; receive_age_ms=90; awaiting_authoritative_polymarket_snapshot=true; raw_error=stale price for btc/usd: 216s old".to_string(),
        ),
        normalized_outcome_label: None,
        direction: None,
        market_slug: BTC_MARKET_5M.to_string(),
        event_url: format!("https://polymarket.com/event/{BTC_MARKET_5M}"),
        timeframe: Some("5m".to_string()),
        asset: Some("btc".to_string()),
        price_to_beat: Some(70_484.80743654),
        price_to_beat_source: Some("chainlink_carryover".to_string()),
        price_to_beat_source_latency_ms: Some(217_000),
        current_price: None,
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: None,
        threshold_value: 20.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 20.0,
    };

    let message = build_price_to_beat_guard_waiting_notification_message(&evaluation);
    assert!(message.contains("Price to Beat verisi henuz hazir degil"));
    assert!(message.contains("awaiting_authoritative_polymarket_snapshot=true"));
    assert!(message.contains("Price to Beat Source: chainlink_carryover"));
    assert!(message.contains("gap_ms=217000"));
}

#[test]
fn chainlink_carryover_no_cached_current_price_uses_unknown_age_placeholders() {
    let (reason_code, reason_detail) = map_current_price_error(
        PriceToBeatSource::ChainlinkCarryover,
        BTC_MARKET_5M,
        "btc",
        Some(217_000),
        "no cached price for btc/usd; last ws error: live data websocket stream ended",
        Some("coinbase 503"),
    );
    assert_eq!(reason_code, "price_to_beat_pending");
    assert!(reason_detail.contains("provider_age_ms=unknown"));
    assert!(reason_detail.contains("receive_age_ms=unknown"));
    assert!(reason_detail.contains("gap_ms=217000"));
    assert!(reason_detail.contains("fallback_error=coinbase 503"));
}

#[test]
fn resolve_current_price_result_uses_coinbase_fallback_for_all_supported_assets() {
    for ((asset, market_slug), expected_price) in SUPPORTED_ASSET_MARKETS
        .into_iter()
        .zip([69_720.16, 2_124.25, 88.77, 1.43])
    {
        let resolved = resolve_current_price_result(
            PriceToBeatSource::Polymarket,
            market_slug,
            asset,
            None,
            Err("stale price for test/usd: 700s old"),
            Some(Ok(expected_price)),
        )
        .expect("coinbase fallback should resolve");
        assert_eq!(
            resolved,
            (expected_price, CURRENT_PRICE_SOURCE_COINBASE_FALLBACK)
        );
    }
}

#[test]
fn resolve_current_price_result_keeps_failure_when_coinbase_fails_for_all_supported_assets() {
    for (asset, market_slug) in SUPPORTED_ASSET_MARKETS {
        let (reason_code, reason_detail) = resolve_current_price_result(
            PriceToBeatSource::Polymarket,
            market_slug,
            asset,
            None,
            Err("stale price for test/usd: 700s old"),
            Some(Err("coinbase timeout")),
        )
        .unwrap_err();
        assert_eq!(reason_code, "current_price_unavailable");
        assert!(reason_detail.contains(&format!("asset={asset}")));
        assert!(reason_detail.contains(&format!("market_slug={market_slug}")));
        assert!(reason_detail.contains("fallback_source=coinbase_spot"));
    }
}

#[test]
fn blocked_notification_uses_source_aware_current_price_label() {
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
        price_to_beat_source: Some("polymarket".to_string()),
        price_to_beat_source_latency_ms: None,
        current_price: Some(2_124.25),
        current_price_source: CURRENT_PRICE_SOURCE_COINBASE_FALLBACK,
        directional_gap: Some(4.25),
        gap_abs: Some(4.25),
        threshold_value: 4.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 4.0,
    };

    let message = build_price_to_beat_guard_blocked_notification_message(&evaluation);
    assert!(message.contains("Current (Coinbase fallback): 2124.25000000"));
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
        price_to_beat_source: Some("polymarket".to_string()),
        price_to_beat_source_latency_ms: None,
        current_price: Some(2368.11),
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: Some(1.14),
        threshold_value: 4.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 4.0,
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
