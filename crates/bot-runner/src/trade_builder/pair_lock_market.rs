#[derive(Debug, Clone, PartialEq, Eq)]
struct PairLockResolvedTokenPair {
    yes_token_id: String,
    no_token_id: String,
    token_resolution_source: &'static str,
    trigger_node_market_slug: Option<String>,
}

fn pair_lock_token_resolution_payload(resolved: &PairLockResolvedTokenPair) -> Value {
    json!({
        "resolved_yes_token_id": resolved.yes_token_id,
        "resolved_no_token_id": resolved.no_token_id,
        "token_resolution_source": resolved.token_resolution_source,
        "trigger_node_market_slug": resolved.trigger_node_market_slug,
    })
}

async fn resolve_trade_builder_pair_lock_session_config(
    repo: &PostgresRepository,
    session: &TradeBuilderPairSession,
) -> Result<Option<ActionPlaceOrderPairLockConfig>> {
    let Some(node) = resolve_trade_builder_pair_lock_node(repo, session).await? else {
        return Ok(None);
    };
    resolve_action_place_order_pair_lock_config(&node)
}

fn pair_lock_counter_market_end_eligible_before_at(market_slug: &str) -> Option<DateTime<Utc>> {
    let scope = find_updown_scope_by_slug(market_slug)?;
    let market_start = MarketCycleId(market_slug.to_string()).start_time()?;
    Some(market_start + ChronoDuration::seconds(updown_scope_window_seconds(scope)))
}

fn pair_lock_counter_waits_until_market_end(
    pair_lock: &ActionPlaceOrderPairLockConfig,
    market_slug: &str,
) -> bool {
    pair_lock.sizing_mode == ActionPlaceOrderPairLockSizingMode::AutoRemainingBudget
        && pair_lock.orphan_grace_ms == 0
        && pair_lock_counter_market_end_eligible_before_at(market_slug).is_some()
}

#[derive(Debug, Clone, PartialEq)]
struct PairLockAutoCounterRebalance {
    size_basis: &'static str,
    size_usdc: f64,
    remaining_size: Option<f64>,
    target_qty: Option<f64>,
    remaining_qty: Option<f64>,
    effective_max_price: Option<f64>,
    required_notional_usdc: Option<f64>,
    affordable_qty: Option<f64>,
    sizing_mode_after_rebase: &'static str,
}

fn resolve_pair_lock_auto_counter_rebalance(
    _order: &TradeBuilderOrder,
    session: &TradeBuilderPairSession,
    _pair_lock: &ActionPlaceOrderPairLockConfig,
    remaining_budget_usdc: f64,
) -> PairLockAutoCounterRebalance {
    let effective_max_price = trade_builder_pair_lock_effective_counter_cap(session);
    let Some(lead_net_qty) = session.primary_net_qty.filter(|value| *value > 0.0) else {
        return PairLockAutoCounterRebalance {
            size_basis: TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC,
            size_usdc: remaining_budget_usdc,
            remaining_size: Some(remaining_budget_usdc),
            target_qty: None,
            remaining_qty: None,
            effective_max_price,
            required_notional_usdc: None,
            affordable_qty: None,
            sizing_mode_after_rebase: "full_budget_notional",
        };
    };
    let Some(counter_cap) = effective_max_price.filter(|value| *value > 0.0) else {
        return PairLockAutoCounterRebalance {
            size_basis: TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC,
            size_usdc: remaining_budget_usdc,
            remaining_size: Some(remaining_budget_usdc),
            target_qty: None,
            remaining_qty: None,
            effective_max_price,
            required_notional_usdc: None,
            affordable_qty: None,
            sizing_mode_after_rebase: "full_budget_notional",
        };
    };

    let required_notional_usdc = lead_net_qty * counter_cap;
    if required_notional_usdc <= remaining_budget_usdc {
        return PairLockAutoCounterRebalance {
            size_basis: TRADE_BUILDER_SIZE_BASIS_SHARES,
            size_usdc: required_notional_usdc,
            remaining_size: Some(required_notional_usdc),
            target_qty: Some(lead_net_qty),
            remaining_qty: Some(lead_net_qty),
            effective_max_price: Some(counter_cap),
            required_notional_usdc: Some(required_notional_usdc),
            affordable_qty: Some(lead_net_qty),
            sizing_mode_after_rebase: "lead_qty_match",
        };
    }

    let affordable_qty = round_trade_builder_share_qty(remaining_budget_usdc / counter_cap);
    PairLockAutoCounterRebalance {
        size_basis: TRADE_BUILDER_SIZE_BASIS_SHARES,
        size_usdc: remaining_budget_usdc,
        remaining_size: Some(remaining_budget_usdc),
        target_qty: Some(affordable_qty),
        remaining_qty: Some(affordable_qty),
        effective_max_price: Some(counter_cap),
        required_notional_usdc: Some(required_notional_usdc),
        affordable_qty: Some(affordable_qty),
        sizing_mode_after_rebase: "partial_hedge",
    }
}

fn pair_lock_auto_counter_rebalance_changed(
    order: &TradeBuilderOrder,
    rebalance: &PairLockAutoCounterRebalance,
    counter_eligible_before_at: Option<DateTime<Utc>>,
) -> bool {
    let size_basis_changed =
        normalize_trade_builder_size_basis(&order.size_basis) != rebalance.size_basis;
    let eligible_before_changed = counter_eligible_before_at != order.eligible_before_at;
    if rebalance.size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        let size_usdc_changed = (order.size_usdc - rebalance.size_usdc).abs() >= 0.000001;
        let target_qty_changed = order
            .target_qty
            .zip(rebalance.target_qty)
            .map(|(lhs, rhs)| (lhs - rhs).abs() >= 0.000001)
            .unwrap_or(order.target_qty != rebalance.target_qty);
        let remaining_qty_changed = order
            .remaining_qty
            .zip(rebalance.remaining_qty)
            .map(|(lhs, rhs)| (lhs - rhs).abs() >= 0.000001)
            .unwrap_or(order.remaining_qty != rebalance.remaining_qty);
        let remaining_size_changed = order
            .remaining_size
            .zip(rebalance.remaining_size)
            .map(|(lhs, rhs)| (lhs - rhs).abs() >= 0.000001)
            .unwrap_or(order.remaining_size != rebalance.remaining_size);
        return size_basis_changed
            || eligible_before_changed
            || size_usdc_changed
            || target_qty_changed
            || remaining_qty_changed
            || remaining_size_changed;
    }

    let size_usdc_changed = (order.size_usdc - rebalance.size_usdc).abs() >= 0.000001;
    let remaining_size_changed = order
        .remaining_size
        .zip(rebalance.remaining_size)
        .map(|(lhs, rhs)| (lhs - rhs).abs() >= 0.000001)
        .unwrap_or(order.remaining_size != rebalance.remaining_size);
    size_basis_changed || eligible_before_changed || size_usdc_changed || remaining_size_changed
}

fn pair_lock_auto_counter_has_started_submit_lifecycle(order: &TradeBuilderOrder) -> bool {
    order.active_exchange_order_id.is_some()
        || order.submitted_dynamic_qty.is_some()
        || order.submitted_dynamic_price.is_some()
}

fn pair_lock_auto_counter_can_rebase(order: &TradeBuilderOrder) -> bool {
    matches!(
        order.status.as_str(),
        "pending" | "armed" | "triggered" | "guard_blocked" | "inventory_pending"
    ) && !pair_lock_auto_counter_has_started_submit_lifecycle(order)
}

fn pair_lock_auto_counter_needs_rebase(
    order: &TradeBuilderOrder,
    rebalance: &PairLockAutoCounterRebalance,
    counter_eligible_before_at: Option<DateTime<Utc>>,
) -> bool {
    pair_lock_auto_counter_can_rebase(order)
        && pair_lock_auto_counter_rebalance_changed(order, rebalance, counter_eligible_before_at)
}

fn pair_lock_counter_should_wait_for_lead(
    order: &TradeBuilderOrder,
    session: &TradeBuilderPairSession,
) -> bool {
    session.status == TRADE_BUILDER_PAIR_STATUS_WORKING
        && session.counter_order_id == Some(order.id)
        && session.lead_order_id.is_none()
}

fn pair_lock_primary_done_without_lead_fill(primary: &TradeBuilderOrder) -> bool {
    primary.filled_qty <= TRADE_BUILDER_PAIR_QTY_TOLERANCE
        && (primary.status == "error" || trade_builder_is_terminal_status(&primary.status))
}

async fn maybe_hold_trade_builder_pair_lock_counter_until_lead(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    session: &TradeBuilderPairSession,
) -> Result<bool> {
    if !pair_lock_counter_should_wait_for_lead(order, session) {
        return Ok(false);
    }

    if let Some(primary_order_id) = session.primary_order_id {
        if let Some(primary) = repo.get_trade_builder_order(primary_order_id).await? {
            if pair_lock_primary_done_without_lead_fill(&primary) {
                repo.set_trade_builder_order_status(
                    order.id,
                    "canceled",
                    Some("pair_primary_failed_before_fill"),
                )
                .await?;
                repo.update_trade_builder_pair_session_state(
                    session.id,
                    TRADE_BUILDER_PAIR_STATUS_ERROR,
                    None,
                    None,
                    Some("pair_primary_failed_before_fill"),
                )
                .await?;
                repo.append_trade_builder_order_event(
                    order.id,
                    "pair_lock_counter_canceled_before_lead",
                    &json!({
                        "pair_session_id": session.id,
                        "primary_order_id": primary_order_id,
                        "primary_status": primary.status,
                        "primary_last_error": primary.last_error,
                        "reason": "pair_primary_failed_before_fill",
                    }),
                )
                .await?;
                return Ok(true);
            }
        }
    }

    if order.status != "inventory_pending"
        || order.last_error.as_deref() != Some("pair_counter_waiting_primary_fill")
    {
        repo.set_trade_builder_order_status(
            order.id,
            "inventory_pending",
            Some("pair_counter_waiting_primary_fill"),
        )
        .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "pair_lock_counter_waiting_primary_fill",
            &json!({
                "pair_session_id": session.id,
                "primary_order_id": session.primary_order_id,
                "reason": "counter_before_primary_fill",
            }),
        )
        .await?;
    }
    Ok(true)
}

async fn maybe_prepare_trade_builder_pair_lock_auto_counter(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    session: &TradeBuilderPairSession,
    pair_lock: &ActionPlaceOrderPairLockConfig,
) -> Result<bool> {
    if session.counter_order_id != Some(order.id) {
        return Ok(false);
    }
    if pair_lock.sizing_mode != ActionPlaceOrderPairLockSizingMode::AutoRemainingBudget {
        return Ok(false);
    }
    if session.lead_order_id.is_none() {
        return Ok(true);
    }
    let Some(total_budget_usdc) = pair_lock.total_budget_usdc else {
        return Ok(true);
    };
    let Some(remaining_budget_usdc) =
        trade_builder_pair_lock_remaining_budget_usdc(total_budget_usdc, session)
    else {
        if maybe_skip_trade_builder_pair_protective_unwind(
            repo,
            session,
            "pair_budget_exhausted",
            TRADE_BUILDER_PAIR_STATUS_EXPIRED,
        )
        .await?
        {
            repo.set_trade_builder_order_status(
                order.id,
                "canceled",
                Some("pair_budget_exhausted"),
            )
            .await?;
            return Ok(true);
        }
        let orders = repo
            .list_trade_builder_orders_by_pair_session(session.id)
            .await?;
        schedule_trade_builder_pair_session_unwind(
            repo,
            session,
            &orders,
            TRADE_BUILDER_PAIR_STATUS_UNWINDING,
            "pair_budget_exhausted",
            None,
        )
        .await?;
        return Ok(true);
    };
    let counter_wait_until_market_end =
        pair_lock_counter_waits_until_market_end(pair_lock, &order.market_slug);
    let counter_eligible_before_at = if counter_wait_until_market_end {
        pair_lock_counter_market_end_eligible_before_at(&order.market_slug)
    } else {
        order.eligible_before_at
    };
    let rebalance =
        resolve_pair_lock_auto_counter_rebalance(order, session, pair_lock, remaining_budget_usdc);
    if !pair_lock_auto_counter_needs_rebase(order, &rebalance, counter_eligible_before_at) {
        return Ok(false);
    }
    repo.update_trade_builder_order_sizing_and_state(
        order.id,
        rebalance.size_basis,
        rebalance.size_usdc,
        rebalance.target_qty,
        rebalance.remaining_size,
        rebalance.remaining_qty,
        "pending",
        None,
        order.eligible_after_at,
        counter_eligible_before_at,
        None,
        None,
        None,
    )
    .await?;
    repo.append_trade_builder_order_event(
        order.id,
        "pair_lock_counter_remaining_budget_rebased",
        &json!({
            "pair_session_id": session.id,
            "pair_total_budget_usdc": total_budget_usdc,
            "actual_primary_spend_usdc": trade_builder_pair_lock_actual_primary_spend(session),
            "remaining_budget_usdc": remaining_budget_usdc,
            "counter_wait_mode": if counter_wait_until_market_end { "until_market_end" } else { "orphan_grace" },
            "counter_effective_max_price": rebalance.effective_max_price,
            "counter_eligible_before_at": counter_eligible_before_at.map(|value| value.to_rfc3339()),
            "counter_required_notional_usdc": rebalance.required_notional_usdc,
            "counter_affordable_qty": rebalance.affordable_qty,
            "counter_target_qty_after_rebase": rebalance.target_qty,
            "counter_sizing_mode_after_rebase": rebalance.sizing_mode_after_rebase,
            "status_after": "pending",
        }),
    )
    .await?;
    Ok(true)
}

fn pair_lock_trigger_node_state_token_pair(
    context: &Value,
    trigger_node_key: &str,
    market_slug: &str,
) -> Option<PairLockResolvedTokenPair> {
    let trigger_node_market_slug = node_auto_scope_state_string(
        context,
        trigger_node_key,
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_SLUG,
    )
    .or_else(|| flow_node_state_string(context, trigger_node_key, "last_ws_market_slug"));
    let yes_token_id = node_auto_scope_state_string(
        context,
        trigger_node_key,
        FLOW_NODE_STATE_AUTO_SCOPE_YES_TOKEN_ID,
    );
    let no_token_id = node_auto_scope_state_string(
        context,
        trigger_node_key,
        FLOW_NODE_STATE_AUTO_SCOPE_NO_TOKEN_ID,
    );

    match (
        trigger_node_market_slug.as_deref(),
        yes_token_id,
        no_token_id,
    ) {
        (Some(state_market_slug), Some(yes_token_id), Some(no_token_id))
            if state_market_slug == market_slug =>
        {
            Some(PairLockResolvedTokenPair {
                yes_token_id,
                no_token_id,
                token_resolution_source: "trigger_node_state",
                trigger_node_market_slug,
            })
        }
        _ => None,
    }
}

fn pair_lock_flow_context_token_pair(
    context: &Value,
    trigger_node_market_slug: Option<String>,
    market_slug: &str,
) -> Option<PairLockResolvedTokenPair> {
    let context_market_slug = flow_context_string(context, "marketSlug")?;
    if context_market_slug != market_slug {
        return None;
    }
    Some(PairLockResolvedTokenPair {
        yes_token_id: flow_context_string(context, "yesTokenId")?,
        no_token_id: flow_context_string(context, "noTokenId")?,
        token_resolution_source: "flow_context",
        trigger_node_market_slug,
    })
}

async fn resolve_pair_lock_trigger_scoped_token_pair(
    cfg: &AppConfig,
    market_slug: &str,
    trigger_node_key: &str,
    context: &Value,
) -> Result<PairLockResolvedTokenPair> {
    let trigger_node_market_slug = node_auto_scope_state_string(
        context,
        trigger_node_key,
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_SLUG,
    )
    .or_else(|| flow_node_state_string(context, trigger_node_key, "last_ws_market_slug"));
    if let Some(resolved) =
        pair_lock_trigger_node_state_token_pair(context, trigger_node_key, market_slug)
    {
        return Ok(resolved);
    }
    if let Some(resolved) =
        pair_lock_flow_context_token_pair(context, trigger_node_market_slug.clone(), market_slug)
    {
        return Ok(resolved);
    }

    let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
    let market = gamma
        .get_market_spec_by_slug(market_slug)
        .await?
        .ok_or_else(|| anyhow::anyhow!("pair_lock market spec not found for slug={market_slug}"))?;
    let yes_token_id = market.yes_token_id.ok_or_else(|| {
        anyhow::anyhow!("pair_lock market spec missing yes token for slug={market_slug}")
    })?;
    let no_token_id = market.no_token_id.ok_or_else(|| {
        anyhow::anyhow!("pair_lock market spec missing no token for slug={market_slug}")
    })?;
    Ok(PairLockResolvedTokenPair {
        yes_token_id,
        no_token_id,
        token_resolution_source: "gamma_market_spec",
        trigger_node_market_slug,
    })
}

#[cfg(test)]
fn trade_builder_pair_lock_market_waiting_reason(
    order: &TradeBuilderOrder,
    best_ask: Option<f64>,
) -> Option<&'static str> {
    if order.execution_mode != "market" || order.side != "buy" {
        return None;
    }
    if best_ask
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .is_some()
    {
        return None;
    }
    match order.pair_leg_role.as_deref() {
        Some(TRADE_BUILDER_PAIR_ROLE_COUNTER_CANDIDATE) => {
            Some("pair_counter_best_ask_unavailable")
        }
        Some(TRADE_BUILDER_PAIR_ROLE_LEAD_CANDIDATE) => Some("pair_primary_best_ask_unavailable"),
        _ => None,
    }
}

#[cfg(test)]
mod pair_lock_market_tests {
    use super::*;
    use chrono::Utc;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        path::Path,
        thread,
    };

    fn test_builder_order() -> TradeBuilderOrder {
        TradeBuilderOrder {
            id: 1,
            trade_id: 77,
            user_id: 1,
            kind: "immediate".to_string(),
            status: "pending".to_string(),
            market_slug: "btc-updown-5m-1".to_string(),
            token_id: "tok-up".to_string(),
            outcome_label: "Up".to_string(),
            side: "buy".to_string(),
            execution_mode: "market".to_string(),
            trigger_condition: None,
            trigger_price: None,
            max_price: None,
            size_basis: TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC.to_string(),
            size_usdc: 5.0,
            target_qty: None,
            min_price_distance_cent: 1.0,
            expires_at: None,
            eligible_after_at: None,
            eligible_before_at: None,
            max_triggers: 1,
            triggers_fired: 0,
            active_exchange_order_id: None,
            remaining_size: None,
            remaining_qty: None,
            working_price: None,
            last_seen_price: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            parent_order_id: None,
            origin_flow_definition_id: None,
            origin_flow_run_id: None,
            origin_flow_node_key: None,
            pair_session_id: None,
            pair_leg_role: None,
            tp_enabled: false,
            tp_price: None,
            tp_rules_json: Vec::new(),
            sl_enabled: false,
            sl_price: None,
            sl_rules_json: Vec::new(),
            time_exit_rules_json: Vec::new(),
            filled_qty: 0.0,
            fee_rate_bps: 0,
            trigger_latched: false,
            trigger_latched_reason: None,
            trigger_latched_at: None,
            submitted_dynamic_qty: None,
            submitted_dynamic_price: None,
            runtime_snapshot_json: None,
            fresh_submit_lease_until: None,
            guard_trigger_price: None,
            best_ask_floor_price: None,
            retry_on_trigger_guard_block: false,
            retry_on_execution_floor_guard_block: false,
            retry_on_max_price_block: false,
            ptb_stop_loss_gap_usd: None,
            ptb_reference_price: None,
            ptb_stop_loss_rules_json: Vec::new(),
            ptb_stop_loss_time_decay_mode: None,
            ptb_current_price_source: "chainlink".to_string(),
            staged_sl_retry_only_dust: false,
            staged_sl_retry_dust_metric: None,
            staged_sl_retry_dust_value: None,
            staged_sl_reentry_use_sold_notional: false,
            staged_sl_reentry_only_after_all_stages: false,
            sl_trigger_price_mode: None,
            reenter_on_sl_hit: false,
            reentry_max_attempts: 0,
            reentry_trigger_node_key: None,
            notify_on_order_submitted: false,
            notify_on_fill: false,
            notify_on_order_not_filled: false,
            notify_on_trigger_guard_blocked: false,
            notify_on_execution_floor_blocked: false,
            notify_on_tp_hit: false,
            notify_on_sl_hit: false,
            notify_on_max_price_blocked: false,
            last_guard_notification_reason: None,
            exit_ladder_kind: None,
            exit_ladder_index: None,
            exit_ladder_size_pct: None,
        }
    }

    #[test]
    fn pair_lock_market_waiting_reason_detects_primary_and_counter() {
        let mut primary = test_builder_order();
        primary.execution_mode = "market".to_string();
        primary.pair_leg_role = Some(TRADE_BUILDER_PAIR_ROLE_LEAD_CANDIDATE.to_string());
        assert_eq!(
            trade_builder_pair_lock_market_waiting_reason(&primary, None),
            Some("pair_primary_best_ask_unavailable")
        );

        let mut counter = test_builder_order();
        counter.execution_mode = "market".to_string();
        counter.pair_leg_role = Some(TRADE_BUILDER_PAIR_ROLE_COUNTER_CANDIDATE.to_string());
        assert_eq!(
            trade_builder_pair_lock_market_waiting_reason(&counter, None),
            Some("pair_counter_best_ask_unavailable")
        );
        assert_eq!(
            trade_builder_pair_lock_market_waiting_reason(&counter, Some(0.42)),
            None
        );
    }

    #[test]
    fn pair_lock_trigger_node_state_token_pair_ignores_stale_global_flow_context() {
        let context = json!({
            "flowContext": {
                "marketSlug": "btc-updown-5m-global-stale",
                "yesTokenId": "btc-yes-global-stale",
                "noTokenId": "btc-no-global-stale"
            },
            "nodeState": {
                "trigger_market": {
                    "auto_scope_market_slug": "btc-updown-5m-1776521100",
                    "auto_scope_yes_token_id": "btc-yes-current",
                    "auto_scope_no_token_id": "btc-no-current"
                }
            }
        });

        let resolved = pair_lock_trigger_node_state_token_pair(
            &context,
            "trigger_market",
            "btc-updown-5m-1776521100",
        )
        .expect("trigger node state resolution");

        assert_eq!(resolved.yes_token_id, "btc-yes-current");
        assert_eq!(resolved.no_token_id, "btc-no-current");
        assert_eq!(resolved.token_resolution_source, "trigger_node_state");
    }

    #[test]
    fn pair_lock_counter_waits_until_market_end_when_auto_budget_and_zero_grace() {
        let pair_lock = ActionPlaceOrderPairLockConfig {
            max_total_price: 0.9,
            orphan_grace_ms: 0,
            protective_unwind_enabled: true,
            ignore_stop_loss_after_locked: false,
            notify_on_pair_locked: true,
            notify_on_pair_unwind: true,
            sizing_mode: ActionPlaceOrderPairLockSizingMode::AutoRemainingBudget,
            primary_leg_size_usdc: 5.0,
            total_budget_usdc: Some(10.0),
            counter_leg_size_usdc: Some(5.0),
        };

        assert!(pair_lock_counter_waits_until_market_end(
            &pair_lock,
            "btc-updown-5m-1776522900"
        ));
        assert_eq!(
            pair_lock_counter_market_end_eligible_before_at("btc-updown-5m-1776522900"),
            Some(
                MarketCycleId("btc-updown-5m-1776522900".to_string())
                    .start_time()
                    .expect("market start")
                    + ChronoDuration::seconds(300)
            )
        );
    }

    #[test]
    fn pair_lock_counter_waits_until_market_end_is_disabled_for_manual_or_positive_grace() {
        let mut pair_lock = ActionPlaceOrderPairLockConfig {
            max_total_price: 0.9,
            orphan_grace_ms: 0,
            protective_unwind_enabled: true,
            ignore_stop_loss_after_locked: false,
            notify_on_pair_locked: true,
            notify_on_pair_unwind: true,
            sizing_mode: ActionPlaceOrderPairLockSizingMode::Manual,
            primary_leg_size_usdc: 5.0,
            total_budget_usdc: None,
            counter_leg_size_usdc: Some(5.0),
        };
        assert!(!pair_lock_counter_waits_until_market_end(
            &pair_lock,
            "btc-updown-5m-1776522900"
        ));

        pair_lock.sizing_mode = ActionPlaceOrderPairLockSizingMode::AutoRemainingBudget;
        pair_lock.orphan_grace_ms = 1500;
        assert!(!pair_lock_counter_waits_until_market_end(
            &pair_lock,
            "btc-updown-5m-1776522900"
        ));
    }

    #[test]
    fn pair_lock_counter_waits_for_lead_even_in_manual_sizing() {
        let mut counter = test_builder_order();
        counter.id = 12;
        counter.pair_leg_role = Some(TRADE_BUILDER_PAIR_ROLE_COUNTER_CANDIDATE.to_string());
        let session = TradeBuilderPairSession {
            id: 1,
            user_id: 1,
            flow_definition_id: None,
            flow_run_id: None,
            flow_node_key: None,
            market_slug: "btc-updown-5m-1776522900".to_string(),
            status: TRADE_BUILDER_PAIR_STATUS_WORKING.to_string(),
            pair_target_total_cent: 90.0,
            min_net_profit_usdc: 0.0,
            profit_safety_buffer_usdc: 0.0,
            orphan_grace_ms: 0,
            ignore_stop_loss_after_locked: false,
            notify_on_pair_locked: true,
            notify_on_pair_unwind: true,
            notify_on_pair_no_edge: false,
            primary_order_id: Some(11),
            counter_order_id: Some(12),
            lead_order_id: None,
            primary_fill_qty: None,
            primary_fill_fee_qty: None,
            primary_net_qty: None,
            primary_avg_fill_price: None,
            counter_fill_qty: None,
            counter_fill_fee_qty: None,
            counter_net_qty: None,
            counter_avg_fill_price: None,
            lead_filled_at: None,
            locked_qty: None,
            projected_net_profit_usdc: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(pair_lock_counter_should_wait_for_lead(&counter, &session));
    }

    #[test]
    fn pair_lock_primary_done_without_lead_fill_detects_error_or_terminal_no_fill() {
        let mut primary = test_builder_order();
        primary.status = "error".to_string();
        primary.filled_qty = 0.0;
        assert!(pair_lock_primary_done_without_lead_fill(&primary));

        primary.status = "canceled".to_string();
        assert!(pair_lock_primary_done_without_lead_fill(&primary));

        primary.status = "completed".to_string();
        primary.filled_qty = 1.0;
        assert!(!pair_lock_primary_done_without_lead_fill(&primary));
    }

    #[test]
    fn resolve_pair_lock_auto_counter_rebalance_matches_lead_qty_when_budget_is_sufficient() {
        let order = test_builder_order();
        let pair_lock = ActionPlaceOrderPairLockConfig {
            max_total_price: 0.9,
            orphan_grace_ms: 0,
            protective_unwind_enabled: true,
            ignore_stop_loss_after_locked: false,
            notify_on_pair_locked: true,
            notify_on_pair_unwind: true,
            sizing_mode: ActionPlaceOrderPairLockSizingMode::AutoRemainingBudget,
            primary_leg_size_usdc: 5.0,
            total_budget_usdc: Some(10.0),
            counter_leg_size_usdc: Some(5.0),
        };
        let session = TradeBuilderPairSession {
            id: 1,
            user_id: 1,
            flow_definition_id: None,
            flow_run_id: None,
            flow_node_key: None,
            market_slug: "btc-updown-5m-1776522900".to_string(),
            status: "working".to_string(),
            pair_target_total_cent: 90.0,
            min_net_profit_usdc: 0.0,
            profit_safety_buffer_usdc: 0.0,
            orphan_grace_ms: 0,
            ignore_stop_loss_after_locked: false,
            notify_on_pair_locked: true,
            notify_on_pair_unwind: true,
            notify_on_pair_no_edge: false,
            primary_order_id: Some(1),
            counter_order_id: Some(2),
            lead_order_id: Some(1),
            primary_fill_qty: Some(7.25),
            primary_fill_fee_qty: Some(0.0),
            primary_net_qty: Some(7.25),
            primary_avg_fill_price: Some(0.69),
            counter_fill_qty: None,
            counter_fill_fee_qty: None,
            counter_net_qty: None,
            counter_avg_fill_price: None,
            lead_filled_at: None,
            locked_qty: None,
            projected_net_profit_usdc: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let rebalance =
            resolve_pair_lock_auto_counter_rebalance(&order, &session, &pair_lock, 4.9975);
        assert_eq!(rebalance.size_basis, TRADE_BUILDER_SIZE_BASIS_SHARES);
        assert_eq!(rebalance.sizing_mode_after_rebase, "lead_qty_match");
        assert_eq!(rebalance.target_qty, Some(7.25));
        assert_eq!(rebalance.remaining_qty, Some(7.25));
        assert!((rebalance.size_usdc - 1.5225).abs() < 0.000001);
    }

    #[test]
    fn resolve_pair_lock_auto_counter_rebalance_falls_back_to_partial_hedge_when_budget_is_insufficient(
    ) {
        let order = test_builder_order();
        let pair_lock = ActionPlaceOrderPairLockConfig {
            max_total_price: 0.9,
            orphan_grace_ms: 0,
            protective_unwind_enabled: true,
            ignore_stop_loss_after_locked: false,
            notify_on_pair_locked: true,
            notify_on_pair_unwind: true,
            sizing_mode: ActionPlaceOrderPairLockSizingMode::AutoRemainingBudget,
            primary_leg_size_usdc: 5.0,
            total_budget_usdc: Some(10.0),
            counter_leg_size_usdc: Some(5.0),
        };
        let session = TradeBuilderPairSession {
            id: 1,
            user_id: 1,
            flow_definition_id: None,
            flow_run_id: None,
            flow_node_key: None,
            market_slug: "btc-updown-5m-1776522900".to_string(),
            status: "working".to_string(),
            pair_target_total_cent: 90.0,
            min_net_profit_usdc: 0.0,
            profit_safety_buffer_usdc: 0.0,
            orphan_grace_ms: 0,
            ignore_stop_loss_after_locked: false,
            notify_on_pair_locked: true,
            notify_on_pair_unwind: true,
            notify_on_pair_no_edge: false,
            primary_order_id: Some(1),
            counter_order_id: Some(2),
            lead_order_id: Some(1),
            primary_fill_qty: Some(25.0),
            primary_fill_fee_qty: Some(0.0),
            primary_net_qty: Some(25.0),
            primary_avg_fill_price: Some(0.20),
            counter_fill_qty: None,
            counter_fill_fee_qty: None,
            counter_net_qty: None,
            counter_avg_fill_price: None,
            lead_filled_at: None,
            locked_qty: None,
            projected_net_profit_usdc: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let rebalance = resolve_pair_lock_auto_counter_rebalance(&order, &session, &pair_lock, 5.0);
        assert_eq!(rebalance.size_basis, TRADE_BUILDER_SIZE_BASIS_SHARES);
        assert_eq!(rebalance.sizing_mode_after_rebase, "partial_hedge");
        assert_eq!(rebalance.target_qty, Some(7.14));
        assert_eq!(rebalance.remaining_qty, Some(7.14));
        assert_eq!(rebalance.size_usdc, 5.0);
    }

    #[test]
    fn pair_lock_auto_counter_rebalance_changed_returns_false_when_shares_state_already_applied() {
        let mut order = test_builder_order();
        order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
        order.size_usdc = 1.4973;
        order.remaining_size = Some(1.4973);
        order.target_qty = Some(7.13);
        order.remaining_qty = Some(7.13);
        order.eligible_before_at =
            pair_lock_counter_market_end_eligible_before_at("btc-updown-5m-1776522900");
        let rebalance = PairLockAutoCounterRebalance {
            size_basis: TRADE_BUILDER_SIZE_BASIS_SHARES,
            size_usdc: 1.4973,
            remaining_size: Some(1.4973),
            target_qty: Some(7.13),
            remaining_qty: Some(7.13),
            effective_max_price: Some(0.21),
            required_notional_usdc: Some(1.4973),
            affordable_qty: Some(7.13),
            sizing_mode_after_rebase: "lead_qty_match",
        };

        assert!(!pair_lock_auto_counter_rebalance_changed(
            &order,
            &rebalance,
            order.eligible_before_at,
        ));
    }

    #[test]
    fn pair_lock_auto_counter_rebalance_changed_returns_true_when_shares_transition_is_new() {
        let order = test_builder_order();
        let rebalance = PairLockAutoCounterRebalance {
            size_basis: TRADE_BUILDER_SIZE_BASIS_SHARES,
            size_usdc: 1.4973,
            remaining_size: Some(1.4973),
            target_qty: Some(7.13),
            remaining_qty: Some(7.13),
            effective_max_price: Some(0.21),
            required_notional_usdc: Some(1.4973),
            affordable_qty: Some(7.13),
            sizing_mode_after_rebase: "lead_qty_match",
        };

        assert!(pair_lock_auto_counter_rebalance_changed(
            &order,
            &rebalance,
            pair_lock_counter_market_end_eligible_before_at("btc-updown-5m-1776522900"),
        ));
    }

    #[test]
    fn pair_lock_auto_counter_needs_rebase_only_once_for_stable_pending_order() {
        let mut order = test_builder_order();
        let counter_eligible_before_at =
            pair_lock_counter_market_end_eligible_before_at("btc-updown-5m-1776531600");
        let rebalance = PairLockAutoCounterRebalance {
            size_basis: TRADE_BUILDER_SIZE_BASIS_SHARES,
            size_usdc: 1.406,
            remaining_size: Some(1.406),
            target_qty: Some(7.03),
            remaining_qty: Some(7.03),
            effective_max_price: Some(0.20),
            required_notional_usdc: Some(1.406),
            affordable_qty: Some(7.03),
            sizing_mode_after_rebase: "lead_qty_match",
        };

        assert!(pair_lock_auto_counter_needs_rebase(
            &order,
            &rebalance,
            counter_eligible_before_at,
        ));

        order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
        order.size_usdc = 1.406;
        order.remaining_size = Some(1.406);
        order.target_qty = Some(7.03);
        order.remaining_qty = Some(7.03);
        order.eligible_before_at = counter_eligible_before_at;

        assert!(!pair_lock_auto_counter_needs_rebase(
            &order,
            &rebalance,
            counter_eligible_before_at,
        ));
    }

    #[test]
    fn pair_lock_auto_counter_needs_rebase_for_guard_waiting_order_only_when_state_changes() {
        let mut order = test_builder_order();
        order.status = TRADE_BUILDER_GUARD_BLOCKED_STATUS.to_string();
        order.last_error = Some("above_max_price".to_string());
        let counter_eligible_before_at =
            pair_lock_counter_market_end_eligible_before_at("btc-updown-5m-1776531600");
        let rebalance = PairLockAutoCounterRebalance {
            size_basis: TRADE_BUILDER_SIZE_BASIS_SHARES,
            size_usdc: 1.406,
            remaining_size: Some(1.406),
            target_qty: Some(7.03),
            remaining_qty: Some(7.03),
            effective_max_price: Some(0.20),
            required_notional_usdc: Some(1.406),
            affordable_qty: Some(7.03),
            sizing_mode_after_rebase: "lead_qty_match",
        };

        order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
        order.size_usdc = 1.406;
        order.remaining_size = Some(1.406);
        order.target_qty = Some(7.03);
        order.remaining_qty = Some(7.03);
        order.eligible_before_at = counter_eligible_before_at;

        assert!(!pair_lock_auto_counter_needs_rebase(
            &order,
            &rebalance,
            counter_eligible_before_at,
        ));
    }

    #[test]
    fn pair_lock_auto_counter_rebase_skips_orders_that_started_submit_lifecycle() {
        let mut order = test_builder_order();
        let counter_eligible_before_at =
            pair_lock_counter_market_end_eligible_before_at("btc-updown-5m-1776531600");
        let rebalance = PairLockAutoCounterRebalance {
            size_basis: TRADE_BUILDER_SIZE_BASIS_SHARES,
            size_usdc: 1.406,
            remaining_size: Some(1.406),
            target_qty: Some(7.03),
            remaining_qty: Some(7.03),
            effective_max_price: Some(0.20),
            required_notional_usdc: Some(1.406),
            affordable_qty: Some(7.03),
            sizing_mode_after_rebase: "lead_qty_match",
        };

        order.active_exchange_order_id = Some("ex-13051".to_string());
        assert!(!pair_lock_auto_counter_can_rebase(&order));
        assert!(!pair_lock_auto_counter_needs_rebase(
            &order,
            &rebalance,
            counter_eligible_before_at,
        ));

        order.active_exchange_order_id = None;
        order.submitted_dynamic_qty = Some(7.03);
        order.submitted_dynamic_price = Some(0.20);
        assert!(!pair_lock_auto_counter_can_rebase(&order));
        assert!(!pair_lock_auto_counter_needs_rebase(
            &order,
            &rebalance,
            counter_eligible_before_at,
        ));
    }

    fn spawn_gamma_market_spec_server(response_body: String) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind gamma test server");
        let addr = listener.local_addr().expect("gamma test server addr");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept gamma request");
            let mut buffer = [0_u8; 1024];
            let _ = stream.read(&mut buffer);
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write gamma response");
        });
        (format!("http://{}", addr), handle)
    }

    #[tokio::test]
    async fn resolve_pair_lock_trigger_scoped_token_pair_falls_back_to_gamma_on_market_mismatch() {
        let (server_url, handle) = spawn_gamma_market_spec_server(
            r#"[{"slug":"btc-updown-5m-1776521100","active":true,"closed":false,"yesTokenId":"gamma-yes","noTokenId":"gamma-no"}]"#
                .to_string(),
        );
        let mut cfg = AppConfig::load(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../config"
        )))
        .expect("app config");
        cfg.exchange.gamma_base_url = server_url;
        let context = json!({
            "flowContext": {
                "yesTokenId": "stale-global-yes",
                "noTokenId": "stale-global-no"
            },
            "nodeState": {
                "trigger_market": {
                    "auto_scope_market_slug": "btc-updown-5m-1776518700",
                    "auto_scope_yes_token_id": "stale-node-yes",
                    "auto_scope_no_token_id": "stale-node-no"
                }
            }
        });

        let resolved = resolve_pair_lock_trigger_scoped_token_pair(
            &cfg,
            "btc-updown-5m-1776521100",
            "trigger_market",
            &context,
        )
        .await
        .expect("gamma fallback resolution");

        handle.join().expect("join gamma test server");
        assert_eq!(resolved.yes_token_id, "gamma-yes");
        assert_eq!(resolved.no_token_id, "gamma-no");
        assert_eq!(resolved.token_resolution_source, "gamma_market_spec");
        assert_eq!(
            resolved.trigger_node_market_slug.as_deref(),
            Some("btc-updown-5m-1776518700")
        );
    }

    #[tokio::test]
    async fn resolve_pair_lock_trigger_scoped_token_pair_uses_flow_context_tokens_before_gamma() {
        let mut cfg = AppConfig::load(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../config"
        )))
        .expect("app config");
        cfg.exchange.gamma_base_url = "http://127.0.0.1:1".to_string();
        let context = json!({
            "flowContext": {
                "marketSlug": "btc-updown-5m-1776521100",
                "yesTokenId": "context-yes",
                "noTokenId": "context-no"
            }
        });

        let resolved = resolve_pair_lock_trigger_scoped_token_pair(
            &cfg,
            "btc-updown-5m-1776521100",
            "trigger_market",
            &context,
        )
        .await
        .expect("flow context resolution");

        assert_eq!(resolved.yes_token_id, "context-yes");
        assert_eq!(resolved.no_token_id, "context-no");
        assert_eq!(resolved.token_resolution_source, "flow_context");
    }
}
