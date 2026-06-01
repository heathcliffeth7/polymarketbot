#[cfg(test)]
mod positive_quantity_flip_grid_exit_depth_tests {
    use super::*;
    use super::positive_quantity_flip_grid_test_helpers::*;

    #[test]
    fn sell_side_requires_bid_and_positive_total_net() {
        let cfg = test_config();
        let quote = PositiveQuantityFlipGridQuote {
            grid_side: "up",
            token_id: "up".to_string(),
            outcome_label: "Up".to_string(),
            best_bid: Some(0.98),
            best_ask: Some(0.99),
            quote_snapshot: json!({}),
        };
        let losing_state = TradeBuilderPositiveQuantityFlipGridState {
            up_qty: 3.0,
            net_cost: 3.2,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        assert!(
            positive_quantity_flip_grid_exit_side(&cfg, &losing_state, &[quote.clone()]).is_none()
        );
        let winning_state = TradeBuilderPositiveQuantityFlipGridState {
            up_qty: 3.8,
            net_cost: 3.5,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        assert!(positive_quantity_flip_grid_exit_side(&cfg, &winning_state, &[quote]).is_some());
    }

    #[test]
    fn sell_side_requires_configured_quarter_usdc_profit_target() {
        let mut cfg = test_config();
        cfg.min_sell_net_profit_usdc = 0.25;
        let quote = PositiveQuantityFlipGridQuote {
            grid_side: "up",
            token_id: "up".to_string(),
            outcome_label: "Up".to_string(),
            best_bid: Some(0.98),
            best_ask: Some(0.99),
            quote_snapshot: json!({}),
        };
        let below_target_state = TradeBuilderPositiveQuantityFlipGridState {
            up_qty: 3.8,
            net_cost: 3.5,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        assert!(
            positive_quantity_flip_grid_exit_side(&cfg, &below_target_state, &[quote.clone()])
                .is_none()
        );

        let above_target_state = TradeBuilderPositiveQuantityFlipGridState {
            up_qty: 3.9,
            net_cost: 3.5,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        assert!(
            positive_quantity_flip_grid_exit_side(&cfg, &above_target_state, &[quote]).is_some()
        );
    }

    #[test]
    fn sell_side_accepts_down_bid_at_take_profit_level() {
        let cfg = test_config();
        let quote = quote_with_side_bid("down", 0.98);
        let state = TradeBuilderPositiveQuantityFlipGridState {
            down_qty: 3.8,
            net_cost: 3.5,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };

        let exit = positive_quantity_flip_grid_exit_side(&cfg, &state, &[quote]).expect("exit");
        assert_eq!(exit.0, "down");
        assert_eq!(exit.1, 0.98);
    }

    #[test]
    fn sell_side_chooses_highest_projected_profit_when_both_sides_hit_take_profit() {
        let cfg = test_config();
        let state = TradeBuilderPositiveQuantityFlipGridState {
            up_qty: 4.0,
            down_qty: 5.0,
            net_cost: 3.5,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };

        let exit = positive_quantity_flip_grid_exit_side(
            &cfg,
            &state,
            &[
                quote_with_side_bid("up", 0.98),
                quote_with_side_bid("down", 0.98),
            ],
        )
        .expect("exit");
        assert_eq!(exit.0, "down");
        assert!((exit.2 - 1.4).abs() < 0.000001);
    }

    #[test]
    fn depth_blocks_when_visible_qty_is_below_target() {
        let cfg = test_config();
        let state = TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 1.0,
            total_buy_cost: 1.0,
            buy_count: 1,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let quote = quote_with_ask(0.53);
        let book = order_book(&[(0.53, 1.0)]);
        assert!(
            positive_quantity_flip_grid_buy_candidate(&cfg, &state, quote, false, Some(&book))
                .is_none()
        );
    }

    #[test]
    fn depth_reprices_candidate_with_effective_average_fill() {
        let cfg = test_config();
        let state = TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 1.0,
            total_buy_cost: 1.0,
            buy_count: 1,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let quote = quote_with_ask(0.53);
        let book = order_book(&[(0.53, 1.0), (0.56, 2.0)]);
        let candidate =
            positive_quantity_flip_grid_buy_candidate(&cfg, &state, quote, false, Some(&book))
                .expect("candidate");
        assert!(candidate.effective_ask_price > 0.53);
        assert!(candidate.target_qty > 2.2);
        assert!(candidate.projected_pnl_at_exit >= cfg.min_positive_profit_usdc);
    }

    #[test]
    fn sizing_price_buffer_increases_quantity_for_one_cent_adverse_fill() {
        let mut no_buffer_cfg = test_config();
        no_buffer_cfg.max_single_buy_usdc = Some(20.0);
        no_buffer_cfg.max_total_spent_per_market_usdc = Some(20.0);
        let mut buffered_cfg = no_buffer_cfg.clone();
        buffered_cfg.sizing_price_buffer_cent = 1.0;
        let state = TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 1.0,
            total_buy_cost: 1.0,
            buy_count: 1,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let book = order_book(&[(0.59, 100.0)]);

        let no_buffer = positive_quantity_flip_grid_buy_candidate(
            &no_buffer_cfg,
            &state,
            quote_with_ask(0.59),
            false,
            Some(&book),
        )
        .expect("no-buffer candidate");
        let buffered = positive_quantity_flip_grid_buy_candidate(
            &buffered_cfg,
            &state,
            quote_with_ask(0.59),
            false,
            Some(&book),
        )
        .expect("buffered candidate");

        assert_eq!(buffered.sizing_ask_price, 0.60);
        assert!(buffered.target_qty > no_buffer.target_qty);
        assert!(buffered.actual_buy_usdc > no_buffer.actual_buy_usdc);
        assert!(buffered.projected_pnl_at_exit >= buffered_cfg.min_positive_profit_usdc);
    }

    #[test]
    fn quarter_usdc_profit_target_with_three_cent_buffer_expands_flip_size() {
        let mut cfg = test_config();
        cfg.min_positive_profit_usdc = 0.25;
        cfg.min_sell_net_profit_usdc = 0.25;
        cfg.sizing_price_buffer_cent = 3.0;
        cfg.max_single_buy_usdc = Some(35.0);
        cfg.max_total_spent_per_market_usdc = Some(35.0);
        let state = TradeBuilderPositiveQuantityFlipGridState {
            down_qty: 1.73,
            net_cost: 1.02,
            total_buy_cost: 1.02,
            buy_count: 1,
            last_buy_grid_side: Some("down".to_string()),
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let book = order_book(&[(0.53, 1.0), (0.57, 100.0)]);

        let candidate = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_side_ask("up", 0.53),
            false,
            Some(&book),
        )
        .expect("candidate");

        assert!(candidate.sizing_ask_price > 0.58);
        assert!(candidate.actual_buy_usdc > 1.8);
        assert!(candidate.actual_buy_usdc < 2.1);
        assert!(candidate.projected_pnl_at_exit >= 0.25);
    }

}
