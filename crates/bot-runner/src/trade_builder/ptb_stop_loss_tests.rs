use super::*;
use crate::trade_flow::guards::cex_microstructure::{
    CexBookSample, CexVenue, clear_cex_microstructure_test_state, seed_cex_book_test_sample,
};
use chrono::Utc;
use std::sync::{Mutex, MutexGuard};

static PTB_STOP_LOSS_TEST_LOCK: Mutex<()> = Mutex::new(());

pub(crate) fn lock_ptb_stop_loss_test_state() -> MutexGuard<'static, ()> {
    PTB_STOP_LOSS_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn ptb_stop_loss_requires_supported_market_scope() {
    assert!(!trade_builder_market_supports_ptb_stop_loss(
        "nba-lal-orl-2026-03-21"
    ));
    assert!(trade_builder_market_supports_ptb_stop_loss(
        "eth-updown-5m-1774013100"
    ));
}

pub(crate) fn test_ptb_stop_loss_order(
    market_slug: &str,
    outcome_label: &str,
    gap_usd: f64,
    ptb_reference_price: Option<f64>,
) -> TradeBuilderOrder {
    TradeBuilderOrder {
        id: 1,
        trade_id: 77,
        user_id: 1,
        kind: "conditional".to_string(),
        status: "armed".to_string(),
        market_slug: market_slug.to_string(),
        token_id: "tok-up".to_string(),
        outcome_label: outcome_label.to_string(),
        side: "sell".to_string(),
        execution_mode: "market".to_string(),
        trigger_condition: Some("cross_below".to_string()),
        trigger_price: None,
        max_price: None,
        size_basis: TRADE_BUILDER_SIZE_BASIS_SHARES.to_string(),
        size_usdc: 5.0,
        target_qty: Some(5.0),
        min_price_distance_cent: 1.0,
        expires_at: None,
        eligible_after_at: None,
        eligible_before_at: None,
        max_triggers: 1,
        triggers_fired: 0,
        active_exchange_order_id: None,
        remaining_size: None,
        remaining_qty: Some(5.0),
        working_price: None,
        last_seen_price: None,
        last_error: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        parent_order_id: Some(9),
        origin_flow_definition_id: None,
        origin_flow_run_id: None,
        origin_flow_node_key: None,
        pair_session_id: None,
        pair_leg_role: None,
        tp_enabled: false,
        tp_price: None,
        tp_rules_json: Vec::new(),
        sl_enabled: false,
        sl_price: None,
        sl_rules_json: Vec::new(),
        time_exit_rules_json: Vec::new(),
        filled_qty: 0.0,
        fee_rate_bps: 0,
        trigger_latched: false,
        trigger_latched_reason: None,
        trigger_latched_at: None,
        submitted_dynamic_qty: None,
        submitted_dynamic_price: None,
        runtime_snapshot_json: None,
        fresh_submit_lease_until: None,
        guard_trigger_price: None,
        best_ask_floor_price: None,
        retry_on_trigger_guard_block: false,
        retry_on_execution_floor_guard_block: false,
        retry_on_max_price_block: false,
        ptb_stop_loss_gap_usd: Some(gap_usd),
        ptb_reference_price,
        ptb_stop_loss_rules_json: Vec::new(),
        ptb_stop_loss_time_decay_mode: Some("tighten".to_string()),
        ptb_current_price_source: "chainlink".to_string(),
        staged_sl_retry_only_dust: false,
        staged_sl_retry_dust_metric: None,
        staged_sl_retry_dust_value: None,
        staged_sl_reentry_use_sold_notional: false,
        staged_sl_reentry_only_after_all_stages: false,
        sl_trigger_price_mode: None,
        reenter_on_sl_hit: false,
        reentry_max_attempts: 0,
        reentry_trigger_node_key: None,
        notify_on_order_submitted: false,
        notify_on_fill: false,
        notify_on_order_not_filled: false,
        notify_on_trigger_guard_blocked: false,
        notify_on_execution_floor_blocked: false,
        notify_on_tp_hit: false,
        notify_on_sl_hit: false,
        notify_on_max_price_blocked: false,
        last_guard_notification_reason: None,
        exit_ladder_kind: None,
        exit_ladder_index: None,
        exit_ladder_size_pct: None,
    }
}

pub(crate) fn seed_ptb_stop_loss_current_price(asset: &str, current_chainlink_price: f64) {
    let now_ms = Utc::now().timestamp_millis();
    trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
        asset,
        &[
            (now_ms - 250, current_chainlink_price),
            (now_ms, current_chainlink_price),
        ],
    )
    .expect("seed chainlink ticks");
}

fn seed_ptb_stop_loss_cex_current_price(asset: &str, venue: CexVenue, current_price: f64) {
    let now_ms = Utc::now().timestamp_millis();
    clear_cex_microstructure_test_state();
    seed_ptb_stop_loss_cex_current_price_without_clear(asset, venue, current_price, now_ms);
}

fn seed_ptb_stop_loss_cex_current_price_without_clear(
    asset: &str,
    venue: CexVenue,
    current_price: f64,
    now_ms: i64,
) {
    seed_cex_book_test_sample(CexBookSample {
        venue,
        asset: asset.to_string(),
        timestamp_ms: now_ms,
        bid: current_price - 0.5,
        ask: current_price + 0.5,
        bid_size: Some(1.0),
        ask_size: Some(1.0),
        source: "ticker",
    });
}

pub(crate) fn seed_ptb_stop_loss_cex_window_price(
    asset: &str,
    venue: CexVenue,
    timestamp_ms: i64,
    current_price: f64,
) {
    seed_cex_book_test_sample(CexBookSample {
        venue,
        asset: asset.to_string(),
        timestamp_ms,
        bid: current_price - 0.5,
        ask: current_price + 0.5,
        bid_size: Some(1.0),
        ask_size: Some(1.0),
        source: if matches!(
            venue,
            CexVenue::Binance | CexVenue::Okx | CexVenue::Gateio | CexVenue::Coinbase
        ) && timestamp_ms % 300_000 == 0
        {
            "rest_open"
        } else {
            "ticker"
        },
    });
}

pub(crate) fn current_5m_market_slug(asset: &str) -> (String, i64) {
    let now = Utc::now().timestamp();
    let start = now - (now % 300);
    (format!("{asset}-updown-5m-{start}"), start * 1_000)
}

fn evaluate_test_ptb_stop_loss(
    market_slug: &str,
    asset: &str,
    outcome_label: &str,
    gap_usd: f64,
    ptb_reference_price: f64,
    current_chainlink_price: f64,
) -> TradeBuilderPtbStopLossEvaluation {
    let _guard = lock_ptb_stop_loss_test_state();
    seed_ptb_stop_loss_current_price(asset, current_chainlink_price);
    let order = test_ptb_stop_loss_order(
        market_slug,
        outcome_label,
        gap_usd,
        Some(ptb_reference_price),
    );

    trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval")
}

fn assert_option_f64_close(actual: Option<f64>, expected: f64) {
    let actual = actual.expect("expected numeric value");
    assert!(
        (actual - expected).abs() <= 0.000001,
        "expected {expected}, got {actual}"
    );
}

#[tokio::test]
async fn ptb_stop_loss_triggers_when_up_gap_reverts_to_zero() {
    let _guard = lock_ptb_stop_loss_test_state();
    let now_ms = Utc::now().timestamp_millis();
    trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
        "sol",
        &[(now_ms - 250, 101.0), (now_ms, 99.75)],
    )
    .expect("seed sol ticks");
    let order = test_ptb_stop_loss_order("sol-updown-5m-1774013100", "Up", 0.0, Some(100.0));

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");
    assert_eq!(evaluation.reason_code, "ptb_gap_threshold_hit");
    assert_eq!(evaluation.directional_gap, Some(-0.25));
    assert!(evaluation.should_trigger);
}

#[tokio::test]
async fn ptb_stop_loss_waits_for_negative_overshoot_gap() {
    let _guard = lock_ptb_stop_loss_test_state();
    let now_ms = Utc::now().timestamp_millis();
    trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
        "eth",
        &[(now_ms - 250, 70.0), (now_ms, 79.0)],
    )
    .expect("seed eth ticks");
    let order = test_ptb_stop_loss_order("eth-updown-5m-1774013100", "Up", -20.0, Some(100.0));

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");
    assert_eq!(evaluation.reason_code, "ptb_gap_threshold_hit");
    assert_eq!(evaluation.directional_gap, Some(-21.0));
    assert!(evaluation.should_trigger);
}

#[test]
fn ptb_stop_loss_negative_gap_for_up_triggers_only_after_price_moves_10_below_reference() {
    let market_slug = "eth-updown-5m-1774013100";
    let blocked = evaluate_test_ptb_stop_loss(market_slug, "eth", "Up", -10.0, 100.0, 90.01);
    assert_eq!(blocked.reason_code, "ptb_gap_threshold_not_met");
    assert_option_f64_close(blocked.directional_gap, -9.99);
    assert!(!blocked.should_trigger);

    let triggered = evaluate_test_ptb_stop_loss(market_slug, "eth", "Up", -10.0, 100.0, 90.0);
    assert_eq!(triggered.reason_code, "ptb_gap_threshold_hit");
    assert_option_f64_close(triggered.directional_gap, -10.0);
    assert!(triggered.should_trigger);
}

#[test]
fn ptb_stop_loss_negative_gap_for_down_triggers_only_after_price_moves_10_above_reference() {
    let market_slug = "btc-updown-5m-1774013100";
    let blocked = evaluate_test_ptb_stop_loss(market_slug, "btc", "Down", -10.0, 100.0, 109.99);
    assert_eq!(blocked.reason_code, "ptb_gap_threshold_not_met");
    assert_option_f64_close(blocked.directional_gap, -9.99);
    assert!(!blocked.should_trigger);

    let triggered = evaluate_test_ptb_stop_loss(market_slug, "btc", "Down", -10.0, 100.0, 110.0);
    assert_eq!(triggered.reason_code, "ptb_gap_threshold_hit");
    assert_option_f64_close(triggered.directional_gap, -10.0);
    assert!(triggered.should_trigger);
}

#[tokio::test]
async fn ptb_stop_loss_blocks_when_down_gap_stays_above_threshold() {
    let _guard = lock_ptb_stop_loss_test_state();
    let now_ms = Utc::now().timestamp_millis();
    trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
        "btc",
        &[(now_ms - 250, 99.0), (now_ms, 98.5)],
    )
    .expect("seed btc ticks");
    let order = test_ptb_stop_loss_order("btc-updown-5m-1774013100", "Down", 1.0, Some(100.0));

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");
    assert_eq!(evaluation.reason_code, "ptb_gap_threshold_not_met");
    assert_eq!(evaluation.directional_gap, Some(1.5));
    assert!(!evaluation.should_trigger);
}

#[test]
fn ptb_stop_loss_uses_selected_binance_current_price() {
    let _guard = lock_ptb_stop_loss_test_state();
    seed_ptb_stop_loss_current_price("btc", 140.0);
    seed_ptb_stop_loss_cex_current_price("btc", CexVenue::Binance, 90.0);
    let mut order = test_ptb_stop_loss_order("btc-updown-5m-1774013100", "Up", -10.0, Some(100.0));
    order.ptb_current_price_source = "binance".to_string();

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

    assert_eq!(evaluation.current_price_source, "binance_cex_ws_mid");
    assert_eq!(evaluation.current_price, Some(90.0));
    assert_eq!(evaluation.current_chainlink_price, None);
    assert_eq!(evaluation.directional_gap, Some(-10.0));
    assert!(evaluation.should_trigger);
}

#[test]
fn ptb_stop_loss_uses_selected_hyperliquid_current_price() {
    let _guard = lock_ptb_stop_loss_test_state();
    seed_ptb_stop_loss_current_price("btc", 140.0);
    seed_ptb_stop_loss_cex_current_price("btc", CexVenue::Hyperliquid, 90.0);
    let mut order = test_ptb_stop_loss_order("btc-updown-5m-1774013100", "Up", -10.0, Some(100.0));
    order.ptb_current_price_source = "hyperliquid".to_string();

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

    assert_eq!(evaluation.current_price_source, "hyperliquid_l2book_mid");
    assert_eq!(evaluation.current_price, Some(90.0));
    assert_eq!(evaluation.current_chainlink_price, None);
    assert_eq!(evaluation.directional_gap, Some(-10.0));
    assert!(evaluation.should_trigger);
}

#[test]
fn ptb_stop_loss_binance_hyperliquid_triggers_when_binance_hits_gap() {
    let _guard = lock_ptb_stop_loss_test_state();
    seed_ptb_stop_loss_current_price("btc", 140.0);
    clear_cex_microstructure_test_state();
    let now_ms = Utc::now().timestamp_millis();
    seed_ptb_stop_loss_cex_current_price_without_clear("btc", CexVenue::Binance, 90.0, now_ms);
    seed_ptb_stop_loss_cex_current_price_without_clear("btc", CexVenue::Hyperliquid, 95.0, now_ms);
    let mut order = test_ptb_stop_loss_order("btc-updown-5m-1774013100", "Up", -10.0, Some(100.0));
    order.ptb_current_price_source = "binance_hyperliquid".to_string();

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

    assert_eq!(evaluation.current_price_source, "binance_cex_ws_mid");
    assert_eq!(evaluation.current_price, Some(90.0));
    assert_eq!(evaluation.directional_gap, Some(-10.0));
    assert_eq!(evaluation.source_evaluations.len(), 2);
    assert!(evaluation.should_trigger);
}

#[test]
fn ptb_stop_loss_binance_hyperliquid_triggers_when_hyperliquid_hits_gap() {
    let _guard = lock_ptb_stop_loss_test_state();
    seed_ptb_stop_loss_current_price("btc", 140.0);
    clear_cex_microstructure_test_state();
    let now_ms = Utc::now().timestamp_millis();
    seed_ptb_stop_loss_cex_current_price_without_clear("btc", CexVenue::Binance, 95.0, now_ms);
    seed_ptb_stop_loss_cex_current_price_without_clear("btc", CexVenue::Hyperliquid, 90.0, now_ms);
    let mut order = test_ptb_stop_loss_order("btc-updown-5m-1774013100", "Up", -10.0, Some(100.0));
    order.ptb_current_price_source = "binance_hyperliquid".to_string();

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

    assert_eq!(evaluation.current_price_source, "hyperliquid_l2book_mid");
    assert_eq!(evaluation.current_price, Some(90.0));
    assert_eq!(evaluation.directional_gap, Some(-10.0));
    assert!(evaluation.should_trigger);
}

#[test]
fn ptb_stop_loss_binance_hyperliquid_uses_available_source_when_other_missing() {
    let _guard = lock_ptb_stop_loss_test_state();
    seed_ptb_stop_loss_current_price("btc", 140.0);
    seed_ptb_stop_loss_cex_current_price("btc", CexVenue::Binance, 90.0);
    let mut order = test_ptb_stop_loss_order("btc-updown-5m-1774013100", "Up", -10.0, Some(100.0));
    order.ptb_current_price_source = "binance_hyperliquid".to_string();

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

    assert_eq!(evaluation.current_price_source, "binance_cex_ws_mid");
    assert_eq!(evaluation.current_price, Some(90.0));
    assert_eq!(evaluation.source_evaluations.len(), 2);
    assert_eq!(
        evaluation.source_evaluations[1].error_code,
        Some("current_price_unavailable")
    );
    assert!(evaluation.should_trigger);
}

#[tokio::test]
async fn ptb_stop_loss_cex_consensus_triggers_with_bybit_and_confirmation() {
    let _guard = lock_ptb_stop_loss_test_state();
    clear_cex_microstructure_test_state();
    seed_ptb_stop_loss_current_price("btc", 120.0);
    let (market_slug, start_ms) = current_5m_market_slug("btc");
    let now_ms = Utc::now().timestamp_millis();
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Okx, start_ms, 100.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Okx, now_ms, 90.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Binance, start_ms, 200.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Binance, now_ms, 190.0);
    let mut order = test_ptb_stop_loss_order(&market_slug, "Up", -5.0, Some(100.0));
    order.ptb_current_price_source = "cex_consensus".to_string();

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

    assert_eq!(
        evaluation.current_price_source,
        "cex_consensus_bybit_plus_one"
    );
    assert_eq!(evaluation.current_price, Some(90.0));
    assert_eq!(evaluation.directional_gap, Some(-10.0));
    let cex_metadata = evaluation.source_evaluations[1]
        .metadata
        .as_ref()
        .expect("cex metadata");
    assert_eq!(cex_metadata["confirming_venues"], json!(["binance"]));
    assert!(evaluation.should_trigger);
}

#[tokio::test]
async fn ptb_stop_loss_cex_consensus_triggers_with_coinbase_confirmation() {
    let _guard = lock_ptb_stop_loss_test_state();
    clear_cex_microstructure_test_state();
    seed_ptb_stop_loss_current_price("btc", 120.0);
    let (market_slug, start_ms) = current_5m_market_slug("btc");
    let now_ms = Utc::now().timestamp_millis();
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Okx, start_ms, 100.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Okx, now_ms, 90.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Coinbase, start_ms, 200.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Coinbase, now_ms, 190.0);
    let mut order = test_ptb_stop_loss_order(&market_slug, "Up", -5.0, Some(100.0));
    order.ptb_current_price_source = "cex_consensus".to_string();

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

    assert_eq!(
        evaluation.current_price_source,
        "cex_consensus_bybit_plus_one"
    );
    assert_eq!(evaluation.current_price, Some(90.0));
    assert_eq!(evaluation.directional_gap, Some(-10.0));
    let cex_metadata = evaluation.source_evaluations[1]
        .metadata
        .as_ref()
        .expect("cex metadata");
    assert_eq!(cex_metadata["confirming_venues"], json!(["coinbase"]));
    assert!(evaluation.should_trigger);
}

#[tokio::test]
async fn ptb_stop_loss_cex_consensus_waits_for_confirmation() {
    let _guard = lock_ptb_stop_loss_test_state();
    clear_cex_microstructure_test_state();
    seed_ptb_stop_loss_current_price("btc", 120.0);
    let (market_slug, start_ms) = current_5m_market_slug("btc");
    let now_ms = Utc::now().timestamp_millis();
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Okx, start_ms, 100.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Okx, now_ms, 90.0);
    let mut order = test_ptb_stop_loss_order(&market_slug, "Up", -5.0, Some(100.0));
    order.ptb_current_price_source = "cex_consensus".to_string();

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

    assert_eq!(
        evaluation.current_price_source,
        "cex_consensus_bybit_plus_one"
    );
    assert_eq!(evaluation.directional_gap, Some(-10.0));
    assert_eq!(
        evaluation.source_evaluations[1].error_code,
        Some("cex_consensus_unconfirmed")
    );
    assert!(!evaluation.should_trigger);
}

#[tokio::test]
async fn ptb_stop_loss_cex_consensus_requires_confirm_threshold_not_just_side() {
    let _guard = lock_ptb_stop_loss_test_state();
    clear_cex_microstructure_test_state();
    seed_ptb_stop_loss_current_price("btc", 120.0);
    let (market_slug, start_ms) = current_5m_market_slug("btc");
    let now_ms = Utc::now().timestamp_millis();
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Okx, start_ms, 100.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Okx, now_ms, 90.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Binance, start_ms, 200.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Binance, now_ms, 199.0);
    let mut order = test_ptb_stop_loss_order(&market_slug, "Up", -5.0, Some(100.0));
    order.ptb_current_price_source = "cex_consensus".to_string();

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

    assert_eq!(
        evaluation.current_price_source,
        "cex_consensus_bybit_plus_one"
    );
    assert_eq!(evaluation.directional_gap, Some(-10.0));
    let cex_metadata = evaluation.source_evaluations[1]
        .metadata
        .as_ref()
        .expect("cex metadata");
    assert_eq!(
        cex_metadata["venue_deltas"]["binance"]["directional_gap"],
        json!(-1.0)
    );
    assert_eq!(cex_metadata["confirming_venues"], json!([]));
    assert!(!evaluation.should_trigger);
}

#[tokio::test]
async fn ptb_stop_loss_cex_consensus_requires_bybit_lead() {
    let _guard = lock_ptb_stop_loss_test_state();
    clear_cex_microstructure_test_state();
    seed_ptb_stop_loss_current_price("btc", 120.0);
    let (market_slug, start_ms) = current_5m_market_slug("btc");
    let now_ms = Utc::now().timestamp_millis();
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Binance, start_ms, 200.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Binance, now_ms, 190.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Coinbase, start_ms, 300.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Coinbase, now_ms, 290.0);
    let mut order = test_ptb_stop_loss_order(&market_slug, "Up", -5.0, Some(100.0));
    order.ptb_current_price_source = "cex_consensus".to_string();

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

    assert_eq!(evaluation.current_price_source, "chainlink_live_data_ws");
    assert_eq!(evaluation.current_price, Some(120.0));
    assert_eq!(
        evaluation.source_evaluations[1].error_code,
        Some("cex_consensus_okx_unavailable")
    );
    assert!(!evaluation.should_trigger);
}

#[tokio::test]
async fn ptb_stop_loss_cex_consensus_does_not_raw_trigger_when_window_baseline_is_late() {
    let _guard = lock_ptb_stop_loss_test_state();
    clear_cex_microstructure_test_state();
    seed_ptb_stop_loss_current_price("btc", 120.0);
    let (market_slug, _) = current_5m_market_slug("btc");
    let now_ms = Utc::now().timestamp_millis();
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Okx, now_ms, 88.0);
    seed_ptb_stop_loss_cex_window_price("btc", CexVenue::Binance, now_ms, 87.0);
    let mut order = test_ptb_stop_loss_order(&market_slug, "Up", -10.0, Some(100.0));
    order.ptb_current_price_source = "cex_consensus".to_string();

    let evaluation = trade_builder_evaluate_ptb_stop_loss(&order).expect("ptb eval");

    assert_eq!(evaluation.current_price_source, "chainlink_live_data_ws");
    assert_eq!(evaluation.current_price, Some(120.0));
    assert_eq!(evaluation.directional_gap, Some(20.0));
    assert!(!evaluation.should_trigger);
    assert_eq!(
        evaluation.source_evaluations[1].error_code,
        Some("cex_consensus_okx_unavailable")
    );
    assert_ne!(evaluation.source_evaluations[1].current_price, Some(88.0));
    let cex_metadata = evaluation.source_evaluations[1]
        .metadata
        .as_ref()
        .expect("cex metadata");
    assert!(
        cex_metadata["venue_deltas"]["okx"]["error"]
            .as_str()
            .unwrap_or_default()
            .contains("window open book missing")
    );
}

#[test]
fn ptb_stop_loss_rules_parse_and_validate_descending_weighted_rows() {
    let raw = json!([
        { "gapUsd": 12.5, "sizePct": 25.0 },
        { "gapUsd": 3.0, "sizePct": 75.0 }
    ]);
    let parsed = parse_action_place_order_ptb_stop_loss_rules(Some(&raw), PriceToBeatDiffUnit::Usd)
        .expect("ptb staged rules");

    assert_eq!(
        parsed,
        vec![
            TradeBuilderPtbStopLossRule {
                gap_usd: 12.5,
                size_pct: 25.0,
            },
            TradeBuilderPtbStopLossRule {
                gap_usd: 3.0,
                size_pct: 75.0,
            },
        ]
    );
}

#[test]
fn ptb_stop_loss_rules_accept_negative_descending_rows() {
    let raw = json!([
        { "gapUsd": 20.0, "sizePct": 25.0 },
        { "gapUsd": 0.0, "sizePct": 25.0 },
        { "gapUsd": -20.0, "sizePct": 50.0 }
    ]);
    let parsed = parse_action_place_order_ptb_stop_loss_rules(Some(&raw), PriceToBeatDiffUnit::Usd)
        .expect("negative ptb staged rules");

    assert_eq!(
        parsed,
        vec![
            TradeBuilderPtbStopLossRule {
                gap_usd: 20.0,
                size_pct: 25.0,
            },
            TradeBuilderPtbStopLossRule {
                gap_usd: 0.0,
                size_pct: 25.0,
            },
            TradeBuilderPtbStopLossRule {
                gap_usd: -20.0,
                size_pct: 50.0,
            },
        ]
    );
}

#[test]
fn ptb_stop_loss_rules_normalize_cent_rows_to_usd() {
    let raw = json!([
        { "gapUsd": 20.0, "sizePct": 60.0 },
        { "gapUsd": 0.0, "sizePct": 40.0 }
    ]);
    let parsed =
        parse_action_place_order_ptb_stop_loss_rules(Some(&raw), PriceToBeatDiffUnit::Cent)
            .expect("cent ptb staged rules");

    assert_eq!(
        parsed,
        vec![
            TradeBuilderPtbStopLossRule {
                gap_usd: 0.20,
                size_pct: 60.0,
            },
            TradeBuilderPtbStopLossRule {
                gap_usd: 0.0,
                size_pct: 40.0,
            },
        ]
    );
}

fn test_place_order_node(config: Value) -> TradeFlowNode {
    TradeFlowNode {
        key: "action_test".to_string(),
        node_type: "action.place_order".to_string(),
        config,
    }
}

#[test]
fn ptb_stop_loss_config_accepts_staged_rules_without_legacy_gap() {
    let node = test_place_order_node(json!({
        "ptbStopLossEnabled": true,
        "ptbStopLossRules": [{ "gapUsd": 0.0, "sizePct": 100.0 }]
    }));

    let config =
        resolve_action_place_order_ptb_stop_loss_config(&node, "buy", "eth-updown-5m-1774013100")
            .expect("staged ptb config should resolve")
            .expect("ptb config should be enabled");

    assert_eq!(config.hard_gap_usd, None);
    assert_eq!(
        config.staged_rules,
        vec![TradeBuilderPtbStopLossRule {
            gap_usd: 0.0,
            size_pct: 100.0,
        }]
    );
}

#[test]
fn ptb_stop_loss_config_preserves_legacy_hard_gap() {
    let node = test_place_order_node(json!({
        "ptbStopLossEnabled": true,
        "ptbStopLossGapUsd": 1.25
    }));

    let config =
        resolve_action_place_order_ptb_stop_loss_config(&node, "buy", "eth-updown-5m-1774013100")
            .expect("legacy ptb config should resolve")
            .expect("ptb config should be enabled");

    assert_eq!(config.hard_gap_usd, Some(1.25));
    assert!(config.staged_rules.is_empty());
}

#[test]
fn ptb_stop_loss_config_inherits_entry_current_price_source() {
    let node = test_place_order_node(json!({
        "ptbStopLossEnabled": true,
        "ptbStopLossGapUsd": 1.25,
        "priceToBeatCurrentPriceSource": "binance"
    }));

    let config =
        resolve_action_place_order_ptb_stop_loss_config(&node, "buy", "btc-updown-5m-1774013100")
            .expect("ptb config should resolve")
            .expect("ptb config should be enabled");

    assert_eq!(
        config.current_price_source,
        PriceToBeatCurrentPriceSource::Binance
    );
}

#[test]
fn ptb_stop_loss_config_override_wins_over_entry_current_price_source() {
    let node = test_place_order_node(json!({
        "ptbStopLossEnabled": true,
        "ptbStopLossGapUsd": 1.25,
        "priceToBeatCurrentPriceSource": "binance",
        "ptbStopLossCurrentPriceSource": "coinbase"
    }));

    let config =
        resolve_action_place_order_ptb_stop_loss_config(&node, "buy", "btc-updown-5m-1774013100")
            .expect("ptb config should resolve")
            .expect("ptb config should be enabled");

    assert_eq!(
        config.current_price_source,
        PriceToBeatCurrentPriceSource::Coinbase
    );
}

#[test]
fn ptb_stop_loss_config_normalizes_cent_hard_gap_to_usd() {
    let node = test_place_order_node(json!({
        "ptbStopLossEnabled": true,
        "ptbStopLossGapUsd": 20.0,
        "ptbStopLossGapUnit": "cent"
    }));

    let config =
        resolve_action_place_order_ptb_stop_loss_config(&node, "buy", "eth-updown-5m-1774013100")
            .expect("cent hard ptb config should resolve")
            .expect("ptb config should be enabled");

    assert_eq!(config.hard_gap_usd, Some(0.2));
    assert!(config.staged_rules.is_empty());
}

#[test]
fn ptb_stop_loss_config_preserves_hard_gap_with_staged_rules() {
    let node = test_place_order_node(json!({
        "ptbStopLossEnabled": true,
        "ptbStopLossGapUsd": 2.0,
        "ptbStopLossRules": [{ "gapUsd": 1.0, "sizePct": 100.0 }]
    }));

    let config =
        resolve_action_place_order_ptb_stop_loss_config(&node, "buy", "eth-updown-5m-1774013100")
            .expect("combined ptb config should resolve")
            .expect("ptb config should be enabled");

    assert_eq!(config.hard_gap_usd, Some(2.0));
    assert_eq!(
        config.staged_rules,
        vec![TradeBuilderPtbStopLossRule {
            gap_usd: 1.0,
            size_pct: 100.0,
        }]
    );
}

#[test]
fn ptb_stop_loss_config_normalizes_cent_staged_rules_to_usd() {
    let node = test_place_order_node(json!({
        "ptbStopLossEnabled": true,
        "ptbStopLossGapUnit": "cent",
        "ptbStopLossRules": [{ "gapUsd": 20.0, "sizePct": 100.0 }]
    }));

    let config =
        resolve_action_place_order_ptb_stop_loss_config(&node, "buy", "eth-updown-5m-1774013100")
            .expect("cent staged ptb config should resolve")
            .expect("ptb config should be enabled");

    assert_eq!(config.hard_gap_usd, None);
    assert_eq!(
        config.staged_rules,
        vec![TradeBuilderPtbStopLossRule {
            gap_usd: 0.2,
            size_pct: 100.0,
        }]
    );
}

#[test]
fn ptb_stop_loss_config_preserves_negative_hard_gap() {
    let node = test_place_order_node(json!({
        "ptbStopLossEnabled": true,
        "ptbStopLossGapUsd": -20.0
    }));

    let config =
        resolve_action_place_order_ptb_stop_loss_config(&node, "buy", "eth-updown-5m-1774013100")
            .expect("negative hard ptb config should resolve")
            .expect("ptb config should be enabled");

    assert_eq!(config.hard_gap_usd, Some(-20.0));
    assert!(config.staged_rules.is_empty());
}

#[test]
fn negative_ptb_gap_disables_time_decay() {
    let mut order = test_ptb_stop_loss_order("eth-updown-5m-1774013100", "Up", -20.0, Some(100.0));
    order.ptb_stop_loss_time_decay_mode = Some("tighten".to_string());
    let tighten_value = trade_builder_ptb_stop_loss_effective_gap_usd(&order, -20.0);

    order.ptb_stop_loss_time_decay_mode = Some("relax".to_string());
    let relax_value = trade_builder_ptb_stop_loss_effective_gap_usd(&order, -20.0);

    order.ptb_stop_loss_time_decay_mode = Some("none".to_string());
    let none_value = trade_builder_ptb_stop_loss_effective_gap_usd(&order, -20.0);

    assert_eq!(tighten_value, -20.0);
    assert_eq!(relax_value, -20.0);
    assert_eq!(none_value, -20.0);
}

#[test]
fn ptb_stop_loss_config_still_requires_gap_or_rules_when_enabled() {
    let node = test_place_order_node(json!({
        "ptbStopLossEnabled": true
    }));

    let error =
        resolve_action_place_order_ptb_stop_loss_config(&node, "buy", "eth-updown-5m-1774013100")
            .expect_err("empty ptb config should fail");

    assert!(error.to_string().contains("ptbStopLossGapUsd must be set"));
}

#[test]
fn ptb_stop_loss_rules_reject_non_decreasing_gap_sequence() {
    let raw = json!([
        { "gapUsd": 3.0, "sizePct": 50.0 },
        { "gapUsd": 3.0, "sizePct": 50.0 }
    ]);

    let error = parse_action_place_order_ptb_stop_loss_rules(Some(&raw), PriceToBeatDiffUnit::Usd)
        .expect_err("non-decreasing staged ptb rules should fail");
    assert!(
        error
            .to_string()
            .contains("ptbStopLossRules gapUsd values must be strictly decreasing")
    );
}
