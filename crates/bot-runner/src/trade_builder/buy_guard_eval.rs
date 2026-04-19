#[derive(Debug, Clone)]
struct TradeBuilderBuyGuardEvaluation {
    trigger_price_guard_payload: Value,
    execution_floor_payload: Value,
    max_price_payload: Value,
    trigger_guard_reference_price: f64,
    trigger_guard_reference_source: &'static str,
    trigger_price_guard_blocked: bool,
    execution_floor_reason: Option<&'static str>,
    max_price_reference: f64,
    max_price_reference_source: &'static str,
    max_price_blocked: bool,
    pair_lock_market_waiting_reason: Option<&'static str>,
    effective_decision: &'static str,
    effective_reason_code: &'static str,
}

fn trade_builder_guard_diagnostic_payload(
    configured: bool,
    decision: &str,
    reason_code: &str,
    details: Value,
) -> Value {
    json!({
        "configured": configured,
        "decision": decision,
        "reason_code": reason_code,
        "details": details,
    })
}

#[allow(clippy::too_many_arguments)]
fn evaluate_trade_builder_buy_guards(
    execution_mode: &str,
    pair_leg_role: Option<&str>,
    current_price: f64,
    best_ask: Option<f64>,
    desired_price: f64,
    guard_trigger_price: Option<f64>,
    max_price: Option<f64>,
    best_ask_floor_price: Option<f64>,
    retry_on_trigger_guard_block: bool,
    retry_on_execution_floor_guard_block: bool,
    retry_on_max_price_block: bool,
) -> TradeBuilderBuyGuardEvaluation {
    let trigger_price_guard_configured = guard_trigger_price.is_some();
    let (trigger_guard_reference_price, trigger_guard_reference_source) = match best_ask
        .and_then(|value| normalize_trade_builder_reference_price(Some(value)))
        .map(clamp_probability)
    {
        Some(best_ask) => (best_ask, "best_ask"),
        None => (
            normalize_trade_builder_reference_price(Some(current_price))
                .map(clamp_probability)
                .unwrap_or_else(|| clamp_probability(current_price)),
            "current_price_fallback",
        ),
    };
    let trigger_price_guard_blocked = guard_trigger_price
        .map(|guard| trigger_guard_reference_price.is_finite() && trigger_guard_reference_price < guard)
        .unwrap_or(false);
    let trigger_price_guard_payload = if trigger_price_guard_blocked {
        trade_builder_guard_diagnostic_payload(
            true,
            if retry_on_trigger_guard_block {
                "waiting"
            } else {
                "blocked"
            },
            "below_trigger_price_guard",
            json!({
                "guard_trigger_price": guard_trigger_price,
                "current_price": current_price,
                "trigger_guard_reference_price": trigger_guard_reference_price,
                "trigger_guard_reference_source": trigger_guard_reference_source,
            }),
        )
    } else if trigger_price_guard_configured {
        trade_builder_guard_diagnostic_payload(
            true,
            "passed",
            "passed",
            json!({
                "guard_trigger_price": guard_trigger_price,
                "current_price": current_price,
                "trigger_guard_reference_price": trigger_guard_reference_price,
                "trigger_guard_reference_source": trigger_guard_reference_source,
            }),
        )
    } else {
        trade_builder_guard_diagnostic_payload(false, "not_configured", "not_configured", Value::Null)
    };

    let execution_floor_reason = if best_ask_floor_price.is_some()
        && best_ask
            .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
            .is_none()
    {
        Some("best_ask_unavailable")
    } else {
        match (
            best_ask_floor_price,
            best_ask.filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0),
        ) {
            (Some(floor), Some(best_ask)) if best_ask < floor => Some("below_best_ask_floor"),
            _ => None,
        }
    };
    let pair_lock_market_waiting_reason =
        if execution_mode == "market"
            && best_ask
                .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
                .is_none()
        {
            match pair_leg_role {
                Some("counter_candidate") => Some("pair_counter_best_ask_unavailable"),
                Some("lead_candidate") => Some("pair_primary_best_ask_unavailable"),
                _ => None,
            }
        } else {
            None
        };
    let execution_floor_configured = best_ask_floor_price.is_some();
    let execution_floor_payload = if let Some(reason_code) = execution_floor_reason {
        trade_builder_guard_diagnostic_payload(
            true,
            if reason_code == "best_ask_unavailable" || retry_on_execution_floor_guard_block {
                "waiting"
            } else {
                "blocked"
            },
            reason_code,
            json!({
                "best_ask_floor_price": best_ask_floor_price,
                "best_ask": best_ask,
            }),
        )
    } else if execution_floor_configured {
        trade_builder_guard_diagnostic_payload(
            true,
            "passed",
            "passed",
            json!({
                "best_ask_floor_price": best_ask_floor_price,
                "best_ask": best_ask,
            }),
        )
    } else {
        trade_builder_guard_diagnostic_payload(false, "not_configured", "not_configured", Value::Null)
    };

    let max_price_configured = max_price.is_some();
    let (max_price_reference, max_price_reference_source) = match best_ask
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
    {
        Some(best_ask) => (best_ask, "best_ask"),
        None => (desired_price, "desired_price_fallback"),
    };
    let max_price_blocked = max_price
        .map(|threshold| max_price_reference.is_finite() && max_price_reference > threshold)
        .unwrap_or(false);
    let max_price_payload = if max_price_blocked {
        trade_builder_guard_diagnostic_payload(
            true,
            if retry_on_max_price_block {
                "waiting"
            } else {
                "blocked"
            },
            "above_max_price",
            json!({
                "max_price": max_price,
                "current_price": current_price,
                "desired_price": desired_price,
                "reference_price": max_price_reference,
                "reference_price_source": max_price_reference_source,
            }),
        )
    } else if max_price_configured {
        trade_builder_guard_diagnostic_payload(
            true,
            "passed",
            "passed",
            json!({
                "max_price": max_price,
                "current_price": current_price,
                "desired_price": desired_price,
                "reference_price": max_price_reference,
                "reference_price_source": max_price_reference_source,
            }),
        )
    } else {
        trade_builder_guard_diagnostic_payload(false, "not_configured", "not_configured", Value::Null)
    };

    if let Some(reason_code) = pair_lock_market_waiting_reason {
        return TradeBuilderBuyGuardEvaluation {
            trigger_price_guard_payload,
            execution_floor_payload,
            max_price_payload,
            trigger_guard_reference_price,
            trigger_guard_reference_source,
            trigger_price_guard_blocked,
            execution_floor_reason,
            max_price_reference,
            max_price_reference_source,
            max_price_blocked,
            pair_lock_market_waiting_reason: Some(reason_code),
            effective_decision: "waiting",
            effective_reason_code: reason_code,
        };
    }

    if trigger_price_guard_blocked {
        return TradeBuilderBuyGuardEvaluation {
            trigger_price_guard_payload,
            execution_floor_payload,
            max_price_payload,
            trigger_guard_reference_price,
            trigger_guard_reference_source,
            trigger_price_guard_blocked,
            execution_floor_reason,
            max_price_reference,
            max_price_reference_source,
            max_price_blocked,
            pair_lock_market_waiting_reason: None,
            effective_decision: if retry_on_trigger_guard_block {
                "waiting"
            } else {
                "blocked"
            },
            effective_reason_code: "below_trigger_price_guard",
        };
    }

    if let Some(reason_code) = execution_floor_reason {
        return TradeBuilderBuyGuardEvaluation {
            trigger_price_guard_payload,
            execution_floor_payload,
            max_price_payload,
            trigger_guard_reference_price,
            trigger_guard_reference_source,
            trigger_price_guard_blocked,
            execution_floor_reason: Some(reason_code),
            max_price_reference,
            max_price_reference_source,
            max_price_blocked,
            pair_lock_market_waiting_reason: None,
            effective_decision: if reason_code == "best_ask_unavailable"
                || retry_on_execution_floor_guard_block
            {
                "waiting"
            } else {
                "blocked"
            },
            effective_reason_code: reason_code,
        };
    }

    if max_price_blocked {
        return TradeBuilderBuyGuardEvaluation {
            trigger_price_guard_payload,
            execution_floor_payload,
            max_price_payload,
            trigger_guard_reference_price,
            trigger_guard_reference_source,
            trigger_price_guard_blocked,
            execution_floor_reason,
            max_price_reference,
            max_price_reference_source,
            max_price_blocked,
            pair_lock_market_waiting_reason: None,
            effective_decision: if retry_on_max_price_block {
                "waiting"
            } else {
                "blocked"
            },
            effective_reason_code: "above_max_price",
        };
    }

    TradeBuilderBuyGuardEvaluation {
        trigger_price_guard_payload,
        execution_floor_payload,
        max_price_payload,
        trigger_guard_reference_price,
        trigger_guard_reference_source,
        trigger_price_guard_blocked,
        execution_floor_reason,
        max_price_reference,
        max_price_reference_source,
        max_price_blocked,
        pair_lock_market_waiting_reason: None,
        effective_decision: "passed",
        effective_reason_code: "guards_passed",
    }
}
