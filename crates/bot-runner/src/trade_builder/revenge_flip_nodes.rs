fn revenge_flip_child_step(step: &TradeFlowRunStep, input: Value) -> TradeFlowRunStep {
    let mut child = step.clone();
    child.input_json = Some(input);
    child
}

#[allow(clippy::too_many_arguments)]
fn revenge_flip_child_node(
    node: &TradeFlowNode,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    revenge_side: &str,
    intent: &str,
    suffix: &str,
    mut config: serde_json::Map<String, Value>,
) -> TradeFlowNode {
    config.insert("mode".to_string(), json!("single"));
    config.insert(REVENGE_FLIP_ORDER_MARKER_KEY.to_string(), json!(true));
    config.insert(REVENGE_FLIP_ROOT_NODE_KEY.to_string(), json!(node.key));
    config.insert(REVENGE_FLIP_SIDE_KEY.to_string(), json!(revenge_side));
    config.insert(REVENGE_FLIP_INTENT_KEY.to_string(), json!(intent));
    config.insert("marketSlug".to_string(), json!(market_slug));
    config.insert("tokenId".to_string(), json!(token_id));
    config.insert("outcomeLabel".to_string(), json!(outcome_label));
    config.insert(
        "refKey".to_string(),
        json!(format!("{}_{}", node.key, suffix)),
    );
    TradeFlowNode {
        key: node.key.clone(),
        node_type: node.node_type.clone(),
        config: Value::Object(config),
    }
}

fn revenge_flip_default_iv_time_rules() -> Value {
    json!([
        {
            "startRemainingSec": 45,
            "endRemainingSec": 30,
            "minEdge": 0.03,
            "minGapStrength": 0.50,
            "maxPriceCent": 92
        },
        {
            "startRemainingSec": 30,
            "endRemainingSec": 15,
            "minEdge": 0.05,
            "minGapStrength": 0.75,
            "maxPriceCent": 92
        },
        {
            "startRemainingSec": 15,
            "endRemainingSec": 8,
            "minEdge": 0.07,
            "minGapStrength": 1.00,
            "maxPriceCent": 92
        }
    ])
}

fn revenge_flip_insert_default(
    config: &mut serde_json::Map<String, Value>,
    key: &str,
    value: Value,
) {
    if !config.contains_key(key) {
        config.insert(key.to_string(), value);
    }
}

fn revenge_flip_apply_entry_quality_defaults(config: &mut serde_json::Map<String, Value>) {
    revenge_flip_insert_default(
        config,
        "priceToBeatIvTimeRules",
        revenge_flip_default_iv_time_rules(),
    );
    revenge_flip_insert_default(config, "priceToBeatIvEntryQualityPolicy", json!(true));
    revenge_flip_insert_default(
        config,
        "priceToBeatIvEntryQualityChainlinkMaxAgeMs",
        json!(2500),
    );
    revenge_flip_insert_default(
        config,
        "priceToBeatIvEntryQualityHighRiskUnderSec",
        json!(30),
    );
    revenge_flip_insert_default(
        config,
        "priceToBeatIvEntryQualityHighRiskAskCent",
        json!(85),
    );
    revenge_flip_insert_default(
        config,
        "priceToBeatIvEntryQualityHighPriceMaxSpreadCent",
        json!(2),
    );
    revenge_flip_insert_default(config, "priceToBeatIvEntryQualityMaxSpreadCent", json!(3));
    revenge_flip_insert_default(
        config,
        "priceToBeatIvEntryQualityNeutralEdgePenalty",
        json!(0.03),
    );
    revenge_flip_insert_default(
        config,
        "priceToBeatIvEntryQualityNeutralGapStrengthPenalty",
        json!(0.25),
    );
    revenge_flip_insert_default(
        config,
        "priceToBeatIvEntryQualityStaleEdgePenalty",
        json!(0.03),
    );
    revenge_flip_insert_default(
        config,
        "priceToBeatIvEntryQualityStaleGapStrengthPenalty",
        json!(0.25),
    );
}

fn revenge_flip_buy_node(
    node: &TradeFlowNode,
    config: &RevengeFlipConfig,
    effective_ptb: &RevengeFlipEffectivePtb,
    effective_max_price_cent: Option<f64>,
    quote: &RevengeFlipSideQuote,
    notional_usdc: f64,
    stop_loss_pct: f64,
    intent: &str,
    sequence: i64,
) -> TradeFlowNode {
    let mut next = node.config.as_object().cloned().unwrap_or_default();
    next.remove("sourceTradeId");
    next.remove("source_trade_id");
    next.insert("side".to_string(), json!("buy"));
    next.insert("kind".to_string(), json!("immediate"));
    next.insert("executionMode".to_string(), json!("market"));
    next.insert("sizeMode".to_string(), json!("usdc"));
    next.insert("sizeUsdc".to_string(), json!(notional_usdc));
    next.insert("targetNotionalUsdc".to_string(), json!(notional_usdc));
    next.insert("orderType".to_string(), json!(config.order_type));
    next.insert("postOnly".to_string(), json!(false));
    next.insert("tpEnabled".to_string(), json!(false));
    next.insert("slEnabled".to_string(), json!(false));
    next.insert(
        REVENGE_FLIP_STOP_LOSS_ENABLED_KEY.to_string(),
        json!(config.classic_stop_loss_enabled),
    );
    next.insert(
        REVENGE_FLIP_STOP_LOSS_PCT_KEY.to_string(),
        json!(stop_loss_pct),
    );
    next.insert("autoSellOnWindowEnd".to_string(), json!(false));
    next.insert("buyFillLockEnabled".to_string(), json!(false));
    next.insert(
        "priceToBeatGuardEnabled".to_string(),
        json!(effective_ptb.enabled),
    );
    next.insert("priceToBeatGuard".to_string(), json!(effective_ptb.enabled));
    let ptb_mode = if intent == "flip_buy" && !config.post_stop_loss_iv_mismatch_enabled {
        "manual"
    } else {
        config.ptb.mode.as_str()
    };
    next.insert("priceToBeatMode".to_string(), json!(ptb_mode));
    next.insert(
        "priceToBeatMaxDiff".to_string(),
        json!(effective_ptb.max_diff),
    );
    next.insert(
        "priceToBeatMaxDiffUnit".to_string(),
        json!(effective_ptb.unit),
    );
    next.insert(
        "priceToBeatCurrentPriceSource".to_string(),
        json!(effective_ptb.current_price_source.as_str()),
    );
    next.insert("cexDirectionGuardEnabled".to_string(), json!(true));
    next.insert("cexDirectionGuardMode".to_string(), json!("bybit_plus_one"));
    next.insert("cexDirectionGuardFailClosed".to_string(), json!(false));
    if ptb_mode == "iv_mismatch_edge" {
        revenge_flip_apply_entry_quality_defaults(&mut next);
    } else {
        next.remove("priceToBeatIvTimeRules");
        next.insert("priceToBeatIvEntryQualityPolicy".to_string(), json!(false));
    }
    next.insert("retryOnPriceToBeatGuardBlock".to_string(), json!(true));
    if let Some(max_price_cent) = effective_max_price_cent {
        next.insert("maxPriceCent".to_string(), json!(max_price_cent));
    } else if let Some(best_ask) = quote.best_ask {
        next.insert(
            "maxPriceCent".to_string(),
            json!((best_ask * 100.0).min(99.0)),
        );
    }
    revenge_flip_child_node(
        node,
        &quote.market_slug,
        &quote.token_id,
        &quote.outcome_label,
        &quote.revenge_side,
        intent,
        &format!(
            "buy_{}_{}_{}",
            quote.market_slug.replace('-', "_"),
            quote.revenge_side,
            sequence
        ),
        next,
    )
}

fn revenge_flip_stop_loss_sell_node(
    node: &TradeFlowNode,
    config: &RevengeFlipConfig,
    quote: &RevengeFlipSideQuote,
    state: &TradeBuilderRevengeFlipState,
    sequence: i64,
) -> Option<TradeFlowNode> {
    let source_trade_id = state.position_source_trade_id?;
    let position_qty = round_trade_builder_share_qty(state.position_qty);
    if position_qty <= 0.0 || revenge_flip_position_is_dust(position_qty) {
        return None;
    }
    let mut next = node.config.as_object().cloned().unwrap_or_default();
    next.remove("sizeUsdc");
    next.remove("targetNotionalUsdc");
    next.remove("sizePct");
    next.remove("sizePercent");
    next.insert("side".to_string(), json!("sell"));
    next.insert("kind".to_string(), json!("immediate"));
    next.insert("executionMode".to_string(), json!("market"));
    next.insert("sizeMode".to_string(), json!("shares"));
    next.insert("targetQty".to_string(), json!(position_qty));
    next.insert("sourceTradeId".to_string(), json!(source_trade_id));
    next.insert(
        "internalMode".to_string(),
        json!("revenge_flip_stop_loss_sell"),
    );
    next.insert("revengeFlipStopLossSell".to_string(), json!(true));
    if let Some(position_builder_order_id) = state.position_builder_order_id {
        next.insert(
            "parentBuilderOrderId".to_string(),
            json!(position_builder_order_id),
        );
    }
    next.insert("orderType".to_string(), json!(config.order_type));
    next.insert("postOnly".to_string(), json!(false));
    next.insert("tpEnabled".to_string(), json!(false));
    next.insert("slEnabled".to_string(), json!(false));
    next.insert("priceToBeatGuardEnabled".to_string(), json!(false));
    next.insert("priceToBeatGuard".to_string(), json!(false));
    next.insert("cexDirectionGuardEnabled".to_string(), json!(false));
    next.insert("priceToBeatIvEntryQualityPolicy".to_string(), json!(false));
    if let Some(best_bid) = quote.best_bid {
        next.insert(
            "minPriceCent".to_string(),
            json!((best_bid * 100.0).max(1.0)),
        );
    }
    Some(revenge_flip_child_node(
        node,
        &quote.market_slug,
        &quote.token_id,
        &quote.outcome_label,
        &quote.revenge_side,
        "stop_loss_sell",
        &format!(
            "sell_{}_{}_{}",
            quote.market_slug.replace('-', "_"),
            quote.revenge_side,
            sequence
        ),
        next,
    ))
}
