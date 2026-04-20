use super::support::*;
use super::*;

#[test]
fn root_fill_notification_uses_order_filled_type() {
    let mut order = test_builder_order("buy", None);
    order.notify_on_fill = true;

    let (notification_type, message) =
        build_trade_builder_fill_notification(&order, 0.61, 12.5).expect("notification");

    assert_eq!(notification_type, "order_filled");
    assert!(message.contains("Emir Doldu"));
    assert!(message.contains("Sebep: Emir basariyla dolduruldu."));
    assert!(message.contains("Notional USDC: 5.00"));
    assert!(message.contains("Adet: 12.50"));
    assert!(message.contains("Outcome: Up"));
}

#[test]
fn tp_child_fill_notification_uses_tp_type() {
    let mut order = test_builder_order("sell", Some(41));
    order.notify_on_fill = true;
    order.trigger_condition = Some("cross_above".to_string());

    let (notification_type, message) =
        build_trade_builder_fill_notification(&order, 0.82, 4.2).expect("notification");

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
        build_trade_builder_fill_notification(&order, 0.37, 4.2).expect("notification");

    assert_eq!(notification_type, "sl_hit");
    assert!(message.contains("Stop Loss Tetiklendi"));
    assert!(message.contains("Sebep: Stop loss seviyesi goruldugu icin cikis emri dolduruldu."));
}

#[test]
fn fill_notification_respects_toggle() {
    let order = test_builder_order("buy", None);

    assert!(build_trade_builder_fill_notification(&order, 0.51, 3.0).is_none());
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
