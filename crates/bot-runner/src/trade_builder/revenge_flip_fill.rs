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
    let root_node_key = revenge_flip_marker_string(marker_config, REVENGE_FLIP_ROOT_NODE_KEY)
        .or_else(|| order.origin_flow_node_key.clone())
        .unwrap_or_else(|| order.market_slug.clone());
    let Some(revenge_side) = revenge_flip_marker_side(marker_config, &order.outcome_label) else {
        return Ok(());
    };
    if revenge_side != "up" && revenge_side != "down" {
        return Ok(());
    }
    let applied = repo
        .record_trade_builder_revenge_flip_fill(&TradeBuilderRevengeFlipFillInput {
            user_id: order.user_id,
            flow_definition_id,
            flow_run_id: order.origin_flow_run_id,
            root_flow_node_key: root_node_key,
            market_slug: order.market_slug.clone(),
            token_id: order.token_id.clone(),
            outcome_label: order.outcome_label.clone(),
            revenge_side,
            intent: revenge_flip_marker_string(marker_config, REVENGE_FLIP_INTENT_KEY)
                .unwrap_or_else(|| "unknown".to_string()),
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
    Ok(())
}
