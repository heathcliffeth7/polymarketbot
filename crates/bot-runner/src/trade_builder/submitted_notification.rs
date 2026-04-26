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
    let fee = trade_builder_iv_expected_fee_rate(iv)
        * actual_fill_price
        * (1.0 - actual_fill_price);
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
        .or_else(|| payload.get("best_ask").and_then(trade_builder_notify_value_as_f64));
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
        serde_json::json!(payload.get("normalized_status").and_then(serde_json::Value::as_str)),
    );
    payload.insert(
        "submitted_target_qty".to_string(),
        serde_json::json!(Some(target_qty)),
    );
    trade_builder_append_optional_number(payload, "submitted_best_ask", best_ask);
    trade_builder_append_optional_number(payload, "submitted_estimated_avg_fill", estimated_avg_fill);
    trade_builder_append_optional_number(
        payload,
        "submitted_vwap_slippage",
        iv.and_then(|iv| trade_builder_notify_f64(iv, "vwap_slippage")),
    );
    trade_builder_append_optional_number(payload, "submitted_estimated_notional", estimated_notional);
    trade_builder_append_optional_number(payload, "submitted_estimated_total_cost", estimated_total_cost);
    trade_builder_append_optional_number(payload, "submitted_effective_cost_per_share", cost_per_share);
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
        serde_json::json!(iv
            .and_then(|iv| iv.get("protection_reasons"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|reasons| reasons.iter().any(|reason| {
                reason.as_str() == Some("warn_late_high_price_unconfirmed")
            }))),
    );
    payload.insert(
        "submitted_binance_same_direction".to_string(),
        serde_json::json!(iv.and_then(|iv| trade_builder_notify_bool(iv, "binance_same_direction"))),
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

fn build_trade_builder_submitted_notification_message(
    order: &TradeBuilderOrder,
    submitted_payload: &serde_json::Value,
    flow_payload: Option<&serde_json::Value>,
) -> String {
    let iv = trade_builder_notify_iv(flow_payload);
    let target_qty = trade_builder_submitted_target_qty(order, submitted_payload);
    let expected_vwap = trade_builder_submitted_estimated_avg_fill(submitted_payload);
    let best_ask = trade_builder_submitted_best_ask(submitted_payload);
    let cost_per_share = trade_builder_submitted_cost_per_share(submitted_payload);
    let estimated_notional =
        trade_builder_notify_f64(submitted_payload, "submitted_estimated_notional");
    let q_final = trade_builder_notify_f64(submitted_payload, "submitted_q_final");
    let selected_mid = trade_builder_notify_f64(submitted_payload, "submitted_selected_mid");
    let model_book_gap = trade_builder_notify_f64(submitted_payload, "submitted_model_book_gap");
    let model_book_zone = trade_builder_notify_text(submitted_payload, "submitted_model_book_zone")
        .unwrap_or("N/A");
    let selected_rule = iv.and_then(|iv| iv.get("selected_time_rule"));
    let rule_start = selected_rule.and_then(|rule| trade_builder_notify_f64(rule, "start_remaining_secs"));
    let rule_end = selected_rule.and_then(|rule| trade_builder_notify_f64(rule, "end_remaining_secs"));
    let time_rule = match (rule_start, rule_end) {
        (Some(start), Some(end)) => format!("{start:.0}-{end:.0} sn"),
        _ => "N/A".to_string(),
    };

    format!(
        "Emir Gonderildi - Guard Gecti, Fill Bekleniyor\nMarket: {}\nOutcome: {}\nSide: {}\nOrder Status: SUBMITTED\nOrder Type: {}\n\nKarar Ozeti\nSelected: {}\nMode: IV Mismatch Edge\nRule: {}\nSeconds Left: {}\nProtection Result: {}\n\nSizing / Cost\nSize Mode: {}\nTarget Qty: {}\nBest Ask: {}\nEstimated VWAP Fill: {}\nVWAP Slippage: {}\nEstimated Notional: {}\nEffective Cost/Share: {}\nDepth Guard: {}\nDepth Levels Used: {}\nAvailable at Best Ask: {}\n\nModel / Book\nq_final: {}\nSelected Mid: {}\nModel-Book Gap: {}\nModel-Book Zone: {}\nModel-Book Penalty: {}\n\nEdge\nAdjusted Edge: {}\nThreshold Before Credit: {}\nParticipation Credit: {}\nFinal Threshold: {}\nMargin: {}\n\nEstimated Scenario\nIf {} wins: {}\nIf {} loses: {}\nEV: {}\nEV ROI: {}\n\nRisk Flags\nDepth: {}\nLate High Price: {}\nBinance Same Direction: {}\nSpread: {}\nStale: {}ms",
        order.market_slug,
        order.outcome_label,
        order.side,
        trade_builder_notify_text(submitted_payload, "order_type").unwrap_or("N/A"),
        trade_builder_notify_text(submitted_payload, "submitted_selected_side")
            .or_else(|| iv.and_then(|iv| trade_builder_notify_text(iv, "selected_side")))
            .unwrap_or(order.outcome_label.as_str()),
        time_rule,
        trade_builder_notify_fmt_price(iv.and_then(|iv| trade_builder_notify_f64(iv, "seconds_left"))),
        trade_builder_notify_text(submitted_payload, "submitted_depth_guard_result")
            .or_else(|| iv.and_then(|iv| trade_builder_notify_text(iv, "protection_result")))
            .unwrap_or("PASS"),
        trade_builder_notify_text(submitted_payload, "size_basis").unwrap_or("N/A"),
        trade_builder_notify_fmt_qty(target_qty),
        trade_builder_notify_fmt_price(best_ask),
        trade_builder_notify_fmt_price(expected_vwap),
        trade_builder_notify_fmt_signed(
            trade_builder_notify_f64(submitted_payload, "submitted_vwap_slippage"),
            4,
        ),
        trade_builder_notify_fmt_usdc(estimated_notional),
        trade_builder_notify_fmt_price(cost_per_share),
        trade_builder_notify_text(submitted_payload, "submitted_depth_guard_result").unwrap_or("N/A"),
        trade_builder_notify_fmt_qty(
            trade_builder_notify_f64(submitted_payload, "submitted_depth_levels_used"),
        ),
        trade_builder_notify_fmt_qty(
            trade_builder_notify_f64(submitted_payload, "submitted_available_qty_at_best_ask"),
        ),
        trade_builder_notify_fmt_price(q_final),
        trade_builder_notify_fmt_price(selected_mid),
        trade_builder_notify_fmt_price(model_book_gap),
        model_book_zone,
        trade_builder_notify_fmt_signed(
            trade_builder_notify_f64(submitted_payload, "submitted_model_book_penalty"),
            4,
        ),
        trade_builder_notify_fmt_price(
            trade_builder_notify_f64(submitted_payload, "submitted_adjusted_edge"),
        ),
        trade_builder_notify_fmt_price(
            trade_builder_notify_f64(submitted_payload, "submitted_dynamic_threshold_before_credit"),
        ),
        trade_builder_notify_fmt_price(
            trade_builder_notify_f64(submitted_payload, "submitted_participation_credit"),
        ),
        trade_builder_notify_fmt_price(
            trade_builder_notify_f64(submitted_payload, "submitted_dynamic_threshold_after_credit"),
        ),
        trade_builder_notify_fmt_signed(
            trade_builder_notify_f64(submitted_payload, "submitted_adjusted_margin"),
            4,
        ),
        order.outcome_label,
        trade_builder_notify_fmt_usdc(
            trade_builder_notify_f64(submitted_payload, "submitted_if_win_pnl_est"),
        ),
        order.outcome_label,
        trade_builder_notify_fmt_usdc(
            trade_builder_notify_f64(submitted_payload, "submitted_if_loss_pnl_est"),
        ),
        trade_builder_notify_fmt_usdc(
            trade_builder_notify_f64(submitted_payload, "submitted_ev_est"),
        ),
        trade_builder_notify_fmt_signed(
            trade_builder_notify_f64(submitted_payload, "submitted_ev_roi_est")
                .map(|value| value * 100.0),
            1,
        ),
        trade_builder_notify_text(submitted_payload, "submitted_depth_guard_result").unwrap_or("N/A"),
        trade_builder_notify_fmt_bool(trade_builder_notify_bool(
            submitted_payload,
            "submitted_late_high_price_warning",
        )),
        trade_builder_notify_fmt_bool(trade_builder_notify_bool(
            submitted_payload,
            "submitted_binance_same_direction",
        )),
        trade_builder_notify_fmt_price(
            trade_builder_notify_f64(submitted_payload, "submitted_spread"),
        ),
        trade_builder_notify_fmt_qty(
            trade_builder_notify_f64(submitted_payload, "submitted_stale_ms"),
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
    let idempotency_key =
        trade_builder_submitted_notification_idempotency_key(order, submitted_payload);
    let events = repo
        .list_trade_builder_order_events_for_orders(&[order.id])
        .await?;
    let already_sent = events.iter().any(|event| {
        event.event_type == "notification_sent"
            && event.payload_json.get("notification_type").and_then(serde_json::Value::as_str)
                == Some("order_submitted")
            && event.payload_json.get("idempotency_key").and_then(serde_json::Value::as_str)
                == Some(idempotency_key.as_str())
    });
    if already_sent {
        return Ok(false);
    }

    let message =
        build_trade_builder_submitted_notification_message(order, submitted_payload, flow_payload);
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

    format!(
        "\n\nFill Analysis\nTarget Qty: {}\nFilled Qty: {}\nFill Ratio: {}\nPartial Fill: {}\nActual Fill Price: {}\nActual Notional: {}\nActual Fill Source: {}\n\nComparison\nExpected VWAP: {}\nBest Ask at Submit: {}\nSlippage vs VWAP: {}\nSlippage vs Best Ask: {}\n\nActual Effective Cost\n{} = fill {} + fee {} + buffer {}\n\nPosition Risk\nIf {} wins: {}\nIf {} loses: {}",
        trade_builder_notify_fmt_qty(Some(target_qty)),
        trade_builder_notify_fmt_qty(Some(analysis.actual_filled_qty)),
        fill_ratio
            .map(|ratio| format!("{:.1}%", ratio * 100.0))
            .unwrap_or_else(|| "N/A".to_string()),
        fill_ratio.is_some_and(|ratio| ratio < 0.999),
        trade_builder_notify_fmt_price(Some(analysis.actual_fill_price)),
        trade_builder_notify_fmt_usdc(Some(analysis.actual_notional)),
        analysis.actual_fill_source,
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
        order.outcome_label,
        trade_builder_notify_fmt_usdc(Some(if_win)),
        order.outcome_label,
        trade_builder_notify_fmt_usdc(Some(if_loss)),
    )
}
