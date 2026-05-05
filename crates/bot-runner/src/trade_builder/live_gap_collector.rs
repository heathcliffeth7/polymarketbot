#[derive(Debug, Clone, PartialEq)]
struct ActionPlaceOrderLiveGapCollectorConfig {
    window_start_sec: i64,
    window_end_sec: i64,
    retry_ms: i64,
    hard_max_price: f64,
    binance_max_stale_ms: i64,
    low_clean_gap_usd: f64,
    normal_gap_usd: f64,
    high_gap_usd: f64,
    high_chop_gap_usd: f64,
    latency_buffer_usd: f64,
    strong_only_under_sec: i64,
    no_new_entry_under_sec: i64,
    strong_signal_extra_gap_usd: f64,
    pre_buy_collapse_guard: PreBuyCollapseGuardConfig,
    no_reversal_entry_guard: NoReversalEntryGuardConfig,
    notify_pre_buy_collapse_guard_decision: bool,
    pre_buy_collapse_guard_notification_mode: String,
    live_gap_history_prewarm_enabled: bool,
    live_gap_history_prewarm_sec: i64,
    live_gap_history_prewarm_start_mode: String,
    live_gap_history_prewarm_sides: String,
    live_gap_history_sample_ms: i64,
    live_gap_history_retention_ms: i64,
    notify_on_pre_buy_history_warning: bool,
    pre_buy_history_warning_mode: String,
    ptb_telemetry_enabled: bool,
    notify_on_decision: bool,
    live_gap_stop_loss_enabled: bool,
    live_gap_stop_loss_entry_gap_ratio: f64,
    live_gap_stop_loss_gap_usd: Option<f64>,
    live_gap_stop_loss_min_remaining_sec: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LiveGapCollectorRegime {
    LowClean,
    Normal,
    High,
    HighChop,
    Red,
}

#[derive(Debug, Clone, PartialEq)]
struct LiveGapCollectorDecision {
    passed: bool,
    terminal: bool,
    reason_code: &'static str,
    payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
struct TradeBuilderLiveGapStopLossEvaluation {
    asset: Option<String>,
    direction: Option<String>,
    threshold_gap_usd: f64,
    reference_price: Option<f64>,
    current_price: Option<f64>,
    current_price_source: &'static str,
    current_price_ts_ms: Option<i64>,
    current_price_staleness_ms: Option<i64>,
    directional_gap: Option<f64>,
    remaining_sec: Option<i64>,
    min_remaining_sec: i64,
    reason_code: &'static str,
    should_trigger: bool,
}

fn live_gap_collector_elapsed_ms(started_at: Instant) -> i64 {
    started_at.elapsed().as_millis().min(i64::MAX as u128) as i64
}

fn live_gap_collector_insert_timings(
    payload: &mut Value,
    eval_started_at: Instant,
    orderbook_ms: Option<i64>,
    volume_ms: Option<i64>,
    no_reversal_ms: Option<i64>,
) {
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("orderbook_ms".to_string(), json!(orderbook_ms));
        obj.insert("volume_ms".to_string(), json!(volume_ms));
        obj.insert("no_reversal_ms".to_string(), json!(no_reversal_ms));
        obj.insert(
            "total_eval_ms".to_string(),
            json!(live_gap_collector_elapsed_ms(eval_started_at)),
        );
    }
}

fn resolve_action_place_order_live_gap_collector_config(
    node: &TradeFlowNode,
    side: &str,
) -> Result<Option<ActionPlaceOrderLiveGapCollectorConfig>> {
    if side != "buy"
        || !action_place_order_uses_live_gap_collector(node)
        || !node_config_bool(node, "liveGapCollectorEnabled").unwrap_or(true)
    {
        return Ok(None);
    }
    let hard_max_price =
        (node_config_f64(node, "liveGapCollectorHardMaxPriceCent").unwrap_or(93.0) / 100.0)
            .clamp(0.01, 0.99);
    let config = ActionPlaceOrderLiveGapCollectorConfig {
        window_start_sec: node_config_i64(node, "liveGapCollectorWindowStartSec")
            .unwrap_or(220)
            .clamp(0, 900),
        window_end_sec: node_config_i64(node, "liveGapCollectorWindowEndSec")
            .unwrap_or(285)
            .clamp(0, 900),
        retry_ms: node_config_i64(node, "liveGapCollectorRetryMs")
            .unwrap_or(150)
            .clamp(50, 5_000),
        hard_max_price,
        binance_max_stale_ms: node_config_i64(node, "liveGapCollectorBinanceMaxStaleMs")
            .unwrap_or(1_500)
            .clamp(100, 30_000),
        low_clean_gap_usd: node_config_f64(node, "liveGapCollectorLowCleanGapUsd").unwrap_or(22.0),
        normal_gap_usd: node_config_f64(node, "liveGapCollectorNormalGapUsd").unwrap_or(32.0),
        high_gap_usd: node_config_f64(node, "liveGapCollectorHighGapUsd").unwrap_or(48.0),
        high_chop_gap_usd: node_config_f64(node, "liveGapCollectorHighChopGapUsd").unwrap_or(55.0),
        latency_buffer_usd: node_config_f64(node, "liveGapCollectorLatencyBufferUsd")
            .unwrap_or(2.0)
            .max(0.0),
        strong_only_under_sec: node_config_i64(node, "liveGapCollectorStrongOnlyUnderSec")
            .unwrap_or(20)
            .clamp(0, 300),
        no_new_entry_under_sec: node_config_i64(node, "liveGapCollectorNoNewEntryUnderSec")
            .unwrap_or(15)
            .clamp(0, 300),
        strong_signal_extra_gap_usd: node_config_f64(
            node,
            "liveGapCollectorStrongSignalExtraGapUsd",
        )
        .unwrap_or(8.0)
        .max(0.0),
        pre_buy_collapse_guard: resolve_pre_buy_collapse_guard_config(node),
        no_reversal_entry_guard: resolve_no_reversal_entry_guard_config(node),
        notify_pre_buy_collapse_guard_decision: node_config_bool(
            node,
            "notifyOnPreBuyCollapseGuardDecision",
        )
        .unwrap_or(true),
        pre_buy_collapse_guard_notification_mode: node_config_string(
            node,
            "preBuyCollapseGuardNotificationMode",
        )
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "smart".to_string()),
        live_gap_history_prewarm_enabled: node_config_bool(node, "liveGapHistoryPrewarmEnabled")
            .unwrap_or(true),
        live_gap_history_prewarm_sec: node_config_i64(node, "liveGapHistoryPrewarmSec")
            .unwrap_or(20)
            .clamp(0, 120),
        live_gap_history_prewarm_start_mode: node_config_string(
            node,
            "liveGapHistoryPrewarmStartMode",
        )
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "before_trigger_window".to_string()),
        live_gap_history_prewarm_sides: node_config_string(node, "liveGapHistoryPrewarmSides")
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "both".to_string()),
        live_gap_history_sample_ms: node_config_i64(node, "liveGapHistorySampleMs")
            .unwrap_or(250)
            .clamp(50, 5_000),
        live_gap_history_retention_ms: node_config_i64(node, "liveGapHistoryRetentionMs")
            .unwrap_or(PRE_BUY_COLLAPSE_HISTORY_DEFAULT_RETENTION_MS)
            .clamp(PRE_BUY_COLLAPSE_HISTORY_DEFAULT_RETENTION_MS, 120_000),
        notify_on_pre_buy_history_warning: node_config_bool(node, "notifyOnPreBuyHistoryWarning")
            .unwrap_or(true),
        pre_buy_history_warning_mode: node_config_string(node, "preBuyHistoryWarningMode")
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "smart".to_string()),
        ptb_telemetry_enabled: node_config_bool(node, "liveGapCollectorPtbTelemetryEnabled")
            .unwrap_or(true),
        notify_on_decision: node_config_bool(node, "notifyOnLiveGapCollectorDecision")
            .unwrap_or(true),
        live_gap_stop_loss_enabled: node_config_bool(node, "liveGapStopLossEnabled")
            .unwrap_or(true),
        live_gap_stop_loss_entry_gap_ratio: node_config_f64(node, "liveGapStopLossEntryGapRatio")
            .unwrap_or(0.33)
            .clamp(0.0, 1.0),
        live_gap_stop_loss_gap_usd: node_config_f64(node, "liveGapStopLossGapUsd")
            .filter(|value| value.is_finite() && *value >= 0.0),
        live_gap_stop_loss_min_remaining_sec: node_config_i64(
            node,
            "liveGapStopLossMinRemainingSec",
        )
        .unwrap_or(15)
        .clamp(0, 300),
    };
    anyhow::ensure!(
        config.window_start_sec < config.window_end_sec,
        "action.place_order live_gap_collector_v1 requires liveGapCollectorWindowStartSec < liveGapCollectorWindowEndSec"
    );
    Ok(Some(config))
}

fn live_gap_collector_effective_max_price(
    max_price: Option<f64>,
    config: Option<&ActionPlaceOrderLiveGapCollectorConfig>,
) -> Option<f64> {
    config
        .map(|cfg| {
            max_price
                .unwrap_or(cfg.hard_max_price)
                .min(cfg.hard_max_price)
        })
        .or(max_price)
}

fn live_gap_collector_direction(outcome_label: &str) -> Option<&'static str> {
    match outcome_label.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "long" | "bull" => Some("up"),
        "no" | "down" | "short" | "bear" => Some("down"),
        _ => None,
    }
}

fn live_gap_collector_directional_gap(direction: &str, open_price: f64, current_price: f64) -> f64 {
    if direction == "down" {
        open_price - current_price
    } else {
        current_price - open_price
    }
}

fn live_gap_collector_remaining_sec(market_slug: &str, now: DateTime<Utc>) -> Option<i64> {
    let scope = find_updown_scope_by_slug(market_slug)?;
    let start = MarketCycleId(market_slug.to_string()).start_time()?;
    let window_sec = updown_scope_window_seconds(scope);
    let elapsed = now
        .signed_duration_since(start)
        .num_seconds()
        .clamp(0, window_sec);
    Some((window_sec - elapsed).max(0))
}

fn live_gap_collector_elapsed_sec(market_slug: &str, now: DateTime<Utc>) -> Option<i64> {
    let scope = find_updown_scope_by_slug(market_slug)?;
    let start = MarketCycleId(market_slug.to_string()).start_time()?;
    let window_sec = updown_scope_window_seconds(scope);
    Some(
        now.signed_duration_since(start)
            .num_seconds()
            .clamp(0, window_sec),
    )
}

fn live_gap_collector_price_adjustment(price: Option<f64>) -> f64 {
    let Some(price) = price.filter(|value| value.is_finite()) else {
        return 0.0;
    };
    if price >= 0.90 {
        4.0
    } else if price < 0.85 {
        -4.0
    } else {
        0.0
    }
}

fn live_gap_collector_required_gap(
    config: &ActionPlaceOrderLiveGapCollectorConfig,
    regime: LiveGapCollectorRegime,
    remaining_sec: i64,
    fill_price: Option<f64>,
) -> f64 {
    let base = match regime {
        LiveGapCollectorRegime::LowClean => config.low_clean_gap_usd,
        LiveGapCollectorRegime::Normal => config.normal_gap_usd,
        LiveGapCollectorRegime::High => config.high_gap_usd,
        LiveGapCollectorRegime::HighChop => config.high_chop_gap_usd,
        LiveGapCollectorRegime::Red => f64::INFINITY,
    };
    let late_penalty = if remaining_sec < config.strong_only_under_sec {
        config.strong_signal_extra_gap_usd
    } else {
        0.0
    };
    (base
        + live_gap_collector_price_adjustment(fill_price)
        + late_penalty
        + config.latency_buffer_usd)
        .max(0.0)
}

fn live_gap_collector_regime_label(regime: LiveGapCollectorRegime) -> &'static str {
    match regime {
        LiveGapCollectorRegime::LowClean => "low_clean",
        LiveGapCollectorRegime::Normal => "normal",
        LiveGapCollectorRegime::High => "high",
        LiveGapCollectorRegime::HighChop => "high_chop",
        LiveGapCollectorRegime::Red => "red",
    }
}

fn live_gap_collector_resolved_config_snapshot(
    config: &ActionPlaceOrderLiveGapCollectorConfig,
) -> Value {
    json!({
        "candidateMaxAgeMs": config.pre_buy_collapse_guard.candidate_max_age_ms,
        "lateRemainingSec": config.pre_buy_collapse_guard.late_remaining_sec,
        "highPriceCent": config.pre_buy_collapse_guard.high_price * 100.0,
        "veryHighPriceCent": config.pre_buy_collapse_guard.very_high_price * 100.0,
        "gapDrop3sUsd": config.pre_buy_collapse_guard.high_price_gap_drop_3s_usd,
        "midPriceGapDrop3sUsd": config.pre_buy_collapse_guard.mid_price_gap_drop_3s_usd,
        "gapDrop5sUsd": config.pre_buy_collapse_guard.mid_price_gap_drop_5s_usd,
        "bounceBufferUsd": config.pre_buy_collapse_guard.bounce_buffer_usd,
        "windowStartSec": config.window_start_sec,
        "windowEndSec": config.window_end_sec,
        "noNewEntryUnderSec": config.no_new_entry_under_sec,
        "retryMs": config.retry_ms,
        "hardMaxPriceCent": config.hard_max_price * 100.0,
        "binanceMaxStaleMs": config.binance_max_stale_ms,
        "lowCleanGapUsd": config.low_clean_gap_usd,
        "normalGapUsd": config.normal_gap_usd,
        "highGapUsd": config.high_gap_usd,
        "highChopGapUsd": config.high_chop_gap_usd,
        "latencyBufferUsd": config.latency_buffer_usd,
        "strongOnlyUnderSec": config.strong_only_under_sec,
        "strongSignalExtraGapUsd": config.strong_signal_extra_gap_usd,
        "notifyOnPreBuyCollapseGuardDecision": config.notify_pre_buy_collapse_guard_decision,
        "preBuyCollapseGuardNotificationMode": config.pre_buy_collapse_guard_notification_mode,
        "liveGapHistoryPrewarmEnabled": config.live_gap_history_prewarm_enabled,
        "liveGapHistoryPrewarmSec": config.live_gap_history_prewarm_sec,
        "liveGapHistoryPrewarmStartMode": config.live_gap_history_prewarm_start_mode,
        "liveGapHistoryPrewarmSides": config.live_gap_history_prewarm_sides,
        "liveGapHistorySampleMs": config.live_gap_history_sample_ms,
        "liveGapHistoryRetentionMs": config.live_gap_history_retention_ms,
        "notifyOnPreBuyHistoryWarning": config.notify_on_pre_buy_history_warning,
        "preBuyHistoryWarningMode": config.pre_buy_history_warning_mode,
        "noReversalEntryGuard": no_reversal_entry_guard_config_snapshot(&config.no_reversal_entry_guard),
    })
}

fn live_gap_collector_best_ask(order_book: &OrderBookSnapshot) -> Option<f64> {
    order_book
        .asks
        .iter()
        .filter(|level| level.price.is_finite() && level.price > 0.0 && level.price < 1.0)
        .map(|level| level.price)
        .min_by(f64::total_cmp)
}

fn live_gap_collector_intended_qty(sizing: &ActionPlaceOrderSizing, best_ask: f64) -> Option<f64> {
    sizing
        .target_qty
        .filter(|qty| qty.is_finite() && *qty > 0.0)
        .or_else(|| (best_ask.is_finite() && best_ask > 0.0).then_some(sizing.size_usdc / best_ask))
        .filter(|qty| qty.is_finite() && *qty > 0.0)
}

async fn live_gap_collector_volume_ratio(
    repo: &PostgresRepository,
    market_slug: &str,
    asset: &str,
    now: DateTime<Utc>,
) -> Option<f64> {
    let summary = repo
        .market_trade_volume_summary(market_slug, now)
        .await
        .ok()?;
    let baseline = repo
        .market_trade_volume_bucket_median(asset, 30.0, 0.0, 7, 30, market_slug, now)
        .await
        .ok()
        .filter(|median| median.sample_count >= 20)
        .map(|median| median.median_volume_usdc)
        .filter(|value| value.is_finite() && *value > 0.0)?;
    Some(summary.volume_30s / baseline)
}

fn live_gap_collector_volatility_usd(asset: &str, now_ms: i64) -> Option<f64> {
    let samples = trade_flow::guards::chainlink_price::get_chainlink_price_samples(
        asset,
        now_ms - 15_000,
        now_ms,
    )
    .ok()?;
    let mut min_price = f64::INFINITY;
    let mut max_price = f64::NEG_INFINITY;
    for sample in samples {
        min_price = min_price.min(sample.price);
        max_price = max_price.max(sample.price);
    }
    (min_price.is_finite() && max_price.is_finite()).then_some(max_price - min_price)
}

fn live_gap_collector_regime(
    binance_staleness_ms: i64,
    max_stale_ms: i64,
    volume_ratio: Option<f64>,
    volatility_usd: Option<f64>,
) -> LiveGapCollectorRegime {
    if binance_staleness_ms > max_stale_ms {
        return LiveGapCollectorRegime::Red;
    }
    let volume_ratio = volume_ratio
        .filter(|value| value.is_finite())
        .unwrap_or(1.0);
    let volatility_usd = volatility_usd
        .filter(|value| value.is_finite())
        .unwrap_or(0.0);
    if volume_ratio >= 4.0 || volatility_usd >= 40.0 {
        LiveGapCollectorRegime::HighChop
    } else if volume_ratio >= 2.5 || volatility_usd >= 25.0 {
        LiveGapCollectorRegime::High
    } else if volume_ratio < 1.5 && volatility_usd <= 12.0 {
        LiveGapCollectorRegime::LowClean
    } else {
        LiveGapCollectorRegime::Normal
    }
}

async fn evaluate_action_place_order_live_gap_collector(
    repo: &PostgresRepository,
    client: Option<&dyn OrderExecutor>,
    _node: &TradeFlowNode,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    sizing: &ActionPlaceOrderSizing,
    config: &ActionPlaceOrderLiveGapCollectorConfig,
) -> LiveGapCollectorDecision {
    let eval_started_at = Instant::now();
    let now = Utc::now();
    let now_ms = now.timestamp_millis();
    let Some(scope) = find_updown_scope_by_slug(market_slug) else {
        return live_gap_collector_block("unsupported_market", true, json!({}));
    };
    let Some(direction) = live_gap_collector_direction(outcome_label) else {
        return live_gap_collector_block("unsupported_outcome_label", true, json!({}));
    };
    let Some(window_start) = MarketCycleId(market_slug.to_string()).start_time() else {
        return live_gap_collector_block("market_start_unavailable", true, json!({}));
    };
    let elapsed_sec = live_gap_collector_elapsed_sec(market_slug, now).unwrap_or_default();
    let remaining_sec = live_gap_collector_remaining_sec(market_slug, now).unwrap_or_default();
    let terminal_window = elapsed_sec >= config.window_end_sec;
    if elapsed_sec < config.window_start_sec {
        return live_gap_collector_block(
            "before_live_gap_window",
            false,
            json!({ "elapsed_sec": elapsed_sec, "remaining_sec": remaining_sec }),
        );
    }
    if terminal_window {
        return live_gap_collector_block(
            "after_live_gap_window",
            true,
            json!({ "elapsed_sec": elapsed_sec, "remaining_sec": remaining_sec }),
        );
    }
    if remaining_sec < config.no_new_entry_under_sec {
        return live_gap_collector_block(
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
            return live_gap_collector_block(
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
                return live_gap_collector_block(
                    "binance_price_unavailable",
                    false,
                    json!({ "error": err.to_string() }),
                );
            }
        };
    let Some(client) = client else {
        return live_gap_collector_block("order_executor_unavailable", false, json!({}));
    };
    let orderbook_started_at = Instant::now();
    let order_book = match client.order_book(token_id).await {
        Ok(Some(book)) => book,
        Ok(None) => return live_gap_collector_block("orderbook_unavailable", false, json!({})),
        Err(err) => {
            return live_gap_collector_block(
                "orderbook_unavailable",
                false,
                json!({ "error": err.to_string() }),
            );
        }
    };
    let orderbook_ms = live_gap_collector_elapsed_ms(orderbook_started_at);
    let Some(best_ask) = live_gap_collector_best_ask(&order_book) else {
        return live_gap_collector_block("best_ask_unavailable", false, json!({}));
    };
    let intended_qty = live_gap_collector_intended_qty(sizing, best_ask);
    let depth = trade_flow::guards::price_to_beat::evaluate_price_to_beat_iv_depth(
        Some(&order_book),
        best_ask,
        intended_qty,
        (config.hard_max_price - best_ask).max(0.0),
        true,
    );
    let effective_fill = depth.estimated_avg_fill.or(Some(best_ask));
    let volume_started_at = Instant::now();
    let volume_ratio = live_gap_collector_volume_ratio(repo, market_slug, scope.asset, now).await;
    let volume_ms = live_gap_collector_elapsed_ms(volume_started_at);
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
            market_slug,
            token_id,
            outcome_label,
            PreBuyCollapseSample {
                ts_ms: now_ms,
                live_gap,
                effective_fill: fill_price,
                best_ask,
                sample_source: "action_collector",
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
    payload_obj.insert("decision".to_string(), json!("buy"));
    payload_obj.insert("market_slug".to_string(), json!(market_slug));
    payload_obj.insert("token_id".to_string(), json!(token_id));
    payload_obj.insert("outcome_label".to_string(), json!(outcome_label));
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
        json!("base_collector_pass_pending_guard"),
    );
    payload_obj.insert("candidate_price_floor_seen_below".to_string(), json!(false));
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
    if config.ptb_telemetry_enabled {
        payload_obj.insert(
            "ptb_telemetry".to_string(),
            json!(
                trade_flow::guards::polymarket_price_to_beat::get_price_to_beat_cached(market_slug)
                    .map(|snapshot| json!({
                        "price_to_beat": snapshot.price_to_beat,
                        "fetched_at": snapshot.fetched_at.to_rfc3339(),
                        "source": snapshot.source.as_str(),
                        "source_latency_ms": snapshot.source_latency_ms,
                    }))
            ),
        );
    }
    let mut payload = Value::Object(payload_obj);
    live_gap_collector_insert_timings(
        &mut payload,
        eval_started_at,
        Some(orderbook_ms),
        Some(volume_ms),
        None,
    );
    if regime == LiveGapCollectorRegime::Red {
        return live_gap_collector_block("regime_red_or_stale", false, payload);
    }
    if depth.result != "pass" {
        return live_gap_collector_block(
            depth.block_reason.unwrap_or("depth_guard_blocked"),
            false,
            payload,
        );
    }
    if effective_fill.is_none_or(|price| price > config.hard_max_price) {
        return live_gap_collector_block("effective_fill_above_hard_max", false, payload);
    }
    if live_gap < required_gap {
        return live_gap_collector_block("live_gap_below_required", false, payload);
    }
    let guard_input = PreBuyCollapseGuardInput {
        market_slug,
        token_id,
        outcome_label,
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
        trigger_source: None,
    };
    let guard_decision =
        evaluate_pre_buy_collapse_guard(&guard_input, &config.pre_buy_collapse_guard, now_ms);
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
        live_gap_collector_insert_timings(
            &mut payload,
            eval_started_at,
            Some(orderbook_ms),
            Some(volume_ms),
            None,
        );
        return live_gap_collector_block(guard_decision.reason_code, false, payload);
    }
    let mut final_reason_code = guard_decision.reason_code;
    let mut no_reversal_ms = None;
    if config.no_reversal_entry_guard.enabled {
        let no_reversal_input = NoReversalEntryGuardInput {
            market_slug,
            asset: scope.asset,
            direction,
            remaining_sec,
            effective_fill: effective_fill.unwrap_or(best_ask),
            current_live_gap: live_gap,
            regime: live_gap_collector_regime_label(regime),
            slope_bucket: no_reversal_slope_bucket_from_pre_buy_payload(&guard_decision.payload),
        };
        let no_reversal_started_at = Instant::now();
        let no_reversal_decision = evaluate_no_reversal_entry_guard(
            repo,
            &config.no_reversal_entry_guard,
            &no_reversal_input,
        )
        .await;
        no_reversal_ms = Some(live_gap_collector_elapsed_ms(no_reversal_started_at));
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
            live_gap_collector_insert_timings(
                &mut payload,
                eval_started_at,
                Some(orderbook_ms),
                Some(volume_ms),
                no_reversal_ms,
            );
            return live_gap_collector_block(no_reversal_decision.reason_code, false, payload);
        }
    }
    live_gap_collector_insert_timings(
        &mut payload,
        eval_started_at,
        Some(orderbook_ms),
        Some(volume_ms),
        no_reversal_ms,
    );
    LiveGapCollectorDecision {
        passed: true,
        terminal: false,
        reason_code: final_reason_code,
        payload,
    }
}

fn live_gap_collector_block(
    reason_code: &'static str,
    terminal: bool,
    mut payload: Value,
) -> LiveGapCollectorDecision {
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
    LiveGapCollectorDecision {
        passed: false,
        terminal,
        reason_code,
        payload,
    }
}

#[allow(clippy::too_many_arguments)]
async fn maybe_block_action_place_order_live_gap_collector(
    repo: &PostgresRepository,
    client: Option<&dyn OrderExecutor>,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &mut Value,
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    side: &str,
    execution_mode: &str,
    sizing: &ActionPlaceOrderSizing,
    config: Option<&ActionPlaceOrderLiveGapCollectorConfig>,
) -> Result<Option<TradeFlowNodeExecution>> {
    let Some(config) = config else {
        return Ok(None);
    };
    let decision = evaluate_action_place_order_live_gap_collector(
        repo,
        client,
        node,
        market_slug,
        token_id,
        outcome_label,
        sizing,
        config,
    )
    .await;
    let mut payload = decision.payload.clone();
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("node_key".to_string(), json!(node.key));
        obj.insert("side".to_string(), json!(side));
        obj.insert("execution_mode".to_string(), json!(execution_mode));
    }
    set_flow_context(context, "liveGapCollector", payload.clone());
    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        if decision.passed {
            "live_gap_collector_passed"
        } else {
            "live_gap_collector_blocked"
        },
        &payload,
    )
    .await?;
    if let Some(pre_buy_guard) = payload.get("pre_buy_collapse_guard") {
        repo.append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            if decision.passed {
                "pre_buy_collapse_guard_passed"
            } else {
                "pre_buy_collapse_guard_blocked"
            },
            &json!({
                "node_key": node.key,
                "node_type": node.node_type,
                "side": side,
                "execution_mode": execution_mode,
                "pre_buy_collapse_guard": pre_buy_guard,
            }),
        )
        .await?;
    }
    if let Some(no_reversal_guard) = payload.get("no_reversal_entry_guard") {
        let protection_applied = no_reversal_guard
            .get("protection")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "applied");
        repo.append_trade_flow_event(
            Some(run.id),
            run.definition_id,
            Some(run.version_id),
            if decision.passed {
                if protection_applied {
                    "no_reversal_entry_guard_passed"
                } else {
                    "no_reversal_entry_guard_soft_passed"
                }
            } else {
                "no_reversal_entry_guard_blocked"
            },
            &json!({
                "node_key": node.key,
                "node_type": node.node_type,
                "side": side,
                "execution_mode": execution_mode,
                "no_reversal_entry_guard": no_reversal_guard,
            }),
        )
        .await?;
    }
    if decision.passed {
        maybe_append_live_gap_collector_ptb_late_confirmed_event(repo, run, node, &payload).await?;
    }
    let pre_buy_notification_handled = maybe_send_pre_buy_collapse_guard_notification(
        repo,
        run,
        node,
        context,
        config,
        &decision,
        &payload,
        market_slug,
        token_id,
        outcome_label,
        side,
    )
    .await;
    if !pre_buy_notification_handled {
        maybe_send_live_gap_collector_decision_notification(
            repo, run, node, config, &decision, &payload,
        )
        .await;
    }
    if decision.passed {
        return Ok(None);
    }
    let repeat_at =
        (!decision.terminal).then(|| Utc::now() + ChronoDuration::milliseconds(config.retry_ms));
    Ok(Some(TradeFlowNodeExecution {
        output: json!({
            "node_key": node.key,
            "blocked": true,
            "reason": "live_gap_collector_blocked",
            "reason_code": decision.reason_code,
            "market_slug": market_slug,
            "token_id": token_id,
            "outcome_label": outcome_label,
            "side": side,
            "execution_mode": execution_mode,
            "live_gap_collector": payload,
        }),
        routes: if decision.terminal && decision.reason_code != "too_late_for_new_entry" {
            vec![TradeFlowRouteDecision {
                edge_type: "on_error".to_string(),
                available_at: Utc::now(),
            }]
        } else {
            Vec::new()
        },
        repeat_at,
        repeat_idempotency_key: repeat_at.map(|_| {
            format!(
                "live_gap_collector:{}:{}:{}:{}",
                run.id, node.key, market_slug, token_id
            )
        }),
    }))
}

async fn maybe_append_live_gap_collector_ptb_late_confirmed_event(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    payload: &Value,
) -> Result<()> {
    let Some(ptb) = payload.get("ptb_telemetry").and_then(Value::as_object) else {
        return Ok(());
    };
    if ptb.get("price_to_beat").is_none_or(Value::is_null) {
        return Ok(());
    }
    repo.append_trade_flow_event(
        Some(run.id),
        run.definition_id,
        Some(run.version_id),
        "live_gap_collector_ptb_late_confirmed",
        &json!({
            "node_key": node.key,
            "market_slug": payload.get("market_slug"),
            "token_id": payload.get("token_id"),
            "outcome_label": payload.get("outcome_label"),
            "live_gap_pass_ts_ms": Utc::now().timestamp_millis(),
            "entry_price_at_live_pass": payload.get("effective_fill_price").or_else(|| payload.get("best_ask")),
            "ptb": ptb,
            "telemetry_only": true,
        }),
    )
    .await
}

fn live_gap_collector_context_payload(context: &Value) -> Option<Value> {
    let payload = flow_context_value(context, "liveGapCollector")?;
    let mode = payload.get("mode").and_then(Value::as_str)?;
    (mode == ACTION_PLACE_ORDER_MODE_LIVE_GAP_COLLECTOR_V1).then_some(payload)
}

async fn persist_action_place_order_live_gap_metadata(
    repo: &PostgresRepository,
    builder_order_id: i64,
    context: &Value,
) -> Result<()> {
    if let Some(payload) = live_gap_collector_context_payload(context) {
        repo.set_trade_builder_order_live_gap_metadata(builder_order_id, Some(&payload))
            .await?;
    }
    Ok(())
}

async fn trade_builder_order_has_live_gap_stop_loss_metadata(
    repo: &PostgresRepository,
    builder_order_id: i64,
) -> Result<bool> {
    Ok(repo
        .load_trade_builder_order_live_gap_metadata(builder_order_id)
        .await?
        .and_then(|metadata| {
            metadata
                .get("live_gap_stop_loss")
                .and_then(|value| value.get("enabled"))
                .and_then(Value::as_bool)
        })
        .unwrap_or(false))
}

async fn trade_builder_live_gap_stop_loss_child_metadata(
    repo: &PostgresRepository,
    parent_order: &TradeBuilderOrder,
) -> Result<Option<Value>> {
    let Some(mut metadata) = repo
        .load_trade_builder_order_live_gap_metadata(parent_order.id)
        .await?
    else {
        return Ok(None);
    };
    let enabled = metadata
        .get("live_gap_stop_loss")
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !enabled {
        return Ok(None);
    }
    if let Some(obj) = metadata.as_object_mut() {
        obj.insert(
            "parent_builder_order_id".to_string(),
            json!(parent_order.id),
        );
        obj.insert("sl_type".to_string(), json!("live_gap"));
    }
    Ok(Some(metadata))
}

fn live_gap_metadata_f64(metadata: &Value, key: &str) -> Option<f64> {
    metadata.get(key).and_then(value_as_f64)
}

fn live_gap_stop_loss_threshold(metadata: &Value) -> Option<f64> {
    metadata
        .get("live_gap_stop_loss")
        .and_then(|value| value.get("threshold_gap_usd"))
        .and_then(value_as_f64)
        .filter(|value| value.is_finite())
}

fn live_gap_stop_loss_min_remaining_sec(metadata: &Value) -> i64 {
    metadata
        .get("live_gap_stop_loss")
        .and_then(|value| value.get("min_remaining_sec"))
        .and_then(Value::as_i64)
        .unwrap_or(15)
        .clamp(0, 300)
}

async fn trade_builder_evaluate_live_gap_stop_loss(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
) -> Result<Option<TradeBuilderLiveGapStopLossEvaluation>> {
    if !trade_builder_is_stop_loss_child(order) {
        return Ok(None);
    }
    let Some(metadata) = repo
        .load_trade_builder_order_live_gap_metadata(order.id)
        .await?
    else {
        return Ok(None);
    };
    let threshold_gap_usd = live_gap_stop_loss_threshold(&metadata).unwrap_or(0.0);
    let min_remaining_sec = live_gap_stop_loss_min_remaining_sec(&metadata);
    let asset = metadata
        .get("asset")
        .and_then(Value::as_str)
        .map(str::to_string);
    let direction = metadata
        .get("direction")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| live_gap_collector_direction(&order.outcome_label).map(str::to_string));
    let reference_price = live_gap_metadata_f64(&metadata, "open_price");
    let remaining_sec = live_gap_collector_remaining_sec(&order.market_slug, Utc::now());
    let Some(asset_ref) = asset.as_deref() else {
        return Ok(Some(TradeBuilderLiveGapStopLossEvaluation {
            asset,
            direction,
            threshold_gap_usd,
            reference_price,
            current_price: None,
            current_price_source: "binance_live_data_ws",
            current_price_ts_ms: None,
            current_price_staleness_ms: None,
            directional_gap: None,
            remaining_sec,
            min_remaining_sec,
            reason_code: "asset_unavailable",
            should_trigger: false,
        }));
    };
    let now_ms = Utc::now().timestamp_millis();
    let current =
        match trade_flow::guards::binance_price::get_binance_price_snapshot(asset_ref, now_ms) {
            Ok(snapshot) => snapshot,
            Err(_) => {
                return Ok(Some(TradeBuilderLiveGapStopLossEvaluation {
                    asset,
                    direction,
                    threshold_gap_usd,
                    reference_price,
                    current_price: None,
                    current_price_source: "binance_live_data_ws",
                    current_price_ts_ms: None,
                    current_price_staleness_ms: None,
                    directional_gap: None,
                    remaining_sec,
                    min_remaining_sec,
                    reason_code: "current_price_unavailable",
                    should_trigger: false,
                }));
            }
        };
    let directional_gap = match (direction.as_deref(), reference_price) {
        (Some("down"), Some(open_price)) => Some(open_price - current.price),
        (Some(_), Some(open_price)) => Some(current.price - open_price),
        _ => None,
    };
    let below_min_remaining = remaining_sec.is_some_and(|value| value <= min_remaining_sec);
    let should_trigger =
        !below_min_remaining && directional_gap.is_some_and(|gap| gap <= threshold_gap_usd);
    Ok(Some(TradeBuilderLiveGapStopLossEvaluation {
        asset,
        direction,
        threshold_gap_usd,
        reference_price,
        current_price: Some(current.price),
        current_price_source: "binance_live_data_ws",
        current_price_ts_ms: Some(current.timestamp_ms),
        current_price_staleness_ms: Some(current.staleness_ms),
        directional_gap,
        remaining_sec,
        min_remaining_sec,
        reason_code: if below_min_remaining {
            "below_min_remaining"
        } else if should_trigger {
            "live_gap_threshold_hit"
        } else {
            "live_gap_threshold_not_met"
        },
        should_trigger,
    }))
}

fn append_trade_builder_live_gap_stop_loss_payload(
    payload: &mut serde_json::Map<String, Value>,
    evaluation: &TradeBuilderLiveGapStopLossEvaluation,
) {
    payload.insert(
        "live_gap_stop_loss".to_string(),
        json!({
            "reason_code": evaluation.reason_code,
            "asset": evaluation.asset,
            "direction": evaluation.direction,
            "threshold_gap_usd": evaluation.threshold_gap_usd,
            "reference_price": evaluation.reference_price,
            "current_price": evaluation.current_price,
            "current_price_source": evaluation.current_price_source,
            "current_price_ts_ms": evaluation.current_price_ts_ms,
            "current_price_staleness_ms": evaluation.current_price_staleness_ms,
            "directional_gap": evaluation.directional_gap,
            "remaining_sec": evaluation.remaining_sec,
            "min_remaining_sec": evaluation.min_remaining_sec,
            "should_trigger": evaluation.should_trigger,
        }),
    );
}

async fn create_trade_builder_live_gap_stop_loss_child_if_configured(
    repo: &PostgresRepository,
    ws: &ClobWsClient,
    parent_order: &TradeBuilderOrder,
    canonical_entry_qty: f64,
    execution_price: f64,
) -> Result<Option<i64>> {
    let Some(metadata) =
        trade_builder_live_gap_stop_loss_child_metadata(repo, parent_order).await?
    else {
        return Ok(None);
    };
    let Some(child_sizing) = trade_builder_ladder_child_qty(canonical_entry_qty, 100.0) else {
        return Ok(None);
    };
    let child_size_usdc = (child_sizing.target_qty * execution_price).max(0.0);
    let child_id = repo
        .create_trade_builder_order_with_exit_ladders(
            parent_order.trade_id,
            "conditional",
            "armed",
            &parent_order.market_slug,
            &parent_order.token_id,
            &parent_order.outcome_label,
            "sell",
            "market",
            Some("cross_below"),
            None,
            None,
            None,
            None,
            TRADE_BUILDER_SIZE_BASIS_SHARES,
            child_size_usdc,
            Some(child_sizing.target_qty),
            Some(child_sizing.remaining_qty),
            parent_order.min_price_distance_cent,
            parent_order.expires_at,
            None,
            None,
            1,
            Some(parent_order.id),
            false,
            None,
            None,
            false,
            None,
            None,
            None,
            parent_order.fee_rate_bps,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            false,
            None,
            None,
            false,
            false,
            None,
            false,
            0,
            None,
            false,
            parent_order.notify_on_sl_hit,
            false,
            false,
            false,
            false,
            false,
            false,
            None,
            false,
            false,
            false,
            None,
            None,
            None,
        )
        .await?;
    repo.set_trade_builder_order_live_gap_metadata(child_id, Some(&metadata))
        .await?;
    if let Ok(Some(mut child_order)) = repo.get_trade_builder_order(child_id).await {
        if let Some(snapshot) = ws.get_market_snapshot(&parent_order.token_id).await {
            if let Some(initial_last_seen_price) =
                trade_builder_last_seen_price_from_market_snapshot(&child_order, &snapshot)
            {
                repo.set_trade_builder_last_seen_price(child_id, initial_last_seen_price)
                    .await?;
                child_order.last_seen_price = Some(initial_last_seen_price);
            }
        }
        sync_armed_builder_order_to_cache(child_order).await;
    }
    repo.append_trade_builder_order_event(
        parent_order.id,
        "live_gap_sl_sell_created",
        &json!({
            "child_order_id": child_id,
            "initial_status": "armed",
            "family": "live_gap_sl",
            "exit_mode": TRADE_BUILDER_EXIT_MODE_HARD,
            "sibling_policy": TRADE_BUILDER_EXIT_SIBLING_POLICY_CANCEL_ALL,
            "trigger_price": Value::Null,
            "size_basis": TRADE_BUILDER_SIZE_BASIS_SHARES,
            "size_pct": 100.0,
            "target_qty": child_sizing.target_qty,
            "execution_price": execution_price,
            "metadata": metadata,
        }),
    )
    .await?;
    Ok(Some(child_id))
}
