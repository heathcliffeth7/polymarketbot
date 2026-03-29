use super::support::*;
use super::*;
use bot_infra::exchange::OrderBookLevel;

#[test]
fn exit_child_sizing_uses_filled_qty_as_share_target() {
    let sizing = trade_builder_exit_child_sizing(5.104, 0.98);
    assert_eq!(sizing.target_qty, 5.10);
    assert_eq!(sizing.remaining_qty, 5.10);
    assert!((sizing.size_usdc - 4.998).abs() < 0.000001);
}

#[test]
fn terminal_fill_qty_prefers_positive_filled_size() {
    let candidates = TradeBuilderTerminalFillQtyCandidates {
        order_info_filled_size: Some(11.628),
        synced_db_fill_qty: Some(11.61),
        order_info_size: Some(11.63),
        stored_order_size: Some(11.64),
    };

    let resolved = select_trade_builder_terminal_fill_qty(candidates).unwrap();
    assert_eq!(
        resolved.source,
        TradeBuilderTerminalFillQtySource::OrderInfoFilledSize
    );
    assert_eq!(resolved.qty, 11.63);
}

#[test]
fn terminal_fill_qty_prefers_status_size_before_synced_db_fill() {
    let candidates = TradeBuilderTerminalFillQtyCandidates {
        order_info_filled_size: Some(0.0),
        synced_db_fill_qty: Some(11.571),
        order_info_size: Some(11.63),
        stored_order_size: Some(11.63),
    };

    let resolved = select_trade_builder_terminal_fill_qty(candidates).unwrap();
    assert_eq!(
        resolved.source,
        TradeBuilderTerminalFillQtySource::OrderInfoSize
    );
    assert_eq!(resolved.qty, 11.63);
}

#[test]
fn terminal_fill_qty_falls_back_to_status_size_when_fill_missing() {
    let candidates = TradeBuilderTerminalFillQtyCandidates {
        order_info_filled_size: None,
        synced_db_fill_qty: Some(0.0),
        order_info_size: Some(12.994),
        stored_order_size: Some(12.90),
    };

    let resolved = select_trade_builder_terminal_fill_qty(candidates).unwrap();
    assert_eq!(
        resolved.source,
        TradeBuilderTerminalFillQtySource::OrderInfoSize
    );
    assert_eq!(resolved.qty, 12.99);

    let sizing = trade_builder_exit_child_sizing(resolved.qty, 0.69);
    assert_eq!(sizing.target_qty, 12.99);
    assert!(sizing.size_usdc > 8.96);
}

#[test]
fn terminal_fill_qty_falls_back_to_stored_submitted_size() {
    let candidates = TradeBuilderTerminalFillQtyCandidates {
        order_info_filled_size: Some(0.0),
        synced_db_fill_qty: Some(0.0),
        order_info_size: None,
        stored_order_size: Some(11.634),
    };

    let resolved = select_trade_builder_terminal_fill_qty(candidates).unwrap();
    assert_eq!(
        resolved.source,
        TradeBuilderTerminalFillQtySource::StoredOrderSize
    );
    assert_eq!(resolved.qty, 11.63);
}

#[test]
fn terminal_fill_qty_returns_none_when_all_candidates_are_non_positive() {
    let candidates = TradeBuilderTerminalFillQtyCandidates {
        order_info_filled_size: Some(0.0),
        synced_db_fill_qty: Some(0.004),
        order_info_size: None,
        stored_order_size: Some(f64::NAN),
    };

    assert!(select_trade_builder_terminal_fill_qty(candidates).is_none());
}

#[test]
fn visible_inventory_expectation_prefers_submitted_dynamic_qty_over_resolved_fill_qty() {
    let expectation = trade_builder_visible_inventory_expectation(
        Some(11.57),
        Some(11.63),
        Some(0.58),
        Some(0.86),
        1000,
    )
    .unwrap();

    assert_eq!(expectation.gross_qty_source, "submitted_dynamic_qty");
    assert_eq!(expectation.gross_qty, 11.63);
    assert!(expectation.expected_fee_qty > 0.0);
    assert!(expectation.expected_visible_qty < 11.63);
}

#[test]
fn visible_inventory_expectation_falls_back_to_submitted_qty() {
    let expectation =
        trade_builder_visible_inventory_expectation(None, Some(11.63), None, Some(0.86), 1000)
            .unwrap();

    assert_eq!(expectation.gross_qty_source, "submitted_dynamic_qty");
    assert_eq!(expectation.gross_qty, 11.63);
    assert_eq!(expectation.reference_price, 0.86);
}

#[test]
fn canonical_entry_qty_uses_submitted_dynamic_qty_for_parent_buy() {
    let mut order = test_builder_order("buy", None);
    order.tp_enabled = true;
    order.submitted_dynamic_qty = Some(11.63);

    let (canonical_qty, source) = trade_builder_canonical_entry_qty(&order, Some(11.57)).unwrap();

    assert_eq!(canonical_qty, 11.63);
    assert_eq!(source, "submitted_dynamic_qty");
}

#[test]
fn canonical_entry_qty_uses_cumulative_fill_qty_for_parent_buy() {
    let mut order = test_builder_order("buy", None);
    order.tp_enabled = true;
    order.filled_qty = 3.0;

    let (canonical_qty, source) = trade_builder_canonical_entry_qty(&order, Some(2.81)).unwrap();

    assert_eq!(canonical_qty, 5.81);
    assert_eq!(source, "cumulative_fill_qty");
}

#[test]
fn canonical_entry_qty_no_double_count_when_latest_fill_added_once() {
    let mut order = test_builder_order("buy", None);
    order.tp_enabled = true;
    order.filled_qty = 3.0;

    let (canonical_qty, source) = trade_builder_canonical_entry_qty(&order, Some(3.0)).unwrap();

    assert_eq!(canonical_qty, 6.0);
    assert_eq!(source, "cumulative_fill_qty");
}

#[test]
fn observed_submit_qty_uses_cumulative_fill_qty_when_prior_fill_exists() {
    let mut order = test_builder_order("buy", None);
    order.tp_enabled = true;
    order.filled_qty = 3.0;

    let (observed_qty, source) = trade_builder_observed_submit_qty(&order, Some(2.81)).unwrap();

    assert_eq!(observed_qty, 5.81);
    assert_eq!(source, "cumulative_fill_qty");
}

#[test]
fn observed_fill_qty_uses_cumulative_fill_qty_when_prior_fill_exists() {
    let mut order = test_builder_order("buy", None);
    order.tp_enabled = true;
    order.filled_qty = 3.0;

    let (observed_qty, source) = trade_builder_observed_fill_qty(&order, Some(2.81)).unwrap();

    assert_eq!(observed_qty, 5.81);
    assert_eq!(source, "cumulative_fill_qty");
}

#[test]
fn inventory_expectation_cumulative_fill_uses_resolved_fill_path() {
    let mut order = test_builder_order("buy", None);
    order.tp_enabled = true;
    order.filled_qty = 3.0;

    let observed_fill_qty = trade_builder_observed_fill_qty(&order, Some(2.81))
        .map(|(qty, _)| qty)
        .unwrap();
    let expectation = trade_builder_visible_inventory_expectation(
        Some(observed_fill_qty),
        None,
        Some(0.86),
        Some(0.86),
        1000,
    )
    .unwrap();
    let sizing = trade_builder_exit_child_sizing(observed_fill_qty, 0.86);

    assert_eq!(expectation.gross_qty_source, "resolved_fill_qty");
    assert_eq!(expectation.gross_qty, 5.81);
    assert_eq!(sizing.target_qty, 5.81);
}

#[test]
fn child_execution_price_falls_back_to_submitted_dynamic_price() {
    let mut order = test_builder_order("buy", None);
    order.tp_enabled = true;
    order.submitted_dynamic_price = Some(0.86);

    let price = trade_builder_child_execution_price(&order, None, None, None).unwrap();

    assert_eq!(price, 0.86);
}

#[test]
fn first_visible_inventory_snapshot_uses_baseline_delta() {
    let snapshot = trade_builder_first_visible_inventory_snapshot(
        Some(1.23),
        12.80,
        Some(11.63),
        Some(11.57),
        Some(11.51),
    );

    assert_eq!(snapshot.actual_visible_qty, 12.80);
    assert_eq!(snapshot.visible_delta_qty, Some(11.57));
    assert_eq!(snapshot.gap_vs_submit_qty, Some(-0.06));
    assert_eq!(snapshot.gap_vs_fill_qty, Some(0.0));
    assert_eq!(snapshot.gap_vs_expected_qty, Some(0.06));
}

#[test]
fn first_visible_inventory_snapshot_without_baseline_keeps_gaps_empty() {
    let snapshot = trade_builder_first_visible_inventory_snapshot(
        None,
        11.57,
        Some(11.63),
        Some(11.57),
        Some(11.51),
    );

    assert_eq!(snapshot.actual_visible_qty, 11.57);
    assert_eq!(snapshot.visible_delta_qty, None);
    assert_eq!(snapshot.gap_vs_submit_qty, None);
    assert_eq!(snapshot.gap_vs_fill_qty, None);
    assert_eq!(snapshot.gap_vs_expected_qty, None);
}

#[test]
fn latched_stop_loss_stays_triggered_after_price_recovers() {
    let mut order = test_builder_order("sell", Some(9));
    order.status = "inventory_pending".to_string();
    order.trigger_condition = Some("cross_below".to_string());
    order.trigger_price = Some(0.60);
    order.last_seen_price = Some(0.94);
    order.trigger_latched = true;
    order.trigger_latched_reason = Some("stop_loss".to_string());

    assert!(should_trigger_builder_order(&order, 0.99));
    assert!(should_trigger_builder_order(&order, 0.40));
}

#[test]
fn inventory_pending_tp_uses_slack_window() {
    let mut order = test_builder_order("sell", Some(9));
    order.status = "inventory_pending".to_string();
    order.trigger_condition = Some("cross_above".to_string());
    order.trigger_price = Some(0.98);
    order.last_seen_price = Some(0.99);

    assert!(should_trigger_builder_order(&order, 0.95));
    assert!(!should_trigger_builder_order(&order, 0.92));
}

#[test]
fn pending_child_stop_loss_uses_first_tick_threshold() {
    let mut order = test_builder_order("sell", Some(9));
    order.status = "pending".to_string();
    order.trigger_condition = Some("cross_below".to_string());
    order.trigger_price = Some(0.60);
    order.last_seen_price = None;

    let evaluation = evaluate_trade_builder_order_trigger(&order, None, 0.01);
    assert!(evaluation.should_trigger);
    assert!(evaluation.first_tick_threshold_used);
}

#[test]
fn pending_child_take_profit_uses_first_tick_threshold() {
    let mut order = test_builder_order("sell", Some(9));
    order.status = "pending".to_string();
    order.trigger_condition = Some("cross_above".to_string());
    order.trigger_price = Some(0.98);
    order.last_seen_price = None;

    let evaluation = evaluate_trade_builder_order_trigger(&order, None, 0.99);
    assert!(evaluation.should_trigger);
    assert!(evaluation.first_tick_threshold_used);
}

#[test]
fn guard_blocked_conditional_order_still_requires_trigger_to_hold() {
    let mut order = test_builder_order("buy", None);
    order.status = "guard_blocked".to_string();
    order.trigger_condition = Some("cross_above".to_string());
    order.trigger_price = Some(0.80);
    order.last_seen_price = Some(0.81);

    assert!(!should_trigger_builder_order(&order, 0.79));
    assert!(should_trigger_builder_order(&order, 0.82));
}

#[test]
fn conditional_level_above_order_triggers_while_above_threshold() {
    let mut order = test_builder_order("buy", None);
    order.status = "armed".to_string();
    order.trigger_condition = Some("level_above".to_string());
    order.trigger_price = Some(0.80);
    order.last_seen_price = Some(0.92);

    assert!(should_trigger_builder_order(&order, 0.83));
    assert!(!should_trigger_builder_order(&order, 0.79));
}

#[test]
fn conditional_level_below_order_triggers_while_below_threshold() {
    let mut order = test_builder_order("buy", None);
    order.status = "armed".to_string();
    order.trigger_condition = Some("level_below".to_string());
    order.trigger_price = Some(0.20);
    order.last_seen_price = Some(0.18);

    assert!(should_trigger_builder_order(&order, 0.17));
    assert!(!should_trigger_builder_order(&order, 0.21));
}

#[test]
fn triggered_conditional_sell_orders_stay_latched_after_cross() {
    let mut tp_order = test_builder_order("sell", Some(9));
    tp_order.status = "triggered".to_string();
    tp_order.trigger_condition = Some("cross_above".to_string());
    tp_order.trigger_price = Some(0.80);
    tp_order.last_seen_price = Some(0.81);

    assert!(should_trigger_builder_order(&tp_order, 0.85));
    assert!(!should_trigger_builder_order(&tp_order, 0.79));

    let mut sl_order = test_builder_order("sell", Some(9));
    sl_order.status = "triggered".to_string();
    sl_order.trigger_condition = Some("cross_below".to_string());
    sl_order.trigger_price = Some(0.40);
    sl_order.last_seen_price = Some(0.39);

    assert!(should_trigger_builder_order(&sl_order, 0.35));
    assert!(!should_trigger_builder_order(&sl_order, 0.41));
}

#[test]
fn pending_child_exit_sell_uses_threshold_only_after_previous_tick() {
    let mut tp_order = test_builder_order("sell", Some(9));
    tp_order.status = "pending".to_string();
    tp_order.trigger_condition = Some("cross_above".to_string());
    tp_order.trigger_price = Some(0.80);
    tp_order.last_seen_price = Some(0.85);

    let tp_evaluation = evaluate_trade_builder_order_trigger(&tp_order, Some(0.85), 0.86);
    assert!(tp_evaluation.should_trigger);
    assert!(!tp_evaluation.first_tick_threshold_used);

    let mut sl_order = test_builder_order("sell", Some(9));
    sl_order.status = "pending".to_string();
    sl_order.trigger_condition = Some("cross_below".to_string());
    sl_order.trigger_price = Some(0.40);
    sl_order.last_seen_price = Some(0.35);

    let sl_evaluation = evaluate_trade_builder_order_trigger(&sl_order, Some(0.35), 0.34);
    assert!(sl_evaluation.should_trigger);
    assert!(!sl_evaluation.first_tick_threshold_used);
}

#[test]
fn fast_runtime_helpers_split_trigger_and_execution_prices() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.77,
        source: "ws_fast_book_last_trade",
        runtime_warning: None,
        best_bid: Some(0.76),
        best_ask: Some(0.79),
        last_trade_price: Some(0.77),
    };

    let mut buy_order = test_builder_order("buy", None);
    buy_order.trigger_condition = Some("cross_above".to_string());

    let mut sell_order = test_builder_order("sell", Some(9));
    sell_order.trigger_condition = Some("cross_below".to_string());

    assert_eq!(
        trade_builder_trigger_eval_price_for_order(&buy_order, &runtime_price),
        0.77
    );
    assert_eq!(
        trade_builder_trigger_eval_price_for_order(&sell_order, &runtime_price),
        0.76
    );
    assert_eq!(
        trade_builder_execution_price_for_order(&buy_order, &runtime_price),
        0.79
    );
    assert_eq!(
        trade_builder_execution_price_for_order(&sell_order, &runtime_price),
        0.76
    );
    assert_eq!(
        trade_builder_last_seen_price_for_order(&buy_order, 0.77, 0.79),
        0.77
    );
    assert_eq!(
        trade_builder_last_seen_price_for_order(&sell_order, 0.76, 0.76),
        0.76
    );
}

#[test]
fn immediate_buy_notional_execution_price_prefers_best_ask() {
    let mut buy_order = test_builder_order("buy", None);
    buy_order.kind = "immediate".to_string();
    buy_order.trigger_condition = None;
    buy_order.trigger_price = Some(0.26);
    buy_order.size_usdc = 10.0;
    buy_order.min_price_distance_cent = 1.0;

    let resolution =
        trade_builder_immediate_buy_notional_execution_price(&buy_order, 0.81, Some(0.79))
            .expect("live buy execution price");
    let qty = calc_level_size(buy_order.size_usdc, resolution.price);

    assert_eq!(resolution.price, 0.79);
    assert_eq!(resolution.source, "best_ask");
    assert_eq!(resolution.trigger_reference_price, Some(0.26));
    assert_eq!(qty, 12.66);
}

#[test]
fn conditional_buy_submit_price_keeps_runtime_bump_behavior() {
    let mut buy_order = test_builder_order("buy", None);
    buy_order.kind = "conditional".to_string();
    buy_order.trigger_condition = Some("cross_above".to_string());
    buy_order.trigger_price = Some(0.26);
    buy_order.size_usdc = 10.0;
    buy_order.min_price_distance_cent = 1.0;

    let desired_price = trade_builder_submit_desired_price(&buy_order, 0.81);

    assert!((desired_price - 0.82).abs() < 0.000001);
}

#[test]
fn immediate_buy_notional_execution_price_falls_back_to_current_price() {
    let mut buy_order = test_builder_order("buy", None);
    buy_order.kind = "immediate".to_string();
    buy_order.trigger_condition = None;
    buy_order.trigger_price = Some(0.31);
    buy_order.size_usdc = 10.0;
    buy_order.remaining_size = Some(10.0);
    buy_order.working_price = Some(0.31);

    let resolution = trade_builder_immediate_buy_notional_execution_price(&buy_order, 0.98, None)
        .expect("live buy execution price");
    let qty = calc_level_size(
        buy_order.remaining_size.unwrap_or_default(),
        resolution.price,
    );

    assert!((resolution.price - 0.98).abs() < 0.000001);
    assert_eq!(resolution.source, "current_price_fallback");
    assert_eq!(resolution.trigger_reference_price, Some(0.31));
    assert!((qty - 10.20).abs() < 0.000001);
}

#[test]
fn immediate_buy_execution_price_resolution_ignores_share_basis_orders() {
    let mut buy_order = test_builder_order("buy", None);
    buy_order.kind = "immediate".to_string();
    buy_order.trigger_condition = None;
    buy_order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    buy_order.target_qty = Some(5.0);
    buy_order.remaining_qty = Some(5.0);

    assert!(
        trade_builder_immediate_buy_notional_execution_price(&buy_order, 0.81, Some(0.79))
            .is_none()
    );
}

fn test_stop_loss_sell_order_for_submit() -> TradeBuilderOrder {
    let mut order = test_builder_order("sell", Some(9));
    order.status = "armed".to_string();
    order.trigger_condition = Some("cross_below".to_string());
    order.trigger_price = Some(0.60);
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.target_qty = Some(4.0);
    order.remaining_qty = Some(4.0);
    order
}

#[test]
fn sell_submit_price_uses_best_bid_when_single_level_covers_qty() {
    let order = test_stop_loss_sell_order_for_submit();
    let order_book = OrderBookSnapshot {
        bids: vec![
            OrderBookLevel {
                price: 0.49,
                size: 2.0,
            },
            OrderBookLevel {
                price: 0.50,
                size: 5.0,
            },
        ],
        asks: Vec::new(),
    };

    let resolved = resolve_trade_builder_sell_submit_price_with_book(
        &order,
        0.48,
        Some(0.48),
        Some(0.47),
        Some(4.0),
        Some(&order_book),
    );

    assert_eq!(resolved.source, "orderbook_depth");
    assert_eq!(resolved.desired_price, 0.50);
    assert_eq!(resolved.depth_levels_used, Some(1));
    assert_eq!(resolved.visible_bid_qty, Some(7.0));
    assert_eq!(resolved.requested_qty, Some(4.0));
}

#[test]
fn sell_submit_price_walks_bid_depth_for_multi_level_fill() {
    let mut order = test_stop_loss_sell_order_for_submit();
    order.target_qty = Some(4.5);
    order.remaining_qty = Some(4.5);
    let order_book = OrderBookSnapshot {
        bids: vec![
            OrderBookLevel {
                price: 0.30,
                size: 3.0,
            },
            OrderBookLevel {
                price: 0.31,
                size: 2.0,
            },
            OrderBookLevel {
                price: 0.32,
                size: 1.0,
            },
        ],
        asks: Vec::new(),
    };

    let resolved = resolve_trade_builder_sell_submit_price_with_book(
        &order,
        0.31,
        Some(0.31),
        Some(0.30),
        Some(4.5),
        Some(&order_book),
    );

    assert_eq!(resolved.source, "orderbook_depth");
    assert_eq!(resolved.desired_price, 0.30);
    assert_eq!(resolved.depth_levels_used, Some(3));
    assert_eq!(resolved.visible_bid_qty, Some(6.0));
    assert_eq!(resolved.requested_qty, Some(4.5));
}

#[test]
fn sell_submit_price_falls_back_to_best_bid_when_depth_is_insufficient() {
    let order = test_stop_loss_sell_order_for_submit();
    let order_book = OrderBookSnapshot {
        bids: vec![
            OrderBookLevel {
                price: 0.39,
                size: 1.0,
            },
            OrderBookLevel {
                price: 0.40,
                size: 2.0,
            },
        ],
        asks: Vec::new(),
    };

    let resolved = resolve_trade_builder_sell_submit_price_with_book(
        &order,
        0.38,
        Some(0.41),
        Some(0.37),
        Some(4.0),
        Some(&order_book),
    );

    assert_eq!(resolved.source, "best_bid_fallback");
    assert_eq!(resolved.desired_price, 0.41);
    assert_eq!(resolved.depth_levels_used, None);
    assert_eq!(resolved.visible_bid_qty, Some(3.0));
    assert_eq!(resolved.requested_qty, Some(4.0));
}

#[test]
fn sell_submit_price_falls_back_to_last_trade_then_current_price() {
    let order = test_stop_loss_sell_order_for_submit();

    let last_trade_resolved = resolve_trade_builder_sell_submit_price_with_book(
        &order,
        0.38,
        None,
        Some(0.37),
        Some(4.0),
        None,
    );
    assert_eq!(last_trade_resolved.source, "last_trade_fallback");
    assert_eq!(last_trade_resolved.desired_price, 0.37);

    let price_resolved = resolve_trade_builder_sell_submit_price_with_book(
        &order,
        0.38,
        None,
        None,
        Some(4.0),
        None,
    );
    assert_eq!(price_resolved.source, "price_fallback");
    assert_eq!(price_resolved.desired_price, 0.38);
}

#[test]
fn ws_fast_path_scope_only_includes_tp_sl_child_orders() {
    let mut tp_child = test_builder_order("sell", Some(9));
    tp_child.status = "armed".to_string();
    tp_child.trigger_condition = Some("cross_above".to_string());

    let mut sl_child = test_builder_order("sell", Some(9));
    sl_child.status = "triggered".to_string();
    sl_child.trigger_condition = Some("cross_below".to_string());

    let mut parentless_sell = test_builder_order("sell", None);
    parentless_sell.status = "armed".to_string();

    let mut parent_buy = test_builder_order("buy", None);
    parent_buy.status = "armed".to_string();

    assert!(trade_builder_is_ws_fast_path_tp_sl_child(&tp_child));
    assert!(trade_builder_is_ws_fast_path_tp_sl_child(&sl_child));
    assert!(!trade_builder_is_ws_fast_path_tp_sl_child(&parentless_sell));
    assert!(!trade_builder_is_ws_fast_path_tp_sl_child(&parent_buy));
}

#[test]
fn ws_fast_path_snapshot_last_seen_uses_exit_sell_runtime_price() {
    let mut order = test_builder_order("sell", Some(9));
    order.status = "armed".to_string();
    order.trigger_condition = Some("cross_above".to_string());
    let snapshot = MarketDataSnapshot {
        best_bid: Some(0.76),
        best_ask: Some(0.79),
        last_trade_price: Some(0.77),
        updated_at_ms: 123,
        last_source: "book".to_string(),
    };

    assert_eq!(
        trade_builder_last_seen_price_from_market_snapshot(&order, &snapshot),
        Some(0.76)
    );
}

#[test]
fn stop_loss_trigger_modes_use_selected_source_only() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.70,
        source: "ws_fast_book_last_trade",
        runtime_warning: None,
        best_bid: Some(0.70),
        best_ask: Some(0.74),
        last_trade_price: Some(0.60),
    };

    let mut stop_loss_order = test_builder_order("sell", Some(9));
    stop_loss_order.trigger_condition = Some("cross_below".to_string());
    stop_loss_order.trigger_price = Some(0.65);

    stop_loss_order.sl_trigger_price_mode = Some("best_bid".to_string());
    assert_eq!(
        trade_builder_trigger_eval_price_for_order(&stop_loss_order, &runtime_price),
        0.70
    );
    assert!(!evaluate_trade_builder_order_trigger(&stop_loss_order, None, 0.70).should_trigger);

    stop_loss_order.sl_trigger_price_mode = Some("composite".to_string());
    assert_eq!(
        trade_builder_trigger_eval_price_for_order(&stop_loss_order, &runtime_price),
        0.60
    );
    assert!(evaluate_trade_builder_order_trigger(&stop_loss_order, None, 0.60).should_trigger);

    stop_loss_order.sl_trigger_price_mode = Some("last_trade".to_string());
    assert_eq!(
        trade_builder_trigger_eval_price_for_order(&stop_loss_order, &runtime_price),
        0.60
    );
    assert!(evaluate_trade_builder_order_trigger(&stop_loss_order, None, 0.60).should_trigger);
}

#[test]
fn strict_stop_loss_trigger_modes_require_selected_source() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.62,
        source: "ws_fast_last_trade",
        runtime_warning: None,
        best_bid: None,
        best_ask: Some(0.66),
        last_trade_price: Some(0.62),
    };

    assert_eq!(
        sl_trigger_eval_price_for_mode("best_bid", &runtime_price),
        None
    );
    assert_eq!(
        sl_trigger_eval_price_for_mode("last_trade", &runtime_price),
        Some(0.62)
    );

    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.62,
        source: "ws_fast_book",
        runtime_warning: None,
        best_bid: Some(0.62),
        best_ask: Some(0.66),
        last_trade_price: None,
    };

    assert_eq!(
        sl_trigger_eval_price_for_mode("best_bid", &runtime_price),
        Some(0.62)
    );
    assert_eq!(
        sl_trigger_eval_price_for_mode("last_trade", &runtime_price),
        None
    );
}

#[test]
fn legacy_composite_sl_bid_confirmation_guard_blocks_when_bid_above_trigger() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.60,
        source: "ws_fast_last_trade",
        runtime_warning: None,
        best_bid: Some(0.88),
        best_ask: Some(0.90),
        last_trade_price: Some(0.45),
    };

    let mut order = test_builder_order("sell", Some(9));
    order.trigger_condition = Some("cross_below".to_string());
    order.trigger_price = Some(0.60);
    order.sl_trigger_price_mode = Some("composite".to_string());

    assert!(should_skip_trade_builder_composite_sl_bid_confirmation(
        &order,
        &runtime_price
    ));
}

#[test]
fn composite_sl_bid_confirmation_guard_allows_equal_or_lower_bid() {
    let mut order = test_builder_order("sell", Some(9));
    order.trigger_condition = Some("cross_below".to_string());
    order.trigger_price = Some(0.60);
    order.sl_trigger_price_mode = Some("composite".to_string());

    let equal_bid = TradeBuilderRuntimePrice {
        price: 0.60,
        source: "ws_fast_book_last_trade",
        runtime_warning: None,
        best_bid: Some(0.60),
        best_ask: Some(0.62),
        last_trade_price: Some(0.45),
    };
    assert!(!should_skip_trade_builder_composite_sl_bid_confirmation(
        &order, &equal_bid
    ));

    let lower_bid = TradeBuilderRuntimePrice {
        best_bid: Some(0.20),
        ..equal_bid
    };
    assert!(!should_skip_trade_builder_composite_sl_bid_confirmation(
        &order, &lower_bid
    ));
}

#[test]
fn legacy_composite_sl_bid_confirmation_guard_blocks_without_bid() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.45,
        source: "ws_fast_last_trade",
        runtime_warning: None,
        best_bid: None,
        best_ask: Some(0.47),
        last_trade_price: Some(0.45),
    };

    let mut order = test_builder_order("sell", Some(9));
    order.trigger_condition = Some("cross_below".to_string());
    order.trigger_price = Some(0.60);
    order.sl_trigger_price_mode = Some("composite".to_string());

    assert!(should_skip_trade_builder_composite_sl_bid_confirmation(
        &order,
        &runtime_price
    ));
}

#[test]
fn composite_fast_stop_loss_uses_last_trade_even_when_bid_is_far_higher() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.55,
        source: "ws_fast_book_last_trade",
        runtime_warning: None,
        best_bid: Some(0.55),
        best_ask: Some(0.57),
        last_trade_price: Some(0.20),
    };

    let mut order = test_builder_order("sell", Some(9));
    order.trigger_condition = Some("cross_below".to_string());
    order.trigger_price = Some(0.40);
    order.sl_trigger_price_mode = Some("composite_fast".to_string());

    assert_eq!(
        trade_builder_trigger_eval_price_for_order(&order, &runtime_price),
        0.20
    );
    assert!(evaluate_trade_builder_order_trigger(&order, None, 0.20).should_trigger);
}

#[test]
fn composite_safe_matches_legacy_composite_guard_behavior() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.60,
        source: "ws_fast_last_trade",
        runtime_warning: None,
        best_bid: Some(0.88),
        best_ask: Some(0.90),
        last_trade_price: Some(0.45),
    };

    let mut order = test_builder_order("sell", Some(9));
    order.trigger_condition = Some("cross_below".to_string());
    order.trigger_price = Some(0.60);
    order.sl_trigger_price_mode = Some("composite_safe".to_string());

    assert!(should_skip_trade_builder_composite_sl_bid_confirmation(
        &order,
        &runtime_price
    ));
}

#[test]
fn preempted_stop_loss_is_ready_for_inline_submit_after_bid_confirmation() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.34,
        source: "ws_fast_book_last_trade",
        runtime_warning: None,
        best_bid: Some(0.34),
        best_ask: Some(0.36),
        last_trade_price: Some(0.34),
    };

    let mut order = test_builder_order("sell", Some(9));
    order.status = "armed".to_string();
    order.trigger_condition = Some("cross_below".to_string());
    order.trigger_price = Some(0.40);
    order.last_seen_price = Some(0.61);
    order.sl_trigger_price_mode = Some("composite_safe".to_string());

    let decision = evaluate_trade_builder_preempted_stop_loss(&order, &runtime_price)
        .expect("stop loss should be ready once bid confirms below the trigger");

    assert_eq!(decision.current_price, 0.34);
    assert!(decision.ready_for_inline_submit);
}

#[test]
fn preempted_stop_loss_waits_for_composite_bid_confirmation() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.34,
        source: "ws_fast_book_last_trade",
        runtime_warning: None,
        best_bid: Some(0.55),
        best_ask: Some(0.57),
        last_trade_price: Some(0.34),
    };

    let mut order = test_builder_order("sell", Some(9));
    order.status = "armed".to_string();
    order.trigger_condition = Some("cross_below".to_string());
    order.trigger_price = Some(0.40);
    order.last_seen_price = Some(0.61);
    order.sl_trigger_price_mode = Some("composite_safe".to_string());

    assert!(evaluate_trade_builder_preempted_stop_loss(&order, &runtime_price).is_none());
}

#[test]
fn preempted_stop_loss_skips_inline_submit_when_live_order_exists() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.34,
        source: "ws_fast_book_last_trade",
        runtime_warning: None,
        best_bid: Some(0.34),
        best_ask: Some(0.36),
        last_trade_price: Some(0.34),
    };

    let mut order = test_builder_order("sell", Some(9));
    order.status = "armed".to_string();
    order.trigger_condition = Some("cross_below".to_string());
    order.trigger_price = Some(0.40);
    order.last_seen_price = Some(0.61);
    order.sl_trigger_price_mode = Some("composite_safe".to_string());
    order.active_exchange_order_id = Some("live-sl-order".to_string());

    let decision = evaluate_trade_builder_preempted_stop_loss(&order, &runtime_price)
        .expect("stop loss should still trigger while a live exchange order exists");

    assert_eq!(decision.current_price, 0.34);
    assert!(!decision.ready_for_inline_submit);
}

#[test]
fn composite_fast_does_not_wait_for_bid_confirmation() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.60,
        source: "ws_fast_last_trade",
        runtime_warning: None,
        best_bid: Some(0.88),
        best_ask: Some(0.90),
        last_trade_price: Some(0.45),
    };

    let mut order = test_builder_order("sell", Some(9));
    order.trigger_condition = Some("cross_below".to_string());
    order.trigger_price = Some(0.60);
    order.sl_trigger_price_mode = Some("composite_fast".to_string());

    assert!(!should_skip_trade_builder_composite_sl_bid_confirmation(
        &order,
        &runtime_price
    ));
}

#[test]
fn composite_sl_bid_confirmation_guard_ignores_other_orders() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.45,
        source: "ws_fast_book_last_trade",
        runtime_warning: None,
        best_bid: Some(0.88),
        best_ask: Some(0.90),
        last_trade_price: Some(0.45),
    };

    let mut best_bid_sl = test_builder_order("sell", Some(9));
    best_bid_sl.trigger_condition = Some("cross_below".to_string());
    best_bid_sl.trigger_price = Some(0.60);
    best_bid_sl.sl_trigger_price_mode = Some("best_bid".to_string());
    assert!(!should_skip_trade_builder_composite_sl_bid_confirmation(
        &best_bid_sl,
        &runtime_price
    ));

    let mut last_trade_sl = test_builder_order("sell", Some(9));
    last_trade_sl.trigger_condition = Some("cross_below".to_string());
    last_trade_sl.trigger_price = Some(0.60);
    last_trade_sl.sl_trigger_price_mode = Some("last_trade".to_string());
    assert!(!should_skip_trade_builder_composite_sl_bid_confirmation(
        &last_trade_sl,
        &runtime_price
    ));

    let mut buy_order = test_builder_order("buy", None);
    buy_order.trigger_condition = Some("cross_above".to_string());
    buy_order.trigger_price = Some(0.60);
    buy_order.sl_trigger_price_mode = Some("composite".to_string());
    assert!(!should_skip_trade_builder_composite_sl_bid_confirmation(
        &buy_order,
        &runtime_price
    ));
}

#[test]
fn sl_trigger_mode_is_ignored_for_non_stop_loss_orders() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.77,
        source: "ws_fast_book_last_trade",
        runtime_warning: None,
        best_bid: Some(0.76),
        best_ask: Some(0.79),
        last_trade_price: Some(0.77),
    };

    let mut buy_order = test_builder_order("buy", None);
    buy_order.trigger_condition = Some("cross_above".to_string());
    buy_order.sl_trigger_price_mode = Some("best_bid".to_string());

    let mut take_profit_order = test_builder_order("sell", Some(9));
    take_profit_order.trigger_condition = Some("cross_above".to_string());
    take_profit_order.sl_trigger_price_mode = Some("last_trade".to_string());

    assert_eq!(
        trade_builder_trigger_eval_price_for_order(&buy_order, &runtime_price),
        0.77
    );
    assert_eq!(
        trade_builder_trigger_eval_price_for_order(&take_profit_order, &runtime_price),
        0.77
    );
}

#[test]
fn fast_runtime_scope_excludes_parentless_conditional_sell() {
    let child_sell = test_builder_order("sell", Some(9));

    let mut entry_buy = test_builder_order("buy", None);
    entry_buy.trigger_condition = Some("cross_above".to_string());

    let mut workflow_sell = test_builder_order("sell", None);
    workflow_sell.trigger_condition = Some("cross_above".to_string());

    assert!(trade_builder_uses_fast_runtime_pricing(&child_sell));
    assert!(trade_builder_uses_fast_runtime_pricing(&entry_buy));
    assert!(!trade_builder_uses_fast_runtime_pricing(&workflow_sell));
}

#[test]
fn parent_position_seed_qty_prefers_actual_visible_inventory() {
    let seed = TradeBuilderParentPositionSeed {
        actual_visible_qty: Some(2.846),
        expected_visible_qty: Some(2.91),
        reference_price: Some(0.68),
        qty_source: Some("available_token_qty".to_string()),
    };

    let (qty, source) = trade_builder_parent_position_seed_qty(Some(&seed), 2.94);

    assert_eq!(qty, 2.85);
    assert_eq!(source, "inventory_actual_visible_qty");
}

#[test]
fn parent_position_seed_qty_falls_back_to_expected_visible_inventory() {
    let seed = TradeBuilderParentPositionSeed {
        actual_visible_qty: None,
        expected_visible_qty: Some(2.8888),
        reference_price: Some(0.68),
        qty_source: Some("available_token_qty".to_string()),
    };

    let (qty, source) = trade_builder_parent_position_seed_qty(Some(&seed), 2.94);

    assert_eq!(qty, 2.89);
    assert_eq!(source, "inventory_expected_visible_qty");
}

#[test]
fn parent_position_seed_qty_falls_back_to_canonical_fill_qty() {
    let (qty, source) = trade_builder_parent_position_seed_qty(None, 2.944);

    assert_eq!(qty, 2.94);
    assert_eq!(source, "canonical_entry_qty");
}

#[test]
fn staged_exit_children_initial_sizing_uses_total_entry_qty() {
    let first_sl = trade_builder_ladder_child_qty(2.78, 50.0).expect("first staged sl");
    let second_sl = trade_builder_ladder_child_qty(2.78, 50.0).expect("second staged sl");

    assert_eq!(first_sl.target_qty, 1.39);
    assert_eq!(first_sl.remaining_qty, 1.39);
    assert_eq!(second_sl.target_qty, 1.39);
    assert_eq!(second_sl.remaining_qty, 1.39);
}

#[test]
fn preempted_staged_stop_loss_inventory_plan_ignores_take_profit_remaining_qty() {
    let mut tp = test_builder_order("sell", Some(9));
    tp.id = 31;
    tp.status = "armed".to_string();
    tp.trigger_condition = Some("cross_above".to_string());
    tp.trigger_price = Some(0.84);
    tp.exit_ladder_kind = Some("tp".to_string());
    tp.exit_ladder_index = Some(0);
    tp.exit_ladder_size_pct = Some(50.0);
    tp.target_qty = Some(0.71);
    tp.remaining_qty = Some(0.71);

    let mut sl = test_builder_order("sell", Some(9));
    sl.id = 32;
    sl.status = "armed".to_string();
    sl.trigger_condition = Some("cross_below".to_string());
    sl.trigger_price = Some(0.50);
    sl.exit_ladder_kind = Some("sl".to_string());
    sl.exit_ladder_index = Some(1);
    sl.exit_ladder_size_pct = Some(50.0);
    sl.target_qty = Some(1.41);
    sl.remaining_qty = Some(1.41);

    let plan = plan_trade_builder_preempted_stop_loss_inventory(&[tp, sl], Some(1.41));

    assert_eq!(plan.current_parent_qty, Some(1.41));
    assert_eq!(plan.remaining_qty_for(32), Some(1.41));
}

#[test]
fn staged_take_profit_last_stage_absorbs_rounding_remainder() {
    let mut first_tp = test_builder_order("sell", Some(9));
    first_tp.id = 41;
    first_tp.status = "armed".to_string();
    first_tp.trigger_condition = Some("cross_above".to_string());
    first_tp.trigger_price = Some(0.80);
    first_tp.exit_ladder_kind = Some("tp".to_string());
    first_tp.exit_ladder_index = Some(0);
    first_tp.exit_ladder_size_pct = Some(33.33);

    let mut second_tp = first_tp.clone();
    second_tp.id = 42;
    second_tp.exit_ladder_index = Some(1);

    let mut third_tp = first_tp.clone();
    third_tp.id = 43;
    third_tp.exit_ladder_index = Some(2);
    third_tp.exit_ladder_size_pct = Some(33.34);

    let targets = trade_builder_ladder_family_target_qtys(&[&first_tp, &second_tp, &third_tp], 1.0);

    assert_eq!(targets, vec![(41, 0.33), (42, 0.33), (43, 0.34)]);
}

#[test]
fn single_live_take_profit_stage_becomes_full_close() {
    let mut last_tp = test_builder_order("sell", Some(9));
    last_tp.id = 52;
    last_tp.status = "armed".to_string();
    last_tp.trigger_condition = Some("cross_above".to_string());
    last_tp.trigger_price = Some(0.99);
    last_tp.exit_ladder_kind = Some("tp".to_string());
    last_tp.exit_ladder_index = Some(1);
    last_tp.exit_ladder_size_pct = Some(50.0);

    let targets = trade_builder_ladder_family_target_qtys(&[&last_tp], 1.41);

    assert_eq!(targets, vec![(52, 1.41)]);
}

#[test]
fn parallel_exit_batch_selects_all_eligible_tp_levels() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.88,
        source: "ws_fast_book_last_trade",
        runtime_warning: None,
        best_bid: Some(0.87),
        best_ask: Some(0.89),
        last_trade_price: Some(0.88),
    };

    let mut first_tp = test_builder_order("sell", Some(9));
    first_tp.id = 11;
    first_tp.status = "armed".to_string();
    first_tp.trigger_condition = Some("cross_above".to_string());
    first_tp.trigger_price = Some(0.80);
    first_tp.last_seen_price = Some(0.70);
    first_tp.exit_ladder_kind = Some("tp".to_string());
    first_tp.exit_ladder_index = Some(0);

    let mut second_tp = first_tp.clone();
    second_tp.id = 12;
    second_tp.trigger_price = Some(0.84);
    second_tp.last_seen_price = Some(0.79);
    second_tp.exit_ladder_index = Some(1);

    let selection =
        trade_builder_parallel_exit_batch_selection(&[first_tp, second_tp], &runtime_price);

    assert_eq!(selection.stop_loss_order_ids, Vec::<i64>::new());
    assert_eq!(selection.take_profit_order_ids, vec![11, 12]);
    assert_eq!(selection.selected_order_ids, vec![11, 12]);
    assert_eq!(selection.deferred_order_ids, Vec::<i64>::new());
}

#[test]
fn parallel_exit_batch_selects_all_eligible_sl_levels() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.31,
        source: "ws_fast_book_last_trade",
        runtime_warning: None,
        best_bid: Some(0.31),
        best_ask: Some(0.33),
        last_trade_price: Some(0.31),
    };

    let mut first_sl = test_builder_order("sell", Some(9));
    first_sl.id = 21;
    first_sl.status = "armed".to_string();
    first_sl.trigger_condition = Some("cross_below".to_string());
    first_sl.trigger_price = Some(0.40);
    first_sl.last_seen_price = Some(0.61);
    first_sl.exit_ladder_kind = Some("sl".to_string());
    first_sl.exit_ladder_index = Some(0);
    first_sl.sl_trigger_price_mode = Some("composite_safe".to_string());

    let mut second_sl = first_sl.clone();
    second_sl.id = 22;
    second_sl.trigger_price = Some(0.35);
    second_sl.last_seen_price = Some(0.50);
    second_sl.exit_ladder_index = Some(1);

    let selection =
        trade_builder_parallel_exit_batch_selection(&[first_sl, second_sl], &runtime_price);

    assert_eq!(selection.stop_loss_order_ids, vec![21, 22]);
    assert_eq!(selection.take_profit_order_ids, Vec::<i64>::new());
    assert_eq!(selection.selected_order_ids, vec![21, 22]);
    assert_eq!(selection.deferred_order_ids, Vec::<i64>::new());
}

#[test]
fn parallel_exit_batch_prefers_stop_loss_when_tp_and_sl_are_both_eligible() {
    let runtime_price = TradeBuilderRuntimePrice {
        price: 0.35,
        source: "ws_fast_book_last_trade",
        runtime_warning: None,
        best_bid: Some(0.35),
        best_ask: Some(0.37),
        last_trade_price: Some(0.35),
    };

    let mut tp = test_builder_order("sell", Some(9));
    tp.id = 31;
    tp.status = "armed".to_string();
    tp.trigger_condition = Some("cross_above".to_string());
    tp.trigger_price = Some(0.30);
    tp.last_seen_price = Some(0.20);
    tp.exit_ladder_kind = Some("tp".to_string());
    tp.exit_ladder_index = Some(0);

    let mut sl = test_builder_order("sell", Some(9));
    sl.id = 32;
    sl.status = "armed".to_string();
    sl.trigger_condition = Some("cross_below".to_string());
    sl.trigger_price = Some(0.40);
    sl.last_seen_price = Some(0.60);
    sl.exit_ladder_kind = Some("sl".to_string());
    sl.exit_ladder_index = Some(0);
    sl.sl_trigger_price_mode = Some("composite_safe".to_string());

    let selection = trade_builder_parallel_exit_batch_selection(&[tp, sl], &runtime_price);

    assert_eq!(selection.stop_loss_order_ids, vec![32]);
    assert_eq!(selection.take_profit_order_ids, vec![31]);
    assert_eq!(selection.selected_order_ids, vec![32]);
    assert_eq!(selection.deferred_order_ids, vec![31]);
}

#[test]
fn window_end_auto_sell_sizing_uses_full_parent_qty() {
    let sizing = action_place_order_window_end_auto_sell_sizing_from_parent_qty(0.774, 0.26)
        .expect("window-end sizing");

    assert_eq!(sizing.size_basis, TRADE_BUILDER_SIZE_BASIS_SHARES);
    assert_eq!(sizing.target_qty, Some(0.77));
    assert_eq!(sizing.remaining_qty, Some(0.77));
    assert_eq!(sizing.resolved_size_mode, "pct");
    assert_eq!(sizing.resolved_size_pct, Some(100.0));
    assert!((sizing.size_usdc - 0.2002).abs() < 0.000001);
}

#[test]
fn armed_builder_ws_eval_logs_meaningful_activity_without_passive_sample() {
    let activity = ArmedBuilderWsEvalActivity {
        selected_token_count: 1,
        selected_order_count: 1,
        evaluated_order_count: 1,
        last_seen_update_count: 1,
        triggered_order_count: 0,
        composite_waiting_count: 1,
        composite_released_count: 0,
        selected_source_missing_count: 0,
    };

    assert!(armed_builder_ws_eval_should_log(activity, false));
}

#[test]
fn armed_builder_ws_eval_skips_passive_cycle_without_sample() {
    let activity = ArmedBuilderWsEvalActivity {
        selected_token_count: 1,
        selected_order_count: 2,
        evaluated_order_count: 2,
        last_seen_update_count: 2,
        triggered_order_count: 0,
        composite_waiting_count: 0,
        composite_released_count: 0,
        selected_source_missing_count: 0,
    };

    assert!(!armed_builder_ws_eval_should_log(activity, false));
    assert!(armed_builder_ws_eval_should_log(activity, true));
}

#[test]
fn share_basis_exit_sell_orders_are_retryable() {
    let mut exit_sell = test_builder_order("sell", Some(9));
    exit_sell.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    exit_sell.target_qty = Some(5.10);
    exit_sell.remaining_qty = Some(5.10);

    let mut buy_order = exit_sell.clone();
    buy_order.side = "buy".to_string();

    let mut sell_notional = exit_sell.clone();
    sell_notional.size_basis = TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC.to_string();
    sell_notional.target_qty = None;
    sell_notional.remaining_qty = None;

    assert!(trade_builder_should_retry_exit_sell(&exit_sell));
    assert!(!trade_builder_should_retry_exit_sell(&buy_order));
    assert!(!trade_builder_should_retry_exit_sell(&sell_notional));
}
