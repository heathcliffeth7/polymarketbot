const TRADE_BUILDER_MARKET_SPEC_CACHE_TTL_SECS: u64 = 300;

#[derive(Debug, Clone, Copy, Default)]
struct TradeBuilderMarketSpec {
    neg_risk: bool,
    order_price_min_tick_size: Option<f64>,
    order_min_size: Option<f64>,
}

static TRADE_BUILDER_MARKET_SPEC_CACHE: LazyLock<
    StdMutex<HashMap<String, (Instant, TradeBuilderMarketSpec)>>,
> = LazyLock::new(|| StdMutex::new(HashMap::new()));

fn normalize_trade_builder_market_spec_number(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value > 0.0)
}

fn trade_builder_market_spec_slug_candidates(market_slug: &str) -> Vec<String> {
    let normalized = market_slug.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Vec::new();
    }
    let mut candidates = vec![normalized.clone()];
    let mut current = normalized.as_str();
    for _ in 0..4 {
        let Some((parent, _)) = current.rsplit_once('-') else { break };
        if parent.len() < 3 { break }
        if !candidates.iter().any(|c| c == parent) {
            candidates.push(parent.to_string());
        }
        current = parent;
    }
    candidates
}

fn trade_builder_market_spec_cache_get(market_slug: &str) -> Option<TradeBuilderMarketSpec> {
    let cache = TRADE_BUILDER_MARKET_SPEC_CACHE.lock().ok()?;
    let (cached_at, spec) = cache.get(market_slug)?;
    if cached_at.elapsed().as_secs() > TRADE_BUILDER_MARKET_SPEC_CACHE_TTL_SECS {
        return None;
    }
    Some(*spec)
}

fn trade_builder_market_spec_cache_put(market_slug: &str, spec: TradeBuilderMarketSpec) {
    if let Ok(mut cache) = TRADE_BUILDER_MARKET_SPEC_CACHE.lock() {
        cache.insert(market_slug.to_string(), (Instant::now(), spec));
    }
}

async fn resolve_trade_builder_market_spec(
    cfg: &AppConfig,
    market_slug: &str,
    token_id: &str,
) -> Option<TradeBuilderMarketSpec> {
    let candidates = trade_builder_market_spec_slug_candidates(market_slug);

    if !candidates.is_empty() {
        for candidate in &candidates {
            if let Some(spec) = trade_builder_market_spec_cache_get(candidate) {
                if !candidates.is_empty() {
                    trade_builder_market_spec_cache_put(&candidates[0], spec);
                }
                return Some(spec);
            }
        }

        let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
        for candidate in &candidates {
            let Ok(Some(market)) = gamma.get_market_spec_by_slug(candidate).await else {
                continue;
            };
            let spec = TradeBuilderMarketSpec {
                neg_risk: market.neg_risk,
                order_price_min_tick_size: normalize_trade_builder_market_spec_number(
                    market.order_price_min_tick_size,
                ),
                order_min_size: normalize_trade_builder_market_spec_number(market.order_min_size),
            };
            trade_builder_market_spec_cache_put(candidate, spec);
            trade_builder_market_spec_cache_put(&candidates[0], spec);
            return Some(spec);
        }
    }

    // Slug lookup failed (e.g. negRisk market with parent slug) — fallback to token_id lookup
    if !token_id.is_empty() {
        let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
        if let Ok(Some(market)) = gamma.get_market_spec_by_token_id(token_id).await {
            let spec = TradeBuilderMarketSpec {
                neg_risk: market.neg_risk,
                order_price_min_tick_size: normalize_trade_builder_market_spec_number(
                    market.order_price_min_tick_size,
                ),
                order_min_size: normalize_trade_builder_market_spec_number(market.order_min_size),
            };
            if !candidates.is_empty() {
                trade_builder_market_spec_cache_put(&candidates[0], spec);
            }
            return Some(spec);
        }
    }

    warn!(
        market_slug,
        token_id,
        candidates = ?candidates,
        "TRADE_BUILDER_MARKET_SPEC_UNRESOLVED"
    );
    None
}

fn trade_builder_guard_diagnostic_payload(
    configured: bool,
    decision: &str,
    reason_code: &str,
    details: Value,
) -> Value {
    json!({
        "configured": configured,
        "decision": decision,
        "reason_code": reason_code,
        "details": details,
    })
}

async fn append_trade_builder_guard_diagnostics_event(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    current_price: f64,
    desired_price: f64,
    best_ask: Option<f64>,
    trigger_price_guard: Value,
    execution_floor_guard: Value,
    max_price_guard: Value,
    effective_guard_scope: Option<&str>,
    effective_decision: &str,
    effective_reason_code: &str,
) -> Result<()> {
    repo.append_trade_builder_order_event(
        order.id,
        "guard_evaluated",
        &json!({
            "market_slug": &order.market_slug,
            "token_id": &order.token_id,
            "outcome_label": &order.outcome_label,
            "status_before": &order.status,
            "current_price": current_price,
            "desired_price": desired_price,
            "best_ask": best_ask,
            "trigger_price_guard": trigger_price_guard,
            "execution_floor_guard": execution_floor_guard,
            "max_price_guard": max_price_guard,
            "effective_guard_scope": effective_guard_scope,
            "effective_decision": effective_decision,
            "effective_reason_code": effective_reason_code,
        }),
    )
    .await
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderResolvedSellSubmitPrice {
    desired_price: f64,
    uncapped_desired_price: f64,
    source: &'static str,
    depth_levels_used: Option<usize>,
    visible_bid_qty: Option<f64>,
    requested_qty: Option<f64>,
}

fn normalize_trade_builder_visible_qty(value: Option<f64>) -> Option<f64> {
    let value = value?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    Some(round_trade_builder_share_qty(value))
}

fn trade_builder_order_book_visible_bid_qty(snapshot: &OrderBookSnapshot) -> Option<f64> {
    let visible_qty = snapshot
        .bids
        .iter()
        .map(|level| level.size)
        .filter(|size| size.is_finite() && *size > 0.0)
        .sum::<f64>();
    normalize_trade_builder_visible_qty(Some(visible_qty))
}

fn trade_builder_order_book_sweep_sell_price(
    snapshot: &OrderBookSnapshot,
    requested_qty: f64,
) -> Option<(f64, usize)> {
    let requested_qty = normalize_trade_builder_visible_qty(Some(requested_qty))?;
    let mut cumulative_qty = 0.0;
    let mut depth_levels_used = 0usize;

    for level in snapshot.bids.iter().rev() {
        if !level.price.is_finite() || !level.size.is_finite() || level.price <= 0.0 || level.size <= 0.0
        {
            continue;
        }
        cumulative_qty = round_trade_builder_share_qty(cumulative_qty + level.size);
        depth_levels_used += 1;
        if cumulative_qty + TRADE_BUILDER_EXIT_QTY_TOLERANCE >= requested_qty {
            return Some((clamp_probability(level.price), depth_levels_used));
        }
    }

    None
}

async fn resolve_trade_builder_sell_submit_price(
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
    current_price: f64,
    runtime_best_bid: Option<f64>,
    runtime_last_trade_price: Option<f64>,
    requested_qty: Option<f64>,
) -> TradeBuilderResolvedSellSubmitPrice {
    let order_book = client.order_book(&order.token_id).await.ok().flatten();
    resolve_trade_builder_sell_submit_price_with_book(
        order,
        current_price,
        runtime_best_bid,
        runtime_last_trade_price,
        requested_qty,
        order_book.as_ref(),
    )
}

fn resolve_trade_builder_sell_submit_price_with_book(
    order: &TradeBuilderOrder,
    current_price: f64,
    runtime_best_bid: Option<f64>,
    runtime_last_trade_price: Option<f64>,
    requested_qty: Option<f64>,
    order_book: Option<&OrderBookSnapshot>,
) -> TradeBuilderResolvedSellSubmitPrice {
    let requested_qty = normalize_trade_builder_visible_qty(requested_qty);
    let visible_bid_qty = order_book.and_then(trade_builder_order_book_visible_bid_qty);

    if let (Some(snapshot), Some(requested_qty)) = (order_book, requested_qty) {
        if let Some((sweep_price, depth_levels_used)) =
            trade_builder_order_book_sweep_sell_price(snapshot, requested_qty)
        {
            return TradeBuilderResolvedSellSubmitPrice {
                desired_price: trade_builder_cap_exit_sell_price(order, sweep_price),
                uncapped_desired_price: sweep_price,
                source: "orderbook_depth",
                depth_levels_used: Some(depth_levels_used),
                visible_bid_qty,
                requested_qty: Some(requested_qty),
            };
        }
    }

    let (uncapped_desired_price, source) = runtime_best_bid
        .and_then(|value| normalize_trade_builder_reference_price(Some(value)))
        .map(|value| (clamp_probability(value), "best_bid_fallback"))
        .or_else(|| {
            runtime_last_trade_price
                .and_then(|value| normalize_trade_builder_reference_price(Some(value)))
                .map(|value| (clamp_probability(value), "last_trade_fallback"))
        })
        .unwrap_or((clamp_probability(current_price), "price_fallback"));

    TradeBuilderResolvedSellSubmitPrice {
        desired_price: trade_builder_cap_exit_sell_price(order, uncapped_desired_price),
        uncapped_desired_price,
        source,
        depth_levels_used: None,
        visible_bid_qty,
        requested_qty,
    }
}

#[allow(clippy::too_many_arguments)]
async fn submit_trade_builder_trigger_order(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    order: &mut TradeBuilderOrder,
    current_price: f64,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
    fee_rate_bps: u64,
    resolved_size_usdc: f64,
    trigger_size_mode: Option<String>,
    trigger_size_value: Option<f64>,
    trigger_size_index: usize,
    submit_context: &TradeBuilderSubmitAttemptContext,
) -> Result<()> {
    let submit_started_at = Utc::now();
    let size_basis = normalize_trade_builder_size_basis(&order.size_basis);
    let submit_price_requested_qty = if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        trade_builder_share_request_qty(order)
    } else {
        None
    };
    let immediate_buy_execution_price =
        trade_builder_immediate_buy_notional_execution_price(order, current_price, best_ask);
    let (sell_submit_price, prefetched_available_qty) = prefetch_trade_builder_sell_submit_inputs(
        client,
        order,
        run_id,
        current_price,
        best_bid,
        last_trade_price,
        submit_price_requested_qty,
        size_basis,
    )
    .await;
    let desired_price = sell_submit_price
        .map(|resolution| resolution.desired_price)
        .or_else(|| immediate_buy_execution_price.map(|resolution| resolution.price))
        .unwrap_or_else(|| trade_builder_submit_desired_price(order, current_price));
    let uncapped_desired_price = sell_submit_price
        .map(|resolution| resolution.uncapped_desired_price)
        .or_else(|| immediate_buy_execution_price.map(|resolution| resolution.price))
        .unwrap_or_else(|| {
            aggressive_price_for_side(&order.side, current_price, order.min_price_distance_cent)
        });
    if immediate_buy_execution_price.is_none()
        && (desired_price - uncapped_desired_price).abs() >= 0.000001
    {
        repo.append_trade_builder_order_event(
            order.id,
            "exit_price_capped",
            &json!({
                "current_price": current_price,
                "uncapped_desired_price": uncapped_desired_price,
                "capped_desired_price": desired_price,
                "price_floor": trade_builder_exit_sell_price_floor(order),
                "trigger_price": order.trigger_price,
            }),
        )
        .await?;
    }

    let (remaining_usdc, remaining_qty, size, proposed_notional_usdc) =
        if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
            let qty = trade_builder_share_request_qty(order).ok_or_else(|| {
                anyhow::anyhow!("share-basis builder order requires target_qty or remaining_qty")
            })?;
            let remaining_usdc = (qty * desired_price).max(0.0);
            (Some(remaining_usdc), Some(qty), qty, remaining_usdc)
        } else {
            let remaining_usdc = order.remaining_size.unwrap_or(resolved_size_usdc);
            let size = calc_level_size(remaining_usdc, desired_price);
            (Some(remaining_usdc), None, size, resolved_size_usdc)
        };
    anyhow::ensure!(size > 0.0, "computed builder order size is zero");

    let trigger_price_guard_configured = order.side == "buy" && order.guard_trigger_price.is_some();
    let guard_eval_started = Instant::now();
    let (trigger_guard_reference_price, trigger_guard_reference_source) =
        trade_builder_resolve_trigger_guard_reference_price(order, current_price, best_ask);
    let trigger_price_guard_blocked =
        trigger_price_guard_configured
            && trade_builder_price_below_guard_trigger(order, trigger_guard_reference_price);
    let trigger_price_guard_payload = if trigger_price_guard_blocked {
        trade_builder_guard_diagnostic_payload(
            true,
            if order.retry_on_trigger_guard_block {
                "waiting"
            } else {
                "blocked"
            },
            "below_trigger_price_guard",
            json!({
                "guard_trigger_price": order.guard_trigger_price,
                "current_price": current_price,
                "trigger_guard_reference_price": trigger_guard_reference_price,
                "trigger_guard_reference_source": trigger_guard_reference_source,
            }),
        )
    } else if trigger_price_guard_configured {
        trade_builder_guard_diagnostic_payload(
            true,
            "passed",
            "passed",
            json!({
                "guard_trigger_price": order.guard_trigger_price,
                "current_price": current_price,
                "trigger_guard_reference_price": trigger_guard_reference_price,
                "trigger_guard_reference_source": trigger_guard_reference_source,
            }),
        )
    } else {
        trade_builder_guard_diagnostic_payload(false, "not_configured", "not_configured", Value::Null)
    };

    let execution_floor_reason = if trade_builder_execution_floor_missing_best_ask(order, best_ask) {
        Some("best_ask_unavailable")
    } else {
        trade_builder_execution_floor_block_reason(order, best_ask)
    };
    let execution_floor_configured = order.side == "buy" && order.best_ask_floor_price.is_some();
    let execution_floor_payload = if let Some(reason_code) = execution_floor_reason {
        trade_builder_guard_diagnostic_payload(
            true,
            if trade_builder_execution_floor_should_wait(order, reason_code) {
                "waiting"
            } else {
                "blocked"
            },
            reason_code,
            json!({
                "best_ask_floor_price": order.best_ask_floor_price,
                "best_ask": best_ask,
            }),
        )
    } else if execution_floor_configured {
        trade_builder_guard_diagnostic_payload(
            true,
            "passed",
            "passed",
            json!({
                "best_ask_floor_price": order.best_ask_floor_price,
                "best_ask": best_ask,
            }),
        )
    } else {
        trade_builder_guard_diagnostic_payload(false, "not_configured", "not_configured", Value::Null)
    };

    let max_price_configured = order.side == "buy" && order.max_price.is_some();
    let (max_price_reference, max_price_reference_source) =
        trade_builder_resolve_max_price_reference(order, best_ask, desired_price);
    let max_price_blocked =
        max_price_configured && trade_builder_price_exceeds_max_price(order, max_price_reference);
    let max_price_payload = if max_price_blocked {
        trade_builder_guard_diagnostic_payload(
            true,
            if order.retry_on_max_price_block {
                "waiting"
            } else {
                "blocked"
            },
            "above_max_price",
            json!({
                "max_price": order.max_price,
                "current_price": current_price,
                "desired_price": desired_price,
                "reference_price": max_price_reference,
                "reference_price_source": max_price_reference_source,
            }),
        )
    } else if max_price_configured {
        trade_builder_guard_diagnostic_payload(
            true,
            "passed",
            "passed",
            json!({
                "max_price": order.max_price,
                "current_price": current_price,
                "desired_price": desired_price,
                "reference_price": max_price_reference,
                "reference_price_source": max_price_reference_source,
            }),
        )
    } else {
        trade_builder_guard_diagnostic_payload(false, "not_configured", "not_configured", Value::Null)
    };

    if trigger_price_guard_blocked {
        let candidate_reason =
            build_guard_notification_reason("trigger_price", "below_trigger_price_guard");
        append_trade_builder_guard_diagnostics_event(
            repo,
            order,
            current_price,
            desired_price,
            best_ask,
            trigger_price_guard_payload.clone(),
            execution_floor_payload.clone(),
            max_price_payload.clone(),
            Some("trigger_price"),
            if order.retry_on_trigger_guard_block {
                "waiting"
            } else {
                "blocked"
            },
            "below_trigger_price_guard",
        )
        .await?;
        if order.retry_on_trigger_guard_block {
            let notification_message = order
                .notify_on_trigger_guard_blocked
                .then(|| {
                    build_trigger_guard_waiting_notification_message(
                        order,
                        trigger_guard_reference_price,
                        trigger_guard_reference_source,
                    )
                });
            transition_trade_builder_order_to_guard_waiting(
                repo,
                order,
                "below_trigger_price_guard",
                "trigger_price_waiting",
                &json!({
                    "reason_code": "below_trigger_price_guard",
                    "reason_message": "Current price is below the trigger price guard floor. Order is waiting for recovery.",
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "guard_trigger_price": order.guard_trigger_price,
                    "current_price": current_price,
                    "trigger_guard_reference_price": trigger_guard_reference_price,
                    "trigger_guard_reference_source": trigger_guard_reference_source,
                    "desired_price": desired_price,
                    "status_before": &order.status,
                    "status_after": TRADE_BUILDER_GUARD_BLOCKED_STATUS
                }),
                remaining_usdc,
                remaining_qty,
                Some(candidate_reason.as_str()),
                order
                    .notify_on_trigger_guard_blocked
                    .then_some("trigger_price_waiting"),
                notification_message,
            )
            .await?;
        } else {
            repo.set_trade_builder_order_status(
                order.id,
                "canceled",
                Some("below_trigger_price_guard"),
            )
            .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "trigger_price_blocked",
                &json!({
                    "reason_code": "below_trigger_price_guard",
                    "reason_message": "Current price is below the trigger price guard floor.",
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "guard_trigger_price": order.guard_trigger_price,
                    "current_price": current_price,
                    "trigger_guard_reference_price": trigger_guard_reference_price,
                    "trigger_guard_reference_source": trigger_guard_reference_source,
                    "desired_price": desired_price,
                    "status_before": &order.status,
                    "status_after": "canceled"
                }),
            )
            .await?;
            let message = build_trigger_guard_blocked_notification_message(
                order,
                trigger_guard_reference_price,
                trigger_guard_reference_source,
            );
            maybe_send_guard_transition_notification(
                repo,
                order,
                candidate_reason.as_str(),
                order.notify_on_trigger_guard_blocked,
                "trigger_price_blocked",
                &message,
            )
            .await?;
        }
        warn!(
            run_id,
            builder_order_id = order.id,
            market = %order.market_slug,
            token_id = %order.token_id,
            current_price,
            trigger_guard_reference_price,
            trigger_guard_reference_source,
            desired_price,
            guard_trigger_price = ?order.guard_trigger_price,
            waiting = order.retry_on_trigger_guard_block,
            reason_code = "below_trigger_price_guard",
            "TRADE_BUILDER_ORDER_TRIGGER_PRICE_BLOCKED"
        );
        return Ok(());
    }

    if let Some(reason_code) = execution_floor_reason {
        let candidate_reason = build_guard_notification_reason("execution_floor", reason_code);
        let should_wait = trade_builder_execution_floor_should_wait(order, reason_code);
        append_trade_builder_guard_diagnostics_event(
            repo,
            order,
            current_price,
            desired_price,
            best_ask,
            trigger_price_guard_payload.clone(),
            execution_floor_payload.clone(),
            max_price_payload.clone(),
            Some("execution_floor"),
            if should_wait { "waiting" } else { "blocked" },
            reason_code,
        )
        .await?;
        let reason_message = match reason_code {
            "best_ask_unavailable" => {
                "Best ask is unavailable, so the execution floor guard blocked the buy order."
            }
            "below_best_ask_floor" => "Best ask is below the configured execution floor.",
            _ => "Execution floor guard blocked the buy order.",
        };
        if should_wait {
            let notification_message = order
                .notify_on_execution_floor_blocked
                .then(|| build_execution_floor_waiting_notification_message(order, best_ask));
            transition_trade_builder_order_to_guard_waiting(
                repo,
                order,
                reason_code,
                "execution_floor_waiting",
                &json!({
                    "reason_code": reason_code,
                    "reason_message": "Execution floor guard moved the order into waiting mode.",
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "best_ask_floor_price": order.best_ask_floor_price,
                    "best_ask": best_ask,
                    "current_price": current_price,
                    "desired_price": desired_price,
                    "status_before": &order.status,
                    "status_after": TRADE_BUILDER_GUARD_BLOCKED_STATUS
                }),
                remaining_usdc,
                remaining_qty,
                Some(candidate_reason.as_str()),
                order
                    .notify_on_execution_floor_blocked
                    .then_some("execution_floor_waiting"),
                notification_message,
            )
            .await?;
        } else {
            repo.set_trade_builder_order_status(order.id, "canceled", Some(reason_code))
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "execution_floor_blocked",
                &json!({
                    "reason_code": reason_code,
                    "reason_message": reason_message,
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "best_ask_floor_price": order.best_ask_floor_price,
                    "best_ask": best_ask,
                    "current_price": current_price,
                    "desired_price": desired_price,
                    "status_before": &order.status,
                    "status_after": "canceled"
                }),
            )
            .await?;
            let message = build_execution_floor_blocked_notification_message(order, best_ask);
            maybe_send_guard_transition_notification(
                repo,
                order,
                candidate_reason.as_str(),
                order.notify_on_execution_floor_blocked,
                "execution_floor_blocked",
                &message,
            )
            .await?;
        }
        warn!(
            run_id,
            builder_order_id = order.id,
            market = %order.market_slug,
            token_id = %order.token_id,
            current_price,
            best_ask,
            desired_price,
            best_ask_floor_price = ?order.best_ask_floor_price,
            waiting = should_wait,
            reason_code,
            "TRADE_BUILDER_ORDER_EXECUTION_FLOOR_BLOCKED"
        );
        return Ok(());
    }

    if max_price_blocked {
        let candidate_reason = build_guard_notification_reason("max_price", "above_max_price");
        append_trade_builder_guard_diagnostics_event(
            repo,
            order,
            current_price,
            desired_price,
            best_ask,
            trigger_price_guard_payload,
            execution_floor_payload,
            max_price_payload,
            Some("max_price"),
            if order.retry_on_max_price_block {
                "waiting"
            } else {
                "blocked"
            },
            "above_max_price",
        )
        .await?;
        if order.retry_on_max_price_block {
            let notification_message = order.notify_on_max_price_blocked.then(|| {
                build_max_price_waiting_notification_message(
                    order,
                    current_price,
                    max_price_reference,
                    max_price_reference_source,
                )
            });
            transition_trade_builder_order_to_guard_waiting(
                repo,
                order,
                "above_max_price",
                "max_price_waiting",
                &json!({
                    "reason_code": "above_max_price",
                    "reason_message": "Max price guard moved the order into waiting mode.",
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "max_price": order.max_price,
                    "current_price": current_price,
                    "desired_price": desired_price,
                    "reference_price": max_price_reference,
                    "reference_price_source": max_price_reference_source,
                    "status_before": &order.status,
                    "status_after": TRADE_BUILDER_GUARD_BLOCKED_STATUS
                }),
                remaining_usdc,
                remaining_qty,
                Some(candidate_reason.as_str()),
                order
                    .notify_on_max_price_blocked
                    .then_some("max_price_waiting"),
                notification_message,
            )
            .await?;
        } else {
            repo.set_trade_builder_order_status(order.id, "canceled", Some("above_max_price"))
                .await?;
            repo.append_trade_builder_order_event(
                order.id,
                "max_price_blocked",
                &json!({
                    "reason_code": "above_max_price",
                    "reason_message": "Reference price would exceed the configured max price.",
                    "market_slug": &order.market_slug,
                    "token_id": &order.token_id,
                    "trigger_condition": order.trigger_condition.as_deref(),
                    "trigger_price": order.trigger_price,
                    "max_price": order.max_price,
                    "current_price": current_price,
                    "desired_price": desired_price,
                    "reference_price": max_price_reference,
                    "reference_price_source": max_price_reference_source,
                    "status_before": &order.status,
                    "status_after": "canceled"
                }),
            )
            .await?;
            if let Some((notification_type, message)) =
                build_max_price_blocked_notification(
                    order,
                    current_price,
                    max_price_reference,
                    max_price_reference_source,
                )
            {
                maybe_send_guard_transition_notification(
                    repo,
                    order,
                    candidate_reason.as_str(),
                    true,
                    notification_type,
                    &message,
                )
                .await?;
            }
        }
        warn!(
            run_id,
            builder_order_id = order.id,
            market = %order.market_slug,
            token_id = %order.token_id,
            current_price,
            desired_price,
            reference_price = max_price_reference,
            reference_price_source = max_price_reference_source,
            max_price = ?order.max_price,
            waiting = order.retry_on_max_price_block,
            reason_code = "above_max_price",
            "TRADE_BUILDER_ORDER_MAX_PRICE_BLOCKED"
        );
        return Ok(());
    }

    append_trade_builder_guard_diagnostics_event(
        repo,
        order,
        current_price,
        desired_price,
        best_ask,
        trigger_price_guard_payload,
        execution_floor_payload,
        max_price_payload,
        None,
        "passed",
        "guards_passed",
    )
    .await?;
    let guard_eval_ms = guard_eval_started.elapsed().as_millis() as i64;

    let risk = risk_gate_manual_order(
        repo,
        run_id,
        cfg,
        Some(order.user_id),
        order.trade_id,
        proposed_notional_usdc,
        limits,
        policy,
    )
    .await?;
    if !matches!(risk, RiskDecision::Allow) {
        repo.set_trade_builder_order_status(order.id, "blocked", Some("risk_block"))
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "blocked_by_risk",
            &json!({
                "reason_code": "risk_blocked",
                "reason_message": "Order blocked by risk policy.",
                "decision": format!("{risk:?}"),
                "market_slug": &order.market_slug,
                "token_id": &order.token_id,
                "trigger_condition": order.trigger_condition.as_deref(),
                "trigger_price": order.trigger_price,
                "current_price": current_price
            }),
        )
        .await?;
        warn!(
            run_id,
            builder_order_id = order.id,
            market = %order.market_slug,
            token_id = %order.token_id,
            reason_code = "risk_blocked",
            decision = %format!("{risk:?}"),
            current_price,
            "TRADE_BUILDER_ORDER_BLOCKED"
        );
        return Ok(());
    }

    let requested_share_qty = if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        Some(size)
    } else {
        None
    };
    let optimistic_exit_submit = size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
        && trade_builder_should_use_optimistic_exit_submit(order);
    let optimistic_exit_stage =
        optimistic_exit_submit.then(|| trade_builder_current_exit_submit_stage(order));
    let mut available_qty = prefetched_available_qty;
    let mut submit_partial_visible_inventory = false;
    let mut submit_size = size;
    let mut submit_remaining_usdc = remaining_usdc;
    let mut submit_remaining_qty = remaining_qty;

    if optimistic_exit_submit
        && optimistic_exit_stage == Some(TradeBuilderExitSubmitStage::DynamicGross)
    {
        if let Some(estimated) = trade_builder_estimated_visible_exit_qty(order, size) {
            if estimated.submit_qty < submit_size {
                submit_size = estimated.submit_qty;
                submit_remaining_qty = Some(estimated.submit_qty);
                submit_remaining_usdc = Some((estimated.submit_qty * desired_price).max(0.0));
                repo.append_trade_builder_order_event(
                    order.id,
                    "dynamic_gross_fee_adjusted",
                    &json!({
                        "submit_kind": "submit",
                        "original_qty": size,
                        "adjusted_qty": estimated.submit_qty,
                        "estimated_fee_qty": estimated.estimated_fee_qty,
                        "execution_price": estimated.execution_price,
                        "fee_rate_bps": estimated.fee_rate_bps,
                        "buffer_qty": trade_builder_exit_qty_buffer(order.target_qty.unwrap_or(size)),
                    }),
                )
                .await?;
            }
        }
    }

    if order.side == "sell"
        && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
        && !optimistic_exit_submit
    {
        if available_qty.is_none() {
            match client.available_token_qty(&order.token_id).await {
                Ok(quantity) => {
                    available_qty = quantity;
                }
                Err(err) => {
                    warn!(
                        run_id,
                        builder_order_id = order.id,
                        token_id = %order.token_id,
                        error = %err,
                        "TRADE_BUILDER_EXIT_INVENTORY_CHECK_FAILED"
                    );
                }
            }
        }
        let Some(inventory_resolution) =
            resolve_trade_builder_exit_inventory(order, size, available_qty)
        else {
            let reason = "exit inventory not yet available";
            mark_trade_builder_inventory_pending(
                repo,
                order,
                reason,
                current_price,
                size,
                available_qty,
            )
            .await?;
            return Ok(());
        };
        if let (Some(visible), Some(local_fallback_qty)) = (
            inventory_resolution.visible_qty,
            inventory_resolution.local_fallback_qty,
        ) {
            if (visible - local_fallback_qty).abs() >= 0.02 {
                repo.append_trade_builder_order_event(
                    order.id,
                    "inventory_source_mismatch",
                    &json!({
                        "visible_qty": visible,
                        "local_fallback_qty": local_fallback_qty,
                        "requested_qty": size
                    }),
                )
                .await?;
            }
        }
        if inventory_resolution.local_fallback_qty.is_some()
            && inventory_resolution.visible_qty.unwrap_or_default() <= 0.0
        {
            repo.append_trade_builder_order_event(
                order.id,
                "local_inventory_fallback_used",
                &json!({
                    "requested_qty": size,
                    "submit_qty": inventory_resolution.submit_qty,
                    "visible_qty": inventory_resolution.visible_qty,
                    "estimated_fee_qty": inventory_resolution.local_fallback_fee_qty,
                    "entry_price": inventory_resolution.local_fallback_entry_price,
                    "fee_rate_bps": inventory_resolution.local_fallback_fee_rate_bps
                }),
            )
            .await?;
        }
        submit_partial_visible_inventory = inventory_resolution.submit_partial_visible_inventory;
        submit_size = inventory_resolution.submit_qty;
        submit_remaining_qty = Some(inventory_resolution.submit_qty);
        submit_remaining_usdc = Some((inventory_resolution.submit_qty * desired_price).max(0.0));
    } else if order.side == "sell"
        && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
        && optimistic_exit_stage == Some(TradeBuilderExitSubmitStage::VisibleInventory)
    {
        if available_qty.is_none() {
            match client.available_token_qty(&order.token_id).await {
                Ok(quantity) => {
                    available_qty = quantity;
                }
                Err(err) => {
                    warn!(
                        run_id,
                        builder_order_id = order.id,
                        token_id = %order.token_id,
                        error = %err,
                        "TRADE_BUILDER_EXIT_INVENTORY_CHECK_FAILED"
                    );
                }
            }
        }
        let Some(visible_inventory_resolution) =
            resolve_trade_builder_visible_inventory_submit(size, available_qty)
        else {
            schedule_trade_builder_exit_sell_retry(
                repo,
                order,
                "submit_retry_scheduled",
                "exit inventory not yet available",
                current_price,
                desired_price,
                requested_share_qty,
                available_qty,
                Some(size),
                None,
                optimistic_exit_stage,
                optimistic_exit_stage,
            )
            .await?;
            return Ok(());
        };
        submit_partial_visible_inventory =
            visible_inventory_resolution.submit_partial_visible_inventory;
        submit_size = visible_inventory_resolution.submit_qty;
        submit_remaining_qty = Some(visible_inventory_resolution.submit_qty);
        submit_remaining_usdc =
            Some((visible_inventory_resolution.submit_qty * desired_price).max(0.0));
    }

    let intent = if order.kind == "immediate" {
        "manual_immediate"
    } else {
        "manual_trigger"
    };
    let normalized_execution_mode = normalize_trade_builder_execution_mode(&order.execution_mode);
    let order_type = clob_order_type_for_execution_mode(normalized_execution_mode);
    let client_order_id = format!("tb-{}", Uuid::new_v4());
    let market_spec = trade_builder_runtime_snapshot_from_order(order)
        .filter(|snapshot| trade_builder_runtime_snapshot_is_fresh(snapshot, submit_started_at))
        .and_then(|snapshot| trade_builder_market_spec_from_runtime_snapshot(&snapshot))
        .or(resolve_trade_builder_market_spec(cfg, &order.market_slug, &order.token_id).await);
    let req = PlaceOrderRequest {
        market: order.market_slug.clone(),
        token_id: Some(order.token_id.clone()),
        side: order.side.clone(),
        price: desired_price,
        size: submit_size,
        intent: intent.to_string(),
        order_type: order_type.to_string(),
        client_order_id: client_order_id.clone(),
        leg_side: None,
        fee_rate_bps,
        neg_risk: market_spec.is_some_and(|spec| spec.neg_risk),
    };

    maybe_record_trade_builder_buy_inventory_baseline(
        repo,
        run_id,
        client,
        order,
        desired_price,
        fee_rate_bps,
    )
    .await;
    if optimistic_exit_submit {
        repo.append_trade_builder_order_event(
            order.id,
            "optimistic_exit_submit_used",
            &json!({
                "submit_kind": "submit",
                "attempt_stage": optimistic_exit_stage.map(TradeBuilderExitSubmitStage::as_str),
                "status_before": &order.status,
                "requested_qty": requested_share_qty,
                "attempted_qty": submit_size,
                "current_price": current_price,
                "desired_price": desired_price,
                "size_basis": size_basis,
                "available_qty": available_qty,
                "precheck_skipped": optimistic_exit_stage != Some(TradeBuilderExitSubmitStage::VisibleInventory),
                "submit_price_source": sell_submit_price.map(|resolution| resolution.source),
                "submit_price_depth_levels_used": sell_submit_price.and_then(|resolution| resolution.depth_levels_used),
                "submit_price_visible_bid_qty": sell_submit_price.and_then(|resolution| resolution.visible_bid_qty),
                "submit_price_requested_qty": sell_submit_price.and_then(|resolution| resolution.requested_qty),
            }),
        )
        .await?;
    }

    let ack = match client.place(&req).await {
        Ok(ack) => ack,
        Err(err) => {
            let error_text = err.to_string();
            if trade_builder_error_is_fatal_exchange_rejection(&error_text) {
                repo.set_trade_builder_order_status(order.id, "error", Some(&error_text))
                    .await?;
                repo.append_trade_builder_order_event(
                    order.id,
                    "fatal_exchange_rejection",
                    &json!({
                        "error": error_text,
                        "status_before": &order.status,
                        "side": &order.side,
                        "market_slug": &order.market_slug,
                        "token_id": &order.token_id,
                        "attempted_qty": submit_size,
                        "desired_price": desired_price,
                        "neg_risk": req.neg_risk,
                        "order_price_min_tick_size": market_spec.and_then(|spec| spec.order_price_min_tick_size),
                        "order_min_size": market_spec.and_then(|spec| spec.order_min_size),
                        "submit_price_source": sell_submit_price.map(|resolution| resolution.source),
                        "submit_price_depth_levels_used": sell_submit_price.and_then(|resolution| resolution.depth_levels_used),
                        "submit_price_visible_bid_qty": sell_submit_price.and_then(|resolution| resolution.visible_bid_qty),
                        "submit_price_requested_qty": sell_submit_price.and_then(|resolution| resolution.requested_qty),
                    }),
                )
                .await?;
                warn!(
                    run_id,
                    builder_order_id = order.id,
                    market = %order.market_slug,
                    error = %error_text,
                    neg_risk = req.neg_risk,
                    "TRADE_BUILDER_FATAL_EXCHANGE_REJECTION"
                );
                maybe_send_trade_builder_system_alert(
                    repo,
                    order,
                    "fatal_exchange_rejection",
                    &error_text,
                )
                .await;
                return Ok(());
            }
            if order.side == "sell"
                && size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES
                && trade_builder_error_indicates_balance_or_allowance(&error_text)
            {
                if optimistic_exit_submit {
                    let current_attempt_stage =
                        optimistic_exit_stage.unwrap_or(TradeBuilderExitSubmitStage::DynamicGross);
                    let next_attempt_stage =
                        trade_builder_next_optimistic_exit_stage_after_balance_reject(
                            current_attempt_stage,
                        );
                    repo.append_trade_builder_order_event(
                        order.id,
                        "optimistic_exit_balance_rejected",
                        &json!({
                            "reason": error_text,
                            "attempt_stage": current_attempt_stage.as_str(),
                            "next_attempt_stage": next_attempt_stage.as_str(),
                            "status_before": &order.status,
                            "current_price": current_price,
                            "desired_price": desired_price,
                            "requested_qty": requested_share_qty,
                            "attempted_qty": submit_size,
                            "available_qty": available_qty,
                        }),
                    )
                    .await?;
                    schedule_trade_builder_exit_sell_retry(
                        repo,
                        order,
                        "submit_retry_scheduled",
                        &error_text,
                        current_price,
                        desired_price,
                        requested_share_qty,
                        available_qty,
                        Some(submit_size),
                        None,
                        Some(current_attempt_stage),
                        Some(next_attempt_stage),
                    )
                    .await?;
                    return Ok(());
                }
                if trade_builder_stop_loss_latched(order) {
                    schedule_trade_builder_exit_sell_retry(
                        repo,
                        order,
                        "submit_retry_scheduled",
                        &error_text,
                        current_price,
                        desired_price,
                        requested_share_qty,
                        available_qty,
                        Some(submit_size),
                        None,
                        None,
                        None,
                    )
                    .await?;
                    return Ok(());
                }
                let rechecked_qty = match client.available_token_qty(&order.token_id).await {
                    Ok(quantity) => quantity,
                    Err(recheck_err) => {
                        warn!(
                            run_id,
                            builder_order_id = order.id,
                            token_id = %order.token_id,
                            error = %recheck_err,
                            "TRADE_BUILDER_EXIT_INVENTORY_RECHECK_FAILED"
                        );
                        None
                    }
                };
                available_qty = rechecked_qty;
                if rechecked_qty
                    .and_then(|qty| clamp_trade_builder_visible_share_qty(size, Some(qty)))
                    .is_some()
                {
                    mark_trade_builder_inventory_pending(
                        repo,
                        order,
                        "exchange rejected sell before inventory synced",
                        current_price,
                        size,
                        rechecked_qty,
                    )
                    .await?;
                    return Ok(());
                }
            }
            if trade_builder_should_retry_exit_sell(order) {
                schedule_trade_builder_exit_sell_retry(
                    repo,
                    order,
                    "submit_retry_scheduled",
                    &error_text,
                    current_price,
                    desired_price,
                    requested_share_qty,
                    available_qty,
                    Some(submit_size),
                    None,
                    None,
                    None,
                )
                .await?;
                return Ok(());
            }
            return Err(err);
        }
    };
    let submit_finished_at = Utc::now();

    let exchange_order_id = ack
        .exchange_order_id
        .clone()
        .unwrap_or_else(|| ack.client_order_id.clone());
    let normalized_status = normalize_exchange_status(&ack.status);
    let mut raw = json!({
        "builder_order_id": order.id,
        "client_order_id": ack.client_order_id,
        "exchange_order_id": exchange_order_id,
        "status": ack.status,
        "normalized_status": normalized_status,
        "trigger_price": order.trigger_price,
        "max_price": order.max_price,
        "guard_trigger_price": order.guard_trigger_price,
        "best_ask_floor_price": order.best_ask_floor_price,
        "current_price": current_price,
        "best_ask": best_ask,
        "execution_price": desired_price,
        "execution_price_source": immediate_buy_execution_price
            .map(|resolution| resolution.source)
            .unwrap_or_else(|| sell_submit_price.map(|resolution| resolution.source).unwrap_or("runtime_price")),
        "trigger_reference_price": immediate_buy_execution_price
            .and_then(|resolution| resolution.trigger_reference_price),
        "submit_price_source": sell_submit_price.map(|resolution| resolution.source),
        "submit_price_depth_levels_used": sell_submit_price.and_then(|resolution| resolution.depth_levels_used),
        "submit_price_visible_bid_qty": sell_submit_price.and_then(|resolution| resolution.visible_bid_qty),
        "submit_price_requested_qty": sell_submit_price.and_then(|resolution| resolution.requested_qty),
        "execution_mode": normalized_execution_mode,
        "order_type": order_type,
        "size_basis": size_basis,
        "size": submit_size,
        "requested_qty": requested_share_qty,
        "clamped_qty": submit_remaining_qty,
        "partial_visible_inventory_submit": submit_partial_visible_inventory,
        "target_qty": order.target_qty,
        "remaining_qty": submit_remaining_qty,
        "size_mode": trigger_size_mode,
        "trigger_size_value": trigger_size_value,
        "trigger_size_index": trigger_size_index + 1,
        "resolved_size_usdc": resolved_size_usdc,
        "remaining_usdc": submit_remaining_usdc,
        "available_qty": available_qty,
        "fee_rate_bps": fee_rate_bps,
        "reject_reason": ack.reject_reason,
        "raw_status": ack.raw_status,
        "exchange_ts": ack.exchange_ts
    });
    append_trade_builder_submit_telemetry(
        raw.as_object_mut().expect("submitted payload"),
        submit_context,
        &TradeBuilderSubmitTiming {
            submit_started_at,
            submit_finished_at,
            guard_eval_ms,
        },
        Some(&ack),
    );

    repo.upsert_order_by_exchange_id(
        order.trade_id,
        &exchange_order_id,
        Some(&client_order_id),
        intent,
        &order.side,
        desired_price,
        submit_size,
        normalized_status,
        ack.exchange_ts,
        ack.reject_reason.as_deref(),
        &raw,
    )
    .await?;
    repo.set_trade_builder_order_working_state(
        order.id,
        Some(&exchange_order_id),
        Some(desired_price),
        submit_remaining_usdc,
        submit_remaining_qty,
        normalized_status,
    )
    .await?;
    maybe_persist_trade_builder_submitted_dynamic(repo, run_id, order, submit_size, desired_price)
        .await;
    if submit_partial_visible_inventory {
        repo.append_trade_builder_order_event(
            order.id,
            "partial_visible_inventory_submit",
            &json!({
                "requested_qty": requested_share_qty,
                "available_qty": available_qty,
                "submitted_qty": submit_size,
                "residual_qty_ignored": requested_share_qty.map(|qty| (qty - submit_size).max(0.0)),
            }),
        )
        .await?;
    }
    repo.append_trade_builder_order_event(order.id, "submitted", &raw)
        .await?;
    maybe_record_trade_builder_buy_submit_observation(
        repo,
        run_id,
        order,
        &exchange_order_id,
        submit_size,
        desired_price,
        fee_rate_bps,
        normalized_status,
        raw.clone(),
    )
    .await;

    if normalized_status == "filled" {
        let (
            canonical_entry_qty,
            canonical_entry_qty_source,
            actual_fill_qty,
            actual_fill_qty_source,
        ) = if trade_builder_should_track_buy_inventory_observation(order) {
            let (canonical_entry_qty, canonical_entry_qty_source) =
                trade_builder_canonical_entry_qty(order, Some(submit_size)).ok_or_else(|| {
                    anyhow::anyhow!(
                        "builder order canonical fill qty unresolved for exchange_order_id={exchange_order_id}"
                    )
                })?;
            (canonical_entry_qty, canonical_entry_qty_source, None, None)
        } else {
            (
                submit_size,
                "actual_fill_qty",
                Some(submit_size),
                Some("submitted_order_size"),
            )
        };
        finalize_builder_fill(
            repo,
            ws,
            order,
            &exchange_order_id,
            canonical_entry_qty,
            canonical_entry_qty_source,
            actual_fill_qty,
            desired_price,
            false,
            actual_fill_qty_source,
        )
        .await?;
    }

    Ok(())
}
