const PRE_BUY_COLLAPSE_GUARD_NOTIFICATION_STATE_KEY: &str = "preBuyCollapseGuardNotificationState";

fn should_send_live_gap_collector_decision_notification(
    config: &ActionPlaceOrderLiveGapCollectorConfig,
    decision: &LiveGapCollectorDecision,
    payload: &Value,
) -> bool {
    if let Some(no_reversal) = payload.get("no_reversal_entry_guard") {
        let no_reversal_reason = no_reversal
            .get("reason_code")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let protection = no_reversal
            .get("protection")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !decision.passed && no_reversal_reason == decision.reason_code {
            return true;
        }
        if decision.passed
            && protection == "not_applied"
            && no_reversal_reason == decision.reason_code
        {
            return true;
        }
    }
    config.notify_on_decision && (decision.passed || decision.terminal)
}

fn live_gap_collector_decision_notification_type(
    decision: &LiveGapCollectorDecision,
) -> &'static str {
    if decision.passed {
        "live_gap_collector_buy"
    } else {
        "live_gap_collector_block"
    }
}

fn live_gap_collector_format_f64(
    payload: &Value,
    key: &str,
    digits: usize,
    suffix: &str,
) -> String {
    payload
        .get(key)
        .and_then(value_as_f64)
        .map(|value| format!("{value:.digits$}{suffix}"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn live_gap_collector_format_str(payload: &Value, key: &str) -> String {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| "N/A".to_string())
}

fn live_gap_collector_format_i64(payload: &Value, key: &str) -> String {
    payload
        .get(key)
        .and_then(Value::as_i64)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

fn live_gap_collector_ptb_notification_line(payload: &Value) -> String {
    let Some(ptb) = payload.get("ptb_telemetry").and_then(Value::as_object) else {
        return "PTB: telemetry only / N/A".to_string();
    };
    let price_to_beat = ptb
        .get("price_to_beat")
        .and_then(value_as_f64)
        .map(|value| format!("{value:.4}"))
        .unwrap_or_else(|| "N/A".to_string());
    let latency = ptb
        .get("ptb_lag_ms")
        .or_else(|| ptb.get("lag_ms"))
        .or_else(|| ptb.get("source_latency_ms"))
        .and_then(value_as_f64)
        .map(|value| format!("{value:.0}ms"))
        .unwrap_or_else(|| "N/A".to_string());
    let source = ptb
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    format!("PTB: telemetry only, Value: {price_to_beat}, Lag: {latency}, Source: {source}")
}

fn live_gap_collector_adaptive_low_gap_notification_line(payload: &Value) -> Option<String> {
    payload.get("adaptive_low_gap")?;
    Some(format!(
        "Adaptive Low Gap:\nStatus: {}\nScope: {}\nMarket Near Miss Count: {}\nPre Required: {}\nAdaptive Required: {}\nRelax: {}\nShortfall: {} / {}\nSaved From Block: {}\nReason: {}",
        live_gap_collector_format_str(payload, "adaptive_low_gap_status"),
        live_gap_collector_format_str(payload, "adaptive_low_gap_scope"),
        live_gap_collector_format_i64(payload, "adaptive_low_gap_market_near_miss_count"),
        live_gap_collector_format_f64(payload, "pre_adaptive_required_gap_usd", 2, " USD"),
        live_gap_collector_format_f64(payload, "adaptive_required_gap_usd", 2, " USD"),
        live_gap_collector_format_f64(payload, "adaptive_low_gap_relax_pct", 3, ""),
        live_gap_collector_format_f64(payload, "adaptive_low_gap_shortfall_usd", 2, " USD"),
        live_gap_collector_format_f64(payload, "adaptive_low_gap_shortfall_pct", 3, ""),
        payload
            .get("adaptive_saved_from_block")
            .and_then(Value::as_bool)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "N/A".to_string()),
        live_gap_collector_format_str(payload, "adaptive_low_gap_reason"),
    ))
}

fn live_gap_collector_format_pct(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| format!("{:.1}%", value * 100.0))
        .unwrap_or_else(|| "N/A".to_string())
}

fn build_live_gap_adaptive_low_gap_change_notification_message(
    payload: &Value,
    change: &LiveGapAdaptiveLowGapChangeNotification,
) -> String {
    format!(
        "Adaptive Low Gap Changed\nMarket: {}\nOutcome: {}\nBand: {}\nPrice Bucket: {}\nPrevious Relax: {}\nNew Relax: {}\nPre Required Gap: {}\nAdaptive Required Gap: {}\nMarket Near Miss Count: {}\nShortfall: {} / {}\nSaved From Block: {}\nReason: {}",
        live_gap_collector_format_str(payload, "market_slug"),
        live_gap_collector_format_str(payload, "outcome_label"),
        live_gap_collector_format_str(payload, "gap_band"),
        payload
            .get("adaptive_low_gap")
            .and_then(|value| value.get("price_bucket"))
            .and_then(Value::as_str)
            .unwrap_or("N/A"),
        live_gap_collector_format_pct(change.previous_relax_pct),
        live_gap_collector_format_pct(Some(change.new_relax_pct)),
        live_gap_collector_format_f64(payload, "pre_adaptive_required_gap_usd", 2, " USD"),
        live_gap_collector_format_f64(payload, "adaptive_required_gap_usd", 2, " USD"),
        live_gap_collector_format_i64(payload, "adaptive_low_gap_market_near_miss_count"),
        live_gap_collector_format_f64(payload, "adaptive_low_gap_shortfall_usd", 2, " USD"),
        live_gap_collector_format_pct(payload.get("adaptive_low_gap_shortfall_pct").and_then(value_as_f64)),
        payload
            .get("adaptive_saved_from_block")
            .and_then(Value::as_bool)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "N/A".to_string()),
        live_gap_collector_format_str(payload, "adaptive_low_gap_reason"),
    )
}

fn live_gap_guard_text<'a>(guard: &'a Value, key: &str) -> Option<&'a str> {
    guard.get(key).and_then(Value::as_str)
}

fn live_gap_guard_f64(guard: &Value, key: &str) -> Option<f64> {
    guard.get(key).and_then(value_as_f64)
}

fn live_gap_guard_i64(guard: &Value, key: &str) -> Option<i64> {
    guard.get(key).and_then(value_as_i64)
}

fn live_gap_guard_signed_usd(guard: &Value, key: &str) -> String {
    live_gap_guard_f64(guard, key)
        .map(|value| format!("{value:+.1} USD"))
        .unwrap_or_else(|| "not_available".to_string())
}

fn live_gap_guard_unsigned_usd(guard: &Value, key: &str) -> String {
    live_gap_guard_f64(guard, key)
        .map(|value| format!("{value:.1} USD"))
        .unwrap_or_else(|| "not_available".to_string())
}

fn live_gap_guard_slope(guard: &Value, key: &str) -> String {
    live_gap_guard_f64(guard, key)
        .map(|value| format!("{value:+.2} USD/s"))
        .unwrap_or_else(|| "not_available".to_string())
}

fn live_gap_guard_history_secs(guard: &Value, key: &str) -> String {
    live_gap_guard_i64(guard, key)
        .map(|value| format!("{:.0}s", value as f64 / 1_000.0))
        .unwrap_or_else(|| "insufficient_samples".to_string())
}

fn live_gap_guard_count(guard: &Value, key: &str) -> String {
    live_gap_guard_i64(guard, key)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "not_available".to_string())
}

fn live_gap_guard_largest_gap(guard: &Value) -> String {
    live_gap_guard_i64(guard, "largest_sample_gap_ms")
        .map(|value| format!("{value}ms"))
        .unwrap_or_else(|| "not_available".to_string())
}

fn live_gap_guard_display_value(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) if !value.trim().is_empty() => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => "not_available".to_string(),
    }
}

fn live_gap_guard_hash_short(guard: &Value) -> String {
    guard
        .get("profile_lookup_key")
        .and_then(|key| key.get("profile_config_hash"))
        .and_then(Value::as_str)
        .map(|value| value.chars().take(8).collect::<String>())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "not_available".to_string())
}

fn live_gap_guard_profile_lookup_key_line(guard: &Value) -> Option<String> {
    let key = guard.get("profile_lookup_key")?;
    let window_start = live_gap_guard_display_value(key.get("target_window_start"));
    let node_key = live_gap_guard_display_value(key.get("node_key"));
    let direction = live_gap_guard_display_value(key.get("direction"));
    let remaining = live_gap_guard_display_value(key.get("remaining_bucket"));
    let price = live_gap_guard_display_value(key.get("price_bucket"));
    let gap = live_gap_guard_display_value(key.get("gap_bucket"));
    let slope = live_gap_guard_display_value(key.get("slope_bucket"));
    Some(format!(
        "Profile Lookup Key: window_start={window_start}, node={node_key}, direction={direction}, hash={}, buckets={remaining}/{price}/{gap}/{slope}",
        live_gap_guard_hash_short(guard),
    ))
}

fn live_gap_guard_decision_label(guard: &Value) -> &'static str {
    match live_gap_guard_text(guard, "decision") {
        Some("pass") => "PASS",
        Some("block_retry") | Some("block_terminal") => "BLOCK",
        _ => "UNKNOWN",
    }
}

fn live_gap_collector_no_reversal_notification_line(payload: &Value) -> Option<String> {
    let guard = payload.get("no_reversal_entry_guard")?;
    let selected = guard
        .get("selected_adverse_usd")
        .and_then(value_as_f64)
        .map(|value| format!("{value:.2} USD"))
        .unwrap_or_else(|| "N/A".to_string());
    let source_buffer = guard
        .get("source_buffer_usd")
        .and_then(value_as_f64)
        .map(|value| format!("{value:.2} USD"))
        .unwrap_or_else(|| "N/A".to_string());
    let worst_gap = guard
        .get("worst_expected_gap_usd")
        .and_then(value_as_f64)
        .map(|value| format!("{value:+.2} USD"))
        .unwrap_or_else(|| "N/A".to_string());
    let floor = guard
        .get("ptb_floor_usd")
        .and_then(value_as_f64)
        .map(|value| format!("{value:+.2} USD"))
        .unwrap_or_else(|| "N/A".to_string());
    let fallback = guard
        .get("fallback_level")
        .and_then(Value::as_str)
        .unwrap_or("N/A");
    let profile = guard
        .get("profile_source")
        .and_then(Value::as_str)
        .unwrap_or("N/A");
    let profile_lookup_fallback =
        live_gap_guard_text(guard, "profile_lookup_fallback_level").unwrap_or(fallback);
    let profile_lookup_status =
        live_gap_guard_text(guard, "profile_lookup_status").unwrap_or(profile);
    let prewarmer_status_line = live_gap_guard_text(guard, "prewarmer_status")
        .map(|status| format!("\nPrewarmer Status: {status}"))
        .unwrap_or_default();
    let prewarm_detail_line = {
        let priority = live_gap_guard_text(guard, "prewarm_priority");
        let slot_status = live_gap_guard_text(guard, "prewarm_slot_status");
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
    let protection = guard
        .get("protection")
        .and_then(Value::as_str)
        .unwrap_or("N/A");
    let reason = live_gap_guard_text(guard, "reason_code").unwrap_or("N/A");
    let fallback_source = live_gap_guard_text(guard, "runtime_fallback_source")
        .or_else(|| live_gap_guard_text(guard, "local_path_fallback_source"))
        .unwrap_or(fallback);
    let mut block = format!(
        "No-Reversal:\nProfile: {profile}\nProfile Status: {profile_lookup_status}{prewarmer_status_line}{prewarm_detail_line}\nProfile Lookup Fallback: {profile_lookup_fallback}\nFallback: {fallback_source}\nProtection: {protection}\nFloor: {floor}\nSelected: {selected}\nBuffer: {source_buffer}\nWorst: {worst_gap}\nReason: {reason}"
    );
    if let Some(line) = live_gap_guard_profile_lookup_key_line(guard) {
        block.push('\n');
        block.push_str(&line);
    }
    if protection == "local_path_applied" || guard.get("local_path_fallback_source").is_some() {
        let decision_reason =
            live_gap_guard_text(guard, "local_path_decision_reason").unwrap_or(reason);
        block.push_str(&format!(
            "\nLocal Path:\nHistory: {}\nSamples: {}\nLargest Sample Gap: {}\nMin Gap 30s: {}\nMin Gap 60s: {}\nMin Gap 2m: {}\nDrop10/30/60: {} / {} / {}\nSlope 3s/10s/30s: {} / {} / {}\nDecision: {}\nReason: {}",
            live_gap_guard_history_secs(guard, "local_path_history_ms"),
            live_gap_guard_count(guard, "local_path_sample_count"),
            live_gap_guard_largest_gap(guard),
            live_gap_guard_signed_usd(guard, "local_path_min_gap_30s"),
            live_gap_guard_signed_usd(guard, "local_path_min_gap_60s"),
            live_gap_guard_signed_usd(guard, "local_path_min_gap_2m"),
            live_gap_guard_unsigned_usd(guard, "local_path_drop_10s"),
            live_gap_guard_unsigned_usd(guard, "local_path_drop_30s"),
            live_gap_guard_unsigned_usd(guard, "local_path_drop_60s"),
            live_gap_guard_slope(guard, "local_path_slope_3s"),
            live_gap_guard_slope(guard, "local_path_slope_10s"),
            live_gap_guard_slope(guard, "local_path_slope_30s"),
            live_gap_guard_decision_label(guard),
            decision_reason,
        ));
    }
    Some(block)
}

fn build_live_gap_collector_decision_notification_message(
    decision: &LiveGapCollectorDecision,
    payload: &Value,
) -> String {
    let title = if decision.passed {
        "Live Gap Collector BUY"
    } else {
        "Live Gap Collector BLOCK"
    };
    let decision_label = if decision.passed { "BUY" } else { "BLOCK" };
    let market_slug = live_gap_collector_format_str(payload, "market_slug");
    let outcome = live_gap_collector_format_str(payload, "outcome_label");
    let side = live_gap_collector_format_str(payload, "side");
    let regime = live_gap_collector_format_str(payload, "regime");
    let gap_band = live_gap_collector_format_str(payload, "gap_band");
    let old_regime = live_gap_collector_format_str(payload, "old_4_band_equivalent");
    let band_reason = live_gap_collector_format_str(payload, "band_reason");
    let local_path_decision = live_gap_collector_format_str(payload, "local_path_decision");
    let remaining = payload
        .get("remaining_sec")
        .and_then(Value::as_i64)
        .map(|value| format!("{value}s"))
        .unwrap_or_else(|| "N/A".to_string());
    let mut message = format!(
        "{title}\nMarket: {market_slug}\nOutcome: {outcome}\nSide: {side}\nDecision: {decision_label}\nBest Ask: {}\nEffective Fill: {}\nLive Gap: {}\nRequired Gap: {}\nGap Band: {gap_band}\nBase Required Gap: {}\nFinal Required Gap: {}\nOld 4-Band Equivalent: {old_regime}\nVolume: ratio={}, bucket={}, 10/30/60/90/120={}/{}/{}/{}/{}, trades10/30/60/90/120={}/{}/{}/{}/{}\nVolatility 15s: {}\nLocal Path Decision: {local_path_decision}\nBand Reason: {band_reason}\nRegime: {regime}\nRemaining: {remaining}\nReason: {}\n{}",
        live_gap_collector_format_f64(payload, "best_ask", 4, ""),
        live_gap_collector_format_f64(payload, "effective_fill_price", 4, ""),
        live_gap_collector_format_f64(payload, "live_gap_usd", 2, " USD"),
        live_gap_collector_format_f64(payload, "required_gap_usd", 2, " USD"),
        live_gap_collector_format_f64(payload, "base_required_gap_usd", 2, " USD"),
        live_gap_collector_format_f64(payload, "final_required_gap_usd", 2, " USD"),
        live_gap_collector_format_f64(payload, "volume_ratio_30s", 3, ""),
        live_gap_collector_format_str(payload, "volume_bucket"),
        live_gap_collector_format_f64(payload, "volume_10s", 2, ""),
        live_gap_collector_format_f64(payload, "volume_30s", 2, ""),
        live_gap_collector_format_f64(payload, "volume_60s", 2, ""),
        live_gap_collector_format_f64(payload, "volume_90s", 2, ""),
        live_gap_collector_format_f64(payload, "volume_120s", 2, ""),
        live_gap_collector_format_i64(payload, "trade_count_10s"),
        live_gap_collector_format_i64(payload, "trade_count_30s"),
        live_gap_collector_format_i64(payload, "trade_count_60s"),
        live_gap_collector_format_i64(payload, "trade_count_90s"),
        live_gap_collector_format_i64(payload, "trade_count_120s"),
        live_gap_collector_format_f64(payload, "volatility_usd_15s", 4, " USD"),
        decision.reason_code,
        live_gap_collector_ptb_notification_line(payload),
    );
    if let Some(line) = live_gap_collector_no_reversal_notification_line(payload) {
        message.push('\n');
        message.push_str(&line);
    }
    if let Some(line) = live_gap_collector_adaptive_low_gap_notification_line(payload) {
        message.push('\n');
        message.push_str(&line);
    }
    message
}

async fn maybe_send_live_gap_collector_decision_notification(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    config: &ActionPlaceOrderLiveGapCollectorConfig,
    decision: &LiveGapCollectorDecision,
    payload: &Value,
) -> bool {
    if !should_send_live_gap_collector_decision_notification(config, decision, payload) {
        return false;
    }
    let message = build_live_gap_collector_decision_notification_message(decision, payload);
    send_trade_flow_notification(
        repo,
        run,
        &node.key,
        live_gap_collector_decision_notification_type(decision),
        &message,
    )
    .await
}

async fn maybe_send_live_gap_adaptive_low_gap_change_notification(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    config: &ActionPlaceOrderLiveGapCollectorConfig,
    payload: &Value,
) -> bool {
    if !config.notify_on_adaptive_low_gap_change {
        return false;
    }
    let now_ms = Utc::now().timestamp_millis();
    let Some(change) = live_gap_mark_adaptive_low_gap_change_notified(payload, now_ms) else {
        return false;
    };
    let mut event_payload = payload.clone();
    if let Some(obj) = event_payload.as_object_mut() {
        obj.insert("notification_type".to_string(), json!("live_gap_adaptive_low_gap_changed"));
        obj.insert("previous_relax_pct".to_string(), json!(change.previous_relax_pct));
        obj.insert(
            "previous_adaptive_required_gap_usd".to_string(),
            json!(change.previous_adaptive_required_gap_usd),
        );
        obj.insert("new_relax_pct".to_string(), json!(change.new_relax_pct));
        obj.insert(
            "new_adaptive_required_gap_usd".to_string(),
            json!(change.new_adaptive_required_gap_usd),
        );
        obj.insert("notified_at_ms".to_string(), json!(change.notified_at_ms));
    }
    if let Err(err) = repo
        .append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            "live_gap_adaptive_low_gap_changed",
            &event_payload,
        )
        .await
    {
        warn!(flow_run_id = run.id, node_key = node.key, error = ?err, "LIVE_GAP_ADAPTIVE_LOW_GAP_CHANGE_EVENT_FAILED");
    }
    let message = build_live_gap_adaptive_low_gap_change_notification_message(payload, &change);
    send_trade_flow_notification(
        repo,
        run,
        &node.key,
        "live_gap_adaptive_low_gap_changed",
        &message,
    )
    .await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreBuyCollapseNotificationDecision {
    BlockRetry,
    BlockTerminal,
    Buy,
}

impl PreBuyCollapseNotificationDecision {
    fn as_str(self) -> &'static str {
        match self {
            Self::BlockRetry => "block_retry",
            Self::BlockTerminal => "block_terminal",
            Self::Buy => "buy",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::BlockRetry => "BLOCK_RETRY",
            Self::BlockTerminal => "BLOCK_TERMINAL",
            Self::Buy => "BUY",
        }
    }
}

fn pre_buy_collapse_guard_notification_mode_is_off(mode: &str) -> bool {
    mode.trim().eq_ignore_ascii_case("off") || mode.trim().eq_ignore_ascii_case("none")
}

fn pre_buy_collapse_guard_notification_identity(
    node_key: &str,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
) -> String {
    format!(
        "{node_key}:{market_slug}:{token_id}:{}",
        outcome_label.trim().to_ascii_lowercase()
    )
}

fn pre_buy_collapse_payload_child(payload: &Value) -> Option<&Value> {
    payload.get("pre_buy_collapse_guard")
}

fn pre_buy_collapse_payload_value<'a>(payload: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let child = pre_buy_collapse_payload_child(payload);
    for key in keys {
        if let Some(value) = child.and_then(|child| child.get(*key)) {
            return Some(value);
        }
        if let Some(value) = payload.get(*key) {
            return Some(value);
        }
    }
    None
}

fn pre_buy_collapse_payload_f64(payload: &Value, keys: &[&str]) -> Option<f64> {
    pre_buy_collapse_payload_value(payload, keys).and_then(value_as_f64)
}

fn pre_buy_collapse_payload_i64(payload: &Value, keys: &[&str]) -> Option<i64> {
    pre_buy_collapse_payload_value(payload, keys).and_then(value_as_i64)
}

fn pre_buy_collapse_payload_bool(payload: &Value, keys: &[&str]) -> Option<bool> {
    pre_buy_collapse_payload_value(payload, keys).and_then(Value::as_bool)
}

fn pre_buy_collapse_payload_str(payload: &Value, keys: &[&str]) -> Option<String> {
    pre_buy_collapse_payload_value(payload, keys)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn pre_buy_collapse_nested_value<'a>(
    payload: &'a Value,
    object_key: &str,
    keys: &[&str],
) -> Option<&'a Value> {
    let child = pre_buy_collapse_payload_child(payload).unwrap_or(payload);
    let nested = child.get(object_key).or_else(|| payload.get(object_key));
    for key in keys {
        if let Some(value) = nested.and_then(|nested| nested.get(*key)) {
            return Some(value);
        }
    }
    None
}

fn pre_buy_collapse_nested_i64(payload: &Value, object_key: &str, keys: &[&str]) -> Option<i64> {
    pre_buy_collapse_nested_value(payload, object_key, keys).and_then(value_as_i64)
}

fn pre_buy_collapse_nested_str(payload: &Value, object_key: &str, keys: &[&str]) -> Option<String> {
    pre_buy_collapse_nested_value(payload, object_key, keys)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn pre_buy_collapse_payload_missing_reasons(payload: &Value) -> Vec<String> {
    pre_buy_collapse_payload_value(payload, &["missing_reasons"])
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn pre_buy_format_value(value: f64, digits: usize) -> String {
    format!("{value:.digits$}")
}

fn pre_buy_format_signed_value(value: f64, digits: usize) -> String {
    format!("{value:+.digits$}")
}

fn pre_buy_format_cents(value: Option<f64>) -> String {
    value
        .map(|price| format!("{:.1}c", price * 100.0))
        .unwrap_or_else(|| "N/A".to_string())
}

fn pre_buy_format_usd(value: Option<f64>, signed: bool) -> String {
    value
        .map(|amount| {
            if signed {
                format!("{} USD", pre_buy_format_signed_value(amount, 1))
            } else {
                format!("{} USD", pre_buy_format_value(amount, 1))
            }
        })
        .unwrap_or_else(|| "N/A".to_string())
}

fn pre_buy_format_slope(value: Option<f64>) -> String {
    value
        .map(|slope| pre_buy_format_signed_value(slope, 1))
        .unwrap_or_else(|| "N/A".to_string())
}

fn pre_buy_format_remaining(value: Option<i64>) -> String {
    value
        .map(|seconds| format!("{seconds}s"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn pre_buy_format_history_ms(value: Option<i64>) -> String {
    value
        .map(|ms| format!("{ms}ms"))
        .unwrap_or_else(|| "N/A".to_string())
}

fn pre_buy_metric_status(value: Option<bool>) -> &'static str {
    if value.unwrap_or(false) { "OK" } else { "N/A" }
}

fn pre_buy_collapse_guard_current_state(
    context: &Value,
    identity: &str,
) -> Option<(String, String, Option<String>, Option<String>)> {
    let state = flow_context_value(context, PRE_BUY_COLLAPSE_GUARD_NOTIFICATION_STATE_KEY)?;
    let entry = state.get(identity)?;
    Some((
        entry.get("decision")?.as_str()?.to_string(),
        entry.get("reasonCode")?.as_str()?.to_string(),
        entry
            .get("missingReason")
            .and_then(Value::as_str)
            .map(str::to_string),
        entry
            .get("clearKind")
            .and_then(Value::as_str)
            .map(str::to_string),
    ))
}

fn set_pre_buy_collapse_guard_notification_state(
    context: &mut Value,
    identity: &str,
    decision: PreBuyCollapseNotificationDecision,
    reason_code: &str,
    missing_reason: Option<&str>,
    clear_kind: Option<&str>,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
) {
    let mut state = flow_context_value(context, PRE_BUY_COLLAPSE_GUARD_NOTIFICATION_STATE_KEY)
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    state.insert(
        identity.to_string(),
        json!({
            "decision": decision.as_str(),
            "reasonCode": reason_code,
            "missingReason": missing_reason,
            "clearKind": clear_kind,
            "marketSlug": market_slug,
            "tokenId": token_id,
            "outcomeLabel": outcome_label,
            "updatedAtMs": Utc::now().timestamp_millis(),
        }),
    );
    set_flow_context(
        context,
        PRE_BUY_COLLAPSE_GUARD_NOTIFICATION_STATE_KEY,
        Value::Object(state),
    );
}

fn should_notify_pre_buy_collapse_guard_state_change(
    previous: Option<(String, String, Option<String>, Option<String>)>,
    decision: PreBuyCollapseNotificationDecision,
    reason_code: &str,
    missing_reason: Option<&str>,
    clear_kind: Option<&str>,
    mode: &str,
) -> bool {
    let mode = mode.trim().to_ascii_lowercase();
    if pre_buy_collapse_guard_notification_mode_is_off(&mode) {
        return false;
    }
    if mode == "all" {
        return true;
    }
    match decision {
        PreBuyCollapseNotificationDecision::BlockRetry => !matches!(
            previous.as_ref(),
            Some((prev_decision, prev_reason, prev_missing, _))
                if prev_decision == decision.as_str()
                    && prev_reason == reason_code
                    && prev_missing.as_deref() == missing_reason
        ),
        PreBuyCollapseNotificationDecision::BlockTerminal => !matches!(
            previous.as_ref(),
            Some((prev_decision, prev_reason, prev_missing, _))
                if prev_decision == decision.as_str()
                    && prev_reason == reason_code
                    && prev_missing.as_deref() == missing_reason
        ),
        PreBuyCollapseNotificationDecision::Buy => {
            (reason_code == "retrace_stabilized" || clear_kind.is_some())
                && matches!(
                    previous.as_ref(),
                    Some((prev_decision, _, _, _))
                        if prev_decision
                            == PreBuyCollapseNotificationDecision::BlockRetry.as_str()
                            || prev_decision
                                == PreBuyCollapseNotificationDecision::BlockTerminal.as_str()
                )
        }
    }
}

fn remember_pre_buy_collapse_guard_notification_state(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    decision: PreBuyCollapseNotificationDecision,
    reason_code: &str,
    missing_reason: Option<&str>,
    clear_kind: Option<&str>,
    mode: &str,
) -> bool {
    let identity = pre_buy_collapse_guard_notification_identity(
        node_key,
        market_slug,
        token_id,
        outcome_label,
    );
    let previous = pre_buy_collapse_guard_current_state(context, &identity);
    let should_notify = should_notify_pre_buy_collapse_guard_state_change(
        previous,
        decision,
        reason_code,
        missing_reason,
        clear_kind,
        mode,
    );
    set_pre_buy_collapse_guard_notification_state(
        context,
        &identity,
        decision,
        reason_code,
        missing_reason,
        clear_kind,
        market_slug,
        token_id,
        outcome_label,
    );
    should_notify
}

fn pre_buy_collapse_guard_notification_decision(
    decision: &LiveGapCollectorDecision,
) -> PreBuyCollapseNotificationDecision {
    if decision.passed {
        PreBuyCollapseNotificationDecision::Buy
    } else if decision.terminal {
        PreBuyCollapseNotificationDecision::BlockTerminal
    } else {
        PreBuyCollapseNotificationDecision::BlockRetry
    }
}

fn pre_buy_collapse_guard_notification_applicable(
    decision: &LiveGapCollectorDecision,
    payload: &Value,
) -> bool {
    if payload
        .get("no_reversal_entry_guard")
        .and_then(|value| value.get("reason_code"))
        .and_then(Value::as_str)
        .is_some_and(|reason| reason == decision.reason_code)
    {
        return false;
    }
    pre_buy_collapse_payload_child(payload).is_some()
        || matches!(decision.reason_code, "too_late_for_new_entry")
}

fn pre_buy_collapse_guard_notification_type(
    decision: PreBuyCollapseNotificationDecision,
    reason_code: &str,
    clear_kind: Option<&str>,
) -> &'static str {
    if reason_code == "insufficient_collapse_history" {
        return "pre_buy_history_warning";
    }
    if decision == PreBuyCollapseNotificationDecision::Buy && clear_kind.is_some() {
        return "pre_buy_history_guard_pass";
    }
    match decision {
        PreBuyCollapseNotificationDecision::BlockRetry => "pre_buy_collapse_guard_block",
        PreBuyCollapseNotificationDecision::BlockTerminal => {
            "pre_buy_collapse_guard_terminal_block"
        }
        PreBuyCollapseNotificationDecision::Buy => "pre_buy_collapse_guard_buy",
    }
}

fn build_pre_buy_collapse_guard_notification_message(
    decision: PreBuyCollapseNotificationDecision,
    reason_code: &str,
    payload: &Value,
    market_slug: &str,
    outcome_label: &str,
    side: &str,
) -> String {
    let clear_kind = pre_buy_collapse_payload_str(payload, &["clear_kind"]);
    let title = if reason_code == "insufficient_collapse_history" {
        "Pre-Buy History Warning"
    } else if decision == PreBuyCollapseNotificationDecision::Buy {
        match clear_kind.as_deref() {
            Some("short_history_clear" | "retrace_stabilized_short_history") => {
                "Short History Guard Pass"
            }
            Some("full_history_clear" | "retrace_stabilized_full_history") => {
                "Full History Guard Pass"
            }
            _ => "Bounce Confirmed BUY",
        }
    } else {
        match decision {
            PreBuyCollapseNotificationDecision::BlockRetry => "Pre-Buy Collapse Block",
            PreBuyCollapseNotificationDecision::BlockTerminal => "Pre-Buy Collapse Terminal Block",
            PreBuyCollapseNotificationDecision::Buy => "Bounce Confirmed BUY",
        }
    };
    let price = pre_buy_format_cents(pre_buy_collapse_payload_f64(
        payload,
        &[
            "effective_fill",
            "effective_fill_price",
            "candidate_effective_fill",
            "best_ask",
        ],
    ));
    let live_gap = pre_buy_format_usd(
        pre_buy_collapse_payload_f64(payload, &["live_gap", "live_gap_usd", "candidate_live_gap"]),
        true,
    );
    let required_gap = pre_buy_format_usd(
        pre_buy_collapse_payload_f64(
            payload,
            &["required_gap", "required_gap_usd", "candidate_required_gap"],
        ),
        false,
    );
    let gap_drop_3s = pre_buy_format_usd(
        pre_buy_collapse_payload_f64(payload, &["gap_drop_3s_usd"]),
        false,
    );
    let slope_1s = pre_buy_format_slope(pre_buy_collapse_payload_f64(
        payload,
        &["gap_slope_1s_usd_per_sec"],
    ));
    let slope_3s = pre_buy_format_slope(pre_buy_collapse_payload_f64(
        payload,
        &["gap_slope_3s_usd_per_sec"],
    ));
    let remaining =
        pre_buy_format_remaining(pre_buy_collapse_payload_i64(payload, &["remaining_sec"]));
    let side = if side.trim().is_empty() {
        outcome_label
    } else {
        side
    };
    let history_age = pre_buy_format_history_ms(
        pre_buy_collapse_nested_i64(payload, "history", &["age_ms"])
            .or_else(|| pre_buy_collapse_payload_i64(payload, &["history_age_ms"])),
    );
    let min_history = pre_buy_format_history_ms(
        pre_buy_collapse_nested_i64(payload, "history", &["min_required_ms"])
            .or_else(|| pre_buy_collapse_payload_i64(payload, &["history_min_age_ms"])),
    );
    let samples = pre_buy_collapse_nested_i64(payload, "history", &["sample_count"])
        .or_else(|| pre_buy_collapse_payload_i64(payload, &["sample_count"]))
        .map(|value| value.to_string())
        .unwrap_or_else(|| "N/A".to_string());
    let gap_1s_available = pre_buy_collapse_payload_bool(payload, &["gap_1s_available"]);
    let gap_3s_available = pre_buy_collapse_payload_bool(payload, &["gap_3s_available"]);
    let gap_5s_available = pre_buy_collapse_payload_bool(payload, &["gap_5s_available"]);
    let metrics_line = format!(
        "Metrics: 1s={}, 3s={}, 5s={}",
        pre_buy_metric_status(gap_1s_available),
        pre_buy_metric_status(gap_3s_available),
        pre_buy_metric_status(gap_5s_available),
    );
    let mut message = format!(
        "{title}\nMarket: {market_slug}\nSide: {side}\nPrice: {price}\nRemaining: {remaining}\nLive Gap: {live_gap} / Required: {required_gap}\nGap Drop 3s: {gap_drop_3s}\nSlope 1s/3s: {slope_1s} / {slope_3s} USD/s\nDecision: {}\nReason: {reason_code}",
        decision.label(),
    );
    if reason_code == "insufficient_collapse_history" || clear_kind.is_some() {
        message.push_str(&format!(
            "\nHistory: {history_age} / min {min_history}\nSamples: {samples}\n{metrics_line}"
        ));
    }
    let missing_reasons = pre_buy_collapse_payload_missing_reasons(payload);
    if !missing_reasons.is_empty() {
        message.push_str("\nWhy:");
        for reason in missing_reasons {
            message.push_str(&format!("\n- {reason}"));
        }
    }
    let detail = pre_buy_collapse_payload_str(payload, &["missing_reason_detail"])
        .or_else(|| pre_buy_collapse_nested_str(payload, "history", &["missing_reason_detail"]));
    if let Some(detail) = detail.filter(|value| !value.trim().is_empty()) {
        message.push_str(&format!("\nDetail: {detail}"));
    }
    if let Some(clear_kind) = clear_kind {
        message.push_str(&format!("\nClear Kind: {clear_kind}"));
    }
    message
}

async fn maybe_send_pre_buy_collapse_guard_notification(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &mut Value,
    config: &ActionPlaceOrderLiveGapCollectorConfig,
    decision: &LiveGapCollectorDecision,
    payload: &Value,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    side: &str,
) -> bool {
    if !pre_buy_collapse_guard_notification_applicable(decision, payload) {
        return false;
    }
    let is_history_warning = decision.reason_code == "insufficient_collapse_history";
    let notification_enabled = if is_history_warning {
        config.notify_on_pre_buy_history_warning
    } else {
        config.notify_pre_buy_collapse_guard_decision
    };
    let notification_mode = if is_history_warning {
        &config.pre_buy_history_warning_mode
    } else {
        &config.pre_buy_collapse_guard_notification_mode
    };
    if !notification_enabled || pre_buy_collapse_guard_notification_mode_is_off(notification_mode) {
        return true;
    }
    let notification_decision = pre_buy_collapse_guard_notification_decision(decision);
    let missing_reason = pre_buy_collapse_payload_str(payload, &["missing_reason"]);
    let clear_kind = pre_buy_collapse_payload_str(payload, &["clear_kind"]);
    if !remember_pre_buy_collapse_guard_notification_state(
        context,
        &node.key,
        market_slug,
        token_id,
        outcome_label,
        notification_decision,
        decision.reason_code,
        missing_reason.as_deref(),
        clear_kind.as_deref(),
        notification_mode,
    ) {
        return true;
    }
    let message = build_pre_buy_collapse_guard_notification_message(
        notification_decision,
        decision.reason_code,
        payload,
        market_slug,
        outcome_label,
        side,
    );
    send_trade_flow_notification(
        repo,
        run,
        &node.key,
        pre_buy_collapse_guard_notification_type(
            notification_decision,
            decision.reason_code,
            clear_kind.as_deref(),
        ),
        &message,
    )
    .await;
    true
}

fn live_gap_submit_revalidation_notifications_enabled(metadata: &Value) -> bool {
    let enabled = metadata
        .pointer("/resolved_guard_config/notifyOnPreBuyCollapseGuardDecision")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let mode = metadata
        .pointer("/resolved_guard_config/preBuyCollapseGuardNotificationMode")
        .and_then(Value::as_str)
        .unwrap_or("smart");
    enabled && !pre_buy_collapse_guard_notification_mode_is_off(mode)
}

fn build_live_gap_submit_revalidation_notification_message(
    order: &TradeBuilderOrder,
    payload: &Value,
) -> String {
    build_live_gap_submit_revalidation_notification_message_from_fields(
        &order.market_slug,
        &order.outcome_label,
        order
            .working_price
            .or(order.submitted_dynamic_price)
            .or(order.last_seen_price),
        payload,
    )
}

fn build_live_gap_submit_revalidation_notification_message_from_fields(
    market_slug: &str,
    outcome_label: &str,
    fallback_price: Option<f64>,
    payload: &Value,
) -> String {
    let fresh_decision = pre_buy_collapse_payload_str(payload, &["fresh_revalidation_decision"])
        .unwrap_or_else(|| LIVE_GAP_SUBMIT_REVALIDATION_BLOCK_RETRY.to_string());
    let title = if fresh_decision == LIVE_GAP_SUBMIT_REVALIDATION_FRESH_PASS {
        "Pre-Buy Submit Revalidation Pass"
    } else {
        "Pre-Buy Submit Revalidation Block"
    };
    let price = pre_buy_format_cents(
        pre_buy_collapse_payload_f64(
            payload,
            &[
                "effective_fill",
                "effective_fill_price",
                "candidate_effective_fill",
                "best_ask",
            ],
        )
        .or(fallback_price),
    );
    let live_gap = pre_buy_format_usd(
        pre_buy_collapse_payload_f64(payload, &["live_gap", "live_gap_usd", "candidate_live_gap"]),
        true,
    );
    let required_gap = pre_buy_format_usd(
        pre_buy_collapse_payload_f64(
            payload,
            &["required_gap", "required_gap_usd", "candidate_required_gap"],
        ),
        false,
    );
    let age =
        pre_buy_collapse_payload_i64(payload, &["original_candidate_age_ms", "candidate_age_ms"])
            .map(|value| format!("{value}ms"))
            .unwrap_or_else(|| "N/A".to_string());
    let candidate_created_at = pre_buy_collapse_payload_i64(payload, &["candidate_created_at_ms"])
        .map(|value| format!("{value}ms"))
        .unwrap_or_else(|| "N/A".to_string());
    let fresh_snapshot_age = pre_buy_collapse_payload_i64(payload, &["fresh_snapshot_age_ms"])
        .map(|value| format!("{value}ms"))
        .unwrap_or_else(|| "N/A".to_string());
    let fresh_revalidation_ts =
        pre_buy_collapse_payload_i64(payload, &["fresh_revalidation_ts_ms"])
            .map(|value| format!("{value}ms"))
            .unwrap_or_else(|| "N/A".to_string());
    let max_age =
        pre_buy_collapse_payload_i64(payload, &["candidate_reuse_max_ms", "candidate_max_age_ms"])
            .map(|value| format!("{value}ms"))
            .unwrap_or_else(|| "N/A".to_string());
    let remaining =
        pre_buy_format_remaining(pre_buy_collapse_payload_i64(payload, &["remaining_sec"]));
    let trigger = pre_buy_collapse_payload_str(payload, &["revalidation_trigger"])
        .unwrap_or_else(|| "N/A".to_string());
    let candidate_reuse =
        pre_buy_collapse_payload_str(payload, &["candidate_reuse", "candidate_reuse_decision"])
        .unwrap_or_else(|| "N/A".to_string());
    let decision_reason = pre_buy_collapse_payload_str(payload, &["decision_reason"])
        .unwrap_or_else(|| "N/A".to_string());
    let clob = pre_buy_collapse_payload_str(payload, &["clob_submit_decision"])
        .unwrap_or_else(|| "N/A".to_string());
    let fresh_guard_reason = pre_buy_collapse_payload_str(payload, &["fresh_guard_reason"])
        .map(|value| format!("\nFresh Guard Reason: {value}"))
        .unwrap_or_default();
    let late_high_price = payload
        .pointer("/late_high_price_risk/mode")
        .and_then(Value::as_str)
        .filter(|mode| *mode == "notify_only")
        .map(|mode| format!("\nLate High Price: {mode}"))
        .unwrap_or_default();
    let no_reversal = live_gap_collector_no_reversal_notification_line(payload)
        .map(|block| format!("\n\n{block}"))
        .unwrap_or_default();
    format!(
        "{title}\nMarket: {market_slug}\nSide: {outcome_label}\nPrice: {price}\nRemaining: {remaining}\nLive Gap: {live_gap} / Required: {required_gap}\nTrigger: {trigger}\nOriginal Candidate Age: {age} / Reuse Max: {max_age}\nCandidate Created At: {candidate_created_at}\nFresh Snapshot Age: {fresh_snapshot_age}\nFresh Revalidation TS: {fresh_revalidation_ts}\nCandidate Reuse: {candidate_reuse}\nFresh Revalidation: {fresh_decision}\nDecision Reason: {decision_reason}{fresh_guard_reason}\nCLOB: {clob}{late_high_price}{no_reversal}"
    )
}

fn live_gap_submit_revalidation_notification_type(payload: &Value) -> &'static str {
    match payload
        .get("fresh_revalidation_decision")
        .and_then(Value::as_str)
    {
        Some(LIVE_GAP_SUBMIT_REVALIDATION_FRESH_PASS) => "pre_buy_submit_revalidation_pass",
        Some(LIVE_GAP_SUBMIT_REVALIDATION_BLOCK_TERMINAL) => {
            "pre_buy_submit_revalidation_terminal_block"
        }
        _ => "pre_buy_submit_revalidation_block",
    }
}

fn live_gap_submit_revalidation_notification_target(
    metadata: &Value,
    order: &TradeBuilderOrder,
    payload: &mut Value,
) -> (Option<&'static str>, Option<String>) {
    if !live_gap_submit_revalidation_notifications_enabled(metadata) {
        return (None, None);
    }
    let mode = metadata
        .pointer("/resolved_guard_config/preBuyCollapseGuardNotificationMode")
        .and_then(Value::as_str)
        .unwrap_or("smart");
    if !remember_live_gap_submit_revalidation_notification_state(payload, order, mode) {
        return (None, None);
    }
    (
        Some(live_gap_submit_revalidation_notification_type(payload)),
        Some(build_live_gap_submit_revalidation_notification_message(
            order, payload,
        )),
    )
}

#[cfg(test)]
mod pre_buy_collapse_notification_tests {
    use super::*;

    fn empty_context() -> Value {
        json!({})
    }

    #[test]
    fn smart_mode_notifies_first_block_only_once_for_same_reason() {
        let mut context = empty_context();
        let first = remember_pre_buy_collapse_guard_notification_state(
            &mut context,
            "node",
            "btc-updown-5m-1777900500",
            "tok-up",
            "Up",
            PreBuyCollapseNotificationDecision::BlockRetry,
            "late_high_price_gap_collapsing",
            None,
            None,
            "smart",
        );
        let second = remember_pre_buy_collapse_guard_notification_state(
            &mut context,
            "node",
            "btc-updown-5m-1777900500",
            "tok-up",
            "Up",
            PreBuyCollapseNotificationDecision::BlockRetry,
            "late_high_price_gap_collapsing",
            None,
            None,
            "smart",
        );
        assert!(first);
        assert!(!second);
    }

    #[test]
    fn smart_mode_notifies_when_block_reason_changes() {
        let mut context = empty_context();
        assert!(remember_pre_buy_collapse_guard_notification_state(
            &mut context,
            "node",
            "btc-updown-5m-1777900500",
            "tok-up",
            "Up",
            PreBuyCollapseNotificationDecision::BlockRetry,
            "insufficient_collapse_history",
            Some("history_not_started_yet"),
            None,
            "smart",
        ));
        assert!(remember_pre_buy_collapse_guard_notification_state(
            &mut context,
            "node",
            "btc-updown-5m-1777900500",
            "tok-up",
            "Up",
            PreBuyCollapseNotificationDecision::BlockRetry,
            "late_high_price_gap_collapsing",
            None,
            None,
            "smart",
        ));
    }

    #[test]
    fn smart_mode_notifies_when_missing_reason_changes() {
        let mut context = empty_context();
        assert!(remember_pre_buy_collapse_guard_notification_state(
            &mut context,
            "node",
            "btc-updown-5m-1777900500",
            "tok-up",
            "Up",
            PreBuyCollapseNotificationDecision::BlockRetry,
            "insufficient_collapse_history",
            Some("history_not_started_yet"),
            None,
            "smart",
        ));
        assert!(remember_pre_buy_collapse_guard_notification_state(
            &mut context,
            "node",
            "btc-updown-5m-1777900500",
            "tok-up",
            "Up",
            PreBuyCollapseNotificationDecision::BlockRetry,
            "insufficient_collapse_history",
            Some("trigger_armed_late"),
            None,
            "smart",
        ));
    }

    #[test]
    fn smart_mode_notifies_retrace_buy_after_block() {
        let mut context = empty_context();
        assert!(remember_pre_buy_collapse_guard_notification_state(
            &mut context,
            "node",
            "btc-updown-5m-1777900500",
            "tok-up",
            "Up",
            PreBuyCollapseNotificationDecision::BlockRetry,
            "late_high_price_gap_collapsing",
            None,
            None,
            "smart",
        ));
        assert!(remember_pre_buy_collapse_guard_notification_state(
            &mut context,
            "node",
            "btc-updown-5m-1777900500",
            "tok-up",
            "Up",
            PreBuyCollapseNotificationDecision::Buy,
            "retrace_stabilized",
            None,
            Some("retrace_stabilized_full_history"),
            "smart",
        ));
    }

    #[test]
    fn off_mode_suppresses_notification_but_updates_state() {
        let mut context = empty_context();
        assert!(!remember_pre_buy_collapse_guard_notification_state(
            &mut context,
            "node",
            "btc-updown-5m-1777900500",
            "tok-up",
            "Up",
            PreBuyCollapseNotificationDecision::BlockRetry,
            "late_high_price_gap_collapsing",
            None,
            None,
            "off",
        ));
        assert!(
            flow_context_value(&context, PRE_BUY_COLLAPSE_GUARD_NOTIFICATION_STATE_KEY).is_some()
        );
    }

    #[test]
    fn block_message_uses_operator_friendly_fields() {
        let payload = json!({
            "pre_buy_collapse_guard": {
                "effective_fill": 0.889,
                "remaining_sec": 58,
                "live_gap": 42.7,
                "required_gap": 34.0,
                "gap_drop_3s_usd": 9.4,
                "gap_slope_1s_usd_per_sec": -3.4,
                "gap_slope_3s_usd_per_sec": -3.1
            }
        });
        let message = build_pre_buy_collapse_guard_notification_message(
            PreBuyCollapseNotificationDecision::BlockRetry,
            "late_high_price_gap_collapsing",
            &payload,
            "btc-updown-5m-1777900500",
            "Up",
            "Up",
        );
        assert!(message.contains("Pre-Buy Collapse Block"));
        assert!(message.contains("Price: 88.9c"));
        assert!(message.contains("Live Gap: +42.7 USD / Required: 34.0 USD"));
        assert!(message.contains("Decision: BLOCK_RETRY"));
    }

    #[test]
    fn history_warning_message_explains_missing_metrics() {
        let payload = json!({
            "pre_buy_collapse_guard": {
                "effective_fill": 0.887,
                "remaining_sec": 56,
                "live_gap": 38.4,
                "required_gap": 34.0,
                "history": {
                    "age_ms": 312,
                    "min_required_ms": 750,
                    "sample_count": 3,
                    "gap_1s_available": false,
                    "gap_3s_available": false,
                    "gap_5s_available": false
                },
                "gap_1s_available": false,
                "gap_3s_available": false,
                "gap_5s_available": false,
                "missing_reasons": ["action_started_recently", "no_3s_history_yet"],
                "missing_reason_detail": "action has only watched this side for 312ms"
            }
        });
        let message = build_pre_buy_collapse_guard_notification_message(
            PreBuyCollapseNotificationDecision::BlockRetry,
            "insufficient_collapse_history",
            &payload,
            "btc-updown-5m-1777900500",
            "Up",
            "Up",
        );
        assert!(message.contains("Pre-Buy History Warning"));
        assert!(message.contains("History: 312ms / min 750ms"));
        assert!(message.contains("Metrics: 1s=N/A, 3s=N/A, 5s=N/A"));
        assert!(message.contains("Why:"));
        assert!(message.contains("Detail: action has only watched this side for 312ms"));
    }
}
