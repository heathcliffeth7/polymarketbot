use super::iv_mismatch_depth::PriceToBeatIvDepthEvaluation;
use super::signal_formula::signal_formula_taker_fee;
use serde_json::{json, Map, Value};

const HIGH_DISLOCATION_Q: f64 = 0.95;
const HIGH_DISLOCATION_GAP: f64 = 0.20;
const DEFAULT_EXECUTION_VWAP_MAX_SLIPPAGE: f64 = 0.02;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvExecutionVwapConfig {
    pub(crate) enabled: bool,
    pub(crate) required_on_high_dislocation: bool,
    pub(crate) limit_by_vwap_enabled: bool,
    pub(crate) max_slippage: f64,
}

impl Default for PriceToBeatIvExecutionVwapConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            required_on_high_dislocation: false,
            limit_by_vwap_enabled: false,
            max_slippage: DEFAULT_EXECUTION_VWAP_MAX_SLIPPAGE,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvExecutionVwapEvaluation {
    pub(crate) enabled: bool,
    pub(crate) time_rule_price_blocked: bool,
    pub(crate) time_rule_max_price: Option<f64>,
    pub(crate) model_ask: Option<f64>,
    pub(crate) execution_best_ask: Option<f64>,
    pub(crate) execution_vwap: Option<f64>,
    pub(crate) qty_requested: Option<f64>,
    pub(crate) qty_available: Option<f64>,
    pub(crate) depth_coverage_ratio: Option<f64>,
    pub(crate) depth_levels_used: Option<usize>,
    pub(crate) book_depth_ok: Option<bool>,
    pub(crate) fallback_reason: Option<&'static str>,
    pub(crate) cost_source: Option<&'static str>,
    pub(crate) raw_execution_cost: Option<f64>,
    pub(crate) fee_buffer: Option<f64>,
    pub(crate) safety_buffer: Option<f64>,
    pub(crate) dynamic_threshold: Option<f64>,
    pub(crate) execution_cost_for_edge: Option<f64>,
    pub(crate) edge_margin: Option<f64>,
    pub(crate) submit_limit_price: Option<f64>,
    pub(crate) limit_by_vwap_action: &'static str,
    pub(crate) block_reason: Option<&'static str>,
}

impl Default for PriceToBeatIvExecutionVwapEvaluation {
    fn default() -> Self {
        Self {
            enabled: false,
            time_rule_price_blocked: false,
            time_rule_max_price: None,
            model_ask: None,
            execution_best_ask: None,
            execution_vwap: None,
            qty_requested: None,
            qty_available: None,
            depth_coverage_ratio: None,
            depth_levels_used: None,
            book_depth_ok: None,
            fallback_reason: None,
            cost_source: None,
            raw_execution_cost: None,
            fee_buffer: None,
            safety_buffer: None,
            dynamic_threshold: None,
            execution_cost_for_edge: None,
            edge_margin: None,
            submit_limit_price: None,
            limit_by_vwap_action: "disabled",
            block_reason: None,
        }
    }
}

impl PriceToBeatIvExecutionVwapEvaluation {
    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert(
            "execution_vwap_guard_enabled".to_string(),
            json!(self.enabled),
        );
        obj.insert(
            "execution_limit_by_vwap_enabled".to_string(),
            json!(self.limit_by_vwap_action != "disabled"),
        );
        obj.insert(
            "time_rule_price_blocked".to_string(),
            json!(self.time_rule_price_blocked),
        );
        obj.insert(
            "time_rule_max_price_cent".to_string(),
            json!(price_to_cent(self.time_rule_max_price)),
        );
        obj.insert(
            "model_ask_cent".to_string(),
            json!(price_to_cent(self.model_ask)),
        );
        obj.insert(
            "execution_best_ask_cent".to_string(),
            json!(price_to_cent(self.execution_best_ask)),
        );
        obj.insert(
            "execution_vwap_cent".to_string(),
            json!(price_to_cent(self.execution_vwap)),
        );
        obj.insert(
            "expected_vwap_cent".to_string(),
            json!(price_to_cent(self.execution_vwap)),
        );
        obj.insert(
            "execution_vwap_qty_requested".to_string(),
            json!(self.qty_requested),
        );
        obj.insert(
            "execution_vwap_qty_available".to_string(),
            json!(self.qty_available),
        );
        obj.insert(
            "execution_vwap_depth_coverage_ratio".to_string(),
            json!(self.depth_coverage_ratio),
        );
        obj.insert(
            "execution_vwap_levels_used".to_string(),
            json!(self.depth_levels_used),
        );
        obj.insert(
            "execution_vwap_book_depth_ok".to_string(),
            json!(self.book_depth_ok),
        );
        obj.insert(
            "execution_vwap_fallback_reason".to_string(),
            json!(self.fallback_reason),
        );
        obj.insert("execution_cost_source".to_string(), json!(self.cost_source));
        obj.insert(
            "raw_execution_cost_cent".to_string(),
            json!(price_to_cent(self.raw_execution_cost)),
        );
        obj.insert(
            "fee_buffer_cent".to_string(),
            json!(price_to_cent(self.fee_buffer)),
        );
        obj.insert(
            "safety_buffer_cent".to_string(),
            json!(price_to_cent(self.safety_buffer)),
        );
        obj.insert(
            "dynamic_threshold_cent".to_string(),
            json!(price_to_cent(self.dynamic_threshold)),
        );
        obj.insert(
            "execution_cost_for_edge_cent".to_string(),
            json!(price_to_cent(self.execution_cost_for_edge)),
        );
        obj.insert(
            "execution_vwap_edge_margin".to_string(),
            json!(price_to_cent(self.edge_margin)),
        );
        obj.insert(
            "submit_limit_price_cent".to_string(),
            json!(price_to_cent(self.submit_limit_price)),
        );
        obj.insert(
            "execution_limit_by_vwap_action".to_string(),
            json!(self.limit_by_vwap_action),
        );
        obj.insert(
            "execution_vwap_block_reason".to_string(),
            json!(self.block_reason),
        );
    }
}

pub(crate) struct PriceToBeatIvExecutionVwapInput<'a> {
    pub(crate) config: PriceToBeatIvExecutionVwapConfig,
    pub(crate) time_rule_price_blocked: bool,
    pub(crate) time_rule_max_price: Option<f64>,
    pub(crate) model_ask: f64,
    pub(crate) depth: &'a PriceToBeatIvDepthEvaluation,
    pub(crate) effective_max_price: Option<f64>,
    pub(crate) q_final: f64,
    pub(crate) dynamic_threshold: f64,
    pub(crate) safety_buffer: f64,
}

pub(crate) fn evaluate_price_to_beat_iv_execution_vwap(
    input: PriceToBeatIvExecutionVwapInput<'_>,
) -> PriceToBeatIvExecutionVwapEvaluation {
    let fresh_execution_best_ask = input.depth.book_best_ask.or(input.depth.best_ask);
    let execution_best_ask = fresh_execution_best_ask.or(Some(input.model_ask));
    let qty_requested = input.depth.intended_qty;
    let qty_available = input.depth.visible_ask_qty;
    let depth_coverage_ratio =
        qty_requested
            .zip(qty_available)
            .and_then(|(requested, available)| {
                (requested.is_finite() && requested > 0.0 && available.is_finite())
                    .then_some((available / requested).max(0.0))
            });
    let full_depth_available = depth_coverage_ratio
        .map(|coverage| coverage + 0.000001 >= 1.0)
        .unwrap_or(false);
    let execution_vwap = input
        .depth
        .estimated_avg_fill
        .filter(|value| value.is_finite() && *value > 0.0 && *value < 1.0);
    let fallback_reason = if execution_vwap.is_some() && !full_depth_available {
        Some("insufficient_depth")
    } else if execution_vwap.is_some() {
        None
    } else if matches!(depth_coverage_ratio, Some(coverage) if coverage < 1.0) {
        Some("insufficient_depth")
    } else if qty_requested.is_none() {
        Some("target_qty_unavailable")
    } else {
        Some("orderbook_unavailable")
    };
    let raw_execution_cost = execution_vwap
        .map(|vwap| input.model_ask.max(vwap))
        .or(Some(input.model_ask));
    let fee_buffer = raw_execution_cost.map(signal_formula_taker_fee);
    let safety_buffer = input.safety_buffer.max(0.0);
    let execution_cost_for_edge = raw_execution_cost
        .zip(fee_buffer)
        .map(|(raw, fee)| raw + fee + safety_buffer);
    let edge_margin =
        execution_cost_for_edge.map(|cost| input.q_final - cost - input.dynamic_threshold);
    let high_dislocation_block = if (input.config.enabled || input.config.limit_by_vwap_enabled)
        && input.config.required_on_high_dislocation
        && execution_vwap.is_none()
    {
        fresh_execution_best_ask
            .map(|reference| input.q_final - reference)
            .filter(|dislocation| {
                input.q_final >= HIGH_DISLOCATION_Q && *dislocation >= HIGH_DISLOCATION_GAP
            })
            .map(|_| "blocked_execution_vwap_unavailable_high_dislocation")
    } else {
        None
    };
    let submit_limit_price = submit_limit_by_best_ask(
        input.config.limit_by_vwap_enabled,
        fresh_execution_best_ask,
        input.config.max_slippage,
        input.effective_max_price,
        input.q_final,
        input.dynamic_threshold,
    );
    let limit_by_vwap_action = limit_by_vwap_action(
        input.config.limit_by_vwap_enabled,
        fresh_execution_best_ask,
        input.effective_max_price,
        submit_limit_price,
        high_dislocation_block,
    );
    let block_reason = if let Some(reason) = high_dislocation_block {
        Some(reason)
    } else if input.config.limit_by_vwap_enabled && fresh_execution_best_ask.is_none() {
        Some("blocked_no_best_ask_for_limit")
    } else if input.config.enabled {
        if let Some((vwap, max_price)) = execution_vwap.zip(input.effective_max_price) {
            if vwap > max_price + 0.000000001 {
                Some("blocked_execution_vwap_above_max_price")
            } else {
                edge_margin
                    .filter(|margin| *margin < 0.0)
                    .map(|_| "blocked_execution_vwap_edge_below_threshold")
            }
        } else {
            edge_margin
                .filter(|margin| execution_vwap.is_some() && *margin < 0.0)
                .map(|_| "blocked_execution_vwap_edge_below_threshold")
        }
    } else {
        None
    };

    PriceToBeatIvExecutionVwapEvaluation {
        enabled: input.config.enabled,
        time_rule_price_blocked: input.time_rule_price_blocked,
        time_rule_max_price: input.time_rule_max_price,
        model_ask: Some(input.model_ask),
        execution_best_ask,
        execution_vwap,
        qty_requested,
        qty_available,
        depth_coverage_ratio,
        depth_levels_used: input.depth.depth_levels_used,
        book_depth_ok: Some(full_depth_available),
        fallback_reason,
        cost_source: Some(if execution_vwap.is_some() {
            "execution_vwap"
        } else {
            "model_ask_fallback"
        }),
        raw_execution_cost,
        fee_buffer,
        safety_buffer: Some(safety_buffer),
        dynamic_threshold: Some(input.dynamic_threshold),
        execution_cost_for_edge,
        edge_margin,
        submit_limit_price,
        limit_by_vwap_action,
        block_reason,
    }
}

fn submit_limit_by_best_ask(
    enabled: bool,
    best_ask: Option<f64>,
    max_slippage: f64,
    effective_max_price: Option<f64>,
    q_final: f64,
    min_edge_after_fill: f64,
) -> Option<f64> {
    if !enabled {
        return None;
    }
    let best_ask = best_ask.filter(|value| value.is_finite() && *value > 0.0 && *value < 1.0)?;
    let slippage_cap = best_ask + max_slippage.max(0.0);
    let edge_cap = (q_final - min_edge_after_fill.max(0.0)).max(0.0);
    let limit = effective_max_price
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .map(|max_price| max_price.min(slippage_cap).min(edge_cap))
        .unwrap_or(slippage_cap.min(edge_cap))
        .clamp(0.0, 1.0);
    Some(limit)
}

fn limit_by_vwap_action(
    enabled: bool,
    best_ask: Option<f64>,
    effective_max_price: Option<f64>,
    submit_limit_price: Option<f64>,
    high_dislocation_block: Option<&'static str>,
) -> &'static str {
    if !enabled {
        "disabled"
    } else if high_dislocation_block.is_some() {
        "block_vwap_unavailable_high_dislocation"
    } else if best_ask.is_none() {
        "block_no_best_ask"
    } else if submit_limit_price
        .zip(effective_max_price)
        .map(|(submit_limit, max_price)| submit_limit + 0.000000001 < max_price)
        .unwrap_or(false)
    {
        "clamp"
    } else {
        "pass"
    }
}

fn price_to_cent(value: Option<f64>) -> Option<f64> {
    value
        .filter(|value| value.is_finite())
        .map(|value| value * 100.0)
}

#[cfg(test)]
mod tests {
    use super::super::iv_mismatch_depth::evaluate_price_to_beat_iv_depth;
    use super::*;
    use bot_infra::exchange::{OrderBookLevel, OrderBookSnapshot};

    #[test]
    fn execution_vwap_uses_depth_sweep_for_target_size() {
        let depth = evaluate_price_to_beat_iv_depth(
            Some(&OrderBookSnapshot {
                bids: Vec::new(),
                asks: vec![
                    OrderBookLevel {
                        price: 0.60,
                        size: 1.0,
                    },
                    OrderBookLevel {
                        price: 0.7273,
                        size: 7.3334,
                    },
                ],
            }),
            0.60,
            Some(8.3334),
            1.0,
            true,
        );
        let evaluation =
            evaluate_price_to_beat_iv_execution_vwap(PriceToBeatIvExecutionVwapInput {
                config: PriceToBeatIvExecutionVwapConfig {
                    enabled: true,
                    ..Default::default()
                },
                time_rule_price_blocked: false,
                time_rule_max_price: Some(0.77),
                model_ask: 0.60,
                depth: &depth,
                effective_max_price: Some(0.77),
                q_final: 0.90,
                dynamic_threshold: 0.02,
                safety_buffer: 0.005,
            });

        let vwap = evaluation.execution_vwap.expect("vwap");
        assert!((vwap - 0.7119).abs() < 0.001);
        assert_eq!(evaluation.execution_best_ask, Some(0.60));
        assert!(evaluation.depth_coverage_ratio.expect("coverage") >= 1.0);
        assert_eq!(evaluation.fallback_reason, None);
    }

    #[test]
    fn execution_vwap_uses_partial_depth_estimate_when_depth_does_not_cover_target() {
        let depth = evaluate_price_to_beat_iv_depth(
            Some(&OrderBookSnapshot {
                bids: Vec::new(),
                asks: vec![OrderBookLevel {
                    price: 0.60,
                    size: 1.0,
                }],
            }),
            0.60,
            Some(5.0),
            1.0,
            true,
        );
        let evaluation =
            evaluate_price_to_beat_iv_execution_vwap(PriceToBeatIvExecutionVwapInput {
                config: PriceToBeatIvExecutionVwapConfig {
                    enabled: true,
                    ..Default::default()
                },
                time_rule_price_blocked: false,
                time_rule_max_price: Some(0.77),
                model_ask: 0.60,
                depth: &depth,
                effective_max_price: Some(0.77),
                q_final: 0.90,
                dynamic_threshold: 0.02,
                safety_buffer: 0.005,
            });

        assert_eq!(evaluation.execution_vwap, Some(0.60));
        assert_eq!(evaluation.fallback_reason, Some("insufficient_depth"));
        assert_eq!(evaluation.cost_source, Some("execution_vwap"));
        assert_eq!(evaluation.book_depth_ok, Some(false));
    }

    #[test]
    fn unavailable_vwap_blocks_high_model_book_dislocation_when_required() {
        let depth = PriceToBeatIvDepthEvaluation {
            best_ask: Some(0.68),
            book_best_ask: Some(0.68),
            intended_qty: Some(8.0),
            estimated_avg_fill: None,
            ..PriceToBeatIvDepthEvaluation::off()
        };
        let evaluation =
            evaluate_price_to_beat_iv_execution_vwap(PriceToBeatIvExecutionVwapInput {
                config: PriceToBeatIvExecutionVwapConfig {
                    enabled: true,
                    required_on_high_dislocation: true,
                    ..Default::default()
                },
                time_rule_price_blocked: false,
                time_rule_max_price: Some(0.77),
                model_ask: 0.68,
                depth: &depth,
                effective_max_price: Some(0.77),
                q_final: 0.9893,
                dynamic_threshold: 0.02,
                safety_buffer: 0.005,
            });

        assert_eq!(
            evaluation.block_reason,
            Some("blocked_execution_vwap_unavailable_high_dislocation")
        );
        assert_eq!(evaluation.execution_vwap, None);
    }

    #[test]
    fn execution_limit_by_best_ask_sets_submit_limit() {
        let depth = PriceToBeatIvDepthEvaluation {
            best_ask: Some(0.53),
            book_best_ask: Some(0.53),
            intended_qty: Some(8.0),
            estimated_avg_fill: Some(0.5346),
            visible_ask_qty: Some(8.0),
            ..PriceToBeatIvDepthEvaluation::off()
        };
        let evaluation =
            evaluate_price_to_beat_iv_execution_vwap(PriceToBeatIvExecutionVwapInput {
                config: PriceToBeatIvExecutionVwapConfig {
                    limit_by_vwap_enabled: true,
                    max_slippage: 0.02,
                    ..Default::default()
                },
                time_rule_price_blocked: false,
                time_rule_max_price: Some(0.80),
                model_ask: 0.53,
                depth: &depth,
                effective_max_price: Some(0.80),
                q_final: 0.90,
                dynamic_threshold: 0.02,
                safety_buffer: 0.005,
            });

        assert_eq!(evaluation.execution_vwap, Some(0.5346));
        assert_eq!(evaluation.submit_limit_price, Some(0.55));
        assert_eq!(evaluation.limit_by_vwap_action, "clamp");
    }

    #[test]
    fn best_ask_cap_binds_when_thinner_than_vwap_cap() {
        // best_ask=0.61, slippage=0.08 -> slippage_cap=0.69
        // vwap=0.81 (swept book), old vwap_cap would be 0.83
        // max_price=0.75, q_final=0.9872, min_edge=0.05 -> edge_cap=0.9372
        // limit = min(0.69, 0.75, 0.9372) = 0.69
        let depth = PriceToBeatIvDepthEvaluation {
            best_ask: Some(0.61),
            book_best_ask: Some(0.61),
            intended_qty: Some(3.7),
            estimated_avg_fill: Some(0.81),
            visible_ask_qty: Some(3.69),
            ..PriceToBeatIvDepthEvaluation::off()
        };
        let evaluation =
            evaluate_price_to_beat_iv_execution_vwap(PriceToBeatIvExecutionVwapInput {
                config: PriceToBeatIvExecutionVwapConfig {
                    limit_by_vwap_enabled: true,
                    max_slippage: 0.08,
                    ..Default::default()
                },
                time_rule_price_blocked: false,
                time_rule_max_price: Some(0.75),
                model_ask: 0.61,
                depth: &depth,
                effective_max_price: Some(0.75),
                q_final: 0.9872,
                dynamic_threshold: 0.05,
                safety_buffer: 0.005,
            });

        assert_eq!(evaluation.execution_vwap, Some(0.81));
        assert_eq!(evaluation.submit_limit_price, Some(0.69));
        assert_eq!(evaluation.limit_by_vwap_action, "clamp");
    }

    #[test]
    fn no_best_ask_blocks_when_limit_by_vwap_enabled() {
        let depth = PriceToBeatIvDepthEvaluation {
            best_ask: None,
            book_best_ask: None,
            intended_qty: Some(3.7),
            estimated_avg_fill: None,
            visible_ask_qty: None,
            ..PriceToBeatIvDepthEvaluation::off()
        };
        let evaluation =
            evaluate_price_to_beat_iv_execution_vwap(PriceToBeatIvExecutionVwapInput {
                config: PriceToBeatIvExecutionVwapConfig {
                    limit_by_vwap_enabled: true,
                    max_slippage: 0.08,
                    ..Default::default()
                },
                time_rule_price_blocked: false,
                time_rule_max_price: Some(0.75),
                model_ask: 0.61,
                depth: &depth,
                effective_max_price: Some(0.75),
                q_final: 0.9872,
                dynamic_threshold: 0.05,
                safety_buffer: 0.005,
            });

        assert_eq!(evaluation.submit_limit_price, None);
        assert_eq!(evaluation.limit_by_vwap_action, "block_no_best_ask");
        assert_eq!(evaluation.block_reason, Some("blocked_no_best_ask_for_limit"));
    }

    #[test]
    fn edge_cap_binds_when_q_minus_min_edge_below_slippage_cap() {
        // best_ask=0.71, slippage=0.08 -> slippage_cap=0.79
        // max_price=0.75, q_final=0.74, min_edge=0.05 -> edge_cap=0.69
        // limit = min(0.79, 0.75, 0.69) = 0.69
        let depth = PriceToBeatIvDepthEvaluation {
            best_ask: Some(0.71),
            book_best_ask: Some(0.71),
            intended_qty: Some(3.0),
            estimated_avg_fill: Some(0.72),
            visible_ask_qty: Some(3.0),
            ..PriceToBeatIvDepthEvaluation::off()
        };
        let evaluation =
            evaluate_price_to_beat_iv_execution_vwap(PriceToBeatIvExecutionVwapInput {
                config: PriceToBeatIvExecutionVwapConfig {
                    limit_by_vwap_enabled: true,
                    max_slippage: 0.08,
                    ..Default::default()
                },
                time_rule_price_blocked: false,
                time_rule_max_price: Some(0.75),
                model_ask: 0.71,
                depth: &depth,
                effective_max_price: Some(0.75),
                q_final: 0.74,
                dynamic_threshold: 0.05,
                safety_buffer: 0.005,
            });

        assert_eq!(evaluation.submit_limit_price, Some(0.69));
        assert_eq!(evaluation.limit_by_vwap_action, "clamp");
    }

    #[test]
    fn max_price_cap_binds_when_below_slippage_and_edge_caps() {
        // best_ask=0.70, slippage=0.08 -> slippage_cap=0.78
        // max_price=0.75, q_final=0.95, min_edge=0.05 -> edge_cap=0.90
        // limit = min(0.78, 0.75, 0.90) = 0.75
        let depth = PriceToBeatIvDepthEvaluation {
            best_ask: Some(0.70),
            book_best_ask: Some(0.70),
            intended_qty: Some(3.0),
            estimated_avg_fill: Some(0.71),
            visible_ask_qty: Some(3.0),
            ..PriceToBeatIvDepthEvaluation::off()
        };
        let evaluation =
            evaluate_price_to_beat_iv_execution_vwap(PriceToBeatIvExecutionVwapInput {
                config: PriceToBeatIvExecutionVwapConfig {
                    limit_by_vwap_enabled: true,
                    max_slippage: 0.08,
                    ..Default::default()
                },
                time_rule_price_blocked: false,
                time_rule_max_price: Some(0.75),
                model_ask: 0.70,
                depth: &depth,
                effective_max_price: Some(0.75),
                q_final: 0.95,
                dynamic_threshold: 0.05,
                safety_buffer: 0.005,
            });

        assert_eq!(evaluation.submit_limit_price, Some(0.75));
        assert_eq!(evaluation.limit_by_vwap_action, "pass");
    }
}
