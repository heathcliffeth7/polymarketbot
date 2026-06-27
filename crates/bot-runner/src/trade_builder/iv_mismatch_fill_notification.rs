fn trade_builder_iv_mismatch_fill_formula_block(
    flow_created_payload: Option<&Value>,
    submitted_payload: Option<&Value>,
    execution_price: f64,
    fill_analysis: Option<&TradeBuilderFillExecutionAnalysis>,
) -> Option<String> {
    let guard = flow_created_payload?.get("price_to_beat_guard")?;
    if guard.get("threshold_mode").and_then(Value::as_str) != Some("iv_mismatch_edge") {
        return None;
    }
    let iv = guard.get("iv_mismatch_edge")?;
    let passed = iv.get("passed").and_then(Value::as_bool).unwrap_or(false);
    let decision = iv
        .get("decision_reason")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if !passed || decision != "selected_edge_passed" {
        return None;
    }

    let q = trade_builder_iv_mismatch_value(iv, "q");
    let edge = trade_builder_iv_mismatch_value(iv, "edge");
    let threshold = trade_builder_iv_mismatch_value(iv, "threshold");
    let margin = edge
        .zip(threshold)
        .map(|(edge, threshold)| edge - threshold);
    let q_final = trade_builder_iv_mismatch_value(iv, "q_final").or(q);
    let edge_adj = trade_builder_iv_mismatch_value(iv, "edge_adjusted_decision")
        .or_else(|| trade_builder_iv_mismatch_value(iv, "edge_adj"))
        .or(edge);
    let dynamic_threshold = trade_builder_iv_mismatch_value(iv, "dynamic_threshold").or(threshold);
    let adjusted_margin = edge_adj
        .zip(dynamic_threshold)
        .map(|(edge, threshold)| edge - threshold);
    let min_gap_strength_margin = trade_builder_iv_mismatch_value(iv, "gap_gate_min_margin")
        .or_else(|| trade_builder_iv_mismatch_value(iv, "min_gap_strength_margin"));
    let model_ask = trade_builder_iv_mismatch_value(iv, "ask");
    let execution_vwap = trade_builder_iv_mismatch_value(iv, "execution_vwap_cent")
        .map(|value| value / 100.0)
        .or_else(|| trade_builder_iv_mismatch_value(iv, "estimated_avg_fill"))
        .or_else(|| submitted_payload.and_then(trade_builder_submitted_estimated_avg_fill));
    let execution_vs_model = execution_vwap
        .zip(model_ask)
        .map(|(vwap, model)| vwap - model);
    let decision_cost_block =
        trade_builder_iv_mismatch_decision_cost_block(iv, execution_price, fill_analysis);
    let ptb_chop_block = trade_builder_iv_mismatch_ptb_chop_block(iv);
    let medium_chop_margin_block = trade_builder_iv_mismatch_medium_chop_margin_block(iv);
    let high_price_early_block = trade_builder_iv_mismatch_high_price_early_block(iv);
    let cex_open_gap_block = trade_builder_iv_mismatch_cex_open_gap_block(iv);
    let execution_limit_block = trade_builder_iv_mismatch_execution_limit_block(iv);
    let diagnostic_blocks = format!(
        "{ptb_chop_block}{medium_chop_margin_block}{high_price_early_block}{cex_open_gap_block}{execution_limit_block}"
    );

    Some(format!(
        "\n\nIV Mismatch Edge Basarili\nKarar: {decision}\nSecilen: {} (candidate={}, selected={})\nFill: {:.4} | Model Ask: {} | Execution VWAP: {} | Execution vs Model Ask: {}\nq raw: {} (Up={}, Down={})\nq floor: before {} | after {} | final {} | q_chain_adj {} | q_binance {}\n{}\nEdge raw: {} = q - decision cost | Base threshold: {} | Raw margin: {}\nEdge adjusted: {} = q_final - decision cost | Dynamic threshold: {} | Adj margin: {}\nQuality: min margin {} | thin {} | min q {} | confidence {}\nDisagreement: adverse {} | abs {} | bucket {} | penalty {}\nProtection: mode {} | result {} | reasons {} | threshold penalty {} | gap penalty {}\nBook lead: side {} | up {}/{} mid {} | down {}/{} mid {} | opposite mid {} | model-book gap {}\nGap margin: strength {} / min {} | usd {} / min {} | binance same dir {} | falling knife {}\nRule: {}-{} sn | rule max {} | node max {} | effective max {}\nGap strength: {} | required {} | required USD {}\nPTB: {} | Current: {} | Gap: {} | Seconds: {}\nSigma: {} | Sigma fast: {} | Sigma eff: {}\nExpected Move raw/model/floor/eff: {} / {} / {} / {} | z: {}\nAdaptive floor: mode {} | base bps {} | effective bps {} | floor USD {} | reason {}\nStale/drop: x_now {} | x_prev {} | v {} USD/s | L {}s | x_eff {} | drop_z {}\nPenalties: high_price {} | stale {} | drop {} | binance_missing {}\nBinance: status {} | price {} | stale {} | q {}\nBook: bid {} / ask {} / spread {}\nIV ratio: {} | zero_cross: {} | samples: {} | stale: {}{}",
        trade_builder_iv_mismatch_guard_label(guard, "normalized_outcome_label")
            .or_else(|| trade_builder_iv_mismatch_guard_label(guard, "direction"))
            .unwrap_or("N/A"),
        trade_builder_iv_mismatch_text(iv, "candidate_side"),
        trade_builder_iv_mismatch_text(iv, "selected_side"),
        execution_price,
        trade_builder_iv_mismatch_format(iv, "ask", 4),
        trade_builder_iv_mismatch_optional(execution_vwap, 4),
        trade_builder_iv_mismatch_optional(execution_vs_model, 4),
        trade_builder_iv_mismatch_optional(q, 4),
        trade_builder_iv_mismatch_format(iv, "q_up", 4),
        trade_builder_iv_mismatch_format(iv, "q_down", 4),
        trade_builder_iv_mismatch_format(iv, "q_before_floor", 4),
        trade_builder_iv_mismatch_format(iv, "q_after_floor", 4),
        trade_builder_iv_mismatch_optional(q_final, 4),
        trade_builder_iv_mismatch_format(iv, "q_chain_adj", 4),
        trade_builder_iv_mismatch_format(iv, "q_binance", 4),
        decision_cost_block,
        trade_builder_iv_mismatch_optional(edge, 4),
        trade_builder_iv_mismatch_optional(threshold, 4),
        trade_builder_iv_mismatch_optional(margin, 4),
        trade_builder_iv_mismatch_optional(edge_adj, 4),
        trade_builder_iv_mismatch_optional(dynamic_threshold, 4),
        trade_builder_iv_mismatch_optional(adjusted_margin, 4),
        trade_builder_iv_mismatch_format(iv, "min_adjusted_margin", 4),
        trade_builder_iv_mismatch_bool(iv, "thin_margin_flag"),
        trade_builder_iv_mismatch_format(iv, "min_final_q", 4),
        trade_builder_iv_mismatch_format(iv, "confidence_score", 4),
        trade_builder_iv_mismatch_format(iv, "q_disagreement", 4),
        trade_builder_iv_mismatch_format(iv, "q_disagreement_abs", 4),
        trade_builder_iv_mismatch_text(iv, "q_disagreement_bucket"),
        trade_builder_iv_mismatch_format(iv, "binance_disagreement_penalty", 4),
        trade_builder_iv_mismatch_text(iv, "protection_mode"),
        trade_builder_iv_mismatch_text(iv, "protection_result"),
        trade_builder_iv_mismatch_string_list(iv, "protection_reasons"),
        trade_builder_iv_mismatch_format(iv, "protection_threshold_penalty", 4),
        trade_builder_iv_mismatch_format(iv, "protection_gap_strength_penalty", 4),
        trade_builder_iv_mismatch_text(iv, "book_side"),
        trade_builder_iv_mismatch_format(iv, "up_bid", 4),
        trade_builder_iv_mismatch_format(iv, "up_ask", 4),
        trade_builder_iv_mismatch_format(iv, "up_mid", 4),
        trade_builder_iv_mismatch_format(iv, "down_bid", 4),
        trade_builder_iv_mismatch_format(iv, "down_ask", 4),
        trade_builder_iv_mismatch_format(iv, "down_mid", 4),
        trade_builder_iv_mismatch_format(iv, "opposite_mid", 4),
        trade_builder_iv_mismatch_format(iv, "model_book_gap", 4),
        trade_builder_iv_mismatch_format(iv, "gap_strength_margin", 4),
        trade_builder_iv_mismatch_optional(min_gap_strength_margin, 4),
        trade_builder_iv_mismatch_format(iv, "gap_usd_margin", 4),
        trade_builder_iv_mismatch_format(iv, "min_gap_usd_margin", 4),
        trade_builder_iv_mismatch_bool(iv, "binance_same_direction"),
        trade_builder_iv_mismatch_bool(iv, "falling_knife_flag"),
        trade_builder_iv_mismatch_nested_format(
            iv,
            "selected_time_rule",
            "start_remaining_secs",
            0
        ),
        trade_builder_iv_mismatch_nested_format(iv, "selected_time_rule", "end_remaining_secs", 0),
        trade_builder_iv_mismatch_format(iv, "time_rule_max_price", 4),
        trade_builder_iv_mismatch_format(iv, "node_max_price", 4),
        trade_builder_iv_mismatch_format(iv, "effective_max_price", 4),
        trade_builder_iv_mismatch_format(iv, "gap_strength", 4),
        trade_builder_iv_mismatch_format(iv, "required_gap_strength", 4),
        trade_builder_iv_mismatch_format(iv, "required_gap_usd", 4),
        trade_builder_iv_mismatch_guard_format(guard, "price_to_beat", 8),
        trade_builder_iv_mismatch_guard_format(guard, "current_price", 8),
        trade_builder_iv_mismatch_guard_format(guard, "directional_gap", 8),
        trade_builder_iv_mismatch_format(iv, "seconds_left", 2),
        trade_builder_iv_mismatch_format(iv, "sigma", 8),
        trade_builder_iv_mismatch_format(iv, "sigma_15", 8),
        trade_builder_iv_mismatch_format(iv, "sigma_eff", 8),
        trade_builder_iv_mismatch_format(iv, "expected_move_raw", 8),
        trade_builder_iv_mismatch_format(iv, "expected_move_model", 8),
        trade_builder_iv_mismatch_format(iv, "expected_move_floor", 8),
        trade_builder_iv_mismatch_format(iv, "expected_move_eff", 8),
        trade_builder_iv_mismatch_format(iv, "z", 4),
        trade_builder_iv_mismatch_text(iv, "expected_move_floor_mode"),
        trade_builder_iv_mismatch_format(iv, "expected_move_floor_bps_base", 4),
        trade_builder_iv_mismatch_format(iv, "expected_move_floor_bps_effective", 4),
        trade_builder_iv_mismatch_format(iv, "expected_move_floor_usd", 8),
        trade_builder_iv_mismatch_text(iv, "expected_move_floor_reason"),
        trade_builder_iv_mismatch_format(iv, "x_now", 4),
        trade_builder_iv_mismatch_format(iv, "x_prev", 4),
        trade_builder_iv_mismatch_format(iv, "gap_velocity", 4),
        trade_builder_iv_mismatch_format(iv, "latency_horizon_secs", 3),
        trade_builder_iv_mismatch_format(iv, "x_eff", 4),
        trade_builder_iv_mismatch_format(iv, "drop_z", 4),
        trade_builder_iv_mismatch_format(iv, "high_price_penalty", 4),
        trade_builder_iv_mismatch_format(iv, "stale_penalty", 4),
        trade_builder_iv_mismatch_format(iv, "drop_penalty", 4),
        trade_builder_iv_mismatch_format(iv, "binance_missing_penalty", 4),
        trade_builder_iv_mismatch_text(iv, "binance_veto_status"),
        trade_builder_iv_mismatch_format(iv, "binance_price", 4),
        trade_builder_iv_mismatch_format(iv, "binance_staleness_ms", 0),
        trade_builder_iv_mismatch_format(iv, "q_binance", 4),
        trade_builder_iv_mismatch_format(iv, "bid", 4),
        trade_builder_iv_mismatch_format(iv, "ask", 4),
        trade_builder_iv_mismatch_format(iv, "spread", 4),
        trade_builder_iv_mismatch_format(iv, "iv_ratio", 4),
        trade_builder_iv_mismatch_format(iv, "zero_cross_count", 0),
        trade_builder_iv_mismatch_format(iv, "sample_count", 0),
        trade_builder_iv_mismatch_format(iv, "chainlink_staleness_ms", 0),
        diagnostic_blocks,
    ))
}

fn trade_builder_iv_mismatch_medium_chop_margin_block(iv: &Value) -> String {
    let result = trade_builder_iv_mismatch_text(iv, "medium_chop_margin_result");
    if matches!(result, "N/A" | "off") {
        return String::new();
    }

    format!(
        "\nMedium Chop Margin Guard:\nmode={} decision_ref={} adjusted_margin={} required_margin={}\ncomponents: base={} high_price={} binance_fail_open={} stale={} result={}",
        trade_builder_iv_mismatch_text(iv, "medium_chop_margin_mode"),
        trade_builder_iv_mismatch_cent(iv, "medium_chop_margin_decision_ref_cent"),
        trade_builder_iv_mismatch_format(iv, "medium_chop_margin_adjusted_margin", 4),
        trade_builder_iv_mismatch_format(iv, "medium_chop_margin_required_margin", 4),
        trade_builder_iv_mismatch_format(iv, "medium_chop_margin_base", 4),
        trade_builder_iv_mismatch_format(iv, "medium_chop_margin_high_price_add", 4),
        trade_builder_iv_mismatch_format(iv, "medium_chop_margin_binance_fail_open_add", 4),
        trade_builder_iv_mismatch_format(iv, "medium_chop_margin_stale_add", 4),
        result,
    )
}

fn trade_builder_iv_mismatch_high_price_early_block(iv: &Value) -> String {
    let result = trade_builder_iv_mismatch_text(iv, "high_price_early_guard_result");
    if matches!(result, "N/A" | "off" | "not_applicable") {
        return String::new();
    }

    format!(
        "\nHigh Price Early Reversal Guard:\nenabled={} applies={} result={} reasons={}\ndecision_ref={} seconds={} q_final={} q_extreme={}\ngap: base={} stale_add={} binance_add={} effective={}\nconfirm: q_binance_available={} stale_ms={} cex={} clean={}",
        trade_builder_iv_mismatch_bool(iv, "high_price_early_guard_enabled"),
        trade_builder_iv_mismatch_bool(iv, "high_price_early_applies"),
        result,
        trade_builder_iv_mismatch_string_list(iv, "high_price_early_guard_reasons"),
        trade_builder_iv_mismatch_cent(iv, "high_price_early_decision_ref_cent"),
        trade_builder_iv_mismatch_format(iv, "high_price_early_seconds_left", 2),
        trade_builder_iv_mismatch_q_cent(iv, "high_price_early_q_final"),
        trade_builder_iv_mismatch_bool(iv, "high_price_early_q_extreme"),
        trade_builder_iv_mismatch_format(iv, "high_price_early_base_required_gap_strength", 4),
        trade_builder_iv_mismatch_format(iv, "high_price_early_stale_gap_add_applied", 4),
        trade_builder_iv_mismatch_format(iv, "high_price_early_binance_missing_gap_add_applied", 4),
        trade_builder_iv_mismatch_format(iv, "high_price_early_effective_required_gap_strength", 4),
        trade_builder_iv_mismatch_bool(iv, "high_price_early_q_binance_available"),
        trade_builder_iv_mismatch_format(iv, "high_price_early_chainlink_staleness_ms", 0),
        trade_builder_iv_mismatch_text(iv, "high_price_early_cex_consensus"),
        trade_builder_iv_mismatch_bool(iv, "high_price_early_cex_clean"),
    )
}

fn trade_builder_iv_mismatch_ptb_chop_block(iv: &Value) -> String {
    if !trade_builder_iv_mismatch_has_non_null(iv, "ptb_chop_guard_enabled")
        && !trade_builder_iv_mismatch_has_non_null(iv, "ptb_chop_risk")
        && !trade_builder_iv_mismatch_has_non_null(iv, "ptb_movement_mode")
    {
        return String::new();
    }

    let mode = trade_builder_iv_mismatch_non_null_text(iv, "ptb_movement_mode")
        .unwrap_or_else(|| trade_builder_iv_mismatch_text(iv, "ptb_chop_risk"));
    let action = trade_builder_iv_mismatch_non_null_text(iv, "ptb_movement_action")
        .unwrap_or_else(|| trade_builder_iv_mismatch_text(iv, "ptb_chop_action"));
    let reason = trade_builder_iv_mismatch_non_null_text(iv, "ptb_movement_reason")
        .or_else(|| trade_builder_iv_mismatch_non_null_text(iv, "ptb_chop_block_reason"))
        .unwrap_or("N/A");

    format!(
        "\nPTB Movement: mode={} | action={} | reason={}\ncross10={} cross15={} path10={} z={} efficiency={} maxJumpZ={}\noppDepth={} sameSideAge={}s cex={} bookDislocation={} penalty={}",
        mode,
        action,
        reason,
        trade_builder_iv_mismatch_format(iv, "ptb_chop_zero_cross_count_10s", 0),
        trade_builder_iv_mismatch_format(iv, "ptb_chop_zero_cross_count_15s", 0),
        trade_builder_iv_mismatch_format(iv, "ptb_chop_gap_path_10s", 4),
        trade_builder_iv_mismatch_format(iv, "ptb_chop_gap_path_z_10s", 4),
        trade_builder_iv_mismatch_format(iv, "ptb_chop_efficiency_ratio_10s", 4),
        trade_builder_iv_mismatch_format(iv, "ptb_chop_max_1s_jump_z_10s", 4),
        trade_builder_iv_mismatch_format(iv, "ptb_chop_opposite_depth_z_10s", 4),
        trade_builder_iv_mismatch_format(iv, "ptb_chop_same_side_age_seconds", 2),
        trade_builder_iv_mismatch_text(iv, "ptb_movement_cex_consensus"),
        trade_builder_iv_mismatch_format(iv, "ptb_movement_model_book_dislocation", 4),
        trade_builder_iv_mismatch_format(iv, "ptb_chop_gap_strength_penalty", 4),
    )
}

fn trade_builder_iv_mismatch_cex_open_gap_block(iv: &Value) -> String {
    if !trade_builder_iv_mismatch_should_show_cex_open_gap(iv) {
        return String::new();
    }

    let reason = trade_builder_iv_mismatch_non_null_text(iv, "cex_open_gap_block_reason")
        .or_else(|| {
            trade_builder_iv_mismatch_non_null_text(iv, "chainlink_cex_book_mismatch_reason")
        })
        .unwrap_or("N/A");
    let anchor_label = trade_builder_iv_mismatch_text(iv, "cex_open_gap_anchor_venue");

    format!(
        "\nCEX Open Gap:\nConsensus: {} | clean={} | cap={}\nBinance: open={} current={} gap={} z={} state={}\nAnchor({}): open={} current={} gap={} z={} state={}\nChainlink/CEX: chainlink={} conservative={} effective={} diff={} z={} bps={}\nq consensus: before={} after={}\nReason: {}",
        trade_builder_iv_mismatch_text(iv, "cex_open_gap_consensus"),
        trade_builder_iv_mismatch_bool(iv, "cex_open_gap_clean_lane"),
        trade_builder_iv_mismatch_bool(iv, "cex_consensus_q_cap_applied"),
        trade_builder_iv_mismatch_format(iv, "binance_5m_open", 8),
        trade_builder_iv_mismatch_format(iv, "binance_current_mid", 8),
        trade_builder_iv_mismatch_format(iv, "binance_signed_gap", 8),
        trade_builder_iv_mismatch_format(iv, "binance_gap_z", 8),
        trade_builder_iv_mismatch_text(iv, "binance_state"),
        anchor_label,
        trade_builder_iv_mismatch_format(iv, "anchor_5m_open", 8),
        trade_builder_iv_mismatch_format(iv, "anchor_current_mid", 8),
        trade_builder_iv_mismatch_format(iv, "anchor_signed_gap", 8),
        trade_builder_iv_mismatch_format(iv, "anchor_gap_z", 8),
        trade_builder_iv_mismatch_text(iv, "anchor_state"),
        trade_builder_iv_mismatch_format(iv, "chainlink_signed_gap", 8),
        trade_builder_iv_mismatch_format(iv, "conservative_cex_gap", 8),
        trade_builder_iv_mismatch_format(iv, "effective_consensus_gap_usd", 8),
        trade_builder_iv_mismatch_format(iv, "chainlink_cex_diff_usd", 8),
        trade_builder_iv_mismatch_format(iv, "chainlink_cex_diff_z", 8),
        trade_builder_iv_mismatch_format(iv, "chainlink_cex_diff_bps", 8),
        trade_builder_iv_mismatch_q_cent(iv, "q_final_before_cex_consensus"),
        trade_builder_iv_mismatch_q_cent(iv, "q_final_after_cex_consensus"),
        reason,
    )
}

fn trade_builder_iv_mismatch_execution_limit_block(iv: &Value) -> String {
    if !trade_builder_iv_mismatch_has_non_null(iv, "expected_vwap_cent")
        && !trade_builder_iv_mismatch_has_non_null(iv, "submit_limit_price_cent")
        && !trade_builder_iv_mismatch_has_non_null(iv, "execution_limit_by_vwap_action")
    {
        return String::new();
    }

    format!(
        "\nExecution Limit:\nExpected VWAP: {} | Submit Limit: {} | Limit Action: {}",
        trade_builder_iv_mismatch_cent(iv, "expected_vwap_cent"),
        trade_builder_iv_mismatch_cent(iv, "submit_limit_price_cent"),
        trade_builder_iv_mismatch_text(iv, "execution_limit_by_vwap_action"),
    )
}

fn trade_builder_iv_mismatch_should_show_cex_open_gap(iv: &Value) -> bool {
    let enabled = iv
        .get("cex_open_gap_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let consensus = iv
        .get("cex_open_gap_consensus")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let cap_applied = iv
        .get("cex_consensus_q_cap_applied")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    enabled
        || cap_applied
        || !matches!(consensus, "" | "unavailable" | "disabled")
        || trade_builder_iv_mismatch_has_non_null(iv, "cex_open_gap_block_reason")
        || trade_builder_iv_mismatch_has_non_null(iv, "chainlink_cex_book_mismatch_reason")
}

fn trade_builder_iv_mismatch_decision_cost_block(
    iv: &Value,
    execution_price: f64,
    fill_analysis: Option<&TradeBuilderFillExecutionAnalysis>,
) -> String {
    let (decision_ref_label, decision_ref_value) = trade_builder_iv_mismatch_cost_basis(iv);
    let fee = trade_builder_iv_mismatch_value(iv, "fee_buffer_cent")
        .map(|value| value / 100.0)
        .or_else(|| trade_builder_iv_mismatch_value(iv, "fee"));
    let buffer = trade_builder_iv_mismatch_value(iv, "safety_buffer_cent")
        .map(|value| value / 100.0)
        .or_else(|| trade_builder_iv_mismatch_value(iv, "buffer"));
    let decision_cost = trade_builder_iv_mismatch_value(iv, "execution_cost_for_edge_cent")
        .map(|value| value / 100.0)
        .or_else(|| {
            decision_ref_value
                .zip(fee)
                .zip(buffer)
                .map(|((reference, fee), buffer)| reference + fee + buffer)
        })
        .or_else(|| trade_builder_iv_mismatch_value(iv, "cost"));
    let actual_fill = fill_analysis
        .map(|analysis| analysis.actual_fill_price)
        .unwrap_or(execution_price);
    let (actual_effective_cost, _, _) =
        trade_builder_effective_actual_cost_parts(actual_fill, Some(iv));
    let fill_source = fill_analysis
        .map(|analysis| analysis.actual_fill_source)
        .unwrap_or("N/A");
    let fill_source_warning = if fill_source == "fallback_execution_price" {
        "\nFill Source Warning: fallback execution price used"
    } else {
        ""
    };

    format!(
        "Decision Cost: {} = {} {} + fee {} + buffer {}\nDecision Ref: {} {}\nActual Fill: {} | Actual Effective Cost: {} | Fill Source: {}{}",
        trade_builder_iv_mismatch_optional(decision_cost, 4),
        decision_ref_label,
        trade_builder_iv_mismatch_optional(decision_ref_value, 4),
        trade_builder_iv_mismatch_optional(fee, 4),
        trade_builder_iv_mismatch_optional(buffer, 4),
        decision_ref_label,
        trade_builder_iv_mismatch_optional(decision_ref_value, 4),
        trade_builder_iv_mismatch_optional(Some(actual_fill), 4),
        trade_builder_iv_mismatch_optional(Some(actual_effective_cost), 4),
        fill_source,
        fill_source_warning,
    )
}

fn trade_builder_iv_mismatch_cost_basis(iv: &Value) -> (&'static str, Option<f64>) {
    if let Some(raw_execution_cost) =
        trade_builder_iv_mismatch_value(iv, "raw_execution_cost_cent").map(|value| value / 100.0)
    {
        return ("execution cost", Some(raw_execution_cost));
    }
    let execution_cost_source = iv
        .get("execution_cost_source")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if execution_cost_source == "execution_vwap" {
        if let Some(execution_vwap) =
            trade_builder_iv_mismatch_value(iv, "execution_vwap_cent").map(|value| value / 100.0)
        {
            return ("execution cost", Some(execution_vwap));
        }
    }
    (
        "model ask fallback",
        trade_builder_iv_mismatch_value(iv, "ask"),
    )
}

#[cfg(test)]
mod iv_mismatch_fill_notification_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cost_basis_uses_execution_cost_when_present() {
        let iv = json!({
            "ask": 0.61,
            "execution_vwap_cent": 66.0,
            "raw_execution_cost_cent": 66.0,
            "execution_cost_source": "execution_vwap"
        });

        let (label, value) = trade_builder_iv_mismatch_cost_basis(&iv);

        assert_eq!(label, "execution cost");
        assert_eq!(value, Some(0.66));
    }

    #[test]
    fn cost_basis_falls_back_to_model_ask_without_iv_execution_cost() {
        let iv = json!({
            "ask": 0.61,
            "execution_cost_source": "model_ask_fallback"
        });

        let (label, value) = trade_builder_iv_mismatch_cost_basis(&iv);

        assert_eq!(label, "model ask fallback");
        assert_eq!(value, Some(0.61));
    }

    #[test]
    fn decision_cost_uses_execution_cost_for_edge_when_present() {
        let iv = json!({
            "ask": 0.60,
            "cost": 0.5529,
            "raw_execution_cost_cent": 60.0,
            "fee_buffer_cent": 1.79,
            "safety_buffer_cent": 0.50,
            "execution_cost_for_edge_cent": 62.29,
            "execution_cost_source": "execution_vwap",
            "buffer": 0.005,
            "fee_rate": 0.072
        });
        let analysis = TradeBuilderFillExecutionAnalysis {
            actual_fill_price: 0.54,
            actual_filled_qty: 9.26,
            actual_notional: 5.0004,
            actual_fill_source: "fallback_execution_price",
        };

        let block = trade_builder_iv_mismatch_decision_cost_block(&iv, 0.54, Some(&analysis));

        assert!(block.contains(
            "Decision Cost: 0.6229 = execution cost 0.6000 + fee 0.0179 + buffer 0.0050"
        ));
        assert!(block.contains("Actual Fill: 0.5400 | Actual Effective Cost: 0.5629"));
        assert!(block.contains("Fill Source Warning: fallback execution price used"));
    }

    #[test]
    fn ptb_chop_block_formats_metrics() {
        let iv = json!({
            "ptb_chop_guard_enabled": true,
            "ptb_chop_risk": "TOXIC",
            "ptb_chop_action": "BLOCK",
            "ptb_chop_block_reason": "blocked_ptb_chop_volatility",
            "ptb_movement_mode": "toxic_chop",
            "ptb_movement_action": "block",
            "ptb_movement_reason": "blocked_ptb_chop_volatility",
            "ptb_chop_zero_cross_count_10s": 3,
            "ptb_chop_zero_cross_count_15s": 4,
            "ptb_chop_gap_path_10s": 2.28,
            "ptb_chop_gap_path_z_10s": 1.82,
            "ptb_chop_efficiency_ratio_10s": 0.11,
            "ptb_chop_max_1s_jump_z_10s": 0.86,
            "ptb_chop_opposite_depth_z_10s": 0.74,
            "ptb_chop_same_side_age_seconds": 2.1,
            "ptb_movement_cex_consensus": "mixed",
            "ptb_movement_model_book_dislocation": 0.24,
            "ptb_chop_gap_strength_penalty": 0.35,
        });

        let block = trade_builder_iv_mismatch_ptb_chop_block(&iv);

        assert!(block.contains("PTB Movement: mode=toxic_chop | action=block"));
        assert!(block.contains("reason=blocked_ptb_chop_volatility"));
        assert!(block.contains("cross10=3 cross15=4"));
        assert!(block.contains("maxJumpZ=0.8600"));
        assert!(block.contains("cex=mixed bookDislocation=0.2400"));
        assert!(block.contains("penalty=0.3500"));
    }

    #[test]
    fn medium_chop_margin_block_formats_components() {
        let iv = json!({
            "medium_chop_margin_result": "blocked_medium_chop_adj_margin",
            "medium_chop_margin_mode": "medium_chop",
            "medium_chop_margin_decision_ref_cent": 83.0,
            "medium_chop_margin_adjusted_margin": 0.04,
            "medium_chop_margin_required_margin": 0.06,
            "medium_chop_margin_base": 0.045,
            "medium_chop_margin_high_price_add": 0.005,
            "medium_chop_margin_binance_fail_open_add": 0.005,
            "medium_chop_margin_stale_add": 0.005,
        });

        let block = trade_builder_iv_mismatch_medium_chop_margin_block(&iv);

        assert!(block.contains("Medium Chop Margin Guard:"));
        assert!(block.contains("mode=medium_chop decision_ref=83.00c"));
        assert!(block.contains("adjusted_margin=0.0400 required_margin=0.0600"));
        assert!(block.contains("base=0.0450 high_price=0.0050"));
        assert!(block.contains("result=blocked_medium_chop_adj_margin"));
    }

    #[test]
    fn fill_formula_block_includes_cex_open_gap_and_execution_limit() {
        let mut iv = json!({
            "passed": true,
            "decision_reason": "selected_edge_passed",
            "candidate_side": "up",
            "selected_side": "up",
            "ask": 0.55,
            "q": 0.97,
            "q_final": 0.82,
            "cost": 0.555,
            "edge": 0.415,
            "edge_adj": 0.265,
            "threshold": 0.0075,
            "dynamic_threshold": 0.0275,
            "fee": 0.010,
            "buffer": 0.005,
        });
        let iv_obj = iv.as_object_mut().expect("iv object");
        iv_obj.insert("raw_execution_cost_cent".to_string(), json!(54.0));
        iv_obj.insert("execution_vwap_cent".to_string(), json!(54.0));
        iv_obj.insert("expected_vwap_cent".to_string(), json!(54.0));
        iv_obj.insert("submit_limit_price_cent".to_string(), json!(56.0));
        iv_obj.insert("execution_limit_by_vwap_action".to_string(), json!("clamp"));
        iv_obj.insert("cex_open_gap_enabled".to_string(), json!(true));
        iv_obj.insert("cex_open_gap_consensus".to_string(), json!("mixed"));
        iv_obj.insert("cex_open_gap_clean_lane".to_string(), json!(false));
        iv_obj.insert("cex_consensus_q_cap_applied".to_string(), json!(true));
        iv_obj.insert("binance_5m_open".to_string(), json!(60750.0));
        iv_obj.insert("binance_current_mid".to_string(), json!(60768.0));
        iv_obj.insert("binance_signed_gap".to_string(), json!(18.0));
        iv_obj.insert("binance_gap_z".to_string(), json!(0.72));
        iv_obj.insert("binance_state".to_string(), json!("supporting"));
        iv_obj.insert("cex_open_gap_anchor_venue".to_string(), json!("okx"));
        iv_obj.insert("anchor_5m_open".to_string(), json!(60750.0));
        iv_obj.insert("anchor_current_mid".to_string(), json!(60752.4));
        iv_obj.insert("anchor_signed_gap".to_string(), json!(2.4));
        iv_obj.insert("anchor_gap_z".to_string(), json!(0.10));
        iv_obj.insert("anchor_state".to_string(), json!("weak_positive"));
        iv_obj.insert("chainlink_signed_gap".to_string(), json!(30.8));
        iv_obj.insert("conservative_cex_gap".to_string(), json!(2.4));
        iv_obj.insert("effective_consensus_gap_usd".to_string(), json!(2.4));
        iv_obj.insert("chainlink_cex_diff_usd".to_string(), json!(28.4));
        iv_obj.insert("chainlink_cex_diff_z".to_string(), json!(1.18));
        iv_obj.insert("chainlink_cex_diff_bps".to_string(), json!(4.67));
        iv_obj.insert("q_final_before_cex_consensus".to_string(), json!(0.972));
        iv_obj.insert("q_final_after_cex_consensus".to_string(), json!(0.824));
        iv_obj.insert(
            "chainlink_cex_book_mismatch_reason".to_string(),
            json!("blocked_chainlink_cex_book_mismatch"),
        );

        let flow_created_payload = json!({
            "price_to_beat_guard": {
                "threshold_mode": "iv_mismatch_edge",
                "normalized_outcome_label": "Up",
                "price_to_beat": 60769.5,
                "current_price": 60778.2,
                "directional_gap": 8.7,
                "iv_mismatch_edge": iv
            }
        });

        let message = trade_builder_iv_mismatch_fill_formula_block(
            Some(&flow_created_payload),
            None,
            0.54,
            None,
        )
        .expect("fill formula block");

        assert!(message.contains("CEX Open Gap:"));
        assert!(message.contains("Consensus: mixed | clean=false | cap=true"));
        assert!(message.contains("q consensus: before=97.20c after=82.40c"));
        assert!(message.contains("Reason: blocked_chainlink_cex_book_mismatch"));
        assert!(message.contains("Execution Limit:"));
        assert!(
            message.contains("Expected VWAP: 54.00c | Submit Limit: 56.00c | Limit Action: clamp")
        );
        assert!(message.contains(
            "Decision Cost: 0.5550 = execution cost 0.5400 + fee 0.0100 + buffer 0.0050"
        ));
    }

    #[test]
    fn fill_formula_block_omits_cex_open_gap_when_telemetry_missing() {
        let flow_created_payload = json!({
            "price_to_beat_guard": {
                "threshold_mode": "iv_mismatch_edge",
                "normalized_outcome_label": "Up",
                "iv_mismatch_edge": {
                    "passed": true,
                    "decision_reason": "selected_edge_passed",
                    "candidate_side": "up",
                    "selected_side": "up",
                    "ask": 0.55,
                    "q": 0.70,
                    "q_final": 0.70,
                    "cost": 0.57,
                    "edge": 0.13,
                    "threshold": 0.0075
                }
            }
        });

        let message = trade_builder_iv_mismatch_fill_formula_block(
            Some(&flow_created_payload),
            None,
            0.55,
            None,
        )
        .expect("fill formula block");

        assert!(!message.contains("CEX Open Gap:"));
    }
}

fn trade_builder_iv_mismatch_value(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(value_as_f64)
}

fn trade_builder_iv_mismatch_has_non_null(value: &Value, key: &str) -> bool {
    matches!(value.get(key), Some(value) if !value.is_null())
}

fn trade_builder_iv_mismatch_non_null_text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .filter(|value| !value.is_null())
        .and_then(Value::as_str)
}

fn trade_builder_iv_mismatch_text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("N/A")
}

fn trade_builder_iv_mismatch_bool(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_bool)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

fn trade_builder_iv_mismatch_string_list(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "N/A".to_string())
}

fn trade_builder_iv_mismatch_guard_label<'a>(guard: &'a Value, key: &str) -> Option<&'a str> {
    guard
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn trade_builder_iv_mismatch_format(value: &Value, key: &str, decimals: usize) -> String {
    trade_builder_iv_mismatch_optional(trade_builder_iv_mismatch_value(value, key), decimals)
}

fn trade_builder_iv_mismatch_cent(value: &Value, key: &str) -> String {
    trade_builder_iv_mismatch_value(value, key)
        .map(|value| format!("{value:.2}c"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn trade_builder_iv_mismatch_q_cent(value: &Value, key: &str) -> String {
    trade_builder_iv_mismatch_value(value, key)
        .map(|value| format!("{:.2}c", value * 100.0))
        .unwrap_or_else(|| "N/A".to_string())
}

fn trade_builder_iv_mismatch_nested_format(
    value: &Value,
    key: &str,
    nested_key: &str,
    decimals: usize,
) -> String {
    let parsed = value
        .get(key)
        .and_then(|nested| nested.get(nested_key))
        .and_then(value_as_f64);
    trade_builder_iv_mismatch_optional(parsed, decimals)
}

fn trade_builder_iv_mismatch_guard_format(guard: &Value, key: &str, decimals: usize) -> String {
    trade_builder_iv_mismatch_optional(trade_builder_iv_mismatch_value(guard, key), decimals)
}

fn trade_builder_iv_mismatch_optional(value: Option<f64>, decimals: usize) -> String {
    match value {
        Some(value) if value.is_finite() => format!("{value:.decimals$}"),
        _ => "N/A".to_string(),
    }
}
