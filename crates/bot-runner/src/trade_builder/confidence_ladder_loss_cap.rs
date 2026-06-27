#[derive(Debug, Clone, Copy, PartialEq)]
struct ConfidenceLadderLossCapPlan {
    quantity: f64,
    reason: &'static str,
}

fn confidence_ladder_ceil_share_qty(value: f64) -> f64 {
    ((value.max(0.0)) * 100.0).ceil() / 100.0
}

fn confidence_ladder_unmatched_qty(
    state: &TradeBuilderConfidenceLadderState,
    held_side: &str,
    opposite_side: &str,
) -> f64 {
    (confidence_ladder_state_qty(state, held_side)
        - confidence_ladder_state_qty(state, opposite_side))
    .max(0.0)
}

fn confidence_ladder_max_opposite_qty(
    config: &ConfidenceLadderConfig,
    state: &TradeBuilderConfidenceLadderState,
    held_side: &str,
    opposite_side: &str,
    all_in_cost: f64,
) -> f64 {
    let unmatched_qty = confidence_ladder_unmatched_qty(state, held_side, opposite_side);
    let budget_qty = if all_in_cost > 0.0 {
        (config.max_total_cost_per_market_usdc - state.total_cost_usdc) / all_in_cost
    } else {
        unmatched_qty
    };
    let held_win_qty = if all_in_cost > 0.0 {
        (confidence_ladder_state_qty(state, held_side) - state.total_cost_usdc
            + config.max_loss_per_market_usdc)
            / all_in_cost
    } else {
        unmatched_qty
    };
    round_trade_builder_share_qty(unmatched_qty.min(budget_qty).min(held_win_qty).max(0.0))
}

fn confidence_ladder_quantity_for_worst_case_target(
    config: &ConfidenceLadderConfig,
    state: &TradeBuilderConfidenceLadderState,
    held_side: &str,
    opposite_side: &str,
    all_in_cost: f64,
    target_pnl: f64,
) -> Option<f64> {
    if all_in_cost <= 0.0 || all_in_cost >= 1.0 {
        return None;
    }
    let opposite_qty = confidence_ladder_state_qty(state, opposite_side);
    let needed = (target_pnl + state.total_cost_usdc - opposite_qty) / (1.0 - all_in_cost);
    let quantity = confidence_ladder_ceil_share_qty(needed.max(0.0));
    let max_qty =
        confidence_ladder_max_opposite_qty(config, state, held_side, opposite_side, all_in_cost);
    if quantity <= 0.0 || quantity > max_qty {
        return None;
    }
    let projected =
        confidence_ladder_projected_worst_case_pnl(state, opposite_side, quantity, all_in_cost);
    (projected + 0.000001 >= target_pnl).then_some(quantity)
}

fn confidence_ladder_strong_profit_lock_quantity(
    config: &ConfidenceLadderConfig,
    state: &TradeBuilderConfidenceLadderState,
    held_side: &str,
    opposite_side: &str,
    all_in_cost: f64,
) -> Option<f64> {
    let target_qty = confidence_ladder_state_qty(state, held_side)
        * config.target_hedge_ratio_max.clamp(0.0, 1.0);
    let missing_qty = (target_qty - confidence_ladder_state_qty(state, opposite_side)).max(0.0);
    let max_qty =
        confidence_ladder_max_opposite_qty(config, state, held_side, opposite_side, all_in_cost);
    let quantity = round_trade_builder_share_qty(missing_qty.min(max_qty));
    (quantity > 0.0).then_some(quantity)
}

fn confidence_ladder_profit_lock_quantity(
    config: &ConfidenceLadderConfig,
    state: &TradeBuilderConfidenceLadderState,
    held_side: &str,
    opposite_side: &str,
    all_in_cost: f64,
) -> Option<ConfidenceLadderLossCapPlan> {
    if let Some(quantity) = confidence_ladder_quantity_for_worst_case_target(
        config,
        state,
        held_side,
        opposite_side,
        all_in_cost,
        0.0,
    ) {
        return Some(ConfidenceLadderLossCapPlan {
            quantity,
            reason: "breakeven_lock",
        });
    }
    let ratio_floor = confidence_ladder_state_qty(state, held_side)
        * config.target_hedge_ratio_min.clamp(0.0, 1.0);
    let floor_qty = confidence_ladder_ceil_share_qty(
        (ratio_floor - confidence_ladder_state_qty(state, opposite_side)).max(0.0),
    );
    let loss_cap = confidence_ladder_loss_cap_quantity(
        config,
        state,
        held_side,
        opposite_side,
        all_in_cost,
        -config.max_loss_per_market_usdc,
    )?;
    Some(ConfidenceLadderLossCapPlan {
        quantity: loss_cap.quantity.max(floor_qty),
        reason: loss_cap.reason,
    })
}

fn confidence_ladder_loss_cap_quantity(
    config: &ConfidenceLadderConfig,
    state: &TradeBuilderConfidenceLadderState,
    held_side: &str,
    opposite_side: &str,
    all_in_cost: f64,
    target_pnl: f64,
) -> Option<ConfidenceLadderLossCapPlan> {
    confidence_ladder_quantity_for_worst_case_target(
        config,
        state,
        held_side,
        opposite_side,
        all_in_cost,
        target_pnl,
    )
    .map(|quantity| ConfidenceLadderLossCapPlan {
        quantity,
        reason: "max_loss_cap",
    })
}

#[cfg(test)]
mod confidence_ladder_loss_cap_tests {
    use super::*;

    fn state(
        up_qty: f64,
        down_qty: f64,
        total_cost_usdc: f64,
    ) -> TradeBuilderConfidenceLadderState {
        TradeBuilderConfidenceLadderState {
            up_qty,
            down_qty,
            total_cost_usdc,
            worst_case_pnl: up_qty.min(down_qty) - total_cost_usdc,
            ..TradeBuilderConfidenceLadderState::default()
        }
    }

    #[test]
    fn confidence_ladder_strong_profit_lock_targets_unmatched_qty() {
        let config = ConfidenceLadderConfig::default();
        let state = state(12.0, 0.0, 8.28);
        let qty =
            confidence_ladder_strong_profit_lock_quantity(&config, &state, "up", "down", 0.14)
                .expect("strong lock quantity");
        assert_eq!(qty, 12.0);
    }

    #[test]
    fn confidence_ladder_loss_cap_quantity_targets_three_usdc_cap() {
        let config = ConfidenceLadderConfig::default();
        let state = state(12.0, 0.0, 8.28);
        let plan = confidence_ladder_loss_cap_quantity(&config, &state, "up", "down", 0.54, -3.0)
            .expect("loss cap quantity");
        assert!((11.48..=11.50).contains(&plan.quantity));
        let projected =
            confidence_ladder_projected_worst_case_pnl(&state, "down", plan.quantity, 0.54);
        assert!(projected >= -3.0);
    }

    #[test]
    fn confidence_ladder_loss_cap_never_exceeds_held_side() {
        let config = ConfidenceLadderConfig::default();
        let state = state(4.0, 0.0, 8.28);
        assert!(
            confidence_ladder_loss_cap_quantity(&config, &state, "up", "down", 0.54, -3.0)
                .is_none()
        );
    }
}
