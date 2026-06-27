use super::*;
use serde_json::json;

const MARKET: &str = "btc-updown-5m-1780762500";

fn wait_reprice_node() -> crate::TradeFlowNode {
    test_action_place_order_node(json!({
        "priceToBeatIvWaitRepriceGuardEnabled": true,
        "priceToBeatIvWaitMaxAgeMsEarly": 8000,
        "priceToBeatIvWaitMaxAgeMsMid": 5000,
        "priceToBeatIvWaitMaxAgeMsLate": 3000,
        "priceToBeatIvWaitInitialAskMaxOverCapCent": 10,
        "priceToBeatIvFallingIntoCapGuardEnabled": true,
        "priceToBeatIvFallingIntoCapDropCentEarly": 15,
        "priceToBeatIvFallingIntoCapDropCentMid": 12,
        "priceToBeatIvFallingIntoCapDropCentLate": 8,
        "priceToBeatIvLateExpensiveEntryGuardEnabled": true,
        "priceToBeatIvLateExpensiveSeconds": 45,
        "priceToBeatIvLateExpensiveVwapCent": 70,
        "priceToBeatIvLateExpensiveMinQCent": 92,
        "priceToBeatIvLateExpensiveMinGapStrengthExtra": 0.50
    }))
}

fn low_quality_recheck_node() -> crate::TradeFlowNode {
    test_action_place_order_node(json!({
        "priceToBeatIvWaitRepriceGuardEnabled": false,
        "priceToBeatIvLowQualityEdgeRecheckEnabled": true,
        "priceToBeatIvLowQualityGapMargin": 0.10,
        "priceToBeatIvLowQualityEdgeMarginCent": 5.0
    }))
}

fn iv_wait_evaluation(
    reason: &str,
    seconds_left: f64,
    execution_vwap_cent: f64,
    gap_strength: f64,
    required_gap_strength: f64,
    q_final: f64,
) -> PriceToBeatGuardEvaluation {
    let mut evaluation = default_guard_evaluation();
    evaluation.passed = reason == "selected_edge_passed";
    evaluation.reason_code = reason.to_string();
    evaluation.configured_threshold_mode = Some("iv_mismatch_edge".to_string());
    evaluation.market_slug = MARKET.to_string();
    evaluation.iv_mismatch_edge = Some(json!({
        "passed": evaluation.passed,
        "decision_reason": reason,
        "seconds_left": seconds_left,
        "time_rule_max_price_cent": 77.0,
        "execution_vwap_cent": execution_vwap_cent,
        "gap_strength": gap_strength,
        "required_gap_strength": required_gap_strength,
        "q_final": q_final,
        "adjusted_margin": 0.08,
        "ptb_movement_mode": "clean_trend",
        "ptb_chop_gap_strength_penalty": 0.0,
        "cex_open_gap_consensus": "strong",
        "book_confirmation_missing": false,
        "binance_veto_status": "fresh",
        "all_reasons": [reason]
    }));
    evaluation
}

fn waiting_context_with_reason(
    reason_code: &str,
    started_at_ms: i64,
    initial_ask_cent: f64,
    max_ask_cent: f64,
    initial_gap_strength: f64,
    initial_q_final_cent: f64,
) -> serde_json::Value {
    json!({
        "flowContext": {
                "priceToBeatGuardWaiting": {
                    "marketSlug": MARKET,
                    "reasonCode": reason_code,
                    "startedAtMs": started_at_ms,
                "updatedAtMs": started_at_ms,
                "initialExecutionAskCent": initial_ask_cent,
                "maxExecutionAskCent": max_ask_cent,
                "lastExecutionAskCent": initial_ask_cent,
                "initialGapStrength": initial_gap_strength,
                "initialQFinalCent": initial_q_final_cent
            }
        }
    })
}

fn waiting_context(
    started_at_ms: i64,
    initial_ask_cent: f64,
    max_ask_cent: f64,
    initial_gap_strength: f64,
    initial_q_final_cent: f64,
) -> serde_json::Value {
    waiting_context_with_reason(
        "blocked_time_rule_max_price",
        started_at_ms,
        initial_ask_cent,
        max_ask_cent,
        initial_gap_strength,
        initial_q_final_cent,
    )
}

fn apply_guard(
    evaluation: &mut PriceToBeatGuardEvaluation,
    context: &serde_json::Value,
    now_ms: i64,
) {
    super::super::wait_reprice_guard::maybe_apply_wait_reprice_guard(
        &wait_reprice_node(),
        context,
        evaluation,
        now_ms,
    );
}

fn apply_guard_with_node(
    node: &crate::TradeFlowNode,
    evaluation: &mut PriceToBeatGuardEvaluation,
    context: &serde_json::Value,
    now_ms: i64,
) {
    super::super::wait_reprice_guard::maybe_apply_wait_reprice_guard(
        node, context, evaluation, now_ms,
    );
}

fn mark_low_quality_edge(evaluation: &mut PriceToBeatGuardEvaluation) {
    if let Some(serde_json::Value::Object(iv)) = evaluation.iv_mismatch_edge.as_mut() {
        iv.insert("adjusted_margin".to_string(), json!(0.0398));
        iv.insert("ptb_movement_mode".to_string(), json!("medium_chop"));
        iv.insert("binance_veto_status".to_string(), json!("fail_open_stale"));
        iv.insert("binance_staleness_ms".to_string(), json!(1_161_391));
    }
}

fn mark_medium_chop_expensive_entry(evaluation: &mut PriceToBeatGuardEvaluation) {
    if let Some(serde_json::Value::Object(iv)) = evaluation.iv_mismatch_edge.as_mut() {
        iv.insert("ptb_movement_mode".to_string(), json!("medium_chop"));
        iv.insert("ptb_chop_gap_strength_penalty".to_string(), json!(0.10));
        iv.insert("book_confirmation_missing".to_string(), json!(true));
        iv.insert("adjusted_margin".to_string(), json!(0.1118));
    }
}

#[test]
fn wait_reprice_blocks_initial_ask_too_far_above_cap() {
    let context = json!({});
    let mut evaluation =
        iv_wait_evaluation("blocked_time_rule_max_price", 70.0, 88.0, 1.8, 1.5, 0.95);

    apply_guard(&mut evaluation, &context, 1_000);

    assert!(!evaluation.passed);
    assert_eq!(
        evaluation.reason_code,
        "blocked_initial_ask_too_far_above_cap"
    );
}

#[test]
fn wait_reprice_blocks_falling_into_cap_after_wait() {
    let context = waiting_context(1_000, 92.0, 96.0, 1.9, 95.0);
    let mut evaluation = iv_wait_evaluation("selected_edge_passed", 44.6, 73.0, 1.7, 1.5, 0.93);

    apply_guard(&mut evaluation, &context, 2_000);

    assert!(!evaluation.passed);
    assert_eq!(
        evaluation.reason_code,
        "blocked_falling_into_cap_after_wait"
    );
    let debug = evaluation
        .iv_mismatch_edge
        .as_ref()
        .and_then(|iv| iv.get("wait_reprice_guard"))
        .and_then(serde_json::Value::as_object)
        .expect("wait reprice debug");
    assert_eq!(
        debug
            .get("fell_into_cap")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[test]
fn wait_reprice_blocks_expired_wait_signal() {
    let context = waiting_context(0, 80.0, 80.0, 2.1, 95.0);
    let mut evaluation =
        iv_wait_evaluation("blocked_time_rule_max_price", 44.6, 78.0, 2.1, 1.5, 0.95);

    apply_guard(&mut evaluation, &context, 34_000);

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason_code, "blocked_wait_signal_expired");
}

#[test]
fn wait_reprice_blocks_late_expensive_weak_quality_entry() {
    let context = waiting_context(1_000, 80.0, 80.0, 1.8, 93.0);
    let mut evaluation = iv_wait_evaluation("selected_edge_passed", 44.6, 73.0, 1.8, 1.5, 0.89);

    apply_guard(&mut evaluation, &context, 2_000);

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason_code, "blocked_late_expensive_entry");
}

#[test]
fn wait_reprice_flag_off_preserves_existing_decision() {
    let node = test_action_place_order_node(json!({
        "priceToBeatIvWaitRepriceGuardEnabled": false
    }));
    let context = waiting_context(0, 92.0, 96.0, 1.9, 95.0);
    let mut evaluation = iv_wait_evaluation("selected_edge_passed", 44.6, 73.0, 1.7, 1.5, 0.89);

    super::super::wait_reprice_guard::maybe_apply_wait_reprice_guard(
        &node,
        &context,
        &mut evaluation,
        34_000,
    );

    assert!(evaluation.passed);
    assert_eq!(evaluation.reason_code, "selected_edge_passed");
}

#[test]
fn wait_reprice_omits_zero_gap_when_no_time_rule_matched() {
    let context = json!({});
    let mut evaluation =
        iv_wait_evaluation("blocked_no_matching_time_rule", 0.0, 0.0, 0.0, 0.0, 0.0);

    apply_guard(&mut evaluation, &context, 1_000);

    let debug = evaluation
        .iv_mismatch_edge
        .as_ref()
        .and_then(|iv| iv.get("wait_reprice_guard"))
        .and_then(serde_json::Value::as_object)
        .expect("wait reprice debug");
    assert!(
        debug
            .get("wait_initial_gap_strength")
            .is_some_and(serde_json::Value::is_null)
    );
    assert!(
        debug
            .get("wait_current_gap_strength")
            .is_some_and(serde_json::Value::is_null)
    );
}

#[test]
fn low_quality_edge_requests_recheck_on_first_pass() {
    let node = low_quality_recheck_node();
    let context = json!({});
    let mut evaluation =
        iv_wait_evaluation("selected_edge_passed", 62.61, 75.13, 1.8832, 1.85, 0.8171);
    mark_low_quality_edge(&mut evaluation);

    apply_guard_with_node(&node, &mut evaluation, &context, 2_000);

    assert!(!evaluation.passed);
    assert_eq!(evaluation.reason_code, "recheck_low_quality_edge");
    let debug = evaluation
        .iv_mismatch_edge
        .as_ref()
        .and_then(|iv| iv.get("wait_reprice_guard"))
        .and_then(serde_json::Value::as_object)
        .expect("wait reprice debug");
    assert_eq!(
        debug
            .get("low_quality_edge")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[test]
fn low_quality_edge_blocks_when_recheck_stays_weak() {
    let node = low_quality_recheck_node();
    let context = waiting_context_with_reason(
        "recheck_low_quality_edge",
        1_000,
        75.13,
        75.13,
        1.8832,
        81.71,
    );
    let mut evaluation =
        iv_wait_evaluation("selected_edge_passed", 62.61, 75.13, 1.8832, 1.85, 0.8171);
    mark_low_quality_edge(&mut evaluation);

    apply_guard_with_node(&node, &mut evaluation, &context, 2_000);

    assert!(!evaluation.passed);
    assert_eq!(
        evaluation.reason_code,
        "blocked_low_quality_edge_recheck_failed"
    );
    assert!(
        super::super::wait_reprice_guard::wait_reprice_reason_disables_retry(
            &evaluation.reason_code
        )
    );
}

#[test]
fn no_matching_time_rule_disables_retry() {
    assert!(
        super::super::wait_reprice_guard::wait_reprice_reason_disables_retry(
            "blocked_no_matching_time_rule"
        )
    );
}

#[test]
fn medium_chop_expensive_entry_requests_recheck_on_first_pass() {
    let node = low_quality_recheck_node();
    let context = json!({});
    let mut evaluation =
        iv_wait_evaluation("selected_edge_passed", 42.0, 74.05, 1.9951, 2.0, 0.8987);
    mark_medium_chop_expensive_entry(&mut evaluation);

    apply_guard_with_node(&node, &mut evaluation, &context, 2_000);

    assert!(!evaluation.passed);
    assert_eq!(
        evaluation.reason_code,
        "recheck_medium_chop_expensive_entry"
    );
    let debug = evaluation
        .iv_mismatch_edge
        .as_ref()
        .and_then(|iv| iv.get("wait_reprice_guard"))
        .and_then(serde_json::Value::as_object)
        .expect("wait reprice debug");
    assert_eq!(
        debug
            .get("medium_chop_expensive_entry")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[test]
fn medium_chop_expensive_entry_blocks_when_recheck_stays_weak() {
    let node = low_quality_recheck_node();
    let context = waiting_context_with_reason(
        "recheck_medium_chop_expensive_entry",
        1_000,
        74.05,
        74.05,
        1.9951,
        89.87,
    );
    let mut evaluation =
        iv_wait_evaluation("selected_edge_passed", 40.0, 74.05, 1.9951, 2.0, 0.8987);
    mark_medium_chop_expensive_entry(&mut evaluation);

    apply_guard_with_node(&node, &mut evaluation, &context, 2_000);

    assert!(!evaluation.passed);
    assert_eq!(
        evaluation.reason_code,
        "blocked_low_quality_edge_recheck_failed"
    );
}
