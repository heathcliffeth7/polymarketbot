#[derive(Debug, Clone, Default)]
struct ArmedBuilderOrderCache {
    by_token: HashMap<String, Vec<TradeBuilderOrder>>,
}

static ARMED_BUILDER_ORDER_CACHE: LazyLock<RwLock<ArmedBuilderOrderCache>> =
    LazyLock::new(|| RwLock::new(ArmedBuilderOrderCache::default()));
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
        by_token
            .entry(order.token_id.clone())
            .or_default()
            .push(order);
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

async fn rearm_builder_order_to_cache(order: TradeBuilderOrder) {
    if order.status == "armed" {
        insert_into_armed_builder_order_cache(order).await;
    }
}

async fn rearm_builder_order_to_cache_if_armed(repo: &PostgresRepository, order_id: i64) {
    if let Ok(Some(order)) = repo.get_trade_builder_order(order_id).await {
        rearm_builder_order_to_cache(order).await;
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
            cache.by_token.remove(&token_id);
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
                rearm_builder_order_to_cache_if_armed(&repo, order_id).await;
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
                rearm_builder_order_to_cache_if_armed(&repo, order_id).await;
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
        rearm_builder_order_to_cache_if_armed(&repo, order_id).await;
        sync_guarded_buy_order_cache_for_order(&repo, order_id).await;
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
            origin_flow_definition_id: None,
            origin_flow_run_id: None,
            origin_flow_node_key: None,
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
        cache.by_token.clear();
    }

    #[tokio::test]
    async fn insert_cache_upserts_existing_order() {
        reset_armed_builder_order_cache().await;
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
    async fn rearm_cache_helper_reinserts_armed_orders() {
        reset_armed_builder_order_cache().await;
        let mut order = test_tp_sl_child_order();
        order.token_id = "tok-up-rearm-armed".to_string();

        rearm_builder_order_to_cache(order.clone()).await;

        let cache = ARMED_BUILDER_ORDER_CACHE.read().await;
        let bucket = cache.by_token.get(&order.token_id).expect("bucket");
        assert_eq!(bucket.len(), 1);
        assert_eq!(bucket[0].id, order.id);
    }

    #[tokio::test]
    async fn rearm_cache_helper_skips_non_armed_orders() {
        reset_armed_builder_order_cache().await;
        let mut order = test_tp_sl_child_order();
        order.token_id = "tok-up-rearm-skip".to_string();
        order.status = "triggered".to_string();

        rearm_builder_order_to_cache(order.clone()).await;

        let cache = ARMED_BUILDER_ORDER_CACHE.read().await;
        assert!(cache.by_token.get(&order.token_id).is_none());
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
