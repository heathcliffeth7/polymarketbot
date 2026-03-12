fn open_position_ws_price_node_specs(
    node: &TradeFlowNode,
    context: &Value,
) -> Vec<WsOpenPositionPriceNodeSpec> {
    if node.node_type != "trigger.open_positions" && node.node_type != "trigger.market_price" {
        return Vec::new();
    }
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
        .filter(|s| s == "first" || s == "last");
    let cycle_window_secs = node
        .config
        .get("cycleWindowSecs")
        .and_then(value_as_i64_strict)
        .filter(|v| *v > 0);
    let (cycle_window_mode, cycle_window_secs) = match (&cycle_window_mode, cycle_window_secs) {
        (Some(mode), Some(secs)) => (Some(mode.clone()), Some(secs)),
        _ => (None, None),
    };
    let market_slug = if node_market_mode(node) == "auto_scope" {
        flow_context_string(context, "marketSlug")
            .or_else(|| node_config_string(node, "marketSlug"))
    } else {
        node_config_string(node, "marketSlug")
            .or_else(|| flow_context_string(context, "marketSlug"))
    };
    let protection_asset = if protection_mode == TRIGGER_PROTECTION_MODE_UNDERLYING_CONFIRM
        && node_market_mode(node) == "auto_scope"
    {
        resolve_auto_scope_underlying_asset(node, context, market_slug.as_deref())
    } else {
        None
    };
    // Multi-outcome path
    if let Some(conditions) = node
        .config
        .get("outcomeConditions")
        .and_then(|v| v.as_array())
    {
        let mut specs = Vec::new();
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
            if node_market_mode(node) == "auto_scope" && !cond_outcome_label.is_empty() {
                token_id = resolve_token_id_for_outcome_label(cond_outcome_label, context)
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
            if token_id.is_empty()
                || !matches!(trigger_condition.as_str(), "cross_above" | "cross_below")
            {
                continue;
            }
            let tp = match trigger_price {
                Some(v) if v > 0.0 && v <= 1.0 => v,
                _ => continue,
            };
            let max_price = cond
                .get("maxPriceCent")
                .and_then(value_as_f64)
                .map(|v| v / 100.0)
                .or_else(|| cond.get("maxPrice").and_then(value_as_f64))
                .filter(|v| *v > 0.0 && *v <= 1.0);
            specs.push(WsOpenPositionPriceNodeSpec {
                node_key: node.key.clone(),
                node_type: node.node_type.clone(),
                once_mode,
                once_scope_market,
                auto_scope: node_market_mode(node) == "auto_scope",
                price_mode,
                market_slug: market_slug.clone(),
                token_id,
                outcome_label: cond_outcome_label.to_string(),
                trigger_condition,
                trigger_price: tp,
                max_price,
                protection_mode: protection_mode.clone(),
                protection_asset: protection_asset.clone(),
                confirmation_ms,
                cycle_window_mode: cycle_window_mode.clone(),
                cycle_window_secs,
            });
        }
        return specs;
    }
    // Legacy single-token path
    let trigger_condition = match node_config_string(node, "triggerCondition") {
        Some(tc) => tc,
        None => return Vec::new(),
    };
    if !matches!(trigger_condition.as_str(), "cross_above" | "cross_below") {
        return Vec::new();
    }
    let trigger_price = match node_config_f64(node, "triggerPrice")
        .or_else(|| node_config_f64(node, "triggerPriceCent").map(|v| v / 100.0))
    {
        Some(v) if v > 0.0 && v <= 1.0 => v,
        _ => return Vec::new(),
    };
    let token_id = match node_config_string(node, "tokenId")
        .or_else(|| flow_context_string(context, "tokenId"))
        .or_else(|| {
            if node_market_mode(node) != "auto_scope" {
                return None;
            }
            let outcome = node_config_string(node, "outcomeLabel")
                .or_else(|| flow_context_string(context, "outcomeLabel"))?;
            resolve_token_id_for_outcome_label(&outcome, context)
        }) {
        Some(id) if !id.is_empty() => id,
        _ => return Vec::new(),
    };
    let max_price = node_config_f64(node, "maxPrice")
        .or_else(|| node_config_f64(node, "maxPriceCent").map(|v| v / 100.0))
        .filter(|v| *v > 0.0 && *v <= 1.0);
    let outcome_label = node_config_string(node, "outcomeLabel")
        .or_else(|| flow_context_string(context, "outcomeLabel"))
        .unwrap_or_default();
    vec![WsOpenPositionPriceNodeSpec {
        node_key: node.key.clone(),
        node_type: node.node_type.clone(),
        once_mode,
        once_scope_market,
        auto_scope: node_market_mode(node) == "auto_scope",
        price_mode,
        market_slug,
        token_id,
        outcome_label,
        trigger_condition,
        trigger_price,
        max_price,
        protection_mode,
        protection_asset,
        confirmation_ms,
        cycle_window_mode,
        cycle_window_secs,
    }]
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
) -> String {
    if once_mode {
        // once mode must enqueue at most one step for a once scope.
        if once_scope_market {
            let scope_slug = market_slug
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .unwrap_or("unknown-market");
            format!("ws-once:{run_id}:{node_key}:{scope_slug}")
        } else {
            format!("ws-once:{run_id}:{node_key}")
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
