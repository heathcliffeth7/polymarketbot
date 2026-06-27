use super::super::iv_chainlink_stale_strong_gap_exception::ChainlinkStaleStrongGapExceptionDecision;
use super::super::signal_formula::SIGNAL_FORMULA_TAKER_FEE_RATE;
use super::{
    PriceToBeatIvMismatchEdgeEvaluation, DEFAULT_FAST_VOL_WINDOW_SECS, DEFAULT_MIN_VOL_SAMPLES,
    DEFAULT_VOL_WINDOW_SECS,
};
use crate::trade_flow::guards::chainlink_price::chainlink_live_data_ws_diagnostics;
use crate::trade_flow::guards::polymarket_live_data_stream::chainlink_live_data_refresh_diagnostics;
use serde_json::{json, Map, Value};

fn vol_sample_status(evaluation: &PriceToBeatIvMismatchEdgeEvaluation) -> Option<&'static str> {
    if evaluation.reason == "blocked_insufficient_vol_samples" {
        return match (evaluation.sample_count, evaluation.delta_count) {
            (None, _) => Some("fetch_error"),
            (Some(samples), None) if samples < DEFAULT_MIN_VOL_SAMPLES => {
                Some("insufficient_samples")
            }
            (Some(_), Some(deltas)) if deltas + 1 < DEFAULT_MIN_VOL_SAMPLES => {
                Some("insufficient_deltas")
            }
            _ => Some("insufficient_deltas"),
        };
    }

    if evaluation.sample_count.is_some() && evaluation.delta_count.is_some() {
        Some("ready")
    } else {
        None
    }
}

fn chainlink_stale_kind(reason: &str) -> Option<&'static str> {
    match reason {
        "chainlink_provider_stale_global" => Some("provider_stale_global"),
        "chainlink_provider_stale_entry_quality" => Some("provider_stale_entry_quality"),
        _ => None,
    }
}

fn legacy_chainlink_stale_reason(reason: &str) -> Option<&'static str> {
    match reason {
        "chainlink_provider_stale_global" => Some("blocked_rtds_stale"),
        "chainlink_provider_stale_entry_quality" => Some("chainlink_stale"),
        _ => None,
    }
}

pub(crate) fn to_value(evaluation: &PriceToBeatIvMismatchEdgeEvaluation) -> Value {
    let mut all_reasons = evaluation.all_reasons.clone();
    if evaluation.reason != "pending" && !all_reasons.contains(&evaluation.reason) {
        all_reasons.insert(0, evaluation.reason);
    }
    let ws_diagnostics = chainlink_live_data_ws_diagnostics();
    let refresh_diagnostics = chainlink_live_data_refresh_diagnostics();
    let chainlink_ws_receipt_age_ms = evaluation
        .last_symbol_received_age_ms
        .or(refresh_diagnostics.latest_sample_age_ms);
    let chainlink_ws_receipt_age_scope = if evaluation.last_symbol_received_age_ms.is_some() {
        "symbol_specific"
    } else if refresh_diagnostics.latest_sample_age_ms.is_some() {
        "global_min"
    } else {
        "unknown"
    };
    let mut obj = Map::new();
    macro_rules! put {
        ($key:literal, $value:expr) => {
            obj.insert($key.to_string(), json!($value));
        };
    }
    put!("passed", evaluation.passed);
    put!("decision_reason", evaluation.reason);
    put!("all_reasons", all_reasons);
    put!("selected_side", evaluation.selected_side);
    put!("candidate_side", evaluation.candidate_side);
    put!("q", evaluation.q);
    put!("q_up", evaluation.q_up);
    put!("q_down", evaluation.q_down);
    put!("cost", evaluation.cost);
    put!("edge", evaluation.edge);
    put!("edge_cost_source", evaluation.edge_cost_source);
    put!("edge_cost_warning", evaluation.edge_cost_warning);
    put!("telemetry_cost", evaluation.telemetry_cost);
    put!("decision_cost", evaluation.decision_cost);
    put!("executable_all_in_cost", evaluation.executable_all_in_cost);
    put!(
        "edge_adjusted_telemetry",
        evaluation.edge_adjusted_telemetry
    );
    put!("edge_adjusted_decision", evaluation.edge_adjusted_decision);
    put!("sigma", evaluation.sigma);
    put!("iv_ratio", evaluation.iv_ratio);
    put!("zero_cross_count", evaluation.zero_cross_count);
    put!("chainlink_staleness_ms", evaluation.chainlink_staleness_ms);
    put!(
        "chainlink_oracle_price_age_ms",
        evaluation.chainlink_staleness_ms
    );
    put!("provider_age_ms", evaluation.chainlink_staleness_ms);
    put!("receive_age_ms", chainlink_ws_receipt_age_ms);
    put!("global_limit_ms", evaluation.chainlink_stale_ms_effective);
    put!(
        "entry_quality_limit_ms",
        evaluation.entry_quality_chainlink_max_age_ms_effective
    );
    put!(
        "chainlink_stale_kind",
        chainlink_stale_kind(evaluation.reason)
    );
    put!(
        "legacy_reason_code",
        legacy_chainlink_stale_reason(evaluation.reason)
    );
    put!(
        "chainlink_normal_stale_limit_ms",
        evaluation.chainlink_normal_stale_limit_ms
    );
    put!(
        "chainlink_stale_ms_effective",
        evaluation.chainlink_stale_ms_effective
    );
    put!(
        "entry_quality_chainlink_max_age_ms_effective",
        evaluation.entry_quality_chainlink_max_age_ms_effective
    );
    put!(
        "chainlink_stale_override_source",
        evaluation.chainlink_stale_override_source
    );
    put!(
        "chainlink_stale_tolerance_result",
        evaluation.chainlink_stale_tolerance_result
    );
    put!("chainlink_ws_receipt_age_ms", chainlink_ws_receipt_age_ms);
    put!(
        "chainlink_ws_receipt_age_scope",
        chainlink_ws_receipt_age_scope
    );
    put!("spread", evaluation.spread);
    put!("threshold", evaluation.threshold);
    put!("seconds_left", evaluation.seconds_left);
    put!("ask", evaluation.ask);
    put!("bid", evaluation.bid);
    put!("node_max_price", evaluation.node_max_price);
    put!("effective_max_price", evaluation.effective_max_price);
    put!("fee", evaluation.fee);
    put!("buffer", evaluation.buffer);
    put!("chainlink_symbol", evaluation.chainlink_symbol);
    put!("sample_window_start_ms", evaluation.sample_window_start_ms);
    put!("sample_window_end_ms", evaluation.sample_window_end_ms);
    put!("sample_window_secs", evaluation.sample_window_secs);
    put!("vol_sample_error", evaluation.vol_sample_error);
    put!("sample_count", evaluation.sample_count);
    put!("delta_count", evaluation.delta_count);
    put!(
        "last_symbol_tick_age_ms",
        evaluation.last_symbol_tick_age_ms
    );
    put!(
        "last_symbol_received_age_ms",
        evaluation.last_symbol_received_age_ms
    );
    put!("min_vol_samples", DEFAULT_MIN_VOL_SAMPLES);
    put!("vol_sample_status", vol_sample_status(evaluation));
    put!("last_ws_error_class", ws_diagnostics.last_ws_error_class);
    put!("last_ws_http_status", ws_diagnostics.last_ws_http_status);
    put!(
        "last_successful_tick_age_ms",
        ws_diagnostics.last_successful_tick_age_ms
    );
    put!("live_data_ws_proxy_mode", ws_diagnostics.proxy_mode);
    put!(
        "chainlink_refresh_requested",
        refresh_diagnostics.chainlink_refresh_requested
    );
    put!(
        "chainlink_refresh_interval_ms",
        refresh_diagnostics.refresh_interval_ms
    );
    put!(
        "chainlink_global_min_ws_receipt_age_ms",
        refresh_diagnostics.latest_sample_age_ms
    );
    put!(
        "chainlink_refresh_last_request_age_ms",
        refresh_diagnostics.last_request_age_ms
    );
    put!("expected_move", evaluation.expected_move);
    put!("expected_move_raw", evaluation.expected_move);
    put!("z", evaluation.z);
    put!("vol_window_sec", DEFAULT_VOL_WINDOW_SECS);
    put!("fast_vol_window_sec", DEFAULT_FAST_VOL_WINDOW_SECS);
    put!("fee_rate", SIGNAL_FORMULA_TAKER_FEE_RATE);
    let mut value = Value::Object(obj);
    if let Some(obj) = value.as_object_mut() {
        obj.insert("x_now".to_string(), json!(evaluation.x_now));
        obj.insert("x_prev".to_string(), json!(evaluation.x_prev));
        obj.insert("gap_velocity".to_string(), json!(evaluation.gap_velocity));
        obj.insert(
            "latency_horizon_secs".to_string(),
            json!(evaluation.latency_horizon_secs),
        );
        obj.insert("x_eff".to_string(), json!(evaluation.x_eff));
        obj.insert("sigma_15".to_string(), json!(evaluation.sigma_15));
        obj.insert("cex_sigma".to_string(), json!(evaluation.cex_sigma));
        obj.insert("sigma_eff".to_string(), json!(evaluation.sigma_eff));
        obj.insert(
            "sigma_eff_source".to_string(),
            json!(evaluation.sigma_eff_source),
        );
        obj.insert(
            "chainlink_stale_strong_gap_exception_passed".to_string(),
            json!(evaluation.chainlink_stale_exception_passed),
        );
        obj.insert(
            "chainlink_stale_strong_gap_exception".to_string(),
            evaluation
                .chainlink_stale_strong_gap_exception
                .as_ref()
                .map(ChainlinkStaleStrongGapExceptionDecision::to_value)
                .unwrap_or(Value::Null),
        );
        obj.insert(
            "expected_move_model".to_string(),
            json!(evaluation.expected_move_model),
        );
        obj.insert(
            "expected_move_floor".to_string(),
            json!(evaluation.expected_move_floor),
        );
        obj.insert(
            "ptb_chop_guard_enabled".to_string(),
            json!(evaluation.ptb_chop.enabled),
        );
        obj.insert("ptb_chop_risk".to_string(), json!(evaluation.ptb_chop.risk));
        obj.insert(
            "ptb_chop_action".to_string(),
            json!(evaluation.ptb_chop.action),
        );
        obj.insert(
            "ptb_chop_block_reason".to_string(),
            json!(evaluation.ptb_chop.block_reason),
        );
        obj.insert(
            "ptb_movement_mode".to_string(),
            json!(evaluation.ptb_chop.movement_mode),
        );
        obj.insert(
            "ptb_movement_action".to_string(),
            json!(evaluation.ptb_chop.movement_action),
        );
        obj.insert(
            "ptb_movement_reason".to_string(),
            json!(evaluation.ptb_chop.movement_reason),
        );
        obj.insert(
            "ptb_chop_gap_strength_penalty".to_string(),
            json!(evaluation.ptb_chop.gap_strength_penalty),
        );
        obj.insert(
            "ptb_chop_lookback_seconds".to_string(),
            json!(evaluation.ptb_chop.lookback_seconds),
        );
        obj.insert(
            "ptb_chop_extended_lookback_seconds".to_string(),
            json!(evaluation.ptb_chop.extended_lookback_seconds),
        );
        obj.insert(
            "ptb_chop_deadband_usd".to_string(),
            json!(evaluation.ptb_chop.deadband_usd),
        );
        obj.insert(
            "ptb_chop_zero_cross_count_10s".to_string(),
            json!(evaluation.ptb_chop.zero_cross_count_10s),
        );
        obj.insert(
            "ptb_chop_zero_cross_count_15s".to_string(),
            json!(evaluation.ptb_chop.zero_cross_count_15s),
        );
        obj.insert(
            "ptb_chop_gap_path_10s".to_string(),
            json!(evaluation.ptb_chop.gap_path_10s),
        );
        obj.insert(
            "ptb_chop_gap_path_z_10s".to_string(),
            json!(evaluation.ptb_chop.gap_path_z_10s),
        );
        obj.insert(
            "ptb_chop_net_gap_change_10s".to_string(),
            json!(evaluation.ptb_chop.net_gap_change_10s),
        );
        obj.insert(
            "ptb_chop_efficiency_ratio_10s".to_string(),
            json!(evaluation.ptb_chop.efficiency_ratio_10s),
        );
        obj.insert(
            "ptb_chop_max_1s_jump_10s".to_string(),
            json!(evaluation.ptb_chop.max_1s_jump_10s),
        );
        obj.insert(
            "ptb_chop_max_1s_jump_z_10s".to_string(),
            json!(evaluation.ptb_chop.max_1s_jump_z_10s),
        );
        obj.insert(
            "ptb_movement_max_1s_jump_z_10s".to_string(),
            json!(evaluation.ptb_chop.max_1s_jump_z_10s),
        );
        obj.insert(
            "ptb_chop_opposite_depth_usd_10s".to_string(),
            json!(evaluation.ptb_chop.opposite_depth_usd_10s),
        );
        obj.insert(
            "ptb_chop_opposite_depth_z_10s".to_string(),
            json!(evaluation.ptb_chop.opposite_depth_z_10s),
        );
        obj.insert(
            "ptb_chop_same_side_age_seconds".to_string(),
            json!(evaluation.ptb_chop.same_side_age_seconds),
        );
        obj.insert(
            "ptb_movement_cex_consensus".to_string(),
            json!(evaluation.ptb_chop.cex_consensus),
        );
        obj.insert(
            "ptb_movement_model_book_dislocation".to_string(),
            json!(evaluation.ptb_chop.model_book_dislocation),
        );
        obj.insert(
            "q_before_floor".to_string(),
            json!(evaluation.q_before_floor),
        );
        obj.insert("q_after_floor".to_string(), json!(evaluation.q_after_floor));
        obj.insert("q_chain_adj".to_string(), json!(evaluation.q_chain_adj));
        obj.insert("binance_price".to_string(), json!(evaluation.binance_price));
        obj.insert(
            "binance_staleness_ms".to_string(),
            json!(evaluation.binance_staleness_ms),
        );
        obj.insert("q_binance".to_string(), json!(evaluation.q_binance));
        obj.insert("q_final".to_string(), json!(evaluation.q_final));
        obj.insert("edge_adj".to_string(), json!(evaluation.edge_adj));
        obj.insert(
            "adjusted_margin".to_string(),
            json!(evaluation.adjusted_margin),
        );
        obj.insert(
            "min_adjusted_margin".to_string(),
            json!(evaluation.min_adjusted_margin),
        );
        obj.insert(
            "thin_margin_flag".to_string(),
            json!(evaluation.thin_margin_flag),
        );
        obj.insert("min_final_q".to_string(), json!(evaluation.min_final_q));
        obj.insert(
            "q_disagreement".to_string(),
            json!(evaluation.q_disagreement),
        );
        obj.insert(
            "q_disagreement_abs".to_string(),
            json!(evaluation.q_disagreement_abs),
        );
        obj.insert(
            "q_disagreement_bucket".to_string(),
            json!(evaluation.q_disagreement_bucket),
        );
        obj.insert(
            "dynamic_threshold_before_participation".to_string(),
            json!(evaluation.dynamic_threshold_before_participation),
        );
        obj.insert(
            "dynamic_threshold".to_string(),
            json!(evaluation.dynamic_threshold),
        );
        obj.insert(
            "participation_credit".to_string(),
            json!(evaluation.participation_credit),
        );
        obj.insert(
            "participation_last_fill_age_minutes".to_string(),
            json!(evaluation.participation_last_fill_age_minutes),
        );
        obj.insert(
            "high_price_penalty".to_string(),
            json!(evaluation.high_price_penalty_applied),
        );
        obj.insert(
            "stale_penalty".to_string(),
            json!(evaluation.stale_penalty_applied),
        );
        obj.insert(
            "drop_penalty".to_string(),
            json!(evaluation.drop_penalty_applied),
        );
        obj.insert(
            "binance_missing_penalty".to_string(),
            json!(evaluation.binance_missing_penalty_applied),
        );
        obj.insert(
            "binance_disagreement_penalty".to_string(),
            json!(evaluation.binance_disagreement_penalty_applied),
        );
        obj.insert(
            "confidence_score".to_string(),
            json!(evaluation.confidence_score),
        );
        obj.insert("drop_z".to_string(), json!(evaluation.drop_z));
        obj.insert(
            "binance_veto_status".to_string(),
            json!(evaluation.binance_veto_status),
        );
        obj.insert(
            "selected_time_rule_index".to_string(),
            json!(evaluation.selected_time_rule_index),
        );
        obj.insert(
            "selected_time_rule".to_string(),
            json!(evaluation
                .selected_time_rule
                .zip(evaluation.selected_time_rule_index)
                .map(|(rule, index)| rule.to_value(index))),
        );
        obj.insert(
            "time_rule_max_price".to_string(),
            json!(evaluation.time_rule_max_price),
        );
        obj.insert(
            "expected_move_eff".to_string(),
            json!(evaluation.expected_move_eff),
        );
        obj.insert("gap_strength".to_string(), json!(evaluation.gap_strength));
        obj.insert(
            "gap_strength_chainlink_raw".to_string(),
            json!(evaluation
                .x_now
                .map(|x| x / evaluation.expected_move_eff.unwrap_or(1.0))),
        );
        obj.insert(
            "required_gap_strength".to_string(),
            json!(evaluation.required_gap_strength),
        );
        obj.insert(
            "required_gap_usd".to_string(),
            json!(evaluation.required_gap_usd),
        );
        obj.insert("gap_gate_mode".to_string(), json!(evaluation.gap_gate.mode));
        obj.insert(
            "gap_gate_enforced".to_string(),
            json!(evaluation.gap_gate.enforced),
        );
        obj.insert(
            "gap_gate_result".to_string(),
            json!(evaluation.gap_gate.result),
        );
        obj.insert(
            "gap_gate_reason".to_string(),
            json!(evaluation.gap_gate.reason),
        );
        obj.insert(
            "gap_gate_actual_strength".to_string(),
            json!(evaluation.gap_gate.actual_strength),
        );
        obj.insert(
            "gap_gate_required_strength".to_string(),
            json!(evaluation.gap_gate.required_strength),
        );
        obj.insert(
            "gap_gate_margin".to_string(),
            json!(evaluation.gap_gate.margin),
        );
        obj.insert(
            "gap_gate_min_margin".to_string(),
            json!(evaluation.gap_gate.min_margin),
        );
        obj.insert(
            "gap_gate_warning".to_string(),
            json!(evaluation.gap_gate.warning),
        );
        obj.insert(
            "gap_strength_stale_penalty".to_string(),
            json!(evaluation.gap_strength_stale_penalty),
        );
        obj.insert(
            "gap_strength_velocity_penalty".to_string(),
            json!(evaluation.gap_strength_velocity_penalty),
        );
        obj.insert(
            "protection_mode".to_string(),
            json!(evaluation.protection_mode),
        );
        obj.insert(
            "protection_result".to_string(),
            json!(evaluation.protection_result),
        );
        obj.insert(
            "protection_reasons".to_string(),
            json!(evaluation.protection_reasons),
        );
        obj.insert(
            "protection_threshold_penalty".to_string(),
            json!(evaluation.protection_threshold_penalty),
        );
        obj.insert(
            "protection_gap_strength_penalty".to_string(),
            json!(evaluation.protection_gap_strength_penalty),
        );
        obj.insert("up_bid".to_string(), json!(evaluation.up_bid));
        obj.insert("up_ask".to_string(), json!(evaluation.up_ask));
        obj.insert("down_bid".to_string(), json!(evaluation.down_bid));
        obj.insert("down_ask".to_string(), json!(evaluation.down_ask));
        evaluation.depth.append_to_json(obj);
        evaluation.depth_runtime_diagnostics.append_to_json(obj);
        obj.insert(
            "depth_guard_hard_block_enabled".to_string(),
            json!(evaluation.depth_guard_hard_block_enabled),
        );
        obj.insert("up_mid".to_string(), json!(evaluation.up_mid));
        obj.insert("down_mid".to_string(), json!(evaluation.down_mid));
        obj.insert("book_side".to_string(), json!(evaluation.book_side));
        obj.insert("book_mid_diff".to_string(), json!(evaluation.book_mid_diff));
        obj.insert("opposite_mid".to_string(), json!(evaluation.opposite_mid));
        obj.insert("selected_mid".to_string(), json!(evaluation.selected_mid));
        obj.insert("selected_ask".to_string(), json!(evaluation.selected_ask));
        obj.insert(
            "model_book_gap".to_string(),
            json!(evaluation.model_book_gap),
        );
        obj.insert(
            "model_book_gap_warn_threshold".to_string(),
            json!(evaluation.model_book_gap_warn_threshold),
        );
        obj.insert(
            "too_good_threshold".to_string(),
            json!(evaluation.too_good_threshold),
        );
        obj.insert(
            "book_confirmation_result".to_string(),
            json!(evaluation.book_confirmation_result),
        );
        obj.insert(
            "gap_strength_margin".to_string(),
            json!(evaluation.gap_strength_margin),
        );
        obj.insert(
            "gap_usd_margin".to_string(),
            json!(evaluation.gap_usd_margin),
        );
        obj.insert(
            "min_gap_strength_margin".to_string(),
            json!(evaluation.min_gap_strength_margin),
        );
        obj.insert(
            "min_gap_usd_margin".to_string(),
            json!(evaluation.min_gap_usd_margin),
        );
        obj.insert(
            "binance_same_direction".to_string(),
            json!(evaluation.binance_same_direction),
        );
        obj.insert(
            "falling_knife_flag".to_string(),
            json!(evaluation.falling_knife_flag),
        );
        if let Some(entry_quality) = &evaluation.entry_quality {
            obj.insert("entry_quality_debug".to_string(), entry_quality.to_value());
        }
        evaluation.cex_magnitude.append_to_json(obj);
        evaluation.cex_open_gap.append_to_json(obj);
        evaluation.execution_vwap.append_to_json(obj);
        evaluation.price_band_guard.append_to_json(obj);
        evaluation.gap_fail_cex_book.append_to_json(obj);
        evaluation.oracle_lag_book_lead.append_to_json(obj);
        evaluation.borderline_pump_book_lead.append_to_json(obj);
        evaluation.pump_shock.append_to_json(obj);
        evaluation.oracle_tick_jump.append_to_json(obj);
        evaluation.expected_move_floor_debug.append_to_json(obj);
        evaluation.medium_chop_margin.append_to_json(obj);
        evaluation.high_price_early_reversal.append_to_json(obj);
        if let Some(adaptive) = &evaluation.adaptive {
            adaptive.append_to_json(obj);
        }
        evaluation.token_crash_cooldown.append_to_json(obj);
    }
    value
}
