const DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_EVALUATED: bool = false;
const DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_RELAX: bool = true;
const DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_RELAX_SL: bool = true;
const DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_NO_RELAX_IMPORTANT: bool = true;
const DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_MISS_RESOLVED: bool = true;
const DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_COOLDOWN: bool = true;
const DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_SUMMARY: bool = true;
const DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_ALL_NO_RELAX: bool = false;
const DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_MIN_INTERVAL_SEC: i64 = 30;
const DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_INCLUDE_PAYLOAD: bool = false;
const DEFAULT_ADAPTIVE_MAX_PRICE_SUMMARY_EVERY_MARKETS: usize = 5;

const ADAPTIVE_MAX_PRICE_EVENT_RELAX_ALLOWED: &str = "adaptive_max_price_relax_allowed";
const ADAPTIVE_MAX_PRICE_EVENT_NO_RELAX_IMPORTANT: &str =
    "adaptive_max_price_no_relax_important";
const ADAPTIVE_MAX_PRICE_EVENT_MISS_RESOLVED: &str = "adaptive_max_price_miss_resolved";
const ADAPTIVE_MAX_PRICE_EVENT_COOLDOWN_STARTED: &str =
    "adaptive_max_price_cooldown_started";
const ADAPTIVE_MAX_PRICE_EVENT_COOLDOWN_CLEARED: &str =
    "adaptive_max_price_cooldown_cleared";
const ADAPTIVE_MAX_PRICE_EVENT_RELAX_SL: &str = "adaptive_max_price_relax_sl";
const ADAPTIVE_MAX_PRICE_EVENT_SUMMARY: &str = "adaptive_max_price_summary";

#[derive(Debug, Clone, Copy)]
struct PairLockAdaptiveMaxPriceNotifyConfig {
    notify_evaluated: bool,
    notify_relax: bool,
    notify_relax_sl: bool,
    notify_no_relax_important: bool,
    notify_miss_resolved: bool,
    notify_cooldown: bool,
    notify_summary: bool,
    notify_all_no_relax: bool,
    min_interval_sec: i64,
    include_payload: bool,
    summary_every_markets: usize,
}

fn resolve_pair_lock_adaptive_max_price_notify_config(
    node: &TradeFlowNode,
) -> Result<PairLockAdaptiveMaxPriceNotifyConfig> {
    let min_interval_sec = node_config_i64(node, "adaptiveMaxPriceNotifyMinIntervalSec")
        .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_MIN_INTERVAL_SEC);
    let summary_every_markets = node_config_i64(node, "adaptiveMaxPriceSummaryEveryMarkets")
        .filter(|value| *value > 0)
        .map(|value| value as usize)
        .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_SUMMARY_EVERY_MARKETS);
    anyhow::ensure!(
        min_interval_sec >= 0,
        "action.place_order adaptiveMaxPriceNotifyMinIntervalSec must be >= 0"
    );

    Ok(PairLockAdaptiveMaxPriceNotifyConfig {
        notify_evaluated: node_config_bool(node, "notifyOnAdaptiveMaxPriceEvaluated")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_EVALUATED),
        notify_relax: node_config_bool(node, "notifyOnAdaptiveMaxPriceRelax")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_RELAX),
        notify_relax_sl: node_config_bool(node, "notifyOnAdaptiveMaxPriceRelaxSl")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_RELAX_SL),
        notify_no_relax_important: node_config_bool(
            node,
            "notifyOnAdaptiveMaxPriceNoRelaxImportant",
        )
        .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_NO_RELAX_IMPORTANT),
        notify_miss_resolved: node_config_bool(node, "notifyOnAdaptiveMaxPriceMissResolved")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_MISS_RESOLVED),
        notify_cooldown: node_config_bool(node, "notifyOnAdaptiveMaxPriceCooldown")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_COOLDOWN),
        notify_summary: node_config_bool(node, "notifyOnAdaptiveMaxPriceSummary")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_SUMMARY),
        notify_all_no_relax: node_config_bool(node, "notifyOnAdaptiveMaxPriceAllNoRelax")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_ALL_NO_RELAX),
        min_interval_sec,
        include_payload: node_config_bool(node, "adaptiveMaxPriceNotifyIncludePayload")
            .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_NOTIFY_INCLUDE_PAYLOAD),
        summary_every_markets,
    })
}

fn pair_lock_adaptive_notify_cent(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.1}¢"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn pair_lock_adaptive_notify_usdc(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.2} USDC"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn pair_lock_adaptive_notify_i64(value: Option<i64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

fn pair_lock_adaptive_notify_f64(payload: &Value, key: &str) -> Option<f64> {
    payload.get(key).and_then(value_as_f64)
}

fn pair_lock_adaptive_notify_str<'a>(payload: &'a Value, key: &str) -> &'a str {
    payload.get(key).and_then(Value::as_str).unwrap_or("unknown")
}

fn pair_lock_adaptive_notify_scope_side(market_slug: &str, outcome_label: &str) -> String {
    let scope = find_updown_scope_by_slug(market_slug)
        .map(|scope| scope.scope.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let side = normalize_pair_lock_binary_outcome(outcome_label)
        .map(|value| value.to_ascii_uppercase())
        .unwrap_or_else(|| outcome_label.trim().to_ascii_uppercase());
    format!("{scope}:{side}")
}

fn pair_lock_adaptive_no_relax_reason_is_important(reason: &str) -> bool {
    matches!(
        reason,
        "high_volume"
            | "ptb_not_expanding"
            | "pair_cap_below_estimated_fill"
            | "edge_price_cap_below_estimated_fill"
            | "late_risk_block"
            | "recent_sl_cooldown"
            | "depth_guard_not_passed"
            | "counter_depth_not_ok"
            | "book_reliability_not_ok"
            | "q_final_unavailable"
            | "dynamic_threshold_unavailable"
    )
}

fn pair_lock_adaptive_should_notify_no_relax(
    cfg: PairLockAdaptiveMaxPriceNotifyConfig,
    reason: &str,
) -> bool {
    cfg.notify_evaluated
        || cfg.notify_all_no_relax
        || (cfg.notify_no_relax_important
            && pair_lock_adaptive_no_relax_reason_is_important(reason))
}

fn pair_lock_adaptive_notify_signature(
    event_type: &str,
    market_slug: &str,
    outcome_label: &str,
    reason: &str,
) -> String {
    format!(
        "{event_type}:{market_slug}:{}:{reason}",
        normalize_pair_lock_binary_outcome(outcome_label).unwrap_or(outcome_label)
    )
}

fn pair_lock_adaptive_notify_state_key(event_type: &str, suffix: &str) -> String {
    format!("adaptive_max_price_notify_{event_type}_{suffix}")
}

fn pair_lock_adaptive_notify_allowed(
    context: &Value,
    node_key: &str,
    event_type: &str,
    signature: &str,
    cfg: PairLockAdaptiveMaxPriceNotifyConfig,
    force: bool,
) -> bool {
    if force {
        return true;
    }
    let signature_key = pair_lock_adaptive_notify_state_key(event_type, "signature");
    if flow_node_state_string(context, node_key, &signature_key).as_deref() == Some(signature) {
        return false;
    }
    if cfg.min_interval_sec <= 0 {
        return true;
    }
    let sent_at_key = pair_lock_adaptive_notify_state_key(event_type, "sent_at_ms");
    let now_ms = Utc::now().timestamp_millis();
    match flow_node_state_i64(context, node_key, &sent_at_key) {
        Some(previous_ms) => now_ms.saturating_sub(previous_ms) >= cfg.min_interval_sec * 1000,
        None => true,
    }
}

fn set_pair_lock_adaptive_notify_state(
    context: &mut Value,
    node_key: &str,
    event_type: &str,
    signature: &str,
) {
    let signature_key = pair_lock_adaptive_notify_state_key(event_type, "signature");
    let sent_at_key = pair_lock_adaptive_notify_state_key(event_type, "sent_at_ms");
    set_flow_node_state(context, node_key, &signature_key, json!(signature));
    set_flow_node_state(
        context,
        node_key,
        &sent_at_key,
        json!(Utc::now().timestamp_millis()),
    );
}

fn build_pair_lock_adaptive_relax_allowed_message(
    market_slug: &str,
    outcome_label: &str,
    adaptive: &Value,
) -> String {
    let timing = adaptive.get("timing").unwrap_or(&Value::Null);
    let history = adaptive.get("history").unwrap_or(&Value::Null);
    format!(
        "🟢 Adaptive MaxPrice RELAX {} {}\nbase={} -> eff={} | ask={} | fill={}\nsize={} -> {}\nq={} | threshold={} | edgeCap={}\ncounterVWAP={} | pairLimit={}\nvol={} | trend={} | t={}s | lateRisk={}\nledger good={} block={} pending={}\nreason={}",
        pair_lock_adaptive_asset_label(market_slug),
        outcome_label,
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "base_max_price_cent")),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "effective_max_price_cent")),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "ask_cent")),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "estimated_avg_fill_cent")),
        pair_lock_adaptive_notify_usdc(pair_lock_adaptive_notify_f64(adaptive, "base_size_usdc")),
        pair_lock_adaptive_notify_usdc(pair_lock_adaptive_notify_f64(adaptive, "effective_size_usdc")),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "q_final_cent")),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "dynamic_threshold_cent")),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "edge_price_cap_cent")),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "counter_estimated_avg_fill_cent")),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "pair_cap_price_limit_cent")),
        pair_lock_adaptive_notify_str(adaptive, "volume_regime"),
        pair_lock_adaptive_notify_str(adaptive, "ptb_trend"),
        pair_lock_adaptive_notify_i64(timing.get("market_elapsed_s").and_then(Value::as_i64)),
        timing
            .get("late_risk_active")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        history.get("resolved_good_miss_count").and_then(Value::as_u64).unwrap_or(0),
        history.get("resolved_good_block_count").and_then(Value::as_u64).unwrap_or(0),
        history.get("pending_miss_count").and_then(Value::as_u64).unwrap_or(0),
        pair_lock_adaptive_notify_str(adaptive, "reason"),
    )
}

fn build_pair_lock_adaptive_no_relax_message(
    market_slug: &str,
    outcome_label: &str,
    adaptive: &Value,
) -> String {
    let timing = adaptive.get("timing").unwrap_or(&Value::Null);
    format!(
        "🟠 Adaptive MaxPrice NO_RELAX {} {}\nreason={}\nvol={} | trend={} | t={}s | lateRisk={}\nbase={} | ask={} | fill={}\nq={} | threshold={}\nedgeCap={} | pairLimit={} | counterVWAP={}\nAction: no price relax",
        pair_lock_adaptive_asset_label(market_slug),
        outcome_label,
        pair_lock_adaptive_notify_str(adaptive, "reason"),
        pair_lock_adaptive_notify_str(adaptive, "volume_regime"),
        pair_lock_adaptive_notify_str(adaptive, "ptb_trend"),
        pair_lock_adaptive_notify_i64(timing.get("market_elapsed_s").and_then(Value::as_i64)),
        timing
            .get("late_risk_active")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "base_max_price_cent")),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "ask_cent")),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "estimated_avg_fill_cent")),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "q_final_cent")),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "dynamic_threshold_cent")),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "edge_price_cap_cent")),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "pair_cap_price_limit_cent")),
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "counter_estimated_avg_fill_cent")),
    )
}

fn pair_lock_adaptive_asset_label(market_slug: &str) -> String {
    find_updown_scope_by_slug(market_slug)
        .map(|scope| scope.asset.to_ascii_uppercase())
        .unwrap_or_else(|| "MARKET".to_string())
}

fn maybe_append_pair_lock_adaptive_debug_payload(
    mut message: String,
    cfg: PairLockAdaptiveMaxPriceNotifyConfig,
    payload: &Value,
) -> String {
    if cfg.include_payload {
        if let Ok(raw) = serde_json::to_string(payload) {
            message.push_str("\n\npayload=");
            message.push_str(&raw.chars().take(2500).collect::<String>());
        }
    }
    message
}

#[allow(clippy::too_many_arguments)]
async fn emit_pair_lock_adaptive_max_price_notification(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &mut Value,
    event_type: &'static str,
    market_slug: &str,
    outcome_label: &str,
    reason: &str,
    adaptive_payload: Value,
    message: String,
    force: bool,
) -> Result<bool> {
    let cfg = resolve_pair_lock_adaptive_max_price_notify_config(node)?;
    let signature =
        pair_lock_adaptive_notify_signature(event_type, market_slug, outcome_label, reason);
    if !pair_lock_adaptive_notify_allowed(context, &node.key, event_type, &signature, cfg, force) {
        return Ok(false);
    }
    let message = maybe_append_pair_lock_adaptive_debug_payload(message, cfg, &adaptive_payload);
    let sent = send_trade_flow_notification(repo, run, &node.key, event_type, &message).await;
    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        event_type,
        &json!({
            "node_key": node.key,
            "market_slug": market_slug,
            "outcome_label": outcome_label,
            "reason": reason,
            "telegram_sent": sent,
            "scope_side": pair_lock_adaptive_notify_scope_side(market_slug, outcome_label),
            "adaptive_max_price": adaptive_payload,
        }),
    )
    .await?;
    if sent {
        set_pair_lock_adaptive_notify_state(context, &node.key, event_type, &signature);
    }
    Ok(sent)
}

async fn maybe_notify_pair_lock_adaptive_no_relax(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &mut Value,
    market_slug: &str,
    outcome_label: &str,
    adaptive_payload: &Value,
) -> Result<()> {
    let cfg = resolve_pair_lock_adaptive_max_price_notify_config(node)?;
    let reason = pair_lock_adaptive_notify_str(adaptive_payload, "reason");
    if !pair_lock_adaptive_should_notify_no_relax(cfg, reason) {
        return Ok(());
    }
    emit_pair_lock_adaptive_max_price_notification(
        repo,
        run,
        node,
        context,
        ADAPTIVE_MAX_PRICE_EVENT_NO_RELAX_IMPORTANT,
        market_slug,
        outcome_label,
        reason,
        adaptive_payload.clone(),
        build_pair_lock_adaptive_no_relax_message(market_slug, outcome_label, adaptive_payload),
        false,
    )
    .await?;
    Ok(())
}

async fn maybe_notify_pair_lock_adaptive_relax_allowed(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &mut Value,
    market_slug: &str,
    outcome_label: &str,
    adaptive_payload: &Value,
) -> Result<()> {
    let cfg = resolve_pair_lock_adaptive_max_price_notify_config(node)?;
    if !cfg.notify_relax {
        return Ok(());
    }
    emit_pair_lock_adaptive_max_price_notification(
        repo,
        run,
        node,
        context,
        ADAPTIVE_MAX_PRICE_EVENT_RELAX_ALLOWED,
        market_slug,
        outcome_label,
        pair_lock_adaptive_notify_str(adaptive_payload, "reason"),
        adaptive_payload.clone(),
        build_pair_lock_adaptive_relax_allowed_message(market_slug, outcome_label, adaptive_payload),
        true,
    )
    .await?;
    Ok(())
}

fn pair_lock_adaptive_payload_is_applied(adaptive_payload: &Value) -> bool {
    adaptive_payload
        .get("relax_applied")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn pair_lock_adaptive_payload_from_flow_payload(flow_payload: &Value) -> Option<Value> {
    [
        flow_payload.get("adaptive_max_price"),
        flow_payload.pointer("/node_snapshot/action_node/config/adaptiveMaxPrice"),
        flow_payload.pointer("/action_node/config/adaptiveMaxPrice"),
    ]
    .into_iter()
    .flatten()
    .find(|payload| pair_lock_adaptive_payload_is_applied(payload))
    .cloned()
}

fn pair_lock_adaptive_node_from_flow_payload(
    parent_order: &TradeBuilderOrder,
    flow_payload: &Value,
) -> Option<TradeFlowNode> {
    let config = flow_payload
        .pointer("/node_snapshot/action_node/config")
        .or_else(|| flow_payload.pointer("/action_node/config"))?
        .clone();
    Some(TradeFlowNode {
        key: parent_order
            .origin_flow_node_key
            .clone()
            .unwrap_or_else(|| "action.place_order".to_string()),
        node_type: "action.place_order".to_string(),
        config,
    })
}

async fn pair_lock_adaptive_source_action_node(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    parent_order: &TradeBuilderOrder,
    flow_payload: &Value,
) -> Result<Option<TradeFlowNode>> {
    if let Some(node_key) = parent_order
        .origin_flow_node_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Some(version) = repo.get_trade_flow_version(run.version_id).await? {
            let graph = parse_trade_flow_graph(&version)?;
            if let Some(node) = flow_node(&graph, node_key) {
                return Ok(Some(node.clone()));
            }
        }
    }
    Ok(pair_lock_adaptive_node_from_flow_payload(
        parent_order,
        flow_payload,
    ))
}

fn build_pair_lock_adaptive_relax_sl_message(
    parent_order: &TradeBuilderOrder,
    stop_loss_order: &TradeBuilderOrder,
    execution_price: f64,
    filled_qty: f64,
    adaptive: &Value,
) -> String {
    let entry_cent = pair_lock_adaptive_notify_f64(adaptive, "estimated_avg_fill_cent")
        .or_else(|| pair_lock_adaptive_notify_f64(adaptive, "effective_max_price_cent"));
    let pnl = entry_cent.map(|cent| (execution_price - cent / 100.0) * filled_qty);
    format!(
        "🔴 Adaptive Relax SL {} {}\nentry={} | exit={} | size={}\nSL child={} | PnL={}\nCooldown:\nscope={}\nadaptive relax disabled for next markets\nrelax credit reset",
        pair_lock_adaptive_asset_label(&parent_order.market_slug),
        parent_order.outcome_label,
        pair_lock_adaptive_notify_cent(entry_cent),
        pair_lock_adaptive_notify_cent(Some(execution_price * 100.0)),
        pair_lock_adaptive_notify_usdc(Some(filled_qty * execution_price)),
        stop_loss_order.id,
        pair_lock_adaptive_notify_usdc(pnl),
        pair_lock_adaptive_notify_scope_side(&parent_order.market_slug, &parent_order.outcome_label),
    )
}

fn build_pair_lock_adaptive_cooldown_started_message(
    parent_order: &TradeBuilderOrder,
    sl_cooldown_markets: usize,
) -> String {
    format!(
        "🔴 Adaptive MaxPrice Cooldown {} {}\nscope={}\nadaptive relax disabled for next {} markets",
        pair_lock_adaptive_asset_label(&parent_order.market_slug),
        parent_order.outcome_label,
        pair_lock_adaptive_notify_scope_side(&parent_order.market_slug, &parent_order.outcome_label),
        sl_cooldown_markets,
    )
}

async fn pair_lock_adaptive_event_already_exists(
    repo: &PostgresRepository,
    run_id: i64,
    event_type: &'static str,
    market_slug: &str,
    outcome_label: &str,
    extra_key: &str,
    extra_value: &str,
) -> Result<bool> {
    let events = repo
        .list_trade_flow_events_for_run_types(run_id, &[event_type])
        .await?;
    fn event_extra_matches(payload: &Value, key: &str, expected: &str) -> bool {
        let value = payload
            .get(key)
            .or_else(|| payload.get("adaptive_max_price").and_then(|nested| nested.get(key)));
        match value {
            Some(Value::String(value)) => value == expected,
            Some(Value::Number(value)) => value.to_string() == expected,
            Some(Value::Bool(value)) => value.to_string() == expected,
            _ => false,
        }
    }
    Ok(events.iter().any(|event| {
        event.payload_json.get("market_slug").and_then(Value::as_str) == Some(market_slug)
            && event
                .payload_json
                .get("outcome_label")
                .and_then(Value::as_str)
                .and_then(normalize_pair_lock_binary_outcome)
                == normalize_pair_lock_binary_outcome(outcome_label)
            && event_extra_matches(&event.payload_json, extra_key, extra_value)
    }))
}

async fn pair_lock_adaptive_scope_rate_limited(
    repo: &PostgresRepository,
    run_id: i64,
    event_type: &'static str,
    scope_side: &str,
    min_interval_sec: i64,
) -> Result<bool> {
    if min_interval_sec <= 0 {
        return Ok(false);
    }
    let events = repo
        .list_trade_flow_events_for_run_types(run_id, &[event_type])
        .await?;
    let now = Utc::now();
    Ok(events.iter().any(|event| {
        event.payload_json.get("scope_side").and_then(Value::as_str) == Some(scope_side)
            && now
                .signed_duration_since(event.created_at)
                .num_seconds()
                < min_interval_sec
    }))
}

#[allow(clippy::too_many_arguments)]
async fn emit_pair_lock_adaptive_db_notification(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    cfg: PairLockAdaptiveMaxPriceNotifyConfig,
    event_type: &'static str,
    market_slug: &str,
    outcome_label: &str,
    reason: &str,
    payload: Value,
    message: String,
) -> Result<bool> {
    let message = maybe_append_pair_lock_adaptive_debug_payload(message, cfg, &payload);
    let sent = send_trade_flow_notification(repo, run, &node.key, event_type, &message).await;
    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        event_type,
        &json!({
            "node_key": node.key,
            "market_slug": market_slug,
            "outcome_label": outcome_label,
            "reason": reason,
            "telegram_sent": sent,
            "scope_side": pair_lock_adaptive_notify_scope_side(market_slug, outcome_label),
            "adaptive_max_price": payload,
        }),
    )
    .await?;
    Ok(sent)
}

async fn maybe_notify_pair_lock_adaptive_relax_sl(
    repo: &PostgresRepository,
    parent_order: &TradeBuilderOrder,
    stop_loss_order: &TradeBuilderOrder,
    execution_price: f64,
    filled_qty: f64,
) -> Result<()> {
    let Some(run_id) = parent_order.origin_flow_run_id else {
        return Ok(());
    };
    let Some(run) = repo.get_trade_flow_run(run_id).await? else {
        return Ok(());
    };
    let Some(flow_payload) = load_trade_builder_latest_flow_payload(repo, parent_order.id).await?
    else {
        return Ok(());
    };
    let Some(adaptive) = pair_lock_adaptive_payload_from_flow_payload(&flow_payload) else {
        return Ok(());
    };
    let Some(node) = pair_lock_adaptive_source_action_node(repo, &run, parent_order, &flow_payload)
        .await?
    else {
        return Ok(());
    };
    let cfg = resolve_pair_lock_adaptive_max_price_notify_config(&node)?;
    let cooldown_markets = node_config_i64(&node, "adaptiveMaxPriceSlCooldownMarkets")
        .filter(|value| *value >= 0)
        .map(|value| value as usize)
        .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_SL_COOLDOWN_MARKETS);
    let payload = json!({
        "parent_builder_order_id": parent_order.id,
        "sl_builder_order_id": stop_loss_order.id,
        "execution_price": execution_price,
        "filled_qty": filled_qty,
        "cooldown_markets": cooldown_markets,
        "adaptive_max_price": adaptive,
    });

    if cfg.notify_relax_sl
        && !pair_lock_adaptive_event_already_exists(
            repo,
            run.id,
            ADAPTIVE_MAX_PRICE_EVENT_RELAX_SL,
            &parent_order.market_slug,
            &parent_order.outcome_label,
            "sl_builder_order_id",
            &stop_loss_order.id.to_string(),
        )
        .await?
    {
        emit_pair_lock_adaptive_db_notification(
            repo,
            &run,
            &node,
            cfg,
            ADAPTIVE_MAX_PRICE_EVENT_RELAX_SL,
            &parent_order.market_slug,
            &parent_order.outcome_label,
            "adaptive_relaxed_trade_sl",
            payload.clone(),
            build_pair_lock_adaptive_relax_sl_message(
                parent_order,
                stop_loss_order,
                execution_price,
                filled_qty,
                payload
                    .get("adaptive_max_price")
                    .unwrap_or(&Value::Null),
            ),
        )
        .await?;
    }

    if cfg.notify_cooldown
        && !pair_lock_adaptive_event_already_exists(
            repo,
            run.id,
            ADAPTIVE_MAX_PRICE_EVENT_COOLDOWN_STARTED,
            &parent_order.market_slug,
            &parent_order.outcome_label,
            "sl_builder_order_id",
            &stop_loss_order.id.to_string(),
        )
        .await?
    {
        let mut cooldown_payload = payload.clone();
        if let Some(target) = cooldown_payload.as_object_mut() {
            target.insert(
                "sl_builder_order_id".to_string(),
                json!(stop_loss_order.id.to_string()),
            );
        }
        emit_pair_lock_adaptive_db_notification(
            repo,
            &run,
            &node,
            cfg,
            ADAPTIVE_MAX_PRICE_EVENT_COOLDOWN_STARTED,
            &parent_order.market_slug,
            &parent_order.outcome_label,
            "adaptive_relaxed_trade_sl_cooldown",
            cooldown_payload,
            build_pair_lock_adaptive_cooldown_started_message(parent_order, cooldown_markets),
        )
        .await?;
    }

    Ok(())
}

fn pair_lock_adaptive_input_outcome_label(
    input: &bot_infra::db::TradeFlowAutoTuneMarketSummaryInput,
) -> String {
    input
        .metrics_json
        .get("adaptive_max_price")
        .and_then(|value| value.get("outcome_label"))
        .and_then(Value::as_str)
        .or_else(|| input.metrics_json.get("outcome_label").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string()
}

fn build_pair_lock_adaptive_miss_resolved_message(
    market_slug: &str,
    outcome_label: &str,
    classification: &str,
    input: &bot_infra::db::TradeFlowAutoTuneMarketSummaryInput,
) -> String {
    let adaptive = input
        .metrics_json
        .get("adaptive_max_price")
        .unwrap_or(&Value::Null);
    let pnl = pair_lock_adaptive_notify_f64(adaptive, "shadow_hold_pnl_usdc")
        .or_else(|| pair_lock_adaptive_notify_f64(adaptive, "shadow_tp_sl_pnl_usdc"));
    let title = if classification == "good_miss" {
        "✅ Adaptive Miss Resolved: GOOD_MISS"
    } else {
        "🧱 Adaptive Miss Resolved: GOOD_BLOCK"
    };
    format!(
        "{title} {} {}\nmiss: PTB pass + maxPrice block\nshadow fill={}\nshadow pnl={}\nledger={}\nResult: {}",
        pair_lock_adaptive_asset_label(market_slug),
        outcome_label,
        pair_lock_adaptive_notify_cent(pair_lock_adaptive_notify_f64(adaptive, "estimated_avg_fill_cent")),
        pair_lock_adaptive_notify_usdc(pnl),
        pair_lock_adaptive_notify_scope_side(market_slug, outcome_label),
        if classification == "good_miss" {
            "adaptive relax may be eligible next safe setup"
        } else {
            "maxPrice block was protective"
        },
    )
}

fn build_pair_lock_adaptive_summary_message(
    market_slug: &str,
    outcome_label: &str,
    resolved_count: usize,
    good_count: usize,
    block_count: usize,
    pending_count: usize,
    unknown_count: usize,
    relaxed_sl_count: usize,
) -> String {
    format!(
        "📊 Adaptive MaxPrice Summary {} {}\nresolved={resolved_count}\ngood_miss={good_count} | good_block={block_count}\npending={pending_count} | unknown={unknown_count}\nrelaxed SL={relaxed_sl_count}\nscope={}",
        pair_lock_adaptive_asset_label(market_slug),
        outcome_label,
        pair_lock_adaptive_notify_scope_side(market_slug, outcome_label),
    )
}

fn build_pair_lock_adaptive_cooldown_cleared_message(
    market_slug: &str,
    outcome_label: &str,
) -> String {
    format!(
        "🟢 Adaptive MaxPrice Cooldown Cleared {} {}\nscope={}\nadaptive relax can evaluate again on safe setups",
        pair_lock_adaptive_asset_label(market_slug),
        outcome_label,
        pair_lock_adaptive_notify_scope_side(market_slug, outcome_label),
    )
}

fn pair_lock_adaptive_summary_event_newer_than_start(
    events: &[TradeFlowEventRecord],
    cleared: bool,
) -> bool {
    let start = events
        .iter()
        .find(|event| event.event_type == ADAPTIVE_MAX_PRICE_EVENT_COOLDOWN_STARTED);
    let clear = events
        .iter()
        .find(|event| event.event_type == ADAPTIVE_MAX_PRICE_EVENT_COOLDOWN_CLEARED);
    match (start, clear) {
        (Some(start), Some(clear)) => {
            let clear_is_newer = clear.created_at > start.created_at;
            if cleared { clear_is_newer } else { !clear_is_newer }
        }
        (Some(_), None) => !cleared,
        _ => false,
    }
}

async fn maybe_notify_pair_lock_adaptive_market_summary(
    repo: &PostgresRepository,
    run_spec: &WsOpenPositionPriceRunSpec,
    node: &TradeFlowNode,
    input: &bot_infra::db::TradeFlowAutoTuneMarketSummaryInput,
) -> Result<()> {
    if !action_place_order_uses_adaptive_max_price_strategy(node) {
        return Ok(());
    }
    let Some(run) = repo.get_trade_flow_run(run_spec.run_id).await? else {
        return Ok(());
    };
    let cfg = resolve_pair_lock_adaptive_max_price_notify_config(node)?;
    let outcome_label = pair_lock_adaptive_input_outcome_label(input);
    let scope_side = pair_lock_adaptive_notify_scope_side(&input.market_slug, &outcome_label);
    let classification = pair_lock_adaptive_miss_classification_from_metrics(&input.metrics_json);

    if cfg.notify_miss_resolved
        && matches!(classification, "good_miss" | "good_block")
        && !pair_lock_adaptive_event_already_exists(
            repo,
            run.id,
            ADAPTIVE_MAX_PRICE_EVENT_MISS_RESOLVED,
            &input.market_slug,
            &outcome_label,
            "classification",
            classification,
        )
        .await?
        && !pair_lock_adaptive_scope_rate_limited(
            repo,
            run.id,
            ADAPTIVE_MAX_PRICE_EVENT_MISS_RESOLVED,
            &scope_side,
            cfg.min_interval_sec,
        )
        .await?
    {
        emit_pair_lock_adaptive_db_notification(
            repo,
            &run,
            node,
            cfg,
            ADAPTIVE_MAX_PRICE_EVENT_MISS_RESOLVED,
            &input.market_slug,
            &outcome_label,
            classification,
            json!({
                "classification": classification,
                "market_summary": input.metrics_json,
            }),
            build_pair_lock_adaptive_miss_resolved_message(
                &input.market_slug,
                &outcome_label,
                classification,
                input,
            ),
        )
        .await?;
    }

    let summaries = repo
        .list_trade_flow_auto_tune_market_summaries(
            input.definition_id,
            input.version_id,
            &input.node_key,
            &input.market_scope,
            100,
        )
        .await?;
    let side_summaries = summaries
        .iter()
        .filter(|summary| pair_lock_adaptive_summary_outcome_matches(summary, &outcome_label))
        .collect::<Vec<_>>();
    let miss_summaries = side_summaries
        .iter()
        .copied()
        .filter(|summary| pair_lock_adaptive_summary_is_ptb_max_price_miss(summary))
        .collect::<Vec<_>>();
    let good_count = miss_summaries
        .iter()
        .filter(|summary| pair_lock_adaptive_miss_classification(summary) == "good_miss")
        .count();
    let block_count = miss_summaries
        .iter()
        .filter(|summary| pair_lock_adaptive_miss_classification(summary) == "good_block")
        .count();
    let pending_count = miss_summaries
        .iter()
        .filter(|summary| pair_lock_adaptive_miss_classification(summary) == "pending_miss")
        .count();
    let unknown_count = miss_summaries
        .iter()
        .filter(|summary| {
            !matches!(
                pair_lock_adaptive_miss_classification(summary),
                "good_miss" | "good_block" | "pending_miss"
            )
        })
        .count();
    let resolved_count = good_count + block_count;
    let relaxed_sl_count = side_summaries
        .iter()
        .filter(|summary| summary.sl_hit)
        .count();

    if cfg.notify_summary
        && matches!(classification, "good_miss" | "good_block")
        && resolved_count > 0
        && resolved_count % cfg.summary_every_markets == 0
        && !pair_lock_adaptive_event_already_exists(
            repo,
            run.id,
            ADAPTIVE_MAX_PRICE_EVENT_SUMMARY,
            &input.market_slug,
            &outcome_label,
            "resolved_count",
            &resolved_count.to_string(),
        )
        .await?
        && !pair_lock_adaptive_scope_rate_limited(
            repo,
            run.id,
            ADAPTIVE_MAX_PRICE_EVENT_SUMMARY,
            &scope_side,
            cfg.min_interval_sec,
        )
        .await?
    {
        emit_pair_lock_adaptive_db_notification(
            repo,
            &run,
            node,
            cfg,
            ADAPTIVE_MAX_PRICE_EVENT_SUMMARY,
            &input.market_slug,
            &outcome_label,
            "resolved_market_summary",
            json!({
                "resolved_count": resolved_count.to_string(),
                "resolved_good_miss_count": good_count,
                "resolved_good_block_count": block_count,
                "pending_miss_count": pending_count,
                "unknown_miss_count": unknown_count,
                "relaxed_sl_count": relaxed_sl_count,
                "scope_side": scope_side,
            }),
            build_pair_lock_adaptive_summary_message(
                &input.market_slug,
                &outcome_label,
                resolved_count,
                good_count,
                block_count,
                pending_count,
                unknown_count,
                relaxed_sl_count,
            ),
        )
        .await?;
    }

    let cooldown_markets = node_config_i64(node, "adaptiveMaxPriceSlCooldownMarkets")
        .filter(|value| *value >= 0)
        .map(|value| value as usize)
        .unwrap_or(DEFAULT_ADAPTIVE_MAX_PRICE_SL_COOLDOWN_MARKETS);
    let cooldown_active = cooldown_markets > 0
        && side_summaries
            .iter()
            .take(cooldown_markets)
            .any(|summary| summary.sl_hit);
    if cfg.notify_cooldown && !cooldown_active {
        let events = repo
            .list_trade_flow_events_for_run_types(
                run.id,
                &[
                    ADAPTIVE_MAX_PRICE_EVENT_COOLDOWN_STARTED,
                    ADAPTIVE_MAX_PRICE_EVENT_COOLDOWN_CLEARED,
                ],
            )
            .await?
            .into_iter()
            .filter(|event| {
                event.payload_json.get("scope_side").and_then(Value::as_str)
                    == Some(scope_side.as_str())
            })
            .collect::<Vec<_>>();
        if pair_lock_adaptive_summary_event_newer_than_start(&events, false) {
            emit_pair_lock_adaptive_db_notification(
                repo,
                &run,
                node,
                cfg,
                ADAPTIVE_MAX_PRICE_EVENT_COOLDOWN_CLEARED,
                &input.market_slug,
                &outcome_label,
                "adaptive_sl_cooldown_cleared",
                json!({
                    "scope_side": scope_side,
                    "cooldown_markets": cooldown_markets,
                }),
                build_pair_lock_adaptive_cooldown_cleared_message(
                    &input.market_slug,
                    &outcome_label,
                ),
            )
            .await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod pair_lock_adaptive_notify_tests {
    use super::*;

    fn notify_node(config: Value) -> TradeFlowNode {
        TradeFlowNode {
            key: "pair_buy".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    #[test]
    fn adaptive_no_relax_important_reason_filter_is_selective() {
        assert!(pair_lock_adaptive_no_relax_reason_is_important("high_volume"));
        assert!(pair_lock_adaptive_no_relax_reason_is_important("ptb_not_expanding"));
        assert!(!pair_lock_adaptive_no_relax_reason_is_important(
            "resolved_good_miss_below_required"
        ));
        assert!(!pair_lock_adaptive_no_relax_reason_is_important(
            "estimated_avg_fill_unavailable"
        ));
    }

    #[test]
    fn adaptive_notify_config_defaults_to_filtered_enabled() {
        let cfg = resolve_pair_lock_adaptive_max_price_notify_config(&notify_node(json!({})))
            .expect("notify config");
        assert!(!cfg.notify_evaluated);
        assert!(cfg.notify_relax);
        assert!(cfg.notify_relax_sl);
        assert!(cfg.notify_no_relax_important);
        assert!(cfg.notify_miss_resolved);
        assert!(cfg.notify_cooldown);
        assert!(cfg.notify_summary);
        assert!(!cfg.notify_all_no_relax);
        assert_eq!(cfg.min_interval_sec, 30);
        assert_eq!(cfg.summary_every_markets, 5);
    }

    #[test]
    fn adaptive_notify_dedupes_same_market_outcome_reason() {
        let cfg = resolve_pair_lock_adaptive_max_price_notify_config(&notify_node(json!({})))
            .expect("notify config");
        let mut context = json!({});
        let event_type = ADAPTIVE_MAX_PRICE_EVENT_NO_RELAX_IMPORTANT;
        let signature = pair_lock_adaptive_notify_signature(
            event_type,
            "eth-updown-5m-1",
            "Up",
            "high_volume",
        );
        assert!(pair_lock_adaptive_notify_allowed(
            &context, "pair_buy", event_type, &signature, cfg, false
        ));
        set_pair_lock_adaptive_notify_state(&mut context, "pair_buy", event_type, &signature);
        assert!(!pair_lock_adaptive_notify_allowed(
            &context, "pair_buy", event_type, &signature, cfg, false
        ));
        assert!(pair_lock_adaptive_notify_allowed(
            &context, "pair_buy", event_type, &signature, cfg, true
        ));
    }
}
