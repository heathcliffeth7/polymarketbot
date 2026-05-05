#[derive(Debug, Clone)]
struct ActionPlaceOrderExitConfig {
    tp_enabled: bool,
    tp_price: Option<f64>,
    sl_enabled: bool,
    sl_price: Option<f64>,
    sl_trigger_price_mode: Option<String>,
    reenter_on_sl_hit: bool,
    reentry_max_attempts: i32,
    reentry_trigger_node_key: Option<String>,
    staged_sl_behavior: TradeBuilderStagedSlBehaviorConfig,
    ptb_stop_loss_gap_usd: Option<f64>,
    ptb_reference_price: Option<f64>,
    ptb_stop_loss_time_decay_mode: Option<String>,
    ptb_current_price_source: Option<String>,
}

fn resolve_action_place_order_exit_config(
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    side: &str,
    kind: &str,
    is_internal_time_exit: bool,
    tp_rules: &[TradeBuilderPriceExitRule],
    sl_rules: &[TradeBuilderPriceExitRule],
    ptb_stop_loss: Option<&ActionPlaceOrderPtbStopLossConfig>,
    ptb_stop_loss_rules: &[bot_infra::db::TradeBuilderPtbStopLossRule],
) -> Result<ActionPlaceOrderExitConfig> {
    let ptb_stop_loss_gap_usd = ptb_stop_loss.and_then(|config| config.hard_gap_usd);
    let ptb_reference_price = ptb_stop_loss.and_then(|config| config.reference_price);
    let ptb_stop_loss_time_decay_mode =
        ptb_stop_loss.and_then(|config| config.time_decay_mode.clone());
    let ptb_current_price_source =
        ptb_stop_loss.map(|config| config.current_price_source.as_config_str().to_string());

    let hard_tp_price = if is_internal_time_exit {
        None
    } else {
        resolve_action_place_order_exit_price(
            node,
            side,
            node_config_bool(node, "tpEnabled").unwrap_or(false),
            "tpPriceCent",
            "tpPrice",
            "tp",
        )?
    };
    let hard_sl_price = if is_internal_time_exit {
        None
    } else {
        resolve_action_place_order_exit_price(
            node,
            side,
            node_config_bool(node, "slEnabled").unwrap_or(false),
            "slPriceCent",
            "slPrice",
            "sl",
        )?
    };

    let tp_enabled = hard_tp_price.is_some() || !tp_rules.is_empty();
    let sl_enabled = hard_sl_price.is_some() || !sl_rules.is_empty();
    let sl_trigger_price_mode = if sl_enabled {
        let raw = node_config_string(node, "slTriggerPriceMode");
        let mode = match raw.as_deref() {
            Some("best_bid") => "best_bid",
            Some("composite") => "composite",
            Some("composite_safe") => "composite_safe",
            Some("composite_fast") => "composite_fast",
            Some("last_trade") => "last_trade",
            Some(other) => {
                anyhow::bail!(
                    "action.place_order slTriggerPriceMode must be best_bid|composite|composite_safe|composite_fast|last_trade, got: {other}"
                )
            }
            None => "best_bid",
        };
        Some(mode.to_string())
    } else {
        None
    };

    let reenter_on_sl_hit = !is_internal_time_exit
        && node_config_bool(node, "reenterOnSlHit").unwrap_or(false)
        && (sl_enabled || ptb_stop_loss.is_some())
        && side == "buy";
    let reentry_max_attempts = if reenter_on_sl_hit {
        node_config_i64(node, "reentryMaxAttempts")
            .unwrap_or(1)
            .clamp(1, 10) as i32
    } else {
        0
    };
    let reentry_trigger_node_key = if reenter_on_sl_hit {
        anyhow::ensure!(
            kind == "immediate",
            "action.place_order reenterOnSlHit is only supported for immediate buy nodes"
        );
        let trigger_key = node_config_string(node, "reentryTriggerNodeKey")
            .filter(|value| !value.trim().is_empty())
            .or_else(|| find_upstream_market_price_trigger_key(&node.key, graph))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "action.place_order reenterOnSlHit requires exactly one upstream trigger.market_price"
                )
            })?;
        let trigger_node = flow_node(graph, &trigger_key).ok_or_else(|| {
            anyhow::anyhow!(
                "action.place_order upstream trigger.market_price missing for re-entry: {trigger_key}"
            )
        })?;
        anyhow::ensure!(
            node_repeat_mode(trigger_node) == "once",
            "action.place_order reenterOnSlHit requires upstream trigger.market_price repeatMode=once"
        );
        Some(trigger_key)
    } else {
        None
    };

    let staged_sl_behavior = resolve_action_place_order_staged_sl_behavior_config(
        node,
        side,
        sl_rules,
        ptb_stop_loss_rules,
        reenter_on_sl_hit,
    );
    if let (Some(tp_price), Some(sl_price)) = (hard_tp_price, hard_sl_price) {
        anyhow::ensure!(
            sl_price < tp_price,
            "action.place_order requires slPrice < tpPrice when both stop loss and take profit are enabled"
        );
    }

    Ok(ActionPlaceOrderExitConfig {
        tp_enabled,
        tp_price: hard_tp_price,
        sl_enabled,
        sl_price: hard_sl_price,
        sl_trigger_price_mode,
        reenter_on_sl_hit,
        reentry_max_attempts,
        reentry_trigger_node_key,
        staged_sl_behavior,
        ptb_stop_loss_gap_usd,
        ptb_reference_price,
        ptb_stop_loss_time_decay_mode,
        ptb_current_price_source,
    })
}
