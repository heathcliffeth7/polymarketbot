use super::support::*;
use super::*;

#[test]
fn place_order_existing_order_reuses_active_matching_order() {
    let mut order = test_builder_order("buy", None);
    order.status = "open".to_string();

    assert_eq!(
        classify_action_place_order_existing_order(
            &order,
            "buy",
            77,
            "btc-updown-5m-1",
            "tok-up",
            "conditional",
            "market",
        ),
        ActionPlaceOrderExistingOrderDecision::ReuseActive
    );
}

#[test]
fn place_order_existing_order_rearms_matching_sell_error() {
    let mut order = test_builder_order("sell", Some(9));
    order.status = "error".to_string();
    order.kind = "immediate".to_string();
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.target_qty = Some(5.10);
    order.remaining_qty = Some(5.10);

    assert_eq!(
        classify_action_place_order_existing_order(
            &order,
            "sell",
            77,
            "btc-updown-5m-1",
            "tok-up",
            "immediate",
            "market",
        ),
        ActionPlaceOrderExistingOrderDecision::RearmErrorSell
    );
}

#[test]
fn place_order_existing_order_ignores_market_mismatch() {
    let order = test_builder_order("buy", None);

    assert_eq!(
        classify_action_place_order_existing_order(
            &order,
            "buy",
            77,
            "btc-updown-5m-2",
            "tok-up",
            "conditional",
            "market",
        ),
        ActionPlaceOrderExistingOrderDecision::Ignore("market_slug_mismatch")
    );
}

#[test]
fn place_order_existing_order_ignores_terminal_status() {
    let mut order = test_builder_order("buy", None);
    order.status = "completed".to_string();

    assert_eq!(
        classify_action_place_order_existing_order(
            &order,
            "buy",
            77,
            "btc-updown-5m-1",
            "tok-up",
            "conditional",
            "market",
        ),
        ActionPlaceOrderExistingOrderDecision::Ignore("terminal_status")
    );
}

#[test]
fn set_flow_ref_removes_key_when_value_is_null() {
    let mut context = json!({
        "flowContext": {},
        "vars": {},
        "state": {},
        "refs": { "preset_place_order": 859 },
        "nodeState": {}
    });

    set_flow_ref(&mut context, "preset_place_order", Value::Null);

    assert!(context
        .get("refs")
        .and_then(|refs| refs.get("preset_place_order"))
        .is_none());
}

#[test]
fn place_order_resolves_runtime_binding_from_step_input() {
    let node = test_node(json!({}));
    let step = test_step(json!({
        "market_slug": "btc-updown-5m-1772729700",
        "triggered_token_id": "tok-up",
        "triggered_outcome_label": "Up",
        "triggered_price": 0.875,
        "sourceTradeId": 77
    }));
    let context = json!({
        "flowContext": {},
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {}
    });

    assert_eq!(
        resolve_action_place_order_string(
            &node,
            &context,
            &step,
            "marketSlug",
            "marketSlug",
            &["market_slug", "marketSlug", "wsMarketSlug"],
        )
        .as_deref(),
        Some("btc-updown-5m-1772729700")
    );
    assert_eq!(
        resolve_action_place_order_string(
            &node,
            &context,
            &step,
            "tokenId",
            "tokenId",
            &["triggered_token_id", "tokenId"],
        )
        .as_deref(),
        Some("tok-up")
    );
    assert_eq!(
        resolve_action_place_order_string(
            &node,
            &context,
            &step,
            "outcomeLabel",
            "outcomeLabel",
            &["triggered_outcome_label", "outcomeLabel"],
        )
        .as_deref(),
        Some("Up")
    );
    assert_eq!(
        step_input_i64(&step, &["sourceTradeId", "source_trade_id"]),
        Some(77)
    );
    assert_eq!(
        resolve_action_place_order_reference_price(&node, &step),
        Some(0.875)
    );
}

#[test]
fn place_order_resolves_inherited_max_price_from_context_before_step_input() {
    let step = test_step(json!({
        "max_price": 0.95,
        "maxPrice": 0.94
    }));
    let context = json!({
        "flowContext": { "maxPrice": 0.9 },
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {}
    });

    assert_eq!(
        resolve_action_place_order_max_price(&context, &step),
        Some(0.9)
    );

    let empty_context = json!({
        "flowContext": {},
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {}
    });
    assert_eq!(
        resolve_action_place_order_max_price(&empty_context, &step),
        Some(0.95)
    );
}

#[test]
fn resolve_flow_source_trade_id_ignores_non_positive_values() {
    let node_missing = test_node(json!({ "sourceTradeId": 0 }));
    let context_missing = json!({
        "flowContext": { "sourceTradeId": 0 },
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {}
    });
    assert_eq!(
        resolve_flow_source_trade_id(&node_missing, &context_missing),
        None
    );

    let node_config = test_node(json!({ "sourceTradeId": 12 }));
    assert_eq!(
        resolve_flow_source_trade_id(&node_config, &context_missing),
        Some(12)
    );

    let node_context = test_node(json!({}));
    let context_positive = json!({
        "flowContext": { "sourceTradeId": 34 },
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {}
    });
    assert_eq!(
        resolve_flow_source_trade_id(&node_context, &context_positive),
        Some(34)
    );
}

#[test]
fn place_order_exit_price_parses_tp_and_sl_from_cent_values() {
    let node = test_node(json!({
        "tpPriceCent": 98,
        "slPriceCent": 72
    }));

    assert_eq!(
        resolve_action_place_order_exit_price(&node, "buy", true, "tpPriceCent", "tpPrice", "tp")
            .unwrap(),
        Some(0.98)
    );
    assert_eq!(
        resolve_action_place_order_exit_price(&node, "buy", true, "slPriceCent", "slPrice", "sl")
            .unwrap(),
        Some(0.72)
    );
}

#[test]
fn place_order_exit_price_rejects_sell_side() {
    let node = test_node(json!({ "slPriceCent": 72 }));
    let err =
        resolve_action_place_order_exit_price(&node, "sell", true, "slPriceCent", "slPrice", "sl")
            .unwrap_err()
            .to_string();
    assert!(err.contains("slEnabled is only valid for side=buy"));
}

#[test]
fn oco_cancel_only_applies_to_child_sell_orders_with_live_status() {
    let sell_child = test_builder_order("sell", Some(9));
    let buy_parent = test_builder_order("buy", None);
    let sell_without_parent = test_builder_order("sell", None);

    assert!(should_request_trade_builder_oco_cancel(&sell_child, "open"));
    assert!(should_request_trade_builder_oco_cancel(
        &sell_child,
        "partially_filled"
    ));
    assert!(should_request_trade_builder_oco_cancel(
        &sell_child,
        "filled"
    ));
    assert!(!should_request_trade_builder_oco_cancel(
        &sell_child,
        "rejected"
    ));
    assert!(!should_request_trade_builder_oco_cancel(
        &buy_parent,
        "open"
    ));
    assert!(!should_request_trade_builder_oco_cancel(
        &sell_without_parent,
        "open"
    ));
}

#[test]
fn normalize_exchange_status_treats_matched_as_filled() {
    assert_eq!(normalize_exchange_status("matched"), "filled");
    assert_eq!(normalize_exchange_status("MATCHED"), "filled");
}

#[test]
fn matched_cancel_errors_are_treated_as_terminal_match() {
    assert!(cancel_error_indicates_terminal_match(
        "matched orders can't be canceled"
    ));
    assert!(cancel_error_indicates_terminal_match(
        "cannot cancel order because it is already matched"
    ));
    assert!(!cancel_error_indicates_terminal_match(
        "not enough balance / allowance"
    ));
}

#[test]
fn desired_price_above_builder_max_price_is_blocked() {
    let mut order = test_builder_order("buy", None);
    order.max_price = Some(0.90);

    assert!(!trade_builder_price_exceeds_max_price(&order, 0.90));
    assert!(trade_builder_price_exceeds_max_price(&order, 0.91));
}

#[test]
fn trigger_market_price_rejects_cross_when_above_max_price() {
    let (matched, reason) = evaluate_trigger_market_price_condition(
        Some(0.76),
        0.93,
        0.77,
        "cross_above",
        true,
        Some(0.90),
    );

    assert!(!matched);
    assert_eq!(reason, "above_max_price");
}

#[test]
fn trigger_market_price_triggers_when_reentering_band_from_above() {
    let (matched, reason) = evaluate_trigger_market_price_condition(
        Some(0.95),
        0.85,
        0.77,
        "cross_above",
        false,
        Some(0.90),
    );

    assert!(matched);
    assert_eq!(reason, "range_entry_from_above");
}

#[test]
fn trigger_market_price_does_not_retrigger_while_remaining_in_band() {
    let (matched, reason) = evaluate_trigger_market_price_condition(
        Some(0.82),
        0.85,
        0.77,
        "cross_above",
        false,
        Some(0.90),
    );

    assert!(!matched);
    assert_eq!(reason, "no_cross");
}
