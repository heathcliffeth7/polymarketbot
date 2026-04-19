#[derive(Debug, Clone, Default)]
struct ArmedBuilderOrderCache {
    price_by_token: HashMap<String, Vec<TradeBuilderOrder>>,
    ptb_by_asset: HashMap<String, Vec<TradeBuilderOrder>>,
    ptb_by_market_slug: HashMap<String, Vec<TradeBuilderOrder>>,
}

static ARMED_BUILDER_ORDER_CACHE: LazyLock<RwLock<ArmedBuilderOrderCache>> =
    LazyLock::new(|| RwLock::new(ArmedBuilderOrderCache::default()));
#[cfg(test)]
static ARMED_BUILDER_ORDER_CACHE_TEST_MUTEX: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));
static LAST_ARMED_BUILDER_WS_PASSIVE_LOG_AT_MS: LazyLock<std::sync::atomic::AtomicI64> =
    LazyLock::new(|| std::sync::atomic::AtomicI64::new(0));

const ARMED_BUILDER_WS_PASSIVE_LOG_INTERVAL_MS: i64 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ArmedBuilderWsEvalActivity {
    selected_token_count: usize,
    selected_order_count: usize,
    evaluated_order_count: usize,
    last_seen_update_count: usize,
    triggered_order_count: usize,
    composite_waiting_count: usize,
    composite_released_count: usize,
    selected_source_missing_count: usize,
}

fn armed_builder_ws_eval_should_log(
    activity: ArmedBuilderWsEvalActivity,
    passive_sample_due: bool,
) -> bool {
    activity.triggered_order_count > 0
        || activity.composite_waiting_count > 0
        || activity.composite_released_count > 0
        || activity.selected_source_missing_count > 0
        || (passive_sample_due && activity.evaluated_order_count > 0)
}

fn armed_builder_ws_eval_passive_sample_due(now_ms: i64) -> bool {
    let last_logged_at =
        LAST_ARMED_BUILDER_WS_PASSIVE_LOG_AT_MS.load(std::sync::atomic::Ordering::Relaxed);
    if now_ms.saturating_sub(last_logged_at) < ARMED_BUILDER_WS_PASSIVE_LOG_INTERVAL_MS {
        return false;
    }
    true
}

fn mark_armed_builder_ws_eval_logged_at(now_ms: i64) {
    LAST_ARMED_BUILDER_WS_PASSIVE_LOG_AT_MS.store(now_ms, std::sync::atomic::Ordering::Relaxed);
}

fn trade_builder_is_ws_fast_path_price_child(order: &TradeBuilderOrder) -> bool {
    trade_builder_is_child_exit_sell(order)
        && matches!(order.status.as_str(), "armed" | "triggered")
        && order.trigger_condition.is_some()
        && order.trigger_price.is_some()
}

#[cfg(test)]
fn trade_builder_is_ws_fast_path_tp_sl_child(order: &TradeBuilderOrder) -> bool {
    trade_builder_is_ws_fast_path_price_child(order)
}

fn trade_builder_is_ws_fast_path_ptb_child(order: &TradeBuilderOrder) -> bool {
    trade_builder_is_child_exit_sell(order)
        && matches!(order.status.as_str(), "armed" | "triggered")
        && order.ptb_stop_loss_gap_usd.is_some()
}

fn trade_builder_ptb_asset_cache_key(order: &TradeBuilderOrder) -> Option<String> {
    find_updown_scope_by_slug(&order.market_slug).map(|scope| scope.asset.to_string())
}

fn trade_builder_ptb_market_slug_cache_key(order: &TradeBuilderOrder) -> String {
    order.market_slug.trim().to_ascii_lowercase()
}

fn remove_order_from_armed_builder_bucket(
    buckets: &mut HashMap<String, Vec<TradeBuilderOrder>>,
    order_id: i64,
) {
    buckets.retain(|_, bucket| {
        bucket.retain(|existing| existing.id != order_id);
        !bucket.is_empty()
    });
}

fn push_order_into_armed_builder_bucket(
    buckets: &mut HashMap<String, Vec<TradeBuilderOrder>>,
    key: String,
    order: &TradeBuilderOrder,
) {
    let bucket = buckets.entry(key).or_default();
    bucket.push(order.clone());
    bucket.sort_by_key(|existing| existing.created_at);
}

fn remove_order_from_armed_builder_cache(cache: &mut ArmedBuilderOrderCache, order_id: i64) {
    remove_order_from_armed_builder_bucket(&mut cache.price_by_token, order_id);
    remove_order_from_armed_builder_bucket(&mut cache.ptb_by_asset, order_id);
    remove_order_from_armed_builder_bucket(&mut cache.ptb_by_market_slug, order_id);
}

fn sync_armed_builder_order_cache_entry(
    cache: &mut ArmedBuilderOrderCache,
    order: TradeBuilderOrder,
) {
    remove_order_from_armed_builder_cache(cache, order.id);

    if trade_builder_is_ws_fast_path_price_child(&order) {
        push_order_into_armed_builder_bucket(
            &mut cache.price_by_token,
            order.token_id.clone(),
            &order,
        );
    }

    if trade_builder_is_ws_fast_path_ptb_child(&order) {
        if let Some(asset) = trade_builder_ptb_asset_cache_key(&order) {
            push_order_into_armed_builder_bucket(&mut cache.ptb_by_asset, asset, &order);
        }
        push_order_into_armed_builder_bucket(
            &mut cache.ptb_by_market_slug,
            trade_builder_ptb_market_slug_cache_key(&order),
            &order,
        );
    }
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
    let mut cache = ArmedBuilderOrderCache::default();
    for order in orders {
        sync_armed_builder_order_cache_entry(&mut cache, order);
    }

    let mut shared_cache = ARMED_BUILDER_ORDER_CACHE.write().await;
    *shared_cache = cache;
}

async fn sync_armed_builder_order_to_cache(order: TradeBuilderOrder) {
    let mut cache = ARMED_BUILDER_ORDER_CACHE.write().await;
    sync_armed_builder_order_cache_entry(&mut cache, order);
}

#[cfg(test)]
async fn rearm_builder_order_to_cache(order: TradeBuilderOrder) {
    if order.status == "armed" {
        sync_armed_builder_order_to_cache(order).await;
    }
}

async fn sync_armed_builder_order_cache_for_order(repo: &PostgresRepository, order_id: i64) {
    let order = repo.get_trade_builder_order(order_id).await.ok().flatten();
    let mut cache = ARMED_BUILDER_ORDER_CACHE.write().await;
    remove_order_from_armed_builder_cache(&mut cache, order_id);
    if let Some(order) = order {
        sync_armed_builder_order_cache_entry(&mut cache, order);
    }
}

async fn armed_builder_order_cache_token_ids() -> Vec<String> {
    let cache = ARMED_BUILDER_ORDER_CACHE.read().await;
    cache.price_by_token.keys().cloned().collect()
}

async fn ensure_fast_path_market_stream_union(ws: &ClobWsClient) -> Result<()> {
    let flow_token_ids = {
        let cache = TRADE_FLOW_WS_FAST_PATH_CACHE.read().await;
        cache.token_targets.keys().cloned().collect::<Vec<_>>()
    };
    let builder_token_ids = armed_builder_order_cache_token_ids().await;
    let guarded_buy_token_ids = guarded_buy_order_cache_token_ids().await;
    let all_token_ids: Vec<String> = flow_token_ids
        .into_iter()
        .chain(builder_token_ids)
        .chain(guarded_buy_token_ids)
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
                    .price_by_token
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
    let mut composite_bid_entering: Vec<(i64, Option<f64>, Option<f64>, Option<f64>)> = Vec::new();
    let mut composite_bid_releasing: Vec<(i64, f64, Option<f64>)> = Vec::new();
    let selected_token_count = selected_orders.len();
    let mut selected_order_count = 0usize;
    let mut evaluated_order_count = 0usize;
    let mut selected_source_missing_count = 0usize;

    for (token_id, orders) in selected_orders {
        selected_token_ids.push(token_id.clone());
        selected_order_count += orders.len();
        let Some(snapshot) = market_snapshots.get(&token_id) else {
            continue;
        };
        let Some(runtime_price) = build_trade_builder_runtime_price_from_market_snapshot(snapshot)
        else {
            continue;
        };

        for order in orders {
            if let Some(mode) = order.sl_trigger_price_mode.as_deref() {
                if sl_trigger_eval_price_for_mode(mode, &runtime_price).is_none() {
                    selected_source_missing_count += 1;
                    continue;
                }
            }

            evaluated_order_count += 1;
            let trigger_eval_price =
                trade_builder_trigger_eval_price_for_order(&order, &runtime_price);
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
                if should_skip_trade_builder_composite_sl_bid_confirmation(&order, &runtime_price) {
                    last_seen_updates.insert(order.id, persisted_last_seen_price);
                    if order.last_error.as_deref() != Some("composite_bid_confirmation_waiting") {
                        composite_bid_entering.push((
                            order.id,
                            order.trigger_price,
                            runtime_price.best_bid,
                            runtime_price.last_trade_price,
                        ));
                    }
                } else {
                    triggered_order_ids.insert(order.id);
                }
            } else {
                if order.last_error.as_deref() == Some("composite_bid_confirmation_waiting") {
                    composite_bid_releasing.push((
                        order.id,
                        trigger_eval_price,
                        runtime_price.best_bid,
                    ));
                }
                last_seen_updates.insert(order.id, persisted_last_seen_price);
            }
        }
    }

    let has_composite_changes =
        !composite_bid_entering.is_empty() || !composite_bid_releasing.is_empty();
    let activity = ArmedBuilderWsEvalActivity {
        selected_token_count,
        selected_order_count,
        evaluated_order_count,
        last_seen_update_count: last_seen_updates.len(),
        triggered_order_count: triggered_order_ids.len(),
        composite_waiting_count: composite_bid_entering.len(),
        composite_released_count: composite_bid_releasing.len(),
        selected_source_missing_count,
    };
    let now_ms = Utc::now().timestamp_millis();
    let passive_sample_due = armed_builder_ws_eval_passive_sample_due(now_ms);
    if armed_builder_ws_eval_should_log(activity, passive_sample_due) {
        mark_armed_builder_ws_eval_logged_at(now_ms);
        info!(
            run_id,
            dirty_token_count = dirty_token_ids.len(),
            selected_token_count = activity.selected_token_count,
            selected_order_count = activity.selected_order_count,
            evaluated_order_count = activity.evaluated_order_count,
            last_seen_update_count = activity.last_seen_update_count,
            triggered_order_count = activity.triggered_order_count,
            composite_waiting_count = activity.composite_waiting_count,
            composite_released_count = activity.composite_released_count,
            selected_source_missing_count = activity.selected_source_missing_count,
            passive_sample_due,
            "ARMED_BUILDER_WS_EVAL_CYCLE"
        );
    }
    if triggered_order_ids.is_empty() && last_seen_updates.is_empty() && !has_composite_changes {
        return Ok(());
    }

    let composite_entering_ids: HashSet<i64> =
        composite_bid_entering.iter().map(|(id, ..)| *id).collect();
    let composite_releasing_ids: HashSet<i64> =
        composite_bid_releasing.iter().map(|(id, ..)| *id).collect();

    let mut orders_to_spawn = Vec::new();
    {
        let mut cache = ARMED_BUILDER_ORDER_CACHE.write().await;
        let mut empty_tokens = Vec::new();

        for token_id in selected_token_ids {
            let Some(bucket) = cache.price_by_token.get_mut(&token_id) else {
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
                if composite_entering_ids.contains(&order_id) {
                    bucket[idx].last_error = Some("composite_bid_confirmation_waiting".to_string());
                }
                if composite_releasing_ids.contains(&order_id) {
                    bucket[idx].last_error = None;
                }
                idx += 1;
            }
            if bucket.is_empty() {
                empty_tokens.push(token_id);
            }
        }

        for token_id in empty_tokens {
            cache.price_by_token.remove(&token_id);
        }
        for order_id in &triggered_order_ids {
            remove_order_from_armed_builder_bucket(&mut cache.ptb_by_asset, *order_id);
            remove_order_from_armed_builder_bucket(&mut cache.ptb_by_market_slug, *order_id);
        }
    }

    for (order_id, user_id) in orders_to_spawn {
        spawn_armed_order_immediate_processing(repo, run_id, ws, order_id, user_id, None);
    }

    for (order_id, trigger_price, best_bid, last_trade_price) in composite_bid_entering {
        let _ = repo
            .set_trade_builder_order_last_error(
                order_id,
                Some("composite_bid_confirmation_waiting"),
            )
            .await;
        let _ = repo
            .append_trade_builder_order_event(
                order_id,
                "composite_bid_confirmation_waiting",
                &json!({
                    "trigger_price": trigger_price,
                    "best_bid": best_bid,
                    "last_trade_price": last_trade_price,
                    "source": "ws_fast_path",
                }),
            )
            .await;
        info!(
            run_id,
            builder_order_id = order_id,
            ?trigger_price,
            ?best_bid,
            ?last_trade_price,
            "COMPOSITE_BID_CONFIRMATION_WAITING"
        );
    }

    for (order_id, trigger_eval_price, best_bid) in composite_bid_releasing {
        let _ = repo
            .set_trade_builder_order_last_error(order_id, None)
            .await;
        let _ = repo
            .append_trade_builder_order_event(
                order_id,
                "composite_bid_confirmation_released",
                &json!({
                    "reason_code": "trigger_no_longer_valid",
                    "trigger_eval_price": trigger_eval_price,
                    "best_bid": best_bid,
                    "source": "ws_fast_path",
                }),
            )
            .await;
        info!(
            run_id,
            builder_order_id = order_id,
            trigger_eval_price,
            ?best_bid,
            "COMPOSITE_BID_CONFIRMATION_RELEASED"
        );
    }

    Ok(())
}

fn collect_armed_builder_ptb_orders_for_dirty_context(
    cache: &ArmedBuilderOrderCache,
    dirty_assets: &[String],
    dirty_market_slugs: &[String],
) -> Vec<TradeBuilderOrder> {
    let mut selected_orders = HashMap::new();

    for asset in dirty_assets {
        let asset_key = asset.trim().to_ascii_lowercase();
        let Some(bucket) = cache.ptb_by_asset.get(&asset_key) else {
            continue;
        };
        for order in bucket {
            selected_orders.insert(order.id, order.clone());
        }
    }

    for market_slug in dirty_market_slugs {
        let market_key = market_slug.trim().to_ascii_lowercase();
        let Some(bucket) = cache.ptb_by_market_slug.get(&market_key) else {
            continue;
        };
        for order in bucket {
            selected_orders.insert(order.id, order.clone());
        }
    }

    selected_orders.into_values().collect()
}

fn evaluate_armed_builder_ptb_dirty_orders(orders: &[TradeBuilderOrder]) -> HashSet<i64> {
    orders
        .iter()
        .filter_map(|order| {
            trade_builder_evaluate_ptb_stop_loss(order)
                .filter(|evaluation| evaluation.should_trigger)
                .map(|_| order.id)
        })
        .collect()
}

async fn evaluate_armed_builder_ptb_orders_for_dirty_context(
    repo: &PostgresRepository,
    run_id: i64,
    ws: &ClobWsClient,
    dirty_assets: &[String],
    dirty_market_slugs: &[String],
) -> Result<()> {
    if dirty_assets.is_empty() && dirty_market_slugs.is_empty() {
        return Ok(());
    }

    let selected_orders = {
        let cache = ARMED_BUILDER_ORDER_CACHE.read().await;
        collect_armed_builder_ptb_orders_for_dirty_context(&cache, dirty_assets, dirty_market_slugs)
    };
    if selected_orders.is_empty() {
        return Ok(());
    }

    let triggered_order_ids = evaluate_armed_builder_ptb_dirty_orders(&selected_orders);
    info!(
        run_id,
        dirty_asset_count = dirty_assets.len(),
        dirty_market_slug_count = dirty_market_slugs.len(),
        selected_order_count = selected_orders.len(),
        triggered_order_count = triggered_order_ids.len(),
        "ARMED_BUILDER_PTB_DIRTY_EVAL_CYCLE"
    );
    if triggered_order_ids.is_empty() {
        return Ok(());
    }

    let mut orders_to_spawn = Vec::new();
    {
        let mut cache = ARMED_BUILDER_ORDER_CACHE.write().await;
        for order in &selected_orders {
            if triggered_order_ids.contains(&order.id) {
                remove_order_from_armed_builder_cache(&mut cache, order.id);
                orders_to_spawn.push((order.id, order.user_id));
            }
        }
    }

    for (order_id, user_id) in orders_to_spawn {
        spawn_armed_order_immediate_processing(repo, run_id, ws, order_id, user_id, None);
    }

    Ok(())
}

fn spawn_armed_order_immediate_processing(
    repo: &PostgresRepository,
    run_id: i64,
    ws: &ClobWsClient,
    order_id: i64,
    user_id: i64,
    batch_telemetry: Option<TradeBuilderParallelExitBatchMemberTelemetry>,
) {
    let repo = repo.clone();
    let ws = ws.clone();
    tokio::spawn(async move {
        let mut user_cfg_cache = HashMap::new();
        let mut user_executor_cache = HashMap::new();
        let user_cfg = match load_user_app_config_cached(&repo, user_id, &mut user_cfg_cache).await
        {
            Ok(cfg) => cfg,
            Err(err) => {
                warn!(
                    run_id,
                    builder_order_id = order_id,
                    user_id,
                    error = %err,
                    "ARMED_ORDER_WS_USER_CONFIG_LOAD_FAILED"
                );
                sync_armed_builder_order_cache_for_order(&repo, order_id).await;
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
                sync_armed_builder_order_cache_for_order(&repo, order_id).await;
                return;
            }
        };

        let result = try_immediate_submit_single_builder_order(
            &repo, run_id, &user_cfg, &ws, client, order_id, "ws_armed",
        )
        .await;
        if let Err(err) = result {
            warn!(
                run_id,
                builder_order_id = order_id,
                user_id,
                error = %err,
                "ARMED_ORDER_WS_IMMEDIATE_SUBMIT_FAILED"
            );
        }
        if let Some(batch_telemetry) = batch_telemetry {
            let latest_status = repo
                .get_trade_builder_order(order_id)
                .await
                .ok()
                .flatten()
                .map(|order| order.status);
            let _ = repo
                .append_trade_builder_order_event(
                    order_id,
                    "parallel_exit_batch_member_result",
                    &json!({
                        "parent_order_id": batch_telemetry.parent_order_id,
                        "batch_owner_order_id": batch_telemetry.batch_owner_order_id,
                        "batch_path": batch_telemetry.batch_path,
                        "selected_order_ids": batch_telemetry.selected_order_ids,
                        "status_after": latest_status,
                    }),
                )
                .await;
        }
        sync_armed_builder_order_cache_for_order(&repo, order_id).await;
        sync_guarded_buy_order_cache_for_order(&repo, order_id).await;
    });
}

#[cfg(test)]
mod armed_ws_eval_tests {
    use super::*;
    use crate::trade_flow::guards::polymarket_price_to_beat::seed_price_to_beat_from_chainlink;

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
            staged_sl_retry_only_dust: false,
            staged_sl_retry_dust_metric: None,
            staged_sl_retry_dust_value: None,
            staged_sl_reentry_use_sold_notional: false,
            staged_sl_reentry_only_after_all_stages: false,
            sl_trigger_price_mode: None,
            reenter_on_sl_hit: false,
            reentry_max_attempts: 0,
            reentry_trigger_node_key: None,
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

    fn test_ptb_child_order() -> TradeBuilderOrder {
        TradeBuilderOrder {
            id: 2,
            trade_id: 78,
            user_id: 2,
            kind: "conditional".to_string(),
            status: "armed".to_string(),
            market_slug: "btc-updown-5m-2774013100".to_string(),
            token_id: "tok-ptb".to_string(),
            outcome_label: "Up".to_string(),
            side: "sell".to_string(),
            execution_mode: "market".to_string(),
            trigger_condition: Some("cross_below".to_string()),
            trigger_price: None,
            max_price: None,
            size_basis: TRADE_BUILDER_SIZE_BASIS_SHARES.to_string(),
            size_usdc: 5.0,
            target_qty: Some(5.0),
            min_price_distance_cent: 1.0,
            expires_at: None,
            eligible_after_at: None,
            eligible_before_at: None,
            max_triggers: 1,
            triggers_fired: 0,
            active_exchange_order_id: None,
            remaining_size: None,
            remaining_qty: Some(5.0),
            working_price: None,
            last_seen_price: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            parent_order_id: Some(43),
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
            ptb_stop_loss_gap_usd: Some(0.0),
            ptb_reference_price: None,
            ptb_stop_loss_rules_json: Vec::new(),
            ptb_stop_loss_time_decay_mode: Some("tighten".to_string()),
            staged_sl_retry_only_dust: false,
            staged_sl_retry_dust_metric: None,
            staged_sl_retry_dust_value: None,
            staged_sl_reentry_use_sold_notional: false,
            staged_sl_reentry_only_after_all_stages: false,
            sl_trigger_price_mode: None,
            reenter_on_sl_hit: false,
            reentry_max_attempts: 0,
            reentry_trigger_node_key: None,
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

    async fn reset_armed_builder_order_cache() {
        let mut cache = ARMED_BUILDER_ORDER_CACHE.write().await;
        cache.price_by_token.clear();
        cache.ptb_by_asset.clear();
        cache.ptb_by_market_slug.clear();
    }

    #[tokio::test]
    async fn insert_cache_upserts_existing_order() {
        let _test_guard = ARMED_BUILDER_ORDER_CACHE_TEST_MUTEX.lock().await;
        reset_armed_builder_order_cache().await;
        let mut order = test_tp_sl_child_order();
        sync_armed_builder_order_to_cache(order.clone()).await;

        order.last_seen_price = Some(0.77);
        sync_armed_builder_order_to_cache(order.clone()).await;

        let cache = ARMED_BUILDER_ORDER_CACHE.read().await;
        let bucket = cache
            .price_by_token
            .get(&order.token_id)
            .expect("bucket");
        assert_eq!(bucket.len(), 1);
        assert_eq!(bucket[0].last_seen_price, Some(0.77));
    }

    #[tokio::test]
    async fn sync_cache_routes_ptb_child_into_ptb_indexes_only() {
        let _test_guard = ARMED_BUILDER_ORDER_CACHE_TEST_MUTEX.lock().await;
        reset_armed_builder_order_cache().await;
        let order = test_ptb_child_order();

        sync_armed_builder_order_to_cache(order.clone()).await;

        let cache = ARMED_BUILDER_ORDER_CACHE.read().await;
        assert!(cache.price_by_token.get(&order.token_id).is_none());
        let asset_bucket = cache.ptb_by_asset.get("btc").expect("asset bucket");
        assert_eq!(asset_bucket.len(), 1);
        assert_eq!(asset_bucket[0].id, order.id);
        let market_bucket = cache
            .ptb_by_market_slug
            .get("btc-updown-5m-2774013100")
            .expect("market bucket");
        assert_eq!(market_bucket.len(), 1);
        assert_eq!(market_bucket[0].id, order.id);
    }

    #[tokio::test]
    async fn rearm_cache_helper_reinserts_armed_orders() {
        let _test_guard = ARMED_BUILDER_ORDER_CACHE_TEST_MUTEX.lock().await;
        reset_armed_builder_order_cache().await;
        let mut order = test_tp_sl_child_order();
        order.token_id = "tok-up-rearm-armed".to_string();

        rearm_builder_order_to_cache(order.clone()).await;

        let cache = ARMED_BUILDER_ORDER_CACHE.read().await;
        let bucket = cache
            .price_by_token
            .get(&order.token_id)
            .expect("bucket");
        assert_eq!(bucket.len(), 1);
        assert_eq!(bucket[0].id, order.id);
    }

    #[tokio::test]
    async fn rearm_cache_helper_skips_non_armed_orders() {
        let _test_guard = ARMED_BUILDER_ORDER_CACHE_TEST_MUTEX.lock().await;
        reset_armed_builder_order_cache().await;
        let mut order = test_tp_sl_child_order();
        order.token_id = "tok-up-rearm-skip".to_string();
        order.status = "triggered".to_string();

        rearm_builder_order_to_cache(order.clone()).await;

        let cache = ARMED_BUILDER_ORDER_CACHE.read().await;
        assert!(cache.price_by_token.get(&order.token_id).is_none());
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

    #[tokio::test]
    async fn collect_ptb_orders_filters_by_dirty_asset() {
        let _test_guard = ARMED_BUILDER_ORDER_CACHE_TEST_MUTEX.lock().await;
        reset_armed_builder_order_cache().await;
        let btc_order = test_ptb_child_order();
        let mut eth_order = test_ptb_child_order();
        eth_order.id = 3;
        eth_order.market_slug = "eth-updown-5m-2774013100".to_string();
        eth_order.token_id = "tok-ptb-eth".to_string();

        sync_armed_builder_order_to_cache(btc_order.clone()).await;
        sync_armed_builder_order_to_cache(eth_order.clone()).await;

        let cache = ARMED_BUILDER_ORDER_CACHE.read().await;
        let selected =
            collect_armed_builder_ptb_orders_for_dirty_context(&cache, &["btc".to_string()], &[]);
        drop(cache);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, btc_order.id);
    }

    #[tokio::test]
    async fn ptb_dirty_evaluation_uses_seeded_reference_price_from_market_dirty_context() {
        let _test_guard = ARMED_BUILDER_ORDER_CACHE_TEST_MUTEX.lock().await;
        let now_ms = Utc::now().timestamp_millis();
        trade_flow::guards::chainlink_price::seed_chainlink_price_test_ticks(
            "btc",
            &[(now_ms - 250, 70_000.0), (now_ms, 69_999.5)],
        )
        .expect("seed btc ticks");
        assert!(seed_price_to_beat_from_chainlink(
            "btc-updown-5m-2774013100",
            "btc",
            "5m",
            70_000.0,
            Some(0)
        ));
        let order = test_ptb_child_order();

        let triggered = evaluate_armed_builder_ptb_dirty_orders(&[order]);
        assert!(triggered.contains(&2));
    }
}
