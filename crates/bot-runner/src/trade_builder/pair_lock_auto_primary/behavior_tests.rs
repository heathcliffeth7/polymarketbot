use super::*;

fn pair_lock_candidate(
    token_id: &str,
    outcome_label: &str,
    decision: &'static str,
    reason_code: &str,
    best_ask: Option<f64>,
) -> ActionPlaceOrderPairLockPrimaryCandidateEval {
    let current_price = best_ask.unwrap_or(0.5);
    ActionPlaceOrderPairLockPrimaryCandidateEval {
        token_id: token_id.to_string(),
        outcome_label: outcome_label.to_string(),
        decision,
        reason_code: reason_code.to_string(),
        quote: PairLockResolvedQuote {
            best_bid: best_ask.map(|price| (price - 0.01).max(0.0)),
            best_ask,
            last_trade_price: best_ask,
            current_price,
            quote_source_kind: "test",
            quote_ws_state: "live_ws_not_subscribed",
            quote_event_ts: None,
            quote_snapshot_age_ms: None,
            quote_source_detail: "test".to_string(),
            quote_book_missing_fields: Vec::new(),
            quote_snapshot_used: Value::Null,
        },
        diagnostics: json!({
            "decision": decision,
            "reason_code": reason_code,
            "outcome_label": outcome_label,
            "best_ask": best_ask
        }),
        adaptive_max_price_override: None,
        manual_adaptive_risk_override: None,
    }
}

fn notification_node(config: Value) -> TradeFlowNode {
    TradeFlowNode {
        key: "pair_buy".to_string(),
        node_type: "action.place_order".to_string(),
        config,
    }
}

#[test]
fn pair_lock_auto_primary_selects_lower_best_ask_when_both_candidates_pass() {
    let selection = resolve_action_place_order_pair_lock_primary_selection_attempt(
        pair_lock_candidate("yes", "Up", "passed", "passed", Some(0.53)),
        pair_lock_candidate("no", "Down", "passed", "passed", Some(0.50)),
    );

    let selected = selection.selection.expect("primary selection");
    assert_eq!(selected.token_id, "no");
    assert_eq!(selected.outcome_label, "Down");
    assert_eq!(selected.selection_mode, "auto_guarded_best_ask");
    assert_eq!(selected.guard_reason, "selected_lower_best_ask");
}

#[test]
fn pair_lock_auto_primary_selects_up_when_both_candidates_pass_with_equal_best_ask() {
    let selection = resolve_action_place_order_pair_lock_primary_selection_attempt(
        pair_lock_candidate("yes", "Up", "passed", "passed", Some(0.50)),
        pair_lock_candidate("no", "Down", "passed", "passed", Some(0.50)),
    );

    let selected = selection.selection.expect("primary selection");
    assert_eq!(selected.token_id, "yes");
    assert_eq!(selected.outcome_label, "Up");
    assert_eq!(selected.selection_mode, "auto_guarded_best_ask");
    assert_eq!(selected.guard_reason, "selected_equal_best_ask_up_fallback");
}

#[test]
fn pair_lock_primary_notification_uses_enabled_max_price_when_floor_notify_is_disabled() {
    let node = notification_node(json!({
        "notifyOnMaxPriceBlocked": true,
        "notifyOnExecutionFloorBlocked": false
    }));
    let diagnostics = json!({
        "selection_mode": "auto_guarded",
        "yes_candidate_guard": {
            "decision": "waiting",
            "reason_code": "below_best_ask_floor",
            "outcome_label": "Up"
        },
        "no_candidate_guard": {
            "decision": "waiting",
            "reason_code": "above_max_price",
            "outcome_label": "Down"
        }
    });

    let reason = notification_selection::resolve_for_node(&node, &diagnostics).expect("reason");

    assert_eq!(reason.scope, "max_price");
    assert_eq!(reason.reason_code, "above_max_price");
    assert_eq!(
        reason
            .secondary_candidate
            .as_ref()
            .and_then(|value| value.get("reason_code"))
            .and_then(Value::as_str),
        Some("below_best_ask_floor")
    );
}
