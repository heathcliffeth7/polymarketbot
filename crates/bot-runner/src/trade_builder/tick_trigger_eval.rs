#[derive(Debug, Clone, PartialEq)]
struct TickTrigger {
    order_id: i64,
    user_id: i64,
    token_id: String,
    trigger_price: f64,
    tick_price: f64,
    trigger_kind: &'static str,
}

fn tick_trigger_kind(order: &TradeBuilderOrder) -> Option<&'static str> {
    match order.trigger_condition.as_deref() {
        Some("cross_below") => Some("sl"),
        Some("cross_above") => Some("tp"),
        _ => None,
    }
}

fn tick_trigger_eval_price(
    order: &TradeBuilderOrder,
    snapshot: &MarketDataSnapshot,
) -> Option<f64> {
    let runtime_price = build_trade_builder_runtime_price_from_market_snapshot(snapshot)?;
    if trade_builder_is_stop_loss_child(order) {
        if let Some(mode) = order.sl_trigger_price_mode.as_deref() {
            return sl_trigger_eval_price_for_mode(mode, &runtime_price);
        }
    }
    Some(trade_builder_trigger_eval_price_for_order(order, &runtime_price))
}

fn evaluate_tick_triggers_for_token(
    token_id: &str,
    snapshot: &MarketDataSnapshot,
    cache: &ArmedBuilderOrderCache,
) -> Vec<TickTrigger> {
    let Some(orders) = cache.by_token.get(token_id) else {
        return Vec::new();
    };

    orders
        .iter()
        .filter_map(|order| {
            if !trade_builder_is_child_exit_sell(order) {
                return None;
            }
            let trigger_kind = tick_trigger_kind(order)?;
            let trigger_price = order.trigger_price?;
            let tick_price = tick_trigger_eval_price(order, snapshot)?;
            let evaluation =
                evaluate_trade_builder_order_trigger(order, order.last_seen_price, tick_price);
            if !evaluation.should_trigger {
                return None;
            }
            Some(TickTrigger {
                order_id: order.id,
                user_id: order.user_id,
                token_id: token_id.to_string(),
                trigger_price,
                tick_price,
                trigger_kind,
            })
        })
        .collect()
}

fn build_tick_trigger_callback(
    tx: tokio::sync::mpsc::UnboundedSender<TickTrigger>,
) -> MarketTickCallback {
    Arc::new(move |token_id, snapshot| {
        let Ok(cache) = ARMED_BUILDER_ORDER_CACHE.try_read() else {
            return;
        };
        let triggers = evaluate_tick_triggers_for_token(token_id, snapshot, &cache);
        drop(cache);
        for trigger in triggers {
            let _ = tx.send(trigger);
        }
    })
}

async fn take_armed_builder_order_from_cache(
    token_id: &str,
    order_id: i64,
) -> Option<TradeBuilderOrder> {
    let mut cache = ARMED_BUILDER_ORDER_CACHE.write().await;
    let (order, remove_bucket) = {
        let bucket = cache.by_token.get_mut(token_id)?;
        let idx = bucket.iter().position(|order| order.id == order_id)?;
        let order = bucket.remove(idx);
        (order, bucket.is_empty())
    };
    if remove_bucket {
        cache.by_token.remove(token_id);
    }
    Some(order)
}

#[cfg(test)]
mod tick_trigger_eval_tests {
    use super::*;

    fn test_builder_order(side: &str, parent_order_id: Option<i64>) -> TradeBuilderOrder {
        TradeBuilderOrder {
            id: 1,
            trade_id: 77,
            user_id: 1,
            kind: "conditional".to_string(),
            status: "armed".to_string(),
            market_slug: "btc-updown-5m-1".to_string(),
            token_id: "tok-up".to_string(),
            outcome_label: "Up".to_string(),
            side: side.to_string(),
            execution_mode: "market".to_string(),
            trigger_condition: Some("cross_above".to_string()),
            trigger_price: Some(0.8),
            max_price: None,
            size_basis: TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC.to_string(),
            size_usdc: 5.0,
            target_qty: None,
            min_price_distance_cent: 1.0,
            expires_at: None,
            eligible_after_at: None,
            eligible_before_at: None,
            max_triggers: 1,
            triggers_fired: 0,
            active_exchange_order_id: None,
            remaining_size: None,
            remaining_qty: None,
            working_price: None,
            last_seen_price: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            parent_order_id,
            origin_flow_definition_id: None,
            origin_flow_run_id: None,
            origin_flow_node_key: None,
            tp_enabled: false,
            tp_price: None,
            sl_enabled: false,
            sl_price: None,
            filled_qty: 0.0,
            fee_rate_bps: 0,
            trigger_latched: false,
            trigger_latched_reason: None,
            trigger_latched_at: None,
            submitted_dynamic_qty: None,
            submitted_dynamic_price: None,
            guard_trigger_price: None,
            best_ask_floor_price: None,
            retry_on_trigger_guard_block: false,
            retry_on_execution_floor_guard_block: false,
            retry_on_max_price_block: false,
            sl_trigger_price_mode: None,
            reenter_on_sl_hit: false,
            reentry_max_attempts: 0,
            reentry_trigger_node_key: None,
            notify_on_fill: false,
            notify_on_trigger_guard_blocked: false,
            notify_on_execution_floor_blocked: false,
            notify_on_tp_hit: false,
            notify_on_sl_hit: false,
            notify_on_max_price_blocked: false,
        }
    }

    #[test]
    fn stop_loss_tick_eval_modes_use_selected_source_only() {
        let snapshot = MarketDataSnapshot {
            best_bid: Some(0.70),
            best_ask: Some(0.74),
            last_trade_price: Some(0.60),
            updated_at_ms: 1,
            last_source: "book".to_string(),
        };

        let mut order = test_builder_order("sell", Some(9));
        order.trigger_condition = Some("cross_below".to_string());
        order.trigger_price = Some(0.65);

        order.sl_trigger_price_mode = Some("best_bid".to_string());
        assert_eq!(tick_trigger_eval_price(&order, &snapshot), Some(0.70));

        order.sl_trigger_price_mode = Some("composite".to_string());
        assert_eq!(tick_trigger_eval_price(&order, &snapshot), Some(0.60));

        order.sl_trigger_price_mode = Some("last_trade".to_string());
        assert_eq!(tick_trigger_eval_price(&order, &snapshot), Some(0.60));
    }

    #[test]
    fn take_profit_tick_eval_uses_composite_max_price() {
        let snapshot = MarketDataSnapshot {
            best_bid: Some(0.76),
            best_ask: Some(0.79),
            last_trade_price: Some(0.78),
            updated_at_ms: 1,
            last_source: "book".to_string(),
        };

        let mut order = test_builder_order("sell", Some(9));
        order.trigger_condition = Some("cross_above".to_string());
        order.trigger_price = Some(0.77);

        assert_eq!(tick_trigger_eval_price(&order, &snapshot), Some(0.78));
    }

    #[test]
    fn evaluate_tick_triggers_emits_tp_and_sl_threshold_hits() {
        let mut cache = ArmedBuilderOrderCache::default();

        let mut tp_order = test_builder_order("sell", Some(9));
        tp_order.id = 11;
        tp_order.trigger_condition = Some("cross_above".to_string());
        tp_order.trigger_price = Some(0.80);

        let mut sl_order = test_builder_order("sell", Some(9));
        sl_order.id = 12;
        sl_order.trigger_condition = Some("cross_below".to_string());
        sl_order.trigger_price = Some(0.60);
        sl_order.sl_trigger_price_mode = Some("composite".to_string());

        cache.by_token.insert(
            "tok-up".to_string(),
            vec![tp_order.clone(), sl_order.clone()],
        );

        let tp_snapshot = MarketDataSnapshot {
            best_bid: Some(0.81),
            best_ask: Some(0.83),
            last_trade_price: Some(0.82),
            updated_at_ms: 1,
            last_source: "book".to_string(),
        };
        let sl_snapshot = MarketDataSnapshot {
            best_bid: Some(0.59),
            best_ask: Some(0.61),
            last_trade_price: Some(0.58),
            updated_at_ms: 2,
            last_source: "last_trade_price".to_string(),
        };

        let tp_triggers = evaluate_tick_triggers_for_token("tok-up", &tp_snapshot, &cache);
        assert_eq!(tp_triggers.len(), 1);
        assert_eq!(tp_triggers[0].order_id, tp_order.id);
        assert_eq!(tp_triggers[0].trigger_kind, "tp");
        assert_eq!(tp_triggers[0].tick_price, 0.82);

        let sl_triggers = evaluate_tick_triggers_for_token("tok-up", &sl_snapshot, &cache);
        assert_eq!(sl_triggers.len(), 1);
        assert_eq!(sl_triggers[0].order_id, sl_order.id);
        assert_eq!(sl_triggers[0].trigger_kind, "sl");
        assert_eq!(sl_triggers[0].tick_price, 0.58);
    }

    #[test]
    fn evaluate_tick_triggers_filters_non_child_exit_sells() {
        let mut cache = ArmedBuilderOrderCache::default();
        let mut order = test_builder_order("sell", None);
        order.trigger_condition = Some("cross_above".to_string());
        order.trigger_price = Some(0.80);
        cache.by_token.insert("tok-up".to_string(), vec![order]);

        let snapshot = MarketDataSnapshot {
            best_bid: Some(0.81),
            best_ask: Some(0.83),
            last_trade_price: Some(0.82),
            updated_at_ms: 1,
            last_source: "book".to_string(),
        };

        assert!(evaluate_tick_triggers_for_token("tok-up", &snapshot, &cache).is_empty());
    }
}
