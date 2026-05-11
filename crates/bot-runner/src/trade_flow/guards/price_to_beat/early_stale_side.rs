use super::PriceToBeatGuardEvaluation;
use crate::trade_flow::guards::cex_microstructure::{
    ensure_cex_microstructure_started, get_cex_microstructure_snapshot, CexConsensusSnapshot,
    CexMicrostructureSnapshotConfig,
};
use chrono::{Duration as ChronoDuration, Utc};
use serde_json::{json, Value};

const DEFAULT_MIN_ELAPSED_SEC: f64 = 5.0;
const DEFAULT_MAX_ELAPSED_SEC: f64 = 30.0;
const DEFAULT_MIN_ASK: f64 = 0.50;
const DEFAULT_MAX_ASK: f64 = 0.65;
const DEFAULT_MIN_FAIR_EDGE: f64 = 0.10;
const DEFAULT_MIN_FINAL_Q: f64 = 0.65;
const DEFAULT_MIN_EV_PROB_BUFFER: f64 = 0.02;
const DEFAULT_MAX_REQUIRED_MOVE_USD: f64 = 45.0;
const DEFAULT_MIN_WINNING_BUFFER_USD: f64 = 18.0;
const DEFAULT_MAX_NORMALIZED_SOURCE_SKEW_USD: f64 = 15.0;
const DEFAULT_FEE_RATE: f64 = 0.018;

#[derive(Debug, Clone)]
pub(crate) struct EarlyStaleSideConfig {
    pub(crate) enabled: bool,
    pub(crate) min_elapsed_sec: f64,
    pub(crate) max_elapsed_sec: f64,
    pub(crate) min_ask: f64,
    pub(crate) max_ask: f64,
    pub(crate) min_fair_edge: f64,
    pub(crate) min_final_q: f64,
    pub(crate) min_ev_prob_buffer: f64,
    pub(crate) max_required_move_usd: f64,
    pub(crate) min_winning_buffer_usd: f64,
    pub(crate) max_normalized_source_skew_usd: f64,
    pub(crate) cex: CexMicrostructureSnapshotConfig,
}

impl EarlyStaleSideConfig {
    pub(crate) fn from_node(node: &crate::TradeFlowNode) -> Self {
        Self {
            enabled: crate::node_config_bool(node, "priceToBeatEarlyStaleSideEnabled")
                .unwrap_or(false),
            min_elapsed_sec: cfg_f64(
                node,
                "priceToBeatEarlyStaleMinElapsedSec",
                DEFAULT_MIN_ELAPSED_SEC,
            ),
            max_elapsed_sec: cfg_f64(
                node,
                "priceToBeatEarlyStaleMaxElapsedSec",
                DEFAULT_MAX_ELAPSED_SEC,
            ),
            min_ask: cfg_cent(
                node,
                "priceToBeatEarlyStaleMinAskCent",
                "priceToBeatEarlyStaleMinAsk",
                DEFAULT_MIN_ASK,
            ),
            max_ask: cfg_cent(
                node,
                "priceToBeatEarlyStaleMaxAskCent",
                "priceToBeatEarlyStaleMaxAsk",
                DEFAULT_MAX_ASK,
            ),
            min_fair_edge: cfg_f64(
                node,
                "priceToBeatEarlyStaleMinFairEdge",
                DEFAULT_MIN_FAIR_EDGE,
            ),
            min_final_q: cfg_f64(node, "priceToBeatEarlyStaleMinFinalQ", DEFAULT_MIN_FINAL_Q),
            min_ev_prob_buffer: cfg_f64(
                node,
                "priceToBeatEarlyStaleMinEvProbBuffer",
                DEFAULT_MIN_EV_PROB_BUFFER,
            ),
            max_required_move_usd: cfg_f64(
                node,
                "priceToBeatEarlyStaleMaxRequiredMoveUsd",
                DEFAULT_MAX_REQUIRED_MOVE_USD,
            ),
            min_winning_buffer_usd: cfg_f64(
                node,
                "priceToBeatEarlyStaleMinWinningBufferUsd",
                DEFAULT_MIN_WINNING_BUFFER_USD,
            ),
            max_normalized_source_skew_usd: cfg_f64(
                node,
                "priceToBeatEarlyStaleMaxNormalizedSourceSkewUsd",
                DEFAULT_MAX_NORMALIZED_SOURCE_SKEW_USD,
            ),
            cex: CexMicrostructureSnapshotConfig {
                impulse_window_ms: cfg_i64(node, "priceToBeatEarlyStaleImpulseWindowMs", 15_000),
                min_move_usd: cfg_f64(node, "priceToBeatEarlyStaleMinMoveUsd", 12.0),
                min_velocity_usd_per_sec: cfg_f64(
                    node,
                    "priceToBeatEarlyStaleMinVelocityUsdPerSec",
                    0.50,
                ),
                min_taker_imbalance: cfg_f64(node, "priceToBeatEarlyStaleMinTakerImbalance", 0.58),
                source_skew_baseline_window_ms: cfg_i64(
                    node,
                    "priceToBeatEarlyStaleSourceSkewBaselineWindowMs",
                    60_000,
                ),
                max_book_stale_ms: cfg_i64(node, "priceToBeatEarlyStaleMaxBookStaleMs", 750),
                max_trade_stale_ms: cfg_i64(node, "priceToBeatEarlyStaleMaxTradeStaleMs", 1_000),
                max_ticker_stale_ms: cfg_i64(node, "priceToBeatEarlyStaleMaxTickerStaleMs", 750),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EarlyStaleSideEvaluation {
    pub(crate) passed: bool,
    pub(crate) reason_code: &'static str,
    pub(crate) reason_detail: Option<String>,
    pub(crate) value: Value,
}

pub(crate) fn apply_action_place_order_early_stale_side_guard(
    node: &crate::TradeFlowNode,
    market_slug: &str,
    outcome_label: &str,
    evaluation: &mut PriceToBeatGuardEvaluation,
) {
    let config = EarlyStaleSideConfig::from_node(node);
    if !config.enabled {
        return;
    }
    if !evaluation.passed {
        return;
    }

    let guard = evaluate_action_place_order_early_stale_side(
        market_slug,
        outcome_label,
        evaluation,
        &config,
    );
    evaluation.early_stale_side = Some(guard.value.clone());
    if !guard.passed {
        evaluation.passed = false;
        evaluation.reason_code = guard.reason_code.to_string();
        evaluation.reason_detail = guard.reason_detail;
    }
}

fn evaluate_action_place_order_early_stale_side(
    market_slug: &str,
    outcome_label: &str,
    evaluation: &PriceToBeatGuardEvaluation,
    config: &EarlyStaleSideConfig,
) -> EarlyStaleSideEvaluation {
    let Some(asset) = evaluation.asset.as_deref().map(str::to_ascii_lowercase) else {
        return block(
            "early_stale_side_missing_asset",
            None,
            base_value(market_slug, outcome_label),
        );
    };
    ensure_cex_microstructure_started(&asset);
    let cex = match get_cex_microstructure_snapshot(&asset, &config.cex) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            return block(
                "early_stale_side_cex_unavailable",
                Some(err.to_string()),
                base_value(market_slug, outcome_label),
            );
        }
    };
    let input = match EarlyStaleSideInput::from_guard(market_slug, outcome_label, evaluation) {
        Some(input) => input,
        None => {
            return block_with_value(
                "early_stale_side_missing_probability",
                Some("settlement_prob_estimate, selected_ask, or strike unavailable".to_string()),
                base_value(market_slug, outcome_label).with_cex(&cex),
            );
        }
    };
    evaluate_early_stale_side(input, config, cex)
}

#[derive(Debug, Clone)]
struct EarlyStaleSideInput {
    market_slug: String,
    selected_outcome: &'static str,
    selected_side: &'static str,
    selected_ask: f64,
    selected_mid: Option<f64>,
    settlement_prob_estimate: f64,
    strike: f64,
    fee_rate: f64,
    elapsed_sec: f64,
    remaining_sec: f64,
}

impl EarlyStaleSideInput {
    fn from_guard(
        market_slug: &str,
        outcome_label: &str,
        evaluation: &PriceToBeatGuardEvaluation,
    ) -> Option<Self> {
        let (selected_outcome, selected_side) = normalize_outcome(outcome_label)?;
        let iv = evaluation.iv_mismatch_edge.as_ref()?;
        let selected_ask = json_f64(iv, "selected_ask").or_else(|| json_f64(iv, "ask"))?;
        let settlement_prob_estimate = json_f64(iv, "q_final").or_else(|| json_f64(iv, "q"))?;
        let strike = evaluation.price_to_beat?;
        let (elapsed_sec, remaining_sec) = market_elapsed_remaining_sec(market_slug)?;
        Some(Self {
            market_slug: market_slug.to_string(),
            selected_outcome,
            selected_side,
            selected_ask,
            selected_mid: json_f64(iv, "selected_mid"),
            settlement_prob_estimate,
            strike,
            fee_rate: json_f64(iv, "fee_rate")
                .unwrap_or(DEFAULT_FEE_RATE)
                .max(0.0),
            elapsed_sec,
            remaining_sec,
        })
    }
}

fn evaluate_early_stale_side(
    input: EarlyStaleSideInput,
    config: &EarlyStaleSideConfig,
    cex: CexConsensusSnapshot,
) -> EarlyStaleSideEvaluation {
    let mut value = base_value(&input.market_slug, input.selected_outcome).0;
    append_input_fields(&mut value, &input);
    append_cex_fields(&mut value, &cex);

    if input.elapsed_sec < config.min_elapsed_sec || input.elapsed_sec > config.max_elapsed_sec {
        return block_with_value(
            "early_stale_side_time_window_block",
            Some(format!(
                "elapsed_sec={} outside {}..={}",
                input.elapsed_sec, config.min_elapsed_sec, config.max_elapsed_sec
            )),
            value,
        );
    }
    if input.selected_ask < config.min_ask || input.selected_ask > config.max_ask {
        return block_with_value(
            "early_stale_side_price_band_block",
            Some(format!(
                "selected_ask={} outside {}..={}",
                input.selected_ask, config.min_ask, config.max_ask
            )),
            value,
        );
    }
    if cex.normalized_source_skew_usd.abs() > config.max_normalized_source_skew_usd {
        return block_with_value(
            "early_stale_side_source_skew_block",
            Some(format!(
                "normalized_source_skew_usd={} max={}",
                cex.normalized_source_skew_usd, config.max_normalized_source_skew_usd
            )),
            value,
        );
    }
    if cex.consensus_side != Some(input.selected_side) {
        return block_with_value(
            "early_stale_side_direction_mismatch",
            Some(format!(
                "cex_consensus_side={:?} selected_side={}",
                cex.consensus_side, input.selected_side
            )),
            value,
        );
    }

    let required_q = required_probability(
        input.selected_ask,
        config.min_final_q,
        config.min_fair_edge,
        input.fee_rate,
        config.min_ev_prob_buffer,
    );
    let fair_edge = input.settlement_prob_estimate - input.selected_ask;
    insert(&mut value, "required_q", json!(required_q));
    insert(&mut value, "fair_edge", json!(fair_edge));
    if input.settlement_prob_estimate < required_q {
        return block_with_value(
            "early_stale_side_fair_edge_block",
            Some(format!(
                "settlement_prob_estimate={} required_q={}",
                input.settlement_prob_estimate, required_q
            )),
            value,
        );
    }

    let geometry = side_geometry(input.selected_side, cex.spot_mid, input.strike);
    insert(
        &mut value,
        "spot_vs_strike_usd",
        json!(cex.spot_mid - input.strike),
    );
    insert(
        &mut value,
        "required_move_usd",
        json!(geometry.required_move),
    );
    insert(
        &mut value,
        "winning_buffer_usd",
        json!(geometry.winning_buffer),
    );

    if let Some(required_move) = geometry.required_move {
        if required_move > config.max_required_move_usd {
            return block_with_value(
                "early_stale_side_required_move_block",
                Some(format!(
                    "required_move_usd={required_move} max={}",
                    config.max_required_move_usd
                )),
                value,
            );
        }
    } else if let Some(winning_buffer) = geometry.winning_buffer {
        if winning_buffer < config.min_winning_buffer_usd {
            return block_with_value(
                "early_stale_side_winning_buffer_block",
                Some(format!(
                    "winning_buffer_usd={winning_buffer} min={}",
                    config.min_winning_buffer_usd
                )),
                value,
            );
        }
    } else {
        return block_with_value("early_stale_side_geometry_block", None, value);
    }

    insert(&mut value, "decision", json!("pass"));
    insert(&mut value, "block_reason", Value::Null);
    EarlyStaleSideEvaluation {
        passed: true,
        reason_code: "early_stale_side_pass",
        reason_detail: None,
        value,
    }
}

fn required_probability(
    selected_ask: f64,
    min_final_q: f64,
    min_fair_edge: f64,
    fee_rate: f64,
    min_ev_prob_buffer: f64,
) -> f64 {
    min_final_q
        .max(selected_ask + min_fair_edge)
        .max(selected_ask * (1.0 + fee_rate) + min_ev_prob_buffer)
}

#[derive(Debug, Clone, Copy)]
struct SideGeometry {
    required_move: Option<f64>,
    winning_buffer: Option<f64>,
}

fn side_geometry(side: &str, spot: f64, strike: f64) -> SideGeometry {
    match side {
        "up" if spot >= strike => SideGeometry {
            required_move: None,
            winning_buffer: Some(spot - strike),
        },
        "up" => SideGeometry {
            required_move: Some(strike - spot),
            winning_buffer: None,
        },
        "down" if spot <= strike => SideGeometry {
            required_move: None,
            winning_buffer: Some(strike - spot),
        },
        _ => SideGeometry {
            required_move: Some(spot - strike),
            winning_buffer: None,
        },
    }
}

fn market_elapsed_remaining_sec(market_slug: &str) -> Option<(f64, f64)> {
    let scope = crate::find_updown_scope_by_slug(market_slug)?;
    let start = crate::MarketCycleId(market_slug.to_string()).start_time()?;
    let end = start + ChronoDuration::seconds(crate::updown_scope_window_seconds(scope));
    let now = Utc::now();
    Some((
        now.signed_duration_since(start).num_milliseconds() as f64 / 1_000.0,
        end.signed_duration_since(now).num_milliseconds() as f64 / 1_000.0,
    ))
}

fn normalize_outcome(outcome_label: &str) -> Option<(&'static str, &'static str)> {
    match outcome_label.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "long" | "bull" => Some(("Up", "up")),
        "no" | "down" | "short" | "bear" => Some(("Down", "down")),
        _ => None,
    }
}

fn json_f64(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(crate::value_as_f64)
}

fn cfg_f64(node: &crate::TradeFlowNode, key: &str, default: f64) -> f64 {
    crate::node_config_f64(node, key).unwrap_or(default)
}

fn cfg_i64(node: &crate::TradeFlowNode, key: &str, default: i64) -> i64 {
    crate::node_config_i64(node, key).unwrap_or(default)
}

fn cfg_cent(node: &crate::TradeFlowNode, cent_key: &str, raw_key: &str, default: f64) -> f64 {
    crate::node_config_f64(node, cent_key)
        .map(|value| value / 100.0)
        .or_else(|| crate::node_config_f64(node, raw_key))
        .unwrap_or(default)
}

#[derive(Debug, Clone)]
struct EarlyValue(Value);

fn base_value(market_slug: &str, outcome_label: &str) -> EarlyValue {
    EarlyValue(json!({
        "guard": "early_stale_side",
        "market_id": market_slug,
        "market_slug": market_slug,
        "condition_id": market_slug,
        "selected_outcome": outcome_label,
        "decision": "block",
        "block_reason": Value::Null,
    }))
}

impl EarlyValue {
    fn with_cex(mut self, cex: &CexConsensusSnapshot) -> Value {
        append_cex_fields(&mut self.0, cex);
        self.0
    }
}

fn append_input_fields(value: &mut Value, input: &EarlyStaleSideInput) {
    insert(value, "selected_outcome", json!(input.selected_outcome));
    insert(value, "selected_side", json!(input.selected_side));
    insert(value, "elapsed_sec", json!(input.elapsed_sec));
    insert(value, "remaining_sec", json!(input.remaining_sec));
    insert(value, "strike", json!(input.strike));
    insert(value, "selected_ask", json!(input.selected_ask));
    insert(value, "selected_mid", json!(input.selected_mid));
    insert(
        value,
        "settlement_prob_estimate",
        json!(input.settlement_prob_estimate),
    );
    insert(value, "fee_rate", json!(input.fee_rate));
}

fn append_cex_fields(value: &mut Value, cex: &CexConsensusSnapshot) {
    insert(value, "cex", cex.to_value());
    insert(value, "binance_mid", json!(cex.binance.mid));
    insert(value, "coinbase_mid", json!(cex.coinbase.mid));
    insert(
        value,
        "normalized_source_skew_usd",
        json!(cex.normalized_source_skew_usd),
    );
    insert(
        value,
        "binance_velocity_15s",
        json!(cex.binance.impulse.velocity_usd_per_sec),
    );
    insert(
        value,
        "coinbase_velocity_15s",
        json!(cex.coinbase.impulse.velocity_usd_per_sec),
    );
    insert(
        value,
        "binance_taker_imbalance",
        json!(cex.binance.impulse.taker_imbalance),
    );
    insert(
        value,
        "coinbase_taker_imbalance",
        json!(cex.coinbase.impulse.taker_imbalance),
    );
    insert(value, "binance_side", json!(cex.binance.impulse.side));
    insert(value, "coinbase_side", json!(cex.coinbase.impulse.side));
    insert(value, "cex_consensus_side", json!(cex.consensus_side));
    insert(value, "absorption_flag", json!(false));
    insert(value, "source_stale", json!(false));
}

fn block(
    reason_code: &'static str,
    reason_detail: Option<String>,
    value: EarlyValue,
) -> EarlyStaleSideEvaluation {
    block_with_value(reason_code, reason_detail, value.0)
}

fn block_with_value(
    reason_code: &'static str,
    reason_detail: Option<String>,
    mut value: Value,
) -> EarlyStaleSideEvaluation {
    insert(&mut value, "decision", json!("block"));
    insert(&mut value, "block_reason", json!(reason_code));
    if let Some(detail) = reason_detail.as_ref() {
        insert(&mut value, "reason_detail", json!(detail));
    }
    EarlyStaleSideEvaluation {
        passed: false,
        reason_code,
        reason_detail,
        value,
    }
}

fn insert(value: &mut Value, key: &str, item: Value) {
    if let Some(obj) = value.as_object_mut() {
        obj.insert(key.to_string(), item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade_flow::guards::cex_microstructure::{
        CexBookSample, CexImpulseSnapshot, CexSourceSnapshot, CexTradeSample, CexVenue, TakerSide,
    };

    fn active_market_slug(elapsed_sec: i64) -> String {
        let start = Utc::now().timestamp() - elapsed_sec;
        format!("btc-updown-5m-{start}")
    }

    fn source(venue: CexVenue, mid: f64, side: &'static str) -> CexConsensusSnapshot {
        let impulse = CexConsensusSnapshot {
            asset: "btc".to_string(),
            binance: snapshot_source(CexVenue::Binance, 67_520.0, side),
            coinbase: snapshot_source(CexVenue::Coinbase, 67_519.0, side),
            consensus_side: Some(side),
            spot_mid: mid,
            source_skew_usd: 1.0,
            baseline_source_skew_usd: Some(1.0),
            normalized_source_skew_usd: 0.0,
        };
        let _ = venue;
        impulse
    }

    fn snapshot_source(venue: CexVenue, mid: f64, side: &'static str) -> CexSourceSnapshot {
        CexSourceSnapshot {
            venue,
            mid,
            bid: mid - 1.0,
            ask: mid + 1.0,
            book_staleness_ms: 100,
            trade_staleness_ms: 100,
            ticker_staleness_ms: 100,
            impulse: CexImpulseSnapshot {
                side: Some(side),
                move_usd: if side == "up" { 20.0 } else { -20.0 },
                velocity_usd_per_sec: if side == "up" { 1.5 } else { -1.5 },
                taker_imbalance: 0.66,
                trade_count: 10,
            },
        }
    }

    fn input(
        side: &'static str,
        ask: f64,
        prob: f64,
        _spot: f64,
        strike: f64,
    ) -> EarlyStaleSideInput {
        EarlyStaleSideInput {
            market_slug: active_market_slug(12),
            selected_outcome: if side == "up" { "Up" } else { "Down" },
            selected_side: side,
            selected_ask: ask,
            selected_mid: Some(ask - 0.01),
            settlement_prob_estimate: prob,
            strike,
            fee_rate: 0.018,
            elapsed_sec: 12.0,
            remaining_sec: 288.0,
        }
    }

    #[test]
    fn ask_065_q_066_blocks_even_if_min_final_q_passes() {
        let config = EarlyStaleSideConfig::from_node(&crate::TradeFlowNode {
            key: "n".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({"priceToBeatEarlyStaleSideEnabled": true}),
        });
        let result = evaluate_early_stale_side(
            input("up", 0.65, 0.66, 67_520.0, 67_500.0),
            &config,
            source(CexVenue::Binance, 67_520.0, "up"),
        );

        assert!(!result.passed, "{result:?}");
        assert_eq!(result.reason_code, "early_stale_side_fair_edge_block");
    }

    #[test]
    fn ask_060_q_070_passes_with_min_edge_010() {
        let config = EarlyStaleSideConfig::from_node(&crate::TradeFlowNode {
            key: "n".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({"priceToBeatEarlyStaleSideEnabled": true}),
        });
        let result = evaluate_early_stale_side(
            input("up", 0.60, 0.70, 67_520.0, 67_500.0),
            &config,
            source(CexVenue::Binance, 67_520.0, "up"),
        );

        assert!(result.passed, "{result:?}");
    }

    #[test]
    fn up_impulse_blocks_if_still_too_far_below_strike() {
        let config = EarlyStaleSideConfig::from_node(&crate::TradeFlowNode {
            key: "n".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({"priceToBeatEarlyStaleSideEnabled": true}),
        });
        let result = evaluate_early_stale_side(
            input("up", 0.55, 0.70, 67_420.0, 67_500.0),
            &config,
            source(CexVenue::Binance, 67_420.0, "up"),
        );

        assert!(!result.passed, "{result:?}");
        assert_eq!(result.reason_code, "early_stale_side_required_move_block");
    }

    #[test]
    fn down_impulse_blocks_if_still_too_far_above_strike() {
        let config = EarlyStaleSideConfig::from_node(&crate::TradeFlowNode {
            key: "n".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({"priceToBeatEarlyStaleSideEnabled": true}),
        });
        let result = evaluate_early_stale_side(
            input("down", 0.55, 0.70, 67_580.0, 67_500.0),
            &config,
            source(CexVenue::Binance, 67_580.0, "down"),
        );

        assert!(!result.passed, "{result:?}");
        assert_eq!(result.reason_code, "early_stale_side_required_move_block");
    }

    #[test]
    fn buffered_winning_side_passes() {
        let config = EarlyStaleSideConfig::from_node(&crate::TradeFlowNode {
            key: "n".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({"priceToBeatEarlyStaleSideEnabled": true}),
        });
        let result = evaluate_early_stale_side(
            input("down", 0.55, 0.70, 67_470.0, 67_500.0),
            &config,
            source(CexVenue::Coinbase, 67_470.0, "down"),
        );

        assert!(result.passed, "{result:?}");
    }

    #[test]
    fn replay_fixture_up_passes_and_sibling_down_blocks() {
        let config = EarlyStaleSideConfig::from_node(&crate::TradeFlowNode {
            key: "n".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({"priceToBeatEarlyStaleSideEnabled": true}),
        });
        let cex = source(CexVenue::Binance, 67_520.0, "up");
        let up = evaluate_early_stale_side(
            input("up", 0.56, 0.72, 67_520.0, 67_500.0),
            &config,
            cex.clone(),
        );
        let down =
            evaluate_early_stale_side(input("down", 0.56, 0.72, 67_520.0, 67_500.0), &config, cex);

        assert!(up.passed, "{up:?}");
        assert!(!down.passed, "{down:?}");
        assert_eq!(down.reason_code, "early_stale_side_direction_mismatch");
    }

    #[test]
    fn fee_adjusted_required_q_blocks_borderline_trade() {
        let required = required_probability(0.65, 0.65, 0.00, 0.018, 0.02);

        assert!(required > 0.681);
    }

    #[allow(dead_code)]
    fn _compile_exported_test_types(_: CexBookSample, _: CexTradeSample, _: TakerSide) {}
}
