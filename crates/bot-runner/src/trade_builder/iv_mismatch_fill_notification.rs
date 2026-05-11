fn trade_builder_iv_mismatch_fill_formula_block(
    flow_created_payload: Option<&Value>,
    execution_price: f64,
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
    let cost = trade_builder_iv_mismatch_value(iv, "cost");
    let edge = trade_builder_iv_mismatch_value(iv, "edge");
    let threshold = trade_builder_iv_mismatch_value(iv, "threshold");
    let margin = edge
        .zip(threshold)
        .map(|(edge, threshold)| edge - threshold);
    let q_final = trade_builder_iv_mismatch_value(iv, "q_final").or(q);
    let edge_adj = trade_builder_iv_mismatch_value(iv, "edge_adj").or(edge);
    let dynamic_threshold = trade_builder_iv_mismatch_value(iv, "dynamic_threshold").or(threshold);
    let adjusted_margin = edge_adj
        .zip(dynamic_threshold)
        .map(|(edge, threshold)| edge - threshold);

    Some(format!(
        "\n\nIV Mismatch Edge Basarili\nKarar: {decision}\nSecilen: {} (candidate={}, selected={})\nFill: {:.4} | Ask: {}\nq raw: {} (Up={}, Down={})\nq floor: before {} | after {} | final {} | q_chain_adj {} | q_binance {}\nCost: {} = ask {} + fee {} + buffer {}\nEdge raw: {} = q - cost | Base threshold: {} | Raw margin: {}\nEdge adjusted: {} = q_final - cost | Dynamic threshold: {} | Adj margin: {}\nQuality: min margin {} | thin {} | min q {} | confidence {}\nDisagreement: adverse {} | abs {} | bucket {} | penalty {}\nProtection: mode {} | result {} | reasons {} | threshold penalty {} | gap penalty {}\nBook lead: side {} | up {}/{} mid {} | down {}/{} mid {} | opposite mid {} | model-book gap {}\nGap margin: strength {} / min {} | usd {} / min {} | binance same dir {} | falling knife {}\nRule: {}-{} sn | rule max {} | node max {} | effective max {}\nGap strength: {} | required {} | required USD {}\nPTB: {} | Current: {} | Gap: {} | Seconds: {}\nSigma: {} | Sigma fast: {} | Sigma eff: {}\nExpected Move raw/model/floor/eff: {} / {} / {} / {} | z: {}\nStale/drop: x_now {} | x_prev {} | v {} USD/s | L {}s | x_eff {} | drop_z {}\nPenalties: high_price {} | stale {} | drop {} | binance_missing {}\nBinance: status {} | price {} | stale {} | q {}\nBook: bid {} / ask {} / spread {}\nIV ratio: {} | zero_cross: {} | samples: {} | stale: {}",
        trade_builder_iv_mismatch_guard_label(guard, "normalized_outcome_label")
            .or_else(|| trade_builder_iv_mismatch_guard_label(guard, "direction"))
            .unwrap_or("N/A"),
        trade_builder_iv_mismatch_text(iv, "candidate_side"),
        trade_builder_iv_mismatch_text(iv, "selected_side"),
        execution_price,
        trade_builder_iv_mismatch_format(iv, "ask", 4),
        trade_builder_iv_mismatch_optional(q, 4),
        trade_builder_iv_mismatch_format(iv, "q_up", 4),
        trade_builder_iv_mismatch_format(iv, "q_down", 4),
        trade_builder_iv_mismatch_format(iv, "q_before_floor", 4),
        trade_builder_iv_mismatch_format(iv, "q_after_floor", 4),
        trade_builder_iv_mismatch_optional(q_final, 4),
        trade_builder_iv_mismatch_format(iv, "q_chain_adj", 4),
        trade_builder_iv_mismatch_format(iv, "q_binance", 4),
        trade_builder_iv_mismatch_optional(cost, 4),
        trade_builder_iv_mismatch_format(iv, "ask", 4),
        trade_builder_iv_mismatch_format(iv, "fee", 4),
        trade_builder_iv_mismatch_format(iv, "buffer", 4),
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
        trade_builder_iv_mismatch_format(iv, "min_gap_strength_margin", 4),
        trade_builder_iv_mismatch_format(iv, "gap_usd_margin", 4),
        trade_builder_iv_mismatch_format(iv, "min_gap_usd_margin", 4),
        trade_builder_iv_mismatch_bool(iv, "binance_same_direction"),
        trade_builder_iv_mismatch_bool(iv, "falling_knife_flag"),
        trade_builder_iv_mismatch_nested_format(iv, "selected_time_rule", "start_remaining_secs", 0),
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
    ))
}

fn trade_builder_iv_mismatch_value(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(value_as_f64)
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
