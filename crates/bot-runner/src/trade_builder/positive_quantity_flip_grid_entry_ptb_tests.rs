#[cfg(test)]
mod positive_quantity_flip_grid_entry_ptb_tests {
    use super::*;
    use serde_json::json;

    fn entry_ptb_test_config() -> PositiveQuantityFlipGridConfig {
        PositiveQuantityFlipGridConfig {
            base_buy_usdc: 1.0,
            min_marketable_buy_usdc: POSITIVE_QUANTITY_FLIP_GRID_MIN_MARKETABLE_BUY_USDC,
            entry_band_min_cent: 45.0,
            entry_band_max_cent: 60.0,
            preferred_trigger_cent: 53.0,
            trigger_tolerance_cent: 10.0,
            exit_price_for_sizing: 0.98,
            sizing_price_buffer_cent: 0.0,
            partial_recovery_enabled: false,
            partial_recovery_min_loss_reduction_usdc: 0.10,
            partial_recovery_balance_reserve_usdc: 1.0,
            partial_recovery_max_buy_usdc: None,
            partial_recovery_ignore_market_budget: true,
            quantity_sizing_mode: PositiveQuantityFlipGridQuantitySizingMode::ProfitTarget,
            inventory_balance_lead_qty: 0.0,
            min_positive_profit_usdc: 0.03,
            min_sell_net_profit_usdc: 0.03,
            max_single_buy_usdc: Some(5.0),
            max_total_spent_per_market_usdc: Some(10.0),
            max_active_markets: 1,
            max_open_grid_buys_per_market: 8,
            sell_bid_min: 0.98,
            hard_max_price_cent: 60.0,
            worst_price_cent: 60.0,
            rescue_buy_enabled: false,
            rescue_buy_min_price_cent: 60.0,
            rescue_buy_max_price_cent: 70.0,
            block_consecutive_same_side_buys: true,
            no_buy_ranges: Vec::new(),
            cycle_window_mode: None,
            cycle_window_secs: None,
            cycle_window_start_sec: None,
            cycle_window_end_sec: None,
            new_grid_buy_start_remaining_sec: 285,
            new_grid_buy_end_remaining_sec: 90,
            positive_completion_buy_end_remaining_sec: 30,
            no_new_buy_under_sec: 30,
            order_type: "FAK",
            pairlock_compression_enabled: false,
            stop_buys_after_pairlock_merge: false,
            target_pairlock_profit: 0.05,
            fee_buffer: 0.01,
            max_pair_cost: 0.94,
            pairlock_order_type: "FOK",
            max_unmerged_exposure_usdc: 2.0,
            min_basket_profit_usdc: 0.06,
            min_direct_profit_usdc: 0.05,
            basket_exit_enabled: false,
            direct_exit_enabled: false,
            execution_floor_guard_enabled: true,
            execution_floor_price_cent: None,
            trigger_price_guard_enabled: false,
            ptb_guard_enabled: true,
            ptb_min_diff: 80.0,
            ptb_rescue_min_diff: Some(40.0),
            ptb_diff_unit: crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Usd,
            ptb_current_price_source:
                crate::trade_flow::guards::price_to_beat::PriceToBeatCurrentPriceSource::Chainlink,
            depth_guard_enabled: true,
        }
    }

    fn quote_with_side_ask(grid_side: &'static str, ask: f64) -> PositiveQuantityFlipGridQuote {
        PositiveQuantityFlipGridQuote {
            grid_side,
            token_id: grid_side.to_string(),
            outcome_label: if grid_side == "up" { "Up" } else { "Down" }.to_string(),
            best_bid: Some(1.0 - ask),
            best_ask: Some(ask),
            quote_snapshot: json!({}),
        }
    }

    fn rescue_eligible_state() -> TradeBuilderPositiveQuantityFlipGridState {
        TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 3.0,
            total_buy_cost: 3.0,
            buy_count: 1,
            down_qty: 2.0,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        }
    }

    #[test]
    fn positive_quantity_flip_grid_entry_ptb_uses_rescue_diff_when_rescue_buy_eligible() {
        let mut cfg = entry_ptb_test_config();
        cfg.rescue_buy_enabled = true;
        cfg.min_positive_profit_usdc = 1.0;
        let state = rescue_eligible_state();

        let (value, mode) = positive_quantity_flip_grid_ptb_threshold_for_quote(
            &cfg,
            &state,
            "down",
            0.65,
        );

        assert_eq!(mode, "rescue");
        assert_eq!(value, 40.0);

        let blocked = positive_quantity_flip_grid_ptb_gap_guard_report_for_test(
            &cfg,
            &state,
            &quote_with_side_ask("down", 0.65),
            35.0,
        )
        .expect("gap below rescue threshold blocks");
        assert_eq!(blocked["guard_details"]["ptb_threshold_mode"], "rescue");
        assert_eq!(blocked["guard_details"]["ptb_threshold_value"], 40.0);
    }

    #[test]
    fn positive_quantity_flip_grid_entry_ptb_uses_normal_diff_when_not_rescue_eligible() {
        let mut cfg = entry_ptb_test_config();
        cfg.rescue_buy_enabled = true;
        let state = TradeBuilderPositiveQuantityFlipGridState::default();

        let (value, mode) = positive_quantity_flip_grid_ptb_threshold_for_quote(
            &cfg,
            &state,
            "down",
            0.65,
        );

        assert_eq!(mode, "normal");
        assert_eq!(value, 80.0);
    }

    #[test]
    fn positive_quantity_flip_grid_entry_ptb_falls_back_to_normal_when_rescue_diff_unset() {
        let mut cfg = entry_ptb_test_config();
        cfg.rescue_buy_enabled = true;
        cfg.min_positive_profit_usdc = 1.0;
        cfg.ptb_rescue_min_diff = None;
        let state = rescue_eligible_state();

        let (value, mode) = positive_quantity_flip_grid_ptb_threshold_for_quote(
            &cfg,
            &state,
            "down",
            0.65,
        );

        assert_eq!(mode, "rescue");
        assert_eq!(value, 80.0);
    }
}
