const FLOW_CONTEXT_BUY_FILL_LOCKS_KEY: &str = "__buyFillLocks";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActionPlaceOrderBuyFillLockConfig {
    group: String,
    release_on_stop_loss: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TradeFlowBuyFillLockRecord {
    group: String,
    market_slug: String,
    builder_order_id: i64,
    source_node_key: String,
    filled_at: String,
    release_on_stop_loss: bool,
}

impl ActionPlaceOrderBuyFillLockConfig {
    fn to_value(&self) -> Value {
        serde_json::json!({
            "enabled": true,
            "group": self.group,
            "releaseOnStopLoss": self.release_on_stop_loss,
        })
    }
}

impl TradeFlowBuyFillLockRecord {
    fn to_value(&self) -> Value {
        serde_json::json!({
            "group": self.group,
            "marketSlug": self.market_slug,
            "builderOrderId": self.builder_order_id,
            "sourceNodeKey": self.source_node_key,
            "filledAt": self.filled_at,
            "releaseOnStopLoss": self.release_on_stop_loss,
        })
    }
}

fn resolve_action_place_order_buy_fill_lock_config(
    node: &TradeFlowNode,
    side: &str,
) -> Result<Option<ActionPlaceOrderBuyFillLockConfig>> {
    if side != "buy" {
        return Ok(None);
    }
    if !node_config_bool(node, "buyFillLockEnabled").unwrap_or(false) {
        return Ok(None);
    }

    let group = node_config_string(node, "buyFillLockGroup")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("action.place_order buyFillLockGroup is required"))?;
    let release_on_stop_loss =
        node_config_bool(node, "releaseBuyFillLockOnStopLoss").unwrap_or(false);

    Ok(Some(ActionPlaceOrderBuyFillLockConfig {
        group,
        release_on_stop_loss,
    }))
}

fn action_place_order_buy_fill_lock_config_from_payload(
    payload: Option<&Value>,
) -> Option<ActionPlaceOrderBuyFillLockConfig> {
    let payload = payload?;
    let lock = payload.get("buy_fill_lock")?;
    let group = lock
        .get("group")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let release_on_stop_loss = lock
        .get("releaseOnStopLoss")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some(ActionPlaceOrderBuyFillLockConfig {
        group,
        release_on_stop_loss,
    })
}

fn resolve_action_place_order_selected_entry_timing_profile_value(
    step: &TradeFlowRunStep,
    context: &Value,
) -> Option<Value> {
    step_input_value(step, &["selectedEntryTimingProfile"])
        .cloned()
        .or_else(|| flow_context_value(context, "selectedEntryTimingProfile"))
}

fn resolve_action_place_order_selected_entry_size_usdc(
    step: &TradeFlowRunStep,
    context: &Value,
) -> Option<f64> {
    step_input_f64(step, &["selectedEntrySizeUsdc"])
        .or_else(|| flow_context_f64(context, "selectedEntrySizeUsdc"))
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn parse_trade_flow_buy_fill_lock_record(
    group: &str,
    value: &Value,
) -> Option<TradeFlowBuyFillLockRecord> {
    let market_slug = value
        .get("marketSlug")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|raw| !raw.is_empty())?
        .to_string();
    let builder_order_id = value
        .get("builderOrderId")
        .and_then(value_as_i64)
        .filter(|value| *value > 0)?;
    let source_node_key = value
        .get("sourceNodeKey")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|raw| !raw.is_empty())?
        .to_string();
    let filled_at = value
        .get("filledAt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|raw| !raw.is_empty())?
        .to_string();

    Some(TradeFlowBuyFillLockRecord {
        group: group.to_string(),
        market_slug,
        builder_order_id,
        source_node_key,
        filled_at,
        release_on_stop_loss: value
            .get("releaseOnStopLoss")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn find_trade_flow_buy_fill_lock(context: &Value, group: &str) -> Option<TradeFlowBuyFillLockRecord> {
    context
        .get("flowContext")
        .and_then(|flow_context| flow_context.get(FLOW_CONTEXT_BUY_FILL_LOCKS_KEY))
        .and_then(Value::as_object)
        .and_then(|locks| locks.get(group))
        .and_then(|value| parse_trade_flow_buy_fill_lock_record(group, value))
}

fn find_trade_flow_buy_fill_lock_for_market(
    context: &Value,
    group: &str,
    market_slug: &str,
) -> Option<TradeFlowBuyFillLockRecord> {
    let record = find_trade_flow_buy_fill_lock(context, group)?;
    (record.market_slug == market_slug).then_some(record)
}

fn set_trade_flow_buy_fill_lock(context: &mut Value, record: &TradeFlowBuyFillLockRecord) {
    let flow_context = ensure_nested_object(context, "flowContext");
    if !flow_context
        .get(FLOW_CONTEXT_BUY_FILL_LOCKS_KEY)
        .map(Value::is_object)
        .unwrap_or(false)
    {
        flow_context.insert(FLOW_CONTEXT_BUY_FILL_LOCKS_KEY.to_string(), serde_json::json!({}));
    }
    if let Some(locks) = flow_context
        .get_mut(FLOW_CONTEXT_BUY_FILL_LOCKS_KEY)
        .and_then(Value::as_object_mut)
    {
        locks.insert(record.group.clone(), record.to_value());
    }
}

fn clear_trade_flow_buy_fill_lock(context: &mut Value, group: &str) -> bool {
    let Some(flow_context) = context.get_mut("flowContext").and_then(Value::as_object_mut) else {
        return false;
    };
    let Some(locks) = flow_context
        .get_mut(FLOW_CONTEXT_BUY_FILL_LOCKS_KEY)
        .and_then(Value::as_object_mut)
    else {
        return false;
    };
    let removed = locks.remove(group).is_some();
    if locks.is_empty() {
        flow_context.remove(FLOW_CONTEXT_BUY_FILL_LOCKS_KEY);
    }
    removed
}

fn should_release_trade_flow_buy_fill_lock(
    record: &TradeFlowBuyFillLockRecord,
    parent_order: &TradeBuilderOrder,
    parent_position: &TradeBuilderParentPosition,
) -> bool {
    record.builder_order_id == parent_order.id
        && record.market_slug == parent_order.market_slug
        && parent_position.current_qty <= TRADE_BUILDER_EXIT_QTY_TOLERANCE
}

async fn maybe_block_action_place_order_buy_fill_lock(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &Value,
    side: &str,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    execution_mode: &str,
    source_trade_id: i64,
) -> Result<Option<TradeFlowNodeExecution>> {
    let Some(lock_config) = resolve_action_place_order_buy_fill_lock_config(node, side)? else {
        return Ok(None);
    };
    let Some(record) = find_trade_flow_buy_fill_lock_for_market(
        context,
        &lock_config.group,
        market_slug,
    ) else {
        return Ok(None);
    };

    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "place_order_buy_fill_lock_blocked",
        &serde_json::json!({
            "node_key": node.key,
            "node_type": node.node_type,
            "source_trade_id": source_trade_id,
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "side": side,
            "execution_mode": execution_mode,
            "buy_fill_lock_group": lock_config.group,
            "buy_fill_lock": record.to_value(),
        }),
    )
    .await?;

    Ok(Some(TradeFlowNodeExecution {
        output: serde_json::json!({
            "node_key": node.key,
            "skipped": true,
            "reason": "buy_fill_lock_blocked",
            "source_trade_id": source_trade_id,
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "side": side,
            "execution_mode": execution_mode,
            "buy_fill_lock_group": lock_config.group,
            "buy_fill_lock": record.to_value(),
        }),
        routes: Vec::new(),
        repeat_at: None,
        repeat_idempotency_key: None,
    }))
}

async fn maybe_record_action_place_order_buy_fill_lock_fill(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    flow_created_payload: Option<&Value>,
) -> Result<()> {
    if order.side != "buy" {
        return Ok(());
    }
    let Some(lock_config) = action_place_order_buy_fill_lock_config_from_payload(flow_created_payload) else {
        return Ok(());
    };
    let Some(run_id) = order.origin_flow_run_id else {
        return Ok(());
    };
    let Some(source_node_key) = order
        .origin_flow_node_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };
    let Some(flow_run) = repo.get_trade_flow_run(run_id).await? else {
        return Ok(());
    };
    if flow_run.status != "running" {
        return Ok(());
    }

    let filled_at = chrono::Utc::now().to_rfc3339();
    let mut context = flow_run.context_json.clone();
    let record = TradeFlowBuyFillLockRecord {
        group: lock_config.group.clone(),
        market_slug: order.market_slug.clone(),
        builder_order_id: order.id,
        source_node_key: source_node_key.to_string(),
        filled_at: filled_at.clone(),
        release_on_stop_loss: lock_config.release_on_stop_loss,
    };
    set_trade_flow_buy_fill_lock(&mut context, &record);

    repo.update_trade_flow_run_context(run_id, &context).await?;
    let cache_updated = replace_trade_flow_ws_fast_path_run_context(run_id, &context).await;
    repo.append_trade_builder_order_event(
        order.id,
        "buy_fill_lock_recorded",
        &serde_json::json!({
            "flow_run_id": run_id,
            "group": lock_config.group,
            "market_slug": &order.market_slug,
            "builder_order_id": order.id,
            "source_node_key": source_node_key,
            "filled_at": filled_at,
            "release_on_stop_loss": lock_config.release_on_stop_loss,
            "fast_path_cache_updated": cache_updated,
        }),
    )
    .await?;
    Ok(())
}

async fn maybe_release_action_place_order_buy_fill_lock_on_stop_loss(
    repo: &PostgresRepository,
    parent_order: &TradeBuilderOrder,
    stop_loss_order: &TradeBuilderOrder,
    parent_position: Option<&TradeBuilderParentPosition>,
) -> Result<()> {
    let Some(parent_position) = parent_position else {
        return Ok(());
    };
    let flow_created_payload = repo
        .load_trade_builder_order_flow_created_payload(parent_order.id)
        .await?;
    let Some(lock_config) =
        action_place_order_buy_fill_lock_config_from_payload(flow_created_payload.as_ref())
    else {
        return Ok(());
    };
    if !lock_config.release_on_stop_loss {
        return Ok(());
    }
    let Some(run_id) = parent_order.origin_flow_run_id else {
        return Ok(());
    };
    let Some(flow_run) = repo.get_trade_flow_run(run_id).await? else {
        return Ok(());
    };
    if flow_run.status != "running" {
        return Ok(());
    }

    let mut context = flow_run.context_json.clone();
    let Some(record) = find_trade_flow_buy_fill_lock(&context, &lock_config.group) else {
        return Ok(());
    };
    if !should_release_trade_flow_buy_fill_lock(&record, parent_order, parent_position) {
        return Ok(());
    }
    if !clear_trade_flow_buy_fill_lock(&mut context, &lock_config.group) {
        return Ok(());
    }

    repo.update_trade_flow_run_context(run_id, &context).await?;
    let cache_updated = replace_trade_flow_ws_fast_path_run_context(run_id, &context).await;
    repo.append_trade_builder_order_event(
        parent_order.id,
        "buy_fill_lock_released_on_stop_loss",
        &serde_json::json!({
            "flow_run_id": run_id,
            "group": lock_config.group,
            "market_slug": &parent_order.market_slug,
            "builder_order_id": parent_order.id,
            "stop_loss_order_id": stop_loss_order.id,
            "filled_at": record.filled_at,
            "remaining_qty_after_stop_loss": parent_position.current_qty,
            "fast_path_cache_updated": cache_updated,
        }),
    )
    .await?;
    Ok(())
}
