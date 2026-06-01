fn live_gap_adaptive_low_gap_metadata_matches_order(
    metadata: &Value,
    order: &TradeBuilderOrder,
) -> bool {
    metadata.get("market_slug").and_then(Value::as_str) == Some(order.market_slug.as_str())
        && metadata.get("token_id").and_then(Value::as_str) == Some(order.token_id.as_str())
        && metadata.get("outcome_label").and_then(Value::as_str)
            == Some(order.outcome_label.as_str())
}

fn live_gap_adaptive_low_gap_near_miss_notify_enabled(metadata: &Value) -> bool {
    metadata
        .pointer("/resolved_guard_config/notifyOnLiveGapAdaptiveLowGapNearMissChange")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn live_gap_adaptive_low_gap_guard_miss_price(price: Option<f64>) -> String {
    price
        .filter(|value| value.is_finite())
        .map(|value| format!("{:.0}c", value * 100.0))
        .unwrap_or_else(|| "N/A".to_string())
}

fn live_gap_adaptive_low_gap_guard_miss_usd(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{value:.2} USD"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn build_live_gap_adaptive_low_gap_near_miss_change_message(
    metadata: &Value,
    order: &TradeBuilderOrder,
    reason_code: &str,
    change: &LiveGapAdaptiveLowGapChangeNotification,
) -> String {
    format!(
        "Adaptive Low Gap Near-Miss Changed\nMarket: {}\nOutcome: {}\nReason: {}\nKey: {}\nNear Miss Count: {}\nPrevious Relax: {}\nNew Relax: {}\nPre Required Gap: {}\nAdaptive Required Gap: {} -> {}\nShortfall: {} / {}\nSaved From Block: {}\nOrder Max Price: {}\nBest Ask Relax: {}",
        live_gap_collector_format_str(metadata, "market_slug"),
        live_gap_collector_format_str(metadata, "outcome_label"),
        reason_code,
        live_gap_collector_format_str(metadata, "adaptive_low_gap_key"),
        live_gap_collector_format_i64(metadata, "adaptive_low_gap_market_near_miss_count"),
        live_gap_collector_format_pct(change.previous_relax_pct),
        live_gap_collector_format_pct(Some(change.new_relax_pct)),
        live_gap_collector_format_f64(metadata, "pre_adaptive_required_gap_usd", 2, " USD"),
        live_gap_adaptive_low_gap_guard_miss_usd(change.previous_adaptive_required_gap_usd),
        live_gap_adaptive_low_gap_guard_miss_usd(Some(change.new_adaptive_required_gap_usd)),
        live_gap_collector_format_f64(metadata, "adaptive_low_gap_shortfall_usd", 2, " USD"),
        live_gap_collector_format_pct(
            metadata
                .get("adaptive_low_gap_shortfall_pct")
                .and_then(value_as_f64)
        ),
        metadata
            .get("adaptive_saved_from_block")
            .and_then(Value::as_bool)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "N/A".to_string()),
        live_gap_adaptive_low_gap_guard_miss_price(order.max_price),
        metadata
            .pointer("/best_ask_unavailable_relax/applied")
            .and_then(Value::as_bool)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "false".to_string()),
    )
}

fn build_live_gap_adaptive_low_gap_near_miss_change_payload(
    metadata: &Value,
    order: &TradeBuilderOrder,
    reason_code: &str,
    change: &LiveGapAdaptiveLowGapChangeNotification,
) -> Value {
    json!({
        "notification_type": "live_gap_adaptive_low_gap_near_miss_changed",
        "reason_code": reason_code,
        "market_slug": &order.market_slug,
        "token_id": &order.token_id,
        "outcome_label": &order.outcome_label,
        "adaptive_low_gap": metadata.get("adaptive_low_gap").cloned().unwrap_or(Value::Null),
        "adaptive_low_gap_key": metadata.get("adaptive_low_gap_key").cloned().unwrap_or(Value::Null),
        "adaptive_low_gap_market_near_miss_count": metadata.get("adaptive_low_gap_market_near_miss_count").cloned().unwrap_or(Value::Null),
        "previous_relax_pct": change.previous_relax_pct,
        "previous_adaptive_required_gap_usd": change.previous_adaptive_required_gap_usd,
        "new_relax_pct": change.new_relax_pct,
        "new_adaptive_required_gap_usd": change.new_adaptive_required_gap_usd,
        "shortfall_usd": metadata.get("adaptive_low_gap_shortfall_usd").cloned().unwrap_or(Value::Null),
        "shortfall_pct": metadata.get("adaptive_low_gap_shortfall_pct").cloned().unwrap_or(Value::Null),
        "saved_from_block": metadata.get("adaptive_saved_from_block").cloned().unwrap_or(Value::Null),
        "order_max_price": order.max_price,
        "best_ask_unavailable_relax": metadata.get("best_ask_unavailable_relax").cloned().unwrap_or(Value::Null),
        "fallback_best_bid": metadata.get("fallback_best_bid").cloned().unwrap_or(Value::Null),
        "fallback_best_ask": metadata.get("fallback_best_ask").cloned().unwrap_or(Value::Null),
        "notified_at_ms": change.notified_at_ms,
    })
}

async fn maybe_send_live_gap_adaptive_low_gap_near_miss_change_notification(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    reason_code: &str,
    metadata: &Value,
    now_ms: i64,
) -> bool {
    if !live_gap_adaptive_low_gap_near_miss_notify_enabled(metadata) {
        return false;
    }
    let Some(change) = live_gap_mark_adaptive_low_gap_near_miss_change_notified(metadata, now_ms)
    else {
        return false;
    };
    let event_payload = build_live_gap_adaptive_low_gap_near_miss_change_payload(
        metadata,
        order,
        reason_code,
        &change,
    );
    if let Err(err) = repo
        .append_trade_builder_order_event(
            order.id,
            "live_gap_adaptive_low_gap_near_miss_changed",
            &event_payload,
        )
        .await
    {
        warn!(
            builder_order_id = order.id,
            reason_code,
            error = %err,
            "ADAPTIVE_LOW_GAP_NEAR_MISS_CHANGE_EVENT_FAILED"
        );
    }
    let message = build_live_gap_adaptive_low_gap_near_miss_change_message(
        metadata,
        order,
        reason_code,
        &change,
    );
    send_trade_builder_notification_with_payload(
        repo,
        order,
        "live_gap_adaptive_low_gap_near_miss_changed",
        &message,
        Some(event_payload),
    )
    .await
}

async fn maybe_record_live_gap_adaptive_low_gap_guard_miss(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    reason_code: &str,
    now_ms: i64,
) -> Result<Option<Value>> {
    if !trade_builder_is_primary_buy_entry(order) {
        return Ok(None);
    }
    let Some(mut metadata) = repo
        .load_trade_builder_order_live_gap_metadata(order.id)
        .await?
    else {
        return Ok(None);
    };
    if !trade_builder_live_gap_metadata_mode(Some(&metadata))
        || !live_gap_adaptive_low_gap_metadata_matches_order(&metadata, order)
    {
        return Ok(None);
    }

    live_gap_record_adaptive_low_gap_near_miss_from_payload(&mut metadata, reason_code, now_ms);
    let recorded = metadata
        .get("adaptive_low_gap_near_miss_recorded")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !recorded {
        return Ok(None);
    }

    repo.set_trade_builder_order_live_gap_metadata(order.id, Some(&metadata))
        .await?;
    let payload = json!({
        "reason_code": reason_code,
        "market_slug": &order.market_slug,
        "token_id": &order.token_id,
        "outcome_label": &order.outcome_label,
        "adaptive_low_gap": metadata.get("adaptive_low_gap").cloned().unwrap_or(Value::Null),
        "adaptive_low_gap_key": metadata.get("adaptive_low_gap_key"),
        "adaptive_low_gap_market_near_miss_count": metadata.get("adaptive_low_gap_market_near_miss_count"),
        "adaptive_low_gap_near_miss_recorded": true,
    });
    repo.append_trade_builder_order_event(
        order.id,
        "adaptive_low_gap_guard_near_miss_recorded",
        &payload,
    )
    .await?;
    maybe_send_live_gap_adaptive_low_gap_near_miss_change_notification(
        repo,
        order,
        reason_code,
        &metadata,
        now_ms,
    )
    .await;
    Ok(Some(payload))
}

async fn record_live_gap_adaptive_low_gap_guard_miss_or_warn(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    reason_code: &str,
    now_ms: i64,
) {
    if let Err(err) =
        maybe_record_live_gap_adaptive_low_gap_guard_miss(repo, order, reason_code, now_ms).await
    {
        warn!(
            builder_order_id = order.id,
            reason_code,
            error = %err,
            "ADAPTIVE_LOW_GAP_GUARD_MISS_RECORD_FAILED"
        );
    }
}

async fn record_adaptive_low_gap_above_max_or_warn(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    now_ms: i64,
) {
    record_live_gap_adaptive_low_gap_guard_miss_or_warn(repo, order, "above_max_price", now_ms)
        .await;
}
