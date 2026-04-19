use super::support::*;
use super::*;

#[test]
fn pair_lock_build_nodes_preserve_supported_stop_loss_fields() {
    let node = test_node(json!({
        "mode": "pair_lock",
        "side": "buy",
        "executionMode": "market",
        "sizeUsdc": 5,
        "pairMaxTotalCent": 90,
        "counterLegEnabled": true,
        "counterLegSizeUsdc": 5,
        "counterLegOutcomeLabel": "opposite",
        "slEnabled": true,
        "slPriceCent": 45,
        "slTriggerPriceMode": "composite_safe",
        "ptbStopLossEnabled": true,
        "ptbStopLossGapUsd": 0.0,
        "ptbStopLossTimeDecayMode": "tighten",
        "notifyOnSlHit": true,
        "reenterOnSlHit": true,
        "reentryMaxAttempts": 2,
        "reentryCooldownSec": 15,
    }));
    let pair_lock = resolve_action_place_order_pair_lock_config(&node)
        .expect("pair lock config parse")
        .expect("pair lock config");

    let primary = build_pair_lock_single_leg_node(
        &node,
        "btc-updown-5m-1",
        "tok-up",
        "Up",
        "trigger_pair",
    );
    let counter = build_pair_lock_counter_leg_node(
        &node,
        "btc-updown-5m-1",
        &ActionPlaceOrderPairResolvedCounterLeg {
            token_id: "tok-down".to_string(),
            outcome_label: "Down".to_string(),
        },
        &pair_lock,
        "trigger_pair",
    );

    for candidate in [&primary, &counter] {
        assert_eq!(candidate.config.get("mode").and_then(Value::as_str), Some("single"));
        assert_eq!(candidate.config.get("slEnabled").and_then(Value::as_bool), Some(true));
        assert_eq!(candidate.config.get("slPriceCent").and_then(Value::as_i64), Some(45));
        assert_eq!(
            candidate.config.get("slTriggerPriceMode").and_then(Value::as_str),
            Some("composite_safe")
        );
        assert_eq!(
            candidate.config.get("ptbStopLossEnabled").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            candidate.config.get("ptbStopLossGapUsd").and_then(Value::as_f64),
            Some(0.0)
        );
        assert_eq!(
            candidate
                .config
                .get("ptbStopLossTimeDecayMode")
                .and_then(Value::as_str),
            Some("tighten")
        );
        assert_eq!(candidate.config.get("notifyOnSlHit").and_then(Value::as_bool), Some(true));
        assert_eq!(candidate.config.get("reenterOnSlHit").and_then(Value::as_bool), Some(true));
        assert_eq!(
            candidate.config.get("reentryMaxAttempts").and_then(Value::as_i64),
            Some(2)
        );
        assert_eq!(
            candidate.config.get("reentryCooldownSec").and_then(Value::as_i64),
            Some(15)
        );
        assert_eq!(
            candidate
                .config
                .get("reentryTriggerNodeKey")
                .and_then(Value::as_str),
            Some("trigger_pair")
        );
        assert!(candidate.config.get("tpEnabled").is_none());
        assert!(candidate.config.get("ptbStopLossRules").is_none());
        assert!(candidate.config.get("slRules").is_none());
    }
}

#[test]
fn pair_lock_buy_guard_eval_marks_max_price_as_waiting_when_retry_enabled() {
    let evaluation = evaluate_trade_builder_buy_guards(
        "market",
        Some("lead_candidate"),
        0.74,
        Some(0.75),
        0.75,
        None,
        Some(0.70),
        None,
        false,
        false,
        true,
    );

    assert_eq!(evaluation.effective_decision, "waiting");
    assert_eq!(evaluation.effective_reason_code, "above_max_price");
}

#[test]
fn pair_lock_buy_guard_eval_marks_execution_floor_as_waiting_when_retry_enabled() {
    let evaluation = evaluate_trade_builder_buy_guards(
        "market",
        Some("lead_candidate"),
        0.57,
        Some(0.57),
        0.57,
        None,
        Some(0.70),
        Some(0.80),
        false,
        true,
        false,
    );

    assert_eq!(evaluation.effective_decision, "waiting");
    assert_eq!(evaluation.effective_reason_code, "below_best_ask_floor");
}
