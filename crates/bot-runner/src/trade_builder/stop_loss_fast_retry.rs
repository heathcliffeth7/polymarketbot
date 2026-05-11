const TRADE_BUILDER_FAST_SL_RETRY_MAX_ATTEMPTS: usize = 5;
const TRADE_BUILDER_FAST_SL_RETRY_WS_MAX_AGE_MS: i64 = 1_000;

#[derive(Debug, Clone, Copy)]
struct TradeBuilderFastStopLossSubmitPrice {
    sell_submit_price: TradeBuilderResolvedSellSubmitPrice,
    fresh_quote_age_ms: Option<i64>,
    quote_source: &'static str,
}

#[derive(Debug)]
struct TradeBuilderFastStopLossRetrySuccess {
    ack: bot_infra::exchange::OrderAck,
    client_order_id: String,
    desired_price: f64,
    sell_submit_price: TradeBuilderResolvedSellSubmitPrice,
    submit_started_at: DateTime<Utc>,
    submit_finished_at: DateTime<Utc>,
    attempt_payload: Value,
    attempt_events: Vec<Value>,
}

#[derive(Debug)]
struct TradeBuilderFastStopLossRetryFailure {
    attempt_events: Vec<Value>,
    last_error_text: Option<String>,
}

#[derive(Debug)]
enum TradeBuilderFastStopLossRetryOutcome {
    NotEligible,
    Exhausted(TradeBuilderFastStopLossRetryFailure),
    Success(TradeBuilderFastStopLossRetrySuccess),
}

#[derive(Debug)]
enum TradeBuilderFastStopLossInitialSubmitOutcome {
    NotEligible,
    ScheduledRetry,
    Resolved {
        sell_submit_price: TradeBuilderResolvedSellSubmitPrice,
        payload: Value,
    },
}

fn trade_builder_error_is_fak_no_match(error_text: &str) -> bool {
    let normalized = error_text.to_ascii_lowercase();
    normalized.contains("no orders found to match")
        || (normalized.contains("fak") && normalized.contains("no match"))
        || normalized.contains("partially filled or killed if no match")
}

fn trade_builder_should_fast_retry_stop_loss_submit(
    order: &TradeBuilderOrder,
    order_type: &str,
    size_basis: &str,
    error_text: &str,
) -> bool {
    trade_builder_is_stop_loss_child(order)
        && trade_builder_stop_loss_latched(order)
        && order.side == "sell"
        && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
        && matches!(order_type.trim().to_ascii_uppercase().as_str(), "FAK" | "IOC")
        && trade_builder_error_is_fak_no_match(error_text)
        && !trade_builder_error_indicates_balance_or_allowance(error_text)
        && !trade_builder_error_is_fatal_exchange_rejection(error_text)
}

fn trade_builder_fast_sl_retry_price(
    order: &TradeBuilderOrder,
    desired_price: f64,
) -> f64 {
    aggressive_price_for_side(&order.side, desired_price, order.min_price_distance_cent)
}

fn trade_builder_should_use_fast_stop_loss_submit_price(
    order: &TradeBuilderOrder,
    size_basis: &str,
) -> bool {
    trade_builder_is_stop_loss_child(order)
        && trade_builder_stop_loss_latched(order)
        && order.side == "sell"
        && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
}

fn trade_builder_fast_stop_loss_submit_price(
    order: &TradeBuilderOrder,
    mut resolution: TradeBuilderResolvedSellSubmitPrice,
    fresh_quote_age_ms: Option<i64>,
    quote_source: &'static str,
) -> TradeBuilderFastStopLossSubmitPrice {
    resolution.uncapped_desired_price = resolution.desired_price;
    resolution.desired_price = trade_builder_fast_sl_retry_price(order, resolution.desired_price);
    TradeBuilderFastStopLossSubmitPrice {
        sell_submit_price: resolution,
        fresh_quote_age_ms,
        quote_source,
    }
}

fn trade_builder_ws_snapshot_age_ms(snapshot: &MarketDataSnapshot, now: DateTime<Utc>) -> i64 {
    now.timestamp_millis()
        .saturating_sub(snapshot.updated_at_ms)
        .max(0)
}

#[allow(clippy::too_many_arguments)]
async fn resolve_trade_builder_fast_sl_retry_submit_price(
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    order: &TradeBuilderOrder,
    _current_price: f64,
    _runtime_best_bid: Option<f64>,
    _runtime_last_trade_price: Option<f64>,
    requested_qty: Option<f64>,
) -> Option<TradeBuilderFastStopLossSubmitPrice> {
    let (order_book_result, ws_snapshot) = tokio::join!(
        client.order_book(&order.token_id),
        ws.get_market_snapshot(&order.token_id)
    );
    let order_book = order_book_result.ok().flatten();
    let now = Utc::now();
    let fresh_ws_snapshot = ws_snapshot.filter(|snapshot| {
        trade_builder_ws_snapshot_age_ms(snapshot, now) <= TRADE_BUILDER_FAST_SL_RETRY_WS_MAX_AGE_MS
    });

    if order_book.is_none() && fresh_ws_snapshot.is_none() {
        return None;
    }

    let rest_best_bid = order_book
        .as_ref()
        .and_then(trade_builder_order_book_best_bid);
    let ws_best_bid = fresh_ws_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.best_bid)
        .and_then(|price| normalize_trade_builder_reference_price(Some(price)))
        .map(clamp_probability);
    let best_bid = rest_best_bid.or(ws_best_bid)?;
    let ws_age_ms = fresh_ws_snapshot
        .as_ref()
        .map(|snapshot| trade_builder_ws_snapshot_age_ms(snapshot, now));
    let fresh_quote_age_ms = if rest_best_bid.is_some() {
        Some(0)
    } else {
        ws_age_ms
    };
    let quote_source = if rest_best_bid.is_some() {
        "rest_orderbook"
    } else {
        "ws_snapshot"
    };
    let last_trade = fresh_ws_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.last_trade_price);
    let fresh_current_price = best_bid;
    let resolution = resolve_trade_builder_sell_submit_price_with_book(
        order,
        fresh_current_price,
        Some(best_bid),
        last_trade,
        requested_qty,
        order_book.as_ref(),
    );
    Some(trade_builder_fast_stop_loss_submit_price(
        order,
        resolution,
        fresh_quote_age_ms,
        quote_source,
    ))
}

#[allow(clippy::too_many_arguments)]
async fn resolve_trade_builder_fast_stop_loss_initial_submit_price(
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    order: &TradeBuilderOrder,
    current_price: f64,
    runtime_best_bid: Option<f64>,
    runtime_last_trade_price: Option<f64>,
    requested_qty: Option<f64>,
    size_basis: &str,
    exit_fast_quote: Option<&ExitFastSubmitQuote>,
) -> Option<TradeBuilderFastStopLossSubmitPrice> {
    if !trade_builder_should_use_fast_stop_loss_submit_price(order, size_basis) {
        return None;
    }

    if let Some(quote) = exit_fast_quote {
        let now = Utc::now();
        let resolution = resolve_trade_builder_sell_submit_price_with_book(
            order,
            current_price,
            runtime_best_bid,
            runtime_last_trade_price,
            requested_qty,
            Some(&quote.order_book),
        );
        return Some(trade_builder_fast_stop_loss_submit_price(
            order,
            resolution,
            Some(exit_fast_submit_quote_age_ms(quote, now)),
            "exit_fast_quote",
        ));
    }

    resolve_trade_builder_fast_sl_retry_submit_price(
        client,
        ws,
        order,
        current_price,
        runtime_best_bid,
        runtime_last_trade_price,
        requested_qty,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn prepare_trade_builder_fast_stop_loss_initial_submit(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    order: &TradeBuilderOrder,
    submit_started_at: DateTime<Utc>,
    current_price: f64,
    runtime_best_bid: Option<f64>,
    runtime_last_trade_price: Option<f64>,
    requested_qty: Option<f64>,
    size_basis: &str,
    exit_fast_quote: Option<&ExitFastSubmitQuote>,
) -> Result<TradeBuilderFastStopLossInitialSubmitOutcome> {
    if !trade_builder_should_use_fast_stop_loss_submit_price(order, size_basis) {
        return Ok(TradeBuilderFastStopLossInitialSubmitOutcome::NotEligible);
    }

    let Some(fast_submit_price) = resolve_trade_builder_fast_stop_loss_initial_submit_price(
        client,
        ws,
        order,
        current_price,
        runtime_best_bid,
        runtime_last_trade_price,
        requested_qty,
        size_basis,
        exit_fast_quote,
    )
    .await
    else {
        repo.append_trade_builder_order_event(
            order.id,
            "sl_initial_quote_unavailable",
            &json!({
                "reason": "fresh_bid_unavailable",
                "current_price": current_price,
                "runtime_best_bid": runtime_best_bid,
                "runtime_last_trade_price": runtime_last_trade_price,
                "requested_qty": requested_qty,
                "exit_fast_quote_used": exit_fast_quote.is_some(),
            }),
        )
        .await?;
        schedule_trade_builder_exit_sell_retry(
            repo,
            order,
            "submit_retry_scheduled",
            "sl_initial_quote_unavailable",
            current_price,
            current_price,
            requested_qty,
            None,
            None,
            None,
            None,
            None,
        )
        .await?;
        return Ok(TradeBuilderFastStopLossInitialSubmitOutcome::ScheduledRetry);
    };

    let sl_trigger_to_submit_ms = order.trigger_latched_at.map(|latched_at| {
        submit_started_at
            .signed_duration_since(latched_at)
            .num_milliseconds()
            .max(0)
    });
    let price_source = format!(
        "{}:{}",
        fast_submit_price.quote_source, fast_submit_price.sell_submit_price.source
    );
    Ok(TradeBuilderFastStopLossInitialSubmitOutcome::Resolved {
        sell_submit_price: fast_submit_price.sell_submit_price,
        payload: json!({
            "sl_trigger_to_submit_ms": sl_trigger_to_submit_ms,
            "sl_quote_age_ms": fast_submit_price.fresh_quote_age_ms,
            "sl_first_submit_price": fast_submit_price.sell_submit_price.desired_price,
            "sl_first_submit_price_source": price_source,
            "sl_initial_quote_source": fast_submit_price.quote_source,
            "sl_initial_submit_price_source": fast_submit_price.sell_submit_price.source,
            "sl_initial_aggressive_ticks": 1,
        }),
    })
}

async fn append_trade_builder_fast_stop_loss_retry_attempt_events(
    repo: &PostgresRepository,
    order_id: i64,
    events: &[Value],
) -> Result<()> {
    for event in events {
        repo.append_trade_builder_order_event(order_id, "fast_sl_retry_attempt", event)
            .await?;
    }
    Ok(())
}

fn append_trade_builder_fast_stop_loss_initial_submit_payload(
    raw_payload: &mut serde_json::Map<String, Value>,
    payload: Value,
) {
    raw_payload.insert("fast_sl_initial_submit".to_string(), payload.clone());
    for key in [
        "sl_trigger_to_submit_ms",
        "sl_quote_age_ms",
        "sl_first_submit_price",
        "sl_first_submit_price_source",
    ] {
        if let Some(value) = payload.get(key) {
            raw_payload.insert(key.to_string(), value.clone());
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn try_trade_builder_fast_stop_loss_retry(
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    order: &TradeBuilderOrder,
    base_req: &PlaceOrderRequest,
    order_type: &str,
    size_basis: &str,
    original_error_text: &str,
    current_price: f64,
    runtime_best_bid: Option<f64>,
    runtime_last_trade_price: Option<f64>,
    requested_qty: Option<f64>,
) -> TradeBuilderFastStopLossRetryOutcome {
    if !trade_builder_should_fast_retry_stop_loss_submit(
        order,
        order_type,
        size_basis,
        original_error_text,
    ) {
        return TradeBuilderFastStopLossRetryOutcome::NotEligible;
    }

    let mut attempt_events = Vec::new();
    let mut last_error_text = Some(original_error_text.to_string());
    for attempt in 1..=TRADE_BUILDER_FAST_SL_RETRY_MAX_ATTEMPTS {
        let Some(fast_submit_price) =
            resolve_trade_builder_fast_sl_retry_submit_price(
                client,
                ws,
                order,
                current_price,
                runtime_best_bid,
                runtime_last_trade_price,
                requested_qty,
            )
            .await
        else {
            attempt_events.push(json!({
                "fast_sl_retry_attempt": attempt,
                "retry_reason": original_error_text,
                "retry_price": Value::Null,
                "fresh_quote_age_ms": Value::Null,
                "quote_source": "unavailable",
                "result": "quote_unavailable",
            }));
            break;
        };
        let sell_submit_price = fast_submit_price.sell_submit_price;

        let client_order_id = format!("tb-{}", Uuid::new_v4());
        let mut retry_req = base_req.clone();
        retry_req.price = sell_submit_price.desired_price;
        retry_req.client_order_id = client_order_id.clone();
        let submit_started_at = Utc::now();
        let ack = client.place(&retry_req).await;
        let submit_finished_at = Utc::now();
        let mut attempt_payload = json!({
            "fast_sl_retry_attempt": attempt,
            "retry_reason": original_error_text,
            "retry_price": sell_submit_price.desired_price,
            "fresh_quote_age_ms": fast_submit_price.fresh_quote_age_ms,
            "quote_source": fast_submit_price.quote_source,
            "submit_price_source": sell_submit_price.source,
            "submit_price_depth_levels_used": sell_submit_price.depth_levels_used,
            "submit_price_visible_bid_qty": sell_submit_price.visible_bid_qty,
            "submit_price_requested_qty": sell_submit_price.requested_qty,
            "client_order_id": client_order_id,
            "requested_qty": requested_qty,
            "submitted_at": submit_started_at.to_rfc3339(),
            "finished_at": submit_finished_at.to_rfc3339(),
        });

        match ack {
            Ok(ack) => {
                if let Some(payload) = attempt_payload.as_object_mut() {
                    payload.insert("result".to_string(), json!("submitted"));
                    payload.insert("status".to_string(), json!(ack.status.clone()));
                    payload.insert("raw_status".to_string(), json!(ack.raw_status.clone()));
                    payload.insert("reject_reason".to_string(), json!(ack.reject_reason.clone()));
                }
                attempt_events.push(attempt_payload.clone());
                return TradeBuilderFastStopLossRetryOutcome::Success(
                    TradeBuilderFastStopLossRetrySuccess {
                        ack,
                        client_order_id: retry_req.client_order_id,
                        desired_price: sell_submit_price.desired_price,
                        sell_submit_price,
                        submit_started_at,
                        submit_finished_at,
                        attempt_payload,
                        attempt_events,
                    },
                );
            }
            Err(err) => {
                let error_text = err.to_string();
                if let Some(payload) = attempt_payload.as_object_mut() {
                    payload.insert("result".to_string(), json!("error"));
                    payload.insert("error".to_string(), json!(error_text.clone()));
                }
                attempt_events.push(attempt_payload);
                last_error_text = Some(error_text.clone());
                if !trade_builder_error_is_fak_no_match(&error_text) {
                    break;
                }
            }
        }
    }

    TradeBuilderFastStopLossRetryOutcome::Exhausted(TradeBuilderFastStopLossRetryFailure {
        attempt_events,
        last_error_text,
    })
}
