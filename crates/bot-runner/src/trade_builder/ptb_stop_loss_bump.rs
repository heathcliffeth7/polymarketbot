#[derive(Debug, Clone, Copy, PartialEq)]
struct ActionPlaceOrderPtbStopLossBumpConfig {
    amount: f64,
    unit: trade_flow::guards::price_to_beat::PriceToBeatDiffUnit,
    max_value: Option<f64>,
    decay_windows: Option<i64>,
    scope_mode: ActionPlaceOrderPtbStopLossBumpScopeMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionPlaceOrderPtbStopLossBumpScopeMode {
    Global,
    PerScope,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct ActionPlaceOrderPtbStopLossBumpState {
    pub(crate) count: i64,
    pub(crate) applied_count: i64,
    pub(crate) accumulated_bump_usd: f64,
    pub(crate) applied_bump_usd: f64,
    pub(crate) last_bump_increment_usd: f64,
    pub(crate) last_market_slug: Option<String>,
    pub(crate) last_child_order_id: Option<i64>,
    pub(crate) current_market_excluded: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct ActionPlaceOrderPtbStopLossBumpStateUpdate {
    applied: bool,
    previous_count: i64,
    next_count: i64,
    previous_accumulated_bump_usd: f64,
    next_accumulated_bump_usd: f64,
    applied_increment_usd: f64,
    previous_market_slug: Option<String>,
    max_notified_for_market: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct ActionPlaceOrderPtbStopLossBumpCurrentPtbSnapshot {
    value: f64,
    unit: String,
    usd: f64,
}

const FLOW_NODE_STATE_PTB_SL_BUMP_MAX_NOTIFIED: &str = "ptb_stop_loss_bump_max_notified";
const FLOW_NODE_STATE_PTB_SL_BUMP_MAX_NOTIFIED_MARKET_SLUG: &str =
    "ptb_stop_loss_bump_max_notified_market_slug";
const FLOW_NODE_STATE_PTB_SL_BUMP_SCOPE_MAP: &str = "ptb_stop_loss_bump_scope_map";

fn resolve_action_place_order_ptb_stop_loss_bump_scope_mode(
    node: &TradeFlowNode,
) -> ActionPlaceOrderPtbStopLossBumpScopeMode {
    match node_config_string(node, "priceToBeatStopLossBumpScope")
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("global") => ActionPlaceOrderPtbStopLossBumpScopeMode::Global,
        _ => ActionPlaceOrderPtbStopLossBumpScopeMode::PerScope,
    }
}

fn resolve_action_place_order_ptb_stop_loss_bump_scope_key(
    market_slug: &str,
    outcome_label: &str,
) -> Option<String> {
    let scope = find_updown_scope_by_slug(market_slug)?;
    let direction = match outcome_label.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "long" | "bull" => "up",
        "no" | "down" | "short" | "bear" => "down",
        _ => return None,
    };
    Some(format!("{}:{}:{direction}", scope.asset, scope.timeframe))
}

fn resolve_action_place_order_ptb_stop_loss_bump_current_ptb_snapshot(
    context: &Value,
    market_slug: &str,
) -> Option<ActionPlaceOrderPtbStopLossBumpCurrentPtbSnapshot> {
    let snapshot = crate::flow_context_value(context, "priceToBeatGuard")?;
    let snapshot_market_slug = snapshot
        .get("market_slug")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if snapshot_market_slug != market_slug {
        return None;
    }

    let threshold_value = snapshot
        .get("threshold_value")
        .and_then(crate::value_as_f64)
        .filter(|value| value.is_finite())?;
    let threshold_unit = snapshot
        .get("threshold_unit")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)?;
    let threshold_usd = snapshot
        .get("threshold_usd")
        .and_then(crate::value_as_f64)
        .filter(|value| value.is_finite())?;

    Some(ActionPlaceOrderPtbStopLossBumpCurrentPtbSnapshot {
        value: threshold_value,
        unit: threshold_unit,
        usd: threshold_usd,
    })
}

fn resolve_action_place_order_ptb_stop_loss_bump_unit(
    node: &TradeFlowNode,
) -> Result<trade_flow::guards::price_to_beat::PriceToBeatDiffUnit> {
    let raw_unit = node_config_string(node, "priceToBeatStopLossBumpUnit")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    if let Some(raw_unit) = raw_unit.as_deref() {
        return trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::parse(Some(raw_unit))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "action.place_order priceToBeatStopLossBumpUnit must be one of: usd, cent"
                )
            });
    }

    let primary_unit = node_config_string(node, "priceToBeatMaxDiffUnit")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| value == "usd" || value == "cent");
    Ok(primary_unit
        .as_deref()
        .and_then(|value| {
            trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::parse(Some(value))
        })
        .unwrap_or(trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Cent))
}

fn resolve_action_place_order_ptb_stop_loss_bump_config(
    node: &TradeFlowNode,
    side: &str,
) -> Result<Option<ActionPlaceOrderPtbStopLossBumpConfig>> {
    let enabled = node_config_bool(node, "priceToBeatStopLossBumpEnabled").unwrap_or(false);
    if !enabled {
        return Ok(None);
    }

    anyhow::ensure!(
        side == "buy",
        "action.place_order priceToBeatStopLossBumpEnabled is only valid for side=buy"
    );
    anyhow::ensure!(
        node_config_bool(node, "priceToBeatGuardEnabled").unwrap_or(false),
        "action.place_order priceToBeatStopLossBumpEnabled requires priceToBeatGuardEnabled=true"
    );
    let amount = node_config_f64(node, "priceToBeatStopLossBumpAmount").unwrap_or(0.0);
    anyhow::ensure!(
        amount.is_finite() && amount > 0.0,
        "action.place_order priceToBeatStopLossBumpAmount must be > 0"
    );
    let max_value = node_config_f64(node, "priceToBeatStopLossBumpMaxValue")
        .filter(|value| value.is_finite() && *value > 0.0);
    if let Some(max_value) = max_value {
        anyhow::ensure!(
            max_value >= amount,
            "action.place_order priceToBeatStopLossBumpMaxValue must be >= priceToBeatStopLossBumpAmount"
        );
    }
    let unit = resolve_action_place_order_ptb_stop_loss_bump_unit(node)?;
    let decay_windows =
        node_config_i64(node, "priceToBeatStopLossBumpDecayWindows").filter(|value| *value > 0);

    Ok(Some(ActionPlaceOrderPtbStopLossBumpConfig {
        amount,
        unit,
        max_value,
        decay_windows,
        scope_mode: resolve_action_place_order_ptb_stop_loss_bump_scope_mode(node),
    }))
}

fn resolve_action_place_order_ptb_stop_loss_bump_scope_entry(
    context: &Value,
    node_key: &str,
    scope_key: &str,
) -> Option<Value> {
    context
        .get("nodeState")
        .and_then(|value| value.get(node_key))
        .and_then(|value| value.get(FLOW_NODE_STATE_PTB_SL_BUMP_SCOPE_MAP))
        .and_then(Value::as_object)
        .and_then(|map| map.get(scope_key))
        .cloned()
}

fn set_action_place_order_ptb_stop_loss_bump_scope_entry(
    context: &mut Value,
    node_key: &str,
    scope_key: &str,
    entry: Value,
) {
    if !context.is_object() {
        *context = json!({});
    }
    let root = context.as_object_mut().expect("context object");
    let node_state_root = root
        .entry("nodeState".to_string())
        .or_insert_with(|| json!({}));
    if !node_state_root.is_object() {
        *node_state_root = json!({});
    }
    let node_state_root = node_state_root.as_object_mut().expect("nodeState object");
    let node_entry = node_state_root
        .entry(node_key.to_string())
        .or_insert_with(|| json!({}));
    if !node_entry.is_object() {
        *node_entry = json!({});
    }
    let node_entry = node_entry.as_object_mut().expect("node entry object");
    let scope_map = node_entry
        .entry(FLOW_NODE_STATE_PTB_SL_BUMP_SCOPE_MAP.to_string())
        .or_insert_with(|| json!({}));
    if !scope_map.is_object() {
        *scope_map = json!({});
    }
    scope_map
        .as_object_mut()
        .expect("scope map object")
        .insert(scope_key.to_string(), entry);
}

fn decayed_ptb_stop_loss_bump_count(
    count: i64,
    last_market_slug: Option<&str>,
    current_market_slug: &str,
    decay_windows: Option<i64>,
) -> i64 {
    let Some(decay_windows) = decay_windows.filter(|value| *value > 0) else {
        return count.max(0);
    };
    let Some(last_market_slug) = last_market_slug else {
        return count.max(0);
    };
    let Some(windows_since_last_sl) =
        ptb_stop_loss_bump_market_windows_since(last_market_slug, current_market_slug)
    else {
        return count.max(0);
    };
    let decay_steps = windows_since_last_sl / decay_windows;
    count.saturating_sub(decay_steps).max(0)
}

fn decayed_ptb_stop_loss_bump_usd(
    accumulated_bump_usd: f64,
    base_bump_usd: f64,
    last_market_slug: Option<&str>,
    current_market_slug: &str,
    decay_windows: Option<i64>,
) -> f64 {
    let Some(decay_windows) = decay_windows.filter(|value| *value > 0) else {
        return accumulated_bump_usd.max(0.0);
    };
    let Some(last_market_slug) = last_market_slug else {
        return accumulated_bump_usd.max(0.0);
    };
    let Some(windows_since_last_sl) =
        ptb_stop_loss_bump_market_windows_since(last_market_slug, current_market_slug)
    else {
        return accumulated_bump_usd.max(0.0);
    };
    let decay_steps = windows_since_last_sl / decay_windows;
    (accumulated_bump_usd - (base_bump_usd * decay_steps as f64)).max(0.0)
}

fn resolve_action_place_order_ptb_stop_loss_bump_base_usd(
    config: &ActionPlaceOrderPtbStopLossBumpConfig,
) -> f64 {
    trade_flow::guards::price_to_beat::normalize_price_to_beat_threshold_usd(
        config.amount,
        config.unit,
    )
}

fn ptb_stop_loss_bump_market_windows_since(
    previous_market_slug: &str,
    current_market_slug: &str,
) -> Option<i64> {
    let previous_scope = find_updown_scope_by_slug(previous_market_slug)?;
    let current_scope = find_updown_scope_by_slug(current_market_slug)?;
    if previous_scope.timeframe != current_scope.timeframe {
        return None;
    }
    let previous_start = MarketCycleId(previous_market_slug.to_string()).start_time()?;
    let current_start = MarketCycleId(current_market_slug.to_string()).start_time()?;
    let window_seconds = updown_scope_window_seconds(previous_scope);
    let delta_seconds = current_start
        .signed_duration_since(previous_start)
        .num_seconds();
    (delta_seconds >= 0).then_some(delta_seconds / window_seconds)
}

pub(crate) fn resolve_action_place_order_ptb_stop_loss_bump_state(
    context: &Value,
    node: &TradeFlowNode,
    node_key: &str,
    market_slug: &str,
    outcome_label: &str,
) -> ActionPlaceOrderPtbStopLossBumpState {
    let config = resolve_action_place_order_ptb_stop_loss_bump_config(node, "buy")
        .ok()
        .flatten();
    let (
        count,
        accumulated_bump_usd,
        last_bump_increment_usd,
        last_market_slug,
        last_child_order_id,
    ) = match config
        .as_ref()
        .map(|config| config.scope_mode)
        .unwrap_or(ActionPlaceOrderPtbStopLossBumpScopeMode::PerScope)
    {
        ActionPlaceOrderPtbStopLossBumpScopeMode::Global => (
            flow_node_state_i64(context, node_key, FLOW_NODE_STATE_PTB_SL_BUMP_COUNT)
                .unwrap_or(0)
                .max(0),
            flow_node_state(context, node_key, "ptb_stop_loss_bump_accumulated_usd")
                .and_then(crate::value_as_f64)
                .unwrap_or(0.0)
                .max(0.0),
            flow_node_state(context, node_key, "ptb_stop_loss_bump_last_increment_usd")
                .and_then(crate::value_as_f64)
                .unwrap_or(0.0)
                .max(0.0),
            flow_node_state_string(
                context,
                node_key,
                FLOW_NODE_STATE_PTB_SL_BUMP_LAST_MARKET_SLUG,
            ),
            flow_node_state_i64(
                context,
                node_key,
                FLOW_NODE_STATE_PTB_SL_BUMP_LAST_CHILD_ORDER_ID,
            ),
        ),
        ActionPlaceOrderPtbStopLossBumpScopeMode::PerScope => {
            let Some(scope_key) =
                resolve_action_place_order_ptb_stop_loss_bump_scope_key(market_slug, outcome_label)
            else {
                return ActionPlaceOrderPtbStopLossBumpState::default();
            };
            let entry = resolve_action_place_order_ptb_stop_loss_bump_scope_entry(
                context, node_key, &scope_key,
            );
            (
                entry
                    .as_ref()
                    .and_then(|value| value.get("count"))
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
                    .max(0),
                entry
                    .as_ref()
                    .and_then(|value| value.get("accumulated_bump_usd"))
                    .and_then(crate::value_as_f64)
                    .unwrap_or(0.0)
                    .max(0.0),
                entry
                    .as_ref()
                    .and_then(|value| value.get("last_increment_usd"))
                    .and_then(crate::value_as_f64)
                    .unwrap_or(0.0)
                    .max(0.0),
                entry
                    .as_ref()
                    .and_then(|value| value.get("last_market_slug"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                entry
                    .as_ref()
                    .and_then(|value| value.get("last_child_order_id"))
                    .and_then(Value::as_i64),
            )
        }
    };
    let base_bump_usd = config
        .as_ref()
        .map(resolve_action_place_order_ptb_stop_loss_bump_base_usd)
        .unwrap_or(0.0);
    let count = decayed_ptb_stop_loss_bump_count(
        count,
        last_market_slug.as_deref(),
        market_slug,
        config.as_ref().and_then(|value| value.decay_windows),
    );
    let accumulated_bump_usd = decayed_ptb_stop_loss_bump_usd(
        accumulated_bump_usd,
        base_bump_usd,
        last_market_slug.as_deref(),
        market_slug,
        config.as_ref().and_then(|value| value.decay_windows),
    );
    let current_market_excluded = last_market_slug.as_deref() == Some(market_slug);
    let applied_count = if current_market_excluded {
        count.saturating_sub(1)
    } else {
        count
    };
    let applied_bump_usd = if current_market_excluded {
        (accumulated_bump_usd - last_bump_increment_usd).max(0.0)
    } else {
        accumulated_bump_usd
    };

    ActionPlaceOrderPtbStopLossBumpState {
        count,
        applied_count,
        accumulated_bump_usd,
        applied_bump_usd,
        last_bump_increment_usd,
        last_market_slug,
        last_child_order_id,
        current_market_excluded,
    }
}

fn apply_action_place_order_ptb_stop_loss_bump_state(
    context: &mut Value,
    node: &TradeFlowNode,
    node_key: &str,
    market_slug: &str,
    outcome_label: &str,
    child_order_id: i64,
    bump_increment_usd: f64,
    updated_at: DateTime<Utc>,
) -> ActionPlaceOrderPtbStopLossBumpStateUpdate {
    let config = resolve_action_place_order_ptb_stop_loss_bump_config(node, "buy")
        .ok()
        .flatten();
    let previous_state = resolve_action_place_order_ptb_stop_loss_bump_state(
        context,
        node,
        node_key,
        market_slug,
        outcome_label,
    );
    if previous_state.last_market_slug.as_deref() == Some(market_slug) {
        return ActionPlaceOrderPtbStopLossBumpStateUpdate {
            applied: false,
            previous_count: previous_state.count,
            next_count: previous_state.count,
            previous_accumulated_bump_usd: previous_state.accumulated_bump_usd,
            next_accumulated_bump_usd: previous_state.accumulated_bump_usd,
            applied_increment_usd: 0.0,
            previous_market_slug: previous_state.last_market_slug,
            max_notified_for_market: flow_node_state_truthy(
                context,
                node_key,
                FLOW_NODE_STATE_PTB_SL_BUMP_MAX_NOTIFIED,
            ) && flow_node_state_string(
                context,
                node_key,
                FLOW_NODE_STATE_PTB_SL_BUMP_MAX_NOTIFIED_MARKET_SLUG,
            )
            .as_deref()
                == Some(market_slug),
        };
    }

    let next_count = previous_state.count.saturating_add(1);
    let next_accumulated_bump_usd =
        (previous_state.accumulated_bump_usd + bump_increment_usd).max(0.0);
    match config
        .as_ref()
        .map(|config| config.scope_mode)
        .unwrap_or(ActionPlaceOrderPtbStopLossBumpScopeMode::PerScope)
    {
        ActionPlaceOrderPtbStopLossBumpScopeMode::Global => {
            set_flow_node_state(
                context,
                node_key,
                FLOW_NODE_STATE_PTB_SL_BUMP_COUNT,
                json!(next_count),
            );
            set_flow_node_state(
                context,
                node_key,
                FLOW_NODE_STATE_PTB_SL_BUMP_LAST_MARKET_SLUG,
                json!(market_slug),
            );
            set_flow_node_state(
                context,
                node_key,
                FLOW_NODE_STATE_PTB_SL_BUMP_LAST_CHILD_ORDER_ID,
                json!(child_order_id),
            );
            set_flow_node_state(
                context,
                node_key,
                "ptb_stop_loss_bump_accumulated_usd",
                json!(next_accumulated_bump_usd),
            );
            set_flow_node_state(
                context,
                node_key,
                "ptb_stop_loss_bump_last_increment_usd",
                json!(bump_increment_usd),
            );
        }
        ActionPlaceOrderPtbStopLossBumpScopeMode::PerScope => {
            if let Some(scope_key) =
                resolve_action_place_order_ptb_stop_loss_bump_scope_key(market_slug, outcome_label)
            {
                set_action_place_order_ptb_stop_loss_bump_scope_entry(
                    context,
                    node_key,
                    &scope_key,
                    json!({
                        "count": next_count,
                        "accumulated_bump_usd": next_accumulated_bump_usd,
                        "last_increment_usd": bump_increment_usd,
                        "last_market_slug": market_slug,
                        "last_child_order_id": child_order_id,
                        "updated_at": updated_at.to_rfc3339(),
                    }),
                );
            }
        }
    }
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PTB_SL_BUMP_UPDATED_AT,
        json!(updated_at.to_rfc3339()),
    );

    ActionPlaceOrderPtbStopLossBumpStateUpdate {
        applied: true,
        previous_count: previous_state.count,
        next_count,
        previous_accumulated_bump_usd: previous_state.accumulated_bump_usd,
        next_accumulated_bump_usd,
        applied_increment_usd: bump_increment_usd,
        previous_market_slug: previous_state.last_market_slug,
        max_notified_for_market: false,
    }
}

fn mark_action_place_order_ptb_stop_loss_bump_max_notified(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
) {
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PTB_SL_BUMP_MAX_NOTIFIED,
        json!(true),
    );
    set_flow_node_state(
        context,
        node_key,
        FLOW_NODE_STATE_PTB_SL_BUMP_MAX_NOTIFIED_MARKET_SLUG,
        json!(market_slug),
    );
}

async fn maybe_record_action_place_order_ptb_stop_loss_bump(
    repo: &PostgresRepository,
    parent_order: &TradeBuilderOrder,
    stop_loss_order: &TradeBuilderOrder,
) -> Result<()> {
    let Some(run_id) = parent_order.origin_flow_run_id else {
        return Ok(());
    };
    let Some(action_node_key) = parent_order
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
    let Some(version) = repo.get_trade_flow_version(flow_run.version_id).await? else {
        return Ok(());
    };
    let graph = parse_trade_flow_graph(&version)?;
    let Some(node) = flow_node(&graph, action_node_key) else {
        return Ok(());
    };
    let Some(config) =
        resolve_action_place_order_ptb_stop_loss_bump_config(node, &parent_order.side)?
    else {
        return Ok(());
    };

    let mut context = flow_run.context_json.clone();
    let updated_at = Utc::now();
    let base_bump_usd = resolve_action_place_order_ptb_stop_loss_bump_base_usd(&config);
    let bump_increment_usd = base_bump_usd;
    let update = apply_action_place_order_ptb_stop_loss_bump_state(
        &mut context,
        node,
        action_node_key,
        &stop_loss_order.market_slug,
        &parent_order.outcome_label,
        stop_loss_order.id,
        bump_increment_usd,
        updated_at,
    );

    if !update.applied {
        repo.append_trade_builder_order_event(
            parent_order.id,
            "ptb_stop_loss_bump_duplicate_market_ignored",
            &json!({
                "sl_child_order_id": stop_loss_order.id,
                "flow_run_id": run_id,
                "node_key": action_node_key,
                "market_slug": &stop_loss_order.market_slug,
                "current_count": update.next_count,
                "config_snapshot": {
                    "enabled": true,
                    "amount": config.amount,
                    "unit": config.unit.as_str(),
                    "base_bump_usd": base_bump_usd,
                    "bump_increment_usd": bump_increment_usd,
                    "decay_windows": config.decay_windows,
                    "scope_mode": match config.scope_mode {
                        ActionPlaceOrderPtbStopLossBumpScopeMode::Global => "global",
                        ActionPlaceOrderPtbStopLossBumpScopeMode::PerScope => "per_scope",
                    },
                },
            }),
        )
        .await?;
        return Ok(());
    }

    repo.update_trade_flow_run_context(run_id, &context).await?;
    let cache_updated = replace_trade_flow_ws_fast_path_run_context(run_id, &context).await;
    repo.append_trade_builder_order_event(
        parent_order.id,
        "ptb_stop_loss_bump_applied",
        &json!({
            "sl_child_order_id": stop_loss_order.id,
            "flow_run_id": run_id,
            "node_key": action_node_key,
            "market_slug": &stop_loss_order.market_slug,
            "previous_market_slug": update.previous_market_slug,
            "previous_count": update.previous_count,
            "next_count": update.next_count,
            "previous_accumulated_bump_usd": update.previous_accumulated_bump_usd,
            "next_accumulated_bump_usd": update.next_accumulated_bump_usd,
            "bump_increment_usd": update.applied_increment_usd,
            "updated_at": updated_at.to_rfc3339(),
            "fast_path_cache_updated": cache_updated,
            "config_snapshot": {
                "enabled": true,
                "amount": config.amount,
                "unit": config.unit.as_str(),
                "base_bump_usd": base_bump_usd,
                "decay_windows": config.decay_windows,
                "scope_mode": match config.scope_mode {
                    ActionPlaceOrderPtbStopLossBumpScopeMode::Global => "global",
                    ActionPlaceOrderPtbStopLossBumpScopeMode::PerScope => "per_scope",
                },
            },
        }),
    )
    .await?;

    let previous_raw_bump_usd = update.previous_accumulated_bump_usd;
    let raw_bump_usd = update.next_accumulated_bump_usd;
    let max_bump_usd = config.max_value.map(|max_value| {
        trade_flow::guards::price_to_beat::normalize_price_to_beat_threshold_usd(
            max_value,
            config.unit,
        )
    });
    let previous_applied_bump_usd = max_bump_usd
        .map(|max_usd| previous_raw_bump_usd.min(max_usd))
        .unwrap_or(previous_raw_bump_usd);
    let applied_bump_usd = max_bump_usd
        .map(|max_usd| raw_bump_usd.min(max_usd))
        .unwrap_or(raw_bump_usd);
    let current_ptb_snapshot = resolve_action_place_order_ptb_stop_loss_bump_current_ptb_snapshot(
        &context,
        &stop_loss_order.market_slug,
    );

    if let Some(max_value) = config.max_value {
        let mut messages = vec![
            trade_flow::guards::price_to_beat::build_price_to_beat_bump_increased_notification_message(
                &stop_loss_order.market_slug,
                config.amount,
                config.unit.as_str(),
                update.next_count,
                previous_applied_bump_usd,
                applied_bump_usd,
                current_ptb_snapshot.as_ref().map(|snapshot| snapshot.value),
                current_ptb_snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.unit.as_str()),
                current_ptb_snapshot.as_ref().map(|snapshot| snapshot.usd),
            ),
        ];
        if raw_bump_usd > applied_bump_usd && !update.max_notified_for_market {
            messages.push(
                trade_flow::guards::price_to_beat::build_price_to_beat_bump_max_reached_notification_message(
                    &stop_loss_order.market_slug,
                    raw_bump_usd,
                    applied_bump_usd,
                    max_value,
                    config.unit.as_str(),
                    current_ptb_snapshot.as_ref().map(|snapshot| snapshot.value),
                    current_ptb_snapshot
                        .as_ref()
                        .map(|snapshot| snapshot.unit.as_str()),
                    current_ptb_snapshot.as_ref().map(|snapshot| snapshot.usd),
                ),
            );
        }
        for message in &messages {
            let _ = trade_flow::guards::price_to_beat::send_price_to_beat_guard_notification(
                repo,
                parent_order.user_id,
                message,
            )
            .await;
        }
        if raw_bump_usd > applied_bump_usd && !update.max_notified_for_market {
            mark_action_place_order_ptb_stop_loss_bump_max_notified(
                &mut context,
                action_node_key,
                &stop_loss_order.market_slug,
            );
            repo.update_trade_flow_run_context(run_id, &context).await?;
            let _ = replace_trade_flow_ws_fast_path_run_context(run_id, &context).await;
        }
    } else {
        let message =
            trade_flow::guards::price_to_beat::build_price_to_beat_bump_increased_notification_message(
                &stop_loss_order.market_slug,
                config.amount,
                config.unit.as_str(),
                update.next_count,
                previous_applied_bump_usd,
                applied_bump_usd,
                current_ptb_snapshot.as_ref().map(|snapshot| snapshot.value),
                current_ptb_snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.unit.as_str()),
                current_ptb_snapshot.as_ref().map(|snapshot| snapshot.usd),
            );
        let _ = trade_flow::guards::price_to_beat::send_price_to_beat_guard_notification(
            repo,
            parent_order.user_id,
            &message,
        )
        .await;
    }

    Ok(())
}
