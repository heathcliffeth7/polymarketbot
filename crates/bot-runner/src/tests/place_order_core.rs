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
fn place_order_existing_order_reuses_guard_blocked_matching_order() {
    let mut order = test_builder_order("buy", None);
    order.status = "guard_blocked".to_string();

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
    assert_eq!(resolve_action_place_order_guard_trigger_price(&step), None);
}

#[test]
fn place_order_resolves_runtime_binding_prefers_step_input_over_flow_context() {
    let node = test_node(json!({}));
    let step = test_step(json!({
        "market_slug": "btc-updown-5m-1772729700",
        "triggered_token_id": "tok-down",
        "triggered_outcome_label": "Down",
    }));
    let context = json!({
        "flowContext": {
            "marketSlug": "btc-updown-5m-stale",
            "tokenId": "tok-up",
            "outcomeLabel": "Up"
        },
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
        Some("tok-down")
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
        Some("Down")
    );
}

#[test]
fn stale_place_order_retry_detects_old_step_market() {
    let node = test_node(json!({}));
    let step = test_step(json!({
        "market_slug": "btc-updown-5m-1772729700",
        "triggered_token_id": "tok-up",
        "triggered_outcome_label": "Up",
    }));
    let graph = runtime_graph(
        vec![
            ("trigger_1", "trigger.market_price"),
            ("action_1", "action.place_order"),
        ],
        vec![("trigger_1", "action_1")],
    );
    let context = json!({
        "flowContext": {
            "marketSlug": "btc-updown-5m-1772730000"
        },
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {}
    });

    assert_eq!(
        resolve_action_place_order_stale_market_retry(&node, &context, &step, &graph),
        Some(ActionPlaceOrderStaleMarketRetry {
            stale_market_slug: "btc-updown-5m-1772729700".to_string(),
            current_market_slug: "btc-updown-5m-1772730000".to_string(),
        })
    );
}

#[test]
fn stale_place_order_retry_ignores_explicit_market_config() {
    let node = test_node(json!({ "marketSlug": "btc-updown-5m-fixed" }));
    let step = test_step(json!({
        "market_slug": "btc-updown-5m-1772729700",
    }));
    let graph = runtime_graph(
        vec![
            ("trigger_1", "trigger.market_price"),
            ("action_1", "action.place_order"),
        ],
        vec![("trigger_1", "action_1")],
    );
    let context = json!({
        "flowContext": {
            "marketSlug": "btc-updown-5m-1772730000"
        },
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {}
    });

    assert_eq!(
        resolve_action_place_order_stale_market_retry(&node, &context, &step, &graph),
        None
    );
}

#[test]
fn stale_place_order_retry_ignores_matching_market() {
    let node = test_node(json!({}));
    let step = test_step(json!({
        "market_slug": "btc-updown-5m-1772729700",
    }));
    let graph = runtime_graph(
        vec![
            ("trigger_1", "trigger.market_price"),
            ("action_1", "action.place_order"),
        ],
        vec![("trigger_1", "action_1")],
    );
    let context = json!({
        "flowContext": {
            "marketSlug": "btc-updown-5m-1772729700"
        },
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {}
    });

    assert_eq!(
        resolve_action_place_order_stale_market_retry(&node, &context, &step, &graph),
        None
    );
}

#[test]
fn stale_place_order_retry_prefers_upstream_trigger_market_slug_over_global_flow_context() {
    let node = test_node(json!({}));
    let step = test_step(json!({
        "market_slug": "btc-updown-5m-1772730000",
    }));
    let graph = runtime_graph(
        vec![
            ("trigger_1", "trigger.market_price"),
            ("action_1", "action.place_order"),
        ],
        vec![("trigger_1", "action_1")],
    );
    let mut context = json!({
        "flowContext": {
            "marketSlug": "btc-updown-5m-1772729700"
        },
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {}
    });
    set_flow_node_state(
        &mut context,
        "trigger_1",
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_SLUG,
        json!("btc-updown-5m-1772730000"),
    );

    assert_eq!(
        resolve_action_place_order_stale_market_retry(&node, &context, &step, &graph),
        None
    );
}

#[test]
fn place_order_existing_order_id_prefers_node_local_ref_over_shared_ref() {
    let node = test_node(json!({ "refKey": "preset_place_order" }));
    let context = json!({
        "flowContext": {},
        "vars": {},
        "state": {},
        "refs": {
            "action_1": 202,
            "preset_place_order": 101
        },
        "nodeState": {}
    });

    assert_eq!(
        resolve_action_place_order_existing_order_id(&node, &context),
        Some(202)
    );
}

#[test]
fn place_order_existing_order_id_ignores_legacy_shared_preset_ref_when_node_local_missing() {
    let node = test_node(json!({
        "refKey": "preset_place_order",
        "presetKind": "place_order"
    }));
    let context = json!({
        "flowContext": {},
        "vars": {},
        "state": {},
        "refs": {
            "preset_place_order": 101
        },
        "nodeState": {}
    });

    assert_eq!(
        resolve_action_place_order_existing_order_id(&node, &context),
        None
    );
}

#[test]
fn place_order_existing_order_id_falls_back_to_quick_preset_shared_ref() {
    let node = test_node(json!({
        "refKey": "preset_buy_current_position",
        "presetKind": "buy_current_position"
    }));
    let context = json!({
        "flowContext": {},
        "vars": {},
        "state": {},
        "refs": {
            "preset_buy_current_position": 101
        },
        "nodeState": {}
    });

    assert_eq!(
        resolve_action_place_order_existing_order_id(&node, &context),
        Some(101)
    );
}

#[test]
fn place_order_existing_order_id_falls_back_to_custom_shared_ref() {
    let node = test_node(json!({
        "refKey": "team_shared_buy",
        "presetKind": "place_order"
    }));
    let context = json!({
        "flowContext": {},
        "vars": {},
        "state": {},
        "refs": {
            "team_shared_buy": 101
        },
        "nodeState": {}
    });

    assert_eq!(
        resolve_action_place_order_existing_order_id(&node, &context),
        Some(101)
    );
}

#[test]
fn place_order_resolves_max_price_from_node_config_cent_only() {
    let ctx = json!({});
    let step = test_step(json!({}));
    let node = test_node(json!({
        "maxPriceCent": 90,
        "maxPrice": 0.95
    }));
    assert_eq!(
        resolve_action_place_order_max_price(&node, &step, &ctx),
        Some(0.9)
    );

    let empty_node = test_node(json!({}));
    assert_eq!(
        resolve_action_place_order_max_price(&empty_node, &step, &ctx),
        None
    );
}

#[test]
fn place_order_resolves_max_price_from_node_config_raw_legacy() {
    let ctx = json!({});
    let step = test_step(json!({}));
    let node = test_node(json!({ "maxPrice": 0.88 }));
    assert_eq!(
        resolve_action_place_order_max_price(&node, &step, &ctx),
        Some(0.88)
    );
}

#[test]
fn place_order_resolves_max_price_from_step_input_fallback() {
    let ctx = json!({});
    let step = test_step(json!({ "max_price": 0.90 }));
    let node = test_node(json!({}));
    assert_eq!(
        resolve_action_place_order_max_price(&node, &step, &ctx),
        Some(0.9)
    );
}

#[test]
fn place_order_resolves_max_price_from_flow_context_fallback() {
    let ctx = json!({ "flowContext": { "maxPrice": 0.90 } });
    let step = test_step(json!({}));
    let node = test_node(json!({}));
    assert_eq!(
        resolve_action_place_order_max_price(&node, &step, &ctx),
        Some(0.9)
    );
}

#[test]
fn place_order_max_price_priority_order() {
    let ctx = json!({ "flowContext": { "maxPrice": 0.95 } });
    let step = test_step(json!({ "max_price": 0.90 }));

    let node_cent = test_node(json!({ "maxPriceCent": 85, "maxPrice": 0.88 }));
    assert_eq!(
        resolve_action_place_order_max_price(&node_cent, &step, &ctx),
        Some(0.85)
    );

    let node_raw = test_node(json!({ "maxPrice": 0.88 }));
    assert_eq!(
        resolve_action_place_order_max_price(&node_raw, &step, &ctx),
        Some(0.88)
    );

    let empty_node = test_node(json!({}));
    assert_eq!(
        resolve_action_place_order_max_price(&empty_node, &step, &ctx),
        Some(0.90)
    );

    let empty_step = test_step(json!({}));
    assert_eq!(
        resolve_action_place_order_max_price(&empty_node, &empty_step, &ctx),
        Some(0.95)
    );
}

#[test]
fn place_order_max_price_invalid_values_filtered() {
    let node = test_node(json!({}));
    let empty_step = test_step(json!({}));

    assert_eq!(
        resolve_action_place_order_max_price(
            &node,
            &empty_step,
            &json!({ "flowContext": { "maxPrice": 0.0 } })
        ),
        None
    );
    assert_eq!(
        resolve_action_place_order_max_price(
            &node,
            &empty_step,
            &json!({ "flowContext": { "maxPrice": 1.5 } })
        ),
        None
    );
    assert_eq!(
        resolve_action_place_order_max_price(
            &node,
            &empty_step,
            &json!({ "flowContext": { "maxPrice": null } })
        ),
        None
    );

    let step_zero = test_step(json!({ "max_price": 0.0 }));
    assert_eq!(
        resolve_action_place_order_max_price(&node, &step_zero, &json!({})),
        None
    );

    let step_nan = test_step(json!({ "max_price": "not_a_number" }));
    assert_eq!(
        resolve_action_place_order_max_price(&node, &step_nan, &json!({})),
        None
    );
}

#[test]
fn place_order_reentry_guard_resolution_ignores_reentry_band_on_first_entry() {
    let node = test_node(json!({
        "reentryMinPriceCent": 75,
        "reentryMaxPriceCent": 89
    }));

    let resolved = resolve_action_place_order_reentry_guard_resolution(
        &node,
        &json!({}),
        Some(0.80),
        Some(0.90),
    )
    .expect("reentry resolution");

    assert_eq!(resolved.generation, 0);
    assert!(!resolved.band_active);
    assert_eq!(resolved.effective_guard_trigger_price, Some(0.80));
    assert_eq!(resolved.effective_max_price, Some(0.90));
}

#[test]
fn place_order_reentry_guard_resolution_uses_reentry_band_for_reentries() {
    let node = test_node(json!({
        "reentryMinPriceCent": 75,
        "reentryMaxPriceCent": 89
    }));
    let context = json!({
        "nodeState": {
            "action_1": {
                "reentry_generation": 2
            }
        }
    });

    let resolved = resolve_action_place_order_reentry_guard_resolution(
        &node,
        &context,
        Some(0.80),
        Some(0.90),
    )
    .expect("reentry resolution");

    assert_eq!(resolved.generation, 2);
    assert!(resolved.band_active);
    assert_eq!(resolved.configured_min_price, Some(0.75));
    assert_eq!(resolved.configured_max_price, Some(0.89));
    assert_eq!(resolved.effective_guard_trigger_price, Some(0.75));
    assert_eq!(resolved.effective_max_price, Some(0.89));
}

#[test]
fn place_order_reentry_guard_resolution_rejects_inverted_band() {
    let node = test_node(json!({
        "reentryMinPriceCent": 90,
        "reentryMaxPriceCent": 85
    }));
    let context = json!({
        "nodeState": {
            "action_1": {
                "reentry_generation": 1
            }
        }
    });

    let error = resolve_action_place_order_reentry_guard_resolution(
        &node,
        &context,
        Some(0.80),
        Some(0.90),
    )
    .expect_err("inverted band should fail");

    assert!(error
        .to_string()
        .contains("reentryMinPriceCent must be lower than reentryMaxPriceCent"));
}

#[test]
fn place_order_inherits_trigger_max_price_via_step_input() {
    let trigger_output = json!({
        "market_slug": "btc-updown-5m-1",
        "token_id": "tok-up",
        "outcome_label": "Up",
        "triggered_price": 0.89,
        "trigger_price": 0.80,
        "max_price": 0.90,
        "maxPrice": 0.90,
        "triggered_condition": "cross_above"
    });
    let step = test_step(trigger_output);
    let node = test_node(json!({
        "side": "buy",
        "executionMode": "market",
        "sizeUsdc": 10.0
    }));
    let ctx = json!({ "flowContext": { "maxPrice": 0.90 } });

    let resolved = resolve_action_place_order_max_price(&node, &step, &ctx);
    assert_eq!(resolved, Some(0.90));

    let mut order = test_builder_order("buy", None);
    order.max_price = resolved;
    assert!(trade_builder_price_exceeds_max_price(&order, 0.91));
    assert!(!trade_builder_price_exceeds_max_price(&order, 0.90));
}

#[test]
fn place_order_resolves_guard_trigger_price_from_step_input() {
    let step = test_step(json!({
        "trigger_price": 0.77,
        "triggerPrice": 0.76,
        "triggered_price": 0.81
    }));
    assert_eq!(
        resolve_action_place_order_guard_trigger_price(&step),
        Some(0.77)
    );
}

#[test]
fn place_order_resolves_execution_floor_price_from_manual_override_before_step_input() {
    let node = test_node(json!({
        "executionFloorPriceCent": 82
    }));
    let step = test_step(json!({
        "trigger_price": 0.77,
        "triggerPrice": 0.76
    }));

    assert_eq!(
        resolve_action_place_order_execution_floor_price(&node, &step),
        Some(0.82)
    );
}

#[test]
fn place_order_resolves_execution_floor_price_from_step_input_when_override_missing() {
    let node = test_node(json!({}));
    let step = test_step(json!({
        "trigger_price": 0.77,
        "triggerPrice": 0.76
    }));

    assert_eq!(
        resolve_action_place_order_execution_floor_price(&node, &step),
        Some(0.77)
    );
}

#[test]
fn place_order_resolves_cycle_window_datetime_from_step_then_context() {
    let step = test_step(json!({
        "cycleWindowOpenAt": "2026-03-12T12:44:00Z"
    }));
    let context = json!({
        "flowContext": {
            "cycleWindowOpenAt": "2026-03-12T12:43:00Z",
            "cycleWindowEndAt": "2026-03-12T12:45:00Z"
        },
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {}
    });

    assert_eq!(
        resolve_action_place_order_datetime(
            &step,
            &context,
            &["cycleWindowOpenAt"],
            "cycleWindowOpenAt",
        )
        .map(|value| value.to_rfc3339()),
        Some("2026-03-12T12:44:00+00:00".to_string())
    );
    assert_eq!(
        resolve_action_place_order_datetime(
            &step,
            &context,
            &["cycleWindowEndAt"],
            "cycleWindowEndAt",
        )
        .map(|value| value.to_rfc3339()),
        Some("2026-03-12T12:45:00+00:00".to_string())
    );
}

#[test]
fn place_order_retry_snapshot_preserves_guard_and_market_inputs() {
    let node = test_node(json!({}));
    let step = test_step(json!({
        "market_slug": "sol-updown-5m-1773315300",
        "trigger_price": 0.77,
        "triggered_token_id": "tok-up",
        "triggered_outcome_label": "Up",
        "triggered_price": 0.78,
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
        Some("sol-updown-5m-1773315300")
    );
    assert_eq!(
        resolve_action_place_order_guard_trigger_price(&step),
        Some(0.77)
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
fn place_order_exit_price_allows_rule_only_tp_and_sl() {
    let node = test_node(json!({
        "tpRules": [{ "sizePct": 50, "priceCent": 77 }],
        "slRules": [{ "sizePct": 50, "priceCent": 50 }]
    }));

    assert_eq!(
        resolve_action_place_order_exit_price(&node, "buy", true, "tpPriceCent", "tpPrice", "tp")
            .unwrap(),
        None
    );
    assert_eq!(
        resolve_action_place_order_exit_price(&node, "buy", true, "slPriceCent", "slPrice", "sl")
            .unwrap(),
        None
    );
}

#[test]
fn oco_cancel_only_applies_to_child_sell_orders_with_live_status() {
    let sell_child = test_builder_order("sell", Some(9));
    let mut staged_sell_child = test_builder_order("sell", Some(9));
    staged_sell_child.exit_ladder_kind = Some("sl".to_string());
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
        &staged_sell_child,
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
fn resolve_max_price_ref_uses_best_ask_for_buy() {
    let order = test_builder_order("buy", None);

    assert_eq!(
        trade_builder_resolve_max_price_reference(&order, Some(0.62), 0.67),
        (0.62, "best_ask")
    );
}

#[test]
fn resolve_max_price_ref_falls_back_when_best_ask_none() {
    let order = test_builder_order("buy", None);

    assert_eq!(
        trade_builder_resolve_max_price_reference(&order, None, 0.67),
        (0.67, "desired_price_fallback")
    );
}

#[test]
fn resolve_max_price_ref_falls_back_when_best_ask_invalid() {
    let order = test_builder_order("buy", None);

    assert_eq!(
        trade_builder_resolve_max_price_reference(&order, Some(f64::NAN), 0.67),
        (0.67, "desired_price_fallback")
    );
    assert_eq!(
        trade_builder_resolve_max_price_reference(&order, Some(0.0), 0.67),
        (0.67, "desired_price_fallback")
    );
    assert_eq!(
        trade_builder_resolve_max_price_reference(&order, Some(1.01), 0.67),
        (0.67, "desired_price_fallback")
    );
}

#[test]
fn resolve_max_price_ref_uses_desired_price_for_sell() {
    let order = test_builder_order("sell", None);

    assert_eq!(
        trade_builder_resolve_max_price_reference(&order, Some(0.62), 0.67),
        (0.67, "desired_price")
    );
}

#[test]
fn best_ask_below_max_price_passes_guard() {
    let mut order = test_builder_order("buy", None);
    order.max_price = Some(0.63);
    let (reference_price, reference_source) =
        trade_builder_resolve_max_price_reference(&order, Some(0.62), 0.67);

    assert_eq!(reference_price, 0.62);
    assert_eq!(reference_source, "best_ask");
    assert!(!trade_builder_price_exceeds_max_price(
        &order,
        reference_price
    ));
}

#[test]
fn best_ask_above_max_price_blocks_guard() {
    let mut order = test_builder_order("buy", None);
    order.max_price = Some(0.63);
    let (reference_price, reference_source) =
        trade_builder_resolve_max_price_reference(&order, Some(0.64), 0.65);

    assert_eq!(reference_price, 0.64);
    assert_eq!(reference_source, "best_ask");
    assert!(trade_builder_price_exceeds_max_price(
        &order,
        reference_price
    ));
}

#[test]
fn current_price_below_guard_trigger_is_blocked_for_buys_only() {
    let mut buy_order = test_builder_order("buy", None);
    buy_order.guard_trigger_price = Some(0.77);
    assert!(trade_builder_price_below_guard_trigger(&buy_order, 0.76));
    assert!(!trade_builder_price_below_guard_trigger(&buy_order, 0.77));

    let mut sell_order = test_builder_order("sell", None);
    sell_order.guard_trigger_price = Some(0.77);
    assert!(!trade_builder_price_below_guard_trigger(&sell_order, 0.50));
}

#[test]
fn best_ask_below_execution_floor_blocks_buy_orders_only() {
    let mut buy_order = test_builder_order("buy", None);
    buy_order.best_ask_floor_price = Some(0.77);
    assert_eq!(
        trade_builder_execution_floor_block_reason(&buy_order, Some(0.76)),
        Some("below_best_ask_floor")
    );
    assert_eq!(
        trade_builder_execution_floor_block_reason(&buy_order, Some(0.77)),
        None
    );
    assert_eq!(
        trade_builder_execution_floor_block_reason(&buy_order, Some(0.90)),
        None
    );

    let mut sell_order = test_builder_order("sell", None);
    sell_order.best_ask_floor_price = Some(0.77);
    assert_eq!(
        trade_builder_execution_floor_block_reason(&sell_order, Some(0.50)),
        None
    );
}

#[test]
fn missing_best_ask_blocks_when_execution_floor_is_enabled() {
    let mut buy_order = test_builder_order("buy", None);
    buy_order.best_ask_floor_price = Some(0.77);
    assert!(trade_builder_execution_floor_missing_best_ask(
        &buy_order, None
    ));
    assert!(!trade_builder_execution_floor_missing_best_ask(
        &buy_order,
        Some(0.80)
    ));

    let sell_order = test_builder_order("sell", None);
    assert!(!trade_builder_execution_floor_missing_best_ask(
        &sell_order,
        None
    ));
}

#[test]
fn missing_best_ask_forces_execution_floor_waiting() {
    let mut buy_order = test_builder_order("buy", None);
    buy_order.best_ask_floor_price = Some(0.77);
    assert!(trade_builder_execution_floor_should_wait(
        &buy_order,
        "best_ask_unavailable"
    ));
    assert!(!trade_builder_execution_floor_should_wait(
        &buy_order,
        "below_best_ask_floor"
    ));

    buy_order.retry_on_execution_floor_guard_block = true;
    assert!(trade_builder_execution_floor_should_wait(
        &buy_order,
        "below_best_ask_floor"
    ));
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
