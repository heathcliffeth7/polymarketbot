fn extract_builder_order_id(execution: &TradeFlowNodeExecution) -> Result<i64> {
    execution
        .output
        .get("builder_order_id")
        .and_then(Value::as_i64)
        .ok_or_else(|| anyhow::anyhow!("pair_lock child order creation did not return builder_order_id"))
}

fn extract_source_trade_id(execution: &TradeFlowNodeExecution) -> Option<i64> {
    execution
        .output
        .get("source_trade_id")
        .and_then(Value::as_i64)
}

fn strip_action_place_order_pair_fields(config: &mut serde_json::Map<String, Value>) {
    for key in [
        "mode", "pairLockStrategy", "pairMaxTotalCent", "pairTargetTotalCent", "pairSizingMode", "pairTotalBudgetUsdc",
        "pairOrphanGraceMs", "pairProtectiveUnwindEnabled", "pairIgnoreStopLossAfterLocked", "notifyOnPairLocked", "notifyOnPairUnwind", "counterLegEnabled",
        "counterLegOutcomeLabel", "counterLegTriggerCondition", "counterLegTriggerPriceCent",
        "counterLegMaxPriceCent", "counterLegPriceToBeatGuardEnabled", "counterLegPriceToBeatMode",
        "counterLegPriceToBeatCurrentPriceSource", "counterLegPriceToBeatMaxDiff",
        "counterLegPriceToBeatMaxDiffUnit",
        "counterLegExecutionFloorGuardEnabled", "counterLegExecutionFloorPriceCent",
        "counterLegRetryOnPriceToBeatGuardBlock", "counterLegRetryOnExecutionFloorGuardBlock",
        "counterLegRetryOnMaxPriceBlock", "counterLegSizeUsdc", "counterLegTpEnabled",
        "counterLegTpPriceCent", "counterLegTpRules", "counterLegNotifyOnTpHit",
        "counterLegSlEnabled",
        "counterLegSlPriceCent", "counterLegSlTriggerPriceMode", "counterLegPtbStopLossEnabled",
        "counterLegPtbStopLossGapUsd", "counterLegPtbStopLossGapUnit",
        "counterLegPtbStopLossCurrentPriceSource", "counterLegPtbStopLossTimeDecayMode",
        "counterLegNotifyOnSlHit", "tpEnabled", "tpPrice", "tpPriceCent", "tpRules",
        "notifyOnTpHit", "slPrice", "slRules", "ptbStopLossRules", "timeExitRules",
        "reentryTriggerNodeKey",
        "reentryMinPriceCent", "reentryMaxPriceCent", "reentryPriceToBeatMaxDiff",
        "reentryPriceToBeatMaxDiffUnit", "reentrySkipCurrentWindow", "reentryThresholdDecay",
        "reentryMaxPriceTightenBps", "stagedSlReentryOnlyAfterAllStages",
        "biasedHedge", "biasedHedgeStop", "biasedHedgeMaxPairedEffectiveCostCent",
        "adaptiveMaxPriceMissCount", "adaptiveMaxPriceRequiredGoodMissCount",
        "adaptiveMaxPriceRelaxCreditCent", "adaptiveMaxPriceMaxRelaxCreditCent",
        "adaptiveMaxPriceHardCapCent", "adaptiveMaxPriceExtraBufferCent",
        "adaptiveMaxPricePairBufferCent", "adaptiveMaxPriceSizeMultiplier",
        "adaptiveMaxPriceWindowStartSec", "adaptiveMaxPriceWindowEndSec",
        "adaptiveMaxPriceLateRelaxCutoffS", "adaptiveMaxPriceLateRiskEnabled",
        "adaptiveMaxPriceLateRiskAfterSec", "adaptiveMaxPriceLateExtraBufferCent",
        "adaptiveMaxPriceLateSizeMultiplier", "adaptiveMaxPriceSlCooldownMarkets",
        "notifyOnAdaptiveMaxPriceEvaluated", "notifyOnAdaptiveMaxPriceRelax",
        "notifyOnAdaptiveMaxPriceRelaxSl", "notifyOnAdaptiveMaxPriceNoRelaxImportant",
        "notifyOnAdaptiveMaxPriceMissResolved", "notifyOnAdaptiveMaxPriceCooldown",
        "notifyOnAdaptiveMaxPriceSummary", "notifyOnAdaptiveMaxPriceAllNoRelax",
        "adaptiveMaxPriceNotifyMinIntervalSec", "adaptiveMaxPriceNotifyIncludePayload",
        "adaptiveMaxPriceSummaryEveryMarkets",
        "manualAdaptiveWindowStartSec", "manualAdaptiveWindowEndSec",
        "manualAdaptiveVolumeNormalLt", "manualAdaptiveVolumeElevatedLt",
        "manualAdaptiveVolumeHighLt", "manualAdaptiveTrendDeltaUsd",
        "manualAdaptiveNormalFlatMaxPriceSubCent",
        "manualAdaptiveNormalFlatSizeMultiplier",
        "manualAdaptiveNormalFlatPtbGapAddCent",
        "manualAdaptiveNormalCollapsingMaxPriceCent",
        "manualAdaptiveNormalCollapsingSizeMultiplier",
        "manualAdaptiveNormalCollapsingPtbGapAddCent",
        "manualAdaptiveElevatedMaxPriceCent", "manualAdaptiveElevatedSizeMultiplier",
        "manualAdaptiveElevatedPtbGapAddCent", "manualAdaptiveHighMaxPriceCent",
        "manualAdaptiveHighSizeMultiplier", "manualAdaptiveHighPtbGapAddCent",
        "manualAdaptiveAfterSlMaxPriceSubCent", "manualAdaptiveAfterSlPtbGapAddCent",
        "manualAdaptiveSlCooldownMarkets", "manualAdaptivePairBufferCent",
        "manualAdaptiveSelfTuneEnabled", "manualAdaptiveMissRelaxEnabled",
        "manualAdaptiveMissRelaxAfterNoOrderMarkets", "manualAdaptiveTrendDeltaUsdByScope",
        "manualAdaptivePtbRelaxStepCent", "manualAdaptivePtbRelaxMaxCent",
        "manualAdaptiveMaxPriceRelaxStepCent", "manualAdaptiveMaxPriceRelaxMaxCent",
        "manualAdaptiveMaxPriceRelaxHardCapCent", "manualAdaptiveMissRelaxSizeMultiplier",
        "manualAdaptiveSlTightenEnabled", "manualAdaptivePtbSlBumpStepCent",
        "manualAdaptivePtbSlBumpMaxCent", "manualAdaptiveMaxPriceSlPenaltyStepCent",
        "manualAdaptiveMaxPriceSlPenaltyMaxCent", "manualAdaptiveSlDisableReentry",
        "manualAdaptiveConsecutiveSlLockdownAfter",
        "manualAdaptiveLockdownReleaseCleanMarkets", "manualAdaptiveLockdownMaxMarkets",
        "manualAdaptiveCleanMarketDecayEnabled", "manualAdaptivePtbRelaxDecayPerMarketCent",
        "manualAdaptivePtbSlBumpDecayPerCleanMarketCent",
        "manualAdaptiveMaxPriceRelaxDecayPerMarketCent",
        "manualAdaptiveMaxPriceSlPenaltyDecayPerCleanMarketCent",
        "notifyOnManualAdaptiveRiskBlock", "notifyOnManualAdaptiveRiskStrict",
        "notifyOnManualAdaptiveRiskSlBump", "notifyOnManualAdaptiveRiskSummary",
        "notifyOnManualAdaptiveCounterCap", "manualAdaptiveCounterCapNotifyMinDeltaCent",
        "manualAdaptiveNotifySummaryEveryMarkets",
        "manualAdaptiveNotifyMinIntervalSec", "manualAdaptiveNotifyIncludePayload",
    ] {
        config.remove(key);
    }
}

fn copy_pair_lock_primary_reentry_fields(
    node: &TradeFlowNode,
    config: &mut serde_json::Map<String, Value>,
) {
    for key in [
        "reentryMinPriceCent",
        "reentryMaxPriceCent",
        "reentryPriceToBeatMaxDiff",
        "reentryPriceToBeatMaxDiffUnit",
        "reentrySkipCurrentWindow",
        "reentryThresholdDecay",
        "reentryMaxPriceTightenBps",
    ] {
        if let Some(value) = node.config.get(key) {
            config.insert(key.to_string(), value.clone());
        }
    }
}

fn pair_lock_child_price_cent(price: f64) -> f64 {
    ((price * 100.0) * 1000.0).round() / 1000.0
}

fn normalize_child_ptb_stop_loss_current_price_source(config: &mut serde_json::Map<String, Value>) {
    let ptb_stop_loss_active =
        config
            .get("ptbStopLossEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || config
                .get("ptbStopLossRules")
                .and_then(Value::as_array)
                .is_some_and(|rules| !rules.is_empty());
    if !ptb_stop_loss_active {
        config.remove("ptbStopLossCurrentPriceSource");
        return;
    }

    let valid_source = |key: &str| {
        config
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .filter(|value| matches!(value.as_str(), "chainlink" | "binance" | "coinbase"))
    };
    let source = valid_source("ptbStopLossCurrentPriceSource")
        .or_else(|| valid_source("priceToBeatCurrentPriceSource"))
        .unwrap_or_else(|| "chainlink".to_string());
    config.insert("ptbStopLossCurrentPriceSource".to_string(), json!(source));
}

fn build_pair_lock_single_leg_node(
    node: &TradeFlowNode,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    trigger_node_key: &str,
    adaptive_max_price_override: Option<&PairLockAdaptiveMaxPriceOverride>,
    manual_adaptive_risk_override: Option<&PairLockManualAdaptiveRiskOverride>,
) -> TradeFlowNode {
    let mut config = node
        .config
        .as_object()
        .cloned()
        .unwrap_or_default();
    strip_action_place_order_pair_fields(&mut config);
    config.insert("mode".to_string(), json!(ACTION_PLACE_ORDER_MODE_SINGLE));
    config.remove("sourceTradeId");
    config.insert("marketSlug".to_string(), json!(market_slug));
    config.insert("tokenId".to_string(), json!(token_id));
    config.insert("outcomeLabel".to_string(), json!(outcome_label));
    config.insert("reentryTriggerNodeKey".to_string(), json!(trigger_node_key));
    copy_pair_lock_primary_take_profit_fields(node, &mut config);
    copy_pair_lock_primary_reentry_fields(node, &mut config);
    if let Some(value) = node.config.get("slRules") {
        config.insert("slRules".to_string(), value.clone());
    }
    if let Some(value) = node.config.get("ptbStopLossRules") {
        config.insert("ptbStopLossRules".to_string(), value.clone());
    }
    if let Some(adaptive) = adaptive_max_price_override {
        config.insert(
            "maxPriceCent".to_string(),
            json!(pair_lock_child_price_cent(adaptive.effective_max_price)),
        );
        config.insert("sizeMode".to_string(), json!("usdc"));
        config.insert("sizeUsdc".to_string(), json!(adaptive.effective_size_usdc));
        config.insert("adaptiveMaxPriceApplied".to_string(), json!(true));
        config.insert(
            "adaptiveMaxPrice".to_string(),
            adaptive.diagnostics.clone(),
        );
    }
    if let Some(manual) = manual_adaptive_risk_override {
        config.insert(
            "maxPriceCent".to_string(),
            json!(pair_lock_child_price_cent(manual.effective_max_price)),
        );
        config.insert("sizeMode".to_string(), json!("usdc"));
        config.insert("sizeUsdc".to_string(), json!(manual.effective_size_usdc));
        config.insert("reenterOnSlHit".to_string(), json!(false));
        config.insert("manualAdaptiveRiskApplied".to_string(), json!(true));
        config.insert(
            "manualAdaptiveRisk".to_string(),
            manual.diagnostics.clone(),
        );
    }
    normalize_child_ptb_stop_loss_current_price_source(&mut config);

    TradeFlowNode {
        key: node.key.clone(),
        node_type: node.node_type.clone(),
        config: Value::Object(config),
    }
}

fn build_pair_lock_counter_leg_node(
    node: &TradeFlowNode,
    market_slug: &str,
    counter: &ActionPlaceOrderPairResolvedCounterLeg,
    pair_lock: &ActionPlaceOrderPairLockConfig,
    trigger_node_key: &str,
    manual_adaptive_risk_override: Option<&PairLockManualAdaptiveRiskOverride>,
) -> TradeFlowNode {
    let mut config = node
        .config
        .as_object()
        .cloned()
        .unwrap_or_default();
    strip_action_place_order_pair_fields(&mut config);
    config.insert("mode".to_string(), json!(ACTION_PLACE_ORDER_MODE_SINGLE));
    config.insert(
        "refKey".to_string(),
        json!(format!("{}__counter", action_place_order_pair_lock_ref_key(node))),
    );
    config.remove("sourceTradeId");
    config.insert("marketSlug".to_string(), json!(market_slug));
    config.insert("tokenId".to_string(), json!(&counter.token_id));
    config.insert("outcomeLabel".to_string(), json!(&counter.outcome_label));
    config.insert("reentryTriggerNodeKey".to_string(), json!(trigger_node_key));
    config.insert(
        ACTION_PLACE_ORDER_INTERNAL_PAIR_LOCK_CHILD_ROLE_KEY.to_string(),
        json!(TRADE_BUILDER_PAIR_ROLE_COUNTER_CANDIDATE),
    );
    config.insert(
        ACTION_PLACE_ORDER_INTERNAL_INITIAL_STATUS_KEY.to_string(),
        json!(ACTION_PLACE_ORDER_INTERNAL_BLOCKED_STATUS),
    );
    copy_pair_lock_counter_take_profit_fields(node, &mut config);

    if let Some(counter_size) = pair_lock.counter_leg_size_usdc {
        if counter_size > 0.0 {
            config.insert("sizeUsdc".to_string(), json!(counter_size));
            config.insert("sizeMode".to_string(), json!("usdc"));
            config.remove("sizePct");
            config.remove("sizePercent");
        }
    }

    for (source_key, target_key) in [
        ("counterLegTriggerCondition", "triggerCondition"),
        ("counterLegTriggerPriceCent", "triggerPriceCent"),
        ("counterLegMaxPriceCent", "maxPriceCent"),
        ("counterLegPriceToBeatGuardEnabled", "priceToBeatGuardEnabled"),
        ("counterLegPriceToBeatMode", "priceToBeatMode"),
        ("counterLegPriceToBeatCurrentPriceSource", "priceToBeatCurrentPriceSource"),
        ("counterLegPriceToBeatMaxDiff", "priceToBeatMaxDiff"),
        ("counterLegPriceToBeatMaxDiffUnit", "priceToBeatMaxDiffUnit"),
        ("counterLegExecutionFloorGuardEnabled", "executionFloorGuardEnabled"),
        ("counterLegExecutionFloorPriceCent", "executionFloorPriceCent"),
        ("counterLegRetryOnPriceToBeatGuardBlock", "retryOnPriceToBeatGuardBlock"),
        ("counterLegRetryOnExecutionFloorGuardBlock", "retryOnExecutionFloorGuardBlock"),
        ("counterLegRetryOnMaxPriceBlock", "retryOnMaxPriceBlock"),
        ("counterLegTpEnabled", "tpEnabled"),
        ("counterLegTpPriceCent", "tpPriceCent"),
        ("counterLegNotifyOnTpHit", "notifyOnTpHit"),
        ("counterLegSlEnabled", "slEnabled"),
        ("counterLegSlPriceCent", "slPriceCent"),
        ("counterLegSlTriggerPriceMode", "slTriggerPriceMode"),
        ("counterLegPtbStopLossEnabled", "ptbStopLossEnabled"),
        ("counterLegPtbStopLossGapUsd", "ptbStopLossGapUsd"),
        ("counterLegPtbStopLossGapUnit", "ptbStopLossGapUnit"),
        ("counterLegPtbStopLossCurrentPriceSource", "ptbStopLossCurrentPriceSource"),
        ("counterLegPtbStopLossTimeDecayMode", "ptbStopLossTimeDecayMode"),
        ("counterLegNotifyOnSlHit", "notifyOnSlHit"),
    ] {
        if let Some(value) = node.config.get(source_key) {
            config.insert(target_key.to_string(), value.clone());
        } else {
            config.remove(target_key);
        }
    }
    normalize_child_ptb_stop_loss_current_price_source(&mut config);
    if let Some(max_price) =
        manual_adaptive_risk_override.and_then(|manual| manual.counter_max_price)
    {
        let existing = config
            .get("maxPriceCent")
            .and_then(value_as_f64)
            .map(|value| value / 100.0);
        let effective = existing.map(|value| value.min(max_price)).unwrap_or(max_price);
        let effective_cent = pair_lock_child_price_cent(effective);
        config.insert("maxPriceCent".to_string(), json!(effective_cent));
        config.insert("manualAdaptiveRiskCounterCapApplied".to_string(), json!(true));
        config.insert(
            "manualAdaptiveRiskCounterCapCent".to_string(),
            json!(effective_cent),
        );
    }
    if !pair_lock.protective_unwind_enabled {
        config.insert("retryOnPriceToBeatGuardBlock".to_string(), json!(true));
        config.insert("retryOnExecutionFloorGuardBlock".to_string(), json!(true));
        config.insert("retryOnMaxPriceBlock".to_string(), json!(true));
    }

    TradeFlowNode {
        key: format!("{}__counter", node.key),
        node_type: node.node_type.clone(),
        config: Value::Object(config),
    }
}

async fn cancel_pair_lock_order_if_created(
    repo: &PostgresRepository,
    builder_order_id: Option<i64>,
    reason: &str,
) {
    let Some(builder_order_id) = builder_order_id else {
        return;
    };
    let _ = repo
        .set_trade_builder_order_status(builder_order_id, "canceled", Some(reason))
        .await;
}

async fn hold_pair_lock_counter_after_session_attach(
    repo: &PostgresRepository,
    pair_session_id: i64,
    primary_order_id: i64,
    counter_order_id: i64,
) -> Result<()> {
    repo.set_trade_builder_order_status(
        counter_order_id,
        "inventory_pending",
        Some("pair_counter_waiting_primary_fill"),
    )
    .await?;
    repo.append_trade_builder_order_event(
        counter_order_id,
        "pair_lock_counter_waiting_primary_fill",
        &json!({
            "pair_session_id": pair_session_id,
            "primary_order_id": primary_order_id,
            "reason": "counter_session_attached_before_primary_fill",
            "status_after": "inventory_pending",
        }),
    )
    .await
}
