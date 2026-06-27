#[derive(Debug, Clone, PartialEq)]
struct AvgReboundDecision {
    session_id: i64,
    leg_role: &'static str,
    intent: &'static str,
    session_status: &'static str,
    stage_id: Option<String>,
    tier_or_leg_id: String,
    token_id: String,
    outcome_label: String,
    qty: rust_decimal::Decimal,
    limit_price: rust_decimal::Decimal,
    vwap: rust_decimal::Decimal,
    notional: rust_decimal::Decimal,
    decision_id: String,
    vwap_quote: AvgReboundVwapQuote,
    diagnostics: Value,
}

#[derive(Debug, Clone)]
struct AvgReboundTokenResolution {
    primary_token_id: String,
    primary_outcome_label: String,
    opposite_token_id: String,
    opposite_outcome_label: String,
}

#[derive(Debug, Clone)]
struct AvgReboundCheapestPrimarySelection {
    tokens: AvgReboundTokenResolution,
    primary_book: OrderBookSnapshot,
    opposite_book: OrderBookSnapshot,
    rejections: Vec<Value>,
}

#[derive(Debug, Clone)]
struct AvgReboundPrimarySideCandidate {
    side: &'static str,
    tokens: AvgReboundTokenResolution,
    primary_book: OrderBookSnapshot,
    opposite_book: OrderBookSnapshot,
    quote: AvgReboundVwapQuote,
}

fn avg_rebound_normalized_side(label: &str) -> Result<&'static str> {
    normalize_pair_lock_binary_outcome(label).ok_or_else(|| {
        anyhow::anyhow!(
            "avg_rebound_pairlock_rescue_v1 supports only binary YES/NO or Up/Down labels"
        )
    })
}

fn avg_rebound_primary_outcome_is_auto(config: &AvgReboundPairlockRescueConfig) -> bool {
    config
        .primary_outcome_label
        .trim()
        .eq_ignore_ascii_case("auto")
}

fn avg_rebound_token_resolution_for_side(
    resolved: &PairLockResolvedTokenPair,
    market_slug: &str,
    primary_side: &'static str,
) -> AvgReboundTokenResolution {
    let (yes_label, no_label) = pair_lock_monitor_outcome_labels(Some(market_slug));
    match primary_side {
        "no" => AvgReboundTokenResolution {
            primary_token_id: resolved.no_token_id.clone(),
            primary_outcome_label: no_label.to_string(),
            opposite_token_id: resolved.yes_token_id.clone(),
            opposite_outcome_label: yes_label.to_string(),
        },
        _ => AvgReboundTokenResolution {
            primary_token_id: resolved.yes_token_id.clone(),
            primary_outcome_label: yes_label.to_string(),
            opposite_token_id: resolved.no_token_id.clone(),
            opposite_outcome_label: no_label.to_string(),
        },
    }
}

fn avg_rebound_token_resolution_from_session(
    session: &bot_infra::db::TradeBuilderAvgReboundPairlockRescueSession,
) -> AvgReboundTokenResolution {
    AvgReboundTokenResolution {
        primary_token_id: session.primary_token_id.clone(),
        primary_outcome_label: session.primary_outcome_label.clone(),
        opposite_token_id: session.opposite_token_id.clone(),
        opposite_outcome_label: session.opposite_outcome_label.clone(),
    }
}

async fn avg_rebound_resolve_tokens(
    client: &dyn OrderExecutor,
    resolved: &PairLockResolvedTokenPair,
    market_slug: &str,
    config: &AvgReboundPairlockRescueConfig,
) -> Result<AvgReboundTokenResolution> {
    let primary_side = avg_rebound_normalized_side(&config.primary_outcome_label)?;
    let tokens = avg_rebound_token_resolution_for_side(resolved, market_slug, primary_side);
    if config
        .opposite_outcome_label
        .trim()
        .eq_ignore_ascii_case("opposite")
    {
        if let Some(info) = client
            .clob_market_info_by_token(&tokens.primary_token_id)
            .await?
        {
            anyhow::ensure!(
                info.tokens.len() == 2,
                "avg_rebound_pairlock_rescue_v1 opposite=opposite requires a binary market"
            );
        }
        return Ok(tokens);
    }

    let configured_opposite = avg_rebound_normalized_side(&config.opposite_outcome_label)?;
    anyhow::ensure!(
        configured_opposite != primary_side,
        "avg_rebound_pairlock_rescue_v1 oppositeOutcomeLabel must be opposite primary side"
    );
    Ok(tokens)
}

fn avg_rebound_primary_candidate(
    config: &AvgReboundPairlockRescueConfig,
    state: &AvgReboundRuntimeState,
    resolved: &PairLockResolvedTokenPair,
    market_slug: &str,
    side: &'static str,
    primary_book: OrderBookSnapshot,
    opposite_book: OrderBookSnapshot,
) -> std::result::Result<AvgReboundPrimarySideCandidate, Value> {
    let Some(tier) = avg_rebound_next_primary_tier(config, state) else {
        return Err(json!({
            "reason": "no_next_primary_tier",
            "side": side,
        }));
    };
    let quote = avg_rebound_vwap_for_fok_limit(
        &primary_book,
        tier.qty,
        tier.price_cap,
        rust_decimal::Decimal::ZERO,
    )
    .map_err(|rejection| {
        json!({
            "reason": "vwap_rejected",
            "side": side,
            "tier_or_leg_id": tier.id,
            "details": avg_rebound_vwap_rejection_json(&rejection),
        })
    })?;
    let min_notional = avg_rebound_market_min_notional_usdc();
    if quote.notional < min_notional {
        return Err(json!({
            "reason": "marketable_buy_min_notional_below_strategy_qty",
            "side": side,
            "tier_or_leg_id": tier.id,
            "notional": quote.notional.to_string(),
            "min_notional_usdc": min_notional.to_string(),
            "qty": tier.qty.to_string(),
            "vwap": quote.vwap.to_string(),
            "cap": tier.price_cap.to_string(),
        }));
    }
    Ok(AvgReboundPrimarySideCandidate {
        side,
        tokens: avg_rebound_token_resolution_for_side(resolved, market_slug, side),
        primary_book,
        opposite_book,
        quote,
    })
}

#[allow(clippy::too_many_arguments)]
fn avg_rebound_select_cheapest_primary(
    config: &AvgReboundPairlockRescueConfig,
    state: &AvgReboundRuntimeState,
    resolved: &PairLockResolvedTokenPair,
    market_slug: &str,
    yes_book: OrderBookSnapshot,
    no_book: OrderBookSnapshot,
) -> std::result::Result<AvgReboundCheapestPrimarySelection, Vec<Value>> {
    let up_candidate = avg_rebound_primary_candidate(
        config,
        state,
        resolved,
        market_slug,
        "yes",
        yes_book.clone(),
        no_book.clone(),
    );
    let down_candidate = avg_rebound_primary_candidate(
        config,
        state,
        resolved,
        market_slug,
        "no",
        no_book,
        yes_book,
    );
    let mut rejections = Vec::new();
    let mut candidates = Vec::new();
    for candidate in [up_candidate, down_candidate] {
        match candidate {
            Ok(candidate) => candidates.push(candidate),
            Err(rejection) => rejections.push(rejection),
        }
    }
    let Some(best) = candidates.into_iter().min_by(|left, right| {
        left.quote
            .vwap
            .cmp(&right.quote.vwap)
            .then_with(|| left.quote.notional.cmp(&right.quote.notional))
            .then_with(|| left.side.cmp(right.side))
    }) else {
        return Err(rejections);
    };
    Ok(AvgReboundCheapestPrimarySelection {
        tokens: best.tokens,
        primary_book: best.primary_book,
        opposite_book: best.opposite_book,
        rejections,
    })
}

fn avg_rebound_decision_id(
    session_id: i64,
    intent: &str,
    stage_id: Option<&str>,
    tier_or_leg_id: &str,
    qty: rust_decimal::Decimal,
    limit_price: rust_decimal::Decimal,
) -> String {
    format!(
        "{session_id}:{intent}:{}:{tier_or_leg_id}:{qty}:{limit_price}",
        stage_id.unwrap_or("none")
    )
}

fn avg_rebound_market_min_notional_usdc() -> rust_decimal::Decimal {
    avg_rebound_dec("1.0")
}

#[allow(clippy::too_many_arguments)]
fn avg_rebound_build_decision(
    config: &AvgReboundPairlockRescueConfig,
    state: &AvgReboundRuntimeState,
    token_id: &str,
    outcome_label: &str,
    leg_role: &'static str,
    intent: &'static str,
    session_status: &'static str,
    stage_id: Option<&str>,
    tier_or_leg_id: &str,
    qty: rust_decimal::Decimal,
    cap: rust_decimal::Decimal,
    book: &OrderBookSnapshot,
    extra_diagnostics: Value,
) -> std::result::Result<AvgReboundDecision, Value> {
    let Some(session_id) = state.session_id else {
        return Err(json!({ "reason": "missing_session_id" }));
    };
    let safety_buffer = if leg_role == "opposite" {
        config.extra_vwap_safety_buffer
    } else {
        rust_decimal::Decimal::ZERO
    };
    let vwap_quote = match avg_rebound_vwap_for_fok_limit(book, qty, cap, safety_buffer) {
        Ok(quote) => quote,
        Err(rejection) => {
            return Err(json!({
                "reason": "vwap_rejected",
                "details": avg_rebound_vwap_rejection_json(&rejection),
                "extra": extra_diagnostics,
            }));
        }
    };
    let min_notional = avg_rebound_market_min_notional_usdc();
    if vwap_quote.notional < min_notional {
        return Err(json!({
            "reason": "marketable_buy_min_notional_below_strategy_qty",
            "notional": vwap_quote.notional.to_string(),
            "min_notional_usdc": min_notional.to_string(),
            "qty": qty.to_string(),
            "vwap": vwap_quote.vwap.to_string(),
            "cap": cap.to_string(),
            "extra": extra_diagnostics,
        }));
    }
    if !avg_rebound_projected_spend_allowed(config, state, vwap_quote.notional) {
        return Err(json!({
            "reason": "budget_buffer_exceeded",
            "projected_spend": (state.primary_total_cost + state.opposite_total_cost + vwap_quote.notional).to_string(),
            "budget_limit": avg_rebound_budget_limit(config).to_string(),
            "extra": extra_diagnostics,
        }));
    }
    let decision_id = avg_rebound_decision_id(
        session_id,
        intent,
        stage_id,
        tier_or_leg_id,
        qty,
        vwap_quote.limit_price,
    );
    let projected_locked_pnl = if leg_role == "opposite" {
        Some(avg_rebound_projected_locked_pnl(
            state,
            qty,
            vwap_quote.vwap,
        ))
    } else {
        None
    };
    Ok(AvgReboundDecision {
        session_id,
        leg_role,
        intent,
        session_status,
        stage_id: stage_id.map(str::to_string),
        tier_or_leg_id: tier_or_leg_id.to_string(),
        token_id: token_id.to_string(),
        outcome_label: outcome_label.to_string(),
        qty,
        limit_price: vwap_quote.limit_price,
        vwap: vwap_quote.vwap,
        notional: vwap_quote.notional,
        decision_id,
        vwap_quote: vwap_quote.clone(),
        diagnostics: json!({
            "intent": intent,
            "role": leg_role,
            "stage_id": stage_id,
            "tier_or_leg_id": tier_or_leg_id,
            "qty": qty.to_string(),
            "cap": cap.to_string(),
            "vwap": avg_rebound_vwap_quote_json(&vwap_quote),
            "projected_locked_pnl": projected_locked_pnl.map(|value| value.to_string()),
            "extra": extra_diagnostics,
        }),
    })
}

fn avg_rebound_next_profit_leg<'a>(
    stage: &'a AvgReboundStageConfig,
    state: &AvgReboundRuntimeState,
) -> Option<&'a AvgReboundProfitLegConfig> {
    stage
        .profit_legs
        .iter()
        .find(|leg| !avg_rebound_has_opposite_leg(state, &leg.id))
}

fn avg_rebound_opposite_current_price(quote: &PairLockResolvedQuote) -> rust_decimal::Decimal {
    avg_rebound_decimal_from_f64(quote.current_price)
}

fn avg_rebound_dynamic_profit_cap(
    config: &AvgReboundPairlockRescueConfig,
    state: &AvgReboundRuntimeState,
    qty: rust_decimal::Decimal,
) -> Option<(rust_decimal::Decimal, rust_decimal::Decimal)> {
    let target_profit = config.target_profit_usdc?;
    if qty <= rust_decimal::Decimal::ZERO || state.primary_total_qty <= rust_decimal::Decimal::ZERO
    {
        return None;
    }
    let redeem_qty = avg_rebound_qty_min(state.primary_total_qty, state.opposite_filled_qty + qty);
    let current_total_cost = state.primary_total_cost + state.opposite_total_cost;
    let max_new_opposite_notional = redeem_qty - current_total_cost - target_profit;
    if max_new_opposite_notional <= rust_decimal::Decimal::ZERO {
        return None;
    }
    let effective_cap = max_new_opposite_notional / qty;
    if effective_cap <= rust_decimal::Decimal::ZERO {
        return None;
    }
    let raw_cap = effective_cap + config.extra_vwap_safety_buffer;
    let max_raw_cap = avg_rebound_dec("0.999999");
    Some((
        if raw_cap >= rust_decimal::Decimal::ONE {
            max_raw_cap
        } else {
            raw_cap
        },
        if effective_cap >= rust_decimal::Decimal::ONE {
            max_raw_cap
        } else {
            effective_cap
        },
    ))
}

#[allow(clippy::too_many_arguments)]
fn avg_rebound_profit_decision(
    config: &AvgReboundPairlockRescueConfig,
    state: &AvgReboundRuntimeState,
    tokens: &AvgReboundTokenResolution,
    opposite_book: &OrderBookSnapshot,
) -> Option<std::result::Result<AvgReboundDecision, Value>> {
    let stage = avg_rebound_current_stage(config, state)?;
    let leg = avg_rebound_next_profit_leg(stage, state)?;
    if state.open_primary_qty <= rust_decimal::Decimal::ZERO {
        return None;
    }
    let qty = avg_rebound_qty_min(leg.qty, state.open_primary_qty);
    let dynamic_cap = avg_rebound_dynamic_profit_cap(config, state, qty);
    let cap = dynamic_cap
        .map(|(raw_cap, _)| raw_cap)
        .unwrap_or(leg.opposite_vwap_cap);
    Some(avg_rebound_build_decision(
        config,
        state,
        &tokens.opposite_token_id,
        &tokens.opposite_outcome_label,
        "opposite",
        AVG_REBOUND_INTENT_PROFIT_PAIRLOCK,
        AVG_REBOUND_STATUS_PROFIT_LOCKING,
        Some(&stage.id),
        &leg.id,
        qty,
        cap,
        opposite_book,
        json!({
            "stage_ready": stage.id,
            "open_primary_qty": state.open_primary_qty.to_string(),
            "configured_opposite_vwap_cap": leg.opposite_vwap_cap.to_string(),
            "target_profit_usdc": config.target_profit_usdc.map(|value| value.to_string()),
            "dynamic_profit_raw_cap": dynamic_cap.map(|(raw_cap, _)| raw_cap.to_string()),
            "dynamic_profit_effective_cap": dynamic_cap.map(|(_, effective_cap)| effective_cap.to_string()),
        }),
    ))
}

#[allow(clippy::too_many_arguments)]
fn avg_rebound_guard_decision(
    config: &AvgReboundPairlockRescueConfig,
    state: &AvgReboundRuntimeState,
    tokens: &AvgReboundTokenResolution,
    opposite_quote: &PairLockResolvedQuote,
    opposite_book: &OrderBookSnapshot,
) -> Option<std::result::Result<AvgReboundDecision, Value>> {
    let stage = avg_rebound_current_stage(config, state)?;
    if !state.profit_started || state.open_primary_qty <= rust_decimal::Decimal::ZERO {
        return None;
    }
    let full_ladder_filled = avg_rebound_full_ladder_filled(config, state);
    if (!full_ladder_filled && !config.pre_full_giveback_guard_enabled)
        || (full_ladder_filled && !config.full_giveback_guard_enabled)
    {
        return None;
    }
    let current = avg_rebound_opposite_current_price(opposite_quote);
    if current < stage.giveback_guard.trigger {
        return None;
    }
    Some(avg_rebound_build_decision(
        config,
        state,
        &tokens.opposite_token_id,
        &tokens.opposite_outcome_label,
        "opposite",
        AVG_REBOUND_INTENT_GIVEBACK_GUARD,
        AVG_REBOUND_STATUS_GUARD_EXIT,
        Some(&stage.id),
        "giveback_guard",
        state.open_primary_qty,
        stage.giveback_guard.max_execution_vwap,
        opposite_book,
        json!({
            "guard_trigger": stage.giveback_guard.trigger.to_string(),
            "opposite_current_price": current.to_string(),
        }),
    ))
}

#[allow(clippy::too_many_arguments)]
fn avg_rebound_rescue_decision(
    config: &AvgReboundPairlockRescueConfig,
    state: &AvgReboundRuntimeState,
    tokens: &AvgReboundTokenResolution,
    opposite_book: &OrderBookSnapshot,
) -> Option<std::result::Result<AvgReboundDecision, Value>> {
    if config.rescue.enabled_only_after_full_ladder
        && !avg_rebound_full_ladder_filled(config, state)
    {
        return None;
    }
    if state.open_primary_qty <= rust_decimal::Decimal::ZERO {
        return None;
    }
    let stage_id = avg_rebound_current_stage(config, state).map(|stage| stage.id.as_str());
    let mut rescue_steps = vec![
        (
            AVG_REBOUND_INTENT_NORMAL_RESCUE,
            config.rescue.normal_vwap_cap,
            "normal_rescue",
        ),
        (
            AVG_REBOUND_INTENT_EMERGENCY_RESCUE,
            config.rescue.emergency_vwap_cap,
            "emergency_rescue",
        ),
        (
            AVG_REBOUND_INTENT_HARD_RESCUE,
            config.rescue.hard_max_vwap_cap,
            "hard_rescue",
        ),
    ];
    if let Some(cap) = config.rescue.last_chance_vwap_cap {
        rescue_steps.push((
            AVG_REBOUND_INTENT_LAST_CHANCE_RESCUE,
            cap,
            "last_chance_rescue",
        ));
    }
    for (intent, cap, leg_id) in rescue_steps {
        let decision = avg_rebound_build_decision(
            config,
            state,
            &tokens.opposite_token_id,
            &tokens.opposite_outcome_label,
            "opposite",
            intent,
            AVG_REBOUND_STATUS_RESCUE_EXIT,
            stage_id,
            leg_id,
            state.open_primary_qty,
            cap,
            opposite_book,
            json!({
                "full_ladder_filled": avg_rebound_full_ladder_filled(config, state),
            }),
        );
        if decision.is_ok() {
            return Some(decision);
        }
    }
    None
}

fn avg_rebound_primary_decision(
    config: &AvgReboundPairlockRescueConfig,
    state: &AvgReboundRuntimeState,
    tokens: &AvgReboundTokenResolution,
    primary_book: &OrderBookSnapshot,
) -> Option<std::result::Result<AvgReboundDecision, Value>> {
    if state.profit_started
        && (!config.allow_primary_after_partial_profit
            || state.open_primary_qty <= avg_rebound_dec("0.0001"))
    {
        return None;
    }
    let tier = avg_rebound_next_primary_tier(config, state)?;
    Some(avg_rebound_build_decision(
        config,
        state,
        &tokens.primary_token_id,
        &tokens.primary_outcome_label,
        "primary",
        AVG_REBOUND_INTENT_PRIMARY_LADDER,
        AVG_REBOUND_STATUS_BUILDING_PRIMARY,
        None,
        &tier.id,
        tier.qty,
        tier.price_cap,
        primary_book,
        json!({
            "next_primary_tier": tier.id,
        }),
    ))
}

fn avg_rebound_select_decision(
    config: &AvgReboundPairlockRescueConfig,
    state: &AvgReboundRuntimeState,
    tokens: &AvgReboundTokenResolution,
    primary_book: &OrderBookSnapshot,
    opposite_quote: &PairLockResolvedQuote,
    opposite_book: &OrderBookSnapshot,
) -> (Option<AvgReboundDecision>, Vec<Value>) {
    let mut rejections = Vec::new();
    if let Some(decision) = avg_rebound_profit_decision(config, state, tokens, opposite_book) {
        match decision {
            Ok(decision) => return (Some(decision), rejections),
            Err(rejection) => rejections.push(rejection),
        }
    }

    if avg_rebound_full_ladder_filled(config, state) {
        if let Some(decision) =
            avg_rebound_guard_decision(config, state, tokens, opposite_quote, opposite_book)
        {
            match decision {
                Ok(decision) => return (Some(decision), rejections),
                Err(rejection) => rejections.push(rejection),
            }
        }
        if let Some(decision) = avg_rebound_rescue_decision(config, state, tokens, opposite_book) {
            match decision {
                Ok(decision) => return (Some(decision), rejections),
                Err(rejection) => rejections.push(rejection),
            }
        }
    }

    if let Some(decision) = avg_rebound_primary_decision(config, state, tokens, primary_book) {
        match decision {
            Ok(decision) => return (Some(decision), rejections),
            Err(rejection) => rejections.push(rejection),
        }
    }
    (None, rejections)
}
