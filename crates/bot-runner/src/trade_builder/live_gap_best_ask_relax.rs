const LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_MAX_PRICE: f64 = 0.99;
const LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_REASON: &str = "best_ask_unavailable_relaxed_waiting";
const LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_SKIP: &str =
    "skipped_by_best_ask_unavailable_relax";

fn live_gap_collector_effective_max_price(
    max_price: Option<f64>,
    config: Option<&ActionPlaceOrderLiveGapCollectorConfig>,
    context: Option<&Value>,
) -> Option<f64> {
    config
        .map(|cfg| {
            if live_gap_collector_best_ask_unavailable_relax_applied(context) {
                return LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_MAX_PRICE;
            }
            max_price
                .unwrap_or(cfg.hard_max_price)
                .min(cfg.hard_max_price)
        })
        .or(max_price)
}

fn live_gap_collector_best_ask(order_book: &OrderBookSnapshot) -> Option<f64> {
    order_book
        .asks
        .iter()
        .filter(|level| level.price.is_finite() && level.price > 0.0 && level.price < 1.0)
        .map(|level| level.price)
        .min_by(f64::total_cmp)
}

fn live_gap_collector_normalize_probability(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value > 0.0 && *value < 1.0)
}

fn live_gap_collector_best_ask_unavailable_relax_applied(context: Option<&Value>) -> bool {
    context
        .and_then(live_gap_collector_context_payload)
        .and_then(|payload| {
            payload
                .get("best_ask_unavailable_relax")
                .and_then(|value| value.get("applied"))
                .and_then(Value::as_bool)
        })
        .unwrap_or(false)
}

fn live_gap_collector_append_best_ask_unavailable_relax(
    payload: &mut Value,
    fallback_best_bid: Option<f64>,
    fallback_best_ask: Option<f64>,
    best_ask_source: &str,
) {
    let skipped = json!([
        "depth_guard",
        "effective_fill_hard_max",
        "pre_buy_collapse",
        "no_reversal"
    ]);
    if let Some(obj) = payload.as_object_mut() {
        obj.insert(
            "best_ask_unavailable_relax".to_string(),
            json!({
                "applied": true,
                "reason_code": LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_REASON,
                "best_ask_source": best_ask_source,
                "fallback_best_bid": fallback_best_bid,
                "fallback_best_ask": fallback_best_ask,
                "relaxed_effective_max_price": LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_MAX_PRICE,
                "price_dependent_guards_skipped": skipped,
            }),
        );
        obj.insert("fallback_best_bid".to_string(), json!(fallback_best_bid));
        obj.insert("fallback_best_ask".to_string(), json!(fallback_best_ask));
        obj.insert(
            "relaxed_effective_max_price".to_string(),
            json!(LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_MAX_PRICE),
        );
        obj.insert("price_dependent_guards_skipped".to_string(), skipped);
        obj.insert(
            "depth_guard_result".to_string(),
            json!(LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_SKIP),
        );
        obj.insert(
            "depth_guard_reason".to_string(),
            json!(LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_SKIP),
        );
        obj.insert(
            "effective_fill_hard_max_guard".to_string(),
            json!(LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_SKIP),
        );
        obj.insert(
            "pre_buy_collapse_guard".to_string(),
            json!({ "decision": LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_SKIP }),
        );
        obj.insert(
            "no_reversal_entry_guard".to_string(),
            json!({ "decision": LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_SKIP }),
        );
        obj.insert(
            "candidate_guard_reason".to_string(),
            json!(LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_REASON),
        );
        obj.insert(
            "reason_code".to_string(),
            json!(LIVE_GAP_BEST_ASK_UNAVAILABLE_RELAX_REASON),
        );
    }
}

fn live_gap_collector_intended_qty(sizing: &ActionPlaceOrderSizing, best_ask: f64) -> Option<f64> {
    sizing
        .target_qty
        .filter(|qty| qty.is_finite() && *qty > 0.0)
        .or_else(|| {
            (best_ask.is_finite() && best_ask > 0.0).then_some(sizing.size_usdc / best_ask)
        })
        .filter(|qty| qty.is_finite() && *qty > 0.0)
}
