use super::support::*;
use super::*;

#[test]
fn parse_drawdown_rules_supports_up_direction_and_defaults_to_down() {
    let node = drawdown_node(json!({
        "lossRules": [
            { "lossPct": 10 },
            { "lossPct": 15, "direction": "up", "windowMs": 5000 }
        ]
    }));

    let rules = parse_position_drawdown_rules(&node);
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].direction, PositionDrawdownDirection::Down);
    assert_eq!(rules[1].direction, PositionDrawdownDirection::Up);
    assert_eq!(rules[1].window_ms, Some(5000));
}

#[test]
fn parse_drawdown_rules_ignores_invalid_direction_values() {
    let node = drawdown_node(json!({
        "lossRules": [
            { "lossPct": 10, "direction": "sideways" },
            { "lossPct": 7, "direction": "down" }
        ]
    }));

    let rules = parse_position_drawdown_rules(&node);
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].direction, PositionDrawdownDirection::Down);
    assert!((rules[0].loss_pct - 7.0).abs() < f64::EPSILON);
}

#[test]
fn drawdown_detects_deprecated_window_sec_fields() {
    let legacy_root = drawdown_node(json!({
        "lossPct": 10,
        "windowSec": 5
    }));
    assert!(has_deprecated_drawdown_window_sec(&legacy_root));

    let legacy_rule = drawdown_node(json!({
        "lossRules": [
            { "lossPct": 10, "windowSec": 5 }
        ]
    }));
    assert!(has_deprecated_drawdown_window_sec(&legacy_rule));

    let modern = drawdown_node(json!({
        "lossRules": [
            { "lossPct": 10, "windowMs": 5000 }
        ]
    }));
    assert!(!has_deprecated_drawdown_window_sec(&modern));
}

#[test]
fn ws_once_idempotency_key_is_stable_per_run_and_node() {
    let key_1 = ws_price_trigger_step_idempotency_key(
        42,
        "trigger_market",
        "cross_above",
        0.41,
        Some(1000),
        true,
        false,
        None,
    );
    let key_2 = ws_price_trigger_step_idempotency_key(
        42,
        "trigger_market",
        "cross_below",
        0.67,
        Some(2000),
        true,
        false,
        None,
    );

    assert_eq!(key_1, "ws-once:42:trigger_market");
    assert_eq!(key_2, "ws-once:42:trigger_market");
}

#[test]
fn ws_loop_idempotency_key_depends_on_event_and_price() {
    let key_a = ws_price_trigger_step_idempotency_key(
        42,
        "trigger_market",
        "cross_above",
        0.41,
        Some(1000),
        false,
        false,
        None,
    );
    let key_b = ws_price_trigger_step_idempotency_key(
        42,
        "trigger_market",
        "cross_above",
        0.41,
        Some(1000),
        false,
        false,
        None,
    );
    let key_c = ws_price_trigger_step_idempotency_key(
        42,
        "trigger_market",
        "cross_above",
        0.41,
        Some(1001),
        false,
        false,
        None,
    );

    assert_eq!(key_a, key_b);
    assert_ne!(key_a, key_c);
}

#[test]
fn ws_market_once_idempotency_key_is_scoped_by_market_slug() {
    let key_1 = ws_price_trigger_step_idempotency_key(
        42,
        "trigger_market",
        "cross_above",
        0.41,
        Some(1000),
        true,
        true,
        Some("btc-updown-5m-1"),
    );
    let key_2 = ws_price_trigger_step_idempotency_key(
        42,
        "trigger_market",
        "cross_above",
        0.42,
        Some(1001),
        true,
        true,
        Some("btc-updown-5m-2"),
    );

    assert_eq!(key_1, "ws-once:42:trigger_market:btc-updown-5m-1");
    assert_eq!(key_2, "ws-once:42:trigger_market:btc-updown-5m-2");
    assert_ne!(key_1, key_2);
}

#[test]
fn resolve_ws_previous_price_prefers_ws_payload_values() {
    let token = "tok-yes";
    let ws_prev_payload = json!({ token: 0.34 });
    let ws_prev_map = ws_prev_payload.as_object();

    let from_map = resolve_ws_previous_price(
        true,
        Some(0.30),
        token,
        Some(token),
        Some(0.31),
        true,
        ws_prev_map,
    );
    assert!(from_map.is_some());
    assert!((from_map.unwrap_or_default() - 0.34).abs() < 1e-9);

    let from_single =
        resolve_ws_previous_price(true, Some(0.30), token, Some(token), Some(0.33), true, None);
    assert!(from_single.is_some());
    assert!((from_single.unwrap_or_default() - 0.33).abs() < 1e-9);

    let from_state = resolve_ws_previous_price(
        true,
        Some(0.30),
        token,
        Some("tok-no"),
        Some(0.33),
        true,
        None,
    );
    assert!(from_state.is_some());
    assert!((from_state.unwrap_or_default() - 0.30).abs() < 1e-9);

    let ws_prev_null_payload = json!({ token: null });
    let ws_prev_null_map = ws_prev_null_payload.as_object();
    let explicit_null_from_map = resolve_ws_previous_price(
        true,
        Some(0.30),
        token,
        Some(token),
        Some(0.33),
        true,
        ws_prev_null_map,
    );
    assert!(explicit_null_from_map.is_none());

    let explicit_null_from_single =
        resolve_ws_previous_price(true, Some(0.30), token, Some(token), None, true, None);
    assert!(explicit_null_from_single.is_none());

    let non_ws = resolve_ws_previous_price(
        false,
        Some(0.22),
        token,
        Some(token),
        Some(0.99),
        true,
        ws_prev_map,
    );
    assert!(non_ws.is_some());
    assert!((non_ws.unwrap_or_default() - 0.22).abs() < 1e-9);
}

#[test]
fn ws_previous_price_preserves_cross_detection_after_context_update() {
    let token = "tok-yes";
    let trigger_price = 0.30;
    let current_price = 0.30;
    let state_prev = Some(0.30);

    let ws_prev_payload = json!({ token: 0.34 });
    let ws_prev_map = ws_prev_payload.as_object();
    let effective_prev = resolve_ws_previous_price(
        true,
        state_prev,
        token,
        Some(token),
        Some(0.34),
        true,
        ws_prev_map,
    );
    let (pass_with_ws_prev, mode_with_ws_prev) = evaluate_trigger_market_price_condition(
        effective_prev,
        current_price,
        trigger_price,
        "cross_below",
        true,
        None,
    );
    assert!(pass_with_ws_prev);
    assert_eq!(mode_with_ws_prev, "cross_detected");

    let (pass_with_state_prev, mode_with_state_prev) = evaluate_trigger_market_price_condition(
        state_prev,
        current_price,
        trigger_price,
        "cross_below",
        true,
        None,
    );
    assert!(!pass_with_state_prev);
    assert_eq!(mode_with_state_prev, "no_cross");
}

#[test]
fn auto_scope_specs_resolve_token_from_outcome_label() {
    let node = TradeFlowNode {
        key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        config: json!({
            "repeatMode": "once",
            "onceScope": "market",
            "marketMode": "auto_scope",
            "outcomeConditions": [{
                "outcomeLabel": "Up",
                "triggerCondition": "cross_above",
                "triggerPriceCent": 60
            }]
        }),
    };
    let context = json!({
        "flowContext": {
            "marketSlug": "btc-updown-5m-1772296200",
            "yesTokenId": "yes-token",
            "noTokenId": "no-token"
        }
    });

    let specs = open_position_ws_price_node_specs(&node, &context);
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].token_id, "yes-token");
    assert!(specs[0].once_mode);
    assert!(specs[0].once_scope_market);
    assert_eq!(specs[0].price_mode, WsPriceMode::Midpoint);
    assert_eq!(
        specs[0].market_slug.as_deref(),
        Some("btc-updown-5m-1772296200")
    );
}

#[test]
fn auto_scope_specs_prefer_context_market_slug_over_stale_config_slug() {
    let node = TradeFlowNode {
        key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        config: json!({
            "repeatMode": "once",
            "onceScope": "market",
            "marketMode": "auto_scope",
            "marketSlug": "btc-updown-15m-stale",
            "outcomeConditions": [{
                "outcomeLabel": "Up",
                "triggerCondition": "cross_above",
                "triggerPriceCent": 60
            }]
        }),
    };
    let context = json!({
        "flowContext": {
            "marketSlug": "btc-updown-15m-fresh",
            "yesTokenId": "yes-token",
            "noTokenId": "no-token"
        }
    });

    let specs = open_position_ws_price_node_specs(&node, &context);
    assert_eq!(specs.len(), 1);
    assert_eq!(
        specs[0].market_slug.as_deref(),
        Some("btc-updown-15m-fresh")
    );
}

#[test]
fn market_price_specs_parse_price_mode_and_default_to_midpoint() {
    let context = json!({
        "flowContext": {
            "marketSlug": "epl-test",
            "yesTokenId": "yes-token",
            "noTokenId": "no-token"
        }
    });
    let default_node = TradeFlowNode {
        key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        config: json!({
            "marketMode": "auto_scope",
            "outcomeConditions": [{
                "outcomeLabel": "Yes",
                "triggerCondition": "cross_above",
                "triggerPriceCent": 60
            }]
        }),
    };
    let raw_node = TradeFlowNode {
        key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        config: json!({
            "marketMode": "auto_scope",
            "priceMode": "raw",
            "outcomeConditions": [{
                "outcomeLabel": "Yes",
                "triggerCondition": "cross_above",
                "triggerPriceCent": 60
            }]
        }),
    };
    let site_display_node = TradeFlowNode {
        key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        config: json!({
            "marketMode": "auto_scope",
            "priceMode": "site_display",
            "outcomeConditions": [{
                "outcomeLabel": "Yes",
                "triggerCondition": "cross_above",
                "triggerPriceCent": 60
            }]
        }),
    };
    let last_trade_node = TradeFlowNode {
        key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        config: json!({
            "marketMode": "auto_scope",
            "priceMode": "last_trade",
            "outcomeConditions": [{
                "outcomeLabel": "Yes",
                "triggerCondition": "cross_above",
                "triggerPriceCent": 60
            }]
        }),
    };

    let default_specs = open_position_ws_price_node_specs(&default_node, &context);
    let raw_specs = open_position_ws_price_node_specs(&raw_node, &context);
    let site_display_specs = open_position_ws_price_node_specs(&site_display_node, &context);
    let last_trade_specs = open_position_ws_price_node_specs(&last_trade_node, &context);
    assert_eq!(default_specs.len(), 1);
    assert_eq!(raw_specs.len(), 1);
    assert_eq!(site_display_specs.len(), 1);
    assert_eq!(last_trade_specs.len(), 1);
    assert_eq!(default_specs[0].price_mode, WsPriceMode::Midpoint);
    assert_eq!(raw_specs[0].price_mode, WsPriceMode::Raw);
    assert_eq!(site_display_specs[0].price_mode, WsPriceMode::SiteDisplay);
    assert_eq!(last_trade_specs[0].price_mode, WsPriceMode::LastTrade);
}

#[test]
fn open_positions_specs_keep_raw_price_mode() {
    let node = TradeFlowNode {
        key: "trigger_open".to_string(),
        node_type: "trigger.open_positions".to_string(),
        config: json!({
            "marketSlug": "epl-test",
            "outcomeConditions": [{
                "tokenId": "tok-yes",
                "triggerCondition": "cross_above",
                "triggerPriceCent": 55
            }]
        }),
    };

    let specs = open_position_ws_price_node_specs(&node, &json!({}));
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].price_mode, WsPriceMode::Raw);
}

#[test]
fn ws_market_slug_override_is_ignored_for_auto_scope_when_resolved_slug_exists() {
    let auto_scope_node = TradeFlowNode {
        key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        config: json!({
            "marketMode": "auto_scope"
        }),
    };
    let fixed_node = TradeFlowNode {
        key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        config: json!({
            "marketMode": "fixed"
        }),
    };

    assert!(!should_accept_ws_market_slug_override(
        &auto_scope_node,
        "btc-updown-15m-1772300000"
    ));
    assert!(should_accept_ws_market_slug_override(&auto_scope_node, ""));
    assert!(should_accept_ws_market_slug_override(
        &fixed_node,
        "btc-updown-15m-1772300000"
    ));
}

#[test]
fn auto_scope_market_cache_refresh_forces_current_window_reselection() {
    let node = TradeFlowNode {
        key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        config: json!({
            "marketMode": "auto_scope",
            "marketSelection": "latest_by_slug"
        }),
    };
    let now = DateTime::parse_from_rfc3339("2026-03-09T17:16:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let scope_def = find_updown_scope_by_scope("sol_5m_updown").unwrap();

    assert!(should_force_auto_scope_market_cache_refresh(
        &node,
        scope_def,
        Some("sol-updown-5m-1773076200"),
        now
    ));
    assert!(!should_force_auto_scope_market_cache_refresh(
        &node,
        scope_def,
        Some("sol-updown-5m-1773076500"),
        now
    ));
}

#[test]
fn fixed_market_boundary_open_allows_first_tick_override() {
    assert!(should_allow_ws_first_tick_threshold_override(
        true,
        "trigger.market_price",
        false,
        "first_tick_in_range",
        true,
        None,
    ));
    assert!(!should_allow_ws_first_tick_threshold_override(
        true,
        "trigger.market_price",
        false,
        "first_tick_in_range",
        false,
        None,
    ));
}

#[test]
fn cycle_window_boundary_due_target_only_fires_once_per_market_window() {
    let node_spec = WsOpenPositionPriceNodeSpec {
        node_key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        once_mode: true,
        once_scope_market: false,
        auto_scope: false,
        price_mode: WsPriceMode::SiteDisplay,
        market_slug: Some("sol-updown-5m-1773079500".to_string()),
        token_id: "tok-down".to_string(),
        outcome_label: "Down".to_string(),
        trigger_condition: "cross_above".to_string(),
        trigger_price: 0.77,
        max_price: Some(0.90),
        protection_mode: "off".to_string(),
        protection_asset: None,
        confirmation_ms: None,
        cycle_window_mode: Some("last".to_string()),
        cycle_window_secs: Some(120),
    };
    let now = DateTime::parse_from_rfc3339("2026-03-09T18:08:01Z")
        .unwrap()
        .with_timezone(&Utc);
    let mut run_spec = WsOpenPositionPriceRunSpec {
        run_id: 77,
        definition_id: 88,
        version_id: 99,
        context: json!({
            "flowContext": {},
            "vars": {},
            "state": {},
            "refs": {},
            "nodeState": {}
        }),
        nodes: vec![node_spec.clone()],
        context_dirty: false,
    };

    let due = cycle_window_boundary_due_target(&run_spec, 0, &node_spec, 0, now)
        .expect("boundary should be due");
    assert_eq!(
        due.boundary_marker,
        "last:sol-updown-5m-1773079500:120".to_string()
    );

    set_flow_node_state(
        &mut run_spec.context,
        &node_spec.node_key,
        &cycle_window_boundary_state_key(&node_spec),
        json!(due.boundary_marker),
    );
    assert!(cycle_window_boundary_due_target(&run_spec, 0, &node_spec, 0, now).is_none());
}

#[test]
fn cycle_window_last_eval_state_payload_captures_boundary_diagnostics() {
    let node_spec = WsOpenPositionPriceNodeSpec {
        node_key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        once_mode: true,
        once_scope_market: false,
        auto_scope: false,
        price_mode: WsPriceMode::SiteDisplay,
        market_slug: Some("sol-updown-5m-1773079500".to_string()),
        token_id: "tok-down".to_string(),
        outcome_label: "Down".to_string(),
        trigger_condition: "cross_above".to_string(),
        trigger_price: 0.77,
        max_price: Some(0.90),
        protection_mode: "off".to_string(),
        protection_asset: None,
        confirmation_ms: None,
        cycle_window_mode: Some("last".to_string()),
        cycle_window_secs: Some(120),
    };
    let now = DateTime::parse_from_rfc3339("2026-03-09T18:08:01Z")
        .unwrap()
        .with_timezone(&Utc);
    let diagnostics =
        cycle_window_eval_diagnostics(&node_spec, "last", now).expect("diagnostics should exist");
    let (window_open_at, window_end_at) =
        cycle_window_bounds(&node_spec).expect("bounds should exist");
    let mut context = json!({
        "flowContext": {},
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {}
    });

    set_cycle_window_last_eval_state(
        &mut context,
        &node_spec,
        &diagnostics,
        "condition_not_met",
        Some(0.7245),
        Some("no_cross"),
    );

    let stored = flow_node_state(
        &context,
        &node_spec.node_key,
        &cycle_window_last_eval_state_key(&node_spec.token_id),
    )
    .expect("state should be stored");
    assert_eq!(stored["window_mode"], json!("last"));
    assert_eq!(stored["cycle_window_secs"], json!(120));
    assert_eq!(stored["window_open_at"], json!(window_open_at.to_rfc3339()));
    assert_eq!(stored["window_end_at"], json!(window_end_at.to_rfc3339()));
    assert_eq!(stored["evaluated_at"], json!(now.to_rfc3339()));
    assert_eq!(stored["boundary_lag_ms"], json!(1_000));
    assert_eq!(stored["price"], json!(0.7245));
    assert_eq!(stored["trigger_price"], json!(0.77));
    assert_eq!(stored["max_price"], json!(0.90));
    assert_eq!(stored["result"], json!("condition_not_met"));
    assert_eq!(stored["evaluation_mode"], json!("no_cross"));
    assert_eq!(stored["market_slug"], json!("sol-updown-5m-1773079500"));
}

#[test]
fn cycle_window_followup_diagnostics_report_ms_since_window_open() {
    let mut context = json!({
        "flowContext": {},
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {
            "trigger_market": {
                "cycle_window_last_eval_tok-down": {
                    "window_mode": "last",
                    "cycle_window_secs": 120,
                    "window_open_at": "2026-03-09T18:08:00+00:00",
                    "window_end_at": "2026-03-09T18:10:00+00:00",
                    "evaluated_at": "2026-03-09T18:08:01+00:00",
                    "boundary_lag_ms": 1000,
                    "price": 0.7245,
                    "trigger_price": 0.77,
                    "max_price": 0.90,
                    "result": "condition_not_met",
                    "evaluation_mode": "no_cross",
                    "market_slug": "sol-updown-5m-1773079500"
                }
            }
        }
    });
    let observed_at = DateTime::parse_from_rfc3339("2026-03-09T18:08:44Z")
        .unwrap()
        .with_timezone(&Utc);

    let followup = cycle_window_followup_diagnostics_from_context(
        &context,
        "trigger_market",
        "tok-down",
        observed_at,
    )
    .expect("followup diagnostics should exist");

    assert_eq!(followup["cycle_window_mode"], json!("last"));
    assert_eq!(followup["cycle_window_secs"], json!(120));
    assert_eq!(
        followup["window_open_at"],
        json!("2026-03-09T18:08:00+00:00")
    );
    assert_eq!(
        followup["window_end_at"],
        json!("2026-03-09T18:10:00+00:00")
    );
    assert_eq!(followup["ms_since_window_open"], json!(44_000));
    assert_eq!(followup["boundary_result"], json!("condition_not_met"));

    remove_flow_node_state(
        &mut context,
        "trigger_market",
        &cycle_window_last_eval_state_key("tok-down"),
    );
    assert!(cycle_window_followup_diagnostics_from_context(
        &context,
        "trigger_market",
        "tok-down",
        observed_at,
    )
    .is_none());
}

#[test]
fn auto_scope_market_rollover_updates_last_ws_slug_and_clears_transient_state() {
    let previous_context = json!({
        "flowContext": {
            "marketSlug": "btc-updown-5m-1773083700"
        },
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {
            "trigger_market": {
                "last_price": 0.89,
                "previous_price": 0.88,
                "last_ws_market_slug": "btc-updown-5m-1773083700",
                "previous_price_tok-old": 0.88,
                "cross_pending_at_tok-old": "2026-03-09T19:18:01Z",
                "cross_pending_price_tok-old": 0.89,
                "cross_pending_prev_tok-old": 0.87,
                "cycle_window_boundary_marker_tok-old": "last:btc-updown-5m-1773083700:120",
                "cycle_window_last_eval_tok-old": {
                    "result": "condition_not_met"
                }
            }
        }
    });
    let mut context = previous_context.clone();
    let node_specs = vec![WsOpenPositionPriceNodeSpec {
        node_key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        once_mode: true,
        once_scope_market: true,
        auto_scope: true,
        price_mode: WsPriceMode::SiteDisplay,
        market_slug: Some("btc-updown-5m-1773084000".to_string()),
        token_id: "tok-new".to_string(),
        outcome_label: "Up".to_string(),
        trigger_condition: "cross_above".to_string(),
        trigger_price: 0.77,
        max_price: Some(0.90),
        protection_mode: "off".to_string(),
        protection_asset: None,
        confirmation_ms: None,
        cycle_window_mode: Some("last".to_string()),
        cycle_window_secs: Some(120),
    }];

    let rotations = sync_trade_flow_auto_scope_market_rollover_state(
        &previous_context,
        &mut context,
        &node_specs,
    );

    assert_eq!(
        rotations,
        vec![AutoScopeMarketRotation {
            node_key: "trigger_market".to_string(),
            old_market_slug: "btc-updown-5m-1773083700".to_string(),
            new_market_slug: "btc-updown-5m-1773084000".to_string(),
        }]
    );
    assert_eq!(
        flow_node_state_string(&context, "trigger_market", "last_ws_market_slug").as_deref(),
        Some("btc-updown-5m-1773084000")
    );
    for cleared_key in [
        "last_price",
        "previous_price",
        "previous_price_tok-old",
        "cross_pending_at_tok-old",
        "cross_pending_price_tok-old",
        "cross_pending_prev_tok-old",
        "cycle_window_boundary_marker_tok-old",
        "cycle_window_last_eval_tok-old",
    ] {
        assert!(
            flow_node_state(&context, "trigger_market", cleared_key).is_none(),
            "{cleared_key} should be cleared on market rollover"
        );
    }
}

#[test]
fn auto_scope_market_rollover_ignores_non_market_price_nodes() {
    let previous_context = json!({
        "flowContext": {
            "marketSlug": "btc-updown-5m-1773083700"
        },
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {
            "trigger_open": {
                "last_ws_market_slug": "btc-updown-5m-1773083700",
                "previous_price_tok-old": 0.88
            }
        }
    });
    let mut context = previous_context.clone();
    let node_specs = vec![WsOpenPositionPriceNodeSpec {
        node_key: "trigger_open".to_string(),
        node_type: "trigger.open_positions".to_string(),
        once_mode: false,
        once_scope_market: false,
        auto_scope: true,
        price_mode: WsPriceMode::Raw,
        market_slug: Some("btc-updown-5m-1773084000".to_string()),
        token_id: "tok-new".to_string(),
        outcome_label: "Up".to_string(),
        trigger_condition: "cross_above".to_string(),
        trigger_price: 0.77,
        max_price: None,
        protection_mode: "off".to_string(),
        protection_asset: None,
        confirmation_ms: None,
        cycle_window_mode: None,
        cycle_window_secs: None,
    }];

    let rotations = sync_trade_flow_auto_scope_market_rollover_state(
        &previous_context,
        &mut context,
        &node_specs,
    );

    assert!(rotations.is_empty());
    assert_eq!(
        flow_node_state_string(&context, "trigger_open", "last_ws_market_slug").as_deref(),
        Some("btc-updown-5m-1773083700")
    );
    assert_eq!(
        flow_node_state(&context, "trigger_open", "previous_price_tok-old"),
        Some(&json!(0.88))
    );
}

#[test]
fn cross_below_requires_actual_crossing() {
    // None previous → false (ilk tick, sadece fiyat kaydedilir)
    assert!(!crossed_below_strict(None, 0.25, 0.30));
    assert!(!crossed_below_strict(None, 0.35, 0.30));
    // Gercek crossing: yukaridan asagiya
    assert!(crossed_below_strict(Some(0.31), 0.30, 0.30));
    assert!(crossed_below_strict(Some(0.35), 0.29, 0.30));
    // Zaten asagida, crossing yok
    assert!(!crossed_below_strict(Some(0.28), 0.27, 0.30));
}

#[test]
fn cross_above_requires_actual_crossing() {
    assert!(!crossed_above_strict(None, 0.35, 0.30));
    assert!(!crossed_above_strict(None, 0.25, 0.30));
    assert!(crossed_above_strict(Some(0.29), 0.30, 0.30));
    assert!(crossed_above_strict(Some(0.25), 0.31, 0.30));
    assert!(!crossed_above_strict(Some(0.32), 0.33, 0.30));
}

#[test]
fn trigger_market_price_allows_first_tick_threshold_hit() {
    let (pass_above, mode_above) =
        evaluate_trigger_market_price_condition(None, 0.35, 0.30, "cross_above", true, None);
    assert!(pass_above);
    assert_eq!(mode_above, "first_tick_threshold");

    let (pass_below, mode_below) =
        evaluate_trigger_market_price_condition(None, 0.25, 0.30, "cross_below", true, None);
    assert!(pass_below);
    assert_eq!(mode_below, "first_tick_threshold");

    let (strict_pass, strict_mode) =
        evaluate_trigger_market_price_condition(None, 0.35, 0.30, "cross_above", false, None);
    assert!(!strict_pass);
    assert_eq!(strict_mode, "no_previous");
}

#[test]
fn extract_price_ignores_price_changes_without_asset_id() {
    let events = vec![WsEvent {
        channel: WsChannel::Market,
        payload: json!({
            "price_changes": [
                { "price": "0.71", "timestamp": 12345 }
            ]
        }),
        event_type: WsEventType::PriceChange,
        market: None,
        order_id: None,
        fill_id: None,
        status: None,
        price: None,
        size: None,
        ts: Some(12345),
    }];

    assert!(extract_price_from_market_events(&events, "tok-yes").is_none());

    let events_with_asset = vec![WsEvent {
        channel: WsChannel::Market,
        payload: json!({
            "price_changes": [
                { "asset_id": "tok-yes", "price": "0.71", "timestamp": 12345 }
            ]
        }),
        event_type: WsEventType::PriceChange,
        market: None,
        order_id: None,
        fill_id: None,
        status: None,
        price: None,
        size: None,
        ts: Some(12345),
    }];

    let extracted = extract_price_from_market_events(&events_with_asset, "tok-yes");
    assert_eq!(extracted, Some((0.71, Some(12345))));
}
