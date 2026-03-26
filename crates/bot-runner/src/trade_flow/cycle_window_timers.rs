#[derive(Debug, Clone)]
struct DueCycleWindowBoundaryTarget {
    run_index: usize,
    node_index: usize,
    window_mode: String,
    boundary_marker: String,
}

const FLOW_NODE_STATE_CYCLE_WINDOW_LAST_EVAL_PREFIX: &str = "cycle_window_last_eval_";

#[derive(Debug, Clone)]
struct DueWindowEndSellTarget {
    run_index: usize,
    node_index: usize,
    window_end_at: DateTime<Utc>,
}

fn cycle_window_end_sell_state_key(node_spec: &WsOpenPositionPriceNodeSpec) -> String {
    format!(
        "{}{}",
        FLOW_NODE_STATE_CYCLE_WINDOW_END_SELL_MARKER_PREFIX, node_spec.token_id
    )
}

fn cycle_window_end_auto_sell_builder_order_id(
    output: &Value,
    expected_market_slug: Option<&str>,
) -> Option<i64> {
    let output_market_slug = output
        .get("market_slug")
        .and_then(Value::as_str)
        .or_else(|| output.get("marketSlug").and_then(Value::as_str));
    if let Some(expected_market_slug) = expected_market_slug {
        if output_market_slug != Some(expected_market_slug) {
            return None;
        }
    }

    output
        .get("builderOrderId")
        .and_then(Value::as_i64)
        .or_else(|| output.get("builder_order_id").and_then(Value::as_i64))
}

fn build_cycle_window_end_auto_sell_input(
    node_spec: &WsOpenPositionPriceNodeSpec,
    parent_order: &TradeBuilderOrder,
) -> Value {
    json!({
        "side": "sell",
        "marketSlug": parent_order.market_slug,
        "tokenId": parent_order.token_id,
        "outcomeLabel": parent_order.outcome_label,
        "sourceTradeId": parent_order.trade_id,
        "parentBuilderOrderId": parent_order.id,
        "executionMode": "market",
        "windowEndAutoSell": true,
        "windowEndTriggerNodeKey": node_spec.node_key,
    })
}

fn cycle_window_end_sell_due_target(
    run_spec: &WsOpenPositionPriceRunSpec,
    run_index: usize,
    node_spec: &WsOpenPositionPriceNodeSpec,
    node_index: usize,
    now: DateTime<Utc>,
) -> Option<DueWindowEndSellTarget> {
    if !node_spec.auto_sell_on_window_end {
        return None;
    }
    let (_, window_end_at) = cycle_window_bounds(node_spec)?;
    if now < window_end_at {
        return None;
    }
    // Check trigger fired (buy happened)
    if !flow_node_state_truthy(&run_spec.context, &node_spec.node_key, FLOW_NODE_STATE_ONCE_FIRED)
    {
        return None;
    }
    // Check market slug matches
    if let Some(fired_slug) = flow_node_state_string(
        &run_spec.context,
        &node_spec.node_key,
        FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG,
    ) {
        if node_spec
            .market_slug
            .as_deref()
            .map_or(true, |s| s != fired_slug)
        {
            return None;
        }
    }
    // Idempotency: check if already processed
    let sell_state_key = cycle_window_end_sell_state_key(node_spec);
    if flow_node_state_truthy(&run_spec.context, &node_spec.node_key, &sell_state_key) {
        return None;
    }
    Some(DueWindowEndSellTarget {
        run_index,
        node_index,
        window_end_at,
    })
}

#[derive(Debug, Clone)]
struct CycleWindowEvalDiagnostics {
    window_mode: String,
    cycle_window_secs: i64,
    window_open_at: DateTime<Utc>,
    window_end_at: DateTime<Utc>,
    evaluated_at: DateTime<Utc>,
    boundary_lag_ms: i64,
}

fn cycle_window_boundary_state_key(node_spec: &WsOpenPositionPriceNodeSpec) -> String {
    format!(
        "{}{}",
        FLOW_NODE_STATE_CYCLE_WINDOW_BOUNDARY_MARKER_PREFIX, node_spec.token_id
    )
}

fn cycle_window_boundary_marker(node_spec: &WsOpenPositionPriceNodeSpec) -> Option<String> {
    let mode = node_spec.cycle_window_mode.as_deref()?;
    let slug = node_spec.market_slug.as_deref()?;
    if mode == "custom_range" {
        Some(format!(
            "custom_range:{}:{}:{}",
            slug,
            node_spec.cycle_window_start_sec?,
            node_spec.cycle_window_end_sec?
        ))
    } else {
        Some(format!("{}:{}:{}", mode, slug, node_spec.cycle_window_secs?))
    }
}

fn cycle_window_last_eval_state_key(token_id: &str) -> String {
    format!("{FLOW_NODE_STATE_CYCLE_WINDOW_LAST_EVAL_PREFIX}{token_id}")
}

fn cycle_window_last_eval_state_key_for_node(node_spec: &WsOpenPositionPriceNodeSpec) -> String {
    cycle_window_last_eval_state_key(&node_spec.token_id)
}

fn cycle_window_bounds(
    node_spec: &WsOpenPositionPriceNodeSpec,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    if node_spec.node_type != "trigger.market_price" {
        return None;
    }
    let market_slug = node_spec.market_slug.as_deref()?;
    let cycle_window_mode = node_spec.cycle_window_mode.as_deref()?;
    let start_at = MarketCycleId(market_slug.to_string()).start_time()?;
    let scope_def = find_updown_scope_by_slug(market_slug)?;
    let duration_secs = updown_scope_window_seconds(scope_def);
    let end_at = start_at + ChronoDuration::seconds(duration_secs);

    match cycle_window_mode {
        "first" => {
            let effective = node_spec.cycle_window_secs?.clamp(1, duration_secs);
            Some((start_at, start_at + ChronoDuration::seconds(effective)))
        }
        "last" => {
            let effective = node_spec.cycle_window_secs?.clamp(1, duration_secs);
            Some((end_at - ChronoDuration::seconds(effective), end_at))
        }
        "custom_range" => {
            let s = node_spec.cycle_window_start_sec?;
            let e = node_spec.cycle_window_end_sec?;
            if s >= e || e > duration_secs {
                return None;
            }
            Some((
                start_at + ChronoDuration::seconds(s),
                start_at + ChronoDuration::seconds(e),
            ))
        }
        _ => None,
    }
}

fn cycle_window_eval_diagnostics(
    node_spec: &WsOpenPositionPriceNodeSpec,
    window_mode: &str,
    evaluated_at: DateTime<Utc>,
) -> Option<CycleWindowEvalDiagnostics> {
    let (window_open_at, window_end_at) = cycle_window_bounds(node_spec)?;
    let effective_secs = if node_spec.cycle_window_mode.as_deref() == Some("custom_range") {
        let s = node_spec.cycle_window_start_sec.unwrap_or(0);
        let e = node_spec.cycle_window_end_sec.unwrap_or(0);
        e - s
    } else {
        node_spec.cycle_window_secs?
    };
    Some(CycleWindowEvalDiagnostics {
        window_mode: window_mode.to_string(),
        cycle_window_secs: effective_secs,
        window_open_at,
        window_end_at,
        evaluated_at,
        boundary_lag_ms: evaluated_at
            .signed_duration_since(window_open_at)
            .num_milliseconds()
            .max(0),
    })
}

fn append_json_object_fields(target: &mut Value, extra: &Value) {
    let Some(target_obj) = target.as_object_mut() else {
        return;
    };
    let Some(extra_obj) = extra.as_object() else {
        return;
    };
    for (key, value) in extra_obj {
        target_obj.insert(key.clone(), value.clone());
    }
}

fn cycle_window_eval_payload_fields(diagnostics: &CycleWindowEvalDiagnostics) -> Value {
    json!({
        "window_open_at": diagnostics.window_open_at.to_rfc3339(),
        "window_end_at": diagnostics.window_end_at.to_rfc3339(),
        "evaluated_at": diagnostics.evaluated_at.to_rfc3339(),
        "boundary_lag_ms": diagnostics.boundary_lag_ms,
        "cycle_window_secs": diagnostics.cycle_window_secs,
    })
}

fn cycle_window_last_eval_state_payload(
    node_spec: &WsOpenPositionPriceNodeSpec,
    diagnostics: &CycleWindowEvalDiagnostics,
    result: &str,
    price: Option<f64>,
    evaluation_mode: Option<&str>,
) -> Value {
    json!({
        "window_mode": diagnostics.window_mode,
        "cycle_window_secs": diagnostics.cycle_window_secs,
        "window_open_at": diagnostics.window_open_at.to_rfc3339(),
        "window_end_at": diagnostics.window_end_at.to_rfc3339(),
        "evaluated_at": diagnostics.evaluated_at.to_rfc3339(),
        "boundary_lag_ms": diagnostics.boundary_lag_ms,
        "price": price,
        "trigger_price": node_spec.trigger_price,
        "max_price": node_spec.max_price,
        "result": result,
        "evaluation_mode": evaluation_mode,
        "market_slug": node_spec.market_slug,
    })
}

fn set_cycle_window_last_eval_state(
    context: &mut Value,
    node_spec: &WsOpenPositionPriceNodeSpec,
    diagnostics: &CycleWindowEvalDiagnostics,
    result: &str,
    price: Option<f64>,
    evaluation_mode: Option<&str>,
) {
    set_flow_node_state(
        context,
        &node_spec.node_key,
        &cycle_window_last_eval_state_key_for_node(node_spec),
        cycle_window_last_eval_state_payload(
            node_spec,
            diagnostics,
            result,
            price,
            evaluation_mode,
        ),
    );
}

fn cycle_window_followup_diagnostics_from_context(
    context: &Value,
    node_key: &str,
    token_id: &str,
    observed_at: DateTime<Utc>,
) -> Option<Value> {
    let state = flow_node_state(
        context,
        node_key,
        &cycle_window_last_eval_state_key(token_id),
    )?
    .as_object()?;
    let window_open_at = state
        .get("window_open_at")
        .and_then(Value::as_str)
        .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
        .map(|parsed| parsed.with_timezone(&Utc))?;

    Some(json!({
        "cycle_window_mode": state.get("window_mode").cloned().unwrap_or(Value::Null),
        "cycle_window_secs": state.get("cycle_window_secs").cloned().unwrap_or(Value::Null),
        "window_open_at": state.get("window_open_at").cloned().unwrap_or(Value::Null),
        "window_end_at": state.get("window_end_at").cloned().unwrap_or(Value::Null),
        "ms_since_window_open": observed_at
            .signed_duration_since(window_open_at)
            .num_milliseconds()
            .max(0),
        "boundary_result": state.get("result").cloned().unwrap_or(Value::Null),
    }))
}

fn cycle_window_boundary_due_target(
    run_spec: &WsOpenPositionPriceRunSpec,
    run_index: usize,
    node_spec: &WsOpenPositionPriceNodeSpec,
    node_index: usize,
    now: DateTime<Utc>,
) -> Option<DueCycleWindowBoundaryTarget> {
    let (window_open_at, window_end_at) = cycle_window_bounds(node_spec)?;
    if now < window_open_at || now >= window_end_at {
        return None;
    }
    let boundary_marker = cycle_window_boundary_marker(node_spec)?;
    let boundary_state_key = cycle_window_boundary_state_key(node_spec);
    if flow_node_state_string(&run_spec.context, &node_spec.node_key, &boundary_state_key)
        .as_deref()
        == Some(boundary_marker.as_str())
    {
        return None;
    }
    Some(DueCycleWindowBoundaryTarget {
        run_index,
        node_index,
        window_mode: node_spec.cycle_window_mode.clone().unwrap_or_default(),
        boundary_marker,
    })
}

fn cycle_window_confirmation_deadline(
    run_spec: &WsOpenPositionPriceRunSpec,
    node_spec: &WsOpenPositionPriceNodeSpec,
) -> Option<DateTime<Utc>> {
    let confirmation_ms = market_price_confirmation_ms(node_spec)?;
    let pending_at_key = format!("cross_pending_at_{}", node_spec.token_id);
    let pending_at_raw =
        flow_node_state_string(&run_spec.context, &node_spec.node_key, &pending_at_key)?;
    let pending_at = DateTime::parse_from_rfc3339(&pending_at_raw)
        .ok()?
        .with_timezone(&Utc);
    Some(pending_at + ChronoDuration::milliseconds(confirmation_ms))
}

async fn trade_flow_next_trigger_market_price_timer_delay() -> Option<Duration> {
    let cache = TRADE_FLOW_WS_FAST_PATH_CACHE.read().await;
    if cache.run_specs.is_empty() {
        return None;
    }

    let now = Utc::now();
    let mut next_delay: Option<Duration> = None;
    for run_spec in &cache.run_specs {
        for node_spec in &run_spec.nodes {
            if node_spec.node_type != "trigger.market_price" {
                continue;
            }

            if cycle_window_boundary_due_target(run_spec, 0, node_spec, 0, now).is_some() {
                return Some(Duration::from_millis(0));
            }

            if let Some((window_open_at, _)) = cycle_window_bounds(node_spec) {
                if now < window_open_at {
                    let delay_ms = window_open_at
                        .signed_duration_since(now)
                        .num_milliseconds()
                        .max(0) as u64;
                    let delay = Duration::from_millis(delay_ms);
                    next_delay = Some(next_delay.map_or(delay, |current| current.min(delay)));
                }
            }

            if let Some(deadline) = cycle_window_confirmation_deadline(run_spec, node_spec) {
                if deadline <= now {
                    return Some(Duration::from_millis(0));
                }
                let delay_ms = deadline
                    .signed_duration_since(now)
                    .num_milliseconds()
                    .max(0) as u64;
                let delay = Duration::from_millis(delay_ms);
                next_delay = Some(next_delay.map_or(delay, |current| current.min(delay)));
            }

            // Window-end auto-sell: schedule timer for window_end_at
            if node_spec.auto_sell_on_window_end {
                if cycle_window_end_sell_due_target(run_spec, 0, node_spec, 0, now).is_some() {
                    return Some(Duration::from_millis(0));
                }
                if let Some((_, window_end_at)) = cycle_window_bounds(node_spec) {
                    if now < window_end_at {
                        let delay_ms = window_end_at
                            .signed_duration_since(now)
                            .num_milliseconds()
                            .max(0) as u64;
                        let delay = Duration::from_millis(delay_ms);
                        next_delay =
                            Some(next_delay.map_or(delay, |current| current.min(delay)));
                    }
                }
            }
        }
    }

    next_delay
}

fn auto_scope_market_boundary_delay(
    node_spec: &WsOpenPositionPriceNodeSpec,
    now: DateTime<Utc>,
) -> Option<Duration> {
    if node_spec.node_type != "trigger.market_price" || !node_spec.auto_scope {
        return None;
    }

    let market_slug = node_spec.market_slug.as_deref()?;
    let scope_def = find_updown_scope_by_slug(market_slug)?;
    let window_secs = updown_scope_window_seconds(scope_def);
    let market_start = MarketCycleId(market_slug.to_string()).start_time()?;
    let market_end = market_start + ChronoDuration::seconds(window_secs);
    if now >= market_end {
        return Some(Duration::ZERO);
    }

    let delay_ms = market_end
        .signed_duration_since(now)
        .num_milliseconds()
        .max(0) as u64;
    Some(Duration::from_millis(delay_ms))
}

async fn trade_flow_next_auto_scope_boundary_refresh_delay() -> Option<Duration> {
    let cache = TRADE_FLOW_WS_FAST_PATH_CACHE.read().await;
    if cache.run_specs.is_empty() {
        return None;
    }

    let now = Utc::now();
    let mut next_delay: Option<Duration> = None;
    for run_spec in &cache.run_specs {
        for node_spec in &run_spec.nodes {
            let Some(delay) = auto_scope_market_boundary_delay(node_spec, now) else {
                continue;
            };
            next_delay = Some(next_delay.map_or(delay, |current| current.min(delay)));
        }
    }

    next_delay
}

async fn resolve_ws_fast_path_trigger_price(
    run_id: i64,
    node_spec: &WsOpenPositionPriceNodeSpec,
    market_snapshots: &HashMap<String, MarketDataSnapshot>,
    client: Option<&dyn OrderExecutor>,
) -> Option<ResolvedTriggerPrice> {
    let resolved = market_snapshots
        .get(&node_spec.token_id)
        .and_then(|snapshot| {
            resolve_trigger_price_from_market_snapshot(
                snapshot,
                node_spec.price_mode,
                Some(node_spec.trigger_condition.as_str()),
            )
        });
    if let Some(resolved) = resolved {
        return Some(resolved);
    }
    if let Some(cl) = client {
        match resolve_trigger_price_from_rest(
            cl,
            &node_spec.token_id,
            node_spec.price_mode,
            Some(node_spec.trigger_condition.as_str()),
        )
        .await
        {
            Ok(resolved) => return Some(resolved),
            Err(_) => {
                debug!(
                    run_id,
                    node_key = %node_spec.node_key,
                    token_id = %node_spec.token_id,
                    market = ?node_spec.market_slug,
                    "TRIGGER_WS_NO_PRICE_DATA_NO_REST"
                );
                return None;
            }
        }
    }

    debug!(
        run_id,
        node_key = %node_spec.node_key,
        token_id = %node_spec.token_id,
        market = ?node_spec.market_slug,
        "TRIGGER_WS_NO_PRICE_DATA"
    );
    None
}

async fn append_cycle_window_event(
    repo: &PostgresRepository,
    run_spec: &WsOpenPositionPriceRunSpec,
    node_spec: &WsOpenPositionPriceNodeSpec,
    event_type: &str,
    payload_json: &Value,
) {
    if let Err(err) = repo
        .append_trade_flow_event(
            Some(run_spec.run_id),
            run_spec.definition_id,
            Some(run_spec.version_id),
            event_type,
            payload_json,
        )
        .await
    {
        warn!(
            flow_run_id = run_spec.run_id,
            node_key = %node_spec.node_key,
            error = %err,
            "TRADE_FLOW_CYCLE_WINDOW_EVENT_FAILED"
        );
    }
}

async fn enqueue_cycle_window_prevalidated_step(
    repo: &PostgresRepository,
    run_spec: &WsOpenPositionPriceRunSpec,
    node_spec: &WsOpenPositionPriceNodeSpec,
    resolved_price: &ResolvedTriggerPrice,
    evaluation_mode: &str,
    window_mode: &str,
    diagnostics: Option<&CycleWindowEvalDiagnostics>,
    price_to_beat_trigger_gate: Option<&Value>,
) -> Result<bool> {
    let queued_at = Utc::now();
    let queued_at_rfc3339 = queued_at.to_rfc3339();
    let idempotency_key = if node_spec.once_mode {
        ws_price_trigger_step_idempotency_key(
            run_spec.run_id,
            &node_spec.node_key,
            &node_spec.trigger_condition,
            resolved_price.price,
            resolved_price.ts,
            true,
            node_spec.once_scope_market,
            node_spec.market_slug.as_deref(),
            flow_node_reentry_generation(&run_spec.context, &node_spec.node_key),
        )
    } else {
        format!(
            "ws-window-open:{}:{}:{}:{}:{}",
            run_spec.run_id,
            node_spec.node_key,
            node_spec.market_slug.as_deref().unwrap_or("unknown-market"),
            window_mode,
            node_spec.token_id
        )
    };
    let mut input_json = json!({
        "triggerSource": "ws_market_price",
        "tokenId": node_spec.token_id,
        "wsPrice": resolved_price.price,
        "wsPrices": { node_spec.token_id.clone(): resolved_price.price },
        "wsPreviousPrice": Value::Null,
        "wsPreviousPrices": { node_spec.token_id.clone(): Value::Null },
        "wsEventTs": resolved_price.ts,
        "wsMarketSlug": node_spec.market_slug.clone(),
        "wsEvaluationMode": evaluation_mode,
        "wsPriceMode": node_spec.price_mode.as_str(),
        "wsPriceSource": resolved_price.source.clone(),
        "wsPriceSourceDetail": resolved_price.detail.source_detail.clone(),
        "wsBestBid": resolved_price.detail.best_bid,
        "wsBestAsk": resolved_price.detail.best_ask,
        "wsLastTradePrice": resolved_price.detail.last_trade_price,
        "wsSnapshotAgeMs": resolved_price.detail.snapshot_age_ms,
        "wsSiteDisplayModeDecision": resolved_price.detail.site_display_mode_decision,
        "wsAllowFirstTickReplay": window_mode == "last" || window_mode == "custom_range",
        "windowBoundaryOpen": true,
        "windowBoundaryMode": window_mode,
        "versionId": run_spec.version_id,
        "versionNo": run_spec.version_no,
        "queuedAt": queued_at_rfc3339
    });
    if let Some(diagnostics) = diagnostics {
        let mut diag_json = json!({
            "cycleWindowSecs": diagnostics.cycle_window_secs,
            "windowOpenAt": diagnostics.window_open_at.to_rfc3339(),
            "windowEndAt": diagnostics.window_end_at.to_rfc3339(),
            "boundaryEvaluatedAt": diagnostics.evaluated_at.to_rfc3339(),
            "boundaryLagMs": diagnostics.boundary_lag_ms,
            "boundaryResult": "matched",
        });
        if window_mode == "custom_range" {
            append_json_object_fields(
                &mut diag_json,
                &json!({
                    "cycleWindowStartSec": node_spec.cycle_window_start_sec,
                    "cycleWindowEndSec": node_spec.cycle_window_end_sec,
                }),
            );
        }
        append_json_object_fields(&mut input_json, &diag_json);
    }
    if let Some(gate) = price_to_beat_trigger_gate {
        append_trigger_market_price_ptb_gate(&mut input_json, gate);
    }
    let enqueued = repo
        .enqueue_trade_flow_step(
            run_spec.run_id,
            &node_spec.node_key,
            &node_spec.node_type,
            1,
            Some(&input_json),
            queued_at,
            None,
            Some(&idempotency_key),
        )
        .await?;
    if enqueued.is_some() {
        FLOW_PROCESS_NOTIFY.notify_one();
        repo.append_trade_flow_event(
            Some(run_spec.run_id),
            run_spec.definition_id,
            Some(run_spec.version_id),
            "trigger_cycle_window_condition_met",
            &{
                let mut payload = json!({
                "node_key": node_spec.node_key,
                "node_type": node_spec.node_type,
                "token_id": node_spec.token_id,
                "market_slug": node_spec.market_slug,
                "window_mode": window_mode,
                "price": resolved_price.price,
                "trigger_condition": node_spec.trigger_condition,
                "trigger_price": node_spec.trigger_price,
                "max_price": node_spec.max_price,
                "evaluation_mode": evaluation_mode,
                "price_mode": node_spec.price_mode.as_str(),
                "price_source": input_json.get("wsPriceSource"),
                "version_id": run_spec.version_id,
                "version_no": run_spec.version_no,
                "idempotency_key": idempotency_key,
                });
                if let Some(gate) = price_to_beat_trigger_gate {
                    append_trigger_market_price_ptb_gate(&mut payload, gate);
                }
                if let Some(diagnostics) = diagnostics {
                    append_json_object_fields(
                        &mut payload,
                        &cycle_window_eval_payload_fields(diagnostics),
                    );
                }
                payload
            },
        )
        .await?;
    }
    Ok(enqueued.is_some())
}

async fn process_trade_flow_trigger_market_price_timers(
    repo: &PostgresRepository,
    run_id: i64,
    ws: &ClobWsClient,
    client: Option<&dyn OrderExecutor>,
) -> Result<bool> {
    let cache_snapshot = {
        let cache = TRADE_FLOW_WS_FAST_PATH_CACHE.read().await;
        cache.clone()
    };
    if cache_snapshot.run_specs.is_empty() {
        return Ok(false);
    }

    let now = Utc::now();
    let mut due_boundaries = Vec::new();
    let mut due_confirmation_token_ids = BTreeSet::new();
    let mut due_window_end_sells = Vec::new();
    for (run_index, run_spec) in cache_snapshot.run_specs.iter().enumerate() {
        for (node_index, node_spec) in run_spec.nodes.iter().enumerate() {
            if let Some(boundary_target) =
                cycle_window_boundary_due_target(run_spec, run_index, node_spec, node_index, now)
            {
                due_boundaries.push(boundary_target);
            }
            if let Some(deadline) = cycle_window_confirmation_deadline(run_spec, node_spec) {
                if deadline <= now {
                    due_confirmation_token_ids.insert(node_spec.token_id.clone());
                }
            }
            if let Some(sell_target) =
                cycle_window_end_sell_due_target(run_spec, run_index, node_spec, node_index, now)
            {
                due_window_end_sells.push(sell_target);
            }
        }
    }
    if due_boundaries.is_empty()
        && due_confirmation_token_ids.is_empty()
        && due_window_end_sells.is_empty()
    {
        return Ok(false);
    }

    let mut run_specs = cache_snapshot.run_specs;
    let token_targets = cache_snapshot.token_targets;
    let market_targets = cache_snapshot.market_targets;
    let boundary_token_ids: Vec<String> = due_boundaries
        .iter()
        .filter_map(|target| {
            run_specs
                .get(target.run_index)
                .and_then(|run_spec| run_spec.nodes.get(target.node_index))
                .map(|node_spec| node_spec.token_id.clone())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let boundary_snapshots = ws.get_market_snapshots(&boundary_token_ids).await;
    let mut touched = false;

    for target in due_boundaries {
        let Some(node_spec) = run_specs
            .get(target.run_index)
            .and_then(|run_spec| run_spec.nodes.get(target.node_index))
            .cloned()
        else {
            continue;
        };
        let Some(run_spec) = run_specs.get_mut(target.run_index) else {
            continue;
        };
        touched = true;

        let boundary_state_key = cycle_window_boundary_state_key(&node_spec);
        let diagnostics = cycle_window_eval_diagnostics(&node_spec, &target.window_mode, now);

        let mut entered_payload = json!({
            "node_key": node_spec.node_key,
            "node_type": node_spec.node_type,
            "token_id": node_spec.token_id,
            "market_slug": node_spec.market_slug,
            "window_mode": target.window_mode,
            "trigger_condition": node_spec.trigger_condition,
            "trigger_price": node_spec.trigger_price,
            "max_price": node_spec.max_price,
            "price_mode": node_spec.price_mode.as_str(),
        });
        if let Some(diagnostics) = diagnostics.as_ref() {
            append_json_object_fields(
                &mut entered_payload,
                &cycle_window_eval_payload_fields(diagnostics),
            );
        }
        append_cycle_window_event(
            repo,
            run_spec,
            &node_spec,
            "trigger_cycle_window_entered",
            &entered_payload,
        )
        .await;

        if node_spec.once_mode
            && trade_flow_market_price_once_fired_for_scope(
                &run_spec.context,
                &node_spec.node_key,
                node_spec.once_scope_market,
                node_spec.market_slug.as_deref(),
            )
        {
            set_flow_node_state(
                &mut run_spec.context,
                &node_spec.node_key,
                &boundary_state_key,
                json!(target.boundary_marker),
            );
            run_spec.context_dirty = true;
            continue;
        }

        let Some(resolved_price) =
            resolve_ws_fast_path_trigger_price(run_id, &node_spec, &boundary_snapshots, client)
                .await
        else {
            if let Some(diagnostics) = diagnostics.as_ref() {
                set_cycle_window_last_eval_state(
                    &mut run_spec.context,
                    &node_spec,
                    diagnostics,
                    "price_unavailable",
                    None,
                    None,
                );
                run_spec.context_dirty = true;
            }
            let mut not_met_payload = json!({
                "node_key": node_spec.node_key,
                "token_id": node_spec.token_id,
                "market_slug": node_spec.market_slug,
                "window_mode": target.window_mode,
                "reason": "price_unavailable",
                "price_mode": node_spec.price_mode.as_str(),
            });
            if let Some(diagnostics) = diagnostics.as_ref() {
                append_json_object_fields(
                    &mut not_met_payload,
                    &cycle_window_eval_payload_fields(diagnostics),
                );
            }
            append_cycle_window_event(
                repo,
                run_spec,
                &node_spec,
                "trigger_cycle_window_condition_not_met",
                &not_met_payload,
            )
            .await;
            continue;
        };

        set_flow_node_state(
            &mut run_spec.context,
            &node_spec.node_key,
            &boundary_state_key,
            json!(target.boundary_marker),
        );
        run_spec.context_dirty = true;

        set_flow_node_state(
            &mut run_spec.context,
            &node_spec.node_key,
            "last_price",
            json!(resolved_price.price),
        );
        set_flow_node_state(
            &mut run_spec.context,
            &node_spec.node_key,
            &format!("previous_price_{}", node_spec.token_id),
            json!(resolved_price.price),
        );
        run_spec.context_dirty = true;

        let ptb_config = trigger_market_price_ptb_config_from_spec(&node_spec);
        let Some(gate_mode) =
            trigger_market_price_gate_mode(&node_spec.trigger_condition, ptb_config)
        else {
            continue;
        };
        let allow_first_tick_at_boundary =
            target.window_mode == "last" || target.window_mode == "custom_range";
        let (matched, evaluation_mode) = if matches!(
            gate_mode,
            TriggerMarketPriceGateMode::StandardOnly
                | TriggerMarketPriceGateMode::StandardAndPtb
        ) {
            evaluate_trigger_market_price_condition(
                None,
                resolved_price.price,
                node_spec.trigger_price,
                &node_spec.trigger_condition,
                allow_first_tick_at_boundary,
                node_spec.max_price,
            )
        } else {
            (true, "ptb_only")
        };

        if !matched {
            let event_type = if node_spec
                .max_price
                .map(|max_price| {
                    resolved_price.price >= node_spec.trigger_price
                        && resolved_price.price > max_price
                })
                .unwrap_or(false)
            {
                "trigger_cycle_window_above_max"
            } else {
                "trigger_cycle_window_condition_not_met"
            };
            if let Some(diagnostics) = diagnostics.as_ref() {
                set_cycle_window_last_eval_state(
                    &mut run_spec.context,
                    &node_spec,
                    diagnostics,
                    if event_type == "trigger_cycle_window_above_max" {
                        "above_max"
                    } else {
                        "condition_not_met"
                    },
                    Some(resolved_price.price),
                    Some(evaluation_mode),
                );
                run_spec.context_dirty = true;
            }
            let mut event_payload = json!({
                "node_key": node_spec.node_key,
                "token_id": node_spec.token_id,
                "market_slug": node_spec.market_slug,
                "window_mode": target.window_mode,
                "price": resolved_price.price,
                "trigger_condition": node_spec.trigger_condition,
                "trigger_price": node_spec.trigger_price,
                "max_price": node_spec.max_price,
                "evaluation_mode": evaluation_mode,
                "price_mode": node_spec.price_mode.as_str(),
            });
            if let Some(diagnostics) = diagnostics.as_ref() {
                append_json_object_fields(
                    &mut event_payload,
                    &cycle_window_eval_payload_fields(diagnostics),
                );
            }
            append_cycle_window_event(repo, run_spec, &node_spec, event_type, &event_payload).await;
            continue;
        }

        if let Some(diagnostics) = diagnostics.as_ref() {
            set_cycle_window_last_eval_state(
                &mut run_spec.context,
                &node_spec,
                diagnostics,
                "matched",
                Some(resolved_price.price),
                Some(evaluation_mode),
            );
            run_spec.context_dirty = true;
        }

        if matches!(
            gate_mode,
            TriggerMarketPriceGateMode::StandardOnly
                | TriggerMarketPriceGateMode::StandardAndPtb
        ) {
            if let Some(confirmation_ms) = market_price_confirmation_ms(&node_spec) {
            let cpend_at_key = format!("cross_pending_at_{}", node_spec.token_id);
            let cpend_price_key = format!("cross_pending_price_{}", node_spec.token_id);
            let cpend_prev_key = format!("cross_pending_prev_{}", node_spec.token_id);
            set_flow_node_state(
                &mut run_spec.context,
                &node_spec.node_key,
                &cpend_at_key,
                json!(Utc::now().to_rfc3339()),
            );
            set_flow_node_state(
                &mut run_spec.context,
                &node_spec.node_key,
                &cpend_price_key,
                json!(resolved_price.price),
            );
            set_flow_node_state(
                &mut run_spec.context,
                &node_spec.node_key,
                &cpend_prev_key,
                Value::Null,
            );
            run_spec.context_dirty = true;
            append_cycle_window_event(
                repo,
                run_spec,
                &node_spec,
                "trigger_cycle_window_confirmation_started",
                &{
                    let mut payload = json!({
                        "node_key": node_spec.node_key,
                        "token_id": node_spec.token_id,
                        "market_slug": node_spec.market_slug,
                        "window_mode": target.window_mode,
                        "price": resolved_price.price,
                        "confirmation_ms": confirmation_ms,
                        "evaluation_mode": evaluation_mode,
                        "price_mode": node_spec.price_mode.as_str(),
                    });
                    if let Some(diagnostics) = diagnostics.as_ref() {
                        append_json_object_fields(
                            &mut payload,
                            &cycle_window_eval_payload_fields(diagnostics),
                        );
                    }
                    payload
                },
            )
            .await;
            continue;
        }
        }

        let mut ptb_gate_output = Value::Null;
        if matches!(
            gate_mode,
            TriggerMarketPriceGateMode::PtbOnly | TriggerMarketPriceGateMode::StandardAndPtb
        ) {
            if let Some(ptb_gate) = evaluate_trigger_market_price_ptb_gate_for_spec(&node_spec) {
                ptb_gate_output = ptb_gate.to_value();
                if !ptb_gate.passed {
                    if let Some(diagnostics) = diagnostics.as_ref() {
                        set_cycle_window_last_eval_state(
                            &mut run_spec.context,
                            &node_spec,
                            diagnostics,
                            "ptb_gate_blocked",
                            Some(resolved_price.price),
                            Some(evaluation_mode),
                        );
                        run_spec.context_dirty = true;
                    }
                    append_cycle_window_event(
                        repo,
                        run_spec,
                        &node_spec,
                        "trigger_cycle_window_price_to_beat_gate_blocked",
                        &{
                            let mut payload = json!({
                                "node_key": node_spec.node_key,
                                "node_type": node_spec.node_type,
                                "token_id": node_spec.token_id,
                                "market_slug": node_spec.market_slug,
                                "window_mode": target.window_mode,
                                "price": resolved_price.price,
                                "evaluation_mode": evaluation_mode,
                                "price_mode": node_spec.price_mode.as_str(),
                                "price_to_beat_trigger_gate": ptb_gate_output.clone(),
                            });
                            if let Some(diagnostics) = diagnostics.as_ref() {
                                append_json_object_fields(
                                    &mut payload,
                                    &cycle_window_eval_payload_fields(diagnostics),
                                );
                            }
                            payload
                        },
                    )
                    .await;
                    continue;
                }
            }
        }

        let _ = enqueue_cycle_window_prevalidated_step(
            repo,
            run_spec,
            &node_spec,
            &resolved_price,
            evaluation_mode,
            &target.window_mode,
            diagnostics.as_ref(),
            (!ptb_gate_output.is_null()).then_some(&ptb_gate_output),
        )
        .await?;
    }

    // Window-end auto-sell processing
    for target in due_window_end_sells {
        let Some(node_spec) = run_specs
            .get(target.run_index)
            .and_then(|run_spec| run_spec.nodes.get(target.node_index))
            .cloned()
        else {
            continue;
        };
        let Some(run_spec) = run_specs.get_mut(target.run_index) else {
            continue;
        };

        let buy_output = match repo
            .find_latest_completed_place_order_output_for_node(run_spec.run_id, &node_spec.node_key)
            .await
        {
            Ok(Some(output)) => output,
            Ok(None) => {
                let sell_state_key = cycle_window_end_sell_state_key(&node_spec);
                set_flow_node_state(
                    &mut run_spec.context,
                    &node_spec.node_key,
                    &sell_state_key,
                    json!({ "status": "skipped", "reason": "no_completed_buy_output" }),
                );
                run_spec.context_dirty = true;
                touched = true;
                info!(
                    run_id,
                    flow_run_id = run_spec.run_id,
                    node_key = %node_spec.node_key,
                    token_id = %node_spec.token_id,
                    "WINDOW_END_AUTO_SELL_SKIP_NO_BUY"
                );
                continue;
            }
            Err(e) => {
                warn!(
                    run_id,
                    flow_run_id = run_spec.run_id,
                    error = %e,
                    "WINDOW_END_AUTO_SELL_QUERY_FAILED"
                );
                continue;
            }
        };

        let builder_order_id = cycle_window_end_auto_sell_builder_order_id(
            &buy_output,
            node_spec.market_slug.as_deref(),
        );

        let Some(builder_order_id) = builder_order_id else {
            let sell_state_key = cycle_window_end_sell_state_key(&node_spec);
            set_flow_node_state(
                &mut run_spec.context,
                &node_spec.node_key,
                &sell_state_key,
                json!({ "status": "skipped", "reason": "no_matching_builder_order" }),
            );
            run_spec.context_dirty = true;
            touched = true;
            info!(
                run_id,
                flow_run_id = run_spec.run_id,
                node_key = %node_spec.node_key,
                "WINDOW_END_AUTO_SELL_SKIP_NO_BUILDER_ORDER"
            );
            continue;
        };

        let Some(parent_order) = repo.get_trade_builder_order(builder_order_id).await? else {
            let sell_state_key = cycle_window_end_sell_state_key(&node_spec);
            set_flow_node_state(
                &mut run_spec.context,
                &node_spec.node_key,
                &sell_state_key,
                json!({ "status": "skipped", "reason": "missing_parent_builder_order" }),
            );
            run_spec.context_dirty = true;
            touched = true;
            continue;
        };

        let resolved_parent_inventory = resolve_trade_builder_parent_exit_inventory(
            repo,
            &parent_order,
            "window_end_auto_sell_due",
        )
        .await?;
        let current_parent_qty = resolved_parent_inventory.map(|(qty, _)| qty);
        if current_parent_qty.unwrap_or_default() <= TRADE_BUILDER_EXIT_QTY_TOLERANCE {
            let sell_state_key = cycle_window_end_sell_state_key(&node_spec);
            set_flow_node_state(
                &mut run_spec.context,
                &node_spec.node_key,
                &sell_state_key,
                json!({ "status": "skipped", "reason": "already_closed" }),
            );
            run_spec.context_dirty = true;
            touched = true;
            repo.append_trade_builder_order_event(
                parent_order.id,
                "window_end_auto_sell_skipped_closed",
                &json!({
                    "node_key": node_spec.node_key,
                    "window_end_at": target.window_end_at.to_rfc3339(),
                    "current_parent_qty": current_parent_qty,
                }),
            )
            .await?;
            continue;
        }

        let sell_input = build_cycle_window_end_auto_sell_input(&node_spec, &parent_order);

        let idempotency_key = format!(
            "window_end_sell:{}:{}:{}",
            run_spec.run_id,
            node_spec.token_id,
            target.window_end_at.timestamp()
        );

        let enqueued = repo
            .enqueue_trade_flow_step(
                run_spec.run_id,
                &node_spec.node_key,
                "action.place_order",
                1,
                Some(&sell_input),
                Utc::now(),
                None,
                Some(&idempotency_key),
            )
            .await;

        let enqueued_ok = matches!(&enqueued, Ok(Some(_)) | Ok(None));
        if enqueued_ok {
            let sell_state_key = cycle_window_end_sell_state_key(&node_spec);
            set_flow_node_state(
                &mut run_spec.context,
                &node_spec.node_key,
                &sell_state_key,
                json!({ "status": "enqueued", "idempotency_key": idempotency_key }),
            );
            run_spec.context_dirty = true;
            touched = true;
        }

        append_cycle_window_event(
            repo,
            run_spec,
            &node_spec,
            "trigger_cycle_window_end_auto_sell",
            &json!({
                "node_key": node_spec.node_key,
                "token_id": node_spec.token_id,
                "market_slug": node_spec.market_slug,
                "builder_order_id": builder_order_id,
                "parent_builder_order_id": parent_order.id,
                "source_trade_id": parent_order.trade_id,
                "current_parent_qty": current_parent_qty,
                "window_end_at": target.window_end_at.to_rfc3339(),
                "idempotency_key": idempotency_key,
                "enqueued": enqueued_ok,
            }),
        )
        .await;

        if let Ok(Some(step_id)) = enqueued {
            repo.append_trade_builder_order_event(
                parent_order.id,
                "window_end_auto_sell_enqueued",
                &json!({
                    "node_key": node_spec.node_key,
                    "step_id": step_id,
                    "window_end_at": target.window_end_at.to_rfc3339(),
                    "source_trade_id": parent_order.trade_id,
                    "current_parent_qty": current_parent_qty,
                }),
            )
            .await?;
            info!(
                run_id,
                flow_run_id = run_spec.run_id,
                step_id,
                node_key = %node_spec.node_key,
                token_id = %node_spec.token_id,
                builder_order_id = parent_order.id,
                "WINDOW_END_AUTO_SELL_ENQUEUED"
            );
        }
    }

    persist_trade_flow_ws_run_specs_contexts(repo, &mut run_specs).await?;
    {
        let mut cache = TRADE_FLOW_WS_FAST_PATH_CACHE.write().await;
        cache.run_specs = run_specs;
        cache.token_targets = token_targets;
        cache.market_targets = market_targets;
    }

    if !due_confirmation_token_ids.is_empty() {
        let confirmation_token_ids = due_confirmation_token_ids.into_iter().collect::<Vec<_>>();
        touched |= enqueue_trade_flow_ws_open_position_price_steps_from_cache(
            repo,
            run_id,
            ws,
            client,
            Some(&confirmation_token_ids),
        )
        .await?;
    }

    Ok(touched)
}
