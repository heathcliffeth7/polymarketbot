use super::trade_builder_ptb_stop_loss_tests::{
    current_5m_market_slug, lock_ptb_stop_loss_test_state, seed_ptb_stop_loss_cex_window_price,
    seed_ptb_stop_loss_current_price, test_ptb_stop_loss_order,
};
use super::*;
use crate::trade_flow::guards::cex_microstructure::{
    clear_cex_microstructure_test_state, get_cex_venue_delta_snapshot, seed_cex_book_test_sample,
    seed_cex_open_test_sample, CexBookSample, CexVenue,
};
use chrono::Utc;

const REPLAY_MARKET_SLUG: &str = "btc-updown-5m-1780270800";
const REPLAY_WINDOW_START_MS: i64 = 1_780_270_800_000;
const REPLAY_PTB: f64 = 73_457.58743974549;

fn seed_replay_rest_open(venue: CexVenue, open_mid: f64) {
    seed_cex_open_test_sample(CexBookSample {
        venue,
        asset: "btc".to_string(),
        timestamp_ms: REPLAY_WINDOW_START_MS,
        bid: open_mid - 0.5,
        ask: open_mid + 0.5,
        bid_size: Some(1.0),
        ask_size: Some(1.0),
        source: "rest_open",
    });
}

fn seed_replay_current(venue: CexVenue, current_mid: f64) {
    let now_ms = Utc::now().timestamp_millis();
    seed_cex_book_test_sample(CexBookSample {
        venue,
        asset: "btc".to_string(),
        timestamp_ms: now_ms,
        bid: current_mid - 0.5,
        ask: current_mid + 0.5,
        bid_size: Some(1.0),
        ask_size: Some(1.0),
        source: "ticker",
    });
}

fn replay_down_order() -> TradeBuilderOrder {
    let mut order = test_ptb_stop_loss_order(REPLAY_MARKET_SLUG, "Down", 1.0, Some(REPLAY_PTB));
    order.ptb_current_price_source = "cex_consensus".to_string();
    order.ptb_stop_loss_time_decay_mode = Some("none".to_string());
    order
}

fn assert_option_f64_close(actual: Option<f64>, expected: f64) {
    let actual = actual.expect("expected numeric value");
    assert!(
        (actual - expected).abs() <= 0.01,
        "expected {expected}, got {actual}"
    );
}

#[tokio::test]
async fn cex_consensus_triggers_from_chainlink_when_cex_missing() {
    let _guard = lock_ptb_stop_loss_test_state();
    clear_cex_microstructure_test_state();
    seed_ptb_stop_loss_current_price("btc", 90.0);
    let (market_slug, _) = current_5m_market_slug("btc");
    let mut order = test_ptb_stop_loss_order(&market_slug, "Up", -10.0, Some(100.0));
    order.ptb_current_price_source = "cex_consensus".to_string();

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

    assert!(evaluation.should_trigger);
    assert_eq!(evaluation.current_price_source, "chainlink_live_data_ws");
    assert_eq!(evaluation.current_price, Some(90.0));
    assert_eq!(evaluation.source_evaluations.len(), 2);
    assert_eq!(
        evaluation.source_evaluations[0].reason_code,
        "chainlink_threshold_hit"
    );
    assert_eq!(
        evaluation.source_evaluations[1].reason_code,
        "threshold_not_met"
    );
}

#[tokio::test]
async fn cex_consensus_payload_keeps_chainlink_and_cex_when_both_hit() {
    let _guard = lock_ptb_stop_loss_test_state();
    clear_cex_microstructure_test_state();
    seed_ptb_stop_loss_current_price("btc", 90.0);
    let (market_slug, start_ms) = current_5m_market_slug("btc");
    let now_ms = Utc::now().timestamp_millis();
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Bybit, start_ms, 100.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Bybit, now_ms, 90.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Binance, start_ms, 200.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Binance, now_ms, 190.0);
    let mut order = test_ptb_stop_loss_order(&market_slug, "Up", -10.0, Some(100.0));
    order.ptb_current_price_source = "cex_consensus".to_string();

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

    assert!(evaluation.should_trigger);
    assert_eq!(evaluation.current_price_source, "chainlink_live_data_ws");
    assert_eq!(evaluation.source_evaluations.len(), 2);
    assert_eq!(
        evaluation.source_evaluations[0].reason_code,
        "chainlink_threshold_hit"
    );
    assert_eq!(
        evaluation.source_evaluations[1].reason_code,
        "cex_consensus_threshold_hit"
    );
}

#[test]
fn cex_consensus_rejects_ws_only_bybit_open_for_delta() {
    let _guard = lock_ptb_stop_loss_test_state();
    clear_cex_microstructure_test_state();
    seed_cex_book_test_sample(CexBookSample {
        venue: CexVenue::Bybit,
        asset: "btc".to_string(),
        timestamp_ms: REPLAY_WINDOW_START_MS - 42,
        bid: 73_570.05,
        ask: 73_571.05,
        bid_size: Some(1.0),
        ask_size: Some(1.0),
        source: "ticker",
    });
    seed_replay_current(CexVenue::Bybit, 73_581.95);

    let error = get_cex_venue_delta_snapshot("btc", CexVenue::Bybit, REPLAY_WINDOW_START_MS, 1.0, 2_500)
        .expect_err("ws-only bybit open must not be used");

    assert!(error.to_string().contains("window open book missing"));
}

#[tokio::test]
async fn ptb_stop_loss_1780270800_down_gap_2_5_does_not_trigger() {
    let _guard = lock_ptb_stop_loss_test_state();
    clear_cex_microstructure_test_state();
    seed_ptb_stop_loss_current_price("btc", REPLAY_PTB - 2.5);
    seed_replay_rest_open(CexVenue::Bybit, 73_570.55);
    seed_replay_rest_open(CexVenue::Binance, 73_561.99);
    seed_replay_rest_open(CexVenue::Coinbase, 73_459.99);
    seed_replay_current(CexVenue::Bybit, 73_568.05);
    seed_replay_current(CexVenue::Binance, 73_564.0);
    seed_replay_current(CexVenue::Coinbase, 73_462.0);

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&replay_down_order()).expect("ptb eval");

    assert!(!evaluation.should_trigger);
    assert_eq!(evaluation.reason_code, "ptb_gap_threshold_not_met");
    assert_option_f64_close(evaluation.directional_gap, 2.5);
}

#[tokio::test]
async fn ptb_stop_loss_1780270800_down_threshold_boundary_1_5_does_not_trigger() {
    let _guard = lock_ptb_stop_loss_test_state();
    clear_cex_microstructure_test_state();
    let bybit_open = 73_550.0;
    seed_ptb_stop_loss_current_price("btc", REPLAY_PTB - 2.0);
    seed_replay_rest_open(CexVenue::Bybit, bybit_open);
    seed_replay_rest_open(CexVenue::Binance, bybit_open);
    seed_replay_rest_open(CexVenue::Coinbase, bybit_open);
    seed_replay_current(CexVenue::Bybit, bybit_open - 1.5);
    seed_replay_current(CexVenue::Binance, bybit_open - 1.5);
    seed_replay_current(CexVenue::Coinbase, bybit_open - 1.5);

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&replay_down_order()).expect("ptb eval");

    assert!(!evaluation.should_trigger);
    let cex_eval = &evaluation.source_evaluations[1];
    assert_option_f64_close(cex_eval.directional_gap, 1.5);
}

#[tokio::test]
async fn ptb_stop_loss_1780270800_down_adverse_flip_triggers() {
    let _guard = lock_ptb_stop_loss_test_state();
    clear_cex_microstructure_test_state();
    seed_ptb_stop_loss_current_price("btc", REPLAY_PTB - 2.33);
    let bybit_open = 73_550.0;
    seed_replay_rest_open(CexVenue::Bybit, bybit_open);
    seed_replay_rest_open(CexVenue::Binance, bybit_open);
    seed_replay_rest_open(CexVenue::Coinbase, bybit_open);
    seed_replay_current(CexVenue::Bybit, 73_567.5);
    seed_replay_current(CexVenue::Binance, 73_569.965);
    seed_replay_current(CexVenue::Coinbase, 73_470.245);

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&replay_down_order()).expect("ptb eval");

    assert!(evaluation.should_trigger);
    assert_eq!(evaluation.current_price_source, "cex_consensus_bybit_plus_one");
    assert_option_f64_close(evaluation.directional_gap, -17.5);
    assert_eq!(
        evaluation.source_evaluations[1].reason_code,
        "cex_consensus_threshold_hit"
    );
}

#[tokio::test]
async fn ptb_stop_loss_1780270800_down_threshold_cross_0_5_triggers() {
    let _guard = lock_ptb_stop_loss_test_state();
    clear_cex_microstructure_test_state();
    let bybit_open = 73_550.0;
    seed_ptb_stop_loss_current_price("btc", REPLAY_PTB - 2.0);
    seed_replay_rest_open(CexVenue::Bybit, bybit_open);
    seed_replay_rest_open(CexVenue::Binance, bybit_open);
    seed_replay_rest_open(CexVenue::Coinbase, bybit_open);
    seed_replay_current(CexVenue::Bybit, bybit_open - 0.5);
    seed_replay_current(CexVenue::Binance, bybit_open - 0.5);
    seed_replay_current(CexVenue::Coinbase, bybit_open - 0.5);

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&replay_down_order()).expect("ptb eval");

    assert!(evaluation.should_trigger);
    assert_option_f64_close(evaluation.directional_gap, 0.5);
}
