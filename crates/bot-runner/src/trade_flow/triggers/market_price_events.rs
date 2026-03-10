static TRADE_FLOW_REALTIME_PRICE_TICK_THROTTLE: LazyLock<StdMutex<HashMap<String, i64>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

fn should_emit_trade_flow_realtime_price_tick(throttle_key: &str, now_ms: i64) -> bool {
    let mut guard = TRADE_FLOW_REALTIME_PRICE_TICK_THROTTLE
        .lock()
        .expect("price tick throttle mutex poisoned");
    match guard.get(throttle_key).copied() {
        Some(last_ms) if now_ms.saturating_sub(last_ms) < 100 => false,
        _ => {
            guard.insert(throttle_key.to_string(), now_ms);
            true
        }
    }
}

async fn notify_trade_flow_realtime_price_tick(
    repo: &PostgresRepository,
    run_id: i64,
    definition_id: i64,
    version_id: i64,
    node_key: &str,
    node_type: &str,
    market_slug: Option<&str>,
    token_id: &str,
    outcome_label: &str,
    current_price: f64,
    price_mode: WsPriceMode,
    price_source: &str,
    price_source_detail: &str,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
    snapshot_age_ms: Option<i64>,
    event_ts: Option<i64>,
    evaluation_mode: Option<&str>,
) {
    let market_slug = market_slug.unwrap_or_default();
    let throttle_key = format!("{market_slug}:{token_id}");
    let created_at = Utc::now();
    let created_at_ms = created_at.timestamp_millis();
    if !should_emit_trade_flow_realtime_price_tick(&throttle_key, created_at_ms) {
        return;
    }

    let payload = json!({
        "kind": "price_tick",
        "run_id": run_id,
        "definition_id": definition_id,
        "version_id": version_id,
        "node_key": node_key,
        "node_type": node_type,
        "market_slug": if market_slug.is_empty() { Value::Null } else { json!(market_slug) },
        "token_id": token_id,
        "outcome_label": if outcome_label.trim().is_empty() { Value::Null } else { json!(outcome_label.trim()) },
        "price": current_price,
        "best_bid": best_bid,
        "best_ask": best_ask,
        "last_trade_price": last_trade_price,
        "price_mode": price_mode.as_str(),
        "price_source": price_source,
        "price_source_detail": price_source_detail,
        "evaluation_mode": evaluation_mode,
        "snapshot_age_ms": snapshot_age_ms,
        "event_ts": event_ts,
        "created_at": created_at.to_rfc3339(),
    });
    let _ = repo.notify_trade_flow_realtime(&payload).await;
}

async fn record_trigger_ws_price_ignored_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    reason: &str,
    price_mode: WsPriceMode,
    trigger_source: Option<&str>,
    market_slug: &str,
    ws_market_slug_from_step: Option<String>,
    ws_token_id_from_step: Option<String>,
    triggered_token_id: &str,
    ws_price_from_step: Option<f64>,
    ws_previous_price_from_step: Option<f64>,
    ws_price_mode_from_step: &str,
    ws_price_source_from_step: &str,
    effective_previous_price: Option<f64>,
) {
    if let Err(err) = repo
        .append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "trigger_ws_price_ignored",
            &json!({
                "node_key": node.key,
                "node_type": node.node_type,
                "reason": reason,
                "price_mode": price_mode.as_str(),
                "trigger_source": trigger_source,
                "market_slug": market_slug,
                "ws_market_slug": ws_market_slug_from_step,
                "ws_token_id": ws_token_id_from_step,
                "expected_token_id": if triggered_token_id.is_empty() { Value::Null } else { json!(triggered_token_id) },
                "ws_price": ws_price_from_step,
                "ws_previous_price": ws_previous_price_from_step,
                "ws_price_mode": ws_price_mode_from_step,
                "ws_price_source": ws_price_source_from_step,
                "effective_previous_price": effective_previous_price
            }),
        )
        .await
    {
        warn!(
            flow_run_id = run.id,
            node_key = %node.key,
            error = %err,
            "TRADE_FLOW_WS_IGNORE_EVENT_FAILED"
        );
    }
}

async fn record_trigger_once_blocked_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    reason: &str,
    once_scope_market: bool,
    market_slug: &str,
    trigger_source: Option<&str>,
    ws_sourced: Option<bool>,
    idempotency_key: Option<&str>,
) {
    if let Err(err) = repo
        .append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "trigger_once_blocked",
            &json!({
                "node_key": node.key,
                "node_type": node.node_type,
                "reason": reason,
                "idempotency_key": idempotency_key,
                "trigger_source": trigger_source,
                "market_slug": market_slug,
                "once_scope": if once_scope_market { "market" } else { "run" },
                "ws_sourced": ws_sourced,
            }),
        )
        .await
    {
        warn!(
            flow_run_id = run.id,
            node_key = %node.key,
            error = %err,
            "TRADE_FLOW_ONCE_BLOCK_EVENT_FAILED"
        );
    }
}

async fn record_trigger_ws_cross_confirmed_applied_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    market_slug: &str,
    price_mode: WsPriceMode,
    ws_market_slug_from_step: Option<String>,
    ws_token_id_from_step: Option<String>,
    ws_price_from_step: Option<f64>,
    ws_previous_price_from_step: Option<f64>,
    ws_evaluation_mode_from_step: &str,
    ws_price_mode_from_step: &str,
    ws_price_source_from_step: &str,
    once_mode: bool,
    once_scope_market: bool,
) {
    if let Err(err) = repo
        .append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "trigger_ws_cross_confirmed_applied",
            &json!({
                "node_key": node.key,
                "node_type": node.node_type,
                "market_slug": market_slug,
                "price_mode": price_mode.as_str(),
                "ws_market_slug": ws_market_slug_from_step,
                "ws_token_id": ws_token_id_from_step,
                "ws_price": ws_price_from_step,
                "ws_previous_price": ws_previous_price_from_step,
                "ws_evaluation_mode": ws_evaluation_mode_from_step,
                "ws_price_mode": ws_price_mode_from_step,
                "ws_price_source": ws_price_source_from_step,
                "once_mode": once_mode,
                "once_scope": if once_scope_market { "market" } else { "run" }
            }),
        )
        .await
    {
        warn!(
            flow_run_id = run.id,
            node_key = %node.key,
            error = %err,
            "TRADE_FLOW_WS_CROSS_CONFIRMED_APPLIED_EVENT_FAILED"
        );
    }
}

async fn record_trigger_ws_cross_confirmed_unexpected_fail_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    market_slug: &str,
    price_mode: WsPriceMode,
    ws_market_slug_from_step: Option<String>,
    ws_token_id_from_step: Option<String>,
    ws_price_from_step: Option<f64>,
    ws_previous_price_from_step: Option<f64>,
    ws_evaluation_mode_from_step: &str,
    ws_price_mode_from_step: &str,
    ws_price_source_from_step: &str,
    trigger_evaluation_mode: &str,
    ws_ignore_reason: Option<String>,
    effective_previous_price: Option<f64>,
    ws_cross_confirmed_short_circuit_applied: bool,
) {
    if let Err(err) = repo
        .append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "trigger_ws_cross_confirmed_unexpected_fail",
            &json!({
                "node_key": node.key,
                "node_type": node.node_type,
                "market_slug": market_slug,
                "price_mode": price_mode.as_str(),
                "ws_market_slug": ws_market_slug_from_step,
                "ws_token_id": ws_token_id_from_step,
                "ws_price": ws_price_from_step,
                "ws_previous_price": ws_previous_price_from_step,
                "ws_evaluation_mode": ws_evaluation_mode_from_step,
                "ws_price_mode": ws_price_mode_from_step,
                "ws_price_source": ws_price_source_from_step,
                "evaluation_mode": trigger_evaluation_mode,
                "ws_ignored_reason": ws_ignore_reason,
                "effective_previous_price": effective_previous_price,
                "short_circuit_applied": ws_cross_confirmed_short_circuit_applied
            }),
        )
        .await
    {
        warn!(
            flow_run_id = run.id,
            node_key = %node.key,
            error = %err,
            "TRADE_FLOW_WS_CROSS_CONFIRMED_UNEXPECTED_FAIL_EVENT_FAILED"
        );
    }
}

async fn record_trigger_protection_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    protection_passed: bool,
    market_slug: &str,
    triggered_token_id: &str,
    triggered_outcome_label: &str,
    triggered_condition: &str,
    triggered_price: Option<f64>,
    triggered_max_price: Option<f64>,
    triggered_poly_delta_10s_cent: Option<f64>,
    protection_output: &Value,
) {
    let event_type = if protection_passed {
        "trigger_protection_passed"
    } else {
        "trigger_protection_blocked"
    };
    if let Err(err) = repo
        .append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            event_type,
            &json!({
                "node_key": node.key,
                "node_type": node.node_type,
                "market_slug": market_slug,
                "triggered_token_id": if triggered_token_id.is_empty() { Value::Null } else { json!(triggered_token_id) },
                "triggered_outcome_label": if triggered_outcome_label.is_empty() { Value::Null } else { json!(triggered_outcome_label) },
                "triggered_condition": if triggered_condition.is_empty() { Value::Null } else { json!(triggered_condition) },
                "triggered_price": triggered_price,
                "max_price": triggered_max_price,
                "poly_delta_10s_cent": triggered_poly_delta_10s_cent,
                "protection": protection_output
            }),
        )
        .await
    {
        warn!(
            flow_run_id = run.id,
            node_key = %node.key,
            error = %err,
            "TRADE_FLOW_TRIGGER_PROTECTION_EVENT_FAILED"
        );
    }
}

async fn record_trigger_once_fired_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    market_slug: &str,
    price_mode: WsPriceMode,
    triggered_token_id: &str,
    triggered_outcome_label: &str,
    triggered_condition: &str,
    triggered_price: Option<f64>,
    triggered_max_price: Option<f64>,
    triggered_previous_price: Option<f64>,
    triggered_poly_delta_10s_cent: Option<f64>,
    protection_output: &Value,
    trigger_evaluation_mode: &str,
    current_price: Option<f64>,
    ws_sourced: bool,
    ws_price_mode_from_step: &str,
    ws_price_source_from_step: &str,
    once_scope_market: bool,
    fired_at: DateTime<Utc>,
    once_fire_key: &str,
    cycle_window_diagnostics: Option<&Value>,
) {
    let mut payload = json!({
        "node_key": node.key,
        "node_type": node.node_type,
        "market_slug": market_slug,
        "price_mode": price_mode.as_str(),
        "triggered_token_id": triggered_token_id,
        "triggered_outcome_label": triggered_outcome_label,
        "triggered_condition": triggered_condition,
        "triggered_price": triggered_price,
        "max_price": triggered_max_price,
        "previous_price": triggered_previous_price,
        "poly_delta_10s_cent": triggered_poly_delta_10s_cent,
        "protection": protection_output,
        "evaluation_mode": trigger_evaluation_mode,
        "price": current_price,
        "ws_sourced": ws_sourced,
        "ws_price_mode": ws_price_mode_from_step,
        "ws_price_source": ws_price_source_from_step,
        "once_scope": if once_scope_market { "market" } else { "run" },
        "fired_at": fired_at,
        "idempotency_key": once_fire_key
    });
    if let Some(diagnostics) = cycle_window_diagnostics {
        append_json_object_fields(&mut payload, diagnostics);
    }
    if let Err(err) = repo
        .append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "trigger_once_fired",
            &payload,
        )
        .await
    {
        warn!(
            flow_run_id = run.id,
            node_key = %node.key,
            error = %err,
            "TRADE_FLOW_ONCE_FIRED_EVENT_FAILED"
        );
    }
}

fn apply_trigger_market_price_context_updates(
    context: &mut Value,
    node: &TradeFlowNode,
    var_key: &str,
    current_price: Option<f64>,
    triggered_token_id: &str,
    triggered_outcome_label: &str,
    triggered_condition: &str,
    triggered_price: Option<f64>,
    triggered_max_price: Option<f64>,
    pass: bool,
) {
    if let Some(price) = current_price {
        set_flow_node_state(context, &node.key, "last_price", json!(price));
        set_flow_var(context, &format!("{var_key}_price"), json!(price));
    }
    if !triggered_token_id.is_empty() {
        set_flow_var(
            context,
            &format!("{var_key}_token_id"),
            json!(triggered_token_id),
        );
        if pass {
            set_flow_context(context, "tokenId", json!(triggered_token_id));
        }
    }
    if !triggered_outcome_label.is_empty() {
        set_flow_var(
            context,
            &format!("{var_key}_outcome_label"),
            json!(triggered_outcome_label),
        );
        if pass {
            set_flow_context(context, "outcomeLabel", json!(triggered_outcome_label));
        }
    }
    if !triggered_condition.is_empty() {
        set_flow_var(
            context,
            &format!("{var_key}_triggered_condition"),
            json!(triggered_condition),
        );
    }
    if let Some(tp) = triggered_price {
        set_flow_var(context, &format!("{var_key}_triggered_price"), json!(tp));
    }
    if let Some(max_price) = triggered_max_price {
        set_flow_var(context, &format!("{var_key}_max_price"), json!(max_price));
    }
    if pass {
        if let Some(max_price) = triggered_max_price {
            set_flow_context(context, "maxPrice", json!(max_price));
        } else {
            set_flow_context(context, "maxPrice", Value::Null);
        }
    }
}

fn build_trigger_market_price_output(
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &Value,
    market_slug: &str,
    price_mode: WsPriceMode,
    triggered_token_id: &str,
    triggered_outcome_label: &str,
    triggered_condition: &str,
    triggered_price: Option<f64>,
    triggered_max_price: Option<f64>,
    protection_output: &Value,
    triggered_previous_price: Option<f64>,
    ws_previous_price_from_step: Option<f64>,
    effective_previous_price: Option<f64>,
    trigger_evaluation_mode: &str,
    ws_evaluation_mode_from_step: &str,
    ws_price_mode_from_step: &str,
    ws_price_source_from_step: &str,
    ws_price_source_detail_from_step: &str,
    ws_best_bid_from_step: Option<f64>,
    ws_best_ask_from_step: Option<f64>,
    ws_last_trade_price_from_step: Option<f64>,
    ws_snapshot_age_ms_from_step: Option<i64>,
    ws_site_display_mode_decision_from_step: &str,
    ws_cross_confirmed_short_circuit_applied: bool,
    current_price: Option<f64>,
    pass: bool,
    var_key: &str,
    outcome_conditions: &Option<Vec<Value>>,
    ws_sourced: bool,
    ws_ignore_reason: Option<String>,
    once_mode: bool,
    once_scope_market: bool,
    queued_at_from_step: Option<&str>,
) -> Value {
    json!({
        "run_id": run.id,
        "node_key": node.key,
        "market_slug": market_slug,
        "price_mode": price_mode.as_str(),
        "triggered_token_id": triggered_token_id,
        "triggered_outcome_label": triggered_outcome_label,
        "triggered_condition": triggered_condition,
        "triggered_price": triggered_price,
        "max_price": triggered_max_price,
        "maxPrice": triggered_max_price,
        "poly_delta_10s_cent": Value::Null,
        "protection": protection_output,
        "previous_price": triggered_previous_price,
        "ws_previous_price": ws_previous_price_from_step,
        "effective_previous_price": effective_previous_price,
        "evaluation_mode": trigger_evaluation_mode,
        "ws_evaluation_mode_from_step": ws_evaluation_mode_from_step,
        "ws_price_mode_from_step": ws_price_mode_from_step,
        "ws_price_source_from_step": ws_price_source_from_step,
        "ws_price_source_detail_from_step": ws_price_source_detail_from_step,
        "ws_best_bid_from_step": ws_best_bid_from_step,
        "ws_best_ask_from_step": ws_best_ask_from_step,
        "ws_last_trade_price_from_step": ws_last_trade_price_from_step,
        "ws_snapshot_age_ms_from_step": ws_snapshot_age_ms_from_step,
        "ws_site_display_mode_decision_from_step": ws_site_display_mode_decision_from_step,
        "cross_confirmed_short_circuit_applied": ws_cross_confirmed_short_circuit_applied,
        "price": current_price,
        "pass": pass,
        "var_key": var_key,
        "multi_outcome": outcome_conditions.is_some(),
        "ws_sourced": ws_sourced,
        "ws_ignored_reason": ws_ignore_reason,
        "once_mode": once_mode,
        "once_scope": if once_scope_market { "market" } else { "run" },
        "queued_at": queued_at_from_step,
        "once_fired": trade_flow_market_price_once_fired_for_scope(
            context,
            &node.key,
            once_scope_market,
            Some(market_slug)
        )
    })
}

fn finish_trigger_market_price_execution(
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &Value,
    market_slug: &str,
    price_mode: WsPriceMode,
    triggered_token_id: &str,
    triggered_outcome_label: &str,
    triggered_condition: &str,
    triggered_price: Option<f64>,
    triggered_max_price: Option<f64>,
    protection_output: &Value,
    triggered_previous_price: Option<f64>,
    ws_previous_price_from_step: Option<f64>,
    effective_previous_price: Option<f64>,
    trigger_evaluation_mode: &str,
    ws_evaluation_mode_from_step: &str,
    ws_price_mode_from_step: &str,
    ws_price_source_from_step: &str,
    ws_price_source_detail_from_step: &str,
    ws_best_bid_from_step: Option<f64>,
    ws_best_ask_from_step: Option<f64>,
    ws_last_trade_price_from_step: Option<f64>,
    ws_snapshot_age_ms_from_step: Option<i64>,
    ws_site_display_mode_decision_from_step: &str,
    ws_cross_confirmed_short_circuit_applied: bool,
    current_price: Option<f64>,
    pass: bool,
    var_key: &str,
    outcome_conditions: &Option<Vec<Value>>,
    ws_sourced: bool,
    ws_ignore_reason: Option<String>,
    once_mode: bool,
    once_scope_market: bool,
    queued_at_from_step: Option<&str>,
    interval_ms: i64,
) -> TradeFlowNodeExecution {
    let repeat_at = if ws_sourced {
        None
    } else if once_mode {
        None
    } else {
        Some(Utc::now() + ChronoDuration::milliseconds(interval_ms))
    };
    let routes = if pass {
        vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at: Utc::now(),
        }]
    } else {
        Vec::new()
    };
    let output = build_trigger_market_price_output(
        run,
        node,
        context,
        market_slug,
        price_mode,
        triggered_token_id,
        triggered_outcome_label,
        triggered_condition,
        triggered_price,
        triggered_max_price,
        protection_output,
        triggered_previous_price,
        ws_previous_price_from_step,
        effective_previous_price,
        trigger_evaluation_mode,
        ws_evaluation_mode_from_step,
        ws_price_mode_from_step,
        ws_price_source_from_step,
        ws_price_source_detail_from_step,
        ws_best_bid_from_step,
        ws_best_ask_from_step,
        ws_last_trade_price_from_step,
        ws_snapshot_age_ms_from_step,
        ws_site_display_mode_decision_from_step,
        ws_cross_confirmed_short_circuit_applied,
        current_price,
        pass,
        var_key,
        outcome_conditions,
        ws_sourced,
        ws_ignore_reason,
        once_mode,
        once_scope_market,
        queued_at_from_step,
    );
    info!(
        flow_run_id = run.id,
        node_key = %node.key,
        pass,
        trigger_evaluation_mode,
        ?current_price,
        ?effective_previous_price,
        price_mode = price_mode.as_str(),
        once_mode,
        ws_sourced,
        ws_price_source = ws_price_source_from_step,
        ws_price_source_detail = ws_price_source_detail_from_step,
        ws_best_bid = ?ws_best_bid_from_step,
        ws_best_ask = ?ws_best_ask_from_step,
        ws_last_trade_price = ?ws_last_trade_price_from_step,
        ws_snapshot_age_ms = ?ws_snapshot_age_ms_from_step,
        ws_site_display_mode_decision = ws_site_display_mode_decision_from_step,
        routes_count = routes.len(),
        "TRIGGER_MARKET_PRICE_EVALUATED"
    );
    TradeFlowNodeExecution {
        output,
        routes,
        repeat_at,
        repeat_idempotency_key: None,
    }
}
