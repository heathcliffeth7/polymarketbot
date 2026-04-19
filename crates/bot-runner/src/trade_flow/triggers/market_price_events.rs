static TRADE_FLOW_REALTIME_PRICE_TICK_THROTTLE: LazyLock<StdMutex<HashMap<String, i64>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

#[derive(Debug, Clone, PartialEq)]
struct TriggerWsConditionNotMetLogFields {
    node_key: String,
    node_type: String,
    outcome_label: String,
    token_id: String,
    current_price: f64,
    previous_price: Option<f64>,
    trigger_price: f64,
    max_price: Option<f64>,
    trigger_condition: String,
    evaluation_mode: String,
    gate_mode: String,
    price_mode: String,
    price_source: String,
    price_source_detail: String,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
    snapshot_age_ms: Option<i64>,
    dirty_token_id: Option<String>,
    ws_reevaluation_reason: String,
    resolved_market_slug: Option<String>,
    once_mode: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct TriggerWsTargetLogFields {
    node_key: String,
    node_type: String,
    outcome_label: String,
    token_id: String,
    dirty_token_id: Option<String>,
    ws_reevaluation_reason: String,
    resolved_market_slug: Option<String>,
    price_mode: String,
    trigger_condition: String,
    trigger_price: f64,
    max_price: Option<f64>,
    once_mode: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct TriggerWsCacheNodeLogFields {
    node_key: String,
    node_type: String,
    outcome_label: Option<String>,
    token_id: Option<String>,
    resolved_market_slug: Option<String>,
    auto_scope: bool,
    version_id: Option<i64>,
    version_no: Option<i32>,
    price_mode: Option<String>,
    trigger_condition: Option<String>,
    trigger_price: Option<f64>,
    max_price: Option<f64>,
    cycle_window_mode: Option<String>,
    cycle_window_secs: Option<i64>,
    cycle_window_start_sec: Option<i64>,
    cycle_window_end_sec: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
struct TriggerWsExecuteBeginLogFields {
    node_key: String,
    node_type: String,
    market_slug: String,
    ws_market_slug: Option<String>,
    ws_token_id: Option<String>,
    ws_sourced: bool,
    path: &'static str,
    once_mode: bool,
    once_already_fired: bool,
    ws_hard_ignore_reason: Option<String>,
    ws_first_tick_threshold_override: bool,
}

fn log_trigger_ws_cache_node_skipped(
    run_id: i64,
    flow_run_id: i64,
    reason: &str,
    fields: &TriggerWsCacheNodeLogFields,
) {
    debug!(
        run_id,
        flow_run_id,
        reason,
        node_key = %fields.node_key,
        node_type = %fields.node_type,
        outcome_label = ?fields.outcome_label,
        token_id = ?fields.token_id,
        resolved_market_slug = ?fields.resolved_market_slug,
        auto_scope = fields.auto_scope,
        version_id = ?fields.version_id,
        version_no = ?fields.version_no,
        price_mode = ?fields.price_mode,
        trigger_condition = ?fields.trigger_condition,
        trigger_price = ?fields.trigger_price,
        max_price = ?fields.max_price,
        cycle_window_mode = ?fields.cycle_window_mode,
        cycle_window_secs = ?fields.cycle_window_secs,
        cycle_window_start_sec = ?fields.cycle_window_start_sec,
        cycle_window_end_sec = ?fields.cycle_window_end_sec,
        "TRIGGER_WS_CACHE_NODE_SKIPPED"
    );
}

fn log_trigger_ws_cache_node_indexed(
    run_id: i64,
    flow_run_id: i64,
    fields: &TriggerWsCacheNodeLogFields,
) {
    debug!(
        run_id,
        flow_run_id,
        node_key = %fields.node_key,
        node_type = %fields.node_type,
        outcome_label = ?fields.outcome_label,
        token_id = ?fields.token_id,
        resolved_market_slug = ?fields.resolved_market_slug,
        auto_scope = fields.auto_scope,
        version_id = ?fields.version_id,
        version_no = ?fields.version_no,
        price_mode = ?fields.price_mode,
        trigger_condition = ?fields.trigger_condition,
        trigger_price = ?fields.trigger_price,
        max_price = ?fields.max_price,
        cycle_window_mode = ?fields.cycle_window_mode,
        cycle_window_secs = ?fields.cycle_window_secs,
        cycle_window_start_sec = ?fields.cycle_window_start_sec,
        cycle_window_end_sec = ?fields.cycle_window_end_sec,
        "TRIGGER_WS_CACHE_NODE_INDEXED"
    );
}

fn build_trigger_ws_cache_node_log_fields_from_node(
    node: &TradeFlowNode,
    context: &Value,
    outcome_label: Option<&str>,
) -> TriggerWsCacheNodeLogFields {
    TriggerWsCacheNodeLogFields {
        node_key: node.key.clone(),
        node_type: node.node_type.clone(),
        outcome_label: outcome_label
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        token_id: None,
        resolved_market_slug: node_auto_scope_market_slug(context, &node.key)
            .or_else(|| flow_context_string(context, "marketSlug"))
            .or_else(|| node_config_string(node, "marketSlug")),
        auto_scope: node_market_mode(node) == "auto_scope",
        version_id: None,
        version_no: None,
        price_mode: node
            .config
            .get("priceMode")
            .and_then(Value::as_str)
            .map(str::to_string),
        trigger_condition: node_config_string(node, "triggerCondition"),
        trigger_price: node_config_f64(node, "triggerPrice")
            .or_else(|| node_config_f64(node, "triggerPriceCent").map(|v| v / 100.0)),
        max_price: node_config_f64(node, "maxPrice")
            .or_else(|| node_config_f64(node, "maxPriceCent").map(|v| v / 100.0)),
        cycle_window_mode: node_config_string(node, "cycleWindowMode"),
        cycle_window_secs: node_config_i64(node, "cycleWindowSecs"),
        cycle_window_start_sec: node_config_i64(node, "cycleWindowStartSec"),
        cycle_window_end_sec: node_config_i64(node, "cycleWindowEndSec"),
    }
}

fn build_trigger_ws_cache_node_log_fields_from_spec(
    spec: &WsOpenPositionPriceNodeSpec,
    version_id: i64,
    version_no: Option<i32>,
) -> TriggerWsCacheNodeLogFields {
    TriggerWsCacheNodeLogFields {
        node_key: spec.node_key.clone(),
        node_type: spec.node_type.clone(),
        outcome_label: (!spec.outcome_label.is_empty()).then_some(spec.outcome_label.clone()),
        token_id: Some(spec.token_id.clone()),
        resolved_market_slug: spec.market_slug.clone(),
        auto_scope: spec.auto_scope,
        version_id: Some(version_id),
        version_no,
        price_mode: Some(spec.price_mode.as_str().to_string()),
        trigger_condition: Some(spec.trigger_condition.clone()),
        trigger_price: Some(spec.trigger_price),
        max_price: spec.max_price,
        cycle_window_mode: spec.cycle_window_mode.clone(),
        cycle_window_secs: spec.cycle_window_secs,
        cycle_window_start_sec: spec.cycle_window_start_sec,
        cycle_window_end_sec: spec.cycle_window_end_sec,
    }
}

fn log_trigger_ws_dirty_token_unmapped(run_id: i64, dirty_token_id: &str) {
    debug!(run_id, dirty_token_id, "TRIGGER_WS_DIRTY_TOKEN_UNMAPPED");
}

fn log_trigger_ws_execute_begin(
    run_id: i64,
    flow_run_id: i64,
    fields: &TriggerWsExecuteBeginLogFields,
) {
    debug!(
        run_id,
        flow_run_id,
        node_key = %fields.node_key,
        node_type = %fields.node_type,
        market_slug = %fields.market_slug,
        ws_market_slug = ?fields.ws_market_slug,
        ws_token_id = ?fields.ws_token_id,
        ws_sourced = fields.ws_sourced,
        path = fields.path,
        once_mode = fields.once_mode,
        once_already_fired = fields.once_already_fired,
        ws_hard_ignore_reason = ?fields.ws_hard_ignore_reason,
        ws_first_tick_threshold_override = fields.ws_first_tick_threshold_override,
        "TRIGGER_WS_EXEC_BEGIN"
    );
}

fn build_trigger_ws_target_log_fields(
    node_spec: &WsOpenPositionPriceNodeSpec,
    dirty_token_id: Option<&str>,
    ws_reevaluation_reason: &str,
    resolved_market_slug: Option<&str>,
) -> TriggerWsTargetLogFields {
    TriggerWsTargetLogFields {
        node_key: node_spec.node_key.clone(),
        node_type: node_spec.node_type.clone(),
        outcome_label: node_spec.outcome_label.clone(),
        token_id: node_spec.token_id.clone(),
        dirty_token_id: dirty_token_id.map(str::to_string),
        ws_reevaluation_reason: ws_reevaluation_reason.to_string(),
        resolved_market_slug: resolved_market_slug.map(str::to_string),
        price_mode: node_spec.price_mode.as_str().to_string(),
        trigger_condition: node_spec.trigger_condition.clone(),
        trigger_price: node_spec.trigger_price,
        max_price: node_spec.max_price,
        once_mode: node_spec.once_mode,
    }
}

fn log_trigger_ws_target_selected(
    run_id: i64,
    flow_run_id: i64,
    fields: &TriggerWsTargetLogFields,
) {
    debug!(
        run_id,
        flow_run_id,
        node_key = %fields.node_key,
        node_type = %fields.node_type,
        outcome_label = %fields.outcome_label,
        token_id = %fields.token_id,
        dirty_token_id = ?fields.dirty_token_id,
        ws_reevaluation_reason = %fields.ws_reevaluation_reason,
        resolved_market_slug = ?fields.resolved_market_slug,
        price_mode = %fields.price_mode,
        trigger_condition = %fields.trigger_condition,
        trigger_price = fields.trigger_price,
        max_price = ?fields.max_price,
        once_mode = fields.once_mode,
        "TRIGGER_WS_TARGET_SELECTED"
    );
}

fn log_trigger_ws_target_started(run_id: i64, flow_run_id: i64, fields: &TriggerWsTargetLogFields) {
    debug!(
        run_id,
        flow_run_id,
        node_key = %fields.node_key,
        node_type = %fields.node_type,
        outcome_label = %fields.outcome_label,
        token_id = %fields.token_id,
        dirty_token_id = ?fields.dirty_token_id,
        ws_reevaluation_reason = %fields.ws_reevaluation_reason,
        resolved_market_slug = ?fields.resolved_market_slug,
        price_mode = %fields.price_mode,
        trigger_condition = %fields.trigger_condition,
        trigger_price = fields.trigger_price,
        max_price = ?fields.max_price,
        once_mode = fields.once_mode,
        "TRIGGER_WS_TARGET_EVALUATION_STARTED"
    );
}

fn log_trigger_ws_target_skipped(
    run_id: i64,
    flow_run_id: i64,
    reason: &str,
    fields: &TriggerWsTargetLogFields,
) {
    debug!(
        run_id,
        flow_run_id,
        reason,
        node_key = %fields.node_key,
        node_type = %fields.node_type,
        outcome_label = %fields.outcome_label,
        token_id = %fields.token_id,
        dirty_token_id = ?fields.dirty_token_id,
        ws_reevaluation_reason = %fields.ws_reevaluation_reason,
        resolved_market_slug = ?fields.resolved_market_slug,
        price_mode = %fields.price_mode,
        trigger_condition = %fields.trigger_condition,
        trigger_price = fields.trigger_price,
        max_price = ?fields.max_price,
        once_mode = fields.once_mode,
        "TRIGGER_WS_TARGET_SKIPPED"
    );
}

fn log_trigger_ws_target_dropped(
    run_id: i64,
    flow_run_id: Option<i64>,
    reason: &str,
    fields: &TriggerWsTargetLogFields,
) {
    debug!(
        run_id,
        flow_run_id = ?flow_run_id,
        reason,
        node_key = %fields.node_key,
        node_type = %fields.node_type,
        outcome_label = %fields.outcome_label,
        token_id = %fields.token_id,
        dirty_token_id = ?fields.dirty_token_id,
        ws_reevaluation_reason = %fields.ws_reevaluation_reason,
        resolved_market_slug = ?fields.resolved_market_slug,
        price_mode = %fields.price_mode,
        trigger_condition = %fields.trigger_condition,
        trigger_price = fields.trigger_price,
        max_price = ?fields.max_price,
        once_mode = fields.once_mode,
        "TRIGGER_WS_TARGET_DROPPED"
    );
}

fn trigger_market_price_gate_mode_label(gate_mode: TriggerMarketPriceGateMode) -> &'static str {
    match gate_mode {
        TriggerMarketPriceGateMode::StandardOnly => "standard_only",
        TriggerMarketPriceGateMode::StandardAndPtb => "standard_and_ptb",
        TriggerMarketPriceGateMode::PtbOnly => "ptb_only",
    }
}

fn build_trigger_ws_condition_not_met_log_fields(
    node_spec: &WsOpenPositionPriceNodeSpec,
    current_price: f64,
    previous_price: Option<f64>,
    evaluation_mode: &str,
    gate_mode: TriggerMarketPriceGateMode,
    price_source: &str,
    price_source_detail: &str,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
    snapshot_age_ms: Option<i64>,
    dirty_token_id: Option<&str>,
    ws_reevaluation_reason: &str,
    resolved_market_slug: Option<&str>,
) -> TriggerWsConditionNotMetLogFields {
    TriggerWsConditionNotMetLogFields {
        node_key: node_spec.node_key.clone(),
        node_type: node_spec.node_type.clone(),
        outcome_label: node_spec.outcome_label.clone(),
        token_id: node_spec.token_id.clone(),
        current_price,
        previous_price,
        trigger_price: node_spec.trigger_price,
        max_price: node_spec.max_price,
        trigger_condition: node_spec.trigger_condition.clone(),
        evaluation_mode: evaluation_mode.to_string(),
        gate_mode: trigger_market_price_gate_mode_label(gate_mode).to_string(),
        price_mode: node_spec.price_mode.as_str().to_string(),
        price_source: price_source.to_string(),
        price_source_detail: price_source_detail.to_string(),
        best_bid,
        best_ask,
        last_trade_price,
        snapshot_age_ms,
        dirty_token_id: dirty_token_id.map(str::to_string),
        ws_reevaluation_reason: ws_reevaluation_reason.to_string(),
        resolved_market_slug: resolved_market_slug.map(str::to_string),
        once_mode: node_spec.once_mode,
    }
}

fn log_trigger_ws_condition_not_met(
    run_id: i64,
    flow_run_id: i64,
    fields: TriggerWsConditionNotMetLogFields,
) {
    debug!(
        run_id,
        flow_run_id,
        node_key = %fields.node_key,
        node_type = %fields.node_type,
        outcome_label = %fields.outcome_label,
        token_id = %fields.token_id,
        current_price = fields.current_price,
        previous_price = ?fields.previous_price,
        trigger_price = fields.trigger_price,
        max_price = ?fields.max_price,
        trigger_condition = %fields.trigger_condition,
        evaluation_mode = %fields.evaluation_mode,
        gate_mode = %fields.gate_mode,
        price_mode = %fields.price_mode,
        price_source = %fields.price_source,
        price_source_detail = %fields.price_source_detail,
        best_bid = ?fields.best_bid,
        best_ask = ?fields.best_ask,
        last_trade_price = ?fields.last_trade_price,
        snapshot_age_ms = ?fields.snapshot_age_ms,
        dirty_token_id = ?fields.dirty_token_id,
        ws_reevaluation_reason = %fields.ws_reevaluation_reason,
        resolved_market_slug = ?fields.resolved_market_slug,
        once_mode = fields.once_mode,
        "TRIGGER_WS_CONDITION_NOT_MET"
    );
}

fn should_emit_trade_flow_realtime_price_tick(throttle_key: &str, now_ms: i64) -> bool {
    let mut guard = TRADE_FLOW_REALTIME_PRICE_TICK_THROTTLE
        .lock()
        .expect("price tick throttle mutex poisoned");
    match guard.get(throttle_key).copied() {
        Some(last_ms) if now_ms.saturating_sub(last_ms) < 100 => false,
        _ => {
            guard.insert(throttle_key.to_string(), now_ms);
            true
        }
    }
}

async fn notify_trade_flow_realtime_price_tick(
    repo: &PostgresRepository,
    run_id: i64,
    definition_id: i64,
    version_id: i64,
    node_key: &str,
    node_type: &str,
    market_slug: Option<&str>,
    token_id: &str,
    outcome_label: &str,
    current_price: f64,
    price_mode: WsPriceMode,
    price_source: &str,
    price_source_detail: &str,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
    snapshot_age_ms: Option<i64>,
    event_ts: Option<i64>,
    evaluation_mode: Option<&str>,
) {
    let market_slug = market_slug.unwrap_or_default();
    let throttle_key = format!("{market_slug}:{token_id}");
    let created_at = Utc::now();
    let created_at_ms = created_at.timestamp_millis();
    if !should_emit_trade_flow_realtime_price_tick(&throttle_key, created_at_ms) {
        return;
    }

    let payload = json!({
        "kind": "price_tick",
        "run_id": run_id,
        "definition_id": definition_id,
        "version_id": version_id,
        "node_key": node_key,
        "node_type": node_type,
        "market_slug": if market_slug.is_empty() { Value::Null } else { json!(market_slug) },
        "token_id": token_id,
        "outcome_label": if outcome_label.trim().is_empty() { Value::Null } else { json!(outcome_label.trim()) },
        "price": current_price,
        "best_bid": best_bid,
        "best_ask": best_ask,
        "last_trade_price": last_trade_price,
        "price_mode": price_mode.as_str(),
        "price_source": price_source,
        "price_source_detail": price_source_detail,
        "evaluation_mode": evaluation_mode,
        "snapshot_age_ms": snapshot_age_ms,
        "event_ts": event_ts,
        "created_at": created_at.to_rfc3339(),
    });
    let _ = repo.notify_trade_flow_realtime(&payload).await;
}

async fn record_trigger_ws_price_ignored_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    reason: &str,
    price_mode: WsPriceMode,
    trigger_source: Option<&str>,
    market_slug: &str,
    ws_market_slug_from_step: Option<String>,
    ws_token_id_from_step: Option<String>,
    triggered_token_id: &str,
    ws_price_from_step: Option<f64>,
    ws_previous_price_from_step: Option<f64>,
    ws_price_mode_from_step: &str,
    ws_price_source_from_step: &str,
    effective_previous_price: Option<f64>,
) {
    if let Err(err) = repo
        .append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "trigger_ws_price_ignored",
            &json!({
                "node_key": node.key,
                "node_type": node.node_type,
                "reason": reason,
                "price_mode": price_mode.as_str(),
                "trigger_source": trigger_source,
                "market_slug": market_slug,
                "ws_market_slug": ws_market_slug_from_step,
                "ws_token_id": ws_token_id_from_step,
                "expected_token_id": if triggered_token_id.is_empty() { Value::Null } else { json!(triggered_token_id) },
                "ws_price": ws_price_from_step,
                "ws_previous_price": ws_previous_price_from_step,
                "ws_price_mode": ws_price_mode_from_step,
                "ws_price_source": ws_price_source_from_step,
                "effective_previous_price": effective_previous_price
            }),
        )
        .await
    {
        warn!(
            flow_run_id = run.id,
            node_key = %node.key,
            error = %err,
            "TRADE_FLOW_WS_IGNORE_EVENT_FAILED"
        );
    }
}

async fn record_trigger_once_blocked_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    reason: &str,
    once_scope_market: bool,
    market_slug: &str,
    trigger_source: Option<&str>,
    ws_sourced: Option<bool>,
    idempotency_key: Option<&str>,
) {
    if let Err(err) = repo
        .append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "trigger_once_blocked",
            &json!({
                "node_key": node.key,
                "node_type": node.node_type,
                "reason": reason,
                "idempotency_key": idempotency_key,
                "trigger_source": trigger_source,
                "market_slug": market_slug,
                "once_scope": if once_scope_market { "market" } else { "run" },
                "ws_sourced": ws_sourced,
            }),
        )
        .await
    {
        warn!(
            flow_run_id = run.id,
            node_key = %node.key,
            error = %err,
            "TRADE_FLOW_ONCE_BLOCK_EVENT_FAILED"
        );
    }
}

async fn record_trigger_ws_cross_confirmed_applied_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    market_slug: &str,
    price_mode: WsPriceMode,
    ws_market_slug_from_step: Option<String>,
    ws_token_id_from_step: Option<String>,
    ws_price_from_step: Option<f64>,
    ws_previous_price_from_step: Option<f64>,
    ws_evaluation_mode_from_step: &str,
    ws_price_mode_from_step: &str,
    ws_price_source_from_step: &str,
    once_mode: bool,
    once_scope_market: bool,
) {
    if let Err(err) = repo
        .append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "trigger_ws_cross_confirmed_applied",
            &json!({
                "node_key": node.key,
                "node_type": node.node_type,
                "market_slug": market_slug,
                "price_mode": price_mode.as_str(),
                "ws_market_slug": ws_market_slug_from_step,
                "ws_token_id": ws_token_id_from_step,
                "ws_price": ws_price_from_step,
                "ws_previous_price": ws_previous_price_from_step,
                "ws_evaluation_mode": ws_evaluation_mode_from_step,
                "ws_price_mode": ws_price_mode_from_step,
                "ws_price_source": ws_price_source_from_step,
                "once_mode": once_mode,
                "once_scope": if once_scope_market { "market" } else { "run" }
            }),
        )
        .await
    {
        warn!(
            flow_run_id = run.id,
            node_key = %node.key,
            error = %err,
            "TRADE_FLOW_WS_CROSS_CONFIRMED_APPLIED_EVENT_FAILED"
        );
    }
}

async fn record_trigger_ws_cross_confirmed_unexpected_fail_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    market_slug: &str,
    price_mode: WsPriceMode,
    ws_market_slug_from_step: Option<String>,
    ws_token_id_from_step: Option<String>,
    ws_price_from_step: Option<f64>,
    ws_previous_price_from_step: Option<f64>,
    ws_evaluation_mode_from_step: &str,
    ws_price_mode_from_step: &str,
    ws_price_source_from_step: &str,
    trigger_evaluation_mode: &str,
    ws_ignore_reason: Option<String>,
    effective_previous_price: Option<f64>,
    ws_cross_confirmed_short_circuit_applied: bool,
) {
    if let Err(err) = repo
        .append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "trigger_ws_cross_confirmed_unexpected_fail",
            &json!({
                "node_key": node.key,
                "node_type": node.node_type,
                "market_slug": market_slug,
                "price_mode": price_mode.as_str(),
                "ws_market_slug": ws_market_slug_from_step,
                "ws_token_id": ws_token_id_from_step,
                "ws_price": ws_price_from_step,
                "ws_previous_price": ws_previous_price_from_step,
                "ws_evaluation_mode": ws_evaluation_mode_from_step,
                "ws_price_mode": ws_price_mode_from_step,
                "ws_price_source": ws_price_source_from_step,
                "evaluation_mode": trigger_evaluation_mode,
                "ws_ignored_reason": ws_ignore_reason,
                "effective_previous_price": effective_previous_price,
                "short_circuit_applied": ws_cross_confirmed_short_circuit_applied
            }),
        )
        .await
    {
        warn!(
            flow_run_id = run.id,
            node_key = %node.key,
            error = %err,
            "TRADE_FLOW_WS_CROSS_CONFIRMED_UNEXPECTED_FAIL_EVENT_FAILED"
        );
    }
}

async fn record_trigger_protection_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    protection_passed: bool,
    market_slug: &str,
    triggered_token_id: &str,
    triggered_outcome_label: &str,
    triggered_condition: &str,
    triggered_price: Option<f64>,
    triggered_max_price: Option<f64>,
    triggered_poly_delta_10s_cent: Option<f64>,
    protection_output: &Value,
) {
    let event_type = if protection_passed {
        "trigger_protection_passed"
    } else {
        "trigger_protection_blocked"
    };
    if let Err(err) = repo
        .append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            event_type,
            &json!({
                "node_key": node.key,
                "node_type": node.node_type,
                "market_slug": market_slug,
                "triggered_token_id": if triggered_token_id.is_empty() { Value::Null } else { json!(triggered_token_id) },
                "triggered_outcome_label": if triggered_outcome_label.is_empty() { Value::Null } else { json!(triggered_outcome_label) },
                "triggered_condition": if triggered_condition.is_empty() { Value::Null } else { json!(triggered_condition) },
                "triggered_price": triggered_price,
                "max_price": triggered_max_price,
                "poly_delta_10s_cent": triggered_poly_delta_10s_cent,
                "protection": protection_output
            }),
        )
        .await
    {
        warn!(
            flow_run_id = run.id,
            node_key = %node.key,
            error = %err,
            "TRADE_FLOW_TRIGGER_PROTECTION_EVENT_FAILED"
        );
    }
}

async fn record_trigger_once_fired_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    market_slug: &str,
    price_mode: WsPriceMode,
    triggered_token_id: &str,
    triggered_outcome_label: &str,
    triggered_condition: &str,
    triggered_price: Option<f64>,
    triggered_max_price: Option<f64>,
    triggered_previous_price: Option<f64>,
    triggered_poly_delta_10s_cent: Option<f64>,
    protection_output: &Value,
    trigger_evaluation_mode: &str,
    current_price: Option<f64>,
    ws_sourced: bool,
    ws_price_mode_from_step: &str,
    ws_price_source_from_step: &str,
    once_scope_market: bool,
    fired_at: DateTime<Utc>,
    once_fire_key: &str,
    cycle_window_diagnostics: Option<&Value>,
) {
    let mut payload = json!({
        "node_key": node.key,
        "node_type": node.node_type,
        "market_slug": market_slug,
        "price_mode": price_mode.as_str(),
        "triggered_token_id": triggered_token_id,
        "triggered_outcome_label": triggered_outcome_label,
        "triggered_condition": triggered_condition,
        "triggered_price": triggered_price,
        "max_price": triggered_max_price,
        "previous_price": triggered_previous_price,
        "poly_delta_10s_cent": triggered_poly_delta_10s_cent,
        "protection": protection_output,
        "evaluation_mode": trigger_evaluation_mode,
        "price": current_price,
        "ws_sourced": ws_sourced,
        "ws_price_mode": ws_price_mode_from_step,
        "ws_price_source": ws_price_source_from_step,
        "once_scope": if once_scope_market { "market" } else { "run" },
        "fired_at": fired_at,
        "idempotency_key": once_fire_key
    });
    if let Some(diagnostics) = cycle_window_diagnostics {
        append_json_object_fields(&mut payload, diagnostics);
    }
    if let Err(err) = repo
        .append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "trigger_once_fired",
            &payload,
        )
        .await
    {
        warn!(
            flow_run_id = run.id,
            node_key = %node.key,
            error = %err,
            "TRADE_FLOW_ONCE_FIRED_EVENT_FAILED"
        );
    }
}

fn apply_trigger_market_price_context_updates(
    context: &mut Value,
    node: &TradeFlowNode,
    market_slug: &str,
    var_key: &str,
    current_price: Option<f64>,
    triggered_token_id: &str,
    triggered_outcome_label: &str,
    triggered_condition: &str,
    triggered_trigger_price: Option<f64>,
    triggered_price: Option<f64>,
    triggered_max_price: Option<f64>,
    cycle_window_mode: Option<&str>,
    cycle_window_secs: Option<i64>,
    cycle_window_open_at: Option<DateTime<Utc>>,
    cycle_window_end_at: Option<DateTime<Utc>>,
    pass: bool,
) {
    if let Some(price) = current_price {
        set_flow_node_state(context, &node.key, "last_price", json!(price));
        set_flow_var(context, &format!("{var_key}_price"), json!(price));
    }
    if !triggered_token_id.is_empty() {
        set_flow_var(
            context,
            &format!("{var_key}_token_id"),
            json!(triggered_token_id),
        );
        if pass {
            set_flow_context(context, "tokenId", json!(triggered_token_id));
        }
    }
    if !triggered_outcome_label.is_empty() {
        set_flow_var(
            context,
            &format!("{var_key}_outcome_label"),
            json!(triggered_outcome_label),
        );
        if pass {
            set_flow_context(context, "outcomeLabel", json!(triggered_outcome_label));
        }
    }
    if !triggered_condition.is_empty() {
        set_flow_var(
            context,
            &format!("{var_key}_triggered_condition"),
            json!(triggered_condition),
        );
    }
    if let Some(trigger_price) = triggered_trigger_price {
        set_flow_var(
            context,
            &format!("{var_key}_trigger_price"),
            json!(trigger_price),
        );
    }
    if let Some(tp) = triggered_price {
        set_flow_var(context, &format!("{var_key}_triggered_price"), json!(tp));
    }
    if let Some(max_price) = triggered_max_price {
        set_flow_var(context, &format!("{var_key}_max_price"), json!(max_price));
    }
    if pass {
        if node_market_mode(node) == "auto_scope" {
            promote_trigger_node_auto_scope_context_to_flow_context(
                context,
                &node.key,
                market_slug,
            );
        } else {
            set_flow_context(context, "marketSlug", json!(market_slug));
        }
        if let Some(max_price) = triggered_max_price {
            set_flow_context(context, "maxPrice", json!(max_price));
        } else {
            set_flow_context(context, "maxPrice", Value::Null);
        }
        set_flow_context(
            context,
            "cycleWindowMode",
            cycle_window_mode.map_or(Value::Null, |value| json!(value)),
        );
        set_flow_context(
            context,
            "cycleWindowSecs",
            cycle_window_secs.map_or(Value::Null, |value| json!(value)),
        );
        set_flow_context(
            context,
            "cycleWindowOpenAt",
            cycle_window_open_at
                .map(|value| json!(value.to_rfc3339()))
                .unwrap_or(Value::Null),
        );
        set_flow_context(
            context,
            "cycleWindowEndAt",
            cycle_window_end_at
                .map(|value| json!(value.to_rfc3339()))
                .unwrap_or(Value::Null),
        );
    }
}

fn build_trigger_market_price_output(
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &Value,
    market_slug: &str,
    price_mode: WsPriceMode,
    triggered_token_id: &str,
    triggered_outcome_label: &str,
    triggered_condition: &str,
    triggered_trigger_price: Option<f64>,
    triggered_price: Option<f64>,
    triggered_max_price: Option<f64>,
    protection_output: &Value,
    triggered_previous_price: Option<f64>,
    ws_previous_price_from_step: Option<f64>,
    effective_previous_price: Option<f64>,
    trigger_evaluation_mode: &str,
    ws_evaluation_mode_from_step: &str,
    ws_price_mode_from_step: &str,
    ws_price_source_from_step: &str,
    ws_price_source_detail_from_step: &str,
    ws_best_bid_from_step: Option<f64>,
    ws_best_ask_from_step: Option<f64>,
    ws_last_trade_price_from_step: Option<f64>,
    ws_snapshot_age_ms_from_step: Option<i64>,
    ws_site_display_mode_decision_from_step: &str,
    ws_cross_confirmed_short_circuit_applied: bool,
    current_price: Option<f64>,
    pass: bool,
    price_to_beat_trigger_gate: &Value,
    var_key: &str,
    outcome_conditions: &Option<Vec<Value>>,
    ws_sourced: bool,
    ws_ignore_reason: Option<String>,
    once_mode: bool,
    once_scope_market: bool,
    queued_at_from_step: Option<&str>,
    version_id: i64,
    version_no: Option<i32>,
    cycle_window_mode: Option<&str>,
    cycle_window_secs: Option<i64>,
    cycle_window_start_sec: Option<i64>,
    cycle_window_end_sec: Option<i64>,
    cycle_window_open_at: Option<DateTime<Utc>>,
    cycle_window_end_at: Option<DateTime<Utc>>,
) -> Value {
    let binding_mode = match node_config_string(node, "bindingMode")
        .unwrap_or_else(|| "standard".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "pair_lock_only" => "pair_lock_only",
        _ => "standard",
    };
    let mut output = json!({
        "run_id": run.id,
        "node_key": node.key,
        "binding_mode": binding_mode,
        "market_slug": market_slug,
        "price_mode": price_mode.as_str(),
        "triggered_token_id": triggered_token_id,
        "triggered_outcome_label": triggered_outcome_label,
        "triggered_condition": triggered_condition,
        "trigger_price": triggered_trigger_price,
        "triggered_price": triggered_price,
        "max_price": triggered_max_price,
        "maxPrice": triggered_max_price,
        "poly_delta_10s_cent": Value::Null,
        "protection": protection_output,
        "previous_price": triggered_previous_price,
        "ws_previous_price": ws_previous_price_from_step,
        "effective_previous_price": effective_previous_price,
        "evaluation_mode": trigger_evaluation_mode,
        "ws_evaluation_mode_from_step": ws_evaluation_mode_from_step,
        "ws_price_mode_from_step": ws_price_mode_from_step,
        "ws_price_source_from_step": ws_price_source_from_step,
        "ws_price_source_detail_from_step": ws_price_source_detail_from_step,
        "ws_best_bid_from_step": ws_best_bid_from_step,
        "ws_best_ask_from_step": ws_best_ask_from_step,
        "ws_last_trade_price_from_step": ws_last_trade_price_from_step,
        "ws_snapshot_age_ms_from_step": ws_snapshot_age_ms_from_step,
        "ws_site_display_mode_decision_from_step": ws_site_display_mode_decision_from_step,
        "cross_confirmed_short_circuit_applied": ws_cross_confirmed_short_circuit_applied,
        "price": current_price,
        "pass": pass,
        "var_key": var_key,
        "multi_outcome": outcome_conditions.is_some(),
        "ws_sourced": ws_sourced,
        "ws_ignored_reason": ws_ignore_reason,
        "once_mode": once_mode,
        "once_scope": if once_scope_market { "market" } else { "run" },
        "queued_at": queued_at_from_step,
    });
    append_json_object_fields(
        &mut output,
        &json!({
            "marketScope": flow_context_string(context, "marketScope"),
            "marketAsset": flow_context_string(context, "marketAsset"),
            "marketTimeframe": flow_context_string(context, "marketTimeframe"),
            "yesTokenId": flow_context_string(context, "yesTokenId"),
            "noTokenId": flow_context_string(context, "noTokenId"),
        }),
    );
    append_json_object_fields(
        &mut output,
        &json!({
            "versionId": version_id,
            "versionNo": version_no,
            "cycleWindowMode": cycle_window_mode,
            "cycleWindowSecs": cycle_window_secs,
            "cycleWindowStartSec": cycle_window_start_sec,
            "cycleWindowEndSec": cycle_window_end_sec,
            "cycleWindowOpenAt": cycle_window_open_at.map(|value| value.to_rfc3339()),
            "cycleWindowEndAt": cycle_window_end_at.map(|value| value.to_rfc3339()),
            "once_fired": trade_flow_market_price_once_fired_for_scope(
                context,
                &node.key,
                once_scope_market,
                Some(market_slug)
            ),
        }),
    );
    if !price_to_beat_trigger_gate.is_null() {
        append_trigger_market_price_ptb_gate(&mut output, price_to_beat_trigger_gate);
    }
    output
}

fn finish_trigger_market_price_execution(
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &Value,
    market_slug: &str,
    price_mode: WsPriceMode,
    triggered_token_id: &str,
    triggered_outcome_label: &str,
    triggered_condition: &str,
    triggered_trigger_price: Option<f64>,
    triggered_price: Option<f64>,
    triggered_max_price: Option<f64>,
    protection_output: &Value,
    triggered_previous_price: Option<f64>,
    ws_previous_price_from_step: Option<f64>,
    effective_previous_price: Option<f64>,
    trigger_evaluation_mode: &str,
    ws_evaluation_mode_from_step: &str,
    ws_price_mode_from_step: &str,
    ws_price_source_from_step: &str,
    ws_price_source_detail_from_step: &str,
    ws_best_bid_from_step: Option<f64>,
    ws_best_ask_from_step: Option<f64>,
    ws_last_trade_price_from_step: Option<f64>,
    ws_snapshot_age_ms_from_step: Option<i64>,
    ws_site_display_mode_decision_from_step: &str,
    ws_cross_confirmed_short_circuit_applied: bool,
    current_price: Option<f64>,
    pass: bool,
    price_to_beat_trigger_gate: &Value,
    var_key: &str,
    outcome_conditions: &Option<Vec<Value>>,
    ws_sourced: bool,
    ws_ignore_reason: Option<String>,
    once_mode: bool,
    once_scope_market: bool,
    queued_at_from_step: Option<&str>,
    version_id: i64,
    version_no: Option<i32>,
    cycle_window_mode: Option<&str>,
    cycle_window_secs: Option<i64>,
    cycle_window_start_sec: Option<i64>,
    cycle_window_end_sec: Option<i64>,
    cycle_window_open_at: Option<DateTime<Utc>>,
    cycle_window_end_at: Option<DateTime<Utc>>,
    interval_ms: i64,
) -> TradeFlowNodeExecution {
    let repeat_at = if ws_sourced {
        None
    } else if once_mode {
        None
    } else {
        Some(Utc::now() + ChronoDuration::milliseconds(interval_ms))
    };
    let routes = if pass {
        vec![TradeFlowRouteDecision {
            edge_type: "default".to_string(),
            available_at: Utc::now(),
        }]
    } else {
        Vec::new()
    };
    let output = build_trigger_market_price_output(
        run,
        node,
        context,
        market_slug,
        price_mode,
        triggered_token_id,
        triggered_outcome_label,
        triggered_condition,
        triggered_trigger_price,
        triggered_price,
        triggered_max_price,
        protection_output,
        triggered_previous_price,
        ws_previous_price_from_step,
        effective_previous_price,
        trigger_evaluation_mode,
        ws_evaluation_mode_from_step,
        ws_price_mode_from_step,
        ws_price_source_from_step,
        ws_price_source_detail_from_step,
        ws_best_bid_from_step,
        ws_best_ask_from_step,
        ws_last_trade_price_from_step,
        ws_snapshot_age_ms_from_step,
        ws_site_display_mode_decision_from_step,
        ws_cross_confirmed_short_circuit_applied,
        current_price,
        pass,
        price_to_beat_trigger_gate,
        var_key,
        outcome_conditions,
        ws_sourced,
        ws_ignore_reason,
        once_mode,
        once_scope_market,
        queued_at_from_step,
        version_id,
        version_no,
        cycle_window_mode,
        cycle_window_secs,
        cycle_window_start_sec,
        cycle_window_end_sec,
        cycle_window_open_at,
        cycle_window_end_at,
    );
    info!(
        flow_run_id = run.id,
        node_key = %node.key,
        pass,
        trigger_evaluation_mode,
        ?current_price,
        ?effective_previous_price,
        price_mode = price_mode.as_str(),
        once_mode,
        ws_sourced,
        ws_price_source = ws_price_source_from_step,
        ws_price_source_detail = ws_price_source_detail_from_step,
        ws_best_bid = ?ws_best_bid_from_step,
        ws_best_ask = ?ws_best_ask_from_step,
        ws_last_trade_price = ?ws_last_trade_price_from_step,
        ws_snapshot_age_ms = ?ws_snapshot_age_ms_from_step,
        ws_site_display_mode_decision = ws_site_display_mode_decision_from_step,
        routes_count = routes.len(),
        "TRIGGER_MARKET_PRICE_EVALUATED"
    );
    TradeFlowNodeExecution {
        output,
        routes,
        repeat_at,
        repeat_idempotency_key: None,
    }
}
