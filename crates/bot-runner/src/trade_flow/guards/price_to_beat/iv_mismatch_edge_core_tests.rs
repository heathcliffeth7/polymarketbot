use super::*;
use crate::trade_flow::guards::binance_price::clear_binance_price_test_state;
use crate::trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks;

fn active_market_slug(seconds_left: i64) -> String {
    active_market_slug_for("btc", seconds_left)
}

fn active_market_slug_for(asset: &str, seconds_left: i64) -> String {
    let start = Utc::now().timestamp() - (300 - seconds_left);
    format!("{asset}-updown-5m-{start}")
}

fn seed_ticks(prices: &[f64], latest_age_ms: i64) {
    seed_ticks_for_asset_with_interval("btc", prices, latest_age_ms, 5_000);
}

fn seed_ticks_with_interval(prices: &[f64], latest_age_ms: i64, interval_ms: i64) {
    seed_ticks_for_asset_with_interval("btc", prices, latest_age_ms, interval_ms);
}

fn seed_ticks_for_asset_with_interval(
    asset: &str,
    prices: &[f64],
    latest_age_ms: i64,
    interval_ms: i64,
) {
    let now_ms = Utc::now().timestamp_millis();
    let count = prices.len() as i64;
    let samples = prices
        .iter()
        .enumerate()
        .map(|(index, price)| {
            (
                now_ms - latest_age_ms - (count - index as i64 - 1) * interval_ms,
                *price,
            )
        })
        .collect::<Vec<_>>();
    seed_chainlink_price_test_ticks(asset, &samples).expect("seed chainlink ticks");
}

fn market(best_bid: f64, best_ask: f64) -> PriceToBeatSignalFormulaMarketInput {
    PriceToBeatSignalFormulaMarketInput {
        best_bid: Some(best_bid),
        best_ask: Some(best_ask),
    }
}

fn config_with_time_rule(
    best_bid: f64,
    best_ask: f64,
    max_price: Option<f64>,
    min_edge: f64,
    min_gap_strength: f64,
) -> PriceToBeatIvMismatchEdgeConfig {
    let mut config = PriceToBeatIvMismatchEdgeConfig::crypto_defaults(market(best_bid, best_ask));
    config.time_rules = vec![PriceToBeatIvMismatchTimeRule {
        start_remaining_secs: 120.0,
        end_remaining_secs: 10.0,
        max_price,
        min_edge,
        min_gap_strength,
        min_expected_move_usd: None,
        min_gap_strength_margin: None,
        min_gap_usd_margin: None,
    }];
    config
}

#[tokio::test]
async fn iv_mismatch_edge_passes_when_probability_beats_cost() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        500,
    );
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        PriceToBeatIvMismatchEdgeConfig::crypto_defaults(market(0.42, 0.44)),
    );

    assert!(evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "selected_edge_passed");
    assert_eq!(evaluation.selected_side, Some("up"));
    assert!(evaluation.edge.expect("edge") > 0.06);
    assert_eq!(
        evaluation.binance_veto_status.as_deref(),
        Some("fail_open_unavailable:no cached binance price for btcusdt")
    );
    assert!(evaluation.edge_adj.expect("edge_adj") > 0.06);
}

#[tokio::test]
async fn iv_mismatch_edge_passes_matching_time_rule() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        500,
    );
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        config_with_time_rule(0.42, 0.44, Some(0.65), 0.06, 0.10),
    );

    assert!(evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.selected_time_rule_index, Some(0));
    assert_eq!(evaluation.time_rule_max_price, Some(0.65));
    assert!(evaluation.gap_strength.expect("gap_strength") >= 0.10);
    assert_eq!(evaluation.expected_move_floor, None);
    assert_eq!(evaluation.q_before_floor, evaluation.q_after_floor);
}

#[tokio::test]
async fn iv_mismatch_edge_blocks_when_time_rule_missing() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        500,
    );
    let mut config = config_with_time_rule(0.42, 0.44, Some(0.65), 0.06, 0.10);
    config.time_rules[0].start_remaining_secs = 30.0;
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason, "blocked_no_matching_time_rule");
}

#[tokio::test]
async fn iv_mismatch_edge_blocks_time_rule_max_price() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        500,
    );
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        config_with_time_rule(0.63, 0.66, Some(0.65), 0.06, 0.10),
    );

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason, "blocked_time_rule_max_price");
}

#[tokio::test]
async fn iv_mismatch_edge_blocks_gap_strength_below_time_rule() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        500,
    );
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        config_with_time_rule(0.42, 0.44, Some(0.65), 0.06, 20.0),
    );

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason, "blocked_gap_strength_below_threshold");
    assert_eq!(evaluation.required_gap_strength, Some(20.0));
}

#[tokio::test]
async fn iv_mismatch_edge_applies_expected_move_floor() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        500,
    );
    let mut config = config_with_time_rule(0.42, 0.44, Some(0.65), 0.20, 0.0);
    config.time_rules[0].min_expected_move_usd = Some(100.0);
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason, "blocked_edge_below_threshold");
    assert_eq!(evaluation.expected_move_floor, Some(100.0));
    assert_eq!(evaluation.expected_move_eff, Some(100.0));
    assert!(evaluation.q_after_floor < evaluation.q_before_floor);
    assert_eq!(evaluation.q_chain_adj, evaluation.q_after_floor);
}

#[tokio::test]
async fn iv_mismatch_edge_adds_stale_gap_strength_penalty() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        500,
    );
    let mut config = config_with_time_rule(0.42, 0.44, Some(0.65), 0.06, 0.10);
    config.stale_gap_strength_penalty_ms = 100;
    config.stale_gap_strength_penalty = 20.0;
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason, "blocked_gap_strength_below_threshold");
    assert_eq!(evaluation.gap_strength_stale_penalty, Some(20.0));
}

#[tokio::test]
async fn iv_mismatch_edge_adds_negative_velocity_gap_strength_penalty() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks_with_interval(
        &[100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0, 105.0],
        500,
        1_000,
    );
    let mut config = config_with_time_rule(0.42, 0.44, Some(0.65), 0.06, 0.10);
    config.negative_velocity_gap_strength_penalty = 20.0;
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        105.0,
        100.0,
        config,
    );

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason, "blocked_gap_strength_below_threshold");
    assert_eq!(evaluation.gap_strength_velocity_penalty, Some(20.0));
}

#[tokio::test]
async fn iv_mismatch_edge_blocks_stale_chainlink_samples() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        4_000,
    );
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        PriceToBeatIvMismatchEdgeConfig::crypto_defaults(market(0.42, 0.44)),
    );

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason, "blocked_rtds_stale");
}

#[tokio::test]
async fn iv_mismatch_edge_blocks_falling_knife_drop() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks_with_interval(
        &[
            110.0, 110.1, 109.9, 110.0, 110.1, 109.9, 110.0, 110.1, 109.9, 110.0, 100.0,
        ],
        500,
        1_000,
    );
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        100.0,
        50.0,
        PriceToBeatIvMismatchEdgeConfig::crypto_defaults(market(0.79, 0.80)),
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_falling_knife_drop");
    assert!(evaluation.edge.expect("raw edge") > 0.06);
    assert!(evaluation.drop_z.expect("drop_z") > 1.0);
}
