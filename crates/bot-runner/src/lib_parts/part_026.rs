async fn finalize_builder_fill(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    exchange_order_id: &str,
    canonical_entry_qty: f64,
    canonical_entry_qty_source: &str,
    actual_fill_qty: Option<f64>,
    execution_price: f64,
    force_terminal: bool,
    actual_fill_qty_source: Option<&str>,
) -> Result<()> {
    let canonical_entry_qty = round_trade_builder_share_qty(canonical_entry_qty);
    let actual_fill_qty = normalize_trade_builder_terminal_fill_qty_candidate(actual_fill_qty);
    if order.side == "buy" && (order.tp_enabled || order.sl_enabled) {
        anyhow::ensure!(
            canonical_entry_qty > 0.0,
            "builder buy fill qty must be > 0 before creating exit children"
        );
    }
    repo.increment_trade_builder_trigger_count(order.id).await?;
    if let Some(actual_fill_qty) = actual_fill_qty {
        repo.set_trade_builder_order_filled_qty(order.id, actual_fill_qty)
            .await?;
    }
    if order.side == "buy" {
        maybe_record_trade_builder_buy_fill_observation(
            repo,
            order,
            exchange_order_id,
            actual_fill_qty,
            execution_price,
            actual_fill_qty_source,
            force_terminal,
        )
        .await;
        if canonical_entry_qty_source == "submitted_dynamic_qty" {
            repo.append_trade_builder_order_event(
                order.id,
                "dynamic_qty_used_for_children",
                &json!({
                    "exchange_order_id": exchange_order_id,
                    "canonical_entry_qty": canonical_entry_qty,
                    "canonical_entry_qty_source": canonical_entry_qty_source,
                    "actual_fill_qty": actual_fill_qty,
                    "actual_fill_qty_source": actual_fill_qty_source,
                    "execution_price": execution_price,
                }),
            )
            .await?;
            match actual_fill_qty {
                Some(actual_fill_qty)
                    if (actual_fill_qty - canonical_entry_qty).abs()
                        >= TRADE_BUILDER_EXIT_QTY_TOLERANCE =>
                {
                    repo.append_trade_builder_order_event(
                        order.id,
                        "dynamic_vs_actual_fill_mismatch",
                        &json!({
                            "exchange_order_id": exchange_order_id,
                            "canonical_entry_qty": canonical_entry_qty,
                            "actual_fill_qty": actual_fill_qty,
                            "actual_fill_qty_source": actual_fill_qty_source,
                            "qty_delta": round_trade_builder_signed_qty(
                                canonical_entry_qty - actual_fill_qty
                            ),
                        }),
                    )
                    .await?;
                }
                None => {
                    repo.append_trade_builder_order_event(
                        order.id,
                        "actual_fill_qty_unresolved",
                        &json!({
                            "exchange_order_id": exchange_order_id,
                            "canonical_entry_qty": canonical_entry_qty,
                            "canonical_entry_qty_source": canonical_entry_qty_source,
                            "submitted_dynamic_price": trade_builder_submitted_dynamic_price(order),
                        }),
                    )
                    .await?;
                }
                _ => {}
            }
        }
    }

    let next_trigger_count = order.triggers_fired + 1;
    let reached_limit = next_trigger_count >= order.max_triggers;
    let next_status = if force_terminal || order.kind == "immediate" || reached_limit {
        "completed"
    } else {
        "armed"
    };

    repo.clear_trade_builder_active_exchange_order(order.id, next_status)
        .await?;
    repo.append_trade_builder_order_event(
        order.id,
        "filled",
        &json!({
            "exchange_order_id": exchange_order_id,
            "canonical_entry_qty": canonical_entry_qty,
            "canonical_entry_qty_source": canonical_entry_qty_source,
            "actual_fill_qty": actual_fill_qty,
            "actual_fill_qty_source": actual_fill_qty_source,
            "execution_price": execution_price,
            "triggers_fired": next_trigger_count,
            "max_triggers": order.max_triggers,
            "next_status": next_status
        }),
    )
    .await?;

    if should_request_trade_builder_oco_cancel(order, "filled") {
        request_trade_builder_oco_cancel_for_siblings(repo, order, "child_exit_filled").await?;
    }

    // Take Profit / Stop Loss: buy fill olunca otomatik conditional IOC sell child orderlari olustur
    if order.side == "buy" && order.tp_enabled {
        if let Some(tp_price) = order.tp_price {
            let tp_sizing = trade_builder_exit_child_sizing(canonical_entry_qty, execution_price);
            let tp_sell_id = repo
                .create_trade_builder_order(
                    order.trade_id,
                    "conditional",
                    "pending",
                    &order.market_slug,
                    &order.token_id,
                    &order.outcome_label,
                    "sell",
                    "market",
                    Some("cross_above"),
                    Some(tp_price),
                    None,
                    TRADE_BUILDER_SIZE_BASIS_SHARES,
                    tp_sizing.size_usdc,
                    Some(tp_sizing.target_qty),
                    Some(tp_sizing.remaining_qty),
                    order.min_price_distance_cent,
                    order.expires_at,
                    1,
                    Some(order.id),
                    false,
                    None,
                    false,
                    None,
                    order.fee_rate_bps,
                    None,
                    None,
                    None,
                )
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "tp_sell_created",
                &json!({
                    "child_order_id": tp_sell_id,
                    "tp_price": tp_price,
                    "tp_execution_mode": "market_ioc",
                    "size_basis": TRADE_BUILDER_SIZE_BASIS_SHARES,
                    "target_qty": tp_sizing.target_qty,
                    "canonical_entry_qty": canonical_entry_qty,
                    "actual_fill_qty": actual_fill_qty,
                    "execution_price": execution_price,
                }),
            )
            .await?;
            info!(
                builder_order_id = order.id,
                tp_sell_order_id = tp_sell_id,
                tp_price,
                "TRADE_BUILDER_TP_SELL_CREATED"
            );
        }
    }
    if order.side == "buy" && order.sl_enabled {
        if let Some(sl_price) = order.sl_price {
            let sl_sizing = trade_builder_exit_child_sizing(canonical_entry_qty, execution_price);
            let sl_sell_id = repo
                .create_trade_builder_order(
                    order.trade_id,
                    "conditional",
                    "pending",
                    &order.market_slug,
                    &order.token_id,
                    &order.outcome_label,
                    "sell",
                    "market",
                    Some("cross_below"),
                    Some(sl_price),
                    None,
                    TRADE_BUILDER_SIZE_BASIS_SHARES,
                    sl_sizing.size_usdc,
                    Some(sl_sizing.target_qty),
                    Some(sl_sizing.remaining_qty),
                    order.min_price_distance_cent,
                    order.expires_at,
                    1,
                    Some(order.id),
                    false,
                    None,
                    false,
                    None,
                    order.fee_rate_bps,
                    None,
                    None,
                    None,
                )
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "sl_sell_created",
                &json!({
                    "child_order_id": sl_sell_id,
                    "sl_price": sl_price,
                    "sl_execution_mode": "market_ioc",
                    "size_basis": TRADE_BUILDER_SIZE_BASIS_SHARES,
                    "target_qty": sl_sizing.target_qty,
                    "canonical_entry_qty": canonical_entry_qty,
                    "actual_fill_qty": actual_fill_qty,
                    "execution_price": execution_price,
                }),
            )
            .await?;
            info!(
                builder_order_id = order.id,
                sl_sell_order_id = sl_sell_id,
                sl_price,
                "TRADE_BUILDER_SL_SELL_CREATED"
            );
        }
    }

    // Unblock next DCA level for the same trade + token
    if let Ok(Some(unblocked_id)) = repo
        .unblock_next_trade_builder_order(order.trade_id, &order.token_id)
        .await
    {
        info!(
            builder_order_id = order.id,
            unblocked_order_id = unblocked_id,
            trade_id = order.trade_id,
            "TRADE_BUILDER_DCA_NEXT_LEVEL_UNBLOCKED"
        );
    }

    Ok(())
}

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

fn resolve_trigger_price_from_market_snapshot(
    snapshot: &MarketDataSnapshot,
    mode: WsPriceMode,
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

    match mode {
        WsPriceMode::Midpoint => extract_midpoint(best_bid, best_ask)
            .map(|price| build(price, "ws_cache_midpoint", None)),
        WsPriceMode::Raw => last_trade_price
            .map(|price| build(price, "ws_cache_last_trade", None))
            .or_else(|| {
                extract_midpoint(best_bid, best_ask)
                    .map(|price| build(price, "ws_cache_midpoint_fallback", None))
            }),
        WsPriceMode::LastTrade => {
            last_trade_price.map(|price| build(price, "ws_cache_last_trade_strict", None))
        }
        WsPriceMode::SiteDisplay => {
            let (price, decision) =
                resolve_site_display_price(best_bid, best_ask, last_trade_price)?;
            Some(build(
                price,
                &format!("ws_cache_{decision}"),
                Some(decision),
            ))
        }
        WsPriceMode::BestBid => best_bid.map(|price| build(price, "ws_cache_best_bid", None)),
        WsPriceMode::BestAsk => best_ask.map(|price| build(price, "ws_cache_best_ask", None)),
    }
}

async fn resolve_trigger_price_from_rest(
    client: &dyn OrderExecutor,
    token_id: &str,
    price_mode: WsPriceMode,
) -> Result<ResolvedTriggerPrice> {
    let (best_bid, best_ask) = if matches!(
        price_mode,
        WsPriceMode::Midpoint
            | WsPriceMode::SiteDisplay
            | WsPriceMode::BestBid
            | WsPriceMode::BestAsk
    ) {
        client.best_bid_ask(token_id).await.unwrap_or((None, None))
    } else {
        (None, None)
    };
    let last_trade_price = if matches!(
        price_mode,
        WsPriceMode::Raw | WsPriceMode::LastTrade | WsPriceMode::SiteDisplay
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

    let resolved = match price_mode {
        WsPriceMode::Midpoint => extract_midpoint(best_bid, best_ask)
            .map(|price| build(price, "rest_best_bid_ask", None)),
        WsPriceMode::Raw => last_trade_price
            .map(|price| build(price, "rest_last_trade", None))
            .or_else(|| {
                extract_midpoint(best_bid, best_ask)
                    .map(|price| build(price, "rest_midpoint_fallback", None))
            }),
        WsPriceMode::LastTrade => {
            last_trade_price.map(|price| build(price, "rest_last_trade_strict", None))
        }
        WsPriceMode::SiteDisplay => {
            resolve_site_display_price(best_bid, best_ask, last_trade_price)
                .map(|(price, decision)| build(price, &format!("rest_{decision}"), Some(decision)))
        }
        WsPriceMode::BestBid => best_bid.map(|price| build(price, "rest_best_bid", None)),
        WsPriceMode::BestAsk => best_ask.map(|price| build(price, "rest_best_ask", None)),
    };
    if let Some(resolved) = resolved {
        return Ok(resolved);
    }

    if matches!(price_mode, WsPriceMode::LastTrade) {
        return Err(anyhow::anyhow!(
            "last_trade price unavailable for token_id={token_id}"
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
    match mode {
        WsPriceMode::Midpoint => extract_price_from_market_events_midpoint(events, token_id),
        WsPriceMode::Raw => extract_price_from_market_events_raw(events, token_id),
        WsPriceMode::LastTrade => extract_price_from_market_events_last_trade(events, token_id),
        WsPriceMode::SiteDisplay => extract_price_from_market_events_site_display(events, token_id),
        WsPriceMode::BestBid | WsPriceMode::BestAsk => {
            extract_price_from_market_events_book_side(events, token_id, mode)
        }
    }
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
