use super::support::*;
use super::*;

#[test]
fn optimistic_exit_stage_defaults_to_dynamic_gross() {
    let mut child_sell = test_builder_order("sell", Some(9));
    child_sell.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    child_sell.target_qty = Some(5.10);
    child_sell.remaining_qty = Some(5.10);

    assert_eq!(
        trade_builder_current_exit_submit_stage(&child_sell),
        TradeBuilderExitSubmitStage::DynamicGross
    );
}

#[test]
fn optimistic_exit_stage_parses_last_error_marker() {
    let mut child_sell = test_builder_order("sell", Some(9));
    child_sell.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    child_sell.target_qty = Some(5.10);
    child_sell.remaining_qty = Some(5.10);
    child_sell.last_error =
        Some("not enough balance / allowance [exit_submit_stage=visible_inventory]".to_string());

    assert_eq!(
        trade_builder_current_exit_submit_stage(&child_sell),
        TradeBuilderExitSubmitStage::VisibleInventory
    );
}

#[test]
fn optimistic_exit_submit_scope_targets_child_share_sells() {
    let mut child_sell = test_builder_order("sell", Some(9));
    child_sell.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    child_sell.target_qty = Some(5.10);
    child_sell.remaining_qty = Some(5.10);

    let mut buy_child = child_sell.clone();
    buy_child.side = "buy".to_string();

    let mut parent_sell = child_sell.clone();
    parent_sell.parent_order_id = None;

    let mut notional_child = child_sell.clone();
    notional_child.size_basis = TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC.to_string();
    notional_child.target_qty = None;
    notional_child.remaining_qty = None;

    assert!(trade_builder_should_use_optimistic_exit_submit(&child_sell));
    assert!(!trade_builder_should_use_optimistic_exit_submit(&buy_child));
    assert!(!trade_builder_should_use_optimistic_exit_submit(
        &parent_sell
    ));
    assert!(!trade_builder_should_use_optimistic_exit_submit(
        &notional_child
    ));
}

#[test]
fn optimistic_exit_submit_scope_targets_flow_immediate_share_sells() {
    let mut flow_sell = test_builder_order("sell", None);
    flow_sell.kind = "immediate".to_string();
    flow_sell.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    flow_sell.target_qty = Some(5.10);
    flow_sell.remaining_qty = Some(5.10);
    flow_sell.origin_flow_run_id = Some(42);

    let mut flow_buy = flow_sell.clone();
    flow_buy.side = "buy".to_string();

    let mut no_origin = flow_sell.clone();
    no_origin.origin_flow_run_id = None;

    let mut notional_flow = flow_sell.clone();
    notional_flow.size_basis = TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC.to_string();
    notional_flow.target_qty = None;
    notional_flow.remaining_qty = None;

    assert!(trade_builder_should_use_optimistic_exit_submit(&flow_sell));
    assert!(!trade_builder_should_use_optimistic_exit_submit(&flow_buy));
    assert!(!trade_builder_should_use_optimistic_exit_submit(&no_origin));
    assert!(!trade_builder_should_use_optimistic_exit_submit(
        &notional_flow
    ));
}

#[test]
fn midpoint_404_processing_error_is_retryable_for_exit_sell() {
    let mut order = test_builder_order("sell", Some(9));
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.last_error = Some(
            "HTTP status client error (404 Not Found) for url (https://clob.polymarket.com/midpoint?token_id=tok)"
                .to_string(),
        );

    assert!(trade_builder_should_retry_after_processing_error(&order));
}

#[test]
fn fatal_exchange_rejection_catches_invalid_signature() {
    assert!(trade_builder_error_is_fatal_exchange_rejection(
        r#"HTTP status 400 Bad Request for POST /order | body: {"error":"invalid signature"}"#
    ));
}

#[test]
fn fatal_exchange_rejection_does_not_catch_balance() {
    assert!(!trade_builder_error_is_fatal_exchange_rejection(
        "not enough balance / allowance"
    ));
}

#[test]
fn fee_rate_lookup_result_accepts_zero_fee_without_fallback() {
    let (resolved_fee_rate_bps, used_fallback) =
        resolve_trade_builder_fee_rate_lookup_result(0, Some(0));

    assert_eq!(resolved_fee_rate_bps, 0);
    assert!(!used_fallback);
}

#[test]
fn fee_rate_lookup_result_falls_back_when_lookup_is_missing() {
    let (resolved_fee_rate_bps, used_fallback) =
        resolve_trade_builder_fee_rate_lookup_result(0, None);

    assert_eq!(resolved_fee_rate_bps, DEFAULT_TRADE_BUILDER_FEE_RATE_BPS);
    assert!(used_fallback);
}

#[test]
fn should_retry_exit_sell_false_on_fatal_error() {
    let mut order = test_builder_order("sell", Some(9));
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.target_qty = Some(5.10);
    order.remaining_qty = Some(5.10);
    order.last_error = Some("invalid signature".to_string());

    assert!(!trade_builder_should_retry_exit_sell(&order));
}

#[test]
fn should_retry_after_processing_error_false_on_fatal() {
    let mut order = test_builder_order("sell", Some(9));
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.target_qty = Some(5.10);
    order.remaining_qty = Some(5.10);
    order.last_error = Some("invalid signature".to_string());

    assert!(!trade_builder_should_retry_after_processing_error(&order));
}

#[test]
fn trade_flow_should_inline_submit_only_when_flagged() {
    assert!(trade_flow_should_inline_submit(&json!({
        "builder_order_id": 7,
        "should_inline_submit": true
    })));
    assert!(!trade_flow_should_inline_submit(&json!({
        "builder_order_id": 7,
        "should_inline_submit": false
    })));
    assert!(!trade_flow_should_inline_submit(&json!({
        "builder_order_id": 7
    })));
}

#[test]
fn trade_builder_order_processing_guard_serializes_same_order_id() {
    let first_guard = try_acquire_trade_builder_order_processing_guard(99).expect("first guard");
    assert!(try_acquire_trade_builder_order_processing_guard(99).is_none());
    drop(first_guard);
    assert!(try_acquire_trade_builder_order_processing_guard(99).is_some());
}

#[test]
fn runtime_price_fallback_prefers_last_seen_price() {
    let mut order = test_builder_order("sell", Some(9));
    order.last_seen_price = Some(0.74);
    order.working_price = Some(0.81);

    let fallback = trade_builder_runtime_price_fallback(&order).unwrap();
    assert_eq!(fallback.source, "last_seen_price");
    assert_eq!(fallback.price, 0.74);
    assert_eq!(fallback.best_bid, None);
    assert_eq!(fallback.best_ask, None);
    assert_eq!(fallback.last_trade_price, None);
}

#[test]
fn runtime_price_fallback_uses_working_price_when_last_seen_missing() {
    let mut order = test_builder_order("sell", Some(9));
    order.last_seen_price = None;
    order.working_price = Some(0.81);

    let fallback = trade_builder_runtime_price_fallback(&order).unwrap();
    assert_eq!(fallback.source, "working_price");
    assert_eq!(fallback.price, 0.81);
}

#[test]
fn fast_runtime_price_rest_partial_failure_uses_last_trade_only() {
    let (runtime_price, runtime_warning) =
        resolve_trade_builder_fast_runtime_price_from_rest_results(
            Err(anyhow::anyhow!("book offline")),
            Ok(Some(0.73)),
        );

    let runtime_price = runtime_price.expect("runtime price");
    assert_eq!(runtime_price.source, "rest_fast_last_trade");
    assert_eq!(runtime_price.price, 0.73);
    assert_eq!(runtime_price.best_bid, None);
    assert_eq!(runtime_price.best_ask, None);
    assert_eq!(runtime_price.last_trade_price, Some(0.73));
    assert!(runtime_warning.unwrap().contains("best_bid_ask"));
}

#[test]
fn fast_runtime_price_rest_partial_failure_uses_book_only() {
    let (runtime_price, runtime_warning) =
        resolve_trade_builder_fast_runtime_price_from_rest_results(
            Ok((Some(0.61), Some(0.63))),
            Err(anyhow::anyhow!("trade offline")),
        );

    let runtime_price = runtime_price.expect("runtime price");
    assert_eq!(runtime_price.source, "rest_fast_book");
    assert_eq!(runtime_price.price, 0.61);
    assert_eq!(runtime_price.best_bid, Some(0.61));
    assert_eq!(runtime_price.best_ask, Some(0.63));
    assert_eq!(runtime_price.last_trade_price, None);
    assert!(runtime_warning.unwrap().contains("last_trade_price"));
}

#[test]
fn exit_sell_price_floor_uses_trigger_buffer() {
    let mut tp_order = test_builder_order("sell", Some(9));
    tp_order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    tp_order.target_qty = Some(7.35);
    tp_order.remaining_qty = Some(7.35);
    tp_order.trigger_condition = Some("cross_above".to_string());
    tp_order.trigger_price = Some(0.98);

    let mut sl_order = tp_order.clone();
    sl_order.trigger_condition = Some("cross_below".to_string());
    sl_order.trigger_price = Some(0.60);

    assert_eq!(trade_builder_exit_sell_price_floor(&tp_order), Some(0.93));
    assert_eq!(trade_builder_exit_sell_price_floor(&sl_order), None);
}

#[test]
fn exit_sell_price_cap_never_chases_beyond_trigger_buffer() {
    let mut tp_order = test_builder_order("sell", Some(9));
    tp_order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    tp_order.target_qty = Some(7.35);
    tp_order.remaining_qty = Some(7.35);
    tp_order.trigger_condition = Some("cross_above".to_string());
    tp_order.trigger_price = Some(0.98);

    let mut sl_order = tp_order.clone();
    sl_order.trigger_condition = Some("cross_below".to_string());
    sl_order.trigger_price = Some(0.60);

    assert_eq!(trade_builder_cap_exit_sell_price(&tp_order, 0.27), 0.93);
    assert_eq!(trade_builder_cap_exit_sell_price(&sl_order, 0.27), 0.27);
    assert_eq!(trade_builder_cap_exit_sell_price(&tp_order, 0.97), 0.97);
}

#[test]
fn post_cancel_fill_qty_prefers_status_filled_size() {
    assert_eq!(
        trade_builder_detected_cancel_fill_qty(Some(5.811), 2.81),
        Some(5.81)
    );
}

#[test]
fn post_cancel_fill_qty_falls_back_to_db_aggregate() {
    assert_eq!(
        trade_builder_detected_cancel_fill_qty(None, 2.814),
        Some(2.81)
    );
    assert_eq!(trade_builder_detected_cancel_fill_qty(None, 0.0), None);
}

#[test]
fn post_cancel_fill_notional_prefers_db_then_price_fallback() {
    assert_eq!(
        trade_builder_detected_cancel_fill_notional(4.91, Some(5.81), Some(0.83), Some(0.86), 0.9),
        4.91
    );
    assert!(
        (trade_builder_detected_cancel_fill_notional(0.0, Some(5.81), Some(0.83), Some(0.86), 0.9)
            - 4.8223)
            .abs()
            < 0.000001
    );
    assert!(
        (trade_builder_detected_cancel_fill_notional(0.0, Some(5.81), None, Some(0.86), 0.9)
            - 4.9966)
            .abs()
            < 0.000001
    );
}

#[test]
fn post_cancel_full_fill_detection_uses_status_or_size_match() {
    assert!(trade_builder_cancel_fill_is_full("filled", None, 2.0));
    assert!(trade_builder_cancel_fill_is_full("open", Some(5.812), 5.81));
    assert!(!trade_builder_cancel_fill_is_full("open", Some(6.25), 5.81));
}

#[test]
fn post_cancel_partial_fill_remaining_usdc_clamps_at_zero() {
    assert_eq!(
        trade_builder_remaining_usdc_after_partial_fill(Some(5.0), None, 5.0, 2.24),
        2.76
    );
    assert_eq!(
        trade_builder_remaining_usdc_after_partial_fill(None, Some(5.0), 5.0, 8.0),
        0.0
    );
}

#[test]
fn take_profit_child_detection_distinguishes_tp_from_sl() {
    let mut tp_order = test_builder_order("sell", Some(9));
    tp_order.trigger_condition = Some("cross_above".to_string());

    let mut sl_order = tp_order.clone();
    sl_order.trigger_condition = Some("cross_below".to_string());

    assert!(trade_builder_is_take_profit_child(&tp_order));
    assert!(!trade_builder_is_take_profit_child(&sl_order));
    assert!(trade_builder_is_stop_loss_child(&sl_order));
}

#[test]
fn share_basis_remaining_qty_does_not_expand_at_low_price() {
    let mut order = test_builder_order("sell", Some(9));
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.target_qty = Some(5.10);
    order.remaining_qty = Some(5.10);

    let order_info = OrderInfo {
        order_id: "ord-1".to_string(),
        client_order_id: None,
        status: "live".to_string(),
        price: Some(0.01),
        size: Some(5.10),
        filled_size: Some(0.0),
    };

    let (remaining_usdc, remaining_qty) =
        estimate_remaining_trade_builder_sizing(&order, &order_info, 0.01);
    assert_eq!(remaining_qty, Some(5.10));
    assert_eq!(remaining_usdc, Some(0.051));
}

#[test]
fn visible_share_qty_is_clamped_with_floor_precision() {
    assert_eq!(
        clamp_trade_builder_visible_share_qty(6.02, Some(5.9815)),
        Some(5.98)
    );
    assert_eq!(
        clamp_trade_builder_visible_share_qty(6.02, Some(6.50)),
        Some(6.02)
    );
    assert_eq!(
        clamp_trade_builder_visible_share_qty(6.02, Some(0.009)),
        None
    );
}

#[test]
fn visible_inventory_submit_clamps_requested_qty() {
    let resolution = resolve_trade_builder_visible_inventory_submit(6.02, Some(5.9815)).unwrap();
    assert_eq!(resolution.submit_qty, 5.98);
    assert!(resolution.submit_partial_visible_inventory);
}

#[test]
fn stop_loss_local_inventory_fallback_shaves_buy_fee_and_buffer() {
    let mut order = test_builder_order("sell", Some(9));
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.target_qty = Some(11.63);
    order.remaining_qty = Some(11.63);
    order.size_usdc = 10.0018;
    order.trigger_condition = Some("cross_below".to_string());
    order.trigger_latched = true;
    order.trigger_latched_reason = Some("stop_loss".to_string());
    order.fee_rate_bps = 1000;

    let fallback = trade_builder_local_inventory_fallback(&order, 11.63).unwrap();
    assert_eq!(fallback.submit_qty, 11.46);
    assert!(fallback.estimated_fee_qty > 0.04);
    assert!(fallback.estimated_fee_qty < 0.06);
}

#[test]
fn estimated_visible_exit_qty_applies_to_take_profit_children() {
    let mut order = test_builder_order("sell", Some(9));
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.target_qty = Some(11.63);
    order.remaining_qty = Some(11.63);
    order.size_usdc = 10.0018;
    order.trigger_condition = Some("cross_above".to_string());
    order.fee_rate_bps = 1000;

    let estimate = trade_builder_estimated_visible_exit_qty(&order, 11.63).unwrap();
    assert_eq!(estimate.submit_qty, 11.46);
    assert!(estimate.estimated_fee_qty > 0.04);
    assert!(estimate.estimated_fee_qty < 0.06);
}

#[test]
fn optimistic_exit_retry_qty_prefers_estimated_visible_qty() {
    let mut order = test_builder_order("sell", Some(9));
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.target_qty = Some(11.63);
    order.remaining_qty = Some(11.63);
    order.size_usdc = 10.0018;
    order.trigger_condition = Some("cross_above".to_string());
    order.fee_rate_bps = 1000;

    let resolution = resolve_trade_builder_exit_retry_qty(&order, 11.63).unwrap();
    assert_eq!(
        resolution.source,
        TradeBuilderExitRetryQtySource::EstimatedVisibleQty
    );
    assert_eq!(resolution.next_qty, 11.46);
    assert!(resolution.estimated_fee_qty.unwrap() > 0.04);
}

#[test]
fn optimistic_exit_retry_qty_accepts_one_tick_estimated_decrement() {
    let mut order = test_builder_order("sell", Some(9));
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.target_qty = Some(5.05);
    order.remaining_qty = Some(5.05);
    order.size_usdc = 4.949;
    order.trigger_condition = Some("cross_above".to_string());
    order.fee_rate_bps = 1000;

    let resolution = resolve_trade_builder_exit_retry_qty(&order, 5.05).unwrap();
    assert_eq!(
        resolution.source,
        TradeBuilderExitRetryQtySource::EstimatedVisibleQty
    );
    assert_eq!(resolution.formula_qty, Some(5.0));
    assert_eq!(resolution.next_qty, 5.0);
    assert_eq!(resolution.forced_tick_qty, None);
}

#[test]
fn optimistic_exit_retry_qty_forces_one_tick_when_formula_does_not_reduce() {
    let mut order = test_builder_order("sell", Some(9));
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.target_qty = Some(10.00);
    order.remaining_qty = Some(10.00);
    order.size_usdc = 9.80;
    order.trigger_condition = Some("cross_above".to_string());
    order.fee_rate_bps = 1000;

    let resolution = resolve_trade_builder_exit_retry_qty(&order, 5.05).unwrap();
    assert_eq!(
        resolution.source,
        TradeBuilderExitRetryQtySource::ForcedTickQty
    );
    assert_eq!(resolution.formula_qty, Some(5.05));
    assert_eq!(resolution.forced_tick_qty, Some(5.04));
    assert_eq!(resolution.next_qty, 5.04);
}

#[test]
fn next_retry_share_qty_shaves_one_tick() {
    assert_eq!(trade_builder_next_retry_share_qty(5.05), Some(5.04));
    assert_eq!(trade_builder_next_retry_share_qty(0.01), None);
}

#[test]
fn stop_loss_inventory_resolution_prefers_local_fallback_when_visible_zero() {
    let mut order = test_builder_order("sell", Some(9));
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.target_qty = Some(12.05);
    order.remaining_qty = Some(12.05);
    order.size_usdc = 10.0015;
    order.trigger_condition = Some("cross_below".to_string());
    order.trigger_latched = true;
    order.trigger_latched_reason = Some("stop_loss".to_string());
    order.fee_rate_bps = 1000;

    let resolution = resolve_trade_builder_exit_inventory(&order, 12.05, Some(0.0)).unwrap();
    assert_eq!(resolution.submit_qty, 11.86);
    assert_eq!(resolution.local_fallback_qty, Some(11.86));
    assert!(resolution.submit_partial_visible_inventory);
}

#[test]
fn exit_qty_buffer_uses_floor_and_rate() {
    assert_eq!(trade_builder_exit_qty_buffer(2.0), 0.03);
    assert_eq!(trade_builder_exit_qty_buffer(5.0), 0.05);
    assert_eq!(trade_builder_exit_qty_buffer(10.0), 0.1);
    assert_eq!(trade_builder_exit_qty_buffer(15.0), 0.15);
}

#[test]
fn inventory_pending_tp_trigger_price_applies_slack_only_to_tp_children() {
    let mut tp_order = test_builder_order("sell", Some(9));
    tp_order.status = "inventory_pending".to_string();
    tp_order.trigger_condition = Some("cross_above".to_string());
    tp_order.trigger_price = Some(0.98);

    let mut sl_order = test_builder_order("sell", Some(9));
    sl_order.status = "inventory_pending".to_string();
    sl_order.trigger_condition = Some("cross_below".to_string());
    sl_order.trigger_price = Some(0.60);

    assert_eq!(
        trade_builder_inventory_pending_tp_trigger_price(&tp_order),
        Some(0.93)
    );
    assert_eq!(
        trade_builder_inventory_pending_tp_trigger_price(&sl_order),
        Some(0.60)
    );
}
