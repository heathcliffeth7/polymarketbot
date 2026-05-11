use super::*;

pub(super) fn from_candidate(
    selected: &ActionPlaceOrderPairLockPrimaryCandidateEval,
    selection_mode: &'static str,
    guard_reason: String,
) -> ActionPlaceOrderPairLockPrimarySelection {
    ActionPlaceOrderPairLockPrimarySelection {
        token_id: selected.token_id.clone(),
        outcome_label: selected.outcome_label.clone(),
        selection_mode,
        guard_reason,
        adaptive_max_price_override: selected.adaptive_max_price_override.clone(),
        manual_adaptive_risk_override: selected.manual_adaptive_risk_override.clone(),
    }
}

pub(super) fn attempt_with_selection(
    up_candidate: ActionPlaceOrderPairLockPrimaryCandidateEval,
    down_candidate: ActionPlaceOrderPairLockPrimaryCandidateEval,
    diagnostics: Value,
    primary_selection: ActionPlaceOrderPairLockPrimarySelection,
) -> ActionPlaceOrderPairLockPrimarySelectionAttempt {
    ActionPlaceOrderPairLockPrimarySelectionAttempt {
        selection: Some(primary_selection),
        waiting: false,
        failure_reason: None,
        yes_candidate: up_candidate,
        no_candidate: down_candidate,
        diagnostics,
    }
}

pub(super) fn preferred_best_ask_candidate<'a>(
    up_candidate: &'a ActionPlaceOrderPairLockPrimaryCandidateEval,
    down_candidate: &'a ActionPlaceOrderPairLockPrimaryCandidateEval,
) -> (
    &'a ActionPlaceOrderPairLockPrimaryCandidateEval,
    &'static str,
) {
    match (up_candidate.quote.best_ask, down_candidate.quote.best_ask) {
        (Some(up_ask), Some(down_ask)) if down_ask < up_ask => {
            (down_candidate, "selected_lower_best_ask")
        }
        (Some(up_ask), Some(down_ask)) if (up_ask - down_ask).abs() <= f64::EPSILON => {
            (up_candidate, "selected_equal_best_ask_up_fallback")
        }
        (None, Some(_)) => (down_candidate, "selected_lower_best_ask"),
        (Some(_), None) => (up_candidate, "selected_lower_best_ask"),
        (None, None) => (up_candidate, "selected_missing_best_ask_up_fallback"),
        _ => (up_candidate, "selected_lower_best_ask"),
    }
}
