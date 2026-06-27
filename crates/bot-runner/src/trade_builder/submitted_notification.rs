#[derive(Debug, Clone, PartialEq)]
struct TradeBuilderFillExecutionAnalysis {
    actual_fill_price: f64,
    actual_filled_qty: f64,
    actual_notional: f64,
    actual_fill_source: &'static str,
}

fn trade_builder_notify_f64(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(trade_builder_notify_value_as_f64)
}

fn trade_builder_notify_value_as_f64(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
    .filter(|value| value.is_finite())
}

fn trade_builder_notify_text<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

fn trade_builder_notify_bool(value: &serde_json::Value, key: &str) -> Option<bool> {
    value.get(key).and_then(serde_json::Value::as_bool)
}

fn trade_builder_notify_string_list(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "N/A".to_string())
}

fn trade_builder_notify_has_non_null(value: &serde_json::Value, key: &str) -> bool {
    matches!(value.get(key), Some(value) if !value.is_null())
}

fn trade_builder_notify_guard(payload: Option<&serde_json::Value>) -> Option<&serde_json::Value> {
    payload?.get("price_to_beat_guard")
}

fn trade_builder_notify_iv(payload: Option<&serde_json::Value>) -> Option<&serde_json::Value> {
    trade_builder_notify_guard(payload)?.get("iv_mismatch_edge")
}

fn trade_builder_latest_event_payload(
    events: Vec<TradeBuilderOrderEventRecord>,
    event_types: &[&str],
) -> Option<serde_json::Value> {
    events
        .into_iter()
        .rev()
        .find(|event| event_types.contains(&event.event_type.as_str()))
        .map(|event| event.payload_json)
}

async fn load_trade_builder_latest_flow_payload(
    repo: &PostgresRepository,
    builder_order_id: i64,
) -> Result<Option<serde_json::Value>> {
    let events = repo
        .list_trade_builder_order_events_for_orders(&[builder_order_id])
        .await?;
    Ok(trade_builder_latest_event_payload(
        events,
        &["flow_rearmed", "flow_created"],
    ))
}

async fn load_trade_builder_latest_submitted_payload(
    repo: &PostgresRepository,
    builder_order_id: i64,
) -> Result<Option<serde_json::Value>> {
    let events = repo
        .list_trade_builder_order_events_for_orders(&[builder_order_id])
        .await?;
    Ok(trade_builder_latest_event_payload(events, &["submitted"]))
}

fn trade_builder_model_book_zone(iv: Option<&serde_json::Value>) -> Option<&'static str> {
    let iv = iv?;
    let gap = trade_builder_notify_f64(iv, "model_book_gap")?;
    let hard = trade_builder_notify_f64(iv, "too_good_threshold").unwrap_or(0.45);
    let warn = trade_builder_notify_f64(iv, "model_book_gap_warn_threshold").unwrap_or(0.30);
    Some(if gap >= hard {
        "HARD_BLOCK"
    } else if gap >= warn {
        "WARNING"
    } else {
        "NORMAL"
    })
}

fn trade_builder_notify_fmt_price(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.4}"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn trade_builder_notify_fmt_number(value: Option<f64>, decimals: usize) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.decimals$}"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn trade_builder_notify_fmt_qty(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn trade_builder_notify_fmt_usdc(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.2} USDC"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn trade_builder_notify_fmt_signed(value: Option<f64>, decimals: usize) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:+.decimals$}"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn trade_builder_notify_fmt_bool(value: Option<bool>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

fn trade_builder_notify_fmt_cent(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.2}c"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn trade_builder_notify_fmt_q_cent(value: Option<f64>) -> String {
    trade_builder_notify_fmt_cent(value.map(|value| value * 100.0))
}

fn trade_builder_notify_fmt_signed_usd(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:+.1} USD"))
        .unwrap_or_else(|| "not_available".to_string())
}

fn trade_builder_notify_fmt_seconds(value: Option<i64>) -> String {
    value
        .map(|value| format!("{value}s"))
        .unwrap_or_else(|| "not_available".to_string())
}

fn trade_builder_notify_upper_text(value: Option<&str>) -> String {
    value
        .map(|value| value.to_ascii_uppercase())
        .unwrap_or_else(|| "N/A".to_string())
}

fn trade_builder_live_gap_metadata_mode(metadata: Option<&serde_json::Value>) -> bool {
    metadata
        .and_then(|metadata| metadata.get("mode"))
        .and_then(serde_json::Value::as_str)
        == Some(ACTION_PLACE_ORDER_MODE_LIVE_GAP_COLLECTOR_V1)
}

fn trade_builder_is_primary_buy_entry(order: &TradeBuilderOrder) -> bool {
    order.parent_order_id.is_none() && order.side == "buy"
}

fn trade_builder_should_use_live_gap_submitted_template(
    order: &TradeBuilderOrder,
    live_gap_metadata: Option<&serde_json::Value>,
) -> bool {
    trade_builder_is_primary_buy_entry(order)
        && trade_builder_live_gap_metadata_mode(live_gap_metadata)
}

fn trade_builder_should_use_exit_child_submitted_template(order: &TradeBuilderOrder) -> bool {
    order.parent_order_id.is_some() && order.side == "sell"
}

fn trade_builder_submitted_target_qty(
    order: &TradeBuilderOrder,
    submitted_payload: &serde_json::Value,
) -> Option<f64> {
    trade_builder_notify_f64(submitted_payload, "submitted_target_qty")
        .or_else(|| trade_builder_notify_f64(submitted_payload, "target_qty"))
        .or_else(|| trade_builder_notify_f64(submitted_payload, "size"))
        .or(order.target_qty)
}

fn trade_builder_submitted_estimated_avg_fill(
    submitted_payload: &serde_json::Value,
) -> Option<f64> {
    trade_builder_notify_f64(submitted_payload, "submitted_estimated_avg_fill")
        .or_else(|| trade_builder_notify_f64(submitted_payload, "estimated_avg_fill"))
        .or_else(|| trade_builder_notify_f64(submitted_payload, "execution_price"))
}

fn trade_builder_submitted_best_ask(submitted_payload: &serde_json::Value) -> Option<f64> {
    trade_builder_notify_f64(submitted_payload, "submitted_best_ask")
        .or_else(|| trade_builder_notify_f64(submitted_payload, "best_ask"))
}

fn trade_builder_submitted_cost_per_share(submitted_payload: &serde_json::Value) -> Option<f64> {
    trade_builder_notify_f64(submitted_payload, "submitted_effective_cost_per_share")
        .or_else(|| trade_builder_notify_f64(submitted_payload, "cost_per_share"))
        .or_else(|| trade_builder_notify_f64(submitted_payload, "execution_price"))
}

fn trade_builder_submitted_should_show_cex_open_gap(iv: &serde_json::Value) -> bool {
    let enabled = trade_builder_notify_bool(iv, "cex_open_gap_enabled").unwrap_or(false);
    let consensus = trade_builder_notify_text(iv, "cex_open_gap_consensus").unwrap_or_default();
    let cap_applied = trade_builder_notify_bool(iv, "cex_consensus_q_cap_applied").unwrap_or(false);

    enabled
        || cap_applied
        || !matches!(consensus, "" | "unavailable" | "disabled")
        || trade_builder_notify_has_non_null(iv, "cex_open_gap_block_reason")
        || trade_builder_notify_has_non_null(iv, "chainlink_cex_book_mismatch_reason")
}

fn trade_builder_submitted_cex_open_gap_block(iv: Option<&serde_json::Value>) -> String {
    let Some(iv) = iv.filter(|iv| trade_builder_submitted_should_show_cex_open_gap(iv)) else {
        return String::new();
    };
    let reason = trade_builder_notify_text(iv, "cex_open_gap_block_reason")
        .or_else(|| trade_builder_notify_text(iv, "chainlink_cex_book_mismatch_reason"))
        .unwrap_or("N/A");
    let anchor_label = trade_builder_notify_text(iv, "cex_open_gap_anchor_venue").unwrap_or("anchor");

    format!(
        "\n\nCEX Open Gap:\nConsensus: {} | clean={} | cap={}\nBinance: open={} current={} gap={} z={} state={}\nAnchor({}): open={} current={} gap={} z={} state={}\nChainlink/CEX: chainlink={} conservative={} effective={} diff={} z={} bps={}\nq consensus: before={} after={}\nReason: {}",
        trade_builder_notify_text(iv, "cex_open_gap_consensus").unwrap_or("N/A"),
        trade_builder_notify_fmt_bool(trade_builder_notify_bool(iv, "cex_open_gap_clean_lane")),
        trade_builder_notify_fmt_bool(trade_builder_notify_bool(iv, "cex_consensus_q_cap_applied")),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(iv, "binance_5m_open")),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(iv, "binance_current_mid")),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(iv, "binance_signed_gap")),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(iv, "binance_gap_z")),
        trade_builder_notify_text(iv, "binance_state").unwrap_or("N/A"),
        anchor_label,
        trade_builder_notify_fmt_price(trade_builder_notify_f64(iv, "anchor_5m_open")),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(iv, "anchor_current_mid")),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(iv, "anchor_signed_gap")),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(iv, "anchor_gap_z")),
        trade_builder_notify_text(iv, "anchor_state").unwrap_or("N/A"),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(iv, "chainlink_signed_gap")),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(iv, "conservative_cex_gap")),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(iv, "effective_consensus_gap_usd")),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(iv, "chainlink_cex_diff_usd")),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(iv, "chainlink_cex_diff_z")),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(iv, "chainlink_cex_diff_bps")),
        trade_builder_notify_fmt_q_cent(trade_builder_notify_f64(
            iv,
            "q_final_before_cex_consensus"
        )),
        trade_builder_notify_fmt_q_cent(trade_builder_notify_f64(
            iv,
            "q_final_after_cex_consensus"
        )),
        reason,
    )
}

fn trade_builder_submitted_execution_limit_block(iv: Option<&serde_json::Value>) -> String {
    let Some(iv) = iv.filter(|iv| {
        trade_builder_notify_has_non_null(iv, "expected_vwap_cent")
            || trade_builder_notify_has_non_null(iv, "submit_limit_price_cent")
            || trade_builder_notify_has_non_null(iv, "execution_limit_by_vwap_action")
    }) else {
        return String::new();
    };

    format!(
        "\n\nExecution Limit:\nExpected VWAP: {} | Submit Limit: {} | Limit Action: {}",
        trade_builder_notify_fmt_cent(trade_builder_notify_f64(iv, "expected_vwap_cent")),
        trade_builder_notify_fmt_cent(trade_builder_notify_f64(iv, "submit_limit_price_cent")),
        trade_builder_notify_text(iv, "execution_limit_by_vwap_action").unwrap_or("N/A"),
    )
}

fn trade_builder_iv_expected_fee_rate(iv: Option<&serde_json::Value>) -> f64 {
    iv.and_then(|payload| trade_builder_notify_f64(payload, "fee_rate"))
        .unwrap_or(0.072)
        .max(0.0)
}

fn trade_builder_iv_expected_buffer(iv: Option<&serde_json::Value>) -> f64 {
    iv.and_then(|payload| trade_builder_notify_f64(payload, "buffer"))
        .unwrap_or(0.0)
        .max(0.0)
}

fn trade_builder_effective_actual_cost_parts(
    actual_fill_price: f64,
    iv: Option<&serde_json::Value>,
) -> (f64, f64, f64) {
    let fee =
        trade_builder_iv_expected_fee_rate(iv) * actual_fill_price * (1.0 - actual_fill_price);
    let buffer = trade_builder_iv_expected_buffer(iv);
    (actual_fill_price + fee + buffer, fee, buffer)
}

fn trade_builder_append_optional_number(
    payload: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: Option<f64>,
) {
    payload.insert(key.to_string(), serde_json::json!(value));
}

fn trade_builder_append_submitted_telemetry(
    payload: &mut serde_json::Map<String, serde_json::Value>,
    order: &TradeBuilderOrder,
    flow_payload: Option<&serde_json::Value>,
    submitted_qty: f64,
    desired_price: f64,
) {
    let iv = trade_builder_notify_iv(flow_payload);
    let best_ask = iv
        .and_then(|iv| trade_builder_notify_f64(iv, "depth_best_ask"))
        .or_else(|| iv.and_then(|iv| trade_builder_notify_f64(iv, "ask")))
        .or_else(|| {
            payload
                .get("best_ask")
                .and_then(trade_builder_notify_value_as_f64)
        });
    let estimated_avg_fill = iv
        .and_then(|iv| trade_builder_notify_f64(iv, "estimated_avg_fill"))
        .or(Some(desired_price));
    let cost_per_share = iv
        .and_then(|iv| trade_builder_notify_f64(iv, "cost"))
        .or(estimated_avg_fill);
    let target_qty = order.target_qty.unwrap_or(submitted_qty);
    let estimated_notional = estimated_avg_fill.map(|price| price * target_qty);
    let estimated_total_cost = cost_per_share.map(|cost| cost * target_qty);
    let q_final = iv
        .and_then(|iv| trade_builder_notify_f64(iv, "q_final"))
        .or_else(|| iv.and_then(|iv| trade_builder_notify_f64(iv, "q")));
    let if_win = estimated_total_cost.map(|cost| target_qty - cost);
    let if_loss = estimated_total_cost.map(|cost| -cost);
    let ev = q_final
        .zip(if_win)
        .zip(if_loss)
        .map(|((q, win), loss)| q * win + (1.0 - q) * loss);
    let ev_roi = ev
        .zip(estimated_total_cost)
        .and_then(|(ev, cost)| (cost > 0.0).then_some(ev / cost));
    let model_book_zone = trade_builder_model_book_zone(iv);

    payload.insert(
        "order_status".to_string(),
        serde_json::json!(
            payload
                .get("normalized_status")
                .and_then(serde_json::Value::as_str)
        ),
    );
    payload.insert(
        "submitted_target_qty".to_string(),
        serde_json::json!(Some(target_qty)),
    );
    trade_builder_append_optional_number(payload, "submitted_best_ask", best_ask);
    trade_builder_append_optional_number(
        payload,
        "submitted_estimated_avg_fill",
        estimated_avg_fill,
    );
    trade_builder_append_optional_number(
        payload,
        "submitted_vwap_slippage",
        iv.and_then(|iv| trade_builder_notify_f64(iv, "vwap_slippage")),
    );
    trade_builder_append_optional_number(
        payload,
        "submitted_estimated_notional",
        estimated_notional,
    );
    trade_builder_append_optional_number(
        payload,
        "submitted_estimated_total_cost",
        estimated_total_cost,
    );
    trade_builder_append_optional_number(
        payload,
        "submitted_effective_cost_per_share",
        cost_per_share,
    );
    trade_builder_append_optional_number(
        payload,
        "submitted_available_qty_at_best_ask",
        iv.and_then(|iv| trade_builder_notify_f64(iv, "available_qty_at_best_ask")),
    );
    trade_builder_append_optional_number(
        payload,
        "submitted_depth_levels_used",
        iv.and_then(|iv| trade_builder_notify_f64(iv, "depth_levels_used")),
    );
    payload.insert(
        "submitted_depth_guard_result".to_string(),
        serde_json::json!(iv.and_then(|iv| trade_builder_notify_text(iv, "depth_guard_result"))),
    );
    payload.insert(
        "submitted_depth_guard_reason".to_string(),
        serde_json::json!(iv.and_then(|iv| trade_builder_notify_text(iv, "depth_guard_reason"))),
    );
    trade_builder_append_optional_number(payload, "submitted_q_final", q_final);
    trade_builder_append_optional_number(
        payload,
        "submitted_selected_mid",
        iv.and_then(|iv| trade_builder_notify_f64(iv, "selected_mid")),
    );
    trade_builder_append_optional_number(
        payload,
        "submitted_model_book_gap",
        iv.and_then(|iv| trade_builder_notify_f64(iv, "model_book_gap")),
    );
    payload.insert(
        "submitted_model_book_zone".to_string(),
        serde_json::json!(model_book_zone),
    );
    trade_builder_append_optional_number(
        payload,
        "submitted_model_book_penalty",
        iv.and_then(|iv| trade_builder_notify_f64(iv, "protection_threshold_penalty")),
    );
    trade_builder_append_optional_number(
        payload,
        "submitted_dynamic_threshold_before_credit",
        iv.and_then(|iv| trade_builder_notify_f64(iv, "dynamic_threshold_before_participation")),
    );
    trade_builder_append_optional_number(
        payload,
        "submitted_participation_credit",
        iv.and_then(|iv| trade_builder_notify_f64(iv, "participation_credit")),
    );
    trade_builder_append_optional_number(
        payload,
        "submitted_dynamic_threshold_after_credit",
        iv.and_then(|iv| trade_builder_notify_f64(iv, "dynamic_threshold")),
    );
    trade_builder_append_optional_number(
        payload,
        "submitted_adjusted_edge",
        iv.and_then(|iv| trade_builder_notify_f64(iv, "edge_adj")),
    );
    trade_builder_append_optional_number(
        payload,
        "submitted_adjusted_margin",
        iv.and_then(|iv| trade_builder_notify_f64(iv, "adjusted_margin")),
    );
    trade_builder_append_optional_number(payload, "submitted_if_win_pnl_est", if_win);
    trade_builder_append_optional_number(payload, "submitted_if_loss_pnl_est", if_loss);
    trade_builder_append_optional_number(payload, "submitted_ev_est", ev);
    trade_builder_append_optional_number(payload, "submitted_ev_roi_est", ev_roi);
    payload.insert(
        "submitted_late_high_price_warning".to_string(),
        serde_json::json!(
            iv.and_then(|iv| iv.get("protection_reasons"))
                .and_then(serde_json::Value::as_array)
                .is_some_and(|reasons| reasons
                    .iter()
                    .any(|reason| { reason.as_str() == Some("warn_late_high_price_unconfirmed") }))
        ),
    );
    payload.insert(
        "submitted_binance_same_direction".to_string(),
        serde_json::json!(
            iv.and_then(|iv| trade_builder_notify_bool(iv, "binance_same_direction"))
        ),
    );
    trade_builder_append_optional_number(
        payload,
        "submitted_spread",
        iv.and_then(|iv| trade_builder_notify_f64(iv, "spread")),
    );
    trade_builder_append_optional_number(
        payload,
        "submitted_stale_ms",
        iv.and_then(|iv| trade_builder_notify_f64(iv, "chainlink_staleness_ms")),
    );
}

fn trade_builder_submitted_live_gap_no_reversal_block(
    live_gap_metadata: &serde_json::Value,
) -> String {
    let Some(guard) = live_gap_metadata.get("no_reversal_entry_guard") else {
        return "No-Reversal:\nnot_available".to_string();
    };
    let profile = trade_builder_notify_text(guard, "profile_source").unwrap_or("not_available");
    let profile_status =
        trade_builder_notify_text(guard, "profile_lookup_status").unwrap_or(profile);
    let prewarmer_status = trade_builder_notify_text(guard, "prewarmer_status")
        .map(|status| format!("\nPrewarmer Status: {status}"))
        .unwrap_or_default();
    let prewarm_detail = {
        let priority = trade_builder_notify_text(guard, "prewarm_priority");
        let slot_status = trade_builder_notify_text(guard, "prewarm_slot_status");
        let age_ms = guard.get("prewarm_age_ms").and_then(value_as_i64);
        if priority.is_some() || slot_status.is_some() || age_ms.is_some() {
            format!(
                "\nPrewarm Detail: priority={}, slot={}, age={}",
                priority.unwrap_or("not_available"),
                slot_status.unwrap_or("not_available"),
                age_ms
                    .map(|value| format!("{value}ms"))
                    .unwrap_or_else(|| "not_available".to_string())
            )
        } else {
            String::new()
        }
    };
    let profile_lookup_fallback = trade_builder_notify_text(guard, "profile_lookup_fallback_level")
        .or_else(|| trade_builder_notify_text(guard, "fallback_level"))
        .unwrap_or("not_available");
    let fallback = trade_builder_notify_text(guard, "local_path_fallback_source")
        .or_else(|| trade_builder_notify_text(guard, "runtime_fallback_source"))
        .or_else(|| trade_builder_notify_text(guard, "fallback_level"))
        .unwrap_or("not_available");
    let protection = trade_builder_notify_text(guard, "protection").unwrap_or("not_available");
    let reason = trade_builder_notify_text(guard, "reason_code").unwrap_or("not_available");
    let lookup_key = live_gap_guard_profile_lookup_key_line(guard)
        .map(|line| format!("\n{line}"))
        .unwrap_or_default();
    format!(
        "No-Reversal:\nProfile: {profile}\nProfile Status: {profile_status}{prewarmer_status}{prewarm_detail}\nProfile Lookup Fallback: {profile_lookup_fallback}\nFallback: {fallback}\nProtection: {protection}\nFloor: {}\nReason: {reason}{lookup_key}",
        trade_builder_notify_fmt_signed_usd(trade_builder_notify_f64(guard, "ptb_floor_usd")),
    )
}

fn build_trade_builder_live_gap_submitted_notification_message(
    order: &TradeBuilderOrder,
    submitted_payload: &serde_json::Value,
    live_gap_metadata: &serde_json::Value,
) -> String {
    let target_qty = trade_builder_submitted_target_qty(order, submitted_payload);
    let estimated_vwap = trade_builder_submitted_estimated_avg_fill(submitted_payload);
    let best_ask = trade_builder_submitted_best_ask(submitted_payload)
        .or_else(|| trade_builder_notify_f64(live_gap_metadata, "best_ask"));
    let effective_fill = trade_builder_notify_f64(live_gap_metadata, "effective_fill_price")
        .or_else(|| trade_builder_submitted_cost_per_share(submitted_payload))
        .or(estimated_vwap);
    let estimated_notional =
        trade_builder_notify_f64(submitted_payload, "submitted_estimated_notional").or_else(|| {
            estimated_vwap
                .zip(target_qty)
                .map(|(price, qty)| price * qty)
        });
    let remaining_sec = live_gap_metadata
        .get("remaining_sec")
        .and_then(value_as_i64)
        .or_else(|| {
            live_gap_metadata
                .get("candidate_remaining_sec")
                .and_then(value_as_i64)
        });

    format!(
        "Emir Gonderildi - Live Gap Collector\nMarket: {}\nOutcome: {}\nSide: {}\nOrder Status: SUBMITTED\nOrder Type: {}\n\nLive Gap\nCurrent Gap: {}\nRequired Gap: {}\nRegime: {}\nRemaining: {}\n\n{}\n\nPricing\nBest Ask: {}\nEstimated VWAP: {}\nEffective Fill: {}\nSize Mode: {}\nTarget Qty: {}\nEstimated Notional: {}\nCLOB: SUBMITTED",
        order.market_slug,
        order.outcome_label,
        order.side,
        trade_builder_notify_upper_text(trade_builder_notify_text(submitted_payload, "order_type")),
        trade_builder_notify_fmt_signed_usd(trade_builder_notify_f64(
            live_gap_metadata,
            "live_gap_usd"
        )),
        trade_builder_notify_fmt_signed_usd(trade_builder_notify_f64(
            live_gap_metadata,
            "required_gap_usd"
        )),
        trade_builder_notify_text(live_gap_metadata, "regime").unwrap_or("not_available"),
        trade_builder_notify_fmt_seconds(remaining_sec),
        trade_builder_submitted_live_gap_no_reversal_block(live_gap_metadata),
        trade_builder_notify_fmt_price(best_ask),
        trade_builder_notify_fmt_price(estimated_vwap),
        trade_builder_notify_fmt_price(effective_fill),
        trade_builder_notify_text(submitted_payload, "size_basis").unwrap_or("N/A"),
        trade_builder_notify_fmt_qty(target_qty),
        trade_builder_notify_fmt_usdc(estimated_notional),
    )
}

fn trade_builder_exit_child_label(order: &TradeBuilderOrder) -> &'static str {
    match order.trigger_condition.as_deref() {
        Some("cross_above") => "TP Child Exit",
        Some("cross_below") => "SL Child Exit",
        _ => "Exit Child",
    }
}

fn build_trade_builder_exit_child_submitted_notification_message(
    order: &TradeBuilderOrder,
    submitted_payload: &serde_json::Value,
) -> String {
    let target_qty = trade_builder_submitted_target_qty(order, submitted_payload);
    let submit_price = trade_builder_submitted_estimated_avg_fill(submitted_payload)
        .or_else(|| trade_builder_notify_f64(submitted_payload, "execution_price"));
    let estimated_proceeds = submit_price.zip(target_qty).map(|(price, qty)| price * qty);
    format!(
        "Emir Gonderildi - {}\nMarket: {}\nOutcome: {}\nSide: {}\nOrder Status: SUBMITTED\nOrder Type: {}\n\nExit Submit\nTarget Qty: {}\nSubmit Price: {}\nEstimated Proceeds: {}\nRemaining Qty Before Submit: {}\nCLOB: SUBMITTED",
        trade_builder_exit_child_label(order),
        order.market_slug,
        order.outcome_label,
        order.side,
        trade_builder_notify_upper_text(trade_builder_notify_text(submitted_payload, "order_type")),
        trade_builder_notify_fmt_qty(target_qty),
        trade_builder_notify_fmt_price(submit_price),
        trade_builder_notify_fmt_usdc(estimated_proceeds),
        trade_builder_notify_fmt_qty(order.remaining_qty),
    )
}

fn trade_builder_exit_fill_position_summary_block(
    order: &TradeBuilderOrder,
    summary: &TradeBuilderExitFillPositionSummary,
) -> String {
    let sold = match summary.target_qty {
        Some(target_qty) => format!(
            "{} / {}",
            trade_builder_notify_fmt_qty(Some(summary.sold_this_fill_qty)),
            trade_builder_notify_fmt_qty(Some(target_qty))
        ),
        None => trade_builder_notify_fmt_qty(Some(summary.sold_this_fill_qty)),
    };
    let source = if summary.remaining_qty_estimated {
        format!("estimated: {}", summary.remaining_qty_source)
    } else {
        summary.remaining_qty_source.to_string()
    };
    let state = if summary.closed { "closed" } else { "open" };
    format!(
        "\n\nRemaining Position\nSold This Fill: {sold}\nRemaining Qty: {} {} ({source})\nRemaining Mark: {} @ {} ({})\nRemaining Max Loss: {}\nIf {} wins: {}\nState: {state}",
        trade_builder_notify_fmt_qty(summary.remaining_qty),
        order.outcome_label,
        trade_builder_notify_fmt_usdc(summary.remaining_mark_value),
        trade_builder_notify_fmt_price(summary.mark_price),
        summary.mark_price_source,
        trade_builder_notify_fmt_usdc(summary.remaining_max_loss),
        order.outcome_label,
        trade_builder_notify_fmt_usdc(summary.remaining_if_win),
    )
}

#[cfg(test)]
fn build_trade_builder_submitted_notification_message(
    order: &TradeBuilderOrder,
    submitted_payload: &serde_json::Value,
    flow_payload: Option<&serde_json::Value>,
) -> String {
    build_trade_builder_submitted_notification_message_with_live_gap(
        order,
        submitted_payload,
        flow_payload,
        None,
    )
}

fn build_trade_builder_submitted_notification_message_with_live_gap(
    order: &TradeBuilderOrder,
    submitted_payload: &serde_json::Value,
    flow_payload: Option<&serde_json::Value>,
    live_gap_metadata: Option<&serde_json::Value>,
) -> String {
    if trade_builder_should_use_exit_child_submitted_template(order) {
        return build_trade_builder_exit_child_submitted_notification_message(
            order,
            submitted_payload,
        );
    }
    if trade_builder_should_use_live_gap_submitted_template(order, live_gap_metadata) {
        if let Some(metadata) = live_gap_metadata {
            return build_trade_builder_live_gap_submitted_notification_message(
                order,
                submitted_payload,
                metadata,
            );
        }
    }

    let iv = trade_builder_notify_iv(flow_payload);
    let target_qty = trade_builder_submitted_target_qty(order, submitted_payload);
    let expected_vwap = trade_builder_submitted_estimated_avg_fill(submitted_payload);
    let best_ask = trade_builder_submitted_best_ask(submitted_payload);
    let model_ask = iv.and_then(|iv| trade_builder_notify_f64(iv, "ask"));
    let execution_vs_model = expected_vwap
        .zip(model_ask)
        .map(|(vwap, model)| vwap - model);
    let cost_per_share = trade_builder_submitted_cost_per_share(submitted_payload);
    let estimated_notional =
        trade_builder_notify_f64(submitted_payload, "submitted_estimated_notional");
    let q_final = trade_builder_notify_f64(submitted_payload, "submitted_q_final");
    let selected_mid = trade_builder_notify_f64(submitted_payload, "submitted_selected_mid");
    let model_book_gap = trade_builder_notify_f64(submitted_payload, "submitted_model_book_gap");
    let model_book_zone =
        trade_builder_notify_text(submitted_payload, "submitted_model_book_zone").unwrap_or("N/A");
    let selected_rule = iv.and_then(|iv| iv.get("selected_time_rule"));
    let rule_start =
        selected_rule.and_then(|rule| trade_builder_notify_f64(rule, "start_remaining_secs"));
    let rule_end =
        selected_rule.and_then(|rule| trade_builder_notify_f64(rule, "end_remaining_secs"));
    let time_rule = match (rule_start, rule_end) {
        (Some(start), Some(end)) => format!("{start:.0}-{end:.0} sn"),
        _ => "N/A".to_string(),
    };
    let depth_guard_summary =
        match trade_builder_notify_text(submitted_payload, "submitted_depth_guard_result") {
            Some(result) => {
                match trade_builder_notify_text(submitted_payload, "submitted_depth_guard_reason") {
                    Some(reason) if reason != "null" => format!("{result} reason={reason}"),
                    _ => result.to_string(),
                }
            }
            None => "N/A".to_string(),
        };
    let cex_open_gap_block = trade_builder_submitted_cex_open_gap_block(iv);
    let execution_limit_block = trade_builder_submitted_execution_limit_block(iv);
    let ptb_chop_block = trade_builder_submitted_ptb_chop_block(iv);
    let medium_chop_margin_block = trade_builder_submitted_medium_chop_margin_block(iv);
    let high_price_early_block = trade_builder_submitted_high_price_early_block(iv);

    format!(
        "Emir Gonderildi - Guard Gecti, Fill Bekleniyor\nMarket: {}\nOutcome: {}\nSide: {}\nOrder Status: SUBMITTED\nOrder Type: {}\n\nKarar Ozeti\nSelected: {}\nMode: IV Mismatch Edge\nRule: {}\nSeconds Left: {}\nProtection Result: {}\n\nSizing / Cost\nSize Mode: {}\nTarget Qty: {}\nModel Ask: {}\nExecution Best Ask: {}\nExecution VWAP Fill: {}\nExecution vs Model Ask: {}\nVWAP Slippage: {}\nEstimated Notional: {}\nEffective Cost/Share: {}\nDepth Guard: {}\nDepth Levels Used: {}\nAvailable at Best Ask: {}\n\nModel / Book\nq_final: {}\nSelected Mid: {}\nModel-Book Gap: {}\nModel-Book Zone: {}\nModel-Book Penalty: {}\n\nEdge\nAdjusted Edge: {}\nThreshold Before Credit: {}\nParticipation Credit: {}\nFinal Threshold: {}\nMargin: {}\n\nEstimated Scenario\nIf {} wins: {}\nIf {} loses: {}\nEV: {}\nEV ROI: {}\n\nRisk Flags\nDepth: {}\nLate High Price: {}\nBinance Same Direction: {}\nSpread: {}\nStale: {}ms{}{}{}{}{}",
        order.market_slug,
        order.outcome_label,
        order.side,
        trade_builder_notify_text(submitted_payload, "order_type").unwrap_or("N/A"),
        trade_builder_notify_text(submitted_payload, "submitted_selected_side")
            .or_else(|| iv.and_then(|iv| trade_builder_notify_text(iv, "selected_side")))
            .unwrap_or(order.outcome_label.as_str()),
        time_rule,
        trade_builder_notify_fmt_price(
            iv.and_then(|iv| trade_builder_notify_f64(iv, "seconds_left"))
        ),
        iv.and_then(|iv| trade_builder_notify_text(iv, "protection_result"))
            .unwrap_or("N/A"),
        trade_builder_notify_text(submitted_payload, "size_basis").unwrap_or("N/A"),
        trade_builder_notify_fmt_qty(target_qty),
        trade_builder_notify_fmt_price(model_ask),
        trade_builder_notify_fmt_price(best_ask),
        trade_builder_notify_fmt_price(expected_vwap),
        trade_builder_notify_fmt_signed(execution_vs_model, 4),
        trade_builder_notify_fmt_signed(
            trade_builder_notify_f64(submitted_payload, "submitted_vwap_slippage"),
            4,
        ),
        trade_builder_notify_fmt_usdc(estimated_notional),
        trade_builder_notify_fmt_price(cost_per_share),
        depth_guard_summary,
        trade_builder_notify_fmt_qty(trade_builder_notify_f64(
            submitted_payload,
            "submitted_depth_levels_used"
        ),),
        trade_builder_notify_fmt_qty(trade_builder_notify_f64(
            submitted_payload,
            "submitted_available_qty_at_best_ask"
        ),),
        trade_builder_notify_fmt_price(q_final),
        trade_builder_notify_fmt_price(selected_mid),
        trade_builder_notify_fmt_price(model_book_gap),
        model_book_zone,
        trade_builder_notify_fmt_signed(
            trade_builder_notify_f64(submitted_payload, "submitted_model_book_penalty"),
            4,
        ),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(
            submitted_payload,
            "submitted_adjusted_edge"
        ),),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(
            submitted_payload,
            "submitted_dynamic_threshold_before_credit"
        ),),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(
            submitted_payload,
            "submitted_participation_credit"
        ),),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(
            submitted_payload,
            "submitted_dynamic_threshold_after_credit"
        ),),
        trade_builder_notify_fmt_signed(
            trade_builder_notify_f64(submitted_payload, "submitted_adjusted_margin"),
            4,
        ),
        order.outcome_label,
        trade_builder_notify_fmt_usdc(trade_builder_notify_f64(
            submitted_payload,
            "submitted_if_win_pnl_est"
        ),),
        order.outcome_label,
        trade_builder_notify_fmt_usdc(trade_builder_notify_f64(
            submitted_payload,
            "submitted_if_loss_pnl_est"
        ),),
        trade_builder_notify_fmt_usdc(trade_builder_notify_f64(
            submitted_payload,
            "submitted_ev_est"
        ),),
        trade_builder_notify_fmt_signed(
            trade_builder_notify_f64(submitted_payload, "submitted_ev_roi_est")
                .map(|value| value * 100.0),
            1,
        ),
        trade_builder_notify_text(submitted_payload, "submitted_depth_guard_result")
            .unwrap_or("N/A"),
        trade_builder_notify_fmt_bool(trade_builder_notify_bool(
            submitted_payload,
            "submitted_late_high_price_warning",
        )),
        trade_builder_notify_fmt_bool(trade_builder_notify_bool(
            submitted_payload,
            "submitted_binance_same_direction",
        )),
        trade_builder_notify_fmt_price(trade_builder_notify_f64(
            submitted_payload,
            "submitted_spread"
        ),),
        trade_builder_notify_fmt_qty(trade_builder_notify_f64(
            submitted_payload,
            "submitted_stale_ms"
        ),),
        ptb_chop_block,
        medium_chop_margin_block,
        high_price_early_block,
        cex_open_gap_block,
        execution_limit_block,
    )
}

fn trade_builder_submitted_medium_chop_margin_block(iv: Option<&serde_json::Value>) -> String {
    let Some(iv) = iv else {
        return String::new();
    };
    let result = trade_builder_notify_text(iv, "medium_chop_margin_result").unwrap_or("N/A");
    if matches!(result, "N/A" | "off") {
        return String::new();
    }

    format!(
        "\nMedium Chop Margin Guard:\nmode={} decision_ref={} adjusted_margin={} required_margin={}\ncomponents: base={} high_price={} binance_fail_open={} stale={} result={}",
        trade_builder_notify_text(iv, "medium_chop_margin_mode").unwrap_or("N/A"),
        trade_builder_notify_fmt_cent(trade_builder_notify_f64(
            iv,
            "medium_chop_margin_decision_ref_cent",
        )),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "medium_chop_margin_adjusted_margin"),
            4,
        ),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "medium_chop_margin_required_margin"),
            4,
        ),
        trade_builder_notify_fmt_number(trade_builder_notify_f64(iv, "medium_chop_margin_base"), 4),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "medium_chop_margin_high_price_add"),
            4,
        ),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "medium_chop_margin_binance_fail_open_add"),
            4,
        ),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "medium_chop_margin_stale_add"),
            4,
        ),
        result,
    )
}

fn trade_builder_submitted_high_price_early_block(iv: Option<&serde_json::Value>) -> String {
    let Some(iv) = iv else {
        return String::new();
    };
    let result = trade_builder_notify_text(iv, "high_price_early_guard_result").unwrap_or("N/A");
    if matches!(result, "N/A" | "off" | "not_applicable") {
        return String::new();
    }

    format!(
        "\nHigh Price Early Reversal Guard:\nenabled={} applies={} result={} reasons={}\ndecision_ref={} seconds={} q_final={} q_extreme={}\ngap: base={} stale_add={} binance_add={} effective={}\nconfirm: q_binance_available={} stale_ms={} cex={} clean={}",
        trade_builder_notify_fmt_bool(trade_builder_notify_bool(
            iv,
            "high_price_early_guard_enabled",
        )),
        trade_builder_notify_fmt_bool(trade_builder_notify_bool(iv, "high_price_early_applies")),
        result,
        trade_builder_notify_string_list(iv, "high_price_early_guard_reasons"),
        trade_builder_notify_fmt_cent(trade_builder_notify_f64(
            iv,
            "high_price_early_decision_ref_cent",
        )),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "high_price_early_seconds_left"),
            2,
        ),
        trade_builder_notify_fmt_cent(
            trade_builder_notify_f64(iv, "high_price_early_q_final").map(|value| value * 100.0),
        ),
        trade_builder_notify_fmt_bool(trade_builder_notify_bool(iv, "high_price_early_q_extreme")),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "high_price_early_base_required_gap_strength"),
            4,
        ),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "high_price_early_stale_gap_add_applied"),
            4,
        ),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "high_price_early_binance_missing_gap_add_applied"),
            4,
        ),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "high_price_early_effective_required_gap_strength"),
            4,
        ),
        trade_builder_notify_fmt_bool(trade_builder_notify_bool(
            iv,
            "high_price_early_q_binance_available",
        )),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "high_price_early_chainlink_staleness_ms"),
            0,
        ),
        trade_builder_notify_text(iv, "high_price_early_cex_consensus").unwrap_or("N/A"),
        trade_builder_notify_fmt_bool(trade_builder_notify_bool(iv, "high_price_early_cex_clean")),
    )
}

fn trade_builder_submitted_ptb_chop_block(iv: Option<&serde_json::Value>) -> String {
    let Some(iv) = iv.filter(|iv| {
        trade_builder_notify_has_non_null(iv, "ptb_chop_guard_enabled")
            || trade_builder_notify_has_non_null(iv, "ptb_chop_risk")
            || trade_builder_notify_has_non_null(iv, "ptb_movement_mode")
    }) else {
        return String::new();
    };

    let mode = trade_builder_notify_text(iv, "ptb_movement_mode")
        .or_else(|| trade_builder_notify_text(iv, "ptb_chop_risk"))
        .unwrap_or("N/A");
    let action = trade_builder_notify_text(iv, "ptb_movement_action")
        .or_else(|| trade_builder_notify_text(iv, "ptb_chop_action"))
        .unwrap_or("N/A");
    let reason = trade_builder_notify_text(iv, "ptb_movement_reason")
        .or_else(|| trade_builder_notify_text(iv, "ptb_chop_block_reason"))
        .unwrap_or("N/A");

    format!(
        "\nPTB Movement: mode={} | action={} | reason={}\ncross10={} cross15={} path10={} z={} efficiency={} maxJumpZ={}\noppDepth={} sameSideAge={}s cex={} bookDislocation={} penalty={}",
        mode,
        action,
        reason,
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "ptb_chop_zero_cross_count_10s"),
            0,
        ),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "ptb_chop_zero_cross_count_15s"),
            0,
        ),
        trade_builder_notify_fmt_number(trade_builder_notify_f64(iv, "ptb_chop_gap_path_10s"), 4),
        trade_builder_notify_fmt_number(trade_builder_notify_f64(iv, "ptb_chop_gap_path_z_10s"), 4,),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "ptb_chop_efficiency_ratio_10s"),
            4,
        ),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "ptb_chop_max_1s_jump_z_10s"),
            4,
        ),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "ptb_chop_opposite_depth_z_10s"),
            4,
        ),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "ptb_chop_same_side_age_seconds"),
            2,
        ),
        trade_builder_notify_text(iv, "ptb_movement_cex_consensus").unwrap_or("N/A"),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "ptb_movement_model_book_dislocation"),
            4,
        ),
        trade_builder_notify_fmt_number(
            trade_builder_notify_f64(iv, "ptb_chop_gap_strength_penalty"),
            4,
        ),
    )
}

fn trade_builder_submitted_notification_idempotency_key(
    order: &TradeBuilderOrder,
    submitted_payload: &serde_json::Value,
) -> String {
    let exchange_id = trade_builder_notify_text(submitted_payload, "exchange_order_id")
        .or_else(|| trade_builder_notify_text(submitted_payload, "client_order_id"))
        .unwrap_or("unknown");
    let attempt = trade_builder_notify_f64(submitted_payload, "trigger_size_index")
        .unwrap_or(order.triggers_fired as f64 + 1.0);
    format!("{}:{exchange_id}:{attempt:.0}", order.id)
}

async fn maybe_send_trade_builder_submitted_notification(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    submitted_payload: &serde_json::Value,
    flow_payload: Option<&serde_json::Value>,
) -> Result<bool> {
    if !order.notify_on_order_submitted {
        return Ok(false);
    }
    let live_gap_metadata = if trade_builder_is_primary_buy_entry(order) {
        repo.load_trade_builder_order_live_gap_metadata(order.id)
            .await?
    } else {
        None
    };
    let idempotency_key =
        trade_builder_submitted_notification_idempotency_key(order, submitted_payload);
    let events = repo
        .list_trade_builder_order_events_for_orders(&[order.id])
        .await?;
    let already_sent = events.iter().any(|event| {
        event.event_type == "notification_sent"
            && event
                .payload_json
                .get("notification_type")
                .and_then(serde_json::Value::as_str)
                == Some("order_submitted")
            && event
                .payload_json
                .get("idempotency_key")
                .and_then(serde_json::Value::as_str)
                == Some(idempotency_key.as_str())
    });
    if already_sent {
        return Ok(false);
    }

    let message = build_trade_builder_submitted_notification_message_with_live_gap(
        order,
        submitted_payload,
        flow_payload,
        live_gap_metadata.as_ref(),
    );
    Ok(send_trade_builder_notification_with_payload(
        repo,
        order,
        "order_submitted",
        &message,
        Some(serde_json::json!({
            "idempotency_key": idempotency_key,
            "exchange_order_id": trade_builder_notify_text(submitted_payload, "exchange_order_id"),
            "client_order_id": trade_builder_notify_text(submitted_payload, "client_order_id"),
        })),
    )
    .await)
}

async fn resolve_trade_builder_fill_execution_analysis(
    repo: &PostgresRepository,
    exchange_order_id: &str,
    fallback_price: f64,
    fallback_qty: f64,
    fallback_qty_source: Option<&str>,
) -> TradeBuilderFillExecutionAnalysis {
    match repo
        .aggregate_fill_metrics_by_exchange_order_id(exchange_order_id)
        .await
    {
        Ok((qty, notional)) if qty > 0.0 && notional > 0.0 => TradeBuilderFillExecutionAnalysis {
            actual_fill_price: notional / qty,
            actual_filled_qty: qty,
            actual_notional: notional,
            actual_fill_source: "fills_aggregate",
        },
        _ if fallback_qty_source == Some("order_status_size_matched") => {
            TradeBuilderFillExecutionAnalysis {
                actual_fill_price: fallback_price,
                actual_filled_qty: fallback_qty,
                actual_notional: fallback_price * fallback_qty,
                actual_fill_source: "order_status_size_matched_with_execution_price",
            }
        }
        _ => TradeBuilderFillExecutionAnalysis {
            actual_fill_price: fallback_price,
            actual_filled_qty: fallback_qty,
            actual_notional: fallback_price * fallback_qty,
            actual_fill_source: "fallback_execution_price",
        },
    }
}

fn build_trade_builder_fill_execution_payload(
    order: &TradeBuilderOrder,
    analysis: &TradeBuilderFillExecutionAnalysis,
    flow_payload: Option<&serde_json::Value>,
    submitted_payload: Option<&serde_json::Value>,
) -> serde_json::Value {
    let iv = trade_builder_notify_iv(flow_payload);
    let target_qty = submitted_payload
        .and_then(|payload| trade_builder_submitted_target_qty(order, payload))
        .or(order.target_qty)
        .unwrap_or(analysis.actual_filled_qty);
    let fill_ratio = if target_qty > 0.0 {
        Some((analysis.actual_filled_qty / target_qty).clamp(0.0, 1.0))
    } else {
        None
    };
    let expected_vwap = submitted_payload.and_then(trade_builder_submitted_estimated_avg_fill);
    let best_ask = submitted_payload.and_then(trade_builder_submitted_best_ask);
    let (effective_cost, fee_component, buffer_component) =
        trade_builder_effective_actual_cost_parts(analysis.actual_fill_price, iv);
    let effective_total_cost = effective_cost * analysis.actual_filled_qty;
    let if_win = analysis.actual_filled_qty - effective_total_cost;
    let if_loss = -effective_total_cost;

    serde_json::json!({
        "fill_actual_price": analysis.actual_fill_price,
        "fill_actual_qty": analysis.actual_filled_qty,
        "fill_actual_notional": analysis.actual_notional,
        "fill_source": analysis.actual_fill_source,
        "fill_target_qty": target_qty,
        "fill_ratio": fill_ratio,
        "is_partial_fill": fill_ratio.is_some_and(|ratio| ratio < 0.999),
        "fill_slippage_vs_vwap": expected_vwap.map(|value| analysis.actual_fill_price - value),
        "fill_slippage_vs_best_ask": best_ask.map(|value| analysis.actual_fill_price - value),
        "fill_effective_actual_cost": effective_cost,
        "fill_effective_fee_component": fee_component,
        "fill_effective_buffer_component": buffer_component,
        "fill_effective_total_cost": effective_total_cost,
        "fill_if_win_pnl_est": if_win,
        "fill_if_loss_pnl_est": if_loss,
    })
}

fn build_trade_builder_fill_analysis_block(
    order: &TradeBuilderOrder,
    analysis: &TradeBuilderFillExecutionAnalysis,
    flow_payload: Option<&serde_json::Value>,
    submitted_payload: Option<&serde_json::Value>,
    exit_position_summary: Option<&TradeBuilderExitFillPositionSummary>,
) -> String {
    let iv = trade_builder_notify_iv(flow_payload);
    let target_qty = submitted_payload
        .and_then(|payload| trade_builder_submitted_target_qty(order, payload))
        .or(order.target_qty)
        .unwrap_or(analysis.actual_filled_qty);
    let fill_ratio = if target_qty > 0.0 {
        Some((analysis.actual_filled_qty / target_qty).clamp(0.0, 1.0))
    } else {
        None
    };
    let expected_vwap = submitted_payload.and_then(trade_builder_submitted_estimated_avg_fill);
    let best_ask = submitted_payload.and_then(trade_builder_submitted_best_ask);
    let (effective_cost, fee_component, buffer_component) =
        trade_builder_effective_actual_cost_parts(analysis.actual_fill_price, iv);
    let total_cost = effective_cost * analysis.actual_filled_qty;
    let if_win = analysis.actual_filled_qty - total_cost;
    let if_loss = -total_cost;
    let fill_source_warning = if analysis.actual_fill_source == "fallback_execution_price" {
        "\nFill Source Warning: fallback execution price used"
    } else {
        ""
    };

    let position_block = exit_position_summary
        .map(|summary| trade_builder_exit_fill_position_summary_block(order, summary))
        .unwrap_or_else(|| {
            format!(
                "\n\nPosition Risk\nIf {} wins: {}\nIf {} loses: {}",
                order.outcome_label,
                trade_builder_notify_fmt_usdc(Some(if_win)),
                order.outcome_label,
                trade_builder_notify_fmt_usdc(Some(if_loss)),
            )
        });

    format!(
        "\n\nFill Analysis\nTarget Qty: {}\nFilled Qty: {}\nFill Ratio: {}\nPartial Fill: {}\nActual Fill Price: {}\nActual Notional: {}\nActual Fill Source: {}{}\n\nComparison\nExpected VWAP: {}\nBest Ask at Submit: {}\nSlippage vs VWAP: {}\nSlippage vs Best Ask: {}\n\nActual Effective Cost\n{} = fill {} + fee {} + buffer {}{}",
        trade_builder_notify_fmt_qty(Some(target_qty)),
        trade_builder_notify_fmt_qty(Some(analysis.actual_filled_qty)),
        fill_ratio
            .map(|ratio| format!("{:.1}%", ratio * 100.0))
            .unwrap_or_else(|| "N/A".to_string()),
        fill_ratio.is_some_and(|ratio| ratio < 0.999),
        trade_builder_notify_fmt_price(Some(analysis.actual_fill_price)),
        trade_builder_notify_fmt_usdc(Some(analysis.actual_notional)),
        analysis.actual_fill_source,
        fill_source_warning,
        trade_builder_notify_fmt_price(expected_vwap),
        trade_builder_notify_fmt_price(best_ask),
        trade_builder_notify_fmt_signed(
            expected_vwap.map(|value| analysis.actual_fill_price - value),
            4,
        ),
        trade_builder_notify_fmt_signed(
            best_ask.map(|value| analysis.actual_fill_price - value),
            4,
        ),
        trade_builder_notify_fmt_price(Some(effective_cost)),
        trade_builder_notify_fmt_price(Some(analysis.actual_fill_price)),
        trade_builder_notify_fmt_price(Some(fee_component)),
        trade_builder_notify_fmt_price(Some(buffer_component)),
        position_block,
    )
}

#[cfg(test)]
mod submitted_notification_tests {
    use super::*;
    use serde_json::json;

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

        let block = trade_builder_submitted_ptb_chop_block(Some(&iv));

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
            "medium_chop_margin_result": "pass",
            "medium_chop_margin_mode": "medium_chop",
            "medium_chop_margin_decision_ref_cent": 83.0,
            "medium_chop_margin_adjusted_margin": 0.081,
            "medium_chop_margin_required_margin": 0.06,
            "medium_chop_margin_base": 0.045,
            "medium_chop_margin_high_price_add": 0.005,
            "medium_chop_margin_binance_fail_open_add": 0.005,
            "medium_chop_margin_stale_add": 0.005,
        });

        let block = trade_builder_submitted_medium_chop_margin_block(Some(&iv));

        assert!(block.contains("Medium Chop Margin Guard:"));
        assert!(block.contains("mode=medium_chop decision_ref=83.00c"));
        assert!(block.contains("adjusted_margin=0.0810 required_margin=0.0600"));
        assert!(block.contains("binance_fail_open=0.0050 stale=0.0050 result=pass"));
    }
}
