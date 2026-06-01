fn action_place_order_revenge_flip_buy_uses_auto_source_trade(
    node: &TradeFlowNode,
    side: &str,
) -> bool {
    side == "buy"
        && node_config_bool(node, REVENGE_FLIP_ORDER_MARKER_KEY).unwrap_or(false)
        && node_config_string(node, "sizeMode")
            .map(|value| value.trim().eq_ignore_ascii_case("usdc"))
            .unwrap_or(false)
}

fn resolve_flow_source_trade_id(node: &TradeFlowNode, context: &Value) -> Option<i64> {
    node_config_i64(node, "sourceTradeId")
        .or_else(|| {
            context
                .get("flowContext")
                .and_then(|v| v.get("sourceTradeId"))
                .and_then(value_as_i64)
        })
        .filter(|value| *value > 0)
}

fn resolve_action_place_order_source_trade_id(
    node: &TradeFlowNode,
    context: &Value,
    step: &TradeFlowRunStep,
    side: &str,
) -> Option<i64> {
    if action_place_order_revenge_flip_buy_uses_auto_source_trade(node, side) {
        return None;
    }

    node_config_i64(node, "sourceTradeId")
        .or_else(|| {
            context
                .get("flowContext")
                .and_then(|v| v.get("sourceTradeId"))
                .and_then(value_as_i64)
        })
        .or_else(|| {
            step_input_i64(step, &["sourceTradeId", "source_trade_id"]).filter(|value| *value > 0)
        })
        .filter(|value| *value > 0)
}

fn resolve_action_place_order_revenge_flip_stop_loss_reference_price(
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
) -> Option<f64> {
    step_input_f64(
        step,
        &[
            "wsBestBid",
            "ws_best_bid",
            "currentPrice",
            "current_price",
            "triggered_price",
            "price",
            "wsPrice",
        ],
    )
    .or_else(|| resolve_action_place_order_reference_price(node, step))
    .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
    .map(clamp_probability)
}

fn resolve_action_place_order_revenge_flip_stop_loss_sell_sizing(
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
    trigger_size_for_first_fire: Option<f64>,
    configured_target_qty: Option<f64>,
) -> Result<ActionPlaceOrderSizing> {
    let target_qty = trigger_size_for_first_fire
        .or(configured_target_qty)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "action.place_order requires targetQty > 0 when sizeMode is shares"
            )
        })?;
    anyhow::ensure!(
        target_qty > 0.0 && target_qty.is_finite(),
        "action.place_order targetQty must be > 0"
    );
    let target_qty = round_trade_builder_share_qty(target_qty);
    anyhow::ensure!(
        target_qty > 0.0,
        "action.place_order sell resolved target qty must be > 0"
    );
    let reference_price = resolve_action_place_order_revenge_flip_stop_loss_reference_price(
        node, step,
    )
    .unwrap_or_else(|| action_place_order_reference_price_for_share_sizing(None));

    Ok(ActionPlaceOrderSizing {
        size_usdc: (target_qty * reference_price).max(0.0),
        size_basis: TRADE_BUILDER_SIZE_BASIS_SHARES,
        target_qty: Some(target_qty),
        remaining_qty: Some(target_qty),
        resolved_size_mode: "shares",
        resolved_size_pct: None,
    })
}

#[cfg(test)]
mod action_place_order_source_trade_tests {
    use super::*;

    fn node(config: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "order".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    fn step(input: Value) -> TradeFlowRunStep {
        TradeFlowRunStep {
            id: 1,
            run_id: 1,
            node_key: "order".to_string(),
            node_type: "action.place_order".to_string(),
            status: "pending".to_string(),
            attempt: 1,
            input_json: Some(input),
            output_json: None,
            error_text: None,
            started_at: None,
            ended_at: None,
            available_at: Utc::now(),
            parent_step_id: None,
            idempotency_key: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn revenge_flip_buy_ignores_stale_context_and_step_source_trade_id() {
        let node = node(json!({
            "revengeFlipOrder": true,
            "side": "buy",
            "sizeMode": "usdc",
        }));
        let context = json!({ "flowContext": { "sourceTradeId": 108233 } });
        let step = step(json!({ "sourceTradeId": 108233 }));

        assert_eq!(
            resolve_action_place_order_source_trade_id(&node, &context, &step, "buy"),
            None
        );
    }

    #[test]
    fn normal_sell_still_uses_context_source_trade_id() {
        let node = node(json!({ "side": "sell" }));
        let context = json!({ "flowContext": { "sourceTradeId": 42 } });
        let step = step(json!({}));

        assert_eq!(
            resolve_action_place_order_source_trade_id(&node, &context, &step, "sell"),
            Some(42)
        );
    }

    #[test]
    fn revenge_flip_stop_loss_sell_sizing_uses_target_qty_without_position_lookup() {
        let node = node(json!({
            "revengeFlipStopLossSell": true,
            "targetQty": 3.39,
        }));
        let step = step(json!({ "wsBestBid": 0.44, "currentPrice": 0.45 }));

        let sizing = resolve_action_place_order_revenge_flip_stop_loss_sell_sizing(
            &node,
            &step,
            None,
            action_place_order_target_qty(&node),
        )
        .expect("sizing");

        assert_eq!(sizing.size_basis, TRADE_BUILDER_SIZE_BASIS_SHARES);
        assert_eq!(sizing.target_qty, Some(3.39));
        assert_eq!(sizing.remaining_qty, Some(3.39));
        assert!((sizing.size_usdc - 1.4916).abs() < 0.0000001);
    }
}
