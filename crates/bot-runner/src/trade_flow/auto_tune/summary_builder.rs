pub(super) async fn maybe_record_trade_flow_auto_tune_market(
    repo: &PostgresRepository,
    client: Option<&dyn OrderExecutor>,
    run_spec: &WsOpenPositionPriceRunSpec,
    node_spec: &WsOpenPositionPriceNodeSpec,
    window_end_at: DateTime<Utc>,
) -> Result<()> {
    if node_spec.node_type != "trigger.market_price" {
        return Ok(());
    }
    let Some(version) = repo.get_trade_flow_version(run_spec.version_id).await? else {
        return Ok(());
    };
    let graph = parse_trade_flow_graph(&version)?;
    let Some(market_slug) = node_spec.market_slug.as_deref() else {
        return Ok(());
    };
    let action_nodes = auto_tune_downstream_action_place_order_nodes(&graph, &node_spec.node_key);
    let enabled_action_nodes = action_nodes
        .into_iter()
        .filter_map(|action_node| {
            let cfg = AutoTuneConfig::from_action_graph_and_run_context(
                Some(&action_node.config),
                Some(&graph.context),
                &run_spec.context,
            );
            let adaptive_summary_enabled =
                crate::action_place_order_uses_adaptive_max_price_strategy(&action_node);
            (cfg.advice_enabled() || adaptive_summary_enabled).then_some((action_node, cfg))
        })
        .collect::<Vec<_>>();
    if enabled_action_nodes.is_empty() {
        return Ok(());
    }
    let events = repo
        .list_trade_flow_events_for_run_types(
            run_spec.run_id,
            missed_market_notification_event_types(),
        )
        .await?;

    for (action_node, cfg) in enabled_action_nodes {
        let action_node_keys = vec![action_node.key.clone()];
        let action_steps = repo
            .list_completed_place_order_blocked_steps_for_nodes_market_token(
                run_spec.run_id,
                &action_node_keys,
                market_slug,
                &node_spec.token_id,
            )
            .await?;
        let failed_steps = repo
            .list_failed_place_order_steps_for_nodes_market_token(
                run_spec.run_id,
                &action_node_keys,
                market_slug,
                &node_spec.token_id,
            )
            .await?;
        let order_rollup = repo
            .load_trade_flow_auto_tune_order_rollup(run_spec.run_id, &action_node.key, market_slug)
            .await?;
        let terminal = first_terminal_guard_for_auto_tune(
            &events,
            &action_steps,
            &failed_steps,
            node_spec,
            &action_node,
            market_slug,
            window_end_at,
        );
        let summary = terminal
            .as_ref()
            .map(|(_, summary)| summary.clone())
            .unwrap_or_else(|| build_default_missed_market_summary(node_spec, window_end_at));
        let diagnosis = build_missed_market_no_order_diagnosis_payload(
            repo,
            client,
            &run_spec.context,
            &node_spec.node_key,
            market_slug,
            &node_spec.token_id,
            &node_spec.outcome_label,
            window_end_at,
            &summary,
            &events,
            &action_steps,
        )
        .await;
        let input = auto_tune_summary_input(
            run_spec,
            node_spec,
            &action_node,
            market_slug,
            window_end_at,
            terminal,
            &summary,
            &diagnosis,
            &action_steps,
            &failed_steps,
            order_rollup,
        );
        repo.upsert_trade_flow_auto_tune_market_summary(&input).await?;
        if crate::action_place_order_uses_adaptive_max_price_strategy(&action_node) {
            crate::maybe_notify_pair_lock_adaptive_market_summary(
                repo,
                run_spec,
                &action_node,
                &input,
            )
            .await?;
        }
        if cfg.advice_enabled() {
            maybe_emit_trade_flow_auto_tune_advice(
                repo,
                run_spec,
                &action_node,
                &input.market_scope,
                &cfg,
            )
            .await?;
        }
    }

    Ok(())
}

fn auto_tune_downstream_action_place_order_nodes(
    graph: &TradeFlowGraphRuntime,
    source_node_key: &str,
) -> Vec<TradeFlowNode> {
    let mut stack = vec![source_node_key.to_string()];
    let mut visited = HashSet::new();
    let mut action_nodes = Vec::new();

    while let Some(source) = stack.pop() {
        for edge in graph.edges.iter().filter(|edge| edge.source == source) {
            if !visited.insert(edge.target.clone()) {
                continue;
            }
            let Some(node) = flow_node(&graph, &edge.target) else {
                continue;
            };
            if node.node_type == "action.place_order" {
                action_nodes.push(node.clone());
                continue;
            }
            stack.push(edge.target.clone());
        }
    }

    action_nodes
}

fn auto_tune_adaptive_max_price_payload_for_outcome(
    metrics_source: &Value,
    outcome_label: &str,
) -> Value {
    let target = normalize_pair_lock_binary_outcome(outcome_label);
    let candidates = [
        metrics_source.get("adaptive_max_price"),
        metrics_source.pointer("/primary_selection/adaptive_max_price"),
        metrics_source.pointer("/yes_candidate_guard/adaptive_max_price"),
        metrics_source.pointer("/no_candidate_guard/adaptive_max_price"),
        metrics_source.pointer("/primary_selection/yes_candidate_guard/adaptive_max_price"),
        metrics_source.pointer("/primary_selection/no_candidate_guard/adaptive_max_price"),
    ];
    for candidate in candidates.into_iter().flatten() {
        let candidate_outcome = candidate
            .get("outcome_label")
            .and_then(Value::as_str)
            .and_then(normalize_pair_lock_binary_outcome);
        if candidate_outcome.is_none() || candidate_outcome == target {
            return candidate.clone();
        }
    }
    Value::Null
}

fn first_terminal_guard_for_auto_tune(
    events: &[TradeFlowEventRecord],
    action_steps: &[TradeFlowRunStep],
    failed_steps: &[TradeFlowRunStep],
    node_spec: &WsOpenPositionPriceNodeSpec,
    action_node: &TradeFlowNode,
    market_slug: &str,
    window_end_at: DateTime<Utc>,
) -> Option<(AutoTuneGuardDecision, TradeBuilderNoFillReasonSummary)> {
    let mut candidates = Vec::new();
    for step in action_steps {
        let Some(output) = step.output_json.as_ref() else {
            continue;
        };
        let Some(summary) = no_fill_summary_from_action_place_order_output(
            output,
            &node_spec.token_id,
            &node_spec.outcome_label,
        ) else {
            continue;
        };
        candidates.push((
            no_order_step_time(step),
            Some(step.node_key.clone()),
            summary,
        ));
    }
    for step in failed_steps {
        let summary =
            build_action_failed_missed_market_summary(node_spec, market_slug, window_end_at, step);
        candidates.push((
            no_order_step_time(step),
            Some(step.node_key.clone()),
            summary,
        ));
    }
    for event in events.iter().rev() {
        if !trade_flow_event_matches_market_node(
            event,
            &node_spec.node_key,
            market_slug,
            &node_spec.token_id,
        ) {
            continue;
        }
        let Some(summary) = no_fill_summary_from_trade_flow_event(event) else {
            continue;
        };
        candidates.push((
            event.created_at,
            Some(node_spec.node_key.clone()),
            summary,
        ));
    }

    candidates
        .into_iter()
        .min_by_key(|(at, _, _)| *at)
        .map(|(at, node_key, summary)| {
            (
                AutoTuneGuardDecision {
                    scope: Some(summary.scope.clone()),
                    code: Some(summary.reason_code.clone()),
                    node_key: node_key.or_else(|| Some(action_node.key.clone())),
                    at: Some(at),
                },
                summary,
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn auto_tune_summary_input(
    run_spec: &WsOpenPositionPriceRunSpec,
    node_spec: &WsOpenPositionPriceNodeSpec,
    action_node: &TradeFlowNode,
    market_slug: &str,
    window_end_at: DateTime<Utc>,
    terminal: Option<(AutoTuneGuardDecision, TradeBuilderNoFillReasonSummary)>,
    summary: &TradeBuilderNoFillReasonSummary,
    diagnosis: &Value,
    action_steps: &[TradeFlowRunStep],
    failed_steps: &[TradeFlowRunStep],
    order_rollup: bot_infra::db::TradeFlowAutoTuneOrderRollup,
) -> bot_infra::db::TradeFlowAutoTuneMarketSummaryInput {
    let first_guard = terminal.map(|(guard, _)| guard);
    let latest_output = action_steps
        .iter()
        .rev()
        .find_map(|step| step.output_json.as_ref());
    let metrics_source = latest_output.unwrap_or(&summary.payload);
    let first_scope = first_guard
        .as_ref()
        .and_then(|guard| guard.scope.clone())
        .or_else(|| diagnosis.get("last_guard_scope").and_then(Value::as_str).map(str::to_string));
    let first_code = first_guard
        .as_ref()
        .and_then(|guard| guard.code.clone())
        .or_else(|| diagnosis.get("last_guard_code").and_then(Value::as_str).map(str::to_string));
    let last_guard_scope = diagnosis
        .get("last_guard_scope")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| first_scope.clone());
    let last_guard_code = diagnosis
        .get("last_guard_code")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| first_code.clone());
    let (max_price_block, execution_floor_block, ptb_block, pair_total_block, counter_max_block, counter_floor_block, risk_block, data_problem_block) =
        classify_auto_tune_blockers(first_scope.as_deref(), first_code.as_deref(), diagnosis);
    let remaining_sec = auto_tune_remaining_sec(window_end_at, first_guard.as_ref().and_then(|guard| guard.at));
    let best_ask_at_block = auto_tune_json_f64_path(
        diagnosis,
        &[&["best_ask_at_block"], &["best_ask_at_window_end"], &["selected_ask"]],
    );
    let execution_floor_effective = auto_tune_json_f64_path(diagnosis, &[&["execution_floor"]]);
    let max_price_effective = auto_tune_price_value(auto_tune_json_f64_path(
        &summary.payload,
        &[
            &["details", "max_price"],
            &["max_price"],
            &["effective_max_price"],
            &["max_price_usdc"],
        ],
    ));
    let pair_total_effective = auto_tune_price_value(auto_tune_json_f64_path(
        metrics_source,
        &[
            &["pair_total"],
            &["pair_total_effective"],
            &["details", "pair_total"],
            &["primary_selection", "pair_total"],
        ],
    ));
    let counter_price_effective = auto_tune_price_value(auto_tune_json_f64_path(
        metrics_source,
        &[
            &["counter_price"],
            &["counter_best_ask"],
            &["counter", "best_ask"],
            &["primary_selection", "counter", "best_ask"],
        ],
    ));
    let iv_edge_margin = auto_tune_json_f64_path(
        metrics_source,
        &[
            &["iv_edge_margin"],
            &["edge_margin"],
            &["price_to_beat_guard", "iv_edge_margin"],
            &["price_to_beat_guard", "edge_margin"],
        ],
    );
    let iv_dynamic_threshold = auto_tune_json_f64_path(
        metrics_source,
        &[
            &["iv_dynamic_threshold"],
            &["dynamic_threshold"],
            &["price_to_beat_guard", "dynamic_threshold"],
        ],
    );
    let gap_strength = auto_tune_json_f64_path(
        metrics_source,
        &[
            &["gap_strength"],
            &["gap_strength_margin"],
            &["price_to_beat_guard", "gap_strength"],
        ],
    );
    let required_gap_strength = auto_tune_json_f64_path(
        metrics_source,
        &[
            &["required_gap_strength"],
            &["price_to_beat_guard", "required_gap_strength"],
        ],
    );
    let binance_stale_ms = auto_tune_json_i64_path(
        metrics_source,
        &[&["binance_stale_ms"], &["snapshot_age_ms"], &["price_to_beat_guard", "binance_stale_ms"]],
    );
    let binance_same_direction = auto_tune_json_bool_path(
        metrics_source,
        &[&["binance_same_direction"], &["same_direction"], &["price_to_beat_guard", "same_direction"]],
    );
    let selected_depth = auto_tune_json_f64_path(diagnosis, &[&["selected_side_depth"]]);
    let book_complete =
        diagnosis.get("book_data_status").and_then(Value::as_str) == Some("complete_pair_book");
    let depth_ok = selected_depth
        .map(|value| value > 0.0)
        .or(Some(book_complete))
        .filter(|value| *value || selected_depth.is_some());
    let max_best_ask_after_block =
        auto_tune_json_f64_path(diagnosis, &[&["max_best_ask_during_wait"]]);
    let tradable_seconds_count = auto_tune_json_i64_path(
        metrics_source,
        &[
            &["tradable_seconds_count"],
            &["max_price_relax_tradable_seconds_count"],
            &["floor_tradable_seconds_count"],
        ],
    );
    let depth_ok_seconds_count = auto_tune_json_i64_path(
        metrics_source,
        &[&["depth_ok_seconds_count"], &["floor_depth_ok_seconds_count"]],
    );
    let metrics_json = json!({
        "mode": "advice",
        "source": "auto_tune_v1",
        "trigger_node_key": node_spec.node_key,
        "action_node_key": action_node.key,
        "market_slug": market_slug,
        "token_id": node_spec.token_id,
        "outcome_label": node_spec.outcome_label,
        "remaining_sec_at_first_terminal_guard": remaining_sec,
        "first_terminal_guard_scope": first_scope,
        "first_terminal_guard_code": first_code,
        "last_guard_scope": last_guard_scope,
        "last_guard_code": last_guard_code,
        "action_blocked_step_count": action_steps.len(),
        "action_failed_step_count": failed_steps.len(),
        "book_data_status": diagnosis.get("book_data_status").cloned().unwrap_or(Value::Null),
        "quote_missing_reason": diagnosis.get("quote_missing_reason").cloned().unwrap_or(Value::Null),
        "depth_ok_seconds_count": depth_ok_seconds_count,
        "adaptive_max_price": auto_tune_adaptive_max_price_payload_for_outcome(metrics_source, &node_spec.outcome_label),
    });

    bot_infra::db::TradeFlowAutoTuneMarketSummaryInput {
        definition_id: run_spec.definition_id,
        version_id: run_spec.version_id,
        flow_run_id: Some(run_spec.run_id),
        node_key: action_node.key.clone(),
        market_scope: auto_tune_scope_for_market(market_slug),
        market_slug: market_slug.to_string(),
        window_start: auto_tune_market_window_start(market_slug),
        window_end: Some(window_end_at),
        completed_at: Utc::now(),
        trigger_passed: !action_steps.is_empty()
            || !failed_steps.is_empty()
            || order_rollup.builder_order_created,
        action_started: !action_steps.is_empty()
            || !failed_steps.is_empty()
            || order_rollup.builder_order_created,
        builder_order_created: order_rollup.builder_order_created,
        order_submitted: order_rollup.order_submitted,
        order_filled: order_rollup.order_filled,
        first_terminal_guard_scope: first_guard.as_ref().and_then(|guard| guard.scope.clone()),
        first_terminal_guard_code: first_guard.as_ref().and_then(|guard| guard.code.clone()),
        first_terminal_guard_node: first_guard.as_ref().and_then(|guard| guard.node_key.clone()),
        first_terminal_guard_at: first_guard.as_ref().and_then(|guard| guard.at),
        last_guard_scope,
        last_guard_code,
        max_price_block,
        execution_floor_block,
        ptb_block,
        pair_total_block,
        counter_max_block,
        counter_floor_block,
        risk_block,
        data_problem_block,
        best_ask_at_block,
        max_price_effective,
        execution_floor_effective,
        pair_total_effective,
        counter_price_effective,
        iv_edge_margin,
        iv_dynamic_threshold,
        gap_strength,
        required_gap_strength,
        binance_stale_ms,
        binance_same_direction,
        depth_ok,
        floor_recovered_once: diagnosis
            .get("floor_recovered_once")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        max_best_ask_after_block,
        tradable_seconds_count,
        depth_ok_seconds_count,
        pair_session_id: order_rollup.pair_session_id,
        pair_locked: order_rollup.pair_locked,
        locked_qty: order_rollup.locked_qty,
        unpaired_qty: order_rollup.unpaired_qty,
        locked_profit_per_share: order_rollup.locked_profit_per_share,
        orphan_detected: order_rollup.orphan_detected,
        protective_unwind_triggered: order_rollup.protective_unwind_triggered,
        sl_hit: order_rollup.sl_hit,
        tp_hit: order_rollup.tp_hit,
        realized_pnl_usdc: order_rollup.realized_pnl_usdc,
        metrics_json,
    }
}

fn classify_auto_tune_blockers(
    scope: Option<&str>,
    code: Option<&str>,
    diagnosis: &Value,
) -> (bool, bool, bool, bool, bool, bool, bool, bool) {
    let scope = scope.unwrap_or_default();
    let code = code.unwrap_or_default();
    let code_lc = code.to_ascii_lowercase();
    let max_price_block = scope == "max_price" || code_lc.contains("above_max_price");
    let execution_floor_block = scope == "execution_floor"
        || code_lc.contains("below_best_ask_floor")
        || code_lc.contains("execution_floor");
    let ptb_block = scope == "price_to_beat" || code_lc.contains("price_to_beat");
    let pair_total_block = code_lc.contains("pair_total")
        || code_lc.contains("pair_max_total")
        || code_lc.contains("pair_total_above_max");
    let counter_max_block =
        code_lc.contains("counter") && (code_lc.contains("above_max") || code_lc.contains("max_price"));
    let counter_floor_block = code_lc.contains("counter") && code_lc.contains("floor");
    let risk_block = scope.contains("risk") || code_lc.contains("risk") || code_lc.contains("budget");
    let data_problem_block = scope == "runtime_price"
        || code_lc.contains("stale")
        || code_lc.contains("unavailable")
        || code_lc.contains("missing")
        || code_lc.contains("depth")
        || matches!(
            diagnosis.get("book_data_status").and_then(Value::as_str),
            Some("unavailable" | "incomplete_pair_book")
        );
    (
        max_price_block,
        execution_floor_block,
        ptb_block,
        pair_total_block,
        counter_max_block,
        counter_floor_block,
        risk_block,
        data_problem_block,
    )
}
