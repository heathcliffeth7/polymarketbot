#[cfg(test)]
mod positive_quantity_flip_grid_test_helpers {
    use super::*;

    pub(crate) fn test_config() -> PositiveQuantityFlipGridConfig {
        PositiveQuantityFlipGridConfig {
            base_buy_usdc: 1.0,
            min_marketable_buy_usdc: POSITIVE_QUANTITY_FLIP_GRID_MIN_MARKETABLE_BUY_USDC,
            entry_band_min_cent: 50.0,
            entry_band_max_cent: 60.0,
            preferred_trigger_cent: 53.0,
            trigger_tolerance_cent: 3.0,
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
            max_single_buy_usdc: Some(2.2),
            max_total_spent_per_market_usdc: Some(9.5),
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

    pub(crate) fn quote_with_ask(ask: f64) -> PositiveQuantityFlipGridQuote {
        quote_with_side_ask("down", ask)
    }

    pub(crate) fn quote_with_side_ask(grid_side: &'static str, ask: f64) -> PositiveQuantityFlipGridQuote {
        PositiveQuantityFlipGridQuote {
            grid_side,
            token_id: grid_side.to_string(),
            outcome_label: if grid_side == "up" { "Up" } else { "Down" }.to_string(),
            best_bid: Some(1.0 - ask),
            best_ask: Some(ask),
            quote_snapshot: json!({}),
        }
    }

    pub(crate) fn quote_with_side_bid(grid_side: &'static str, bid: f64) -> PositiveQuantityFlipGridQuote {
        PositiveQuantityFlipGridQuote {
            grid_side,
            token_id: grid_side.to_string(),
            outcome_label: if grid_side == "up" { "Up" } else { "Down" }.to_string(),
            best_bid: Some(bid),
            best_ask: Some((bid + 0.01).min(1.0)),
            quote_snapshot: json!({}),
        }
    }

    pub(crate) fn order_book(asks: &[(f64, f64)]) -> OrderBookSnapshot {
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

    pub(crate) fn positive_grid_node(grid: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "positive_grid".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({
                "mode": ACTION_PLACE_ORDER_MODE_POSITIVE_QUANTITY_FLIP_GRID_V1,
                "positiveQuantityFlipGrid": grid,
            }),
        }
    }

    pub(crate) fn positive_flip_pairlock_compression_node(grid: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "positive_flip_pairlock_compression".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({
                "mode": ACTION_PLACE_ORDER_MODE_POSITIVE_FLIP_PAIRLOCK_COMPRESSION_V1,
                "positiveQuantityFlipGrid": grid,
            }),
        }
    }

    pub(crate) fn pairlock_config() -> PositiveQuantityFlipGridConfig {
        let mut cfg = test_config();
        cfg.pairlock_compression_enabled = true;
        cfg.stop_buys_after_pairlock_merge = true;
        cfg.target_pairlock_profit = 0.05;
        cfg.fee_buffer = 0.0;
        cfg.max_pair_cost = 0.95;
        cfg.pairlock_order_type = "FOK";
        cfg.max_unmerged_exposure_usdc = 2.0;
        cfg.max_total_spent_per_market_usdc = Some(2.5);
        cfg
    }

    pub(crate) fn lot(
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

    pub(crate) fn test_step(input_json: Option<Value>) -> TradeFlowRunStep {
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

}
