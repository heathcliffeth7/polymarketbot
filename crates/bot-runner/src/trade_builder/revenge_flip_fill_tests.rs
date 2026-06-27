#[cfg(test)]
mod revenge_flip_fill_tests {
    use super::*;

    fn wake_order(side: &str, trade_id: i64) -> TradeBuilderOrder {
        let now = Utc::now();
        TradeBuilderOrder {
            id: 42,
            trade_id,
            user_id: 7,
            kind: "manual".to_string(),
            status: "filled".to_string(),
            market_slug: "btc-updown-5m-1780478400".to_string(),
            token_id: "token-up".to_string(),
            outcome_label: "Up".to_string(),
            side: side.to_string(),
            execution_mode: "immediate".to_string(),
            trigger_condition: None,
            trigger_price: None,
            max_price: None,
            size_basis: TRADE_BUILDER_SIZE_BASIS_SHARES.to_string(),
            size_usdc: 0.0,
            target_qty: Some(1.0),
            min_price_distance_cent: 0.0,
            expires_at: None,
            eligible_after_at: None,
            eligible_before_at: None,
            max_triggers: 1,
            triggers_fired: 1,
            active_exchange_order_id: None,
            remaining_size: None,
            remaining_qty: None,
            working_price: None,
            last_seen_price: None,
            last_error: None,
            created_at: now,
            updated_at: now,
            parent_order_id: Some(41),
            origin_flow_definition_id: Some(5),
            origin_flow_run_id: Some(9),
            origin_flow_node_key: Some("action_revenge_flip".to_string()),
            pair_session_id: None,
            pair_leg_role: None,
            tp_enabled: false,
            tp_price: None,
            tp_rules_json: Vec::new(),
            sl_enabled: false,
            sl_price: None,
            sl_rules_json: Vec::new(),
            time_exit_rules_json: Vec::new(),
            filled_qty: 1.0,
            fee_rate_bps: 1000,
            trigger_latched: false,
            trigger_latched_reason: None,
            trigger_latched_at: None,
            submitted_dynamic_qty: None,
            submitted_dynamic_price: None,
            runtime_snapshot_json: None,
            fresh_submit_lease_until: None,
            guard_trigger_price: None,
            best_ask_floor_price: None,
            retry_on_trigger_guard_block: false,
            retry_on_execution_floor_guard_block: false,
            retry_on_max_price_block: false,
            ptb_stop_loss_gap_usd: None,
            ptb_reference_price: None,
            ptb_stop_loss_rules_json: Vec::new(),
            ptb_stop_loss_time_decay_mode: None,
            ptb_current_price_source: "best_bid".to_string(),
            staged_sl_retry_only_dust: false,
            staged_sl_retry_dust_metric: None,
            staged_sl_retry_dust_value: None,
            staged_sl_reentry_use_sold_notional: false,
            staged_sl_reentry_only_after_all_stages: false,
            sl_trigger_price_mode: None,
            reenter_on_sl_hit: false,
            reentry_max_attempts: 0,
            reentry_trigger_node_key: None,
            notify_on_order_submitted: false,
            notify_on_fill: false,
            notify_on_order_not_filled: false,
            notify_on_trigger_guard_blocked: false,
            notify_on_execution_floor_blocked: false,
            notify_on_tp_hit: false,
            notify_on_sl_hit: false,
            notify_on_max_price_blocked: false,
            last_guard_notification_reason: None,
            exit_ladder_kind: None,
            exit_ladder_index: None,
            exit_ladder_size_pct: None,
        }
    }

    #[test]
    fn revenge_flip_stop_loss_sell_marker_is_wake_eligible() {
        let order = wake_order("sell", 101);

        assert!(revenge_flip_stop_loss_wake_eligible(
            &order,
            "stop_loss_sell",
            true,
            Some("action_revenge_flip")
        ));
    }

    #[test]
    fn revenge_flip_initial_and_flip_buy_fills_are_not_wake_eligible() {
        let order = wake_order("buy", 101);

        assert!(!revenge_flip_stop_loss_wake_eligible(
            &order,
            "initial_buy",
            true,
            Some("action_revenge_flip")
        ));
        assert!(!revenge_flip_stop_loss_wake_eligible(
            &order,
            "flip_buy",
            true,
            Some("action_revenge_flip")
        ));
    }

    #[test]
    fn revenge_flip_non_stop_loss_sell_is_not_wake_eligible() {
        let order = wake_order("sell", 101);

        assert!(!revenge_flip_stop_loss_wake_eligible(
            &order,
            "take_profit_sell",
            true,
            Some("action_revenge_flip")
        ));
    }

    #[test]
    fn revenge_flip_stop_loss_wake_idempotency_key_is_stable_for_same_sell_order() {
        let first = revenge_flip_stop_loss_wake_idempotency_key(9, "action_revenge_flip", 42);
        let second = revenge_flip_stop_loss_wake_idempotency_key(9, "action_revenge_flip", 42);

        assert_eq!(first, second);
        assert_eq!(
            first,
            "revenge_flip_stop_loss_wake:9:action_revenge_flip:42"
        );
    }

    #[test]
    fn revenge_flip_stop_loss_wake_input_carries_source_context() {
        let order = wake_order("sell", 123);
        let payload = revenge_flip_stop_loss_wake_input(&order);

        assert_eq!(
            payload.get("marketSlug").and_then(Value::as_str),
            Some("btc-updown-5m-1780478400")
        );
        assert_eq!(
            payload.get("wakeReason").and_then(Value::as_str),
            Some("revenge_flip_stop_loss_fill")
        );
        assert_eq!(
            payload
                .get("sourceBuilderOrderId")
                .and_then(Value::as_i64),
            Some(42)
        );
        assert_eq!(payload.get("sourceTradeId").and_then(Value::as_i64), Some(123));
    }
}
