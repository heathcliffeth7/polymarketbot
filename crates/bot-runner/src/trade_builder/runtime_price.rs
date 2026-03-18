fn evaluate_trade_builder_order_trigger(
    order: &TradeBuilderOrder,
    previous_price: Option<f64>,
    current_price: f64,
) -> TradeBuilderTriggerEvaluation {
    if order.kind == "immediate" {
        return TradeBuilderTriggerEvaluation {
            should_trigger: matches!(
                order.status.as_str(),
                "pending" | "armed" | "triggered" | "blocked" | "guard_blocked"
            ),
            first_tick_threshold_used: false,
        };
    }
    if trade_builder_stop_loss_latched(order) {
        return TradeBuilderTriggerEvaluation {
            should_trigger: true,
            first_tick_threshold_used: false,
        };
    }

    let Some(trigger_price) = trade_builder_inventory_pending_tp_trigger_price(order) else {
        return TradeBuilderTriggerEvaluation {
            should_trigger: false,
            first_tick_threshold_used: false,
        };
    };
    let Some(trigger_condition) = order.trigger_condition.as_deref() else {
        return TradeBuilderTriggerEvaluation {
            should_trigger: false,
            first_tick_threshold_used: false,
        };
    };

    if trade_builder_is_child_exit_sell(order) {
        let should_trigger = match trigger_condition {
            "cross_above" | "level_above" => current_price >= trigger_price,
            "cross_below" | "level_below" => current_price <= trigger_price,
            _ => false,
        };
        return TradeBuilderTriggerEvaluation {
            should_trigger,
            first_tick_threshold_used: should_trigger && previous_price.is_none(),
        };
    }

    let first_tick_threshold_used =
        previous_price.is_none() && trade_builder_is_child_exit_sell(order);
    let should_trigger = match trigger_condition {
        "level_above" => current_price >= trigger_price,
        "level_below" => current_price <= trigger_price,
        "cross_above" if first_tick_threshold_used => current_price >= trigger_price,
        "cross_below" if first_tick_threshold_used => current_price <= trigger_price,
        "cross_above"
            if matches!(
                order.status.as_str(),
                "triggered" | "blocked" | "guard_blocked" | "inventory_pending"
            ) =>
        {
            current_price >= trigger_price
        }
        "cross_below"
            if matches!(
                order.status.as_str(),
                "triggered" | "blocked" | "guard_blocked" | "inventory_pending"
            ) =>
        {
            current_price <= trigger_price
        }
        "cross_above" => crossed_above_strict(previous_price, current_price, trigger_price),
        "cross_below" => crossed_below_strict(previous_price, current_price, trigger_price),
        _ => false,
    };

    TradeBuilderTriggerEvaluation {
        should_trigger,
        first_tick_threshold_used: should_trigger
            && (first_tick_threshold_used
                || (previous_price.is_none()
                    && matches!(trigger_condition, "level_above" | "level_below"))),
    }
}

#[cfg(test)]
fn should_trigger_builder_order(order: &TradeBuilderOrder, current_price: f64) -> bool {
    evaluate_trade_builder_order_trigger(order, order.last_seen_price, current_price).should_trigger
}

fn trade_builder_runtime_price_fallback(
    order: &TradeBuilderOrder,
) -> Option<TradeBuilderRuntimePrice> {
    if let Some(price) = normalize_trade_builder_reference_price(order.last_seen_price) {
        return Some(TradeBuilderRuntimePrice {
            price: clamp_probability(price),
            source: "last_seen_price",
            runtime_warning: None,
            best_bid: None,
            best_ask: None,
            last_trade_price: None,
        });
    }
    if let Some(price) = normalize_trade_builder_reference_price(order.working_price) {
        return Some(TradeBuilderRuntimePrice {
            price: clamp_probability(price),
            source: "working_price",
            runtime_warning: None,
            best_bid: None,
            best_ask: None,
            last_trade_price: None,
        });
    }
    None
}

fn trade_builder_runtime_warning(errors: Vec<String>) -> Option<String> {
    let errors: Vec<String> = errors
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    if errors.is_empty() {
        None
    } else {
        Some(errors.join(" | "))
    }
}

fn build_trade_builder_fast_runtime_price(
    source: &'static str,
    runtime_warning: Option<String>,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
) -> Option<TradeBuilderRuntimePrice> {
    let best_bid = best_bid.map(clamp_probability);
    let best_ask = best_ask.map(clamp_probability);
    let last_trade_price = last_trade_price.map(clamp_probability);
    let price = best_bid.or(last_trade_price)?;
    Some(TradeBuilderRuntimePrice {
        price,
        source,
        runtime_warning,
        best_bid,
        best_ask,
        last_trade_price,
    })
}

fn trade_builder_fast_runtime_price_source(
    prefix: &'static str,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
) -> Option<&'static str> {
    let has_book = best_bid.is_some() || best_ask.is_some();
    let has_last_trade = last_trade_price.is_some();
    match (prefix, has_book, has_last_trade) {
        ("ws", true, true) => Some("ws_fast_book_last_trade"),
        ("ws", true, false) => Some("ws_fast_book"),
        ("ws", false, true) => Some("ws_fast_last_trade"),
        ("rest", true, true) => Some("rest_fast_book_last_trade"),
        ("rest", true, false) => Some("rest_fast_book"),
        ("rest", false, true) => Some("rest_fast_last_trade"),
        _ => None,
    }
}

fn extract_trade_builder_fast_ws_components(
    events: &[WsEvent],
    token_id: &str,
) -> (Option<f64>, Option<f64>, Option<f64>) {
    let best_bid =
        extract_price_from_market_events_with_mode(events, token_id, WsPriceMode::BestBid)
            .map(|value| value.price);
    let best_ask =
        extract_price_from_market_events_with_mode(events, token_id, WsPriceMode::BestAsk)
            .map(|value| value.price);
    let last_trade_price =
        extract_price_from_market_events_with_mode(events, token_id, WsPriceMode::LastTrade)
            .map(|value| value.price);
    (best_bid, best_ask, last_trade_price)
}

fn resolve_trade_builder_fast_runtime_price_from_rest_results(
    best_bid_ask_result: Result<(Option<f64>, Option<f64>)>,
    last_trade_result: Result<Option<f64>>,
) -> (Option<TradeBuilderRuntimePrice>, Option<String>) {
    let mut warnings = Vec::new();
    let (best_bid, best_ask) = match best_bid_ask_result {
        Ok(values) => values,
        Err(err) => {
            warnings.push(format!("best_bid_ask: {err}"));
            (None, None)
        }
    };
    let last_trade_price = match last_trade_result {
        Ok(value) => value,
        Err(err) => {
            warnings.push(format!("last_trade_price: {err}"));
            None
        }
    };
    let runtime_warning = trade_builder_runtime_warning(warnings);
    let runtime_price =
        trade_builder_fast_runtime_price_source("rest", best_bid, best_ask, last_trade_price)
            .and_then(|source| {
                build_trade_builder_fast_runtime_price(
                    source,
                    runtime_warning.clone(),
                    best_bid,
                    best_ask,
                    last_trade_price,
                )
            });
    (runtime_price, runtime_warning)
}

async fn resolve_trade_builder_fast_runtime_price(
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
) -> Result<TradeBuilderRuntimePriceFetch> {
    let mut warnings = Vec::new();
    match ws
        .subscribe_once(WsChannel::Market, &[order.token_id.clone()])
        .await
    {
        Ok(events) => {
            let (best_bid, best_ask, last_trade_price) =
                extract_trade_builder_fast_ws_components(&events, &order.token_id);
            if let Some(source) =
                trade_builder_fast_runtime_price_source("ws", best_bid, best_ask, last_trade_price)
            {
                let runtime_warning = trade_builder_runtime_warning(warnings.clone());
                if let Some(runtime_price) = build_trade_builder_fast_runtime_price(
                    source,
                    runtime_warning,
                    best_bid,
                    best_ask,
                    last_trade_price,
                ) {
                    return Ok(TradeBuilderRuntimePriceFetch::Resolved(runtime_price));
                }
            }
        }
        Err(err) => warnings.push(format!("ws_market: {err}")),
    }

    let (best_bid_ask_result, last_trade_result) = tokio::join!(
        client.best_bid_ask(&order.token_id),
        client.last_trade_price(&order.token_id)
    );
    let (runtime_price, runtime_warning) = resolve_trade_builder_fast_runtime_price_from_rest_results(
        best_bid_ask_result,
        last_trade_result,
    );
    if let Some(mut runtime_price) = runtime_price {
        if runtime_price.runtime_warning.is_none() {
            runtime_price.runtime_warning = trade_builder_runtime_warning(warnings);
        } else if let Some(ws_warning) = trade_builder_runtime_warning(warnings) {
            if let Some(existing_warning) = runtime_price.runtime_warning.take() {
                runtime_price.runtime_warning =
                    trade_builder_runtime_warning(vec![ws_warning, existing_warning]);
            } else {
                runtime_price.runtime_warning = Some(ws_warning);
            }
        }
        return Ok(TradeBuilderRuntimePriceFetch::Resolved(runtime_price));
    }

    if let Some(mut fallback) = trade_builder_runtime_price_fallback(order) {
        let mut fallback_warnings = warnings;
        if let Some(runtime_warning) = runtime_warning {
            fallback_warnings.push(runtime_warning);
        }
        fallback.runtime_warning = trade_builder_runtime_warning(fallback_warnings);
        return Ok(TradeBuilderRuntimePriceFetch::Resolved(fallback));
    }

    let mut retry_errors = warnings;
    if let Some(runtime_warning) = runtime_warning {
        retry_errors.push(runtime_warning);
    }
    let error_text = trade_builder_runtime_warning(retry_errors)
        .unwrap_or_else(|| "runtime price unavailable and no fallback price was present".to_string());
    Ok(TradeBuilderRuntimePriceFetch::Retry { error_text })
}

async fn resolve_trade_builder_runtime_price(
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
) -> Result<TradeBuilderRuntimePriceFetch> {
    if let Some(ws_price) = fetch_price_from_market_ws(ws, &order.token_id).await {
        return Ok(TradeBuilderRuntimePriceFetch::Resolved(
            TradeBuilderRuntimePrice {
                price: clamp_probability(ws_price),
                source: "ws_market",
                runtime_warning: None,
                best_bid: None,
                best_ask: None,
                last_trade_price: None,
            },
        ));
    }

    match client.midpoint(&order.token_id).await {
        Ok(snapshot) => Ok(TradeBuilderRuntimePriceFetch::Resolved(
            TradeBuilderRuntimePrice {
                price: clamp_probability(snapshot.price),
                source: "midpoint",
                runtime_warning: None,
                best_bid: None,
                best_ask: None,
                last_trade_price: None,
            },
        )),
        Err(err) => {
            let error_text = err.to_string();
            if trade_builder_error_indicates_midpoint_not_found(&error_text) {
                if let Some(mut fallback) = trade_builder_runtime_price_fallback(order) {
                    fallback.runtime_warning = Some(error_text);
                    return Ok(TradeBuilderRuntimePriceFetch::Resolved(fallback));
                }
                return Ok(TradeBuilderRuntimePriceFetch::Retry { error_text });
            }
            Err(err)
        }
    }
}

pub(crate) async fn fetch_price_from_market_ws(ws: &ClobWsClient, token_id: &str) -> Option<f64> {
    fetch_price_from_market_ws_with_mode(ws, token_id, WsPriceMode::Raw).await
}

async fn fetch_price_from_market_ws_with_mode(
    ws: &ClobWsClient,
    token_id: &str,
    mode: WsPriceMode,
) -> Option<f64> {
    let events = ws
        .subscribe_once(WsChannel::Market, &[token_id.to_string()])
        .await
        .ok()?;
    extract_price_from_market_events_with_mode(&events, token_id, mode).map(|value| value.price)
}
