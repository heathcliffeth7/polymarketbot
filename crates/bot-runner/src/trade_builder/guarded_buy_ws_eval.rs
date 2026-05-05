#[derive(Debug, Clone, Default)]
struct GuardedBuyOrderCache {
    by_token: HashMap<String, Vec<TradeBuilderOrder>>,
}

static GUARDED_BUY_ORDER_CACHE: LazyLock<RwLock<GuardedBuyOrderCache>> =
    LazyLock::new(|| RwLock::new(GuardedBuyOrderCache::default()));

fn trade_builder_is_ws_guard_recoverable_buy(order: &TradeBuilderOrder) -> bool {
    order.side == "buy"
        && order.kind == "immediate"
        && order.status == TRADE_BUILDER_GUARD_BLOCKED_STATUS
        && order.active_exchange_order_id.is_none()
        && (order.guard_trigger_price.is_some()
            || order.best_ask_floor_price.is_some()
            || order.max_price.is_some())
}

async fn refresh_guarded_buy_order_cache(orders: Vec<TradeBuilderOrder>) {
    let mut by_token: HashMap<String, Vec<TradeBuilderOrder>> = HashMap::new();
    for order in orders {
        if !trade_builder_is_ws_guard_recoverable_buy(&order) {
            continue;
        }
        by_token
            .entry(order.token_id.clone())
            .or_default()
            .push(order);
    }

    let mut cache = GUARDED_BUY_ORDER_CACHE.write().await;
    cache.by_token = by_token;
}

async fn sync_guarded_buy_order_to_cache(order: TradeBuilderOrder) {
    let token_id = order.token_id.clone();
    let mut cache = GUARDED_BUY_ORDER_CACHE.write().await;
    let bucket = cache.by_token.entry(token_id.clone()).or_default();
    bucket.retain(|existing| existing.id != order.id);
    if trade_builder_is_ws_guard_recoverable_buy(&order) {
        bucket.push(order);
        bucket.sort_by_key(|existing| existing.created_at);
    }
    if bucket.is_empty() {
        cache.by_token.remove(&token_id);
    }
}

async fn sync_guarded_buy_order_cache_for_order(repo: &PostgresRepository, order_id: i64) {
    let Ok(order) = repo.get_trade_builder_order(order_id).await else {
        return;
    };
    if let Some(order) = order {
        sync_guarded_buy_order_to_cache(order).await;
        return;
    }

    let mut cache = GUARDED_BUY_ORDER_CACHE.write().await;
    for orders in cache.by_token.values_mut() {
        orders.retain(|existing| existing.id != order_id);
    }
    cache.by_token.retain(|_, orders| !orders.is_empty());
}

async fn guarded_buy_order_cache_token_ids() -> Vec<String> {
    let cache = GUARDED_BUY_ORDER_CACHE.read().await;
    cache.by_token.keys().cloned().collect()
}

fn trade_builder_guard_blocked_buy_ready_from_snapshot(
    order: &TradeBuilderOrder,
    runtime_price: &TradeBuilderRuntimePrice,
) -> bool {
    if matches!(
        order.last_error.as_deref(),
        Some("pair_primary_best_ask_unavailable" | "pair_counter_best_ask_unavailable")
    ) && runtime_price.best_ask.is_none()
    {
        return false;
    }
    let current_price = trade_builder_execution_price_for_order(order, runtime_price);
    let desired_price = trade_builder_market_buy_execution_price(
        order,
        current_price,
        runtime_price.best_ask,
    )
    .map(|resolution| resolution.price)
    .unwrap_or_else(|| trade_builder_submit_desired_price(order, current_price));
    let (trigger_guard_reference_price, _) =
        trade_builder_resolve_trigger_guard_reference_price(order, current_price, runtime_price.best_ask);
    let (max_price_reference, _) =
        trade_builder_resolve_max_price_reference(order, runtime_price.best_ask, desired_price);

    !trade_builder_price_below_guard_trigger(order, trigger_guard_reference_price)
        && trade_builder_execution_floor_block_reason(order, runtime_price.best_ask).is_none()
        && !trade_builder_price_exceeds_max_price(order, max_price_reference)
}

async fn evaluate_guard_blocked_buy_orders_for_dirty_tokens(
    repo: &PostgresRepository,
    run_id: i64,
    ws: &ClobWsClient,
    dirty_token_ids: &[String],
) -> Result<()> {
    if dirty_token_ids.is_empty() {
        return Ok(());
    }

    let selected_orders: Vec<(String, Vec<TradeBuilderOrder>)> = {
        let cache = GUARDED_BUY_ORDER_CACHE.read().await;
        dirty_token_ids
            .iter()
            .filter_map(|token_id| {
                cache
                    .by_token
                    .get(token_id)
                    .cloned()
                    .map(|orders| (token_id.clone(), orders))
            })
            .collect()
    };
    if selected_orders.is_empty() {
        return Ok(());
    }

    let market_snapshots = ws.get_market_snapshots(dirty_token_ids).await;
    let mut orders_to_spawn = Vec::new();
    {
        let mut cache = GUARDED_BUY_ORDER_CACHE.write().await;
        let mut empty_tokens = Vec::new();

        for (token_id, orders) in selected_orders {
            let Some(bucket) = cache.by_token.get_mut(&token_id) else {
                continue;
            };
            let runtime_price = market_snapshots
                .get(&token_id)
                .and_then(build_trade_builder_runtime_price_from_market_snapshot);
            let Some(runtime_price) = runtime_price else {
                continue;
            };

            let ready_ids: HashSet<i64> = orders
                .iter()
                .filter(|order| {
                    trade_builder_guard_blocked_buy_ready_from_snapshot(order, &runtime_price)
                })
                .map(|order| order.id)
                .collect();

            if ready_ids.is_empty() {
                continue;
            }

            let mut idx = 0usize;
            while idx < bucket.len() {
                if ready_ids.contains(&bucket[idx].id) {
                    let order = bucket.remove(idx);
                    orders_to_spawn.push((order.id, order.user_id));
                    continue;
                }
                idx += 1;
            }

            if bucket.is_empty() {
                empty_tokens.push(token_id);
            }
        }

        for token_id in empty_tokens {
            cache.by_token.remove(&token_id);
        }
    }

    for (order_id, user_id) in orders_to_spawn {
        info!(
            run_id,
            builder_order_id = order_id,
            user_id,
            "GUARD_BLOCKED_BUY_WS_READY"
        );
        spawn_armed_order_immediate_processing(repo, run_id, ws, order_id, user_id, None);
    }

    Ok(())
}
