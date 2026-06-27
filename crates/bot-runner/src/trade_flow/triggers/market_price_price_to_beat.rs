#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriggerMarketPriceGateMode {
    StandardOnly,
    StandardAndPtb,
    PtbOnly,
}

#[derive(Debug, Clone, Copy)]
struct TriggerMarketPricePtbConfig {
    mode: crate::trade_flow::guards::price_to_beat::PriceToBeatMode,
    min_gap: Option<f64>,
    max_gap: Option<f64>,
    unit: crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit,
}

fn trigger_market_price_standard_trigger_enabled(trigger_condition: &str) -> bool {
    !trigger_condition.trim().is_empty()
}

fn trigger_market_price_ptb_config_from_spec(
    node_spec: &WsOpenPositionPriceNodeSpec,
) -> Option<TriggerMarketPricePtbConfig> {
    if !node_spec.price_to_beat_trigger_enabled {
        return None;
    }
    Some(TriggerMarketPricePtbConfig {
        mode: node_spec.price_to_beat_mode,
        min_gap: node_spec.price_to_beat_trigger_min_gap,
        max_gap: node_spec.price_to_beat_trigger_max_gap,
        unit: node_spec.price_to_beat_trigger_unit,
    })
}

fn trigger_market_price_ptb_config_from_node(
    node: &TradeFlowNode,
    selected_entry_timing_profile: Option<&TriggerMarketEntryTimingProfileSelection>,
) -> Option<TriggerMarketPricePtbConfig> {
    if node.node_type != "trigger.market_price" || node_market_mode(node) != "auto_scope" {
        return None;
    }
    if !node_config_bool(node, "priceToBeatTriggerEnabled").unwrap_or(false) {
        return None;
    }
    let mode = crate::trade_flow::guards::price_to_beat::PriceToBeatMode::parse(
        node_config_string(node, "priceToBeatMode").as_deref(),
    )
    .unwrap_or(crate::trade_flow::guards::price_to_beat::PriceToBeatMode::Manual);
    let min_gap = selected_entry_timing_profile
        .and_then(|profile| profile.price_to_beat_trigger_min_gap)
        .or_else(|| node_config_f64(node, "priceToBeatTriggerMinGap"))
        .filter(|value| value.is_finite() && *value > 0.0);
    if matches!(mode, crate::trade_flow::guards::price_to_beat::PriceToBeatMode::Manual)
        && min_gap.is_none()
    {
        return None;
    }
    let max_gap = selected_entry_timing_profile
        .and_then(|profile| profile.price_to_beat_trigger_max_gap)
        .or_else(|| node_config_f64(node, "priceToBeatTriggerMaxGap"))
        .filter(|value| value.is_finite() && *value > 0.0)
        .filter(|value| min_gap.map(|min_gap| *value >= min_gap).unwrap_or(false));
    let unit = crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::parse(
        node_config_string(node, "priceToBeatTriggerUnit").as_deref(),
    )
    .unwrap_or(crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Usd);
    Some(TriggerMarketPricePtbConfig {
        mode,
        min_gap,
        max_gap,
        unit,
    })
}

fn trigger_market_price_gate_mode(
    trigger_condition: &str,
    ptb_config: Option<TriggerMarketPricePtbConfig>,
) -> Option<TriggerMarketPriceGateMode> {
    let has_standard_trigger = trigger_market_price_standard_trigger_enabled(trigger_condition);
    match (has_standard_trigger, ptb_config.is_some()) {
        (true, true) => Some(TriggerMarketPriceGateMode::StandardAndPtb),
        (true, false) => Some(TriggerMarketPriceGateMode::StandardOnly),
        (false, true) => Some(TriggerMarketPriceGateMode::PtbOnly),
        (false, false) => None,
    }
}

fn evaluate_trigger_market_price_ptb_gate(
    market_slug: &str,
    outcome_label: &str,
    ptb_config: TriggerMarketPricePtbConfig,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
) -> crate::trade_flow::guards::price_to_beat::PriceToBeatTriggerGateResult {
    let signal_config = Some(
        crate::trade_flow::guards::price_to_beat::PriceToBeatSignalFormulaConfig::taker(
            crate::trade_flow::guards::price_to_beat::PriceToBeatSignalFormulaMarketInput {
                best_bid,
                best_ask,
            },
        ),
    );
    crate::trade_flow::guards::price_to_beat::evaluate_price_to_beat_trigger_gate(
        market_slug,
        outcome_label,
        ptb_config.mode,
        ptb_config.min_gap,
        ptb_config.max_gap,
        ptb_config.unit,
        signal_config,
    )
}

fn evaluate_trigger_market_price_ptb_gate_for_spec(
    node_spec: &WsOpenPositionPriceNodeSpec,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
) -> Option<crate::trade_flow::guards::price_to_beat::PriceToBeatTriggerGateResult> {
    let ptb_config = trigger_market_price_ptb_config_from_spec(node_spec)?;
    Some(evaluate_trigger_market_price_ptb_gate(
        node_spec.market_slug.as_deref().unwrap_or_default(),
        &node_spec.outcome_label,
        ptb_config,
        best_bid,
        best_ask,
    ))
}

fn evaluate_trigger_market_price_ptb_gate_for_node(
    node: &TradeFlowNode,
    market_slug: &str,
    outcome_label: &str,
    selected_entry_timing_profile: Option<&TriggerMarketEntryTimingProfileSelection>,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
) -> Option<crate::trade_flow::guards::price_to_beat::PriceToBeatTriggerGateResult> {
    let ptb_config =
        trigger_market_price_ptb_config_from_node(node, selected_entry_timing_profile)?;
    Some(evaluate_trigger_market_price_ptb_gate(
        market_slug,
        outcome_label,
        ptb_config,
        best_bid,
        best_ask,
    ))
}

fn unsupported_price_to_beat_trigger_gate(
) -> crate::trade_flow::guards::price_to_beat::PriceToBeatTriggerGateResult {
    crate::trade_flow::guards::price_to_beat::PriceToBeatTriggerGateResult {
        passed: false,
        reason: "unsupported_outcome_label",
        directional_gap: None,
        price_to_beat: None,
        price_to_beat_status: None,
        current_price: None,
        threshold_mode: "manual".to_string(),
        min_gap: 0.0,
        max_gap: None,
        auto_threshold_usd: None,
        lookback_windows_used: None,
        current_windows_used: None,
        avg_up_excursion_usd: None,
        avg_down_excursion_usd: None,
        lookback_market_slugs: None,
        lookback_window_snapshots: None,
        baseline_pct: None,
        current_pct: None,
        vol_factor: None,
        threshold_pct: None,
        base_pct: None,
        floor_usd: None,
        ceiling_usd: None,
        threshold_was_clamped: None,
        signal_formula: None,
        iv_mismatch_edge: None,
    }
}

fn append_trigger_market_price_ptb_gate(target: &mut Value, gate: &Value) {
    let Some(target_obj) = target.as_object_mut() else {
        return;
    };
    target_obj.insert("priceToBeatTriggerGate".to_string(), gate.clone());
}

fn step_price_to_beat_trigger_gate(step: &TradeFlowRunStep) -> Option<Value> {
    step.input_json
        .as_ref()
        .and_then(|input| input.get("priceToBeatTriggerGate"))
        .cloned()
}

fn price_to_beat_trigger_gate_passed(gate: &Value) -> Option<bool> {
    gate.get("passed").and_then(Value::as_bool)
}
