use bot_infra::db::{TradeBuilderConfidenceLadderFillInput, TradeBuilderConfidenceLadderState};

const ACTION_PLACE_ORDER_MODE_CONFIDENCE_LADDER_HEDGE_LOCK_V1: &str =
    "confidence_ladder_hedge_lock_v1";
const CONFIDENCE_LADDER_BINDING_MODE: &str = "confidence_ladder_only";
const CONFIDENCE_LADDER_CONFIG_KEY: &str = "confidenceLadder";
const CONFIDENCE_LADDER_ORDER_MARKER_KEY: &str = "confidenceLadderOrder";
const CONFIDENCE_LADDER_ROOT_NODE_KEY: &str = "confidenceLadderRootNodeKey";
const CONFIDENCE_LADDER_SIDE_KEY: &str = "confidenceLadderSide";
const CONFIDENCE_LADDER_INTENT_KEY: &str = "confidenceLadderIntent";
const CONFIDENCE_LADDER_MODEL_PROBABILITY_KEY: &str = "confidenceLadderModelProbability";
const CONFIDENCE_LADDER_EDGE_KEY: &str = "confidenceLadderEdge";

#[derive(Debug, Clone, Copy, PartialEq)]
struct ConfidenceLadderBand {
    price_min: f64,
    price_max: f64,
    add_shares: f64,
    min_model_edge: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct ConfidenceLadderConfig {
    base_probe_shares: f64,
    max_loss_per_market_usdc: f64,
    max_total_cost_per_market_usdc: f64,
    entry_window_start_sec: i64,
    entry_window_end_sec: i64,
    no_new_dominant_buy_last_sec: i64,
    probe_price_min: f64,
    probe_price_max: f64,
    max_spread: f64,
    dominance_gap: f64,
    chop_probability_max: f64,
    hard_no_chase_above: f64,
    taker_fee_rate: f64,
    slippage_buffer: f64,
    prefer_post_only: bool,
    taker_allowed_only_if_edge_above: f64,
    opposite_price_max: f64,
    hedge_min_reversal_edge: f64,
    profit_lock_pair_cost_max: f64,
    strong_profit_lock_pair_cost: f64,
    damage_control_price_min: f64,
    damage_control_price_max: f64,
    target_hedge_ratio_min: f64,
    target_hedge_ratio_max: f64,
    max_direction_flips: i64,
    ladder: Vec<ConfidenceLadderBand>,
}

impl Default for ConfidenceLadderConfig {
    fn default() -> Self {
        Self {
            base_probe_shares: 2.0,
            max_loss_per_market_usdc: 3.0,
            max_total_cost_per_market_usdc: 25.0,
            entry_window_start_sec: 3,
            entry_window_end_sec: 285,
            no_new_dominant_buy_last_sec: 15,
            probe_price_min: 0.45,
            probe_price_max: 0.55,
            max_spread: 0.03,
            dominance_gap: 0.12,
            chop_probability_max: 0.45,
            hard_no_chase_above: 0.93,
            taker_fee_rate: 0.07,
            slippage_buffer: 0.005,
            prefer_post_only: true,
            taker_allowed_only_if_edge_above: 0.05,
            opposite_price_max: 0.20,
            hedge_min_reversal_edge: 0.03,
            profit_lock_pair_cost_max: 0.97,
            strong_profit_lock_pair_cost: 0.94,
            damage_control_price_min: 0.35,
            damage_control_price_max: 0.60,
            target_hedge_ratio_min: 0.35,
            target_hedge_ratio_max: 1.0,
            max_direction_flips: 2,
            ladder: vec![
                ConfidenceLadderBand {
                    price_min: 0.45,
                    price_max: 0.55,
                    add_shares: 2.0,
                    min_model_edge: 0.02,
                },
                ConfidenceLadderBand {
                    price_min: 0.55,
                    price_max: 0.65,
                    add_shares: 3.0,
                    min_model_edge: 0.02,
                },
                ConfidenceLadderBand {
                    price_min: 0.65,
                    price_max: 0.75,
                    add_shares: 5.0,
                    min_model_edge: 0.025,
                },
                ConfidenceLadderBand {
                    price_min: 0.75,
                    price_max: 0.85,
                    add_shares: 5.0,
                    min_model_edge: 0.03,
                },
                ConfidenceLadderBand {
                    price_min: 0.85,
                    price_max: 0.92,
                    add_shares: 2.0,
                    min_model_edge: 0.04,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ConfidenceLadderQuote {
    ladder_side: &'static str,
    token_id: String,
    outcome_label: String,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    best_bid_size: Option<f64>,
    best_ask_size: Option<f64>,
    last_trade_price: Option<f64>,
    current_price: f64,
    snapshot: Value,
}

#[derive(Debug, Clone, PartialEq)]
struct ConfidenceLadderModel {
    p_up: f64,
    p_down: f64,
    chop: f64,
    score_up: f64,
    score_down: f64,
    dominant_side: Option<&'static str>,
    diagnostics: Value,
}

#[derive(Debug, Clone, PartialEq)]
struct ConfidenceLadderDecision {
    ladder_side: &'static str,
    intent: &'static str,
    quantity: f64,
    price: f64,
    all_in_cost: f64,
    model_probability: f64,
    edge: f64,
    min_edge: f64,
    post_only: bool,
    order_type: &'static str,
    execution_mode: &'static str,
    diagnostics: Value,
}

fn action_place_order_uses_confidence_ladder(node: &TradeFlowNode) -> bool {
    action_place_order_mode(node) == ACTION_PLACE_ORDER_MODE_CONFIDENCE_LADDER_HEDGE_LOCK_V1
}

fn confidence_ladder_raw_config(node: &TradeFlowNode) -> Option<&Value> {
    node.config.get(CONFIDENCE_LADDER_CONFIG_KEY)
}

fn confidence_ladder_config_value<'a>(node: &'a TradeFlowNode, key: &str) -> Option<&'a Value> {
    confidence_ladder_raw_config(node)
        .and_then(|value| value.get(key))
        .or_else(|| node.config.get(key))
}

fn confidence_ladder_config_f64(node: &TradeFlowNode, key: &str) -> Option<f64> {
    confidence_ladder_config_value(node, key)
        .and_then(value_as_f64)
        .filter(|value| value.is_finite())
}

fn confidence_ladder_config_i64(node: &TradeFlowNode, key: &str) -> Option<i64> {
    confidence_ladder_config_value(node, key).and_then(value_as_i64)
}

fn confidence_ladder_config_bool(node: &TradeFlowNode, key: &str) -> Option<bool> {
    confidence_ladder_config_value(node, key).and_then(|value| match value {
        Value::Bool(value) => Some(*value),
        Value::Number(value) => value.as_i64().map(|number| number != 0),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        },
        _ => None,
    })
}

fn parse_confidence_ladder_config(node: &TradeFlowNode) -> Result<ConfidenceLadderConfig> {
    let mut config = ConfidenceLadderConfig::default();
    config.base_probe_shares =
        confidence_ladder_config_f64(node, "baseProbeShares").unwrap_or(config.base_probe_shares);
    config.max_loss_per_market_usdc = confidence_ladder_config_f64(node, "maxLossPerMarketUsdc")
        .unwrap_or(config.max_loss_per_market_usdc);
    config.max_total_cost_per_market_usdc =
        confidence_ladder_config_f64(node, "maxTotalCostPerMarketUsdc")
            .unwrap_or(config.max_total_cost_per_market_usdc);
    config.entry_window_start_sec = confidence_ladder_config_i64(node, "entryWindowStartSec")
        .unwrap_or_else(|| {
            confidence_ladder_raw_config(node)
                .and_then(|value| value.get("entryWindow"))
                .and_then(|value| value.get("startSec"))
                .and_then(value_as_i64)
                .unwrap_or(config.entry_window_start_sec)
        });
    config.entry_window_end_sec = confidence_ladder_config_i64(node, "entryWindowEndSec")
        .unwrap_or_else(|| {
            confidence_ladder_raw_config(node)
                .and_then(|value| value.get("entryWindow"))
                .and_then(|value| value.get("endSec"))
                .and_then(value_as_i64)
                .unwrap_or(config.entry_window_end_sec)
        });
    config.no_new_dominant_buy_last_sec =
        confidence_ladder_config_i64(node, "noNewDominantBuyLastSec")
            .unwrap_or(config.no_new_dominant_buy_last_sec);
    config.probe_price_min =
        confidence_ladder_config_f64(node, "probePriceMin").unwrap_or_else(|| {
            confidence_ladder_raw_config(node)
                .and_then(|value| value.get("probe"))
                .and_then(|value| value.get("min"))
                .and_then(value_as_f64)
                .map(|value| value / 100.0)
                .unwrap_or(config.probe_price_min)
        });
    config.probe_price_max =
        confidence_ladder_config_f64(node, "probePriceMax").unwrap_or_else(|| {
            confidence_ladder_raw_config(node)
                .and_then(|value| value.get("probe"))
                .and_then(|value| value.get("max"))
                .and_then(value_as_f64)
                .map(|value| value / 100.0)
                .unwrap_or(config.probe_price_max)
        });
    config.max_spread =
        confidence_ladder_config_f64(node, "maxSpread").unwrap_or(config.max_spread);
    config.dominance_gap =
        confidence_ladder_config_f64(node, "dominanceGap").unwrap_or(config.dominance_gap);
    config.chop_probability_max = confidence_ladder_config_f64(node, "chopProbabilityMax")
        .unwrap_or(config.chop_probability_max);
    config.hard_no_chase_above = confidence_ladder_config_f64(node, "hardNoChaseAbove")
        .unwrap_or(config.hard_no_chase_above);
    config.taker_fee_rate =
        confidence_ladder_config_f64(node, "takerFeeRate").unwrap_or(config.taker_fee_rate);
    config.slippage_buffer =
        confidence_ladder_config_f64(node, "slippageBuffer").unwrap_or(config.slippage_buffer);
    config.prefer_post_only =
        confidence_ladder_config_bool(node, "preferPostOnly").unwrap_or(config.prefer_post_only);
    config.taker_allowed_only_if_edge_above =
        confidence_ladder_config_f64(node, "takerAllowedOnlyIfEdgeAbove")
            .unwrap_or(config.taker_allowed_only_if_edge_above);
    config.opposite_price_max = confidence_ladder_config_f64(node, "oppositePriceMax")
        .unwrap_or_else(|| {
            confidence_ladder_raw_config(node)
                .and_then(|value| value.get("hedge"))
                .and_then(|value| value.get("oppositePriceMax"))
                .and_then(value_as_f64)
                .unwrap_or(config.opposite_price_max)
        });
    config.hedge_min_reversal_edge = confidence_ladder_config_f64(node, "minReversalEdge")
        .unwrap_or_else(|| {
            confidence_ladder_raw_config(node)
                .and_then(|value| value.get("hedge"))
                .and_then(|value| value.get("minReversalEdge"))
                .and_then(value_as_f64)
                .unwrap_or(config.hedge_min_reversal_edge)
        });
    config.profit_lock_pair_cost_max = confidence_ladder_config_f64(node, "profitLockPairCostMax")
        .unwrap_or_else(|| {
            confidence_ladder_raw_config(node)
                .and_then(|value| value.get("hedge"))
                .and_then(|value| value.get("profitLockPairCostMax"))
                .and_then(value_as_f64)
                .unwrap_or(config.profit_lock_pair_cost_max)
        });
    config.strong_profit_lock_pair_cost =
        confidence_ladder_config_f64(node, "strongProfitLockPairCost").unwrap_or_else(|| {
            confidence_ladder_raw_config(node)
                .and_then(|value| value.get("hedge"))
                .and_then(|value| value.get("strongProfitLockPairCost"))
                .and_then(value_as_f64)
                .unwrap_or(config.strong_profit_lock_pair_cost)
        });
    config.damage_control_price_min = confidence_ladder_config_f64(node, "damageControlPriceMin")
        .unwrap_or_else(|| {
            confidence_ladder_raw_config(node)
                .and_then(|value| value.get("hedge"))
                .and_then(|value| value.get("damageControlPriceMin"))
                .and_then(value_as_f64)
                .unwrap_or(config.damage_control_price_min)
        });
    config.damage_control_price_max = confidence_ladder_config_f64(node, "damageControlPriceMax")
        .unwrap_or_else(|| {
            confidence_ladder_raw_config(node)
                .and_then(|value| value.get("hedge"))
                .and_then(|value| value.get("damageControlPriceMax"))
                .and_then(value_as_f64)
                .unwrap_or(config.damage_control_price_max)
        });
    config.target_hedge_ratio_min = confidence_ladder_config_f64(node, "targetHedgeRatioMin")
        .unwrap_or_else(|| {
            confidence_ladder_raw_config(node)
                .and_then(|value| value.get("hedge"))
                .and_then(|value| value.get("targetHedgeRatioMin"))
                .and_then(value_as_f64)
                .unwrap_or(config.target_hedge_ratio_min)
        });
    config.target_hedge_ratio_max = confidence_ladder_config_f64(node, "targetHedgeRatioMax")
        .unwrap_or_else(|| {
            confidence_ladder_raw_config(node)
                .and_then(|value| value.get("hedge"))
                .and_then(|value| value.get("targetHedgeRatioMax"))
                .and_then(value_as_f64)
                .unwrap_or(config.target_hedge_ratio_max)
        });
    config.max_direction_flips = confidence_ladder_config_i64(node, "maxDirectionFlips")
        .unwrap_or_else(|| {
            confidence_ladder_raw_config(node)
                .and_then(|value| value.get("stop"))
                .and_then(|value| value.get("maxDirectionFlips"))
                .and_then(value_as_i64)
                .unwrap_or(config.max_direction_flips)
        });

    anyhow::ensure!(
        config.base_probe_shares > 0.0,
        "confidenceLadder.baseProbeShares must be > 0"
    );
    anyhow::ensure!(
        config.max_loss_per_market_usdc > 0.0,
        "confidenceLadder.maxLossPerMarketUsdc must be > 0"
    );
    anyhow::ensure!(
        config.max_total_cost_per_market_usdc > 0.0,
        "confidenceLadder.maxTotalCostPerMarketUsdc must be > 0"
    );
    anyhow::ensure!(
        config.entry_window_start_sec >= 0
            && config.entry_window_start_sec < config.entry_window_end_sec,
        "confidenceLadder entry window must have start >= 0 and start < end"
    );
    anyhow::ensure!(
        config.probe_price_min > 0.0 && config.probe_price_min < config.probe_price_max,
        "confidenceLadder probe range must have min > 0 and min < max"
    );
    anyhow::ensure!(
        config.max_spread > 0.0 && config.max_spread < 1.0,
        "confidenceLadder.maxSpread must be in (0, 1)"
    );
    anyhow::ensure!(
        config.damage_control_price_min > 0.0
            && config.damage_control_price_min < config.damage_control_price_max
            && config.damage_control_price_max < 1.0,
        "confidenceLadder.hedge damage control prices must satisfy 0 < min < max < 1"
    );
    Ok(config)
}

fn confidence_ladder_binding_trigger_key(
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
) -> Result<String> {
    let trigger_key = find_upstream_market_price_trigger_key(&node.key, graph).ok_or_else(|| {
        anyhow::anyhow!(
            "confidence_ladder_hedge_lock_v1 requires upstream trigger.market_price bindingMode=confidence_ladder_only"
        )
    })?;
    let trigger_node = flow_node(graph, &trigger_key).ok_or_else(|| {
        anyhow::anyhow!("confidence_ladder_hedge_lock_v1 upstream trigger node not found")
    })?;
    let binding_mode = node_config_string(trigger_node, "bindingMode")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    anyhow::ensure!(
        binding_mode == CONFIDENCE_LADDER_BINDING_MODE,
        "confidence_ladder_hedge_lock_v1 requires upstream trigger.market_price bindingMode=confidence_ladder_only"
    );
    Ok(trigger_key)
}

fn confidence_ladder_market_slug(
    node: &TradeFlowNode,
    step: &TradeFlowRunStep,
    context: &Value,
) -> Result<String> {
    step_input_string(step, &["marketSlug", "market_slug", "wsMarketSlug"])
        .or_else(|| flow_context_string(context, "marketSlug"))
        .or_else(|| node_config_string(node, "marketSlug"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("confidence_ladder_hedge_lock_v1 requires marketSlug"))
}

fn confidence_ladder_remaining_sec(market_slug: &str) -> Option<i64> {
    positive_quantity_flip_grid_remaining_sec(market_slug)
}

fn confidence_ladder_elapsed_sec(market_slug: &str) -> Option<i64> {
    confidence_ladder_remaining_sec(market_slug).map(|remaining| (300 - remaining).clamp(0, 300))
}

fn confidence_ladder_in_entry_window(config: &ConfidenceLadderConfig, market_slug: &str) -> bool {
    confidence_ladder_elapsed_sec(market_slug)
        .map(|elapsed| {
            elapsed >= config.entry_window_start_sec && elapsed <= config.entry_window_end_sec
        })
        .unwrap_or(true)
}

fn confidence_ladder_is_late_for_dominant_buy(
    config: &ConfidenceLadderConfig,
    market_slug: &str,
) -> bool {
    confidence_ladder_remaining_sec(market_slug)
        .map(|remaining| remaining <= config.no_new_dominant_buy_last_sec)
        .unwrap_or(false)
}

fn confidence_ladder_side_probability(model: &ConfidenceLadderModel, side: &str) -> f64 {
    if side == "down" {
        model.p_down
    } else {
        model.p_up
    }
}

fn confidence_ladder_quote_price(quote: &ConfidenceLadderQuote) -> Option<f64> {
    quote
        .best_ask
        .or(Some(quote.current_price))
        .map(clamp_probability)
        .filter(|value| *value > 0.0 && *value < 1.0)
}

fn confidence_ladder_quote_mid(quote: &ConfidenceLadderQuote) -> f64 {
    match (quote.best_bid, quote.best_ask) {
        (Some(bid), Some(ask)) => clamp_probability((bid + ask) / 2.0),
        (Some(bid), None) => clamp_probability(bid),
        (None, Some(ask)) => clamp_probability(ask),
        _ => clamp_probability(quote.current_price),
    }
}

fn confidence_ladder_spread(quote: &ConfidenceLadderQuote) -> Option<f64> {
    match (quote.best_bid, quote.best_ask) {
        (Some(bid), Some(ask)) if ask >= bid => Some(ask - bid),
        _ => None,
    }
}

fn confidence_ladder_book_pressure(quote: &ConfidenceLadderQuote) -> f64 {
    let bid = quote.best_bid_size.unwrap_or(0.0).max(0.0);
    let ask = quote.best_ask_size.unwrap_or(0.0).max(0.0);
    let total = bid + ask;
    if total <= 0.0 {
        0.0
    } else {
        ((bid - ask) / total).clamp(-1.0, 1.0)
    }
}

fn confidence_ladder_signal_from_step_or_context(
    step: &TradeFlowRunStep,
    context: &Value,
    direct_keys: &[&str],
    context_keys: &[&str],
) -> f64 {
    direct_keys
        .iter()
        .find_map(|key| step_input_f64(step, &[*key]))
        .or_else(|| {
            context_keys
                .iter()
                .find_map(|key| flow_context_f64(context, key))
        })
        .unwrap_or(0.0)
        .clamp(-1.0, 1.0)
}

fn confidence_ladder_heuristic_model(
    config: &ConfidenceLadderConfig,
    state: &TradeBuilderConfidenceLadderState,
    up: &ConfidenceLadderQuote,
    down: &ConfidenceLadderQuote,
    step: &TradeFlowRunStep,
    context: &Value,
) -> ConfidenceLadderModel {
    let up_mid = confidence_ladder_quote_mid(up);
    let down_mid = confidence_ladder_quote_mid(down);
    let poly_momentum = ((up_mid - down_mid) / 0.50).clamp(-1.0, 1.0);
    let book_pressure =
        (confidence_ladder_book_pressure(up) - confidence_ladder_book_pressure(down)) / 2.0;
    let up_trade = up.last_trade_price.unwrap_or(up_mid);
    let down_trade = down.last_trade_price.unwrap_or(down_mid);
    let trade_flow = (((up_trade - up_mid) - (down_trade - down_mid)) / 0.10).clamp(-1.0, 1.0);
    let btc_gap = confidence_ladder_signal_from_step_or_context(
        step,
        context,
        &[
            "btcCycleOpenGap",
            "btc_cycle_open_gap",
            "btcImpulse",
            "btc_impulse",
            "cexImpulse",
            "cex_impulse",
        ],
        &[
            "btcCycleOpenGap",
            "btc_cycle_open_gap",
            "btcImpulse",
            "btc_impulse",
            "cexImpulse",
            "cex_impulse",
        ],
    );
    let max_spread = confidence_ladder_spread(up)
        .unwrap_or(config.max_spread)
        .max(confidence_ladder_spread(down).unwrap_or(config.max_spread));
    let balance_penalty = if (up_mid - down_mid).abs() < 0.08 {
        0.12
    } else {
        0.0
    };
    let flip_penalty = (state.side_switch_count as f64 * 0.18).min(0.45);
    let spread_penalty = (max_spread / config.max_spread).min(2.0) * 0.10;
    let chop = (0.08 + balance_penalty + flip_penalty + spread_penalty).clamp(0.0, 0.95);
    let directional =
        (0.35 * btc_gap + 0.25 * poly_momentum + 0.20 * book_pressure + 0.10 * trade_flow)
            .clamp(-1.0, 1.0);
    let confidence_scale = (1.0 - chop * 0.5).clamp(0.25, 1.0);
    let score_up = clamp_probability(0.5 + directional * 0.5 * confidence_scale);
    let score_down = clamp_probability(1.0 - score_up);
    let p_up = clamp_probability(0.5 + directional * 0.32 * confidence_scale - chop * 0.03);
    let p_down = clamp_probability(1.0 - p_up);
    let dominant_side = if score_up > score_down + config.dominance_gap {
        Some("up")
    } else if score_down > score_up + config.dominance_gap {
        Some("down")
    } else {
        None
    };
    ConfidenceLadderModel {
        p_up,
        p_down,
        chop,
        score_up,
        score_down,
        dominant_side,
        diagnostics: json!({
            "p_up": p_up,
            "p_down": p_down,
            "chop": chop,
            "score_up": score_up,
            "score_down": score_down,
            "dominant_side": dominant_side,
            "btc_gap": btc_gap,
            "polymarket_mid_momentum": poly_momentum,
            "book_pressure": book_pressure,
            "trade_flow": trade_flow,
            "up_mid": up_mid,
            "down_mid": down_mid,
            "max_spread": max_spread,
            "side_switch_count": state.side_switch_count,
        }),
    }
}

fn confidence_ladder_fee(price: f64, fee_rate: f64) -> f64 {
    fee_rate.max(0.0) * price.max(0.0) * (1.0 - price.clamp(0.0, 1.0))
}

fn confidence_ladder_all_in_cost(
    price: f64,
    config: &ConfidenceLadderConfig,
    include_slippage: bool,
) -> f64 {
    price
        + confidence_ladder_fee(price, config.taker_fee_rate)
        + if include_slippage {
            config.slippage_buffer.max(0.0)
        } else {
            0.0
        }
}

fn confidence_ladder_band_for_price<'a>(
    config: &'a ConfidenceLadderConfig,
    price: f64,
) -> Option<&'a ConfidenceLadderBand> {
    if price >= config.hard_no_chase_above {
        return None;
    }
    config
        .ladder
        .iter()
        .find(|band| price >= band.price_min && price < band.price_max)
}

fn confidence_ladder_projected_worst_case_pnl(
    state: &TradeBuilderConfidenceLadderState,
    side: &str,
    quantity: f64,
    all_in_cost: f64,
) -> f64 {
    let projected_up_qty = state.up_qty + if side == "up" { quantity } else { 0.0 };
    let projected_down_qty = state.down_qty + if side == "down" { quantity } else { 0.0 };
    let projected_cost = state.total_cost_usdc + quantity * all_in_cost;
    (projected_up_qty - projected_cost).min(projected_down_qty - projected_cost)
}

fn confidence_ladder_projected_total_cost(
    state: &TradeBuilderConfidenceLadderState,
    quantity: f64,
    all_in_cost: f64,
) -> f64 {
    state.total_cost_usdc + quantity * all_in_cost
}

fn confidence_ladder_risk_allows_buy(
    config: &ConfidenceLadderConfig,
    state: &TradeBuilderConfidenceLadderState,
    side: &str,
    quantity: f64,
    all_in_cost: f64,
) -> bool {
    confidence_ladder_projected_total_cost(state, quantity, all_in_cost)
        <= config.max_total_cost_per_market_usdc
        && confidence_ladder_projected_worst_case_pnl(state, side, quantity, all_in_cost)
            >= -config.max_loss_per_market_usdc
}

fn confidence_ladder_side_quote<'a>(
    side: &str,
    up: &'a ConfidenceLadderQuote,
    down: &'a ConfidenceLadderQuote,
) -> &'a ConfidenceLadderQuote {
    if side == "down" {
        down
    } else {
        up
    }
}

fn confidence_ladder_held_side(state: &TradeBuilderConfidenceLadderState) -> Option<&'static str> {
    if state.up_qty <= 0.0 && state.down_qty <= 0.0 {
        None
    } else if state.up_qty >= state.down_qty {
        Some("up")
    } else {
        Some("down")
    }
}

fn confidence_ladder_state_qty(state: &TradeBuilderConfidenceLadderState, side: &str) -> f64 {
    if side == "down" {
        state.down_qty
    } else {
        state.up_qty
    }
}

fn confidence_ladder_state_avg_cost(
    state: &TradeBuilderConfidenceLadderState,
    side: &str,
) -> Option<f64> {
    if side == "down" {
        state.down_avg_cost
    } else {
        state.up_avg_cost
    }
}

fn confidence_ladder_opposite_side(side: &str) -> &'static str {
    if side == "down" {
        "up"
    } else {
        "down"
    }
}

fn confidence_ladder_side_flip_disabled(
    config: &ConfidenceLadderConfig,
    state: &TradeBuilderConfidenceLadderState,
) -> bool {
    state.side_switch_count >= config.max_direction_flips
}

fn confidence_ladder_execution_shape(
    config: &ConfidenceLadderConfig,
    edge: f64,
) -> (&'static str, &'static str, bool) {
    if !config.prefer_post_only && edge >= config.taker_allowed_only_if_edge_above {
        ("market", "FAK", false)
    } else {
        ("limit", "GTC", true)
    }
}

fn confidence_ladder_dominant_buy_decision(
    config: &ConfidenceLadderConfig,
    state: &TradeBuilderConfidenceLadderState,
    model: &ConfidenceLadderModel,
    up: &ConfidenceLadderQuote,
    down: &ConfidenceLadderQuote,
    market_slug: &str,
) -> Option<ConfidenceLadderDecision> {
    if !confidence_ladder_in_entry_window(config, market_slug)
        || confidence_ladder_is_late_for_dominant_buy(config, market_slug)
        || model.chop > config.chop_probability_max
        || confidence_ladder_side_flip_disabled(config, state)
    {
        return None;
    }
    let dominant_side = model.dominant_side?;
    let quote = confidence_ladder_side_quote(dominant_side, up, down);
    let price = confidence_ladder_quote_price(quote)?;
    let spread = confidence_ladder_spread(quote)?;
    if spread > config.max_spread {
        return None;
    }
    let probability = confidence_ladder_side_probability(model, dominant_side);
    let all_in = confidence_ladder_all_in_cost(price, config, true);
    let edge = probability - all_in;
    let (intent, quantity, min_edge) = if state.buy_count <= 0 {
        if price < config.probe_price_min || price > config.probe_price_max {
            return None;
        }
        ("probe", config.base_probe_shares, 0.02)
    } else {
        let band = confidence_ladder_band_for_price(config, price)?;
        ("ladder_add", band.add_shares, band.min_model_edge)
    };
    if edge + 0.000001 < min_edge {
        return None;
    }
    if !confidence_ladder_risk_allows_buy(config, state, dominant_side, quantity, all_in) {
        return None;
    }
    let (execution_mode, order_type, post_only) = confidence_ladder_execution_shape(config, edge);
    Some(ConfidenceLadderDecision {
        ladder_side: dominant_side,
        intent,
        quantity,
        price,
        all_in_cost: all_in,
        model_probability: probability,
        edge,
        min_edge,
        post_only,
        order_type,
        execution_mode,
        diagnostics: json!({
            "decision": intent,
            "side": dominant_side,
            "price": price,
            "spread": spread,
            "probability": probability,
            "all_in_cost": all_in,
            "edge": edge,
            "min_edge": min_edge,
            "projected_worst_case_pnl": confidence_ladder_projected_worst_case_pnl(state, dominant_side, quantity, all_in),
            "projected_total_cost": confidence_ladder_projected_total_cost(state, quantity, all_in),
        }),
    })
}

fn confidence_ladder_hedge_decision(
    config: &ConfidenceLadderConfig,
    state: &TradeBuilderConfidenceLadderState,
    model: &ConfidenceLadderModel,
    up: &ConfidenceLadderQuote,
    down: &ConfidenceLadderQuote,
) -> Option<ConfidenceLadderDecision> {
    let held_side = confidence_ladder_held_side(state)?;
    let opposite_side = confidence_ladder_opposite_side(held_side);
    let held_qty = confidence_ladder_state_qty(state, held_side);
    let opposite_qty = confidence_ladder_state_qty(state, opposite_side);
    let held_avg_cost = confidence_ladder_state_avg_cost(state, held_side)?;
    let opposite_quote = confidence_ladder_side_quote(opposite_side, up, down);
    let price = confidence_ladder_quote_price(opposite_quote)?;
    let spread = confidence_ladder_spread(opposite_quote)?;
    if price > config.damage_control_price_max || spread > config.max_spread {
        return None;
    }
    let opposite_all_in = confidence_ladder_all_in_cost(price, config, true);
    let pair_cost = held_avg_cost + opposite_all_in;
    let reversal_probability = confidence_ladder_side_probability(model, opposite_side);
    let edge = reversal_probability - opposite_all_in;
    let target = if pair_cost <= config.strong_profit_lock_pair_cost {
        confidence_ladder_strong_profit_lock_quantity(
            config,
            state,
            held_side,
            opposite_side,
            opposite_all_in,
        )
        .map(|quantity| ("profit_lock_strong", quantity, "full_lock"))
    } else if pair_cost <= config.profit_lock_pair_cost_max {
        confidence_ladder_profit_lock_quantity(
            config,
            state,
            held_side,
            opposite_side,
            opposite_all_in,
        )
        .map(|plan| ("hedge_lock", plan.quantity, plan.reason))
    } else if price >= config.damage_control_price_min
        && state.worst_case_pnl < -config.max_loss_per_market_usdc
    {
        confidence_ladder_loss_cap_quantity(
            config,
            state,
            held_side,
            opposite_side,
            opposite_all_in,
            -config.max_loss_per_market_usdc,
        )
        .map(|plan| ("damage_control_hedge", plan.quantity, plan.reason))
    } else {
        None
    };
    let (intent, quantity, hedge_reason) = target?;
    if quantity <= 0.0 {
        return None;
    }
    if !confidence_ladder_risk_allows_buy(config, state, opposite_side, quantity, opposite_all_in) {
        return None;
    }
    let (execution_mode, order_type, post_only) = confidence_ladder_execution_shape(config, edge);
    Some(ConfidenceLadderDecision {
        ladder_side: opposite_side,
        intent,
        quantity,
        price,
        all_in_cost: opposite_all_in,
        model_probability: reversal_probability,
        edge,
        min_edge: config.hedge_min_reversal_edge,
        post_only,
        order_type,
        execution_mode,
        diagnostics: json!({
            "decision": intent,
            "held_side": held_side,
            "opposite_side": opposite_side,
            "held_qty": held_qty,
            "opposite_qty": opposite_qty,
            "hedge_reason": hedge_reason,
            "price": price,
            "spread": spread,
            "held_avg_cost": held_avg_cost,
            "opposite_all_in": opposite_all_in,
            "pair_cost": pair_cost,
            "strong_profit_lock": pair_cost <= config.strong_profit_lock_pair_cost,
            "damage_control": intent == "damage_control_hedge",
            "reversal_probability": reversal_probability,
            "edge": edge,
            "projected_worst_case_pnl": confidence_ladder_projected_worst_case_pnl(state, opposite_side, quantity, opposite_all_in),
            "projected_total_cost": confidence_ladder_projected_total_cost(state, quantity, opposite_all_in),
        }),
    })
}

async fn confidence_ladder_resolve_side_quote(
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    step: &TradeFlowRunStep,
    ladder_side: &'static str,
    token_id: &str,
    outcome_label: &str,
) -> ConfidenceLadderQuote {
    let quote = resolve_pair_lock_action_candidate_quote(
        ws,
        client,
        step,
        token_id,
        outcome_label,
        step_input_f64(step, &["currentPrice", "price", "wsPrice"]),
    )
    .await;
    let live_snapshot = ws
        .inspect_market_snapshot(token_id, PAIR_LOCK_QUOTE_FRESHNESS_MAX_AGE_MS)
        .await
        .snapshot;
    ConfidenceLadderQuote {
        ladder_side,
        token_id: token_id.to_string(),
        outcome_label: outcome_label.to_string(),
        best_bid: quote.best_bid,
        best_ask: quote.best_ask,
        best_bid_size: live_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.best_bid_size),
        best_ask_size: live_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.best_ask_size),
        last_trade_price: quote.last_trade_price,
        current_price: quote.current_price,
        snapshot: quote.quote_snapshot_used,
    }
}

async fn confidence_ladder_resolve_quotes(
    cfg: &AppConfig,
    client: &dyn OrderExecutor,
    ws: &ClobWsClient,
    step: &TradeFlowRunStep,
    context: &Value,
    market_slug: &str,
    trigger_key: &str,
) -> Result<(ConfidenceLadderQuote, ConfidenceLadderQuote)> {
    let resolved =
        resolve_trade_builder_pair_lock_yes_no_tokens(cfg, market_slug, trigger_key, context)
            .await?;
    let (up_label, down_label) = pair_lock_monitor_outcome_labels(Some(market_slug));
    let up = confidence_ladder_resolve_side_quote(
        ws,
        client,
        step,
        "up",
        &resolved.yes_token_id,
        up_label,
    )
    .await;
    let down = confidence_ladder_resolve_side_quote(
        ws,
        client,
        step,
        "down",
        &resolved.no_token_id,
        down_label,
    )
    .await;
    Ok((up, down))
}

fn confidence_ladder_output_skipped(
    node: &TradeFlowNode,
    market_slug: &str,
    reason: &str,
    details: Value,
) -> TradeFlowNodeExecution {
    TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "mode": ACTION_PLACE_ORDER_MODE_CONFIDENCE_LADDER_HEDGE_LOCK_V1,
            "market_slug": market_slug,
            "skipped": true,
            "reason": reason,
            "details": details,
        }),
        routes: Vec::new(),
        repeat_at: None,
        repeat_idempotency_key: None,
    }
}

fn build_confidence_ladder_child_node(
    node: &TradeFlowNode,
    market_slug: &str,
    quote: &ConfidenceLadderQuote,
    decision: &ConfidenceLadderDecision,
    trigger_node_key: &str,
    model: &ConfidenceLadderModel,
    state: &TradeBuilderConfidenceLadderState,
) -> TradeFlowNode {
    let mut config = node.config.as_object().cloned().unwrap_or_default();
    config.insert("mode".to_string(), json!(ACTION_PLACE_ORDER_MODE_SINGLE));
    config.insert("kind".to_string(), json!("immediate"));
    config.insert("side".to_string(), json!("buy"));
    config.insert("executionMode".to_string(), json!(decision.execution_mode));
    config.insert("orderType".to_string(), json!(decision.order_type));
    config.insert("postOnly".to_string(), json!(decision.post_only));
    config.insert("sizeMode".to_string(), json!("shares"));
    config.insert("targetQty".to_string(), json!(decision.quantity));
    config.insert("marketSlug".to_string(), json!(market_slug));
    config.insert("tokenId".to_string(), json!(&quote.token_id));
    config.insert("outcomeLabel".to_string(), json!(&quote.outcome_label));
    config.insert("reentryTriggerNodeKey".to_string(), json!(trigger_node_key));
    config.insert("maxPriceCent".to_string(), json!(decision.price * 100.0));
    config.insert("minPriceDistanceCent".to_string(), json!(0.1));
    config.insert("tpEnabled".to_string(), json!(false));
    config.insert("slEnabled".to_string(), json!(false));
    config.insert("ptbStopLossEnabled".to_string(), json!(false));
    config.insert("tpRules".to_string(), json!([]));
    config.insert("slRules".to_string(), json!([]));
    config.insert("ptbStopLossRules".to_string(), json!([]));
    config.insert(CONFIDENCE_LADDER_ORDER_MARKER_KEY.to_string(), json!(true));
    config.insert(CONFIDENCE_LADDER_ROOT_NODE_KEY.to_string(), json!(node.key));
    config.insert(
        CONFIDENCE_LADDER_SIDE_KEY.to_string(),
        json!(decision.ladder_side),
    );
    config.insert(
        CONFIDENCE_LADDER_INTENT_KEY.to_string(),
        json!(decision.intent),
    );
    config.insert(
        CONFIDENCE_LADDER_MODEL_PROBABILITY_KEY.to_string(),
        json!(decision.model_probability),
    );
    config.insert(CONFIDENCE_LADDER_EDGE_KEY.to_string(), json!(decision.edge));
    config.insert(
        "confidenceLadderDiagnostics".to_string(),
        json!({
            "decision": decision.diagnostics,
            "model": model.diagnostics,
            "state": {
                "up_qty": state.up_qty,
                "down_qty": state.down_qty,
                "total_cost_usdc": state.total_cost_usdc,
                "up_avg_cost": state.up_avg_cost,
                "down_avg_cost": state.down_avg_cost,
                "worst_case_pnl": state.worst_case_pnl,
                "side_switch_count": state.side_switch_count,
            },
            "quote": {
                "side": quote.ladder_side,
                "best_bid": quote.best_bid,
                "best_ask": quote.best_ask,
                "best_bid_size": quote.best_bid_size,
                "best_ask_size": quote.best_ask_size,
                "last_trade_price": quote.last_trade_price,
                "current_price": quote.current_price,
            }
        }),
    );
    TradeFlowNode {
        key: node.key.clone(),
        node_type: node.node_type.clone(),
        config: Value::Object(config),
    }
}

async fn execute_action_place_order_confidence_ladder(
    repo: &PostgresRepository,
    run_id: i64,
    cfg: &AppConfig,
    limits: &RiskLimits,
    policy: &impl RiskPolicy,
    client: Option<&dyn OrderExecutor>,
    ws: &ClobWsClient,
    run: &TradeFlowRun,
    step: &TradeFlowRunStep,
    node: &TradeFlowNode,
    graph: &TradeFlowGraphRuntime,
    context: &mut Value,
) -> Result<TradeFlowNodeExecution> {
    let market_slug = confidence_ladder_market_slug(node, step, context)?;
    let trigger_key = confidence_ladder_binding_trigger_key(node, graph)?;
    let Some(client) = client else {
        return Ok(confidence_ladder_output_skipped(
            node,
            &market_slug,
            "missing_order_executor",
            json!({}),
        ));
    };
    let config = parse_confidence_ladder_config(node)?;
    let Some(lock) = repo
        .try_acquire_trade_builder_confidence_ladder_lock(
            run.user_id,
            run.definition_id,
            &node.key,
            &market_slug,
        )
        .await?
    else {
        return Ok(confidence_ladder_output_skipped(
            node,
            &market_slug,
            "execution_lock_busy",
            json!({}),
        ));
    };
    let execution = async {
        if repo
            .has_active_trade_builder_confidence_ladder_order(
                run.user_id,
                Some(run.definition_id),
                &node.key,
                &market_slug,
            )
            .await?
        {
            return Ok(confidence_ladder_output_skipped(
                node,
                &market_slug,
                "active_order_exists",
                json!({}),
            ));
        }
        let state = repo
            .load_trade_builder_confidence_ladder_state(
                run.user_id,
                Some(run.definition_id),
                &node.key,
                &market_slug,
            )
            .await?;
        let (up_quote, down_quote) = confidence_ladder_resolve_quotes(
            cfg,
            client,
            ws,
            step,
            context,
            &market_slug,
            &trigger_key,
        )
        .await?;
        let model = confidence_ladder_heuristic_model(
            &config,
            &state,
            &up_quote,
            &down_quote,
            step,
            context,
        );
        let hedge_decision =
            confidence_ladder_hedge_decision(&config, &state, &model, &up_quote, &down_quote);
        let stop_adding = model.dominant_side.is_none()
            || model.chop > config.chop_probability_max
            || confidence_ladder_side_flip_disabled(&config, &state)
            || confidence_ladder_is_late_for_dominant_buy(&config, &market_slug);
        let decision = if stop_adding {
            hedge_decision
        } else {
            confidence_ladder_dominant_buy_decision(
                &config,
                &state,
                &model,
                &up_quote,
                &down_quote,
                &market_slug,
            )
            .or(hedge_decision)
        };
        let Some(decision) = decision else {
            return Ok(confidence_ladder_output_skipped(
                node,
                &market_slug,
                "no_eligible_confidence_ladder_action",
                json!({
                    "model": model.diagnostics,
                    "state": {
                        "up_qty": state.up_qty,
                        "down_qty": state.down_qty,
                        "total_cost_usdc": state.total_cost_usdc,
                        "worst_case_pnl": state.worst_case_pnl,
                        "side_switch_count": state.side_switch_count,
                    },
                    "remaining_sec": confidence_ladder_remaining_sec(&market_slug),
                    "stop_adding": stop_adding,
                }),
            ));
        };
        let quote = confidence_ladder_side_quote(decision.ladder_side, &up_quote, &down_quote);
        let child_node = build_confidence_ladder_child_node(
            node,
            &market_slug,
            quote,
            &decision,
            &trigger_key,
            &model,
            &state,
        );
        let mut child_execution = execute_action_place_order(
            repo,
            run_id,
            cfg,
            limits,
            policy,
            Some(client),
            run,
            step,
            &child_node,
            graph,
            context,
        )
        .await?;
        if let Some(output) = child_execution.output.as_object_mut() {
            output.insert(
                "confidence_ladder".to_string(),
                json!({
                    "mode": ACTION_PLACE_ORDER_MODE_CONFIDENCE_LADDER_HEDGE_LOCK_V1,
                    "market_slug": market_slug,
                    "side": decision.ladder_side,
                    "intent": decision.intent,
                    "quantity": decision.quantity,
                    "price": decision.price,
                    "all_in_cost": decision.all_in_cost,
                    "model_probability": decision.model_probability,
                    "edge": decision.edge,
                    "post_only": decision.post_only,
                    "order_type": decision.order_type,
                    "execution_mode": decision.execution_mode,
                    "diagnostics": decision.diagnostics,
                }),
            );
        }
        if let Some(builder_order_id) = child_execution
            .output
            .get("builder_order_id")
            .and_then(Value::as_i64)
        {
            repo.append_trade_builder_order_event(
                builder_order_id,
                "confidence_ladder_decision",
                &json!({
                    "mode": ACTION_PLACE_ORDER_MODE_CONFIDENCE_LADDER_HEDGE_LOCK_V1,
                    "market_slug": market_slug,
                    "side": decision.ladder_side,
                    "intent": decision.intent,
                    "quantity": decision.quantity,
                    "price": decision.price,
                    "all_in_cost": decision.all_in_cost,
                    "model_probability": decision.model_probability,
                    "edge": decision.edge,
                    "model": model.diagnostics,
                }),
            )
            .await?;
        }
        Ok(child_execution)
    }
    .await;
    lock.release().await;
    execution
}

fn confidence_ladder_marker_config(payload: &Value) -> Option<&Value> {
    payload
        .pointer("/node_snapshot/action_node/config")
        .filter(|config| {
            config
                .get(CONFIDENCE_LADDER_ORDER_MARKER_KEY)
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
}

fn confidence_ladder_marker_string(config: &Value, key: &str) -> Option<String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn confidence_ladder_marker_f64(config: &Value, key: &str) -> Option<f64> {
    config.get(key).and_then(value_as_f64)
}

fn confidence_ladder_marker_side(config: &Value, outcome_label: &str) -> Option<String> {
    confidence_ladder_marker_string(config, CONFIDENCE_LADDER_SIDE_KEY).or_else(|| {
        if normalize_pair_lock_binary_outcome(outcome_label) == Some("no") {
            Some("down".to_string())
        } else {
            Some("up".to_string())
        }
    })
}

async fn maybe_record_confidence_ladder_fill(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    parent_order: Option<&TradeBuilderOrder>,
    flow_created_payload: Option<&Value>,
    fill_qty: f64,
    execution_price: f64,
) -> Result<()> {
    let Some(marker_config) = flow_created_payload.and_then(confidence_ladder_marker_config) else {
        return Ok(());
    };
    let root_node_key =
        confidence_ladder_marker_string(marker_config, CONFIDENCE_LADDER_ROOT_NODE_KEY)
            .unwrap_or_else(|| {
                order
                    .origin_flow_node_key
                    .clone()
                    .unwrap_or_else(|| order.market_slug.clone())
            });
    let Some(ladder_side) = confidence_ladder_marker_side(marker_config, &order.outcome_label)
    else {
        return Ok(());
    };
    if ladder_side != "up" && ladder_side != "down" {
        return Ok(());
    }
    let applied = repo
        .record_trade_builder_confidence_ladder_fill(
            &TradeBuilderConfidenceLadderFillInput {
                user_id: order.user_id,
                flow_definition_id: order.origin_flow_definition_id,
                flow_run_id: order.origin_flow_run_id,
                root_flow_node_key: root_node_key,
                market_slug: order.market_slug.clone(),
                token_id: order.token_id.clone(),
                outcome_label: order.outcome_label.clone(),
                ladder_side,
                intent: confidence_ladder_marker_string(marker_config, CONFIDENCE_LADDER_INTENT_KEY)
                    .unwrap_or_else(|| "unknown".to_string()),
                order_side: order.side.clone(),
                builder_order_id: order.id,
                parent_builder_order_id: order.parent_order_id.or_else(|| parent_order.map(|parent| parent.id)),
                quantity: fill_qty.max(0.0),
                execution_price: clamp_probability(execution_price.max(0.0)),
                notional_usdc: (fill_qty.max(0.0) * execution_price.max(0.0)).max(0.0),
                model_probability: confidence_ladder_marker_f64(
                    marker_config,
                    CONFIDENCE_LADDER_MODEL_PROBABILITY_KEY,
                ),
                edge: confidence_ladder_marker_f64(marker_config, CONFIDENCE_LADDER_EDGE_KEY),
                payload_json: json!({
                    "builder_order_id": order.id,
                    "parent_builder_order_id": order.parent_order_id.or_else(|| parent_order.map(|parent| parent.id)),
                    "flow_created": flow_created_payload,
                }),
            },
        )
        .await?;
    if applied {
        repo.append_trade_builder_order_event(
            order.id,
            "confidence_ladder_fill_recorded",
            &json!({
                "order_side": order.side,
                "quantity": fill_qty,
                "execution_price": execution_price,
            }),
        )
        .await?;
    }
    Ok(())
}
