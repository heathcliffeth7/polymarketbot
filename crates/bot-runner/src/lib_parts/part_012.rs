async fn sync_trade_flow_definition_run(
    repo: &PostgresRepository,
    run_id: i64,
    definition: &TradeFlowDefinitionRuntime,
) -> Result<()> {
    let Some(published_version_id) = definition.published_version_id else {
        return Ok(());
    };

    let version = repo
        .get_trade_flow_version(published_version_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("published trade flow version not found"))?;
    let graph = parse_trade_flow_graph(&version)?;
    let publish_marker = trade_flow_publish_marker(&version);

    let mut needs_new_run = false;
    if let Some(active_run) = repo.get_active_trade_flow_run(definition.id).await? {
        if active_run.version_id != version.id {
            repo.set_trade_flow_run_status(active_run.id, "canceled", Some("version_changed"))
                .await?;
            repo.append_trade_flow_event(
                Some(active_run.id),
                definition.id,
                Some(active_run.version_id),
                "run_canceled_version_changed",
                &json!({
                    "previous_version_id": active_run.version_id,
                    "next_version_id": version.id
                }),
            )
            .await?;
            needs_new_run = true;
        } else {
            let mut context =
                normalize_trade_flow_context(active_run.context_json.clone(), &graph.context);
            let (previous_publish_marker, reset_nodes) =
                sync_trade_flow_once_state_for_publish(&graph, &mut context, &publish_marker);
            let publish_marker_changed =
                previous_publish_marker.as_deref() != Some(publish_marker.as_str());
            if publish_marker_changed {
                repo.update_trade_flow_run_context(active_run.id, &context)
                    .await?;
                if let Some(prev_marker) = previous_publish_marker {
                    repo.append_trade_flow_event(
                        Some(active_run.id),
                        definition.id,
                        Some(active_run.version_id),
                        "trigger_once_reset_on_publish",
                        &json!({
                            "previous_publish_marker": prev_marker,
                            "next_publish_marker": publish_marker.clone(),
                            "version_id": version.id,
                            "reset_node_keys": reset_nodes,
                        }),
                    )
                    .await?;
                }
            }
        }
    } else {
        needs_new_run = true;
    }

    if !needs_new_run {
        return Ok(());
    }

    // Defensive: cancel any stale 'running' run that might cause unique constraint violation.
    // This handles crash recovery and concurrent-start edge cases.
    if let Some(stale_run) = repo.get_active_trade_flow_run(definition.id).await? {
        warn!(
            run_id,
            definition_id = definition.id,
            stale_run_id = stale_run.id,
            "TRADE_FLOW_STALE_RUN_CLEANUP"
        );
        repo.set_trade_flow_run_status(stale_run.id, "canceled", Some("stale_run_cleanup"))
            .await?;
    }

    let mut context_json = build_initial_trade_flow_context(&graph.context);
    set_flow_state(
        &mut context_json,
        FLOW_STATE_PUBLISH_MARKER,
        json!(publish_marker),
    );
    let run = repo
        .create_trade_flow_run(
            definition.id,
            version.id,
            Some("runner_auto_start"),
            &context_json,
        )
        .await?;
    repo.append_trade_flow_event(
        Some(run.id),
        definition.id,
        Some(version.id),
        "run_started",
        &json!({
            "run_id": run.id,
            "version_id": version.id,
            "definition_name": definition.name
        }),
    )
    .await?;
    seed_trade_flow_trigger_steps(repo, run_id, &run, &graph).await?;
    Ok(())
}

fn build_initial_trade_flow_context(graph_context: &Value) -> Value {
    let mut context = json!({
        "flowContext": {},
        "vars": {},
        "state": {},
        "refs": {},
        "nodeState": {}
    });

    let flow_context = if graph_context.is_object() {
        graph_context.clone()
    } else {
        json!({})
    };
    if let Some(map) = context.as_object_mut() {
        map.insert("flowContext".to_string(), flow_context);
    }
    context
}

fn normalize_trade_flow_context(context_json: Value, graph_context: &Value) -> Value {
    let mut normalized = if context_json.is_object() {
        context_json
    } else {
        json!({})
    };

    let graph_ctx = if graph_context.is_object() {
        graph_context.clone()
    } else {
        json!({})
    };

    {
        let root = ensure_object_mut(&mut normalized);
        if !root
            .get("flowContext")
            .map(Value::is_object)
            .unwrap_or(false)
        {
            root.insert("flowContext".to_string(), graph_ctx);
        }
        for key in ["vars", "state", "refs", "nodeState"] {
            if !root.get(key).map(Value::is_object).unwrap_or(false) {
                root.insert(key.to_string(), json!({}));
            }
        }
    }

    normalized
}

fn ensure_object_mut(value: &mut Value) -> &mut serde_json::Map<String, Value> {
    if !value.is_object() {
        *value = json!({});
    }
    value
        .as_object_mut()
        .expect("value should be object after normalization")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TradeFlowSeedMode {
    Trigger,
    DcaLiveRoot,
}

impl TradeFlowSeedMode {
    fn as_str(self) -> &'static str {
        match self {
            TradeFlowSeedMode::Trigger => "trigger",
            TradeFlowSeedMode::DcaLiveRoot => "dca_live_root",
        }
    }
}

fn collect_trade_flow_root_nodes<'a>(graph: &'a TradeFlowGraphRuntime) -> Vec<&'a TradeFlowNode> {
    let incoming_targets: HashSet<&str> = graph
        .edges
        .iter()
        .map(|edge| edge.target.as_str())
        .collect();

    graph
        .nodes
        .iter()
        .filter(|node| !incoming_targets.contains(node.key.as_str()))
        .collect()
}

fn select_trade_flow_initial_seed_nodes<'a>(
    graph: &'a TradeFlowGraphRuntime,
) -> std::result::Result<(TradeFlowSeedMode, Vec<&'a TradeFlowNode>), &'static str> {
    let trigger_nodes: Vec<&TradeFlowNode> = graph
        .nodes
        .iter()
        .filter(|node| node.node_type.starts_with("trigger."))
        .collect();
    if !trigger_nodes.is_empty() {
        return Ok((TradeFlowSeedMode::Trigger, trigger_nodes));
    }

    let has_dca_live_root = graph
        .nodes
        .iter()
        .any(|node| node.node_type == "action.place_order" && action_place_order_uses_dca_live(node));
    if !has_dca_live_root {
        return Err("flow_missing_trigger");
    }

    let root_nodes = collect_trade_flow_root_nodes(graph);
    if !root_nodes.is_empty()
        && root_nodes
            .iter()
            .all(|node| node.node_type == "action.place_order" && action_place_order_uses_dca_live(node))
    {
        return Ok((TradeFlowSeedMode::DcaLiveRoot, root_nodes));
    }

    Err("flow_invalid_roots_without_trigger")
}

async fn seed_trade_flow_trigger_steps(
    repo: &PostgresRepository,
    run_id: i64,
    run: &TradeFlowRun,
    graph: &TradeFlowGraphRuntime,
) -> Result<()> {
    let now = Utc::now();
    let trigger_count = graph
        .nodes
        .iter()
        .filter(|node| node.node_type.starts_with("trigger."))
        .count();
    let (seed_mode, nodes_to_seed) = match select_trade_flow_initial_seed_nodes(graph) {
        Ok(selection) => selection,
        Err(reason) => {
            let has_dca_live_root = graph
                .nodes
                .iter()
                .any(|node| node.node_type == "action.place_order" && action_place_order_uses_dca_live(node));
            let root_nodes_payload: Vec<Value> = collect_trade_flow_root_nodes(graph)
                .iter()
                .map(|node| {
                    json!({
                        "key": node.key.as_str(),
                        "type": node.node_type.as_str()
                    })
                })
                .collect();
            repo.set_trade_flow_run_status(run.id, "failed", Some(reason))
                .await?;
            repo.append_trade_flow_event(
                Some(run.id),
                run.definition_id,
                Some(run.version_id),
                "run_failed",
                &json!({
                    "reason": reason,
                    "hasDcaLiveRoot": has_dca_live_root,
                    "rootNodes": root_nodes_payload
                }),
            )
            .await?;
            return Ok(());
        }
    };

    let start_count = nodes_to_seed.len();
    for node in nodes_to_seed {
        let idempotency_key = format!("seed:{}:{}", run.id, node.key);
        let _ = repo
            .enqueue_trade_flow_step(
                run.id,
                &node.key,
                &node.node_type,
                1,
                None,
                now,
                None,
                Some(&idempotency_key),
            )
            .await?;
    }

    info!(
        run_id,
        flow_run_id = run.id,
        trigger_count,
        start_mode = seed_mode.as_str(),
        start_count,
        "TRADE_FLOW_RUN_INITIAL_STEPS_SEEDED"
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn process_trade_flow_step(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: Option<SharedOrderExecutor>,
    ws: &ClobWsClient,
    step: &TradeFlowRunStep,
) -> Result<()> {
    let step_started = Instant::now();
    let processing_started_at = Utc::now();
    let claim_to_start_ms = step
        .started_at
        .map(|started_at| {
            processing_started_at
                .signed_duration_since(started_at)
                .num_milliseconds()
                .max(0)
        })
        .unwrap_or(0);
    let run = match repo.get_trade_flow_run(step.run_id).await? {
        Some(run) => run,
        None => {
            repo.mark_trade_flow_step_skipped(step.id, None).await?;
            return Ok(());
        }
    };
    if run.status != "running" {
        repo.mark_trade_flow_step_skipped(step.id, None).await?;
        return Ok(());
    }

    let definition = repo
        .get_trade_flow_definition(run.definition_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("flow definition not found for run"))?;
    if definition.status != "published" {
        repo.set_trade_flow_run_status(run.id, "canceled", Some("definition_not_published"))
            .await?;
        repo.mark_trade_flow_step_skipped(step.id, None).await?;
        return Ok(());
    }
    let published_version_id = definition.published_version_id;
    if published_version_id != Some(run.version_id) {
        let output_json = json!({
            "reason": "version_changed",
            "run_version_id": run.version_id,
            "published_version_id": published_version_id,
            "node_key": step.node_key,
            "node_type": step.node_type,
        });
        repo.set_trade_flow_run_status(run.id, "canceled", Some("version_changed"))
            .await?;
        repo.mark_trade_flow_step_skipped(step.id, Some(&output_json))
            .await?;
        repo.append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "step_skipped_version_changed",
            &output_json,
        )
        .await?;
        return Ok(());
    }

    let version = repo
        .get_trade_flow_version(run.version_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("flow version not found for run"))?;
    let graph = parse_trade_flow_graph(&version)?;
    let node = graph
        .nodes
        .iter()
        .find(|node| node.key == step.node_key)
        .ok_or_else(|| anyhow::anyhow!("flow node not found for step"))?;

    let mut context = normalize_trade_flow_context(run.context_json.clone(), &graph.context);
    let execute_started = Instant::now();
    let result = execute_trade_flow_node(
        repo,
        run_id,
        cfg,
        limits,
        policy,
        client.as_deref(),
        ws,
        &run,
        step,
        node,
        &graph,
        &mut context,
    )
    .await;
    let execute_ms = execute_started.elapsed().as_millis() as i64;

    match result {
        Ok(execution) => {
            let completion_started = Instant::now();
            let latest_run = repo.get_trade_flow_run(run.id).await?;
            let run_still_running = matches!(
                latest_run.as_ref().map(|current| current.status.as_str()),
                Some("running")
            );
            if !run_still_running {
                cancel_flow_step_side_effects_after_stop(repo, &run, step, node, &execution.output)
                    .await?;
                warn!(
                    run_id,
                    flow_run_id = run.id,
                    step_id = step.id,
                    node_key = %node.key,
                    "TRADE_FLOW_STEP_ABORTED_RUN_STOPPED"
                );
                info!(
                    run_id,
                    flow_run_id = run.id,
                    step_id = step.id,
                    node_key = %node.key,
                    node_type = %node.node_type,
                    outcome = "aborted_run_stopped",
                    claim_to_start_ms,
                    execute_ms,
                    completion_ms = completion_started.elapsed().as_millis() as i64,
                    total_ms = step_started.elapsed().as_millis() as i64,
                    "STEP_LATENCY_TRACE"
                );
                return Ok(());
            }

            repo.update_trade_flow_run_context(run.id, &context).await?;
            repo.mark_trade_flow_step_completed(step.id, Some(&execution.output))
                .await?;
            spawn_trade_flow_immediate_submit_if_needed(
                repo,
                run_id,
                cfg,
                ws,
                client.clone(),
                &execution.output,
            );
            repo.append_trade_flow_event(
                Some(run.id),
                run.definition_id,
                Some(run.version_id),
                "step_completed",
                &json!({
                    "step_id": step.id,
                    "node_key": node.key,
                    "node_type": node.node_type,
                    "routes": execution.routes.iter().map(|r| r.edge_type.clone()).collect::<Vec<_>>(),
                    "triggered": !execution.routes.is_empty(),
                    "trigger_price": execution.output.get("trigger_price"),
                    "triggered_price": execution.output.get("triggered_price"),
                    "max_price": execution.output.get("max_price"),
                    "trigger_condition": execution.output.get("triggered_condition"),
                    "current_price": execution.output.get("price"),
                    "market_slug": execution.output.get("market_slug")
                }),
            )
            .await?;

            for route in &execution.routes {
                enqueue_trade_flow_edges(
                    repo,
                    &run,
                    &graph,
                    &node.key,
                    &route.edge_type,
                    route.available_at,
                    step.id,
                    &execution.output,
                    &context,
                )
                .await?;
            }

            if let Some(repeat_at) = execution.repeat_at {
                let repeat_input = build_trade_flow_repeat_step_input(step, &execution.output);
                let enqueued = repo
                    .enqueue_trade_flow_step(
                        run.id,
                        &node.key,
                        &node.node_type,
                        step.attempt,
                        repeat_input.as_ref(),
                        repeat_at,
                        Some(step.id),
                        execution.repeat_idempotency_key.as_deref(),
                    )
                    .await?;
                if enqueued.is_some() {
                    schedule_flow_process_notify_at(repeat_at);
                }
            }
            info!(
                run_id,
                flow_run_id = run.id,
                step_id = step.id,
                node_key = %node.key,
                node_type = %node.node_type,
                outcome = "completed",
                claim_to_start_ms,
                execute_ms,
                completion_ms = completion_started.elapsed().as_millis() as i64,
                total_ms = step_started.elapsed().as_millis() as i64,
                "STEP_LATENCY_TRACE"
            );
        }
        Err(err) => {
            let completion_started = Instant::now();
            let output_json = json!({
                "error": err.to_string(),
                "node_key": node.key,
                "node_type": node.node_type
            });
            repo.mark_trade_flow_step_failed(step.id, Some(&output_json), &err.to_string())
                .await?;
            repo.append_trade_flow_event(
                Some(run.id),
                run.definition_id,
                Some(run.version_id),
                "step_failed",
                &json!({
                    "step_id": step.id,
                    "node_key": node.key,
                    "error": err.to_string()
                }),
            )
            .await?;
            enqueue_trade_flow_edges(
                repo,
                &run,
                &graph,
                &node.key,
                "on_error",
                Utc::now(),
                step.id,
                &output_json,
                &context,
            )
            .await?;
            info!(
                run_id,
                flow_run_id = run.id,
                step_id = step.id,
                node_key = %node.key,
                node_type = %node.node_type,
                outcome = "failed",
                claim_to_start_ms,
                execute_ms,
                completion_ms = completion_started.elapsed().as_millis() as i64,
                total_ms = step_started.elapsed().as_millis() as i64,
                error = %err,
                "STEP_LATENCY_TRACE"
            );
        }
    }

    Ok(())
}

fn trade_flow_should_inline_submit(output: &Value) -> bool {
    output
        .get("should_inline_submit")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn spawn_trade_flow_immediate_submit_if_needed(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    ws: &ClobWsClient,
    client: Option<SharedOrderExecutor>,
    output: &Value,
) {
    let Some(builder_order_id) = output.get("builder_order_id").and_then(Value::as_i64) else {
        return;
    };
    if !trade_flow_should_inline_submit(output) {
        return;
    }
    let Some(client) = client else {
        return;
    };

    let repo = repo.clone();
    let cfg = cfg.clone();
    let ws = ws.clone();
    tokio::spawn(async move {
        if let Err(err) = try_immediate_submit_single_builder_order(
            &repo,
            run_id,
            &cfg,
            &ws,
            client,
            builder_order_id,
            "immediate",
        )
        .await
        {
            warn!(
                run_id,
                builder_order_id,
                error = %err,
                "IMMEDIATE_SUBMIT_FAILED_WILL_RETRY_IN_HOUSEKEEPING"
            );
        }
    });
}

async fn cancel_flow_step_side_effects_after_stop(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    output: &Value,
) -> Result<()> {
    if let Some(builder_order_id) = output.get("builder_order_id").and_then(Value::as_i64) {
        repo.set_trade_builder_order_status(
            builder_order_id,
            "canceled_requested",
            Some("flow_run_stopped"),
        )
        .await?;
        repo.append_trade_builder_order_event(
            builder_order_id,
            "flow_run_stopped",
            &json!({
                "flow_run_id": run.id,
                "flow_definition_id": run.definition_id,
                "step_id": step.id,
                "node_key": node.key,
                "reason": "flow_run_stopped"
            }),
        )
        .await?;
    }

    if let Some(job_id) = output.get("job_id").and_then(Value::as_i64) {
        repo.set_trade_flow_dual_dca_job_status(job_id, "canceled", Some("flow_run_stopped"))
            .await?;
        repo.append_trade_flow_dual_dca_event(
            job_id,
            None,
            "job_canceled",
            &json!({
                "flow_run_id": run.id,
                "flow_definition_id": run.definition_id,
                "step_id": step.id,
                "node_key": node.key,
                "reason": "flow_run_stopped"
            }),
        )
        .await?;
    }

    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "step_canceled_run_stopped",
        &json!({
            "step_id": step.id,
            "node_key": node.key,
            "node_type": node.node_type,
            "reason": "flow_run_stopped"
        }),
    )
    .await?;

    Ok(())
}

fn parse_trade_flow_graph(version: &TradeFlowVersionRuntime) -> Result<TradeFlowGraphRuntime> {
    let root = version
        .graph_json
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("trade flow graph_json must be an object"))?;

    let context = root.get("context").cloned().unwrap_or_else(|| json!({}));

    let mut nodes = Vec::new();
    if let Some(raw_nodes) = root.get("nodes").and_then(Value::as_array) {
        for raw in raw_nodes {
            let Some(raw_obj) = raw.as_object() else {
                continue;
            };
            let key = raw_obj
                .get("key")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            let node_type = raw_obj
                .get("type")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            if key.is_empty() || node_type.is_empty() {
                continue;
            }
            nodes.push(TradeFlowNode {
                key: key.to_string(),
                node_type: node_type.to_string(),
                config: raw_obj.get("config").cloned().unwrap_or_else(|| json!({})),
            });
        }
    }

    let mut edges = Vec::new();
    if let Some(raw_edges) = root.get("edges").and_then(Value::as_array) {
        for raw in raw_edges {
            let Some(raw_obj) = raw.as_object() else {
                continue;
            };
            let source = raw_obj
                .get("source")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            let target = raw_obj
                .get("target")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            if source.is_empty() || target.is_empty() {
                continue;
            }
            let edge_type = raw_obj
                .get("type")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .unwrap_or("default");
            let condition = raw_obj
                .get("condition")
                .filter(|value| value.is_object())
                .cloned();
            edges.push(TradeFlowEdge {
                source: source.to_string(),
                target: target.to_string(),
                edge_type: edge_type.to_string(),
                condition,
            });
        }
    }

    Ok(TradeFlowGraphRuntime {
        context,
        nodes,
        edges,
    })
}

fn flow_node<'a>(graph: &'a TradeFlowGraphRuntime, key: &str) -> Option<&'a TradeFlowNode> {
    graph.nodes.iter().find(|node| node.key == key)
}

fn resolve_trade_flow_candidate_edges<'a>(
    graph: &'a TradeFlowGraphRuntime,
    source_key: &str,
    edge_type: &str,
) -> Vec<&'a TradeFlowEdge> {
    let mut edges = graph
        .edges
        .iter()
        .filter(|edge| edge.source == source_key && edge.edge_type == edge_type)
        .collect::<Vec<_>>();

    if edges.is_empty() && edge_type != "default" {
        edges = graph
            .edges
            .iter()
            .filter(|edge| edge.source == source_key && edge.edge_type == "default")
            .collect::<Vec<_>>();
    }

    edges
}

fn trade_flow_edge_condition_passes(edge: &TradeFlowEdge, eval_data: &Value) -> bool {
    edge.condition
        .as_ref()
        .map(|condition| value_truthy(&evaluate_jsonlogic(condition, eval_data)))
        .unwrap_or(true)
}

fn resolve_trade_flow_targets(
    graph: &TradeFlowGraphRuntime,
    source_key: &str,
    edge_type: &str,
    eval_data: &Value,
) -> Vec<String> {
    resolve_trade_flow_candidate_edges(graph, source_key, edge_type)
        .into_iter()
        .filter(|edge| trade_flow_edge_condition_passes(edge, eval_data))
        .map(|edge| edge.target.clone())
        .collect()
}

async fn enqueue_trade_flow_edges(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    graph: &TradeFlowGraphRuntime,
    source_key: &str,
    edge_type: &str,
    available_at: DateTime<Utc>,
    parent_step_id: i64,
    input_json: &Value,
    context: &Value,
) -> Result<()> {
    let eval_data = build_trade_flow_route_eval_data(context, input_json);
    let targets = resolve_trade_flow_targets(graph, source_key, edge_type, &eval_data);
    for target_key in targets {
        let Some(target_node) = flow_node(graph, &target_key) else {
            continue;
        };

        let attempt = if target_key == source_key {
            input_json
                .get("next_attempt")
                .and_then(value_as_i64)
                .unwrap_or(1)
                .max(1) as i32
        } else {
            1
        };

        let enqueued = repo
            .enqueue_trade_flow_step(
                run.id,
                &target_node.key,
                &target_node.node_type,
                attempt,
                Some(input_json),
                available_at,
                Some(parent_step_id),
                None,
            )
            .await?;
        schedule_enqueued_flow_process_notify(enqueued, available_at);
    }
    Ok(())
}

fn schedule_flow_process_notify_at(available_at: DateTime<Utc>) {
    schedule_flow_process_notify_at_with_notify(&FLOW_PROCESS_NOTIFY, available_at);
}

fn schedule_enqueued_flow_process_notify(
    enqueued_step_id: Option<i64>,
    available_at: DateTime<Utc>,
) {
    schedule_enqueued_flow_process_notify_with_notify(
        &FLOW_PROCESS_NOTIFY,
        enqueued_step_id,
        available_at,
    );
}

fn schedule_flow_process_notify_at_with_notify(
    notify: &'static Notify,
    available_at: DateTime<Utc>,
) {
    let delay = (available_at - Utc::now()).to_std().unwrap_or_default();
    if delay.is_zero() {
        notify.notify_one();
        return;
    }
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        notify.notify_one();
    });
}

fn schedule_enqueued_flow_process_notify_with_notify(
    notify: &'static Notify,
    enqueued_step_id: Option<i64>,
    available_at: DateTime<Utc>,
) {
    if enqueued_step_id.is_some() {
        schedule_flow_process_notify_at_with_notify(notify, available_at);
    }
}

#[cfg(test)]
mod part_012_tests {
    use super::*;

    fn test_repeat_step(input_json: Option<Value>) -> TradeFlowRunStep {
        TradeFlowRunStep {
            id: 1,
            run_id: 42,
            node_key: "action_1".to_string(),
            node_type: "action.place_order".to_string(),
            status: "queued".to_string(),
            attempt: 1,
            input_json,
            output_json: None,
            error_text: None,
            started_at: None,
            ended_at: None,
            available_at: Utc::now(),
            parent_step_id: None,
            idempotency_key: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn trade_flow_repeat_step_input_preserves_original_payload() {
        let input_json = json!({
            "market_slug": "sol-updown-5m-1773315300",
            "trigger_price": 0.77,
            "triggered_token_id": "tok-up"
        });
        let step = test_repeat_step(Some(input_json.clone()));

        assert_eq!(
            build_trade_flow_repeat_step_input(&step, &json!({ "reason": "other" })),
            Some(input_json)
        );
    }

    #[test]
    fn trade_flow_repeat_step_input_returns_none_without_original_payload() {
        let step = test_repeat_step(None);

        assert_eq!(
            build_trade_flow_repeat_step_input(&step, &json!({ "reason": "other" })),
            None
        );
    }

    #[tokio::test]
    async fn schedule_flow_process_notify_at_notifies_immediately_for_due_steps() {
        let notify = Box::leak(Box::new(Notify::new()));

        schedule_flow_process_notify_at_with_notify(notify, Utc::now());

        tokio::time::timeout(Duration::from_millis(20), notify.notified())
            .await
            .expect("immediate wake");
    }

    #[tokio::test]
    async fn schedule_flow_process_notify_at_waits_for_future_steps() {
        let notify = Box::leak(Box::new(Notify::new()));

        schedule_flow_process_notify_at_with_notify(
            notify,
            Utc::now() + ChronoDuration::milliseconds(50),
        );

        assert!(
            tokio::time::timeout(Duration::from_millis(10), notify.notified())
                .await
                .is_err(),
            "wake should not happen before the scheduled time",
        );
        tokio::time::timeout(Duration::from_millis(200), notify.notified())
            .await
            .expect("delayed wake");
    }

    #[tokio::test]
    async fn schedule_enqueued_flow_process_notify_skips_duplicate_noop_enqueue() {
        let notify = Box::leak(Box::new(Notify::new()));

        schedule_enqueued_flow_process_notify_with_notify(
            notify,
            None,
            Utc::now() + ChronoDuration::milliseconds(20),
        );

        assert!(
            tokio::time::timeout(Duration::from_millis(50), notify.notified())
                .await
                .is_err(),
            "no wake should be scheduled when enqueue was ignored",
        );
    }
}
