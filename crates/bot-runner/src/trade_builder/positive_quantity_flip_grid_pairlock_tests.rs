#[cfg(test)]
mod positive_quantity_flip_grid_pairlock_tests {
    use super::*;
    use super::positive_quantity_flip_grid_test_helpers::*;

    #[test]
    fn pairlock_rejects_blind_55_55_pair() {
        let cfg = pairlock_config();
        let lots = vec![lot(1, "up", 1.0, 0.55)];
        let state = TradeBuilderPositiveQuantityFlipGridState {
            up_qty: 1.0,
            total_buy_cost: 0.55,
            net_cost: 0.55,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let opportunity = positive_flip_pairlock_find_buy_opportunity(
            &lots,
            &[quote_with_side_ask("down", 0.55)],
            &state,
            &cfg,
        );
        assert!(opportunity.is_none());
    }

    #[test]
    fn pairlock_finds_down_56_plus_up_33_compression_buy() {
        let cfg = pairlock_config();
        let lots = vec![lot(1, "down", 1.84, 0.56)];
        let state = TradeBuilderPositiveQuantityFlipGridState {
            down_qty: 1.84,
            total_buy_cost: 1.03,
            net_cost: 1.03,
            buy_count: 1,
            last_buy_grid_side: Some("down".to_string()),
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let opportunity = positive_flip_pairlock_find_buy_opportunity(
            &lots,
            &[quote_with_side_ask("up", 0.33)],
            &state,
            &cfg,
        )
        .expect("pairlock buy at 33c");
        assert_eq!(opportunity.candidate.quote.grid_side, "up");
        assert!((opportunity.pair_cost - 0.89).abs() < 0.000001);
        assert!((opportunity.locked_profit_per_share - 0.11).abs() < 0.000001);
    }

    #[test]
    fn normal_buy_still_blocked_below_entry_band_at_33() {
        let cfg = pairlock_config();
        let state = TradeBuilderPositiveQuantityFlipGridState {
            last_buy_grid_side: Some("down".to_string()),
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let report = positive_quantity_flip_grid_entry_guard_report(
            &cfg,
            &state,
            &quote_with_side_ask("up", 0.33),
        )
        .expect("guard report");
        assert_eq!(report["guard_reason"], "below_entry_band");
    }

    #[test]
    fn fill_parent_builder_order_id_defaults_to_order_id_for_standalone_buy() {
        assert_eq!(
            positive_quantity_flip_grid_resolve_fill_parent_builder_order_id(
                18117,
                None,
                "buy",
                None,
            ),
            Some(18117)
        );
        assert_eq!(
            positive_quantity_flip_grid_resolve_fill_parent_builder_order_id(
                18118,
                None,
                "sell",
                None,
            ),
            None
        );
        assert_eq!(
            positive_quantity_flip_grid_resolve_fill_parent_builder_order_id(
                18119,
                Some(99),
                "buy",
                None,
            ),
            Some(99)
        );
    }

    #[test]
    fn compression_buy_node_disables_execution_floor_guard() {
        let mut cfg = pairlock_config();
        cfg.depth_guard_enabled = false;
        let node = positive_flip_pairlock_compression_node(json!({}));
        let candidate = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &TradeBuilderPositiveQuantityFlipGridState::default(),
            quote_with_side_ask("up", 0.53),
            false,
            None,
        )
        .expect("candidate");
        let buy_node = positive_quantity_flip_grid_buy_node(
            &node,
            &cfg,
            "btc-updown-5m-1779975000",
            &candidate,
            1,
            Some("pairlock_compression_buy"),
            Some("FOK"),
        );
        assert_eq!(
            buy_node.config["executionFloorGuardEnabled"],
            json!(false)
        );
        let normal_node = positive_quantity_flip_grid_buy_node(
            &node,
            &cfg,
            "btc-updown-5m-1779975000",
            &candidate,
            2,
            None,
            None,
        );
        assert_eq!(
            normal_node.config["executionFloorGuardEnabled"],
            json!(true)
        );
    }

    #[test]
    fn pairlock_finds_down_55_plus_up_40_compression_buy() {
        let cfg = pairlock_config();
        let lots = vec![lot(1, "down", 2.0, 0.55)];
        let state = TradeBuilderPositiveQuantityFlipGridState {
            down_qty: 2.0,
            total_buy_cost: 1.10,
            net_cost: 1.10,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let opportunity = positive_flip_pairlock_find_buy_opportunity(
            &lots,
            &[quote_with_side_ask("up", 0.40)],
            &state,
            &cfg,
        )
        .expect("pairlock buy");
        assert_eq!(opportunity.candidate.quote.grid_side, "up");
        assert!((opportunity.pair_cost - 0.95).abs() < 0.000001);
        assert!((opportunity.locked_profit_per_share - 0.05).abs() < 0.000001);
    }

    #[test]
    fn pairlock_fee_buffer_requires_lower_ask() {
        let mut cfg = pairlock_config();
        cfg.fee_buffer = 0.01;
        cfg.max_pair_cost = 0.94;
        let lots = vec![lot(1, "down", 2.0, 0.55)];
        let state = TradeBuilderPositiveQuantityFlipGridState {
            down_qty: 2.0,
            total_buy_cost: 1.10,
            net_cost: 1.10,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        assert!(positive_flip_pairlock_find_buy_opportunity(
            &lots,
            &[quote_with_side_ask("up", 0.40)],
            &state,
            &cfg,
        )
        .is_none());
        assert!(positive_flip_pairlock_find_buy_opportunity(
            &lots,
            &[quote_with_side_ask("up", 0.39)],
            &state,
            &cfg,
        )
        .is_some());
    }

    #[test]
    fn pairlock_merge_plan_uses_min_quantity() {
        let cfg = pairlock_config();
        let lots = vec![lot(1, "up", 3.0, 0.40), lot(2, "down", 1.25, 0.55)];
        let plan = positive_flip_pairlock_find_merge_plan(&lots, &cfg).expect("merge plan");
        assert_eq!(plan.quantity, 1.25);
        assert_eq!(positive_flip_pairlock_merge_return(plan.quantity), 1.25);
    }

    #[test]
    fn pairlock_buys_not_blocked_before_profit_lock_with_inventory() {
        let cfg = pairlock_config();
        let state = TradeBuilderPositiveQuantityFlipGridState {
            up_qty: 2.0,
            down_qty: 2.0,
            total_merge_return: 0.0,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        assert!(!positive_flip_pairlock_buys_blocked_after_profit_lock(
            &cfg, &state
        ));
    }

    #[test]
    fn pairlock_buys_blocked_after_profit_lock_when_flag_enabled() {
        let cfg = pairlock_config();
        let state = TradeBuilderPositiveQuantityFlipGridState {
            up_qty: 1.0,
            down_qty: 0.5,
            total_merge_return: 1.25,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        assert!(positive_flip_pairlock_buys_blocked_after_profit_lock(
            &cfg, &state
        ));
    }

    #[test]
    fn pairlock_buys_continue_after_profit_lock_when_flag_disabled() {
        let mut cfg = pairlock_config();
        cfg.stop_buys_after_pairlock_merge = false;
        let state = TradeBuilderPositiveQuantityFlipGridState {
            up_qty: 1.0,
            down_qty: 0.5,
            total_merge_return: 1.25,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        assert!(!positive_flip_pairlock_buys_blocked_after_profit_lock(
            &cfg, &state
        ));
    }

    #[test]
    fn pairlock_merge_plan_unaffected_by_profit_lock_buy_guard() {
        let cfg = pairlock_config();
        let lots = vec![lot(1, "up", 3.0, 0.40), lot(2, "down", 1.25, 0.55)];
        let state = TradeBuilderPositiveQuantityFlipGridState {
            total_merge_return: 2.0,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        assert!(positive_flip_pairlock_buys_blocked_after_profit_lock(
            &cfg, &state
        ));
        assert!(positive_flip_pairlock_find_merge_plan(&lots, &cfg).is_some());
    }

    #[test]
    fn in_flight_buy_details_include_active_order_metadata() {
        let active_buys = vec![
            TradeBuilderPositiveQuantityFlipGridActiveBuy {
                order_id: 42,
                status: "open".to_string(),
                grid_side: "down".to_string(),
                outcome_label: "Down".to_string(),
                size_usdc: 1.05,
                target_qty: 1.95,
                created_at: Utc::now(),
            },
            TradeBuilderPositiveQuantityFlipGridActiveBuy {
                order_id: 43,
                status: "partially_filled".to_string(),
                grid_side: "down".to_string(),
                outcome_label: "Down".to_string(),
                size_usdc: 0.55,
                target_qty: 1.0,
                created_at: Utc::now(),
            },
        ];
        let state = TradeBuilderPositiveQuantityFlipGridState {
            total_buy_cost: 1.0,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let details =
            positive_quantity_flip_grid_in_flight_buy_details(&active_buys, &state).expect("details");
        assert_eq!(details["active_order_id"], 42);
        assert_eq!(details["active_order_count"], 2);
        assert_eq!(details["active_grid_side"], "down");
        assert_eq!(details["active_buy_notional_usdc"], 1.6);
        assert_eq!(details["projected_total_buy_cost_usdc"], 2.6);
    }

    #[test]
    fn buy_execution_coalesced_skip_is_market_scoped() {
        use bot_infra::db::positive_quantity_flip_grid_buy_execution_lock_keys;

        let node = TradeFlowNode {
            key: "action_positive_grid_buy".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({}),
        };
        let skip = positive_quantity_flip_grid_output_skipped(
            &node,
            "btc-updown-5m-1779973200",
            "buy_execution_coalesced",
            json!({
                "market_slug": "btc-updown-5m-1779973200",
                "contention": true,
            }),
        );
        assert_eq!(skip.output["reason"], "buy_execution_coalesced");
        assert_eq!(skip.output["details"]["contention"], true);

        let lock_a = positive_quantity_flip_grid_buy_execution_lock_keys(
            1,
            4327,
            "action_positive_grid_buy",
            "btc-updown-5m-1779973200",
        );
        let lock_b = positive_quantity_flip_grid_buy_execution_lock_keys(
            1,
            4327,
            "action_positive_grid_buy",
            "btc-updown-5m-1779971700",
        );
        assert_ne!(lock_a, lock_b);
    }
}
