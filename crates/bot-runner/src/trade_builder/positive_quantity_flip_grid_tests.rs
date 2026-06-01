#[cfg(test)]
mod positive_quantity_flip_grid_tests {
    use super::*;
    use super::positive_quantity_flip_grid_test_helpers::*;

    #[test]
    fn sizing_second_53c_flip_requires_about_121_usdc() {
        let required = positive_quantity_flip_grid_required_buy_usdc(1.0, 0.03, 0.98, 0.0, 0.53)
            .expect("required");
        assert!((required - 1.213).abs() < 0.01);
        assert_eq!(positive_quantity_flip_grid_round_up_cent(required), 1.22);
    }

    #[test]
    fn sizing_returns_zero_when_current_side_already_covers_target() {
        let required = positive_quantity_flip_grid_required_buy_usdc(1.0, 0.03, 0.98, 2.0, 0.53)
            .expect("required");
        assert_eq!(required, 0.0);
    }

    #[test]
    fn inventory_balance_sizing_caps_buy_to_available_budget() {
        let mut cfg = test_config();
        cfg.quantity_sizing_mode = PositiveQuantityFlipGridQuantitySizingMode::InventoryBalance;
        cfg.max_single_buy_usdc = Some(5.0);
        cfg.max_total_spent_per_market_usdc = Some(25.0);
        let state = TradeBuilderPositiveQuantityFlipGridState {
            up_qty: 7.85,
            down_qty: 17.64,
            net_cost: 17.21,
            total_buy_cost: 17.21,
            buy_count: 5,
            last_buy_grid_side: Some("down".to_string()),
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let candidate = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_side_ask("up", 0.56),
            false,
            Some(&order_book(&[(0.56, 100.0)])),
        )
        .expect("inventory candidate");

        assert_eq!(candidate.target_qty, 8.92);
        assert!(candidate.actual_buy_usdc <= 5.0);
        assert!(candidate.projected_pnl_at_exit < cfg.min_positive_profit_usdc);
    }

    #[test]
    fn inventory_balance_sizing_blocks_side_that_already_covers_inventory() {
        let mut cfg = test_config();
        cfg.quantity_sizing_mode = PositiveQuantityFlipGridQuantitySizingMode::InventoryBalance;
        let state = TradeBuilderPositiveQuantityFlipGridState {
            up_qty: 18.0,
            down_qty: 10.0,
            buy_count: 2,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let blocked = positive_quantity_flip_grid_evaluate_buy_candidate(
            &cfg,
            &state,
            quote_with_side_ask("up", 0.53),
            false,
            Some(&order_book(&[(0.53, 100.0)])),
        );

        assert!(blocked.candidate.is_none());
        assert_eq!(
            blocked.report["guard_reason"],
            "inventory_side_already_balanced"
        );
    }

    #[test]
    fn quantity_sizing_mode_config_parses_inventory_balance() {
        let cfg = resolve_positive_quantity_flip_grid_config(&positive_grid_node(json!({
            "quantitySizingMode": "inventory_balance",
            "inventoryBalanceLeadQty": 1.5,
        })))
        .expect("valid inventory balance config");

        assert_eq!(
            cfg.quantity_sizing_mode,
            PositiveQuantityFlipGridQuantitySizingMode::InventoryBalance
        );
        assert_eq!(cfg.inventory_balance_lead_qty, 1.5);
    }

    fn fixed_usdc_pairlock_config() -> PositiveQuantityFlipGridConfig {
        let mut cfg = pairlock_config();
        cfg.quantity_sizing_mode = PositiveQuantityFlipGridQuantitySizingMode::FixedUsdc;
        cfg.base_buy_usdc = 2.0;
        cfg.min_marketable_buy_usdc = 1.05;
        cfg.sizing_price_buffer_cent = 1.0;
        cfg.hard_max_price_cent = 58.0;
        cfg.worst_price_cent = 58.0;
        cfg.max_single_buy_usdc = None;
        cfg.max_total_spent_per_market_usdc = None;
        cfg
    }

    #[test]
    fn quantity_sizing_mode_config_parses_fixed_usdc_for_pairlock() {
        let cfg = resolve_positive_quantity_flip_grid_config(
            &positive_flip_pairlock_compression_node(json!({
                "quantitySizingMode": "fixed_usdc",
            })),
        )
        .expect("valid fixed usdc config");

        assert_eq!(
            cfg.quantity_sizing_mode,
            PositiveQuantityFlipGridQuantitySizingMode::FixedUsdc
        );
    }

    #[test]
    fn fixed_usdc_rejects_non_pairlock_mode() {
        let err = resolve_positive_quantity_flip_grid_config(&positive_grid_node(json!({
            "quantitySizingMode": "fixed_usdc",
        })))
        .expect_err("fixed usdc requires pairlock mode");

        assert!(err
            .to_string()
            .contains("fixed_usdc requires action.place_order mode=positive_flip_pairlock_compression_v1"));
    }

    #[test]
    fn fixed_usdc_flip_constant_qty_at_55c() {
        let cfg = fixed_usdc_pairlock_config();
        let first = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &TradeBuilderPositiveQuantityFlipGridState::default(),
            quote_with_side_ask("up", 0.55),
            false,
            Some(&order_book(&[(0.55, 100.0)])),
        )
        .expect("first buy");
        let state = TradeBuilderPositiveQuantityFlipGridState {
            up_qty: first.target_qty,
            total_buy_cost: first.actual_buy_usdc,
            net_cost: first.actual_buy_usdc,
            buy_count: 1,
            last_buy_grid_side: Some("up".to_string()),
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let flip = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_side_ask("down", 0.55),
            false,
            Some(&order_book(&[(0.55, 100.0)])),
        )
        .expect("flip buy");

        assert_eq!(flip.target_qty, first.target_qty);
        assert!((flip.actual_buy_usdc - first.actual_buy_usdc).abs() < 0.02);
        assert!(flip.actual_buy_usdc <= 2.01);
    }

    #[test]
    fn fixed_usdc_flip_qty_stays_below_profit_target_flip() {
        let mut profit_cfg = pairlock_config();
        profit_cfg.max_single_buy_usdc = None;
        profit_cfg.max_total_spent_per_market_usdc = None;
        let fixed_cfg = fixed_usdc_pairlock_config();
        let state = TradeBuilderPositiveQuantityFlipGridState {
            up_qty: 3.58,
            total_buy_cost: 1.97,
            net_cost: 1.97,
            buy_count: 1,
            last_buy_grid_side: Some("up".to_string()),
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let quote = quote_with_side_ask("down", 0.55);
        let book = order_book(&[(0.55, 100.0)]);

        let profit_flip = positive_quantity_flip_grid_buy_candidate(
            &profit_cfg,
            &state,
            quote.clone(),
            false,
            Some(&book),
        )
        .expect("profit target flip");
        let fixed_flip = positive_quantity_flip_grid_buy_candidate(
            &fixed_cfg,
            &state,
            quote,
            false,
            Some(&book),
        )
        .expect("fixed usdc flip");

        assert!(fixed_flip.target_qty + 0.000001 < profit_flip.target_qty);
        assert!(fixed_flip.actual_buy_usdc <= 2.01);
    }

    #[test]
    fn fixed_usdc_respects_configured_base_buy_usdc() {
        let mut cfg = fixed_usdc_pairlock_config();
        cfg.base_buy_usdc = 10.0;
        let candidate = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &TradeBuilderPositiveQuantityFlipGridState::default(),
            quote_with_side_ask("up", 0.55),
            false,
            Some(&order_book(&[(0.55, 100.0)])),
        )
        .expect("ten usdc buy");

        assert!(candidate.actual_buy_usdc >= 9.99);
        assert!(candidate.actual_buy_usdc <= 10.01);
    }

    #[test]
    fn fixed_usdc_respects_min_marketable() {
        let mut cfg = fixed_usdc_pairlock_config();
        cfg.base_buy_usdc = 0.8;
        let candidate = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &TradeBuilderPositiveQuantityFlipGridState::default(),
            quote_with_side_ask("up", 0.55),
            false,
            Some(&order_book(&[(0.55, 100.0)])),
        )
        .expect("min marketable buy");

        assert!(candidate.actual_buy_usdc >= 1.05);
        assert!(candidate.target_qty * 0.55 >= 1.05);
    }

    #[test]
    fn positive_flip_pairlock_compression_mode_uses_small_bankroll_defaults() {
        let node = positive_flip_pairlock_compression_node(json!({}));
        let cfg = resolve_positive_quantity_flip_grid_config(&node).expect("valid config");

        assert!(action_place_order_uses_positive_quantity_flip_grid(&node));
        assert_eq!(
            action_place_order_positive_grid_mode(&node),
            Some(ACTION_PLACE_ORDER_MODE_POSITIVE_FLIP_PAIRLOCK_COMPRESSION_V1)
        );
        assert_eq!(cfg.base_buy_usdc, 2.0);
        assert_eq!(cfg.entry_band_min_cent, 52.0);
        assert_eq!(cfg.entry_band_max_cent, 58.0);
        assert_eq!(cfg.min_positive_profit_usdc, 0.05);
        assert_eq!(cfg.max_total_spent_per_market_usdc, None);
        assert_eq!(cfg.max_open_grid_buys_per_market, 10);
        assert_eq!(cfg.sell_bid_min, 0.59);
        assert!(cfg.pairlock_compression_enabled);
        assert!(cfg.stop_buys_after_pairlock_merge);
    }

    #[test]
    fn partial_recovery_config_parses_optional_cap() {
        let cfg = resolve_positive_quantity_flip_grid_config(&positive_grid_node(json!({
            "partialRecoveryEnabled": true,
            "partialRecoveryMinLossReductionUsdc": 0.1,
            "partialRecoveryBalanceReserveUsdc": 1,
            "partialRecoveryMaxBuyUsdc": 13,
            "partialRecoveryIgnoreMarketBudget": true,
        })))
        .expect("valid partial recovery config");

        assert!(cfg.partial_recovery_enabled);
        assert_eq!(cfg.partial_recovery_min_loss_reduction_usdc, 0.1);
        assert_eq!(cfg.partial_recovery_balance_reserve_usdc, 1.0);
        assert_eq!(cfg.partial_recovery_max_buy_usdc, Some(13.0));
        assert!(cfg.partial_recovery_ignore_market_budget);
    }

    #[test]
    fn partial_recovery_builds_candidate_when_full_recovery_exceeds_cap() {
        let mut cfg = test_config();
        cfg.partial_recovery_enabled = true;
        cfg.max_single_buy_usdc = Some(1.2);
        cfg.max_total_spent_per_market_usdc = Some(20.0);
        let state = TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 8.0,
            total_buy_cost: 8.0,
            buy_count: 4,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let evaluation = positive_quantity_flip_grid_evaluate_buy_candidate_with_balance(
            &cfg,
            &state,
            quote_with_side_ask("up", 0.56),
            true,
            Some(&order_book(&[(0.56, 100.0)])),
            Some(6.0),
        );
        let candidate = evaluation.candidate.expect("partial candidate");

        assert!(candidate.partial_recovery);
        assert!(candidate.actual_buy_usdc <= 5.0);
        assert!(candidate.projected_pnl_at_exit > candidate.pre_pnl_at_exit);
        assert_eq!(evaluation.report["guard_details"]["partial_recovery"], true);
        assert_eq!(
            candidate.partial_recovery_details.as_ref().unwrap()["balance_reserve_usdc"],
            1.0
        );
    }

    #[test]
    fn partial_recovery_does_not_run_outside_completion_window() {
        let mut cfg = test_config();
        cfg.partial_recovery_enabled = true;
        cfg.max_single_buy_usdc = Some(1.2);
        let state = TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 8.0,
            total_buy_cost: 8.0,
            buy_count: 4,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let evaluation = positive_quantity_flip_grid_evaluate_buy_candidate_with_balance(
            &cfg,
            &state,
            quote_with_side_ask("up", 0.56),
            false,
            Some(&order_book(&[(0.56, 100.0)])),
            Some(20.0),
        );

        assert!(evaluation.candidate.is_none());
        assert_eq!(evaluation.report["guard_reason"], "max_single_buy_usdc");
    }

    #[test]
    fn partial_recovery_skips_when_balance_is_unavailable() {
        let mut cfg = test_config();
        cfg.partial_recovery_enabled = true;
        cfg.max_single_buy_usdc = Some(1.2);
        let state = TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 8.0,
            total_buy_cost: 8.0,
            buy_count: 4,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let evaluation = positive_quantity_flip_grid_evaluate_buy_candidate_with_balance(
            &cfg,
            &state,
            quote_with_side_ask("up", 0.56),
            true,
            Some(&order_book(&[(0.56, 100.0)])),
            None,
        );

        assert!(evaluation.candidate.is_none());
        assert_eq!(
            evaluation.report["guard_reason"],
            "partial_recovery_balance_unavailable"
        );
    }

    #[test]
    fn partial_recovery_requires_min_loss_reduction() {
        let mut cfg = test_config();
        cfg.partial_recovery_enabled = true;
        cfg.partial_recovery_min_loss_reduction_usdc = 10.0;
        cfg.max_single_buy_usdc = Some(1.2);
        let state = TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 8.0,
            total_buy_cost: 8.0,
            buy_count: 4,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let evaluation = positive_quantity_flip_grid_evaluate_buy_candidate_with_balance(
            &cfg,
            &state,
            quote_with_side_ask("up", 0.56),
            true,
            Some(&order_book(&[(0.56, 100.0)])),
            Some(6.0),
        );

        assert!(evaluation.candidate.is_none());
        assert_eq!(
            evaluation.report["guard_reason"],
            "partial_recovery_loss_reduction_below_min"
        );
    }

    #[test]
    fn partial_recovery_reserve_can_leave_budget_below_min_marketable() {
        let mut cfg = test_config();
        cfg.partial_recovery_enabled = true;
        cfg.max_single_buy_usdc = Some(1.2);
        let state = TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 8.0,
            total_buy_cost: 8.0,
            buy_count: 4,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let evaluation = positive_quantity_flip_grid_evaluate_buy_candidate_with_balance(
            &cfg,
            &state,
            quote_with_side_ask("up", 0.56),
            true,
            Some(&order_book(&[(0.56, 100.0)])),
            Some(1.5),
        );

        assert!(evaluation.candidate.is_none());
        assert_eq!(
            evaluation.report["guard_reason"],
            "partial_recovery_below_min_marketable"
        );
    }

    #[test]
    fn partial_recovery_uses_full_recovery_amount_when_balance_allows() {
        let mut cfg = test_config();
        cfg.partial_recovery_enabled = true;
        cfg.max_single_buy_usdc = Some(1.2);
        cfg.max_total_spent_per_market_usdc = Some(20.0);
        let state = TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 8.0,
            total_buy_cost: 8.0,
            buy_count: 4,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let evaluation = positive_quantity_flip_grid_evaluate_buy_candidate_with_balance(
            &cfg,
            &state,
            quote_with_side_ask("up", 0.56),
            true,
            Some(&order_book(&[(0.56, 100.0)])),
            Some(50.0),
        );
        let candidate = evaluation.candidate.expect("full-size partial candidate");

        assert!(candidate.partial_recovery);
        assert!(candidate.actual_buy_usdc > 10.0);
        assert!(candidate.projected_pnl_at_exit >= cfg.min_positive_profit_usdc);
    }

    #[test]
    fn consecutive_same_side_buy_is_blocked() {
        let cfg = test_config();
        let state = TradeBuilderPositiveQuantityFlipGridState {
            last_buy_grid_side: Some("up".to_string()),
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let evaluation = positive_quantity_flip_grid_evaluate_buy_candidate(
            &cfg,
            &state,
            quote_with_side_ask("up", 0.53),
            false,
            Some(&order_book(&[(0.53, 10.0)])),
        );

        assert!(evaluation.candidate.is_none());
        assert_eq!(evaluation.report["blocked_by"], "same_side_rebuy");
        assert_eq!(
            evaluation.report["guard_reason"],
            "consecutive_same_side_buy"
        );
        assert_eq!(
            evaluation.report["guard_details"]["last_buy_grid_side"],
            "up"
        );
        assert_eq!(
            evaluation.report["guard_details"]["candidate_grid_side"],
            "up"
        );
    }

    #[test]
    fn opposite_side_buy_is_allowed_after_last_buy() {
        let cfg = test_config();
        let state = TradeBuilderPositiveQuantityFlipGridState {
            last_buy_grid_side: Some("up".to_string()),
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };

        assert!(positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_side_ask("down", 0.53),
            false,
            Some(&order_book(&[(0.53, 10.0)])),
        )
        .is_some());
    }

    #[test]
    fn first_buy_is_allowed_without_last_buy_side() {
        let cfg = test_config();
        let state = TradeBuilderPositiveQuantityFlipGridState::default();

        assert!(positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_side_ask("up", 0.53),
            false,
            Some(&order_book(&[(0.53, 10.0)])),
        )
        .is_some());
    }

    #[test]
    fn first_buy_share_qty_covers_market_buy_min_notional_at_best_ask() {
        let cfg = test_config();
        let state = TradeBuilderPositiveQuantityFlipGridState::default();
        let candidate = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_side_ask("up", 0.50),
            false,
            Some(&order_book(&[(0.50, 10.0)])),
        )
        .expect("candidate");

        assert!(candidate.target_qty * 0.50 >= POSITIVE_QUANTITY_FLIP_GRID_MIN_MARKETABLE_BUY_USDC);
        assert!(candidate.target_qty >= 2.02);
    }

    #[test]
    fn cycle_window_custom_range_blocks_before_start_and_allows_inside() {
        let mut cfg = test_config();
        cfg.cycle_window_mode = Some("custom_range".to_string());
        cfg.cycle_window_start_sec = Some(120);
        cfg.cycle_window_end_sec = Some(300);

        let blocked = positive_quantity_flip_grid_cycle_window_skip_details(&cfg, 300, 200)
            .expect("elapsed 100 is before 120");
        assert_eq!(blocked["blocked_by"], "cycle_window");
        assert_eq!(
            blocked["guard_reason"],
            "outside_positive_grid_cycle_window"
        );
        assert_eq!(blocked["elapsed_sec"], 100);

        assert!(positive_quantity_flip_grid_cycle_window_skip_details(&cfg, 300, 150).is_none());
    }

    #[test]
    fn cycle_window_last_seconds_blocks_until_remaining_is_inside() {
        let mut cfg = test_config();
        cfg.cycle_window_mode = Some("last".to_string());
        cfg.cycle_window_secs = Some(120);

        assert!(positive_quantity_flip_grid_cycle_window_skip_details(&cfg, 300, 150).is_some());
        assert!(positive_quantity_flip_grid_cycle_window_skip_details(&cfg, 300, 90).is_none());
    }
}
