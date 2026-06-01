fn positive_quantity_flip_grid_child_step(
    step: &TradeFlowRunStep,
    input: Value,
) -> TradeFlowRunStep {
    let mut child = step.clone();
    child.input_json = Some(input);
    child
}

fn positive_quantity_flip_grid_child_node(
    node: &TradeFlowNode,
    root_node_key: &str,
    grid_side: &str,
    intent: &str,
    unique_suffix: &str,
    mut config: serde_json::Map<String, Value>,
) -> TradeFlowNode {
    config.insert("mode".to_string(), json!("single"));
    config.insert(
        POSITIVE_QUANTITY_FLIP_GRID_ORDER_MARKER_KEY.to_string(),
        json!(true),
    );
    config.insert(
        POSITIVE_QUANTITY_FLIP_GRID_ROOT_NODE_KEY.to_string(),
        json!(root_node_key),
    );
    config.insert(
        POSITIVE_QUANTITY_FLIP_GRID_SIDE_KEY.to_string(),
        json!(grid_side),
    );
    config.insert(
        POSITIVE_QUANTITY_FLIP_GRID_INTENT_KEY.to_string(),
        json!(intent),
    );
    config.insert(
        "refKey".to_string(),
        json!(format!("{root_node_key}_{unique_suffix}")),
    );
    TradeFlowNode {
        key: node.key.clone(),
        node_type: node.node_type.clone(),
        config: Value::Object(config),
    }
}

fn positive_quantity_flip_grid_child_bool(
    config: &serde_json::Map<String, Value>,
    key: &str,
) -> bool {
    match config.get(key) {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        ),
        _ => false,
    }
}

fn positive_quantity_flip_grid_child_non_empty_array(
    config: &serde_json::Map<String, Value>,
    key: &str,
) -> bool {
    matches!(config.get(key), Some(Value::Array(items)) if !items.is_empty())
}

fn positive_quantity_flip_grid_child_non_empty_string(
    config: &serde_json::Map<String, Value>,
    key: &str,
) -> bool {
    matches!(config.get(key), Some(Value::String(value)) if !value.trim().is_empty())
}

fn positive_quantity_flip_grid_child_has_ptb_stop_loss(
    config: &serde_json::Map<String, Value>,
) -> bool {
    positive_quantity_flip_grid_child_bool(config, "ptbStopLossEnabled")
        || positive_quantity_flip_grid_child_non_empty_array(config, "ptbStopLossRules")
        || config.get("ptbStopLossGapUsd").is_some()
}

fn positive_quantity_flip_grid_clear_child_stop_loss_config(
    config: &mut serde_json::Map<String, Value>,
) {
    for key in [
        "slEnabled",
        "slPriceCent",
        "slPrice",
        "slRules",
        "slTriggerPriceMode",
        "ptbStopLossEnabled",
        "ptbStopLossGapUsd",
        "ptbStopLossGapUnit",
        "ptbStopLossRules",
        "ptbStopLossTimeDecayMode",
        "ptbStopLossCurrentPriceSource",
        "priceToBeatCurrentPriceSource",
        "notifyOnSlHit",
        "reenterOnSlHit",
        "reentryMaxAttempts",
        "reentryCooldownSec",
        "reentrySkipCurrentWindow",
        "reentryMinPriceCent",
        "reentryMaxPriceCent",
        "reentryThresholdDecay",
        "reentryMaxPriceTightenBps",
        "reentryPriceToBeatMaxDiff",
        "reentryPriceToBeatMaxDiffUnit",
        "stagedSlReentryOnlyAfterAllStages",
    ] {
        config.remove(key);
    }
}

fn positive_quantity_flip_grid_sync_normal_buy_stop_loss_source(
    config: &mut serde_json::Map<String, Value>,
    grid_config: &PositiveQuantityFlipGridConfig,
) {
    if !positive_quantity_flip_grid_child_has_ptb_stop_loss(config) {
        config.remove("priceToBeatCurrentPriceSource");
        return;
    }
    if positive_quantity_flip_grid_child_non_empty_string(config, "ptbStopLossCurrentPriceSource") {
        return;
    }
    config.insert(
        "priceToBeatCurrentPriceSource".to_string(),
        json!(grid_config.ptb_current_price_source.as_config_str()),
    );
}

fn positive_quantity_flip_grid_buy_node(
    node: &TradeFlowNode,
    config: &PositiveQuantityFlipGridConfig,
    market_slug: &str,
    candidate: &PositiveQuantityFlipGridBuyCandidate,
    sequence: i64,
    intent_override: Option<&str>,
    order_type_override: Option<&str>,
) -> TradeFlowNode {
    let mut next = node.config.as_object().cloned().unwrap_or_default();
    let intent = intent_override.unwrap_or(if candidate.partial_recovery {
        "partial_recovery_buy"
    } else {
        "buy"
    });
    let normal_buy = intent == "buy"
        && intent_override.is_none()
        && !candidate.rescue_buy
        && !candidate.partial_recovery;
    next.insert("side".to_string(), json!("buy"));
    next.insert("kind".to_string(), json!("immediate"));
    next.insert("executionMode".to_string(), json!("limit"));
    next.insert("sizeMode".to_string(), json!("shares"));
    next.insert("sizeUsdc".to_string(), json!(candidate.actual_buy_usdc));
    next.insert("targetQty".to_string(), json!(candidate.target_qty));
    next.insert("marketSlug".to_string(), json!(market_slug));
    next.insert("tokenId".to_string(), json!(candidate.quote.token_id));
    next.insert(
        "outcomeLabel".to_string(),
        json!(candidate.quote.outcome_label),
    );
    next.insert(
        "maxPriceCent".to_string(),
        json!(candidate.worst_price * 100.0),
    );
    next.insert(
        "minPriceDistanceCent".to_string(),
        json!(((candidate.worst_price - candidate.effective_ask_price) * 100.0).max(0.01)),
    );
    next.insert(
        "sizingPriceBufferCent".to_string(),
        json!(config.sizing_price_buffer_cent),
    );
    next.insert(
        "orderType".to_string(),
        json!(order_type_override.unwrap_or(config.order_type)),
    );
    next.insert("postOnly".to_string(), json!(false));
    next.insert("tpEnabled".to_string(), json!(false));
    next.insert("autoSellOnWindowEnd".to_string(), json!(false));
    next.insert("buyFillLockEnabled".to_string(), json!(false));
    next.insert("priceToBeatGuardEnabled".to_string(), json!(false));
    next.insert("priceToBeatGuard".to_string(), json!(false));
    next.insert("triggerPriceGuardEnabled".to_string(), json!(false));
    if normal_buy {
        positive_quantity_flip_grid_sync_normal_buy_stop_loss_source(&mut next, config);
    } else {
        positive_quantity_flip_grid_clear_child_stop_loss_config(&mut next);
        next.insert("slEnabled".to_string(), json!(false));
    }
    let compression_buy = intent == "pairlock_compression_buy";
    next.insert(
        "executionFloorGuardEnabled".to_string(),
        json!(config.execution_floor_guard_enabled && !compression_buy),
    );
    next.insert(
        "executionFloorPriceCent".to_string(),
        json!(positive_quantity_flip_grid_execution_floor_price_cent(
            config
        )),
    );
    next.remove("tpPriceCent");
    next.remove("tpPrice");
    next.remove("tpRules");
    next.remove("notifyOnTpHit");
    next.remove("timeExitRules");
    positive_quantity_flip_grid_child_node(
        node,
        &node.key,
        candidate.quote.grid_side,
        intent,
        &format!(
            "buy_{}_{}_{}",
            market_slug.replace('-', "_"),
            candidate.quote.grid_side,
            sequence
        ),
        next,
    )
}

fn positive_quantity_flip_grid_sell_node(
    node: &TradeFlowNode,
    config: &PositiveQuantityFlipGridConfig,
    position: &TradeBuilderParentPosition,
    sequence: i64,
) -> TradeFlowNode {
    let mut next = node.config.as_object().cloned().unwrap_or_default();
    next.insert("side".to_string(), json!("sell"));
    next.insert("kind".to_string(), json!("immediate"));
    next.insert("executionMode".to_string(), json!("market"));
    next.insert("sizeMode".to_string(), json!("pct"));
    next.insert("sizePct".to_string(), json!(100.0));
    next.insert("marketSlug".to_string(), json!(position.market_slug));
    next.insert("tokenId".to_string(), json!(position.token_id));
    next.insert("outcomeLabel".to_string(), json!(position.outcome_label));
    next.insert("sourceTradeId".to_string(), json!(position.source_trade_id));
    next.insert("orderType".to_string(), json!(config.order_type));
    next.insert("postOnly".to_string(), json!(false));
    next.insert("tpEnabled".to_string(), json!(false));
    next.insert("slEnabled".to_string(), json!(false));
    next.remove("tpRules");
    next.remove("slRules");
    next.remove("timeExitRules");
    let grid_side = if normalize_pair_lock_binary_outcome(&position.outcome_label) == Some("no") {
        "down"
    } else {
        "up"
    };
    positive_quantity_flip_grid_child_node(
        node,
        &node.key,
        grid_side,
        "sell",
        &format!(
            "sell_{}_{}_{}",
            position.market_slug.replace('-', "_"),
            position.parent_builder_order_id,
            sequence
        ),
        next,
    )
}

fn positive_quantity_flip_grid_step_with_price(
    step: &TradeFlowRunStep,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    price: f64,
) -> TradeFlowRunStep {
    positive_quantity_flip_grid_child_step(
        step,
        json!({
            "marketSlug": market_slug,
            "tokenId": token_id,
            "outcomeLabel": outcome_label,
            "triggered_price": price,
            "price": price,
            "wsPrice": price,
        }),
    )
}

fn positive_quantity_flip_grid_sell_step(
    step: &TradeFlowRunStep,
    position: &TradeBuilderParentPosition,
    bid_price: f64,
) -> TradeFlowRunStep {
    positive_quantity_flip_grid_child_step(
        step,
        json!({
            "internalMode": "positive_quantity_flip_grid_sell",
            "parentBuilderOrderId": position.parent_builder_order_id,
            "sourceTradeId": position.source_trade_id,
            "marketSlug": position.market_slug,
            "tokenId": position.token_id,
            "outcomeLabel": position.outcome_label,
            "triggered_price": bid_price,
            "price": bid_price,
            "wsPrice": bid_price,
        }),
    )
}
