#[derive(Debug, Clone, Default)]
struct ArmedBuilderOrderCache {
    by_token: HashMap<String, Vec<TradeBuilderOrder>>,
}

static ARMED_BUILDER_ORDER_CACHE: LazyLock<RwLock<ArmedBuilderOrderCache>> =
    LazyLock::new(|| RwLock::new(ArmedBuilderOrderCache::default()));

fn trade_builder_is_ws_fast_path_tp_sl_child(order: &TradeBuilderOrder) -> bool {
    trade_builder_is_child_exit_sell(order)
        && matches!(order.status.as_str(), "armed" | "triggered")
        && order.trigger_condition.is_some()
        && order.trigger_price.is_some()
}

fn build_trade_builder_runtime_price_from_market_snapshot(
    snapshot: &MarketDataSnapshot,
) -> Option<TradeBuilderRuntimePrice> {
    let source = trade_builder_fast_runtime_price_source(
        "ws",
        snapshot.best_bid,
        snapshot.best_ask,
        snapshot.last_trade_price,
    )?;
    build_trade_builder_fast_runtime_price(
        source,
        None,
        snapshot.best_bid,
        snapshot.best_ask,
        snapshot.last_trade_price,
    )
}

fn trade_builder_last_seen_price_from_market_snapshot(
    order: &TradeBuilderOrder,
    snapshot: &MarketDataSnapshot,
) -> Option<f64> {
    let runtime_price = build_trade_builder_runtime_price_from_market_snapshot(snapshot)?;
    let trigger_eval_price = trade_builder_trigger_eval_price_for_order(order, &runtime_price);
    let execution_price = trade_builder_execution_price_for_order(order, &runtime_price);
    Some(trade_builder_last_seen_price_for_order(
        order,
        trigger_eval_price,
        execution_price,
    ))
}

async fn refresh_armed_builder_order_cache(orders: Vec<TradeBuilderOrder>) {
    let mut by_token: HashMap<String, Vec<TradeBuilderOrder>> = HashMap::new();
    for order in orders {
        if !trade_builder_is_ws_fast_path_tp_sl_child(&order) {
            continue;
        }
        by_token.entry(order.token_id.clone()).or_default().push(order);
    }

    let mut cache = ARMED_BUILDER_ORDER_CACHE.write().await;
    cache.by_token = by_token;
}

async fn insert_into_armed_builder_order_cache(order: TradeBuilderOrder) {
    if !trade_builder_is_ws_fast_path_tp_sl_child(&order) {
        return;
    }

    let mut cache = ARMED_BUILDER_ORDER_CACHE.write().await;
    let bucket = cache.by_token.entry(order.token_id.clone()).or_default();
    if let Some(existing) = bucket.iter_mut().find(|existing| existing.id == order.id) {
        *existing = order;
    } else {
        bucket.push(order);
        bucket.sort_by_key(|existing| existing.created_at);
    }
}

async fn armed_builder_order_cache_token_ids() -> Vec<String> {
    let cache = ARMED_BUILDER_ORDER_CACHE.read().await;
    cache.by_token.keys().cloned().collect()
}

async fn ensure_fast_path_market_stream_union(ws: &ClobWsClient) -> Result<()> {
    let flow_token_ids = {
        let cache = TRADE_FLOW_WS_FAST_PATH_CACHE.read().await;
        cache.token_targets.keys().cloned().collect::<Vec<_>>()
    };
    let builder_token_ids = armed_builder_order_cache_token_ids().await;
    let all_token_ids: Vec<String> = flow_token_ids
        .into_iter()
        .chain(builder_token_ids)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    ws.ensure_market_stream(&all_token_ids).await
}

async fn evaluate_armed_builder_orders_for_dirty_tokens(
    repo: &PostgresRepository,
    run_id: i64,
    ws: &ClobWsClient,
    dirty_token_ids: &[String],
) -> Result<()> {
    if dirty_token_ids.is_empty() {
        return Ok(());
    }

    let selected_orders: Vec<(String, Vec<TradeBuilderOrder>)> = {
        let cache = ARMED_BUILDER_ORDER_CACHE.read().await;
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
    let mut last_seen_updates: HashMap<i64, f64> = HashMap::new();
    let mut triggered_order_ids: HashSet<i64> = HashSet::new();
    let mut selected_token_ids = Vec::with_capacity(selected_orders.len());

    for (token_id, orders) in selected_orders {
        selected_token_ids.push(token_id.clone());
        let Some(snapshot) = market_snapshots.get(&token_id) else {
            continue;
        };
        let Some(runtime_price) = build_trade_builder_runtime_price_from_market_snapshot(snapshot)
        else {
            continue;
        };

        for order in orders {
            if order
                .sl_trigger_price_mode
                .as_deref()
                .is_some_and(|mode| sl_trigger_eval_price_for_mode(mode, &runtime_price).is_none())
            {
                continue;
            }

            let trigger_eval_price = trade_builder_trigger_eval_price_for_order(&order, &runtime_price);
            let execution_price = trade_builder_execution_price_for_order(&order, &runtime_price);
            let persisted_last_seen_price = trade_builder_last_seen_price_for_order(
                &order,
                trigger_eval_price,
                execution_price,
            );
            let evaluation = evaluate_trade_builder_order_trigger(
                &order,
                order.last_seen_price,
                trigger_eval_price,
            );
            if evaluation.should_trigger {
                triggered_order_ids.insert(order.id);
            } else {
                last_seen_updates.insert(order.id, persisted_last_seen_price);
            }
        }
    }

    if triggered_order_ids.is_empty() && last_seen_updates.is_empty() {
        return Ok(());
    }

    let mut orders_to_spawn = Vec::new();
    {
        let mut cache = ARMED_BUILDER_ORDER_CACHE.write().await;
        let mut empty_tokens = Vec::new();

        for token_id in selected_token_ids {
            let Some(bucket) = cache.by_token.get_mut(&token_id) else {
                continue;
            };
            let mut idx = 0usize;
            while idx < bucket.len() {
                let order_id = bucket[idx].id;
                if triggered_order_ids.contains(&order_id) {
                    let order = bucket.remove(idx);
                    orders_to_spawn.push((order.id, order.user_id));
                    continue;
                }
                if let Some(last_seen_price) = last_seen_updates.get(&order_id).copied() {
                    bucket[idx].last_seen_price = Some(last_seen_price);
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
        spawn_armed_order_immediate_processing(repo, run_id, ws, order_id, user_id);
    }

    Ok(())
}

fn spawn_armed_order_immediate_processing(
    repo: &PostgresRepository,
    run_id: i64,
    ws: &ClobWsClient,
    order_id: i64,
    user_id: i64,
) {
    let repo = repo.clone();
    let ws = ws.clone();
    tokio::spawn(async move {
        let mut user_cfg_cache = HashMap::new();
        let mut user_executor_cache = HashMap::new();
        let user_cfg =
            match load_user_app_config_cached(&repo, user_id, &mut user_cfg_cache).await {
                Ok(cfg) => cfg,
                Err(err) => {
                    warn!(
                        run_id,
                        builder_order_id = order_id,
                        user_id,
                        error = %err,
                        "ARMED_ORDER_WS_USER_CONFIG_LOAD_FAILED"
                    );
                    return;
                }
            };
        let client = match load_user_order_executor_cached(
            &repo,
            user_id,
            &mut user_cfg_cache,
            &mut user_executor_cache,
        )
        .await
        {
            Ok(client) => client,
            Err(err) => {
                warn!(
                    run_id,
                    builder_order_id = order_id,
                    user_id,
                    error = %err,
                    "ARMED_ORDER_WS_EXECUTOR_LOAD_FAILED"
                );
                return;
            }
        };

        if let Err(err) = try_immediate_submit_single_builder_order(
            &repo,
            run_id,
            &user_cfg,
            &ws,
            client,
            order_id,
            "ws_armed",
        )
        .await
        {
            warn!(
                run_id,
                builder_order_id = order_id,
                user_id,
                error = %err,
                "ARMED_ORDER_WS_IMMEDIATE_SUBMIT_FAILED"
            );
        }
    });
}

#[cfg(test)]
mod armed_ws_eval_tests {
    use super::*;

    fn test_tp_sl_child_order() -> TradeBuilderOrder {
        TradeBuilderOrder {
            id: 1,
            trade_id: 77,
            user_id: 1,
            kind: "conditional".to_string(),
            status: "armed".to_string(),
            market_slug: "btc-updown-5m-1".to_string(),
            token_id: "tok-up".to_string(),
            outcome_label: "Up".to_string(),
            side: "sell".to_string(),
            execution_mode: "market".to_string(),
            trigger_condition: Some("cross_above".to_string()),
            trigger_price: Some(0.8),
            max_price: None,
            size_basis: TRADE_BUILDER_SIZE_BASIS_SHARES.to_string(),
            size_usdc: 5.0,
            target_qty: Some(5.1),
            min_price_distance_cent: 1.0,
            expires_at: None,
            eligible_after_at: None,
            eligible_before_at: None,
            max_triggers: 1,
            triggers_fired: 0,
            active_exchange_order_id: None,
            remaining_size: None,
            remaining_qty: Some(5.1),
            working_price: None,
            last_seen_price: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            parent_order_id: Some(42),
            origin_flow_run_id: None,
            tp_enabled: false,
            tp_price: None,
            sl_enabled: false,
            sl_price: None,
            filled_qty: 0.0,
            fee_rate_bps: 0,
            trigger_latched: false,
            trigger_latched_reason: None,
            submitted_dynamic_qty: None,
            submitted_dynamic_price: None,
            guard_trigger_price: None,
            best_ask_floor_price: None,
            retry_on_trigger_guard_block: false,
            retry_on_execution_floor_guard_block: false,
            retry_on_max_price_block: false,
            sl_trigger_price_mode: None,
            notify_on_fill: false,
            notify_on_trigger_guard_blocked: false,
            notify_on_execution_floor_blocked: false,
            notify_on_tp_hit: false,
            notify_on_sl_hit: false,
            notify_on_max_price_blocked: false,
        }
    }

    fn reset_armed_builder_order_cache() {
        if let Ok(mut cache) = ARMED_BUILDER_ORDER_CACHE.try_write() {
            cache.by_token.clear();
        }
    }

    #[tokio::test]
    async fn insert_cache_upserts_existing_order() {
        reset_armed_builder_order_cache();
        let mut order = test_tp_sl_child_order();
        insert_into_armed_builder_order_cache(order.clone()).await;

        order.last_seen_price = Some(0.77);
        insert_into_armed_builder_order_cache(order.clone()).await;

        let cache = ARMED_BUILDER_ORDER_CACHE.read().await;
        let bucket = cache.by_token.get(&order.token_id).expect("bucket");
        assert_eq!(bucket.len(), 1);
        assert_eq!(bucket[0].last_seen_price, Some(0.77));
    }

    #[tokio::test]
    async fn snapshot_last_seen_helper_uses_exit_sell_runtime_semantics() {
        let order = test_tp_sl_child_order();
        let snapshot = MarketDataSnapshot {
            best_bid: Some(0.76),
            best_ask: Some(0.79),
            last_trade_price: Some(0.77),
            updated_at_ms: 1,
            last_source: "book".to_string(),
        };

        assert_eq!(
            trade_builder_last_seen_price_from_market_snapshot(&order, &snapshot),
            Some(0.76)
        );
    }
}
