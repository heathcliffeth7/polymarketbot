const BOT_DECISION_LOG_SCHEMA_VERSION: i32 = 1;
const DECISION_LOG_VOLUME_LOOKBACK_DAYS: i64 = 7;
const DECISION_LOG_VOLUME_BASELINE_MIN_SAMPLES: i64 = 20;
const DECISION_LOG_VOLUME_WINDOW_SEC: i64 = 30;

#[derive(Debug, Clone, Default)]
struct TradeBuilderDecisionLogOptions {
    idempotency_key: Option<String>,
    decision_id: Option<String>,
    sl_event_id: Option<String>,
    fill_event_id: Option<String>,
    exchange_order_id: Option<String>,
    parent_order_id: Option<String>,
    child_order_id: Option<String>,
    event_ts: Option<DateTime<Utc>>,
}

fn trade_builder_decision_id_for_order(order: &TradeBuilderOrder) -> String {
    let root_order_id = order.parent_order_id.unwrap_or(order.id);
    format!("dec:tb:{root_order_id}")
}

fn trade_builder_sl_event_id_for_order(order: &TradeBuilderOrder) -> String {
    let root_order_id = order.parent_order_id.unwrap_or(order.id);
    let child_order_id = if order.parent_order_id.is_some() {
        order.id
    } else {
        root_order_id
    };
    format!("sl:tb:{root_order_id}:{child_order_id}")
}

fn trade_builder_fill_event_id_for_order(order: &TradeBuilderOrder, exchange_order_id: &str) -> String {
    format!("fill:tb:{}:{exchange_order_id}", order.id)
}

fn trade_builder_asset_from_market_slug(market_slug: &str) -> Option<String> {
    find_updown_scope_by_slug(market_slug).map(|scope| scope.asset.to_ascii_uppercase())
}

fn trade_builder_outcome_norm(outcome_label: &str) -> Option<&'static str> {
    match outcome_label.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "long" | "bull" => Some("UP"),
        "no" | "down" | "short" | "bear" => Some("DOWN"),
        _ => None,
    }
}

fn trade_builder_decision_log_payload(
    event_type: &str,
    order: &TradeBuilderOrder,
    event_ts: DateTime<Utc>,
    options: &TradeBuilderDecisionLogOptions,
    payload: Value,
) -> Value {
    let mut obj = match payload {
        Value::Object(obj) => obj,
        other => {
            let mut obj = serde_json::Map::new();
            obj.insert("raw_payload".to_string(), other);
            obj
        }
    };
    let root_order_id = order.parent_order_id.unwrap_or(order.id).to_string();
    let decision_id = options
        .decision_id
        .clone()
        .unwrap_or_else(|| trade_builder_decision_id_for_order(order));
    obj.insert("schema_version".to_string(), json!(BOT_DECISION_LOG_SCHEMA_VERSION));
    obj.insert("event".to_string(), json!(event_type));
    obj.insert("event_ts".to_string(), json!(event_ts.to_rfc3339()));
    obj.insert("decision_id".to_string(), json!(decision_id));
    obj.insert("root_order_id".to_string(), json!(root_order_id));
    obj.insert("order_id".to_string(), json!(order.id.to_string()));
    obj.insert("source_trade_id".to_string(), json!(order.trade_id.to_string()));
    obj.insert("market_slug".to_string(), json!(&order.market_slug));
    obj.insert("outcome".to_string(), json!(&order.outcome_label));
    obj.insert("outcome_norm".to_string(), json!(trade_builder_outcome_norm(&order.outcome_label)));
    obj.insert("outcome_token_id".to_string(), json!(&order.token_id));
    obj.insert("side".to_string(), json!(&order.side));
    obj.insert("kind".to_string(), json!(&order.kind));
    obj.insert("status_at_event".to_string(), json!(&order.status));
    obj.insert("flow_run_id".to_string(), json!(order.origin_flow_run_id));
    obj.insert(
        "flow_definition_id".to_string(),
        json!(order.origin_flow_definition_id),
    );
    obj.insert(
        "workflow".to_string(),
        json!(order.origin_flow_node_key.as_deref()),
    );
    if let Some(sl_event_id) = options.sl_event_id.as_ref() {
        obj.insert("sl_event_id".to_string(), json!(sl_event_id));
    }
    if let Some(fill_event_id) = options.fill_event_id.as_ref() {
        obj.insert("fill_event_id".to_string(), json!(fill_event_id));
    }
    Value::Object(obj)
}

fn trade_builder_spawn_decision_log(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    event_type: &'static str,
    payload: Value,
    options: TradeBuilderDecisionLogOptions,
) {
    let event_ts = options.event_ts.unwrap_or_else(Utc::now);
    let decision_id = options
        .decision_id
        .clone()
        .unwrap_or_else(|| trade_builder_decision_id_for_order(order));
    let root_order_id = order.parent_order_id.unwrap_or(order.id).to_string();
    let payload = trade_builder_decision_log_payload(event_type, order, event_ts, &options, payload);
    let input = bot_infra::db::BotDecisionLogInput {
        event_id: Uuid::new_v4(),
        idempotency_key: options.idempotency_key,
        schema_version: BOT_DECISION_LOG_SCHEMA_VERSION,
        event_type: event_type.to_string(),
        event_ts,
        decision_id: Some(decision_id),
        sl_event_id: options.sl_event_id,
        fill_event_id: options.fill_event_id,
        market_slug: Some(order.market_slug.clone()),
        root_order_id: Some(root_order_id),
        order_id: Some(order.id.to_string()),
        exchange_order_id: options.exchange_order_id,
        parent_order_id: options
            .parent_order_id
            .or_else(|| order.parent_order_id.map(|id| id.to_string())),
        child_order_id: options.child_order_id,
        source_trade_id: Some(order.trade_id.to_string()),
        flow_run_id: order.origin_flow_run_id.map(|id| id.to_string()),
        flow_definition_id: order.origin_flow_definition_id.map(|id| id.to_string()),
        pair_session_id: order.pair_session_id.map(|id| id.to_string()),
        asset: trade_builder_asset_from_market_slug(&order.market_slug),
        workflow: order.origin_flow_node_key.clone(),
        outcome: Some(order.outcome_label.clone()),
        outcome_token_id: Some(order.token_id.clone()),
        opposite_token_id: None,
        payload,
    };
    let repo = repo.clone();
    tokio::spawn(async move {
        if let Err(err) = repo.append_bot_decision_log(&input).await {
            warn!(
                target_event = %input.event_type,
                idempotency_key = input.idempotency_key.as_deref().unwrap_or(""),
                error = %err,
                "DECISION_LOG_APPEND_FAILED"
            );
        }
    });
}

fn decision_volume_regime(ratio: Option<f64>) -> &'static str {
    let Some(ratio) = ratio.filter(|value| value.is_finite() && *value >= 0.0) else {
        return "unknown";
    };
    if ratio < 1.5 {
        "normal"
    } else if ratio < 2.5 {
        "elevated"
    } else if ratio < 4.0 {
        "high"
    } else {
        "extreme"
    }
}

async fn build_trade_builder_volume_payload(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    event_ts: DateTime<Utc>,
) -> Value {
    let summary = match repo.market_trade_volume_summary(&order.market_slug, event_ts).await {
        Ok(summary) => summary,
        Err(err) => {
            warn!(
                market_slug = %order.market_slug,
                error = %err,
                "DECISION_LOG_VOLUME_SUMMARY_FAILED"
            );
            return json!({
                "polymarket": {"known": false, "reason": "query_failed"},
                "underlying": {"known": false, "source": "unavailable_in_current_feed"}
            });
        }
    };
    let baseline = if let Some(scope) = find_updown_scope_by_slug(&order.market_slug) {
        let bucket_start_sec = 30.0;
        let bucket_end_sec = 0.0;
        repo.market_trade_volume_bucket_median(
            scope.asset,
            bucket_start_sec,
            bucket_end_sec,
            DECISION_LOG_VOLUME_LOOKBACK_DAYS,
            DECISION_LOG_VOLUME_WINDOW_SEC,
            &order.market_slug,
            event_ts,
        )
        .await
        .ok()
        .filter(|median| {
            median.sample_count >= DECISION_LOG_VOLUME_BASELINE_MIN_SAMPLES
                && median.median_volume_usdc.is_finite()
                && median.median_volume_usdc > 0.0
        })
        .map(|median| median.median_volume_usdc)
    } else {
        None
    };
    let ratio = baseline.map(|value| summary.volume_30s / value);
    json!({
        "polymarket": {
            "known": true,
            "recent_notional_10s": summary.volume_10s,
            "recent_notional_30s": summary.volume_30s,
            "recent_notional_60s": summary.volume_60s,
            "recent_trade_count_10s": summary.trade_count_10s,
            "recent_trade_count_30s": summary.trade_count_30s,
            "recent_trade_count_60s": summary.trade_count_60s,
            "ratio": ratio,
            "regime": decision_volume_regime(ratio),
        },
        "underlying": {
            "known": false,
            "source": "unavailable_in_current_feed"
        },
        "volume_ratio_definition": {
            "numerator": "recent_notional_30s",
            "baseline": "rolling_median_notional_30s_last_7d_same_market_bucket",
            "baseline_value": baseline
        }
    })
}

fn directional_ptb_gap(snapshot: &TradeBuilderMarketSecondSnapshot, outcome_norm: &str) -> Option<f64> {
    let ptb = snapshot.ptb_ref_price?;
    let chainlink = snapshot.chainlink_price?;
    Some(if outcome_norm == "DOWN" {
        ptb - chainlink
    } else {
        chainlink - ptb
    })
    .filter(|value| value.is_finite())
}

fn nearest_gap_at_or_before(
    samples: &[(DateTime<Utc>, f64)],
    target: DateTime<Utc>,
) -> Option<f64> {
    samples
        .iter()
        .rev()
        .find(|(ts, _)| *ts <= target)
        .map(|(_, gap)| *gap)
        .or_else(|| samples.first().map(|(_, gap)| *gap))
}

fn build_ptb_payload_from_snapshots(
    order: &TradeBuilderOrder,
    event_ts: DateTime<Utc>,
    snapshots: &[TradeBuilderMarketSecondSnapshot],
) -> Value {
    let outcome_norm = trade_builder_outcome_norm(&order.outcome_label).unwrap_or("UP");
    let mut samples = snapshots
        .iter()
        .filter(|snapshot| snapshot.market_slug == order.market_slug)
        .filter_map(|snapshot| {
            directional_ptb_gap(snapshot, outcome_norm).map(|gap| (snapshot.second_ts, gap))
        })
        .collect::<Vec<_>>();
    samples.sort_by_key(|(ts, _)| *ts);

    let now = nearest_gap_at_or_before(&samples, event_ts);
    let gap_3s = nearest_gap_at_or_before(&samples, event_ts - ChronoDuration::seconds(3));
    let gap_5s = nearest_gap_at_or_before(&samples, event_ts - ChronoDuration::seconds(5));
    let gap_10s = nearest_gap_at_or_before(&samples, event_ts - ChronoDuration::seconds(10));
    let slope_5s = now.zip(gap_5s).map(|(current, past)| current - past);
    let trend = match slope_5s {
        Some(value) if value > 2.0 => "expanding",
        Some(value) if value < -2.0 => "collapsing",
        Some(_) => "flat",
        None => "unknown",
    };
    let cutoff = event_ts - ChronoDuration::seconds(30);
    let peak = samples
        .iter()
        .filter(|(ts, _)| *ts >= cutoff && *ts <= event_ts)
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .copied();
    json!({
        "history_key": format!("{}:{}:{}", order.market_slug, outcome_norm, order.token_id),
        "gap_now": now,
        "gap_3s_ago": gap_3s,
        "gap_5s_ago": gap_5s,
        "gap_10s_ago": gap_10s,
        "slope_3s": now.zip(gap_3s).map(|(current, past)| current - past),
        "slope_5s": slope_5s,
        "slope_10s": now.zip(gap_10s).map(|(current, past)| current - past),
        "trend": trend,
        "peak_last_30s": peak.map(|(_, gap)| gap),
        "seconds_since_peak": peak.map(|(ts, _)| event_ts.signed_duration_since(ts).num_milliseconds() as f64 / 1000.0),
        "drawdown_from_peak": peak.and_then(|(_, gap)| now.map(|current| gap - current)),
    })
}

async fn build_trade_builder_ptb_payload(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    event_ts: DateTime<Utc>,
) -> Value {
    let snapshots = repo
        .list_trade_builder_market_second_snapshots(&[order.market_slug.clone()])
        .await
        .unwrap_or_default();
    build_ptb_payload_from_snapshots(order, event_ts, &snapshots)
}

fn build_trade_builder_market_timing_payload(
    order: &TradeBuilderOrder,
    event_ts: DateTime<Utc>,
) -> Value {
    if let Some((market_start, market_end)) = trade_builder_second_snapshot_window(&order.market_slug) {
        let elapsed = event_ts.signed_duration_since(market_start).num_seconds().max(0);
        let remaining = market_end.signed_duration_since(event_ts).num_seconds().max(0);
        json!({
            "market_slug": &order.market_slug,
            "asset": trade_builder_asset_from_market_slug(&order.market_slug),
            "outcome": &order.outcome_label,
            "market_start_source": "slug_epoch",
            "market_start_ts": market_start.to_rfc3339(),
            "market_elapsed_s": elapsed,
            "remaining_s": remaining,
        })
    } else {
        json!({
            "market_slug": &order.market_slug,
            "asset": trade_builder_asset_from_market_slug(&order.market_slug),
            "outcome": &order.outcome_label,
            "market_start_source": "unavailable",
        })
    }
}

fn build_shadow_volume_guard_payload(ptb: &Value, volume: &Value) -> Value {
    let slope = ptb.get("slope_5s").and_then(Value::as_f64);
    let regime = volume
        .get("polymarket")
        .and_then(|value| value.get("regime"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let high_volume = matches!(regime, "high" | "extreme");
    let collapsing = slope.is_some_and(|value| value < -2.0);
    let would_block = high_volume && collapsing;
    json!({
        "evaluated": true,
        "would_block": would_block,
        "reason": if would_block {
            "high_volume_gap_collapsing"
        } else {
            "not_high_volume_gap_collapsing"
        }
    })
}

fn trade_builder_spawn_ptb_stop_loss_triggered_log(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    evaluation: &TradeBuilderPtbStopLossEvaluation,
    execution_price: f64,
) {
    let root_order_id = order.parent_order_id.unwrap_or(order.id);
    let sl_event_id = trade_builder_sl_event_id_for_order(order);
    trade_builder_spawn_decision_log(
        repo,
        order,
        "PTB_STOP_LOSS_TRIGGERED",
        json!({
            "sl_event_id": &sl_event_id,
            "sl_child_order_id": order.id.to_string(),
            "entry": {
                "root_order_id": root_order_id.to_string(),
                "ptb_reference_price": evaluation.ptb_reference_price,
            },
            "sl": {
                "seconds_since_entry": order.trigger_latched_at.map(|latched_at| {
                    Utc::now().signed_duration_since(latched_at).num_seconds().max(0)
                }),
                "ptb_gap_sl": evaluation.directional_gap,
                "ptb_gap_slope_5s_before_sl": Value::Null,
                "volume_regime_sl": Value::Null,
                "sl_trigger_price": execution_price,
                "sl_limit_price": order.working_price,
                "sl_fill_price": Value::Null,
                "slippage_cent": Value::Null,
                "sl_reason": evaluation.reason_code,
            },
            "ptb_stop_loss": {
                "asset": evaluation.asset.as_deref(),
                "direction": evaluation.direction.as_deref(),
                "threshold_gap_usd": evaluation.threshold_gap_usd,
                "ptb_reference_price": evaluation.ptb_reference_price,
                "current_chainlink_price": evaluation.current_chainlink_price,
                "directional_gap": evaluation.directional_gap,
                "reason_code": evaluation.reason_code,
                "should_trigger": evaluation.should_trigger
            }
        }),
        TradeBuilderDecisionLogOptions {
            idempotency_key: Some(format!(
                "PTB_STOP_LOSS_TRIGGERED:{root_order_id}:{}",
                order.id
            )),
            sl_event_id: Some(sl_event_id.clone()),
            child_order_id: Some(order.id.to_string()),
            parent_order_id: order.parent_order_id.map(|id| id.to_string()),
            ..TradeBuilderDecisionLogOptions::default()
        },
    );
    trade_builder_spawn_post_sl_followups(repo, order, sl_event_id);
}

fn trade_builder_spawn_post_sl_followups(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    sl_event_id: String,
) {
    let root_order_id = order.parent_order_id.unwrap_or(order.id);
    for check_after_s in [10_u64, 30_u64] {
        let repo = repo.clone();
        let order = order.clone();
        let sl_event_id = sl_event_id.clone();
        tokio::spawn(async move {
            let scheduled_for = Utc::now() + ChronoDuration::seconds(check_after_s as i64);
            sleep(Duration::from_secs(check_after_s)).await;
            let executed_at = Utc::now();
            let actual_delay_s = executed_at
                .signed_duration_since(scheduled_for - ChronoDuration::seconds(check_after_s as i64))
                .num_milliseconds() as f64
                / 1000.0;
            let fresh_order = repo.get_trade_builder_order(order.id).await.ok().flatten();
            let current_price = fresh_order
                .as_ref()
                .and_then(|fresh| fresh.last_seen_price)
                .or(order.last_seen_price);
            trade_builder_spawn_decision_log(
                &repo,
                fresh_order.as_ref().unwrap_or(&order),
                "POST_SL_CHECK",
                json!({
                    "sl_event_id": &sl_event_id,
                    "check_after_s": check_after_s,
                    "catch_up": false,
                    "actual_delay_s": actual_delay_s,
                    "scheduled_for": scheduled_for.to_rfc3339(),
                    "executed_at": executed_at.to_rfc3339(),
                    "current_token_price": current_price,
                    "max_token_price_since_sl": current_price,
                    "recovered_to_entry": Value::Null,
                    "recovered_to_tp": Value::Null,
                    "continued_against_us": Value::Null
                }),
                TradeBuilderDecisionLogOptions {
                    idempotency_key: Some(format!(
                        "POST_SL_CHECK:{}:{}:{}s",
                        root_order_id, sl_event_id, check_after_s
                    )),
                    sl_event_id: Some(sl_event_id),
                    child_order_id: Some(order.id.to_string()),
                    parent_order_id: order.parent_order_id.map(|id| id.to_string()),
                    event_ts: Some(executed_at),
                    ..TradeBuilderDecisionLogOptions::default()
                },
            );
        });
    }

    if let Some((_, market_end)) = trade_builder_second_snapshot_window(&order.market_slug) {
        let repo = repo.clone();
        let order = order.clone();
        tokio::spawn(async move {
            let now = Utc::now();
            if market_end > now {
                let delay_ms = market_end.signed_duration_since(now).num_milliseconds().max(0) as u64;
                sleep(Duration::from_millis(delay_ms)).await;
            }
            let executed_at = Utc::now();
            trade_builder_spawn_decision_log(
                &repo,
                &order,
                "POST_SL_MARKET_END",
                json!({
                    "sl_event_id": &sl_event_id,
                    "market_slug": &order.market_slug,
                    "market_end_at": market_end.to_rfc3339(),
                    "executed_at": executed_at.to_rfc3339(),
                    "resolution": {"known": false, "reason": "resolution_not_checked_in_market_end_event"}
                }),
                TradeBuilderDecisionLogOptions {
                    idempotency_key: Some(format!(
                        "POST_SL_MARKET_END:{}:{}:{}",
                        root_order_id, sl_event_id, order.market_slug
                    )),
                    sl_event_id: Some(sl_event_id),
                    child_order_id: Some(order.id.to_string()),
                    parent_order_id: order.parent_order_id.map(|id| id.to_string()),
                    event_ts: Some(executed_at),
                    ..TradeBuilderDecisionLogOptions::default()
                },
            );
        });
    }
}

fn risk_tags_from_entry_payload(
    desired_price: f64,
    ptb: &Value,
    volume: &Value,
    market: &Value,
    shadow: &Value,
) -> Vec<&'static str> {
    let mut tags = Vec::new();
    if market
        .get("market_elapsed_s")
        .and_then(Value::as_i64)
        .is_some_and(|value| value >= 210)
    {
        tags.push("late_entry");
    }
    if market
        .get("remaining_s")
        .and_then(Value::as_i64)
        .is_some_and(|value| value <= 60)
    {
        tags.push("last_60s");
    }
    if desired_price >= 0.85 {
        tags.push("very_high_price");
    } else if desired_price >= 0.75 {
        tags.push("high_price");
    }
    match volume
        .get("polymarket")
        .and_then(|value| value.get("regime"))
        .and_then(Value::as_str)
    {
        Some("high") => tags.push("high_volume"),
        Some("extreme") => tags.push("extreme_volume"),
        _ => {}
    }
    if ptb
        .get("slope_5s")
        .and_then(Value::as_f64)
        .is_some_and(|value| value < 0.0)
    {
        tags.push("gap_slope_negative");
    }
    if ptb.get("trend").and_then(Value::as_str) == Some("collapsing") {
        tags.push("gap_collapsing");
    }
    if ptb
        .get("drawdown_from_peak")
        .and_then(Value::as_f64)
        .is_some_and(|value| value >= 6.0)
    {
        tags.push("gap_peak_drawdown_high");
    }
    if shadow
        .get("would_block")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        tags.push("shadow_volume_guard_block");
    }
    tags
}

fn risk_tag_values_from_entry_payload(
    desired_price: f64,
    ptb: &Value,
    market: &Value,
) -> Value {
    json!({
        "high_price": {"actual": desired_price, "threshold": 0.75},
        "very_high_price": {"actual": desired_price, "threshold": 0.85},
        "gap_slope_negative": {
            "actual": ptb.get("slope_5s").cloned().unwrap_or(Value::Null),
            "threshold": 0.0
        },
        "gap_collapsing": {
            "actual": ptb.get("slope_5s").cloned().unwrap_or(Value::Null),
            "threshold": -2.0
        },
        "late_entry": {
            "actual_elapsed_s": market.get("market_elapsed_s").cloned().unwrap_or(Value::Null),
            "threshold_s": 210
        }
    })
}

fn risk_tag_thresholds_payload() -> Value {
    json!({
        "late_entry_elapsed_s": 210,
        "last_60s_remaining_s": 60,
        "high_price": 0.75,
        "very_high_price": 0.85,
        "gap_slope_negative_threshold": 0.0,
        "gap_collapsing_threshold": -2.0,
        "gap_peak_drawdown_high_threshold": 6.0,
        "wide_spread_cent": 3.0,
        "high_slippage_cent": 2.0
    })
}

#[allow(clippy::too_many_arguments)]
fn trade_builder_spawn_entry_evaluated_decision_log(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    current_price: f64,
    desired_price: f64,
    best_ask: Option<f64>,
    trigger_price_guard: Value,
    execution_floor_guard: Value,
    max_price_guard: Value,
    effective_guard_scope: Option<&str>,
    effective_decision: &str,
    effective_reason_code: &str,
) {
    let repo = repo.clone();
    let order = order.clone();
    let effective_guard_scope = effective_guard_scope.map(str::to_string);
    let effective_decision = effective_decision.to_string();
    let effective_reason_code = effective_reason_code.to_string();
    tokio::spawn(async move {
        let event_ts = Utc::now();
        let market = build_trade_builder_market_timing_payload(&order, event_ts);
        let ptb = build_trade_builder_ptb_payload(&repo, &order, event_ts).await;
        let volume = build_trade_builder_volume_payload(&repo, &order, event_ts).await;
        let node_snapshot = repo
            .get_trade_builder_order_node_snapshot(order.id)
            .await
            .ok()
            .flatten()
            .map(|record| record.snapshot_json)
            .unwrap_or(Value::Null);
        let shadow = build_shadow_volume_guard_payload(&ptb, &volume);
        let risk_tags = risk_tags_from_entry_payload(desired_price, &ptb, &volume, &market, &shadow);
        let payload = json!({
            "decision": effective_decision,
            "decision_reason": effective_reason_code,
            "market": market,
            "price": {
                "token_ask": best_ask.or(Some(current_price)),
                "best_ask": best_ask,
                "estimated_avg_fill": desired_price,
                "max_price_allowed": order.max_price,
                "spread_cent": Value::Null,
                "vwap_slippage_cent": Value::Null
            },
            "ptb": ptb,
            "volume": volume,
            "guard_breakdown": {
                "ptb": trigger_price_guard,
                "execution_floor": execution_floor_guard,
                "max_price": max_price_guard,
                "risk_gate": {"pass": true, "reason": "not_evaluated_in_entry_logger"},
                "shadow_volume_guard": shadow
            },
            "effective_guard_scope": effective_guard_scope,
            "stop_loss_config_at_entry": {
                "sl_enabled": order.sl_enabled || order.ptb_stop_loss_gap_usd.is_some(),
                "sl_type": if order.ptb_stop_loss_gap_usd.is_some() { "ptb" } else { "price" },
                "ptb_stop_gap": order.ptb_stop_loss_gap_usd,
                "ptb_warning_gap": order.ptb_stop_loss_gap_usd.map(|gap| gap + 4.0),
                "will_create_child_order": order.sl_enabled || !order.ptb_stop_loss_rules_json.is_empty() || order.ptb_stop_loss_gap_usd.is_some()
            },
            "risk_tags": risk_tags,
            "risk_tag_values": risk_tag_values_from_entry_payload(desired_price, &ptb, &market),
            "risk_tag_thresholds": risk_tag_thresholds_payload(),
            "node_snapshot": node_snapshot,
            "config": {
                "strategy_config_version": "runtime",
                "workflow_config_hash": order.origin_flow_definition_id.map(|id| format!("flow_definition:{id}")),
                "ptb_config_version": "runtime",
                "sl_config_version": "runtime"
            },
            "data_freshness": {
                "chainlink_age_ms": Value::Null,
                "binance_age_ms": Value::Null,
                "orderbook_age_ms": Value::Null,
                "polymarket_ticks_age_ms": Value::Null,
                "underlying_volume_age_ms": Value::Null
            }
        });
        trade_builder_spawn_decision_log(
            &repo,
            &order,
            "ENTRY_EVALUATED",
            payload,
            TradeBuilderDecisionLogOptions {
                idempotency_key: Some(format!("ENTRY_EVALUATED:{}", trade_builder_decision_id_for_order(&order))),
                event_ts: Some(event_ts),
                ..TradeBuilderDecisionLogOptions::default()
            },
        );
    });
}

#[cfg(test)]
mod trade_builder_decision_log_tests {
    use super::*;

    #[test]
    fn decision_volume_regime_uses_forensic_thresholds() {
        assert_eq!(decision_volume_regime(Some(1.49)), "normal");
        assert_eq!(decision_volume_regime(Some(1.5)), "elevated");
        assert_eq!(decision_volume_regime(Some(2.5)), "high");
        assert_eq!(decision_volume_regime(Some(4.0)), "extreme");
        assert_eq!(decision_volume_regime(None), "unknown");
    }

    #[test]
    fn entry_risk_tags_capture_late_high_volume_collapse() {
        let ptb = json!({
            "slope_5s": -6.8,
            "trend": "collapsing",
            "drawdown_from_peak": 9.2
        });
        let volume = json!({
            "polymarket": {
                "regime": "high"
            }
        });
        let market = json!({
            "market_elapsed_s": 228,
            "remaining_s": 72
        });
        let shadow = json!({
            "would_block": true
        });
        let tags = risk_tags_from_entry_payload(0.82, &ptb, &volume, &market, &shadow);
        assert!(tags.contains(&"late_entry"));
        assert!(tags.contains(&"high_price"));
        assert!(tags.contains(&"high_volume"));
        assert!(tags.contains(&"gap_slope_negative"));
        assert!(tags.contains(&"gap_collapsing"));
        assert!(tags.contains(&"gap_peak_drawdown_high"));
        assert!(tags.contains(&"shadow_volume_guard_block"));
    }
}
