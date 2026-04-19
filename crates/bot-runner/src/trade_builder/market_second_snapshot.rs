use bot_infra::db::TradeBuilderMarketSecondSnapshotInput;

#[derive(Debug, Clone)]
struct MarketSecondSnapshotTick {
    token_id: String,
    snapshot: MarketDataSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MarketSecondSnapshotTokenContext {
    market_slug: String,
    asset: String,
    outcome_side: &'static str,
}

fn normalize_market_second_snapshot_outcome_side(label: &str) -> Option<&'static str> {
    match label.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "long" | "bull" => Some("yes"),
        "no" | "down" | "short" | "bear" => Some("no"),
        _ => None,
    }
}

fn market_second_snapshot_contexts_for_token(
    token_id: &str,
) -> Vec<MarketSecondSnapshotTokenContext> {
    let mut seen = HashSet::new();
    let mut contexts = Vec::new();

    if let Ok(cache) = TRADE_FLOW_WS_FAST_PATH_CACHE.try_read() {
        if let Some(targets) = cache.token_targets.get(token_id) {
            for (run_index, node_index) in targets {
                let Some(spec) = cache
                    .run_specs
                    .get(*run_index)
                    .and_then(|run_spec| run_spec.nodes.get(*node_index))
                else {
                    continue;
                };
                let Some(market_slug) = spec.market_slug.as_deref() else {
                    continue;
                };
                let Some(scope) = find_updown_scope_by_slug(market_slug) else {
                    continue;
                };
                let Some(outcome_side) =
                    normalize_market_second_snapshot_outcome_side(&spec.outcome_label)
                else {
                    continue;
                };
                let key = (market_slug.to_string(), outcome_side);
                if seen.insert(key.clone()) {
                    contexts.push(MarketSecondSnapshotTokenContext {
                        market_slug: key.0,
                        asset: scope.asset.to_string(),
                        outcome_side,
                    });
                }
            }
        }
    }

    if let Ok(cache) = ARMED_BUILDER_ORDER_CACHE.try_read() {
        if let Some(orders) = cache.price_by_token.get(token_id) {
            for order in orders {
                let Some(scope) = find_updown_scope_by_slug(&order.market_slug) else {
                    continue;
                };
                let Some(outcome_side) =
                    normalize_market_second_snapshot_outcome_side(&order.outcome_label)
                else {
                    continue;
                };
                let key = (order.market_slug.clone(), outcome_side);
                if seen.insert(key.clone()) {
                    contexts.push(MarketSecondSnapshotTokenContext {
                        market_slug: key.0,
                        asset: scope.asset.to_string(),
                        outcome_side,
                    });
                }
            }
        }
    }

    if let Ok(cache) = GUARDED_BUY_ORDER_CACHE.try_read() {
        if let Some(orders) = cache.by_token.get(token_id) {
            for order in orders {
                let Some(scope) = find_updown_scope_by_slug(&order.market_slug) else {
                    continue;
                };
                let Some(outcome_side) =
                    normalize_market_second_snapshot_outcome_side(&order.outcome_label)
                else {
                    continue;
                };
                let key = (order.market_slug.clone(), outcome_side);
                if seen.insert(key.clone()) {
                    contexts.push(MarketSecondSnapshotTokenContext {
                        market_slug: key.0,
                        asset: scope.asset.to_string(),
                        outcome_side,
                    });
                }
            }
        }
    }

    contexts
}

pub(crate) fn trade_builder_second_snapshot_window(
    market_slug: &str,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let scope = find_updown_scope_by_slug(market_slug)?;
    let window_start = MarketCycleId(market_slug.to_string()).start_time()?;
    let window_end = window_start + ChronoDuration::seconds(updown_scope_window_seconds(scope));
    Some((window_start, window_end))
}

fn order_book_best_bid(book: &OrderBookSnapshot) -> Option<f64> {
    book.bids
        .iter()
        .max_by(|left, right| left.price.total_cmp(&right.price))
        .map(|level| clamp_probability(level.price))
}

fn order_book_best_ask(book: &OrderBookSnapshot) -> Option<f64> {
    book.asks
        .iter()
        .min_by(|left, right| left.price.total_cmp(&right.price))
        .map(|level| clamp_probability(level.price))
}

fn order_book_best_ask_depth_usdc(book: &OrderBookSnapshot) -> Option<f64> {
    let best_ask = order_book_best_ask(book)?;
    let depth = book
        .asks
        .iter()
        .filter(|level| (level.price - best_ask).abs() <= f64::EPSILON)
        .map(|level| level.price * level.size)
        .sum::<f64>();
    (depth.is_finite() && depth > 0.0).then_some(depth)
}

fn build_market_second_snapshot_input(
    context: &MarketSecondSnapshotTokenContext,
    second_ts: DateTime<Utc>,
    ptb_ref_price: Option<f64>,
    chainlink_price: Option<f64>,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    ask_depth_usdc: Option<f64>,
) -> Option<TradeBuilderMarketSecondSnapshotInput> {
    let (window_start, window_end) = trade_builder_second_snapshot_window(&context.market_slug)?;
    Some(TradeBuilderMarketSecondSnapshotInput {
        market_slug: context.market_slug.clone(),
        asset: context.asset.clone(),
        window_start,
        window_end,
        second_ts,
        ptb_ref_price,
        chainlink_price,
        yes_best_bid: (context.outcome_side == "yes").then_some(best_bid).flatten(),
        yes_best_ask: (context.outcome_side == "yes").then_some(best_ask).flatten(),
        yes_ask_depth_usdc: (context.outcome_side == "yes")
            .then_some(ask_depth_usdc)
            .flatten(),
        no_best_bid: (context.outcome_side == "no").then_some(best_bid).flatten(),
        no_best_ask: (context.outcome_side == "no").then_some(best_ask).flatten(),
        no_ask_depth_usdc: (context.outcome_side == "no")
            .then_some(ask_depth_usdc)
            .flatten(),
        sample_count: 1,
    })
}

fn build_market_second_snapshot_callback(
    tx: tokio::sync::mpsc::UnboundedSender<MarketSecondSnapshotTick>,
) -> MarketTickCallback {
    Arc::new(move |token_id, snapshot| {
        let _ = tx.send(MarketSecondSnapshotTick {
            token_id: token_id.to_string(),
            snapshot: snapshot.clone(),
        });
    })
}

fn build_combined_market_tick_callback(callbacks: Vec<MarketTickCallback>) -> MarketTickCallback {
    Arc::new(move |token_id, snapshot| {
        for callback in &callbacks {
            callback(token_id, snapshot);
        }
    })
}

async fn run_market_second_snapshot_recorder<C>(
    repo: PostgresRepository,
    client: C,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<MarketSecondSnapshotTick>,
) where
    C: OrderExecutor + Clone + Send + Sync + 'static,
{
    let mut order_book_cache: HashMap<(String, i64), Option<OrderBookSnapshot>> = HashMap::new();
    while let Some(tick) = rx.recv().await {
        let contexts = market_second_snapshot_contexts_for_token(&tick.token_id);
        if contexts.is_empty() {
            continue;
        }

        let second_bucket = tick.snapshot.updated_at_ms.div_euclid(1_000);
        if second_bucket <= 0 {
            continue;
        }
        let Some(second_ts) = DateTime::<Utc>::from_timestamp(second_bucket, 0) else {
            continue;
        };
        let order_book = if let Some(cached) =
            order_book_cache.get(&(tick.token_id.clone(), second_bucket))
        {
            cached.clone()
        } else {
            let fetched = client.order_book(&tick.token_id).await.ok().flatten();
            order_book_cache.insert((tick.token_id.clone(), second_bucket), fetched.clone());
            fetched
        };

        let book_best_bid = order_book.as_ref().and_then(order_book_best_bid);
        let book_best_ask = order_book.as_ref().and_then(order_book_best_ask);
        let book_best_ask_depth_usdc = order_book.as_ref().and_then(order_book_best_ask_depth_usdc);
        let best_bid = book_best_bid.or(tick.snapshot.best_bid);
        let best_ask = book_best_ask.or(tick.snapshot.best_ask);

        for context in &contexts {
            let ptb_ref_price =
                trade_flow::guards::polymarket_price_to_beat::get_price_to_beat_cached(
                    &context.market_slug,
                )
                .map(|snapshot| snapshot.price_to_beat)
                .filter(|value| value.is_finite() && *value > 0.0);
            let chainlink_price =
                trade_flow::guards::chainlink_price::get_chainlink_price_cached(&context.asset).ok();
            let Some(input) = build_market_second_snapshot_input(
                context,
                second_ts,
                ptb_ref_price,
                chainlink_price,
                best_bid,
                best_ask,
                book_best_ask_depth_usdc,
            ) else {
                continue;
            };
            if let Err(err) = repo
                .upsert_trade_builder_market_second_snapshot(&input)
                .await
            {
                warn!(
                    token_id = %tick.token_id,
                    market_slug = %context.market_slug,
                    error = %err,
                    "MARKET_SECOND_SNAPSHOT_UPSERT_FAILED"
                );
            }
        }

        if order_book_cache.len() > 256 {
            order_book_cache.retain(|(_, second), _| *second >= second_bucket.saturating_sub(2));
        }
    }
}
