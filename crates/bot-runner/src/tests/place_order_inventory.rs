use super::support::*;
use super::*;

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
fn composite_sl_bid_confirmation_guard_blocks_when_bid_above_trigger() {
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
        &runtime_price));
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
        &order, &equal_bid));

    let lower_bid = TradeBuilderRuntimePrice {
        best_bid: Some(0.20),
        ..equal_bid
    };
    assert!(!should_skip_trade_builder_composite_sl_bid_confirmation(
        &order, &lower_bid));
}

#[test]
fn composite_sl_bid_confirmation_guard_blocks_without_bid() {
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
        &runtime_price));
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
        &runtime_price));

    let mut last_trade_sl = test_builder_order("sell", Some(9));
    last_trade_sl.trigger_condition = Some("cross_below".to_string());
    last_trade_sl.trigger_price = Some(0.60);
    last_trade_sl.sl_trigger_price_mode = Some("last_trade".to_string());
    assert!(!should_skip_trade_builder_composite_sl_bid_confirmation(
        &last_trade_sl,
        &runtime_price));

    let mut buy_order = test_builder_order("buy", None);
    buy_order.trigger_condition = Some("cross_above".to_string());
    buy_order.trigger_price = Some(0.60);
    buy_order.sl_trigger_price_mode = Some("composite".to_string());
    assert!(!should_skip_trade_builder_composite_sl_bid_confirmation(
        &buy_order,
        &runtime_price));
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
