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
        "pairIgnoreStopLossAfterLocked": true,
        "counterLegEnabled": true,
        "counterLegSizeUsdc": 5,
        "counterLegOutcomeLabel": "opposite",
        "tpEnabled": true,
        "tpPriceCent": 95,
        "tpRules": [{ "priceCent": 95.0, "sizePct": 100.0 }],
        "notifyOnTpHit": true,
        "slEnabled": true,
        "slPriceCent": 45,
        "slTriggerPriceMode": "composite_safe",
        "slRules": [{ "priceCent": 45.0, "sizePct": 60.0 }, { "priceCent": 40.0, "sizePct": 40.0 }],
        "ptbStopLossEnabled": true,
        "ptbStopLossGapUsd": 0.0,
        "ptbStopLossGapUnit": "usd",
        "ptbStopLossTimeDecayMode": "tighten",
        "ptbStopLossRules": [{ "gapUsd": 7.0, "sizePct": 60.0 }, { "gapUsd": 0.0, "sizePct": 40.0 }],
        "notifyOnSlHit": true,
        "reenterOnSlHit": true,
        "reentryMaxAttempts": 2,
        "reentryCooldownSec": 15,
        "reentryMinPriceCent": 40,
        "reentryMaxPriceCent": 88,
        "reentryPriceToBeatMaxDiff": 3,
        "reentryPriceToBeatMaxDiffUnit": "usd",
        "reentrySkipCurrentWindow": true,
        "reentryThresholdDecay": 0.8,
        "reentryMaxPriceTightenBps": 500,
    }));
    let pair_lock = resolve_action_place_order_pair_lock_config(&node)
        .expect("pair lock config parse")
        .expect("pair lock config");
    assert!(pair_lock.ignore_stop_loss_after_locked);

    let primary = build_pair_lock_single_leg_node(
        &node,
        "btc-updown-5m-1",
        "tok-up",
        "Up",
        "trigger_pair",
        None,
        None,
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
        None,
    );

    for candidate in [&primary, &counter] {
        assert_eq!(
            candidate.config.get("mode").and_then(Value::as_str),
            Some("single")
        );
        assert_eq!(
            candidate
                .config
                .get("reenterOnSlHit")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            candidate
                .config
                .get("reentryMaxAttempts")
                .and_then(Value::as_i64),
            Some(2)
        );
        assert_eq!(
            candidate
                .config
                .get("reentryCooldownSec")
                .and_then(Value::as_i64),
            Some(15)
        );
        assert_eq!(
            candidate
                .config
                .get("reentryTriggerNodeKey")
                .and_then(Value::as_str),
            Some("trigger_pair")
        );
    }

    assert_eq!(
        primary.config.get("tpEnabled").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        primary.config.get("tpPriceCent").and_then(Value::as_i64),
        Some(95)
    );
    assert_eq!(primary.config.get("tpRules"), node.config.get("tpRules"));
    assert_eq!(
        primary.config.get("notifyOnTpHit").and_then(Value::as_bool),
        Some(true)
    );

    assert_eq!(
        primary.config.get("slEnabled").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        primary.config.get("slPriceCent").and_then(Value::as_i64),
        Some(45)
    );
    assert_eq!(
        primary
            .config
            .get("slTriggerPriceMode")
            .and_then(Value::as_str),
        Some("composite_safe")
    );
    assert_eq!(primary.config.get("slRules"), node.config.get("slRules"));
    assert_eq!(
        primary
            .config
            .get("ptbStopLossEnabled")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        primary
            .config
            .get("ptbStopLossGapUsd")
            .and_then(Value::as_f64),
        Some(0.0)
    );
    assert_eq!(
        primary
            .config
            .get("ptbStopLossGapUnit")
            .and_then(Value::as_str),
        Some("usd")
    );
    assert_eq!(
        primary
            .config
            .get("ptbStopLossTimeDecayMode")
            .and_then(Value::as_str),
        Some("tighten")
    );
    assert_eq!(
        primary.config.get("ptbStopLossRules"),
        node.config.get("ptbStopLossRules")
    );
    assert_eq!(
        primary.config.get("notifyOnSlHit").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        primary
            .config
            .get("reentryMinPriceCent")
            .and_then(Value::as_i64),
        Some(40)
    );
    assert_eq!(
        primary
            .config
            .get("reentryMaxPriceCent")
            .and_then(Value::as_i64),
        Some(88)
    );
    assert_eq!(
        primary
            .config
            .get("reentryPriceToBeatMaxDiff")
            .and_then(Value::as_i64),
        Some(3)
    );
    assert_eq!(
        primary
            .config
            .get("reentryPriceToBeatMaxDiffUnit")
            .and_then(Value::as_str),
        Some("usd")
    );
    assert_eq!(
        primary
            .config
            .get("reentrySkipCurrentWindow")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        primary
            .config
            .get("reentryThresholdDecay")
            .and_then(Value::as_f64),
        Some(0.8)
    );
    assert_eq!(
        primary
            .config
            .get("reentryMaxPriceTightenBps")
            .and_then(Value::as_i64),
        Some(500)
    );

    assert!(counter.config.get("tpEnabled").is_none());
    assert!(counter.config.get("tpPriceCent").is_none());
    assert!(counter.config.get("tpRules").is_none());
    assert!(counter.config.get("notifyOnTpHit").is_none());
    assert!(counter.config.get("slEnabled").is_none());
    assert!(counter.config.get("slPriceCent").is_none());
    assert!(counter.config.get("slTriggerPriceMode").is_none());
    assert!(counter.config.get("slRules").is_none());
    assert!(counter.config.get("ptbStopLossEnabled").is_none());
    assert!(counter.config.get("ptbStopLossGapUsd").is_none());
    assert!(counter.config.get("ptbStopLossGapUnit").is_none());
    assert!(counter.config.get("ptbStopLossTimeDecayMode").is_none());
    assert!(counter.config.get("ptbStopLossRules").is_none());
    assert!(counter.config.get("notifyOnSlHit").is_none());
    assert!(counter.config.get("reentryMinPriceCent").is_none());
    assert!(counter.config.get("reentryMaxPriceCent").is_none());
    assert!(counter.config.get("reentryPriceToBeatMaxDiff").is_none());
    assert!(counter
        .config
        .get("reentryPriceToBeatMaxDiffUnit")
        .is_none());
    assert!(counter.config.get("reentrySkipCurrentWindow").is_none());
    assert!(counter.config.get("reentryThresholdDecay").is_none());
    assert!(counter.config.get("reentryMaxPriceTightenBps").is_none());
}

#[test]
fn pair_lock_adaptive_max_price_override_only_changes_primary_child() {
    let node = test_node(json!({
        "mode": "pair_lock",
        "pairLockStrategy": "adaptive_max_price_v1",
        "side": "buy",
        "executionMode": "market",
        "sizeUsdc": 5,
        "maxPriceCent": 70,
        "pairMaxTotalCent": 96,
        "priceToBeatGuardEnabled": true,
        "priceToBeatMode": "iv_mismatch_edge",
        "counterLegEnabled": true,
        "counterLegSizeUsdc": 5,
        "counterLegOutcomeLabel": "opposite",
        "counterLegMaxPriceCent": 40,
    }));
    let pair_lock = resolve_action_place_order_pair_lock_config(&node)
        .expect("pair lock config parse")
        .expect("pair lock config");
    let adaptive = PairLockAdaptiveMaxPriceOverride {
        effective_max_price: 0.72,
        effective_size_usdc: 2.5,
        diagnostics: json!({"decision": "RELAX_ALLOW"}),
    };

    let primary = build_pair_lock_single_leg_node(
        &node,
        "btc-updown-5m-1",
        "tok-up",
        "Up",
        "trigger_pair",
        Some(&adaptive),
        None,
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
        None,
    );

    assert_eq!(
        primary.config.get("maxPriceCent").and_then(Value::as_f64),
        Some(72.0)
    );
    assert_eq!(
        primary.config.get("sizeUsdc").and_then(Value::as_f64),
        Some(2.5)
    );
    assert_eq!(
        primary
            .config
            .get("adaptiveMaxPriceApplied")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        counter.config.get("maxPriceCent").and_then(Value::as_i64),
        Some(40)
    );
    assert_eq!(
        counter.config.get("sizeUsdc").and_then(Value::as_f64),
        Some(5.0)
    );
    assert_eq!(
        counter
            .config
            .get(ACTION_PLACE_ORDER_INTERNAL_PAIR_LOCK_CHILD_ROLE_KEY)
            .and_then(Value::as_str),
        Some(TRADE_BUILDER_PAIR_ROLE_COUNTER_CANDIDATE)
    );
    assert_eq!(
        counter
            .config
            .get(ACTION_PLACE_ORDER_INTERNAL_INITIAL_STATUS_KEY)
            .and_then(Value::as_str),
        Some(ACTION_PLACE_ORDER_INTERNAL_BLOCKED_STATUS)
    );
    assert!(counter.config.get("adaptiveMaxPriceApplied").is_none());
    assert!(counter.config.get("adaptiveMaxPrice").is_none());
}

#[test]
fn pair_lock_manual_adaptive_override_tightens_primary_and_counter_cap() {
    let node = test_node(json!({
        "mode": "pair_lock",
        "pairLockStrategy": "manual_adaptive_risk_v1",
        "side": "buy",
        "executionMode": "market",
        "sizeUsdc": 5,
        "maxPriceCent": 70,
        "pairMaxTotalCent": 96,
        "priceToBeatGuardEnabled": true,
        "priceToBeatMode": "manual",
        "priceToBeatMaxDiff": 20,
        "priceToBeatMaxDiffUnit": "cent",
        "counterLegEnabled": true,
        "counterLegSizeUsdc": 5,
        "counterLegOutcomeLabel": "opposite",
        "counterLegMaxPriceCent": 70,
        "reenterOnSlHit": true,
    }));
    let pair_lock = resolve_action_place_order_pair_lock_config(&node)
        .expect("pair lock config parse")
        .expect("pair lock config");
    let manual = PairLockManualAdaptiveRiskOverride {
        effective_max_price: 0.58,
        effective_size_usdc: 1.5,
        counter_max_price: Some(0.37),
        diagnostics: json!({"decision": "ALLOW_STRICT"}),
    };

    let primary = build_pair_lock_single_leg_node(
        &node,
        "eth-updown-5m-1",
        "tok-up",
        "Up",
        "trigger_pair",
        None,
        Some(&manual),
    );
    let counter = build_pair_lock_counter_leg_node(
        &node,
        "eth-updown-5m-1",
        &ActionPlaceOrderPairResolvedCounterLeg {
            token_id: "tok-down".to_string(),
            outcome_label: "Down".to_string(),
        },
        &pair_lock,
        "trigger_pair",
        Some(&manual),
    );

    assert_eq!(
        primary.config.get("maxPriceCent").and_then(Value::as_f64),
        Some(58.0)
    );
    assert_eq!(
        primary.config.get("sizeUsdc").and_then(Value::as_f64),
        Some(1.5)
    );
    assert_eq!(
        primary
            .config
            .get("priceToBeatMaxDiff")
            .and_then(Value::as_f64),
        Some(20.0)
    );
    assert_eq!(
        primary
            .config
            .get("reenterOnSlHit")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        primary
            .config
            .get("manualAdaptiveRiskApplied")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        counter.config.get("maxPriceCent").and_then(Value::as_f64),
        Some(37.0)
    );
    assert_eq!(
        counter
            .config
            .get("manualAdaptiveRiskCounterCapApplied")
            .and_then(Value::as_bool),
        Some(true)
    );
}

#[test]
fn pair_lock_counter_leg_prefers_independent_stop_loss_fields() {
    let node = test_node(json!({
        "mode": "pair_lock",
        "side": "buy",
        "executionMode": "market",
        "sizeUsdc": 5,
        "pairMaxTotalCent": 90,
        "counterLegEnabled": true,
        "counterLegSizeUsdc": 5,
        "counterLegOutcomeLabel": "opposite",
        "tpEnabled": true,
        "tpPriceCent": 95,
        "tpRules": [{ "priceCent": 95.0, "sizePct": 100.0 }],
        "notifyOnTpHit": true,
        "slEnabled": true,
        "slPriceCent": 45,
        "slTriggerPriceMode": "composite_safe",
        "ptbStopLossEnabled": true,
        "ptbStopLossGapUsd": 0.0,
        "ptbStopLossGapUnit": "usd",
        "ptbStopLossCurrentPriceSource": "coinbase",
        "priceToBeatCurrentPriceSource": "binance",
        "ptbStopLossTimeDecayMode": "tighten",
        "notifyOnSlHit": true,
        "counterLegSlEnabled": true,
        "counterLegSlPriceCent": 38,
        "counterLegSlTriggerPriceMode": "best_bid",
        "counterLegPtbStopLossEnabled": true,
        "counterLegPtbStopLossGapUsd": -2.0,
        "counterLegPtbStopLossGapUnit": "cent",
        "counterLegPtbStopLossCurrentPriceSource": "binance",
        "counterLegPriceToBeatCurrentPriceSource": "coinbase",
        "counterLegPtbStopLossTimeDecayMode": "relax",
        "counterLegTpEnabled": true,
        "counterLegTpPriceCent": 82,
        "counterLegTpRules": [{ "priceCent": 82.0, "sizePct": 100.0 }],
        "counterLegNotifyOnTpHit": false,
        "counterLegNotifyOnSlHit": false,
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
        None,
        None,
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
        None,
    );

    assert_eq!(
        primary.config.get("slPriceCent").and_then(Value::as_i64),
        Some(45)
    );
    assert_eq!(
        primary
            .config
            .get("slTriggerPriceMode")
            .and_then(Value::as_str),
        Some("composite_safe")
    );
    assert_eq!(
        primary.config.get("tpPriceCent").and_then(Value::as_i64),
        Some(95)
    );
    assert_eq!(primary.config.get("tpRules"), node.config.get("tpRules"));
    assert_eq!(
        primary.config.get("notifyOnTpHit").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        primary
            .config
            .get("ptbStopLossGapUsd")
            .and_then(Value::as_f64),
        Some(0.0)
    );
    assert_eq!(
        primary
            .config
            .get("ptbStopLossGapUnit")
            .and_then(Value::as_str),
        Some("usd")
    );
    assert_eq!(
        primary
            .config
            .get("ptbStopLossCurrentPriceSource")
            .and_then(Value::as_str),
        Some("coinbase")
    );
    assert_eq!(
        primary
            .config
            .get("priceToBeatCurrentPriceSource")
            .and_then(Value::as_str),
        Some("binance")
    );
    assert_eq!(
        primary
            .config
            .get("ptbStopLossTimeDecayMode")
            .and_then(Value::as_str),
        Some("tighten")
    );
    assert_eq!(
        primary.config.get("notifyOnSlHit").and_then(Value::as_bool),
        Some(true)
    );

    assert_eq!(
        counter.config.get("tpEnabled").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        counter.config.get("tpPriceCent").and_then(Value::as_i64),
        Some(82)
    );
    assert_eq!(
        counter.config.get("tpRules"),
        node.config.get("counterLegTpRules")
    );
    assert_eq!(
        counter.config.get("notifyOnTpHit").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        counter.config.get("slPriceCent").and_then(Value::as_i64),
        Some(38)
    );
    assert_eq!(
        counter
            .config
            .get("slTriggerPriceMode")
            .and_then(Value::as_str),
        Some("best_bid")
    );
    assert_eq!(
        counter
            .config
            .get("ptbStopLossGapUsd")
            .and_then(Value::as_f64),
        Some(-2.0)
    );
    assert_eq!(
        counter
            .config
            .get("ptbStopLossGapUnit")
            .and_then(Value::as_str),
        Some("cent")
    );
    assert_eq!(
        counter
            .config
            .get("ptbStopLossCurrentPriceSource")
            .and_then(Value::as_str),
        Some("binance")
    );
    assert_eq!(
        counter
            .config
            .get("priceToBeatCurrentPriceSource")
            .and_then(Value::as_str),
        Some("coinbase")
    );
    assert_eq!(
        counter
            .config
            .get("ptbStopLossTimeDecayMode")
            .and_then(Value::as_str),
        Some("relax")
    );
    assert_eq!(
        counter.config.get("notifyOnSlHit").and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn pair_lock_counter_leg_keeps_stop_loss_fields_empty_when_counter_fields_missing() {
    let node = test_node(json!({
        "mode": "pair_lock",
        "side": "buy",
        "executionMode": "market",
        "sizeUsdc": 5,
        "pairMaxTotalCent": 90,
        "counterLegEnabled": true,
        "counterLegSizeUsdc": 5,
        "counterLegOutcomeLabel": "opposite",
        "tpEnabled": true,
        "tpPriceCent": 95,
        "tpRules": [{ "priceCent": 95.0, "sizePct": 100.0 }],
        "notifyOnTpHit": true,
        "slEnabled": true,
        "slPriceCent": 45,
        "slTriggerPriceMode": "composite_safe",
        "ptbStopLossEnabled": true,
        "ptbStopLossGapUsd": 0.0,
        "ptbStopLossGapUnit": "usd",
        "ptbStopLossTimeDecayMode": "tighten",
        "notifyOnSlHit": true,
    }));
    let pair_lock = resolve_action_place_order_pair_lock_config(&node)
        .expect("pair lock config parse")
        .expect("pair lock config");

    let counter = build_pair_lock_counter_leg_node(
        &node,
        "btc-updown-5m-1",
        &ActionPlaceOrderPairResolvedCounterLeg {
            token_id: "tok-down".to_string(),
            outcome_label: "Down".to_string(),
        },
        &pair_lock,
        "trigger_pair",
        None,
    );

    assert!(counter.config.get("tpEnabled").is_none());
    assert!(counter.config.get("tpPriceCent").is_none());
    assert!(counter.config.get("tpRules").is_none());
    assert!(counter.config.get("notifyOnTpHit").is_none());
    assert!(counter.config.get("slEnabled").is_none());
    assert!(counter.config.get("slPriceCent").is_none());
    assert!(counter.config.get("slTriggerPriceMode").is_none());
    assert!(counter.config.get("ptbStopLossEnabled").is_none());
    assert!(counter.config.get("ptbStopLossGapUsd").is_none());
    assert!(counter.config.get("ptbStopLossGapUnit").is_none());
    assert!(counter.config.get("ptbStopLossTimeDecayMode").is_none());
    assert!(counter.config.get("notifyOnSlHit").is_none());
}

#[test]
fn pair_lock_protective_unwind_defaults_on() {
    let node = test_node(json!({
        "mode": "pair_lock",
        "sizeUsdc": 5,
        "pairMaxTotalCent": 90,
        "counterLegEnabled": true,
        "counterLegSizeUsdc": 5,
    }));

    let pair_lock = resolve_action_place_order_pair_lock_config(&node)
        .expect("pair lock config parse")
        .expect("pair lock config");

    assert!(pair_lock.protective_unwind_enabled);
}

#[test]
fn pair_lock_disabled_protective_unwind_forces_counter_guard_retries() {
    let node = test_node(json!({
        "mode": "pair_lock",
        "sizeUsdc": 5,
        "pairMaxTotalCent": 90,
        "pairProtectiveUnwindEnabled": false,
        "counterLegEnabled": true,
        "counterLegSizeUsdc": 5,
        "counterLegOutcomeLabel": "opposite",
        "counterLegRetryOnMaxPriceBlock": false,
        "counterLegRetryOnExecutionFloorGuardBlock": false,
        "counterLegRetryOnPriceToBeatGuardBlock": false,
    }));
    let pair_lock = resolve_action_place_order_pair_lock_config(&node)
        .expect("pair lock config parse")
        .expect("pair lock config");
    let counter = build_pair_lock_counter_leg_node(
        &node,
        "btc-updown-5m-1",
        &ActionPlaceOrderPairResolvedCounterLeg {
            token_id: "tok-down".to_string(),
            outcome_label: "Down".to_string(),
        },
        &pair_lock,
        "trigger_pair",
        None,
    );

    assert!(!pair_lock.protective_unwind_enabled);
    assert_eq!(
        counter
            .config
            .get("retryOnMaxPriceBlock")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        counter
            .config
            .get("retryOnExecutionFloorGuardBlock")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        counter
            .config
            .get("retryOnPriceToBeatGuardBlock")
            .and_then(Value::as_bool),
        Some(true)
    );
}

fn test_pair_lock_session(
    status: &str,
    primary_order_id: i64,
    counter_order_id: i64,
) -> TradeBuilderPairSession {
    TradeBuilderPairSession {
        id: 17,
        user_id: 1,
        flow_definition_id: None,
        flow_run_id: None,
        flow_node_key: Some("action_1".to_string()),
        market_slug: "btc-updown-5m-1".to_string(),
        status: status.to_string(),
        pair_target_total_cent: 90.0,
        min_net_profit_usdc: 0.0,
        profit_safety_buffer_usdc: 0.0,
        orphan_grace_ms: 1500,
        ignore_stop_loss_after_locked: false,
        notify_on_pair_locked: false,
        notify_on_pair_unwind: false,
        notify_on_pair_no_edge: false,
        primary_order_id: Some(primary_order_id),
        counter_order_id: Some(counter_order_id),
        lead_order_id: Some(primary_order_id),
        primary_fill_qty: None,
        primary_fill_fee_qty: None,
        primary_net_qty: None,
        primary_avg_fill_price: None,
        counter_fill_qty: None,
        counter_fill_fee_qty: None,
        counter_net_qty: None,
        counter_avg_fill_price: None,
        lead_filled_at: None,
        locked_qty: None,
        projected_net_profit_usdc: None,
        last_error: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[test]
fn pair_lock_locked_session_skips_ptb_stop_loss_bump_for_candidate_orders() {
    let mut counter = test_builder_order("buy", None);
    counter.id = 12;
    counter.pair_session_id = Some(17);
    counter.pair_leg_role = Some(TRADE_BUILDER_PAIR_ROLE_COUNTER_CANDIDATE.to_string());

    let locked_session = test_pair_lock_session(TRADE_BUILDER_PAIR_STATUS_LOCKED, 11, 12);

    assert!(
        trade_builder_pair_lock_ptb_stop_loss_bump_should_skip_from_session(
            &locked_session,
            &counter,
        )
    );
}

#[test]
fn pair_lock_working_session_keeps_ptb_stop_loss_bump_enabled_for_lead_candidate() {
    let mut lead = test_builder_order("buy", None);
    lead.id = 11;
    lead.pair_session_id = Some(17);
    lead.pair_leg_role = Some(TRADE_BUILDER_PAIR_ROLE_LEAD_CANDIDATE.to_string());

    let working_session = test_pair_lock_session(TRADE_BUILDER_PAIR_STATUS_WORKING, 11, 12);

    assert!(
        !trade_builder_pair_lock_ptb_stop_loss_bump_should_skip_from_session(
            &working_session,
            &lead,
        )
    );
}

#[test]
fn pair_lock_buy_guard_eval_marks_max_price_as_waiting_when_retry_enabled() {
    let evaluation = evaluate_trade_builder_buy_guards(
        "market",
        Some("lead_candidate"),
        0.72,
        Some(0.73),
        0.73,
        None,
        Some(0.53),
        None,
        false,
        false,
        true,
    );

    assert_eq!(evaluation.effective_decision, "waiting");
    assert_eq!(evaluation.effective_reason_code, "above_max_price");
}

#[test]
fn pair_lock_counter_buy_guard_waits_when_current_ask_is_above_dynamic_cap() {
    let evaluation = evaluate_trade_builder_buy_guards(
        "market",
        Some("counter_candidate"),
        0.17,
        Some(0.17),
        0.17,
        None,
        Some(0.11),
        None,
        false,
        false,
        true,
    );

    assert_eq!(evaluation.effective_decision, "waiting");
    assert_eq!(evaluation.effective_reason_code, "above_max_price");
}

#[test]
fn pair_lock_counter_market_buy_uses_best_ask_submit_price_under_dynamic_cap() {
    let mut counter = test_builder_order("buy", None);
    counter.kind = "immediate".to_string();
    counter.trigger_condition = None;
    counter.trigger_price = None;
    counter.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    counter.size_usdc = 4.1985;
    counter.target_qty = Some(9.33);
    counter.remaining_qty = Some(9.33);
    counter.max_price = Some(0.45);
    counter.pair_leg_role = Some("counter_candidate".to_string());

    let resolution = trade_builder_market_buy_execution_price(&counter, 0.39, Some(0.41))
        .expect("counter market buy price");
    let evaluation = evaluate_trade_builder_buy_guards(
        "market",
        counter.pair_leg_role.as_deref(),
        0.39,
        Some(0.41),
        resolution.price,
        None,
        counter.max_price,
        None,
        false,
        false,
        true,
    );

    assert_eq!(resolution.price, 0.41);
    assert_eq!(resolution.source, "best_ask");
    assert_eq!(evaluation.effective_decision, "passed");
    assert_eq!(evaluation.effective_reason_code, "guards_passed");
}

#[test]
fn pair_lock_counter_market_buy_waits_when_best_ask_exceeds_dynamic_cap() {
    let evaluation = evaluate_trade_builder_buy_guards(
        "market",
        Some("counter_candidate"),
        0.39,
        Some(0.46),
        0.46,
        None,
        Some(0.45),
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
