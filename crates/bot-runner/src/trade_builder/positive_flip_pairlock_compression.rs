use bot_infra::claim::CtfMergeExecutor;
use bot_infra::db::{
    TradeBuilderPositiveQuantityFlipGridLot, TradeBuilderPositiveQuantityFlipGridMergeInput,
    TradeBuilderPositiveQuantityFlipGridMergeLegInput,
};

const POSITIVE_FLIP_PAIRLOCK_MIN_MARKETABLE_BUY_USDC: f64 = 1.0;

#[derive(Debug, Clone)]
struct PositiveFlipPairlockMergePlan {
    up_leg: TradeBuilderPositiveQuantityFlipGridLot,
    down_leg: TradeBuilderPositiveQuantityFlipGridLot,
    quantity: f64,
    pair_cost: f64,
    locked_profit_per_share: f64,
}

#[derive(Debug, Clone)]
struct PositiveFlipPairlockBuyOpportunity {
    matched_lot: TradeBuilderPositiveQuantityFlipGridLot,
    candidate: PositiveQuantityFlipGridBuyCandidate,
    pair_cost: f64,
    locked_profit_per_share: f64,
    min_marketable_buy_usdc: f64,
    base_target_qty: f64,
    base_buy_usdc: f64,
    target_qty_after_min_notional: f64,
    min_notional_top_up_applied: bool,
}

#[derive(Debug, Default)]
struct PositiveFlipPairlockBuyOpportunityScan {
    opportunity: Option<PositiveFlipPairlockBuyOpportunity>,
    diagnostics: Vec<Value>,
}

fn positive_flip_pairlock_buys_blocked_after_profit_lock(
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
) -> bool {
    config.pairlock_compression_enabled
        && config.stop_buys_after_pairlock_merge
        && state.total_merge_return > 0.000001
}

fn positive_flip_pairlock_positive_buy_intent(
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
) -> Option<&'static str> {
    config
        .pairlock_compression_enabled
        .then_some(if state.buy_count == 0 {
            "core_positive"
        } else {
            "flip_positive"
        })
}

fn positive_flip_pairlock_side_quote<'a>(
    quotes: &'a [PositiveQuantityFlipGridQuote],
    grid_side: &str,
) -> Option<&'a PositiveQuantityFlipGridQuote> {
    quotes.iter().find(|quote| quote.grid_side == grid_side)
}

fn positive_flip_pairlock_opposite_side(grid_side: &str) -> &'static str {
    if grid_side == "down" {
        "up"
    } else {
        "down"
    }
}

fn positive_flip_pairlock_pair_is_profitable(
    matched_price: f64,
    new_price: f64,
    config: &PositiveQuantityFlipGridConfig,
) -> Option<(f64, f64)> {
    let pair_cost = matched_price + new_price;
    let locked_profit = 1.0 - pair_cost - config.fee_buffer;
    (locked_profit + 0.000001 >= config.target_pairlock_profit
        && pair_cost <= config.max_pair_cost + 0.000001)
        .then_some((pair_cost, locked_profit))
}

fn positive_flip_pairlock_merge_return(quantity: f64) -> f64 {
    floor_trade_builder_share_qty(quantity.max(0.0))
}

fn positive_flip_pairlock_min_marketable_share_qty(ask_price: f64) -> f64 {
    if !ask_price.is_finite() || ask_price <= 0.0 {
        return 0.0;
    }
    positive_quantity_flip_grid_round_up_share_qty(
        POSITIVE_FLIP_PAIRLOCK_MIN_MARKETABLE_BUY_USDC / ask_price,
    )
}

fn positive_flip_pairlock_find_merge_plan(
    lots: &[TradeBuilderPositiveQuantityFlipGridLot],
    config: &PositiveQuantityFlipGridConfig,
) -> Option<PositiveFlipPairlockMergePlan> {
    if !config.pairlock_compression_enabled || config.max_unmerged_exposure_usdc <= 0.0 {
        return None;
    }
    let up_lots = lots.iter().filter(|lot| lot.grid_side == "up");
    let down_lots = lots
        .iter()
        .filter(|lot| lot.grid_side == "down")
        .collect::<Vec<_>>();
    for up in up_lots {
        for down in &down_lots {
            let Some((pair_cost, locked_profit)) = positive_flip_pairlock_pair_is_profitable(
                up.execution_price,
                down.execution_price,
                config,
            ) else {
                continue;
            };
            let quantity = positive_flip_pairlock_merge_return(
                up.quantity
                    .min(down.quantity)
                    .min(config.max_unmerged_exposure_usdc),
            );
            if quantity <= 0.000001 {
                continue;
            }
            return Some(PositiveFlipPairlockMergePlan {
                up_leg: up.clone(),
                down_leg: (*down).clone(),
                quantity,
                pair_cost,
                locked_profit_per_share: locked_profit,
            });
        }
    }
    None
}

#[cfg(test)]
fn positive_flip_pairlock_find_buy_opportunity(
    lots: &[TradeBuilderPositiveQuantityFlipGridLot],
    quotes: &[PositiveQuantityFlipGridQuote],
    state: &TradeBuilderPositiveQuantityFlipGridState,
    config: &PositiveQuantityFlipGridConfig,
) -> Option<PositiveFlipPairlockBuyOpportunity> {
    positive_flip_pairlock_scan_buy_opportunity(lots, quotes, state, config).opportunity
}

fn positive_flip_pairlock_scan_buy_opportunity(
    lots: &[TradeBuilderPositiveQuantityFlipGridLot],
    quotes: &[PositiveQuantityFlipGridQuote],
    state: &TradeBuilderPositiveQuantityFlipGridState,
    config: &PositiveQuantityFlipGridConfig,
) -> PositiveFlipPairlockBuyOpportunityScan {
    if !config.pairlock_compression_enabled {
        return PositiveFlipPairlockBuyOpportunityScan::default();
    }
    let remaining_budget = config
        .max_total_spent_per_market_usdc
        .map(|cap| (cap - state.total_buy_cost).max(0.0))
        .unwrap_or(f64::MAX);
    if remaining_budget <= 0.000001 || config.max_unmerged_exposure_usdc <= 0.0 {
        return PositiveFlipPairlockBuyOpportunityScan::default();
    }
    let mut scan = PositiveFlipPairlockBuyOpportunityScan::default();
    for lot in lots {
        let buy_side = positive_flip_pairlock_opposite_side(&lot.grid_side);
        let Some(quote) = positive_flip_pairlock_side_quote(quotes, buy_side).cloned() else {
            continue;
        };
        let Some(ask) = quote.best_ask else {
            continue;
        };
        if ask <= 0.0 || ask >= 1.0 {
            continue;
        }
        let Some((pair_cost, locked_profit)) =
            positive_flip_pairlock_pair_is_profitable(lot.execution_price, ask, config)
        else {
            continue;
        };
        let max_single_qty = config
            .max_single_buy_usdc
            .map(|cap| cap / ask)
            .unwrap_or(f64::MAX);
        let remaining_budget_qty = remaining_budget / ask;
        let max_unmerged_qty = config.max_unmerged_exposure_usdc / ask;
        let hard_cap_qty = max_single_qty
            .min(remaining_budget_qty)
            .min(max_unmerged_qty);
        let base_target_qty = floor_trade_builder_share_qty(lot.quantity.min(hard_cap_qty));
        if base_target_qty <= 0.000001 {
            continue;
        }
        let base_buy_usdc = base_target_qty * ask;
        let min_marketable_qty = positive_flip_pairlock_min_marketable_share_qty(ask);
        let min_notional_top_up_applied =
            base_buy_usdc + 0.000001 < POSITIVE_FLIP_PAIRLOCK_MIN_MARKETABLE_BUY_USDC;
        let target_qty = if min_notional_top_up_applied {
            min_marketable_qty
        } else {
            base_target_qty
        };
        if target_qty <= 0.000001 {
            continue;
        }
        if target_qty > hard_cap_qty + 0.000001 {
            scan.diagnostics.push(json!({
                "reason": "pairlock_min_notional_exceeds_cap",
                "grid_side": buy_side,
                "matched_parent_builder_order_id": lot.parent_builder_order_id,
                "matched_lot_qty": lot.quantity,
                "matched_lot_price": lot.execution_price,
                "ask_price": ask,
                "min_marketable_buy_usdc": POSITIVE_FLIP_PAIRLOCK_MIN_MARKETABLE_BUY_USDC,
                "base_target_qty": base_target_qty,
                "base_buy_usdc": base_buy_usdc,
                "target_qty_after_min_notional": target_qty,
                "max_single_qty": max_single_qty,
                "remaining_budget_qty": remaining_budget_qty,
                "max_unmerged_qty": max_unmerged_qty,
                "hard_cap_qty": hard_cap_qty,
            }));
            continue;
        }
        let actual_buy_usdc = target_qty * ask;
        let candidate = PositiveFlipPairlockBuyOpportunity {
            matched_lot: lot.clone(),
            pair_cost,
            locked_profit_per_share: locked_profit,
            min_marketable_buy_usdc: POSITIVE_FLIP_PAIRLOCK_MIN_MARKETABLE_BUY_USDC,
            base_target_qty,
            base_buy_usdc,
            target_qty_after_min_notional: target_qty,
            min_notional_top_up_applied,
            candidate: PositiveQuantityFlipGridBuyCandidate {
                quote,
                ask_price: ask,
                effective_ask_price: ask,
                sizing_ask_price: ask,
                worst_price: ask,
                required_buy_usdc: actual_buy_usdc,
                actual_buy_usdc,
                target_qty,
                projected_side_qty: positive_quantity_flip_grid_side_qty(state, buy_side)
                    + target_qty,
                projected_net_cost: state.net_cost + actual_buy_usdc,
                projected_pnl_at_exit: positive_quantity_flip_grid_pnl_at_exit(
                    positive_quantity_flip_grid_side_qty(state, buy_side) + target_qty,
                    config.exit_price_for_sizing,
                    state.net_cost + actual_buy_usdc,
                ),
                pre_pnl_at_exit: positive_quantity_flip_grid_pnl_at_exit(
                    positive_quantity_flip_grid_side_qty(state, buy_side),
                    config.exit_price_for_sizing,
                    state.net_cost,
                ),
                preferred_band_rank: 0,
                depth_result: json!({
                    "result": "pass",
                    "reason": "pairlock_compression",
                    "depth_guard_enabled": false,
                    "min_marketable_buy_usdc": POSITIVE_FLIP_PAIRLOCK_MIN_MARKETABLE_BUY_USDC,
                    "base_target_qty": base_target_qty,
                    "base_buy_usdc": base_buy_usdc,
                    "target_qty_after_min_notional": target_qty,
                    "min_notional_top_up_applied": min_notional_top_up_applied
                }),
                rescue_buy: false,
                partial_recovery: false,
                partial_recovery_details: None,
            },
        };
        let replace_best = match scan.opportunity.as_ref() {
            Some(best) => candidate.locked_profit_per_share > best.locked_profit_per_share,
            None => true,
        };
        if replace_best {
            scan.opportunity = Some(candidate);
        }
    }
    scan
}

#[allow(clippy::too_many_arguments)]
async fn positive_flip_pairlock_try_basket_exit(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    context: &mut Value,
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
    market_slug: &str,
    quotes: &[PositiveQuantityFlipGridQuote],
) -> Result<Option<TradeFlowNodeExecution>> {
    if !config.pairlock_compression_enabled || !config.basket_exit_enabled {
        return Ok(None);
    }
    let up_bid = positive_flip_pairlock_side_quote(quotes, "up").and_then(|quote| quote.best_bid);
    let down_bid =
        positive_flip_pairlock_side_quote(quotes, "down").and_then(|quote| quote.best_bid);
    let basket_value =
        state.up_qty * up_bid.unwrap_or(0.0) + state.down_qty * down_bid.unwrap_or(0.0);
    let pnl = basket_value - state.net_cost;
    if pnl + 0.000001 < config.min_basket_profit_usdc {
        return Ok(None);
    }

    let mut sell_outputs = Vec::new();
    if state.up_qty > 0.000001 {
        if let Some(bid) = up_bid {
            sell_outputs.extend(
                positive_quantity_flip_grid_execute_sell_side(
                    repo,
                    run_id,
                    cfg,
                    limits,
                    policy,
                    Some(client),
                    run,
                    step,
                    node,
                    graph,
                    context,
                    config,
                    market_slug,
                    "up",
                    bid,
                )
                .await?,
            );
        }
    }
    if state.down_qty > 0.000001 {
        if let Some(bid) = down_bid {
            sell_outputs.extend(
                positive_quantity_flip_grid_execute_sell_side(
                    repo,
                    run_id,
                    cfg,
                    limits,
                    policy,
                    Some(client),
                    run,
                    step,
                    node,
                    graph,
                    context,
                    config,
                    market_slug,
                    "down",
                    bid,
                )
                .await?,
            );
        }
    }

    Ok(Some(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "mode": ACTION_PLACE_ORDER_MODE_POSITIVE_FLIP_PAIRLOCK_COMPRESSION_V1,
            "market_slug": market_slug,
            "decision": "basket_exit",
            "basket_value_usdc": basket_value,
            "basket_pnl_usdc": pnl,
            "min_basket_profit_usdc": config.min_basket_profit_usdc,
            "state": {
                "up_qty": state.up_qty,
                "down_qty": state.down_qty,
                "total_buy_cost": state.total_buy_cost,
                "total_sell_revenue": state.total_sell_revenue,
                "total_merge_return": state.total_merge_return,
                "net_cost": state.net_cost,
                "buy_count": state.buy_count,
            },
            "sell_outputs": sell_outputs,
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    }))
}

#[allow(clippy::too_many_arguments)]
async fn positive_flip_pairlock_try_compression(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    context: &mut Value,
    config: &PositiveQuantityFlipGridConfig,
    state: &TradeBuilderPositiveQuantityFlipGridState,
    market_slug: &str,
    quotes: &[PositiveQuantityFlipGridQuote],
) -> Result<Option<TradeFlowNodeExecution>> {
    if !config.pairlock_compression_enabled {
        return Ok(None);
    }
    let lots = repo
        .list_open_positive_quantity_flip_grid_lots(
            run.user_id,
            Some(run.definition_id),
            &node.key,
            market_slug,
        )
        .await?;
    if let Some(plan) = positive_flip_pairlock_find_merge_plan(&lots, config) {
        return positive_flip_pairlock_execute_merge(
            repo,
            cfg,
            client,
            run,
            node,
            market_slug,
            plan,
        )
        .await
        .map(Some);
    }
    if positive_flip_pairlock_buys_blocked_after_profit_lock(config, state) {
        return Ok(None);
    }
    let buy_scan = positive_flip_pairlock_scan_buy_opportunity(&lots, quotes, state, config);
    if buy_scan.opportunity.is_none() && !buy_scan.diagnostics.is_empty() {
        debug!(
            market_slug = %market_slug,
            diagnostics = %json!(buy_scan.diagnostics),
            "positive flip pairlock compression buy skipped"
        );
    }
    let Some(opportunity) = buy_scan.opportunity else {
        return Ok(None);
    };
    let buy_lock = match positive_quantity_flip_grid_prepare_buy_order_submission(
        repo,
        run,
        node,
        market_slug,
        state,
    )
    .await?
    {
        Ok(lock) => lock,
        Err(skip) => return Ok(Some(skip)),
    };
    let buy_node = positive_quantity_flip_grid_buy_node(
        node,
        config,
        market_slug,
        &opportunity.candidate,
        Utc::now().timestamp_millis(),
        Some("pairlock_compression_buy"),
        Some(config.pairlock_order_type),
    );
    let buy_step = positive_quantity_flip_grid_step_with_price(
        step,
        market_slug,
        &opportunity.candidate.quote.token_id,
        &opportunity.candidate.quote.outcome_label,
        opportunity.candidate.sizing_ask_price,
    );
    clear_action_place_order_ref_bindings(
        context,
        &buy_node,
        &action_place_order_ref_key(&buy_node),
    );
    let execution = execute_action_place_order(
        repo,
        run_id,
        cfg,
        limits,
        policy,
        Some(client),
        run,
        &buy_step,
        &buy_node,
        graph,
        context,
    )
    .await?;
    buy_lock.release().await;

    Ok(Some(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "mode": ACTION_PLACE_ORDER_MODE_POSITIVE_FLIP_PAIRLOCK_COMPRESSION_V1,
            "market_slug": market_slug,
            "decision": "pairlock_compression_buy",
            "grid_side": opportunity.candidate.quote.grid_side,
            "matched_parent_builder_order_id": opportunity.matched_lot.parent_builder_order_id,
            "matched_lot_price": opportunity.matched_lot.execution_price,
            "buy_price": opportunity.candidate.ask_price,
            "pair_cost": opportunity.pair_cost,
            "locked_profit_per_share": opportunity.locked_profit_per_share,
            "target_qty": opportunity.candidate.target_qty,
            "actual_buy_usdc": opportunity.candidate.actual_buy_usdc,
            "min_marketable_buy_usdc": opportunity.min_marketable_buy_usdc,
            "base_target_qty": opportunity.base_target_qty,
            "base_buy_usdc": opportunity.base_buy_usdc,
            "target_qty_after_min_notional": opportunity.target_qty_after_min_notional,
            "min_notional_top_up_applied": opportunity.min_notional_top_up_applied,
            "pairlock_order_type": config.pairlock_order_type,
            "child_output": execution.output,
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    }))
}

async fn positive_flip_pairlock_execute_merge(
    repo: &PostgresRepository,
    cfg: &AppConfig,
    client: &dyn OrderExecutor,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    market_slug: &str,
    plan: PositiveFlipPairlockMergePlan,
) -> Result<TradeFlowNodeExecution> {
    let market_info = client
        .clob_market_info_by_token(&plan.up_leg.token_id)
        .await?
        .or(client
            .clob_market_info_by_token(&plan.down_leg.token_id)
            .await?);
    let Some(market_info) = market_info else {
        return Ok(positive_quantity_flip_grid_output_skipped(
            node,
            market_slug,
            "pairlock_condition_id_unavailable",
            json!({
                "up_token_id": plan.up_leg.token_id,
                "down_token_id": plan.down_leg.token_id
            }),
        ));
    };
    if market_info.neg_risk {
        return Ok(positive_quantity_flip_grid_output_skipped(
            node,
            market_slug,
            "pairlock_neg_risk_unsupported",
            json!({ "condition_id": market_info.condition_id }),
        ));
    }
    let executor = match CtfMergeExecutor::from_app_config(run.user_id, cfg) {
        Ok(executor) => executor,
        Err(err) => {
            return Ok(positive_quantity_flip_grid_output_skipped(
                node,
                market_slug,
                "pairlock_merge_executor_unavailable",
                json!({ "error": err.to_string() }),
            ));
        }
    };
    let submission = match executor
        .submit_merge(&market_info.condition_id, plan.quantity)
        .await
    {
        Ok(submission) => submission,
        Err(err) => {
            return Ok(positive_quantity_flip_grid_output_skipped(
                node,
                market_slug,
                "pairlock_merge_failed",
                json!({
                    "condition_id": market_info.condition_id,
                    "quantity": plan.quantity,
                    "error": err.to_string()
                }),
            ));
        }
    };
    repo.record_positive_quantity_flip_grid_merge(
        &TradeBuilderPositiveQuantityFlipGridMergeInput {
            user_id: run.user_id,
            flow_definition_id: Some(run.definition_id),
            flow_run_id: Some(run.id),
            root_flow_node_key: node.key.clone(),
            market_slug: market_slug.to_string(),
            condition_id: market_info.condition_id.clone(),
            quantity: plan.quantity,
            returned_usdc: submission.returned_usdc,
            tx_hash: submission.tx_hash.clone(),
            submission_mode: submission.submission_mode.to_string(),
            payload_json: json!({
                "role": "merge",
                "intent": "pairlock_compression_merge",
                "condition_id": market_info.condition_id,
                "amount_raw": submission.amount_raw,
                "pair_cost": plan.pair_cost,
                "locked_profit_per_share": plan.locked_profit_per_share,
                "partition": [1, 2],
                "up_leg": {
                    "parent_builder_order_id": plan.up_leg.parent_builder_order_id,
                    "price": plan.up_leg.execution_price,
                    "quantity": plan.up_leg.quantity,
                    "intent": plan.up_leg.intent,
                },
                "down_leg": {
                    "parent_builder_order_id": plan.down_leg.parent_builder_order_id,
                    "price": plan.down_leg.execution_price,
                    "quantity": plan.down_leg.quantity,
                    "intent": plan.down_leg.intent,
                }
            }),
            up_legs: vec![TradeBuilderPositiveQuantityFlipGridMergeLegInput {
                parent_builder_order_id: plan.up_leg.parent_builder_order_id,
                grid_side: "up".to_string(),
                quantity: plan.quantity,
            }],
            down_legs: vec![TradeBuilderPositiveQuantityFlipGridMergeLegInput {
                parent_builder_order_id: plan.down_leg.parent_builder_order_id,
                grid_side: "down".to_string(),
                quantity: plan.quantity,
            }],
        },
    )
    .await?;

    Ok(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "mode": ACTION_PLACE_ORDER_MODE_POSITIVE_FLIP_PAIRLOCK_COMPRESSION_V1,
            "market_slug": market_slug,
            "decision": "pairlock_merge",
            "condition_id": market_info.condition_id,
            "quantity": plan.quantity,
            "returned_usdc": submission.returned_usdc,
            "tx_hash": submission.tx_hash,
            "submission_mode": submission.submission_mode,
            "pair_cost": plan.pair_cost,
            "locked_profit_per_share": plan.locked_profit_per_share,
        }),
        routes: vec![TradeFlowRouteDecision {
            edge_type: "on_success".to_string(),
            available_at: Utc::now(),
        }],
        repeat_at: None,
        repeat_idempotency_key: None,
    })
}

#[cfg(test)]
mod positive_flip_pairlock_compression_tests {
    use super::*;

    fn pairlock_config() -> PositiveQuantityFlipGridConfig {
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
            max_total_spent_per_market_usdc: Some(3.0),
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
            pairlock_compression_enabled: true,
            stop_buys_after_pairlock_merge: true,
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

    #[test]
    fn pairlock_compression_buy_tops_up_to_market_min_notional() {
        let cfg = pairlock_config();
        let lots = vec![lot(18136, "down", 2.33, 0.53)];
        let state = TradeBuilderPositiveQuantityFlipGridState {
            down_qty: 2.33,
            total_buy_cost: 1.2349,
            net_cost: 1.2349,
            buy_count: 2,
            last_buy_grid_side: Some("down".to_string()),
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };

        let opportunity = positive_flip_pairlock_find_buy_opportunity(
            &lots,
            &[quote_with_side_ask("up", 0.34)],
            &state,
            &cfg,
        )
        .expect("pairlock min-notional buy");

        assert_eq!(opportunity.candidate.quote.grid_side, "up");
        assert!((opportunity.base_target_qty - 2.33).abs() < 0.000001);
        assert!((opportunity.base_buy_usdc - 0.7922).abs() < 0.000001);
        assert!(opportunity.min_notional_top_up_applied);
        assert!((opportunity.candidate.target_qty - 2.95).abs() < 0.000001);
        assert!(opportunity.candidate.actual_buy_usdc >= 1.0);
        assert_eq!(
            opportunity.candidate.depth_result["min_notional_top_up_applied"],
            json!(true)
        );
    }

    #[test]
    fn pairlock_compression_min_notional_respects_remaining_budget_cap() {
        let mut cfg = pairlock_config();
        cfg.max_total_spent_per_market_usdc = Some(2.0);
        let lots = vec![lot(18136, "down", 2.33, 0.53)];
        let state = TradeBuilderPositiveQuantityFlipGridState {
            down_qty: 2.33,
            total_buy_cost: 1.2349,
            net_cost: 1.2349,
            buy_count: 2,
            last_buy_grid_side: Some("down".to_string()),
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };

        let scan = positive_flip_pairlock_scan_buy_opportunity(
            &lots,
            &[quote_with_side_ask("up", 0.34)],
            &state,
            &cfg,
        );

        assert!(scan.opportunity.is_none());
        assert_eq!(
            scan.diagnostics[0]["reason"],
            json!("pairlock_min_notional_exceeds_cap")
        );
    }

    #[test]
    fn pairlock_compression_buy_keeps_existing_qty_when_above_market_min() {
        let mut cfg = pairlock_config();
        cfg.max_total_spent_per_market_usdc = Some(9.5);
        let lots = vec![lot(18136, "down", 4.0, 0.53)];
        let state = TradeBuilderPositiveQuantityFlipGridState {
            down_qty: 4.0,
            total_buy_cost: 2.12,
            net_cost: 2.12,
            buy_count: 2,
            last_buy_grid_side: Some("down".to_string()),
            ..TradeBuilderPositiveQuantityFlipGridState::default()
        };

        let opportunity = positive_flip_pairlock_find_buy_opportunity(
            &lots,
            &[quote_with_side_ask("up", 0.34)],
            &state,
            &cfg,
        )
        .expect("pairlock buy above min");

        assert!(!opportunity.min_notional_top_up_applied);
        assert_eq!(opportunity.base_target_qty, 4.0);
        assert_eq!(opportunity.candidate.target_qty, 4.0);
        assert!((opportunity.candidate.actual_buy_usdc - 1.36).abs() < 0.000001);
    }

    #[test]
    fn pairlock_merge_plan_uses_new_profitable_lot_after_top_up() {
        let mut cfg = pairlock_config();
        cfg.max_unmerged_exposure_usdc = 10.0;
        let lots = vec![
            lot(18135, "up", 1.69, 0.60),
            lot(18136, "down", 2.33, 0.53),
            lot(18138, "up", 2.95, 0.34),
        ];

        let plan = positive_flip_pairlock_find_merge_plan(&lots, &cfg).expect("merge plan");

        assert_eq!(plan.up_leg.parent_builder_order_id, 18138);
        assert_eq!(plan.down_leg.parent_builder_order_id, 18136);
        assert!((plan.quantity - 2.33).abs() < 0.000001);
        assert!((plan.pair_cost - 0.87).abs() < 0.000001);
        assert!(plan.up_leg.quantity > plan.quantity);
    }
}
