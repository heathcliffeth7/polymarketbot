fn parse_market_price_ptb_trigger_config(
    node: &TradeFlowNode,
) -> (
    bool,
    crate::trade_flow::guards::price_to_beat::PriceToBeatMode,
    Option<f64>,
    Option<f64>,
    crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit,
) {
    let default_mode = crate::trade_flow::guards::price_to_beat::PriceToBeatMode::Manual;
    let default_unit = crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Usd;
    if node.node_type != "trigger.market_price" || node_market_mode(node) != "auto_scope" {
        return (false, default_mode, None, None, default_unit);
    }

    let enabled = node_config_bool(node, "priceToBeatTriggerEnabled").unwrap_or(false);
    let mode = crate::trade_flow::guards::price_to_beat::PriceToBeatMode::parse(
        node_config_string(node, "priceToBeatMode").as_deref(),
    )
    .unwrap_or(default_mode);
    let unit = crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::parse(
        node_config_string(node, "priceToBeatTriggerUnit").as_deref(),
    )
    .unwrap_or(default_unit);
    let min_gap = node_config_f64(node, "priceToBeatTriggerMinGap")
        .filter(|value| value.is_finite() && *value > 0.0);
    let max_gap = node_config_f64(node, "priceToBeatTriggerMaxGap")
        .filter(|value| value.is_finite() && *value > 0.0)
        .filter(|value| min_gap.map(|min_gap| *value >= min_gap).unwrap_or(false));

    let trigger_enabled = match mode {
        crate::trade_flow::guards::price_to_beat::PriceToBeatMode::Manual => {
            enabled && min_gap.is_some()
        }
        crate::trade_flow::guards::price_to_beat::PriceToBeatMode::AutoLast3AvgExcursion
        | crate::trade_flow::guards::price_to_beat::PriceToBeatMode::AutoVolPct
        | crate::trade_flow::guards::price_to_beat::PriceToBeatMode::SignalFormula
        | crate::trade_flow::guards::price_to_beat::PriceToBeatMode::IvMismatchEdge => {
            enabled
        }
    };

    (trigger_enabled, mode, min_gap, max_gap, unit)
}

fn is_pair_lock_only_market_trigger(node: &TradeFlowNode) -> bool {
    node.node_type == "trigger.market_price"
        && node_config_string(node, "bindingMode")
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("pair_lock_only"))
}

fn is_dca_live_only_market_trigger(node: &TradeFlowNode) -> bool {
    node.node_type == "trigger.market_price"
        && node_config_string(node, "bindingMode")
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("dca_live_only"))
}

fn pair_lock_monitor_outcome_labels(market_slug: Option<&str>) -> (&'static str, &'static str) {
    if market_slug.is_some_and(|slug| slug.contains("-updown-")) {
        ("Up", "Down")
    } else {
        ("Yes", "No")
    }
}

fn resolve_ws_market_price_trigger_fields(
    trigger_condition: String,
    trigger_price: Option<f64>,
    max_price: Option<f64>,
    ptb_trigger_enabled: bool,
    once_mode: bool,
) -> Option<(String, f64, Option<f64>)> {
    let has_condition_input = !trigger_condition.is_empty();
    let has_trigger_price_input = trigger_price.is_some();

    if !has_condition_input && !has_trigger_price_input {
        return ptb_trigger_enabled.then_some((String::new(), 0.0, None));
    }
    if !is_supported_market_price_trigger_condition(&trigger_condition) {
        return None;
    }
    if market_price_trigger_condition_requires_once(&trigger_condition) && !once_mode {
        return None;
    }
    let trigger_price = match trigger_price {
        Some(value) if value > 0.0 && value <= 1.0 => value,
        _ => return None,
    };
    Some((trigger_condition, trigger_price, max_price))
}

const WS_NODE_SPEC_SKIP_REASON_MISSING_TOKEN_ID: &str = "missing_token_id";
const WS_NODE_SPEC_SKIP_REASON_INVALID_TRIGGER_FIELDS: &str = "invalid_trigger_fields";
const WS_NODE_SPEC_SKIP_REASON_EMPTY_OUTCOME_CONDITIONS_RESULT: &str =
    "empty_outcome_conditions_result";

#[derive(Debug, Clone, PartialEq, Eq)]
struct WsNodeSpecBuildSkipReason {
    reason: &'static str,
    outcome_label: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct WsNodeSpecBuildResult {
    specs: Vec<WsOpenPositionPriceNodeSpec>,
    skip_reasons: Vec<WsNodeSpecBuildSkipReason>,
}

fn push_ws_node_spec_skip_reason(
    skip_reasons: &mut Vec<WsNodeSpecBuildSkipReason>,
    reason: &'static str,
    outcome_label: Option<&str>,
) {
    skip_reasons.push(WsNodeSpecBuildSkipReason {
        reason,
        outcome_label: outcome_label
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    });
}

fn build_open_position_ws_price_node_specs(
    node: &TradeFlowNode,
    context: &Value,
) -> WsNodeSpecBuildResult {
    let mut result = WsNodeSpecBuildResult::default();
    if node.node_type != "trigger.open_positions" && node.node_type != "trigger.market_price" {
        return result;
    }
    let is_auto_scope = node_market_mode(node) == "auto_scope";
    let once_mode = is_trade_flow_market_price_once_node(node);
    let once_scope_market = is_trade_flow_market_price_once_scope_market(node);
    let price_mode = if node.node_type == "trigger.market_price" {
        WsPriceMode::parse(
            node.config.get("priceMode").and_then(|v| v.as_str()),
            WsPriceMode::Composite,
        )
    } else {
        WsPriceMode::Raw
    };
    let confirmation_ms = node
        .config
        .get("confirmationMs")
        .and_then(value_as_i64_strict)
        .filter(|value| *value >= 0);
    let protection_mode = normalize_trigger_protection_mode(
        node.config.get("protectionMode").and_then(Value::as_str),
    )
    .to_string();
    let cycle_window_mode = node
        .config
        .get("cycleWindowMode")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| s == "first" || s == "last" || s == "custom_range");
    let cycle_window_secs_raw = node
        .config
        .get("cycleWindowSecs")
        .and_then(value_as_i64_strict)
        .filter(|v| *v > 0);
    let cycle_window_start_sec_raw = node
        .config
        .get("cycleWindowStartSec")
        .and_then(value_as_i64_strict)
        .filter(|v| *v >= 0);
    let cycle_window_end_sec_raw = node
        .config
        .get("cycleWindowEndSec")
        .and_then(value_as_i64_strict)
        .filter(|v| *v > 0);
    let (cycle_window_mode, cycle_window_secs, cycle_window_start_sec, cycle_window_end_sec) =
        match &cycle_window_mode {
            Some(mode) if mode == "custom_range" => {
                match (cycle_window_start_sec_raw, cycle_window_end_sec_raw) {
                    (Some(start), Some(end)) if start < end => {
                        (Some(mode.clone()), None, Some(start), Some(end))
                    }
                    _ => (None, None, None, None),
                }
            }
            Some(mode) => match cycle_window_secs_raw {
                Some(secs) => (Some(mode.clone()), Some(secs), None, None),
                None => (None, None, None, None),
            },
            None => (None, None, None, None),
        };
    let auto_sell_on_window_end = cycle_window_mode.as_deref() == Some("custom_range")
        && node
            .config
            .get("autoSellOnWindowEnd")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    let (
        price_to_beat_trigger_enabled,
        price_to_beat_mode,
        price_to_beat_trigger_min_gap,
        price_to_beat_trigger_max_gap,
        price_to_beat_trigger_unit,
    ) = parse_market_price_ptb_trigger_config(node);
    let market_slug = if is_auto_scope {
        node_auto_scope_market_slug(context, &node.key)
            .or_else(|| flow_context_string(context, "marketSlug"))
            .or_else(|| node_config_string(node, "marketSlug"))
    } else {
        node_config_string(node, "marketSlug")
            .or_else(|| flow_context_string(context, "marketSlug"))
    };
    let protection_asset =
        if protection_mode == TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM && is_auto_scope {
            resolve_auto_scope_underlying_asset(node, context, market_slug.as_deref())
        } else {
            None
        };
    if (is_pair_lock_only_market_trigger(node) || is_dca_live_only_market_trigger(node))
        && is_auto_scope
    {
        let (yes_label, no_label) = pair_lock_monitor_outcome_labels(market_slug.as_deref());
        let candidates = [
            (
                node_auto_scope_yes_token_id(context, &node.key)
                    .or_else(|| flow_context_string(context, "yesTokenId")),
                yes_label,
            ),
            (
                node_auto_scope_no_token_id(context, &node.key)
                    .or_else(|| flow_context_string(context, "noTokenId")),
                no_label,
            ),
        ];
        for (token_id, outcome_label) in candidates {
            let Some(token_id) = token_id.filter(|value| !value.trim().is_empty()) else {
                push_ws_node_spec_skip_reason(
                    &mut result.skip_reasons,
                    WS_NODE_SPEC_SKIP_REASON_MISSING_TOKEN_ID,
                    Some(outcome_label),
                );
                continue;
            };
            result.specs.push(WsOpenPositionPriceNodeSpec {
                node_key: node.key.clone(),
                node_type: node.node_type.clone(),
                once_mode,
                once_scope_market,
                pair_lock_only_monitor: true,
                auto_scope: is_auto_scope,
                price_mode,
                market_slug: market_slug.clone(),
                token_id,
                outcome_label: outcome_label.to_string(),
                trigger_condition: String::new(),
                trigger_price: 0.0,
                max_price: None,
                price_to_beat_trigger_enabled: false,
                price_to_beat_mode,
                price_to_beat_trigger_min_gap,
                price_to_beat_trigger_max_gap,
                price_to_beat_trigger_unit,
                protection_mode: protection_mode.clone(),
                protection_asset: protection_asset.clone(),
                confirmation_ms,
                cycle_window_mode: cycle_window_mode.clone(),
                cycle_window_secs,
                cycle_window_start_sec,
                cycle_window_end_sec,
                auto_sell_on_window_end,
            });
        }
        return result;
    }
    // Multi-outcome path
    if let Some(conditions) = node
        .config
        .get("outcomeConditions")
        .and_then(|v| v.as_array())
    {
        for cond in conditions {
            let mut token_id = cond
                .get("tokenId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let cond_outcome_label = cond
                .get("outcomeLabel")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if is_auto_scope && !cond_outcome_label.is_empty() {
                token_id = resolve_token_id_for_outcome_label_for_node(
                    &node.key,
                    cond_outcome_label,
                    context,
                )
                .or_else(|| resolve_token_id_for_outcome_label(cond_outcome_label, context))
                .unwrap_or(token_id);
            }
            let trigger_condition = cond
                .get("triggerCondition")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let trigger_price = cond
                .get("triggerPriceCent")
                .and_then(value_as_f64)
                .map(|v| v / 100.0)
                .or_else(|| cond.get("triggerPrice").and_then(value_as_f64));
            if token_id.is_empty() {
                push_ws_node_spec_skip_reason(
                    &mut result.skip_reasons,
                    WS_NODE_SPEC_SKIP_REASON_MISSING_TOKEN_ID,
                    Some(cond_outcome_label),
                );
                continue;
            }
            let max_price = cond
                .get("maxPriceCent")
                .and_then(value_as_f64)
                .map(|v| v / 100.0)
                .or_else(|| cond.get("maxPrice").and_then(value_as_f64))
                .filter(|v| *v > 0.0 && *v <= 1.0);
            let Some((trigger_condition, trigger_price, max_price)) =
                resolve_ws_market_price_trigger_fields(
                    trigger_condition,
                    trigger_price,
                    max_price,
                    price_to_beat_trigger_enabled,
                    once_mode,
                )
            else {
                push_ws_node_spec_skip_reason(
                    &mut result.skip_reasons,
                    WS_NODE_SPEC_SKIP_REASON_INVALID_TRIGGER_FIELDS,
                    Some(cond_outcome_label),
                );
                continue;
            };
            result.specs.push(WsOpenPositionPriceNodeSpec {
                node_key: node.key.clone(),
                node_type: node.node_type.clone(),
                once_mode,
                once_scope_market,
                pair_lock_only_monitor: false,
                auto_scope: is_auto_scope,
                price_mode,
                market_slug: market_slug.clone(),
                token_id,
                outcome_label: cond_outcome_label.to_string(),
                trigger_condition,
                trigger_price,
                max_price,
                price_to_beat_trigger_enabled,
                price_to_beat_mode,
                price_to_beat_trigger_min_gap,
                price_to_beat_trigger_max_gap,
                price_to_beat_trigger_unit,
                protection_mode: protection_mode.clone(),
                protection_asset: protection_asset.clone(),
                confirmation_ms,
                cycle_window_mode: cycle_window_mode.clone(),
                cycle_window_secs,
                cycle_window_start_sec,
                cycle_window_end_sec,
                auto_sell_on_window_end,
            });
        }
        if result.specs.is_empty() && result.skip_reasons.is_empty() && !conditions.is_empty() {
            push_ws_node_spec_skip_reason(
                &mut result.skip_reasons,
                WS_NODE_SPEC_SKIP_REASON_EMPTY_OUTCOME_CONDITIONS_RESULT,
                None,
            );
        }
        return result;
    }
    // Legacy single-token path
    let trigger_condition = node_config_string(node, "triggerCondition").unwrap_or_default();
    let trigger_price = node_config_f64(node, "triggerPrice")
        .or_else(|| node_config_f64(node, "triggerPriceCent").map(|v| v / 100.0));
    let token_id = match node_config_string(node, "tokenId")
        .or_else(|| {
            if is_auto_scope {
                node_auto_scope_resolved_token_id(context, &node.key)
            } else {
                None
            }
        })
        .or_else(|| flow_context_string(context, "tokenId"))
        .or_else(|| {
            if !is_auto_scope {
                return None;
            }
            let outcome = node_config_string(node, "outcomeLabel")
                .or_else(|| node_auto_scope_resolved_outcome_label(context, &node.key))
                .or_else(|| flow_context_string(context, "outcomeLabel"))?;
            resolve_token_id_for_outcome_label_for_node(&node.key, &outcome, context)
                .or_else(|| resolve_token_id_for_outcome_label(&outcome, context))
        }) {
        Some(id) if !id.is_empty() => id,
        _ => {
            push_ws_node_spec_skip_reason(
                &mut result.skip_reasons,
                WS_NODE_SPEC_SKIP_REASON_MISSING_TOKEN_ID,
                node_config_string(node, "outcomeLabel")
                    .or_else(|| {
                        if is_auto_scope {
                            node_auto_scope_resolved_outcome_label(context, &node.key)
                        } else {
                            None
                        }
                    })
                    .or_else(|| flow_context_string(context, "outcomeLabel"))
                    .as_deref(),
            );
            return result;
        }
    };
    let max_price = node_config_f64(node, "maxPrice")
        .or_else(|| node_config_f64(node, "maxPriceCent").map(|v| v / 100.0))
        .filter(|v| *v > 0.0 && *v <= 1.0);
    let Some((trigger_condition, trigger_price, max_price)) =
        resolve_ws_market_price_trigger_fields(
            trigger_condition,
            trigger_price,
            max_price,
            price_to_beat_trigger_enabled,
            once_mode,
        )
    else {
        push_ws_node_spec_skip_reason(
            &mut result.skip_reasons,
            WS_NODE_SPEC_SKIP_REASON_INVALID_TRIGGER_FIELDS,
            node_config_string(node, "outcomeLabel")
                .or_else(|| {
                    if is_auto_scope {
                        node_auto_scope_resolved_outcome_label(context, &node.key)
                    } else {
                        None
                    }
                })
                .or_else(|| flow_context_string(context, "outcomeLabel"))
                .as_deref(),
        );
        return result;
    };
    let outcome_label = node_config_string(node, "outcomeLabel")
        .or_else(|| {
            if is_auto_scope {
                node_auto_scope_resolved_outcome_label(context, &node.key)
            } else {
                None
            }
        })
        .or_else(|| flow_context_string(context, "outcomeLabel"))
        .unwrap_or_default();
    result.specs.push(WsOpenPositionPriceNodeSpec {
        node_key: node.key.clone(),
        node_type: node.node_type.clone(),
        once_mode,
        once_scope_market,
        pair_lock_only_monitor: false,
        auto_scope: is_auto_scope,
        price_mode,
        market_slug,
        token_id,
        outcome_label,
        trigger_condition,
        trigger_price,
        max_price,
        price_to_beat_trigger_enabled,
        price_to_beat_mode,
        price_to_beat_trigger_min_gap,
        price_to_beat_trigger_max_gap,
        price_to_beat_trigger_unit,
        protection_mode,
        protection_asset,
        confirmation_ms,
        cycle_window_mode,
        cycle_window_secs,
        cycle_window_start_sec,
        cycle_window_end_sec,
        auto_sell_on_window_end,
    });
    result
}

#[cfg(test)]
#[cfg_attr(not(test), allow(dead_code))]
fn open_position_ws_price_node_specs(
    node: &TradeFlowNode,
    context: &Value,
) -> Vec<WsOpenPositionPriceNodeSpec> {
    build_open_position_ws_price_node_specs(node, context).specs
}

fn ws_price_trigger_step_idempotency_key(
    run_id: i64,
    node_key: &str,
    trigger_condition: &str,
    current_price: f64,
    event_ts: Option<i64>,
    once_mode: bool,
    once_scope_market: bool,
    market_slug: Option<&str>,
    generation: i64,
) -> String {
    if once_mode {
        // once mode must enqueue at most one step for a once scope.
        let base = if once_scope_market {
            let scope_slug = market_slug
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .unwrap_or("unknown-market");
            format!("ws-once:{run_id}:{node_key}:{scope_slug}")
        } else {
            format!("ws-once:{run_id}:{node_key}")
        };
        if generation > 0 {
            format!("{base}:gen{generation}")
        } else {
            base
        }
    } else {
        let dedupe_ts = event_ts.unwrap_or_else(|| Utc::now().timestamp_millis());
        format!(
            "ws-open-price:{run_id}:{node_key}:{trigger_condition}:{current_price:.6}:{dedupe_ts}"
        )
    }
}

#[allow(dead_code)]
async fn enqueue_trade_flow_ws_open_position_price_steps(
    repo: &PostgresRepository,
    run_id: i64,
    ws: &ClobWsClient,
    client: Option<&dyn OrderExecutor>,
    user_cfg_cache: &mut HashMap<i64, AppConfig>,
) -> Result<()> {
    let definitions = repo
        .list_published_trade_flow_definitions(FLOW_DEFINITION_PROCESS_LIMIT)
        .await?;
    refresh_trade_flow_ws_fast_path_cache(repo, run_id, ws, &definitions, user_cfg_cache).await?;
    let _ =
        enqueue_trade_flow_ws_open_position_price_steps_from_cache(repo, run_id, ws, client, None)
            .await?;
    Ok(())
}
