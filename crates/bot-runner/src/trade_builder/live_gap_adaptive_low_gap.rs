#[derive(Debug, Clone, Copy, PartialEq)]
struct LiveGapAdaptiveLowGapConfig {
    enabled: bool,
    trigger_count: i64,
    step_pct: f64,
    max_relax_pct: f64,
    max_shortfall_pct: f64,
    max_fill_price: f64,
    min_remaining_sec: i64,
    require_local_path_clean: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct LiveGapAdaptiveLowGapEvaluation {
    status: &'static str,
    scope: &'static str,
    key: String,
    price_bucket: &'static str,
    market_near_miss_count: i64,
    trigger_count: i64,
    pre_required_gap_usd: f64,
    adaptive_required_gap_usd: f64,
    relax_pct: f64,
    shortfall_usd: f64,
    shortfall_pct: f64,
    saved_from_block: bool,
    reason: &'static str,
    can_record_near_miss: bool,
    can_record_guard_miss_near_miss: bool,
}

#[derive(Debug, Clone)]
struct LiveGapAdaptiveLowGapStateEntry {
    count: i64,
    updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq)]
struct LiveGapAdaptiveLowGapChangeNotification {
    previous_relax_pct: Option<f64>,
    previous_adaptive_required_gap_usd: Option<f64>,
    new_relax_pct: f64,
    new_adaptive_required_gap_usd: f64,
    notified_at_ms: i64,
}

#[derive(Debug, Clone)]
struct LiveGapAdaptiveLowGapNotificationStateEntry {
    relax_pct: f64,
    adaptive_required_gap_usd: f64,
    notified_at_ms: i64,
}

#[allow(clippy::too_many_arguments)]
struct LiveGapAdaptiveLowGapInput<'a> {
    config: &'a LiveGapAdaptiveLowGapConfig,
    run_id: Option<i64>,
    node_key: &'a str,
    market_slug: &'a str,
    outcome_label: &'a str,
    asset: &'a str,
    direction: &'a str,
    band: LiveGapDetailedGapBand,
    local_path_decision: &'a str,
    dead_activity_reason: Option<&'static str>,
    effective_fill: Option<f64>,
    remaining_sec: i64,
    live_gap_usd: f64,
    pre_required_gap_usd: f64,
    now_ms: i64,
}

static LIVE_GAP_ADAPTIVE_LOW_GAP_STATE: LazyLock<
    StdMutex<HashMap<String, LiveGapAdaptiveLowGapStateEntry>>,
> = LazyLock::new(|| StdMutex::new(HashMap::new()));
static LIVE_GAP_ADAPTIVE_LOW_GAP_NOTIFICATION_STATE: LazyLock<
    StdMutex<HashMap<String, LiveGapAdaptiveLowGapNotificationStateEntry>>,
> = LazyLock::new(|| StdMutex::new(HashMap::new()));

const LIVE_GAP_ADAPTIVE_LOW_GAP_STATE_TTL_MS: i64 = 2 * 60 * 60 * 1_000;
const LIVE_GAP_ADAPTIVE_LOW_GAP_MODE: &str = "active_direct_market_once_v1";
const LIVE_GAP_ADAPTIVE_LOW_GAP_CHANGE_EPSILON: f64 = 0.000001;

fn resolve_live_gap_adaptive_low_gap_config(node: &TradeFlowNode) -> LiveGapAdaptiveLowGapConfig {
    let mode_ok = node_config_string(node, "liveGapAdaptiveLowGapMode")
        .map(|mode| {
            mode.trim()
                .eq_ignore_ascii_case(LIVE_GAP_ADAPTIVE_LOW_GAP_MODE)
        })
        .unwrap_or(true);
    LiveGapAdaptiveLowGapConfig {
        enabled: node_config_bool(node, "liveGapAdaptiveLowGapEnabled").unwrap_or(false) && mode_ok,
        trigger_count: node_config_i64(node, "liveGapAdaptiveLowGapTriggerCount")
            .unwrap_or(1)
            .clamp(1, 100),
        step_pct: node_config_f64(node, "liveGapAdaptiveLowGapStepPct")
            .unwrap_or(0.05)
            .clamp(0.0, 0.50),
        max_relax_pct: node_config_f64(node, "liveGapAdaptiveLowGapMaxRelaxPct")
            .unwrap_or(0.05)
            .clamp(0.0, 0.50),
        max_shortfall_pct: node_config_f64(node, "liveGapAdaptiveLowGapMaxShortfallPct")
            .unwrap_or(0.20)
            .clamp(0.0, 1.0),
        max_fill_price: (node_config_f64(node, "liveGapAdaptiveLowGapMaxFillCent").unwrap_or(90.0)
            / 100.0)
            .clamp(0.01, 0.99),
        min_remaining_sec: node_config_i64(node, "liveGapAdaptiveLowGapMinRemainingSec")
            .unwrap_or(35)
            .clamp(0, 300),
        require_local_path_clean: node_config_bool(
            node,
            "liveGapAdaptiveLowGapRequireLocalPathClean",
        )
        .unwrap_or(true),
    }
}

fn live_gap_adaptive_low_gap_config_from_metadata(metadata: &Value) -> LiveGapAdaptiveLowGapConfig {
    LiveGapAdaptiveLowGapConfig {
        enabled: metadata
            .pointer("/resolved_guard_config/adaptiveLowGap/enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && metadata
                .pointer("/resolved_guard_config/adaptiveLowGap/mode")
                .and_then(Value::as_str)
                .map(|mode| {
                    mode.trim()
                        .eq_ignore_ascii_case(LIVE_GAP_ADAPTIVE_LOW_GAP_MODE)
                })
                .unwrap_or(true),
        trigger_count: metadata_i64(
            metadata,
            "/resolved_guard_config/adaptiveLowGap/triggerCount",
            1,
        )
        .clamp(1, 100),
        step_pct: metadata_f64(
            metadata,
            "/resolved_guard_config/adaptiveLowGap/stepPct",
            0.05,
        )
        .clamp(0.0, 0.50),
        max_relax_pct: metadata_f64(
            metadata,
            "/resolved_guard_config/adaptiveLowGap/maxRelaxPct",
            0.05,
        )
        .clamp(0.0, 0.50),
        max_shortfall_pct: metadata_f64(
            metadata,
            "/resolved_guard_config/adaptiveLowGap/maxShortfallPct",
            0.20,
        )
        .clamp(0.0, 1.0),
        max_fill_price: (metadata_f64(
            metadata,
            "/resolved_guard_config/adaptiveLowGap/maxFillCent",
            90.0,
        ) / 100.0)
            .clamp(0.01, 0.99),
        min_remaining_sec: metadata_i64(
            metadata,
            "/resolved_guard_config/adaptiveLowGap/minRemainingSec",
            35,
        )
        .clamp(0, 300),
        require_local_path_clean: metadata
            .pointer("/resolved_guard_config/adaptiveLowGap/requireLocalPathClean")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    }
}

fn live_gap_adaptive_low_gap_config_snapshot(config: &LiveGapAdaptiveLowGapConfig) -> Value {
    json!({
        "enabled": config.enabled,
        "mode": LIVE_GAP_ADAPTIVE_LOW_GAP_MODE,
        "triggerCount": config.trigger_count,
        "stepPct": config.step_pct,
        "maxRelaxPct": config.max_relax_pct,
        "maxShortfallPct": config.max_shortfall_pct,
        "maxFillCent": config.max_fill_price * 100.0,
        "minRemainingSec": config.min_remaining_sec,
        "requireLocalPathClean": config.require_local_path_clean,
    })
}

fn live_gap_adaptive_low_gap_price_bucket(fill_price: Option<f64>) -> &'static str {
    let Some(fill_price) = fill_price.filter(|value| value.is_finite()) else {
        return "unknown";
    };
    if fill_price < 0.80 {
        "lt_80"
    } else if fill_price < 0.85 {
        "80_84"
    } else if fill_price < 0.90 {
        "85_89"
    } else {
        "90_plus"
    }
}

fn live_gap_adaptive_low_gap_band_eligible(band: LiveGapDetailedGapBand) -> bool {
    matches!(
        band,
        LiveGapDetailedGapBand::UltraClean
            | LiveGapDetailedGapBand::LowClean
            | LiveGapDetailedGapBand::MildClean
    )
}

#[allow(clippy::too_many_arguments)]
fn live_gap_adaptive_low_gap_key(
    run_id: Option<i64>,
    node_key: &str,
    market_slug: &str,
    outcome_label: &str,
    asset: &str,
    direction: &str,
    band: LiveGapDetailedGapBand,
    price_bucket: &str,
) -> String {
    format!(
        "run={}|node={}|market={}|outcome={}|asset={}|direction={}|band={}|price_bucket={}",
        run_id.unwrap_or_default(),
        node_key,
        market_slug,
        outcome_label,
        asset.to_ascii_lowercase(),
        direction.to_ascii_lowercase(),
        band.label(),
        price_bucket
    )
}

fn live_gap_adaptive_low_gap_count(key: &str, now_ms: i64) -> i64 {
    let mut state = LIVE_GAP_ADAPTIVE_LOW_GAP_STATE
        .lock()
        .expect("adaptive low gap state lock");
    state.retain(|_, entry| {
        now_ms.saturating_sub(entry.updated_at_ms) <= LIVE_GAP_ADAPTIVE_LOW_GAP_STATE_TTL_MS
    });
    state.get(key).map(|entry| entry.count).unwrap_or_default()
}

fn live_gap_adaptive_low_gap_evaluate(
    input: LiveGapAdaptiveLowGapInput<'_>,
) -> LiveGapAdaptiveLowGapEvaluation {
    let price_bucket = live_gap_adaptive_low_gap_price_bucket(input.effective_fill);
    let key = live_gap_adaptive_low_gap_key(
        input.run_id,
        input.node_key,
        input.market_slug,
        input.outcome_label,
        input.asset,
        input.direction,
        input.band,
        price_bucket,
    );
    let pre_required = input.pre_required_gap_usd.max(0.0);
    let shortfall_usd = (pre_required - input.live_gap_usd).max(0.0);
    let shortfall_pct = if pre_required > 0.0 {
        shortfall_usd / pre_required
    } else {
        0.0
    };
    let mut reason = "waiting_for_market_near_miss";
    let mut base_can_record_near_miss = true;
    if !input.config.enabled {
        reason = "adaptive_disabled";
        base_can_record_near_miss = false;
    } else if !live_gap_adaptive_low_gap_band_eligible(input.band) {
        reason = "band_not_eligible";
        base_can_record_near_miss = false;
    } else if input.config.require_local_path_clean && input.local_path_decision != "clean" {
        reason = "local_path_not_clean";
        base_can_record_near_miss = false;
    } else if input.remaining_sec < input.config.min_remaining_sec {
        reason = "remaining_below_adaptive_min";
        base_can_record_near_miss = false;
    } else if input.dead_activity_reason.is_some() {
        reason = "dead_activity";
        base_can_record_near_miss = false;
    } else if input.live_gap_usd < pre_required && shortfall_pct > input.config.max_shortfall_pct {
        reason = "shortfall_too_large";
        base_can_record_near_miss = false;
    }
    let fill_allows_low_gap_near_miss = input
        .effective_fill
        .is_some_and(|price| price.is_finite() && price <= input.config.max_fill_price);
    let can_record_near_miss = base_can_record_near_miss && fill_allows_low_gap_near_miss;
    if base_can_record_near_miss && !fill_allows_low_gap_near_miss {
        reason = if input.effective_fill.is_some() {
            "fill_above_adaptive_max"
        } else {
            "fill_unavailable_for_adaptive"
        };
    }
    let can_record_guard_miss_near_miss = base_can_record_near_miss;
    let near_miss_count = live_gap_adaptive_low_gap_count(&key, input.now_ms);
    let steps = if can_record_near_miss {
        near_miss_count / input.config.trigger_count.max(1)
    } else {
        0
    };
    let relax_pct = (steps as f64 * input.config.step_pct).min(input.config.max_relax_pct);
    let adaptive_required = (pre_required * (1.0 - relax_pct)).max(0.0);
    let saved_from_block =
        input.live_gap_usd < pre_required && input.live_gap_usd >= adaptive_required;
    let status = if relax_pct > 0.0 && can_record_near_miss {
        reason = if saved_from_block {
            "near_miss_relax_saved_from_block"
        } else {
            "near_miss_relax_applied"
        };
        "applied"
    } else {
        "not_applied"
    };
    LiveGapAdaptiveLowGapEvaluation {
        status,
        scope: "market_once",
        key,
        price_bucket,
        market_near_miss_count: near_miss_count,
        trigger_count: input.config.trigger_count,
        pre_required_gap_usd: pre_required,
        adaptive_required_gap_usd: adaptive_required,
        relax_pct,
        shortfall_usd,
        shortfall_pct,
        saved_from_block,
        reason,
        can_record_near_miss,
        can_record_guard_miss_near_miss,
    }
}

fn live_gap_append_adaptive_low_gap_evaluation(
    payload: &mut serde_json::Map<String, Value>,
    evaluation: &LiveGapAdaptiveLowGapEvaluation,
) {
    payload.insert(
        "adaptive_low_gap".to_string(),
        json!({
            "status": evaluation.status,
            "scope": evaluation.scope,
            "key": evaluation.key,
            "price_bucket": evaluation.price_bucket,
            "market_near_miss_count": evaluation.market_near_miss_count,
            "trigger_count": evaluation.trigger_count,
            "pre_required_gap_usd": evaluation.pre_required_gap_usd,
            "adaptive_required_gap_usd": evaluation.adaptive_required_gap_usd,
            "relax_pct": evaluation.relax_pct,
            "shortfall_usd": evaluation.shortfall_usd,
            "shortfall_pct": evaluation.shortfall_pct,
            "saved_from_block": evaluation.saved_from_block,
            "reason": evaluation.reason,
            "can_record_near_miss": evaluation.can_record_near_miss,
            "can_record_guard_miss_near_miss": evaluation.can_record_guard_miss_near_miss,
        }),
    );
    payload.insert(
        "adaptive_low_gap_status".to_string(),
        json!(evaluation.status),
    );
    payload.insert(
        "adaptive_low_gap_scope".to_string(),
        json!(evaluation.scope),
    );
    payload.insert("adaptive_low_gap_key".to_string(), json!(evaluation.key));
    payload.insert(
        "adaptive_low_gap_market_near_miss_count".to_string(),
        json!(evaluation.market_near_miss_count),
    );
    payload.insert(
        "pre_adaptive_required_gap_usd".to_string(),
        json!(evaluation.pre_required_gap_usd),
    );
    payload.insert(
        "adaptive_required_gap_usd".to_string(),
        json!(evaluation.adaptive_required_gap_usd),
    );
    payload.insert(
        "required_gap_usd".to_string(),
        json!(evaluation.adaptive_required_gap_usd),
    );
    payload.insert(
        "final_required_gap_usd".to_string(),
        json!(evaluation.adaptive_required_gap_usd),
    );
    payload.insert(
        "adaptive_low_gap_relax_pct".to_string(),
        json!(evaluation.relax_pct),
    );
    payload.insert(
        "adaptive_low_gap_shortfall_usd".to_string(),
        json!(evaluation.shortfall_usd),
    );
    payload.insert(
        "adaptive_low_gap_shortfall_pct".to_string(),
        json!(evaluation.shortfall_pct),
    );
    payload.insert(
        "adaptive_saved_from_block".to_string(),
        json!(evaluation.saved_from_block),
    );
    payload.insert(
        "adaptive_low_gap_reason".to_string(),
        json!(evaluation.reason),
    );
    payload.insert(
        "adaptive_low_gap_can_record_near_miss".to_string(),
        json!(evaluation.can_record_near_miss),
    );
    payload.insert(
        "adaptive_low_gap_can_record_guard_miss_near_miss".to_string(),
        json!(evaluation.can_record_guard_miss_near_miss),
    );
}

fn live_gap_adaptive_low_gap_reason_can_record(
    reason_code: &str,
    can_record_near_miss: bool,
    can_record_guard_miss_near_miss: bool,
) -> bool {
    match reason_code {
        "live_gap_below_required" => can_record_near_miss,
        "above_max_price" | "best_ask_unavailable" => can_record_guard_miss_near_miss,
        _ => false,
    }
}

fn live_gap_record_adaptive_low_gap_near_miss_with_key(
    payload: &mut Value,
    key: &str,
    can_record: bool,
    previous_count: i64,
    reason_code: &str,
    now_ms: i64,
) {
    let mut recorded = false;
    let mut deduped = false;
    let mut count = previous_count;
    if can_record {
        let mut state = LIVE_GAP_ADAPTIVE_LOW_GAP_STATE
            .lock()
            .expect("adaptive low gap state lock");
        if let Some(entry) = state.get(key) {
            deduped = true;
            count = entry.count;
        } else {
            state.insert(
                key.to_string(),
                LiveGapAdaptiveLowGapStateEntry {
                    count: 1,
                    updated_at_ms: now_ms,
                },
            );
            recorded = true;
            count = 1;
        }
    }
    if let Some(obj) = payload.as_object_mut() {
        obj.insert(
            "adaptive_low_gap_near_miss_recorded".to_string(),
            json!(recorded),
        );
        obj.insert(
            "adaptive_low_gap_near_miss_deduped".to_string(),
            json!(deduped),
        );
        obj.insert(
            "adaptive_low_gap_market_near_miss_count".to_string(),
            json!(count),
        );
        obj.insert(
            "adaptive_low_gap_near_miss_reason".to_string(),
            json!(reason_code),
        );
        if let Some(adaptive) = obj
            .get_mut("adaptive_low_gap")
            .and_then(Value::as_object_mut)
        {
            adaptive.insert("near_miss_recorded".to_string(), json!(recorded));
            adaptive.insert("near_miss_deduped".to_string(), json!(deduped));
            adaptive.insert("market_near_miss_count".to_string(), json!(count));
            adaptive.insert("near_miss_reason".to_string(), json!(reason_code));
        }
    }
}

fn live_gap_record_adaptive_low_gap_near_miss(
    payload: &mut Value,
    evaluation: &LiveGapAdaptiveLowGapEvaluation,
    reason_code: &str,
    now_ms: i64,
) {
    let can_record = live_gap_adaptive_low_gap_reason_can_record(
        reason_code,
        evaluation.can_record_near_miss,
        evaluation.can_record_guard_miss_near_miss,
    );
    live_gap_record_adaptive_low_gap_near_miss_with_key(
        payload,
        &evaluation.key,
        can_record,
        evaluation.market_near_miss_count,
        reason_code,
        now_ms,
    );
}

fn live_gap_record_adaptive_low_gap_near_miss_from_payload(
    payload: &mut Value,
    reason_code: &str,
    now_ms: i64,
) {
    let Some(key) = payload
        .get("adaptive_low_gap_key")
        .and_then(Value::as_str)
        .or_else(|| {
            payload
                .pointer("/adaptive_low_gap/key")
                .and_then(Value::as_str)
        })
        .map(str::to_string)
    else {
        return;
    };
    let can_record_near_miss = payload
        .get("adaptive_low_gap_can_record_near_miss")
        .and_then(Value::as_bool)
        .or_else(|| {
            payload
                .pointer("/adaptive_low_gap/can_record_near_miss")
                .and_then(Value::as_bool)
        })
        .unwrap_or(false);
    let can_record_guard_miss_near_miss = payload
        .get("adaptive_low_gap_can_record_guard_miss_near_miss")
        .and_then(Value::as_bool)
        .or_else(|| {
            payload
                .pointer("/adaptive_low_gap/can_record_guard_miss_near_miss")
                .and_then(Value::as_bool)
        })
        .unwrap_or(false);
    let count = payload
        .get("adaptive_low_gap_market_near_miss_count")
        .and_then(value_as_i64)
        .or_else(|| {
            payload
                .pointer("/adaptive_low_gap/market_near_miss_count")
                .and_then(value_as_i64)
        })
        .unwrap_or_default();
    let can_record = live_gap_adaptive_low_gap_reason_can_record(
        reason_code,
        can_record_near_miss,
        can_record_guard_miss_near_miss,
    );
    live_gap_record_adaptive_low_gap_near_miss_with_key(
        payload,
        &key,
        can_record,
        count,
        reason_code,
        now_ms,
    );
}

fn live_gap_mark_adaptive_low_gap_change_notified_for_values(
    key: &str,
    relax_pct: f64,
    adaptive_required: f64,
    now_ms: i64,
) -> Option<LiveGapAdaptiveLowGapChangeNotification> {
    if key.trim().is_empty() {
        return None;
    }
    if !relax_pct.is_finite() || relax_pct <= 0.0 {
        return None;
    }
    if !adaptive_required.is_finite() {
        return None;
    }
    let mut state = LIVE_GAP_ADAPTIVE_LOW_GAP_NOTIFICATION_STATE
        .lock()
        .expect("adaptive low gap notification state lock");
    state.retain(|_, entry| {
        now_ms.saturating_sub(entry.notified_at_ms) <= LIVE_GAP_ADAPTIVE_LOW_GAP_STATE_TTL_MS
    });
    if let Some(previous) = state.get(key) {
        let same_relax =
            (previous.relax_pct - relax_pct).abs() <= LIVE_GAP_ADAPTIVE_LOW_GAP_CHANGE_EPSILON;
        let same_required = (previous.adaptive_required_gap_usd - adaptive_required).abs()
            <= LIVE_GAP_ADAPTIVE_LOW_GAP_CHANGE_EPSILON;
        if same_relax && same_required {
            return None;
        }
        let change = LiveGapAdaptiveLowGapChangeNotification {
            previous_relax_pct: Some(previous.relax_pct),
            previous_adaptive_required_gap_usd: Some(previous.adaptive_required_gap_usd),
            new_relax_pct: relax_pct,
            new_adaptive_required_gap_usd: adaptive_required,
            notified_at_ms: now_ms,
        };
        state.insert(
            key.to_string(),
            LiveGapAdaptiveLowGapNotificationStateEntry {
                relax_pct,
                adaptive_required_gap_usd: adaptive_required,
                notified_at_ms: now_ms,
            },
        );
        return Some(change);
    }
    state.insert(
        key.to_string(),
        LiveGapAdaptiveLowGapNotificationStateEntry {
            relax_pct,
            adaptive_required_gap_usd: adaptive_required,
            notified_at_ms: now_ms,
        },
    );
    Some(LiveGapAdaptiveLowGapChangeNotification {
        previous_relax_pct: None,
        previous_adaptive_required_gap_usd: None,
        new_relax_pct: relax_pct,
        new_adaptive_required_gap_usd: adaptive_required,
        notified_at_ms: now_ms,
    })
}

fn live_gap_mark_adaptive_low_gap_change_notified(
    payload: &Value,
    now_ms: i64,
) -> Option<LiveGapAdaptiveLowGapChangeNotification> {
    let status = payload
        .get("adaptive_low_gap_status")
        .and_then(Value::as_str)?;
    if status != "applied" {
        return None;
    }
    let key = payload
        .get("adaptive_low_gap_key")
        .and_then(Value::as_str)?;
    let relax_pct = payload
        .get("adaptive_low_gap_relax_pct")
        .and_then(value_as_f64)?;
    let adaptive_required = payload
        .get("adaptive_required_gap_usd")
        .and_then(value_as_f64)?;
    live_gap_mark_adaptive_low_gap_change_notified_for_values(
        key,
        relax_pct,
        adaptive_required,
        now_ms,
    )
}

fn live_gap_mark_adaptive_low_gap_near_miss_change_notified(
    payload: &Value,
    now_ms: i64,
) -> Option<LiveGapAdaptiveLowGapChangeNotification> {
    if !payload
        .get("adaptive_low_gap_near_miss_recorded")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let key = payload
        .get("adaptive_low_gap_key")
        .and_then(Value::as_str)
        .or_else(|| {
            payload
                .pointer("/adaptive_low_gap/key")
                .and_then(Value::as_str)
        })?;
    let count = payload
        .get("adaptive_low_gap_market_near_miss_count")
        .and_then(value_as_i64)
        .or_else(|| {
            payload
                .pointer("/adaptive_low_gap/market_near_miss_count")
                .and_then(value_as_i64)
        })
        .unwrap_or_default()
        .max(0);
    let trigger_count = metadata_i64(
        payload,
        "/resolved_guard_config/adaptiveLowGap/triggerCount",
        1,
    )
    .clamp(1, 100);
    let step_pct = metadata_f64(
        payload,
        "/resolved_guard_config/adaptiveLowGap/stepPct",
        0.05,
    )
    .clamp(0.0, 0.50);
    let max_relax_pct = metadata_f64(
        payload,
        "/resolved_guard_config/adaptiveLowGap/maxRelaxPct",
        0.05,
    )
    .clamp(0.0, 0.50);
    let steps = count / trigger_count;
    let relax_pct = (steps as f64 * step_pct).min(max_relax_pct);
    let pre_required = payload
        .get("pre_adaptive_required_gap_usd")
        .and_then(value_as_f64)
        .or_else(|| {
            payload
                .pointer("/adaptive_low_gap/pre_required_gap_usd")
                .and_then(value_as_f64)
        })?
        .max(0.0);
    let adaptive_required = (pre_required * (1.0 - relax_pct)).max(0.0);
    live_gap_mark_adaptive_low_gap_change_notified_for_values(
        key,
        relax_pct,
        adaptive_required,
        now_ms,
    )
}

#[cfg(test)]
fn live_gap_adaptive_low_gap_reset_state() {
    LIVE_GAP_ADAPTIVE_LOW_GAP_STATE
        .lock()
        .expect("adaptive low gap state lock")
        .clear();
    LIVE_GAP_ADAPTIVE_LOW_GAP_NOTIFICATION_STATE
        .lock()
        .expect("adaptive low gap notification state lock")
        .clear();
}
