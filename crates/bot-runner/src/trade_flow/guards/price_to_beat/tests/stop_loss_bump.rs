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

    let resolved =
        resolve_action_place_order_price_to_beat_guard_resolution(&node, &context, BTC_MARKET_5M)
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

    let resolved =
        resolve_action_place_order_price_to_beat_guard_resolution(&node, &context, BTC_MARKET_5M)
            .expect("ptb bump resolution");

    assert_close_option(resolved.threshold_value, 7.0, 1e-9);
    assert_eq!(resolved.threshold_unit, PriceToBeatDiffUnit::Usd);
    assert_close_option(resolved.effective_threshold_usd, 7.0, 1e-9);
    assert_eq!(resolved.stop_loss_bump_applied_count, 2);
}

#[test]
fn stop_loss_bump_excludes_current_market_increment() {
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

    let resolved =
        resolve_action_place_order_price_to_beat_guard_resolution(&node, &context, BTC_MARKET_5M)
            .expect("ptb bump resolution");

    assert_close_option(resolved.threshold_value, 90.0, 1e-9);
    assert_eq!(resolved.stop_loss_bump_count, 2);
    assert_eq!(resolved.stop_loss_bump_applied_count, 1);
    assert!(resolved.stop_loss_bump_current_market_excluded);
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

    let resolved =
        resolve_action_place_order_price_to_beat_guard_resolution(&node, &context, BTC_MARKET_5M)
            .expect("ptb bump resolution");

    assert_eq!(resolved.stop_loss_bump_amount, Some(10.0));
    assert_eq!(resolved.stop_loss_bump_max_value, Some(30.0));
    assert_close(resolved.stop_loss_bump_raw_usd, 0.4, 1e-9);
    assert_close(resolved.stop_loss_bump_usd, 0.3, 1e-9);
    assert!(resolved.stop_loss_bump_capped);
    assert!(resolved.stop_loss_bump_max_reached);
    assert_close_option(resolved.threshold_value, 110.0, 1e-9);
}
