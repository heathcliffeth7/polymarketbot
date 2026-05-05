#[derive(Debug, Clone, Copy)]
struct TradeBuilderSubmitFastLaneDecision {
    use_fast_lane: bool,
    fee_rate_bps: u64,
    reason: &'static str,
}

fn trade_builder_submit_fast_lane_decision(
    order: &TradeBuilderOrder,
    now: DateTime<Utc>,
    fresh_runtime_snapshot: Option<&TradeBuilderRuntimeSnapshot>,
    exit_fast_quote: Option<&ExitFastSubmitQuote>,
) -> TradeBuilderSubmitFastLaneDecision {
    let market_spec = trade_builder_cached_market_spec_for_order(order, now);
    let snapshot_ready =
        fresh_runtime_snapshot.is_some_and(|snapshot| snapshot.market_spec.is_some());
    let exit_quote_ready = exit_fast_quote.is_some() && trade_builder_is_child_exit_sell(order);
    let use_fast_lane = market_spec.is_some() && (snapshot_ready || exit_quote_ready);
    let fee_rate_bps = fresh_runtime_snapshot
        .and_then(|snapshot| snapshot.fee_rate_bps)
        .unwrap_or_else(|| trade_builder_fee_rate_bps_or_default(order.fee_rate_bps));
    let reason = if !use_fast_lane {
        "slow_path"
    } else if exit_quote_ready {
        "exit_fast_quote"
    } else {
        "runtime_snapshot"
    };

    TradeBuilderSubmitFastLaneDecision {
        use_fast_lane,
        fee_rate_bps,
        reason,
    }
}

#[cfg(test)]
mod submit_fast_lane_tests {
    use super::*;

    fn snapshot(captured_at: DateTime<Utc>) -> TradeBuilderRuntimeSnapshot {
        TradeBuilderRuntimeSnapshot {
            captured_at,
            source: "test".to_string(),
            current_price: Some(0.52),
            best_bid: Some(0.51),
            best_ask: Some(0.53),
            last_trade_price: Some(0.52),
            trigger_reference_price: Some(0.52),
            guard_reference_price: Some(0.52),
            fee_rate_bps: Some(321),
            market_spec: Some(TradeBuilderRuntimeSnapshotMarketSpec {
                neg_risk: true,
                order_price_min_tick_size: Some(0.01),
                order_min_size: Some(5.0),
            }),
        }
    }

    fn test_order(runtime_snapshot_json: Option<Value>) -> TradeBuilderOrder {
        TradeBuilderOrder {
            id: 1,
            trade_id: 77,
            user_id: 1,
            kind: "immediate".to_string(),
            status: "pending".to_string(),
            market_slug: "btc-updown-5m-1".to_string(),
            token_id: "tok-up".to_string(),
            outcome_label: "Up".to_string(),
            side: "buy".to_string(),
            execution_mode: "market".to_string(),
            trigger_condition: None,
            trigger_price: None,
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
            parent_order_id: None,
            origin_flow_definition_id: None,
            origin_flow_run_id: None,
            origin_flow_node_key: None,
            pair_session_id: None,
            pair_leg_role: None,
            tp_enabled: false,
            tp_price: None,
            tp_rules_json: Vec::new(),
            sl_enabled: false,
            sl_price: None,
            sl_rules_json: Vec::new(),
            time_exit_rules_json: Vec::new(),
            filled_qty: 0.0,
            fee_rate_bps: 0,
            trigger_latched: false,
            trigger_latched_reason: None,
            trigger_latched_at: None,
            submitted_dynamic_qty: None,
            submitted_dynamic_price: None,
            runtime_snapshot_json,
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
            ptb_current_price_source: "chainlink".to_string(),
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
    fn submit_fast_lane_uses_fresh_snapshot_fee_and_spec() {
        let now = Utc::now();
        let order = test_order(serde_json::to_value(snapshot(now)).ok());

        let fresh = trade_builder_runtime_snapshot_from_order(&order)
            .filter(|snapshot| trade_builder_runtime_snapshot_is_fresh(snapshot, now));
        let decision = trade_builder_submit_fast_lane_decision(&order, now, fresh.as_ref(), None);

        assert!(decision.use_fast_lane);
        assert_eq!(decision.fee_rate_bps, 321);
        assert_eq!(decision.reason, "runtime_snapshot");
    }

    #[test]
    fn submit_fast_lane_requires_fresh_market_spec() {
        let now = Utc::now();
        let mut stale = snapshot(now - ChronoDuration::milliseconds(700));
        stale.market_spec = None;
        let order = test_order(serde_json::to_value(stale).ok());

        let fresh = trade_builder_runtime_snapshot_from_order(&order)
            .filter(|snapshot| trade_builder_runtime_snapshot_is_fresh(snapshot, now));
        let decision = trade_builder_submit_fast_lane_decision(&order, now, fresh.as_ref(), None);

        assert!(!decision.use_fast_lane);
        assert_eq!(decision.reason, "slow_path");
    }
}
