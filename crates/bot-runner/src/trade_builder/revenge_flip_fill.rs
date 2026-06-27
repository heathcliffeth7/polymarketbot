fn revenge_flip_marker_config(payload: &Value) -> Option<&Value> {
    payload
        .pointer("/node_snapshot/action_node/config")
        .filter(|config| {
            config
                .get(REVENGE_FLIP_ORDER_MARKER_KEY)
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
}

fn revenge_flip_marker_string(config: &Value, key: &str) -> Option<String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn revenge_flip_marker_f64(config: &Value, key: &str) -> Option<f64> {
    config.get(key).and_then(value_as_f64)
}

fn revenge_flip_marker_bool(config: &Value, key: &str) -> Option<bool> {
    match config.get(key)? {
        Value::Bool(value) => Some(*value),
        Value::Number(value) => value.as_i64().map(|number| number != 0),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn revenge_flip_marker_side(config: &Value, outcome_label: &str) -> Option<String> {
    revenge_flip_marker_string(config, REVENGE_FLIP_SIDE_KEY).or_else(|| {
        if normalize_pair_lock_binary_outcome(outcome_label) == Some("no") {
            Some("down".to_string())
        } else {
            Some("up".to_string())
        }
    })
}

const REVENGE_FLIP_STOP_LOSS_SELL_INTENT: &str = "stop_loss_sell";
const REVENGE_FLIP_STOP_LOSS_WAKE_REASON: &str = "revenge_flip_stop_loss_fill";
const REVENGE_FLIP_STOP_LOSS_WAKE_NODE_TYPE: &str = "trigger.market_price";

fn revenge_flip_stop_loss_wake_marker_matches(
    order: &TradeBuilderOrder,
    intent: &str,
    applied: bool,
) -> bool {
    applied && order.side == "sell" && intent == REVENGE_FLIP_STOP_LOSS_SELL_INTENT
}

#[cfg(test)]
fn revenge_flip_stop_loss_wake_eligible(
    order: &TradeBuilderOrder,
    intent: &str,
    applied: bool,
    root_node_key: Option<&str>,
) -> bool {
    revenge_flip_stop_loss_wake_marker_matches(order, intent, applied)
        && order.origin_flow_run_id.is_some()
        && root_node_key
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
}

fn revenge_flip_stop_loss_wake_idempotency_key(
    run_id: i64,
    root_node_key: &str,
    sell_order_id: i64,
) -> String {
    format!("revenge_flip_stop_loss_wake:{run_id}:{root_node_key}:{sell_order_id}")
}

fn revenge_flip_stop_loss_wake_input(order: &TradeBuilderOrder) -> Value {
    let mut payload = json!({
        "marketSlug": order.market_slug,
        "wakeReason": REVENGE_FLIP_STOP_LOSS_WAKE_REASON,
        "sourceBuilderOrderId": order.id,
    });
    if order.trade_id > 0 {
        payload["sourceTradeId"] = json!(order.trade_id);
    }
    payload
}

async fn append_revenge_flip_stop_loss_wake_skipped(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    reason: &str,
    flow_run_id: Option<i64>,
    root_node_key: Option<&str>,
    trigger_node_key: Option<&str>,
    detail: Option<Value>,
) -> Result<()> {
    repo.append_trade_builder_order_event(
        order.id,
        "revenge_flip_stop_loss_wake_skipped",
        &json!({
            "reason": reason,
            "flowRunId": flow_run_id,
            "rootNodeKey": root_node_key,
            "triggerNodeKey": trigger_node_key,
            "marketSlug": order.market_slug,
            "sourceBuilderOrderId": order.id,
            "detail": detail,
        }),
    )
    .await
}

async fn maybe_schedule_revenge_flip_stop_loss_wake(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    root_node_key: Option<&str>,
) -> Result<()> {
    let Some(run_id) = order.origin_flow_run_id else {
        append_revenge_flip_stop_loss_wake_skipped(
            repo,
            order,
            "missing_origin_flow_run",
            None,
            root_node_key,
            None,
            None,
        )
        .await?;
        return Ok(());
    };
    let Some(root_node_key) = root_node_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
    else {
        append_revenge_flip_stop_loss_wake_skipped(
            repo,
            order,
            "missing_root_node_key",
            Some(run_id),
            None,
            None,
            None,
        )
        .await?;
        return Ok(());
    };

    let Some(flow_run) = repo.get_trade_flow_run(run_id).await? else {
        append_revenge_flip_stop_loss_wake_skipped(
            repo,
            order,
            "flow_run_missing",
            Some(run_id),
            Some(root_node_key.as_str()),
            None,
            None,
        )
        .await?;
        return Ok(());
    };
    if flow_run.status != "running" {
        append_revenge_flip_stop_loss_wake_skipped(
            repo,
            order,
            "flow_run_not_running",
            Some(run_id),
            Some(root_node_key.as_str()),
            None,
            Some(json!({ "runStatus": flow_run.status })),
        )
        .await?;
        return Ok(());
    }
    let Some(version) = repo.get_trade_flow_version(flow_run.version_id).await? else {
        append_revenge_flip_stop_loss_wake_skipped(
            repo,
            order,
            "flow_version_missing",
            Some(run_id),
            Some(root_node_key.as_str()),
            None,
            Some(json!({ "versionId": flow_run.version_id })),
        )
        .await?;
        return Ok(());
    };
    let graph = match parse_trade_flow_graph(&version) {
        Ok(graph) => graph,
        Err(err) => {
            append_revenge_flip_stop_loss_wake_skipped(
                repo,
                order,
                "flow_graph_parse_failed",
                Some(run_id),
                Some(root_node_key.as_str()),
                None,
                Some(json!({ "error": err.to_string() })),
            )
            .await?;
            return Ok(());
        }
    };
    let Some(action_node) = flow_node(&graph, &root_node_key) else {
        append_revenge_flip_stop_loss_wake_skipped(
            repo,
            order,
            "root_action_node_missing",
            Some(run_id),
            Some(root_node_key.as_str()),
            None,
            None,
        )
        .await?;
        return Ok(());
    };
    if action_node.node_type != "action.place_order" {
        append_revenge_flip_stop_loss_wake_skipped(
            repo,
            order,
            "root_action_node_type_mismatch",
            Some(run_id),
            Some(root_node_key.as_str()),
            None,
            Some(json!({ "nodeType": action_node.node_type })),
        )
        .await?;
        return Ok(());
    }

    let Some(trigger_node_key) = find_upstream_market_price_trigger_key(&root_node_key, &graph)
    else {
        append_revenge_flip_stop_loss_wake_skipped(
            repo,
            order,
            "upstream_market_price_trigger_missing",
            Some(run_id),
            Some(root_node_key.as_str()),
            None,
            None,
        )
        .await?;
        return Ok(());
    };
    let Some(trigger_node) = flow_node(&graph, &trigger_node_key) else {
        append_revenge_flip_stop_loss_wake_skipped(
            repo,
            order,
            "upstream_market_price_trigger_missing",
            Some(run_id),
            Some(root_node_key.as_str()),
            Some(trigger_node_key.as_str()),
            None,
        )
        .await?;
        return Ok(());
    };
    let trigger_node_key = trigger_node.key.clone();
    let input_json = revenge_flip_stop_loss_wake_input(order);
    let idempotency_key =
        revenge_flip_stop_loss_wake_idempotency_key(run_id, &root_node_key, order.id);
    let enqueued = repo
        .enqueue_trade_flow_step(
            run_id,
            &trigger_node_key,
            REVENGE_FLIP_STOP_LOSS_WAKE_NODE_TYPE,
            1,
            Some(&input_json),
            Utc::now(),
            None,
            Some(&idempotency_key),
        )
        .await?;
    if enqueued.is_some() {
        FLOW_PROCESS_NOTIFY.notify_one();
    }

    repo.append_trade_builder_order_event(
        order.id,
        if enqueued.is_some() {
            "revenge_flip_stop_loss_wake_scheduled"
        } else {
            "revenge_flip_stop_loss_wake_duplicate_ignored"
        },
        &json!({
            "flowRunId": run_id,
            "rootNodeKey": root_node_key,
            "triggerNodeKey": trigger_node_key,
            "stepId": enqueued,
            "idempotencyKey": idempotency_key,
            "input": input_json,
        }),
    )
    .await?;

    Ok(())
}

async fn maybe_record_revenge_flip_fill(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    parent_order: Option<&TradeBuilderOrder>,
    flow_created_payload: Option<&Value>,
    fill_qty: f64,
    execution_price: f64,
) -> Result<()> {
    let Some(marker_config) = flow_created_payload.and_then(revenge_flip_marker_config) else {
        return Ok(());
    };
    let Some(flow_definition_id) = order.origin_flow_definition_id else {
        return Ok(());
    };
    let root_action_node_key = revenge_flip_marker_string(marker_config, REVENGE_FLIP_ROOT_NODE_KEY)
        .or_else(|| order.origin_flow_node_key.clone());
    let root_node_key = root_action_node_key
        .clone()
        .unwrap_or_else(|| order.market_slug.clone());
    let Some(revenge_side) = revenge_flip_marker_side(marker_config, &order.outcome_label) else {
        return Ok(());
    };
    if revenge_side != "up" && revenge_side != "down" {
        return Ok(());
    }
    let intent = revenge_flip_marker_string(marker_config, REVENGE_FLIP_INTENT_KEY)
        .unwrap_or_else(|| "unknown".to_string());
    let applied = repo
        .record_trade_builder_revenge_flip_fill(&TradeBuilderRevengeFlipFillInput {
            user_id: order.user_id,
            flow_definition_id,
            flow_run_id: order.origin_flow_run_id,
            root_flow_node_key: root_node_key.clone(),
            market_slug: order.market_slug.clone(),
            token_id: order.token_id.clone(),
            outcome_label: order.outcome_label.clone(),
            revenge_side,
            intent: intent.clone(),
            order_side: order.side.clone(),
            builder_order_id: order.id,
            parent_builder_order_id: order.parent_order_id.or_else(|| parent_order.map(|parent| parent.id)),
            source_trade_id: Some(order.trade_id).filter(|value| *value > 0),
            quantity: fill_qty.max(0.0),
            execution_price: clamp_probability(execution_price.max(0.0)),
            notional_usdc: (fill_qty.max(0.0) * execution_price.max(0.0)).max(0.0),
            stop_loss_enabled: revenge_flip_marker_bool(
                marker_config,
                REVENGE_FLIP_STOP_LOSS_ENABLED_KEY,
            ),
            stop_loss_pct: revenge_flip_marker_f64(marker_config, REVENGE_FLIP_STOP_LOSS_PCT_KEY)
                .filter(|value| value.is_finite() && *value > 0.0 && *value < 1.0),
            payload_json: json!({
                "builder_order_id": order.id,
                "parent_builder_order_id": order.parent_order_id.or_else(|| parent_order.map(|parent| parent.id)),
                "flow_created": flow_created_payload,
            }),
        })
        .await?;
    if applied {
        repo.append_trade_builder_order_event(
            order.id,
            "revenge_flip_fill_recorded",
            &json!({
                "order_side": order.side,
                "quantity": fill_qty,
                "execution_price": execution_price,
            }),
        )
        .await?;
    }
    if revenge_flip_stop_loss_wake_marker_matches(order, &intent, applied) {
        maybe_schedule_revenge_flip_stop_loss_wake(
            repo,
            order,
            root_action_node_key.as_deref(),
        )
        .await?;
    }
    Ok(())
}
