use super::support::*;
use super::*;

fn live_gap_metadata() -> Value {
    json!({
        "mode": ACTION_PLACE_ORDER_MODE_LIVE_GAP_COLLECTOR_V1,
        "live_gap_usd": 59.4,
        "required_gap_usd": 50.0,
        "regime": "high",
        "remaining_sec": 44,
        "best_ask": 0.88,
        "effective_fill_price": 0.88,
        "no_reversal_entry_guard": {
            "profile_source": "missing",
            "profile_lookup_status": "row_missing",
            "prewarmer_status": "expected_key_timed_out",
            "prewarm_priority": "exact_current",
            "prewarm_slot_status": "timed_out",
            "prewarm_age_ms": 30000,
            "fallback_level": "gap_relaxed",
            "profile_lookup_fallback_level": "gap_relaxed",
            "runtime_fallback_source": "local_2m_path",
            "local_path_fallback_source": "local_2m_path",
            "protection": "local_path_applied",
            "ptb_floor_usd": 13.0,
            "reason_code": "local_path_safe_fallback",
            "profile_lookup_key": {
                "target_market_slug": "btc-updown-5m-1778048400",
                "target_window_start": "2026-05-06T12:00:00Z",
                "definition_id": 4320,
                "node_key": "action_0rt6iz",
                "profile_config_hash": "abcdef1234567890",
                "asset": "btc",
                "direction": "up",
                "remaining_bucket": "30_60s",
                "price_bucket": "very_high",
                "gap_bucket": "large",
                "slope_bucket": "non_negative",
                "quantile": 0.95,
                "high_late": false
            }
        }
    })
}

#[test]
fn live_gap_local_path_window_follows_trigger_custom_range_or_workflow_default() {
    let custom_range_input = json!({
        "windowBoundaryMode": "custom_range",
        "cycleWindowStartSec": 240,
        "cycleWindowEndSec": 285,
    });
    assert_eq!(
        no_reversal_workflow_local_path_lookback_from_input(Some(&custom_range_input)),
        (45_000, "trigger_custom_range")
    );

    let default_input = json!({
        "windowBoundaryMode": "last",
        "cycleWindowSecs": 45,
    });
    assert_eq!(
        no_reversal_workflow_local_path_lookback_from_input(Some(&default_input)),
        (300_000, "default_workflow_5m")
    );
}

#[test]
fn live_gap_primary_buy_submitted_uses_live_gap_template() {
    let mut order = test_builder_order("buy", None);
    order.notify_on_order_submitted = true;
    order.target_qty = Some(5.68);
    let submitted_payload = json!({
        "order_type": "fak",
        "size_basis": "notional_usdc",
        "submitted_target_qty": 5.68,
        "submitted_best_ask": 0.88,
        "submitted_estimated_avg_fill": 0.88,
        "submitted_estimated_notional": 5.0,
        "submitted_effective_cost_per_share": 0.8876
    });

    let message = build_trade_builder_submitted_notification_message_with_live_gap(
        &order,
        &submitted_payload,
        None,
        Some(&live_gap_metadata()),
    );

    assert!(message.contains("Emir Gonderildi - Live Gap Collector"));
    assert!(message.contains("Current Gap: +59.4 USD"));
    assert!(message.contains("Required Gap: +50.0 USD"));
    assert!(message.contains("Fallback: local_2m_path"));
    assert!(message.contains("Prewarmer Status: expected_key_timed_out"));
    assert!(message.contains("Prewarm Detail: priority=exact_current, slot=timed_out, age=30000ms"));
    assert!(message.contains("Profile Lookup Fallback: gap_relaxed"));
    assert!(message.contains("Profile Lookup Key: window_start=2026-05-06T12:00:00Z"));
    assert!(message.contains("hash=abcdef12"));
    assert!(message.contains("Protection: local_path_applied"));
    assert!(!message.contains("Mode: IV Mismatch Edge"));
    assert!(!message.contains("q_final"));
    assert!(!message.contains("Model-Book Gap"));
    assert!(!message.contains("EV:"));
}

#[test]
fn live_gap_metadata_on_child_sell_does_not_use_buy_template() {
    let mut order = test_builder_order("sell", Some(41));
    order.notify_on_order_submitted = true;
    order.trigger_condition = Some("cross_above".to_string());
    order.target_qty = Some(5.68);
    order.remaining_qty = Some(5.68);
    let submitted_payload = json!({
        "order_type": "fak",
        "submitted_target_qty": 5.68,
        "submitted_estimated_avg_fill": 0.98
    });

    let message = build_trade_builder_submitted_notification_message_with_live_gap(
        &order,
        &submitted_payload,
        None,
        Some(&live_gap_metadata()),
    );

    assert!(message.contains("Emir Gonderildi - TP Child Exit"));
    assert!(message.contains("Estimated Proceeds: 5.57 USDC"));
    assert!(!message.contains("Emir Gonderildi - Live Gap Collector"));
    assert!(!message.contains("Mode: IV Mismatch Edge"));
}

#[test]
fn buy_decision_message_explains_local_path_pass() {
    let decision = LiveGapCollectorDecision {
        passed: true,
        terminal: false,
        reason_code: "local_path_safe_fallback",
        payload: json!({}),
    };
    let mut payload = live_gap_metadata();
    let obj = payload.as_object_mut().expect("payload object");
    obj.insert("market_slug".to_string(), json!("btc-updown-5m-1778048400"));
    obj.insert("outcome_label".to_string(), json!("Up"));
    obj.insert("side".to_string(), json!("buy"));
    obj.insert("ptb_telemetry".to_string(), json!({"source": "cache"}));
    let guard = obj
        .get_mut("no_reversal_entry_guard")
        .and_then(Value::as_object_mut)
        .expect("no reversal guard");
    guard.insert("local_path_history_ms".to_string(), json!(118_000));
    guard.insert("local_path_sample_count".to_string(), json!(420));
    guard.insert("largest_sample_gap_ms".to_string(), json!(500));
    guard.insert("local_path_min_gap_30s".to_string(), json!(42.0));
    guard.insert("local_path_min_gap_60s".to_string(), json!(39.5));
    guard.insert("local_path_min_gap_2m".to_string(), json!(31.0));
    guard.insert("local_path_drop_10s".to_string(), json!(1.2));
    guard.insert("local_path_drop_30s".to_string(), json!(4.2));
    guard.insert("local_path_drop_60s".to_string(), json!(4.2));
    guard.insert("local_path_slope_3s".to_string(), json!(0.8));
    guard.insert("local_path_slope_10s".to_string(), json!(0.54));
    guard.insert("local_path_slope_30s".to_string(), json!(0.2));
    guard.insert(
        "local_path_decision_reason".to_string(),
        json!("local_path_safe_fallback"),
    );
    guard.insert("decision".to_string(), json!("pass"));

    let message = build_live_gap_collector_decision_notification_message(&decision, &payload);

    assert!(message.contains("No-Reversal:"));
    assert!(message.contains("Profile: missing"));
    assert!(message.contains("Profile Status: row_missing"));
    assert!(message.contains("Prewarmer Status: expected_key_timed_out"));
    assert!(message.contains("Prewarm Detail: priority=exact_current, slot=timed_out, age=30000ms"));
    assert!(message.contains("Profile Lookup Fallback: gap_relaxed"));
    assert!(message.contains("Profile Lookup Key: window_start=2026-05-06T12:00:00Z"));
    assert!(message.contains("Local Path:"));
    assert!(message.contains("History: 118s"));
    assert!(message.contains("Samples: 420"));
    assert!(message.contains("Largest Sample Gap: 500ms"));
    assert!(message.contains("Min Gap 60s: +39.5 USD"));
    assert!(message.contains("Drop10/30/60: 1.2 USD / 4.2 USD / 4.2 USD"));
    assert!(message.contains("Slope 3s/10s/30s: +0.80 USD/s / +0.54 USD/s / +0.20 USD/s"));
    assert!(message.contains("Decision: PASS"));
}

#[test]
fn tp_fill_summary_uses_actual_parent_remaining_position() {
    let mut order = test_builder_order("sell", Some(41));
    order.notify_on_fill = true;
    order.trigger_condition = Some("cross_above".to_string());
    order.target_qty = Some(5.68);
    order.remaining_qty = Some(5.68);
    let analysis = TradeBuilderFillExecutionAnalysis {
        actual_fill_price: 0.98,
        actual_filled_qty: 5.61,
        actual_notional: 5.4978,
        actual_fill_source: "fills_aggregate",
    };
    let position = TradeBuilderParentPosition {
        parent_builder_order_id: 41,
        user_id: 1,
        source_trade_id: 77,
        market_slug: order.market_slug.clone(),
        token_id: order.token_id.clone(),
        outcome_label: order.outcome_label.clone(),
        baseline_qty: 5.68,
        current_qty: 0.07,
        last_fill_qty: Some(5.61),
        last_fill_price: Some(0.98),
        qty_source: "child_fill:fills_aggregate".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let summary =
        trade_builder_exit_fill_position_summary(&order, Some(5.61), 0.98, Some(&position))
            .expect("summary");

    let (_, message) = build_trade_builder_fill_notification_with_position_summary(
        &order,
        0.98,
        5.61,
        None,
        None,
        Some(&analysis),
        Some(&summary),
    )
    .expect("notification");

    assert!(message.contains("Remaining Position"));
    assert!(message.contains("Sold This Fill: 5.61 / 5.68"));
    assert!(message.contains("Remaining Qty: 0.07 Up (trade_builder_parent_positions)"));
    assert!(message.contains("Remaining Max Loss: 0.07 USDC"));
    assert!(message.contains("State: open"));
    assert!(!message.contains("If Up loses: -"));
}

#[test]
fn full_tp_fill_summary_marks_remaining_position_closed_when_estimated() {
    let mut order = test_builder_order("sell", Some(41));
    order.notify_on_fill = true;
    order.trigger_condition = Some("cross_above".to_string());
    order.target_qty = Some(5.68);
    let analysis = TradeBuilderFillExecutionAnalysis {
        actual_fill_price: 0.98,
        actual_filled_qty: 5.68,
        actual_notional: 5.5664,
        actual_fill_source: "fills_aggregate",
    };
    let summary =
        trade_builder_exit_fill_position_summary(&order, Some(5.68), 0.98, None).expect("summary");

    let (_, message) = build_trade_builder_fill_notification_with_position_summary(
        &order,
        0.98,
        5.68,
        None,
        None,
        Some(&analysis),
        Some(&summary),
    )
    .expect("notification");

    assert!(message.contains("Remaining Qty: 0.00 Up (estimated: target_qty_minus_fill_estimate)"));
    assert!(message.contains("Remaining Max Loss: 0.00 USDC"));
    assert!(message.contains("State: closed"));
}
