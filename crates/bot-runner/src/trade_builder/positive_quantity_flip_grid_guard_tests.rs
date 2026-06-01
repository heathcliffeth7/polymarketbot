#[cfg(test)]
mod positive_quantity_flip_grid_guard_tests {
    use super::*;
    use super::positive_quantity_flip_grid_test_helpers::*;

    #[test]
    fn no_buy_range_blocks_candidate_inside_inclusive_band() {
        let mut cfg = test_config();
        cfg.no_buy_ranges = vec![PositiveQuantityFlipGridNoBuyRange {
            min_cent: 56.0,
            max_cent: 60.0,
        }];
        let state = TradeBuilderPositiveQuantityFlipGridState::default();
        let book = order_book(&[(0.57, 10.0)]);

        assert!(positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_ask(0.57),
            false,
            Some(&book),
        )
        .is_none());
    }

    #[test]
    fn no_buy_candidate_report_includes_guard_details() {
        let mut cfg = test_config();
        cfg.no_buy_ranges = vec![PositiveQuantityFlipGridNoBuyRange {
            min_cent: 56.0,
            max_cent: 60.0,
        }];
        let state = TradeBuilderPositiveQuantityFlipGridState::default();
        let evaluation = positive_quantity_flip_grid_evaluate_buy_candidate(
            &cfg,
            &state,
            quote_with_ask(0.57),
            false,
            Some(&order_book(&[(0.57, 10.0)])),
        );

        assert!(evaluation.candidate.is_none());
        assert_eq!(evaluation.report["grid_side"], "down");
        assert_eq!(evaluation.report["blocked_by"], "no_buy_range");
        assert_eq!(evaluation.report["guard_reason"], "inside_no_buy_range");
        assert_eq!(
            evaluation.report["guard_details"]["range"]["min_cent"],
            56.0
        );
    }

    #[test]
    fn no_buy_range_allows_candidate_outside_band() {
        let mut cfg = test_config();
        cfg.no_buy_ranges = vec![PositiveQuantityFlipGridNoBuyRange {
            min_cent: 56.0,
            max_cent: 60.0,
        }];
        let state = TradeBuilderPositiveQuantityFlipGridState::default();
        let book = order_book(&[(0.55, 10.0)]);

        assert!(positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_ask(0.55),
            false,
            Some(&book),
        )
        .is_some());
    }

    #[test]
    fn no_buy_range_blocks_upper_boundary() {
        let mut cfg = test_config();
        cfg.no_buy_ranges = vec![PositiveQuantityFlipGridNoBuyRange {
            min_cent: 56.0,
            max_cent: 60.0,
        }];
        let state = TradeBuilderPositiveQuantityFlipGridState::default();
        let book = order_book(&[(0.60, 10.0)]);

        assert!(positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_ask(0.60),
            false,
            Some(&book),
        )
        .is_none());
    }

    #[test]
    fn max_buy_price_blocks_above_hard_or_worst_price() {
        let mut cfg = test_config();
        cfg.entry_band_max_cent = 70.0;
        let state = TradeBuilderPositiveQuantityFlipGridState::default();
        let book = order_book(&[(0.605, 10.0)]);

        assert!(positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_ask(0.605),
            false,
            Some(&book),
        )
        .is_none());
    }

    #[test]
    fn execution_floor_blocks_below_floor_and_allows_above() {
        let mut cfg = test_config();
        cfg.execution_floor_price_cent = Some(52.0);
        let state = TradeBuilderPositiveQuantityFlipGridState::default();
        let low_book = order_book(&[(0.51, 10.0)]);
        let ok_book = order_book(&[(0.53, 10.0)]);

        assert!(positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_ask(0.51),
            false,
            Some(&low_book),
        )
        .is_none());
        assert!(positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_ask(0.53),
            false,
            Some(&ok_book),
        )
        .is_some());
    }

    #[test]
    fn trigger_price_guard_blocks_missing_or_below_trigger_price() {
        let mut cfg = test_config();
        cfg.trigger_price_guard_enabled = true;
        let quote = quote_with_ask(0.52);
        let empty_step = test_step(None);
        let below_step = test_step(Some(json!({ "trigger_price": 0.53 })));
        let pass_step = test_step(Some(json!({ "trigger_price": 0.52 })));

        let missing = positive_quantity_flip_grid_trigger_guard_report(&cfg, &empty_step, &quote)
            .expect("missing trigger blocks");
        assert_eq!(missing["blocked_by"], "trigger_price");
        assert_eq!(missing["guard_reason"], "missing_trigger_price");
        let below = positive_quantity_flip_grid_trigger_guard_report(&cfg, &below_step, &quote)
            .expect("below trigger blocks");
        assert_eq!(below["guard_reason"], "below_trigger_price_guard");
        assert!(
            positive_quantity_flip_grid_trigger_guard_report(&cfg, &pass_step, &quote).is_none()
        );
    }

    #[test]
    fn ptb_guard_gap_threshold_supports_usd_and_cent_units() {
        let mut cfg = test_config();
        cfg.ptb_guard_enabled = true;
        cfg.ptb_min_diff = 2.0;
        cfg.ptb_diff_unit = crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Usd;
        let quote = quote_with_ask(0.53);
        let state = TradeBuilderPositiveQuantityFlipGridState::default();

        let blocked = positive_quantity_flip_grid_ptb_gap_guard_report_for_test(
            &cfg,
            &state,
            &quote,
            1.5,
        )
        .expect("usd gap below threshold blocks");
        assert_eq!(blocked["blocked_by"], "price_to_beat");
        assert!(positive_quantity_flip_grid_ptb_gap_guard_report_for_test(
            &cfg, &state, &quote, 2.5
        )
        .is_none());

        cfg.ptb_diff_unit = crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Cent;
        let cent_blocked = positive_quantity_flip_grid_ptb_gap_guard_report_for_test(
            &cfg, &state, &quote, 0.015,
        )
        .expect("cent gap below threshold blocks");
        assert_eq!(cent_blocked["guard_details"]["threshold_usd"], 0.02);
        assert!(positive_quantity_flip_grid_ptb_gap_guard_report_for_test(
            &cfg, &state, &quote, 0.03
        )
        .is_none());
    }

    #[test]
    fn ptb_rescue_min_diff_config_parses() {
        let cfg = resolve_positive_quantity_flip_grid_config(&positive_grid_node(json!({
            "ptbGuardEnabled": true,
            "ptbMinDiff": 80,
            "ptbRescueMinDiff": 40,
        })))
        .expect("valid config");

        assert_eq!(cfg.ptb_min_diff, 80.0);
        assert_eq!(cfg.ptb_rescue_min_diff, Some(40.0));
    }

    #[test]
    fn ptb_rescue_min_diff_rejects_non_positive_values() {
        let err = resolve_positive_quantity_flip_grid_config(&positive_grid_node(json!({
            "ptbGuardEnabled": true,
            "ptbMinDiff": 80,
            "ptbRescueMinDiff": 0,
        })))
        .expect_err("zero rescue ptb");

        assert!(err
            .to_string()
            .contains("positiveQuantityFlipGrid ptbRescueMinDiff must be > 0"));
    }

    #[test]
    fn ptb_threshold_uses_rescue_value_when_rescue_buy_eligible() {
        let mut cfg = test_config();
        cfg.ptb_guard_enabled = true;
        cfg.ptb_min_diff = 80.0;
        cfg.ptb_rescue_min_diff = Some(40.0);
        cfg.rescue_buy_enabled = true;
        cfg.rescue_buy_min_price_cent = 60.0;
        cfg.rescue_buy_max_price_cent = 70.0;
        cfg.min_positive_profit_usdc = 1.0;
        let state = TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 3.0,
            total_buy_cost: 3.0,
            buy_count: 1,
            down_qty: 2.0,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };

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
        assert!(
            positive_quantity_flip_grid_ptb_gap_guard_report_for_test(
                &cfg,
                &state,
                &quote_with_side_ask("down", 0.65),
                45.0,
            )
            .is_none()
        );
        assert!(
            positive_quantity_flip_grid_ptb_gap_guard_report_for_test(
                &cfg,
                &state,
                &quote_with_side_ask("down", 0.65),
                50.0,
            )
            .is_none()
        );
    }

    #[test]
    fn ptb_threshold_uses_normal_when_not_rescue_eligible() {
        let mut cfg = test_config();
        cfg.ptb_guard_enabled = true;
        cfg.ptb_min_diff = 80.0;
        cfg.ptb_rescue_min_diff = Some(40.0);
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
    fn ptb_threshold_falls_back_to_normal_when_rescue_unset() {
        let mut cfg = test_config();
        cfg.ptb_guard_enabled = true;
        cfg.ptb_min_diff = 80.0;
        cfg.ptb_rescue_min_diff = None;
        cfg.rescue_buy_enabled = true;
        cfg.rescue_buy_min_price_cent = 60.0;
        cfg.rescue_buy_max_price_cent = 70.0;
        cfg.min_positive_profit_usdc = 1.0;
        let state = TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 3.0,
            total_buy_cost: 3.0,
            buy_count: 1,
            down_qty: 2.0,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };

        let (value, mode) = positive_quantity_flip_grid_ptb_threshold_for_quote(
            &cfg,
            &state,
            "down",
            0.65,
        );

        assert_eq!(mode, "rescue");
        assert_eq!(value, 80.0);
    }

    #[test]
    fn rescue_buy_allows_above_normal_max_only_when_side_is_not_positive() {
        let mut cfg = test_config();
        cfg.rescue_buy_enabled = true;
        cfg.rescue_buy_max_price_cent = 70.0;
        cfg.max_single_buy_usdc = Some(5.0);
        let state = TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 1.0,
            total_buy_cost: 1.0,
            buy_count: 1,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let candidate = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_ask(0.70),
            false,
            Some(&order_book(&[(0.70, 10.0)])),
        )
        .expect("rescue candidate");

        assert!(candidate.rescue_buy);
        assert_eq!(candidate.worst_price, 0.70);

        let already_positive = TradeBuilderPositiveQuantityFlipGridState {
            down_qty: 2.0,
            net_cost: 1.0,
            total_buy_cost: 1.0,
            buy_count: 1,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let blocked = positive_quantity_flip_grid_evaluate_buy_candidate(
            &cfg,
            &already_positive,
            quote_with_ask(0.70),
            false,
            Some(&order_book(&[(0.70, 10.0)])),
        );
        assert!(blocked.candidate.is_none());
        assert_eq!(
            blocked.report["guard_reason"],
            "rescue_side_already_positive"
        );
    }

    #[test]
    fn rescue_range_config_parses_custom_min_price() {
        let cfg = resolve_positive_quantity_flip_grid_config(&positive_grid_node(json!({
            "entryBandMinCent": 50,
            "entryBandMaxCent": 60,
            "hardMaxPriceCent": 60,
            "worstPriceCent": 60,
            "rescueBuyEnabled": true,
            "rescueBuyMinPriceCent": 63,
            "rescueBuyMaxPriceCent": 70,
        })))
        .expect("valid config");

        assert_eq!(cfg.rescue_buy_min_price_cent, 63.0);
        assert_eq!(cfg.rescue_buy_max_price_cent, 70.0);
    }

    #[test]
    fn entry_band_config_accepts_price_above_60() {
        let cfg = resolve_positive_quantity_flip_grid_config(&positive_grid_node(json!({
            "entryBandMinCent": 50,
            "entryBandMaxCent": 70,
            "hardMaxPriceCent": 70,
            "worstPriceCent": 70,
            "rescueBuyEnabled": true,
            "rescueBuyMinPriceCent": 70,
            "rescueBuyMaxPriceCent": 75,
        })))
        .expect("valid config above legacy 60 cent cap");

        assert_eq!(cfg.entry_band_max_cent, 70.0);
        assert_eq!(cfg.hard_max_price_cent, 70.0);
        assert_eq!(cfg.worst_price_cent, 70.0);
    }

    #[test]
    fn rescue_range_config_rejects_invalid_range() {
        let err = resolve_positive_quantity_flip_grid_config(&positive_grid_node(json!({
            "entryBandMinCent": 50,
            "entryBandMaxCent": 60,
            "hardMaxPriceCent": 60,
            "worstPriceCent": 60,
            "rescueBuyEnabled": true,
            "rescueBuyMinPriceCent": 59,
            "rescueBuyMaxPriceCent": 70,
        })))
        .expect_err("invalid rescue min below normal max");

        assert!(err.to_string().contains("rescue range"));
    }

    #[test]
    fn rescue_buy_does_not_open_initial_position_above_normal_band() {
        let mut cfg = test_config();
        cfg.rescue_buy_enabled = true;
        cfg.rescue_buy_max_price_cent = 70.0;
        let state = TradeBuilderPositiveQuantityFlipGridState::default();
        let blocked = positive_quantity_flip_grid_evaluate_buy_candidate(
            &cfg,
            &state,
            quote_with_ask(0.70),
            false,
            Some(&order_book(&[(0.70, 10.0)])),
        );

        assert!(blocked.candidate.is_none());
        assert_eq!(blocked.report["blocked_by"], "rescue");
        assert_eq!(
            blocked.report["guard_reason"],
            "rescue_requires_existing_buy"
        );
        assert_eq!(blocked.report["guard_details"]["buy_count"], 0);
    }

    #[test]
    fn rescue_buy_blocks_above_rescue_max() {
        let mut cfg = test_config();
        cfg.rescue_buy_enabled = true;
        cfg.rescue_buy_max_price_cent = 70.0;
        cfg.entry_band_max_cent = 60.0;
        let state = TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 1.0,
            total_buy_cost: 1.0,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let blocked = positive_quantity_flip_grid_evaluate_buy_candidate(
            &cfg,
            &state,
            quote_with_ask(0.71),
            false,
            Some(&order_book(&[(0.71, 10.0)])),
        );

        assert!(blocked.candidate.is_none());
        assert_eq!(blocked.report["blocked_by"], "max_price");
        assert_eq!(blocked.report["guard_reason"], "above_rescue_max_price");
    }

    #[test]
    fn rescue_buy_min_price_is_configurable() {
        let mut cfg = test_config();
        cfg.rescue_buy_enabled = true;
        cfg.rescue_buy_min_price_cent = 63.0;
        cfg.rescue_buy_max_price_cent = 70.0;
        cfg.max_single_buy_usdc = Some(10.0);
        let state = TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 1.0,
            total_buy_cost: 1.0,
            buy_count: 1,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };

        let below_rescue = positive_quantity_flip_grid_evaluate_buy_candidate(
            &cfg,
            &state,
            quote_with_ask(0.62),
            false,
            Some(&order_book(&[(0.62, 10.0)])),
        );
        assert!(below_rescue.candidate.is_none());
        assert_eq!(below_rescue.report["blocked_by"], "rescue");
        assert_eq!(below_rescue.report["guard_reason"], "below_rescue_range");

        let candidate = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_ask(0.64),
            false,
            Some(&order_book(&[(0.64, 10.0)])),
        )
        .expect("rescue candidate above custom min");
        assert!(candidate.rescue_buy);
    }

    #[test]
    fn rescue_buy_max_is_inclusive() {
        let mut cfg = test_config();
        cfg.rescue_buy_enabled = true;
        cfg.rescue_buy_max_price_cent = 70.0;
        cfg.max_single_buy_usdc = Some(10.0);
        let state = TradeBuilderPositiveQuantityFlipGridState {
            net_cost: 1.0,
            total_buy_cost: 1.0,
            buy_count: 1,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };

        assert!(positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_ask(0.70),
            false,
            Some(&order_book(&[(0.70, 10.0)])),
        )
        .is_some());

        let blocked = positive_quantity_flip_grid_evaluate_buy_candidate(
            &cfg,
            &state,
            quote_with_ask(0.705),
            false,
            Some(&order_book(&[(0.705, 10.0)])),
        );
        assert!(blocked.candidate.is_none());
        assert_eq!(blocked.report["guard_reason"], "above_rescue_max_price");
    }

    #[test]
    fn sell_side_ignores_zero_open_qty_after_sold() {
        let cfg = test_config();
        let quote = PositiveQuantityFlipGridQuote {
            grid_side: "up",
            token_id: "up".to_string(),
            outcome_label: "Up".to_string(),
            best_bid: Some(0.98),
            best_ask: Some(0.99),
            quote_snapshot: json!({}),
        };
        let state = TradeBuilderPositiveQuantityFlipGridState {
            up_qty: 0.0,
            total_buy_cost: 1.0,
            total_sell_revenue: 1.85,
            net_cost: -0.85,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        assert!(positive_quantity_flip_grid_exit_side(&cfg, &state, &[quote]).is_none());
    }

}
