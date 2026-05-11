use super::support::*;
use super::*;

fn trigger_market_price_node(config: Value) -> TradeFlowNode {
    TradeFlowNode {
        key: "trigger_market".to_string(),
        node_type: "trigger.market_price".to_string(),
        config,
    }
}

#[test]
fn trigger_entry_timing_profile_selects_matching_5m_window() {
    let node = trigger_market_price_node(json!({
        "marketMode": "auto_scope",
        "repeatMode": "once",
        "entryTimingProfiles": [
            {
                "startRemainingSec": 90,
                "endRemainingSec": 45,
                "maxPriceCent": 60,
                "priceToBeatTriggerMinGap": 10,
                "sizeUsdc": 1.5
            },
            {
                "startRemainingSec": 45,
                "endRemainingSec": 20,
                "maxPriceCent": 67,
                "priceToBeatTriggerMinGap": 18,
                "sizeUsdc": 1.0
            }
        ]
    }));

    let selected = resolve_trigger_market_entry_timing_profile(
        &node,
        "btc-updown-5m-1773319200",
        DateTime::<Utc>::from_timestamp_millis(1_773_319_444_900).expect("evaluated_at"),
    )
    .expect("selected profile");

    assert_eq!(selected.index, 0);
    assert_eq!(selected.start_remaining_sec, 90);
    assert_eq!(selected.end_remaining_sec, 45);
    assert_eq!(selected.max_price, Some(0.60));
    assert_eq!(selected.price_to_beat_trigger_min_gap, Some(10.0));
    assert_eq!(selected.size_usdc, Some(1.5));
}

#[test]
fn trigger_entry_timing_profile_uses_relative_remaining_time_for_15m_market() {
    let node = trigger_market_price_node(json!({
        "marketMode": "auto_scope",
        "repeatMode": "once",
        "entryTimingProfiles": [
            { "startRemainingSec": 45, "endRemainingSec": 20, "sizeUsdc": 1.0 },
            { "startRemainingSec": 20, "endRemainingSec": 8, "sizeUsdc": 0.5 }
        ]
    }));

    let selected = resolve_trigger_market_entry_timing_profile(
        &node,
        "btc-updown-15m-1773319200",
        DateTime::<Utc>::from_timestamp_millis(1_773_320_080_500).expect("evaluated_at"),
    )
    .expect("selected profile");

    assert_eq!(selected.index, 1);
    assert_eq!(selected.size_usdc, Some(0.5));
}

#[test]
fn trigger_entry_timing_profile_returns_none_inside_tail_lockout() {
    let node = trigger_market_price_node(json!({
        "marketMode": "auto_scope",
        "repeatMode": "once",
        "entryTimingProfiles": [
            { "startRemainingSec": 20, "endRemainingSec": 8, "sizeUsdc": 0.5 }
        ]
    }));

    let selected = resolve_trigger_market_entry_timing_profile(
        &node,
        "btc-updown-5m-1773319200",
        DateTime::<Utc>::from_timestamp_millis(1_773_319_492_200).expect("evaluated_at"),
    );

    assert!(selected.is_none());
}

#[test]
fn action_place_order_selected_entry_size_prefers_step_input_over_context() {
    let step = test_step(json!({
        "selectedEntrySizeUsdc": 1.5
    }));
    let context = json!({
        "flowContext": {
            "selectedEntrySizeUsdc": 2.0
        }
    });

    assert_eq!(
        resolve_action_place_order_selected_entry_size_usdc(&step, &context),
        Some(1.5)
    );
}

#[test]
fn trade_flow_buy_fill_lock_round_trips_through_flow_context() {
    let mut context = json!({
        "flowContext": {},
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {}
    });
    let record = TradeFlowBuyFillLockRecord {
        group: "late-entry".to_string(),
        market_slug: "btc-updown-5m-1773319200".to_string(),
        builder_order_id: 77,
        source_node_key: "buy_1".to_string(),
        filled_at: "2026-04-22T00:00:00Z".to_string(),
        release_on_stop_loss: true,
    };

    set_trade_flow_buy_fill_lock(&mut context, &record);

    let stored = find_trade_flow_buy_fill_lock_for_market(
        &context,
        "late-entry",
        "btc-updown-5m-1773319200",
    )
    .expect("stored lock");
    assert_eq!(stored, record);
    assert!(clear_trade_flow_buy_fill_lock(&mut context, "late-entry"));
    assert!(
        find_trade_flow_buy_fill_lock(&context, "late-entry").is_none(),
        "lock should be removed"
    );
}

#[test]
fn trade_flow_buy_fill_lock_release_requires_matching_owner_and_zero_inventory() {
    let record = TradeFlowBuyFillLockRecord {
        group: "late-entry".to_string(),
        market_slug: "btc-updown-5m-1773319200".to_string(),
        builder_order_id: 77,
        source_node_key: "buy_1".to_string(),
        filled_at: "2026-04-22T00:00:00Z".to_string(),
        release_on_stop_loss: true,
    };
    let mut parent_order = test_builder_order("buy", None);
    parent_order.id = 77;
    parent_order.market_slug = "btc-updown-5m-1773319200".to_string();

    let closed_position = TradeBuilderParentPosition {
        parent_builder_order_id: 77,
        user_id: 1,
        source_trade_id: 77,
        market_slug: "btc-updown-5m-1773319200".to_string(),
        token_id: "tok-up".to_string(),
        outcome_label: "Up".to_string(),
        baseline_qty: 10.0,
        current_qty: 0.0,
        last_fill_qty: Some(10.0),
        last_fill_price: Some(0.55),
        qty_source: "child_fill:actual".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let open_position = TradeBuilderParentPosition {
        current_qty: 2.5,
        ..closed_position.clone()
    };

    assert!(should_release_trade_flow_buy_fill_lock(
        &record,
        &parent_order,
        &closed_position
    ));
    assert!(!should_release_trade_flow_buy_fill_lock(
        &record,
        &parent_order,
        &open_position
    ));

    parent_order.id = 78;
    assert!(!should_release_trade_flow_buy_fill_lock(
        &record,
        &parent_order,
        &closed_position
    ));
}
