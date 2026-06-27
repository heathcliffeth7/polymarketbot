#[derive(Debug, Clone, Default, PartialEq)]
struct AvgReboundRuntimeState {
    session_id: Option<i64>,
    session_status: Option<String>,
    primary_total_qty: rust_decimal::Decimal,
    primary_total_cost: rust_decimal::Decimal,
    avg_primary_cost: Option<rust_decimal::Decimal>,
    opposite_filled_qty: rust_decimal::Decimal,
    opposite_total_cost: rust_decimal::Decimal,
    open_primary_qty: rust_decimal::Decimal,
    locked_pnl: rust_decimal::Decimal,
    profit_started: bool,
    primary_tier_ids: Vec<String>,
    opposite_leg_ids: Vec<String>,
}

fn avg_rebound_decimal_from_f64(value: f64) -> rust_decimal::Decimal {
    if !value.is_finite() {
        return rust_decimal::Decimal::ZERO;
    }
    format!("{value:.12}")
        .parse::<rust_decimal::Decimal>()
        .unwrap_or(rust_decimal::Decimal::ZERO)
        .normalize()
}

fn avg_rebound_decimal_to_f64(value: rust_decimal::Decimal) -> f64 {
    rust_decimal::prelude::ToPrimitive::to_f64(&value).unwrap_or(0.0)
}

fn avg_rebound_state_from_db(
    state: bot_infra::db::TradeBuilderAvgReboundPairlockRescueState,
) -> AvgReboundRuntimeState {
    AvgReboundRuntimeState {
        session_id: state.session_id,
        session_status: state.session_status,
        primary_total_qty: avg_rebound_decimal_from_f64(state.primary_total_qty),
        primary_total_cost: avg_rebound_decimal_from_f64(state.primary_total_cost),
        avg_primary_cost: state.avg_primary_cost.map(avg_rebound_decimal_from_f64),
        opposite_filled_qty: avg_rebound_decimal_from_f64(state.opposite_filled_qty),
        opposite_total_cost: avg_rebound_decimal_from_f64(state.opposite_total_cost),
        open_primary_qty: avg_rebound_decimal_from_f64(state.open_primary_qty),
        locked_pnl: avg_rebound_decimal_from_f64(state.locked_pnl),
        profit_started: state.profit_started,
        primary_tier_ids: state.primary_tier_ids,
        opposite_leg_ids: state.opposite_leg_ids,
    }
}

fn avg_rebound_has_primary_tier(state: &AvgReboundRuntimeState, tier_id: &str) -> bool {
    state.primary_tier_ids.iter().any(|id| id == tier_id)
}

fn avg_rebound_has_opposite_leg(state: &AvgReboundRuntimeState, leg_id: &str) -> bool {
    state.opposite_leg_ids.iter().any(|id| id == leg_id)
}

fn avg_rebound_stage_is_ready(
    state: &AvgReboundRuntimeState,
    stage: &AvgReboundStageConfig,
) -> bool {
    stage
        .required_primary_tier_ids
        .iter()
        .all(|tier_id| avg_rebound_has_primary_tier(state, tier_id))
}

fn avg_rebound_current_stage<'a>(
    config: &'a AvgReboundPairlockRescueConfig,
    state: &AvgReboundRuntimeState,
) -> Option<&'a AvgReboundStageConfig> {
    config
        .stages
        .iter()
        .filter(|stage| avg_rebound_stage_is_ready(state, stage))
        .max_by_key(|stage| stage.required_primary_tier_ids.len())
}

fn avg_rebound_full_ladder_filled(
    config: &AvgReboundPairlockRescueConfig,
    state: &AvgReboundRuntimeState,
) -> bool {
    config
        .primary_ladder
        .iter()
        .all(|tier| avg_rebound_has_primary_tier(state, &tier.id))
}

fn avg_rebound_next_primary_tier<'a>(
    config: &'a AvgReboundPairlockRescueConfig,
    state: &AvgReboundRuntimeState,
) -> Option<&'a AvgReboundPrimaryTierConfig> {
    config
        .primary_ladder
        .iter()
        .find(|tier| !avg_rebound_has_primary_tier(state, &tier.id))
}

fn avg_rebound_budget_limit(config: &AvgReboundPairlockRescueConfig) -> rust_decimal::Decimal {
    config.session_budget_usdc - config.reserved_budget_buffer_usdc
}

fn avg_rebound_projected_spend_allowed(
    config: &AvgReboundPairlockRescueConfig,
    state: &AvgReboundRuntimeState,
    additional_notional: rust_decimal::Decimal,
) -> bool {
    state.primary_total_cost + state.opposite_total_cost + additional_notional
        <= avg_rebound_budget_limit(config)
}

fn avg_rebound_qty_min(
    left: rust_decimal::Decimal,
    right: rust_decimal::Decimal,
) -> rust_decimal::Decimal {
    if left <= right {
        left
    } else {
        right
    }
}
