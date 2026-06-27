use super::*;
use crate::trade_flow::guards::binance_price::{
    clear_binance_price_test_state, seed_binance_price_test_ticks,
};
use crate::trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks;
use bot_infra::exchange::{OrderBookLevel, OrderBookSnapshot};

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

fn config(best_bid: f64, best_ask: f64) -> PriceToBeatIvMismatchEdgeConfig {
    let mut config = PriceToBeatIvMismatchEdgeConfig::crypto_defaults(market(best_bid, best_ask));
    config.cex_open_gap.decision_gap_enabled = false;
    config.oracle_tick_jump.enabled = false;
    config.time_rules = vec![PriceToBeatIvMismatchTimeRule {
        start_remaining_secs: 120.0,
        end_remaining_secs: 10.0,
        max_price: Some(0.80),
        min_edge: 0.01,
        min_gap_strength: 0.0,
        min_expected_move_usd: None,
        min_gap_strength_margin: None,
        min_gap_usd_margin: None,
    }];
    config.protection_mode = PriceToBeatIvProtectionMode::Hard;
    config.depth_guard_enabled = false;
    config
}

fn opposite_book_quotes() -> PriceToBeatIvBookQuotes {
    PriceToBeatIvBookQuotes {
        up_bid: Some(0.75),
        up_ask: Some(0.77),
        down_bid: Some(0.24),
        down_ask: Some(0.26),
    }
}

#[tokio::test]
async fn iv_mismatch_protection_blocks_opposite_book_lead() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[98.25, 98.2, 98.22, 98.18, 98.21, 98.2, 98.19, 98.2]);
    let mut config = config(0.24, 0.26);
    config.book_lead_guard_enabled = true;
    config.book_quotes = Some(opposite_book_quotes());

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(80),
        "Down",
        "btc",
        98.2,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_book_leads_opposite");
    assert_eq!(evaluation.protection_result, Some("block"));
    assert_eq!(evaluation.book_side, Some("up"));
    assert!(evaluation
        .protection_reasons
        .contains(&"blocked_book_leads_opposite"));
}

#[tokio::test]
async fn iv_mismatch_protection_allows_neutral_book_when_gap_threshold_is_not_hit() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[98.25, 98.2, 98.22, 98.18, 98.21, 98.2, 98.19, 98.2]);
    let mut config = config(0.44, 0.46);
    config.book_lead_guard_enabled = true;
    config.model_book_gap_warn = Some(0.90);
    config.too_good_to_be_true_gap = Some(0.90);
    config.book_quotes = Some(PriceToBeatIvBookQuotes {
        up_bid: Some(0.54),
        up_ask: Some(0.56),
        down_bid: Some(0.44),
        down_ask: Some(0.46),
    });

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(80),
        "Down",
        "btc",
        98.2,
        100.0,
        config,
    );

    assert!(evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.book_side, Some("neutral"));
    assert_eq!(evaluation.protection_result, Some("pass"));
}

#[tokio::test]
async fn iv_mismatch_protection_blocks_missing_binance_under_required_window() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[100.0, 100.2, 100.5, 100.8, 101.0, 101.2, 101.5, 101.8]);
    let mut config = config(0.42, 0.44);
    config.require_binance_fresh_under_sec = Some(60.0);

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(50),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_binance_required");
    assert!(evaluation
        .protection_reasons
        .contains(&"blocked_binance_required"));
}

#[tokio::test]
async fn iv_mismatch_protection_blocks_binance_direction_mismatch() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[98.25, 98.2, 98.22, 98.18, 98.21, 98.2, 98.19, 98.2]);
    let now_ms = Utc::now().timestamp_millis();
    seed_binance_price_test_ticks("btc", &[(now_ms - 300, 101.0)]).expect("seed binance ticks");
    let mut config = config(0.09, 0.10);
    config.require_binance_fresh_under_sec = Some(60.0);
    config.require_binance_same_direction = true;
    config.binance_q_buffer = 0.50;

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(50),
        "Down",
        "btc",
        98.2,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_binance_direction_mismatch");
    assert_eq!(evaluation.binance_same_direction, Some(false));
}

#[tokio::test]
async fn iv_mismatch_protection_blocks_thin_gap_usd_margin() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[100.0, 100.2, 100.5, 100.8, 101.0, 101.2, 101.5, 101.8]);
    let mut config = config(0.42, 0.44);
    config.time_rules[0].min_gap_usd_margin = Some(3.0);

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(80),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_thin_gap_usd_margin");
    assert_eq!(evaluation.min_gap_usd_margin, Some(3.0));
    assert!(evaluation.gap_usd_margin.expect("gap_usd_margin") < 3.0);
}

#[tokio::test]
async fn iv_mismatch_protection_blocks_too_good_to_be_true() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[98.25, 98.2, 98.22, 98.18, 98.21, 98.2, 98.19, 98.2]);
    let mut config = config(0.24, 0.26);
    config.book_lead_guard_enabled = true;
    config.block_on_opposite_book_lead = false;
    config.opposite_mid_block = None;
    config.book_quotes = Some(opposite_book_quotes());
    config.too_good_to_be_true_gap = Some(0.35);

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(80),
        "Down",
        "btc",
        98.2,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_model_book_not_confirmed");
    assert!(evaluation.model_book_gap.expect("model_book_gap") >= 0.35);
}

#[tokio::test]
async fn iv_mismatch_protection_blocks_neutral_book_model_gap() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[98.25, 98.2, 98.22, 98.18, 98.21, 98.2, 98.19, 98.2]);
    let mut config = config(0.49, 0.50);
    config.book_lead_guard_enabled = true;
    config.book_quotes = Some(PriceToBeatIvBookQuotes {
        up_bid: Some(0.50),
        up_ask: Some(0.51),
        down_bid: Some(0.49),
        down_ask: Some(0.50),
    });
    config.too_good_to_be_true_gap = Some(0.30);

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(80),
        "Down",
        "btc",
        98.2,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_model_book_not_confirmed");
    assert_eq!(evaluation.book_side, Some("neutral"));
    assert!(evaluation.model_book_gap.expect("model_book_gap") >= 0.30);
}

#[tokio::test]
async fn iv_mismatch_model_book_warning_adds_penalty_without_blocking() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[98.25, 98.2, 98.22, 98.18, 98.21, 98.2, 98.19, 98.2]);
    let mut config = config(0.59, 0.61);
    config.book_lead_guard_enabled = true;
    config.book_quotes = Some(PriceToBeatIvBookQuotes {
        up_bid: Some(0.39),
        up_ask: Some(0.41),
        down_bid: Some(0.59),
        down_ask: Some(0.61),
    });

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(80),
        "Down",
        "btc",
        98.2,
        100.0,
        config,
    );

    assert!(evaluation.passed, "{evaluation:?}");
    assert_eq!(
        evaluation.book_confirmation_result,
        Some("warn_model_book_not_confirmed")
    );
    assert!(evaluation
        .protection_reasons
        .contains(&"warn_model_book_not_confirmed"));
    assert_eq!(evaluation.protection_threshold_penalty, Some(0.02));
    assert_eq!(evaluation.protection_gap_strength_penalty, Some(0.05));
}

#[tokio::test]
async fn iv_mismatch_adaptive_protection_blocks_model_book_gap() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[98.25, 98.2, 98.22, 98.18, 98.21, 98.2, 98.19, 98.2]);
    let mut config = config(0.49, 0.50);
    config.protection_mode = PriceToBeatIvProtectionMode::Adaptive;
    config.book_lead_guard_enabled = true;
    config.book_quotes = Some(PriceToBeatIvBookQuotes {
        up_bid: Some(0.50),
        up_ask: Some(0.51),
        down_bid: Some(0.49),
        down_ask: Some(0.50),
    });
    config.too_good_to_be_true_gap = Some(0.30);

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(80),
        "Down",
        "btc",
        98.2,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_model_book_not_confirmed");
    assert_eq!(evaluation.protection_result, Some("block"));
}

#[tokio::test]
async fn iv_mismatch_model_book_gap_blocks_outside_book_lead_window() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[98.25, 98.2, 98.22, 98.18, 98.21, 98.2, 98.19, 98.2]);
    let mut config = config(0.49, 0.50);
    config.time_rules[0].start_remaining_secs = 180.0;
    config.book_lead_guard_enabled = true;
    config.book_lead_under_sec = 120.0;
    config.book_quotes = Some(PriceToBeatIvBookQuotes {
        up_bid: Some(0.50),
        up_ask: Some(0.51),
        down_bid: Some(0.49),
        down_ask: Some(0.50),
    });
    config.too_good_to_be_true_gap = Some(0.30);

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(150),
        "Down",
        "btc",
        98.2,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_model_book_not_confirmed");
    assert!(evaluation.model_book_gap.expect("model_book_gap") >= 0.30);
}

#[tokio::test]
async fn iv_mismatch_late_high_price_compound_risk_blocks() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[100.0, 100.2, 100.5, 100.8, 101.0, 101.2, 101.5, 101.8]);
    let mut config = config(0.64, 0.66);
    config.book_quotes = Some(PriceToBeatIvBookQuotes {
        up_bid: Some(0.54),
        up_ask: Some(0.66),
        down_bid: Some(0.34),
        down_ask: Some(0.46),
    });

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(50),
        "Up",
        "btc",
        101.8,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_late_high_price_unconfirmed");
    assert!(evaluation
        .protection_reasons
        .contains(&"warn_late_high_price_unconfirmed"));
}

#[tokio::test]
async fn iv_mismatch_depth_guard_uses_vwap_cost_and_defers_slippage_only_block() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[98.25, 98.2, 98.22, 98.18, 98.21, 98.2, 98.19, 98.2]);
    let mut config = config(0.49, 0.50);
    config.depth_guard_enabled = true;
    config.depth_guard_hard_block_enabled = true;
    config.depth_intended_qty = Some(5.0);
    config.depth_order_book = Some(OrderBookSnapshot {
        bids: vec![OrderBookLevel {
            price: 0.49,
            size: 10.0,
        }],
        asks: vec![
            OrderBookLevel {
                price: 0.50,
                size: 1.0,
            },
            OrderBookLevel {
                price: 0.7125,
                size: 4.0,
            },
        ],
    });

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(80),
        "Down",
        "btc",
        98.2,
        100.0,
        config,
    );

    assert!(evaluation.passed, "{evaluation:?}");
    assert_eq!(
        evaluation.depth.block_reason,
        Some("blocked_depth_slippage_too_high")
    );
    assert_eq!(evaluation.depth.block_kind, Some("slippage_too_high"));
    assert!(evaluation.depth.slippage_deferred_to_execution_vwap);
    assert!(!evaluation.depth.slippage_hard_blocked);
    assert_eq!(evaluation.depth.estimated_avg_fill, Some(0.67));
    assert!(evaluation.depth.vwap_slippage.expect("slippage") > 0.03);
    assert_eq!(evaluation.depth.depth_levels_used, Some(2));
    assert!(evaluation.cost.expect("cost") > 0.69);
}

#[tokio::test]
async fn iv_mismatch_depth_guard_hard_block_still_blocks_insufficient_qty() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[98.25, 98.2, 98.22, 98.18, 98.21, 98.2, 98.19, 98.2]);
    let mut config = config(0.49, 0.50);
    config.protection_mode = PriceToBeatIvProtectionMode::Off;
    config.depth_guard_enabled = true;
    config.depth_guard_hard_block_enabled = true;
    config.depth_intended_qty = Some(5.0);
    config.depth_order_book = Some(OrderBookSnapshot {
        bids: vec![OrderBookLevel {
            price: 0.49,
            size: 10.0,
        }],
        asks: vec![OrderBookLevel {
            price: 0.50,
            size: 1.0,
        }],
    });

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(80),
        "Down",
        "btc",
        98.2,
        100.0,
        config,
    );

    assert!(!evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.reason, "blocked_depth_qty_insufficient");
    assert_eq!(evaluation.protection_result, Some("off"));
    assert_eq!(
        evaluation.depth.block_reason,
        Some("blocked_depth_qty_insufficient")
    );
    assert_eq!(evaluation.depth.block_kind, Some("qty_insufficient"));
    assert_eq!(evaluation.depth.depth_levels_used, Some(1));
    assert_eq!(evaluation.depth.visible_ask_qty, Some(1.0));
}

#[tokio::test]
async fn iv_mismatch_depth_guard_hard_block_flag_false_preserves_protection_off() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[98.25, 98.2, 98.22, 98.18, 98.21, 98.2, 98.19, 98.2]);
    let mut config = config(0.49, 0.50);
    config.protection_mode = PriceToBeatIvProtectionMode::Off;
    config.depth_guard_enabled = true;
    config.depth_guard_hard_block_enabled = false;
    config.depth_intended_qty = Some(5.0);
    config.depth_order_book = Some(OrderBookSnapshot {
        bids: vec![OrderBookLevel {
            price: 0.49,
            size: 10.0,
        }],
        asks: vec![
            OrderBookLevel {
                price: 0.50,
                size: 1.0,
            },
            OrderBookLevel {
                price: 0.7125,
                size: 4.0,
            },
        ],
    });

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(80),
        "Down",
        "btc",
        98.2,
        100.0,
        config,
    );

    assert!(evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.protection_result, Some("off"));
    assert_eq!(
        evaluation.depth.block_reason,
        Some("blocked_depth_slippage_too_high")
    );
    assert_eq!(evaluation.depth.block_kind, Some("slippage_too_high"));
    assert!(!evaluation.depth.slippage_deferred_to_execution_vwap);
}

#[test]
fn iv_mismatch_depth_guard_allows_exact_slippage_boundary() {
    let order_book = OrderBookSnapshot {
        bids: Vec::new(),
        asks: vec![
            OrderBookLevel {
                price: 0.50,
                size: 1.0,
            },
            OrderBookLevel {
                price: 0.5375,
                size: 4.0,
            },
        ],
    };

    let evaluation = super::super::iv_mismatch_depth::evaluate_price_to_beat_iv_depth(
        Some(&order_book),
        0.50,
        Some(5.0),
        0.03,
        true,
    );

    assert_eq!(evaluation.result, "pass");
    assert_eq!(evaluation.block_reason, None);
    assert!((evaluation.vwap_slippage.expect("slippage") - 0.03).abs() < 0.000001);
}

#[test]
fn iv_mismatch_depth_guard_zero_slippage_when_book_best_ask_matches_fill() {
    let order_book = OrderBookSnapshot {
        bids: Vec::new(),
        asks: vec![OrderBookLevel {
            price: 0.53,
            size: 5.0,
        }],
    };

    let evaluation = super::super::iv_mismatch_depth::evaluate_price_to_beat_iv_depth(
        Some(&order_book),
        0.50,
        Some(5.0),
        0.03,
        true,
    );

    assert_eq!(evaluation.result, "pass");
    assert_eq!(evaluation.block_reason, None);
    assert!((evaluation.vwap_slippage.expect("slippage") - 0.0).abs() < 0.000001);
}

#[tokio::test]
async fn iv_mismatch_protection_soft_mode_adds_penalties() {
    let _guard = IV_MISMATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    clear_binance_price_test_state();
    seed_ticks(&[98.25, 98.2, 98.22, 98.18, 98.21, 98.2, 98.19, 98.2]);
    let mut config = config(0.24, 0.26);
    config.protection_mode = PriceToBeatIvProtectionMode::Soft;
    config.book_lead_guard_enabled = true;
    config.book_quotes = Some(opposite_book_quotes());

    let evaluation = evaluate_price_to_beat_iv_mismatch_edge(
        &active_market_slug(80),
        "Down",
        "btc",
        98.2,
        100.0,
        config,
    );

    assert!(evaluation.passed, "{evaluation:?}");
    assert_eq!(evaluation.protection_result, Some("soft_penalty"));
    assert!(
        evaluation
            .protection_threshold_penalty
            .expect("threshold penalty")
            > 0.0
    );
    assert!(
        evaluation
            .protection_gap_strength_penalty
            .expect("gap penalty")
            > 0.0
    );
}
