use crate::trade_flow::guards::polymarket_price_to_beat::PriceToBeatSource;
use crate::trade_flow::guards::price_to_beat::{
    PriceToBeatCurrentPriceSource, PriceToBeatDiffUnit, normalize_price_to_beat_threshold_usd,
    resolve_price_to_beat_current_price_snapshot,
};
use bot_infra::db::TradeBuilderPtbStopLossRule;

#[derive(Debug, Clone, PartialEq)]
struct ActionPlaceOrderPtbStopLossConfig {
    hard_gap_usd: Option<f64>,
    staged_rules: Vec<TradeBuilderPtbStopLossRule>,
    reference_price: Option<f64>,
    time_decay_mode: Option<String>,
    current_price_source: PriceToBeatCurrentPriceSource,
}

mod ptb_stop_loss_cex_median;
#[cfg(test)]
mod ptb_stop_loss_hybrid_tests;

#[derive(Debug, Clone, PartialEq)]
struct TradeBuilderPtbStopLossEvaluation {
    asset: Option<String>,
    direction: Option<String>,
    threshold_gap_usd: f64,
    ptb_reference_price: Option<f64>,
    current_price: Option<f64>,
    current_price_source: &'static str,
    current_chainlink_price: Option<f64>,
    directional_gap: Option<f64>,
    reason_code: &'static str,
    should_trigger: bool,
    source_evaluations: Vec<TradeBuilderPtbStopLossSourceEvaluation>,
}

include!("ptb_stop_loss_cex_consensus.rs");

#[derive(Debug, Clone, PartialEq)]
struct TradeBuilderPtbStopLossSourceEvaluation {
    config_source: &'static str,
    current_price_source: &'static str,
    current_price: Option<f64>,
    directional_gap: Option<f64>,
    reason_code: &'static str,
    should_trigger: bool,
    error_code: Option<&'static str>,
    error_detail: Option<String>,
    metadata: Option<Value>,
}

impl TradeBuilderPtbStopLossSourceEvaluation {
    fn to_value(&self) -> Value {
        json!({
            "config_source": self.config_source,
            "current_price_source": self.current_price_source,
            "current_price": self.current_price,
            "directional_gap": self.directional_gap,
            "reason_code": self.reason_code,
            "should_trigger": self.should_trigger,
            "error_code": self.error_code,
            "error_detail": self.error_detail,
            "metadata": self.metadata,
        })
    }
}

fn trade_builder_market_supports_ptb_stop_loss(market_slug: &str) -> bool {
    let Some(scope) = find_updown_scope_by_slug(market_slug) else {
        return false;
    };
    matches!(scope.timeframe, "5m" | "15m")
}

fn trade_builder_cached_ptb_reference_price(market_slug: &str) -> Option<f64> {
    trade_flow::guards::polymarket_price_to_beat::get_price_to_beat_cached(market_slug)
        .map(|snapshot| snapshot.price_to_beat)
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn resolve_action_place_order_ptb_stop_loss_config(
    node: &TradeFlowNode,
    side: &str,
    market_slug: &str,
) -> Result<Option<ActionPlaceOrderPtbStopLossConfig>> {
    let hard_stop_loss_enabled = node_config_bool(node, "ptbStopLossEnabled").unwrap_or(false);
    let gap_unit = resolve_action_place_order_ptb_stop_loss_gap_unit(node)?;
    let staged_rules = parse_action_place_order_ptb_stop_loss_rules(
        node.config.get("ptbStopLossRules"),
        gap_unit,
    )?;
    if side != "buy" || (!hard_stop_loss_enabled && staged_rules.is_empty()) {
        return Ok(None);
    }

    anyhow::ensure!(
        trade_builder_market_supports_ptb_stop_loss(market_slug),
        "action.place_order ptbStopLossEnabled only supports 5m/15m updown market slugs"
    );

    let hard_gap_usd = node_config_f64(node, "ptbStopLossGapUsd")
        .map(|value| normalize_price_to_beat_threshold_usd(value, gap_unit));
    if hard_stop_loss_enabled && hard_gap_usd.is_none() && staged_rules.is_empty() {
        anyhow::bail!("action.place_order ptbStopLossGapUsd must be set");
    }
    if let Some(gap_usd) = hard_gap_usd {
        anyhow::ensure!(
            gap_usd.is_finite(),
            "action.place_order ptbStopLossGapUsd must be finite"
        );
    }

    let time_decay_mode = node_config_string(node, "ptbStopLossTimeDecayMode")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| value == "none" || value == "tighten" || value == "relax")
        .or_else(|| Some("tighten".to_string()));
    let stop_loss_current_price_source = node_config_string(node, "ptbStopLossCurrentPriceSource");
    let entry_current_price_source = node_config_string(node, "priceToBeatCurrentPriceSource");
    let current_price_source = PriceToBeatCurrentPriceSource::parse(
        stop_loss_current_price_source
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .or(entry_current_price_source.as_deref()),
    );

    Ok(Some(ActionPlaceOrderPtbStopLossConfig {
        hard_gap_usd,
        staged_rules,
        reference_price: trade_builder_cached_ptb_reference_price(market_slug),
        time_decay_mode,
        current_price_source,
    }))
}

fn resolve_action_place_order_ptb_stop_loss_gap_unit(
    node: &TradeFlowNode,
) -> Result<PriceToBeatDiffUnit> {
    let raw = node_config_string(node, "ptbStopLossGapUnit");
    PriceToBeatDiffUnit::parse(raw.as_deref())
        .ok_or_else(|| anyhow::anyhow!("action.place_order ptbStopLossGapUnit must be usd or cent"))
}

#[derive(Debug, Clone, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ActionPlaceOrderPtbStopLossRuleConfig {
    gap_usd: f64,
    size_pct: f64,
}

fn parse_action_place_order_ptb_stop_loss_rules(
    raw_value: Option<&Value>,
    gap_unit: PriceToBeatDiffUnit,
) -> Result<Vec<TradeBuilderPtbStopLossRule>> {
    let Some(raw_value) = raw_value else {
        return Ok(Vec::new());
    };
    let rules: Vec<ActionPlaceOrderPtbStopLossRuleConfig> =
        serde_json::from_value(raw_value.clone())
            .context("action.place_order ptbStopLossRules must be an array")?;
    anyhow::ensure!(
        rules.len() <= TRADE_BUILDER_EXIT_LADDER_MAX_RULES,
        "action.place_order ptbStopLossRules supports at most {} rules",
        TRADE_BUILDER_EXIT_LADDER_MAX_RULES
    );
    let mut parsed = Vec::with_capacity(rules.len());
    let mut previous_gap_usd = None;
    let mut total_size_pct = 0.0_f64;
    for (index, rule) in rules.into_iter().enumerate() {
        anyhow::ensure!(
            rule.gap_usd.is_finite(),
            "action.place_order ptbStopLossRules[{index}].gapUsd must be finite"
        );
        anyhow::ensure!(
            rule.size_pct.is_finite() && rule.size_pct > 0.0 && rule.size_pct <= 100.0,
            "action.place_order ptbStopLossRules[{index}].sizePct must be in (0, 100]"
        );
        if let Some(previous_gap_usd) = previous_gap_usd {
            anyhow::ensure!(
                rule.gap_usd < previous_gap_usd,
                "action.place_order ptbStopLossRules gapUsd values must be strictly decreasing"
            );
        }
        previous_gap_usd = Some(rule.gap_usd);
        total_size_pct += rule.size_pct;
        parsed.push(TradeBuilderPtbStopLossRule {
            gap_usd: normalize_price_to_beat_threshold_usd(rule.gap_usd, gap_unit),
            size_pct: rule.size_pct,
        });
    }
    anyhow::ensure!(
        (total_size_pct - 100.0).abs() <= 0.000001 || parsed.is_empty(),
        "action.place_order ptbStopLossRules sizePct total must equal 100"
    );
    Ok(parsed)
}

fn trade_builder_ptb_stop_loss_target_plan(
    rules: &[TradeBuilderPtbStopLossRule],
    canonical_entry_qty: f64,
    order_min_size: Option<f64>,
) -> TradeBuilderLadderTargetPlan<usize> {
    let weighted_rules = rules
        .iter()
        .enumerate()
        .map(|(index, rule)| (index, rule.size_pct))
        .collect::<Vec<_>>();
    trade_builder_plan_weighted_ladder_targets(&weighted_rules, canonical_entry_qty, order_min_size)
}

fn trade_builder_ptb_stop_loss_gap_usd(order: &TradeBuilderOrder) -> Option<f64> {
    order
        .ptb_stop_loss_gap_usd
        .filter(|value| value.is_finite())
}

#[cfg(test)]
fn trade_builder_ptb_stop_loss_effective_gap_usd(
    order: &TradeBuilderOrder,
    threshold_gap_usd: f64,
) -> f64 {
    trade_builder_ptb_stop_loss_effective_gap_usd_for_market(
        &order.market_slug,
        threshold_gap_usd,
        order.ptb_stop_loss_time_decay_mode.as_deref(),
    )
}

fn trade_builder_ptb_stop_loss_effective_gap_usd_for_market(
    market_slug: &str,
    threshold_gap_usd: f64,
    time_decay_mode: Option<&str>,
) -> f64 {
    let mode = time_decay_mode.unwrap_or("tighten");
    if mode == "none" || threshold_gap_usd < 0.0 {
        return threshold_gap_usd;
    }
    let Some(scope) = find_updown_scope_by_slug(market_slug) else {
        return threshold_gap_usd;
    };
    let Some(window_start) = MarketCycleId(market_slug.to_string()).start_time() else {
        return threshold_gap_usd;
    };
    let window_seconds = updown_scope_window_seconds(scope).max(1);
    let elapsed_ratio = (Utc::now()
        .signed_duration_since(window_start)
        .num_milliseconds()
        .max(0) as f64
        / (window_seconds * 1_000) as f64)
        .clamp(0.0, 1.0);
    match mode {
        "tighten" => (threshold_gap_usd * (1.0 - elapsed_ratio)).max(0.0),
        "relax" => threshold_gap_usd * (1.0 + elapsed_ratio),
        _ => threshold_gap_usd,
    }
}

fn trade_builder_ptb_direction(outcome_label: &str) -> Option<&'static str> {
    match outcome_label.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "long" | "bull" => Some("up"),
        "no" | "down" | "short" | "bear" => Some("down"),
        _ => None,
    }
}

fn trade_builder_ptb_directional_gap(
    direction: &str,
    ptb_reference_price: f64,
    current_price: f64,
) -> f64 {
    if direction == "up" {
        current_price - ptb_reference_price
    } else {
        ptb_reference_price - current_price
    }
}

fn trade_builder_ptb_stop_loss_source_reason_code(
    current_price_source: PriceToBeatCurrentPriceSource,
    should_trigger: bool,
) -> &'static str {
    if !should_trigger {
        return "threshold_not_met";
    }
    match current_price_source {
        PriceToBeatCurrentPriceSource::Chainlink => "chainlink_threshold_hit",
        PriceToBeatCurrentPriceSource::CexConsensus => "cex_consensus_threshold_hit",
        _ => "source_threshold_hit",
    }
}

fn trade_builder_ptb_stop_loss_source_evaluation(
    current_price_source: PriceToBeatCurrentPriceSource,
    market_slug: &str,
    asset: &str,
    direction: &str,
    threshold_gap_usd: f64,
    ptb_reference_price: f64,
) -> TradeBuilderPtbStopLossSourceEvaluation {
    match resolve_price_to_beat_current_price_snapshot(
        current_price_source,
        PriceToBeatSource::Polymarket,
        market_slug,
        asset,
        None,
    ) {
        Ok((current_price, current_price_source_label)) => {
            let directional_gap =
                trade_builder_ptb_directional_gap(direction, ptb_reference_price, current_price);
            let should_trigger = directional_gap <= threshold_gap_usd;
            TradeBuilderPtbStopLossSourceEvaluation {
                config_source: current_price_source.as_config_str(),
                current_price_source: current_price_source_label,
                current_price: Some(current_price),
                directional_gap: Some(directional_gap),
                reason_code: trade_builder_ptb_stop_loss_source_reason_code(
                    current_price_source,
                    should_trigger,
                ),
                should_trigger,
                error_code: None,
                error_detail: None,
                metadata: None,
            }
        }
        Err((error_code, error_detail)) => TradeBuilderPtbStopLossSourceEvaluation {
            config_source: current_price_source.as_config_str(),
            current_price_source: current_price_source.current_price_source_label(),
            current_price: None,
            directional_gap: None,
            reason_code: "threshold_not_met",
            should_trigger: false,
            error_code: Some(error_code),
            error_detail: Some(error_detail),
            metadata: None,
        },
    }
}

fn trade_builder_ptb_stop_loss_source_evaluations(
    current_price_source: PriceToBeatCurrentPriceSource,
    market_slug: &str,
    asset: &str,
    direction: &str,
    threshold_gap_usd: f64,
    base_threshold_gap_usd: f64,
    ptb_reference_price: f64,
) -> Vec<TradeBuilderPtbStopLossSourceEvaluation> {
    let current_price_source = current_price_source.normalize_for_asset(asset);

    if current_price_source == PriceToBeatCurrentPriceSource::CexMedianFast {
        return ptb_stop_loss_cex_median::cex_median_fast_source_evaluations(
            market_slug,
            asset,
            direction,
            threshold_gap_usd,
            base_threshold_gap_usd,
            ptb_reference_price,
        );
    }

    if matches!(
        current_price_source,
        PriceToBeatCurrentPriceSource::CexConsensus
            | PriceToBeatCurrentPriceSource::ChainlinkCexConsensus
            | PriceToBeatCurrentPriceSource::ChainlinkCexConsensusConfirmed
    ) {
        return vec![
            trade_builder_ptb_stop_loss_source_evaluation(
                PriceToBeatCurrentPriceSource::Chainlink,
                market_slug,
                asset,
                direction,
                threshold_gap_usd,
                ptb_reference_price,
            ),
            evaluate_cex_consensus_ptb_stop_loss(
                market_slug,
                direction,
                threshold_gap_usd,
                ptb_reference_price,
            ),
        ];
    }

    if current_price_source == PriceToBeatCurrentPriceSource::BinanceHyperliquid {
        return [
            PriceToBeatCurrentPriceSource::Binance,
            PriceToBeatCurrentPriceSource::Hyperliquid,
        ]
        .into_iter()
        .map(|source| {
            trade_builder_ptb_stop_loss_source_evaluation(
                source,
                market_slug,
                asset,
                direction,
                threshold_gap_usd,
                ptb_reference_price,
            )
        })
        .collect();
    }

    vec![trade_builder_ptb_stop_loss_source_evaluation(
        current_price_source,
        market_slug,
        asset,
        direction,
        threshold_gap_usd,
        ptb_reference_price,
    )]
}

fn trade_builder_evaluate_ptb_stop_loss_inputs(
    market_slug: &str,
    outcome_label: &str,
    threshold_gap_usd: f64,
    ptb_reference_price: Option<f64>,
    current_price_source: PriceToBeatCurrentPriceSource,
    time_decay_mode: Option<&str>,
) -> TradeBuilderPtbStopLossEvaluation {
    let base_threshold_gap_usd = threshold_gap_usd;
    let threshold_gap_usd = trade_builder_ptb_stop_loss_effective_gap_usd_for_market(
        market_slug,
        threshold_gap_usd,
        time_decay_mode,
    );
    let Some(scope) = find_updown_scope_by_slug(market_slug) else {
        let current_price_source_label = current_price_source.current_price_source_label();
        return TradeBuilderPtbStopLossEvaluation {
            asset: None,
            direction: None,
            threshold_gap_usd,
            ptb_reference_price,
            current_price: None,
            current_price_source: current_price_source_label,
            current_chainlink_price: None,
            directional_gap: None,
            reason_code: "unsupported_market",
            should_trigger: false,
            source_evaluations: Vec::new(),
        };
    };
    let current_price_source = current_price_source.normalize_for_asset(scope.asset);
    let current_price_source_label = current_price_source.current_price_source_label();

    if !matches!(scope.timeframe, "5m" | "15m") {
        return TradeBuilderPtbStopLossEvaluation {
            asset: Some(scope.asset.to_string()),
            direction: None,
            threshold_gap_usd,
            ptb_reference_price,
            current_price: None,
            current_price_source: current_price_source_label,
            current_chainlink_price: None,
            directional_gap: None,
            reason_code: "unsupported_market",
            should_trigger: false,
            source_evaluations: Vec::new(),
        };
    }

    let Some(direction) = trade_builder_ptb_direction(outcome_label) else {
        return TradeBuilderPtbStopLossEvaluation {
            asset: Some(scope.asset.to_string()),
            direction: None,
            threshold_gap_usd,
            ptb_reference_price,
            current_price: None,
            current_price_source: current_price_source_label,
            current_chainlink_price: None,
            directional_gap: None,
            reason_code: "unsupported_outcome_label",
            should_trigger: false,
            source_evaluations: Vec::new(),
        };
    };

    let ptb_reference_price = ptb_reference_price
        .filter(|value| value.is_finite() && *value > 0.0)
        .or_else(|| trade_builder_cached_ptb_reference_price(market_slug));
    if ptb_reference_price.is_none() {
        return TradeBuilderPtbStopLossEvaluation {
            asset: Some(scope.asset.to_string()),
            direction: Some(direction.to_string()),
            threshold_gap_usd,
            ptb_reference_price: None,
            current_price: None,
            current_price_source: current_price_source_label,
            current_chainlink_price: None,
            directional_gap: None,
            reason_code: "ptb_reference_pending",
            should_trigger: false,
            source_evaluations: Vec::new(),
        };
    }

    let ptb_reference_price_value = ptb_reference_price.unwrap_or_default();
    let source_evaluations = trade_builder_ptb_stop_loss_source_evaluations(
        current_price_source,
        market_slug,
        scope.asset,
        direction,
        threshold_gap_usd,
        base_threshold_gap_usd,
        ptb_reference_price_value,
    );
    let selected_source = trade_builder_ptb_stop_loss_select_source(&source_evaluations);
    let Some(selected_source) = selected_source else {
        return TradeBuilderPtbStopLossEvaluation {
            asset: Some(scope.asset.to_string()),
            direction: Some(direction.to_string()),
            threshold_gap_usd,
            ptb_reference_price,
            current_price: None,
            current_price_source: current_price_source_label,
            current_chainlink_price: None,
            directional_gap: None,
            reason_code: "ptb_current_price_unavailable",
            should_trigger: false,
            source_evaluations,
        };
    };

    // Negative thresholds intentionally mean "wait for parity, then overshoot against the position".
    let should_trigger = trade_builder_ptb_stop_loss_final_should_trigger(
        current_price_source,
        selected_source,
        &source_evaluations,
        threshold_gap_usd,
    );
    let current_price = selected_source.current_price.unwrap_or_default();
    let current_chainlink_price = source_evaluations
        .iter()
        .find(|evaluation| evaluation.config_source == "chainlink")
        .and_then(|evaluation| evaluation.current_price);
    TradeBuilderPtbStopLossEvaluation {
        asset: Some(scope.asset.to_string()),
        direction: Some(direction.to_string()),
        threshold_gap_usd,
        ptb_reference_price,
        current_price: Some(current_price),
        current_price_source: selected_source.current_price_source,
        current_chainlink_price,
        directional_gap: selected_source.directional_gap,
        reason_code: trade_builder_ptb_stop_loss_final_reason_code(
            current_price_source,
            &source_evaluations,
            should_trigger,
        ),
        should_trigger,
        source_evaluations,
    }
}

fn trade_builder_evaluate_ptb_stop_loss(
    order: &TradeBuilderOrder,
) -> Option<TradeBuilderPtbStopLossEvaluation> {
    let threshold_gap_usd = trade_builder_ptb_stop_loss_gap_usd(order)?;
    if !trade_builder_is_stop_loss_child(order) {
        return None;
    }
    let current_price_source =
        PriceToBeatCurrentPriceSource::parse(Some(order.ptb_current_price_source.as_str()));
    Some(trade_builder_evaluate_ptb_stop_loss_inputs(
        &order.market_slug,
        &order.outcome_label,
        threshold_gap_usd,
        order.ptb_reference_price,
        current_price_source,
        order.ptb_stop_loss_time_decay_mode.as_deref(),
    ))
}

fn trade_builder_ptb_reference_price_persist_candidate(
    order: &TradeBuilderOrder,
    evaluation: &TradeBuilderPtbStopLossEvaluation,
) -> Option<f64> {
    if order
        .ptb_reference_price
        .is_some_and(|value| value.is_finite() && value > 0.0)
    {
        return None;
    }
    evaluation
        .ptb_reference_price
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn append_trade_builder_ptb_stop_loss_payload(
    payload: &mut serde_json::Map<String, Value>,
    evaluation: &TradeBuilderPtbStopLossEvaluation,
) {
    payload.insert(
        "ptb_stop_loss".to_string(),
        json!({
            "reason_code": evaluation.reason_code,
            "asset": evaluation.asset,
            "direction": evaluation.direction,
            "threshold_gap_usd": evaluation.threshold_gap_usd,
            "ptb_reference_price": evaluation.ptb_reference_price,
            "current_price": evaluation.current_price,
            "current_price_source": evaluation.current_price_source,
            "current_chainlink_price": evaluation.current_chainlink_price,
            "directional_gap": evaluation.directional_gap,
            "should_trigger": evaluation.should_trigger,
            "source_evaluations": evaluation
                .source_evaluations
                .iter()
                .map(TradeBuilderPtbStopLossSourceEvaluation::to_value)
                .collect::<Vec<_>>(),
        }),
    );
}

#[cfg(test)]
#[path = "ptb_stop_loss_tests.rs"]
pub(crate) mod trade_builder_ptb_stop_loss_tests;
