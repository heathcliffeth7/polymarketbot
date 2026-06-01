#[derive(Debug, Clone)]
struct PositiveQuantityFlipGridCandidateEvaluation {
    candidate: Option<PositiveQuantityFlipGridBuyCandidate>,
    report: Value,
}

#[derive(Debug, Clone)]
struct PositiveQuantityFlipGridSelection {
    candidate: Option<PositiveQuantityFlipGridBuyCandidate>,
    guard_reports: Vec<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositiveQuantityFlipGridQuantitySizingMode {
    ProfitTarget,
    InventoryBalance,
    FixedUsdc,
}

impl PositiveQuantityFlipGridQuantitySizingMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::ProfitTarget => "profit_target",
            Self::InventoryBalance => "inventory_balance",
            Self::FixedUsdc => "fixed_usdc",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PositiveQuantityFlipGridPriceGate {
    ask_price: f64,
    ask_cent: f64,
    current_side_qty: f64,
    pre_pnl_at_exit: f64,
    max_buy_price_cent: f64,
    rescue_buy: bool,
}

fn positive_quantity_flip_grid_optional_f64(
    map: &serde_json::Map<String, Value>,
    node: &TradeFlowNode,
    key: &str,
) -> Option<f64> {
    positive_quantity_flip_grid_value(map, node, key).and_then(value_as_f64)
}

fn positive_quantity_flip_grid_parse_ptb_diff_unit(
    map: &serde_json::Map<String, Value>,
    node: &TradeFlowNode,
    enabled: bool,
) -> Result<crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit> {
    let raw = positive_quantity_flip_grid_string(map, node, "ptbDiffUnit");
    match crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::parse(raw.as_deref()) {
        Some(unit) => Ok(unit),
        None if enabled => Err(anyhow::anyhow!(
            "positiveQuantityFlipGrid ptbDiffUnit must be usd or cent"
        )),
        None => Ok(crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Usd),
    }
}

fn positive_quantity_flip_grid_parse_quantity_sizing_mode(
    map: &serde_json::Map<String, Value>,
    node: &TradeFlowNode,
) -> Result<PositiveQuantityFlipGridQuantitySizingMode> {
    let raw = positive_quantity_flip_grid_string(map, node, "quantitySizingMode")
        .unwrap_or_else(|| "profit_target".to_string());
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "profit_target" => Ok(PositiveQuantityFlipGridQuantitySizingMode::ProfitTarget),
        "inventory_balance" => Ok(PositiveQuantityFlipGridQuantitySizingMode::InventoryBalance),
        "fixed_usdc" => Ok(PositiveQuantityFlipGridQuantitySizingMode::FixedUsdc),
        _ => Err(anyhow::anyhow!(
            "positiveQuantityFlipGrid quantitySizingMode must be profit_target, inventory_balance, or fixed_usdc"
        )),
    }
}

fn positive_quantity_flip_grid_parse_ptb_current_price_source(
    map: &serde_json::Map<String, Value>,
    node: &TradeFlowNode,
    enabled: bool,
) -> Result<crate::trade_flow::guards::price_to_beat::PriceToBeatCurrentPriceSource> {
    let raw = positive_quantity_flip_grid_string(map, node, "ptbCurrentPriceSource")
        .unwrap_or_else(|| "chainlink".to_string());
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "chainlink" => {
            Ok(crate::trade_flow::guards::price_to_beat::PriceToBeatCurrentPriceSource::Chainlink)
        }
        "binance" => {
            Ok(crate::trade_flow::guards::price_to_beat::PriceToBeatCurrentPriceSource::Binance)
        }
        "coinbase" => {
            Ok(crate::trade_flow::guards::price_to_beat::PriceToBeatCurrentPriceSource::Coinbase)
        }
        "hyperliquid" => Ok(
            crate::trade_flow::guards::price_to_beat::PriceToBeatCurrentPriceSource::Hyperliquid,
        ),
        _ if enabled => Err(anyhow::anyhow!(
            "positiveQuantityFlipGrid ptbCurrentPriceSource must be chainlink, binance, coinbase, or hyperliquid"
        )),
        _ => Ok(crate::trade_flow::guards::price_to_beat::PriceToBeatCurrentPriceSource::Chainlink),
    }
}

fn positive_quantity_flip_grid_execution_floor_price_cent(
    config: &PositiveQuantityFlipGridConfig,
) -> f64 {
    config
        .execution_floor_price_cent
        .unwrap_or(config.entry_band_min_cent)
}

fn positive_quantity_flip_grid_quote_guard_report(
    quote: &PositiveQuantityFlipGridQuote,
    blocked_by: Option<&str>,
    guard_reason: &str,
    guard_details: Value,
) -> Value {
    json!({
        "grid_side": quote.grid_side,
        "token_id": quote.token_id,
        "outcome_label": quote.outcome_label,
        "best_bid": quote.best_bid,
        "best_ask": quote.best_ask,
        "blocked_by": blocked_by,
        "guard_reason": guard_reason,
        "guard_details": guard_details,
        "quote": quote.quote_snapshot,
    })
}

fn positive_quantity_flip_grid_blocked_candidate(
    quote: &PositiveQuantityFlipGridQuote,
    blocked_by: &str,
    guard_reason: &str,
    guard_details: Value,
) -> PositiveQuantityFlipGridCandidateEvaluation {
    PositiveQuantityFlipGridCandidateEvaluation {
        candidate: None,
        report: positive_quantity_flip_grid_quote_guard_report(
            quote,
            Some(blocked_by),
            guard_reason,
            guard_details,
        ),
    }
}

fn positive_quantity_flip_grid_opposite_side_qty(
    state: &TradeBuilderPositiveQuantityFlipGridState,
    grid_side: &str,
) -> f64 {
    if grid_side == "down" {
        state.up_qty
    } else {
        state.down_qty
    }
}

fn positive_quantity_flip_grid_needed_side(
    state: &TradeBuilderPositiveQuantityFlipGridState,
) -> Option<&'static str> {
    if state.up_qty + 0.000001 < state.down_qty {
        Some("up")
    } else if state.down_qty + 0.000001 < state.up_qty {
        Some("down")
    } else {
        None
    }
}

fn positive_quantity_flip_grid_overweight_side(
    state: &TradeBuilderPositiveQuantityFlipGridState,
) -> Option<&'static str> {
    match positive_quantity_flip_grid_needed_side(state) {
        Some("up") => Some("down"),
        Some("down") => Some("up"),
        _ => None,
    }
}

fn positive_quantity_flip_grid_inventory_need_rank(
    state: &TradeBuilderPositiveQuantityFlipGridState,
    grid_side: &str,
) -> i32 {
    match positive_quantity_flip_grid_needed_side(state) {
        Some(needed_side) if needed_side == grid_side => 0,
        Some(_) => 1,
        None => 0,
    }
}

fn positive_quantity_flip_grid_rescue_buy_eligible(
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
    grid_side: &str,
    ask_price: f64,
) -> bool {
    if !config.rescue_buy_enabled || state.buy_count == 0 {
        return false;
    }
    let ask_cent = ask_price * 100.0;
    let rescue_min_cent = config.rescue_buy_min_price_cent;
    let rescue_max_cent = config.rescue_buy_max_price_cent;
    if ask_cent <= rescue_min_cent + 0.000001 || ask_cent > rescue_max_cent + 0.000001 {
        return false;
    }
    let current_side_qty = positive_quantity_flip_grid_side_qty(state, grid_side);
    let pre_pnl_at_exit = positive_quantity_flip_grid_pnl_at_exit(
        current_side_qty,
        config.exit_price_for_sizing,
        state.net_cost,
    );
    pre_pnl_at_exit + 0.000001 < config.min_positive_profit_usdc
}

fn positive_quantity_flip_grid_ptb_threshold_for_quote(
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
    grid_side: &str,
    ask_price: f64,
) -> (f64, &'static str) {
    if positive_quantity_flip_grid_rescue_buy_eligible(config, state, grid_side, ask_price) {
        (
            config.ptb_rescue_min_diff.unwrap_or(config.ptb_min_diff),
            "rescue",
        )
    } else {
        (config.ptb_min_diff, "normal")
    }
}

fn positive_quantity_flip_grid_matching_no_buy_range<'a>(
    config: &'a PositiveQuantityFlipGridConfig,
    ask_cent: f64,
) -> Option<&'a PositiveQuantityFlipGridNoBuyRange> {
    config.no_buy_ranges.iter().find(|range| {
        ask_cent + 0.000001 >= range.min_cent && ask_cent <= range.max_cent + 0.000001
    })
}

fn positive_quantity_flip_grid_price_gate(
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
    quote: &PositiveQuantityFlipGridQuote,
) -> std::result::Result<PositiveQuantityFlipGridPriceGate, Value> {
    let Some(ask_price) = quote.best_ask else {
        return Err(positive_quantity_flip_grid_quote_guard_report(
            quote,
            Some("quote"),
            "best_ask_unavailable",
            json!({ "best_ask": quote.best_ask }),
        ));
    };
    if config.block_consecutive_same_side_buys
        && state.last_buy_grid_side.as_deref() == Some(quote.grid_side)
    {
        return Err(positive_quantity_flip_grid_quote_guard_report(
            quote,
            Some("same_side_rebuy"),
            "consecutive_same_side_buy",
            json!({
                "last_buy_grid_side": state.last_buy_grid_side.as_deref(),
                "candidate_grid_side": quote.grid_side,
            }),
        ));
    }
    if let (Some(needed_side), Some(overweight_side), Some(failed_side)) = (
        positive_quantity_flip_grid_needed_side(state),
        positive_quantity_flip_grid_overweight_side(state),
        state.last_balance_failure_grid_side.as_deref(),
    ) {
        if failed_side == needed_side && quote.grid_side == overweight_side {
            return Err(positive_quantity_flip_grid_quote_guard_report(
                quote,
                Some("balance_fail_imbalance"),
                "balance_blocked_counter_completion",
                json!({
                    "up_qty": state.up_qty,
                    "down_qty": state.down_qty,
                    "blocked_side": quote.grid_side,
                    "needed_side": needed_side,
                    "last_balance_failure_order_id": state.last_balance_failure_order_id,
                    "last_balance_failure_grid_side": failed_side,
                    "reason": "needed_side_balance_or_allowance_failed",
                }),
            ));
        }
    }
    let ask_cent = ask_price * 100.0;
    let current_side_qty = positive_quantity_flip_grid_side_qty(state, quote.grid_side);
    let pre_pnl_at_exit = positive_quantity_flip_grid_pnl_at_exit(
        current_side_qty,
        config.exit_price_for_sizing,
        state.net_cost,
    );
    let side_needs_rescue = pre_pnl_at_exit + 0.000001 < config.min_positive_profit_usdc;
    let rescue_min_cent = config.rescue_buy_min_price_cent;
    let rescue_max_cent = config.rescue_buy_max_price_cent;
    let has_existing_buy = state.buy_count > 0;
    let rescue_buy = positive_quantity_flip_grid_rescue_buy_eligible(
        config,
        state,
        quote.grid_side,
        ask_price,
    );

    if ask_cent < config.entry_band_min_cent {
        return Err(positive_quantity_flip_grid_quote_guard_report(
            quote,
            Some("entry_band"),
            "below_entry_band",
            json!({
                "ask_cent": ask_cent,
                "entry_band_min_cent": config.entry_band_min_cent,
                "entry_band_max_cent": config.entry_band_max_cent,
            }),
        ));
    }
    if ask_cent > config.entry_band_max_cent && !rescue_buy {
        let (blocked_by, guard_reason, details) =
            if config.rescue_buy_enabled && ask_cent > config.entry_band_max_cent {
                if ask_cent > rescue_max_cent + 0.000001 {
                    (
                        "max_price",
                        "above_rescue_max_price",
                        json!({
                            "ask_cent": ask_cent,
                            "hard_max_price_cent": config.hard_max_price_cent,
                            "rescue_buy_min_price_cent": rescue_min_cent,
                            "rescue_buy_max_price_cent": rescue_max_cent,
                        }),
                    )
                } else if ask_cent <= rescue_min_cent + 0.000001 {
                    (
                        "rescue",
                        "below_rescue_range",
                        json!({
                            "ask_cent": ask_cent,
                            "entry_band_max_cent": config.entry_band_max_cent,
                            "rescue_buy_min_price_cent": rescue_min_cent,
                            "rescue_buy_max_price_cent": rescue_max_cent,
                        }),
                    )
                } else if !side_needs_rescue {
                    (
                        "rescue",
                        "rescue_side_already_positive",
                        json!({
                            "ask_cent": ask_cent,
                            "pre_pnl_at_exit": pre_pnl_at_exit,
                            "min_positive_profit_usdc": config.min_positive_profit_usdc,
                        }),
                    )
                } else if !has_existing_buy {
                    (
                        "rescue",
                        "rescue_requires_existing_buy",
                        json!({
                            "ask_cent": ask_cent,
                            "buy_count": state.buy_count,
                            "entry_band_min_cent": config.entry_band_min_cent,
                            "entry_band_max_cent": config.entry_band_max_cent,
                            "rescue_buy_min_price_cent": rescue_min_cent,
                            "rescue_buy_max_price_cent": rescue_max_cent,
                        }),
                    )
                } else {
                    (
                        "entry_band",
                        "above_entry_band",
                        json!({
                            "ask_cent": ask_cent,
                            "entry_band_min_cent": config.entry_band_min_cent,
                            "entry_band_max_cent": config.entry_band_max_cent,
                        }),
                    )
                }
            } else {
                (
                    "entry_band",
                    "above_entry_band",
                    json!({
                        "ask_cent": ask_cent,
                        "entry_band_min_cent": config.entry_band_min_cent,
                        "entry_band_max_cent": config.entry_band_max_cent,
                    }),
                )
            };
        return Err(positive_quantity_flip_grid_quote_guard_report(
            quote,
            Some(blocked_by),
            guard_reason,
            details,
        ));
    }
    if ask_cent > config.hard_max_price_cent && !rescue_buy {
        let (guard_reason, details) = if config.rescue_buy_enabled {
            if ask_cent > rescue_max_cent + 0.000001 {
                (
                    "above_rescue_max_price",
                    json!({
                        "ask_cent": ask_cent,
                        "hard_max_price_cent": config.hard_max_price_cent,
                        "rescue_buy_min_price_cent": rescue_min_cent,
                        "rescue_buy_max_price_cent": rescue_max_cent,
                    }),
                )
            } else if ask_cent <= rescue_min_cent + 0.000001 {
                (
                    "below_rescue_range",
                    json!({
                        "ask_cent": ask_cent,
                        "hard_max_price_cent": config.hard_max_price_cent,
                        "rescue_buy_min_price_cent": rescue_min_cent,
                        "rescue_buy_max_price_cent": rescue_max_cent,
                    }),
                )
            } else if !has_existing_buy {
                (
                    "rescue_requires_existing_buy",
                    json!({
                        "ask_cent": ask_cent,
                        "buy_count": state.buy_count,
                        "hard_max_price_cent": config.hard_max_price_cent,
                        "rescue_buy_min_price_cent": rescue_min_cent,
                        "rescue_buy_max_price_cent": rescue_max_cent,
                    }),
                )
            } else {
                (
                    "rescue_side_already_positive",
                    json!({
                        "ask_cent": ask_cent,
                        "pre_pnl_at_exit": pre_pnl_at_exit,
                        "min_positive_profit_usdc": config.min_positive_profit_usdc,
                    }),
                )
            }
        } else {
            (
                "above_hard_max_price",
                json!({
                    "ask_cent": ask_cent,
                    "hard_max_price_cent": config.hard_max_price_cent,
                }),
            )
        };
        return Err(positive_quantity_flip_grid_quote_guard_report(
            quote,
            Some("max_price"),
            guard_reason,
            details,
        ));
    }
    if ask_cent > config.worst_price_cent && !rescue_buy {
        return Err(positive_quantity_flip_grid_quote_guard_report(
            quote,
            Some("max_price"),
            "above_worst_price",
            json!({
                "ask_cent": ask_cent,
                "worst_price_cent": config.worst_price_cent,
            }),
        ));
    }
    if let Some(range) = positive_quantity_flip_grid_matching_no_buy_range(config, ask_cent) {
        return Err(positive_quantity_flip_grid_quote_guard_report(
            quote,
            Some("no_buy_range"),
            "inside_no_buy_range",
            json!({
                "ask_cent": ask_cent,
                "range": {
                    "min_cent": range.min_cent,
                    "max_cent": range.max_cent,
                },
            }),
        ));
    }
    if config.execution_floor_guard_enabled {
        let floor_cent = positive_quantity_flip_grid_execution_floor_price_cent(config);
        if ask_cent + 0.000001 < floor_cent {
            return Err(positive_quantity_flip_grid_quote_guard_report(
                quote,
                Some("execution_floor"),
                "below_execution_floor",
                json!({
                    "ask_cent": ask_cent,
                    "execution_floor_price_cent": floor_cent,
                }),
            ));
        }
    }
    Ok(PositiveQuantityFlipGridPriceGate {
        ask_price,
        ask_cent,
        current_side_qty,
        pre_pnl_at_exit,
        max_buy_price_cent: if rescue_buy {
            rescue_max_cent
        } else {
            config.worst_price_cent
        },
        rescue_buy,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
fn positive_quantity_flip_grid_evaluate_buy_candidate(
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
    quote: PositiveQuantityFlipGridQuote,
    completion_only: bool,
    order_book: Option<&OrderBookSnapshot>,
) -> PositiveQuantityFlipGridCandidateEvaluation {
    positive_quantity_flip_grid_evaluate_buy_candidate_with_balance(
        config,
        state,
        quote,
        completion_only,
        order_book,
        None,
    )
}

fn positive_quantity_flip_grid_evaluate_buy_candidate_with_balance(
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
    quote: PositiveQuantityFlipGridQuote,
    completion_only: bool,
    order_book: Option<&OrderBookSnapshot>,
    available_collateral_usdc: Option<f64>,
) -> PositiveQuantityFlipGridCandidateEvaluation {
    let gate = match positive_quantity_flip_grid_price_gate(config, state, &quote) {
        Ok(gate) => gate,
        Err(report) => {
            return PositiveQuantityFlipGridCandidateEvaluation {
                candidate: None,
                report,
            };
        }
    };
    let ask_price = gate.ask_price;
    let ask_cent = gate.ask_cent;
    let current_side_qty = gate.current_side_qty;
    let pre_pnl_at_exit = gate.pre_pnl_at_exit;
    let worst_price = gate.max_buy_price_cent / 100.0;
    let rescue_buy = gate.rescue_buy;
    let mut effective_ask_price = ask_price;
    let mut final_depth = None;
    let mut final_required_buy_usdc = 0.0;
    let mut final_actual_buy_usdc = 0.0;
    let mut final_target_qty = 0.0;
    let mut final_sizing_ask_price = effective_ask_price;
    let mut final_formula_target_qty = 0.0;
    let mut final_min_marketable_target_qty = 0.0;
    let mut final_quantity_sizing_details = json!({});
    for _ in 0..3 {
        let sizing_ask_price =
            (effective_ask_price + config.sizing_price_buffer_cent / 100.0).min(worst_price);
        let min_marketable_target_qty = positive_quantity_flip_grid_round_up_share_qty(
            config.min_marketable_buy_usdc / ask_price,
        );
        let remaining_market_budget_usdc = config
            .max_total_spent_per_market_usdc
            .map(|cap| (cap - state.total_buy_cost).max(0.0))
            .unwrap_or(f64::MAX);
        let max_candidate_buy_usdc = config
            .max_single_buy_usdc
            .map(|cap| cap.min(remaining_market_budget_usdc))
            .unwrap_or(remaining_market_budget_usdc);
        let pairlock_first_buy =
            config.pairlock_compression_enabled && state.buy_count == 0;
        let (
            required_buy_usdc,
            requested_buy_usdc,
            formula_target_qty,
            target_qty,
            quantity_sizing_details,
        ) = match config.quantity_sizing_mode {
            PositiveQuantityFlipGridQuantitySizingMode::ProfitTarget => {
                if pairlock_first_buy {
                    let requested_buy_usdc =
                        positive_quantity_flip_grid_round_up_cent(config.base_buy_usdc);
                    let formula_target_qty = positive_quantity_flip_grid_round_up_share_qty(
                        requested_buy_usdc / sizing_ask_price,
                    );
                    (
                        0.0,
                        requested_buy_usdc,
                        formula_target_qty,
                        formula_target_qty.max(min_marketable_target_qty),
                        json!({
                            "mode": config.quantity_sizing_mode.as_str(),
                            "pairlock_first_buy": true,
                            "base_buy_usdc": config.base_buy_usdc,
                        }),
                    )
                } else {
                    let Some(required_buy_usdc) = positive_quantity_flip_grid_required_buy_usdc(
                        state.net_cost,
                        config.min_positive_profit_usdc,
                        config.exit_price_for_sizing,
                        current_side_qty,
                        sizing_ask_price,
                    ) else {
                        return positive_quantity_flip_grid_blocked_candidate(
                            &quote,
                            "sizing",
                            "required_buy_unavailable",
                            json!({
                                "quantity_sizing_mode": config.quantity_sizing_mode.as_str(),
                                "net_cost": state.net_cost,
                                "min_positive_profit_usdc": config.min_positive_profit_usdc,
                                "exit_price_for_sizing": config.exit_price_for_sizing,
                                "current_side_qty": current_side_qty,
                                "effective_ask_price": effective_ask_price,
                                "sizing_ask_price": sizing_ask_price,
                                "sizing_price_buffer_cent": config.sizing_price_buffer_cent,
                            }),
                        );
                    };
                    let requested_buy_usdc = positive_quantity_flip_grid_round_up_cent(
                        config
                            .base_buy_usdc
                            .max(config.min_marketable_buy_usdc)
                            .max(required_buy_usdc),
                    );
                    let formula_target_qty = positive_quantity_flip_grid_round_up_share_qty(
                        requested_buy_usdc / sizing_ask_price,
                    );
                    (
                        required_buy_usdc,
                        requested_buy_usdc,
                        formula_target_qty,
                        formula_target_qty.max(min_marketable_target_qty),
                        json!({ "mode": config.quantity_sizing_mode.as_str() }),
                    )
                }
            }
            PositiveQuantityFlipGridQuantitySizingMode::InventoryBalance => {
                let opposite_side_qty =
                    positive_quantity_flip_grid_opposite_side_qty(state, quote.grid_side);
                let desired_side_qty = opposite_side_qty + config.inventory_balance_lead_qty;
                let inventory_gap_qty = (desired_side_qty - current_side_qty).max(0.0);
                let initial_inventory_buy =
                    state.buy_count == 0 && state.up_qty <= 0.000001 && state.down_qty <= 0.000001;
                if !initial_inventory_buy && inventory_gap_qty <= 0.000001 {
                    return positive_quantity_flip_grid_blocked_candidate(
                        &quote,
                        "sizing",
                        "inventory_side_already_balanced",
                        json!({
                            "quantity_sizing_mode": config.quantity_sizing_mode.as_str(),
                            "current_side_qty": current_side_qty,
                            "opposite_side_qty": opposite_side_qty,
                            "inventory_balance_lead_qty": config.inventory_balance_lead_qty,
                            "desired_side_qty": desired_side_qty,
                        }),
                    );
                }
                let affordable_target_qty =
                    floor_trade_builder_share_qty(max_candidate_buy_usdc / sizing_ask_price);
                if affordable_target_qty + 0.000001 < min_marketable_target_qty {
                    return positive_quantity_flip_grid_blocked_candidate(
                        &quote,
                        "risk",
                        "inventory_budget_below_min_marketable",
                        json!({
                            "quantity_sizing_mode": config.quantity_sizing_mode.as_str(),
                            "max_candidate_buy_usdc": max_candidate_buy_usdc,
                            "remaining_market_budget_usdc": remaining_market_budget_usdc,
                            "sizing_ask_price": sizing_ask_price,
                            "affordable_target_qty": affordable_target_qty,
                            "min_marketable_target_qty": min_marketable_target_qty,
                            "min_marketable_buy_usdc": config.min_marketable_buy_usdc,
                        }),
                    );
                }
                let uncapped_target_qty = if initial_inventory_buy {
                    let base_qty = positive_quantity_flip_grid_round_up_share_qty(
                        config.base_buy_usdc / sizing_ask_price,
                    );
                    if pairlock_first_buy {
                        base_qty.max(min_marketable_target_qty)
                    } else {
                        positive_quantity_flip_grid_round_up_share_qty(
                            config.base_buy_usdc.max(config.min_marketable_buy_usdc)
                                / sizing_ask_price,
                        )
                        .max(min_marketable_target_qty)
                    }
                } else {
                    positive_quantity_flip_grid_round_up_share_qty(inventory_gap_qty)
                }
                .max(min_marketable_target_qty);
                let target_qty = uncapped_target_qty.min(affordable_target_qty);
                let requested_buy_usdc =
                    positive_quantity_flip_grid_round_up_cent(target_qty * sizing_ask_price);
                (
                    inventory_gap_qty * sizing_ask_price,
                    requested_buy_usdc,
                    uncapped_target_qty,
                    target_qty,
                    json!({
                        "mode": config.quantity_sizing_mode.as_str(),
                        "current_side_qty": current_side_qty,
                        "opposite_side_qty": opposite_side_qty,
                        "inventory_balance_lead_qty": config.inventory_balance_lead_qty,
                        "desired_side_qty": desired_side_qty,
                        "inventory_gap_qty": inventory_gap_qty,
                        "initial_inventory_buy": initial_inventory_buy,
                        "max_candidate_buy_usdc": max_candidate_buy_usdc,
                        "remaining_market_budget_usdc": remaining_market_budget_usdc,
                        "affordable_target_qty": affordable_target_qty,
                        "uncapped_target_qty": uncapped_target_qty,
                        "budget_capped": target_qty + 0.000001 < uncapped_target_qty,
                    }),
                )
            }
            PositiveQuantityFlipGridQuantitySizingMode::FixedUsdc => {
                let requested_buy_usdc = positive_quantity_flip_grid_round_up_cent(
                    config
                        .base_buy_usdc
                        .max(config.min_marketable_buy_usdc),
                );
                let formula_target_qty = positive_quantity_flip_grid_round_up_share_qty(
                    requested_buy_usdc / sizing_ask_price,
                );
                (
                    0.0,
                    requested_buy_usdc,
                    formula_target_qty,
                    formula_target_qty.max(min_marketable_target_qty),
                    json!({
                        "mode": config.quantity_sizing_mode.as_str(),
                        "base_buy_usdc": config.base_buy_usdc,
                        "min_marketable_buy_usdc": config.min_marketable_buy_usdc,
                    }),
                )
            }
        };
        if target_qty <= 0.0 {
            return positive_quantity_flip_grid_blocked_candidate(
                &quote,
                "sizing",
                "target_qty_invalid",
                json!({
                    "quantity_sizing_mode": config.quantity_sizing_mode.as_str(),
                    "requested_buy_usdc": requested_buy_usdc,
                    "effective_ask_price": effective_ask_price,
                    "sizing_ask_price": sizing_ask_price,
                    "sizing_price_buffer_cent": config.sizing_price_buffer_cent,
                    "formula_target_qty": formula_target_qty,
                    "min_marketable_target_qty": min_marketable_target_qty,
                    "min_marketable_buy_usdc": config.min_marketable_buy_usdc,
                }),
            );
        }
        let depth = positive_quantity_flip_grid_evaluate_depth(
            config.depth_guard_enabled,
            order_book,
            ask_price,
            target_qty,
            worst_price,
        );
        if depth.blocked {
            let depth_reason = depth
                .payload
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("depth_guard_blocked")
                .to_string();
            return positive_quantity_flip_grid_blocked_candidate(
                &quote,
                "depth",
                &depth_reason,
                depth.payload,
            );
        }
        let depth_effective_price = depth.estimated_avg_fill.unwrap_or(effective_ask_price);
        if depth_effective_price > worst_price + 0.000001 {
            return positive_quantity_flip_grid_blocked_candidate(
                &quote,
                "max_price",
                "effective_fill_above_worst_price",
                json!({
                    "effective_ask_price": depth_effective_price,
                    "worst_price": worst_price,
                    "depth": depth.payload,
                }),
            );
        }
        let converged = (depth_effective_price - effective_ask_price).abs() <= 0.000001;
        let estimated_buy_usdc = target_qty * depth_effective_price;
        let actual_buy_usdc = estimated_buy_usdc.max(target_qty * sizing_ask_price);
        if let Some(max_single_buy_usdc) = config.max_single_buy_usdc {
            if actual_buy_usdc > max_single_buy_usdc + 0.000001 {
                let risk_details = json!({
                    "actual_buy_usdc": actual_buy_usdc,
                    "estimated_buy_usdc": estimated_buy_usdc,
                    "sizing_ask_price": sizing_ask_price,
                    "max_single_buy_usdc": max_single_buy_usdc,
                });
                if let Some(partial) = positive_quantity_flip_grid_try_partial_recovery(
                    config,
                    state,
                    &quote,
                    &gate,
                    completion_only,
                    order_book,
                    available_collateral_usdc,
                    requested_buy_usdc,
                    actual_buy_usdc,
                    depth_effective_price,
                    sizing_ask_price,
                    min_marketable_target_qty,
                    "max_single_buy_usdc",
                    risk_details.clone(),
                ) {
                    return partial;
                }
                return positive_quantity_flip_grid_blocked_candidate(
                    &quote,
                    "risk",
                    "max_single_buy_usdc",
                    risk_details,
                );
            }
        }
        if let Some(max_total_spent_per_market_usdc) = config.max_total_spent_per_market_usdc {
            if state.total_buy_cost + actual_buy_usdc
                > max_total_spent_per_market_usdc + 0.000001
            {
                let risk_details = json!({
                    "total_buy_cost": state.total_buy_cost,
                    "actual_buy_usdc": actual_buy_usdc,
                    "estimated_buy_usdc": estimated_buy_usdc,
                    "sizing_ask_price": sizing_ask_price,
                    "max_total_spent_per_market_usdc": max_total_spent_per_market_usdc,
                });
                if let Some(partial) = positive_quantity_flip_grid_try_partial_recovery(
                    config,
                    state,
                    &quote,
                    &gate,
                    completion_only,
                    order_book,
                    available_collateral_usdc,
                    requested_buy_usdc,
                    actual_buy_usdc,
                    depth_effective_price,
                    sizing_ask_price,
                    min_marketable_target_qty,
                    "max_total_spent_per_market_usdc",
                    risk_details.clone(),
                ) {
                    return partial;
                }
                return positive_quantity_flip_grid_blocked_candidate(
                    &quote,
                    "risk",
                    "max_total_spent_per_market_usdc",
                    risk_details,
                );
            }
        }
        final_depth = Some(depth);
        final_required_buy_usdc = required_buy_usdc;
        final_actual_buy_usdc = actual_buy_usdc;
        final_target_qty = target_qty;
        final_sizing_ask_price = sizing_ask_price;
        final_formula_target_qty = formula_target_qty;
        final_min_marketable_target_qty = min_marketable_target_qty;
        final_quantity_sizing_details = quantity_sizing_details;
        effective_ask_price = depth_effective_price;
        if converged {
            break;
        }
    }
    let Some(depth) = final_depth else {
        return positive_quantity_flip_grid_blocked_candidate(
            &quote,
            "depth",
            "depth_not_evaluated",
            json!({}),
        );
    };
    let actual_buy_usdc = final_actual_buy_usdc;
    let target_qty = final_target_qty;
    let projected_side_qty = current_side_qty + target_qty;
    let projected_net_cost = state.net_cost + actual_buy_usdc;
    let projected_pnl_at_exit = positive_quantity_flip_grid_pnl_at_exit(
        projected_side_qty,
        config.exit_price_for_sizing,
        projected_net_cost,
    );
    if config.quantity_sizing_mode == PositiveQuantityFlipGridQuantitySizingMode::ProfitTarget
        && projected_pnl_at_exit + 0.000001 < config.min_positive_profit_usdc
    {
        return positive_quantity_flip_grid_blocked_candidate(
            &quote,
            "sizing",
            "projected_profit_below_min",
            json!({
                "projected_pnl_at_exit": projected_pnl_at_exit,
                "min_positive_profit_usdc": config.min_positive_profit_usdc,
                "sizing_ask_price": final_sizing_ask_price,
                "sizing_price_buffer_cent": config.sizing_price_buffer_cent,
            }),
        );
    }
    if completion_only && pre_pnl_at_exit + 0.000001 >= config.min_positive_profit_usdc {
        return positive_quantity_flip_grid_blocked_candidate(
            &quote,
            "timing",
            "completion_only_side_already_positive",
            json!({
                "pre_pnl_at_exit": pre_pnl_at_exit,
                "min_positive_profit_usdc": config.min_positive_profit_usdc,
            }),
        );
    }
    let preferred_min = config.preferred_trigger_cent - config.trigger_tolerance_cent;
    let preferred_max = config.preferred_trigger_cent + config.trigger_tolerance_cent;
    let preferred_band_rank = if ask_cent >= preferred_min && ask_cent <= preferred_max {
        0
    } else {
        1
    };
    let report = positive_quantity_flip_grid_quote_guard_report(
        &quote,
        None,
        "passed",
        json!({
            "ask_cent": ask_cent,
            "effective_ask_price": effective_ask_price,
            "sizing_ask_price": final_sizing_ask_price,
            "sizing_price_buffer_cent": config.sizing_price_buffer_cent,
            "quantity_sizing_mode": config.quantity_sizing_mode.as_str(),
            "quantity_sizing_details": final_quantity_sizing_details,
            "actual_buy_usdc": actual_buy_usdc,
            "target_qty": target_qty,
            "formula_target_qty": final_formula_target_qty,
            "min_marketable_target_qty": final_min_marketable_target_qty,
            "min_marketable_buy_usdc": config.min_marketable_buy_usdc,
            "projected_pnl_at_exit": projected_pnl_at_exit,
            "rescue_buy": rescue_buy,
            "rescue_buy_min_price_cent": config.rescue_buy_min_price_cent,
            "max_buy_price_cent": gate.max_buy_price_cent,
            "depth": depth.payload.clone(),
        }),
    );
    PositiveQuantityFlipGridCandidateEvaluation {
        candidate: Some(PositiveQuantityFlipGridBuyCandidate {
            quote,
            ask_price,
            effective_ask_price,
            sizing_ask_price: final_sizing_ask_price,
            worst_price,
            required_buy_usdc: final_required_buy_usdc,
            actual_buy_usdc,
            target_qty,
            projected_side_qty,
            projected_net_cost,
            projected_pnl_at_exit,
            pre_pnl_at_exit,
            preferred_band_rank,
            depth_result: depth.payload,
            rescue_buy,
            partial_recovery: false,
            partial_recovery_details: None,
        }),
        report,
    }
}

#[allow(clippy::too_many_arguments)]
fn positive_quantity_flip_grid_try_partial_recovery(
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
    quote: &PositiveQuantityFlipGridQuote,
    gate: &PositiveQuantityFlipGridPriceGate,
    completion_only: bool,
    order_book: Option<&OrderBookSnapshot>,
    available_collateral_usdc: Option<f64>,
    requested_full_recovery_usdc: f64,
    full_actual_buy_usdc: f64,
    effective_ask_price: f64,
    sizing_ask_price: f64,
    min_marketable_target_qty: f64,
    risk_reason: &str,
    risk_details: Value,
) -> Option<PositiveQuantityFlipGridCandidateEvaluation> {
    if !config.partial_recovery_enabled
        || !completion_only
        || config.quantity_sizing_mode != PositiveQuantityFlipGridQuantitySizingMode::ProfitTarget
        || (risk_reason == "max_total_spent_per_market_usdc"
            && !config.partial_recovery_ignore_market_budget)
    {
        return None;
    }
    if gate.pre_pnl_at_exit + 0.000001 >= config.min_positive_profit_usdc {
        return Some(positive_quantity_flip_grid_blocked_candidate(
            quote,
            "timing",
            "completion_only_side_already_positive",
            json!({
                "partial_recovery": true,
                "pre_pnl_at_exit": gate.pre_pnl_at_exit,
                "min_positive_profit_usdc": config.min_positive_profit_usdc,
            }),
        ));
    }
    let Some(available_balance_usdc) =
        available_collateral_usdc.filter(|value| value.is_finite() && *value >= 0.0)
    else {
        return Some(positive_quantity_flip_grid_blocked_candidate(
            quote,
            "partial_recovery",
            "partial_recovery_balance_unavailable",
            json!({
                "partial_recovery": true,
                "requested_full_recovery_usdc": requested_full_recovery_usdc,
                "full_actual_buy_usdc": full_actual_buy_usdc,
                "risk_reason": risk_reason,
                "risk_details": risk_details,
            }),
        ));
    };
    let spendable_balance_usdc =
        (available_balance_usdc - config.partial_recovery_balance_reserve_usdc).max(0.0);
    let mut partial_buy_usdc = spendable_balance_usdc.min(full_actual_buy_usdc);
    if let Some(max_buy_usdc) = config.partial_recovery_max_buy_usdc {
        partial_buy_usdc = partial_buy_usdc.min(max_buy_usdc);
    }
    if partial_buy_usdc <= 0.000001 {
        return Some(positive_quantity_flip_grid_blocked_candidate(
            quote,
            "partial_recovery",
            "partial_recovery_budget_unavailable",
            json!({
                "partial_recovery": true,
                "available_balance_usdc": available_balance_usdc,
                "balance_reserve_usdc": config.partial_recovery_balance_reserve_usdc,
                "spendable_balance_usdc": spendable_balance_usdc,
                "requested_full_recovery_usdc": requested_full_recovery_usdc,
                "full_actual_buy_usdc": full_actual_buy_usdc,
            }),
        ));
    }
    let mut target_qty = floor_trade_builder_share_qty(partial_buy_usdc / sizing_ask_price);
    if target_qty + 0.000001 < min_marketable_target_qty {
        return Some(positive_quantity_flip_grid_blocked_candidate(
            quote,
            "partial_recovery",
            "partial_recovery_below_min_marketable",
            json!({
                "partial_recovery": true,
                "partial_buy_usdc": partial_buy_usdc,
                "target_qty": target_qty,
                "min_marketable_target_qty": min_marketable_target_qty,
                "min_marketable_buy_usdc": config.min_marketable_buy_usdc,
            }),
        ));
    }
    let worst_price = gate.max_buy_price_cent / 100.0;
    let mut depth = positive_quantity_flip_grid_evaluate_depth(
        config.depth_guard_enabled,
        order_book,
        gate.ask_price,
        target_qty,
        worst_price,
    );
    if depth.blocked {
        let depth_reason = depth
            .payload
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("depth_guard_blocked")
            .to_string();
        return Some(positive_quantity_flip_grid_blocked_candidate(
            quote,
            "depth",
            &depth_reason,
            depth.payload,
        ));
    }
    let mut depth_effective_price = depth.estimated_avg_fill.unwrap_or(effective_ask_price);
    if depth_effective_price > worst_price + 0.000001 {
        return Some(positive_quantity_flip_grid_blocked_candidate(
            quote,
            "max_price",
            "effective_fill_above_worst_price",
            json!({
                "partial_recovery": true,
                "effective_ask_price": depth_effective_price,
                "worst_price": worst_price,
                "depth": depth.payload,
            }),
        ));
    }
    let mut actual_buy_usdc =
        (target_qty * depth_effective_price).max(target_qty * sizing_ask_price);
    if actual_buy_usdc > partial_buy_usdc + 0.000001 {
        target_qty = floor_trade_builder_share_qty(
            partial_buy_usdc / depth_effective_price.max(sizing_ask_price),
        );
        if target_qty + 0.000001 < min_marketable_target_qty {
            return Some(positive_quantity_flip_grid_blocked_candidate(
                quote,
                "partial_recovery",
                "partial_recovery_below_min_marketable_after_depth",
                json!({
                    "partial_recovery": true,
                    "partial_buy_usdc": partial_buy_usdc,
                    "target_qty": target_qty,
                    "min_marketable_target_qty": min_marketable_target_qty,
                    "depth_effective_price": depth_effective_price,
                }),
            ));
        }
        depth = positive_quantity_flip_grid_evaluate_depth(
            config.depth_guard_enabled,
            order_book,
            gate.ask_price,
            target_qty,
            worst_price,
        );
        if depth.blocked {
            let depth_reason = depth
                .payload
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("depth_guard_blocked")
                .to_string();
            return Some(positive_quantity_flip_grid_blocked_candidate(
                quote,
                "depth",
                &depth_reason,
                depth.payload,
            ));
        }
        depth_effective_price = depth.estimated_avg_fill.unwrap_or(effective_ask_price);
        if depth_effective_price > worst_price + 0.000001 {
            return Some(positive_quantity_flip_grid_blocked_candidate(
                quote,
                "max_price",
                "effective_fill_above_worst_price",
                json!({
                    "partial_recovery": true,
                    "effective_ask_price": depth_effective_price,
                    "worst_price": worst_price,
                    "depth": depth.payload,
                }),
            ));
        }
        actual_buy_usdc = (target_qty * depth_effective_price).max(target_qty * sizing_ask_price);
        if actual_buy_usdc > partial_buy_usdc + 0.000001 {
            return Some(positive_quantity_flip_grid_blocked_candidate(
                quote,
                "partial_recovery",
                "partial_recovery_depth_exceeds_budget",
                json!({
                    "partial_recovery": true,
                    "partial_buy_usdc": partial_buy_usdc,
                    "actual_buy_usdc": actual_buy_usdc,
                    "target_qty": target_qty,
                    "depth_effective_price": depth_effective_price,
                }),
            ));
        }
    }
    let projected_side_qty = gate.current_side_qty + target_qty;
    let projected_net_cost = state.net_cost + actual_buy_usdc;
    let projected_pnl_at_exit = positive_quantity_flip_grid_pnl_at_exit(
        projected_side_qty,
        config.exit_price_for_sizing,
        projected_net_cost,
    );
    let loss_reduction_usdc = projected_pnl_at_exit - gate.pre_pnl_at_exit;
    if loss_reduction_usdc + 0.000001 < config.partial_recovery_min_loss_reduction_usdc {
        return Some(positive_quantity_flip_grid_blocked_candidate(
            quote,
            "partial_recovery",
            "partial_recovery_loss_reduction_below_min",
            json!({
                "partial_recovery": true,
                "loss_reduction_usdc": loss_reduction_usdc,
                "min_loss_reduction_usdc": config.partial_recovery_min_loss_reduction_usdc,
                "projected_pnl_at_exit": projected_pnl_at_exit,
                "pre_pnl_at_exit": gate.pre_pnl_at_exit,
                "partial_buy_usdc": actual_buy_usdc,
            }),
        ));
    }
    let details = json!({
        "partial_recovery": true,
        "loss_reduction_usdc": loss_reduction_usdc,
        "available_balance_usdc": available_balance_usdc,
        "balance_reserve_usdc": config.partial_recovery_balance_reserve_usdc,
        "spendable_balance_usdc": spendable_balance_usdc,
        "requested_full_recovery_usdc": requested_full_recovery_usdc,
        "full_actual_buy_usdc": full_actual_buy_usdc,
        "partial_buy_usdc": actual_buy_usdc,
        "partial_target_qty": target_qty,
        "risk_reason": risk_reason,
        "risk_details": risk_details,
        "partial_recovery_max_buy_usdc": config.partial_recovery_max_buy_usdc,
        "partial_recovery_ignore_market_budget": config.partial_recovery_ignore_market_budget,
    });
    let preferred_min = config.preferred_trigger_cent - config.trigger_tolerance_cent;
    let preferred_max = config.preferred_trigger_cent + config.trigger_tolerance_cent;
    let preferred_band_rank = if gate.ask_cent >= preferred_min && gate.ask_cent <= preferred_max {
        0
    } else {
        1
    };
    let report = positive_quantity_flip_grid_quote_guard_report(
        quote,
        None,
        "passed",
        json!({
            "ask_cent": gate.ask_cent,
            "effective_ask_price": depth_effective_price,
            "sizing_ask_price": sizing_ask_price,
            "sizing_price_buffer_cent": config.sizing_price_buffer_cent,
            "actual_buy_usdc": actual_buy_usdc,
            "target_qty": target_qty,
            "min_marketable_target_qty": min_marketable_target_qty,
            "min_marketable_buy_usdc": config.min_marketable_buy_usdc,
            "projected_pnl_at_exit": projected_pnl_at_exit,
            "pre_pnl_at_exit": gate.pre_pnl_at_exit,
            "rescue_buy": gate.rescue_buy,
            "max_buy_price_cent": gate.max_buy_price_cent,
            "depth": depth.payload.clone(),
            "partial_recovery": true,
            "loss_reduction_usdc": loss_reduction_usdc,
            "available_balance_usdc": available_balance_usdc,
            "balance_reserve_usdc": config.partial_recovery_balance_reserve_usdc,
            "requested_full_recovery_usdc": requested_full_recovery_usdc,
            "partial_buy_usdc": actual_buy_usdc,
        }),
    );
    Some(PositiveQuantityFlipGridCandidateEvaluation {
        candidate: Some(PositiveQuantityFlipGridBuyCandidate {
            quote: quote.clone(),
            ask_price: gate.ask_price,
            effective_ask_price: depth_effective_price,
            sizing_ask_price,
            worst_price,
            required_buy_usdc: requested_full_recovery_usdc,
            actual_buy_usdc,
            target_qty,
            projected_side_qty,
            projected_net_cost,
            projected_pnl_at_exit,
            pre_pnl_at_exit: gate.pre_pnl_at_exit,
            preferred_band_rank,
            depth_result: depth.payload,
            rescue_buy: gate.rescue_buy,
            partial_recovery: true,
            partial_recovery_details: Some(details),
        }),
        report,
    })
}

fn positive_quantity_flip_grid_entry_guard_report(
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
    quote: &PositiveQuantityFlipGridQuote,
) -> Option<Value> {
    positive_quantity_flip_grid_price_gate(config, state, quote).err()
}

fn positive_quantity_flip_grid_trigger_guard_price(step: &TradeFlowRunStep) -> Option<f64> {
    resolve_action_place_order_guard_trigger_price(step)
        .or_else(|| step_input_f64(step, &["triggered_trigger_price", "triggeredTriggerPrice"]))
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .map(clamp_probability)
}

fn positive_quantity_flip_grid_trigger_guard_report(
    config: &PositiveQuantityFlipGridConfig,
    step: &TradeFlowRunStep,
    quote: &PositiveQuantityFlipGridQuote,
) -> Option<Value> {
    if !config.trigger_price_guard_enabled {
        return None;
    }
    let Some(ask_price) = quote.best_ask else {
        return None;
    };
    let Some(guard_trigger_price) = positive_quantity_flip_grid_trigger_guard_price(step) else {
        return Some(positive_quantity_flip_grid_quote_guard_report(
            quote,
            Some("trigger_price"),
            "missing_trigger_price",
            json!({
                "best_ask": ask_price,
                "trigger_price_guard_enabled": true,
            }),
        ));
    };
    if ask_price + 0.000001 < guard_trigger_price {
        return Some(positive_quantity_flip_grid_quote_guard_report(
            quote,
            Some("trigger_price"),
            "below_trigger_price_guard",
            json!({
                "best_ask": ask_price,
                "guard_trigger_price": guard_trigger_price,
                "trigger_guard_reference_price": ask_price,
                "trigger_guard_reference_source": "best_ask",
            }),
        ));
    }
    None
}

async fn positive_quantity_flip_grid_ptb_guard_report(
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
    market_slug: &str,
    quote: &PositiveQuantityFlipGridQuote,
) -> Option<Value> {
    if !config.ptb_guard_enabled {
        return None;
    }
    let ask_price = quote.best_ask?;
    let (ptb_threshold_value, ptb_threshold_mode) =
        positive_quantity_flip_grid_ptb_threshold_for_quote(
            config,
            state,
            quote.grid_side,
            ask_price,
        );
    let evaluation =
        crate::trade_flow::guards::price_to_beat::evaluate_price_to_beat_guard_with_current_source(
            market_slug,
            crate::trade_flow::guards::price_to_beat::PriceToBeatMode::Manual,
            Some(ptb_threshold_value),
            config.ptb_diff_unit,
            &quote.outcome_label,
            None,
            config.ptb_current_price_source,
            None,
        )
        .await;
    if evaluation.passed {
        return None;
    }
    let mut guard_details = evaluation.to_value();
    if let Some(details) = guard_details.as_object_mut() {
        details.insert(
            "ptb_threshold_mode".to_string(),
            json!(ptb_threshold_mode),
        );
        details.insert(
            "ptb_threshold_value".to_string(),
            json!(ptb_threshold_value),
        );
    }
    Some(positive_quantity_flip_grid_quote_guard_report(
        quote,
        Some("price_to_beat"),
        &evaluation.reason_code,
        guard_details,
    ))
}

async fn positive_quantity_flip_grid_select_buy_candidate(
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
    step: &TradeFlowRunStep,
    market_slug: &str,
    quotes: &[PositiveQuantityFlipGridQuote],
    completion_only: bool,
    client: &dyn OrderExecutor,
) -> Result<PositiveQuantityFlipGridSelection> {
    let mut candidates = Vec::new();
    let mut guard_reports = Vec::new();
    let available_collateral_usdc = if config.partial_recovery_enabled && completion_only {
        client.available_collateral_usdc().await.unwrap_or(None)
    } else {
        None
    };
    for quote in quotes {
        if let Some(report) = positive_quantity_flip_grid_entry_guard_report(config, state, quote) {
            guard_reports.push(report);
            continue;
        }
        if let Some(report) = positive_quantity_flip_grid_trigger_guard_report(config, step, quote)
        {
            guard_reports.push(report);
            continue;
        }
        if let Some(report) = positive_quantity_flip_grid_ptb_guard_report(
            config,
            state,
            market_slug,
            quote,
        )
        .await
        {
            guard_reports.push(report);
            continue;
        }
        let order_book = if config.depth_guard_enabled {
            client.order_book(&quote.token_id).await.unwrap_or(None)
        } else {
            None
        };
        let evaluation = positive_quantity_flip_grid_evaluate_buy_candidate_with_balance(
            config,
            state,
            quote.clone(),
            completion_only,
            order_book.as_ref(),
            available_collateral_usdc,
        );
        guard_reports.push(evaluation.report);
        if let Some(candidate) = evaluation.candidate {
            candidates.push(candidate);
        }
    }
    candidates.sort_by(|left, right| {
        positive_quantity_flip_grid_inventory_need_rank(state, left.quote.grid_side)
            .cmp(&positive_quantity_flip_grid_inventory_need_rank(
                state,
                right.quote.grid_side,
            ))
            .then_with(|| left.preferred_band_rank.cmp(&right.preferred_band_rank))
            .then_with(|| left.actual_buy_usdc.total_cmp(&right.actual_buy_usdc))
            .then_with(|| left.ask_price.total_cmp(&right.ask_price))
            .then_with(|| left.quote.grid_side.cmp(right.quote.grid_side).reverse())
    });
    Ok(PositiveQuantityFlipGridSelection {
        candidate: candidates.into_iter().next(),
        guard_reports,
    })
}

#[cfg(test)]
fn positive_quantity_flip_grid_ptb_gap_guard_report_for_test(
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
    quote: &PositiveQuantityFlipGridQuote,
    directional_gap_usd: f64,
) -> Option<Value> {
    if !config.ptb_guard_enabled {
        return None;
    }
    let ask_price = quote.best_ask.unwrap_or(0.0);
    let (ptb_threshold_value, ptb_threshold_mode) =
        positive_quantity_flip_grid_ptb_threshold_for_quote(
            config,
            state,
            quote.grid_side,
            ask_price,
        );
    let threshold_usd =
        crate::trade_flow::guards::price_to_beat::normalize_price_to_beat_threshold_usd(
            ptb_threshold_value,
            config.ptb_diff_unit,
        );
    if directional_gap_usd + 0.000001 >= threshold_usd {
        return None;
    }
    Some(positive_quantity_flip_grid_quote_guard_report(
        quote,
        Some("price_to_beat"),
        "price_to_beat_gap_below_threshold",
        json!({
            "directional_gap": directional_gap_usd,
            "threshold_value": ptb_threshold_value,
            "threshold_unit": config.ptb_diff_unit.as_str(),
            "threshold_usd": threshold_usd,
            "ptb_threshold_mode": ptb_threshold_mode,
            "ptb_threshold_value": ptb_threshold_value,
        }),
    ))
}
