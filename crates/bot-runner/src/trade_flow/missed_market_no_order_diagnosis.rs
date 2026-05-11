const NO_ORDER_VOLUME_BASELINE_LOOKBACK_DAYS: i64 = 7;
const NO_ORDER_VOLUME_BASELINE_MIN_SAMPLES: i64 = 20;
const NO_ORDER_VOLUME_WINDOW_SEC: i64 = 30;

fn no_order_valid_f64(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite())
}

fn no_order_json_f64(value: Option<f64>) -> Value {
    match no_order_valid_f64(value) {
        Some(value) => json!(value),
        None => Value::Null,
    }
}

fn no_order_format_ratio(value: Option<f64>) -> String {
    no_order_valid_f64(value)
        .map(|value| format!("{value:.2}x"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn no_order_format_pct(value: Option<f64>) -> String {
    no_order_valid_f64(value)
        .map(|value| format!("{:.2}%", value * 100.0))
        .unwrap_or_else(|| "N/A".to_string())
}

fn no_order_bool_at_path(payload: &Value, path: &[&str]) -> Option<bool> {
    let mut value = payload;
    for key in path {
        value = value.get(*key)?;
    }
    value.as_bool()
}

fn no_order_candidate_best_ask(candidate: &Value) -> Option<f64> {
    no_fill_optional_f64(candidate, &["execution_floor_guard", "details", "best_ask"])
        .or_else(|| no_fill_optional_f64(candidate, &["best_ask"]))
}

fn no_order_candidate_floor(candidate: &Value) -> Option<f64> {
    no_fill_optional_f64(
        candidate,
        &["execution_floor_guard", "details", "best_ask_floor_price"],
    )
    .or_else(|| no_fill_optional_f64(candidate, &["best_ask_floor_price"]))
}

fn no_order_summary_best_ask(summary: &TradeBuilderNoFillReasonSummary) -> Option<f64> {
    let details = no_fill_payload_details(&summary.payload);
    no_fill_optional_f64(details, &["best_ask"])
        .or_else(|| no_fill_optional_f64(&summary.payload, &["best_ask"]))
}

fn no_order_summary_floor(summary: &TradeBuilderNoFillReasonSummary) -> Option<f64> {
    let details = no_fill_payload_details(&summary.payload);
    no_fill_optional_f64(details, &["best_ask_floor_price"])
        .or_else(|| no_fill_optional_f64(&summary.payload, &["best_ask_floor_price"]))
}

fn no_order_step_time(step: &TradeFlowRunStep) -> DateTime<Utc> {
    step.ended_at.or(step.started_at).unwrap_or(step.created_at)
}

fn no_order_step_target_candidate<'a>(
    step: &'a TradeFlowRunStep,
    token_id: &str,
    outcome_label: &str,
) -> Option<&'a Value> {
    let output = step.output_json.as_ref()?;
    no_fill_pair_lock_candidate_for_target(output, token_id, outcome_label)
}

fn no_order_floor_recovered(candidate: &Value, best_ask: Option<f64>, floor: Option<f64>) -> bool {
    no_order_bool_at_path(candidate, &["execution_floor_guard", "passed"]) == Some(true)
        || no_fill_optional_str(candidate, &["execution_floor_guard", "decision"])
            == Some("passed")
        || matches!((best_ask, floor), (Some(best_ask), Some(floor)) if best_ask >= floor)
}

fn no_order_floor_history(
    token_id: &str,
    outcome_label: &str,
    action_steps: &[TradeFlowRunStep],
) -> (Option<i64>, bool, Option<f64>, Option<f64>) {
    let mut first_at: Option<DateTime<Utc>> = None;
    let mut last_at: Option<DateTime<Utc>> = None;
    let mut recovered_once = false;
    let mut min_best_ask: Option<f64> = None;
    let mut max_best_ask: Option<f64> = None;

    for step in action_steps {
        let Some(candidate) = no_order_step_target_candidate(step, token_id, outcome_label) else {
            continue;
        };
        let best_ask = no_order_candidate_best_ask(candidate);
        let floor = no_order_candidate_floor(candidate);
        let step_at = no_order_step_time(step);
        first_at = Some(first_at.map_or(step_at, |current| current.min(step_at)));
        last_at = Some(last_at.map_or(step_at, |current| current.max(step_at)));
        recovered_once |= no_order_floor_recovered(candidate, best_ask, floor);
        if let Some(best_ask) = no_order_valid_f64(best_ask) {
            min_best_ask = Some(min_best_ask.map_or(best_ask, |current| current.min(best_ask)));
            max_best_ask = Some(max_best_ask.map_or(best_ask, |current| current.max(best_ask)));
        }
    }

    let wait_ms = match (first_at, last_at) {
        (Some(first_at), Some(last_at)) => Some(
            last_at
                .signed_duration_since(first_at)
                .num_milliseconds()
                .max(0),
        ),
        _ => None,
    };
    (wait_ms, recovered_once, min_best_ask, max_best_ask)
}

fn no_order_candidate_for_side<'a>(output: &'a Value, side: &str) -> Option<&'a Value> {
    for path in [
        &["yes_candidate_guard"][..],
        &["no_candidate_guard"][..],
        &["primary_selection", "yes_candidate_guard"][..],
        &["primary_selection", "no_candidate_guard"][..],
    ] {
        let Some(candidate) = no_fill_value_at_path(output, path) else {
            continue;
        };
        if candidate
            .get("outcome_label")
            .and_then(Value::as_str)
            .map(|label| no_fill_outcome_matches(label, side))
            .unwrap_or(false)
        {
            return Some(candidate);
        }
    }
    None
}

fn no_order_probability(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
}

fn no_order_mid(bid: Option<f64>, ask: Option<f64>) -> Option<f64> {
    let bid = no_order_probability(bid)?;
    let ask = no_order_probability(ask)?;
    (ask >= bid).then_some((bid + ask) / 2.0)
}

fn no_order_quote_source(
    action_value: Option<f64>,
    snapshot_value: Option<f64>,
    final_value: Option<f64>,
) -> Option<&'static str> {
    if no_order_probability(action_value).is_some() {
        Some("action_output")
    } else if no_order_probability(snapshot_value).is_some() {
        Some("second_snapshot")
    } else if no_order_probability(final_value).is_some() {
        Some("final_fetch")
    } else {
        None
    }
}

fn no_order_select_quote(
    action_value: Option<f64>,
    snapshot_value: Option<f64>,
    final_value: Option<f64>,
) -> Option<f64> {
    no_order_probability(action_value)
        .or_else(|| no_order_probability(snapshot_value))
        .or_else(|| no_order_probability(final_value))
}

fn no_order_snapshot_depth(
    snapshot_value: Option<f64>,
    final_value: Option<f64>,
) -> Option<f64> {
    no_order_valid_f64(snapshot_value).or_else(|| no_order_valid_f64(final_value))
}

fn no_order_final_quote_f64(final_quote_payload: Option<&Value>, key: &str) -> Option<f64> {
    final_quote_payload.and_then(|payload| no_fill_optional_f64(payload, &[key]))
}

fn no_order_book_side_from_quotes(
    up_mid: Option<f64>,
    down_mid: Option<f64>,
    up_ask: Option<f64>,
    down_ask: Option<f64>,
) -> (Option<&'static str>, Option<f64>, Option<&'static str>) {
    if let (Some(up_mid), Some(down_mid)) = (up_mid, down_mid) {
        let diff = up_mid - down_mid;
        let side = if diff > 0.01 {
            "Up"
        } else if diff < -0.01 {
            "Down"
        } else {
            "Balanced"
        };
        return (Some(side), Some(diff), Some("mid_diff"));
    }
    if let (Some(up_ask), Some(down_ask)) = (up_ask, down_ask) {
        let diff = up_ask - down_ask;
        let side = if diff > 0.01 {
            "Up"
        } else if diff < -0.01 {
            "Down"
        } else {
            "Balanced"
        };
        return (Some(side), None, Some("ask_diff"));
    }
    (None, None, None)
}

fn no_order_book_data_status(
    selected_bid: Option<f64>,
    selected_ask: Option<f64>,
    up_bid: Option<f64>,
    up_ask: Option<f64>,
    down_bid: Option<f64>,
    down_ask: Option<f64>,
) -> &'static str {
    let selected_complete = selected_bid.is_some() && selected_ask.is_some();
    let up_complete = up_bid.is_some() && up_ask.is_some();
    let down_complete = down_bid.is_some() && down_ask.is_some();
    if up_complete && down_complete {
        "complete_pair_book"
    } else if selected_complete {
        "selected_side_only"
    } else if [up_bid, up_ask, down_bid, down_ask]
        .into_iter()
        .any(|value| value.is_some())
    {
        "incomplete_pair_book"
    } else {
        "unavailable"
    }
}

fn no_order_quote_missing_reason(
    status: &str,
    selected_is_down: bool,
    up_bid: Option<f64>,
    up_ask: Option<f64>,
    down_bid: Option<f64>,
    down_ask: Option<f64>,
) -> Option<&'static str> {
    match status {
        "complete_pair_book" => None,
        "unavailable" => Some("no quote snapshot available"),
        "selected_side_only" if selected_is_down => Some("Up quote missing"),
        "selected_side_only" => Some("Down quote missing"),
        _ if selected_is_down && (down_bid.is_none() || down_ask.is_none()) => {
            Some("selected Down quote incomplete")
        }
        _ if !selected_is_down && (up_bid.is_none() || up_ask.is_none()) => {
            Some("selected Up quote incomplete")
        }
        _ if up_bid.is_none() || up_ask.is_none() => Some("Up quote incomplete"),
        _ if down_bid.is_none() || down_ask.is_none() => Some("Down quote incomplete"),
        _ => Some("pair quote incomplete"),
    }
}

fn no_order_latest_snapshot<'a>(
    snapshots: &'a [TradeBuilderMarketSecondSnapshot],
    window_end_at: DateTime<Utc>,
) -> Option<&'a TradeBuilderMarketSecondSnapshot> {
    snapshots
        .iter()
        .filter(|snapshot| snapshot.second_ts <= window_end_at)
        .max_by_key(|snapshot| snapshot.second_ts)
        .or_else(|| snapshots.iter().max_by_key(|snapshot| snapshot.second_ts))
}

fn no_order_book_payload(
    output: Option<&Value>,
    outcome_label: &str,
    snapshot: Option<&TradeBuilderMarketSecondSnapshot>,
    final_quote_payload: Option<&Value>,
) -> Value {
    let up_candidate = output.and_then(|output| no_order_candidate_for_side(output, "Up"));
    let down_candidate = output.and_then(|output| no_order_candidate_for_side(output, "Down"));
    let up_bid_action = up_candidate.and_then(|candidate| no_fill_optional_f64(candidate, &["best_bid"]));
    let up_ask_action = up_candidate.and_then(|candidate| no_fill_optional_f64(candidate, &["best_ask"]));
    let down_bid_action =
        down_candidate.and_then(|candidate| no_fill_optional_f64(candidate, &["best_bid"]));
    let down_ask_action =
        down_candidate.and_then(|candidate| no_fill_optional_f64(candidate, &["best_ask"]));
    let up_bid_snapshot = snapshot.and_then(|snapshot| snapshot.yes_best_bid);
    let up_ask_snapshot = snapshot.and_then(|snapshot| snapshot.yes_best_ask);
    let down_bid_snapshot = snapshot.and_then(|snapshot| snapshot.no_best_bid);
    let down_ask_snapshot = snapshot.and_then(|snapshot| snapshot.no_best_ask);
    let up_bid = no_order_select_quote(
        up_bid_action,
        up_bid_snapshot,
        no_order_final_quote_f64(final_quote_payload, "up_bid"),
    );
    let up_ask = no_order_select_quote(
        up_ask_action,
        up_ask_snapshot,
        no_order_final_quote_f64(final_quote_payload, "up_ask"),
    );
    let down_bid = no_order_select_quote(
        down_bid_action,
        down_bid_snapshot,
        no_order_final_quote_f64(final_quote_payload, "down_bid"),
    );
    let down_ask = no_order_select_quote(
        down_ask_action,
        down_ask_snapshot,
        no_order_final_quote_f64(final_quote_payload, "down_ask"),
    );
    let up_mid = no_order_mid(up_bid, up_ask);
    let down_mid = no_order_mid(down_bid, down_ask);
    let (book_side, book_mid_diff, book_side_reason) =
        no_order_book_side_from_quotes(up_mid, down_mid, up_ask, down_ask);
    let selected_is_down = no_fill_outcome_matches(outcome_label, "Down");
    let selected_bid = if selected_is_down { down_bid } else { up_bid };
    let selected_ask = if selected_is_down { down_ask } else { up_ask };
    let selected_mid = if selected_is_down { down_mid } else { up_mid };
    let selected_bid_source = if selected_is_down {
        no_order_quote_source(
            down_bid_action,
            down_bid_snapshot,
            no_order_final_quote_f64(final_quote_payload, "down_bid"),
        )
    } else {
        no_order_quote_source(
            up_bid_action,
            up_bid_snapshot,
            no_order_final_quote_f64(final_quote_payload, "up_bid"),
        )
    };
    let selected_ask_source = if selected_is_down {
        no_order_quote_source(
            down_ask_action,
            down_ask_snapshot,
            no_order_final_quote_f64(final_quote_payload, "down_ask"),
        )
    } else {
        no_order_quote_source(
            up_ask_action,
            up_ask_snapshot,
            no_order_final_quote_f64(final_quote_payload, "up_ask"),
        )
    };
    let quote_snapshot_source = selected_ask_source
        .or(selected_bid_source)
        .or_else(|| final_quote_payload.and_then(|payload| payload.get("quote_snapshot_source").and_then(Value::as_str)))
        .unwrap_or("unavailable");
    let selected_spread = match (selected_bid, selected_ask) {
        (Some(bid), Some(ask)) if ask >= bid => Some(ask - bid),
        _ => None,
    };
    let selected_depth = if selected_is_down {
        no_order_snapshot_depth(
            snapshot.and_then(|snapshot| snapshot.no_ask_depth_usdc),
            no_order_final_quote_f64(final_quote_payload, "down_ask_depth_usdc"),
        )
    } else {
        no_order_snapshot_depth(
            snapshot.and_then(|snapshot| snapshot.yes_ask_depth_usdc),
            no_order_final_quote_f64(final_quote_payload, "up_ask_depth_usdc"),
        )
    };
    let book_data_status =
        no_order_book_data_status(selected_bid, selected_ask, up_bid, up_ask, down_bid, down_ask);
    let quote_missing_reason =
        no_order_quote_missing_reason(book_data_status, selected_is_down, up_bid, up_ask, down_bid, down_ask);

    json!({
        "quote_snapshot_source": quote_snapshot_source,
        "book_data_status": book_data_status,
        "quote_missing_reason": quote_missing_reason,
        "up_bid": no_order_json_f64(up_bid),
        "up_ask": no_order_json_f64(up_ask),
        "down_bid": no_order_json_f64(down_bid),
        "down_ask": no_order_json_f64(down_ask),
        "up_mid": no_order_json_f64(up_mid),
        "down_mid": no_order_json_f64(down_mid),
        "selected_bid": no_order_json_f64(selected_bid),
        "selected_ask": no_order_json_f64(selected_ask),
        "selected_mid": no_order_json_f64(selected_mid),
        "book_side": book_side,
        "book_mid_diff": no_order_json_f64(book_mid_diff),
        "book_side_reason": book_side_reason,
        "selected_side_spread": no_order_json_f64(selected_spread),
        "selected_side_depth": no_order_json_f64(selected_depth),
    })
}

fn no_order_smooth_hourly_volume_baseline(
    hour: i32,
    baselines: &[bot_infra::db::MarketTradeHourlyVolumeMedian],
    min_samples: i64,
) -> (Option<f64>, i64, &'static str) {
    let weights = [
        (hour, 0.60),
        ((hour + 23) % 24, 0.20),
        ((hour + 1) % 24, 0.20),
    ];
    let mut weighted_sum = 0.0;
    let mut weight_sum = 0.0;
    let mut sample_count = 0_i64;
    for (candidate_hour, weight) in weights {
        let Some(baseline) = baselines
            .iter()
            .find(|item| item.hour_utc == candidate_hour)
        else {
            continue;
        };
        if baseline.sample_count <= 0 || !baseline.median_volume_usdc.is_finite() {
            continue;
        }
        weighted_sum += baseline.median_volume_usdc * weight;
        weight_sum += weight;
        sample_count += baseline.sample_count;
    }
    if sample_count >= min_samples.max(0) && weight_sum > 0.0 {
        (
            Some(weighted_sum / weight_sum),
            sample_count,
            "hourly_ready",
        )
    } else {
        (None, sample_count, "hourly_insufficient_samples")
    }
}

fn no_order_liquidity_regime_for_ratio(ratio: Option<f64>) -> Option<&'static str> {
    let ratio = ratio.filter(|value| value.is_finite() && *value >= 0.0)?;
    Some(if ratio < 0.50 {
        "VERY_LOW"
    } else if ratio < 0.80 {
        "LOW"
    } else if ratio < 1.50 {
        "NORMAL"
    } else if ratio < 3.00 {
        "HIGH"
    } else {
        "EXTREME"
    })
}

fn no_order_liquidity_note_for_ratio(ratio: Option<f64>) -> Option<&'static str> {
    let ratio = ratio.filter(|value| value.is_finite() && *value >= 0.0)?;
    if ratio < 0.50 {
        Some("Bu saat/bucket icin hacim cok dusuk.")
    } else if ratio < 0.80 {
        Some("Hacim normalin altinda.")
    } else {
        None
    }
}

async fn build_no_order_liquidity_payload(
    repo: &PostgresRepository,
    market_slug: &str,
    window_end_at: DateTime<Utc>,
) -> Value {
    let volume_summary = match repo
        .market_trade_volume_summary(market_slug, window_end_at)
        .await
    {
        Ok(summary) => summary,
        Err(err) => {
            warn!(
                market_slug = %market_slug,
                error = %err,
                "NO_ORDER_LIQUIDITY_SUMMARY_FAILED"
            );
            return json!({ "liquidity_status": "volume_query_failed" });
        }
    };
    let Some(scope) = find_updown_scope_by_slug(market_slug) else {
        return json!({
            "volume_10s": volume_summary.volume_10s,
            "volume_30s": volume_summary.volume_30s,
            "volume_60s": volume_summary.volume_60s,
            "trade_count_60s": volume_summary.trade_count_60s,
            "liquidity_status": "asset_unavailable",
        });
    };
    let hour = chrono::Timelike::hour(&window_end_at) as i32;
    let hours = [hour, (hour + 23) % 24, (hour + 1) % 24];
    let bucket_start_sec = 30.0;
    let bucket_end_sec = 0.0;
    let bucket_label = "30-0";
    let baselines = repo
        .list_market_trade_hourly_volume_medians(
            scope.asset,
            &hours,
            bucket_start_sec,
            bucket_end_sec,
            NO_ORDER_VOLUME_BASELINE_LOOKBACK_DAYS,
            NO_ORDER_VOLUME_WINDOW_SEC,
            market_slug,
            window_end_at,
        )
        .await
        .unwrap_or_default();
    let (baseline, sample_count, status) = no_order_smooth_hourly_volume_baseline(
        hour,
        &baselines,
        NO_ORDER_VOLUME_BASELINE_MIN_SAMPLES,
    );
    let (baseline, sample_count, status) = if baseline.is_some() {
        (baseline, sample_count, status)
    } else {
        match repo
            .market_trade_volume_bucket_median(
                scope.asset,
                bucket_start_sec,
                bucket_end_sec,
                NO_ORDER_VOLUME_BASELINE_LOOKBACK_DAYS,
                NO_ORDER_VOLUME_WINDOW_SEC,
                market_slug,
                window_end_at,
            )
            .await
        {
            Ok(fallback)
                if fallback.sample_count >= NO_ORDER_VOLUME_BASELINE_MIN_SAMPLES
                    && fallback.median_volume_usdc.is_finite()
                    && fallback.median_volume_usdc > 0.0 =>
            {
                (
                    Some(fallback.median_volume_usdc),
                    fallback.sample_count,
                    "fallback_bucket_ready",
                )
            }
            _ => (None, sample_count, "cold_start"),
        }
    };
    let ratio = baseline
        .filter(|baseline| baseline.is_finite() && *baseline > 0.0)
        .map(|baseline| volume_summary.volume_30s / baseline);
    let regime = no_order_liquidity_regime_for_ratio(ratio);

    json!({
        "volume_10s": volume_summary.volume_10s,
        "volume_30s": volume_summary.volume_30s,
        "volume_60s": volume_summary.volume_60s,
        "trade_count_60s": volume_summary.trade_count_60s,
        "hourly_volume_baseline": no_order_json_f64(baseline),
        "hourly_volume_ratio": no_order_json_f64(ratio),
        "liquidity_regime": regime.unwrap_or("UNKNOWN"),
        "liquidity_note": no_order_liquidity_note_for_ratio(ratio),
        "liquidity_status": status,
        "volume_baseline_sample_count": sample_count,
        "volume_bucket": bucket_label,
    })
}

fn build_no_order_base_diagnosis_payload(
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    window_end_at: DateTime<Utc>,
    summary: &TradeBuilderNoFillReasonSummary,
    action_steps: &[TradeFlowRunStep],
    snapshot: Option<&TradeBuilderMarketSecondSnapshot>,
    final_quote_payload: Option<&Value>,
    liquidity_payload: Option<Value>,
) -> Value {
    let latest_output = action_steps
        .iter()
        .rev()
        .find_map(|step| step.output_json.as_ref());
    let target_candidate = latest_output
        .and_then(|output| no_fill_pair_lock_candidate_for_target(output, token_id, outcome_label));
    let execution_floor = no_order_summary_floor(summary)
        .or_else(|| target_candidate.and_then(no_order_candidate_floor));
    let best_ask_at_window_end = no_order_summary_best_ask(summary)
        .or_else(|| target_candidate.and_then(no_order_candidate_best_ask));
    let floor_distance = match (best_ask_at_window_end, execution_floor) {
        (Some(best_ask), Some(floor)) => Some(best_ask - floor),
        _ => None,
    };
    let floor_distance_pct = match (floor_distance, execution_floor) {
        (Some(distance), Some(floor)) if floor > 0.0 => Some(distance / floor),
        _ => None,
    };
    let (floor_wait_ms, floor_recovered_once, min_best_ask, max_best_ask) =
        no_order_floor_history(token_id, outcome_label, action_steps);
    let book_payload =
        no_order_book_payload(latest_output, outcome_label, snapshot, final_quote_payload);
    let liquidity_payload = liquidity_payload.unwrap_or_else(|| {
        json!({
            "liquidity_status": "not_measured",
            "liquidity_regime": "UNKNOWN",
        })
    });
    let is_action_failed = summary.scope == "action_failed";
    let action_error = summary.payload.get("action_error").and_then(Value::as_str);
    let action_node_key = summary.payload.get("action_node_key").and_then(Value::as_str);
    let waiting_condition = if is_action_failed {
        "action.place_order validation"
    } else if execution_floor.is_some() {
        "best_ask >= floor"
    } else {
        "guard_condition_passed"
    };
    let condition_current = if is_action_failed {
        Some(
            action_error
                .unwrap_or("action.place_order failed before builder order creation")
                .to_string(),
        )
    } else {
        match (best_ask_at_window_end, execution_floor) {
            (Some(best_ask), Some(floor)) => Some(format!("{best_ask:.4} < {floor:.4}")),
            _ => None,
        }
    };
    let protection_result = if is_action_failed {
        Some("builder order was not created because action.place_order failed")
    } else if summary.scope == "execution_floor"
        && floor_distance.map(|value| value < 0.0).unwrap_or(false)
    {
        Some("entry avoided because selected side collapsed below floor")
    } else {
        None
    };
    let why_no_order_summary = if is_action_failed {
        format!(
            "action.place_order failed before builder order creation: {}",
            action_error.unwrap_or("unknown action failure")
        )
    } else {
        match (best_ask_at_window_end, execution_floor) {
            (Some(best_ask), Some(floor)) => format!(
                "{outcome_label} best ask {best_ask:.4} was below required floor {floor:.4} until window ended."
            ),
            _ => format!(
                "{outcome_label} guard condition did not pass before the market window ended."
            ),
        }
    };
    let human_readable_reason = if is_action_failed {
        format!(
            "Trigger passed, but action.place_order failed before creating a builder order{}{}.",
            action_node_key.map(|value| format!(" on {value}")).unwrap_or_default(),
            action_error.map(|value| format!(": {value}")).unwrap_or_default()
        )
    } else if summary.scope == "execution_floor" {
        "Selected side did not recover to the execution floor before window end.".to_string()
    } else {
        format!(
            "{} guard stayed {} before window end.",
            no_fill_scope_label(&summary.scope),
            summary.decision.as_deref().unwrap_or("blocked")
        )
    };
    let final_action_status = if is_action_failed {
        "ACTION_FAILED"
    } else {
        "NO_ORDER"
    };
    let order_status_reason = if is_action_failed {
        "action_failed_before_builder_order"
    } else {
        "guard_waiting_until_window_end"
    };
    let condition_result = if is_action_failed {
        "action_failed_before_order_creation"
    } else {
        "condition_not_met_until_window_end"
    };

    let mut payload = json!({
        "order_created": false,
        "order_submitted": false,
        "order_filled": false,
        "order_not_created": true,
        "final_action_status": final_action_status,
        "order_status_reason": order_status_reason,
        "market_slug": market_slug,
        "token_id": token_id,
        "outcome_label": outcome_label,
        "selected_side": outcome_label,
        "window_end_at": window_end_at.to_rfc3339(),
        "last_guard_name": no_fill_scope_label(&summary.scope),
        "last_guard_scope": summary.scope,
        "last_guard_code": summary.reason_code,
        "last_guard_state": summary.decision,
        "last_blocker": format!("{}:{}", summary.scope, summary.reason_code),
        "source_event": summary.source_event,
        "execution_floor": no_order_json_f64(execution_floor),
        "best_ask_at_window_end": no_order_json_f64(best_ask_at_window_end),
        "best_ask_at_block": no_order_json_f64(best_ask_at_window_end),
        "floor_distance": no_order_json_f64(floor_distance),
        "floor_distance_pct": no_order_json_f64(floor_distance_pct),
        "floor_wait_ms": floor_wait_ms,
        "floor_recovered_once": floor_recovered_once,
        "min_best_ask_during_wait": no_order_json_f64(min_best_ask),
        "max_best_ask_during_wait": no_order_json_f64(max_best_ask),
        "waiting_condition": waiting_condition,
        "condition_current": condition_current,
        "condition_result": condition_result,
        "protection_result": protection_result,
        "action_node_key": action_node_key,
        "action_error": action_error,
        "action_step_id": summary.payload.get("action_step_id").cloned(),
        "why_no_order_summary": why_no_order_summary,
        "human_readable_reason": human_readable_reason,
    });
    if let Some(payload_obj) = payload.as_object_mut() {
        if let Some(book_obj) = book_payload.as_object() {
            for (key, value) in book_obj {
                payload_obj.insert(key.clone(), value.clone());
            }
        }
        if let Some(liquidity_obj) = liquidity_payload.as_object() {
            for (key, value) in liquidity_obj {
                payload_obj.insert(key.clone(), value.clone());
            }
        }
    }
    payload
}

async fn no_order_fetch_book_quote(
    client: &dyn OrderExecutor,
    token_id: Option<&str>,
) -> (Option<f64>, Option<f64>, Option<f64>, bool) {
    let Some(token_id) = token_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return (None, None, None, false);
    };
    match client.order_book(token_id).await {
        Ok(Some(book)) => (
            order_book_best_bid(&book),
            order_book_best_ask(&book),
            order_book_best_ask_depth_usdc(&book),
            false,
        ),
        Ok(None) => (None, None, None, false),
        Err(_) => (None, None, None, true),
    }
}

async fn build_no_order_final_quote_payload(
    client: Option<&dyn OrderExecutor>,
    run_context: &Value,
    token_id: &str,
    outcome_label: &str,
) -> Option<Value> {
    let client = client?;
    let selected_is_down = no_fill_outcome_matches(outcome_label, "Down");
    let selected_token = token_id.trim();
    let up_token = if selected_is_down {
        resolve_token_id_for_outcome_label("Up", run_context)
    } else {
        Some(selected_token.to_string())
    };
    let down_token = if selected_is_down {
        Some(selected_token.to_string())
    } else {
        resolve_token_id_for_outcome_label("Down", run_context)
    };
    let (up_bid, up_ask, up_depth, up_failed) =
        no_order_fetch_book_quote(client, up_token.as_deref()).await;
    let (down_bid, down_ask, down_depth, down_failed) =
        no_order_fetch_book_quote(client, down_token.as_deref()).await;
    let has_quote = [up_bid, up_ask, down_bid, down_ask]
        .into_iter()
        .any(|value| value.is_some());
    let final_fetch_status = if has_quote {
        "ok"
    } else if up_failed || down_failed {
        "failed"
    } else {
        "unavailable"
    };
    Some(json!({
        "quote_snapshot_source": if has_quote { "final_fetch" } else { "unavailable" },
        "final_fetch_status": final_fetch_status,
        "up_bid": no_order_json_f64(up_bid),
        "up_ask": no_order_json_f64(up_ask),
        "up_ask_depth_usdc": no_order_json_f64(up_depth),
        "down_bid": no_order_json_f64(down_bid),
        "down_ask": no_order_json_f64(down_ask),
        "down_ask_depth_usdc": no_order_json_f64(down_depth),
    }))
}

async fn build_missed_market_no_order_diagnosis_payload(
    repo: &PostgresRepository,
    client: Option<&dyn OrderExecutor>,
    run_context: &Value,
    node_key: &str,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    window_end_at: DateTime<Utc>,
    summary: &TradeBuilderNoFillReasonSummary,
    events: &[TradeFlowEventRecord],
    action_steps: &[TradeFlowRunStep],
) -> Value {
    let snapshots = match repo
        .list_trade_builder_market_second_snapshots(&[market_slug.to_string()])
        .await
    {
        Ok(snapshots) => snapshots,
        Err(err) => {
            warn!(
                market_slug = %market_slug,
                error = %err,
                "NO_ORDER_SECOND_SNAPSHOT_LOOKUP_FAILED"
            );
            Vec::new()
        }
    };
    let snapshot = no_order_latest_snapshot(&snapshots, window_end_at);
    let final_quote_payload =
        build_no_order_final_quote_payload(client, run_context, token_id, outcome_label).await;
    let liquidity_payload = build_no_order_liquidity_payload(repo, market_slug, window_end_at).await;
    let mut payload = build_no_order_base_diagnosis_payload(
        market_slug,
        token_id,
        outcome_label,
        window_end_at,
        summary,
        action_steps,
        snapshot,
        final_quote_payload.as_ref(),
        Some(liquidity_payload),
    );
    let timeline = build_no_order_market_timeline_payload(NoOrderMarketTimelineContext {
        node_key,
        market_slug,
        token_id,
        outcome_label,
        window_end_at,
        summary,
        events,
        action_steps,
    });
    if let Some(payload_obj) = payload.as_object_mut() {
        if let Some(timeline_obj) = timeline.as_object() {
            for (key, value) in timeline_obj {
                payload_obj.insert(key.clone(), value.clone());
            }
        }
    }
    payload
}

fn no_order_diag_f64(payload: &Value, key: &str) -> Option<f64> {
    payload.get(key).and_then(value_as_f64)
}

fn no_order_diag_str<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload.get(key).and_then(Value::as_str)
}

fn no_order_diag_bool(payload: &Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(Value::as_bool)
}

fn no_order_diag_line_f64(payload: &Value, key: &str) -> String {
    no_fill_format_price(no_order_diag_f64(payload, key))
}

fn no_order_condition_line(payload: &Value) -> String {
    no_order_diag_str(payload, "condition_current")
        .map(str::to_string)
        .unwrap_or_else(|| "N/A".to_string())
}

fn build_missed_market_no_order_diagnosis_message_block(diagnosis: &Value) -> String {
    let mut lines = Vec::new();
    let last_guard_scope = no_order_diag_str(diagnosis, "last_guard_scope");
    let is_execution_floor = last_guard_scope == Some("execution_floor");
    let is_trigger_condition = last_guard_scope == Some("trigger_condition");
    let is_action_failed = last_guard_scope == Some("action_failed");
    lines.push("Nihai Karar".to_string());
    lines.push(if is_action_failed {
        "Karar: NO ORDER - action failed".to_string()
    } else if is_trigger_condition {
        "Karar: NO ORDER - trigger condition not met".to_string()
    } else if is_execution_floor {
        "Karar: NO ORDER - protected block".to_string()
    } else {
        "Karar: NO ORDER - guard condition not met".to_string()
    });
    lines.push("Order Status: NOT CREATED".to_string());
    lines.push(format!(
        "Order gonderildi mi?: {}",
        if no_order_diag_bool(diagnosis, "order_submitted") == Some(true) {
            "Evet"
        } else {
            "Hayir"
        }
    ));
    if is_action_failed {
        lines.push(
            "Ana sebep: action.place_order fail oldu; builder order olusmadi.".to_string(),
        );
    } else {
        lines.push(format!(
            "Ana sebep: {}",
            no_order_diag_str(diagnosis, "why_no_order_summary")
                .unwrap_or("Guard condition market/window sonuna kadar gecmedi.")
        ));
    }
    lines.push(String::new());
    lines.push("Son Engel".to_string());
    lines.push(format!(
        "Son Engel: {}",
        no_order_diag_str(diagnosis, "last_guard_name").unwrap_or("N/A")
    ));
    lines.push(format!(
        "Engel Kodu: {}",
        no_order_diag_str(diagnosis, "last_guard_code").unwrap_or("N/A")
    ));
    lines.push(format!(
        "Guard state: {}",
        no_order_diag_str(diagnosis, "last_guard_state").unwrap_or("N/A")
    ));
    if is_action_failed {
        if let Some(node_key) = no_order_diag_str(diagnosis, "action_node_key") {
            lines.push(format!("Action Node: {node_key}"));
        }
        if let Some(error) = no_order_diag_str(diagnosis, "action_error") {
            lines.push(format!("Hata: {error}"));
        }
    }
    if is_execution_floor {
        lines.push(format!(
            "Floor wait: {} ms",
            diagnosis
                .get("floor_wait_ms")
                .and_then(Value::as_i64)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "N/A".to_string())
        ));
        lines.push(format!(
            "Floor recovered once?: {}",
            no_order_diag_bool(diagnosis, "floor_recovered_once")
                .map(|value| if value { "Evet" } else { "Hayir" })
                .unwrap_or("N/A")
        ));
    }
    append_no_order_market_timeline_lines(&mut lines, diagnosis);
    lines.push(String::new());
    lines.push("Piyasa Durumu".to_string());
    lines.push(format!(
        "Selected side: {}",
        no_order_diag_str(diagnosis, "selected_side").unwrap_or("N/A")
    ));
    if is_execution_floor {
        lines.push(format!(
            "Best Ask: {}",
            no_order_diag_line_f64(diagnosis, "best_ask_at_window_end")
        ));
        lines.push(format!(
            "Required Floor: {}",
            no_order_diag_line_f64(diagnosis, "execution_floor")
        ));
        lines.push(format!(
            "Floor farki: {}",
            no_fill_format_price(no_order_diag_f64(diagnosis, "floor_distance"))
        ));
        lines.push(format!(
            "Floor farki %: {}",
            no_order_format_pct(no_order_diag_f64(diagnosis, "floor_distance_pct"))
        ));
    } else {
        lines.push("Note: No execution floor was evaluated for this event.".to_string());
    }
    lines.push(format!(
        "Quote snapshot source: {}",
        no_order_diag_str(diagnosis, "quote_snapshot_source").unwrap_or("unavailable")
    ));
    lines.push(format!(
        "Book data status: {}",
        no_order_diag_str(diagnosis, "book_data_status").unwrap_or("unavailable")
    ));
    if let Some(reason) = no_order_diag_str(diagnosis, "quote_missing_reason") {
        lines.push(format!("Quote missing reason: {reason}"));
    }
    lines.push(format!(
        "Book side: {}",
        no_order_diag_str(diagnosis, "book_side").unwrap_or("N/A")
    ));
    lines.push(format!(
        "Selected bid / ask / mid: {} / {} / {}",
        no_order_diag_line_f64(diagnosis, "selected_bid"),
        no_order_diag_line_f64(diagnosis, "selected_ask"),
        no_order_diag_line_f64(diagnosis, "selected_mid")
    ));
    lines.push(format!(
        "Up bid/ask: {} / {}",
        no_order_diag_line_f64(diagnosis, "up_bid"),
        no_order_diag_line_f64(diagnosis, "up_ask")
    ));
    lines.push(format!(
        "Down bid/ask: {} / {}",
        no_order_diag_line_f64(diagnosis, "down_bid"),
        no_order_diag_line_f64(diagnosis, "down_ask")
    ));
    lines.push(format!(
        "Up mid / Down mid: {} / {}",
        no_order_diag_line_f64(diagnosis, "up_mid"),
        no_order_diag_line_f64(diagnosis, "down_mid")
    ));
    lines.push(format!(
        "Spread: {}",
        no_order_diag_line_f64(diagnosis, "selected_side_spread")
    ));
    lines.push(format!(
        "Selected side depth: {}",
        no_order_diag_line_f64(diagnosis, "selected_side_depth")
    ));
    lines.push(String::new());
    lines.push("Likidite / Hacim Durumu".to_string());
    lines.push(format!(
        "Liquidity Regime: {}",
        no_order_diag_str(diagnosis, "liquidity_regime").unwrap_or("UNKNOWN")
    ));
    lines.push(format!(
        "Hourly volume ratio: {}",
        no_order_format_ratio(no_order_diag_f64(diagnosis, "hourly_volume_ratio"))
    ));
    lines.push(format!(
        "Volume 30s: {}",
        no_fill_format_precise(no_order_diag_f64(diagnosis, "volume_30s"))
    ));
    lines.push(format!(
        "Trade count 60s: {}",
        diagnosis
            .get("trade_count_60s")
            .and_then(Value::as_i64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "N/A".to_string())
    ));
    if let Some(note) = no_order_diag_str(diagnosis, "liquidity_note") {
        lines.push(format!("Likidite Notu: {note}"));
    }
    lines.push(String::new());
    lines.push("Botun Bekledigi Sart".to_string());
    lines.push(format!(
        "Expected: {} = true",
        no_order_diag_str(diagnosis, "waiting_condition").unwrap_or("guard_condition_passed")
    ));
    lines.push(format!("Current: {}", no_order_condition_line(diagnosis)));
    lines.push(format!(
        "Result: {}",
        no_order_diag_str(diagnosis, "condition_result").unwrap_or("N/A")
    ));
    lines.push(String::new());
    lines.push("Yorum".to_string());
    lines.push(
        no_order_diag_str(diagnosis, "human_readable_reason")
            .unwrap_or("Guard condition window sonuna kadar gecmedi.")
            .to_string(),
    );
    if let Some(result) = no_order_diag_str(diagnosis, "protection_result") {
        lines.push(format!("Protection Result: {result}."));
    }
    lines.push("Bu bir fill kacirma degil; bot emir olusturmadi.".to_string());

    lines.join("\n")
}
