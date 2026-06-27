const TRADE_BUILDER_PRE_SUBMIT_BOOK_RECHECK_WS_STALE_MS: i64 = 500;

#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderPreSubmitBestAsk {
    best_ask: Option<f64>,
    source: &'static str,
}

fn trade_builder_pre_submit_recheck_eligible(
    order: &TradeBuilderOrder,
    order_type: &str,
) -> bool {
    order.side == "buy"
        && normalize_trade_builder_execution_mode(&order.execution_mode) == "market"
        && order_type.eq_ignore_ascii_case("FAK")
}

fn trade_builder_pre_submit_normalize_best_ask(best_ask: Option<f64>) -> Option<f64> {
    best_ask
        .and_then(|value| normalize_trade_builder_reference_price(Some(value)))
        .map(clamp_probability)
}

fn trade_builder_pre_submit_book_best_ask(book: Option<&OrderBookSnapshot>) -> Option<f64> {
    book.and_then(live_gap_collector_best_ask)
        .and_then(|value| trade_builder_pre_submit_normalize_best_ask(Some(value)))
}

async fn trade_builder_pre_submit_fresh_best_ask(
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    token_id: &str,
) -> TradeBuilderPreSubmitBestAsk {
    match client.order_book(token_id).await {
        Ok(Some(book)) => {
            let best_ask = trade_builder_pre_submit_book_best_ask(Some(&book));
            if best_ask.is_some() {
                return TradeBuilderPreSubmitBestAsk {
                    best_ask,
                    source: "rest_order_book",
                };
            }
        }
        Ok(None) | Err(_) => {}
    }

    let inspection = ws
        .inspect_market_snapshot(token_id, TRADE_BUILDER_PRE_SUBMIT_BOOK_RECHECK_WS_STALE_MS)
        .await;
    if inspection.state == MarketSnapshotWsState::Seeded {
        let best_ask = inspection
            .snapshot
            .as_ref()
            .and_then(|snapshot| trade_builder_pre_submit_normalize_best_ask(snapshot.best_ask));
        if best_ask.is_some() {
            return TradeBuilderPreSubmitBestAsk {
                best_ask,
                source: "fresh_ws_snapshot",
            };
        }
    }

    TradeBuilderPreSubmitBestAsk {
        best_ask: None,
        source: inspection.state.as_str(),
    }
}

fn trade_builder_pre_submit_notional_size(
    size_basis: &str,
    desired_price: f64,
    submit_size: f64,
    submit_remaining_usdc: Option<f64>,
    submit_remaining_qty: Option<f64>,
) -> (f64, Option<f64>, Option<f64>) {
    if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        let remaining_usdc = submit_remaining_qty
            .map(|qty| (qty * desired_price).max(0.0))
            .or(submit_remaining_usdc);
        return (submit_size, remaining_usdc, submit_remaining_qty);
    }

    let remaining_usdc = submit_remaining_usdc.unwrap_or(0.0);
    (
        calc_level_size(remaining_usdc, desired_price),
        Some(remaining_usdc),
        None,
    )
}

fn trade_builder_pre_submit_missing_best_ask_payload(
    fresh_source: &str,
    previous_best_ask: Option<f64>,
    previous_desired_price: f64,
) -> Value {
    trade_builder_guard_diagnostic_payload(
        true,
        "waiting",
        "submit_best_ask_unavailable",
        json!({
            "best_ask": Value::Null,
            "fresh_best_ask_source": fresh_source,
            "previous_best_ask": previous_best_ask,
            "previous_desired_price": previous_desired_price,
        }),
    )
}

fn trade_builder_pre_submit_event_payload(
    order: &TradeBuilderOrder,
    current_price: f64,
    previous_best_ask: Option<f64>,
    previous_desired_price: f64,
    fresh: TradeBuilderPreSubmitBestAsk,
    desired_price: f64,
    submit_size: f64,
    trigger_price_guard: &Value,
    execution_floor_guard: &Value,
    max_price_guard: &Value,
    effective_scope: Option<&str>,
    effective_decision: &str,
    effective_reason_code: &str,
) -> Value {
    json!({
        "market_slug": &order.market_slug,
        "token_id": &order.token_id,
        "outcome_label": &order.outcome_label,
        "status_before": &order.status,
        "previous_best_ask": previous_best_ask,
        "fresh_best_ask": fresh.best_ask,
        "fresh_best_ask_source": fresh.source,
        "previous_desired_price": previous_desired_price,
        "desired_price": desired_price,
        "submit_size": submit_size,
        "current_price": current_price,
        "price": fresh.best_ask,
        "best_ask": fresh.best_ask,
        "reference_price": fresh.best_ask,
        "reference_price_source": fresh.source,
        "trigger_guard_reference_price": fresh.best_ask,
        "trigger_guard_reference_source": fresh.source,
        "trigger_price": order.trigger_price,
        "guard_trigger_price": order.guard_trigger_price,
        "best_ask_floor_price": order.best_ask_floor_price,
        "max_price": order.max_price,
        "trigger_price_guard": trigger_price_guard,
        "execution_floor_guard": execution_floor_guard,
        "max_price_guard": max_price_guard,
        "effective_guard_scope": effective_scope,
        "effective_decision": effective_decision,
        "effective_reason_code": effective_reason_code,
    })
}

fn trade_builder_pre_submit_waiting_event_type(scope: Option<&str>) -> &'static str {
    match scope {
        Some("max_price") => "max_price_waiting",
        Some("trigger_price") => "trigger_price_waiting",
        Some("execution_floor") => "execution_floor_waiting",
        _ => "execution_floor_waiting",
    }
}

fn trade_builder_pre_submit_guard_reason(scope: Option<&str>, reason_code: &str) -> String {
    build_guard_notification_reason(scope.unwrap_or("execution_floor"), reason_code)
}

#[allow(clippy::too_many_arguments)]
async fn maybe_apply_trade_builder_pre_submit_book_recheck(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    order: &mut TradeBuilderOrder,
    current_price: f64,
    previous_best_ask: Option<f64>,
    order_type: &str,
    size_basis: &str,
    retry_on_trigger_guard_block: bool,
    retry_on_execution_floor_guard_block: bool,
    retry_on_max_price_block: bool,
    desired_price: &mut f64,
    submit_size: &mut f64,
    submit_remaining_usdc: &mut Option<f64>,
    submit_remaining_qty: &mut Option<f64>,
    deferred_submit_events: &mut DeferredTradeBuilderSubmitEvents,
) -> Result<bool> {
    if !trade_builder_pre_submit_recheck_eligible(order, order_type) {
        return Ok(false);
    }

    let previous_desired_price = *desired_price;
    let fresh = trade_builder_pre_submit_fresh_best_ask(client, ws, &order.token_id).await;
    let market_buy_execution_price =
        trade_builder_market_buy_execution_price(order, current_price, fresh.best_ask);
    let fresh_desired_price = market_buy_execution_price
        .map(|resolution| resolution.price)
        .unwrap_or_else(|| trade_builder_submit_desired_price(order, current_price));
    let (fresh_size, fresh_remaining_usdc, fresh_remaining_qty) =
        trade_builder_pre_submit_notional_size(
            size_basis,
            fresh_desired_price,
            *submit_size,
            *submit_remaining_usdc,
            *submit_remaining_qty,
        );

    let guard_eval = evaluate_trade_builder_buy_guards(
        &order.execution_mode,
        order.pair_leg_role.as_deref(),
        current_price,
        fresh.best_ask,
        fresh_desired_price,
        order.guard_trigger_price,
        order.max_price,
        order.best_ask_floor_price,
        retry_on_trigger_guard_block,
        retry_on_execution_floor_guard_block,
        retry_on_max_price_block,
    );
    let (effective_scope, effective_decision, effective_reason_code, execution_floor_payload) =
        if fresh.best_ask.is_none() {
            (
                Some("execution_floor"),
                "waiting",
                "submit_best_ask_unavailable",
                trade_builder_pre_submit_missing_best_ask_payload(
                    fresh.source,
                    previous_best_ask,
                    previous_desired_price,
                ),
            )
        } else {
            (
                match guard_eval.effective_decision {
                    "passed" => None,
                    _ => {
                        if guard_eval.trigger_price_guard_blocked {
                            Some("trigger_price")
                        } else if guard_eval.execution_floor_reason.is_some() {
                            Some("execution_floor")
                        } else if guard_eval.max_price_blocked {
                            Some("max_price")
                        } else {
                            Some("execution_floor")
                        }
                    }
                },
                guard_eval.effective_decision,
                guard_eval.effective_reason_code,
                guard_eval.execution_floor_payload.clone(),
            )
        };
    let event_payload = trade_builder_pre_submit_event_payload(
        order,
        current_price,
        previous_best_ask,
        previous_desired_price,
        fresh,
        fresh_desired_price,
        fresh_size,
        &guard_eval.trigger_price_guard_payload,
        &execution_floor_payload,
        &guard_eval.max_price_payload,
        effective_scope,
        effective_decision,
        effective_reason_code,
    );

    *desired_price = fresh_desired_price;
    *submit_size = fresh_size;
    *submit_remaining_usdc = fresh_remaining_usdc;
    *submit_remaining_qty = fresh_remaining_qty;

    if effective_decision == "passed" {
        deferred_submit_events.defer_order_event("pre_submit_book_recheck", event_payload);
        return Ok(false);
    }

    deferred_submit_events.flush(repo, order).await?;
    repo.append_trade_builder_order_event(order.id, "pre_submit_book_recheck", &event_payload)
        .await?;
    let candidate_reason =
        trade_builder_pre_submit_guard_reason(effective_scope, effective_reason_code);
    let notification_type = match effective_scope {
        Some("max_price") if order.notify_on_max_price_blocked => Some("max_price_waiting"),
        Some("trigger_price") if order.notify_on_trigger_guard_blocked => {
            Some("trigger_price_waiting")
        }
        Some("execution_floor") if order.notify_on_execution_floor_blocked => {
            Some("execution_floor_waiting")
        }
        _ => None,
    };
    let notification_message = match effective_scope {
        Some("max_price") if order.notify_on_max_price_blocked => Some(
            build_max_price_waiting_notification_message(
                order,
                current_price,
                guard_eval.max_price_reference,
                guard_eval.max_price_reference_source,
                Some(effective_reason_code),
            ),
        ),
        Some("trigger_price") if order.notify_on_trigger_guard_blocked => Some(
            build_trigger_guard_waiting_notification_message(
                order,
                guard_eval.trigger_guard_reference_price,
                guard_eval.trigger_guard_reference_source,
            ),
        ),
        Some("execution_floor") if order.notify_on_execution_floor_blocked => {
            Some(build_execution_floor_waiting_notification_message(order, fresh.best_ask))
        }
        _ => None,
    };
    transition_trade_builder_order_to_guard_waiting(
        repo,
        order,
        effective_reason_code,
        trade_builder_pre_submit_waiting_event_type(effective_scope),
        &event_payload,
        *submit_remaining_usdc,
        *submit_remaining_qty,
        Some(candidate_reason.as_str()),
        notification_type,
        notification_message,
    )
    .await?;
    Ok(true)
}
