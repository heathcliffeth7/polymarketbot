use super::*;
use crate::trade_flow::guards::binance_price::{
    clear_binance_price_test_state, seed_binance_price_test_ticks,
};
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

fn seed_strong_chain_weak_binance() {
    seed_ticks_with_interval(
        &[
            118.8, 119.0, 119.1, 119.2, 119.3, 119.5, 119.6, 119.7, 119.8, 119.9, 120.0,
        ],
        500,
        1_000,
    );
    let now_ms = Utc::now().timestamp_millis();
    seed_binance_price_test_ticks("btc", &[(now_ms - 300, 100.0)]).expect("seed binance ticks");
}

#[tokio::test]
async fn iv_mismatch_edge_uses_fresh_binance_conservative_veto() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_strong_chain_weak_binance();
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        120.0,
        100.0,
        PriceToBeatIvMismatchEdgeConfig::crypto_defaults(market(0.59, 0.60)),
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_edge_below_threshold");
    assert_eq!(
        evaluation.binance_veto_status.as_deref(),
        Some("fresh_conservative_min")
    );
    let q_binance = evaluation.q_binance.expect("q_binance");
    let q_final = evaluation.q_final.expect("q_final");
    assert!((q_final - (q_binance + DEFAULT_BINANCE_Q_BUFFER)).abs() < 0.000_001);
    assert!(q_final < evaluation.q_chain_adj.expect("q_chain_adj"));
    assert_eq!(evaluation.binance_missing_penalty_applied, Some(0.0));
}

#[tokio::test]
async fn iv_mismatch_edge_adds_binance_missing_penalty_for_high_ask() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        500,
    );
    let mut config = config_with_time_rule(0.64, 0.66, Some(0.70), 0.06, 0.10);
    config.binance_missing_ask_threshold = 0.65;
    config.binance_missing_penalty = 0.02;
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    assert_eq!(evaluation.binance_missing_penalty_applied, Some(0.02));
    assert_eq!(evaluation.dynamic_threshold, Some(0.08));
    assert!(evaluation
        .binance_veto_status
        .as_deref()
        .unwrap_or_default()
        .starts_with("fail_open_unavailable:"));
}

#[tokio::test]
async fn iv_mismatch_edge_supports_all_crypto_rtds_assets() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let now_ms = Utc::now().timestamp_millis();

    for asset in ["eth", "btc", "sol", "xrp"] {
        clear_binance_price_test_state();
        seed_ticks_for_asset_with_interval(
            asset,
            &[
                118.8, 119.0, 119.1, 119.2, 119.3, 119.5, 119.6, 119.7, 119.8, 119.9, 120.0,
            ],
            500,
            1_000,
        );
        seed_binance_price_test_ticks(asset, &[(now_ms - 300, 120.0)]).expect("seed binance ticks");

        let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
            &active_market_slug_for(asset, 60),
            "Up",
            asset,
            120.0,
            100.0,
            PriceToBeatIvMismatchEdgeConfig::crypto_defaults(market(0.42, 0.44)),
        );

        assert!(evaluation.passed, "{asset} evaluation: {evaluation:?}");
        assert_eq!(
            evaluation.binance_veto_status.as_deref(),
            Some("fresh_conservative_min"),
            "{asset} should use fresh Binance veto path"
        );
        assert!(evaluation.q_final.expect("q_final").is_finite());
        assert!(evaluation.edge_adj.expect("edge_adj").is_finite());
    }
}

#[tokio::test]
async fn iv_mismatch_edge_blocks_thin_adjusted_margin() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        500,
    );
    let mut config = config_with_time_rule(0.42, 0.44, Some(0.65), 0.06, 0.10);
    let baseline = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        config.clone(),
    );
    assert!(baseline.passed, "{baseline:?}");
    let baseline_margin = baseline.adjusted_margin.expect("adjusted_margin");

    config.min_adjusted_margin = baseline_margin + 0.001;
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_thin_adjusted_margin");
    assert_eq!(evaluation.thin_margin_flag, Some(true));
    assert!(evaluation.edge_adj.expect("edge_adj") >= evaluation.dynamic_threshold.unwrap());
}

#[tokio::test]
async fn iv_mismatch_edge_adds_large_binance_disagreement_penalty() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_strong_chain_weak_binance();
    let mut config = config_with_time_rule(0.41, 0.42, Some(0.70), 0.06, 0.0);
    config.binance_disagreement_threshold = Some(0.15);
    config.binance_disagreement_penalty = 0.02;
    config.large_binance_disagreement_threshold = Some(0.20);
    config.large_binance_disagreement_penalty = 0.04;
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        120.0,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_edge_below_threshold");
    assert_eq!(evaluation.q_disagreement_bucket, Some("large"));
    assert_eq!(evaluation.binance_disagreement_penalty_applied, Some(0.04));
    assert_eq!(evaluation.dynamic_threshold, Some(0.10));
    assert!(evaluation.q_disagreement.expect("q_disagreement") > 0.20);
}

#[tokio::test]
async fn iv_mismatch_edge_adds_small_binance_disagreement_penalty() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_strong_chain_weak_binance();
    let mut config = config_with_time_rule(0.41, 0.42, Some(0.70), 0.06, 0.0);
    config.binance_disagreement_threshold = Some(0.15);
    config.binance_disagreement_penalty = 0.02;
    config.large_binance_disagreement_threshold = Some(0.90);
    config.large_binance_disagreement_penalty = 0.04;
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        120.0,
        100.0,
        config,
    );

    assert!(evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.q_disagreement_bucket, Some("small"));
    assert_eq!(evaluation.binance_disagreement_penalty_applied, Some(0.02));
    assert_eq!(evaluation.dynamic_threshold, Some(0.08));
}

#[tokio::test]
async fn iv_mismatch_edge_blocks_low_final_q() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_strong_chain_weak_binance();
    let mut config = config_with_time_rule(0.41, 0.42, Some(0.70), 0.01, 0.0);
    config.min_final_q = Some(0.62);
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        120.0,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_low_final_q");
    assert!(evaluation.q_final.expect("q_final") < 0.62);
    assert_eq!(evaluation.min_final_q, Some(0.62));
}

#[test]
fn inverse_normal_cdf_matches_known_shape() {
    assert!(inverse_normal_cdf(0.5).expect("median").abs() < 0.000_001);
    assert!((inverse_normal_cdf(0.841_344_7).expect("one sigma") - 1.0).abs() < 0.000_01);
}
