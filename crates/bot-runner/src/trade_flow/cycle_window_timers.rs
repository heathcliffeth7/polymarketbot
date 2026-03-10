#[derive(Debug, Clone)]
struct DueCycleWindowBoundaryTarget {
    run_index: usize,
    node_index: usize,
    window_mode: String,
    boundary_marker: String,
}

const FLOW_NODE_STATE_CYCLE_WINDOW_LAST_EVAL_PREFIX: &str = "cycle_window_last_eval_";

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
        FLOW_NODE_STATE_CYCLE_WINDOW_BOUNDARY_MARKER_PREFIX,
        node_spec.token_id
    )
}

fn cycle_window_boundary_marker(node_spec: &WsOpenPositionPriceNodeSpec) -> Option<String> {
    Some(format!(
        "{}:{}:{}",
        node_spec.cycle_window_mode.as_deref()?,
        node_spec.market_slug.as_deref()?,
        node_spec.cycle_window_secs?
    ))
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
    let cycle_window_secs = node_spec.cycle_window_secs?;
    let start_at = MarketCycleId(market_slug.to_string()).start_time()?;
    let scope_def = find_updown_scope_by_slug(market_slug)?;
    let duration_secs = updown_scope_window_seconds(scope_def);
    let end_at = start_at + ChronoDuration::seconds(duration_secs);
    let effective_window_secs = cycle_window_secs.clamp(1, duration_secs);

    match cycle_window_mode {
        "first" => Some((
            start_at,
            start_at + ChronoDuration::seconds(effective_window_secs),
        )),
        "last" => Some((
            end_at - ChronoDuration::seconds(effective_window_secs),
            end_at,
        )),
        _ => None,
    }
}

fn cycle_window_eval_diagnostics(
    node_spec: &WsOpenPositionPriceNodeSpec,
    window_mode: &str,
    evaluated_at: DateTime<Utc>,
) -> Option<CycleWindowEvalDiagnostics> {
    let (window_open_at, window_end_at) = cycle_window_bounds(node_spec)?;
    Some(CycleWindowEvalDiagnostics {
        window_mode: window_mode.to_string(),
        cycle_window_secs: node_spec.cycle_window_secs?,
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
    let state = flow_node_state(context, node_key, &cycle_window_last_eval_state_key(token_id))?
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
                let delay_ms =
                    deadline.signed_duration_since(now).num_milliseconds().max(0) as u64;
                let delay = Duration::from_millis(delay_ms);
                next_delay = Some(next_delay.map_or(delay, |current| current.min(delay)));
            }
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
        .and_then(|snapshot| resolve_trigger_price_from_market_snapshot(snapshot, node_spec.price_mode));
    if let Some(resolved) = resolved {
        return Some(resolved);
    }
    if let Some(cl) = client {
        match resolve_trigger_price_from_rest(cl, &node_spec.token_id, node_spec.price_mode).await {
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
        "windowBoundaryOpen": true,
        "windowBoundaryMode": window_mode,
        "queuedAt": queued_at_rfc3339
    });
    if let Some(diagnostics) = diagnostics {
        append_json_object_fields(
            &mut input_json,
            &json!({
                "cycleWindowSecs": diagnostics.cycle_window_secs,
                "windowOpenAt": diagnostics.window_open_at.to_rfc3339(),
                "windowEndAt": diagnostics.window_end_at.to_rfc3339(),
                "boundaryEvaluatedAt": diagnostics.evaluated_at.to_rfc3339(),
                "boundaryLagMs": diagnostics.boundary_lag_ms,
                "boundaryResult": "matched",
            }),
        );
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
                "idempotency_key": idempotency_key,
                });
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
        }
    }
    if due_boundaries.is_empty() && due_confirmation_token_ids.is_empty() {
        return Ok(false);
    }

    let mut run_specs = cache_snapshot.run_specs;
    let token_targets = cache_snapshot.token_targets;
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
        let diagnostics =
            cycle_window_eval_diagnostics(&node_spec, &target.window_mode, now);

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

        let Some(resolved_price) = resolve_ws_fast_path_trigger_price(
            run_id,
            &node_spec,
            &boundary_snapshots,
            client,
        )
        .await else {
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

        let (matched, evaluation_mode) = evaluate_trigger_market_price_condition(
            None,
            resolved_price.price,
            node_spec.trigger_price,
            &node_spec.trigger_condition,
            true,
            node_spec.max_price,
        );

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
            append_cycle_window_event(
                repo,
                run_spec,
                &node_spec,
                event_type,
                &event_payload,
            )
            .await;
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

        let _ = enqueue_cycle_window_prevalidated_step(
            repo,
            run_spec,
            &node_spec,
            &resolved_price,
            evaluation_mode,
            &target.window_mode,
            diagnostics.as_ref(),
        )
        .await?;
    }

    persist_trade_flow_ws_run_specs_contexts(repo, &mut run_specs).await?;
    {
        let mut cache = TRADE_FLOW_WS_FAST_PATH_CACHE.write().await;
        cache.run_specs = run_specs;
        cache.token_targets = token_targets;
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
