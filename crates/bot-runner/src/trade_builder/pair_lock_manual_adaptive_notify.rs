const DEFAULT_MANUAL_ADAPTIVE_NOTIFY_BLOCK: bool = true;
const DEFAULT_MANUAL_ADAPTIVE_NOTIFY_STRICT: bool = true;
const DEFAULT_MANUAL_ADAPTIVE_NOTIFY_SL_BUMP: bool = true;
const DEFAULT_MANUAL_ADAPTIVE_NOTIFY_SUMMARY: bool = true;
const DEFAULT_MANUAL_ADAPTIVE_NOTIFY_MIN_INTERVAL_SEC: i64 = 30;
const DEFAULT_MANUAL_ADAPTIVE_NOTIFY_INCLUDE_PAYLOAD: bool = false;
const DEFAULT_MANUAL_ADAPTIVE_SUMMARY_EVERY_MARKETS: i64 = 5;

const MANUAL_ADAPTIVE_EVENT_BLOCK: &str = "manual_adaptive_risk_block";
const MANUAL_ADAPTIVE_EVENT_STRICT: &str = "manual_adaptive_risk_strict";
const MANUAL_ADAPTIVE_EVENT_SL_BUMP: &str = "manual_adaptive_risk_sl_bump";
const MANUAL_ADAPTIVE_EVENT_SUMMARY: &str = "manual_adaptive_risk_summary";

#[derive(Debug, Clone, Copy)]
struct PairLockManualAdaptiveNotifyConfig {
    notify_block: bool,
    notify_strict: bool,
    notify_sl_bump: bool,
    notify_summary: bool,
    min_interval_sec: i64,
    include_payload: bool,
}

fn resolve_pair_lock_manual_adaptive_notify_config(
    node: &TradeFlowNode,
) -> Result<PairLockManualAdaptiveNotifyConfig> {
    let min_interval_sec = node_config_i64(node, "manualAdaptiveNotifyMinIntervalSec")
        .unwrap_or(DEFAULT_MANUAL_ADAPTIVE_NOTIFY_MIN_INTERVAL_SEC);
    anyhow::ensure!(
        min_interval_sec >= 0,
        "action.place_order manualAdaptiveNotifyMinIntervalSec must be >= 0"
    );
    Ok(PairLockManualAdaptiveNotifyConfig {
        notify_block: node_config_bool(node, "notifyOnManualAdaptiveRiskBlock")
            .unwrap_or(DEFAULT_MANUAL_ADAPTIVE_NOTIFY_BLOCK),
        notify_strict: node_config_bool(node, "notifyOnManualAdaptiveRiskStrict")
            .unwrap_or(DEFAULT_MANUAL_ADAPTIVE_NOTIFY_STRICT),
        notify_sl_bump: node_config_bool(node, "notifyOnManualAdaptiveRiskSlBump")
            .unwrap_or(DEFAULT_MANUAL_ADAPTIVE_NOTIFY_SL_BUMP),
        notify_summary: node_config_bool(node, "notifyOnManualAdaptiveRiskSummary")
            .unwrap_or(DEFAULT_MANUAL_ADAPTIVE_NOTIFY_SUMMARY),
        min_interval_sec,
        include_payload: node_config_bool(node, "manualAdaptiveNotifyIncludePayload")
            .unwrap_or(DEFAULT_MANUAL_ADAPTIVE_NOTIFY_INCLUDE_PAYLOAD),
    })
}

fn pair_lock_manual_notify_cent(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.1}¢"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn pair_lock_manual_notify_usdc(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.2} USDC"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn pair_lock_manual_notify_asset_label(market_slug: &str) -> String {
    find_updown_scope_by_slug(market_slug)
        .map(|scope| scope.asset.to_ascii_uppercase())
        .unwrap_or_else(|| "MARKET".to_string())
}

fn pair_lock_manual_notify_state_key(event_type: &str, suffix: &str) -> String {
    format!("manual_adaptive_risk_notify_{event_type}_{suffix}")
}

fn pair_lock_manual_notify_signature(
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

fn pair_lock_manual_summary_state_key(scope_side: &str, suffix: &str) -> String {
    let normalized = scope_side
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("manual_adaptive_risk_summary_{normalized}_{suffix}")
}

fn pair_lock_manual_notify_allowed(
    context: &Value,
    node_key: &str,
    event_type: &str,
    signature: &str,
    cfg: PairLockManualAdaptiveNotifyConfig,
    force: bool,
) -> bool {
    if force {
        return true;
    }
    let signature_key = pair_lock_manual_notify_state_key(event_type, "signature");
    if flow_node_state_string(context, node_key, &signature_key).as_deref() == Some(signature) {
        return false;
    }
    if cfg.min_interval_sec <= 0 {
        return true;
    }
    let sent_at_key = pair_lock_manual_notify_state_key(event_type, "sent_at_ms");
    let now_ms = Utc::now().timestamp_millis();
    match flow_node_state_i64(context, node_key, &sent_at_key) {
        Some(previous_ms) => now_ms.saturating_sub(previous_ms) >= cfg.min_interval_sec * 1000,
        None => true,
    }
}

fn set_pair_lock_manual_notify_state(
    context: &mut Value,
    node_key: &str,
    event_type: &str,
    signature: &str,
) {
    let signature_key = pair_lock_manual_notify_state_key(event_type, "signature");
    let sent_at_key = pair_lock_manual_notify_state_key(event_type, "sent_at_ms");
    set_flow_node_state(context, node_key, &signature_key, json!(signature));
    set_flow_node_state(
        context,
        node_key,
        &sent_at_key,
        json!(Utc::now().timestamp_millis()),
    );
}

fn maybe_append_pair_lock_manual_debug_payload(
    mut message: String,
    cfg: PairLockManualAdaptiveNotifyConfig,
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

fn build_pair_lock_manual_adaptive_decision_message(
    market_slug: &str,
    outcome_label: &str,
    payload: &Value,
) -> String {
    let decision = payload
        .get("decision")
        .and_then(Value::as_str)
        .unwrap_or("BASE");
    let reason = payload
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let effective = payload.get("effective").unwrap_or(&Value::Null);
    let base = payload.get("base").unwrap_or(&Value::Null);
    let market = payload.get("market").unwrap_or(&Value::Null);
    let title = if decision == "BLOCK" {
        "🟠 Manual Adaptive BLOCK"
    } else {
        "🟡 Manual Adaptive STRICT"
    };
    format!(
        "{title} {} {}\nvol={} | trend={} | reason={}\nmax={} -> {} | size={} -> {}\nptbGap={} {} | ask={} | counterCap={}\nreentry=OFF",
        pair_lock_manual_notify_asset_label(market_slug),
        outcome_label,
        payload.get("volume_regime").and_then(Value::as_str).unwrap_or("unknown"),
        payload.get("ptb_trend").and_then(Value::as_str).unwrap_or("unknown"),
        reason,
        pair_lock_manual_notify_cent(base.get("max_price_cent").and_then(value_as_f64)),
        pair_lock_manual_notify_cent(effective.get("max_price_cent").and_then(value_as_f64)),
        pair_lock_manual_notify_usdc(base.get("size_usdc").and_then(value_as_f64)),
        pair_lock_manual_notify_usdc(effective.get("size_usdc").and_then(value_as_f64)),
        effective
            .get("ptb_threshold_value")
            .and_then(value_as_f64)
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "N/A".to_string()),
        effective
            .get("ptb_threshold_unit")
            .and_then(Value::as_str)
            .unwrap_or(""),
        pair_lock_manual_notify_cent(market.get("ask_cent").and_then(value_as_f64)),
        pair_lock_manual_notify_cent(effective.get("counter_max_price_cent").and_then(value_as_f64)),
    )
}

fn build_pair_lock_manual_adaptive_summary_message(
    market_slug: &str,
    outcome_label: &str,
    payload: &Value,
) -> String {
    format!(
        "📊 Manual Adaptive Summary {} {}\nmarkets={} | evaluated={}\nblocks={} | strict={} | base={}\nlast_reason={}",
        pair_lock_manual_notify_asset_label(market_slug),
        outcome_label,
        payload
            .get("markets")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        payload
            .get("evaluated")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        payload
            .get("blocks")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        payload
            .get("strict")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        payload
            .get("base")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        payload
            .get("last_reason")
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
    )
}

async fn emit_pair_lock_manual_adaptive_notification(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &mut Value,
    event_type: &'static str,
    market_slug: &str,
    outcome_label: &str,
    reason: &str,
    payload: Value,
    message: String,
    force: bool,
) -> Result<bool> {
    let cfg = resolve_pair_lock_manual_adaptive_notify_config(node)?;
    let signature =
        pair_lock_manual_notify_signature(event_type, market_slug, outcome_label, reason);
    if !pair_lock_manual_notify_allowed(context, &node.key, event_type, &signature, cfg, force) {
        return Ok(false);
    }
    let message = maybe_append_pair_lock_manual_debug_payload(message, cfg, &payload);
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
            "scope_side": pair_lock_manual_adaptive_scope_side(market_slug, outcome_label),
            "manual_adaptive_risk": payload,
        }),
    )
    .await?;
    if sent {
        set_pair_lock_manual_notify_state(context, &node.key, event_type, &signature);
    }
    Ok(sent)
}

async fn maybe_notify_pair_lock_manual_adaptive_risk_decision(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &mut Value,
    market_slug: &str,
    outcome_label: &str,
    payload: &Value,
) -> Result<()> {
    let cfg = resolve_pair_lock_manual_adaptive_notify_config(node)?;
    let decision = payload
        .get("decision")
        .and_then(Value::as_str)
        .unwrap_or("BASE");
    let reason = payload
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    maybe_notify_pair_lock_manual_adaptive_summary(
        repo,
        run,
        node,
        context,
        market_slug,
        outcome_label,
        decision,
        reason,
        cfg,
    )
    .await?;
    let event_type = match decision {
        "BLOCK" if cfg.notify_block => MANUAL_ADAPTIVE_EVENT_BLOCK,
        "ALLOW_STRICT" if cfg.notify_strict && reason != "base_normal_expanding" => {
            MANUAL_ADAPTIVE_EVENT_STRICT
        }
        _ => return Ok(()),
    };
    emit_pair_lock_manual_adaptive_notification(
        repo,
        run,
        node,
        context,
        event_type,
        market_slug,
        outcome_label,
        reason,
        payload.clone(),
        build_pair_lock_manual_adaptive_decision_message(market_slug, outcome_label, payload),
        false,
    )
    .await?;
    Ok(())
}

async fn maybe_notify_pair_lock_manual_adaptive_summary(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &mut Value,
    market_slug: &str,
    outcome_label: &str,
    decision: &str,
    reason: &str,
    cfg: PairLockManualAdaptiveNotifyConfig,
) -> Result<()> {
    if !cfg.notify_summary {
        return Ok(());
    }
    let scope_side = pair_lock_manual_adaptive_scope_side(market_slug, outcome_label);
    let market_key = pair_lock_manual_summary_state_key(&scope_side, "last_market");
    let markets_key = pair_lock_manual_summary_state_key(&scope_side, "markets");
    let evaluated_key = pair_lock_manual_summary_state_key(&scope_side, "evaluated");
    let blocks_key = pair_lock_manual_summary_state_key(&scope_side, "blocks");
    let strict_key = pair_lock_manual_summary_state_key(&scope_side, "strict");
    let base_key = pair_lock_manual_summary_state_key(&scope_side, "base");
    let sent_market_key = pair_lock_manual_summary_state_key(&scope_side, "sent_market");
    let previous_market = flow_node_state_string(context, &node.key, &market_key);
    if previous_market.as_deref() != Some(market_slug) {
        let markets = flow_node_state_i64(context, &node.key, &markets_key)
            .unwrap_or_default()
            .saturating_add(1);
        set_flow_node_state(context, &node.key, &markets_key, json!(markets));
        set_flow_node_state(context, &node.key, &market_key, json!(market_slug));
    }
    let evaluated = flow_node_state_i64(context, &node.key, &evaluated_key)
        .unwrap_or_default()
        .saturating_add(1);
    set_flow_node_state(context, &node.key, &evaluated_key, json!(evaluated));
    let count_key = match decision {
        "BLOCK" => Some(blocks_key.as_str()),
        "ALLOW_STRICT" => Some(strict_key.as_str()),
        "BASE" => Some(base_key.as_str()),
        _ => None,
    };
    if let Some(count_key) = count_key {
        let count = flow_node_state_i64(context, &node.key, count_key)
            .unwrap_or_default()
            .saturating_add(1);
        set_flow_node_state(context, &node.key, count_key, json!(count));
    }
    let markets = flow_node_state_i64(context, &node.key, &markets_key).unwrap_or_default();
    if markets <= 0 || markets % DEFAULT_MANUAL_ADAPTIVE_SUMMARY_EVERY_MARKETS != 0 {
        return Ok(());
    }
    if flow_node_state_string(context, &node.key, &sent_market_key).as_deref()
        == Some(market_slug)
    {
        return Ok(());
    }
    let summary_payload = json!({
        "scope_side": scope_side,
        "markets": markets,
        "evaluated": evaluated,
        "blocks": flow_node_state_i64(context, &node.key, &blocks_key).unwrap_or_default(),
        "strict": flow_node_state_i64(context, &node.key, &strict_key).unwrap_or_default(),
        "base": flow_node_state_i64(context, &node.key, &base_key).unwrap_or_default(),
        "last_reason": reason,
    });
    if emit_pair_lock_manual_adaptive_notification(
        repo,
        run,
        node,
        context,
        MANUAL_ADAPTIVE_EVENT_SUMMARY,
        market_slug,
        outcome_label,
        "manual_adaptive_summary",
        summary_payload.clone(),
        build_pair_lock_manual_adaptive_summary_message(
            market_slug,
            outcome_label,
            &summary_payload,
        ),
        false,
    )
    .await?
    {
        set_flow_node_state(context, &node.key, &sent_market_key, json!(market_slug));
    }
    Ok(())
}

fn pair_lock_manual_payload_is_applied(payload: &Value) -> bool {
    payload
        .get("applied")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn pair_lock_manual_payload_from_flow_payload(flow_payload: &Value) -> Option<Value> {
    [
        flow_payload.get("manual_adaptive_risk"),
        flow_payload.pointer("/node_snapshot/action_node/config/manualAdaptiveRisk"),
        flow_payload.pointer("/action_node/config/manualAdaptiveRisk"),
    ]
    .into_iter()
    .flatten()
    .find(|payload| pair_lock_manual_payload_is_applied(payload))
    .cloned()
}

fn pair_lock_manual_node_from_flow_payload(
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

async fn pair_lock_manual_source_action_node(
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
    Ok(pair_lock_manual_node_from_flow_payload(parent_order, flow_payload))
}

fn build_pair_lock_manual_adaptive_sl_bump_message(
    parent_order: &TradeBuilderOrder,
    stop_loss_order: &TradeBuilderOrder,
    payload: &Value,
) -> String {
    let manual = payload
        .get("manual_adaptive_risk")
        .unwrap_or(&Value::Null);
    let effective = manual.get("effective").unwrap_or(&Value::Null);
    format!(
        "🔴 Manual Adaptive SL Bump {} {}\nrecent SL detected\nmax -> {}\nPTB gap strict mode active\nreentry=OFF\ncooldown={} markets\nSL child={}",
        pair_lock_manual_notify_asset_label(&parent_order.market_slug),
        parent_order.outcome_label,
        pair_lock_manual_notify_cent(effective.get("max_price_cent").and_then(value_as_f64)),
        payload
            .get("cooldown_markets")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_MANUAL_ADAPTIVE_SL_COOLDOWN_MARKETS as u64),
        stop_loss_order.id,
    )
}

async fn maybe_notify_pair_lock_manual_adaptive_sl_bump(
    repo: &PostgresRepository,
    parent_order: &TradeBuilderOrder,
    stop_loss_order: &TradeBuilderOrder,
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
    let Some(manual_payload) = pair_lock_manual_payload_from_flow_payload(&flow_payload) else {
        return Ok(());
    };
    let Some(node) = pair_lock_manual_source_action_node(repo, &run, parent_order, &flow_payload)
        .await?
    else {
        return Ok(());
    };
    if !action_place_order_uses_manual_adaptive_risk_strategy(&node) {
        return Ok(());
    }
    let cfg = resolve_pair_lock_manual_adaptive_notify_config(&node)?;
    let cooldown_markets = node_config_i64(&node, "manualAdaptiveSlCooldownMarkets")
        .filter(|value| *value >= 0)
        .map(|value| value as usize)
        .unwrap_or(DEFAULT_MANUAL_ADAPTIVE_SL_COOLDOWN_MARKETS);
    let mut context = run.context_json.clone();
    mark_pair_lock_manual_adaptive_sl_cooldown(
        &mut context,
        &node.key,
        &parent_order.market_slug,
        &parent_order.outcome_label,
        cooldown_markets,
    );
    repo.update_trade_flow_run_context(run.id, &context).await?;
    let _ = replace_trade_flow_ws_fast_path_run_context(run.id, &context).await;
    let payload = json!({
        "parent_builder_order_id": parent_order.id,
        "sl_builder_order_id": stop_loss_order.id,
        "cooldown_markets": cooldown_markets,
        "manual_adaptive_risk": manual_payload,
    });
    if cfg.notify_sl_bump {
        emit_pair_lock_manual_adaptive_notification(
            repo,
            &run,
            &node,
            &mut context,
            MANUAL_ADAPTIVE_EVENT_SL_BUMP,
            &parent_order.market_slug,
            &parent_order.outcome_label,
            "manual_adaptive_sl_cooldown_started",
            payload.clone(),
            build_pair_lock_manual_adaptive_sl_bump_message(
                parent_order,
                stop_loss_order,
                &payload,
            ),
            true,
        )
        .await?;
    }
    Ok(())
}
