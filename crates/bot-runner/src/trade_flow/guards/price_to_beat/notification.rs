use super::current_price::format_current_price_label;
use super::*;

fn format_optional_direction(value: Option<&str>) -> String {
    match value {
        Some("up") => "Up".to_string(),
        Some("down") => "Down".to_string(),
        Some(other) => other.to_string(),
        None => "N/A".to_string(),
    }
}

fn format_optional_guard_number(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.8}"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn format_optional_guard_cent(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.2}c"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn format_optional_guard_bool(value: Option<bool>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

fn format_optional_guard_i64(value: Option<i64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

fn format_optional_guard_text(value: Option<&str>) -> String {
    value.unwrap_or("N/A").to_string()
}

fn format_optional_guard_q_cent(value: Option<f64>) -> String {
    format_optional_guard_cent(value.map(|value| value * 100.0))
}

fn json_child_object<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Option<&'a serde_json::Map<String, Value>> {
    object.get(key).and_then(Value::as_object)
}

fn json_child_array<'a>(
    object: Option<&'a serde_json::Map<String, Value>>,
    key: &str,
) -> Option<&'a Vec<Value>> {
    object
        .and_then(|object| object.get(key))
        .and_then(Value::as_array)
}

fn json_child_number(object: Option<&serde_json::Map<String, Value>>, key: &str) -> Option<f64> {
    object
        .and_then(|object| object.get(key))
        .and_then(crate::value_as_f64)
        .filter(|value| value.is_finite())
}

fn json_child_i64(object: Option<&serde_json::Map<String, Value>>, key: &str) -> Option<i64> {
    object
        .and_then(|object| object.get(key))
        .and_then(crate::value_as_i64)
}

fn json_child_bool(object: Option<&serde_json::Map<String, Value>>, key: &str) -> Option<bool> {
    object
        .and_then(|object| object.get(key))
        .and_then(Value::as_bool)
}

fn json_child_text<'a>(
    object: Option<&'a serde_json::Map<String, Value>>,
    key: &str,
) -> Option<&'a str> {
    object
        .and_then(|object| object.get(key))
        .and_then(Value::as_str)
}

fn normalize_guard_threshold_usd(value: f64, unit: &str) -> Option<f64> {
    match unit.trim().to_ascii_lowercase().as_str() {
        "usd" => Some(value),
        "cent" => Some(value / 100.0),
        _ => None,
    }
}

fn format_guard_threshold_summary(value: f64, unit: &str, usd: f64) -> String {
    let unit = unit.trim();
    if unit.eq_ignore_ascii_case("usd") {
        return format!("{value:.8} USD");
    }

    match normalize_guard_threshold_usd(value, unit) {
        Some(normalized_usd) if (normalized_usd - usd).abs() <= 1e-9 => {
            format!("{value:.8} {unit} (~{usd:.8} USD)")
        }
        _ => format!("{usd:.8} USD"),
    }
}

fn format_optional_guard_threshold_summary(
    value: Option<f64>,
    unit: Option<&str>,
    usd: Option<f64>,
) -> Option<String> {
    let value = value?;
    let unit = unit?.trim();
    if unit.is_empty() {
        return None;
    }

    if let Some(usd) = usd.or_else(|| normalize_guard_threshold_usd(value, unit)) {
        return Some(format_guard_threshold_summary(value, unit, usd));
    }

    Some(format!("{value:.8} {unit}"))
}

fn build_stop_loss_bump_summary(evaluation: &PriceToBeatGuardEvaluation) -> Option<String> {
    if evaluation.stop_loss_bump_amount.is_none()
        && evaluation.stop_loss_bump_count <= 0
        && evaluation.stop_loss_bump_usd <= 0.0
        && evaluation.stop_loss_bump_max_value.is_none()
    {
        return None;
    }

    let mut parts = Vec::new();
    if let (Some(amount), Some(unit)) = (
        evaluation.stop_loss_bump_amount,
        evaluation.stop_loss_bump_unit.as_deref(),
    ) {
        parts.push(format!("kademe {amount:.8} {unit}"));
    }
    if evaluation.stop_loss_bump_count > 0 {
        parts.push(format!("sayac {}", evaluation.stop_loss_bump_count));
    }
    if evaluation.stop_loss_bump_applied_count > 0
        || evaluation.stop_loss_bump_current_market_excluded
    {
        parts.push(format!(
            "uygulanan sayac {}",
            evaluation.stop_loss_bump_applied_count
        ));
    }
    if evaluation.stop_loss_bump_amount.is_some() || evaluation.stop_loss_bump_usd > 0.0 {
        parts.push(format!(
            "uygulanan {:.8} USD",
            evaluation.stop_loss_bump_usd
        ));
    }
    if let (Some(max_value), Some(unit)) = (
        evaluation.stop_loss_bump_max_value,
        evaluation.stop_loss_bump_unit.as_deref(),
    ) {
        parts.push(format!("max {:.8} {unit}", max_value));
    }
    if evaluation.stop_loss_bump_capped {
        parts.push("cap uygulandi".to_string());
    }
    if evaluation.stop_loss_bump_max_reached {
        parts.push("max limite ulasti".to_string());
    }
    if evaluation.stop_loss_bump_current_market_excluded {
        parts.push("bu market dislandi".to_string());
    }

    Some(parts.join(", "))
}

fn stop_loss_bump_is_active(evaluation: &PriceToBeatGuardEvaluation) -> bool {
    const PTB_BUMP_ACTIVE_EPSILON: f64 = 1e-9;

    if !evaluation.stop_loss_bump_usd.is_finite() || evaluation.stop_loss_bump_usd <= 0.0 {
        return false;
    }

    let baseline_usd = evaluation
        .base_threshold_usd
        .or(evaluation.auto_threshold_usd);
    let effective_usd = evaluation
        .current_effective_ptb_usd
        .or(Some(evaluation.threshold_usd))
        .filter(|value| value.is_finite());

    match (baseline_usd, effective_usd) {
        (Some(baseline_usd), Some(effective_usd)) => {
            effective_usd > baseline_usd + PTB_BUMP_ACTIVE_EPSILON
        }
        _ => false,
    }
}

fn build_price_to_beat_summary_block(evaluation: &PriceToBeatGuardEvaluation) -> String {
    let mut lines = Vec::new();
    let configured_mode = evaluation
        .configured_threshold_mode
        .as_deref()
        .unwrap_or(evaluation.threshold_mode.as_str());
    lines.push(format!("Configured Mod: {configured_mode}"));
    lines.push(format!(
        "Efektif PTB: {}",
        format_guard_threshold_summary(
            evaluation.threshold_value,
            &evaluation.threshold_unit,
            evaluation.threshold_usd,
        )
    ));

    if let Some(base_threshold_summary) = format_optional_guard_threshold_summary(
        evaluation.base_threshold_value,
        evaluation.base_threshold_unit.as_deref(),
        evaluation.base_threshold_usd,
    ) {
        lines.push(format!("Base PTB: {base_threshold_summary}"));
    }
    if let Some(auto_threshold_usd) = evaluation.auto_threshold_usd {
        lines.push(format!("Auto Threshold: {auto_threshold_usd:.8} USD"));
    }
    if evaluation.reentry_override_active {
        let reentry_override_summary = format_optional_guard_threshold_summary(
            evaluation.reentry_override_value,
            evaluation.reentry_override_unit.as_deref(),
            None,
        )
        .unwrap_or_else(|| "aktif".to_string());
        lines.push(format!("Re-entry Override: {reentry_override_summary}"));
    }
    if stop_loss_bump_is_active(evaluation) {
        if let Some(stop_loss_bump_summary) = build_stop_loss_bump_summary(evaluation) {
            lines.push(format!("SL Bump: {stop_loss_bump_summary}"));
        }
    }

    format!("\n{}", lines.join("\n"))
}

fn build_iv_mismatch_execution_summary(evaluation: &PriceToBeatGuardEvaluation) -> String {
    let Some(iv) = evaluation
        .iv_mismatch_edge
        .as_ref()
        .and_then(Value::as_object)
    else {
        return String::new();
    };
    let number = |key: &str| iv.get(key).and_then(Value::as_f64);
    let text = |key: &str| iv.get(key).and_then(Value::as_str).unwrap_or("N/A");
    let mut lines = Vec::new();
    if iv.contains_key("depth_guard_result") {
        lines.push("Depth:".to_string());
        lines.push(format!(
            "Model Ask: {}",
            format_optional_guard_number(number("ask"))
        ));
        lines.push(format!(
            "Execution Best Ask: {}",
            format_optional_guard_number(
                number("depth_book_best_ask").or_else(|| number("depth_best_ask"))
            )
        ));
        lines.push(format!(
            "VWAP Target Qty: {}",
            format_optional_guard_number(number("intended_qty"))
        ));
        lines.push(format!(
            "Execution VWAP: {}",
            format_optional_guard_number(number("estimated_avg_fill"))
        ));
        lines.push(format!(
            "VWAP slippage: {}",
            format_optional_guard_number(number("vwap_slippage"))
        ));
        lines.push(format!(
            "Available best ask qty: {}",
            format_optional_guard_number(number("available_qty_at_best_ask"))
        ));
        lines.push(format!(
            "Levels used: {}",
            format_optional_guard_number(number("depth_levels_used"))
        ));
        lines.push(format!("Result: {}", text("depth_guard_result")));
    }
    if iv.contains_key("execution_vwap_guard_enabled") {
        let execution_vs_model = number("execution_vwap_cent")
            .zip(number("model_ask_cent"))
            .map(|(vwap, model)| vwap - model);
        lines.push("Execution VWAP Guard:".to_string());
        lines.push(format!(
            "Model Ask: {} | Execution Best Ask: {} | Execution VWAP: {}",
            format_optional_guard_cent(number("model_ask_cent")),
            format_optional_guard_cent(number("execution_best_ask_cent")),
            format_optional_guard_cent(number("execution_vwap_cent"))
        ));
        lines.push(format!(
            "Execution vs Model Ask: {} | Effective Max: {} | VWAP Edge Margin: {}",
            format_optional_guard_cent(execution_vs_model),
            format_optional_guard_cent(number("effective_max_price").map(|value| value * 100.0)),
            format_optional_guard_cent(number("execution_vwap_edge_margin"))
        ));
        lines.push(format!(
            "VWAP Size: {} | VWAP Levels: {} | VWAP Coverage: {} | Cost Source: {}",
            format_optional_guard_number(number("execution_vwap_qty_requested")),
            format_optional_guard_number(number("execution_vwap_levels_used")),
            format_optional_guard_number(number("execution_vwap_depth_coverage_ratio")),
            text("execution_cost_source")
        ));
        lines.push(format!(
            "Expected VWAP: {} | Submit Limit: {} | Limit Action: {}",
            format_optional_guard_cent(number("expected_vwap_cent")),
            format_optional_guard_cent(number("submit_limit_price_cent")),
            text("execution_limit_by_vwap_action")
        ));
        lines.push(format!(
            "VWAP Fallback: {} | VWAP Block: {}",
            text("execution_vwap_fallback_reason"),
            text("execution_vwap_block_reason")
        ));
    }
    append_iv_cex_open_gap_summary(iv, &mut lines);
    if let Some(wait_reprice) = iv.get("wait_reprice_guard").and_then(Value::as_object) {
        let wait_number = |key: &str| wait_reprice.get(key).and_then(Value::as_f64);
        let wait_i64 = |key: &str| wait_reprice.get(key).and_then(Value::as_i64);
        let wait_bool = |key: &str| wait_reprice.get(key).and_then(Value::as_bool);
        let wait_text = |key: &str| wait_reprice.get(key).and_then(Value::as_str);
        lines.push("Wait Reprice Guard:".to_string());
        lines.push(format!(
            "Blocked: {} reason={} age_ms={} max_age_ms={}",
            format_optional_guard_bool(wait_bool("blocked")),
            format_optional_guard_text(wait_text("reason")),
            format_optional_guard_i64(wait_i64("wait_for_price_age_ms")),
            format_optional_guard_i64(wait_i64("wait_max_age_ms"))
        ));
        lines.push(format!(
            "Ask: initial={} max={} current={} drop={} cap={}",
            format_optional_guard_cent(wait_number("wait_initial_execution_ask_cent")),
            format_optional_guard_cent(wait_number("wait_max_execution_ask_cent")),
            format_optional_guard_cent(wait_number("wait_current_execution_ask_cent")),
            format_optional_guard_cent(wait_number("wait_price_drop_cent")),
            format_optional_guard_cent(wait_number("time_rule_max_price_cent"))
        ));
        lines.push(format!(
            "Signal: gap initial={} current={} q initial={} current={} fell_into_cap={} late_expensive={}",
            format_optional_guard_number(wait_number("wait_initial_gap_strength")),
            format_optional_guard_number(wait_number("wait_current_gap_strength")),
            format_optional_guard_cent(wait_number("wait_initial_q_final_cent")),
            format_optional_guard_cent(wait_number("wait_current_q_final_cent")),
            format_optional_guard_bool(wait_bool("fell_into_cap")),
            format_optional_guard_bool(wait_bool("late_expensive_entry"))
        ));
    }
    if should_show_oracle_lag_book_lead(iv) {
        let dislocation = number("model_book_dislocation_cent");
        lines.push(format!(
            "Oracle/Book Lead: suspicion={} action={}",
            text("oracle_lag_suspicion"),
            text("oracle_lag_action")
        ));
        lines.push(format!(
            "q_final={} execution_ref={} source={} dislocation={}",
            format_optional_guard_cent(number("q_final_cent")),
            format_optional_guard_cent(number("execution_ref_cent")),
            text("execution_ref_source"),
            format_optional_guard_cent(dislocation)
        ));
        lines.push(format!(
            "Book ref: {} age={}ms coverage={}",
            text("oracle_lag_book_reference_status"),
            format_optional_guard_i64(
                iv.get("oracle_lag_book_reference_age_ms")
                    .and_then(Value::as_i64)
            ),
            format_optional_guard_number(number("oracle_lag_book_depth_coverage_ratio"))
        ));
        lines.push(format!("Reason={}", text("oracle_lag_block_reason")));
    }
    if should_show_borderline_pump_book_lead(iv) {
        lines.push(format!(
            "Borderline Pump/Book Lead: {}",
            text("borderline_pump_book_lead_action")
        ));
        lines.push(format!(
            "Gap margin: {} / required +{}",
            format_optional_guard_number(number("borderline_gap_strength_margin")),
            format_optional_guard_number(number("borderline_gap_strength_margin_required"))
        ));
        lines.push(format!(
            "Pump shock: ratio {}",
            format_optional_guard_number(number("borderline_pump_shock_ratio"))
        ));
        lines.push(format!(
            "q_final={} execution_ref={} dislocation={}",
            format_optional_guard_cent(number("borderline_q_final_cent")),
            format_optional_guard_cent(number("borderline_execution_ref_cent")),
            format_optional_guard_cent(number("borderline_model_book_dislocation_cent"))
        ));
        lines.push(format!(
            "Execution ref source: {} status={}",
            text("borderline_execution_ref_source"),
            text("borderline_execution_ref_status")
        ));
        lines.push(format!(
            "Reason={}",
            text("borderline_pump_book_lead_block_reason")
        ));
    }
    if should_show_pump_shock(iv) {
        lines.push(format!(
            "Pump Shock: action={} reason={}",
            text("pump_shock_action"),
            text("pump_shock_block_reason")
        ));
        lines.push(format!(
            "Growth ratio={} persistence={} hold_gap={} retain={}",
            format_optional_guard_number(number("pump_shock_gap_growth_ratio")),
            format_optional_guard_bool(
                iv.get("pump_shock_persistence_ok").and_then(Value::as_bool)
            ),
            format_optional_guard_number(number("pump_shock_hold_gap")),
            format_optional_guard_number(number("pump_shock_buffer_retain"))
        ));
    }
    if iv.contains_key("model_book_gap") {
        lines.push("Model-book:".to_string());
        lines.push(format!(
            "q_final: {}",
            format_optional_guard_number(number("q_final"))
        ));
        lines.push(format!(
            "selected_mid: {}",
            format_optional_guard_number(number("selected_mid"))
        ));
        lines.push(format!(
            "gap: {}",
            format_optional_guard_number(number("model_book_gap"))
        ));
        lines.push(format!(
            "threshold: {}",
            format_optional_guard_number(number("too_good_threshold"))
        ));
        lines.push(format!("Result: {}", text("book_confirmation_result")));
    }
    if lines.is_empty() {
        String::new()
    } else {
        format!("\n{}", lines.join("\n"))
    }
}

fn append_iv_cex_open_gap_summary(iv: &serde_json::Map<String, Value>, lines: &mut Vec<String>) {
    if !should_show_cex_open_gap(iv) {
        return;
    }

    let number = |key: &str| iv.get(key).and_then(Value::as_f64);
    let bool_value = |key: &str| iv.get(key).and_then(Value::as_bool);
    let text = |key: &str| iv.get(key).and_then(Value::as_str);
    let reason = text("cex_open_gap_block_reason")
        .or_else(|| text("gap_fail_cex_book_block_reason"))
        .or_else(|| text("chainlink_cex_book_mismatch_reason"));

    lines.push("CEX Open Gap:".to_string());
    lines.push(format!(
        "Consensus: {} | clean={} | cap={}",
        format_optional_guard_text(text("cex_open_gap_consensus")),
        format_optional_guard_bool(bool_value("cex_open_gap_clean_lane")),
        format_optional_guard_bool(bool_value("cex_consensus_q_cap_applied"))
    ));
    lines.push(format!(
        "Binance: open={} current={} gap={} z={} state={}",
        format_optional_guard_number(number("binance_5m_open")),
        format_optional_guard_number(number("binance_current_mid")),
        format_optional_guard_number(number("binance_signed_gap")),
        format_optional_guard_number(number("binance_gap_z")),
        format_optional_guard_text(text("binance_state"))
    ));
    lines.push(format!(
        "Bybit: open={} current={} gap={} z={} state={}",
        format_optional_guard_number(number("bybit_5m_open")),
        format_optional_guard_number(number("bybit_current_mid")),
        format_optional_guard_number(number("bybit_signed_gap")),
        format_optional_guard_number(number("bybit_gap_z")),
        format_optional_guard_text(text("bybit_state"))
    ));
    lines.push(format!(
        "Chainlink/CEX: chainlink={} conservative={} effective={} diff={} z={} bps={}",
        format_optional_guard_number(number("chainlink_signed_gap")),
        format_optional_guard_number(number("conservative_cex_gap")),
        format_optional_guard_number(number("effective_consensus_gap_usd")),
        format_optional_guard_number(number("chainlink_cex_diff_usd")),
        format_optional_guard_number(number("chainlink_cex_diff_z")),
        format_optional_guard_number(number("chainlink_cex_diff_bps"))
    ));
    lines.push(format!(
        "q consensus: before={} after={}",
        format_optional_guard_q_cent(number("q_final_before_cex_consensus")),
        format_optional_guard_q_cent(number("q_final_after_cex_consensus"))
    ));
    if bool_value("gap_fail_mixed_cex_guard_enabled").unwrap_or(false)
        || bool_value("late_expensive_mixed_cex_guard_enabled").unwrap_or(false)
        || bool_value("chainlink_cex_lag_no_book_guard_enabled").unwrap_or(false)
    {
        let mixed_action = if bool_value("gap_fail_mixed_cex_triggered").unwrap_or(false) {
            "BLOCK"
        } else {
            "PASS"
        };
        lines.push(format!(
            "Mixed CEX Gap-Fail: {} | gap={} required={} fail={} lagHigh={}",
            mixed_action,
            format_optional_guard_number(number("gap_strength")),
            format_optional_guard_number(number("required_gap_strength")),
            format_optional_guard_bool(bool_value("gap_fail")),
            format_optional_guard_bool(bool_value("lag_high"))
        ));
        let late_action = if bool_value("late_expensive_mixed_cex_triggered").unwrap_or(false) {
            "BLOCK"
        } else {
            "PASS"
        };
        lines.push(format!(
            "Late Expensive CEX: {} | seconds={} vwap={} threshold={}",
            late_action,
            format_optional_guard_number(number("seconds_left")),
            format_optional_guard_cent(number("execution_vwap_cent")),
            format_optional_guard_cent(number("late_expensive_min_vwap_cent"))
        ));
        lines.push(format!(
            "Book Confirmation: available={} missing={} noBookBlock={}",
            format_optional_guard_bool(bool_value("book_confirmation_available")),
            format_optional_guard_bool(bool_value("book_confirmation_missing")),
            format_optional_guard_bool(bool_value("chainlink_cex_lag_no_book_triggered"))
        ));
    }
    lines.push(format!("Reason: {}", format_optional_guard_text(reason)));
}

fn should_show_cex_open_gap(iv: &serde_json::Map<String, Value>) -> bool {
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
        || json_child_has_non_null(iv, "cex_open_gap_block_reason")
        || json_child_has_non_null(iv, "gap_fail_cex_book_block_reason")
        || json_child_has_non_null(iv, "chainlink_cex_book_mismatch_reason")
}

fn should_show_pump_shock(iv: &serde_json::Map<String, Value>) -> bool {
    let enabled = iv
        .get("pump_shock_guard_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let action = iv
        .get("pump_shock_action")
        .and_then(Value::as_str)
        .unwrap_or("disabled");
    enabled || !matches!(action, "disabled")
}

fn should_show_borderline_pump_book_lead(iv: &serde_json::Map<String, Value>) -> bool {
    let enabled = iv
        .get("borderline_pump_book_lead_guard_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let action = iv
        .get("borderline_pump_book_lead_action")
        .and_then(Value::as_str)
        .unwrap_or("disabled");
    enabled || !matches!(action, "disabled")
}

fn should_show_oracle_lag_book_lead(iv: &serde_json::Map<String, Value>) -> bool {
    let enabled = iv
        .get("oracle_lag_book_lead_guard_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let suspicion = iv
        .get("oracle_lag_suspicion")
        .and_then(Value::as_str)
        .unwrap_or("disabled");
    enabled || !matches!(suspicion, "disabled" | "unavailable")
}

fn json_child_has_non_null(object: &serde_json::Map<String, Value>, key: &str) -> bool {
    matches!(object.get(key), Some(value) if !value.is_null())
}

fn json_child_has_non_empty_array(object: &serde_json::Map<String, Value>, key: &str) -> bool {
    matches!(object.get(key).and_then(Value::as_array), Some(values) if !values.is_empty())
}

fn format_component_number(value: f64) -> String {
    format!("{value:.2}")
}

fn eq77_risk_cap_debug_present(debug: &serde_json::Map<String, Value>) -> bool {
    [
        "risk_cap_price_cent",
        "ask_over_cap_cent",
        "risk_score",
        "cap_haircut_cent",
        "risk_level",
        "lane",
        "size_multiplier",
    ]
    .iter()
    .any(|key| json_child_has_non_null(debug, key))
        || json_child_has_non_empty_array(debug, "risk_components")
        || json_child_has_non_empty_array(debug, "cap_components")
}

fn format_risk_component(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    let name = json_child_text(Some(object), "name")?;
    let mut metrics = Vec::new();
    if let Some(points) = json_child_number(Some(object), "risk_points") {
        if points.abs() > 1e-9 {
            metrics.push(format!("+{}pt", format_component_number(points)));
        }
    }
    if let Some(haircut) = json_child_number(Some(object), "haircut_cent") {
        if haircut.abs() > 1e-9 {
            metrics.push(format!("-{}c", format_component_number(haircut)));
        }
    }

    if metrics.is_empty() {
        Some(name.to_string())
    } else {
        Some(format!("{name}({})", metrics.join("/")))
    }
}

fn format_cap_component(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    let name = json_child_text(Some(object), "name")?;
    if let Some(cap) = json_child_number(Some(object), "cap_cent") {
        return Some(format!("{name}={}c", format_component_number(cap)));
    }
    if let Some(haircut) = json_child_number(Some(object), "haircut_cent") {
        return Some(format!("{name}=-{}c", format_component_number(haircut)));
    }
    Some(name.to_string())
}

fn format_component_list<F>(values: &[Value], mut formatter: F) -> Option<String>
where
    F: FnMut(&Value) -> Option<String>,
{
    const COMPONENT_LIMIT: usize = 6;

    let components: Vec<String> = values.iter().filter_map(|value| formatter(value)).collect();
    if components.is_empty() {
        return None;
    }

    let mut visible = components
        .iter()
        .take(COMPONENT_LIMIT)
        .cloned()
        .collect::<Vec<_>>();
    if components.len() > COMPONENT_LIMIT {
        visible.push(format!("(+{} more)", components.len() - COMPONENT_LIMIT));
    }
    Some(visible.join(", "))
}

fn append_eq77_risk_cap_summary(debug: &serde_json::Map<String, Value>, lines: &mut Vec<String>) {
    if !eq77_risk_cap_debug_present(debug) {
        return;
    }

    let allowed = json_child_bool(Some(debug), "allowed")
        .or_else(|| json_child_text(Some(debug), "decision").map(|decision| decision == "allow"));
    lines.push("EQ77 Risk Cap:".to_string());
    lines.push(format!(
        "Action: entry_action={} allowed={} hard_block={} deferred={} recheck={}",
        format_optional_guard_text(json_child_text(Some(debug), "entry_action")),
        format_optional_guard_bool(allowed),
        format_optional_guard_bool(json_child_bool(Some(debug), "hard_block")),
        format_optional_guard_bool(json_child_bool(Some(debug), "deferred")),
        format_optional_guard_bool(json_child_bool(Some(debug), "signal_recheck_required"))
    ));
    lines.push(format!(
        "Risk: score={} level={} lane={} size={}",
        format_optional_guard_number(json_child_number(Some(debug), "risk_score")),
        format_optional_guard_text(json_child_text(Some(debug), "risk_level")),
        format_optional_guard_text(json_child_text(Some(debug), "lane")),
        format_optional_guard_number(json_child_number(Some(debug), "size_multiplier"))
    ));
    if json_child_has_non_null(debug, "eq77_lite_profile")
        || json_child_has_non_null(debug, "gap_strength_hard_floor")
    {
        lines.push(format!(
            "Gap Lite: profile={} required={} with_margin={} floor={} deficit={} soft_low={} ratio={} points={}",
            format_optional_guard_text(json_child_text(Some(debug), "eq77_lite_profile")),
            format_optional_guard_number(json_child_number(Some(debug), "gap_strength_required")),
            format_optional_guard_number(json_child_number(Some(debug), "gap_strength_required_with_margin")),
            format_optional_guard_number(json_child_number(Some(debug), "gap_strength_hard_floor")),
            format_optional_guard_number(json_child_number(Some(debug), "gap_strength_deficit")),
            format_optional_guard_bool(json_child_bool(Some(debug), "gap_strength_soft_low")),
            format_optional_guard_number(json_child_number(Some(debug), "gap_strength_soft_low_ratio")),
            format_optional_guard_number(json_child_number(Some(debug), "gap_soft_low_risk_points"))
        ));
    }
    lines.push(format!(
        "Cap: risk={} effective={} haircut={} ask_over={}",
        format_optional_guard_number(json_child_number(Some(debug), "risk_cap_price_cent")),
        format_optional_guard_number(json_child_number(Some(debug), "effective_max_buy_price")),
        format_optional_guard_number(json_child_number(Some(debug), "cap_haircut_cent")),
        format_optional_guard_number(json_child_number(Some(debug), "ask_over_cap_cent"))
    ));
    lines.push(format!(
        "EV: fair={} fee_buffer={} min_edge={} margin={}",
        format_optional_guard_number(json_child_number(Some(debug), "fair_probability")),
        format_optional_guard_number(json_child_number(Some(debug), "fee_buffer")),
        format_optional_guard_number(json_child_number(Some(debug), "min_edge")),
        format_optional_guard_number(json_child_number(Some(debug), "premium_ev_margin_cent"))
    ));
    if let Some(summary) = json_child_array(Some(debug), "risk_components")
        .and_then(|values| format_component_list(values, format_risk_component))
    {
        lines.push(format!("Risk Components: {summary}"));
    }
    if let Some(summary) = json_child_array(Some(debug), "cap_components")
        .and_then(|values| format_component_list(values, format_cap_component))
    {
        lines.push(format!("Cap Components: {summary}"));
    }
}

fn build_iv_entry_quality_debug_summary(evaluation: &PriceToBeatGuardEvaluation) -> String {
    let Some(debug) = evaluation
        .iv_mismatch_edge
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|iv| json_child_object(iv, "entry_quality_debug"))
    else {
        return String::new();
    };

    let ptb_gate = json_child_object(debug, "ptb_gate");
    let iv_edge = json_child_object(debug, "iv_edge");
    let cex = json_child_object(debug, "cex_direction_guard");
    let source = json_child_object(debug, "source");
    let mut lines = Vec::new();

    lines.push("IV Entry Quality:".to_string());
    let decision = json_child_text(Some(debug), "decision").or_else(|| {
        json_child_bool(Some(debug), "allowed")
            .map(|allowed| if allowed { "allow" } else { "skip" })
    });
    let reason = json_child_text(Some(debug), "reason")
        .or_else(|| json_child_text(Some(debug), "primary_reason"));
    lines.push(format!(
        "Decision: {}",
        format_optional_guard_text(decision)
    ));
    lines.push(format!("Reason: {}", format_optional_guard_text(reason)));
    lines.push(format!(
        "PTB Gate: passed={} gap={} required={}",
        format_optional_guard_bool(json_child_bool(ptb_gate, "passed")),
        format_optional_guard_number(json_child_number(ptb_gate, "gapUsd")),
        format_optional_guard_number(json_child_number(ptb_gate, "requiredGapUsd"))
    ));
    lines.push(format!(
        "IV Edge: passed={} edge={} required={} margin={}",
        format_optional_guard_bool(json_child_bool(iv_edge, "passed")),
        format_optional_guard_number(json_child_number(iv_edge, "edge")),
        format_optional_guard_number(json_child_number(iv_edge, "requiredEdge")),
        format_optional_guard_number(json_child_number(iv_edge, "adjustedMargin"))
    ));
    lines.push(format!(
        "Gap Strength: value={} required={} margin={} rule={}",
        format_optional_guard_number(json_child_number(iv_edge, "gapStrength")),
        format_optional_guard_number(json_child_number(iv_edge, "requiredGapStrength")),
        format_optional_guard_number(json_child_number(iv_edge, "gapStrengthMargin")),
        format_optional_guard_text(json_child_text(iv_edge, "matchedRule"))
    ));
    lines.push(format!(
        "CEX Direction Guard: enabled={} mode={} status={} blocking={} reason={}",
        format_optional_guard_bool(json_child_bool(cex, "enabled")),
        format_optional_guard_text(json_child_text(cex, "mode")),
        format_optional_guard_text(json_child_text(cex, "status")),
        format_optional_guard_bool(json_child_bool(cex, "blocking")),
        format_optional_guard_text(json_child_text(cex, "reasonCode"))
    ));
    lines.push(format!(
        "Source: {} age_ms={}",
        format_optional_guard_text(json_child_text(source, "ptbCurrentPriceSource")),
        format_optional_guard_i64(json_child_i64(source, "chainlinkAgeMs"))
    ));
    append_eq77_risk_cap_summary(debug, &mut lines);

    format!("\n{}", lines.join("\n"))
}

fn format_current_ptb_summary(
    current_ptb_value: Option<f64>,
    current_ptb_unit: Option<&str>,
    current_ptb_usd: Option<f64>,
) -> String {
    format_optional_guard_threshold_summary(current_ptb_value, current_ptb_unit, current_ptb_usd)
        .unwrap_or_else(|| "Bilinmiyor".to_string())
}

pub(super) fn build_price_to_beat_guard_blocked_notification_message(
    evaluation: &PriceToBeatGuardEvaluation,
) -> String {
    let reason = match evaluation.reason_code.as_str() {
        "price_to_beat_gap_below_threshold" => {
            "Secilen yon icin Price to Beat farki gereken minimum seviyenin altinda."
        }
        "price_to_beat_pending" => {
            "Price to Beat verisi henuz hazir degil, cycle-open fiyat snapshot'i bekleniyor."
        }
        "price_to_beat_unavailable" => {
            "Polymarket Price to Beat verisi alinamadigi icin emir engellendi."
        }
        "current_price_unavailable" => "Current price verisi alinamadigi icin emir engellendi.",
        "unsupported_market" => "Bu market Price to Beat guard tarafindan desteklenmiyor.",
        "unsupported_outcome_label" => {
            "Outcome label Up/Down veya Yes/No yonlerinden biri olarak taninamadi."
        }
        _ => "Price to Beat guard emri engelledi.",
    };

    let detail_line = evaluation
        .reason_detail
        .as_deref()
        .map(|detail| format!("\nDetay: {detail}"))
        .unwrap_or_default();
    let metric_line = evaluation
        .direction
        .as_deref()
        .map(|_| {
            "\nKarar Metrigi: Yonsel fark kullanilir. Up=current-price_to_beat, Down=price_to_beat-current. Mutlak fark sadece bilgidir."
        })
        .unwrap_or_default();
    let summary_block = build_price_to_beat_summary_block(evaluation);
    let execution_summary = build_iv_mismatch_execution_summary(evaluation);
    let entry_quality_summary = build_iv_entry_quality_debug_summary(evaluation);

    format!(
        "{}\nSebep: {}{}\nYon: {}\nMarket: {}\nAsset: {}\nTimeframe: {}\nPrice to Beat: {}\nPrice to Beat Status: {}\nPrice to Beat Source: {}\n{}: {}\nYonsel Fark: {}\nMutlak Fark: {}{}\nLimit: {:.8} {} (~{:.8} USD){}{}{}",
        "Price to Beat Korumasi Engelledi",
        reason,
        detail_line,
        format_optional_direction(evaluation.direction.as_deref()),
        evaluation.market_slug,
        evaluation.asset.as_deref().unwrap_or("N/A"),
        evaluation.timeframe.as_deref().unwrap_or("N/A"),
        format_optional_guard_number(evaluation.price_to_beat),
        evaluation.price_to_beat_status.as_deref().unwrap_or("N/A"),
        evaluation.price_to_beat_source.as_deref().unwrap_or("N/A"),
        format_current_price_label(evaluation.current_price_source),
        format_optional_guard_number(evaluation.current_price),
        format_optional_guard_number(evaluation.directional_gap),
        format_optional_guard_number(evaluation.gap_abs),
        metric_line,
        evaluation.threshold_value,
        evaluation.threshold_unit,
        evaluation.threshold_usd,
        summary_block,
        execution_summary,
        entry_quality_summary,
    )
}

pub(super) fn build_price_to_beat_guard_waiting_notification_message(
    evaluation: &PriceToBeatGuardEvaluation,
) -> String {
    format!(
        "{}\nDurum: Bekleme moduna alindi. Kosullar duzelince order yeniden denenecek.",
        build_price_to_beat_guard_blocked_notification_message(evaluation)
    )
}

pub(super) fn build_price_to_beat_guard_recovered_notification_message(
    evaluation: &PriceToBeatGuardEvaluation,
    recovered_from_reason_code: &str,
) -> String {
    let summary_block = build_price_to_beat_summary_block(evaluation);
    let execution_summary = build_iv_mismatch_execution_summary(evaluation);
    let entry_quality_summary = build_iv_entry_quality_debug_summary(evaluation);
    format!(
        "{}\nDurum: Price to Beat yeniden uygun hale geldi.\nOnceki Sebep: {}\nYon: {}\nMarket: {}\nAsset: {}\nTimeframe: {}\nPrice to Beat: {}\nPrice to Beat Status: {}\nPrice to Beat Source: {}\n{}: {}\nYonsel Fark: {}\nMutlak Fark: {}\nLimit: {:.8} {} (~{:.8} USD){}{}{}",
        "Price to Beat Korumasi Gecti",
        recovered_from_reason_code,
        format_optional_direction(evaluation.direction.as_deref()),
        evaluation.market_slug,
        evaluation.asset.as_deref().unwrap_or("N/A"),
        evaluation.timeframe.as_deref().unwrap_or("N/A"),
        format_optional_guard_number(evaluation.price_to_beat),
        evaluation.price_to_beat_status.as_deref().unwrap_or("N/A"),
        evaluation.price_to_beat_source.as_deref().unwrap_or("N/A"),
        format_current_price_label(evaluation.current_price_source),
        format_optional_guard_number(evaluation.current_price),
        format_optional_guard_number(evaluation.directional_gap),
        format_optional_guard_number(evaluation.gap_abs),
        evaluation.threshold_value,
        evaluation.threshold_unit,
        evaluation.threshold_usd,
        summary_block,
        execution_summary,
        entry_quality_summary,
    )
}

pub(super) fn build_price_to_beat_relax_changed_notification_message(
    evaluation: &PriceToBeatGuardEvaluation,
    previous_threshold_usd: Option<f64>,
    raw_target_threshold_usd: Option<f64>,
    next_threshold_usd: f64,
    min_gap_usd: Option<f64>,
    buffer_usd: f64,
    floor_usd: f64,
    miss_streak: i64,
    qualified_market_slugs: &[String],
) -> String {
    let qualified_market_summary = if qualified_market_slugs.is_empty() {
        "N/A".to_string()
    } else {
        qualified_market_slugs.join(", ")
    };
    let summary_block = build_price_to_beat_summary_block(evaluation);
    format!(
        "{}\nMarket: {}\nYon: {}\nAsset: {}\nTimeframe: {}\nOnceki Bildirilen Relax PTB: {}\nHam Relax PTB: {}\nBu Market Efektif Relax PTB: {:.8}\nMin Uygun Gap: {}\nTampon: {:.8}\nFloor: {:.8}\nMiss Streak: {}\nQualified Markets: {}{}",
        "Price to Beat Relax Guncellendi",
        evaluation.market_slug,
        format_optional_direction(evaluation.direction.as_deref()),
        evaluation.asset.as_deref().unwrap_or("N/A"),
        evaluation.timeframe.as_deref().unwrap_or("N/A"),
        format_optional_guard_number(previous_threshold_usd),
        format_optional_guard_number(raw_target_threshold_usd),
        next_threshold_usd,
        format_optional_guard_number(min_gap_usd),
        buffer_usd,
        floor_usd,
        miss_streak,
        qualified_market_summary,
        summary_block,
    )
}

pub(super) fn build_price_to_beat_relax_miss_notification_message(
    evaluation: &PriceToBeatGuardEvaluation,
    previous_miss_streak: Option<i64>,
    next_miss_streak: i64,
    missed_market_slug: Option<&str>,
    tradable_seconds_count: i64,
    max_fillability_score: Option<f64>,
    config_miss_count: i64,
    relax_active: bool,
    effective_target_threshold_usd: Option<f64>,
) -> String {
    let previous_miss_streak = previous_miss_streak
        .map(|value| value.to_string())
        .unwrap_or_else(|| "N/A".to_string());
    let missed_market_slug = missed_market_slug.unwrap_or("N/A");
    let relax_status = if relax_active {
        format!(
            "aktif\nGuncel Efektif Relax PTB: {}",
            format_optional_guard_number(effective_target_threshold_usd)
        )
    } else {
        "threshold henuz gevsemedi".to_string()
    };
    let summary_block = build_price_to_beat_summary_block(evaluation);
    format!(
        "{}\nMarket: {}\nMissed Market: {}\nYon: {}\nAsset: {}\nTimeframe: {}\nOnceki Bildirilen Miss Streak: {}\nYeni Miss Streak: {}\nMissed Tradable Seconds: {}\nMissed Max Fillability: {}\nConfigured Miss Count: {}\nRelax Durumu: {}{}",
        "Price to Beat Relax Miss Artti",
        evaluation.market_slug,
        missed_market_slug,
        format_optional_direction(evaluation.direction.as_deref()),
        evaluation.asset.as_deref().unwrap_or("N/A"),
        evaluation.timeframe.as_deref().unwrap_or("N/A"),
        previous_miss_streak,
        next_miss_streak,
        tradable_seconds_count,
        format_optional_guard_number(max_fillability_score),
        config_miss_count,
        relax_status,
        summary_block,
    )
}

pub(crate) fn build_price_to_beat_bump_increased_notification_message(
    market_slug: &str,
    amount: f64,
    unit: &str,
    count: i64,
    previous_bump_usd: f64,
    next_bump_usd: f64,
    previous_ptb_value: Option<f64>,
    previous_ptb_unit: Option<&str>,
    previous_ptb_usd: Option<f64>,
    current_ptb_value: Option<f64>,
    current_ptb_unit: Option<&str>,
    current_ptb_usd: Option<f64>,
) -> String {
    let previous_ptb_summary =
        format_current_ptb_summary(previous_ptb_value, previous_ptb_unit, previous_ptb_usd);
    let current_ptb_summary =
        format_current_ptb_summary(current_ptb_value, current_ptb_unit, current_ptb_usd);
    format!(
        "PTB Stop-Loss Artisi Guncellendi\nMarket: {market_slug}\nKademe: {amount:.8} {unit}\nToplam Artis Sayisi: {count}\nUygulanan Toplam Artis: {previous_bump_usd:.8} USD -> {next_bump_usd:.8} USD\nEfektif PTB: {previous_ptb_summary} -> {current_ptb_summary}\nGuncel PTB: {current_ptb_summary}"
    )
}

pub(crate) fn build_price_to_beat_bump_max_reached_notification_message(
    market_slug: &str,
    raw_bump_usd: f64,
    capped_bump_usd: f64,
    max_value: f64,
    unit: &str,
    current_ptb_value: Option<f64>,
    current_ptb_unit: Option<&str>,
    current_ptb_usd: Option<f64>,
) -> String {
    let current_ptb_summary =
        format_current_ptb_summary(current_ptb_value, current_ptb_unit, current_ptb_usd);
    format!(
        "PTB Stop-Loss Artisi Max Limite Ulasti\nMarket: {market_slug}\nHam Artis: {raw_bump_usd:.8} USD\nUygulanan Artis: {capped_bump_usd:.8} USD\nMax Limit: {max_value:.8} {unit}\nGuncel PTB: {current_ptb_summary}"
    )
}
