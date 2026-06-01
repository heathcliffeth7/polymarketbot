use super::notification::{
    build_price_to_beat_guard_blocked_notification_message,
    build_price_to_beat_guard_recovered_notification_message,
    build_price_to_beat_guard_waiting_notification_message,
    build_price_to_beat_relax_changed_notification_message,
};
use super::notification_state::{
    price_to_beat_guard_notification_phase, price_to_beat_guard_notification_seed_reason,
    price_to_beat_guard_waiting_state, set_price_to_beat_guard_notification_phase,
    set_price_to_beat_guard_notification_seed, set_price_to_beat_guard_waiting_state,
    PriceToBeatGuardNotificationPhase, PRICE_TO_BEAT_GUARD_NOTIFICATION_SEED_KEY,
};
use super::*;

mod shared_eval;
mod stop_loss_bump;

const BTC_MARKET_5M: &str = "btc-updown-5m-1774013100";
const SUPPORTED_ASSET_MARKETS: [(&str, &str); 4] = [
    ("btc", "btc-updown-5m-1774016400"),
    ("eth", "eth-updown-5m-1774016400"),
    ("sol", "sol-updown-5m-1774016400"),
    ("xrp", "xrp-updown-5m-1774016400"),
];
static PRICE_TO_BEAT_TEST_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

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
        configured_threshold_mode: None,
        base_threshold_value: None,
        base_threshold_unit: None,
        base_threshold_usd: None,
        current_effective_ptb_usd: None,
        threshold_value: 0.0,
        threshold_unit: "usd".to_string(),
        threshold_usd: 0.0,
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
        auto_threshold_usd: None,
        lookback_windows_used: None,
        current_windows_used: None,
        avg_up_excursion_usd: None,
        avg_down_excursion_usd: None,
        lookback_market_slugs: None,
        lookback_window_snapshots: None,
        baseline_pct: None,
        current_pct: None,
        vol_factor: None,
        threshold_pct: None,
        base_pct: None,
        floor_usd: None,
        ceiling_usd: None,
        threshold_was_clamped: None,
        signal_formula: None,
        iv_mismatch_edge: None,
        early_stale_side: None,
        cex_direction_guard: None,
    }
}
fn test_action_place_order_node(config: Value) -> crate::TradeFlowNode {
    crate::TradeFlowNode {
        key: "action_1".to_string(),
        node_type: "action.place_order".to_string(),
        config,
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
        current_windows_used: None,
        avg_up_excursion_usd: None,
        avg_down_excursion_usd: None,
        lookback_market_slugs: None,
        lookback_window_snapshots: None,
        baseline_pct: None,
        current_pct: None,
        vol_factor: None,
        threshold_pct: None,
        base_pct: None,
        floor_usd: None,
        ceiling_usd: None,
        threshold_was_clamped: None,
        signal_formula: None,
        iv_mismatch_edge: None,
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {actual} to be within {tolerance} of {expected}"
    );
}

fn assert_close_option(actual: Option<f64>, expected: f64, tolerance: f64) {
    let actual = actual.expect("expected Some(f64)");
    assert_close(actual, expected, tolerance);
}

pub(crate) fn lock_price_to_beat_test_state() -> std::sync::MutexGuard<'static, ()> {
    let guard = PRICE_TO_BEAT_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    crate::trade_flow::guards::polymarket_price_to_beat::clear_price_to_beat_test_state();
    guard
}

fn seed_completed_market_history(
    market_slug: &str,
    asset: &str,
    completed_windows: &[(f64, f64, f64, f64)],
    current_price: f64,
) {
    let current_start = crate::MarketCycleId(market_slug.to_string())
        .start_time()
        .expect("market start");
    let scope = crate::find_updown_scope_by_slug(market_slug).expect("supported scope");
    let window_ms = crate::updown_scope_window_seconds(scope) * 1_000;
    let mut ticks = Vec::with_capacity(completed_windows.len() * 4 + 1);

    for (index, (open_price, high_price, low_price, close_price)) in
        completed_windows.iter().enumerate()
    {
        let offset = (completed_windows.len() - index) as i64;
        let window_start = current_start - crate::ChronoDuration::milliseconds(window_ms * offset);
        let window_end = window_start + crate::ChronoDuration::milliseconds(window_ms);
        ticks.push((window_start.timestamp_millis(), *open_price));
        ticks.push((window_start.timestamp_millis() + window_ms / 4, *high_price));
        ticks.push((window_start.timestamp_millis() + window_ms / 2, *low_price));
        ticks.push((window_end.timestamp_millis() - 1, *close_price));
    }

    ticks.push((chrono::Utc::now().timestamp_millis(), current_price));
    crate::trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(asset, &ticks)
        .expect("seed completed market history");
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
fn parses_auto_vol_pct_mode() {
    assert_eq!(
        PriceToBeatMode::parse(Some("auto_vol_pct")),
        Some(PriceToBeatMode::AutoVolPct)
    );
    assert_eq!(PriceToBeatMode::AutoVolPct.as_str(), "auto_vol_pct");
}

#[test]
fn manual_reentry_ptb_override_uses_explicit_unit_when_provided() {
    let node = test_action_place_order_node(json!({
        "priceToBeatMode": "manual",
        "priceToBeatMaxDiff": 10,
        "priceToBeatMaxDiffUnit": "cent",
        "reentryPriceToBeatMaxDiff": 5,
        "reentryPriceToBeatMaxDiffUnit": "usd"
    }));
    let context = json!({
        "nodeState": {
            "action_1": {
                "reentry_generation": 1
            }
        }
    });

    let resolved = resolve_action_place_order_price_to_beat_guard_resolution(
        &node,
        &context,
        BTC_MARKET_5M,
        "Up",
    )
    .expect("manual reentry ptb resolution with explicit unit");

    assert_eq!(resolved.configured_mode, PriceToBeatMode::Manual);
    assert_eq!(resolved.effective_mode, PriceToBeatMode::Manual);
    assert_eq!(resolved.threshold_value, Some(5.0));
    assert_eq!(resolved.threshold_unit, PriceToBeatDiffUnit::Usd);
    assert_eq!(resolved.reentry_generation, 1);
    assert!(resolved.reentry_override_active);
    assert_eq!(
        resolved.reentry_override_unit,
        Some(PriceToBeatDiffUnit::Usd)
    );
}

#[test]
fn manual_reentry_ptb_override_inherits_primary_unit_when_unit_missing() {
    let node = test_action_place_order_node(json!({
        "priceToBeatMode": "manual",
        "priceToBeatMaxDiff": 10,
        "priceToBeatMaxDiffUnit": "cent",
        "reentryPriceToBeatMaxDiff": 5
    }));
    let context = json!({
        "nodeState": {
            "action_1": {
                "reentry_generation": 1
            }
        }
    });

    let resolved = resolve_action_place_order_price_to_beat_guard_resolution(
        &node,
        &context,
        BTC_MARKET_5M,
        "Up",
    )
    .expect("manual reentry ptb resolution");

    assert_eq!(resolved.configured_mode, PriceToBeatMode::Manual);
    assert_eq!(resolved.effective_mode, PriceToBeatMode::Manual);
    assert_eq!(resolved.threshold_value, Some(5.0));
    assert_eq!(resolved.threshold_unit, PriceToBeatDiffUnit::Cent);
    assert_eq!(resolved.reentry_generation, 1);
    assert!(resolved.reentry_override_active);
    assert_eq!(
        resolved.reentry_override_unit,
        Some(PriceToBeatDiffUnit::Cent)
    );
}

#[test]
fn auto_reentry_ptb_override_uses_manual_style_override_unit() {
    let node = test_action_place_order_node(json!({
        "priceToBeatMode": "auto_vol_pct",
        "reentryPriceToBeatMaxDiff": 7,
        "reentryPriceToBeatMaxDiffUnit": "usd"
    }));
    let context = json!({
        "nodeState": {
            "action_1": {
                "reentry_generation": 2
            }
        }
    });

    let resolved = resolve_action_place_order_price_to_beat_guard_resolution(
        &node,
        &context,
        BTC_MARKET_5M,
        "Up",
    )
    .expect("auto reentry ptb resolution");

    assert_eq!(resolved.configured_mode, PriceToBeatMode::AutoVolPct);
    assert_eq!(resolved.effective_mode, PriceToBeatMode::Manual);
    assert_eq!(resolved.threshold_value, Some(7.0));
    assert_eq!(resolved.threshold_unit, PriceToBeatDiffUnit::Usd);
    assert_eq!(resolved.reentry_generation, 2);
    assert!(resolved.reentry_override_active);
    assert_eq!(
        resolved.reentry_override_unit,
        Some(PriceToBeatDiffUnit::Usd)
    );
}

#[test]
fn first_entry_keeps_primary_ptb_threshold_when_reentry_override_exists() {
    let node = test_action_place_order_node(json!({
        "priceToBeatMode": "manual",
        "priceToBeatMaxDiff": 10,
        "priceToBeatMaxDiffUnit": "usd",
        "reentryPriceToBeatMaxDiff": 5
    }));

    let resolved = resolve_action_place_order_price_to_beat_guard_resolution(
        &node,
        &json!({}),
        BTC_MARKET_5M,
        "Up",
    )
    .expect("first entry ptb resolution");

    assert_eq!(resolved.configured_mode, PriceToBeatMode::Manual);
    assert_eq!(resolved.effective_mode, PriceToBeatMode::Manual);
    assert_eq!(resolved.threshold_value, Some(10.0));
    assert_eq!(resolved.threshold_unit, PriceToBeatDiffUnit::Usd);
    assert_eq!(resolved.reentry_generation, 0);
    assert!(!resolved.reentry_override_active);
}

#[test]
fn auto_vol_pct_blocks_xrp_markets() {
    let market_slug = "xrp-updown-5m-1774118500";
    let gate = evaluate_price_to_beat_trigger_gate(
        market_slug,
        "Up",
        PriceToBeatMode::AutoVolPct,
        None,
        None,
        PriceToBeatDiffUnit::Usd,
        None,
    );

    assert!(!gate.passed);
    assert_eq!(gate.reason, "unsupported_market");
    assert_eq!(gate.threshold_mode, "auto_vol_pct");
}

#[tokio::test]
async fn auto_vol_pct_uses_base_threshold_when_current_matches_baseline() {
    let _guard = lock_price_to_beat_test_state();
    let market_slug = "eth-updown-5m-1774116400";
    let completed_windows = vec![(2_000.0, 2_005.0, 1_995.0, 2_000.0); 20];
    seed_completed_market_history(market_slug, "eth", &completed_windows, 2_003.0);

    let snapshot = match resolve_auto_price_to_beat_threshold(
        AutoPriceToBeatThresholdStrategy::VolPct,
        market_slug,
        "Up",
    ) {
        AutoPriceToBeatThresholdResolution::Ready(snapshot) => snapshot,
        other => panic!("expected Ready snapshot, got {other:?}"),
    };
    let (threshold_usd, was_clamped) = snapshot
        .resolved_threshold_usd(2_000.0)
        .expect("resolved threshold");

    assert_eq!(snapshot.lookback_windows_used, 20);
    assert_eq!(snapshot.current_windows_used, Some(3));
    assert_close_option(snapshot.baseline_pct, 0.005, 1e-9);
    assert_close_option(snapshot.current_pct, 0.005, 1e-9);
    assert_close_option(snapshot.vol_factor, 1.0, 1e-9);
    assert_close_option(snapshot.threshold_pct, 0.0012, 1e-9);
    assert_close(threshold_usd, 2.4, 1e-9);
    assert!(!was_clamped);
}

#[tokio::test]
async fn auto_vol_pct_scales_btc_threshold_down_in_low_volume() {
    let _guard = lock_price_to_beat_test_state();
    let market_slug = "btc-updown-5m-1774116700";
    let mut completed_windows = vec![(70_000.0, 70_105.0, 69_895.0, 70_000.0); 17];
    completed_windows.extend(vec![(70_000.0, 70_052.5, 69_947.5, 70_000.0); 3]);
    seed_completed_market_history(market_slug, "btc", &completed_windows, 70_050.0);

    let snapshot = match resolve_auto_price_to_beat_threshold(
        AutoPriceToBeatThresholdStrategy::VolPct,
        market_slug,
        "Up",
    ) {
        AutoPriceToBeatThresholdResolution::Ready(snapshot) => snapshot,
        other => panic!("expected Ready snapshot, got {other:?}"),
    };
    let (threshold_usd, was_clamped) = snapshot
        .resolved_threshold_usd(70_000.0)
        .expect("resolved threshold");

    assert_close_option(snapshot.baseline_pct, 0.0026109491415613867, 1e-9);
    assert_close_option(snapshot.current_pct, 0.0015, 1e-9);
    assert_close_option(snapshot.vol_factor, 0.7579602377990333, 1e-9);
    assert_close(threshold_usd, 47.7514949813391, 1e-9);
    assert!(!was_clamped);
}

#[tokio::test]
async fn auto_vol_pct_scales_sol_threshold_up_in_high_volume() {
    let _guard = lock_price_to_beat_test_state();
    let market_slug = "sol-updown-5m-1774117000";
    let mut completed_windows = vec![(82.0, 82.030012, 81.969988, 82.0); 17];
    completed_windows.extend(vec![(82.0, 82.05, 81.95, 82.0); 3]);
    seed_completed_market_history(market_slug, "sol", &completed_windows, 82.10);

    let snapshot = match resolve_auto_price_to_beat_threshold(
        AutoPriceToBeatThresholdStrategy::VolPct,
        market_slug,
        "Up",
    ) {
        AutoPriceToBeatThresholdResolution::Ready(snapshot) => snapshot,
        other => panic!("expected Ready snapshot, got {other:?}"),
    };
    let (threshold_usd, was_clamped) = snapshot
        .resolved_threshold_usd(82.0)
        .expect("resolved threshold");

    assert_close_option(snapshot.baseline_pct, 0.0008584446920076274, 1e-9);
    assert_close_option(snapshot.current_pct, 0.0012195121951219512, 1e-9);
    assert_close_option(snapshot.vol_factor, 1.1918920252723535, 1e-9);
    assert_close(threshold_usd, 0.048867573036166494, 1e-9);
    assert!(!was_clamped);
}

#[tokio::test]
async fn auto_vol_pct_clamps_to_floor_when_threshold_gets_too_small() {
    let _guard = lock_price_to_beat_test_state();
    let market_slug = "eth-updown-5m-1774117300";
    let mut completed_windows = vec![(2_000.0, 2_005.0, 1_995.0, 2_000.0); 17];
    completed_windows.extend(vec![(2_000.0, 2_000.01, 1_999.99, 2_000.0); 3]);
    seed_completed_market_history(market_slug, "eth", &completed_windows, 2_001.0);

    let snapshot = match resolve_auto_price_to_beat_threshold(
        AutoPriceToBeatThresholdStrategy::VolPct,
        market_slug,
        "Up",
    ) {
        AutoPriceToBeatThresholdResolution::Ready(snapshot) => snapshot,
        other => panic!("expected Ready snapshot, got {other:?}"),
    };
    let (threshold_usd, was_clamped) = snapshot
        .resolved_threshold_usd(2_000.0)
        .expect("resolved threshold");

    assert_close(threshold_usd, 1.0, 1e-9);
    assert!(was_clamped);
    assert_close_option(snapshot.floor_usd, 1.0, 1e-12);
}

#[tokio::test]
async fn auto_vol_pct_clamps_to_ceiling_for_sol() {
    let _guard = lock_price_to_beat_test_state();
    let market_slug = "sol-updown-5m-1774117600";
    let mut completed_windows = vec![(82.0, 82.030012, 81.969988, 82.0); 17];
    completed_windows.extend(vec![(82.0, 82.0615, 81.9385, 82.0); 3]);
    seed_completed_market_history(market_slug, "sol", &completed_windows, 82.10);

    let snapshot = match resolve_auto_price_to_beat_threshold(
        AutoPriceToBeatThresholdStrategy::VolPct,
        market_slug,
        "Up",
    ) {
        AutoPriceToBeatThresholdResolution::Ready(snapshot) => snapshot,
        other => panic!("expected Ready snapshot, got {other:?}"),
    };
    let (threshold_usd, was_clamped) = snapshot
        .resolved_threshold_usd(82.0)
        .expect("resolved threshold");

    assert_close(threshold_usd, 0.05, 1e-9);
    assert!(was_clamped);
    assert_close_option(snapshot.ceiling_usd, 0.05, 1e-12);
}

#[tokio::test]
async fn auto_vol_pct_falls_back_to_vol_factor_one_when_history_is_flat() {
    let _guard = lock_price_to_beat_test_state();
    let market_slug = "eth-updown-5m-1774117900";
    let completed_windows = vec![(2_000.0, 2_000.0, 2_000.0, 2_000.0); 20];
    seed_completed_market_history(market_slug, "eth", &completed_windows, 2_003.0);

    let snapshot = match resolve_auto_price_to_beat_threshold(
        AutoPriceToBeatThresholdStrategy::VolPct,
        market_slug,
        "Up",
    ) {
        AutoPriceToBeatThresholdResolution::Ready(snapshot) => snapshot,
        other => panic!("expected Ready snapshot, got {other:?}"),
    };
    let (threshold_usd, was_clamped) = snapshot
        .resolved_threshold_usd(2_000.0)
        .expect("resolved threshold");

    assert_close_option(snapshot.baseline_pct, 0.0, 1e-9);
    assert_close_option(snapshot.current_pct, 0.0, 1e-9);
    assert_close_option(snapshot.vol_factor, 1.0, 1e-9);
    assert_close(threshold_usd, 2.4, 1e-9);
    assert!(!was_clamped);
}

#[tokio::test]
async fn auto_vol_pct_blocks_as_pending_when_fewer_than_three_windows_exist() {
    let _guard = lock_price_to_beat_test_state();
    let market_slug = "eth-updown-5m-1774118200";
    let completed_windows = vec![(2_000.0, 2_005.0, 1_995.0, 2_000.0); 2];
    seed_completed_market_history(market_slug, "eth", &completed_windows, 2_003.0);

    let resolution = resolve_auto_price_to_beat_threshold(
        AutoPriceToBeatThresholdStrategy::VolPct,
        market_slug,
        "Up",
    );

    assert!(matches!(
        resolution,
        AutoPriceToBeatThresholdResolution::Pending(_)
    ));
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
        None,
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
    let _guard = lock_price_to_beat_test_state();
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
    assert!(
        crate::trade_flow::guards::polymarket_price_to_beat::seed_price_to_beat_from_chainlink(
            BTC_MARKET_5M,
            "btc",
            "5m",
            100.0,
            None,
        )
    );

    let gate = evaluate_price_to_beat_trigger_gate(
        BTC_MARKET_5M,
        "Up",
        PriceToBeatMode::AutoLast3AvgExcursion,
        None,
        None,
        PriceToBeatDiffUnit::Usd,
        None,
    );

    assert!(gate.passed);
    assert_eq!(gate.reason, "in_range");
    assert_eq!(gate.threshold_mode, "auto_last_3_avg_excursion");
    assert_eq!(gate.lookback_windows_used, Some(3));
    assert_eq!(gate.lookback_market_slugs.as_ref().map(Vec::len), Some(3));
    let window_snapshots = gate
        .lookback_window_snapshots
        .as_ref()
        .expect("window snapshots");
    assert_eq!(window_snapshots.len(), 3);
    assert_eq!(
        window_snapshots[0]
            .get("market_slug")
            .and_then(Value::as_str),
        Some("btc-updown-5m-1774012200")
    );
    assert_eq!(
        window_snapshots[0]
            .get("open_price")
            .and_then(Value::as_f64),
        Some(100.0)
    );
    assert_eq!(
        window_snapshots[0]
            .get("high_price")
            .and_then(Value::as_f64),
        Some(130.0)
    );
    assert_eq!(
        window_snapshots[0].get("low_price").and_then(Value::as_f64),
        Some(95.0)
    );
    assert_eq!(
        window_snapshots[0]
            .get("close_price")
            .and_then(Value::as_f64),
        Some(110.0)
    );
    assert_eq!(gate.avg_up_excursion_usd, Some(22.666666666666668));
    assert_eq!(gate.avg_down_excursion_usd, Some(6.666666666666667));
    assert_eq!(gate.auto_threshold_usd, gate.avg_up_excursion_usd);
    assert_eq!(gate.min_gap, 22.666666666666668);
}

#[tokio::test]
async fn auto_trigger_gate_blocks_as_pending_when_three_completed_markets_are_missing() {
    let _guard = lock_price_to_beat_test_state();
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
        None,
    );

    assert!(!gate.passed);
    assert_eq!(gate.reason, "auto_threshold_pending");
    assert_eq!(gate.threshold_mode, "auto_last_3_avg_excursion");
    assert_eq!(gate.lookback_windows_used, None);
}

#[tokio::test]
async fn auto_guard_uses_dynamic_threshold_diagnostics() {
    let _guard = lock_price_to_beat_test_state();
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
    assert!(
        crate::trade_flow::guards::polymarket_price_to_beat::seed_price_to_beat_from_chainlink(
            "sol-updown-5m-1774013100",
            "sol",
            "5m",
            210.0,
            None,
        )
    );

    let evaluation = evaluate_price_to_beat_guard(
        "sol-updown-5m-1774013100",
        PriceToBeatMode::AutoLast3AvgExcursion,
        None,
        PriceToBeatDiffUnit::Usd,
        "Up",
        None,
    )
    .await;

    assert!(evaluation.passed);
    assert_eq!(evaluation.threshold_mode, "auto_last_3_avg_excursion");
    assert_eq!(evaluation.lookback_windows_used, Some(3));
    assert_eq!(evaluation.avg_up_excursion_usd, Some(16.0));
    assert_eq!(evaluation.avg_down_excursion_usd, Some(4.0));
    assert_eq!(
        evaluation.lookback_window_snapshots.as_ref().map(Vec::len),
        Some(3)
    );
    let value = evaluation.to_value();
    assert_eq!(
        value
            .get("lookback_window_snapshots")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(3)
    );
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
fn relax_changed_notification_message_clarifies_raw_vs_effective_threshold() {
    let evaluation = PriceToBeatGuardEvaluation {
        direction: Some("down".to_string()),
        market_slug: "eth-updown-5m-1776198300".to_string(),
        timeframe: Some("5m".to_string()),
        asset: Some("eth".to_string()),
        ..default_guard_evaluation()
    };

    let message = build_price_to_beat_relax_changed_notification_message(
        &evaluation,
        Some(1.0),
        Some(0.67328696),
        1.0,
        Some(0.42328696),
        0.25,
        1.0,
        11,
        &["eth-updown-5m-1776198000".to_string()],
    );

    assert!(message.contains("Onceki Bildirilen Relax PTB: 1.00000000"));
    assert!(message.contains("Ham Relax PTB: 0.67328696"));
    assert!(message.contains("Bu Market Efektif Relax PTB: 1.00000000"));
    assert!(message.contains("Min Uygun Gap: 0.42328696"));
    assert!(message.contains("Floor: 1.00000000"));
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
        PriceToBeatCurrentPriceSource::Chainlink,
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
        PriceToBeatCurrentPriceSource::Chainlink,
        PriceToBeatSource::Polymarket,
        BTC_MARKET_5M,
        "btc",
        None,
        "stale price for btc/usd: 216s old (provider_age_ms=216937, receive_age_ms=90, provider_timestamp_ms=1774012890000, received_at_ms=1774013106847)",
    );
    assert_eq!(reason_code, "current_price_unavailable");
    assert!(reason_detail.contains("primary_source=chainlink_live_data_ws"));
    assert!(reason_detail.contains("current_price_error=stale price for btc/usd"));
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
        PriceToBeatCurrentPriceSource::Chainlink,
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
