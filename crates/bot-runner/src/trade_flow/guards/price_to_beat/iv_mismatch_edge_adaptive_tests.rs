use super::super::iv_mismatch_adaptive::PriceToBeatIvVolumeBaselineMode;
use super::*;
use crate::trade_flow::guards::binance_price::{
    clear_binance_price_test_state, seed_binance_price_test_ticks,
};
use crate::trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks;

fn active_market_slug(seconds_left: i64) -> String {
    let start = Utc::now().timestamp() - (300 - seconds_left);
    format!("btc-updown-5m-{start}")
}

fn seed_ticks(prices: &[f64]) {
    let now_ms = Utc::now().timestamp_millis();
    let count = prices.len() as i64;
    let samples = prices
        .iter()
        .enumerate()
        .map(|(index, price)| (now_ms - 500 - (count - index as i64 - 1) * 1_000, *price))
        .collect::<Vec<_>>();
    seed_chainlink_price_test_ticks("btc", &samples).expect("seed chainlink ticks");
}

fn market(best_bid: f64, best_ask: f64) -> PriceToBeatSignalFormulaMarketInput {
    PriceToBeatSignalFormulaMarketInput {
        best_bid: Some(best_bid),
        best_ask: Some(best_ask),
    }
}

fn adaptive_config(best_bid: f64, best_ask: f64) -> PriceToBeatIvMismatchEdgeConfig {
    let mut config = PriceToBeatIvMismatchEdgeConfig::crypto_defaults(market(best_bid, best_ask));
    config.time_rules = vec![PriceToBeatIvMismatchTimeRule {
        start_remaining_secs: 120.0,
        end_remaining_secs: 10.0,
        max_price: Some(0.80),
        min_edge: 0.02,
        min_gap_strength: 0.0,
        min_expected_move_usd: None,
        min_gap_strength_margin: None,
        min_gap_usd_margin: Some(0.0),
    }];
    config.protection_mode = PriceToBeatIvProtectionMode::Adaptive;
    config.book_lead_guard_enabled = true;
    config.depth_guard_enabled = false;
    config.adaptive.volume_baseline_mode = PriceToBeatIvVolumeBaselineMode::Hourly;
    config.adaptive.volume_baseline_min_samples = 1;
    config.adaptive.red_block = true;
    config.adaptive_volume = Some(PriceToBeatIvAdaptiveVolumeInput {
        baseline_mode: PriceToBeatIvVolumeBaselineMode::Hourly,
        volume_window_sec: 30,
        current_volume_usdc: Some(200.0),
        baseline_median_usdc: Some(80.0),
        baseline_sample_count: Some(12),
        baseline_key: Some("BTC:12UTC:120-60:30s".to_string()),
        baseline_status: "hourly_ready",
    });
    config
}

fn book_quotes(up_bid: f64, up_ask: f64, down_bid: f64, down_ask: f64) -> PriceToBeatIvBookQuotes {
    PriceToBeatIvBookQuotes {
        up_bid: Some(up_bid),
        up_ask: Some(up_ask),
        down_bid: Some(down_bid),
        down_ask: Some(down_ask),
    }
}

#[tokio::test]
async fn adaptive_blocks_high_volume_reliable_opposite_book() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[100.0, 99.8, 99.5, 99.2, 99.0, 98.8, 98.5, 98.2]);
    let now_ms = Utc::now().timestamp_millis();
    seed_binance_price_test_ticks("btc", &[(now_ms - 300, 98.2)]).expect("seed binance");
    let mut config = adaptive_config(0.24, 0.26);
    config.book_quotes = Some(book_quotes(0.75, 0.77, 0.24, 0.26));
    config.too_good_to_be_true_gap = Some(0.90);

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(80),
        "Down",
        "btc",
        98.2,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(
        evaluation.reason,
        "blocked_adaptive_high_volume_book_opposite"
    );
    let adaptive = evaluation.adaptive.as_ref().expect("adaptive telemetry");
    assert_eq!(adaptive.regime, "red");
    assert_eq!(adaptive.reason, "high_volume_book_opposite");
}

#[tokio::test]
async fn adaptive_low_volume_opposite_book_is_orange_penalty_not_book_block() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[100.0, 99.8, 99.5, 99.2, 99.0, 98.8, 98.5, 98.2]);
    let mut config = adaptive_config(0.09, 0.10);
    config.book_quotes = Some(book_quotes(0.75, 0.77, 0.24, 0.26));
    config.adaptive_volume.as_mut().unwrap().current_volume_usdc = Some(10.0);
    config
        .adaptive_volume
        .as_mut()
        .unwrap()
        .baseline_median_usdc = Some(80.0);
    config.adaptive.orange_edge_delta = 0.01;
    config.adaptive.orange_gap_strength_delta = 0.01;

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(80),
        "Down",
        "btc",
        98.2,
        100.0,
        config,
    );

    assert_ne!(evaluation.reason, "blocked_book_leads_opposite");
    let adaptive = evaluation.adaptive.as_ref().expect("adaptive telemetry");
    assert_eq!(adaptive.regime, "orange");
    assert_eq!(adaptive.reason, "low_reliability_book_opposite");
    assert!(evaluation
        .protection_reasons
        .contains(&"blocked_book_leads_opposite"));
}

#[tokio::test]
async fn adaptive_green_relaxes_edge_and_gap_strength() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[100.0, 100.2, 100.5, 100.8, 101.0, 101.2, 101.5, 101.8]);
    let now_ms = Utc::now().timestamp_millis();
    seed_binance_price_test_ticks("btc", &[(now_ms - 300, 101.8)]).expect("seed binance");
    let mut config = adaptive_config(0.42, 0.44);
    config.time_rules[0].min_edge = 0.08;
    config.time_rules[0].min_gap_strength = 0.90;
    config.book_quotes = Some(book_quotes(0.67, 0.68, 0.32, 0.33));

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(80),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    let adaptive = evaluation.adaptive.as_ref().expect("adaptive telemetry");
    assert_eq!(adaptive.regime, "green");
    assert_eq!(adaptive.reason, "volume_confirms_selected_side");
    assert!(adaptive.adaptive_min_edge < adaptive.base_min_edge);
    assert!(adaptive.adaptive_gap_strength < adaptive.base_gap_strength);
}

#[tokio::test]
async fn adaptive_cold_start_is_yellow_neutral() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[100.0, 100.2, 100.5, 100.8, 101.0, 101.2, 101.5, 101.8]);
    let mut config = adaptive_config(0.42, 0.44);
    config.book_quotes = Some(book_quotes(0.67, 0.68, 0.32, 0.33));
    config.adaptive_volume = Some(PriceToBeatIvAdaptiveVolumeInput::neutral(
        PriceToBeatIvVolumeBaselineMode::Hourly,
        30,
        Some("BTC:12UTC:120-60:30s".to_string()),
        "cold_start_neutral",
    ));

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(80),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    let adaptive = evaluation.adaptive.as_ref().expect("adaptive telemetry");
    assert_eq!(adaptive.regime, "yellow");
    assert_eq!(adaptive.reason, "cold_start_neutral");
    assert_eq!(adaptive.edge_delta, 0.0);
    assert_eq!(adaptive.gap_strength_delta, 0.0);
}
