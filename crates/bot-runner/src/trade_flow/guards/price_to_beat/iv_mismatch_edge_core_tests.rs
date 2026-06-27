use super::super::iv_mismatch_expected_move::PriceToBeatIvMinExpectedMoveMode;
use super::super::iv_mismatch_time_rule::PriceToBeatIvMismatchTimeRule;
use super::*;
use crate::trade_flow::guards::binance_price::{
    clear_binance_price_test_state, seed_binance_price_test_ticks,
};
use crate::trade_flow::guards::cex_microstructure::{
    clear_cex_microstructure_test_state, seed_cex_book_test_sample, seed_cex_open_test_sample,
    CexBookSample, CexVenue,
};
use crate::trade_flow::guards::chainlink_price::{
    clear_chainlink_price_test_state, seed_chainlink_price_test_ticks,
};
use bot_infra::exchange::{OrderBookLevel, OrderBookSnapshot};

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

fn legacy_iv_edge_config(
    market: PriceToBeatSignalFormulaMarketInput,
) -> PriceToBeatIvMismatchEdgeConfig {
    let mut config = PriceToBeatIvMismatchEdgeConfig::crypto_defaults(market);
    config.cex_open_gap.decision_gap_enabled = false;
    config.oracle_tick_jump.enabled = false;
    config
}

fn seed_cex_open_gap_for_market(market_slug: &str, open_mid: f64, current_mid: f64) {
    let start_ms = crate::MarketCycleId(market_slug.to_string())
        .start_time()
        .expect("market start")
        .timestamp_millis();
    let now_ms = Utc::now().timestamp_millis();
    for venue in [CexVenue::Binance, CexVenue::Okx] {
        seed_cex_open_test_sample(CexBookSample {
            venue,
            asset: "btc".to_string(),
            timestamp_ms: start_ms,
            bid: open_mid - 0.5,
            ask: open_mid + 0.5,
            bid_size: Some(1.0),
            ask_size: Some(1.0),
            source: "rest_open",
        });
        seed_cex_book_test_sample(CexBookSample {
            venue,
            asset: "btc".to_string(),
            timestamp_ms: now_ms - 250,
            bid: current_mid - 0.5,
            ask: current_mid + 0.5,
            bid_size: Some(1.0),
            ask_size: Some(1.0),
            source: "ws_book",
        });
    }
}

fn config_with_time_rule(
    best_bid: f64,
    best_ask: f64,
    max_price: Option<f64>,
    min_edge: f64,
    min_gap_strength: f64,
) -> PriceToBeatIvMismatchEdgeConfig {
    let mut config = legacy_iv_edge_config(market(best_bid, best_ask));
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
        legacy_iv_edge_config(market(0.42, 0.44)),
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
    assert!(evaluation.q_final.is_some());
    assert!(evaluation.gap_strength.is_some());
    assert!(evaluation.expected_move_eff.is_some());
    assert!(evaluation
        .all_reasons
        .contains(&"blocked_time_rule_max_price"));
    let payload = evaluation.to_value();
    assert_eq!(
        payload
            .get("time_rule_price_blocked")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[tokio::test]
async fn iv_mismatch_edge_reports_insufficient_vol_sample_readiness() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[100.0, 100.2, 100.4], 500);
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        100.4,
        100.0,
        config_with_time_rule(0.49, 0.50, Some(0.90), 0.06, 0.0),
    );

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason, "blocked_insufficient_vol_samples");
    assert_eq!(evaluation.sample_count, Some(3));
    assert_eq!(evaluation.delta_count, None);
    let payload = evaluation.to_value();
    assert_eq!(
        payload
            .get("vol_sample_status")
            .and_then(serde_json::Value::as_str),
        Some("insufficient_samples")
    );
    assert_eq!(
        payload
            .get("min_vol_samples")
            .and_then(serde_json::Value::as_u64),
        Some(8)
    );
}

#[tokio::test]
async fn iv_mismatch_edge_reports_fetch_error_when_chainlink_samples_missing() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    clear_chainlink_price_test_state();
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug_for("sol", 60),
        "Up",
        "sol",
        65.32,
        65.21,
        config_with_time_rule(0.49, 0.50, Some(0.90), 0.06, 0.0),
    );

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason, "blocked_insufficient_vol_samples");
    let payload = evaluation.to_value();
    assert_eq!(
        payload
            .get("vol_sample_status")
            .and_then(serde_json::Value::as_str),
        Some("fetch_error")
    );
    assert_eq!(
        payload
            .get("chainlink_symbol")
            .and_then(serde_json::Value::as_str),
        Some("sol/usd")
    );
    assert_eq!(
        payload
            .get("vol_sample_error")
            .and_then(serde_json::Value::as_str),
        Some("no cached price for sol/usd")
    );
    assert_eq!(
        payload
            .get("sample_window_secs")
            .and_then(serde_json::Value::as_i64),
        Some(45)
    );
}

#[tokio::test]
async fn iv_mismatch_edge_execution_vwap_guard_blocks_above_max_price() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        500,
    );
    let mut config = config_with_time_rule(0.59, 0.60, Some(0.77), 0.06, 0.10);
    config.execution_vwap_guard.enabled = true;
    config.depth_guard_enabled = true;
    config.depth_max_slippage = 1.0;
    config.depth_intended_qty = Some(5.0);
    config.depth_order_book = Some(OrderBookSnapshot {
        bids: vec![OrderBookLevel {
            price: 0.59,
            size: 10.0,
        }],
        asks: vec![OrderBookLevel {
            price: 0.80,
            size: 5.0,
        }],
    });
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason, "blocked_execution_vwap_above_max_price");
    assert_eq!(
        evaluation.execution_vwap.block_reason,
        Some("blocked_execution_vwap_above_max_price")
    );
    assert!(evaluation
        .all_reasons
        .contains(&"blocked_execution_vwap_above_max_price"));
}

#[tokio::test]
async fn iv_mismatch_edge_time_rule_remains_primary_when_vwap_also_fails() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        500,
    );
    let mut config = config_with_time_rule(0.64, 0.66, Some(0.65), 0.06, 0.10);
    config.execution_vwap_guard.enabled = true;
    config.depth_guard_enabled = true;
    config.depth_max_slippage = 1.0;
    config.depth_intended_qty = Some(5.0);
    config.depth_order_book = Some(OrderBookSnapshot {
        bids: vec![OrderBookLevel {
            price: 0.64,
            size: 10.0,
        }],
        asks: vec![OrderBookLevel {
            price: 0.80,
            size: 5.0,
        }],
    });
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason, "blocked_time_rule_max_price");
    assert!(evaluation
        .all_reasons
        .contains(&"blocked_time_rule_max_price"));
    assert!(evaluation
        .all_reasons
        .contains(&"blocked_execution_vwap_above_max_price"));
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
async fn iv_mismatch_edge_blocks_gap_strength_even_when_eq77_override_allowed() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        500,
    );
    let mut config = config_with_time_rule(0.42, 0.44, Some(0.65), 0.06, 20.0);
    config.entry_quality.enabled = true;
    config.entry_quality.eq77_risk_cap_enabled = true;
    config.entry_quality.risk_score_high_max = 100.0;

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
    assert!(evaluation
        .entry_quality
        .as_ref()
        .is_some_and(|entry| entry.allowed));
    assert!(evaluation.cex_magnitude.eq77_gap_override_requested);
    assert!(evaluation.cex_magnitude.eq77_gap_override_effective);
}

#[tokio::test]
async fn iv_mismatch_edge_gap_gate_overrides_positive_edge_and_strong_consensus() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let _cex_guard =
        crate::trade_flow::guards::cex_microstructure::lock_cex_microstructure_test_state();
    clear_binance_price_test_state();
    clear_cex_microstructure_test_state();
    seed_ticks_for_asset_with_interval(
        "btc",
        &[
            63_040.0, 63_052.0, 63_061.0, 63_072.0, 63_083.0, 63_094.0, 63_104.0, 63_110.95,
        ],
        500,
        5_000,
    );
    let market_slug = active_market_slug(90);
    seed_cex_open_gap_for_market(&market_slug, 63_086.0, 63_110.95);
    let now_ms = Utc::now().timestamp_millis();
    seed_binance_price_test_ticks("btc", &[(now_ms - 300, 63_116.0)]).expect("seed binance ticks");
    let mut config = config_with_time_rule(0.59, 0.60, Some(0.77), 0.0075, 1.90);
    config.time_rules[0].min_expected_move_usd = Some(41.22690899);
    config.cex_open_gap.enabled = true;
    config.cex_open_gap.decision_gap_enabled = true;
    config.cex_open_gap.apply_negative_conservative_cap = true;
    config.oracle_tick_jump.enabled = true;
    config.depth_guard_enabled = true;
    config.depth_max_slippage = 1.0;
    config.depth_intended_qty = Some(8.33);
    config.depth_order_book = Some(OrderBookSnapshot {
        bids: vec![OrderBookLevel {
            price: 0.59,
            size: 10.0,
        }],
        asks: vec![OrderBookLevel {
            price: 0.60,
            size: 10.0,
        }],
    });

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &market_slug,
        "Up",
        "btc",
        63_110.95,
        63_063.81492970,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_gap_strength_below_threshold");
    assert_eq!(evaluation.selected_side, None);
    assert!(evaluation.edge_adjusted_decision.expect("decision edge") > 0.0075);
    assert_eq!(evaluation.binance_same_direction, Some(true));
    assert_eq!(
        evaluation.cex_open_gap.consensus.as_str(),
        "strong",
        "{:?}",
        evaluation.cex_open_gap
    );
    assert!(evaluation.cex_open_gap.clean_lane);
    assert_eq!(evaluation.gap_gate.mode, "hard_block");
    assert_eq!(evaluation.gap_gate.enforced, true);
    assert_eq!(evaluation.gap_gate.result, "fail");
    assert_eq!(
        evaluation.gap_gate.reason,
        Some("blocked_gap_strength_below_threshold")
    );
    assert!(evaluation.gap_gate.margin.expect("gap margin") < 0.0);
    assert_eq!(evaluation.gap_gate.min_margin, Some(0.0));
    let payload = evaluation.to_value();
    assert_eq!(
        payload
            .get("gap_gate_enforced")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        payload
            .get("edge_cost_source")
            .and_then(serde_json::Value::as_str),
        Some("executable_all_in_cost")
    );
}

#[tokio::test]
async fn iv_mismatch_edge_blocks_ptb_chop_volatility() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks_with_interval(
        &[140.0, 60.0, 145.0, 55.0, 150.0, 50.0, 155.0, 150.0],
        500,
        1_000,
    );
    let mut config = config_with_time_rule(0.42, 0.44, Some(0.65), 0.06, 0.0);
    config.ptb_chop.enabled = true;
    config.ptb_chop.deadband_bps = 0.0;
    config.ptb_chop.deadband_min_usd_btc = 0.1;
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        150.0,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_ptb_chop_volatility");
    assert_eq!(
        evaluation.ptb_chop.block_reason,
        Some("blocked_ptb_chop_volatility")
    );
    assert!(evaluation.ptb_chop.zero_cross_count_10s.unwrap_or_default() >= 2);
    assert!(evaluation
        .all_reasons
        .contains(&"blocked_ptb_chop_volatility"));
    let payload = evaluation.to_value();
    assert_eq!(
        payload
            .get("ptb_chop_action")
            .and_then(serde_json::Value::as_str),
        Some("block")
    );
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
async fn iv_mismatch_edge_applies_entry_quality_bps_expected_move_floor() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[
            60_029.5, 60_030.0, 60_029.8, 60_030.2, 60_029.9, 60_030.1, 60_029.8, 60_030.0,
        ],
        500,
    );
    let mut config = config_with_time_rule(0.42, 0.44, Some(0.65), 0.06, 0.0);
    config.entry_quality.enabled = true;
    config.entry_quality.min_expected_move_bps = 2.0;
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        60_030.0,
        60_000.0,
        config,
    );

    let floor = evaluation.expected_move_floor.expect("expected_move_floor");
    let effective = evaluation.expected_move_eff.expect("expected_move_eff");
    assert!((floor - 12.006).abs() < 1e-9, "{floor}");
    assert!((effective - 12.006).abs() < 1e-9, "{effective}");
}

#[tokio::test]
async fn iv_mismatch_edge_applies_adaptive_expected_move_floor() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[
            60_000.0, 60_002.0, 60_006.0, 60_010.0, 60_015.0, 60_020.0, 60_025.0, 60_030.0,
        ],
        2_500,
    );
    let mut config = config_with_time_rule(0.42, 0.46, Some(0.65), 0.06, 0.0);
    config.expected_move_floor.mode = PriceToBeatIvMinExpectedMoveMode::Adaptive;
    config.expected_move_floor.base_bps = 2.0;
    config.expected_move_floor.min_bps = 1.5;
    config.expected_move_floor.max_bps = 5.0;
    config.expected_move_floor.wide_spread_add_bps = 1.0;
    config.expected_move_floor.stale_add_bps = 1.0;
    config.max_spread = 0.05;

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        60_030.0,
        60_000.0,
        config,
    );

    assert_eq!(
        evaluation.expected_move_floor_debug.mode,
        PriceToBeatIvMinExpectedMoveMode::Adaptive
    );
    assert_eq!(
        evaluation.expected_move_floor_debug.effective_bps,
        Some(4.0)
    );
    let floor = evaluation.expected_move_floor.expect("expected_move_floor");
    assert!((floor - 24.012).abs() < 1e-9, "{floor}");
    assert!(evaluation.expected_move_eff.expect("expected_move_eff") >= floor);
    assert!(evaluation
        .expected_move_floor_debug
        .adjustments
        .contains(&"spread_wide"));
    assert!(evaluation
        .expected_move_floor_debug
        .adjustments
        .contains(&"stale"));
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
        legacy_iv_edge_config(market(0.42, 0.44)),
    );

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason, "chainlink_provider_stale_global");
}

#[test]
fn iv_mismatch_edge_chainlink_stale_boundary_is_strictly_greater_than() {
    assert!(!is_chainlink_stale(3_499, 3_500));
    assert!(!is_chainlink_stale(3_500, 3_500));
    assert!(is_chainlink_stale(3_501, 3_500));
    assert!(!is_chainlink_stale(9_100, 9_100));
    assert!(is_chainlink_stale(9_101, 9_100));
}

#[tokio::test]
async fn iv_mismatch_edge_chainlink_override_allows_3200ms_age() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        3_200,
    );
    let mut config = legacy_iv_edge_config(market(0.42, 0.44));
    config.chainlink_stale_ms = 3_500;
    config.chainlink_stale_override_source = "node_config";
    config.entry_quality_chainlink_max_age_ms = Some(3_500);

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    assert_ne!(evaluation.reason, "blocked_rtds_stale", "{evaluation:?}");
    assert_eq!(
        evaluation.chainlink_stale_tolerance_result,
        Some("pass_stale_tolerance")
    );
    assert_eq!(evaluation.chainlink_stale_ms_effective, 3_500);
    assert_eq!(
        evaluation.entry_quality_chainlink_max_age_ms_effective,
        Some(3_500)
    );
    assert_eq!(evaluation.chainlink_stale_override_source, "node_config");
}

#[tokio::test]
async fn iv_mismatch_edge_chainlink_override_allows_3500ms_age() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    // Exact 3500ms strict-boundary coverage is in the pure stale helper test; leave
    // 1ms of headroom here so Utc::now() drift cannot make this integration flaky.
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        3_499,
    );
    let mut config = legacy_iv_edge_config(market(0.42, 0.44));
    config.chainlink_stale_ms = 3_500;
    config.chainlink_stale_override_source = "node_config";
    config.entry_quality_chainlink_max_age_ms = Some(3_500);

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    assert_ne!(
        evaluation.reason, "chainlink_provider_stale_global",
        "{evaluation:?}"
    );
    assert_eq!(
        evaluation.chainlink_stale_tolerance_result,
        Some("pass_stale_tolerance")
    );
}

#[tokio::test]
async fn iv_mismatch_edge_chainlink_override_blocks_above_3500ms() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        3_501,
    );
    let mut config = legacy_iv_edge_config(market(0.42, 0.44));
    config.chainlink_stale_ms = 3_500;
    config.chainlink_stale_override_source = "node_config";
    config.entry_quality_chainlink_max_age_ms = Some(3_500);

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason, "chainlink_provider_stale_global");
    assert_eq!(
        evaluation.chainlink_stale_tolerance_result,
        Some("blocked_chainlink_stale")
    );
}

#[tokio::test]
async fn iv_mismatch_edge_chainlink_override_still_blocks_real_provider_gaps() {
    for stale_age_ms in [9_100, 20_900] {
        let _guard = IV_MISMATCH_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        clear_binance_price_test_state();
        seed_ticks(
            &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
            stale_age_ms,
        );
        let mut config = legacy_iv_edge_config(market(0.42, 0.44));
        config.vol_window_secs = 120;
        config.chainlink_stale_ms = 3_500;
        config.chainlink_stale_override_source = "node_config";
        config.entry_quality_chainlink_max_age_ms = Some(3_500);

        let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
            &active_market_slug(60),
            "Up",
            "btc",
            101.8,
            100.0,
            config,
        );

        assert!(!evaluation.passed, "{evaluation:?}");
        assert_eq!(evaluation.reason, "chainlink_provider_stale_global");
    }
}

#[tokio::test]
async fn iv_mismatch_edge_stale_exception_uses_entry_quality_gap() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks_with_interval(
        &[
            100.00, 100.01, 100.00, 100.01, 100.00, 100.01, 100.00, 100.01,
        ],
        3_200,
        1_000,
    );
    let mut config = legacy_iv_edge_config(market(0.42, 0.44));
    config.chainlink_stale_strong_gap_exception.enabled = true;
    config.chainlink_stale_strong_gap_context = Some(
        super::super::iv_chainlink_stale_strong_gap_exception::ChainlinkStaleStrongGapRuntimeContext {
            cex_confirmed: true,
            anchor_venue: Some("bybit".to_string()),
            anchor_hit: true,
            bybit_hit: true,
            secondary_confirmed: true,
            secondary_sources: vec!["binance".to_string()],
            cex_clean: Some(true),
            cex_direction: Some("clean".to_string()),
        },
    );

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        102.0,
        100.0,
        config,
    );

    assert_ne!(evaluation.reason, "blocked_rtds_stale", "{evaluation:?}");
    assert!(
        evaluation.chainlink_stale_exception_passed,
        "{evaluation:?}"
    );
    let decision = evaluation
        .chainlink_stale_strong_gap_exception
        .as_ref()
        .expect("stale exception decision");
    assert_eq!(
        decision.result,
        "passed_chainlink_stale_strong_gap_exception"
    );
    assert_eq!(decision.gap_source, "entry_quality");
    assert_eq!(decision.iv_gap_strength, None);
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
        legacy_iv_edge_config(market(0.79, 0.80)),
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_falling_knife_drop");
    assert!(evaluation.edge.expect("raw edge") > 0.06);
    assert!(evaluation.drop_z.expect("drop_z") > 1.0);
}

#[tokio::test]
async fn iv_mismatch_edge_blocks_rising_knife_spike() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks_with_interval(
        &[
            100.0, 100.1, 99.9, 100.0, 100.1, 100.0, 100.1, 99.9, 100.0, 100.1, 100.0, 100.1, 99.9,
            100.0, 100.1, 100.0, 100.1, 99.9, 100.0, 100.1, 100.0, 100.1, 99.9, 100.0, 100.1,
            100.0, 99.9, 103.0, 106.0, 109.0, 112.0,
        ],
        500,
        1_000,
    );
    let mut config = legacy_iv_edge_config(market(0.79, 0.80));
    config.rising_knife_drop_z = 2.0;
    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        112.0,
        99.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(
        evaluation.reason, "blocked_rising_knife_spike",
        "{evaluation:?}"
    );
    assert!(evaluation.drop_z.expect("drop_z") < -2.0);
}

#[tokio::test]
async fn iv_mismatch_edge_high_price_early_gap_uses_gap_gate_reason() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        2_190,
    );
    let mut config = config_with_time_rule(0.77, 0.78, None, 0.0, 20.0);
    config.time_rules[0].start_remaining_secs = 240.0;
    config.time_rules[0].end_remaining_secs = 120.0;
    config.high_price_early_reversal.enabled = true;
    config.high_price_early_reversal.q_extreme_require_binance_q = false;
    config
        .high_price_early_reversal
        .q_extreme_require_clean_strong_cex = false;
    config.high_price_early_reversal.q_extreme_max_stale_ms = 10_000;
    config.high_price_early_reversal.q_extreme_min_gap_strength = 0.0;

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(160),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_high_price_early_reversal_gap");
    assert!(evaluation.high_price_early_reversal.applies);
    assert_eq!(
        evaluation.high_price_early_reversal.stale_gap_add_applied,
        0.30
    );
    assert_eq!(
        evaluation
            .high_price_early_reversal
            .binance_missing_gap_add_applied,
        0.35
    );
    assert!(evaluation
        .all_reasons
        .contains(&"gap_below_effective_required"));
}

#[test]
fn required_gap_cap_applied_when_gap_exceeds_cap() {
    // gap=5.0, cap=1.0 → cap devreye girer: (1.0, Some(1.0), true)
    let (gap, applied, capped) = apply_required_gap_cap(5.0, Some(1.0));
    assert!((gap - 1.0).abs() < 1e-9);
    assert_eq!(applied, Some(1.0));
    assert!(capped);
}

#[test]
fn required_gap_cap_not_applied_when_gap_below_cap() {
    // gap=0.5, cap=1.0 → gap korunur: (0.5, Some(1.0), false)
    let (gap, applied, capped) = apply_required_gap_cap(0.5, Some(1.0));
    assert!((gap - 0.5).abs() < 1e-9);
    assert_eq!(applied, Some(1.0));
    assert!(!capped);
}

#[test]
fn required_gap_cap_disabled_when_none() {
    // cap=None → gap korunur, applied=None, capped=false
    let (gap, applied, capped) = apply_required_gap_cap(5.0, None);
    assert!((gap - 5.0).abs() < 1e-9);
    assert_eq!(applied, None);
    assert!(!capped);
}

#[tokio::test]
async fn iv_mismatch_edge_required_gap_usd_uncapped_when_cap_none() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(
        &[100.0, 100.2, 100.5, 100.6, 100.9, 101.1, 101.4, 101.8],
        500,
    );
    // Cap None (default) → cap devre disi, telemetri de None olmali.
    let config = legacy_iv_edge_config(market(0.42, 0.44));

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(60),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    assert_eq!(evaluation.required_gap_usd_cap, None);
    assert_eq!(evaluation.required_gap_usd_capped, Some(false));
}
