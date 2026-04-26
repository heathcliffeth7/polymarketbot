async fn refresh_trade_flow_ws_fast_path_cache(
    repo: &PostgresRepository,
    run_id: i64,
    ws: &ClobWsClient,
    definitions: &[TradeFlowDefinitionRuntime],
    user_cfg_cache: &mut HashMap<i64, AppConfig>,
) -> Result<()> {
    if definitions.is_empty() {
        {
            let mut cache = TRADE_FLOW_WS_FAST_PATH_CACHE.write().await;
            *cache = TradeFlowWsFastPathCache::default();
        }
        if let Err(err) = ensure_fast_path_market_stream_union(ws).await {
            warn!(run_id, error = %err, "TRADE_FLOW_WS_STREAM_UNION_CLEAR_FAILED");
        }
        return Ok(());
    }

    let mut fast_path_cache =
        build_trade_flow_ws_fast_path_cache(repo, run_id, definitions, user_cfg_cache).await?;
    persist_trade_flow_ws_run_specs_contexts(repo, &mut fast_path_cache.run_specs).await?;
    let run_count = fast_path_cache.run_specs.len();
    let token_count = fast_path_cache.token_targets.len();
    {
        let mut cache = TRADE_FLOW_WS_FAST_PATH_CACHE.write().await;
        *cache = fast_path_cache;
    }
    if let Err(err) = ensure_fast_path_market_stream_union(ws).await {
        warn!(
            run_id,
            token_count,
            error = %err,
            "TRADE_FLOW_WS_STREAM_UNION_ENSURE_FAILED"
        );
    }
    info!(
        run_id,
        runs = run_count,
        tokens = token_count,
        "TRADE_FLOW_WS_FAST_PATH_CACHE_REFRESHED"
    );
    Ok(())
}
async fn refresh_trade_flow_ws_fast_path_for_boundary(
    repo: &PostgresRepository,
    run_id: i64,
    ws: &ClobWsClient,
    user_cfg_cache: &mut HashMap<i64, AppConfig>,
) -> Result<()> {
    let definitions = repo
        .list_published_trade_flow_definitions(FLOW_DEFINITION_PROCESS_LIMIT)
        .await?;
    refresh_trade_flow_ws_fast_path_cache(repo, run_id, ws, &definitions, user_cfg_cache).await
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AutoScopeMarketRotation {
    node_key: String,
    old_market_slug: String,
    new_market_slug: String,
    expected_market_start: Option<DateTime<Utc>>,
    rotation_detected_at: DateTime<Utc>,
    selection_reason: Option<String>,
}

fn auto_scope_rotation_lag_ms(rotation: &AutoScopeMarketRotation) -> Option<i64> {
    rotation.expected_market_start.map(|expected_start| {
        rotation
            .rotation_detected_at
            .signed_duration_since(expected_start)
            .num_milliseconds()
            .max(0)
    })
}

const FLOW_NODE_STATE_CYCLE_WINDOW_CONFIG_SNAPSHOT_PREFIX: &str = "cycle_window_config_snapshot_";

fn cycle_window_config_snapshot_state_key(token_id: &str) -> String {
    format!("{FLOW_NODE_STATE_CYCLE_WINDOW_CONFIG_SNAPSHOT_PREFIX}{token_id}")
}

fn build_chainlink_seed_rejected_too_old_payload(
    market_slug: &str,
    asset: &str,
    timeframe: &str,
    expected_market_start: &DateTime<Utc>,
    details: &crate::trade_flow::guards::chainlink_price::ChainlinkNearTimestampRejectionDetails,
) -> Value {
    json!({
        "market_slug": market_slug,
        "asset": asset,
        "timeframe": timeframe,
        "expected_market_start": expected_market_start.to_rfc3339(),
        "gap_ms": details.gap_ms,
        "provider_age_ms": details.provider_age_ms,
        "candidate_timestamp_ms": details.candidate_timestamp_ms,
        "candidate_received_at_ms": details.candidate_received_at_ms,
    })
}

fn clear_trade_flow_market_price_ws_runtime_state(context: &mut Value, node_key: &str) -> bool {
    let mut changed = false;
    let Some(node_state) = context
        .get_mut("nodeState")
        .and_then(Value::as_object_mut)
        .and_then(|node_state| node_state.get_mut(node_key))
        .and_then(Value::as_object_mut)
    else {
        return false;
    };

    for key in ["last_price", "previous_price"] {
        if node_state.remove(key).is_some() {
            changed = true;
        }
    }

    let prefixed_keys = node_state
        .keys()
        .filter(|key| {
            key.starts_with("previous_price_")
                || key.starts_with("cross_pending_at_")
                || key.starts_with("cross_pending_price_")
                || key.starts_with("cross_pending_prev_")
                || key.starts_with(FLOW_NODE_STATE_CYCLE_WINDOW_BOUNDARY_MARKER_PREFIX)
                || key.starts_with(FLOW_NODE_STATE_CYCLE_WINDOW_LAST_EVAL_PREFIX)
        })
        .cloned()
        .collect::<Vec<_>>();
    for key in prefixed_keys {
        if node_state.remove(&key).is_some() {
            changed = true;
        }
    }

    changed
}

async fn replace_trade_flow_ws_fast_path_run_context(run_id: i64, context: &Value) -> bool {
    let mut cache = TRADE_FLOW_WS_FAST_PATH_CACHE.write().await;
    let mut replaced = false;
    for run_spec in &mut cache.run_specs {
        if run_spec.run_id != run_id {
            continue;
        }
        run_spec.context = context.clone();
        run_spec.context_dirty = false;
        replaced = true;
    }
    replaced
}

fn sync_trade_flow_auto_scope_market_rollover_state(
    previous_context: &Value,
    context: &mut Value,
    node_specs: &[WsOpenPositionPriceNodeSpec],
    selected_markets_by_node: &HashMap<String, SelectedLiveMarket>,
    rotation_detected_at: DateTime<Utc>,
) -> Vec<AutoScopeMarketRotation> {
    let mut processed = HashSet::new();
    let mut rotations = Vec::new();

    for node_spec in node_specs {
        if !node_spec.auto_scope || node_spec.node_type != "trigger.market_price" {
            continue;
        }
        if !processed.insert(node_spec.node_key.clone()) {
            continue;
        }

        let Some(new_market_slug) = node_spec
            .market_slug
            .as_deref()
            .map(str::trim)
            .filter(|slug| !slug.is_empty())
        else {
            continue;
        };

        let previous_last_ws_slug =
            flow_node_state_string(previous_context, &node_spec.node_key, "last_ws_market_slug")
                .filter(|slug| !slug.trim().is_empty());
        let previous_node_market_slug =
            node_auto_scope_market_slug(previous_context, &node_spec.node_key)
                .map(|slug| slug.trim().to_string())
                .filter(|slug| !slug.is_empty());
        let previous_flow_slug = previous_context
            .pointer("/flowContext/marketSlug")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|slug| !slug.is_empty())
            .map(str::to_string);
        let old_market_slug = previous_last_ws_slug
            .or(previous_node_market_slug)
            .or(previous_flow_slug);

        if old_market_slug.as_deref() != Some(new_market_slug) {
            clear_trade_flow_market_price_ws_runtime_state(context, &node_spec.node_key);
            clear_trade_flow_market_price_rotation_state(context, &node_spec.node_key);
            if let Some(old_market_slug) = old_market_slug {
                rotations.push(AutoScopeMarketRotation {
                    node_key: node_spec.node_key.clone(),
                    old_market_slug,
                    new_market_slug: new_market_slug.to_string(),
                    expected_market_start: MarketCycleId(new_market_slug.to_string()).start_time(),
                    rotation_detected_at,
                    selection_reason: selected_markets_by_node
                        .get(&node_spec.node_key)
                        .map(|selected| selected.selection_reason.as_str().to_string()),
                });
            }
        }

        set_flow_node_state(
            context,
            &node_spec.node_key,
            "last_ws_market_slug",
            json!(new_market_slug),
        );
    }

    rotations
}

async fn trade_flow_ws_fast_path_cache_requires_refresh_now() -> bool {
    let cache = TRADE_FLOW_WS_FAST_PATH_CACHE.read().await;
    if cache.run_specs.is_empty() {
        return false;
    }

    let now = Utc::now();
    cache.run_specs.iter().any(|run_spec| {
        run_spec.nodes.iter().any(|node_spec| {
            if !node_spec.auto_scope {
                return false;
            }
            let Some(market_slug) = node_spec.market_slug.as_deref() else {
                return false;
            };
            let Some(scope_def) = find_updown_scope_by_slug(market_slug) else {
                return false;
            };
            is_auto_scope_market_stale_for_current_window(scope_def, market_slug, now)
        })
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedWsFastPathTarget {
    run_index: usize,
    node_index: usize,
    dirty_token_id: Option<String>,
    reevaluation_reason: &'static str,
}

async fn enqueue_trade_flow_ws_open_position_price_steps_from_cache(
    repo: &PostgresRepository,
    run_id: i64,
    ws: &ClobWsClient,
    client: Option<&dyn OrderExecutor>,
    dirty_token_ids: Option<&[String]>,
) -> Result<bool> {
    let cache_snapshot = {
        let cache = TRADE_FLOW_WS_FAST_PATH_CACHE.read().await;
        cache.clone()
    };
    if cache_snapshot.run_specs.is_empty() {
        return Ok(false);
    }

    let selected_targets = select_ws_fast_path_targets(run_id, &cache_snapshot, dirty_token_ids);
    if selected_targets.is_empty() {
        return Ok(false);
    }
    for selected_target in &selected_targets {
        let Some(run_spec) = cache_snapshot.run_specs.get(selected_target.run_index) else {
            continue;
        };
        let Some(node_spec) = run_spec.nodes.get(selected_target.node_index) else {
            continue;
        };
        if node_spec.node_type != "trigger.market_price" {
            continue;
        }
        let fields = build_trigger_ws_target_log_fields(
            node_spec,
            selected_target.dirty_token_id.as_deref(),
            selected_target.reevaluation_reason,
            node_spec.market_slug.as_deref(),
        );
        log_trigger_ws_target_selected(run_id, run_spec.run_id, &fields);
    }

    let mut run_specs = cache_snapshot.run_specs;
    let token_targets = cache_snapshot.token_targets;
    let market_targets = cache_snapshot.market_targets;
    let snapshot_token_ids = selected_targets
        .iter()
        .filter_map(|target| {
            run_specs
                .get(target.run_index)
                .and_then(|run_spec| run_spec.nodes.get(target.node_index))
                .map(|node_spec| node_spec.token_id.clone())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let market_snapshots = ws.get_market_snapshots(&snapshot_token_ids).await;
    let mut touched = false;

    for selected_target in selected_targets {
        let Some(node_spec) = run_specs
            .get(selected_target.run_index)
            .and_then(|run_spec| run_spec.nodes.get(selected_target.node_index))
            .cloned()
        else {
            continue;
        };
        let selected_token_id = node_spec.token_id.clone();
        let resolved_market_slug = node_spec.market_slug.clone();
        let target_log_fields = build_trigger_ws_target_log_fields(
            &node_spec,
            selected_target.dirty_token_id.as_deref(),
            selected_target.reevaluation_reason,
            resolved_market_slug.as_deref(),
        );
        let flow_run_id = run_specs
            .get(selected_target.run_index)
            .map(|run_spec| run_spec.run_id);
        if let Some(flow_run_id) = flow_run_id {
            log_trigger_ws_target_started(run_id, flow_run_id, &target_log_fields);
        }

        let resolved = market_snapshots
            .get(&selected_token_id)
            .and_then(|snapshot| {
                resolve_trigger_price_from_market_snapshot(
                    snapshot,
                    node_spec.price_mode,
                    Some(node_spec.trigger_condition.as_str()),
                )
            });
        let resolved_price = if let Some(resolved) = resolved {
            resolved
        } else if let Some(cl) = client {
            match resolve_trigger_price_from_rest(
                cl,
                &selected_token_id,
                node_spec.price_mode,
                Some(node_spec.trigger_condition.as_str()),
            )
            .await
            {
                Ok(resolved) => resolved,
                Err(_) => {
                    debug!(
                        run_id,
                        node_key = %node_spec.node_key,
                        token_id = %node_spec.token_id,
                        dirty_token_id = ?selected_target.dirty_token_id,
                        ws_reevaluation_reason = selected_target.reevaluation_reason,
                        resolved_market_slug = ?node_spec.market_slug,
                        market = ?node_spec.market_slug,
                        "TRIGGER_WS_NO_PRICE_DATA_NO_REST"
                    );
                    continue;
                }
            }
        } else {
            debug!(
                run_id,
                node_key = %node_spec.node_key,
                token_id = %node_spec.token_id,
                dirty_token_id = ?selected_target.dirty_token_id,
                ws_reevaluation_reason = selected_target.reevaluation_reason,
                resolved_market_slug = ?node_spec.market_slug,
                market = ?node_spec.market_slug,
                "TRIGGER_WS_NO_PRICE_DATA"
            );
            continue;
        };
        let current_price = resolved_price.price;
        let event_ts = resolved_price.ts;
        let price_source = resolved_price.source.clone();
        let price_detail = resolved_price.detail;
        let price_source_detail = price_detail.source_detail.clone();
        let price_best_bid = price_detail.best_bid;
        let price_best_ask = price_detail.best_ask;
        let price_last_trade = price_detail.last_trade_price;
        let price_snapshot_age_ms = price_detail.snapshot_age_ms;
        let price_site_display_decision = price_detail.site_display_mode_decision;

        let Some(run_spec) = run_specs.get_mut(selected_target.run_index) else {
            log_trigger_ws_target_dropped(
                run_id,
                flow_run_id,
                "missing_run_spec",
                &target_log_fields,
            );
            continue;
        };
        touched = true;
        if node_spec.auto_scope {
            if let Some(ref slug) = node_spec.market_slug {
                if is_auto_scope_market_expired(slug, 15) {
                    log_trigger_ws_target_skipped(
                        run_id,
                        run_spec.run_id,
                        "market_expired",
                        &target_log_fields,
                    );
                    continue;
                }
            }
        }
        if current_price < 0.03 || current_price > 0.97 {
            debug!(
                run_id,
                flow_run_id = run_spec.run_id,
                node_key = %node_spec.node_key,
                price = current_price,
                token_id = %node_spec.token_id,
                dirty_token_id = ?selected_target.dirty_token_id,
                ws_reevaluation_reason = selected_target.reevaluation_reason,
                resolved_market_slug = ?node_spec.market_slug,
                market = ?node_spec.market_slug,
                price_source = %price_source,
                price_source_detail = %price_source_detail,
                best_bid = ?price_best_bid,
                best_ask = ?price_best_ask,
                last_trade_price = ?price_last_trade,
                snapshot_age_ms = ?price_snapshot_age_ms,
                site_display_mode_decision = ?price_site_display_decision,
                "TRIGGER_SKIP_RESOLUTION_PRICE"
            );
            continue;
        }
        if should_skip_for_cycle_window(
            node_spec.market_slug.as_deref(),
            node_spec.cycle_window_mode.as_deref(),
            node_spec.cycle_window_secs,
            node_spec.cycle_window_start_sec,
            node_spec.cycle_window_end_sec,
        ) {
            debug!(
                run_id,
                flow_run_id = run_spec.run_id,
                node_key = %node_spec.node_key,
                dirty_token_id = ?selected_target.dirty_token_id,
                ws_reevaluation_reason = selected_target.reevaluation_reason,
                resolved_market_slug = ?node_spec.market_slug,
                market_slug = ?node_spec.market_slug,
                cycle_window_mode = ?node_spec.cycle_window_mode,
                cycle_window_secs = ?node_spec.cycle_window_secs,
                auto_scope = node_spec.auto_scope,
                "TRIGGER_SKIP_CYCLE_WINDOW_FOCUS"
            );
            continue;
        }
        if node_spec.protection_mode == TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM {
            if let Some(asset) = node_spec.protection_asset.as_deref() {
                if let Err(err) = UNDERLYING_REFERENCE_SERVICE.prime(asset).await {
                    debug!(
                        run_id,
                        flow_run_id = run_spec.run_id,
                        node_key = %node_spec.node_key,
                        asset = %asset,
                        error = %err,
                        "TRIGGER_UNDERLYING_PRIME_FAILED"
                    );
                }
            }
        }
        sync_trade_flow_market_price_once_scope_state(
            &mut run_spec.context,
            &node_spec.node_key,
            node_spec.once_scope_market,
            node_spec.market_slug.as_deref(),
        );
        if let Some(current_slug) = node_spec.market_slug.as_deref() {
            let last_slug = flow_node_state_string(
                &run_spec.context,
                &node_spec.node_key,
                "last_ws_market_slug",
            );
            let slug_changed = last_slug
                .as_deref()
                .map(|last| last != current_slug)
                .unwrap_or(true);
            if slug_changed {
                let prev_key = format!("previous_price_{}", node_spec.token_id);
                remove_flow_node_state(&mut run_spec.context, &node_spec.node_key, &prev_key);
                remove_flow_node_state(
                    &mut run_spec.context,
                    &node_spec.node_key,
                    "previous_price",
                );
                let cpend_at = format!("cross_pending_at_{}", node_spec.token_id);
                let cpend_price = format!("cross_pending_price_{}", node_spec.token_id);
                let cpend_prev = format!("cross_pending_prev_{}", node_spec.token_id);
                remove_flow_node_state(&mut run_spec.context, &node_spec.node_key, &cpend_at);
                remove_flow_node_state(&mut run_spec.context, &node_spec.node_key, &cpend_price);
                remove_flow_node_state(&mut run_spec.context, &node_spec.node_key, &cpend_prev);
                run_spec.context_dirty = true;
            }
            set_flow_node_state(
                &mut run_spec.context,
                &node_spec.node_key,
                "last_ws_market_slug",
                json!(current_slug),
            );
            if slug_changed {
                run_spec.context_dirty = true;
            }
        }
        notify_trade_flow_realtime_price_tick(
            repo,
            run_spec.run_id,
            run_spec.definition_id,
            run_spec.version_id,
            &node_spec.node_key,
            &node_spec.node_type,
            node_spec.market_slug.as_deref(),
            &node_spec.token_id,
            &node_spec.outcome_label,
            current_price,
            node_spec.price_mode,
            &price_source,
            &price_source_detail,
            price_best_bid,
            price_best_ask,
            price_last_trade,
            price_snapshot_age_ms,
            event_ts,
            None,
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
            log_trigger_ws_target_skipped(
                run_id,
                run_spec.run_id,
                "once_fired_for_scope",
                &target_log_fields,
            );
            if !flow_node_state_truthy(
                &run_spec.context,
                &node_spec.node_key,
                FLOW_NODE_STATE_ONCE_BLOCK_LOGGED,
            ) {
                set_flow_node_state(
                    &mut run_spec.context,
                    &node_spec.node_key,
                    FLOW_NODE_STATE_ONCE_BLOCK_LOGGED,
                    json!(true),
                );
                run_spec.context_dirty = true;
                if let Err(err) = repo
                    .append_trade_flow_event(
                        Some(run_spec.run_id),
                        run_spec.definition_id,
                        Some(run_spec.version_id),
                        "trigger_once_blocked",
                        &json!({
                            "node_key": node_spec.node_key,
                            "node_type": node_spec.node_type,
                            "token_id": selected_token_id.clone(),
                            "reason": "ws_enqueue_once_fired",
                            "once_scope": if node_spec.once_scope_market { "market" } else { "run" },
                            "market_slug": resolved_market_slug.clone()
                        }),
                    )
                    .await
                {
                    warn!(
                        run_id,
                        flow_run_id = run_spec.run_id,
                        node_key = %node_spec.node_key,
                        error = %err,
                        "TRADE_FLOW_ONCE_BLOCK_EVENT_FAILED"
                    );
                }
            }
            continue;
        }

        let prev_key = format!("previous_price_{}", selected_token_id);
        let previous_price = flow_node_state(&run_spec.context, &node_spec.node_key, &prev_key)
            .and_then(value_as_f64);
        let allow_first_tick_threshold =
            allow_first_tick_threshold_for_ws_node(&node_spec, previous_price);
        let ptb_config = trigger_market_price_ptb_config_from_spec(&node_spec);
        let gate_mode = trigger_market_price_gate_mode(&node_spec.trigger_condition, ptb_config);
        let (crossed, evaluation_mode) = if node_spec.pair_lock_only_monitor {
            (true, "pair_lock_only")
        } else if let Some(gate_mode) = gate_mode {
            if matches!(
                gate_mode,
                TriggerMarketPriceGateMode::StandardOnly | TriggerMarketPriceGateMode::StandardAndPtb
            ) {
                let (crossed, evaluation_mode) = evaluate_trigger_market_price_condition(
                    previous_price,
                    current_price,
                    node_spec.trigger_price,
                    &node_spec.trigger_condition,
                    allow_first_tick_threshold,
                    node_spec.max_price,
                );

                if !crossed && evaluation_mode == "no_previous" {
                    debug!(
                        run_id,
                        flow_run_id = run_spec.run_id,
                        node_key = %node_spec.node_key,
                        price = current_price,
                        trigger_price = node_spec.trigger_price,
                        trigger_condition = %node_spec.trigger_condition,
                        once_mode = node_spec.once_mode,
                        token_id = %node_spec.token_id,
                        dirty_token_id = ?selected_target.dirty_token_id,
                        ws_reevaluation_reason = selected_target.reevaluation_reason,
                        resolved_market_slug = ?node_spec.market_slug,
                        market = ?node_spec.market_slug,
                        "TRIGGER_WS_NO_PREVIOUS_PRICE"
                    );
                }
                if !crossed && evaluation_mode != "no_previous" {
                    log_trigger_ws_condition_not_met(
                        run_id,
                        run_spec.run_id,
                        build_trigger_ws_condition_not_met_log_fields(
                            &node_spec,
                            current_price,
                            previous_price,
                            evaluation_mode,
                            gate_mode,
                            &price_source,
                            &price_source_detail,
                            price_best_bid,
                            price_best_ask,
                            price_last_trade,
                            price_snapshot_age_ms,
                            selected_target.dirty_token_id.as_deref(),
                            selected_target.reevaluation_reason,
                            resolved_market_slug.as_deref(),
                        ),
                    );
                }
                (crossed, evaluation_mode)
            } else {
                (false, "ptb_only")
            }
        } else {
            log_trigger_ws_target_skipped(run_id, run_spec.run_id, "missing_gate_mode", &target_log_fields);
            continue;
        };

        set_flow_node_state(&mut run_spec.context, &node_spec.node_key, "last_price", json!(current_price));
        set_flow_node_state(&mut run_spec.context, &node_spec.node_key, &prev_key, json!(current_price));
        run_spec.context_dirty = true;

        let confirmation_ms = market_price_confirmation_ms(&node_spec);
        let mut should_enqueue = node_spec.pair_lock_only_monitor
            || matches!(gate_mode, Some(TriggerMarketPriceGateMode::PtbOnly))
            || should_enqueue_market_price_without_confirmation(
                gate_mode.unwrap_or(TriggerMarketPriceGateMode::StandardOnly),
                crossed,
                confirmation_ms,
            );
        let mut final_eval_mode: &str = evaluation_mode;
        let mut ptb_gate_output = Value::Null;

        if matches!(gate_mode, Some(TriggerMarketPriceGateMode::PtbOnly)) {
            if let Some(ptb_gate) = evaluate_trigger_market_price_ptb_gate_for_spec(&node_spec, price_best_bid, price_best_ask) {
                ptb_gate_output = ptb_gate.to_value();
                should_enqueue = ptb_gate.passed;
                final_eval_mode = "ptb_only";
                if !ptb_gate.passed {
                    debug!(
                        run_id,
                        flow_run_id = run_spec.run_id,
                        node_key = %node_spec.node_key,
                        token_id = %node_spec.token_id,
                        dirty_token_id = ?selected_target.dirty_token_id,
                        ws_reevaluation_reason = selected_target.reevaluation_reason,
                        resolved_market_slug = ?node_spec.market_slug,
                        market_slug = ?node_spec.market_slug,
                        reason = ptb_gate.reason,
                        "TRIGGER_WS_PRICE_TO_BEAT_GATE_BLOCKED"
                    );
                    repo.append_trade_flow_event(
                        Some(run_spec.run_id),
                        run_spec.definition_id,
                        Some(run_spec.version_id),
                        "trigger_ws_price_to_beat_gate_blocked",
                        &json!({
                            "node_key": node_spec.node_key,
                            "node_type": node_spec.node_type,
                            "token_id": selected_token_id.clone(),
                            "outcome_label": node_spec.outcome_label,
                            "market_slug": resolved_market_slug.clone(),
                            "resolved_market_slug": resolved_market_slug.clone(),
                            "dirty_token_id": selected_target.dirty_token_id.clone(),
                            "ws_reevaluation_reason": selected_target.reevaluation_reason,
                            "evaluation_mode": "ptb_only",
                            "price_mode": node_spec.price_mode.as_str(),
                            "price_source": price_source.clone(),
                            "price_to_beat_trigger_gate": ptb_gate_output.clone(),
                        }),
                    )
                    .await?;
                }
            }
        }

        if !node_spec.pair_lock_only_monitor
            && matches!(
                gate_mode,
                Some(
                    TriggerMarketPriceGateMode::StandardOnly
                        | TriggerMarketPriceGateMode::StandardAndPtb
                )
            )
        {
            if let Some(confirmation_ms) = confirmation_ms {
                let cpend_at_key = format!("cross_pending_at_{}", node_spec.token_id);
                let cpend_price_key = format!("cross_pending_price_{}", node_spec.token_id);
                let cpend_prev_key = format!("cross_pending_prev_{}", node_spec.token_id);

                let still_in_zone = match node_spec.trigger_condition.as_str() {
                    "cross_below" => current_price <= node_spec.trigger_price,
                    "level_below" => current_price <= node_spec.trigger_price,
                    "level_above" => {
                        let above_trigger = current_price >= node_spec.trigger_price;
                        let below_max = node_spec.max_price.map_or(true, |mp| current_price <= mp);
                        above_trigger && below_max
                    }
                    "cross_above" => {
                        let above_trigger = current_price >= node_spec.trigger_price;
                        let below_max = node_spec.max_price.map_or(true, |mp| current_price <= mp);
                        above_trigger && below_max
                    }
                    _ => false,
                };

                if crossed {
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
                        json!(current_price),
                    );
                    set_flow_node_state(
                        &mut run_spec.context,
                        &node_spec.node_key,
                        &cpend_prev_key,
                        json!(previous_price),
                    );
                    run_spec.context_dirty = true;
                    should_enqueue = false;
                    info!(
                        run_id = run_spec.run_id,
                        node_key = %node_spec.node_key,
                        price = current_price,
                        prev = ?previous_price,
                        market = ?node_spec.market_slug,
                        "CROSS_PENDING_START: waiting {}ms confirmation",
                        confirmation_ms,
                    );
                } else if let Some(pending_at_str) =
                    flow_node_state_string(&run_spec.context, &node_spec.node_key, &cpend_at_key)
                {
                    if still_in_zone {
                        if let Ok(pending_at) = DateTime::parse_from_rfc3339(&pending_at_str) {
                            let elapsed =
                                Utc::now().signed_duration_since(pending_at.with_timezone(&Utc));
                            if elapsed.num_milliseconds() >= confirmation_ms {
                                should_enqueue = true;
                                final_eval_mode = "cross_confirmed";
                                remove_flow_node_state(
                                    &mut run_spec.context,
                                    &node_spec.node_key,
                                    &cpend_at_key,
                                );
                                remove_flow_node_state(
                                    &mut run_spec.context,
                                    &node_spec.node_key,
                                    &cpend_price_key,
                                );
                                remove_flow_node_state(
                                    &mut run_spec.context,
                                    &node_spec.node_key,
                                    &cpend_prev_key,
                                );
                                run_spec.context_dirty = true;
                                info!(
                                    run_id = run_spec.run_id,
                                    node_key = %node_spec.node_key,
                                    price = current_price,
                                    elapsed_ms = elapsed.num_milliseconds(),
                                    market = ?node_spec.market_slug,
                                    "CROSS_CONFIRMED: sustained for {}ms",
                                    elapsed.num_milliseconds(),
                                );
                            }
                        }
                    } else {
                        remove_flow_node_state(
                            &mut run_spec.context,
                            &node_spec.node_key,
                            &cpend_at_key,
                        );
                        remove_flow_node_state(
                            &mut run_spec.context,
                            &node_spec.node_key,
                            &cpend_price_key,
                        );
                        remove_flow_node_state(
                            &mut run_spec.context,
                            &node_spec.node_key,
                            &cpend_prev_key,
                        );
                        run_spec.context_dirty = true;
                        info!(
                            run_id = run_spec.run_id,
                            node_key = %node_spec.node_key,
                            price = current_price,
                            trigger = node_spec.trigger_price,
                            market = ?node_spec.market_slug,
                            "CROSS_PENDING_RESET: price left trigger zone, confirmation timer cleared",
                        );
                    }
                }
            }
        }

        if should_enqueue
            && matches!(gate_mode, Some(TriggerMarketPriceGateMode::StandardAndPtb))
        {
            if let Some(ptb_gate) = evaluate_trigger_market_price_ptb_gate_for_spec(&node_spec, price_best_bid, price_best_ask) {
                ptb_gate_output = ptb_gate.to_value();
                if !ptb_gate.passed {
                    should_enqueue = false;
                    debug!(
                        run_id,
                        flow_run_id = run_spec.run_id,
                        node_key = %node_spec.node_key,
                        token_id = %node_spec.token_id,
                        dirty_token_id = ?selected_target.dirty_token_id,
                        ws_reevaluation_reason = selected_target.reevaluation_reason,
                        resolved_market_slug = ?node_spec.market_slug,
                        market_slug = ?node_spec.market_slug,
                        reason = ptb_gate.reason,
                        "TRIGGER_WS_PRICE_TO_BEAT_GATE_BLOCKED"
                    );
                    repo.append_trade_flow_event(
                        Some(run_spec.run_id),
                        run_spec.definition_id,
                        Some(run_spec.version_id),
                        "trigger_ws_price_to_beat_gate_blocked",
                        &json!({
                            "node_key": node_spec.node_key,
                            "node_type": node_spec.node_type,
                            "token_id": selected_token_id.clone(),
                            "outcome_label": node_spec.outcome_label,
                            "market_slug": resolved_market_slug.clone(),
                            "resolved_market_slug": resolved_market_slug.clone(),
                            "dirty_token_id": selected_target.dirty_token_id.clone(),
                            "ws_reevaluation_reason": selected_target.reevaluation_reason,
                            "evaluation_mode": final_eval_mode,
                            "price_mode": node_spec.price_mode.as_str(),
                            "price_source": price_source.clone(),
                            "price_to_beat_trigger_gate": ptb_gate_output.clone(),
                        }),
                    )
                    .await?;
                }
            }
        }

        if !should_enqueue {
            continue;
        }

        let queued_at = Utc::now();
        let queued_at_rfc3339 = queued_at.to_rfc3339();
        let cycle_window_followup = cycle_window_followup_diagnostics_from_context(
            &run_spec.context,
            &node_spec.node_key,
            &selected_token_id,
            queued_at,
        );
        let allow_first_tick_replay = matches!(
            final_eval_mode,
            "first_tick_threshold" | "first_tick_in_range"
        );
        let pair_lock_candidate_quotes = build_pair_lock_trigger_candidate_quotes(
            &*run_spec,
            &node_spec,
            &market_snapshots,
            client,
            queued_at,
        )
        .await;
        let mut input_json = json!({
            "triggerSource": "ws_market_price",
            "tokenId": selected_token_id.clone(),
            "wsPrice": current_price,
            "wsPrices": { selected_token_id.clone(): current_price },
            "wsPreviousPrice": previous_price,
            "wsPreviousPrices": { selected_token_id.clone(): previous_price },
            "wsEventTs": event_ts,
            "wsMarketSlug": resolved_market_slug.clone(),
            "wsEvaluationMode": final_eval_mode,
            "wsPriceMode": node_spec.price_mode.as_str(),
            "wsPriceSource": price_source.clone(),
            "wsPriceSourceDetail": price_source_detail.clone(),
            "wsBestBid": price_best_bid,
            "wsBestAsk": price_best_ask,
            "wsLastTradePrice": price_last_trade,
            "wsSnapshotAgeMs": price_snapshot_age_ms,
            "wsSiteDisplayModeDecision": price_site_display_decision,
            "wsAllowFirstTickReplay": allow_first_tick_replay,
            "wsReevaluationReason": selected_target.reevaluation_reason,
            "dirtyTokenId": selected_target.dirty_token_id.clone(),
            "resolvedMarketSlug": resolved_market_slug.clone(),
            "queuedAt": queued_at_rfc3339
        });
        if !pair_lock_candidate_quotes.is_null() { input_json[PAIR_LOCK_TRIGGER_CANDIDATE_QUOTES_KEY] = pair_lock_candidate_quotes; }
        if !ptb_gate_output.is_null() {
            append_trigger_market_price_ptb_gate(&mut input_json, &ptb_gate_output);
        }
        let idempotency_key = ws_price_trigger_step_idempotency_key(
            run_spec.run_id,
            &node_spec.node_key,
            &node_spec.trigger_condition,
            current_price,
            event_ts,
            node_spec.once_mode,
            node_spec.once_scope_market,
            node_spec.market_slug.as_deref(),
            flow_node_reentry_generation(&run_spec.context, &node_spec.node_key),
        );

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
            let mut event_payload = json!({
                "node_key": node_spec.node_key,
                "token_id": selected_token_id.clone(),
                "price": current_price,
                "previous_price": previous_price,
                "trigger_condition": node_spec.trigger_condition,
                "trigger_price": node_spec.trigger_price,
                "max_price": node_spec.max_price,
                "evaluation_mode": final_eval_mode,
                "price_mode": node_spec.price_mode.as_str(),
                "price_source": price_source,
                "price_source_detail": price_source_detail,
                "best_bid": price_best_bid,
                "best_ask": price_best_ask,
                "last_trade_price": price_last_trade,
                "snapshot_age_ms": price_snapshot_age_ms,
                "site_display_mode_decision": price_site_display_decision,
                "event_ts": event_ts,
                "queued_at": queued_at_rfc3339,
                "once_mode": node_spec.once_mode,
                "once_scope": if node_spec.once_scope_market { "market" } else { "run" },
                "market_slug": resolved_market_slug.clone(),
                "resolved_market_slug": resolved_market_slug.clone(),
                "dirty_token_id": selected_target.dirty_token_id.clone(),
                "ws_reevaluation_reason": selected_target.reevaluation_reason,
                "reentry_generation": flow_node_reentry_generation(&run_spec.context, &node_spec.node_key),
                "idempotency_key": idempotency_key
            });
            if !ptb_gate_output.is_null() {
                append_trigger_market_price_ptb_gate(&mut event_payload, &ptb_gate_output);
            }
            if let Some(diagnostics) = cycle_window_followup.as_ref() {
                append_json_object_fields(&mut event_payload, diagnostics);
            }
            repo.append_trade_flow_event(
                Some(run_spec.run_id),
                run_spec.definition_id,
                Some(run_spec.version_id),
                "trigger_ws_price_enqueued",
                &event_payload,
            )
            .await?;
        }
    }

    persist_trade_flow_ws_run_specs_contexts(repo, &mut run_specs).await?;
    {
        let mut cache = TRADE_FLOW_WS_FAST_PATH_CACHE.write().await;
        cache.run_specs = run_specs;
        cache.token_targets = token_targets;
        cache.market_targets = market_targets;
    }
    Ok(touched)
}
async fn build_trade_flow_ws_fast_path_cache(
    repo: &PostgresRepository,
    run_id: i64,
    definitions: &[TradeFlowDefinitionRuntime],
    user_cfg_cache: &mut HashMap<i64, AppConfig>,
) -> Result<TradeFlowWsFastPathCache> {
    crate::trade_flow::guards::chainlink_price::ensure_chainlink_price_stream_started();
    crate::trade_flow::guards::binance_price::ensure_binance_price_stream_started();
    let mut run_specs: Vec<WsOpenPositionPriceRunSpec> = Vec::new();
    let mut token_targets: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    let mut market_targets: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    let mut warm_market_slugs: HashSet<String> = HashSet::new();

    for definition in definitions {
        let Some(run) = repo.get_active_trade_flow_run(definition.id).await? else {
            continue;
        };
        let Some(version) = repo.get_trade_flow_version(run.version_id).await? else {
            continue;
        };
        let flow_cfg =
            match load_user_app_config_cached(repo, definition.user_id, user_cfg_cache).await {
                Ok(cfg) => cfg,
                Err(err) => {
                    warn!(
                        run_id,
                        definition_id = definition.id,
                        user_id = definition.user_id,
                        error = %err,
                        "TRADE_FLOW_USER_CONFIG_LOAD_FAILED"
                    );
                    continue;
                }
            };
        let graph = parse_trade_flow_graph(&version)?;
        let mut context = normalize_trade_flow_context(run.context_json.clone(), &graph.context);
        let mut nodes = Vec::new();
        let mut auto_scope_selected_by_node: HashMap<String, SelectedLiveMarket> = HashMap::new();
        let mut cycle_window_snapshot_recorded = false;
        for node in &graph.nodes {
            if node_market_mode(node) == "auto_scope"
                && matches!(
                    node.node_type.as_str(),
                    "trigger.market_price" | "trigger.open_positions"
                )
            {
                match sync_trigger_market_auto_scope_context(&flow_cfg, node, &mut context).await {
                    Ok(Some(selected)) => {
                        auto_scope_selected_by_node.insert(node.key.clone(), selected);
                    }
                    Ok(None) => {
                        log_trigger_ws_cache_node_skipped(
                            run_id,
                            run.id,
                            "auto_scope_resolve_none",
                            &build_trigger_ws_cache_node_log_fields_from_node(
                                node,
                                &context,
                                node_config_string(node, "outcomeLabel").as_deref(),
                            ),
                        );
                        continue;
                    }
                    Err(err) => {
                        warn!(
                            run_id,
                            flow_run_id = run.id,
                            node_key = %node.key,
                            error = %err,
                            "TRADE_FLOW_TRIGGER_AUTO_SCOPE_RESOLVE_FAILED"
                        );
                        log_trigger_ws_cache_node_skipped(
                            run_id,
                            run.id,
                            "auto_scope_resolve_error",
                            &build_trigger_ws_cache_node_log_fields_from_node(
                                node,
                                &context,
                                node_config_string(node, "outcomeLabel").as_deref(),
                            ),
                        );
                        continue;
                    }
                }
            }
            let spec_result = build_open_position_ws_price_node_specs(node, &context);
            for skip_reason in &spec_result.skip_reasons {
                log_trigger_ws_cache_node_skipped(
                    run_id,
                    run.id,
                    skip_reason.reason,
                    &build_trigger_ws_cache_node_log_fields_from_node(
                        node,
                        &context,
                        skip_reason.outcome_label.as_deref(),
                    ),
                );
            }
            for spec in &spec_result.specs {
                log_trigger_ws_cache_node_indexed(
                    run_id,
                    run.id,
                    &build_trigger_ws_cache_node_log_fields_from_spec(
                        spec,
                        version.id,
                        Some(version.version_no),
                    ),
                );
                if spec.cycle_window_mode.is_some()
                    && !flow_node_state_truthy(&context, &spec.node_key, &cycle_window_config_snapshot_state_key(&spec.token_id))
                {
                    if let Err(err) = repo
                        .append_trade_flow_event(
                            Some(run.id),
                            run.definition_id,
                            Some(run.version_id),
                            "trigger_cycle_window_config_snapshot",
                            &json!({
                                "node_key": spec.node_key,
                                "node_type": spec.node_type,
                                "market_slug": spec.market_slug,
                                "token_id": spec.token_id,
                                "outcome_label": spec.outcome_label,
                                "version_id": version.id,
                                "version_no": version.version_no,
                                "cycle_window_mode": spec.cycle_window_mode,
                                "cycle_window_secs": spec.cycle_window_secs,
                                "cycle_window_start_sec": spec.cycle_window_start_sec,
                                "cycle_window_end_sec": spec.cycle_window_end_sec,
                                "auto_sell_on_window_end": spec.auto_sell_on_window_end,
                            }),
                        )
                        .await
                    {
                        warn!(
                            run_id,
                            flow_run_id = run.id,
                            node_key = %spec.node_key,
                            error = %err,
                            "TRIGGER_CYCLE_WINDOW_CONFIG_SNAPSHOT_EVENT_FAILED"
                        );
                    }
                    set_flow_node_state(&mut context, &spec.node_key, &cycle_window_config_snapshot_state_key(&spec.token_id), json!(true));
                    cycle_window_snapshot_recorded = true;
                }
            }
            nodes.extend(spec_result.specs);
        }
        if nodes.is_empty() {
            continue;
        }

        let run_index = run_specs.len();
        for (node_index, node) in nodes.iter().enumerate() {
            token_targets
                .entry(node.token_id.clone())
                .or_default()
                .push((run_index, node_index));
            if let Some(market_slug) = node.market_slug.as_deref() {
                if node.node_type == "trigger.market_price" {
                    market_targets
                        .entry(market_slug.to_string())
                        .or_default()
                        .push((run_index, node_index));
                }
                warm_market_slugs.insert(market_slug.to_string());
            }
        }

        let original_slug = run
            .context_json
            .pointer("/flowContext/marketSlug")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let updated_slug = context
            .pointer("/flowContext/marketSlug")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        if !updated_slug.is_empty() {
            warm_market_slugs.insert(updated_slug.clone());
        }
        let slug_changed = !updated_slug.is_empty() && original_slug != updated_slug;
        let rotation_detected_at = Utc::now();
        let rotations = sync_trade_flow_auto_scope_market_rollover_state(
            &run.context_json,
            &mut context,
            &nodes,
            &auto_scope_selected_by_node,
            rotation_detected_at,
        );
        if slug_changed {
            info!(
                run_id,
                flow_run_id = run.id,
                old_slug = original_slug,
                new_slug = updated_slug,
                "TRADE_FLOW_CONTEXT_SLUG_CHANGED_PRE_WS"
            );
        }
        for rotation in &rotations {
            warm_market_slugs.insert(rotation.new_market_slug.clone());
            let rotation_lag_ms = auto_scope_rotation_lag_ms(rotation);
            let scope = crate::find_updown_scope_by_slug(&rotation.new_market_slug);
            let selected_market = auto_scope_selected_by_node.get(&rotation.node_key);
            info!(
                run_id,
                flow_run_id = run.id,
                node_key = %rotation.node_key,
                old_slug = %rotation.old_market_slug,
                new_slug = %rotation.new_market_slug,
                expected_market_start = ?rotation.expected_market_start,
                rotation_detected_at = %rotation.rotation_detected_at,
                rotation_lag_ms,
                selection_reason = ?rotation.selection_reason,
                "TRADE_FLOW_TRIGGER_AUTO_SCOPE_MARKET_ROTATED"
            );
            if let Err(err) = repo
                .append_trade_flow_event(
                    Some(run.id),
                    run.definition_id,
                    Some(run.version_id),
                    "trigger_auto_scope_market_rotated",
                    &json!({
                        "node_key": rotation.node_key,
                        "node_type": "trigger.market_price",
                        "market_scope": scope.map(|value| value.scope),
                        "market_asset": scope.map(|value| value.asset),
                        "market_timeframe": scope.map(|value| value.timeframe),
                        "old_market_slug": rotation.old_market_slug,
                        "new_market_slug": rotation.new_market_slug,
                        "yes_token_id": selected_market.and_then(|selected| selected.yes_token_id.clone()),
                        "no_token_id": selected_market.and_then(|selected| selected.no_token_id.clone()),
                        "rotation_reason": "cycle_rollover",
                        "state_reset": true,
                        "expected_market_start": rotation.expected_market_start
                            .map(|value| value.to_rfc3339()),
                        "rotation_detected_at": rotation.rotation_detected_at.to_rfc3339(),
                        "rotation_lag_ms": rotation_lag_ms,
                        "selection_reason": rotation.selection_reason,
                    }),
                )
                .await
            {
                warn!(
                    run_id,
                    flow_run_id = run.id,
                    node_key = %rotation.node_key,
                    error = %err,
                    "TRADE_FLOW_TRIGGER_AUTO_SCOPE_ROTATION_EVENT_FAILED"
                );
            }

            if let Some(expected_market_start) = rotation.expected_market_start.as_ref() {
                if let Some(scope) = crate::find_updown_scope_by_slug(&rotation.new_market_slug) {
                    if matches!(scope.timeframe, "5m" | "15m") {
                        match crate::trade_flow::guards::chainlink_price::get_chainlink_price_start_tick(
                            &scope.asset,
                            expected_market_start.timestamp_millis(),
                        ) {
                            Ok(snapshot) => {
                                let source_latency_ms = Some(
                                    (snapshot.timestamp_ms
                                        - expected_market_start.timestamp_millis())
                                    .abs(),
                                );
                                let seeded = crate::trade_flow::guards::polymarket_price_to_beat::seed_price_to_beat_from_chainlink(
                                    &rotation.new_market_slug,
                                    &scope.asset,
                                    scope.timeframe,
                                    snapshot.price,
                                    source_latency_ms,
                                );
                                if seeded {
                                    warm_market_slugs.remove(&rotation.new_market_slug);
                                    info!(
                                        run_id,
                                        flow_run_id = run.id,
                                        market_slug = %rotation.new_market_slug,
                                        asset = %scope.asset,
                                        chainlink_price = snapshot.price,
                                        chainlink_tick_ts = snapshot.timestamp_ms,
                                        source_latency_ms,
                                        "PRICE_TO_BEAT_SEEDED_FROM_CHAINLINK"
                                    );
                                }
                                crate::trade_flow::guards::polymarket_price_to_beat::schedule_price_to_beat_promotion(
                                    &rotation.new_market_slug,
                                );
                            }
                            Err(err) => {
                                let err_text = err.to_string();
                                warn!(
                                    run_id,
                                    flow_run_id = run.id,
                                    market_slug = %rotation.new_market_slug,
                                    asset = %scope.asset,
                                    error = %err,
                                    "PRICE_TO_BEAT_CHAINLINK_SEED_FAILED"
                                );
                                if let Some(details) = crate::trade_flow::guards::chainlink_price::parse_chainlink_near_timestamp_rejection_details(&err_text) {
                                    let expected_market_start_text = expected_market_start.to_rfc3339();
                                    warn!(
                                        run_id,
                                        flow_run_id = run.id,
                                        market_slug = %rotation.new_market_slug,
                                        asset = %scope.asset,
                                        timeframe = %scope.timeframe,
                                        expected_market_start = %expected_market_start_text,
                                        gap_ms = details.gap_ms,
                                        provider_age_ms = details.provider_age_ms,
                                        candidate_timestamp_ms = details.candidate_timestamp_ms,
                                        candidate_received_at_ms = details.candidate_received_at_ms,
                                        "CHAINLINK_SEED_REJECTED_TOO_OLD"
                                    );
                                    let payload = build_chainlink_seed_rejected_too_old_payload(
                                        &rotation.new_market_slug,
                                        scope.asset,
                                        scope.timeframe,
                                        expected_market_start,
                                        &details,
                                    );
                                    if let Err(event_err) = repo
                                        .append_trade_flow_event(
                                            Some(run.id),
                                            run.definition_id,
                                            Some(run.version_id),
                                            "chainlink_seed_rejected_too_old",
                                            &payload,
                                        )
                                        .await
                                    {
                                        warn!(
                                            run_id,
                                            flow_run_id = run.id,
                                            market_slug = %rotation.new_market_slug,
                                            asset = %scope.asset,
                                            error = %event_err,
                                            "CHAINLINK_SEED_REJECTED_TOO_OLD_EVENT_FAILED"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        run_specs.push(WsOpenPositionPriceRunSpec {
            run_id: run.id,
            definition_id: run.definition_id,
            version_id: run.version_id,
            version_no: version.version_no,
            context,
            nodes,
            context_dirty: slug_changed || !rotations.is_empty() || cycle_window_snapshot_recorded,
        });
    }

    for warm_slug in warm_market_slugs {
        crate::trade_flow::guards::polymarket_price_to_beat::warm_price_to_beat_cache_bg(
            &warm_slug,
        );
    }

    Ok(TradeFlowWsFastPathCache {
        run_specs,
        token_targets,
        market_targets,
    })
}

async fn persist_trade_flow_ws_run_specs_contexts(
    repo: &PostgresRepository,
    run_specs: &mut [WsOpenPositionPriceRunSpec],
) -> Result<()> {
    for run_spec in run_specs {
        if !run_spec.context_dirty {
            continue;
        }
        repo.update_trade_flow_run_context(run_spec.run_id, &run_spec.context)
            .await?;
        run_spec.context_dirty = false;
    }
    Ok(())
}

fn select_ws_fast_path_targets(
    run_id: i64,
    cache: &TradeFlowWsFastPathCache,
    dirty_token_ids: Option<&[String]>,
) -> Vec<SelectedWsFastPathTarget> {
    match dirty_token_ids {
        Some(token_ids) => {
            let mut selected = Vec::new();
            let mut seen_targets = HashSet::new();
            let mut seen_market_pairs = HashSet::new();
            let mut dirty_markets = Vec::new();

            for dirty_token_id in token_ids {
                let Some(targets) = cache.token_targets.get(dirty_token_id.as_str()) else {
                    log_trigger_ws_dirty_token_unmapped(run_id, dirty_token_id);
                    continue;
                };

                for &(run_index, node_index) in targets {
                    let Some(node_spec) = cache
                        .run_specs
                        .get(run_index)
                        .and_then(|run_spec| run_spec.nodes.get(node_index))
                    else {
                        continue;
                    };

                    if seen_targets.insert((run_index, node_index)) {
                        selected.push(SelectedWsFastPathTarget {
                            run_index,
                            node_index,
                            dirty_token_id: Some(dirty_token_id.clone()),
                            reevaluation_reason: "dirty_token_match",
                        });
                    }

                    if let Some(market_slug) = node_spec.market_slug.as_ref() {
                        dirty_markets.push((market_slug.clone(), dirty_token_id.clone()));
                    }
                }
            }

            for (market_slug, dirty_token_id) in dirty_markets {
                if !seen_market_pairs.insert((market_slug.clone(), dirty_token_id.clone())) {
                    continue;
                }
                let Some(targets) = cache.market_targets.get(&market_slug) else {
                    continue;
                };
                for &(run_index, node_index) in targets {
                    if !seen_targets.insert((run_index, node_index)) {
                        continue;
                    }
                    selected.push(SelectedWsFastPathTarget {
                        run_index,
                        node_index,
                        dirty_token_id: Some(dirty_token_id.clone()),
                        reevaluation_reason: "market_dirty_fanout",
                    });
                }
            }

            selected
        }
        None => cache
            .run_specs
            .iter()
            .enumerate()
            .flat_map(|(run_index, run_spec)| {
                run_spec
                    .nodes
                    .iter()
                    .enumerate()
                    .map(move |(node_index, _)| SelectedWsFastPathTarget {
                        run_index,
                        node_index,
                        dirty_token_id: None,
                        reevaluation_reason: "full_refresh",
                    })
            })
            .collect(),
    }
}
