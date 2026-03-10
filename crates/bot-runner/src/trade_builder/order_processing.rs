async fn process_trade_builder_order(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    gamma: &GammaHttpClient,
    ws: &ClobWsClient,
    order: &TradeBuilderOrder,
) -> Result<()> {
    let Some(mut order) = repo.get_trade_builder_order(order.id).await? else {
        return Ok(());
    };
    let retryable_error =
        order.status == "error" && trade_builder_should_retry_after_processing_error(&order);
    if !is_trade_builder_order_processable_status(&order.status) && !retryable_error {
        return Ok(());
    }

    if order.triggers_fired >= order.max_triggers && order.status != "completed" {
        repo.set_trade_builder_order_status(order.id, "completed", None)
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "max_trigger_reached",
            &json!({
                "triggers_fired": order.triggers_fired,
                "max_triggers": order.max_triggers
            }),
        )
        .await?;
        if let Ok(Some(unblocked_id)) = repo
            .unblock_next_trade_builder_order(order.trade_id, &order.token_id)
            .await
        {
            info!(
                builder_order_id = order.id,
                unblocked_order_id = unblocked_id,
                "TRADE_BUILDER_DCA_NEXT_LEVEL_UNBLOCKED"
            );
        }
        return Ok(());
    }

    if let Some(expires_at) = order.expires_at {
        if expires_at <= Utc::now() {
            if order.status != "expired" && order.status != "completed" {
                repo.clear_trade_builder_active_exchange_order(order.id, "expired")
                    .await?;
                repo.append_trade_builder_order_event(
                    order.id,
                    "expired",
                    &json!({ "expires_at": expires_at }),
                )
                .await?;
            }
            return Ok(());
        }
    }

    if maybe_expire_trade_builder_stale_order(repo, gamma, &order).await? {
        return Ok(());
    }

    let previous_price = order.last_seen_price;
    let runtime_price = match if trade_builder_uses_fast_runtime_pricing(&order) {
        resolve_trade_builder_fast_runtime_price(ws, client, &order).await?
    } else {
        resolve_trade_builder_runtime_price(ws, client, &order).await?
    } {
        TradeBuilderRuntimePriceFetch::Resolved(runtime_price) => runtime_price,
        TradeBuilderRuntimePriceFetch::Retry { error_text } => {
            repo.append_trade_builder_order_event(
                order.id,
                "price_unavailable_retry",
                &json!({
                    "reason_code": "runtime_price_unavailable",
                    "reason_message": "Runtime price was unavailable and no fallback price was present.",
                    "status": &order.status,
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "previous_price": previous_price,
                    "working_price": order.working_price,
                    "error": error_text,
                }),
            )
            .await?;
            return Ok(());
        }
    };
    let trigger_eval_price = trade_builder_trigger_eval_price_for_order(&order, &runtime_price);
    let execution_price = trade_builder_execution_price_for_order(&order, &runtime_price);
    let persisted_last_seen_price =
        trade_builder_last_seen_price_for_order(&order, trigger_eval_price, execution_price);
    if let Some(runtime_warning) = runtime_price.runtime_warning.as_deref() {
        repo.append_trade_builder_order_event(
            order.id,
            "runtime_price_fallback_used",
            &json!({
                "source": runtime_price.source,
                "current_price": execution_price,
                "trigger_eval_price": trigger_eval_price,
                "previous_price": previous_price,
                "working_price": order.working_price,
                "runtime_warning": runtime_warning,
                "best_bid": runtime_price.best_bid,
                "best_ask": runtime_price.best_ask,
                "last_trade_price": runtime_price.last_trade_price,
            }),
        )
        .await?;
    }
    let tp_preempted = maybe_preempt_trade_builder_take_profit_for_stop_loss(
        repo,
        &mut order,
        &runtime_price,
    )
    .await?;
    if tp_preempted && order.active_exchange_order_id.is_none() {
        repo.set_trade_builder_last_seen_price(order.id, persisted_last_seen_price)
            .await?;
        order.last_seen_price = Some(persisted_last_seen_price);
        return Ok(());
    }

    if let Some(exchange_order_id) = order.active_exchange_order_id.as_deref() {
        reconcile_trade_builder_open_order(
            repo,
            run_id,
            client,
            &order,
            exchange_order_id,
            execution_price,
        )
        .await?;
        repo.set_trade_builder_last_seen_price(order.id, persisted_last_seen_price)
            .await?;
        order.last_seen_price = Some(persisted_last_seen_price);
        return Ok(());
    }

    if order.status == "canceled_requested" {
        let cancel_reason = order.last_error.as_deref().unwrap_or("user_request");
        repo.set_trade_builder_order_status(order.id, "canceled", None)
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "canceled_without_open_order",
            &json!({ "reason": cancel_reason }),
        )
        .await?;
        return Ok(());
    }

    let trigger_evaluation =
        evaluate_trade_builder_order_trigger(&order, previous_price, trigger_eval_price);
    if !trigger_evaluation.should_trigger {
        if order.kind == "conditional" {
            info!(
                run_id,
                builder_order_id = order.id,
                market = %order.market_slug,
                token_id = %order.token_id,
                trigger_condition = ?order.trigger_condition,
                trigger_price = ?order.trigger_price,
                previous_price = ?previous_price,
                current_price = trigger_eval_price,
                execution_price,
                order_status = %order.status,
                reason_code = "trigger_not_crossed",
                "TRADE_BUILDER_TRIGGER_NOT_MET"
            );
        }
        if order.kind == "conditional" && order.status == "inventory_pending" {
            repo.set_trade_builder_order_status(order.id, "armed", None)
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "inventory_pending_released",
                &json!({
                    "reason_code": "trigger_recheck_failed",
                    "reason_message": "Exit trigger no longer valid while inventory was pending.",
                    "side": &order.side,
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "previous_price": previous_price,
                    "current_price": trigger_eval_price,
                    "execution_price": execution_price,
                    "status_before": &order.status,
                    "status_after": "armed"
                }),
            )
            .await?;
            return Ok(());
        }
        if order.kind == "conditional" && order.status == "pending" {
            repo.set_trade_builder_order_status(order.id, "armed", None)
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "trigger_not_met",
                &json!({
                    "reason_code": "trigger_not_crossed",
                    "reason_message": "Trigger condition has not crossed yet.",
                    "side": &order.side,
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "previous_price": previous_price,
                    "current_price": trigger_eval_price,
                    "execution_price": execution_price,
                    "status_before": &order.status,
                    "status_after": "armed"
                }),
            )
            .await?;
        }
        repo.set_trade_builder_last_seen_price(order.id, persisted_last_seen_price)
            .await?;
        order.last_seen_price = Some(persisted_last_seen_price);
        return Ok(());
    }

    if trigger_evaluation.first_tick_threshold_used {
        repo.append_trade_builder_order_event(
            order.id,
            "trigger_first_tick_threshold_used",
            &json!({
                "status": &order.status,
                "side": &order.side,
                "market_slug": &order.market_slug,
                "token_id": &order.token_id,
                "trigger_condition": order.trigger_condition.as_deref(),
                "trigger_price": order.trigger_price,
                "previous_price": previous_price,
                "current_price": trigger_eval_price,
                "execution_price": execution_price
            }),
        )
        .await?;
    }
    repo.set_trade_builder_last_seen_price(order.id, persisted_last_seen_price)
        .await?;
    order.last_seen_price = Some(persisted_last_seen_price);
    info!(
        run_id,
        builder_order_id = order.id,
        market = %order.market_slug,
        token_id = %order.token_id,
        trigger_condition = ?order.trigger_condition,
        trigger_price = ?order.trigger_price,
        previous_price = ?previous_price,
        current_price = trigger_eval_price,
        execution_price,
        order_status = %order.status,
        "TRADE_BUILDER_TRIGGER_CONDITION_MET"
    );
    maybe_latch_trade_builder_stop_loss(repo, &mut order, trigger_eval_price).await?;
    let fee_rate_bps = resolve_trade_builder_order_fee_rate_bps(repo, client, &mut order).await?;

    let size_basis = normalize_trade_builder_size_basis(&order.size_basis);
    let (
        resolved_size_usdc,
        trigger_size_mode,
        trigger_size_value,
        exhausted_trigger_sizes,
        trigger_size_index,
    ) = if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        (
            order.size_usdc,
            None,
            None,
            false,
            order.triggers_fired.max(0) as usize,
        )
    } else {
        resolve_trade_builder_next_trigger_size_usdc(repo, &order).await?
    };
    if exhausted_trigger_sizes {
        repo.set_trade_builder_order_status(order.id, "completed", None)
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "trigger_size_exhausted",
            &json!({
                "triggers_fired": order.triggers_fired,
                "max_triggers": order.max_triggers,
                "next_trigger_index": trigger_size_index + 1
            }),
        )
        .await?;
        return Ok(());
    }

    submit_trade_builder_trigger_order(
        repo,
        run_id,
        cfg,
        limits,
        policy,
        client,
        &mut order,
        execution_price,
        fee_rate_bps,
        resolved_size_usdc,
        trigger_size_mode,
        trigger_size_value,
        trigger_size_index,
    )
    .await
}
