#[cfg(test)]
mod positive_quantity_flip_grid_min_notional_tests {
    use super::*;

    fn test_config() -> PositiveQuantityFlipGridConfig {
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
            ptb_guard_enabled: false,
            ptb_min_diff: 2.0,
            ptb_rescue_min_diff: None,
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

    fn order_book(asks: &[(f64, f64)]) -> OrderBookSnapshot {
        OrderBookSnapshot {
            bids: Vec::new(),
            asks: asks
                .iter()
                .map(|(price, size)| bot_infra::exchange::OrderBookLevel {
                    price: *price,
                    size: *size,
                })
                .collect(),
        }
    }

    fn test_step(input_json: Option<Value>) -> TradeFlowRunStep {
        TradeFlowRunStep {
            id: 0,
            run_id: 0,
            node_key: "trigger".to_string(),
            node_type: "trigger.market_price".to_string(),
            status: "completed".to_string(),
            attempt: 1,
            input_json,
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

    struct SelectionTestExecutor;

    #[async_trait::async_trait]
    impl OrderExecutor for SelectionTestExecutor {
        async fn midpoint(&self, _market: &str) -> Result<bot_infra::exchange::PriceSnapshot> {
            anyhow::bail!("unused")
        }

        async fn fee_rate_bps(&self, _token_id: &str) -> Result<Option<u64>> {
            Ok(Some(0))
        }

        async fn place(&self, _req: &PlaceOrderRequest) -> Result<bot_infra::exchange::OrderAck> {
            anyhow::bail!("unused")
        }

        async fn cancel(&self, _exchange_order_id: &str) -> Result<()> {
            anyhow::bail!("unused")
        }

        async fn status(&self, _exchange_order_id: &str) -> Result<OrderInfo> {
            anyhow::bail!("unused")
        }

        async fn list_open(&self, _market: Option<&str>) -> Result<Vec<OrderInfo>> {
            anyhow::bail!("unused")
        }

        async fn list_fills(&self, _next_cursor: Option<&str>) -> Result<Vec<FillInfo>> {
            anyhow::bail!("unused")
        }

        async fn available_token_qty(&self, _token_id: &str) -> Result<Option<f64>> {
            Ok(Some(100.0))
        }
    }

    fn lot(
        parent_builder_order_id: i64,
        grid_side: &str,
        quantity: f64,
        execution_price: f64,
    ) -> TradeBuilderPositiveQuantityFlipGridLot {
        TradeBuilderPositiveQuantityFlipGridLot {
            parent_builder_order_id,
            market_slug: "btc-updown-1".to_string(),
            token_id: format!("{grid_side}-token"),
            outcome_label: if grid_side == "up" { "Up" } else { "Down" }.to_string(),
            grid_side: grid_side.to_string(),
            intent: "flip_positive".to_string(),
            quantity,
            execution_price,
            notional_usdc: quantity * execution_price,
            created_at: Utc::now(),
        }
    }

    fn positive_grid_action_with_stop_loss() -> TradeFlowNode {
        TradeFlowNode {
            key: "positive_grid".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({
                "mode": ACTION_PLACE_ORDER_MODE_POSITIVE_QUANTITY_FLIP_GRID_V1,
                "slEnabled": true,
                "slPriceCent": 42.0,
                "slRules": [
                    { "priceCent": 44.0, "sizePct": 60.0 },
                    { "priceCent": 36.0, "sizePct": 40.0 }
                ],
                "ptbStopLossEnabled": true,
                "ptbStopLossRules": [
                    { "gapUsd": 20.0, "sizePct": 60.0 },
                    { "gapUsd": 0.0, "sizePct": 40.0 }
                ],
                "ptbStopLossGapUnit": "cent",
                "ptbStopLossTimeDecayMode": "relax",
                "notifyOnSlHit": true,
                "reenterOnSlHit": true,
                "reentryMaxAttempts": 2,
            }),
        }
    }

    fn normal_child_candidate(
        cfg: &PositiveQuantityFlipGridConfig,
    ) -> PositiveQuantityFlipGridBuyCandidate {
        positive_quantity_flip_grid_buy_candidate(
            cfg,
            &TradeBuilderPositiveQuantityFlipGridState::default(),
            quote_with_side_ask("up", 0.50),
            false,
            Some(&order_book(&[(0.50, 10.0)])),
        )
        .expect("candidate")
    }

    #[test]
    fn normal_buy_child_preserves_stop_loss_config() {
        let mut cfg = test_config();
        cfg.ptb_current_price_source =
            crate::trade_flow::guards::price_to_beat::PriceToBeatCurrentPriceSource::Binance;
        let candidate = normal_child_candidate(&cfg);
        let node = positive_grid_action_with_stop_loss();

        let child = positive_quantity_flip_grid_buy_node(
            &node,
            &cfg,
            "btc-updown-5m-1773319200",
            &candidate,
            1,
            None,
            None,
        );
        let child_config = child.config.as_object().expect("child config");

        assert_eq!(
            child_config.get("tpEnabled").and_then(Value::as_bool),
            Some(false)
        );
        assert!(child_config.get("tpRules").is_none());
        assert!(child_config.get("timeExitRules").is_none());
        assert_eq!(
            child_config.get("slEnabled").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            child_config.get("slPriceCent").and_then(Value::as_f64),
            Some(42.0)
        );
        assert_eq!(
            child_config
                .get("slRules")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2)
        );
        assert_eq!(
            child_config
                .get("ptbStopLossEnabled")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            child_config
                .get("ptbStopLossRules")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(2)
        );
        assert_eq!(
            child_config
                .get("priceToBeatCurrentPriceSource")
                .and_then(Value::as_str),
            Some("binance")
        );
        assert_eq!(
            child_config.get("notifyOnSlHit").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            child_config.get("reenterOnSlHit").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn non_normal_buy_children_strip_stop_loss_config() {
        let cfg = test_config();
        let node = positive_grid_action_with_stop_loss();
        let base_candidate = normal_child_candidate(&cfg);
        let mut rescue_candidate = base_candidate.clone();
        rescue_candidate.rescue_buy = true;
        let mut partial_candidate = base_candidate.clone();
        partial_candidate.partial_recovery = true;

        for (candidate, intent_override) in [
            (rescue_candidate, None),
            (partial_candidate, None),
            (base_candidate, Some("pairlock_compression_buy")),
        ] {
            let child = positive_quantity_flip_grid_buy_node(
                &node,
                &cfg,
                "btc-updown-5m-1773319200",
                &candidate,
                1,
                intent_override,
                None,
            );
            let child_config = child.config.as_object().expect("child config");

            assert_eq!(
                child_config.get("slEnabled").and_then(Value::as_bool),
                Some(false)
            );
            assert!(child_config.get("slPriceCent").is_none());
            assert!(child_config.get("slRules").is_none());
            assert!(child_config.get("ptbStopLossEnabled").is_none());
            assert!(child_config.get("ptbStopLossRules").is_none());
            assert!(child_config.get("priceToBeatCurrentPriceSource").is_none());
            assert!(child_config.get("notifyOnSlHit").is_none());
            assert!(child_config.get("reenterOnSlHit").is_none());
        }
    }

    #[test]
    fn normal_first_buy_uses_configured_base_buy_usdc() {
        let mut cfg = test_config();
        cfg.base_buy_usdc = 1.50;
        cfg.min_marketable_buy_usdc = 1.05;

        let candidate = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &TradeBuilderPositiveQuantityFlipGridState::default(),
            quote_with_side_ask("up", 0.50),
            false,
            Some(&order_book(&[(0.50, 10.0)])),
        )
        .expect("candidate");

        assert_eq!(candidate.target_qty, 3.0);
        assert!(candidate.actual_buy_usdc >= 1.50);
    }

    #[test]
    fn normal_first_buy_uses_min_marketable_floor_when_base_is_smaller() {
        let mut cfg = test_config();
        cfg.base_buy_usdc = 0.80;
        cfg.min_marketable_buy_usdc = 1.05;

        let candidate = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &TradeBuilderPositiveQuantityFlipGridState::default(),
            quote_with_side_ask("down", 0.50),
            false,
            Some(&order_book(&[(0.50, 10.0)])),
        )
        .expect("candidate");

        assert_eq!(candidate.target_qty, 2.10);
        assert!(candidate.actual_buy_usdc >= 1.05);
    }

    fn pairlock_first_buy_config() -> PositiveQuantityFlipGridConfig {
        let mut cfg = test_config();
        cfg.pairlock_compression_enabled = true;
        cfg.stop_buys_after_pairlock_merge = true;
        cfg.max_single_buy_usdc = Some(5.0);
        cfg.max_total_spent_per_market_usdc = Some(10.0);
        cfg
    }

    #[test]
    fn pairlock_first_buy_uses_user_base_buy_usdc() {
        let mut cfg = pairlock_first_buy_config();
        cfg.base_buy_usdc = 2.0;
        cfg.min_marketable_buy_usdc = 1.05;

        let candidate = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &TradeBuilderPositiveQuantityFlipGridState::default(),
            quote_with_side_ask("up", 0.55),
            false,
            Some(&order_book(&[(0.55, 20.0)])),
        )
        .expect("candidate");

        assert!(candidate.actual_buy_usdc >= 1.99);
        assert!(candidate.actual_buy_usdc <= 2.01);
    }

    #[test]
    fn pairlock_first_buy_respects_1_5_not_1_05_floor() {
        let mut cfg = pairlock_first_buy_config();
        cfg.base_buy_usdc = 1.5;
        cfg.min_marketable_buy_usdc = 1.05;

        let candidate = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &TradeBuilderPositiveQuantityFlipGridState::default(),
            quote_with_side_ask("down", 0.55),
            false,
            Some(&order_book(&[(0.55, 20.0)])),
        )
        .expect("candidate");

        assert!(candidate.actual_buy_usdc >= 1.49);
        assert!(candidate.actual_buy_usdc <= 1.51);
    }

    #[test]
    fn pairlock_first_buy_bumps_qty_only_below_min() {
        let mut cfg = pairlock_first_buy_config();
        cfg.base_buy_usdc = 0.80;
        cfg.min_marketable_buy_usdc = 1.05;

        let candidate = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &TradeBuilderPositiveQuantityFlipGridState::default(),
            quote_with_side_ask("up", 0.50),
            false,
            Some(&order_book(&[(0.50, 20.0)])),
        )
        .expect("candidate");

        assert!(candidate.actual_buy_usdc >= 1.05);
        assert!(candidate.target_qty * 0.50 >= 1.05);
    }

    #[test]
    fn pairlock_flip_buy_still_uses_profit_target() {
        let mut cfg = pairlock_first_buy_config();
        cfg.base_buy_usdc = 1.0;
        cfg.min_marketable_buy_usdc = 1.05;
        cfg.min_positive_profit_usdc = 0.50;
        let state = TradeBuilderPositiveQuantityFlipGridState {
            buy_count: 1,
            total_buy_cost: 1.05,
            net_cost: 1.05,
            up_qty: 1.91,
            last_buy_grid_side: Some("up".to_string()),
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };

        let candidate = positive_quantity_flip_grid_buy_candidate(
            &cfg,
            &state,
            quote_with_side_ask("down", 0.53),
            false,
            Some(&order_book(&[(0.53, 20.0)])),
        )
        .expect("candidate");

        assert!(candidate.actual_buy_usdc > 1.05);
    }

    #[test]
    fn balance_failure_on_needed_side_blocks_more_overweight_buys() {
        let mut cfg = test_config();
        cfg.block_consecutive_same_side_buys = false;
        let state = TradeBuilderPositiveQuantityFlipGridState {
            buy_count: 5,
            up_qty: 7.17,
            down_qty: 12.95,
            total_buy_cost: 10.37,
            net_cost: 10.37,
            last_balance_failure_order_id: Some(18280),
            last_balance_failure_grid_side: Some("up".to_string()),
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };

        let evaluation = positive_quantity_flip_grid_evaluate_buy_candidate(
            &cfg,
            &state,
            quote_with_side_ask("down", 0.39),
            false,
            Some(&order_book(&[(0.39, 20.0)])),
        );

        assert!(evaluation.candidate.is_none());
        assert_eq!(
            evaluation.report["guard_reason"],
            "balance_blocked_counter_completion"
        );
        assert_eq!(evaluation.report["guard_details"]["blocked_side"], "down");
        assert_eq!(evaluation.report["guard_details"]["needed_side"], "up");
        assert_eq!(
            evaluation.report["guard_details"]["last_balance_failure_order_id"],
            18280
        );
    }

    #[tokio::test]
    async fn selection_prefers_inventory_needed_side_when_both_candidates_pass() {
        let mut cfg = test_config();
        cfg.depth_guard_enabled = false;
        cfg.block_consecutive_same_side_buys = false;
        let state = TradeBuilderPositiveQuantityFlipGridState {
            buy_count: 2,
            up_qty: 2.0,
            down_qty: 4.0,
            total_buy_cost: 3.0,
            net_cost: 3.0,
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };
        let quotes = vec![
            quote_with_side_ask("down", 0.50),
            quote_with_side_ask("up", 0.58),
        ];

        let selection = positive_quantity_flip_grid_select_buy_candidate(
            &cfg,
            &state,
            &test_step(None),
            "btc-updown-1",
            &quotes,
            false,
            &SelectionTestExecutor,
        )
        .await
        .expect("selection");

        assert_eq!(
            selection.candidate.expect("candidate").quote.grid_side,
            "up"
        );
    }

    #[test]
    fn pairlock_compression_buy_ignores_normal_base_buy_usdc() {
        let mut cfg = test_config();
        cfg.base_buy_usdc = 1.50;
        cfg.pairlock_compression_enabled = true;
        cfg.max_total_spent_per_market_usdc = Some(5.0);
        cfg.max_unmerged_exposure_usdc = 5.0;
        cfg.fee_buffer = 0.01;
        cfg.target_pairlock_profit = 0.05;
        cfg.max_pair_cost = 0.94;

        let opportunity = positive_flip_pairlock_find_buy_opportunity(
            &[lot(11, "down", 2.33, 0.53)],
            &[quote_with_side_ask("up", 0.34)],
            &TradeBuilderPositiveQuantityFlipGridState::default(),
            &cfg,
        )
        .expect("opportunity");

        assert!(opportunity.min_notional_top_up_applied);
        assert!(opportunity.candidate.actual_buy_usdc >= 1.0);
        assert!(opportunity.candidate.actual_buy_usdc < 1.10);
    }

    #[test]
    fn submit_time_share_buy_tops_up_when_price_improves_below_one_usdc() {
        let top_up = trade_builder_marketable_buy_min_notional_top_up(
            "buy",
            "FAK",
            TRADE_BUILDER_SIZE_BASIS_SHARES,
            0.49,
            2.02,
            1.03,
        )
        .expect("top up");

        assert!(!top_up.blocked_by_cap);
        assert_eq!(top_up.adjusted_qty, 2.05);
        assert!(top_up.adjusted_notional_usdc >= 1.0);
    }

    #[test]
    fn submit_time_share_buy_blocks_when_top_up_exceeds_cap() {
        let top_up = trade_builder_marketable_buy_min_notional_top_up(
            "buy",
            "FAK",
            TRADE_BUILDER_SIZE_BASIS_SHARES,
            0.49,
            2.02,
            0.98,
        )
        .expect("top up");

        assert!(top_up.blocked_by_cap);
    }
}
