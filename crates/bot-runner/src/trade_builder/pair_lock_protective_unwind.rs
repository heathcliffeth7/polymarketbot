async fn trade_builder_pair_lock_protective_unwind_enabled_for_session(
    repo: &PostgresRepository,
    session: &TradeBuilderPairSession,
) -> Result<bool> {
    Ok(resolve_trade_builder_pair_lock_session_config(repo, session)
        .await?
        .map(|pair_lock| pair_lock.protective_unwind_enabled)
        .unwrap_or(true))
}

async fn skip_trade_builder_pair_protective_unwind(
    repo: &PostgresRepository,
    session: &TradeBuilderPairSession,
    reason: &str,
    status_after: &str,
) -> Result<()> {
    repo.update_trade_builder_pair_session_state(session.id, status_after, None, None, Some(reason))
        .await?;
    append_trade_builder_pair_lock_event(
        repo,
        session,
        "pair_lock_protective_unwind_skipped",
        json!({
            "pair_session_id": session.id,
            "reason": reason,
            "status_after": status_after,
            "pair_protective_unwind_enabled": false,
        }),
    )
    .await
}

async fn maybe_skip_trade_builder_pair_protective_unwind(
    repo: &PostgresRepository,
    session: &TradeBuilderPairSession,
    reason: &str,
    status_after: &str,
) -> Result<bool> {
    if trade_builder_pair_lock_protective_unwind_enabled_for_session(repo, session).await? {
        return Ok(false);
    }
    skip_trade_builder_pair_protective_unwind(repo, session, reason, status_after).await?;
    Ok(true)
}

async fn trade_builder_pair_lock_counter_forces_guard_waiting(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
) -> Result<bool> {
    if order.pair_leg_role.as_deref() != Some(TRADE_BUILDER_PAIR_ROLE_COUNTER_CANDIDATE) {
        return Ok(false);
    }
    let Some(pair_session_id) = order.pair_session_id else {
        return Ok(false);
    };
    let Some(session) = repo.get_trade_builder_pair_session(pair_session_id).await? else {
        return Ok(false);
    };
    if session.status != TRADE_BUILDER_PAIR_STATUS_WORKING || session.lead_order_id.is_none() {
        return Ok(false);
    }
    Ok(!trade_builder_pair_lock_protective_unwind_enabled_for_session(repo, &session).await?)
}

async fn maybe_abort_trade_builder_pair_session_for_terminal_order(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    reason: &str,
) -> Result<()> {
    let Some(pair_session_id) = order.pair_session_id else {
        return Ok(());
    };
    if !trade_builder_pair_lock_is_candidate_order(order) {
        return Ok(());
    }
    let Some(session) = repo.get_trade_builder_pair_session(pair_session_id).await? else {
        return Ok(());
    };
    if session.status != TRADE_BUILDER_PAIR_STATUS_WORKING || session.lead_order_id.is_none() {
        return Ok(());
    }
    if maybe_skip_trade_builder_pair_protective_unwind(
        repo,
        &session,
        reason,
        TRADE_BUILDER_PAIR_STATUS_EXPIRED,
    )
    .await?
    {
        return Ok(());
    }
    let orders = repo
        .list_trade_builder_orders_by_pair_session(pair_session_id)
        .await?;
    schedule_trade_builder_pair_session_unwind(
        repo,
        &session,
        &orders,
        TRADE_BUILDER_PAIR_STATUS_UNWINDING,
        reason,
        None,
    )
    .await
}
