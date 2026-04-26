use super::support::*;
use super::*;

#[test]
fn root_fill_notification_uses_order_filled_type() {
    let mut order = test_builder_order("buy", None);
    order.notify_on_fill = true;

    let (notification_type, message) =
        build_trade_builder_fill_notification(&order, 0.61, 12.5, None, None, None)
            .expect("notification");

    assert_eq!(notification_type, "order_filled");
    assert!(message.contains("Emir Doldu"));
    assert!(message.contains("Sebep: Emir basariyla dolduruldu."));
    assert!(message.contains("Notional USDC: 5.00"));
    assert!(message.contains("Adet: 12.50"));
    assert!(message.contains("Outcome: Up"));
}

#[test]
fn root_share_fill_notification_reports_qty_mode() {
    let mut order = test_builder_order("buy", None);
    order.notify_on_fill = true;
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.target_qty = Some(5.0);

    let (_, message) = build_trade_builder_fill_notification(&order, 0.61, 5.0, None, None, None)
        .expect("notification");

    assert!(message.contains("Size Mode: shares"));
    assert!(message.contains("Target Qty: 5.00"));
    assert!(message.contains("Estimated Notional: 3.05 USDC"));
}

#[test]
fn submitted_notification_reports_vwap_model_book_and_scenario() {
    let mut order = test_builder_order("buy", None);
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.target_qty = Some(5.0);
    let submitted_payload = json!({
        "order_type": "market",
        "size_basis": "shares",
        "submitted_target_qty": 5.0,
        "submitted_best_ask": 0.58,
        "submitted_estimated_avg_fill": 0.61,
        "submitted_vwap_slippage": 0.03,
        "submitted_estimated_notional": 3.05,
        "submitted_effective_cost_per_share": 0.6321,
        "submitted_depth_guard_result": "PASS",
        "submitted_depth_levels_used": 3,
        "submitted_q_final": 0.9474,
        "submitted_selected_mid": 0.575,
        "submitted_model_book_gap": 0.3724,
        "submitted_model_book_zone": "WARNING",
        "submitted_model_book_penalty": 0.02,
        "submitted_dynamic_threshold_before_credit": 0.11,
        "submitted_participation_credit": 0.01,
        "submitted_dynamic_threshold_after_credit": 0.10,
        "submitted_adjusted_edge": 0.3448,
        "submitted_adjusted_margin": 0.2348,
        "submitted_if_win_pnl_est": 1.8395,
        "submitted_if_loss_pnl_est": -3.1605,
        "submitted_ev_est": 1.5765,
        "submitted_ev_roi_est": 0.4988,
        "submitted_late_high_price_warning": false,
        "submitted_binance_same_direction": true,
        "submitted_spread": 0.01,
        "submitted_stale_ms": 1388
    });
    let flow_payload = json!({
        "price_to_beat_guard": {
            "iv_mismatch_edge": {
                "selected_side": "Down",
                "seconds_left": 56.7,
                "selected_time_rule": {"start_remaining_secs": 60, "end_remaining_secs": 30}
            }
        }
    });

    let message = build_trade_builder_submitted_notification_message(
        &order,
        &submitted_payload,
        Some(&flow_payload),
    );

    assert!(message.contains("Emir Gonderildi - Guard Gecti"));
    assert!(message.contains("Estimated VWAP Fill: 0.6100"));
    assert!(message.contains("Model-Book Zone: WARNING"));
    assert!(message.contains("Participation Credit: 0.0100"));
    assert!(message.contains("EV ROI: +49.9"));
}

#[test]
fn fill_notification_compares_actual_fill_to_submitted_snapshot() {
    let mut order = test_builder_order("buy", None);
    order.notify_on_fill = true;
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.target_qty = Some(5.0);
    let submitted_payload = json!({
        "submitted_target_qty": 5.0,
        "submitted_best_ask": 0.58,
        "submitted_estimated_avg_fill": 0.61
    });
    let flow_payload = json!({
        "price_to_beat_guard": {
            "iv_mismatch_edge": {"buffer": 0.005}
        }
    });
    let analysis = TradeBuilderFillExecutionAnalysis {
        actual_fill_price: 0.61,
        actual_filled_qty: 3.2,
        actual_notional: 1.952,
        actual_fill_source: "fills_aggregate",
    };

    let (_, message) = build_trade_builder_fill_notification(
        &order,
        0.61,
        3.2,
        Some(&flow_payload),
        Some(&submitted_payload),
        Some(&analysis),
    )
    .expect("notification");

    assert!(message.contains("Fill Analysis"));
    assert!(message.contains("Fill Ratio: 64.0%"));
    assert!(message.contains("Partial Fill: true"));
    assert!(message.contains("Actual Fill Source: fills_aggregate"));
    assert!(message.contains("Slippage vs VWAP: +0.0000"));
    assert!(message.contains("Slippage vs Best Ask: +0.0300"));
    assert!(message.contains("0.6321 = fill 0.6100 + fee 0.0171 + buffer 0.0050"));
}

#[test]
fn tp_child_fill_notification_uses_tp_type() {
    let mut order = test_builder_order("sell", Some(41));
    order.notify_on_fill = true;
    order.trigger_condition = Some("cross_above".to_string());

    let (notification_type, message) =
        build_trade_builder_fill_notification(&order, 0.82, 4.2, None, None, None)
            .expect("notification");

    assert_eq!(notification_type, "tp_hit");
    assert!(message.contains("Take Profit Tetiklendi"));
    assert!(message.contains("Sebep: Take profit seviyesi goruldugu icin cikis emri dolduruldu."));
}

#[test]
fn sl_child_fill_notification_uses_sl_type() {
    let mut order = test_builder_order("sell", Some(41));
    order.notify_on_fill = true;
    order.trigger_condition = Some("cross_below".to_string());

    let (notification_type, message) =
        build_trade_builder_fill_notification(&order, 0.37, 4.2, None, None, None)
            .expect("notification");

    assert_eq!(notification_type, "sl_hit");
    assert!(message.contains("Stop Loss Tetiklendi"));
    assert!(message.contains("Sebep: Stop loss seviyesi goruldugu icin cikis emri dolduruldu."));
}

#[test]
fn fill_notification_respects_toggle() {
    let order = test_builder_order("buy", None);

    assert!(build_trade_builder_fill_notification(&order, 0.51, 3.0, None, None, None).is_none());
}

#[test]
fn fill_notification_includes_successful_iv_mismatch_formula_block() {
    let mut order = test_builder_order("buy", None);
    order.notify_on_fill = true;
    let mut payload = json!({
        "price_to_beat_guard": {
            "threshold_mode": "iv_mismatch_edge",
            "normalized_outcome_label": "Up",
            "price_to_beat": 76130.01578425177,
            "current_price": 76139.53643974895,
            "directional_gap": 9.520655497180996,
            "iv_mismatch_edge": {
                "passed": true,
                "decision_reason": "selected_edge_passed",
                "candidate_side": "up",
                "selected_side": "up",
                "q": 0.7300,
                "q_up": 0.7300,
                "q_down": 0.2700,
                "cost": 0.5817,
                "edge": 0.1483,
                "threshold": 0.0600,
                "ask": 0.5600,
                "bid": 0.5400,
                "fee": 0.0167,
                "buffer": 0.0050,
                "spread": 0.0200,
                "seconds_left": 42.0,
                "sigma": 0.15000000,
                "expected_move": 0.97211110,
                "z": 0.8100,
                "iv_ratio": 1.2400,
                "zero_cross_count": 1,
                "sample_count": 10,
                "chainlink_staleness_ms": 450
            }
        }
    });
    let iv = payload
        .get_mut("price_to_beat_guard")
        .and_then(|guard| guard.get_mut("iv_mismatch_edge"))
        .and_then(Value::as_object_mut)
        .expect("iv object");
    iv.insert("q_chain_adj".to_string(), json!(0.7200));
    iv.insert("q_binance".to_string(), json!(0.7000));
    iv.insert("q_final".to_string(), json!(0.7200));
    iv.insert("edge_adj".to_string(), json!(0.1383));
    iv.insert("dynamic_threshold".to_string(), json!(0.0800));
    iv.insert("x_now".to_string(), json!(9.5207));
    iv.insert("x_prev".to_string(), json!(9.1000));
    iv.insert("gap_velocity".to_string(), json!(0.2103));
    iv.insert("latency_horizon_secs".to_string(), json!(1.2000));
    iv.insert("x_eff".to_string(), json!(9.5207));
    iv.insert("sigma_15".to_string(), json!(0.13000000));
    iv.insert("sigma_eff".to_string(), json!(0.16250000));
    iv.insert("drop_z".to_string(), json!(0.2000));
    iv.insert("high_price_penalty".to_string(), json!(0.0000));
    iv.insert("stale_penalty".to_string(), json!(0.0200));
    iv.insert("drop_penalty".to_string(), json!(0.0000));
    iv.insert("binance_price".to_string(), json!(76138.1000));
    iv.insert("binance_staleness_ms".to_string(), json!(300));
    iv.insert(
        "binance_veto_status".to_string(),
        json!("fresh_conservative_min"),
    );

    let (notification_type, message) =
        build_trade_builder_fill_notification(&order, 0.56, 7.5, Some(&payload), None, None)
            .expect("notification");

    assert_eq!(notification_type, "order_filled");
    assert!(message.contains("IV Mismatch Edge Basarili"));
    assert!(message.contains("Karar: selected_edge_passed"));
    assert!(message.contains("Fill: 0.5600 | Ask: 0.5600"));
    assert!(message.contains("Cost: 0.5817 = ask 0.5600 + fee 0.0167 + buffer 0.0050"));
    assert!(message
        .contains("Edge raw: 0.1483 = q - cost | Base threshold: 0.0600 | Raw margin: 0.0883"));
    assert!(message.contains(
        "Edge adjusted: 0.1383 = q_final - cost | Dynamic threshold: 0.0800 | Adj margin: 0.0583"
    ));
    assert!(message.contains(
        "q floor: before N/A | after N/A | final 0.7200 | q_chain_adj 0.7200 | q_binance 0.7000"
    ));
    assert!(message.contains("Stale/drop: x_now 9.5207 | x_prev 9.1000 | v 0.2103 USD/s"));
    assert!(message.contains(
        "Binance: status fresh_conservative_min | price 76138.1000 | stale 300 | q 0.7000"
    ));
    assert!(message.contains("PTB: 76130.01578425"));
    assert!(message.contains("zero_cross: 1 | samples: 10 | stale: 450"));
}

#[test]
fn fill_notification_omits_failed_iv_mismatch_formula_block() {
    let mut order = test_builder_order("buy", None);
    order.notify_on_fill = true;
    let payload = json!({
        "price_to_beat_guard": {
            "threshold_mode": "iv_mismatch_edge",
            "normalized_outcome_label": "Up",
            "iv_mismatch_edge": {
                "passed": false,
                "decision_reason": "blocked_edge_below_threshold",
                "candidate_side": "up",
                "selected_side": null,
                "edge": 0.0100,
                "threshold": 0.0600
            }
        }
    });

    let (_, message) =
        build_trade_builder_fill_notification(&order, 0.56, 7.5, Some(&payload), None, None)
            .expect("notification");

    assert!(!message.contains("IV Mismatch Edge Basarili"));
}

#[test]
fn trigger_guard_notification_includes_reason_line() {
    let mut order = test_builder_order("buy", None);
    order.guard_trigger_price = Some(0.77);

    let message = build_trigger_guard_blocked_notification_message(&order, 0.76, "best_ask");

    assert!(message.contains("Sebep:"));
    assert!(message.contains("guard seviyesinin altina dustu"));
    assert!(message.contains("Referans (best_ask): 0.7600"));
    assert!(message.contains("Guard: 0.7700"));
}

#[test]
fn trigger_guard_waiting_notification_mentions_recovery_retry() {
    let mut order = test_builder_order("buy", None);
    order.guard_trigger_price = Some(0.77);

    let message =
        build_trigger_guard_waiting_notification_message(&order, 0.76, "current_price_fallback");

    assert!(message.contains("Bekleme"));
    assert!(message.contains("yeniden denenecek"));
    assert!(message.contains("Referans (current_price_fallback): 0.7600"));
    assert!(message.contains("Guard: 0.7700"));
}

#[test]
fn execution_floor_notification_describes_missing_best_ask() {
    let mut order = test_builder_order("buy", None);
    order.best_ask_floor_price = Some(0.77);

    let message = build_execution_floor_blocked_notification_message(&order, None);

    assert!(message.contains("Sebep:"));
    assert!(message.contains("Best ask verisi alinamadigi"));
    assert!(message.contains("Best Ask: N/A"));
}

#[test]
fn execution_floor_notification_describes_below_floor() {
    let mut order = test_builder_order("buy", None);
    order.best_ask_floor_price = Some(0.77);

    let message = build_execution_floor_blocked_notification_message(&order, Some(0.75));

    assert!(message.contains("Sebep:"));
    assert!(message.contains("Best ask floor seviyesinin altinda"));
    assert!(message.contains("Floor: 0.7700"));
}

#[test]
fn execution_floor_waiting_notification_mentions_recovery_retry() {
    let mut order = test_builder_order("buy", None);
    order.best_ask_floor_price = Some(0.77);

    let message = build_execution_floor_waiting_notification_message(&order, Some(0.75));

    assert!(message.contains("Bekleme"));
    assert!(message.contains("yeniden denenecek"));
    assert!(message.contains("Floor: 0.7700"));
}

#[test]
fn execution_floor_waiting_notification_describes_missing_best_ask() {
    let mut order = test_builder_order("buy", None);
    order.best_ask_floor_price = Some(0.77);

    let message = build_execution_floor_waiting_notification_message(&order, None);

    assert!(message.contains("Best ask verisi alinamadi"));
    assert!(message.contains("Best Ask: N/A"));
}

#[test]
fn max_price_blocked_notification_respects_toggle() {
    let mut order = test_builder_order("buy", None);
    order.max_price = Some(0.9);

    assert!(build_max_price_blocked_notification(&order, 0.95, 0.96, "best_ask").is_none());

    order.notify_on_max_price_blocked = true;
    let (notification_type, message) =
        build_max_price_blocked_notification(&order, 0.95, 0.96, "best_ask").expect("notification");

    assert_eq!(notification_type, "max_price_blocked");
    assert!(message.contains("Max Fiyat Korumasi Engelledi"));
    assert!(message.contains("Guncel: 0.9500"));
    assert!(message.contains("Referans (best_ask): 0.9600"));
    assert!(message.contains("Max: 0.9000"));
}

#[test]
fn max_price_waiting_notification_mentions_recovery_retry() {
    let mut order = test_builder_order("buy", None);
    order.max_price = Some(0.9);

    let message = build_max_price_waiting_notification_message(
        &order,
        0.95,
        0.96,
        "desired_price_fallback",
        Some("above_max_price"),
    );

    assert!(message.contains("Bekleme"));
    assert!(message.contains("yeniden denenecek"));
    assert!(message.contains("Guncel: 0.9500"));
    assert!(message.contains("Referans (desired_price_fallback): 0.9600"));
    assert!(message.contains("Max: 0.9000"));
}

#[test]
fn max_price_waiting_notification_describes_missing_best_ask() {
    let mut order = test_builder_order("buy", None);
    order.max_price = Some(0.9);

    let message = build_max_price_waiting_notification_message(
        &order,
        0.67,
        0.67,
        "best_ask_unavailable",
        Some("pair_primary_best_ask_unavailable"),
    );

    assert!(message.contains("Best ask verisi bekleniyor"));
    assert!(message.contains("Referans (best_ask_unavailable): 0.6700"));
    assert!(message.contains("Max: 0.9000"));
    assert!(!message.contains("max fiyat limitini asiyor"));
}

#[test]
fn pair_lock_primary_execution_floor_waiting_notification_mentions_secondary_reason() {
    let message = build_pair_lock_primary_execution_floor_notification_message(
        "btc-updown-5m-1",
        &json!({
            "outcome_label": "Up",
            "best_ask": 0.18,
            "reason_code": "below_best_ask_floor"
        }),
        Some(0.20),
        true,
        Some(&json!({
            "outcome_label": "Down",
            "reason_code": "above_max_price"
        })),
    );

    assert!(message.contains("Execution Floor Bekleme Modu"));
    assert!(message.contains("Outcome: Up"));
    assert!(message.contains("Best Ask: 0.1800"));
    assert!(message.contains("Floor: 0.2000"));
    assert!(message.contains("Diger Aday: Down -> above_max_price"));
}

#[test]
fn pair_lock_primary_price_to_beat_waiting_notification_mentions_gap_and_secondary_reason() {
    let message = build_pair_lock_primary_price_to_beat_notification_message(
        "btc-updown-5m-1",
        &json!({
            "outcome_label": "Down",
            "price_to_beat_guard": {
                "reason_detail": "directional gap -9.52065550 (direction=down) is below threshold 20.00000000 usd (~20.00000000 usd)",
                "price_to_beat": 76130.01578425177,
                "current_price": 76139.53643974895,
                "directional_gap": -9.520655497180996,
                "threshold_usd": 20.0,
                "current_price_source": "chainlink_live_data_ws"
            }
        }),
        true,
        Some(&json!({
            "outcome_label": "Up",
            "reason_code": "below_best_ask_floor"
        })),
    );

    assert!(message.contains("Price to Beat Korumasi Bekleme Modu"));
    assert!(message.contains("Outcome: Down"));
    assert!(message.contains("76130.01578425"));
    assert!(message.contains("chainlink_live_data_ws"));
    assert!(message.contains("Diger Aday: Up -> below_best_ask_floor"));
}

#[test]
fn pair_lock_primary_guard_recovered_notification_mentions_previous_reason() {
    let message = build_pair_lock_primary_guard_recovered_notification_message(
        "btc-updown-5m-1",
        "execution_floor",
        "below_best_ask_floor",
    );

    assert!(message.contains("Execution Floor Korumasi Gecti"));
    assert!(message.contains("Market: btc-updown-5m-1"));
    assert!(message.contains("Onceki Sebep: below_best_ask_floor"));
}

#[test]
fn guard_notification_reason_is_namespaced() {
    assert_eq!(
        build_guard_notification_reason("max_price", "above_max_price"),
        "max_price:above_max_price"
    );
    assert_eq!(
        build_guard_notification_reason("execution_floor", "below_best_ask_floor"),
        "execution_floor:below_best_ask_floor"
    );
}

#[test]
fn guard_transition_notification_only_sends_for_new_reason() {
    let mut order = test_builder_order("buy", None);

    assert!(should_send_guard_transition_notification(
        &order,
        "max_price:above_max_price",
        true,
    ));
    assert!(!should_send_guard_transition_notification(
        &order,
        "max_price:above_max_price",
        false,
    ));

    order.last_guard_notification_reason = Some("max_price:above_max_price".to_string());
    assert!(!should_send_guard_transition_notification(
        &order,
        "max_price:above_max_price",
        true,
    ));
    assert!(should_send_guard_transition_notification(
        &order,
        "execution_floor:below_best_ask_floor",
        true,
    ));
}

#[test]
fn order_not_filled_notification_includes_reason_code() {
    let order = test_builder_order("buy", None);

    let message = build_order_not_filled_notification_message(
        &order,
        "outside_cycle_window",
        "Eligible penceresi kapandigi icin emir icra edilemeden expire oldu.",
    );

    assert!(message.contains("Emir Icra Edilemedi"));
    assert!(message.contains("Sebep Kodu: outside_cycle_window"));
}

#[test]
fn order_not_filled_notification_includes_market_details() {
    let order = test_builder_order("buy", None);

    let message =
        build_order_not_filled_notification_message(&order, "ttl_expired", "Sure asildi.");

    assert!(message.contains("Market: btc-updown-5m-1"));
    assert!(message.contains("Outcome: Up"));
    assert!(message.contains("Side: buy"));
}

#[test]
fn order_not_filled_notification_includes_latest_max_price_guard() {
    let order = test_builder_order("buy", None);
    let events = vec![TradeBuilderOrderEventRecord {
        builder_order_id: order.id,
        event_type: "guard_evaluated".to_string(),
        payload_json: json!({
            "effective_guard_scope": "max_price",
            "effective_decision": "waiting",
            "effective_reason_code": "above_max_price",
            "max_price_guard": {
                "configured": true,
                "decision": "waiting",
                "reason_code": "above_max_price",
                "details": {
                    "reference_price": 0.97,
                    "reference_price_source": "best_ask",
                    "desired_price": 0.98,
                    "max_price": 0.95
                }
            }
        }),
        created_at: Utc::now(),
    }];

    let summary = build_order_not_filled_guard_summary(&order, &events).expect("summary");
    let message = build_order_not_filled_notification_message_with_guard(
        &order,
        "outside_cycle_window",
        "Eligible penceresi kapandigi icin emir icra edilemeden expire oldu.",
        Some(&summary),
    );

    assert!(message.contains("Son Engel: Max Fiyat"));
    assert!(message.contains("Engel Kodu: above_max_price"));
    assert!(message.contains("Karar: waiting"));
    assert!(message.contains("Referans: 0.9700"));
    assert!(message.contains("Max: 0.9500"));
}

#[test]
fn order_not_filled_notification_includes_execution_floor_guard() {
    let order = test_builder_order("buy", None);
    let events = vec![TradeBuilderOrderEventRecord {
        builder_order_id: order.id,
        event_type: "guard_evaluated".to_string(),
        payload_json: json!({
            "effective_guard_scope": "execution_floor",
            "effective_decision": "waiting",
            "effective_reason_code": "best_ask_unavailable",
            "execution_floor_guard": {
                "configured": true,
                "decision": "waiting",
                "reason_code": "best_ask_unavailable",
                "details": {
                    "best_ask_floor_price": 0.51,
                    "best_ask": null
                }
            }
        }),
        created_at: Utc::now(),
    }];

    let summary = build_order_not_filled_guard_summary(&order, &events).expect("summary");
    let message = build_order_not_filled_notification_message_with_guard(
        &order,
        "outside_cycle_window",
        "Eligible penceresi kapandigi icin emir icra edilemeden expire oldu.",
        Some(&summary),
    );

    assert!(message.contains("Son Engel: Execution Floor"));
    assert!(message.contains("Engel Kodu: best_ask_unavailable"));
    assert!(message.contains("Best Ask: N/A"));
    assert!(message.contains("Floor: 0.5100"));
}

#[test]
fn order_not_filled_notification_includes_price_to_beat_guard() {
    let order = test_builder_order("buy", None);
    let events = vec![TradeBuilderOrderEventRecord {
        builder_order_id: order.id,
        event_type: "flow_created".to_string(),
        payload_json: json!({
            "price_to_beat_guard": {
                "passed": false,
                "reason_code": "price_to_beat_gap_below_threshold",
                "reason_detail": "Yonsel fark minimum limitin altinda.",
                "price_to_beat": 76130.01578425,
                "current_price": 76131.11578425,
                "directional_gap": 1.1,
                "threshold_usd": 2.5
            }
        }),
        created_at: Utc::now(),
    }];

    let summary = build_order_not_filled_guard_summary(&order, &events).expect("summary");
    let message = build_order_not_filled_notification_message_with_guard(
        &order,
        "outside_cycle_window",
        "Eligible penceresi kapandigi icin emir icra edilemeden expire oldu.",
        Some(&summary),
    );

    assert!(message.contains("Son Engel: Price to Beat"));
    assert!(message.contains("Engel Kodu: price_to_beat_gap_below_threshold"));
    assert!(message.contains("Price to Beat: 76130.01578425"));
    assert!(message.contains("Yonsel Fark: 1.10000000"));
    assert!(message.contains("Limit: 2.50000000"));
}

#[test]
fn trade_flow_missed_market_summary_uses_ptb_gate_event() {
    let events = vec![TradeFlowEventRecord {
        run_id: 7,
        event_type: "trigger_cycle_window_price_to_beat_gate_blocked".to_string(),
        payload_json: json!({
            "node_key": "trigger_1",
            "market_slug": "btc-updown-5m-1",
            "token_id": "token-up",
            "price_to_beat_trigger_gate": {
                "passed": false,
                "reason": "price_to_beat_gap_below_threshold",
                "price_to_beat": 76130.0,
                "current_price": 76131.0,
                "directional_gap": 1.0,
                "threshold_usd": 2.5
            }
        }),
        created_at: Utc::now(),
    }];

    let summary =
        latest_trade_flow_no_fill_summary(&events, "trigger_1", "btc-updown-5m-1", "token-up")
            .expect("summary");
    let block = build_no_fill_guard_summary_block(&summary);

    assert!(block.contains("Son Engel: Price to Beat"));
    assert!(block.contains("Engel Kodu: price_to_beat_gap_below_threshold"));
    assert!(block.contains("Limit: 2.50000000"));
}

#[test]
fn action_place_order_output_summary_uses_target_pair_lock_candidate() {
    let output = json!({
        "reason": "pair_lock_primary_guard_waiting",
        "blocked": true,
        "market_slug": "btc-updown-5m-1",
        "no_candidate_guard": {
            "passed": false,
            "decision": "waiting",
            "reason_code": "below_best_ask_floor",
            "token_id": "down-token",
            "outcome_label": "Down",
            "best_ask": 0.01,
            "execution_floor_guard": {
                "configured": true,
                "decision": "waiting",
                "reason_code": "below_best_ask_floor",
                "details": {
                    "best_ask": 0.01,
                    "best_ask_floor_price": 0.5
                }
            },
            "max_price_guard": {
                "configured": true,
                "decision": "passed",
                "reason_code": "passed",
                "details": {
                    "reference_price": 0.01,
                    "reference_price_source": "best_ask",
                    "desired_price": 0.01,
                    "max_price": 0.75
                }
            }
        },
        "yes_candidate_guard": {
            "passed": false,
            "decision": "waiting",
            "reason_code": "above_max_price",
            "token_id": "up-token",
            "outcome_label": "Up",
            "max_price_guard": {
                "configured": true,
                "decision": "waiting",
                "reason_code": "above_max_price",
                "details": {
                    "reference_price": 0.99,
                    "reference_price_source": "best_ask",
                    "desired_price": 0.99,
                    "max_price": 0.75
                }
            }
        }
    });

    let summary = no_fill_summary_from_action_place_order_output(&output, "down-token", "Down")
        .expect("summary");
    let block = build_no_fill_guard_summary_block(&summary);

    assert!(block.contains("Son Engel: Execution Floor"));
    assert!(block.contains("Engel Kodu: below_best_ask_floor"));
    assert!(block.contains("Karar: waiting"));
    assert!(block.contains("Best Ask: 0.0100"));
    assert!(block.contains("Floor: 0.5000"));
    assert!(!block.contains("Max Fiyat"));
    assert!(!block.contains("0.9900"));
}

#[test]
fn missed_market_no_order_diagnosis_explains_execution_floor_block() {
    let window_end_at = Utc::now();
    let output = json!({
        "reason": "pair_lock_primary_guard_waiting",
        "blocked": true,
        "market_slug": "btc-updown-5m-1",
        "no_candidate_guard": {
            "passed": false,
            "decision": "waiting",
            "reason_code": "below_best_ask_floor",
            "token_id": "down-token",
            "outcome_label": "Down",
            "best_bid": 0.02,
            "best_ask": 0.06,
            "execution_floor_guard": {
                "configured": true,
                "passed": false,
                "decision": "waiting",
                "reason_code": "below_best_ask_floor",
                "details": {
                    "best_ask": 0.06,
                    "best_ask_floor_price": 0.5
                }
            }
        },
        "yes_candidate_guard": {
            "passed": false,
            "decision": "waiting",
            "reason_code": "above_max_price",
            "token_id": "up-token",
            "outcome_label": "Up",
            "best_bid": 0.92,
            "best_ask": 0.94
        }
    });
    let steps = vec![TradeFlowRunStep {
        id: 1,
        run_id: 747,
        node_key: "action_1".to_string(),
        node_type: "action.place_order".to_string(),
        status: "completed".to_string(),
        attempt: 1,
        input_json: None,
        output_json: Some(output.clone()),
        error_text: None,
        started_at: Some(window_end_at - ChronoDuration::seconds(2)),
        ended_at: Some(window_end_at),
        available_at: window_end_at,
        parent_step_id: None,
        idempotency_key: None,
        created_at: window_end_at - ChronoDuration::seconds(2),
    }];
    let summary = no_fill_summary_from_action_place_order_output(&output, "down-token", "Down")
        .expect("summary");
    let diagnosis = build_no_order_base_diagnosis_payload(
        "btc-updown-5m-1",
        "down-token",
        "Down",
        window_end_at,
        &summary,
        &steps,
        None,
        None,
        Some(json!({
            "liquidity_regime": "LOW",
            "hourly_volume_ratio": 0.43,
            "volume_30s": 12.5,
            "trade_count_60s": 2,
            "liquidity_note": "Hacim normalin altinda."
        })),
    );
    let node_spec = WsOpenPositionPriceNodeSpec {
        node_key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        once_mode: true,
        once_scope_market: false,
        pair_lock_only_monitor: false,
        auto_scope: true,
        price_mode: WsPriceMode::Midpoint,
        market_slug: Some("btc-updown-5m-1".to_string()),
        token_id: "down-token".to_string(),
        outcome_label: "Down".to_string(),
        trigger_condition: "cross_below".to_string(),
        trigger_price: 0.0,
        max_price: None,
        price_to_beat_trigger_enabled: false,
        price_to_beat_mode: crate::trade_flow::guards::price_to_beat::PriceToBeatMode::Manual,
        price_to_beat_trigger_min_gap: None,
        price_to_beat_trigger_max_gap: None,
        price_to_beat_trigger_unit:
            crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Usd,
        protection_mode: TRIGGER_PROTECTION_MODE_OFF.to_string(),
        protection_asset: None,
        confirmation_ms: None,
        cycle_window_mode: None,
        cycle_window_secs: None,
        cycle_window_start_sec: None,
        cycle_window_end_sec: None,
        auto_sell_on_window_end: false,
    };
    let message =
        build_missed_market_no_order_notification_message(&node_spec, window_end_at, &diagnosis);

    assert!(message.contains("Order Status: NOT CREATED"));
    assert!(message.contains("Best Ask: 0.0600"));
    assert!(message.contains("Required Floor: 0.5000"));
    assert!(message.contains("Floor farki: -0.4400"));
    assert!(message.contains("Floor farki %: -88.00%"));
    assert!(message.contains("Book data status: complete_pair_book"));
    assert!(message.contains("Book side: Up"));
    assert!(message.contains("Liquidity Regime: LOW"));
    assert!(message.contains("Hourly volume ratio: 0.43x"));
    assert!(message.contains("Current: 0.0600 < 0.5000"));
    assert!(message.contains("condition_not_met_until_window_end"));
    assert!(message.contains("Bu bir fill kacirma degil; bot emir olusturmadi."));
}

#[test]
fn missed_market_no_order_trigger_condition_hides_floor_and_explains_book_status() {
    let window_end_at = Utc::now();
    let summary = no_fill_summary(
        "trigger_condition",
        "no_matching_block_event",
        Some("blocked"),
        json!({}),
        Some("no_matching_event"),
    );
    let diagnosis = build_no_order_base_diagnosis_payload(
        "btc-updown-5m-1",
        "down-token",
        "Down",
        window_end_at,
        &summary,
        &[],
        None,
        Some(&json!({
            "quote_snapshot_source": "final_fetch",
            "down_bid": 0.0600,
            "down_ask": 0.0700,
            "down_ask_depth_usdc": 0.7840
        })),
        Some(json!({
            "liquidity_regime": "UNKNOWN",
            "trade_count_60s": 0
        })),
    );
    let node_spec = WsOpenPositionPriceNodeSpec {
        node_key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        once_mode: true,
        once_scope_market: false,
        pair_lock_only_monitor: false,
        auto_scope: true,
        price_mode: WsPriceMode::Midpoint,
        market_slug: Some("btc-updown-5m-1".to_string()),
        token_id: "down-token".to_string(),
        outcome_label: "Down".to_string(),
        trigger_condition: "cross_below".to_string(),
        trigger_price: 0.0,
        max_price: None,
        price_to_beat_trigger_enabled: false,
        price_to_beat_mode: crate::trade_flow::guards::price_to_beat::PriceToBeatMode::Manual,
        price_to_beat_trigger_min_gap: None,
        price_to_beat_trigger_max_gap: None,
        price_to_beat_trigger_unit:
            crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Usd,
        protection_mode: TRIGGER_PROTECTION_MODE_OFF.to_string(),
        protection_asset: None,
        confirmation_ms: None,
        cycle_window_mode: None,
        cycle_window_secs: None,
        cycle_window_start_sec: None,
        cycle_window_end_sec: None,
        auto_sell_on_window_end: false,
    };
    let message =
        build_missed_market_no_order_notification_message(&node_spec, window_end_at, &diagnosis);

    assert!(message.contains("Emir Acilmadi - Trigger Sarti Saglanmadi"));
    assert!(message.contains("Karar: NO ORDER - trigger condition not met"));
    assert!(message.contains("Son Engel: Trigger Condition"));
    assert!(message.contains("Engel Kodu: no_matching_block_event"));
    assert!(!message.contains("Required Floor:"));
    assert!(!message.contains("Floor farki:"));
    assert!(!message.contains("Floor wait:"));
    assert!(message.contains("Note: No execution floor was evaluated for this event."));
    assert!(message.contains("Quote snapshot source: final_fetch"));
    assert!(message.contains("Book data status: selected_side_only"));
    assert!(message.contains("Quote missing reason: Up quote missing"));
    assert!(message.contains("Selected bid / ask / mid: 0.0600 / 0.0700 / 0.0650"));
    assert!(message.contains("Expected: guard_condition_passed = true"));
}

#[test]
fn missed_market_no_order_diagnosis_tracks_floor_wait_history() {
    let base_at = Utc::now();
    let output_at = |best_ask: f64, ended_at: DateTime<Utc>, id: i64| TradeFlowRunStep {
        id,
        run_id: 747,
        node_key: "action_1".to_string(),
        node_type: "action.place_order".to_string(),
        status: "completed".to_string(),
        attempt: 1,
        input_json: None,
        output_json: Some(json!({
            "blocked": true,
            "market_slug": "btc-updown-5m-1",
            "no_candidate_guard": {
                "passed": false,
                "decision": "waiting",
                "reason_code": "below_best_ask_floor",
                "token_id": "down-token",
                "outcome_label": "Down",
                "best_ask": best_ask,
                "execution_floor_guard": {
                    "passed": false,
                    "decision": "waiting",
                    "reason_code": "below_best_ask_floor",
                    "details": {
                        "best_ask": best_ask,
                        "best_ask_floor_price": 0.5
                    }
                }
            }
        })),
        error_text: None,
        started_at: Some(ended_at - ChronoDuration::milliseconds(100)),
        ended_at: Some(ended_at),
        available_at: ended_at,
        parent_step_id: None,
        idempotency_key: None,
        created_at: ended_at,
    };
    let steps = vec![
        output_at(0.04, base_at, 1),
        output_at(0.06, base_at + ChronoDuration::milliseconds(1_500), 2),
    ];
    let summary = no_fill_summary_from_action_place_order_output(
        steps[1].output_json.as_ref().expect("output"),
        "down-token",
        "Down",
    )
    .expect("summary");
    let diagnosis = build_no_order_base_diagnosis_payload(
        "btc-updown-5m-1",
        "down-token",
        "Down",
        base_at + ChronoDuration::milliseconds(1_500),
        &summary,
        &steps,
        None,
        None,
        None,
    );

    assert_eq!(
        diagnosis.get("floor_wait_ms").and_then(Value::as_i64),
        Some(1_500)
    );
    assert_eq!(
        diagnosis
            .get("floor_recovered_once")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        diagnosis
            .get("min_best_ask_during_wait")
            .and_then(Value::as_f64),
        Some(0.04)
    );
    assert_eq!(
        diagnosis
            .get("max_best_ask_during_wait")
            .and_then(Value::as_f64),
        Some(0.06)
    );
}

#[test]
fn no_order_liquidity_regime_uses_measured_ratio_thresholds() {
    assert_eq!(
        no_order_liquidity_regime_for_ratio(Some(0.49)),
        Some("VERY_LOW")
    );
    assert_eq!(no_order_liquidity_regime_for_ratio(Some(0.50)), Some("LOW"));
    assert_eq!(
        no_order_liquidity_regime_for_ratio(Some(0.80)),
        Some("NORMAL")
    );
    assert_eq!(
        no_order_liquidity_regime_for_ratio(Some(1.50)),
        Some("HIGH")
    );
    assert_eq!(
        no_order_liquidity_regime_for_ratio(Some(3.00)),
        Some("EXTREME")
    );
    assert_eq!(
        no_order_liquidity_note_for_ratio(Some(0.81)),
        None,
        "normal hacimde dusuk hacim notu uretilmemeli"
    );
}

#[test]
fn action_place_order_output_summary_uses_target_max_price_candidate() {
    let output = json!({
        "reason": "pair_lock_primary_guard_waiting",
        "blocked": true,
        "market_slug": "btc-updown-5m-1",
        "no_candidate_guard": {
            "passed": false,
            "decision": "waiting",
            "reason_code": "below_best_ask_floor",
            "token_id": "down-token",
            "outcome_label": "Down",
            "execution_floor_guard": {
                "configured": true,
                "decision": "waiting",
                "reason_code": "below_best_ask_floor",
                "details": {
                    "best_ask": 0.01,
                    "best_ask_floor_price": 0.5
                }
            }
        },
        "yes_candidate_guard": {
            "passed": false,
            "decision": "waiting",
            "reason_code": "above_max_price",
            "token_id": "up-token",
            "outcome_label": "Up",
            "max_price_guard": {
                "configured": true,
                "decision": "waiting",
                "reason_code": "above_max_price",
                "details": {
                    "reference_price": 0.99,
                    "reference_price_source": "best_ask",
                    "desired_price": 0.99,
                    "max_price": 0.75
                }
            }
        }
    });

    let summary =
        no_fill_summary_from_action_place_order_output(&output, "up-token", "Up").expect("summary");
    let block = build_no_fill_guard_summary_block(&summary);

    assert!(block.contains("Son Engel: Max Fiyat"));
    assert!(block.contains("Engel Kodu: above_max_price"));
    assert!(block.contains("Karar: waiting"));
    assert!(block.contains("Referans: 0.9900"));
    assert!(block.contains("Referans Kaynagi: best_ask"));
    assert!(block.contains("Max: 0.7500"));
    assert!(!block.contains("Execution Floor"));
}

#[test]
fn order_not_filled_notification_respects_zero_fill_only() {
    let mut order = test_builder_order("buy", None);
    order.notify_on_order_not_filled = true;

    assert!(should_send_order_not_filled_notification(&order));

    order.filled_qty = 1.0;
    assert!(!should_send_order_not_filled_notification(&order));
}

#[test]
fn order_not_filled_notification_respects_toggle() {
    let order = test_builder_order("buy", None);

    assert!(!should_send_order_not_filled_notification(&order));
}
