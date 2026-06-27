fn avg_rebound_pair_pnl(
    avg_primary_cost: rust_decimal::Decimal,
    opposite_vwap: rust_decimal::Decimal,
    qty: rust_decimal::Decimal,
) -> rust_decimal::Decimal {
    qty * (rust_decimal::Decimal::ONE - avg_primary_cost - opposite_vwap)
}

fn avg_rebound_projected_locked_pnl(
    state: &AvgReboundRuntimeState,
    opposite_qty: rust_decimal::Decimal,
    opposite_vwap: rust_decimal::Decimal,
) -> rust_decimal::Decimal {
    state.locked_pnl
        + state
            .avg_primary_cost
            .map(|avg| avg_rebound_pair_pnl(avg, opposite_vwap, opposite_qty))
            .unwrap_or(rust_decimal::Decimal::ZERO)
}
