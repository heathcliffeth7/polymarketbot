use super::*;

#[test]
fn stop_loss_bump_adds_cent_threshold_after_previous_market_sl() {
    let node = test_action_place_order_node(json!({
        "priceToBeatGuardEnabled": true,
        "priceToBeatMode": "manual",
        "priceToBeatMaxDiff": 80,
        "priceToBeatMaxDiffUnit": "cent",
        "priceToBeatStopLossBumpEnabled": true,
        "priceToBeatStopLossBumpAmount": 10,
        "priceToBeatStopLossBumpUnit": "cent",
        "priceToBeatStopLossBumpScope": "global"
    }));
    let context = json!({
        "nodeState": {
            "action_1": {
                "ptb_stop_loss_bump_count": 1,
                "ptb_stop_loss_bump_accumulated_usd": 0.1,
                "ptb_stop_loss_bump_last_increment_usd": 0.1,
                "ptb_stop_loss_bump_last_market_slug": "btc-updown-5m-1774012800"
            }
        }
    });

    let resolved = resolve_action_place_order_price_to_beat_guard_resolution(
        &node,
        &context,
        BTC_MARKET_5M,
        "Up",
    )
    .expect("ptb bump resolution");

    assert_eq!(resolved.base_threshold_value, Some(80.0));
    assert_close_option(resolved.threshold_value, 90.0, 1e-9);
    assert_eq!(resolved.threshold_unit, PriceToBeatDiffUnit::Cent);
    assert_close_option(resolved.effective_threshold_usd, 0.9, 1e-9);
    assert_eq!(resolved.stop_loss_bump_count, 1);
    assert_eq!(resolved.stop_loss_bump_applied_count, 1);
    assert_close(resolved.stop_loss_bump_usd, 0.1, 1e-9);
}

#[test]
fn stop_loss_bump_adds_usd_threshold_cumulatively() {
    let node = test_action_place_order_node(json!({
        "priceToBeatGuardEnabled": true,
        "priceToBeatMode": "manual",
        "priceToBeatMaxDiff": 5,
        "priceToBeatMaxDiffUnit": "usd",
        "priceToBeatStopLossBumpEnabled": true,
        "priceToBeatStopLossBumpAmount": 1,
        "priceToBeatStopLossBumpUnit": "usd",
        "priceToBeatStopLossBumpScope": "global"
    }));
    let context = json!({
        "nodeState": {
            "action_1": {
                "ptb_stop_loss_bump_count": 2,
                "ptb_stop_loss_bump_accumulated_usd": 2.0,
                "ptb_stop_loss_bump_last_increment_usd": 1.0,
                "ptb_stop_loss_bump_last_market_slug": "btc-updown-5m-1774012800"
            }
        }
    });

    let resolved = resolve_action_place_order_price_to_beat_guard_resolution(
        &node,
        &context,
        BTC_MARKET_5M,
        "Up",
    )
    .expect("ptb bump resolution");

    assert_close_option(resolved.threshold_value, 7.0, 1e-9);
    assert_eq!(resolved.threshold_unit, PriceToBeatDiffUnit::Usd);
    assert_close_option(resolved.effective_threshold_usd, 7.0, 1e-9);
    assert_eq!(resolved.stop_loss_bump_applied_count, 2);
}

#[test]
fn stop_loss_bump_keeps_current_market_increment_in_effective_threshold() {
    let node = test_action_place_order_node(json!({
        "priceToBeatGuardEnabled": true,
        "priceToBeatMode": "manual",
        "priceToBeatMaxDiff": 80,
        "priceToBeatMaxDiffUnit": "cent",
        "priceToBeatStopLossBumpEnabled": true,
        "priceToBeatStopLossBumpAmount": 10,
        "priceToBeatStopLossBumpUnit": "cent",
        "priceToBeatStopLossBumpScope": "global"
    }));
    let context = json!({
        "nodeState": {
            "action_1": {
                "ptb_stop_loss_bump_count": 2,
                "ptb_stop_loss_bump_accumulated_usd": 0.2,
                "ptb_stop_loss_bump_last_increment_usd": 0.1,
                "ptb_stop_loss_bump_last_market_slug": BTC_MARKET_5M
            }
        }
    });

    let resolved = resolve_action_place_order_price_to_beat_guard_resolution(
        &node,
        &context,
        BTC_MARKET_5M,
        "Up",
    )
    .expect("ptb bump resolution");

    assert_close_option(resolved.threshold_value, 100.0, 1e-9);
    assert_eq!(resolved.stop_loss_bump_count, 2);
    assert_eq!(resolved.stop_loss_bump_applied_count, 2);
    assert_close(resolved.stop_loss_bump_usd, 0.2, 1e-9);
    assert!(!resolved.stop_loss_bump_current_market_excluded);
}

#[test]
fn stop_loss_bump_respects_configured_max_limit() {
    let node = test_action_place_order_node(json!({
        "priceToBeatGuardEnabled": true,
        "priceToBeatMode": "manual",
        "priceToBeatMaxDiff": 80,
        "priceToBeatMaxDiffUnit": "cent",
        "priceToBeatStopLossBumpEnabled": true,
        "priceToBeatStopLossBumpAmount": 10,
        "priceToBeatStopLossBumpMaxValue": 30,
        "priceToBeatStopLossBumpUnit": "cent",
        "priceToBeatStopLossBumpScope": "global"
    }));
    let context = json!({
        "nodeState": {
            "action_1": {
                "ptb_stop_loss_bump_count": 4,
                "ptb_stop_loss_bump_accumulated_usd": 0.4,
                "ptb_stop_loss_bump_last_increment_usd": 0.1,
                "ptb_stop_loss_bump_last_market_slug": "btc-updown-5m-1774012800"
            }
        }
    });

    let resolved = resolve_action_place_order_price_to_beat_guard_resolution(
        &node,
        &context,
        BTC_MARKET_5M,
        "Up",
    )
    .expect("ptb bump resolution");

    assert_eq!(resolved.stop_loss_bump_amount, Some(10.0));
    assert_eq!(resolved.stop_loss_bump_max_value, Some(30.0));
    assert_close(resolved.stop_loss_bump_raw_usd, 0.4, 1e-9);
    assert_close(resolved.stop_loss_bump_usd, 0.3, 1e-9);
    assert!(resolved.stop_loss_bump_capped);
    assert!(resolved.stop_loss_bump_max_reached);
    assert_close_option(resolved.threshold_value, 110.0, 1e-9);
}

#[test]
fn stop_loss_bump_uses_persisted_effective_ptb_state() {
    let node = test_action_place_order_node(json!({
        "priceToBeatGuardEnabled": true,
        "priceToBeatMode": "manual",
        "priceToBeatMaxDiff": 100,
        "priceToBeatMaxDiffUnit": "cent",
        "priceToBeatStopLossBumpEnabled": true,
        "priceToBeatStopLossBumpAmount": 25,
        "priceToBeatStopLossBumpUnit": "cent",
        "priceToBeatStopLossBumpScope": "per_scope",
        "outcomeLabel": "Up"
    }));
    let context = json!({
        "nodeState": {
            "action_1": {
                "ptb_stop_loss_bump_scope_map": {
                    "btc:5m:up": {
                        "count": 3,
                        "accumulated_bump_usd": 0.75,
                        "last_increment_usd": 0.25,
                        "last_market_slug": "btc-updown-5m-1774012800",
                        "ptb_current_effective_threshold_usd": 0.6,
                        "ptb_current_effective_bump_usd": 0.75,
                        "ptb_current_effective_source": "relax"
                    }
                }
            }
        }
    });

    let resolved = resolve_action_place_order_price_to_beat_guard_resolution(
        &node,
        &context,
        BTC_MARKET_5M,
        "Up",
    )
    .expect("ptb state resolution");

    assert_close_option(resolved.threshold_value, 60.0, 1e-9);
    assert_close_option(resolved.current_effective_ptb_usd, 0.6, 1e-9);
    assert_close(resolved.stop_loss_bump_usd, 0.75, 1e-9);
}

#[test]
fn stop_loss_bump_decay_reduces_persisted_effective_ptb_state_without_resetting_upward() {
    let current_market = "btc-updown-5m-1774013400";
    let node = test_action_place_order_node(json!({
        "priceToBeatGuardEnabled": true,
        "priceToBeatMode": "manual",
        "priceToBeatMaxDiff": 100,
        "priceToBeatMaxDiffUnit": "cent",
        "priceToBeatStopLossBumpEnabled": true,
        "priceToBeatStopLossBumpAmount": 25,
        "priceToBeatStopLossBumpUnit": "cent",
        "priceToBeatStopLossBumpScope": "global",
        "priceToBeatStopLossBumpDecayWindows": 1
    }));
    let context = json!({
        "nodeState": {
            "action_1": {
                "ptb_stop_loss_bump_count": 3,
                "ptb_stop_loss_bump_accumulated_usd": 0.75,
                "ptb_stop_loss_bump_last_increment_usd": 0.25,
                "ptb_stop_loss_bump_last_market_slug": "btc-updown-5m-1774013100",
                "ptb_current_effective_threshold_usd": 0.6,
                "ptb_current_effective_bump_usd": 0.75,
                "ptb_current_effective_source": "relax"
            }
        }
    });

    let resolved = resolve_action_place_order_price_to_beat_guard_resolution(
        &node,
        &context,
        current_market,
        "Up",
    )
    .expect("ptb decay resolution");

    assert_close(resolved.stop_loss_bump_usd, 0.5, 1e-9);
    assert_close_option(resolved.current_effective_ptb_usd, 0.35, 1e-9);
    assert_close_option(resolved.threshold_value, 35.0, 1e-9);
}

#[test]
fn stop_loss_bump_authoritative_threshold_uses_persisted_state_with_manual_base_floor() {
    let node = test_action_place_order_node(json!({
        "priceToBeatGuardEnabled": true,
        "priceToBeatMode": "manual",
        "priceToBeatMaxDiff": 100,
        "priceToBeatMaxDiffUnit": "cent",
        "priceToBeatStopLossBumpEnabled": true,
        "priceToBeatStopLossBumpAmount": 25,
        "priceToBeatStopLossBumpUnit": "cent",
        "priceToBeatStopLossBumpScope": "global"
    }));
    let context = json!({
        "nodeState": {
            "action_1": {
                "ptb_current_effective_threshold_usd": 0.9,
                "ptb_current_effective_bump_usd": 1.25,
                "ptb_current_effective_source": "relax",
                "ptb_stop_loss_bump_count": 5,
                "ptb_stop_loss_bump_accumulated_usd": 1.25,
                "ptb_stop_loss_bump_last_increment_usd": 0.25,
                "ptb_stop_loss_bump_last_market_slug": "btc-updown-5m-1774013100"
            }
        }
    });

    let authoritative_previous =
        crate::resolve_action_place_order_ptb_authoritative_previous_effective_threshold_usd(
            &context,
            &node,
            "action_1",
            BTC_MARKET_5M,
            "Up",
            Some(2.25),
            Some(1.0),
        );

    assert_close_option(authoritative_previous, 1.0, 1e-9);
    assert_close_option(
        crate::resolve_action_place_order_ptb_next_effective_threshold_usd(
            authoritative_previous,
            0.25,
        ),
        1.25,
        1e-9,
    );
}

#[tokio::test]
async fn live_ptb_snapshot_recomputes_threshold_without_flow_context_snapshot() {
    let _guard = lock_price_to_beat_test_state();
    let market_slug = "eth-updown-5m-1774117900";
    let node = test_action_place_order_node(
        json!({"priceToBeatGuardEnabled":true,"priceToBeatMode":"manual","priceToBeatMaxDiff":80,"priceToBeatMaxDiffUnit":"cent","priceToBeatStopLossBumpEnabled":true,"priceToBeatStopLossBumpAmount":10,"priceToBeatStopLossBumpUnit":"cent","priceToBeatStopLossBumpScope":"global"}),
    );
    let context = json!({"nodeState":{"action_1":{"ptb_stop_loss_bump_count":1,"ptb_stop_loss_bump_accumulated_usd":0.1,"ptb_stop_loss_bump_last_increment_usd":0.1,"ptb_stop_loss_bump_last_market_slug":"eth-updown-5m-1774117600"}}});
    crate::trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
        "eth",
        &[(chrono::Utc::now().timestamp_millis(), 2_001.0)],
    )
    .expect("seed eth tick");
    assert!(
        crate::trade_flow::guards::polymarket_price_to_beat::seed_price_to_beat_from_chainlink(
            market_slug,
            "eth",
            "5m",
            2_000.0,
            None
        )
    );
    let snapshot = crate::resolve_action_place_order_ptb_stop_loss_bump_live_ptb_snapshot(
        None,
        &node,
        &context,
        None,
        market_slug,
        "Up",
    )
    .await
    .expect("live ptb snapshot");
    assert_eq!(snapshot.unit, "cent");
    assert_close(snapshot.value, 90.0, 1e-9);
    assert_close(snapshot.usd, 0.9, 1e-9);
}

#[tokio::test]
async fn live_ptb_snapshot_returns_none_when_auto_threshold_is_not_ready() {
    let _guard = lock_price_to_beat_test_state();
    let node = test_action_place_order_node(
        json!({"priceToBeatGuardEnabled":true,"priceToBeatMode":"auto_last_3_avg_excursion","priceToBeatStopLossBumpEnabled":true,"priceToBeatStopLossBumpAmount":10,"priceToBeatStopLossBumpUnit":"cent","priceToBeatStopLossBumpScope":"global"}),
    );
    let context = json!({"nodeState":{"action_1":{"ptb_stop_loss_bump_count":1,"ptb_stop_loss_bump_accumulated_usd":0.1,"ptb_stop_loss_bump_last_increment_usd":0.1,"ptb_stop_loss_bump_last_market_slug":"eth-updown-5m-1774117600"}}});
    assert!(
        crate::resolve_action_place_order_ptb_stop_loss_bump_live_ptb_snapshot(
            None,
            &node,
            &context,
            None,
            "eth-updown-5m-1774117900",
            "Up"
        )
        .await
        .is_none()
    );
}
