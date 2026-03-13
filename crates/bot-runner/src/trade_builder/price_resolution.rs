#[derive(Debug, Clone, Copy, PartialEq)]
struct ExtractedWsPrice {
    price: f64,
    ts: Option<i64>,
    source: &'static str,
}

#[derive(Debug, Clone, Default)]
struct PriceResolutionDetail {
    source_detail: String,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
    snapshot_age_ms: Option<i64>,
    site_display_mode_decision: Option<&'static str>,
}

#[derive(Debug, Clone)]
struct ResolvedTriggerPrice {
    price: f64,
    ts: Option<i64>,
    source: String,
    detail: PriceResolutionDetail,
}

fn resolve_composite_price(
    trigger_condition: Option<&str>,
    bid: Option<f64>,
    last_trade_price: Option<f64>,
) -> Option<(f64, &'static str)> {
    match (trigger_condition.map(str::trim), bid, last_trade_price) {
        (Some("cross_above"), Some(bid), Some(last_trade_price)) => {
            Some((bid.max(last_trade_price), "composite_max_bid_last_trade"))
        }
        (Some("cross_below"), Some(bid), Some(last_trade_price)) => {
            let filtered = if bid > 0.0
                && last_trade_price < bid * SL_COMPOSITE_DIVERGENCE_KEEP_RATIO
            {
                bid
            } else {
                bid.min(last_trade_price)
            };
            Some((filtered, "composite_min_bid_last_trade"))
        }
        (_, Some(bid), None) => Some((bid, "composite_best_bid_only")),
        (_, None, Some(last_trade_price)) => {
            Some((last_trade_price, "composite_last_trade_only"))
        }
        (_, Some(bid), Some(last_trade_price)) => Some((
            bid.max(last_trade_price),
            "composite_default_max_bid_last_trade",
        )),
        _ => None,
    }
}

fn resolve_trigger_price_components(
    mode: WsPriceMode,
    trigger_condition: Option<&str>,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
) -> Option<(f64, &'static str, Option<&'static str>)> {
    match mode {
        WsPriceMode::Composite => resolve_composite_price(
            trigger_condition,
            best_bid,
            last_trade_price,
        )
        .map(|(price, source)| (price, source, None)),
        WsPriceMode::Midpoint => extract_midpoint(best_bid, best_ask)
            .map(|price| (price, "midpoint", None)),
        WsPriceMode::Raw => last_trade_price
            .map(|price| (price, "last_trade", None))
            .or_else(|| {
                extract_midpoint(best_bid, best_ask)
                    .map(|price| (price, "midpoint_fallback", None))
            }),
        WsPriceMode::LastTrade => {
            last_trade_price.map(|price| (price, "last_trade_strict", None))
        }
        WsPriceMode::SiteDisplay => resolve_site_display_price(best_bid, best_ask, last_trade_price)
            .map(|(price, decision)| (price, decision, Some(decision))),
        WsPriceMode::BestBid => best_bid.map(|price| (price, "best_bid", None)),
        WsPriceMode::BestAsk => best_ask.map(|price| (price, "best_ask", None)),
    }
}

fn resolve_trigger_price_from_market_snapshot(
    snapshot: &MarketDataSnapshot,
    mode: WsPriceMode,
    trigger_condition: Option<&str>,
) -> Option<ResolvedTriggerPrice> {
    let snapshot_age_ms = Some(
        Utc::now()
            .timestamp_millis()
            .saturating_sub(snapshot.updated_at_ms),
    );
    let best_bid = snapshot.best_bid;
    let best_ask = snapshot.best_ask;
    let last_trade_price = snapshot.last_trade_price;

    let build = |price: f64,
                 source: &str,
                 site_display_mode_decision: Option<&'static str>|
     -> ResolvedTriggerPrice {
        ResolvedTriggerPrice {
            price: clamp_probability(price),
            ts: Some(snapshot.updated_at_ms),
            source: source.to_string(),
            detail: PriceResolutionDetail {
                source_detail: source.to_string(),
                best_bid,
                best_ask,
                last_trade_price,
                snapshot_age_ms,
                site_display_mode_decision,
            },
        }
    };

    let (price, source, site_display_mode_decision) = resolve_trigger_price_components(
        mode,
        trigger_condition,
        best_bid,
        best_ask,
        last_trade_price,
    )?;
    Some(build(
        price,
        &format!("ws_cache_{source}"),
        site_display_mode_decision,
    ))
}

async fn resolve_trigger_price_from_rest(
    client: &dyn OrderExecutor,
    token_id: &str,
    price_mode: WsPriceMode,
    trigger_condition: Option<&str>,
) -> Result<ResolvedTriggerPrice> {
    let (best_bid, best_ask) = if matches!(
        price_mode,
        WsPriceMode::Midpoint
            | WsPriceMode::SiteDisplay
            | WsPriceMode::BestBid
            | WsPriceMode::BestAsk
            | WsPriceMode::Composite
    ) {
        client.best_bid_ask(token_id).await.unwrap_or((None, None))
    } else {
        (None, None)
    };
    let last_trade_price = if matches!(
        price_mode,
        WsPriceMode::Raw
            | WsPriceMode::LastTrade
            | WsPriceMode::SiteDisplay
            | WsPriceMode::Composite
    ) {
        client.last_trade_price(token_id).await.unwrap_or(None)
    } else {
        None
    };

    let build = |price: f64,
                 source: &str,
                 site_display_mode_decision: Option<&'static str>|
     -> ResolvedTriggerPrice {
        ResolvedTriggerPrice {
            price: clamp_probability(price),
            ts: None,
            source: source.to_string(),
            detail: PriceResolutionDetail {
                source_detail: source.to_string(),
                best_bid,
                best_ask,
                last_trade_price,
                snapshot_age_ms: None,
                site_display_mode_decision,
            },
        }
    };

    let resolved = resolve_trigger_price_components(
        price_mode,
        trigger_condition,
        best_bid,
        best_ask,
        last_trade_price,
    )
    .map(|(price, source, site_display_mode_decision)| {
        build(
            price,
            &format!("rest_{source}"),
            site_display_mode_decision,
        )
    });
    if let Some(resolved) = resolved {
        return Ok(resolved);
    }

    if matches!(price_mode, WsPriceMode::LastTrade | WsPriceMode::Composite) {
        return Err(anyhow::anyhow!(
            "{} price unavailable for token_id={token_id}",
            price_mode.as_str()
        ));
    }

    let fallback = client.midpoint(token_id).await?;
    Ok(build(fallback.price, "rest_midpoint", None))
}

#[cfg(test)]
fn extract_price_from_market_events(
    events: &[WsEvent],
    token_id: &str,
) -> Option<(f64, Option<i64>)> {
    extract_price_from_market_events_with_mode(events, token_id, WsPriceMode::Raw)
        .map(|value| (value.price, value.ts))
}

fn extract_price_from_market_events_with_mode(
    events: &[WsEvent],
    token_id: &str,
    mode: WsPriceMode,
) -> Option<ExtractedWsPrice> {
    extract_price_from_market_events_with_mode_and_condition(events, token_id, mode, None)
}

fn extract_price_from_market_events_with_mode_and_condition(
    events: &[WsEvent],
    token_id: &str,
    mode: WsPriceMode,
    trigger_condition: Option<&str>,
) -> Option<ExtractedWsPrice> {
    match mode {
        WsPriceMode::Composite => {
            extract_price_from_market_events_composite(events, token_id, trigger_condition)
        }
        WsPriceMode::Midpoint => extract_price_from_market_events_midpoint(events, token_id),
        WsPriceMode::Raw => extract_price_from_market_events_raw(events, token_id),
        WsPriceMode::LastTrade => extract_price_from_market_events_last_trade(events, token_id),
        WsPriceMode::SiteDisplay => extract_price_from_market_events_site_display(events, token_id),
        WsPriceMode::BestBid | WsPriceMode::BestAsk => {
            extract_price_from_market_events_book_side(events, token_id, mode)
        }
    }
}

fn extract_price_from_market_events_composite(
    events: &[WsEvent],
    token_id: &str,
    trigger_condition: Option<&str>,
) -> Option<ExtractedWsPrice> {
    let matches_token = |ev: &WsEvent| -> bool {
        ev.market.as_deref() == Some(token_id)
            || ev.payload.get("asset_id").and_then(|v| v.as_str()) == Some(token_id)
    };

    for event in events.iter().rev() {
        if matches_token(event) {
            if let Some((price, source, _)) = resolve_trigger_price_components(
                WsPriceMode::Composite,
                trigger_condition,
                extract_bid_from_payload(&event.payload),
                extract_ask_from_payload(&event.payload),
                event.price.or_else(|| parse_json_number(event.payload.get("price"))),
            ) {
                return Some(ExtractedWsPrice {
                    price,
                    ts: event.ts,
                    source,
                });
            }
        }

        if let Some(changes) = event
            .payload
            .get("price_changes")
            .and_then(|v| v.as_array())
        {
            for change in changes.iter().rev() {
                let Some(asset_id) = change.get("asset_id").and_then(|v| v.as_str()) else {
                    continue;
                };
                if asset_id != token_id {
                    continue;
                }
                if let Some((price, source, _)) = resolve_trigger_price_components(
                    WsPriceMode::Composite,
                    trigger_condition,
                    extract_bid_from_payload(change),
                    extract_ask_from_payload(change),
                    parse_json_number(change.get("price")),
                ) {
                    let ts = change
                        .get("timestamp")
                        .or_else(|| change.get("ts"))
                        .and_then(value_as_i64)
                        .or(event.ts);
                    return Some(ExtractedWsPrice { price, ts, source });
                }
            }
        }
    }

    None
}

const POLYMARKET_SITE_DISPLAY_MAX_SPREAD: f64 = 0.10;

fn resolve_site_display_price(
    bid: Option<f64>,
    ask: Option<f64>,
    raw_price: Option<f64>,
) -> Option<(f64, &'static str)> {
    if let (Some(bid), Some(ask)) = (bid, ask) {
        let spread = ask - bid;
        if spread >= 0.0 && spread <= POLYMARKET_SITE_DISPLAY_MAX_SPREAD {
            return Some(((bid + ask) / 2.0, "site_display_midpoint"));
        }
    }

    if let Some(price) = raw_price {
        return Some((price, "site_display_last_trade"));
    }

    extract_midpoint(bid, ask).map(|price| (price, "site_display_midpoint_fallback"))
}

fn extract_price_from_market_events_site_display(
    events: &[WsEvent],
    token_id: &str,
) -> Option<ExtractedWsPrice> {
    let matches_token = |ev: &WsEvent| -> bool {
        ev.market.as_deref() == Some(token_id)
            || ev.payload.get("asset_id").and_then(|v| v.as_str()) == Some(token_id)
    };

    for event in events.iter().rev() {
        if matches_token(event) {
            if let Some((price, source)) = resolve_site_display_price(
                extract_bid_from_payload(&event.payload),
                extract_ask_from_payload(&event.payload),
                event
                    .price
                    .or_else(|| parse_json_number(event.payload.get("price"))),
            ) {
                return Some(ExtractedWsPrice {
                    price,
                    ts: event.ts,
                    source,
                });
            }
        }

        if let Some(changes) = event
            .payload
            .get("price_changes")
            .and_then(|v| v.as_array())
        {
            for change in changes.iter().rev() {
                let Some(asset_id) = change.get("asset_id").and_then(|v| v.as_str()) else {
                    continue;
                };
                if asset_id != token_id {
                    continue;
                }
                if let Some((price, source)) = resolve_site_display_price(
                    extract_bid_from_payload(change),
                    extract_ask_from_payload(change),
                    parse_json_number(change.get("price")),
                ) {
                    let ts = change
                        .get("timestamp")
                        .or_else(|| change.get("ts"))
                        .and_then(value_as_i64)
                        .or(event.ts);
                    return Some(ExtractedWsPrice { price, ts, source });
                }
            }
        }
    }

    None
}

fn extract_price_from_market_events_midpoint(
    events: &[WsEvent],
    token_id: &str,
) -> Option<ExtractedWsPrice> {
    let matches_token = |ev: &WsEvent| -> bool {
        ev.market.as_deref() == Some(token_id)
            || ev.payload.get("asset_id").and_then(|v| v.as_str()) == Some(token_id)
    };

    for event in events.iter().rev() {
        if matches_token(event) {
            if let Some(price) = extract_midpoint_from_payload(&event.payload) {
                return Some(ExtractedWsPrice {
                    price,
                    ts: event.ts,
                    source: "best_bid_ask",
                });
            }
        }

        if let Some(changes) = event
            .payload
            .get("price_changes")
            .and_then(|v| v.as_array())
        {
            for change in changes.iter().rev() {
                let Some(asset_id) = change.get("asset_id").and_then(|v| v.as_str()) else {
                    continue;
                };
                if asset_id != token_id {
                    continue;
                }
                if let Some(price) = extract_midpoint_from_payload(change) {
                    let ts = change
                        .get("timestamp")
                        .or_else(|| change.get("ts"))
                        .and_then(value_as_i64)
                        .or(event.ts);
                    return Some(ExtractedWsPrice {
                        price,
                        ts,
                        source: "best_bid_ask",
                    });
                }
            }
        }
    }

    None
}

fn extract_price_from_market_events_book_side(
    events: &[WsEvent],
    token_id: &str,
    side: WsPriceMode,
) -> Option<ExtractedWsPrice> {
    let extract_fn: fn(&Value) -> Option<f64> = match side {
        WsPriceMode::BestBid => extract_bid_from_payload,
        WsPriceMode::BestAsk => extract_ask_from_payload,
        _ => return None,
    };

    let source: &'static str = match side {
        WsPriceMode::BestBid => "best_bid",
        WsPriceMode::BestAsk => "best_ask",
        _ => "unknown",
    };

    let matches_token = |ev: &WsEvent| -> bool {
        ev.market.as_deref() == Some(token_id)
            || ev.payload.get("asset_id").and_then(|v| v.as_str()) == Some(token_id)
    };

    for event in events.iter().rev() {
        if matches_token(event) {
            if let Some(price) = extract_fn(&event.payload) {
                return Some(ExtractedWsPrice {
                    price,
                    ts: event.ts,
                    source,
                });
            }
        }

        if let Some(changes) = event
            .payload
            .get("price_changes")
            .and_then(|v| v.as_array())
        {
            for change in changes.iter().rev() {
                let Some(asset_id) = change.get("asset_id").and_then(|v| v.as_str()) else {
                    continue;
                };
                if asset_id != token_id {
                    continue;
                }
                if let Some(price) = extract_fn(change) {
                    let ts = change
                        .get("timestamp")
                        .or_else(|| change.get("ts"))
                        .and_then(value_as_i64)
                        .or(event.ts);
                    return Some(ExtractedWsPrice { price, ts, source });
                }
            }
        }
    }

    None
}

fn extract_price_from_market_events_raw(
    events: &[WsEvent],
    token_id: &str,
) -> Option<ExtractedWsPrice> {
    let matches_token = |ev: &WsEvent| -> bool {
        ev.market.as_deref() == Some(token_id)
            || ev.payload.get("asset_id").and_then(|v| v.as_str()) == Some(token_id)
    };

    for event in events.iter().rev() {
        if let Some(price) = event.price {
            if matches_token(event) {
                return Some(ExtractedWsPrice {
                    price,
                    ts: event.ts,
                    source: "event_price",
                });
            }
        }

        if matches_token(event) {
            if let Some(price) = parse_json_number(event.payload.get("price")) {
                return Some(ExtractedWsPrice {
                    price,
                    ts: event.ts,
                    source: "payload_price",
                });
            }
        }

        if let Some(changes) = event
            .payload
            .get("price_changes")
            .and_then(|v| v.as_array())
        {
            for change in changes.iter().rev() {
                let Some(asset_id) = change.get("asset_id").and_then(|v| v.as_str()) else {
                    continue;
                };
                if asset_id != token_id {
                    continue;
                }
                if let Some(price) = parse_json_number(change.get("price")) {
                    let ts = change
                        .get("timestamp")
                        .or_else(|| change.get("ts"))
                        .and_then(value_as_i64)
                        .or(event.ts);
                    return Some(ExtractedWsPrice {
                        price,
                        ts,
                        source: "price_changes",
                    });
                }
            }
        }

        if matches_token(event) {
            if let Some(price) = extract_midpoint_from_payload(&event.payload) {
                return Some(ExtractedWsPrice {
                    price,
                    ts: event.ts,
                    source: "best_bid_ask",
                });
            }
        }
    }

    None
}

fn extract_price_from_market_events_last_trade(
    events: &[WsEvent],
    token_id: &str,
) -> Option<ExtractedWsPrice> {
    let matches_token = |ev: &WsEvent| -> bool {
        ev.market.as_deref() == Some(token_id)
            || ev.payload.get("asset_id").and_then(|v| v.as_str()) == Some(token_id)
    };

    for event in events.iter().rev() {
        if let Some(price) = event.price {
            if matches_token(event) {
                return Some(ExtractedWsPrice {
                    price,
                    ts: event.ts,
                    source: "event_price",
                });
            }
        }

        if matches_token(event) {
            if let Some(price) = parse_json_number(event.payload.get("price")) {
                return Some(ExtractedWsPrice {
                    price,
                    ts: event.ts,
                    source: "payload_price",
                });
            }
        }

        if let Some(changes) = event
            .payload
            .get("price_changes")
            .and_then(|v| v.as_array())
        {
            for change in changes.iter().rev() {
                let Some(asset_id) = change.get("asset_id").and_then(|v| v.as_str()) else {
                    continue;
                };
                if asset_id != token_id {
                    continue;
                }
                if let Some(price) = parse_json_number(change.get("price")) {
                    let ts = change
                        .get("timestamp")
                        .or_else(|| change.get("ts"))
                        .and_then(value_as_i64)
                        .or(event.ts);
                    return Some(ExtractedWsPrice {
                        price,
                        ts,
                        source: "price_changes",
                    });
                }
            }
        }
    }

    None
}

fn extract_bid_from_payload(payload: &Value) -> Option<f64> {
    parse_json_number(
        payload
            .get("best_bid")
            .or_else(|| payload.get("bestBid"))
            .or_else(|| payload.get("bid")),
    )
}

fn extract_ask_from_payload(payload: &Value) -> Option<f64> {
    parse_json_number(
        payload
            .get("best_ask")
            .or_else(|| payload.get("bestAsk"))
            .or_else(|| payload.get("ask")),
    )
}

fn extract_midpoint_from_payload(payload: &Value) -> Option<f64> {
    extract_midpoint(
        extract_bid_from_payload(payload),
        extract_ask_from_payload(payload),
    )
}

fn extract_midpoint(bid: Option<f64>, ask: Option<f64>) -> Option<f64> {
    match (bid, ask) {
        (Some(bid), Some(ask)) => Some((bid + ask) / 2.0),
        _ => None,
    }
}

fn parse_json_number(value: Option<&serde_json::Value>) -> Option<f64> {
    match value {
        Some(serde_json::Value::Number(v)) => v.as_f64(),
        Some(serde_json::Value::String(v)) => v.parse::<f64>().ok(),
        _ => None,
    }
}
