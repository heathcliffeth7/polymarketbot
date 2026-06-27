use super::PriceToBeatGuardEvaluation;
use serde_json::{json, Value};

pub(crate) fn build_iv_mismatch_block_summary(
    evaluation: &PriceToBeatGuardEvaluation,
    iv_mismatch_edge: &Value,
) -> Value {
    json!({
        "primary_reason": evaluation.reason_code.as_str(),
        "decision_reason": field(iv_mismatch_edge, "decision_reason"),
        "all_reasons": array_field(iv_mismatch_edge, "all_reasons"),
        "passed": evaluation.passed,
        "selected_side": field(iv_mismatch_edge, "selected_side"),
        "candidate_side": field(iv_mismatch_edge, "candidate_side"),
        "seconds_left": field(iv_mismatch_edge, "seconds_left"),
        "ask": field(iv_mismatch_edge, "ask"),
        "bid": field(iv_mismatch_edge, "bid"),
        "effective_max_price": field(iv_mismatch_edge, "effective_max_price"),
        "q_final": field(iv_mismatch_edge, "q_final"),
        "edge_adj": field(iv_mismatch_edge, "edge_adj"),
        "dynamic_threshold": field(iv_mismatch_edge, "dynamic_threshold"),
        "gap_strength": field(iv_mismatch_edge, "gap_strength"),
        "required_gap_strength": field(iv_mismatch_edge, "required_gap_strength"),
        "required_gap_usd": field(iv_mismatch_edge, "required_gap_usd"),
        "required_gap_usd_cap": field(iv_mismatch_edge, "required_gap_usd_cap"),
        "required_gap_usd_capped": field(iv_mismatch_edge, "required_gap_usd_capped"),
        "execution_vwap_block_reason": field(iv_mismatch_edge, "execution_vwap_block_reason"),
        "execution_vwap_cent": field(iv_mismatch_edge, "execution_vwap_cent"),
        "execution_vwap_edge_margin": field(iv_mismatch_edge, "execution_vwap_edge_margin"),
        "entry_quality_reason": entry_quality_reason(iv_mismatch_edge)
            .unwrap_or_else(|| json!(evaluation.reason_code.as_str())),
        "protection_reasons": array_field(iv_mismatch_edge, "protection_reasons"),
    })
}

fn field(value: &Value, key: &str) -> Value {
    value.get(key).cloned().unwrap_or(Value::Null)
}

fn array_field(value: &Value, key: &str) -> Value {
    value
        .get(key)
        .filter(|field| field.is_array())
        .cloned()
        .unwrap_or_else(|| json!([]))
}

fn entry_quality_reason(iv_mismatch_edge: &Value) -> Option<Value> {
    iv_mismatch_edge
        .get("entry_quality_debug")
        .and_then(|debug| debug.get("reason").or_else(|| debug.get("primary_reason")))
        .cloned()
        .filter(|reason| !reason.is_null())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evaluation(reason_code: &str, passed: bool) -> PriceToBeatGuardEvaluation {
        PriceToBeatGuardEvaluation {
            passed,
            reason_code: reason_code.to_string(),
            reason_detail: None,
            normalized_outcome_label: None,
            direction: None,
            market_slug: "btc-updown-5m-1".to_string(),
            event_url: "https://polymarket.com/event/btc-updown-5m-1".to_string(),
            timeframe: None,
            asset: None,
            price_to_beat: None,
            price_to_beat_status: None,
            price_to_beat_source: None,
            price_to_beat_source_latency_ms: None,
            current_price: None,
            current_price_source: "test",
            directional_gap: None,
            gap_abs: None,
            threshold_mode: "iv_mismatch_edge".to_string(),
            configured_threshold_mode: Some("iv_mismatch_edge".to_string()),
            base_threshold_value: None,
            base_threshold_unit: None,
            base_threshold_usd: None,
            current_effective_ptb_usd: None,
            threshold_value: 0.0,
            threshold_unit: "usd".to_string(),
            threshold_usd: 0.0,
            stop_loss_bump_count: 0,
            stop_loss_bump_applied_count: 0,
            stop_loss_bump_amount: None,
            stop_loss_bump_max_value: None,
            stop_loss_bump_unit: None,
            stop_loss_bump_raw_usd: 0.0,
            stop_loss_bump_usd: 0.0,
            stop_loss_bump_capped: false,
            stop_loss_bump_max_reached: false,
            stop_loss_bump_current_market_excluded: false,
            stop_loss_bump_increment_usd: 0.0,
            reentry_generation: 0,
            reentry_override_active: false,
            reentry_override_value: None,
            reentry_override_unit: None,
            max_price_relax: None,
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
            early_stale_side: None,
            cex_direction_guard: None,
            entry_current_source_debug: None,
        }
    }

    #[test]
    fn summary_copies_key_iv_mismatch_fields() {
        let summary = build_iv_mismatch_block_summary(
            &evaluation("blocked_execution_vwap_edge_below_threshold", false),
            &json!({
                "decision_reason": "blocked_execution_vwap_edge_below_threshold",
                "all_reasons": ["blocked_execution_vwap_edge_below_threshold"],
                "selected_side": "up",
                "candidate_side": "up",
                "seconds_left": 42.0,
                "ask": 0.75,
                "bid": 0.73,
                "effective_max_price": 0.77,
                "q_final": 0.81,
                "edge_adj": 0.02,
                "dynamic_threshold": 0.03,
                "gap_strength": 1.4,
                "required_gap_strength": 1.75,
                "required_gap_usd": 5.0,
                "execution_vwap_block_reason": "blocked_execution_vwap_edge_below_threshold",
                "execution_vwap_cent": 76.2,
                "execution_vwap_edge_margin": -0.4,
                "entry_quality_debug": {
                    "reason": "entry_spread_too_wide"
                },
                "protection_reasons": ["model_book_gap_warn"]
            }),
        );

        assert_eq!(
            summary.get("primary_reason").and_then(Value::as_str),
            Some("blocked_execution_vwap_edge_below_threshold")
        );
        assert_eq!(
            summary
                .get("execution_vwap_block_reason")
                .and_then(Value::as_str),
            Some("blocked_execution_vwap_edge_below_threshold")
        );
        assert_eq!(
            summary
                .get("execution_vwap_edge_margin")
                .and_then(Value::as_f64),
            Some(-0.4)
        );
        assert_eq!(
            summary.get("entry_quality_reason").and_then(Value::as_str),
            Some("entry_spread_too_wide")
        );
    }

    #[test]
    fn summary_is_safe_when_optional_fields_are_missing() {
        let summary = build_iv_mismatch_block_summary(
            &evaluation("iv_edge_below_threshold", false),
            &json!({}),
        );

        assert_eq!(
            summary.get("primary_reason").and_then(Value::as_str),
            Some("iv_edge_below_threshold")
        );
        assert!(summary.get("decision_reason").is_some_and(Value::is_null));
        assert_eq!(
            summary
                .get("all_reasons")
                .and_then(Value::as_array)
                .unwrap(),
            &Vec::<Value>::new()
        );
        assert_eq!(
            summary
                .get("protection_reasons")
                .and_then(Value::as_array)
                .unwrap(),
            &Vec::<Value>::new()
        );
        assert_eq!(
            summary.get("entry_quality_reason").and_then(Value::as_str),
            Some("iv_edge_below_threshold")
        );
    }
}
