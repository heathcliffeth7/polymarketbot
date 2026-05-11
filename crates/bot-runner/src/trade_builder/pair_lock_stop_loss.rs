fn trade_builder_pair_lock_stop_loss_surface_active_from_session(
    session: &TradeBuilderPairSession,
    order_id: i64,
) -> bool {
    match session.status.as_str() {
        TRADE_BUILDER_PAIR_STATUS_WORKING => session.lead_order_id == Some(order_id),
        TRADE_BUILDER_PAIR_STATUS_LOCKED if session.ignore_stop_loss_after_locked => false,
        TRADE_BUILDER_PAIR_STATUS_LOCKED => {
            session.primary_order_id == Some(order_id) || session.counter_order_id == Some(order_id)
        }
        _ => false,
    }
}

async fn trade_builder_pair_lock_stop_loss_surface_active(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
) -> Result<bool> {
    if !trade_builder_order_uses_pair_lock(order)
        || !trade_builder_pair_lock_is_candidate_order(order)
        || order.side != "buy"
    {
        return Ok(true);
    }
    let Some(pair_session_id) = order.pair_session_id else {
        return Ok(false);
    };
    let Some(session) = repo.get_trade_builder_pair_session(pair_session_id).await? else {
        return Ok(false);
    };
    Ok(trade_builder_pair_lock_stop_loss_surface_active_from_session(
        &session, order.id,
    ))
}

async fn maybe_cancel_trade_builder_pair_lock_stop_loss_after_locked(
    repo: &PostgresRepository,
    session: &TradeBuilderPairSession,
    orders: &[TradeBuilderOrder],
) -> Result<()> {
    if !session.ignore_stop_loss_after_locked {
        return Ok(());
    }
    let canceled_stop_loss_child_ids =
        cancel_trade_builder_pair_lock_stop_loss_children(repo, orders, "pair_locked_stop_loss_ignored")
            .await?;
    append_trade_builder_pair_lock_event(
        repo,
        session,
        "pair_lock_locked_stop_loss_ignored",
        json!({
            "pair_session_id": session.id,
            "reason": "pair_locked_stop_loss_ignored",
            "canceled_stop_loss_child_ids": canceled_stop_loss_child_ids,
        }),
    )
    .await?;
    Ok(())
}

async fn cancel_trade_builder_pair_lock_stop_loss_children(
    repo: &PostgresRepository,
    orders: &[TradeBuilderOrder],
    reason: &str,
) -> Result<Vec<i64>> {
    let mut canceled_child_ids = Vec::new();
    for order in orders
        .iter()
        .filter(|order| trade_builder_pair_lock_is_candidate_order(order))
    {
        let children = repo
            .list_trade_builder_child_orders_by_parent(order.id, None)
            .await?;
        for child in children.into_iter().filter(|child| {
            trade_builder_is_stop_loss_child(child) && !trade_builder_is_terminal_status(&child.status)
        }) {
            let next_status = if child.active_exchange_order_id.is_some() {
                "canceled_requested"
            } else {
                "canceled"
            };
            repo.set_trade_builder_order_status(child.id, next_status, Some(reason))
                .await?;
            repo.append_trade_builder_order_event(
                child.id,
                "pair_lock_stop_loss_child_canceled",
                &json!({
                    "pair_session_id": order.pair_session_id,
                    "parent_order_id": order.id,
                    "reason": reason,
                    "status_after": next_status,
                }),
            )
            .await?;
            canceled_child_ids.push(child.id);
        }
    }
    Ok(canceled_child_ids)
}

async fn cancel_trade_builder_pair_lock_unwind_orders(
    repo: &PostgresRepository,
    orders: &[TradeBuilderOrder],
    reason: &str,
) -> Result<Vec<i64>> {
    let mut canceled_unwind_order_ids = Vec::new();
    for order in orders
        .iter()
        .filter(|order| trade_builder_pair_lock_is_unwind_order(order))
    {
        if trade_builder_is_terminal_status(&order.status) {
            continue;
        }
        let next_status = if order.active_exchange_order_id.is_some() {
            "canceled_requested"
        } else {
            "canceled"
        };
        repo.set_trade_builder_order_status(order.id, next_status, Some(reason))
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "pair_lock_unwind_canceled",
            &json!({
                "pair_session_id": order.pair_session_id,
                "reason": reason,
                "status_after": next_status,
            }),
        )
        .await?;
        canceled_unwind_order_ids.push(order.id);
    }
    Ok(canceled_unwind_order_ids)
}

async fn detach_trade_builder_pair_lock_surviving_candidates(
    repo: &PostgresRepository,
    orders: &[TradeBuilderOrder],
    filled_parent_order_id: i64,
    reason: &str,
) -> Result<Vec<i64>> {
    let mut detached_candidate_order_ids = Vec::new();
    for order in orders.iter().filter(|order| {
        trade_builder_pair_lock_is_candidate_order(order) && order.id != filled_parent_order_id
    }) {
        repo.set_trade_builder_order_pair_session(order.id, None, None)
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "pair_lock_candidate_detached",
            &json!({
                "pair_session_id": order.pair_session_id,
                "reason": reason,
            }),
        )
        .await?;
        detached_candidate_order_ids.push(order.id);
    }
    Ok(detached_candidate_order_ids)
}

async fn maybe_finalize_trade_builder_pair_lock_after_lead_stop_loss_fill(
    repo: &PostgresRepository,
    parent_order: &TradeBuilderOrder,
    stop_loss_order: &TradeBuilderOrder,
) -> Result<()> {
    if !trade_builder_is_stop_loss_child(stop_loss_order)
        || !trade_builder_order_uses_pair_lock(parent_order)
        || !trade_builder_pair_lock_is_candidate_order(parent_order)
    {
        return Ok(());
    }

    let Some(pair_session_id) = parent_order.pair_session_id else {
        return Ok(());
    };
    let Some(session) = repo.get_trade_builder_pair_session(pair_session_id).await? else {
        return Ok(());
    };
    if session.status != TRADE_BUILDER_PAIR_STATUS_WORKING {
        return Ok(());
    }
    if !trade_builder_pair_lock_stop_loss_surface_active_from_session(&session, parent_order.id) {
        return Ok(());
    }

    let orders = repo
        .list_trade_builder_orders_by_pair_session(pair_session_id)
        .await?;
    let canceled_stop_loss_child_ids =
        cancel_trade_builder_pair_lock_stop_loss_children(repo, &orders, "lead_leg_stop_loss")
            .await?;
    let mut canceled_candidate_order_ids = Vec::new();
    for order in orders.iter().filter(|order| {
        order.id != parent_order.id
            && trade_builder_pair_lock_is_candidate_order(order)
            && !trade_builder_is_terminal_status(&order.status)
    }) {
        let next_status = if order.active_exchange_order_id.is_some() {
            "canceled_requested"
        } else {
            "canceled"
        };
        repo.set_trade_builder_order_status(order.id, next_status, Some("lead_leg_stop_loss"))
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "pair_lock_candidate_canceled",
            &json!({
                "pair_session_id": session.id,
                "reason": "lead_leg_stop_loss",
                "status_after": next_status,
            }),
        )
        .await?;
        canceled_candidate_order_ids.push(order.id);
    }

    repo.update_trade_builder_pair_session_state(
        session.id,
        TRADE_BUILDER_PAIR_STATUS_COMPLETED,
        session.locked_qty,
        session.projected_net_profit_usdc,
        Some("lead_leg_stop_loss"),
    )
    .await?;
    append_trade_builder_pair_lock_event(
        repo,
        &session,
        "pair_lock_lead_leg_stop_loss",
        json!({
            "pair_session_id": session.id,
            "parent_order_id": parent_order.id,
            "sl_child_order_id": stop_loss_order.id,
            "reason": "lead_leg_stop_loss",
            "canceled_candidate_order_ids": canceled_candidate_order_ids,
            "canceled_stop_loss_child_ids": canceled_stop_loss_child_ids,
        }),
    )
    .await?;
    append_trade_builder_pair_lock_event(
        repo,
        &session,
        "pair_lock_session_state_changed",
        json!({
            "pair_session_id": session.id,
            "status_after": TRADE_BUILDER_PAIR_STATUS_COMPLETED,
            "reason": "lead_leg_stop_loss",
            "canceled_candidate_order_ids": canceled_candidate_order_ids,
            "canceled_stop_loss_child_ids": canceled_stop_loss_child_ids,
        }),
    )
    .await?;
    if let Some(reference_order) = orders
        .iter()
        .find(|candidate| candidate.id == session.lead_order_id.unwrap_or_default())
        .or_else(|| orders.first())
    {
        maybe_send_trade_builder_pair_notification(
            repo,
            reference_order,
            "pair_unwind",
            &build_trade_builder_pair_unwind_message(&session, "lead_leg_stop_loss"),
            session.notify_on_pair_unwind,
        )
        .await;
    }
    Ok(())
}

async fn maybe_finalize_trade_builder_pair_lock_after_locked_leg_stop_loss_fill(
    repo: &PostgresRepository,
    parent_order: &TradeBuilderOrder,
    stop_loss_order: &TradeBuilderOrder,
) -> Result<()> {
    if !trade_builder_is_stop_loss_child(stop_loss_order)
        || !trade_builder_order_uses_pair_lock(parent_order)
        || !trade_builder_pair_lock_is_candidate_order(parent_order)
    {
        return Ok(());
    }

    let Some(pair_session_id) = parent_order.pair_session_id else {
        return Ok(());
    };
    let Some(session) = repo.get_trade_builder_pair_session(pair_session_id).await? else {
        return Ok(());
    };
    if session.status != TRADE_BUILDER_PAIR_STATUS_LOCKED {
        return Ok(());
    }
    if session.ignore_stop_loss_after_locked {
        repo.append_trade_builder_order_event(
            parent_order.id,
            "pair_lock_locked_leg_stop_loss_ignored",
            &json!({
                "pair_session_id": session.id,
                "sl_child_order_id": stop_loss_order.id,
                "reason": "pair_locked_stop_loss_ignored",
            }),
        )
        .await?;
        return Ok(());
    }

    let orders = repo
        .list_trade_builder_orders_by_pair_session(pair_session_id)
        .await?;
    let canceled_unwind_order_ids =
        cancel_trade_builder_pair_lock_unwind_orders(repo, &orders, "locked_leg_stop_loss")
            .await?;
    let detached_candidate_order_ids = detach_trade_builder_pair_lock_surviving_candidates(
        repo,
        &orders,
        parent_order.id,
        "locked_leg_stop_loss",
    )
    .await?;

    repo.update_trade_builder_pair_session_state(
        session.id,
        TRADE_BUILDER_PAIR_STATUS_COMPLETED,
        session.locked_qty,
        session.projected_net_profit_usdc,
        Some("locked_leg_stop_loss"),
    )
    .await?;
    append_trade_builder_pair_lock_event(
        repo,
        &session,
        "pair_lock_locked_leg_stop_loss",
        json!({
            "pair_session_id": session.id,
            "parent_order_id": parent_order.id,
            "sl_child_order_id": stop_loss_order.id,
            "reason": "locked_leg_stop_loss",
            "canceled_unwind_order_ids": canceled_unwind_order_ids,
            "detached_candidate_order_ids": detached_candidate_order_ids,
        }),
    )
    .await?;
    append_trade_builder_pair_lock_event(
        repo,
        &session,
        "pair_lock_session_state_changed",
        json!({
            "pair_session_id": session.id,
            "status_after": TRADE_BUILDER_PAIR_STATUS_COMPLETED,
            "reason": "locked_leg_stop_loss",
            "canceled_unwind_order_ids": canceled_unwind_order_ids,
            "detached_candidate_order_ids": detached_candidate_order_ids,
        }),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod pair_lock_stop_loss_tests {
    use super::*;

    #[test]
    fn pair_lock_stop_loss_surface_only_stays_active_for_working_lead_leg() {
        let session = TradeBuilderPairSession {
            id: 7,
            user_id: 1,
            flow_definition_id: None,
            flow_run_id: None,
            flow_node_key: Some("pair_buy".to_string()),
            market_slug: "btc-updown-5m-1".to_string(),
            status: TRADE_BUILDER_PAIR_STATUS_WORKING.to_string(),
            pair_target_total_cent: 90.0,
            min_net_profit_usdc: 0.0,
            profit_safety_buffer_usdc: 0.0,
            orphan_grace_ms: 1500,
            ignore_stop_loss_after_locked: false,
            notify_on_pair_locked: false,
            notify_on_pair_unwind: false,
            notify_on_pair_no_edge: false,
            primary_order_id: Some(11),
            counter_order_id: Some(12),
            lead_order_id: Some(12),
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

        assert!(trade_builder_pair_lock_stop_loss_surface_active_from_session(
            &session, 12
        ));
        assert!(!trade_builder_pair_lock_stop_loss_surface_active_from_session(
            &session, 11
        ));

        let mut locked_session = session.clone();
        locked_session.status = TRADE_BUILDER_PAIR_STATUS_LOCKED.to_string();
        assert!(trade_builder_pair_lock_stop_loss_surface_active_from_session(
            &locked_session, 12
        ));
        assert!(trade_builder_pair_lock_stop_loss_surface_active_from_session(
            &locked_session, 11
        ));

        locked_session.ignore_stop_loss_after_locked = true;
        assert!(!trade_builder_pair_lock_stop_loss_surface_active_from_session(
            &locked_session, 12
        ));
        assert!(!trade_builder_pair_lock_stop_loss_surface_active_from_session(
            &locked_session, 11
        ));

        let mut completed_session = session;
        completed_session.status = TRADE_BUILDER_PAIR_STATUS_COMPLETED.to_string();
        assert!(!trade_builder_pair_lock_stop_loss_surface_active_from_session(
            &completed_session, 12
        ));
    }
}
