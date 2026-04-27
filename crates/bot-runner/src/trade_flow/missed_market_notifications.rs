#[derive(Debug, Clone)]
struct DueMissedMarketNotificationTarget {
    run_index: usize,
    node_index: usize,
    window_end_at: DateTime<Utc>,
}

const MISSED_MARKET_NOTIFICATION_STATE_PREFIX: &str = "missed_market_order_not_filled_";
const MISSED_MARKET_NOTIFICATION_SENT_EVENT: &str =
    "missed_market_order_not_filled_notification_sent";
const MISSED_MARKET_NOTIFICATION_SKIPPED_EVENT: &str =
    "missed_market_order_not_filled_notification_skipped";
const MISSED_MARKET_NO_ORDER_REASON_CODE: &str = "market_window_no_order";

fn missed_market_notification_state_key(node_spec: &WsOpenPositionPriceNodeSpec) -> String {
    format!(
        "{MISSED_MARKET_NOTIFICATION_STATE_PREFIX}{}",
        node_spec.token_id
    )
}

fn missed_market_notification_marker(
    node_spec: &WsOpenPositionPriceNodeSpec,
    window_end_at: DateTime<Utc>,
) -> Option<String> {
    Some(format!(
        "{}:{}:{}",
        node_spec.market_slug.as_deref()?,
        node_spec.token_id,
        window_end_at.timestamp()
    ))
}

fn missed_market_notification_already_marked(
    context: &Value,
    node_spec: &WsOpenPositionPriceNodeSpec,
    marker: &str,
) -> bool {
    flow_node_state(
        context,
        &node_spec.node_key,
        &missed_market_notification_state_key(node_spec),
    )
    .and_then(|value| value.get("marker"))
    .and_then(Value::as_str)
        == Some(marker)
}

fn set_missed_market_notification_marker(
    context: &mut Value,
    node_spec: &WsOpenPositionPriceNodeSpec,
    marker: &str,
    status: &str,
    reason_code: &str,
    sent: bool,
) {
    set_flow_node_state(
        context,
        &node_spec.node_key,
        &missed_market_notification_state_key(node_spec),
        json!({
            "marker": marker,
            "status": status,
            "reason_code": reason_code,
            "sent": sent,
            "updated_at": Utc::now().to_rfc3339(),
        }),
    );
}

fn missed_market_notification_due_target(
    run_spec: &WsOpenPositionPriceRunSpec,
    run_index: usize,
    node_spec: &WsOpenPositionPriceNodeSpec,
    node_index: usize,
    now: DateTime<Utc>,
) -> Option<DueMissedMarketNotificationTarget> {
    if node_spec.node_type != "trigger.market_price" {
        return None;
    }
    let (_, window_end_at) = cycle_window_bounds(node_spec)?;
    if now < window_end_at {
        return None;
    }
    let marker = missed_market_notification_marker(node_spec, window_end_at)?;
    if missed_market_notification_already_marked(&run_spec.context, node_spec, &marker) {
        return None;
    }
    Some(DueMissedMarketNotificationTarget {
        run_index,
        node_index,
        window_end_at,
    })
}

fn missed_market_notification_event_types() -> &'static [&'static str] {
    &[
        "pre_order_price_to_beat_blocked",
        "trigger_cycle_window_price_to_beat_gate_blocked",
        "trigger_ws_price_to_beat_gate_blocked",
        "trigger_cycle_window_above_max",
        "trigger_cycle_window_condition_not_met",
        MISSED_MARKET_NOTIFICATION_SENT_EVENT,
    ]
}

fn trade_flow_event_marker(event: &TradeFlowEventRecord) -> Option<&str> {
    event.payload_json.get("marker")?.as_str()
}

fn missed_market_sent_event_exists(events: &[TradeFlowEventRecord], marker: &str) -> bool {
    events.iter().any(|event| {
        event.event_type == MISSED_MARKET_NOTIFICATION_SENT_EVENT
            && trade_flow_event_marker(event) == Some(marker)
    })
}

fn downstream_action_place_order_notify_node_keys(
    version: &TradeFlowVersionRuntime,
    source_node_key: &str,
) -> Result<Vec<String>> {
    let graph = parse_trade_flow_graph(version)?;
    let mut stack = vec![source_node_key.to_string()];
    let mut visited = std::collections::HashSet::new();
    let mut action_node_keys = Vec::new();

    while let Some(source) = stack.pop() {
        for edge in graph.edges.iter().filter(|edge| edge.source == source) {
            if !visited.insert(edge.target.clone()) {
                continue;
            }
            let Some(node) = flow_node(&graph, &edge.target) else {
                continue;
            };
            if node.node_type == "action.place_order"
                && node_config_bool(node, "notifyOnOrderNotFilled").unwrap_or(false)
            {
                action_node_keys.push(node.key.clone());
                continue;
            }
            stack.push(edge.target.clone());
        }
    }

    Ok(action_node_keys)
}

async fn missed_market_notify_action_node_keys(
    repo: &PostgresRepository,
    run_spec: &WsOpenPositionPriceRunSpec,
    node_key: &str,
) -> Result<Vec<String>> {
    let Some(version) = repo.get_trade_flow_version(run_spec.version_id).await? else {
        return Ok(Vec::new());
    };
    downstream_action_place_order_notify_node_keys(&version, node_key)
}

fn build_default_missed_market_summary(
    node_spec: &WsOpenPositionPriceNodeSpec,
    window_end_at: DateTime<Utc>,
) -> TradeBuilderNoFillReasonSummary {
    no_fill_summary(
        "trigger_condition",
        "no_matching_block_event",
        Some("blocked"),
        json!({
            "market_slug": node_spec.market_slug,
            "token_id": node_spec.token_id,
            "outcome_label": node_spec.outcome_label,
            "trigger_condition": node_spec.trigger_condition,
            "trigger_price": node_spec.trigger_price,
            "max_price": node_spec.max_price,
            "window_end_at": window_end_at.to_rfc3339(),
        }),
        Some("no_matching_event"),
    )
}

fn action_failed_no_order_reason_code(error_text: &str) -> &'static str {
    let normalized = error_text.to_ascii_lowercase();
    if normalized.contains("requires sizeusdc > 0") {
        "action_failed_size_usdc_missing"
    } else {
        "action_failed"
    }
}

fn action_failed_text(step: &TradeFlowRunStep) -> String {
    step.error_text
        .as_deref()
        .or_else(|| {
            step.output_json
                .as_ref()
                .and_then(|output| output.get("error"))
                .and_then(Value::as_str)
        })
        .unwrap_or("action.place_order failed before order creation")
        .to_string()
}

fn build_action_failed_missed_market_summary(
    node_spec: &WsOpenPositionPriceNodeSpec,
    market_slug: &str,
    window_end_at: DateTime<Utc>,
    step: &TradeFlowRunStep,
) -> TradeBuilderNoFillReasonSummary {
    let error_text = action_failed_text(step);
    no_fill_summary(
        "action_failed",
        action_failed_no_order_reason_code(&error_text),
        Some("failed"),
        json!({
            "market_slug": market_slug,
            "token_id": node_spec.token_id,
            "outcome_label": node_spec.outcome_label,
            "trigger_condition": node_spec.trigger_condition,
            "trigger_price": node_spec.trigger_price,
            "max_price": node_spec.max_price,
            "window_end_at": window_end_at.to_rfc3339(),
            "action_node_key": step.node_key,
            "action_node_type": step.node_type,
            "action_step_id": step.id,
            "action_step_status": step.status,
            "action_error": error_text,
            "started_at": step.started_at.map(|value| value.to_rfc3339()),
            "ended_at": step.ended_at.map(|value| value.to_rfc3339()),
        }),
        Some("action_step_failed"),
    )
}

fn build_missed_market_no_order_notification_message(
    node_spec: &WsOpenPositionPriceNodeSpec,
    window_end_at: DateTime<Utc>,
    diagnosis: &Value,
) -> String {
    let is_trigger_condition =
        diagnosis.get("last_guard_scope").and_then(Value::as_str) == Some("trigger_condition");
    let is_action_failed =
        diagnosis.get("last_guard_scope").and_then(Value::as_str) == Some("action_failed");
    let title = if is_action_failed {
        "Emir Acilmadi - Action Failed"
    } else if is_trigger_condition {
        "Emir Acilmadi - Trigger Sarti Saglanmadi"
    } else {
        "Emir Acilmadi - Guard Beklerken Window Kapandi"
    };
    let reason = if is_action_failed {
        "Trigger gecti; action.place_order fail oldu ve builder order olusturmadi."
    } else if is_trigger_condition {
        "Trigger sarti market/window bitene kadar gecmedi; action.place_order order olusturmadi."
    } else {
        "Market/window bitti; action.place_order order olusturmadan kapandi."
    };
    let reason_code = if is_action_failed {
        diagnosis
            .get("last_guard_code")
            .and_then(Value::as_str)
            .unwrap_or(MISSED_MARKET_NO_ORDER_REASON_CODE)
    } else {
        MISSED_MARKET_NO_ORDER_REASON_CODE
    };
    let mut message = format!(
        "{title}\nSebep Kodu: {reason_code}\nSebep: {reason}\nMarket: {}\nOutcome: {}\nSide: buy\nToken: {}\nWindow End: {}",
        node_spec.market_slug.as_deref().unwrap_or("N/A"),
        node_spec.outcome_label,
        node_spec.token_id,
        window_end_at.to_rfc3339(),
    );
    message.push('\n');
    message.push_str(&build_missed_market_no_order_diagnosis_message_block(
        diagnosis,
    ));
    message
}

async fn mark_missed_market_notification_skipped(
    repo: &PostgresRepository,
    run_spec: &mut WsOpenPositionPriceRunSpec,
    node_spec: &WsOpenPositionPriceNodeSpec,
    marker: &str,
    window_end_at: DateTime<Utc>,
    reason_code: &str,
) -> Result<()> {
    set_missed_market_notification_marker(
        &mut run_spec.context,
        node_spec,
        marker,
        "skipped",
        reason_code,
        false,
    );
    run_spec.context_dirty = true;
    repo.append_trade_flow_event(
        Some(run_spec.run_id),
        run_spec.definition_id,
        Some(run_spec.version_id),
        MISSED_MARKET_NOTIFICATION_SKIPPED_EVENT,
        &json!({
            "marker": marker,
            "node_key": node_spec.node_key,
            "node_type": node_spec.node_type,
            "market_slug": node_spec.market_slug,
            "token_id": node_spec.token_id,
            "outcome_label": node_spec.outcome_label,
            "window_end_at": window_end_at.to_rfc3339(),
            "reason_code": reason_code,
        }),
    )
    .await?;
    Ok(())
}

async fn maybe_send_missed_market_no_order_notification(
    repo: &PostgresRepository,
    client: Option<&dyn OrderExecutor>,
    run_spec: &mut WsOpenPositionPriceRunSpec,
    node_spec: &WsOpenPositionPriceNodeSpec,
    window_end_at: DateTime<Utc>,
) -> Result<bool> {
    let Some(market_slug) = node_spec.market_slug.as_deref() else {
        return Ok(false);
    };
    let Some(marker) = missed_market_notification_marker(node_spec, window_end_at) else {
        return Ok(false);
    };
    if missed_market_notification_already_marked(&run_spec.context, node_spec, &marker) {
        return Ok(false);
    }

    let events = repo
        .list_trade_flow_events_for_run_types(
            run_spec.run_id,
            missed_market_notification_event_types(),
        )
        .await?;
    if missed_market_sent_event_exists(&events, &marker) {
        set_missed_market_notification_marker(
            &mut run_spec.context,
            node_spec,
            &marker,
            "sent",
            MISSED_MARKET_NO_ORDER_REASON_CODE,
            true,
        );
        run_spec.context_dirty = true;
        return Ok(true);
    }

    if repo
        .has_trade_flow_run_builder_order_for_node_market_token(
            run_spec.run_id,
            &node_spec.node_key,
            market_slug,
            &node_spec.token_id,
        )
        .await?
    {
        mark_missed_market_notification_skipped(
            repo,
            run_spec,
            node_spec,
            &marker,
            window_end_at,
            "builder_order_exists",
        )
        .await?;
        return Ok(true);
    }

    let action_node_keys =
        missed_market_notify_action_node_keys(repo, run_spec, &node_spec.node_key).await?;
    if action_node_keys.is_empty() {
        mark_missed_market_notification_skipped(
            repo,
            run_spec,
            node_spec,
            &marker,
            window_end_at,
            "notify_on_order_not_filled_disabled",
        )
        .await?;
        return Ok(true);
    }

    let action_steps = repo
        .list_completed_place_order_blocked_steps_for_nodes_market_token(
            run_spec.run_id,
            &action_node_keys,
            market_slug,
            &node_spec.token_id,
        )
        .await?;
    let failed_action_steps = repo
        .list_failed_place_order_steps_for_nodes_market_token(
            run_spec.run_id,
            &action_node_keys,
            market_slug,
            &node_spec.token_id,
        )
        .await?;
    let action_failure_summary = failed_action_steps
        .last()
        .map(|step| build_action_failed_missed_market_summary(node_spec, market_slug, window_end_at, step));
    let action_output_summary = action_steps
        .iter()
        .rev()
        .filter_map(|step| step.output_json.as_ref())
        .find_map(|output| {
            no_fill_summary_from_action_place_order_output(
                output,
                &node_spec.token_id,
                &node_spec.outcome_label,
            )
        });
    let summary = action_output_summary
        .or(action_failure_summary)
        .or_else(|| {
            latest_trade_flow_no_fill_summary(
                &events,
                &node_spec.node_key,
                market_slug,
                &node_spec.token_id,
            )
        })
        .unwrap_or_else(|| build_default_missed_market_summary(node_spec, window_end_at));
    let mut diagnosis_steps = action_steps.clone();
    diagnosis_steps.extend(failed_action_steps);
    let diagnosis = build_missed_market_no_order_diagnosis_payload(
        repo,
        client,
        &run_spec.context,
        market_slug,
        &node_spec.token_id,
        &node_spec.outcome_label,
        window_end_at,
        &summary,
        &diagnosis_steps,
    )
    .await;
    let Some(run) = repo.get_trade_flow_run(run_spec.run_id).await? else {
        return Ok(false);
    };
    let message =
        build_missed_market_no_order_notification_message(node_spec, window_end_at, &diagnosis);
    let sent = send_trade_flow_notification(
        repo,
        &run,
        &node_spec.node_key,
        "order_not_filled",
        &message,
    )
    .await;
    set_missed_market_notification_marker(
        &mut run_spec.context,
        node_spec,
        &marker,
        if sent { "sent" } else { "attempted" },
        &summary.reason_code,
        sent,
    );
    run_spec.context_dirty = true;
    if sent {
        let mut event_payload = diagnosis.clone();
        if let Some(event_obj) = event_payload.as_object_mut() {
            event_obj.insert("marker".to_string(), json!(marker));
            event_obj.insert("node_key".to_string(), json!(node_spec.node_key));
            event_obj.insert("node_type".to_string(), json!(node_spec.node_type));
            event_obj.insert("market_slug".to_string(), json!(node_spec.market_slug));
            event_obj.insert("token_id".to_string(), json!(node_spec.token_id));
            event_obj.insert("outcome_label".to_string(), json!(node_spec.outcome_label));
            event_obj.insert(
                "window_end_at".to_string(),
                json!(window_end_at.to_rfc3339()),
            );
            event_obj.insert("reason_code".to_string(), json!(summary.reason_code));
            event_obj.insert("scope".to_string(), json!(summary.scope));
            event_obj.insert("source_event".to_string(), json!(summary.source_event));
            event_obj.insert("no_order_diagnosis".to_string(), diagnosis.clone());
        }
        repo.append_trade_flow_event(
            Some(run_spec.run_id),
            run_spec.definition_id,
            Some(run_spec.version_id),
            MISSED_MARKET_NOTIFICATION_SENT_EVENT,
            &event_payload,
        )
        .await?;
    }

    Ok(true)
}
