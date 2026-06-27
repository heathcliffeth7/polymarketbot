async fn process_trade_flows(
    repo: &PostgresRepository,
    run_id: i64,
    _cfg: &AppConfig,
    client: Option<&dyn OrderExecutor>,
    ws: &ClobWsClient,
    flow_runtime_caches: &mut FlowRuntimeCaches,
    auto_claim_runtimes: &mut HashMap<i64, FlowAutoClaimRuntime>,
    flow_timing: &mut crate::trade_flow::housekeeping_timing::FlowHousekeepingTimingStats,
) -> Result<()> {
    maybe_recover_stale_running_trade_flow_steps(repo, run_id, flow_runtime_caches).await;
    let definitions_result = crate::trade_flow::housekeeping_timing::measure_flow_housekeeping_phase(
        flow_timing,
        crate::trade_flow::housekeeping_timing::FlowHousekeepingPhase::LoadDefinitions,
        repo.list_published_trade_flow_definitions(FLOW_DEFINITION_PROCESS_LIMIT),
    )
    .await;
    let definitions = definitions_result?;
    info!(
        run_id,
        count = definitions.len(),
        "TRADE_FLOW_DEFINITIONS_LOADED"
    );
    if definitions.is_empty() {
        let refresh_result =
            crate::trade_flow::housekeeping_timing::measure_flow_housekeeping_phase(
                flow_timing,
                crate::trade_flow::housekeeping_timing::FlowHousekeepingPhase::RefreshWsFastPathCache,
                refresh_trade_flow_ws_fast_path_cache(
                    repo,
                    run_id,
                    ws,
                    &definitions,
                    &mut flow_runtime_caches.user_cfg,
                ),
            )
            .await;
        refresh_result?;
        crate::trade_flow::housekeeping_timing::measure_flow_housekeeping_phase(
            flow_timing,
            crate::trade_flow::housekeeping_timing::FlowHousekeepingPhase::AutoClaim,
            maybe_tick_flow_auto_claims(
                repo,
                run_id,
                &definitions,
                &mut flow_runtime_caches.user_cfg,
                auto_claim_runtimes,
            ),
        )
        .await;
        return Ok(());
    }

    let sync_started = Instant::now();
    for definition in &definitions {
        flow_runtime_caches.touch(definition.user_id);
        if let Err(err) = sync_trade_flow_definition_run(repo, run_id, definition).await {
            warn!(
                run_id,
                definition_id = definition.id,
                error = %err,
                "TRADE_FLOW_RUN_SYNC_ERROR"
            );
        }
    }
    flow_timing.set_phase_ms(
        crate::trade_flow::housekeeping_timing::FlowHousekeepingPhase::SyncDefinitionRuns,
        crate::trade_flow::housekeeping_timing::millis_u64(sync_started.elapsed()),
    );
    let refresh_result = crate::trade_flow::housekeeping_timing::measure_flow_housekeeping_phase(
        flow_timing,
        crate::trade_flow::housekeeping_timing::FlowHousekeepingPhase::RefreshWsFastPathCache,
        refresh_trade_flow_ws_fast_path_cache(
            repo,
            run_id,
            ws,
            &definitions,
            &mut flow_runtime_caches.user_cfg,
        ),
    )
    .await;
    refresh_result?;
    let enqueue_result = crate::trade_flow::housekeeping_timing::measure_flow_housekeeping_phase(
        flow_timing,
        crate::trade_flow::housekeeping_timing::FlowHousekeepingPhase::EnqueueWsOpenPositionSteps,
        enqueue_trade_flow_ws_open_position_price_steps_from_cache(repo, run_id, ws, client, None),
    )
    .await;
    if let Err(err) = enqueue_result {
        warn!(run_id, error = %err, "TRADE_FLOW_WS_TRIGGER_ENQUEUE_FAILED");
    }
    let timers_result = crate::trade_flow::housekeeping_timing::measure_flow_housekeeping_phase(
        flow_timing,
        crate::trade_flow::housekeeping_timing::FlowHousekeepingPhase::ProcessMarketPriceTimers,
        process_trade_flow_trigger_market_price_timers(repo, run_id, ws, client),
    )
    .await;
    if let Err(err) = timers_result {
        warn!(run_id, error = %err, "TRADE_FLOW_CYCLE_WINDOW_TIMER_FAILED");
    }

    crate::trade_flow::housekeeping_timing::measure_flow_housekeeping_phase(
        flow_timing,
        crate::trade_flow::housekeeping_timing::FlowHousekeepingPhase::AutoClaim,
        maybe_tick_flow_auto_claims(
            repo,
            run_id,
            &definitions,
            &mut flow_runtime_caches.user_cfg,
            auto_claim_runtimes,
        ),
    )
    .await;

    crate::trade_flow::housekeeping_timing::measure_flow_housekeeping_phase(
        flow_timing,
        crate::trade_flow::housekeeping_timing::FlowHousekeepingPhase::ProcessReadySteps,
        process_trade_flow_ready_steps(repo, run_id, client, ws, flow_runtime_caches),
    )
    .await
}

async fn maybe_recover_stale_running_trade_flow_steps(
    repo: &PostgresRepository,
    run_id: i64,
    flow_runtime_caches: &mut FlowRuntimeCaches,
) {
    let due = flow_runtime_caches
        .last_stale_running_step_recovery_at
        .map(|last| last.elapsed().as_secs() >= FLOW_STALE_RUNNING_STEP_RECOVERY_INTERVAL_SECS)
        .unwrap_or(true);
    if !due {
        return;
    }
    flow_runtime_caches.last_stale_running_step_recovery_at = Some(Instant::now());
    match repo
        .recover_stale_running_trade_flow_steps(FLOW_STALE_RUNNING_STEP_RECOVERY_AGE_MS, 250)
        .await
    {
        Ok(0) => {}
        Ok(recovered_count) => {
            warn!(
                run_id,
                recovered_count,
                stale_after_ms = FLOW_STALE_RUNNING_STEP_RECOVERY_AGE_MS,
                "TRADE_FLOW_STALE_RUNNING_STEPS_RECOVERED"
            );
        }
        Err(err) => {
            warn!(
                run_id,
                error = %err,
                "TRADE_FLOW_STALE_RUNNING_STEP_RECOVERY_FAILED"
            );
        }
    }
}

async fn process_trade_flow_ws_fast_path(
    repo: &PostgresRepository,
    run_id: i64,
    client: Option<&dyn OrderExecutor>,
    ws: &ClobWsClient,
    flow_runtime_caches: &mut FlowRuntimeCaches,
    dirty_token_ids: &[String],
) -> Result<()> {
    if dirty_token_ids.is_empty() {
        return Ok(());
    }

    if let Err(err) = enqueue_trade_flow_ws_open_position_price_steps_from_cache(
        repo,
        run_id,
        ws,
        client,
        Some(dirty_token_ids),
    )
    .await
    {
        warn!(run_id, error = %err, "TRADE_FLOW_WS_FAST_PATH_ENQUEUE_FAILED");
    }
    process_trade_flow_ready_steps(repo, run_id, client, ws, flow_runtime_caches).await
}

async fn is_near_any_auto_scope_boundary(window_secs: i64) -> bool {
    let cache = TRADE_FLOW_WS_FAST_PATH_CACHE.read().await;
    let now = Utc::now();
    for run_spec in &cache.run_specs {
        for node_spec in &run_spec.nodes {
            if !node_spec.auto_scope {
                continue;
            }
            let Some(slug) = node_spec.market_slug.as_deref() else {
                continue;
            };
            let Some(scope_def) = find_updown_scope_by_slug(slug) else {
                continue;
            };
            let window = updown_scope_window_seconds(scope_def);
            let Some(start) = MarketCycleId(slug.to_string()).start_time() else {
                continue;
            };
            let end = start + ChronoDuration::seconds(window);
            let secs_to_end = end.signed_duration_since(now).num_seconds();
            if secs_to_end.abs() <= window_secs {
                return true;
            }
        }
    }
    false
}

async fn process_trade_flow_ready_steps(
    repo: &PostgresRepository,
    run_id: i64,
    client: Option<&dyn OrderExecutor>,
    ws: &ClobWsClient,
    flow_runtime_caches: &mut FlowRuntimeCaches,
) -> Result<()> {
    crate::trade_flow::guards::price_to_beat::with_price_to_beat_block_event_coalescing_pass_stats(
        crate::trade_flow::guards::price_to_beat::with_price_to_beat_iv_depth_order_book_pass_cache(
            process_trade_flow_ready_steps_inner(repo, run_id, client, ws, flow_runtime_caches),
        ),
    )
    .await
}

async fn process_trade_flow_ready_steps_inner(
    repo: &PostgresRepository,
    run_id: i64,
    client: Option<&dyn OrderExecutor>,
    ws: &ClobWsClient,
    flow_runtime_caches: &mut FlowRuntimeCaches,
) -> Result<()> {
    let policy = DefaultRiskPolicy;
    let step_processing_context = FlowStepProcessingContext::new();
    let mut stats = FlowStepProcessingStats {
        processing_run_id: step_processing_context.id.clone(),
        ..FlowStepProcessingStats::default()
    };
    let max_passes: u8 = if is_near_any_auto_scope_boundary(10).await {
        2
    } else {
        8
    };
    let mut claim_pass = 0u8;
    let mut max_passes_reached = false;
    loop {
        claim_pass += 1;
        let claimed_steps = repo
            .claim_ready_trade_flow_steps_for_processing_run(
                FLOW_STEP_PROCESS_LIMIT,
                Some(&step_processing_context.id),
            )
            .await?;
        if claimed_steps.is_empty() {
            break;
        }
        let last_pass_claimed_steps = claimed_steps.len();
        stats.claim_passes = claim_pass;
        stats.last_pass_claimed_steps = last_pass_claimed_steps;
        stats.claimed_step_count += last_pass_claimed_steps;
        for step in claimed_steps {
            let Some(run) = repo.get_trade_flow_run(step.run_id).await? else {
                let _ = repo.mark_trade_flow_step_skipped(step.id, None).await;
                continue;
            };
            let result = async {
                flow_runtime_caches.touch(run.user_id);
                let flow_cfg = load_user_app_config_cached(
                    repo,
                    run.user_id,
                    &mut flow_runtime_caches.user_cfg,
                )
                .await?;
                let limits = to_risk_limits(&flow_cfg);
                let flow_client = if client.is_some() {
                    Some(
                        load_user_order_executor_cached(
                            repo,
                            run.user_id,
                            &mut flow_runtime_caches.user_cfg,
                            &mut flow_runtime_caches.user_executor,
                        )
                        .await?,
                    )
                } else {
                    None
                };
                process_trade_flow_step(
                    repo,
                    run_id,
                    &flow_cfg,
                    &limits,
                    &policy,
                    flow_client,
                    ws,
                    &step,
                    &step_processing_context,
                    &mut stats,
                )
                .await
            }
            .await;
            if let Err(err) = result {
                warn!(
                    run_id,
                    step_id = step.id,
                    run_id = step.run_id,
                    error = %err,
                    "TRADE_FLOW_STEP_PROCESS_ERROR"
                );
                let error_json = json!({
                    "error": err.to_string(),
                    "node_key": step.node_key,
                    "node_type": step.node_type
                });
                let _ = repo
                    .mark_trade_flow_step_failed(step.id, Some(&error_json), &err.to_string())
                    .await;
            }
        }
        if claim_pass >= max_passes {
            max_passes_reached = true;
            break;
        }
    }

    let backlog = repo
        .count_ready_trade_flow_step_backlog_for_processing_run(Some(&step_processing_context.id))
        .await?;
    stats.ptb_retry_same_run_excluded_count = backlog.ready_same_run_ptb_retry;
    stats.ready_remaining_total_count = backlog.ready_total;
    stats.ready_remaining_ptb_retry_count = backlog.ready_ptb_retry;
    stats.runnable_non_retry_ready_count = backlog.ready_non_retry;
    let iv_depth_stats =
        crate::trade_flow::guards::price_to_beat::price_to_beat_iv_depth_order_book_pass_stats();
    stats.clob_book_fetch_hit_count = iv_depth_stats.cache_hits;
    stats.clob_book_fetch_pass_cache_hit_count = iv_depth_stats.pass_cache_hits;
    stats.clob_book_fetch_process_ttl_hit_count = iv_depth_stats.process_ttl_cache_hits;
    stats.clob_book_fetch_miss_count = iv_depth_stats.cache_misses;
    stats.clob_book_fetch_error_count = iv_depth_stats.fetch_errors;
    stats.unique_book_tokens_fetched = iv_depth_stats.unique_tokens_fetched;
    let coalescing_stats =
        crate::trade_flow::guards::price_to_beat::price_to_beat_block_event_coalescing_pass_stats();
    stats.coalesced_event_suppressed_count = coalescing_stats.suppressed_count;

    if max_passes_reached {
        let retry_only = stats.runnable_non_retry_ready_count == 0;
        let max_passes_reason = if retry_only {
            "retry_only_backlog"
        } else {
            "runnable_backlog"
        };
        if max_passes < 8 {
            debug!(
                run_id,
                claim_pass,
                max_passes,
                max_passes_reason,
                retry_only,
                claimed_steps_total = stats.claimed_step_count,
                last_pass_claimed_steps = stats.last_pass_claimed_steps,
                ready_remaining_total = stats.ready_remaining_total_count,
                ready_remaining_ptb_retry = stats.ready_remaining_ptb_retry_count,
                runnable_non_retry_ready = stats.runnable_non_retry_ready_count,
                ptb_retry_same_run_excluded = stats.ptb_retry_same_run_excluded_count,
                "TRADE_FLOW_STEP_PROCESS_BOUNDARY_THROTTLED"
            );
        } else if retry_only {
            info!(
                run_id,
                claim_pass,
                max_passes,
                max_passes_reason,
                retry_only,
                claimed_steps_total = stats.claimed_step_count,
                last_pass_claimed_steps = stats.last_pass_claimed_steps,
                ready_remaining_total = stats.ready_remaining_total_count,
                ready_remaining_ptb_retry = stats.ready_remaining_ptb_retry_count,
                runnable_non_retry_ready = stats.runnable_non_retry_ready_count,
                ptb_retry_same_run_excluded = stats.ptb_retry_same_run_excluded_count,
                "TRADE_FLOW_STEP_PROCESS_MAX_PASSES_REACHED"
            );
        } else {
            warn!(
                run_id,
                claim_pass,
                max_passes,
                max_passes_reason,
                retry_only,
                claimed_steps_total = stats.claimed_step_count,
                last_pass_claimed_steps = stats.last_pass_claimed_steps,
                ready_remaining_total = stats.ready_remaining_total_count,
                ready_remaining_ptb_retry = stats.ready_remaining_ptb_retry_count,
                runnable_non_retry_ready = stats.runnable_non_retry_ready_count,
                ptb_retry_same_run_excluded = stats.ptb_retry_same_run_excluded_count,
                "TRADE_FLOW_STEP_PROCESS_MAX_PASSES_REACHED"
            );
        }
    }
    flow_runtime_caches.last_step_processing_stats = stats;

    Ok(())
}

fn node_repeat_mode(node: &TradeFlowNode) -> &str {
    node.config
        .get("repeatMode")
        .and_then(Value::as_str)
        .unwrap_or("loop")
}

fn node_trigger_market_once_scope_version(node: &TradeFlowNode) -> i64 {
    node.config
        .get("onceScopeVersion")
        .and_then(value_as_i64)
        .unwrap_or(0)
}

fn node_uses_legacy_auto_scope_once_scope(node: &TradeFlowNode) -> bool {
    is_trade_flow_market_price_once_node(node)
        && node_market_mode(node) == "auto_scope"
        && node_trigger_market_once_scope_version(node) < TRIGGER_MARKET_ONCE_SCOPE_VERSION_CURRENT
}

fn node_once_scope(node: &TradeFlowNode) -> &str {
    if node_uses_legacy_auto_scope_once_scope(node) {
        return "market";
    }
    match node
        .config
        .get("onceScope")
        .and_then(Value::as_str)
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("market") => "market",
        _ => "run",
    }
}

fn is_trade_flow_market_price_once_scope_market(node: &TradeFlowNode) -> bool {
    is_trade_flow_market_price_once_node(node) && node_once_scope(node) == "market"
}

fn allow_first_tick_threshold_for_ws_node(
    node_spec: &WsOpenPositionPriceNodeSpec,
    previous_price: Option<f64>,
) -> bool {
    if node_spec.node_type != "trigger.market_price" {
        return false;
    }
    // auto_scope market rotation may replay the first in-zone tick only for the
    // explicit `last` or `custom_range` window behavior. `first` mode still requires a real cross.
    if node_spec.auto_scope {
        return matches!(
            node_spec.cycle_window_mode.as_deref(),
            Some("last") | Some("custom_range")
        ) && previous_price.is_none();
    }
    if matches!(
        node_spec.cycle_window_mode.as_deref(),
        Some("last") | Some("custom_range")
    ) {
        return false;
    }
    !node_spec.once_mode || previous_price.is_none()
}

fn is_trade_flow_market_price_once_node(node: &TradeFlowNode) -> bool {
    node.node_type == "trigger.market_price" && node_repeat_mode(node) == "once"
}

fn current_updown_scope_window_start(
    scope_def: UpdownScopeDef,
    now: DateTime<Utc>,
) -> DateTime<Utc> {
    let window_secs = updown_scope_window_seconds(scope_def);
    let now_ts = now.timestamp();
    let base_ts = now_ts - now_ts.rem_euclid(window_secs);
    DateTime::<Utc>::from_timestamp(base_ts, 0).unwrap_or(now)
}

fn is_auto_scope_market_stale_for_current_window(
    scope_def: UpdownScopeDef,
    market_slug: &str,
    now: DateTime<Utc>,
) -> bool {
    let Some(current_market_start) = MarketCycleId(market_slug.to_string()).start_time() else {
        return false;
    };

    current_market_start < current_updown_scope_window_start(scope_def, now)
}

fn should_force_auto_scope_market_cache_refresh(
    node: &TradeFlowNode,
    scope_def: UpdownScopeDef,
    current_market_slug: Option<&str>,
    now: DateTime<Utc>,
) -> bool {
    if node_market_selection(node) != "latest_by_slug" {
        return false;
    }

    let Some(current_market_slug) = current_market_slug.map(str::trim).filter(|v| !v.is_empty())
    else {
        return true;
    };
    is_auto_scope_market_stale_for_current_window(scope_def, current_market_slug, now)
}

/// Check if an auto-scope market has expired based on the slug timestamp.
/// Slug format: "btc-updown-5m-{unix_ts}" or "btc-updown-15m-{unix_ts}".
fn is_auto_scope_market_expired(slug: &str, buffer_secs: i64) -> bool {
    let parts: Vec<&str> = slug.rsplit('-').collect();
    let ts: i64 = match parts.first().and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return false,
    };
    let duration = if slug.contains("-5m-") {
        300
    } else if slug.contains("-15m-") {
        900
    } else {
        return false;
    };
    let now = Utc::now().timestamp();
    now >= ts + duration + buffer_secs
}

/// Cycle window focus: returns true if current time is OUTSIDE the configured sub-window.
/// - mode "first": active window is [start_ts, start_ts + window_secs)
/// - mode "last":  active window is [end_ts - window_secs, end_ts)
fn resolve_updown_market_cycle_bounds(slug: &str) -> Option<(DateTime<Utc>, DateTime<Utc>, i64)> {
    let parts: Vec<&str> = slug.rsplit('-').collect();
    let ts: i64 = parts.first().and_then(|s| s.parse().ok())?;
    let duration = if slug.contains("-5m-") {
        300i64
    } else if slug.contains("-15m-") {
        900i64
    } else {
        return None;
    };
    let start = DateTime::<Utc>::from_timestamp(ts, 0)?;
    let end = start + ChronoDuration::seconds(duration);
    Some((start, end, duration))
}

fn resolve_cycle_window_absolute_bounds(
    market_slug: &str,
    mode: &str,
    window_secs: i64,
    start_sec: Option<i64>,
    end_sec: Option<i64>,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let (start, end, duration) = resolve_updown_market_cycle_bounds(market_slug)?;
    match mode {
        "first" => {
            let effective = window_secs.clamp(1, duration);
            Some((start, start + ChronoDuration::seconds(effective)))
        }
        "last" => {
            let effective = window_secs.clamp(1, duration);
            Some((end - ChronoDuration::seconds(effective), end))
        }
        "custom_range" => {
            let s = start_sec?;
            let e = end_sec?;
            if s >= e || e > duration {
                return None;
            }
            Some((
                start + ChronoDuration::seconds(s),
                start + ChronoDuration::seconds(e),
            ))
        }
        _ => None,
    }
}

fn is_outside_cycle_window_focus(
    slug: &str,
    mode: &str,
    window_secs: i64,
    start_sec: Option<i64>,
    end_sec: Option<i64>,
) -> bool {
    let Some((cycle_start, cycle_end, duration)) = resolve_updown_market_cycle_bounds(slug) else {
        return false;
    };
    let now = Utc::now();
    if now < cycle_start || now >= cycle_end {
        return true;
    }
    if mode != "custom_range" && window_secs >= duration {
        return false;
    }
    let Some((window_open, window_end)) =
        resolve_cycle_window_absolute_bounds(slug, mode, window_secs, start_sec, end_sec)
    else {
        return false;
    };
    now < window_open || now >= window_end
}

fn should_skip_for_cycle_window(
    market_slug: Option<&str>,
    cycle_window_mode: Option<&str>,
    cycle_window_secs: Option<i64>,
    cycle_window_start_sec: Option<i64>,
    cycle_window_end_sec: Option<i64>,
) -> bool {
    let Some(cycle_window_mode) = cycle_window_mode else {
        return false;
    };
    let Some(slug) = market_slug else {
        return true;
    };
    if cycle_window_mode == "custom_range" {
        match (cycle_window_start_sec, cycle_window_end_sec) {
            (Some(_), Some(_)) => is_outside_cycle_window_focus(
                slug,
                cycle_window_mode,
                0,
                cycle_window_start_sec,
                cycle_window_end_sec,
            ),
            _ => true,
        }
    } else {
        match cycle_window_secs {
            Some(window_secs) => {
                is_outside_cycle_window_focus(slug, cycle_window_mode, window_secs, None, None)
            }
            _ => true,
        }
    }
}

fn trade_flow_publish_marker(version: &TradeFlowVersionRuntime) -> String {
    let marker_ts = version
        .published_at
        .unwrap_or(version.created_at)
        .timestamp_millis();
    format!("{}:{marker_ts}", version.id)
}

fn is_fixed_once_market_price_node(node: &TradeFlowNode) -> bool {
    is_trade_flow_market_price_once_node(node) && node_market_mode(node) == "fixed"
}

fn clear_trade_flow_market_price_publish_reset_state(context: &mut Value, node_key: &str) -> bool {
    const EXACT_KEYS: [&str; 17] = [
        FLOW_NODE_STATE_ONCE_FIRED,
        FLOW_NODE_STATE_ONCE_FIRED_AT,
        FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG,
        FLOW_NODE_STATE_ONCE_BLOCK_LOGGED,
        FLOW_NODE_STATE_PUBLISH_AUTO_SCOPE_LOCK_MARKET_SLUG,
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_SLUG,
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_SCOPE,
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_ASSET,
        FLOW_NODE_STATE_AUTO_SCOPE_MARKET_TIMEFRAME,
        FLOW_NODE_STATE_AUTO_SCOPE_YES_TOKEN_ID,
        FLOW_NODE_STATE_AUTO_SCOPE_NO_TOKEN_ID,
        FLOW_NODE_STATE_AUTO_SCOPE_RESOLVED_TOKEN_ID,
        FLOW_NODE_STATE_AUTO_SCOPE_RESOLVED_OUTCOME_LABEL,
        FLOW_NODE_STATE_AUTO_SCOPE_SELECTION_REASON,
        "last_price",
        "last_ws_market_slug",
        "previous_price",
    ];
    const PREFIX_KEYS: [&str; 7] = [
        "previous_price_",
        "price_samples_",
        "cross_pending_at_",
        "cross_pending_price_",
        "cross_pending_prev_",
        FLOW_NODE_STATE_CYCLE_WINDOW_BOUNDARY_MARKER_PREFIX,
        FLOW_NODE_STATE_CYCLE_WINDOW_LAST_EVAL_PREFIX,
    ];

    let Some(node_state) = context.get_mut("nodeState").and_then(Value::as_object_mut) else {
        return false;
    };
    let Some(state_for_node) = node_state.get_mut(node_key).and_then(Value::as_object_mut) else {
        return false;
    };

    let mut changed = false;
    for key in EXACT_KEYS {
        changed |= state_for_node.remove(key).is_some();
    }

    let prefixed_keys: Vec<String> = state_for_node
        .keys()
        .filter(|key| PREFIX_KEYS.iter().any(|prefix| key.starts_with(prefix)))
        .cloned()
        .collect();
    for key in prefixed_keys {
        changed |= state_for_node.remove(&key).is_some();
    }

    let remove_node_entry = state_for_node.is_empty();
    if remove_node_entry {
        node_state.remove(node_key);
        changed = true;
    }

    changed
}

fn sync_trade_flow_once_state_for_publish(
    graph: &TradeFlowGraphRuntime,
    context: &mut Value,
    publish_marker: &str,
) -> (Option<String>, Vec<String>) {
    let previous_marker = flow_state_string(context, FLOW_STATE_PUBLISH_MARKER);
    if previous_marker.as_deref() == Some(publish_marker) {
        return (previous_marker, Vec::new());
    }

    let mut reset_nodes = Vec::new();
    for node in &graph.nodes {
        if !is_trade_flow_market_price_once_node(node) {
            continue;
        }

        if is_fixed_once_market_price_node(node) {
            if clear_trade_flow_market_price_publish_reset_state(context, &node.key) {
                reset_nodes.push(node.key.clone());
            }
            continue;
        }

        if node_market_mode(node) == "auto_scope"
            && !is_trade_flow_market_price_once_scope_market(node)
            && flow_node_state_truthy(context, &node.key, FLOW_NODE_STATE_ONCE_FIRED)
        {
            let current_market_slug =
                node_auto_scope_market_slug(context, &node.key).or_else(|| {
                    flow_node_state_string(
                        context,
                        &node.key,
                        FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG,
                    )
                });
            if let Some(current_market_slug) = current_market_slug
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
            {
                set_flow_node_state(
                    context,
                    &node.key,
                    FLOW_NODE_STATE_PUBLISH_AUTO_SCOPE_LOCK_MARKET_SLUG,
                    json!(current_market_slug),
                );
                remove_flow_node_state(context, &node.key, FLOW_NODE_STATE_ONCE_FIRED);
                remove_flow_node_state(context, &node.key, FLOW_NODE_STATE_ONCE_FIRED_AT);
                remove_flow_node_state(context, &node.key, FLOW_NODE_STATE_ONCE_BLOCK_LOGGED);
                remove_flow_node_state(context, &node.key, FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG);
                reset_nodes.push(node.key.clone());
            }
            continue;
        }

        if node_market_mode(node) == "auto_scope" {
            continue;
        }

        if flow_node_state_truthy(context, &node.key, FLOW_NODE_STATE_ONCE_FIRED) {
            reset_nodes.push(node.key.clone());
        }
        clear_trade_flow_market_price_once_state(context, &node.key);
    }

    set_flow_state(context, FLOW_STATE_PUBLISH_MARKER, json!(publish_marker));
    (previous_marker, reset_nodes)
}

fn node_market_mode(node: &TradeFlowNode) -> &str {
    match node
        .config
        .get("marketMode")
        .and_then(Value::as_str)
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("auto_scope") => "auto_scope",
        _ => "fixed",
    }
}

fn node_market_selection(node: &TradeFlowNode) -> String {
    node_config_string(node, "marketSelection")
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "latest_by_slug".to_string())
}

fn should_accept_ws_market_slug_override(node: &TradeFlowNode, current_market_slug: &str) -> bool {
    node_market_mode(node) != "auto_scope" || current_market_slug.trim().is_empty()
}

fn normalized_binary_outcome_label(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "long" | "bull" => Some("yes"),
        "no" | "down" | "short" | "bear" => Some("no"),
        _ => None,
    }
}

fn resolve_token_id_for_outcome_label(outcome_label: &str, context: &Value) -> Option<String> {
    match normalized_binary_outcome_label(outcome_label) {
        Some("yes") => flow_context_string(context, "yesTokenId"),
        Some("no") => flow_context_string(context, "noTokenId"),
        _ => None,
    }
}

fn resolve_token_id_for_outcome_label_for_node(
    node_key: &str,
    outcome_label: &str,
    context: &Value,
) -> Option<String> {
    match normalized_binary_outcome_label(outcome_label) {
        Some("yes") => node_auto_scope_yes_token_id(context, node_key),
        Some("no") => node_auto_scope_no_token_id(context, node_key),
        _ => None,
    }
}

fn normalize_trigger_protection_mode(raw: Option<&str>) -> &'static str {
    match raw
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM => TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM,
        _ => TRIGGER_PROTECTION_MODE_OFF,
    }
}

fn normalize_trigger_protection_preset(raw: Option<&str>) -> &'static str {
    match raw
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        TRIGGER_PROTECTION_PRESET_LOOSE => TRIGGER_PROTECTION_PRESET_LOOSE,
        TRIGGER_PROTECTION_PRESET_STRICT => TRIGGER_PROTECTION_PRESET_STRICT,
        _ => TRIGGER_PROTECTION_PRESET_BALANCED,
    }
}

fn underlying_reference_symbol(asset: &str) -> Option<&'static str> {
    match asset.trim().to_ascii_lowercase().as_str() {
        "btc" => Some("BTC-USD"),
        "eth" => Some("ETH-USD"),
        "sol" => Some("SOL-USD"),
        "xrp" => Some("XRP-USD"),
        _ => None,
    }
}

fn resolve_auto_scope_underlying_asset(
    node: &TradeFlowNode,
    context: &Value,
    market_slug: Option<&str>,
) -> Option<String> {
    node_config_string(node, "marketScope")
        .or_else(|| node_auto_scope_market_scope(node, context))
        .or_else(|| flow_context_string(context, "marketScope"))
        .as_deref()
        .and_then(find_updown_scope_by_scope)
        .map(|scope| scope.asset.to_string())
        .or_else(|| node_auto_scope_market_asset(context, &node.key))
        .or_else(|| flow_context_string(context, "marketAsset"))
        .or_else(|| {
            market_slug
                .and_then(find_updown_scope_by_slug)
                .map(|scope| scope.asset.to_string())
        })
}

fn resolve_underlying_direction_label(outcome_label: &str) -> Option<&'static str> {
    match normalized_binary_outcome_label(outcome_label) {
        Some("yes") => Some("up"),
        Some("no") => Some("down"),
        _ => None,
    }
}

fn direction_multiplier(direction: &str) -> Option<f64> {
    match direction.trim().to_ascii_lowercase().as_str() {
        "up" => Some(1.0),
        "down" => Some(-1.0),
        _ => None,
    }
}

fn parse_trigger_price_samples(value: Option<&Value>) -> Vec<TriggerPriceSample> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let Some(ts_ms) = obj.get("ts_ms").and_then(value_as_i64) else {
            continue;
        };
        let Some(price) = obj.get("price").and_then(value_as_f64) else {
            continue;
        };
        if !price.is_finite() || price < 0.0 || price > 1.0 {
            continue;
        }
        out.push(TriggerPriceSample { ts_ms, price });
    }
    out.sort_by_key(|sample| sample.ts_ms);
    out
}

fn record_trigger_price_sample(
    context: &mut Value,
    node_key: &str,
    token_id: &str,
    price: f64,
    now: DateTime<Utc>,
) -> Option<f64> {
    let state_key = format!("price_samples_{}", token_id);
    let now_ms = now.timestamp_millis();
    let mut samples = parse_trigger_price_samples(flow_node_state(context, node_key, &state_key));
    samples.push(TriggerPriceSample {
        ts_ms: now_ms,
        price,
    });
    let cutoff = now_ms.saturating_sub((UNDERLYING_REFERENCE_HISTORY_WINDOW_SECS as i64) * 1000);
    samples.retain(|sample| sample.ts_ms >= cutoff);
    if samples.len() > 120 {
        let overflow = samples.len() - 120;
        samples.drain(0..overflow);
    }
    let delta_10s_cent = samples
        .iter()
        .rev()
        .find(|sample| sample.ts_ms <= now_ms.saturating_sub(10_000))
        .map(|sample| (price - sample.price) * 100.0);
    set_flow_node_state(
        context,
        node_key,
        &state_key,
        json!(samples
            .iter()
            .map(|sample| json!({
                "ts_ms": sample.ts_ms,
                "price": sample.price
            }))
            .collect::<Vec<Value>>()),
    );
    delta_10s_cent
}

fn underlying_delta_pct_from_ticks(
    ticks: &VecDeque<UnderlyingTick>,
    current_ts: DateTime<Utc>,
    current_price: f64,
    seconds: i64,
) -> Option<f64> {
    let target_ts = current_ts - ChronoDuration::seconds(seconds);
    let previous = ticks
        .iter()
        .rev()
        .find(|sample| sample.ts <= target_ts)
        .or_else(|| ticks.front())?;
    if previous.price <= 0.0 {
        return None;
    }
    Some(((current_price - previous.price) / previous.price) * 100.0)
}

fn build_underlying_protection_config(
    node: &TradeFlowNode,
    context: &Value,
    market_slug: &str,
    outcome_label: &str,
) -> Option<UnderlyingProtectionConfig> {
    let mode = normalize_trigger_protection_mode(
        node.config.get("protectionMode").and_then(Value::as_str),
    );
    if mode != TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM {
        return None;
    }
    if node_market_mode(node) != "auto_scope" {
        return None;
    }
    let asset = resolve_auto_scope_underlying_asset(node, context, Some(market_slug))?;
    let direction = resolve_underlying_direction_label(outcome_label)?.to_string();
    let reference_symbol = underlying_reference_symbol(&asset)?.to_string();
    Some(UnderlyingProtectionConfig {
        mode: mode.to_string(),
        preset: normalize_trigger_protection_preset(
            node.config.get("protectionPreset").and_then(Value::as_str),
        )
        .to_string(),
        asset,
        direction,
        reference_symbol,
    })
}

fn parse_underlying_protection_config(value: Option<Value>) -> Option<UnderlyingProtectionConfig> {
    let value = value?;
    let obj = value.as_object()?;
    let mode = normalize_trigger_protection_mode(obj.get("mode").and_then(Value::as_str));
    if mode != TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM {
        return None;
    }
    let asset = obj
        .get("asset")
        .and_then(Value::as_str)?
        .trim()
        .to_ascii_lowercase();
    let direction = obj
        .get("direction")
        .and_then(Value::as_str)?
        .trim()
        .to_ascii_lowercase();
    let reference_symbol = obj
        .get("reference_symbol")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| underlying_reference_symbol(&asset).map(str::to_string))?;
    Some(UnderlyingProtectionConfig {
        mode: mode.to_string(),
        preset: normalize_trigger_protection_preset(obj.get("preset").and_then(Value::as_str))
            .to_string(),
        asset,
        direction,
        reference_symbol,
    })
}

async fn evaluate_underlying_protection(
    config: &UnderlyingProtectionConfig,
    market_slug: &str,
    poly_delta_10s_cent: Option<f64>,
) -> UnderlyingProtectionEvaluation {
    let base = UnderlyingProtectionEvaluation {
        mode: config.mode.clone(),
        preset: config.preset.clone(),
        asset: config.asset.clone(),
        direction: config.direction.clone(),
        reference_feed: "coinbase_spot".to_string(),
        reference_symbol: config.reference_symbol.clone(),
        passed: false,
        reason_code: "reference_fetch_failed".to_string(),
        reason_detail: None,
        cycle_open_price: None,
        current_price: None,
        delta_10s_pct: None,
        delta_30s_pct: None,
        poly_delta_10s_cent,
        divergence_blocked: false,
    };
    let multiplier = match direction_multiplier(&config.direction) {
        Some(value) => value,
        None => {
            return UnderlyingProtectionEvaluation {
                reason_code: "unsupported_direction".to_string(),
                reason_detail: Some(config.direction.clone()),
                ..base
            }
        }
    };
    let snapshot = match UNDERLYING_REFERENCE_SERVICE
        .snapshot(&config.asset, market_slug)
        .await
    {
        Ok(value) => value,
        Err(err) => {
            return UnderlyingProtectionEvaluation {
                reason_code: "reference_data_unready".to_string(),
                reason_detail: Some(err.to_string()),
                ..base
            }
        }
    };

    let mut evaluation = UnderlyingProtectionEvaluation {
        cycle_open_price: Some(snapshot.cycle_open_price),
        current_price: Some(snapshot.current_price),
        delta_10s_pct: snapshot.delta_10s_pct,
        delta_30s_pct: snapshot.delta_30s_pct,
        reason_code: "passed".to_string(),
        passed: true,
        ..base
    };

    if ((snapshot.current_price - snapshot.cycle_open_price) * multiplier) <= 0.0 {
        evaluation.passed = false;
        evaluation.reason_code = "cycle_open_mismatch".to_string();
        return evaluation;
    }

    let directional_delta_10s = match snapshot.delta_10s_pct {
        Some(value) => value * multiplier,
        None => {
            evaluation.passed = false;
            evaluation.reason_code = "reference_data_unready".to_string();
            return evaluation;
        }
    };

    match config.preset.as_str() {
        TRIGGER_PROTECTION_PRESET_LOOSE => {
            if directional_delta_10s <= 0.0 {
                evaluation.passed = false;
                evaluation.reason_code = "delta_10s_mismatch".to_string();
            }
            return evaluation;
        }
        TRIGGER_PROTECTION_PRESET_STRICT => {
            if directional_delta_10s < UNDERLYING_REFERENCE_STRICT_DELTA_10S_PCT {
                evaluation.passed = false;
                evaluation.reason_code = "delta_10s_mismatch".to_string();
                return evaluation;
            }
        }
        _ => {
            if directional_delta_10s <= 0.0 {
                evaluation.passed = false;
                evaluation.reason_code = "delta_10s_mismatch".to_string();
                return evaluation;
            }
        }
    }

    let directional_delta_30s = match snapshot.delta_30s_pct {
        Some(value) => value * multiplier,
        None => {
            evaluation.passed = false;
            evaluation.reason_code = "reference_data_unready".to_string();
            return evaluation;
        }
    };

    match config.preset.as_str() {
        TRIGGER_PROTECTION_PRESET_STRICT => {
            if directional_delta_30s < UNDERLYING_REFERENCE_STRICT_DELTA_30S_PCT {
                evaluation.passed = false;
                evaluation.reason_code = "delta_30s_mismatch".to_string();
                return evaluation;
            }
        }
        _ => {
            if directional_delta_30s <= 0.0 {
                evaluation.passed = false;
                evaluation.reason_code = "delta_30s_mismatch".to_string();
                return evaluation;
            }
        }
    }

    let divergence_guard_enabled = config.preset == TRIGGER_PROTECTION_PRESET_BALANCED
        || config.preset == TRIGGER_PROTECTION_PRESET_STRICT;
    if divergence_guard_enabled {
        if let Some(poly_delta) = poly_delta_10s_cent {
            let directional_poly_delta = poly_delta * multiplier;
            let abs_delta_10s = snapshot.delta_10s_pct.unwrap_or_default().abs();
            if directional_poly_delta >= UNDERLYING_REFERENCE_POLY_DIVERGENCE_CENT
                && (directional_delta_10s <= 0.0
                    || abs_delta_10s < UNDERLYING_REFERENCE_BALANCED_FLAT_DELTA_PCT)
            {
                evaluation.passed = false;
                evaluation.reason_code = "divergence_detected".to_string();
                evaluation.divergence_blocked = true;
                return evaluation;
            }
        }
    }

    evaluation
}

async fn sync_trigger_market_auto_scope_context(
    cfg: &AppConfig,
    node: &TradeFlowNode,
    context: &mut Value,
) -> Result<Option<SelectedLiveMarket>> {
    if node_market_mode(node) != "auto_scope" {
        return Ok(None);
    }

    let market_scope = node_config_string(node, "marketScope")
        .or_else(|| node_auto_scope_market_scope(node, context))
        .or_else(|| flow_context_string(context, "marketScope"))
        .ok_or_else(|| anyhow::anyhow!("trigger.market_price auto_scope requires marketScope"))?;
    let scope_def = find_updown_scope_by_scope(&market_scope).ok_or_else(|| {
        anyhow::anyhow!(
            "trigger.market_price auto_scope unsupported marketScope={market_scope} (supported: {})",
            SUPPORTED_UPDOWN_SCOPE_DEFS
                .iter()
                .map(|def| def.scope)
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    let market_selection = node_market_selection(node);
    let current_market_slug = node_auto_scope_market_slug(context, &node.key);
    let now = Utc::now();
    let force_refresh = should_force_auto_scope_market_cache_refresh(
        node,
        scope_def,
        current_market_slug.as_deref(),
        now,
    );
    let markets = {
        let cache_hit = if force_refresh {
            None
        } else {
            AUTO_SCOPE_MARKET_CACHE
                .lock()
                .unwrap()
                .get(&market_scope)
                .filter(|(t, _)| {
                    t.elapsed() < std::time::Duration::from_secs(AUTO_SCOPE_CACHE_TTL_SECS)
                })
                .map(|(_, m)| m.clone())
        };
        if let Some(cached) = cache_hit {
            cached
        } else {
            let gamma = GammaHttpClient::new(cfg.exchange.gamma_base_url.clone());
            let fresh = match list_markets_for_scope(&gamma, &market_scope).await {
                Ok(fresh) => fresh,
                Err(err) => {
                    if let Some(selected) = selected_live_market_from_trigger_node_state_cache(
                        context, &node.key, scope_def, now,
                    ) {
                        set_trigger_node_auto_scope_context(
                            context,
                            &node.key,
                            scope_def.scope,
                            scope_def.asset,
                            scope_def.timeframe,
                            &selected,
                            node_config_string(node, "outcomeLabel").as_deref(),
                        );
                        return Ok(Some(selected));
                    }
                    return Err(err);
                }
            };
            AUTO_SCOPE_MARKET_CACHE
                .lock()
                .unwrap()
                .insert(market_scope.clone(), (Instant::now(), fresh.clone()));
            fresh
        }
    };
    let selected = select_market_from_candidates(markets, None, &market_selection, true);
    let Some(selected) = selected else {
        clear_trigger_node_auto_scope_context(context, &node.key);
        return Ok(None);
    };

    set_trigger_node_auto_scope_context(
        context,
        &node.key,
        scope_def.scope,
        scope_def.asset,
        scope_def.timeframe,
        &selected,
        node_config_string(node, "outcomeLabel").as_deref(),
    );

    Ok(Some(selected))
}

fn selected_live_market_from_trigger_node_state_cache(
    context: &Value,
    node_key: &str,
    scope_def: UpdownScopeDef,
    now: DateTime<Utc>,
) -> Option<SelectedLiveMarket> {
    let slug = node_auto_scope_market_slug(context, node_key)?;
    let yes_token_id = node_auto_scope_yes_token_id(context, node_key)?;
    let no_token_id = node_auto_scope_no_token_id(context, node_key)?;
    let slug_scope = find_updown_scope_by_slug(&slug)?;
    if slug_scope.scope != scope_def.scope
        || is_auto_scope_market_stale_for_current_window(scope_def, &slug, now)
    {
        return None;
    }
    Some(SelectedLiveMarket {
        slug,
        yes_token_id: Some(yes_token_id),
        no_token_id: Some(no_token_id),
        maker_base_fee: 0,
        starts_at: None,
        ends_at: None,
        selection_reason: LiveMarketSelectionReason::TriggerNodeStateCache,
    })
}
