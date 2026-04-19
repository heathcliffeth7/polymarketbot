use super::support::*;
use super::*;

fn test_ws_fast_path_node(
    node_key: &str,
    node_type: &str,
    market_slug: Option<&str>,
    token_id: &str,
) -> WsOpenPositionPriceNodeSpec {
    WsOpenPositionPriceNodeSpec {
        node_key: node_key.to_string(),
        node_type: node_type.to_string(),
        once_mode: true,
        once_scope_market: true,
        pair_lock_only_monitor: false,
        auto_scope: true,
        price_mode: WsPriceMode::Composite,
        market_slug: market_slug.map(str::to_string),
        token_id: token_id.to_string(),
        outcome_label: "Up".to_string(),
        trigger_condition: "level_above".to_string(),
        trigger_price: 0.49,
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
    }
}

fn test_market_price_flow_node(config: Value) -> TradeFlowNode {
    TradeFlowNode {
        key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        config,
    }
}

#[test]
fn select_ws_fast_path_targets_expands_market_price_siblings_for_dirty_token() {
    let cache = TradeFlowWsFastPathCache {
        run_specs: vec![WsOpenPositionPriceRunSpec {
            run_id: 1,
            definition_id: 2,
            version_id: 3,
            version_no: 4,
            context: json!({}),
            nodes: vec![
                test_ws_fast_path_node(
                    "trigger_up",
                    "trigger.market_price",
                    Some("eth-updown-5m-1"),
                    "tok-up",
                ),
                test_ws_fast_path_node(
                    "trigger_down",
                    "trigger.market_price",
                    Some("eth-updown-5m-1"),
                    "tok-down",
                ),
            ],
            context_dirty: false,
        }],
        token_targets: HashMap::from([
            ("tok-up".to_string(), vec![(0, 0)]),
            ("tok-down".to_string(), vec![(0, 1)]),
        ]),
        market_targets: HashMap::from([("eth-updown-5m-1".to_string(), vec![(0, 0), (0, 1)])]),
    };

    let selected = select_ws_fast_path_targets(1, &cache, Some(&["tok-down".to_string()]));
    assert_eq!(selected.len(), 2);
    assert!(selected.contains(&SelectedWsFastPathTarget {
        run_index: 0,
        node_index: 1,
        dirty_token_id: Some("tok-down".to_string()),
        reevaluation_reason: "dirty_token_match",
    }));
    assert!(selected.contains(&SelectedWsFastPathTarget {
        run_index: 0,
        node_index: 0,
        dirty_token_id: Some("tok-down".to_string()),
        reevaluation_reason: "market_dirty_fanout",
    }));
}

#[test]
fn select_ws_fast_path_targets_full_refresh_returns_all_nodes() {
    let cache = TradeFlowWsFastPathCache {
        run_specs: vec![WsOpenPositionPriceRunSpec {
            run_id: 1,
            definition_id: 2,
            version_id: 3,
            version_no: 4,
            context: json!({}),
            nodes: vec![
                test_ws_fast_path_node(
                    "trigger_up",
                    "trigger.market_price",
                    Some("eth-updown-5m-1"),
                    "tok-up",
                ),
                test_ws_fast_path_node(
                    "trigger_open",
                    "trigger.open_positions",
                    Some("eth-updown-5m-1"),
                    "tok-open",
                ),
            ],
            context_dirty: false,
        }],
        token_targets: HashMap::from([
            ("tok-up".to_string(), vec![(0, 0)]),
            ("tok-open".to_string(), vec![(0, 1)]),
        ]),
        market_targets: HashMap::from([("eth-updown-5m-1".to_string(), vec![(0, 0)])]),
    };

    let selected = select_ws_fast_path_targets(1, &cache, None);
    assert_eq!(selected.len(), 2);
    assert!(selected
        .iter()
        .all(|target| target.reevaluation_reason == "full_refresh"));
}

#[test]
fn build_trigger_ws_condition_not_met_log_fields_captures_level_not_met() {
    let node_spec = test_ws_fast_path_node(
        "trigger_up",
        "trigger.market_price",
        Some("eth-updown-5m-1"),
        "tok-up",
    );
    let (crossed, evaluation_mode) =
        evaluate_trigger_market_price_condition(Some(0.40), 0.43, 0.49, "level_above", false, None);

    assert!(!crossed);
    assert_eq!(evaluation_mode, "level_not_met");

    let fields = build_trigger_ws_condition_not_met_log_fields(
        &node_spec,
        0.43,
        Some(0.40),
        evaluation_mode,
        TriggerMarketPriceGateMode::StandardOnly,
        "ws_cache_composite_max_bid_last_trade",
        "ws_cache_composite_max_bid_last_trade",
        Some(0.42),
        Some(0.43),
        Some(0.43),
        Some(12),
        Some("tok-down"),
        "market_dirty_fanout",
        Some("eth-updown-5m-1"),
    );

    assert_eq!(fields.node_key, "trigger_up");
    assert_eq!(fields.node_type, "trigger.market_price");
    assert_eq!(fields.outcome_label, "Up");
    assert_eq!(fields.token_id, "tok-up");
    assert_eq!(fields.current_price, 0.43);
    assert_eq!(fields.previous_price, Some(0.40));
    assert_eq!(fields.trigger_price, 0.49);
    assert_eq!(fields.evaluation_mode, "level_not_met");
    assert_eq!(fields.gate_mode, "standard_only");
    assert_eq!(fields.price_mode, "composite");
    assert_eq!(fields.dirty_token_id.as_deref(), Some("tok-down"));
    assert_eq!(fields.ws_reevaluation_reason, "market_dirty_fanout");
    assert_eq!(
        fields.resolved_market_slug.as_deref(),
        Some("eth-updown-5m-1")
    );
    assert!(fields.once_mode);
}

#[test]
fn build_trigger_ws_target_log_fields_captures_selection_metadata() {
    let node_spec = test_ws_fast_path_node(
        "trigger_up",
        "trigger.market_price",
        Some("eth-updown-5m-1"),
        "tok-up",
    );

    let fields = build_trigger_ws_target_log_fields(
        &node_spec,
        Some("tok-down"),
        "market_dirty_fanout",
        Some("eth-updown-5m-1"),
    );

    assert_eq!(fields.node_key, "trigger_up");
    assert_eq!(fields.node_type, "trigger.market_price");
    assert_eq!(fields.outcome_label, "Up");
    assert_eq!(fields.token_id, "tok-up");
    assert_eq!(fields.dirty_token_id.as_deref(), Some("tok-down"));
    assert_eq!(fields.ws_reevaluation_reason, "market_dirty_fanout");
    assert_eq!(
        fields.resolved_market_slug.as_deref(),
        Some("eth-updown-5m-1")
    );
    assert_eq!(fields.price_mode, "composite");
    assert_eq!(fields.trigger_condition, "level_above");
    assert_eq!(fields.trigger_price, 0.49);
    assert_eq!(fields.max_price, None);
    assert!(fields.once_mode);
}

#[test]
fn build_open_position_ws_price_node_specs_reports_missing_condition_token() {
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
            "marketSlug": "btc-updown-5m-1772296200"
        }
    });

    let result = build_open_position_ws_price_node_specs(&node, &context);
    assert!(result.specs.is_empty());
    assert_eq!(
        result.skip_reasons,
        vec![WsNodeSpecBuildSkipReason {
            reason: WS_NODE_SPEC_SKIP_REASON_MISSING_TOKEN_ID,
            outcome_label: Some("Up".to_string()),
        }]
    );
}

#[test]
fn cycle_window_end_auto_sell_builder_order_id_requires_matching_market_slug() {
    let matching = json!({
        "builder_order_id": 42,
        "market_slug": "eth-updown-5m-1"
    });
    let camel_case = json!({
        "builderOrderId": 77,
        "marketSlug": "eth-updown-5m-1"
    });

    assert_eq!(
        cycle_window_end_auto_sell_builder_order_id(&matching, Some("eth-updown-5m-1")),
        Some(42)
    );
    assert_eq!(
        cycle_window_end_auto_sell_builder_order_id(&camel_case, Some("eth-updown-5m-1")),
        Some(77)
    );
    assert_eq!(
        cycle_window_end_auto_sell_builder_order_id(&matching, Some("eth-updown-5m-2")),
        None
    );
}

#[test]
fn build_cycle_window_end_auto_sell_input_uses_parent_trade_id_and_builder_id() {
    let node_spec = test_ws_fast_path_node(
        "trigger_up",
        "trigger.market_price",
        Some("eth-updown-5m-1"),
        "tok-up",
    );
    let mut parent_order = test_builder_order("buy", None);
    parent_order.id = 55;
    parent_order.trade_id = 77;
    parent_order.market_slug = "eth-updown-5m-1".to_string();
    parent_order.token_id = "tok-up".to_string();
    parent_order.outcome_label = "Up".to_string();

    let input = build_cycle_window_end_auto_sell_input(&node_spec, &parent_order);

    assert_eq!(input.get("sourceTradeId").and_then(Value::as_i64), Some(77));
    assert_eq!(
        input.get("parentBuilderOrderId").and_then(Value::as_i64),
        Some(55)
    );
    assert_eq!(
        input.get("windowEndAutoSell").and_then(Value::as_bool),
        Some(true)
    );
}

#[test]
fn build_open_position_ws_price_node_specs_reports_missing_token_for_auto_scope_outcome() {
    let node = test_market_price_flow_node(json!({
        "repeatMode": "once",
        "onceScope": "market",
        "marketMode": "auto_scope",
        "outcomeConditions": [{
            "outcomeLabel": "Up",
            "triggerCondition": "cross_above",
            "triggerPriceCent": 60
        }]
    }));
    let context = json!({
        "flowContext": {
            "marketSlug": "eth-updown-5m-1774059600"
        }
    });

    let result = build_open_position_ws_price_node_specs(&node, &context);
    assert!(result.specs.is_empty());
    assert_eq!(
        result.skip_reasons,
        vec![WsNodeSpecBuildSkipReason {
            reason: WS_NODE_SPEC_SKIP_REASON_MISSING_TOKEN_ID,
            outcome_label: Some("Up".to_string()),
        }]
    );
    assert!(open_position_ws_price_node_specs(&node, &context).is_empty());
}

#[test]
fn build_open_position_ws_price_node_specs_creates_pair_lock_only_auto_scope_monitors() {
    let node = test_market_price_flow_node(json!({
        "repeatMode": "once",
        "onceScope": "market",
        "marketMode": "auto_scope",
        "bindingMode": "pair_lock_only",
        "priceMode": "composite"
    }));
    let context = json!({
        "flowContext": {
            "marketSlug": "btc-updown-5m-1776512400",
            "yesTokenId": "yes-token",
            "noTokenId": "no-token"
        }
    });

    let result = build_open_position_ws_price_node_specs(&node, &context);
    assert!(result.skip_reasons.is_empty());
    assert_eq!(result.specs.len(), 2);
    assert!(result
        .specs
        .iter()
        .all(|spec| spec.pair_lock_only_monitor));
    assert_eq!(result.specs[0].outcome_label, "Up");
    assert_eq!(result.specs[0].token_id, "yes-token");
    assert_eq!(result.specs[1].outcome_label, "Down");
    assert_eq!(result.specs[1].token_id, "no-token");
    assert!(result.specs.iter().all(|spec| spec.trigger_condition.is_empty()));
}

#[test]
fn build_open_position_ws_price_node_specs_preserves_cycle_window_for_pair_lock_only_monitors() {
    let node = test_market_price_flow_node(json!({
        "repeatMode": "once",
        "onceScope": "market",
        "marketMode": "auto_scope",
        "bindingMode": "pair_lock_only",
        "cycleWindowMode": "custom_range",
        "cycleWindowStartSec": 230,
        "cycleWindowEndSec": 290
    }));
    let context = json!({
        "flowContext": {
            "marketSlug": "btc-updown-5m-1776512400",
            "yesTokenId": "yes-token",
            "noTokenId": "no-token"
        }
    });

    let specs = open_position_ws_price_node_specs(&node, &context);
    assert_eq!(specs.len(), 2);
    assert!(specs.iter().all(|spec| spec.cycle_window_mode.as_deref() == Some("custom_range")));
    assert!(specs.iter().all(|spec| spec.cycle_window_start_sec == Some(230)));
    assert!(specs.iter().all(|spec| spec.cycle_window_end_sec == Some(290)));
}

#[test]
fn build_open_position_ws_price_node_specs_reports_invalid_trigger_fields() {
    let node = test_market_price_flow_node(json!({
        "repeatMode": "once",
        "onceScope": "market",
        "marketMode": "auto_scope",
        "outcomeConditions": [{
            "outcomeLabel": "Up",
            "triggerCondition": "cross_above"
        }]
    }));
    let context = json!({
        "flowContext": {
            "marketSlug": "eth-updown-5m-1774059600",
            "yesTokenId": "yes-token",
            "noTokenId": "no-token"
        }
    });

    let result = build_open_position_ws_price_node_specs(&node, &context);
    assert!(result.specs.is_empty());
    assert_eq!(
        result.skip_reasons,
        vec![WsNodeSpecBuildSkipReason {
            reason: WS_NODE_SPEC_SKIP_REASON_INVALID_TRIGGER_FIELDS,
            outcome_label: Some("Up".to_string()),
        }]
    );
}

#[test]
fn build_open_position_ws_price_node_specs_reports_legacy_missing_token() {
    let node = test_market_price_flow_node(json!({
        "marketMode": "auto_scope",
        "outcomeLabel": "Up",
        "triggerCondition": "cross_above",
        "triggerPriceCent": 60
    }));
    let context = json!({
        "flowContext": {
            "marketSlug": "eth-updown-5m-1774059600"
        }
    });

    let result = build_open_position_ws_price_node_specs(&node, &context);
    assert!(result.specs.is_empty());
    assert_eq!(
        result.skip_reasons,
        vec![WsNodeSpecBuildSkipReason {
            reason: WS_NODE_SPEC_SKIP_REASON_MISSING_TOKEN_ID,
            outcome_label: Some("Up".to_string()),
        }]
    );
}

#[test]
fn chainlink_seed_rejected_too_old_payload_includes_structured_fields() {
    let expected_market_start =
        DateTime::<Utc>::from_timestamp(1_774_012_900, 0).expect("expected market start");
    let payload = build_chainlink_seed_rejected_too_old_payload(
        "btc-updown-5m-1774013100",
        "btc",
        "5m",
        &expected_market_start,
        &crate::trade_flow::guards::chainlink_price::ChainlinkNearTimestampRejectionDetails {
            gap_ms: 217_000,
            provider_age_ms: 216_937,
            candidate_timestamp_ms: 1_774_012_890_000,
            candidate_received_at_ms: 1_774_013_106_847,
        },
    );
    assert_eq!(
        payload.get("market_slug").and_then(Value::as_str),
        Some("btc-updown-5m-1774013100")
    );
    assert_eq!(payload.get("asset").and_then(Value::as_str), Some("btc"));
    assert_eq!(payload.get("timeframe").and_then(Value::as_str), Some("5m"));
    assert_eq!(payload.get("gap_ms").and_then(Value::as_i64), Some(217_000));
    assert_eq!(
        payload.get("provider_age_ms").and_then(Value::as_i64),
        Some(216_937)
    );
    assert_eq!(
        payload
            .get("candidate_timestamp_ms")
            .and_then(Value::as_i64),
        Some(1_774_012_890_000)
    );
    assert_eq!(
        payload
            .get("candidate_received_at_ms")
            .and_then(Value::as_i64),
        Some(1_774_013_106_847)
    );
}
