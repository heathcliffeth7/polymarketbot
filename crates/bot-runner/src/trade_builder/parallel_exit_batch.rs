static TRADE_BUILDER_PARENT_EXIT_BATCH_GUARDS: LazyLock<parking_lot::Mutex<HashSet<i64>>> =
    LazyLock::new(|| parking_lot::Mutex::new(HashSet::new()));

struct TradeBuilderParentExitBatchGuard {
    parent_order_id: i64,
}

impl Drop for TradeBuilderParentExitBatchGuard {
    fn drop(&mut self) {
        TRADE_BUILDER_PARENT_EXIT_BATCH_GUARDS
            .lock()
            .remove(&self.parent_order_id);
    }
}

fn try_acquire_trade_builder_parent_exit_batch_guard(
    parent_order_id: i64,
) -> Option<TradeBuilderParentExitBatchGuard> {
    let mut guards = TRADE_BUILDER_PARENT_EXIT_BATCH_GUARDS.lock();
    if !guards.insert(parent_order_id) {
        return None;
    }
    Some(TradeBuilderParentExitBatchGuard { parent_order_id })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct TradeBuilderParallelExitBatchSelection {
    eligible_order_ids: Vec<i64>,
    selected_order_ids: Vec<i64>,
    stop_loss_order_ids: Vec<i64>,
    take_profit_order_ids: Vec<i64>,
    deferred_order_ids: Vec<i64>,
}

#[derive(Debug, Clone)]
struct TradeBuilderParallelExitBatchMemberTelemetry {
    parent_order_id: i64,
    batch_owner_order_id: i64,
    batch_path: &'static str,
    selected_order_ids: Vec<i64>,
}

fn trade_builder_is_parallel_exit_batch_candidate(order: &TradeBuilderOrder) -> bool {
    trade_builder_is_price_exit_ladder_child(order)
        && trade_builder_is_child_exit_sell(order)
        && order.parent_order_id.is_some()
        && order.active_exchange_order_id.is_none()
        && order.status != "canceled_requested"
        && !trade_builder_is_terminal_status(&order.status)
        && order.trigger_condition.is_some()
        && order.trigger_price.is_some()
}

fn trade_builder_parallel_exit_batch_selection(
    orders: &[TradeBuilderOrder],
    runtime_price: &TradeBuilderRuntimePrice,
) -> TradeBuilderParallelExitBatchSelection {
    let mut stop_loss_order_ids = Vec::new();
    let mut take_profit_order_ids = Vec::new();

    let mut candidates = orders
        .iter()
        .filter(|order| trade_builder_is_parallel_exit_batch_candidate(order))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|order| order.id);

    for order in candidates {
        if let Some(mode) = order.sl_trigger_price_mode.as_deref() {
            if sl_trigger_eval_price_for_mode(mode, runtime_price).is_none() {
                continue;
            }
        }

        let current_price = trade_builder_trigger_eval_price_for_order(order, runtime_price);
        let evaluation =
            evaluate_trade_builder_order_trigger(order, order.last_seen_price, current_price);
        if !evaluation.should_trigger
            || should_skip_trade_builder_composite_sl_bid_confirmation(order, runtime_price)
        {
            continue;
        }

        if trade_builder_is_stop_loss_child(order) {
            stop_loss_order_ids.push(order.id);
        } else if trade_builder_is_take_profit_child(order) {
            take_profit_order_ids.push(order.id);
        }
    }

    let selected_order_ids = if !stop_loss_order_ids.is_empty() {
        stop_loss_order_ids.clone()
    } else {
        take_profit_order_ids.clone()
    };
    let deferred_order_ids = if !stop_loss_order_ids.is_empty() {
        take_profit_order_ids.clone()
    } else {
        Vec::new()
    };
    let eligible_order_ids = stop_loss_order_ids
        .iter()
        .chain(take_profit_order_ids.iter())
        .copied()
        .collect::<Vec<_>>();

    TradeBuilderParallelExitBatchSelection {
        eligible_order_ids,
        selected_order_ids,
        stop_loss_order_ids,
        take_profit_order_ids,
        deferred_order_ids,
    }
}

async fn maybe_dispatch_trade_builder_parallel_exit_batch(
    repo: &PostgresRepository,
    run_id: i64,
    ws: &ClobWsClient,
    order: &TradeBuilderOrder,
    runtime_price: &TradeBuilderRuntimePrice,
    batch_path: &'static str,
) -> Result<TradeBuilderParallelExitBatchSelection> {
    if !trade_builder_is_parallel_exit_batch_candidate(order) {
        return Ok(TradeBuilderParallelExitBatchSelection::default());
    }
    let Some(parent_order_id) = order.parent_order_id else {
        return Ok(TradeBuilderParallelExitBatchSelection::default());
    };

    let siblings = repo
        .list_trade_builder_child_orders_by_parent(parent_order_id, None)
        .await?;
    let selection = trade_builder_parallel_exit_batch_selection(&siblings, runtime_price);
    let additional_order_ids = selection
        .selected_order_ids
        .iter()
        .copied()
        .filter(|order_id| *order_id != order.id)
        .collect::<Vec<_>>();
    if additional_order_ids.is_empty() && selection.deferred_order_ids.is_empty() {
        return Ok(selection);
    }

    let Some(_guard) = try_acquire_trade_builder_parent_exit_batch_guard(parent_order_id) else {
        return Ok(selection);
    };

    repo.append_trade_builder_order_event(
        parent_order_id,
        "parallel_exit_batch_selected",
        &json!({
            "batch_path": batch_path,
            "batch_owner_order_id": order.id,
            "eligible_order_ids": selection.eligible_order_ids,
            "selected_order_ids": selection.selected_order_ids,
            "stop_loss_order_ids": selection.stop_loss_order_ids,
            "take_profit_order_ids": selection.take_profit_order_ids,
            "deferred_order_ids": selection.deferred_order_ids,
            "best_bid": runtime_price.best_bid,
            "best_ask": runtime_price.best_ask,
            "last_trade_price": runtime_price.last_trade_price,
            "price_source": &runtime_price.source,
        }),
    )
    .await?;

    if additional_order_ids.is_empty() {
        return Ok(selection);
    }

    repo.append_trade_builder_order_event(
        parent_order_id,
        "parallel_exit_batch_dispatched",
        &json!({
            "batch_path": batch_path,
            "batch_owner_order_id": order.id,
            "dispatched_order_ids": additional_order_ids,
            "selected_order_ids": selection.selected_order_ids,
        }),
    )
    .await?;

    let telemetry = TradeBuilderParallelExitBatchMemberTelemetry {
        parent_order_id,
        batch_owner_order_id: order.id,
        batch_path,
        selected_order_ids: selection.selected_order_ids.clone(),
    };
    for order_id in additional_order_ids {
        spawn_armed_order_immediate_processing(
            repo,
            run_id,
            ws,
            order_id,
            order.user_id,
            Some(telemetry.clone()),
        );
    }

    Ok(selection)
}
