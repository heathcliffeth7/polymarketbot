#[derive(Debug, Clone, Copy, PartialEq)]
struct PreBuyCollapseGuardConfig {
    candidate_max_age_ms: i64,
    late_remaining_sec: i64,
    history_min_age_ms: i64,
    high_price: f64,
    very_high_price: f64,
    high_price_gap_drop_3s_usd: f64,
    mid_price_gap_drop_3s_usd: f64,
    mid_price_gap_drop_5s_usd: f64,
    bounce_buffer_usd: f64,
}

const PRE_BUY_COLLAPSE_HISTORY_DEFAULT_RETENTION_MS: i64 = 30_000;

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreBuyCollapseSample {
    ts_ms: i64,
    live_gap: f64,
    effective_fill: f64,
    best_ask: f64,
    sample_source: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
struct PreBuyCollapseGuardInput<'a> {
    market_slug: &'a str,
    token_id: &'a str,
    outcome_label: &'a str,
    remaining_sec: i64,
    effective_fill: f64,
    best_ask: f64,
    live_gap: f64,
    required_gap: f64,
    retry_ms: i64,
    elapsed_sec: i64,
    window_start_sec: i64,
    no_new_entry_under_sec: i64,
    trigger_condition: Option<&'a str>,
    trigger_price: Option<f64>,
    triggered_price: Option<f64>,
    trigger_source: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PreBuyCollapseMetrics {
    history_age_ms: i64,
    sample_count: usize,
    first_sample_ts_ms: Option<i64>,
    last_sample_ts_ms: Option<i64>,
    oldest_sample_age_ms: Option<i64>,
    newest_sample_age_ms: Option<i64>,
    largest_sample_gap_ms: Option<i64>,
    history_store_uptime_ms: i64,
    first_sample_source: Option<&'static str>,
    last_sample_source: Option<&'static str>,
    gap_1s_ago: Option<f64>,
    gap_3s_ago: Option<f64>,
    gap_5s_ago: Option<f64>,
    gap_1s_available: bool,
    gap_3s_available: bool,
    gap_5s_available: bool,
    gap_drop_3s_usd: Option<f64>,
    gap_drop_5s_usd: Option<f64>,
    gap_slope_1s_usd_per_sec: Option<f64>,
    gap_slope_3s_usd_per_sec: Option<f64>,
    local_price_high_10s: Option<f64>,
    price_retrace_cent: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
struct PreBuyCollapseGuardDecision {
    passed: bool,
    reason_code: &'static str,
    payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
struct LiveGapSubmitRevalidationDecision {
    passed: bool,
    terminal: bool,
    reason_code: &'static str,
    payload: Value,
}

static PRE_BUY_COLLAPSE_HISTORY: LazyLock<
    StdMutex<HashMap<String, VecDeque<PreBuyCollapseSample>>>,
> = LazyLock::new(|| StdMutex::new(HashMap::new()));

static PRE_BUY_COLLAPSE_HISTORY_STORE_STARTED_AT_MS: LazyLock<i64> =
    LazyLock::new(|| Utc::now().timestamp_millis());

fn resolve_pre_buy_collapse_guard_config(node: &TradeFlowNode) -> PreBuyCollapseGuardConfig {
    PreBuyCollapseGuardConfig {
        candidate_max_age_ms: node_config_i64(node, "liveGapCollectorCandidateMaxAgeMs")
            .unwrap_or(500)
            .clamp(50, 5_000),
        late_remaining_sec: node_config_i64(node, "liveGapCollectorCollapseLateRemainingSec")
            .unwrap_or(60)
            .clamp(0, 300),
        history_min_age_ms: node_config_i64(node, "liveGapCollectorCollapseHistoryMinAgeMs")
            .unwrap_or(750)
            .clamp(0, 5_000),
        high_price: (node_config_f64(node, "liveGapCollectorCollapseHighPriceCent")
            .unwrap_or(85.0)
            / 100.0)
            .clamp(0.01, 0.99),
        very_high_price: (node_config_f64(node, "liveGapCollectorCollapseVeryHighPriceCent")
            .unwrap_or(89.0)
            / 100.0)
            .clamp(0.01, 0.99),
        high_price_gap_drop_3s_usd: node_config_f64(
            node,
            "liveGapCollectorCollapseHighPriceGapDrop3sUsd",
        )
        .unwrap_or(8.0)
        .max(0.0),
        mid_price_gap_drop_3s_usd: node_config_f64(
            node,
            "liveGapCollectorCollapseMidPriceGapDrop3sUsd",
        )
        .unwrap_or(12.0)
        .max(0.0),
        mid_price_gap_drop_5s_usd: node_config_f64(
            node,
            "liveGapCollectorCollapseMidPriceGapDrop5sUsd",
        )
        .unwrap_or(16.0)
        .max(0.0),
        bounce_buffer_usd: node_config_f64(node, "liveGapCollectorBounceBufferUsd")
            .unwrap_or(3.0)
            .max(0.0),
    }
}

fn pre_buy_collapse_guard_config_snapshot(config: &PreBuyCollapseGuardConfig) -> Value {
    json!({
        "candidateMaxAgeMs": config.candidate_max_age_ms,
        "lateRemainingSec": config.late_remaining_sec,
        "historyMinAgeMs": config.history_min_age_ms,
        "highPriceCent": config.high_price * 100.0,
        "veryHighPriceCent": config.very_high_price * 100.0,
        "gapDrop3sUsd": config.high_price_gap_drop_3s_usd,
        "midPriceGapDrop3sUsd": config.mid_price_gap_drop_3s_usd,
        "gapDrop5sUsd": config.mid_price_gap_drop_5s_usd,
        "bounceBufferUsd": config.bounce_buffer_usd,
    })
}

fn pre_buy_collapse_guard_config_from_metadata(metadata: &Value) -> PreBuyCollapseGuardConfig {
    let cfg = metadata
        .get("resolved_guard_config")
        .or_else(|| metadata.get("pre_buy_collapse_guard_config"));
    let f64_at = |key: &str| cfg.and_then(|value| value.get(key)).and_then(value_as_f64);
    let i64_at = |key: &str| cfg.and_then(|value| value.get(key)).and_then(value_as_i64);
    PreBuyCollapseGuardConfig {
        candidate_max_age_ms: i64_at("candidateMaxAgeMs").unwrap_or(500).clamp(50, 5_000),
        late_remaining_sec: i64_at("lateRemainingSec").unwrap_or(60).clamp(0, 300),
        history_min_age_ms: i64_at("historyMinAgeMs").unwrap_or(750).clamp(0, 5_000),
        high_price: (f64_at("highPriceCent").unwrap_or(85.0) / 100.0).clamp(0.01, 0.99),
        very_high_price: (f64_at("veryHighPriceCent").unwrap_or(89.0) / 100.0).clamp(0.01, 0.99),
        high_price_gap_drop_3s_usd: f64_at("gapDrop3sUsd").unwrap_or(8.0).max(0.0),
        mid_price_gap_drop_3s_usd: f64_at("midPriceGapDrop3sUsd").unwrap_or(12.0).max(0.0),
        mid_price_gap_drop_5s_usd: f64_at("gapDrop5sUsd").unwrap_or(16.0).max(0.0),
        bounce_buffer_usd: f64_at("bounceBufferUsd").unwrap_or(3.0).max(0.0),
    }
}

fn pre_buy_collapse_guard_key(market_slug: &str, token_id: &str, outcome_label: &str) -> String {
    format!(
        "{market_slug}:{token_id}:{}",
        outcome_label.trim().to_ascii_lowercase()
    )
}

#[cfg(test)]
fn record_pre_buy_collapse_sample(
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    sample: PreBuyCollapseSample,
) {
    record_pre_buy_collapse_sample_with_retention(
        market_slug,
        token_id,
        outcome_label,
        sample,
        PRE_BUY_COLLAPSE_HISTORY_DEFAULT_RETENTION_MS,
    );
}

fn record_pre_buy_collapse_sample_with_retention(
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    sample: PreBuyCollapseSample,
    retention_ms: i64,
) {
    if !sample.live_gap.is_finite()
        || !sample.effective_fill.is_finite()
        || !sample.best_ask.is_finite()
    {
        return;
    }
    let retention_ms = retention_ms
        .clamp(1_000, 120_000)
        .max(PRE_BUY_COLLAPSE_HISTORY_DEFAULT_RETENTION_MS);
    let key = pre_buy_collapse_guard_key(market_slug, token_id, outcome_label);
    let mut history = PRE_BUY_COLLAPSE_HISTORY
        .lock()
        .expect("pre-buy collapse history");
    let bucket = history.entry(key).or_default();
    bucket.push_back(sample);
    while bucket
        .front()
        .is_some_and(|oldest| sample.ts_ms - oldest.ts_ms > retention_ms)
    {
        bucket.pop_front();
    }
    if history.len() > 512 {
        let cutoff_ms = sample.ts_ms - retention_ms.max(20_000);
        history.retain(|_, samples| samples.back().is_some_and(|last| last.ts_ms >= cutoff_ms));
    }
}

fn pre_buy_collapse_sample_at_or_before(
    samples: &VecDeque<PreBuyCollapseSample>,
    ts_ms: i64,
) -> Option<PreBuyCollapseSample> {
    samples
        .iter()
        .rev()
        .find(|sample| sample.ts_ms <= ts_ms)
        .copied()
}

fn pre_buy_collapse_guard_metrics(
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    now_ms: i64,
    effective_fill: f64,
    live_gap: f64,
) -> PreBuyCollapseMetrics {
    let key = pre_buy_collapse_guard_key(market_slug, token_id, outcome_label);
    let samples = PRE_BUY_COLLAPSE_HISTORY
        .lock()
        .expect("pre-buy collapse history")
        .get(&key)
        .cloned()
        .unwrap_or_default();
    let sample_count = samples.len();
    let history_age_ms = samples
        .front()
        .and_then(|first| samples.back().map(|last| last.ts_ms - first.ts_ms))
        .unwrap_or(0)
        .max(0);
    let first_sample_ts_ms = samples.front().map(|sample| sample.ts_ms);
    let last_sample_ts_ms = samples.back().map(|sample| sample.ts_ms);
    let oldest_sample_age_ms = first_sample_ts_ms.map(|ts_ms| (now_ms - ts_ms).max(0));
    let newest_sample_age_ms = last_sample_ts_ms.map(|ts_ms| (now_ms - ts_ms).max(0));
    let largest_sample_gap_ms = samples
        .iter()
        .zip(samples.iter().skip(1))
        .map(|(prev, next)| (next.ts_ms - prev.ts_ms).max(0))
        .max();
    let history_store_uptime_ms = (now_ms - *PRE_BUY_COLLAPSE_HISTORY_STORE_STARTED_AT_MS).max(0);
    let gap_1s_ago = pre_buy_collapse_sample_at_or_before(&samples, now_ms - 1_000)
        .map(|sample| sample.live_gap);
    let gap_3s_ago = pre_buy_collapse_sample_at_or_before(&samples, now_ms - 3_000)
        .map(|sample| sample.live_gap);
    let gap_5s_ago = pre_buy_collapse_sample_at_or_before(&samples, now_ms - 5_000)
        .map(|sample| sample.live_gap);
    let local_price_high_10s = samples
        .iter()
        .filter(|sample| now_ms - sample.ts_ms <= 10_000)
        .map(|sample| sample.effective_fill)
        .filter(|price| price.is_finite())
        .max_by(f64::total_cmp);
    let price_retrace_cent = local_price_high_10s
        .map(|high| ((high - effective_fill).max(0.0)) * 100.0)
        .filter(|value| value.is_finite());
    PreBuyCollapseMetrics {
        history_age_ms,
        sample_count,
        first_sample_ts_ms,
        last_sample_ts_ms,
        oldest_sample_age_ms,
        newest_sample_age_ms,
        largest_sample_gap_ms,
        history_store_uptime_ms,
        first_sample_source: samples.front().map(|sample| sample.sample_source),
        last_sample_source: samples.back().map(|sample| sample.sample_source),
        gap_1s_ago,
        gap_3s_ago,
        gap_5s_ago,
        gap_1s_available: gap_1s_ago.is_some(),
        gap_3s_available: gap_3s_ago.is_some(),
        gap_5s_available: gap_5s_ago.is_some(),
        gap_drop_3s_usd: gap_3s_ago.map(|gap| (gap - live_gap).max(0.0)),
        gap_drop_5s_usd: gap_5s_ago.map(|gap| (gap - live_gap).max(0.0)),
        gap_slope_1s_usd_per_sec: gap_1s_ago.map(|gap| live_gap - gap),
        gap_slope_3s_usd_per_sec: gap_3s_ago.map(|gap| (live_gap - gap) / 3.0),
        local_price_high_10s,
        price_retrace_cent,
    }
}

fn pre_buy_collapse_metrics_payload(metrics: &PreBuyCollapseMetrics) -> Value {
    json!({
        "collapse_history_age_ms": metrics.history_age_ms,
        "history_age_ms": metrics.history_age_ms,
        "sample_count": metrics.sample_count,
        "first_sample_ts_ms": metrics.first_sample_ts_ms,
        "last_sample_ts_ms": metrics.last_sample_ts_ms,
        "oldest_sample_age_ms": metrics.oldest_sample_age_ms,
        "newest_sample_age_ms": metrics.newest_sample_age_ms,
        "largest_sample_gap_ms": metrics.largest_sample_gap_ms,
        "history_store_uptime_ms": metrics.history_store_uptime_ms,
        "first_sample_source": metrics.first_sample_source,
        "last_sample_source": metrics.last_sample_source,
        "gap_1s_available": metrics.gap_1s_available,
        "gap_3s_available": metrics.gap_3s_available,
        "gap_5s_available": metrics.gap_5s_available,
        "gap_1s_ago": metrics.gap_1s_ago,
        "gap_3s_ago": metrics.gap_3s_ago,
        "gap_5s_ago": metrics.gap_5s_ago,
        "gap_drop_3s_usd": metrics.gap_drop_3s_usd,
        "gap_drop_5s_usd": metrics.gap_drop_5s_usd,
        "gap_slope_1s_usd_per_sec": metrics.gap_slope_1s_usd_per_sec,
        "gap_slope_3s_usd_per_sec": metrics.gap_slope_3s_usd_per_sec,
        "local_price_high_10s": metrics.local_price_high_10s,
        "price_retrace_cent": metrics.price_retrace_cent,
    })
}

fn build_pre_buy_collapse_guard_decision(
    input: &PreBuyCollapseGuardInput<'_>,
    config: &PreBuyCollapseGuardConfig,
    metrics: PreBuyCollapseMetrics,
    passed: bool,
    reason_code: &'static str,
) -> PreBuyCollapseGuardDecision {
    let history_context =
        pre_buy_history_context(input.market_slug, input.token_id, input.outcome_label);
    let missing_reasons =
        pre_buy_collapse_missing_reasons(input, config, &metrics, history_context.as_ref());
    let missing_reason = missing_reasons.first().copied();
    let missing_reason_detail = pre_buy_collapse_missing_reason_detail(
        missing_reason,
        input,
        &metrics,
        history_context.as_ref(),
    );
    let clear_kind = pre_buy_collapse_clear_kind(reason_code, &metrics);
    let trigger_condition = input.trigger_condition.map(str::to_string).or_else(|| {
        history_context
            .as_ref()
            .and_then(|ctx| ctx.trigger_condition.clone())
    });
    let trigger_price = input
        .trigger_price
        .or_else(|| history_context.as_ref().and_then(|ctx| ctx.trigger_price));
    let triggered_price = input
        .triggered_price
        .or_else(|| history_context.as_ref().and_then(|ctx| ctx.triggered_price));
    let trigger_source = input.trigger_source.map(str::to_string).or_else(|| {
        history_context
            .as_ref()
            .and_then(|ctx| ctx.trigger_source.clone())
    });
    let mut payload = json!({
        "decision": if passed { "pass" } else { "block_retry" },
        "reason": reason_code,
        "reason_code": reason_code,
        "market_slug": input.market_slug,
        "token_id": input.token_id,
        "outcome_label": input.outcome_label,
        "remaining_sec": input.remaining_sec,
        "effective_fill": input.effective_fill,
        "best_ask": input.best_ask,
        "live_gap": input.live_gap,
        "required_gap": input.required_gap,
        "bounce_buffer_usd": config.bounce_buffer_usd,
        "retry_at_ms": input.retry_ms,
        "clear_kind": clear_kind,
        "missing_reason": missing_reason,
        "missing_reasons": missing_reasons,
        "missing_reason_detail": missing_reason_detail,
        "history": {
            "age_ms": metrics.history_age_ms,
            "min_required_ms": config.history_min_age_ms,
            "sample_count": metrics.sample_count,
            "oldest_sample_age_ms": metrics.oldest_sample_age_ms,
            "newest_sample_age_ms": metrics.newest_sample_age_ms,
            "largest_sample_gap_ms": metrics.largest_sample_gap_ms,
            "first_sample_ts_ms": metrics.first_sample_ts_ms,
            "last_sample_ts_ms": metrics.last_sample_ts_ms,
            "first_sample_source": metrics.first_sample_source,
            "last_sample_source": metrics.last_sample_source,
            "history_store_uptime_ms": metrics.history_store_uptime_ms,
            "gap_1s_available": metrics.gap_1s_available,
            "gap_3s_available": metrics.gap_3s_available,
            "gap_5s_available": metrics.gap_5s_available,
            "missing_reason": missing_reason,
            "missing_reasons": missing_reasons,
            "missing_reason_detail": missing_reason_detail,
        },
        "market_timing": {
            "elapsed_sec": input.elapsed_sec,
            "remaining_sec": input.remaining_sec,
            "window_start_sec": input.window_start_sec,
            "no_new_entry_under_sec": input.no_new_entry_under_sec,
        },
        "trigger_context": {
            "trigger_condition": trigger_condition,
            "trigger_price": trigger_price,
            "triggered_price": triggered_price,
            "trigger_source": trigger_source,
            "trigger_node_key": history_context.as_ref().and_then(|ctx| ctx.trigger_node_key.clone()),
            "prewarm_enabled": history_context.as_ref().map(|ctx| ctx.prewarm_enabled),
            "prewarm_start_elapsed_sec": history_context.as_ref().and_then(|ctx| ctx.prewarm_start_elapsed_sec),
            "trigger_window_start_sec": history_context.as_ref().and_then(|ctx| ctx.trigger_window_start_sec),
            "action_window_start_sec": history_context.as_ref().and_then(|ctx| ctx.action_window_start_sec),
            "action_window_end_sec": history_context.as_ref().and_then(|ctx| ctx.action_window_end_sec),
            "sample_ms": history_context.as_ref().map(|ctx| ctx.sample_ms),
            "retention_ms": history_context.as_ref().map(|ctx| ctx.retention_ms),
        },
    });
    if let Some(obj) = payload.as_object_mut() {
        if let Some(metrics_obj) = pre_buy_collapse_metrics_payload(&metrics).as_object() {
            for (key, value) in metrics_obj {
                obj.insert(key.clone(), value.clone());
            }
        }
    }
    PreBuyCollapseGuardDecision {
        passed,
        reason_code,
        payload,
    }
}

fn evaluate_pre_buy_collapse_guard(
    input: &PreBuyCollapseGuardInput<'_>,
    config: &PreBuyCollapseGuardConfig,
    now_ms: i64,
) -> PreBuyCollapseGuardDecision {
    let metrics = pre_buy_collapse_guard_metrics(
        input.market_slug,
        input.token_id,
        input.outcome_label,
        now_ms,
        input.effective_fill,
        input.live_gap,
    );
    let late = input.remaining_sec <= config.late_remaining_sec;
    if late
        && input.effective_fill >= config.high_price
        && metrics.history_age_ms < config.history_min_age_ms
    {
        return build_pre_buy_collapse_guard_decision(
            input,
            config,
            metrics,
            false,
            "insufficient_collapse_history",
        );
    }
    if late
        && input.effective_fill >= config.very_high_price
        && metrics
            .gap_slope_1s_usd_per_sec
            .is_some_and(|slope| slope < 0.0)
    {
        return build_pre_buy_collapse_guard_decision(
            input,
            config,
            metrics,
            false,
            "very_high_price_negative_slope",
        );
    }
    if late
        && input.effective_fill >= config.high_price
        && metrics
            .gap_drop_3s_usd
            .is_some_and(|drop| drop >= config.high_price_gap_drop_3s_usd)
    {
        return build_pre_buy_collapse_guard_decision(
            input,
            config,
            metrics,
            false,
            "late_high_price_gap_collapsing",
        );
    }
    if (0.80..config.high_price).contains(&input.effective_fill)
        && (metrics
            .gap_drop_3s_usd
            .is_some_and(|drop| drop >= config.mid_price_gap_drop_3s_usd)
            || metrics
                .gap_drop_5s_usd
                .is_some_and(|drop| drop >= config.mid_price_gap_drop_5s_usd))
    {
        return build_pre_buy_collapse_guard_decision(
            input,
            config,
            metrics,
            false,
            "no_bounce_confirmation",
        );
    }
    let bounce_confirmed = metrics
        .gap_slope_1s_usd_per_sec
        .is_some_and(|slope| slope >= 0.0)
        && metrics
            .gap_slope_3s_usd_per_sec
            .is_some_and(|slope| slope >= 0.0)
        && input.live_gap >= input.required_gap + config.bounce_buffer_usd;
    build_pre_buy_collapse_guard_decision(
        input,
        config,
        metrics,
        true,
        if bounce_confirmed {
            "retrace_stabilized"
        } else {
            "collapse_guard_clear"
        },
    )
}

fn metadata_bool(metadata: &Value, path: &str) -> bool {
    metadata
        .pointer(path)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn metadata_i64(metadata: &Value, path: &str, default: i64) -> i64 {
    metadata
        .pointer(path)
        .and_then(value_as_i64)
        .unwrap_or(default)
}

fn metadata_f64(metadata: &Value, path: &str, default: f64) -> f64 {
    metadata
        .pointer(path)
        .and_then(value_as_f64)
        .unwrap_or(default)
}

fn live_gap_collector_config_from_metadata(
    metadata: &Value,
) -> ActionPlaceOrderLiveGapCollectorConfig {
    let pre_buy_collapse_guard = pre_buy_collapse_guard_config_from_metadata(metadata);
    let stop_loss = metadata.get("live_gap_stop_loss");
    ActionPlaceOrderLiveGapCollectorConfig {
        window_start_sec: metadata_i64(metadata, "/resolved_guard_config/windowStartSec", 220)
            .clamp(0, 900),
        window_end_sec: metadata_i64(metadata, "/resolved_guard_config/windowEndSec", 285)
            .clamp(0, 900),
        retry_ms: metadata_i64(metadata, "/resolved_guard_config/retryMs", 150).clamp(50, 5_000),
        hard_max_price: (metadata_f64(metadata, "/resolved_guard_config/hardMaxPriceCent", 93.0)
            / 100.0)
            .clamp(0.01, 0.99),
        binance_max_stale_ms: metadata_i64(
            metadata,
            "/resolved_guard_config/binanceMaxStaleMs",
            1_500,
        )
        .clamp(100, 30_000),
        low_clean_gap_usd: metadata_f64(metadata, "/resolved_guard_config/lowCleanGapUsd", 22.0),
        normal_gap_usd: metadata_f64(metadata, "/resolved_guard_config/normalGapUsd", 32.0),
        high_gap_usd: metadata_f64(metadata, "/resolved_guard_config/highGapUsd", 48.0),
        high_chop_gap_usd: metadata_f64(metadata, "/resolved_guard_config/highChopGapUsd", 55.0),
        latency_buffer_usd: metadata_f64(metadata, "/resolved_guard_config/latencyBufferUsd", 2.0)
            .max(0.0),
        strong_only_under_sec: metadata_i64(
            metadata,
            "/resolved_guard_config/strongOnlyUnderSec",
            20,
        )
        .clamp(0, 300),
        no_new_entry_under_sec: metadata_i64(
            metadata,
            "/resolved_guard_config/noNewEntryUnderSec",
            15,
        )
        .clamp(0, 300),
        strong_signal_extra_gap_usd: metadata_f64(
            metadata,
            "/resolved_guard_config/strongSignalExtraGapUsd",
            8.0,
        )
        .max(0.0),
        pre_buy_collapse_guard,
        no_reversal_entry_guard: no_reversal_entry_guard_config_from_metadata(metadata),
        notify_pre_buy_collapse_guard_decision: metadata
            .pointer("/resolved_guard_config/notifyOnPreBuyCollapseGuardDecision")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        pre_buy_collapse_guard_notification_mode: metadata
            .pointer("/resolved_guard_config/preBuyCollapseGuardNotificationMode")
            .and_then(Value::as_str)
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "smart".to_string()),
        live_gap_history_prewarm_enabled: metadata
            .pointer("/resolved_guard_config/liveGapHistoryPrewarmEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        live_gap_history_prewarm_sec: metadata_i64(
            metadata,
            "/resolved_guard_config/liveGapHistoryPrewarmSec",
            20,
        )
        .clamp(0, 120),
        live_gap_history_prewarm_start_mode: metadata
            .pointer("/resolved_guard_config/liveGapHistoryPrewarmStartMode")
            .and_then(Value::as_str)
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "before_trigger_window".to_string()),
        live_gap_history_prewarm_sides: metadata
            .pointer("/resolved_guard_config/liveGapHistoryPrewarmSides")
            .and_then(Value::as_str)
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "both".to_string()),
        live_gap_history_sample_ms: metadata_i64(
            metadata,
            "/resolved_guard_config/liveGapHistorySampleMs",
            250,
        )
        .clamp(50, 5_000),
        live_gap_history_retention_ms: metadata_i64(
            metadata,
            "/resolved_guard_config/liveGapHistoryRetentionMs",
            PRE_BUY_COLLAPSE_HISTORY_DEFAULT_RETENTION_MS,
        )
        .clamp(PRE_BUY_COLLAPSE_HISTORY_DEFAULT_RETENTION_MS, 120_000),
        notify_on_pre_buy_history_warning: metadata
            .pointer("/resolved_guard_config/notifyOnPreBuyHistoryWarning")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        pre_buy_history_warning_mode: metadata
            .pointer("/resolved_guard_config/preBuyHistoryWarningMode")
            .and_then(Value::as_str)
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "smart".to_string()),
        ptb_telemetry_enabled: true,
        notify_on_decision: false,
        live_gap_stop_loss_enabled: stop_loss
            .and_then(|value| value.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(true),
        live_gap_stop_loss_entry_gap_ratio: stop_loss
            .and_then(|value| value.get("entry_gap_ratio"))
            .and_then(value_as_f64)
            .unwrap_or(0.33)
            .clamp(0.0, 1.0),
        live_gap_stop_loss_gap_usd: stop_loss
            .and_then(|value| value.get("explicit_gap_usd"))
            .and_then(value_as_f64)
            .filter(|value| value.is_finite() && *value >= 0.0),
        live_gap_stop_loss_min_remaining_sec: stop_loss
            .and_then(|value| value.get("min_remaining_sec"))
            .and_then(Value::as_i64)
            .unwrap_or(15)
            .clamp(0, 300),
    }
}

fn live_gap_collector_submit_revalidation_block(
    reason_code: &'static str,
    terminal: bool,
    mut payload: Value,
) -> LiveGapSubmitRevalidationDecision {
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("reason_code".to_string(), json!(reason_code));
        obj.insert(
            "decision".to_string(),
            json!(if terminal {
                "block_terminal"
            } else {
                "block_retry"
            }),
        );
    }
    LiveGapSubmitRevalidationDecision {
        passed: false,
        terminal,
        reason_code,
        payload,
    }
}

fn live_gap_submit_revalidation_intended_qty(
    order: &TradeBuilderOrder,
    best_ask: f64,
) -> Option<f64> {
    if normalize_trade_builder_size_basis(&order.size_basis) == TRADE_BUILDER_SIZE_BASIS_SHARES {
        return order
            .remaining_qty
            .or(order.target_qty)
            .filter(|qty| qty.is_finite() && *qty > 0.0);
    }
    let notional = order.remaining_size.unwrap_or(order.size_usdc).max(0.0);
    (best_ask.is_finite() && best_ask > 0.0)
        .then_some(notional / best_ask)
        .filter(|qty| qty.is_finite() && *qty > 0.0)
}

async fn evaluate_live_gap_submit_revalidation(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
    metadata: &Value,
    config: &ActionPlaceOrderLiveGapCollectorConfig,
) -> LiveGapSubmitRevalidationDecision {
    let now = Utc::now();
    let now_ms = now.timestamp_millis();
    let Some(scope) = find_updown_scope_by_slug(&order.market_slug) else {
        return live_gap_collector_submit_revalidation_block("unsupported_market", true, json!({}));
    };
    let Some(direction) = live_gap_collector_direction(&order.outcome_label) else {
        return live_gap_collector_submit_revalidation_block(
            "unsupported_outcome_label",
            true,
            json!({}),
        );
    };
    let Some(window_start) = MarketCycleId(order.market_slug.clone()).start_time() else {
        return live_gap_collector_submit_revalidation_block(
            "market_start_unavailable",
            true,
            json!({}),
        );
    };
    let elapsed_sec = live_gap_collector_elapsed_sec(&order.market_slug, now).unwrap_or_default();
    let remaining_sec =
        live_gap_collector_remaining_sec(&order.market_slug, now).unwrap_or_default();
    if remaining_sec < config.no_new_entry_under_sec {
        return live_gap_collector_submit_revalidation_block(
            "too_late_for_new_entry",
            true,
            json!({ "elapsed_sec": elapsed_sec, "remaining_sec": remaining_sec }),
        );
    }
    let open_tick = match trade_flow::guards::chainlink_price::get_chainlink_price_start_tick(
        scope.asset,
        window_start.timestamp_millis(),
    ) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            return live_gap_collector_submit_revalidation_block(
                "open_price_unavailable",
                false,
                json!({ "error": err.to_string() }),
            );
        }
    };
    let binance =
        match trade_flow::guards::binance_price::get_binance_price_snapshot(scope.asset, now_ms) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                return live_gap_collector_submit_revalidation_block(
                    "binance_price_unavailable",
                    false,
                    json!({ "error": err.to_string() }),
                );
            }
        };
    let order_book = match client.order_book(&order.token_id).await {
        Ok(Some(book)) => book,
        Ok(None) => {
            return live_gap_collector_submit_revalidation_block(
                "orderbook_unavailable",
                false,
                json!({}),
            );
        }
        Err(err) => {
            return live_gap_collector_submit_revalidation_block(
                "orderbook_unavailable",
                false,
                json!({ "error": err.to_string() }),
            );
        }
    };
    let Some(best_ask) = live_gap_collector_best_ask(&order_book) else {
        return live_gap_collector_submit_revalidation_block(
            "best_ask_unavailable",
            false,
            json!({}),
        );
    };
    let intended_qty = live_gap_submit_revalidation_intended_qty(order, best_ask);
    let depth = trade_flow::guards::price_to_beat::evaluate_price_to_beat_iv_depth(
        Some(&order_book),
        best_ask,
        intended_qty,
        (config.hard_max_price - best_ask).max(0.0),
        true,
    );
    let effective_fill = depth.estimated_avg_fill.or(Some(best_ask));
    let volume_ratio =
        live_gap_collector_volume_ratio(repo, &order.market_slug, scope.asset, now).await;
    let volatility_usd = live_gap_collector_volatility_usd(scope.asset, now_ms);
    let regime = live_gap_collector_regime(
        binance.staleness_ms,
        config.binance_max_stale_ms,
        volume_ratio,
        volatility_usd,
    );
    let live_gap = live_gap_collector_directional_gap(direction, open_tick.price, binance.price);
    let required_gap =
        live_gap_collector_required_gap(config, regime, remaining_sec, effective_fill);
    if let Some(fill_price) = effective_fill {
        record_pre_buy_collapse_sample_with_retention(
            &order.market_slug,
            &order.token_id,
            &order.outcome_label,
            PreBuyCollapseSample {
                ts_ms: now_ms,
                live_gap,
                effective_fill: fill_price,
                best_ask,
                sample_source: "submit_revalidation",
            },
            config.live_gap_history_retention_ms,
        );
    }
    let sl_threshold = config
        .live_gap_stop_loss_gap_usd
        .unwrap_or(live_gap.max(0.0) * config.live_gap_stop_loss_entry_gap_ratio)
        .max(0.0);
    let mut payload_obj = serde_json::Map::new();
    payload_obj.insert(
        "mode".to_string(),
        json!(ACTION_PLACE_ORDER_MODE_LIVE_GAP_COLLECTOR_V1),
    );
    payload_obj.insert("submit_revalidation".to_string(), json!(true));
    payload_obj.insert("decision".to_string(), json!("pass"));
    payload_obj.insert("market_slug".to_string(), json!(order.market_slug));
    payload_obj.insert("token_id".to_string(), json!(order.token_id));
    payload_obj.insert("outcome_label".to_string(), json!(order.outcome_label));
    payload_obj.insert("asset".to_string(), json!(scope.asset));
    payload_obj.insert("timeframe".to_string(), json!(scope.timeframe));
    payload_obj.insert("direction".to_string(), json!(direction));
    payload_obj.insert("open_price".to_string(), json!(open_tick.price));
    payload_obj.insert(
        "open_price_ts_ms".to_string(),
        json!(open_tick.timestamp_ms),
    );
    payload_obj.insert(
        "open_price_source".to_string(),
        json!("chainlink_rtds_start_tick"),
    );
    payload_obj.insert("current_price".to_string(), json!(binance.price));
    payload_obj.insert(
        "current_price_ts_ms".to_string(),
        json!(binance.timestamp_ms),
    );
    payload_obj.insert("binance_price_ts".to_string(), json!(binance.timestamp_ms));
    payload_obj.insert(
        "current_price_source".to_string(),
        json!("binance_live_data_ws"),
    );
    payload_obj.insert(
        "binance_staleness_ms".to_string(),
        json!(binance.staleness_ms),
    );
    payload_obj.insert("elapsed_sec".to_string(), json!(elapsed_sec));
    payload_obj.insert("remaining_sec".to_string(), json!(remaining_sec));
    payload_obj.insert("live_gap_usd".to_string(), json!(live_gap));
    payload_obj.insert("required_gap_usd".to_string(), json!(required_gap));
    payload_obj.insert(
        "regime".to_string(),
        json!(live_gap_collector_regime_label(regime)),
    );
    payload_obj.insert("volume_ratio".to_string(), json!(volume_ratio));
    payload_obj.insert("volatility_usd_15s".to_string(), json!(volatility_usd));
    payload_obj.insert("best_ask".to_string(), json!(best_ask));
    payload_obj.insert("hard_max_price".to_string(), json!(config.hard_max_price));
    payload_obj.insert("effective_fill_price".to_string(), json!(effective_fill));
    payload_obj.insert("candidate_created_at_ms".to_string(), json!(now_ms));
    payload_obj.insert("candidate_live_gap".to_string(), json!(live_gap));
    payload_obj.insert("candidate_required_gap".to_string(), json!(required_gap));
    payload_obj.insert(
        "candidate_effective_fill".to_string(),
        json!(effective_fill),
    );
    payload_obj.insert("candidate_remaining_sec".to_string(), json!(remaining_sec));
    payload_obj.insert(
        "candidate_regime".to_string(),
        json!(live_gap_collector_regime_label(regime)),
    );
    payload_obj.insert(
        "candidate_guard_reason".to_string(),
        json!("submit_revalidation_pending_guard"),
    );
    payload_obj.insert(
        "candidate_price_floor_seen_below".to_string(),
        json!(metadata_bool(metadata, "/candidate_price_floor_seen_below")),
    );
    payload_obj.insert(
        "resolved_guard_config".to_string(),
        live_gap_collector_resolved_config_snapshot(config),
    );
    payload_obj.insert(
        "pre_buy_collapse_guard_config".to_string(),
        pre_buy_collapse_guard_config_snapshot(&config.pre_buy_collapse_guard),
    );
    depth.append_to_json(&mut payload_obj);
    payload_obj.insert(
        "live_gap_stop_loss".to_string(),
        json!({
            "enabled": config.live_gap_stop_loss_enabled,
            "threshold_gap_usd": sl_threshold,
            "entry_gap_ratio": config.live_gap_stop_loss_entry_gap_ratio,
            "explicit_gap_usd": config.live_gap_stop_loss_gap_usd,
            "min_remaining_sec": config.live_gap_stop_loss_min_remaining_sec
        }),
    );
    let payload = Value::Object(payload_obj);
    if regime == LiveGapCollectorRegime::Red {
        return live_gap_collector_submit_revalidation_block("regime_red_or_stale", false, payload);
    }
    if depth.result != "pass" {
        return live_gap_collector_submit_revalidation_block(
            depth.block_reason.unwrap_or("depth_guard_blocked"),
            false,
            payload,
        );
    }
    if effective_fill.is_none_or(|price| price > config.hard_max_price) {
        return live_gap_collector_submit_revalidation_block(
            "effective_fill_above_hard_max",
            false,
            payload,
        );
    }
    if live_gap < required_gap {
        return live_gap_collector_submit_revalidation_block(
            "live_gap_below_required",
            false,
            payload,
        );
    }
    let guard_input = PreBuyCollapseGuardInput {
        market_slug: &order.market_slug,
        token_id: &order.token_id,
        outcome_label: &order.outcome_label,
        remaining_sec,
        effective_fill: effective_fill.unwrap_or(best_ask),
        best_ask,
        live_gap,
        required_gap,
        retry_ms: config.retry_ms,
        elapsed_sec,
        window_start_sec: config.window_start_sec,
        no_new_entry_under_sec: config.no_new_entry_under_sec,
        trigger_condition: None,
        trigger_price: None,
        triggered_price: Some(effective_fill.unwrap_or(best_ask)),
        trigger_source: Some("submit_revalidation"),
    };
    let guard_decision =
        evaluate_pre_buy_collapse_guard(&guard_input, &config.pre_buy_collapse_guard, now_ms);
    let mut payload = payload;
    if let Some(obj) = payload.as_object_mut() {
        obj.insert(
            "pre_buy_collapse_guard".to_string(),
            guard_decision.payload.clone(),
        );
        obj.insert(
            "candidate_guard_reason".to_string(),
            json!(guard_decision.reason_code),
        );
    }
    if !guard_decision.passed {
        return live_gap_collector_submit_revalidation_block(
            guard_decision.reason_code,
            false,
            payload,
        );
    }
    let mut final_reason_code = guard_decision.reason_code;
    if config.no_reversal_entry_guard.enabled {
        let no_reversal_input = NoReversalEntryGuardInput {
            market_slug: &order.market_slug,
            asset: scope.asset,
            direction,
            remaining_sec,
            effective_fill: effective_fill.unwrap_or(best_ask),
            current_live_gap: live_gap,
            regime: live_gap_collector_regime_label(regime),
            slope_bucket: no_reversal_slope_bucket_from_pre_buy_payload(&guard_decision.payload),
        };
        let no_reversal_decision = evaluate_no_reversal_entry_guard(
            repo,
            &config.no_reversal_entry_guard,
            &no_reversal_input,
        )
        .await;
        if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                "no_reversal_entry_guard".to_string(),
                no_reversal_decision.payload.clone(),
            );
            obj.insert(
                "candidate_guard_reason".to_string(),
                json!(no_reversal_decision.reason_code),
            );
        }
        final_reason_code = no_reversal_decision.reason_code;
        if !no_reversal_decision.passed {
            return live_gap_collector_submit_revalidation_block(
                no_reversal_decision.reason_code,
                false,
                payload,
            );
        }
    }
    LiveGapSubmitRevalidationDecision {
        passed: true,
        terminal: false,
        reason_code: final_reason_code,
        payload,
    }
}

fn live_gap_submit_revalidation_floor_invalidated(
    order: &TradeBuilderOrder,
    metadata: &Value,
) -> bool {
    metadata_bool(metadata, "/candidate_price_floor_seen_below")
        || (order.status == TRADE_BUILDER_GUARD_BLOCKED_STATUS
            && matches!(
                order.last_error.as_deref(),
                Some("below_best_ask_floor" | "best_ask_unavailable")
            ))
}

#[allow(clippy::too_many_arguments)]
async fn maybe_block_live_gap_collector_submit_revalidation(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
    remaining_size: Option<f64>,
    remaining_qty: Option<f64>,
) -> Result<bool> {
    if order.side != "buy" {
        return Ok(false);
    }
    let Some(metadata) = repo
        .load_trade_builder_order_live_gap_metadata(order.id)
        .await?
    else {
        return Ok(false);
    };
    if metadata
        .get("mode")
        .and_then(Value::as_str)
        .is_none_or(|mode| mode != ACTION_PLACE_ORDER_MODE_LIVE_GAP_COLLECTOR_V1)
    {
        return Ok(false);
    }
    let config = live_gap_collector_config_from_metadata(&metadata);
    let now_ms = Utc::now().timestamp_millis();
    let candidate_created_at_ms = metadata
        .get("candidate_created_at_ms")
        .and_then(value_as_i64)
        .unwrap_or_else(|| order.created_at.timestamp_millis());
    let candidate_age_ms = (now_ms - candidate_created_at_ms).max(0);
    let candidate_stale = candidate_age_ms > config.pre_buy_collapse_guard.candidate_max_age_ms;
    let floor_invalidated = live_gap_submit_revalidation_floor_invalidated(order, &metadata);
    let mut decision =
        evaluate_live_gap_submit_revalidation(repo, client, order, &metadata, &config).await;
    if let Some(obj) = decision.payload.as_object_mut() {
        obj.insert("builder_order_id".to_string(), json!(order.id));
        obj.insert("status_before".to_string(), json!(order.status));
        obj.insert("last_error_before".to_string(), json!(order.last_error));
    }
    annotate_live_gap_submit_revalidation_payload(
        &mut decision,
        &metadata,
        candidate_age_ms,
        config.pre_buy_collapse_guard.candidate_max_age_ms,
        candidate_stale,
        floor_invalidated,
        now_ms,
    );
    let (notification_type, notification_message) =
        live_gap_submit_revalidation_notification_target(&metadata, order, &mut decision.payload);
    if decision.passed {
        repo.set_trade_builder_order_live_gap_metadata(order.id, Some(&decision.payload))
            .await?;
        repo.append_trade_builder_order_event(
            order.id,
            "live_gap_submit_revalidation_passed",
            &decision.payload,
        )
        .await?;
        if let (Some(notification_type), Some(notification_message)) =
            (notification_type, notification_message)
        {
            send_trade_builder_notification(repo, order, notification_type, &notification_message)
                .await;
        }
        return Ok(false);
    }
    let event_type = if decision.terminal {
        "live_gap_submit_revalidation_terminal_blocked"
    } else {
        "live_gap_submit_revalidation_blocked"
    };
    if decision.terminal {
        repo.set_trade_builder_order_status(order.id, "canceled", Some(decision.reason_code))
            .await?;
        repo.append_trade_builder_order_event(order.id, event_type, &decision.payload)
            .await?;
        if let (Some(notification_type), Some(notification_message)) =
            (notification_type, notification_message)
        {
            send_trade_builder_notification(repo, order, notification_type, &notification_message)
                .await;
        }
    } else {
        let candidate_reason = build_guard_notification_reason("live_gap", decision.reason_code);
        transition_trade_builder_order_to_guard_waiting(
            repo,
            order,
            decision.reason_code,
            event_type,
            &decision.payload,
            remaining_size,
            remaining_qty,
            Some(candidate_reason.as_str()),
            notification_type,
            notification_message,
        )
        .await?;
        sync_guarded_buy_order_cache_for_order(repo, order.id).await;
    }
    Ok(true)
}

#[cfg(test)]
mod pre_buy_collapse_guard_tests {
    use super::*;

    static PRE_BUY_TEST_LOCK: LazyLock<StdMutex<()>> = LazyLock::new(|| StdMutex::new(()));

    fn cfg() -> PreBuyCollapseGuardConfig {
        PreBuyCollapseGuardConfig {
            candidate_max_age_ms: 500,
            late_remaining_sec: 60,
            history_min_age_ms: 750,
            high_price: 0.85,
            very_high_price: 0.89,
            high_price_gap_drop_3s_usd: 8.0,
            mid_price_gap_drop_3s_usd: 12.0,
            mid_price_gap_drop_5s_usd: 16.0,
            bounce_buffer_usd: 3.0,
        }
    }

    fn input(price: f64, gap: f64, remaining_sec: i64) -> PreBuyCollapseGuardInput<'static> {
        PreBuyCollapseGuardInput {
            market_slug: "btc-updown-5m-1777900500",
            token_id: "tok-up",
            outcome_label: "Up",
            remaining_sec,
            effective_fill: price,
            best_ask: price,
            live_gap: gap,
            required_gap: 34.0,
            retry_ms: 150,
            elapsed_sec: 242,
            window_start_sec: 240,
            no_new_entry_under_sec: 15,
            trigger_condition: Some("cross_above"),
            trigger_price: Some(0.80),
            triggered_price: Some(price),
            trigger_source: Some("trigger.market_price"),
        }
    }

    fn reset_history() {
        PRE_BUY_COLLAPSE_HISTORY
            .lock()
            .expect("pre-buy collapse history")
            .clear();
        PRE_BUY_HISTORY_CONTEXTS
            .lock()
            .expect("pre-buy history contexts")
            .clear();
    }

    fn record(ts_ms: i64, price: f64, gap: f64) {
        record_pre_buy_collapse_sample(
            "btc-updown-5m-1777900500",
            "tok-up",
            "Up",
            PreBuyCollapseSample {
                ts_ms,
                live_gap: gap,
                effective_fill: price,
                best_ask: price,
                sample_source: "test",
            },
        );
    }

    fn record_down(ts_ms: i64, price: f64, gap: f64) {
        record_pre_buy_collapse_sample(
            "btc-updown-5m-1777900500",
            "tok-down",
            "Down",
            PreBuyCollapseSample {
                ts_ms,
                live_gap: gap,
                effective_fill: price,
                best_ask: price,
                sample_source: "test",
            },
        );
    }

    fn payload_str_array(payload: &Value, key: &str) -> Vec<String> {
        payload
            .get(key)
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

    #[test]
    fn blocks_expensive_late_entry_without_history() {
        let _guard = PRE_BUY_TEST_LOCK.lock().expect("pre-buy test lock");
        reset_history();
        record(10_000, 0.88, 42.0);
        let decision = evaluate_pre_buy_collapse_guard(&input(0.88, 42.0, 58), &cfg(), 10_100);
        assert!(!decision.passed);
        assert_eq!(decision.reason_code, "insufficient_collapse_history");
        let reasons = payload_str_array(&decision.payload, "missing_reasons");
        assert!(reasons.contains(&"history_not_started_yet".to_string()));
        assert!(reasons.contains(&"cross_above_no_prewarm".to_string()));
    }

    #[test]
    fn blocks_up_falling_knife_on_three_second_gap_drop() {
        let _guard = PRE_BUY_TEST_LOCK.lock().expect("pre-buy test lock");
        reset_history();
        record(7_000, 0.93, 52.0);
        record(10_000, 0.88, 42.0);
        let decision = evaluate_pre_buy_collapse_guard(&input(0.88, 42.0, 58), &cfg(), 10_000);
        assert!(!decision.passed);
        assert_eq!(decision.reason_code, "late_high_price_gap_collapsing");
    }

    #[test]
    fn blocks_cheap_price_when_gap_is_still_collapsing() {
        let _guard = PRE_BUY_TEST_LOCK.lock().expect("pre-buy test lock");
        reset_history();
        record(5_000, 0.93, 47.0);
        record(7_000, 0.84, 39.0);
        record(10_000, 0.82, 31.0);
        let decision = evaluate_pre_buy_collapse_guard(&input(0.82, 31.0, 50), &cfg(), 10_000);
        assert!(!decision.passed);
        assert_eq!(decision.reason_code, "no_bounce_confirmation");
    }

    #[test]
    fn passes_healthy_retrace_after_gap_stabilizes() {
        let _guard = PRE_BUY_TEST_LOCK.lock().expect("pre-buy test lock");
        reset_history();
        record(7_000, 0.93, 39.0);
        record(9_000, 0.85, 39.5);
        record(10_000, 0.84, 40.0);
        let decision = evaluate_pre_buy_collapse_guard(&input(0.84, 40.0, 52), &cfg(), 10_000);
        assert!(decision.passed);
        assert_eq!(decision.reason_code, "retrace_stabilized");
        assert_eq!(
            decision.payload.get("clear_kind").and_then(Value::as_str),
            Some("retrace_stabilized_short_history")
        );
    }

    #[test]
    fn blocks_down_falling_knife_on_directional_gap_drop() {
        let _guard = PRE_BUY_TEST_LOCK.lock().expect("pre-buy test lock");
        reset_history();
        record_down(7_000, 0.93, 52.0);
        record_down(10_000, 0.88, 42.0);
        let mut down_input = input(0.88, 42.0, 58);
        down_input.token_id = "tok-down";
        down_input.outcome_label = "Down";
        let decision = evaluate_pre_buy_collapse_guard(&down_input, &cfg(), 10_000);
        assert!(!decision.passed);
        assert_eq!(decision.reason_code, "late_high_price_gap_collapsing");
    }

    #[test]
    fn passes_healthy_down_retrace_after_directional_gap_stabilizes() {
        let _guard = PRE_BUY_TEST_LOCK.lock().expect("pre-buy test lock");
        reset_history();
        record_down(7_000, 0.93, 39.0);
        record_down(9_000, 0.85, 39.5);
        record_down(10_000, 0.84, 40.0);
        let mut down_input = input(0.84, 40.0, 52);
        down_input.token_id = "tok-down";
        down_input.outcome_label = "Down";
        let decision = evaluate_pre_buy_collapse_guard(&down_input, &cfg(), 10_000);
        assert!(decision.passed);
        assert_eq!(decision.reason_code, "retrace_stabilized");
    }

    #[test]
    fn full_prewarm_history_makes_one_three_five_second_metrics_available() {
        let _guard = PRE_BUY_TEST_LOCK.lock().expect("pre-buy test lock");
        reset_history();
        record(4_000, 0.82, 38.0);
        record(6_000, 0.83, 38.5);
        record(8_000, 0.84, 39.0);
        record(9_000, 0.84, 39.5);
        record(10_000, 0.84, 40.0);
        let metrics = pre_buy_collapse_guard_metrics(
            "btc-updown-5m-1777900500",
            "tok-up",
            "Up",
            10_000,
            0.84,
            40.0,
        );
        assert!(metrics.gap_1s_available);
        assert!(metrics.gap_3s_available);
        assert!(metrics.gap_5s_available);
    }

    #[test]
    fn full_history_clear_kind_is_distinct_from_short_history_clear() {
        let _guard = PRE_BUY_TEST_LOCK.lock().expect("pre-buy test lock");
        reset_history();
        record(4_000, 0.82, 35.0);
        record(7_000, 0.82, 35.0);
        record(9_000, 0.82, 35.0);
        record(10_000, 0.82, 35.0);
        let decision = evaluate_pre_buy_collapse_guard(&input(0.82, 35.0, 52), &cfg(), 10_000);
        assert!(decision.passed);
        assert_eq!(
            decision.payload.get("clear_kind").and_then(Value::as_str),
            Some("full_history_clear")
        );
    }

    #[test]
    fn market_slug_change_does_not_reuse_previous_history() {
        let _guard = PRE_BUY_TEST_LOCK.lock().expect("pre-buy test lock");
        reset_history();
        record(7_000, 0.88, 52.0);
        record(10_000, 0.88, 42.0);
        let mut new_slug_input = input(0.88, 42.0, 58);
        new_slug_input.market_slug = "btc-updown-5m-1777900800";
        let decision = evaluate_pre_buy_collapse_guard(&new_slug_input, &cfg(), 10_000);
        assert!(!decision.passed);
        assert_eq!(decision.reason_code, "insufficient_collapse_history");
        assert_eq!(
            decision
                .payload
                .pointer("/history/sample_count")
                .and_then(Value::as_u64),
            Some(0)
        );
    }
}
