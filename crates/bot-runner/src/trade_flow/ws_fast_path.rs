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
        if let Err(err) = ws.ensure_market_stream(&[]).await {
            warn!(run_id, error = %err, "TRADE_FLOW_WS_STREAM_CLEAR_FAILED");
        }
        return Ok(());
    }

    let mut fast_path_cache =
        build_trade_flow_ws_fast_path_cache(repo, run_id, definitions, user_cfg_cache).await?;
    persist_trade_flow_ws_run_specs_contexts(repo, &mut fast_path_cache.run_specs).await?;

    let all_token_ids: Vec<String> = fast_path_cache.token_targets.keys().cloned().collect();
    if let Err(err) = ws.ensure_market_stream(&all_token_ids).await {
        warn!(
            run_id,
            token_count = all_token_ids.len(),
            error = %err,
            "TRADE_FLOW_WS_STREAM_ENSURE_FAILED"
        );
    }

    let run_count = fast_path_cache.run_specs.len();
    let token_count = fast_path_cache.token_targets.len();
    {
        let mut cache = TRADE_FLOW_WS_FAST_PATH_CACHE.write().await;
        *cache = fast_path_cache;
    }
    info!(
        run_id,
        runs = run_count,
        tokens = token_count,
        "TRADE_FLOW_WS_FAST_PATH_CACHE_REFRESHED"
    );
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AutoScopeMarketRotation {
    node_key: String,
    old_market_slug: String,
    new_market_slug: String,
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

fn sync_trade_flow_auto_scope_market_rollover_state(
    previous_context: &Value,
    context: &mut Value,
    node_specs: &[WsOpenPositionPriceNodeSpec],
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
        let previous_flow_slug = previous_context
            .pointer("/flowContext/marketSlug")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|slug| !slug.is_empty())
            .map(str::to_string);
        let old_market_slug = previous_last_ws_slug.or(previous_flow_slug);

        if old_market_slug.as_deref() != Some(new_market_slug) {
            clear_trade_flow_market_price_ws_runtime_state(context, &node_spec.node_key);
            if let Some(old_market_slug) = old_market_slug {
                rotations.push(AutoScopeMarketRotation {
                    node_key: node_spec.node_key.clone(),
                    old_market_slug,
                    new_market_slug: new_market_slug.to_string(),
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
    if cache_snapshot.run_specs.is_empty() || cache_snapshot.token_targets.is_empty() {
        return Ok(false);
    }

    let selected_token_ids = select_ws_fast_path_token_ids(&cache_snapshot, dirty_token_ids);
    if selected_token_ids.is_empty() {
        return Ok(false);
    }

    let mut run_specs = cache_snapshot.run_specs;
    let token_targets = cache_snapshot.token_targets;
    let market_snapshots = ws.get_market_snapshots(&selected_token_ids).await;
    let mut touched = false;

    for token_id in selected_token_ids {
        let Some(targets) = token_targets.get(&token_id).cloned() else {
            continue;
        };

        for (run_index, node_index) in targets {
            let Some(node_spec) = run_specs
                .get(run_index)
                .and_then(|run_spec| run_spec.nodes.get(node_index))
                .cloned()
            else {
                continue;
            };

            let resolved = market_snapshots.get(&token_id).and_then(|snapshot| {
                resolve_trigger_price_from_market_snapshot(snapshot, node_spec.price_mode)
            });
            let resolved_price = if let Some(resolved) = resolved {
                resolved
            } else if let Some(cl) = client {
                match resolve_trigger_price_from_rest(cl, &token_id, node_spec.price_mode).await {
                    Ok(resolved) => resolved,
                    Err(_) => {
                        debug!(
                            run_id,
                            node_key = %node_spec.node_key,
                            token_id = %node_spec.token_id,
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

            let Some(run_spec) = run_specs.get_mut(run_index) else {
                continue;
            };
            touched = true;
            if node_spec.auto_scope {
                if let Some(ref slug) = node_spec.market_slug {
                    if is_auto_scope_market_expired(slug, 15) {
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
            if node_spec.auto_scope {
                if let Some(ref slug) = node_spec.market_slug {
                    if auto_scope_resolution_window_guard_enabled(
                        node_spec.cycle_window_mode.as_deref(),
                    ) && is_auto_scope_market_in_resolution_window(slug, 120)
                    {
                        debug!(
                            run_id,
                            flow_run_id = run_spec.run_id,
                            node_key = %node_spec.node_key,
                            market_slug = %slug,
                            "TRIGGER_SKIP_RESOLUTION_WINDOW"
                        );
                        continue;
                    }
                }
            }
            if node_spec.auto_scope {
                if let (Some(ref slug), Some(ref cw_mode), Some(cw_secs)) = (
                    &node_spec.market_slug,
                    &node_spec.cycle_window_mode,
                    node_spec.cycle_window_secs,
                ) {
                    if is_outside_cycle_window_focus(slug, cw_mode, cw_secs) {
                        debug!(
                            run_id,
                            flow_run_id = run_spec.run_id,
                            node_key = %node_spec.node_key,
                            market_slug = %slug,
                            cycle_window_mode = %cw_mode,
                            cycle_window_secs = cw_secs,
                            "TRIGGER_SKIP_CYCLE_WINDOW_FOCUS"
                        );
                        continue;
                    }
                }
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
                    remove_flow_node_state(
                        &mut run_spec.context,
                        &node_spec.node_key,
                        &cpend_price,
                    );
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
                                "token_id": node_spec.token_id,
                                "reason": "ws_enqueue_once_fired",
                                "once_scope": if node_spec.once_scope_market { "market" } else { "run" },
                                "market_slug": node_spec.market_slug.clone()
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

            let prev_key = format!("previous_price_{}", node_spec.token_id);
            let previous_price = flow_node_state(&run_spec.context, &node_spec.node_key, &prev_key)
                .and_then(value_as_f64);
            let allow_first_tick_threshold =
                allow_first_tick_threshold_for_ws_node(&node_spec, previous_price);
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
                    market = ?node_spec.market_slug,
                    "TRIGGER_WS_NO_PREVIOUS_PRICE"
                );
            }

            set_flow_node_state(
                &mut run_spec.context,
                &node_spec.node_key,
                "last_price",
                json!(current_price),
            );
            set_flow_node_state(
                &mut run_spec.context,
                &node_spec.node_key,
                &prev_key,
                json!(current_price),
            );
            run_spec.context_dirty = true;

            let mut should_enqueue = crossed;
            let mut final_eval_mode: &str = evaluation_mode;

            if let Some(confirmation_ms) = market_price_confirmation_ms(&node_spec) {
                let cpend_at_key = format!("cross_pending_at_{}", node_spec.token_id);
                let cpend_price_key = format!("cross_pending_price_{}", node_spec.token_id);
                let cpend_prev_key = format!("cross_pending_prev_{}", node_spec.token_id);

                let still_in_zone = match node_spec.trigger_condition.as_str() {
                    "cross_below" => current_price <= node_spec.trigger_price,
                    "cross_above" => {
                        let above_trigger = current_price >= node_spec.trigger_price;
                        let below_max =
                            node_spec.max_price.map_or(true, |mp| current_price <= mp);
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

            if !should_enqueue {
                continue;
            }

            let queued_at = Utc::now();
            let queued_at_rfc3339 = queued_at.to_rfc3339();
            let cycle_window_followup = cycle_window_followup_diagnostics_from_context(
                &run_spec.context,
                &node_spec.node_key,
                &token_id,
                queued_at,
            );
            let input_json = json!({
                "triggerSource": "ws_market_price",
                "tokenId": token_id,
                "wsPrice": current_price,
                "wsPrices": { token_id.clone(): current_price },
                "wsPreviousPrice": previous_price,
                "wsPreviousPrices": { token_id.clone(): previous_price },
                "wsEventTs": event_ts,
                "wsMarketSlug": node_spec.market_slug.clone(),
                "wsEvaluationMode": final_eval_mode,
                "wsPriceMode": node_spec.price_mode.as_str(),
                "wsPriceSource": price_source.clone(),
                "wsPriceSourceDetail": price_source_detail.clone(),
                "wsBestBid": price_best_bid,
                "wsBestAsk": price_best_ask,
                "wsLastTradePrice": price_last_trade,
                "wsSnapshotAgeMs": price_snapshot_age_ms,
                "wsSiteDisplayModeDecision": price_site_display_decision,
                "queuedAt": queued_at_rfc3339
            });
            let idempotency_key = ws_price_trigger_step_idempotency_key(
                run_spec.run_id,
                &node_spec.node_key,
                &node_spec.trigger_condition,
                current_price,
                event_ts,
                node_spec.once_mode,
                node_spec.once_scope_market,
                node_spec.market_slug.as_deref(),
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
                    "token_id": token_id,
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
                    "market_slug": node_spec.market_slug.clone(),
                    "idempotency_key": idempotency_key
                });
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
    }

    persist_trade_flow_ws_run_specs_contexts(repo, &mut run_specs).await?;
    {
        let mut cache = TRADE_FLOW_WS_FAST_PATH_CACHE.write().await;
        cache.run_specs = run_specs;
        cache.token_targets = token_targets;
    }
    Ok(touched)
}

async fn build_trade_flow_ws_fast_path_cache(
    repo: &PostgresRepository,
    run_id: i64,
    definitions: &[TradeFlowDefinitionRuntime],
    user_cfg_cache: &mut HashMap<i64, AppConfig>,
) -> Result<TradeFlowWsFastPathCache> {
    let mut run_specs: Vec<WsOpenPositionPriceRunSpec> = Vec::new();
    let mut token_targets: HashMap<String, Vec<(usize, usize)>> = HashMap::new();

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
        for node in &graph.nodes {
            if node_market_mode(node) == "auto_scope"
                && matches!(
                    node.node_type.as_str(),
                    "trigger.market_price" | "trigger.open_positions"
                )
            {
                match sync_trigger_market_auto_scope_context(&flow_cfg, node, &mut context).await {
                    Ok(Some(_)) => {}
                    Ok(None) => continue,
                    Err(err) => {
                        warn!(
                            run_id,
                            flow_run_id = run.id,
                            node_key = %node.key,
                            error = %err,
                            "TRADE_FLOW_TRIGGER_AUTO_SCOPE_RESOLVE_FAILED"
                        );
                        continue;
                    }
                }
            }
            let specs = open_position_ws_price_node_specs(node, &context);
            nodes.extend(specs);
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
        let slug_changed = !updated_slug.is_empty() && original_slug != updated_slug;
        let rotations = sync_trade_flow_auto_scope_market_rollover_state(
            &run.context_json,
            &mut context,
            &nodes,
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
            info!(
                run_id,
                flow_run_id = run.id,
                node_key = %rotation.node_key,
                old_slug = %rotation.old_market_slug,
                new_slug = %rotation.new_market_slug,
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
                        "old_market_slug": rotation.old_market_slug,
                        "new_market_slug": rotation.new_market_slug,
                        "rotation_reason": "cycle_rollover",
                        "state_reset": true,
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
        }

        run_specs.push(WsOpenPositionPriceRunSpec {
            run_id: run.id,
            definition_id: run.definition_id,
            version_id: run.version_id,
            context,
            nodes,
            context_dirty: slug_changed || !rotations.is_empty(),
        });
    }

    Ok(TradeFlowWsFastPathCache {
        run_specs,
        token_targets,
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

fn select_ws_fast_path_token_ids(
    cache: &TradeFlowWsFastPathCache,
    dirty_token_ids: Option<&[String]>,
) -> Vec<String> {
    match dirty_token_ids {
        Some(token_ids) => token_ids
            .iter()
            .filter(|token_id| cache.token_targets.contains_key(token_id.as_str()))
            .cloned()
            .collect(),
        None => cache.token_targets.keys().cloned().collect(),
    }
}
