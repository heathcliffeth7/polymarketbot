mod pair_lock_biased_hedge_tests {
    use super::*;

    fn biased_test_quote(ask: f64) -> PairLockResolvedQuote {
        PairLockResolvedQuote {
            best_bid: Some((ask - 0.01).max(0.01)),
            best_ask: Some(ask),
            last_trade_price: Some(ask),
            current_price: ask,
            quote_source_kind: "test",
            quote_ws_state: "test",
            quote_event_ts: None,
            quote_snapshot_age_ms: None,
            quote_source_detail: "test".to_string(),
            quote_book_missing_fields: Vec::new(),
            quote_snapshot_used: json!({}),
        }
    }

    fn biased_test_candidate(label: &str, ask: f64, q_final: f64, edge: f64) -> PairLockEdgeCandidate {
        PairLockEdgeCandidate {
            token_id: format!("tok-{}", label.to_ascii_lowercase()),
            outcome_label: label.to_string(),
            quote: biased_test_quote(ask),
            ask: Some(ask),
            fee: Some(0.0),
            cost: Some(q_final - edge),
            q: Some(q_final),
            edge: Some(edge),
            guard_decision: "passed",
            guard_reason: "passed".to_string(),
            diagnostics: json!({
                "guard": {
                    "price_to_beat_guard": {
                        "iv_mismatch_edge": {
                            "q_final": q_final,
                            "edge": edge,
                            "gap_strength": 1.2,
                            "binance_same_direction": true,
                            "depth_guard_result": "pass"
                        }
                    }
                }
            }),
        }
    }

    fn biased_test_config() -> ActionPlaceOrderBiasedHedgeConfig {
        ActionPlaceOrderBiasedHedgeConfig {
            primary_budget_usdc: 2.0,
            hedge_budget_usdc: 0.5,
            min_dominant_share: 0.75,
            max_hedge_spend_ratio: 0.25,
            primary_min_edge: 0.08,
            primary_min_final_q: 0.72,
            hedge_max_price: 0.25,
            hedge_min_price: 0.03,
            hedge_only_if_primary_filled: true,
            disable_new_primary_after_sec: 180,
            disable_any_buy_after_sec: 240,
            max_side_switch_count: 0,
            high_price: 0.70,
            high_price_min_final_q: 0.82,
            high_price_min_edge: 0.10,
            max_paired_effective_cost: Some(0.95),
            stop: ActionPlaceOrderBiasedHedgeStopConfig {
                bias_invalidation_enabled: true,
                min_q_final_to_hold: 0.55,
                min_edge_to_hold: 0.0,
                exit_pct_on_invalidation: 100.0,
                ptb_stop_loss_enabled: true,
                ptb_stop_loss_gap_usd: Some(-3.0),
                ptb_stop_loss_time_decay_mode: Some("tighten".to_string()),
                time_exit_rules: vec![ActionPlaceOrderBiasedHedgeTimeExitRule {
                    elapsed_sec: 90,
                    remaining_pct: 60.0,
                }],
            },
        }
    }

    fn biased_test_node(config: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "biased".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    fn biased_test_trigger_graph(cycle_window_start_sec: i64) -> TradeFlowGraphRuntime {
        TradeFlowGraphRuntime {
            context: json!({}),
            nodes: vec![TradeFlowNode {
                key: "trigger_biased".to_string(),
                node_type: "trigger.market_price".to_string(),
                config: json!({
                    "cycleWindowStartSec": cycle_window_start_sec,
                    "cycleWindowEndSec": 180,
                }),
            }],
            edges: Vec::new(),
        }
    }

    #[test]
    fn biased_hedge_injects_early_iv_time_rule_when_missing() {
        let config = biased_test_config();
        let mut node = biased_test_node(json!({
            "biasedHedge": { "maxPriceCent": 75 },
        }));

        apply_biased_hedge_early_iv_time_rule(
            &mut node,
            &config,
            "btc-updown-5m-1777315200",
            None,
            None,
        );

        let rules = node
            .config
            .get("priceToBeatIvTimeRules")
            .and_then(Value::as_array)
            .expect("default rule should be injected");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].get("startRemainingSec").and_then(Value::as_i64), Some(270));
        assert_eq!(rules[0].get("endRemainingSec").and_then(Value::as_i64), Some(120));
        assert_eq!(rules[0].get("minEdge").and_then(Value::as_f64), Some(0.08));
        assert_eq!(rules[0].get("minGapStrength").and_then(Value::as_i64), Some(0));
    }

    #[test]
    fn biased_hedge_derives_iv_time_rule_from_trigger_cycle_start() {
        let config = biased_test_config();
        let graph = biased_test_trigger_graph(15);
        let mut node = biased_test_node(json!({
            "biasedHedge": { "maxPriceCent": 75 },
        }));

        apply_biased_hedge_early_iv_time_rule(
            &mut node,
            &config,
            "btc-updown-5m-1777315200",
            Some(&graph),
            Some("trigger_biased"),
        );

        let rules = node
            .config
            .get("priceToBeatIvTimeRules")
            .and_then(Value::as_array)
            .expect("default rule should be injected");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].get("startRemainingSec").and_then(Value::as_i64), Some(285));
        assert_eq!(rules[0].get("endRemainingSec").and_then(Value::as_i64), Some(120));
        assert_eq!(rules[0].get("minEdge").and_then(Value::as_f64), Some(0.08));
    }

    #[test]
    fn biased_hedge_preserves_explicit_iv_time_rules() {
        let config = biased_test_config();
        let explicit = json!({
            "startRemainingSec": 180,
            "endRemainingSec": 90,
            "maxPriceCent": 70,
            "minEdge": 0.09,
            "minGapStrength": 0.4,
        });
        let mut node = biased_test_node(json!({
            "priceToBeatIvTimeRules": [explicit.clone()],
        }));

        apply_biased_hedge_early_iv_time_rule(
            &mut node,
            &config,
            "btc-updown-5m-1777315200",
            None,
            None,
        );

        assert_eq!(
            node.config.get("priceToBeatIvTimeRules"),
            Some(&json!([explicit]))
        );
    }

    #[test]
    fn biased_hedge_high_price_requires_stronger_conviction() {
        let config = biased_test_config();
        let weak = biased_test_candidate("Up", 0.72, 0.80, 0.09);
        let strong = biased_test_candidate("Down", 0.72, 0.83, 0.11);

        assert_eq!(
            biased_hedge_primary_block_reason(&weak, &config),
            Some("high_price_confidence_fail")
        );
        assert_eq!(biased_hedge_primary_block_reason(&strong, &config), None);
    }

    #[test]
    fn biased_hedge_selects_best_primary_edge() {
        let config = biased_test_config();
        let up = biased_test_candidate("Up", 0.55, 0.75, 0.09);
        let down = biased_test_candidate("Down", 0.50, 0.76, 0.12);

        assert_eq!(
            biased_hedge_select_primary(&up, &down, &config)
                .unwrap()
                .outcome_label,
            "Down"
        );
    }

    #[test]
    fn biased_hedge_partial_primary_fill_clamps_hedge_size() {
        let config = biased_test_config();

        let hedge_notional = biased_hedge_clamped_hedge_notional(1.0, &config);

        assert!((hedge_notional - (1.0 / 3.0)).abs() < 0.000001);
    }

    #[test]
    fn biased_hedge_live_bias_monitor_invalidates_weak_hold_signal() {
        let config = biased_test_config();

        assert!(biased_hedge_bias_invalidated(
            Some(0.54),
            Some(0.01),
            true,
            true,
            &config
        ));
        assert!(biased_hedge_bias_invalidated(
            Some(0.60),
            Some(0.01),
            false,
            true,
            &config
        ));
        assert!(!biased_hedge_bias_invalidated(
            Some(0.60),
            Some(0.01),
            true,
            true,
            &config
        ));
    }

    #[test]
    fn biased_hedge_dominance_uses_filled_notional() {
        let mut session = TradeBuilderPairSession {
            id: 1,
            user_id: 1,
            flow_definition_id: Some(1),
            flow_run_id: Some(1),
            flow_node_key: Some("node".to_string()),
            market_slug: "btc-updown-15m-1".to_string(),
            status: TRADE_BUILDER_PAIR_STATUS_WORKING.to_string(),
            pair_target_total_cent: 95.0,
            min_net_profit_usdc: 0.0,
            profit_safety_buffer_usdc: 0.0,
            orphan_grace_ms: 1500,
            ignore_stop_loss_after_locked: false,
            notify_on_pair_locked: false,
            notify_on_pair_unwind: false,
            notify_on_pair_no_edge: false,
            primary_order_id: Some(10),
            counter_order_id: Some(11),
            lead_order_id: Some(10),
            primary_fill_qty: Some(2.0),
            primary_fill_fee_qty: Some(0.0),
            primary_net_qty: Some(2.0),
            primary_avg_fill_price: Some(0.50),
            counter_fill_qty: None,
            counter_fill_fee_qty: None,
            counter_net_qty: None,
            counter_avg_fill_price: None,
            lead_filled_at: Some(Utc::now()),
            locked_qty: None,
            projected_net_profit_usdc: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(biased_hedge_dominant_share(&session), Some(1.0));
        session.counter_fill_qty = Some(2.0);
        session.counter_avg_fill_price = Some(0.125);
        assert!((biased_hedge_dominant_share(&session).unwrap() - 0.8).abs() < 0.000001);
    }
}
