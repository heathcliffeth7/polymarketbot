#[derive(Debug, Clone, PartialEq)]
struct NoReversalEntryGuardConfig {
    enabled: bool,
    decision_mode: String,
    lookback_mode: String,
    precomputed_profiles_enabled: bool,
    allow_cold_profile_query: bool,
    baseline_floor_pct: f64,
    daily_fallback_floor_pct: f64,
    source_mismatch_buffer_usd: Option<f64>,
    source_mismatch_buffer_floor_ratio: f64,
    late_high_extra_buffer_usd: Option<f64>,
    freeze_per_market: bool,
    cache_ttl_sec: i64,
    profile_query_timeout_ms: i64,
    profile_lookup_timeout_ms: i64,
    prewarm_query_timeout_ms: i64,
    max_relax_pct_per_window: f64,
    max_tighten_pct_per_window: f64,
    soft_pass_on_insufficient_data: bool,
    use_local_path_fallback_on_missing_profile: bool,
    local_path_fallback_enabled: bool,
    local_path_lookback_ms: i64,
    local_path_lookback_source: String,
    local_path_min_history_ms: i64,
    local_path_gate_mode: String,
    local_path_fresh_retrace_window_ms: i64,
    local_path_fresh_max_drop_usd: f64,
    local_path_fresh_min_history_ms: i64,
    block_if_profile_missing_and_local_path_insufficient: bool,
    profile_missing_emergency_margin_enabled: bool,
    profile_missing_emergency_margin_floor_ratio: f64,
    ptb_floor_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
struct NoReversalEntryGuardInput<'a> {
    market_slug: &'a str,
    token_id: &'a str,
    outcome_label: &'a str,
    definition_id: i64,
    node_key: &'a str,
    asset: &'a str,
    direction: &'a str,
    remaining_sec: i64,
    effective_fill: f64,
    current_live_gap: f64,
    regime: &'a str,
    slope_bucket: &'a str,
}

#[derive(Debug, Clone, PartialEq)]
struct NoReversalEntryGuardDecision {
    passed: bool,
    reason_code: &'static str,
    payload: Value,
}

#[derive(Debug, Clone)]
struct NoReversalCachedProfile {
    created_at_ms: i64,
    profile: NoReversalResolvedProfile,
}

#[derive(Debug, Clone)]
struct NoReversalResolvedProfile {
    selected_adverse: f64,
    raw_selected_adverse: f64,
    clamp_applied: bool,
    previous_selected: Option<f64>,
    selection: NoReversalSelection,
    fallback_level: NoReversalFallbackLevel,
    stats: Vec<NoReversalLookbackStat>,
}

#[derive(Debug, Clone)]
struct NoReversalProfileQuery {
    market_slug: String,
    target_window_start: Option<DateTime<Utc>>,
    definition_id: i64,
    node_key: String,
    profile_config_hash: String,
    asset: String,
    direction: String,
    slope_bucket: String,
    remaining_bucket: NoReversalBucket,
    price_bucket: NoReversalBucket,
    gap_bucket: NoReversalBucket,
    quantile: f64,
    high_late: bool,
}

#[derive(Debug, Clone)]
struct NoReversalProfileLookup {
    profile: Option<NoReversalResolvedProfile>,
    last_stats: Vec<NoReversalLookbackStat>,
    last_fallback: NoReversalFallbackLevel,
    stats_source: &'static str,
    bulk_query_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
struct NoReversalLookbackWindow {
    name: &'static str,
    hours: i64,
    min_samples: i64,
    min_markets: i64,
}

#[derive(Debug, Clone, PartialEq)]
struct NoReversalLookbackStat {
    name: &'static str,
    hours: i64,
    min_samples: i64,
    min_markets: i64,
    adverse_quantile: Option<f64>,
    sample_count: i64,
    market_count: i64,
    valid: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct NoReversalBucket {
    label: String,
    min: f64,
    max: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct NoReversalSelection {
    selected_adverse: f64,
    recent_risk: Option<f64>,
    session_risk: Option<f64>,
    session_source: Option<&'static str>,
    baseline_floor: Option<f64>,
    baseline_source: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoReversalFallbackLevel {
    Exact,
    SlopeRelaxed,
    GapRelaxed,
}

static NO_REVERSAL_ENTRY_GUARD_CACHE: LazyLock<StdMutex<HashMap<String, NoReversalCachedProfile>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

static NO_REVERSAL_PREVIOUS_SELECTED: LazyLock<StdMutex<HashMap<String, f64>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

static NO_REVERSAL_PROFILE_WARMUPS: LazyLock<
    StdMutex<HashMap<String, NoReversalProfilePrewarmPriority>>,
> = LazyLock::new(|| StdMutex::new(HashMap::new()));

const NO_REVERSAL_DECISION_MODE_HISTORICAL_ADAPTIVE: &str = "historical_adaptive";
const NO_REVERSAL_DECISION_MODE_LOCAL_PATH_ONLY: &str = "local_path_only";

const NO_REVERSAL_LOOKBACK_WINDOWS: [NoReversalLookbackWindow; 5] = [
    NoReversalLookbackWindow {
        name: "3h",
        hours: 3,
        min_samples: 80,
        min_markets: 20,
    },
    NoReversalLookbackWindow {
        name: "6h",
        hours: 6,
        min_samples: 120,
        min_markets: 35,
    },
    NoReversalLookbackWindow {
        name: "12h",
        hours: 12,
        min_samples: 180,
        min_markets: 50,
    },
    NoReversalLookbackWindow {
        name: "1d",
        hours: 24,
        min_samples: 250,
        min_markets: 80,
    },
    NoReversalLookbackWindow {
        name: "14d",
        hours: 336,
        min_samples: 500,
        min_markets: 150,
    },
];

fn resolve_no_reversal_entry_guard_config(node: &TradeFlowNode) -> NoReversalEntryGuardConfig {
    NoReversalEntryGuardConfig {
        enabled: node_config_bool(node, "noReversalEntryGuardEnabled").unwrap_or(false),
        decision_mode: no_reversal_decision_mode_from_value(
            node_config_string(node, "noReversalDecisionMode").as_deref(),
        )
        .to_string(),
        lookback_mode: node_config_string(node, "noReversalLookbackMode")
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "multi_window_adaptive".to_string()),
        precomputed_profiles_enabled: node_config_bool(
            node,
            "noReversalPrecomputedProfilesEnabled",
        )
        .unwrap_or(false),
        allow_cold_profile_query: node_config_bool(node, "noReversalAllowColdProfileQuery")
            .unwrap_or(true),
        baseline_floor_pct: node_config_f64(node, "noReversalBaselineFloorPct")
            .unwrap_or(0.80)
            .clamp(0.0, 1.0),
        daily_fallback_floor_pct: node_config_f64(node, "noReversalDailyFallbackFloorPct")
            .unwrap_or(0.70)
            .clamp(0.0, 1.0),
        source_mismatch_buffer_usd: node_config_f64(node, "noReversalSourceMismatchBufferUsd")
            .filter(|value| value.is_finite() && *value >= 0.0),
        source_mismatch_buffer_floor_ratio: node_config_f64(
            node,
            "noReversalSourceMismatchBufferFloorRatio",
        )
        .unwrap_or(0.15)
        .clamp(0.0, 1.0),
        late_high_extra_buffer_usd: node_config_f64(node, "noReversalLateHighExtraBufferUsd")
            .filter(|value| value.is_finite() && *value >= 0.0),
        freeze_per_market: node_config_bool(node, "noReversalFreezePerMarket").unwrap_or(true),
        cache_ttl_sec: node_config_i64(node, "noReversalCacheTtlSec")
            .unwrap_or(60)
            .clamp(1, 3_600),
        profile_query_timeout_ms: node_config_i64(node, "noReversalProfileQueryTimeoutMs")
            .unwrap_or(500)
            .clamp(50, 30_000),
        profile_lookup_timeout_ms: node_config_i64(node, "noReversalProfileLookupTimeoutMs")
            .unwrap_or(500)
            .clamp(10, 5_000),
        prewarm_query_timeout_ms: node_config_i64(node, "noReversalPrewarmQueryTimeoutMs")
            .unwrap_or(30_000)
            .clamp(500, 120_000),
        max_relax_pct_per_window: node_config_f64(node, "noReversalMaxRelaxPctPerWindow")
            .unwrap_or(0.20)
            .clamp(0.0, 1.0),
        max_tighten_pct_per_window: node_config_f64(node, "noReversalMaxTightenPctPerWindow")
            .unwrap_or(0.40)
            .clamp(0.0, 5.0),
        soft_pass_on_insufficient_data: node_config_bool(
            node,
            "noReversalSoftPassOnInsufficientData",
        )
        .unwrap_or(true),
        use_local_path_fallback_on_missing_profile: node_config_bool(
            node,
            "noReversalUseLocalPathFallbackOnMissingProfile",
        )
        .unwrap_or(false),
        local_path_fallback_enabled: node_config_bool(node, "noReversalLocalPathFallbackEnabled")
            .unwrap_or(false),
        local_path_lookback_ms: node_config_i64(node, "noReversalLocalPathLookbackMs")
            .unwrap_or(NO_REVERSAL_LOCAL_PATH_DEFAULT_WORKFLOW_LOOKBACK_MS)
            .clamp(1_000, NO_REVERSAL_LOCAL_PATH_MAX_LOOKBACK_MS),
        local_path_lookback_source: "node_config".to_string(),
        local_path_min_history_ms: node_config_i64(node, "noReversalLocalPathMinHistoryMs")
            .unwrap_or(30_000)
            .clamp(1_000, NO_REVERSAL_LOCAL_PATH_MAX_LOOKBACK_MS),
        local_path_gate_mode: no_reversal_local_path_gate_mode_from_value(node_config_string(node, "noReversalLocalPathGateMode").as_deref()).to_string(),
        local_path_fresh_retrace_window_ms: node_config_i64(node, "noReversalLocalPathFreshRetraceWindowMs").unwrap_or(10_000).clamp(1_000, 60_000),
        local_path_fresh_max_drop_usd: node_config_f64(node, "noReversalLocalPathFreshMaxDropUsd")
            .unwrap_or(5.0)
            .clamp(0.0, 100.0),
        local_path_fresh_min_history_ms: node_config_i64(node, "noReversalLocalPathFreshMinHistoryMs").unwrap_or(1_000).clamp(0, 60_000),
        block_if_profile_missing_and_local_path_insufficient: node_config_bool(
            node,
            "noReversalBlockIfProfileMissingAndLocalPathInsufficient",
        )
        .unwrap_or(false),
        profile_missing_emergency_margin_enabled: node_config_bool(
            node,
            "noReversalProfileMissingEmergencyMarginEnabled",
        )
        .unwrap_or(false),
        profile_missing_emergency_margin_floor_ratio: node_config_f64(
            node,
            "noReversalProfileMissingEmergencyMarginFloorRatio",
        )
        .unwrap_or(0.9)
        .clamp(0.0, 5.0),
        ptb_floor_usd: no_reversal_ptb_floor_from_node(node),
    }
}

fn no_reversal_ptb_floor_from_node(node: &TradeFlowNode) -> Option<f64> {
    let unit = resolve_action_place_order_ptb_stop_loss_gap_unit(node).ok()?;
    parse_action_place_order_ptb_stop_loss_rules(node.config.get("ptbStopLossRules"), unit)
        .ok()?
        .first()
        .map(|rule| rule.gap_usd)
        .filter(|value| value.is_finite())
}

fn no_reversal_entry_guard_config_snapshot(config: &NoReversalEntryGuardConfig) -> Value {
    json!({
        "enabled": config.enabled,
        "decisionMode": config.decision_mode,
        "lookbackMode": config.lookback_mode,
        "precomputedProfilesEnabled": config.precomputed_profiles_enabled,
        "allowColdProfileQuery": config.allow_cold_profile_query,
        "baselineFloorPct": config.baseline_floor_pct,
        "dailyFallbackFloorPct": config.daily_fallback_floor_pct,
        "sourceMismatchBufferUsd": config.source_mismatch_buffer_usd,
        "sourceMismatchBufferFloorRatio": config.source_mismatch_buffer_floor_ratio,
        "lateHighExtraBufferUsd": config.late_high_extra_buffer_usd,
        "freezePerMarket": config.freeze_per_market,
        "cacheTtlSec": config.cache_ttl_sec,
        "profileQueryTimeoutMs": config.profile_query_timeout_ms,
        "profileLookupTimeoutMs": config.profile_lookup_timeout_ms,
        "prewarmQueryTimeoutMs": config.prewarm_query_timeout_ms,
        "maxRelaxPctPerWindow": config.max_relax_pct_per_window,
        "maxTightenPctPerWindow": config.max_tighten_pct_per_window,
        "softPassOnInsufficientData": config.soft_pass_on_insufficient_data,
        "useLocalPathFallbackOnMissingProfile": config.use_local_path_fallback_on_missing_profile,
        "localPathFallbackEnabled": config.local_path_fallback_enabled,
        "localPathLookbackMs": config.local_path_lookback_ms,
        "localPathLookbackSource": config.local_path_lookback_source,
        "localPathMinHistoryMs": config.local_path_min_history_ms,
        "localPathGateMode": config.local_path_gate_mode,
        "localPathFreshRetraceWindowMs": config.local_path_fresh_retrace_window_ms,
        "localPathFreshMaxDropUsd": config.local_path_fresh_max_drop_usd,
        "localPathFreshMinHistoryMs": config.local_path_fresh_min_history_ms,
        "blockIfProfileMissingAndLocalPathInsufficient": config.block_if_profile_missing_and_local_path_insufficient,
        "profileMissingEmergencyMarginEnabled": config.profile_missing_emergency_margin_enabled,
        "profileMissingEmergencyMarginFloorRatio": config.profile_missing_emergency_margin_floor_ratio,
        "ptbFloorUsd": config.ptb_floor_usd,
    })
}

fn no_reversal_entry_guard_config_from_metadata(metadata: &Value) -> NoReversalEntryGuardConfig {
    let cfg = metadata.pointer("/resolved_guard_config/noReversalEntryGuard");
    let f64_at = |key: &str| cfg.and_then(|value| value.get(key)).and_then(value_as_f64);
    let i64_at = |key: &str| cfg.and_then(|value| value.get(key)).and_then(value_as_i64);
    let bool_at = |key: &str| {
        cfg.and_then(|value| value.get(key))
            .and_then(Value::as_bool)
    };
    NoReversalEntryGuardConfig {
        enabled: bool_at("enabled").unwrap_or(false),
        decision_mode: no_reversal_decision_mode_from_value(
            cfg.and_then(|value| value.get("decisionMode"))
                .and_then(Value::as_str),
        )
        .to_string(),
        lookback_mode: cfg
            .and_then(|value| value.get("lookbackMode"))
            .and_then(Value::as_str)
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "multi_window_adaptive".to_string()),
        precomputed_profiles_enabled: bool_at("precomputedProfilesEnabled").unwrap_or(false),
        allow_cold_profile_query: bool_at("allowColdProfileQuery").unwrap_or(true),
        baseline_floor_pct: f64_at("baselineFloorPct").unwrap_or(0.80).clamp(0.0, 1.0),
        daily_fallback_floor_pct: f64_at("dailyFallbackFloorPct")
            .unwrap_or(0.70)
            .clamp(0.0, 1.0),
        source_mismatch_buffer_usd: f64_at("sourceMismatchBufferUsd")
            .filter(|value| value.is_finite() && *value >= 0.0),
        source_mismatch_buffer_floor_ratio: f64_at("sourceMismatchBufferFloorRatio")
            .unwrap_or(0.15)
            .clamp(0.0, 1.0),
        late_high_extra_buffer_usd: f64_at("lateHighExtraBufferUsd")
            .filter(|value| value.is_finite() && *value >= 0.0),
        freeze_per_market: bool_at("freezePerMarket").unwrap_or(true),
        cache_ttl_sec: i64_at("cacheTtlSec").unwrap_or(60).clamp(1, 3_600),
        profile_query_timeout_ms: i64_at("profileQueryTimeoutMs")
            .unwrap_or(500)
            .clamp(50, 30_000),
        profile_lookup_timeout_ms: i64_at("profileLookupTimeoutMs")
            .unwrap_or(500)
            .clamp(10, 5_000),
        prewarm_query_timeout_ms: i64_at("prewarmQueryTimeoutMs")
            .unwrap_or(30_000)
            .clamp(500, 120_000),
        max_relax_pct_per_window: f64_at("maxRelaxPctPerWindow")
            .unwrap_or(0.20)
            .clamp(0.0, 1.0),
        max_tighten_pct_per_window: f64_at("maxTightenPctPerWindow")
            .unwrap_or(0.40)
            .clamp(0.0, 5.0),
        soft_pass_on_insufficient_data: bool_at("softPassOnInsufficientData").unwrap_or(true),
        use_local_path_fallback_on_missing_profile: bool_at("useLocalPathFallbackOnMissingProfile")
            .unwrap_or(false),
        local_path_fallback_enabled: bool_at("localPathFallbackEnabled").unwrap_or(false),
        local_path_lookback_ms: i64_at("localPathLookbackMs")
            .unwrap_or(NO_REVERSAL_LOCAL_PATH_DEFAULT_WORKFLOW_LOOKBACK_MS)
            .clamp(1_000, NO_REVERSAL_LOCAL_PATH_MAX_LOOKBACK_MS),
        local_path_lookback_source: cfg
            .and_then(|value| value.get("localPathLookbackSource"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("metadata")
            .to_string(),
        local_path_min_history_ms: i64_at("localPathMinHistoryMs")
            .unwrap_or(30_000)
            .clamp(1_000, NO_REVERSAL_LOCAL_PATH_MAX_LOOKBACK_MS),
        local_path_gate_mode: no_reversal_local_path_gate_mode_from_value(cfg.and_then(|value| value.get("localPathGateMode")).and_then(Value::as_str)).to_string(),
        local_path_fresh_retrace_window_ms: i64_at("localPathFreshRetraceWindowMs")
            .unwrap_or(10_000)
            .clamp(1_000, 60_000),
        local_path_fresh_max_drop_usd: f64_at("localPathFreshMaxDropUsd")
            .unwrap_or(5.0)
            .clamp(0.0, 100.0),
        local_path_fresh_min_history_ms: i64_at("localPathFreshMinHistoryMs")
            .unwrap_or(1_000)
            .clamp(0, 60_000),
        block_if_profile_missing_and_local_path_insufficient: bool_at(
            "blockIfProfileMissingAndLocalPathInsufficient",
        )
        .unwrap_or(false),
        profile_missing_emergency_margin_enabled: bool_at("profileMissingEmergencyMarginEnabled")
            .unwrap_or(false),
        profile_missing_emergency_margin_floor_ratio: f64_at(
            "profileMissingEmergencyMarginFloorRatio",
        )
        .unwrap_or(0.9)
        .clamp(0.0, 5.0),
        ptb_floor_usd: f64_at("ptbFloorUsd").filter(|value| value.is_finite()),
    }
}

fn no_reversal_decision_mode_from_value(value: Option<&str>) -> &'static str {
    match value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(NO_REVERSAL_DECISION_MODE_HISTORICAL_ADAPTIVE)
    {
        NO_REVERSAL_DECISION_MODE_LOCAL_PATH_ONLY => NO_REVERSAL_DECISION_MODE_LOCAL_PATH_ONLY,
        _ => NO_REVERSAL_DECISION_MODE_HISTORICAL_ADAPTIVE,
    }
}

fn no_reversal_remaining_bucket(remaining_sec: i64) -> NoReversalBucket {
    match remaining_sec {
        i64::MIN..=19 => NoReversalBucket {
            label: "0_20".to_string(),
            min: 0.0,
            max: 20.0,
        },
        20..=29 => NoReversalBucket {
            label: "20_30".to_string(),
            min: 20.0,
            max: 30.0,
        },
        30..=44 => NoReversalBucket {
            label: "30_45".to_string(),
            min: 30.0,
            max: 45.0,
        },
        45..=59 => NoReversalBucket {
            label: "45_60".to_string(),
            min: 45.0,
            max: 60.0,
        },
        60..=89 => NoReversalBucket {
            label: "60_90".to_string(),
            min: 60.0,
            max: 90.0,
        },
        _ => NoReversalBucket {
            label: "90_plus".to_string(),
            min: 90.0,
            max: 900.0,
        },
    }
}

fn no_reversal_price_bucket(price: f64) -> NoReversalBucket {
    if price < 0.80 {
        NoReversalBucket {
            label: "lt_80".to_string(),
            min: 0.0,
            max: 0.80,
        }
    } else if price < 0.85 {
        NoReversalBucket {
            label: "80_84".to_string(),
            min: 0.80,
            max: 0.85,
        }
    } else if price < 0.90 {
        NoReversalBucket {
            label: "85_89".to_string(),
            min: 0.85,
            max: 0.90,
        }
    } else if price < 0.94 {
        NoReversalBucket {
            label: "90_93".to_string(),
            min: 0.90,
            max: 0.94,
        }
    } else {
        NoReversalBucket {
            label: "94_plus".to_string(),
            min: 0.94,
            max: 1.0,
        }
    }
}

fn no_reversal_gap_bucket(gap: f64) -> NoReversalBucket {
    let lower = (gap / 5.0).floor().clamp(-20.0, 40.0) * 5.0;
    let upper = lower + 5.0;
    NoReversalBucket {
        label: format!("{lower:.0}_{upper:.0}"),
        min: lower,
        max: upper,
    }
}

fn no_reversal_slope_bucket_from_pre_buy_payload(payload: &Value) -> &'static str {
    match payload
        .get("gap_slope_3s_usd_per_sec")
        .or_else(|| payload.pointer("/history/gap_slope_3s_usd_per_sec"))
        .and_then(value_as_f64)
    {
        Some(slope) if slope < 0.0 => "negative",
        Some(_) => "non_negative",
        None => "unknown",
    }
}

#[cfg(test)]
fn no_reversal_adverse_move(entry_gap: f64, future_min_gap: f64) -> f64 {
    (entry_gap - future_min_gap).max(0.0)
}

fn no_reversal_high_late(input: &NoReversalEntryGuardInput<'_>) -> bool {
    input.effective_fill >= 0.90 || input.remaining_sec <= 30 || input.regime == "high_chop"
}

fn no_reversal_source_buffer(
    config: &NoReversalEntryGuardConfig,
    asset: &str,
    ptb_floor: f64,
    high_late: bool,
) -> f64 {
    let fixed = config.source_mismatch_buffer_usd.unwrap_or_else(|| {
        if asset.eq_ignore_ascii_case("btc") {
            2.0
        } else {
            0.0
        }
    });
    let base = fixed.max(ptb_floor.abs() * config.source_mismatch_buffer_floor_ratio);
    let extra = if high_late {
        config.late_high_extra_buffer_usd.unwrap_or_else(|| {
            if asset.eq_ignore_ascii_case("btc") {
                4.0
            } else {
                ptb_floor.abs() * 0.30
            }
        })
    } else {
        0.0
    };
    base + extra
}

fn no_reversal_fallback_label(level: NoReversalFallbackLevel) -> &'static str {
    match level {
        NoReversalFallbackLevel::Exact => "exact",
        NoReversalFallbackLevel::SlopeRelaxed => "slope_relaxed",
        NoReversalFallbackLevel::GapRelaxed => "gap_relaxed",
    }
}

fn no_reversal_fallback_from_label(label: Option<&str>) -> NoReversalFallbackLevel {
    match label.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "slope_relaxed" => NoReversalFallbackLevel::SlopeRelaxed,
        "gap_relaxed" => NoReversalFallbackLevel::GapRelaxed,
        _ => NoReversalFallbackLevel::Exact,
    }
}

fn no_reversal_stats_payload(stats: &[NoReversalLookbackStat]) -> Value {
    let mut obj = serde_json::Map::new();
    for stat in stats {
        obj.insert(
            stat.name.to_string(),
            json!({
                "hours": stat.hours,
                "p_quantile": stat.adverse_quantile,
                "samples": stat.sample_count,
                "markets": stat.market_count,
                "min_samples": stat.min_samples,
                "min_markets": stat.min_markets,
                "valid": stat.valid,
            }),
        );
    }
    Value::Object(obj)
}

fn no_reversal_stat_value(stats: &[NoReversalLookbackStat], name: &str) -> Option<f64> {
    stats
        .iter()
        .find(|stat| stat.name == name && stat.valid)
        .and_then(|stat| stat.adverse_quantile)
        .filter(|value| value.is_finite())
}

fn no_reversal_select_adverse(
    stats: &[NoReversalLookbackStat],
    config: &NoReversalEntryGuardConfig,
) -> Option<NoReversalSelection> {
    let p3 = no_reversal_stat_value(stats, "3h");
    let p6 = no_reversal_stat_value(stats, "6h");
    let p12 = no_reversal_stat_value(stats, "12h");
    let p1d = no_reversal_stat_value(stats, "1d");
    let p14d = no_reversal_stat_value(stats, "14d");
    let recent_risk = p3.into_iter().chain(p6).max_by(f64::total_cmp);
    let (session_risk, session_source) = p12
        .map(|value| (Some(value), Some("12h")))
        .unwrap_or_else(|| (p1d, p1d.map(|_| "1d_fallback")));
    let baseline_14d = p14d.map(|value| value * config.baseline_floor_pct);
    let baseline_1d = p1d.map(|value| value * config.daily_fallback_floor_pct);
    let (baseline_floor, baseline_source) = match (baseline_14d, baseline_1d) {
        (Some(a), Some(b)) if a >= b => (Some(a), Some("14d_floor")),
        (Some(_), Some(b)) => (Some(b), Some("1d_floor")),
        (Some(a), None) => (Some(a), Some("14d_floor")),
        (None, Some(b)) => (Some(b), Some("1d_floor")),
        (None, None) => (None, None),
    };
    let selected_adverse = recent_risk
        .into_iter()
        .chain(session_risk)
        .chain(baseline_floor)
        .max_by(f64::total_cmp)?;
    Some(NoReversalSelection {
        selected_adverse,
        recent_risk,
        session_risk,
        session_source,
        baseline_floor,
        baseline_source,
    })
}

fn no_reversal_apply_window_clamp(
    raw: f64,
    previous: Option<f64>,
    max_relax_pct: f64,
    max_tighten_pct: f64,
) -> (f64, bool, Option<f64>) {
    let Some(previous) = previous.filter(|value| value.is_finite() && *value > 0.0) else {
        return (raw, false, None);
    };
    let min_relaxed = previous * (1.0 - max_relax_pct.clamp(0.0, 1.0));
    let max_tightened = previous * (1.0 + max_tighten_pct.max(0.0));
    let clamped = raw.clamp(min_relaxed, max_tightened);
    (
        (clamped),
        (clamped - raw).abs() > f64::EPSILON,
        Some(previous),
    )
}

fn no_reversal_cache_key_from_query(
    config: &NoReversalEntryGuardConfig,
    query: &NoReversalProfileQuery,
) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{:.3}:{:.2}:{:.2}",
        query.market_slug,
        query.definition_id,
        query.node_key,
        query.profile_config_hash,
        query.asset,
        query.direction,
        query.remaining_bucket.label,
        query.price_bucket.label,
        query.gap_bucket.label,
        query.slope_bucket,
        query.high_late,
        query.quantile,
        config.baseline_floor_pct,
        config.daily_fallback_floor_pct,
    )
}

fn no_reversal_config_hash(config: &NoReversalEntryGuardConfig) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    no_reversal_entry_guard_config_snapshot(config)
        .to_string()
        .hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn no_reversal_profile_key_from_query(
    query: &NoReversalProfileQuery,
) -> Option<NoReversalAdverseProfileKey> {
    Some(NoReversalAdverseProfileKey {
        target_market_slug: query.market_slug.clone(),
        target_window_start: query.target_window_start?,
        definition_id: query.definition_id,
        node_key: query.node_key.clone(),
        profile_config_hash: query.profile_config_hash.clone(),
        asset: query.asset.clone(),
        direction: query.direction.clone(),
        remaining_bucket: query.remaining_bucket.label.clone(),
        price_bucket: query.price_bucket.label.clone(),
        gap_bucket: query.gap_bucket.label.clone(),
        slope_bucket: query.slope_bucket.clone(),
        quantile: query.quantile,
        high_late: query.high_late,
    })
}

fn no_reversal_previous_key_from_query(query: &NoReversalProfileQuery) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}",
        query.asset,
        query.direction,
        query.remaining_bucket.label,
        query.price_bucket.label,
        query.gap_bucket.label,
        query.high_late
    )
}

fn no_reversal_cached_profile(
    key: &str,
    config: &NoReversalEntryGuardConfig,
    now_ms: i64,
) -> Option<(NoReversalResolvedProfile, i64)> {
    let cache = NO_REVERSAL_ENTRY_GUARD_CACHE
        .lock()
        .expect("no-reversal cache");
    let cached = cache.get(key)?;
    let age_ms = (now_ms - cached.created_at_ms).max(0);
    if !config.freeze_per_market && age_ms > config.cache_ttl_sec * 1_000 {
        return None;
    }
    Some((cached.profile.clone(), age_ms))
}

fn no_reversal_store_cached_profile(key: String, now_ms: i64, profile: &NoReversalResolvedProfile) {
    let mut cache = NO_REVERSAL_ENTRY_GUARD_CACHE
        .lock()
        .expect("no-reversal cache");
    cache.insert(
        key,
        NoReversalCachedProfile {
            created_at_ms: now_ms,
            profile: profile.clone(),
        },
    );
    if cache.len() > 512 {
        cache.retain(|_, value| now_ms - value.created_at_ms <= 60 * 60 * 1_000);
    }
}

fn no_reversal_soft_pass(
    reason_code: &'static str,
    mut payload: Value,
) -> NoReversalEntryGuardDecision {
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("decision".to_string(), json!("pass"));
        obj.insert("reason".to_string(), json!(reason_code));
        obj.insert("reason_code".to_string(), json!(reason_code));
        obj.insert("protection".to_string(), json!("not_applied"));
    }
    NoReversalEntryGuardDecision {
        passed: true,
        reason_code,
        payload,
    }
}

fn no_reversal_block(
    reason_code: &'static str,
    mut payload: Value,
) -> NoReversalEntryGuardDecision {
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("decision".to_string(), json!("block_retry"));
        obj.insert("reason".to_string(), json!(reason_code));
        obj.insert("reason_code".to_string(), json!(reason_code));
    }
    NoReversalEntryGuardDecision {
        passed: false,
        reason_code,
        payload,
    }
}

#[allow(dead_code)]
async fn no_reversal_stats_for_fallback(
    repo: &PostgresRepository,
    query_profile: &NoReversalProfileQuery,
    fallback_level: NoReversalFallbackLevel,
    now: DateTime<Utc>,
) -> Result<Vec<NoReversalLookbackStat>> {
    let historical_until = query_profile.target_window_start.unwrap_or(now);
    let gap_filter = (fallback_level != NoReversalFallbackLevel::GapRelaxed)
        .then_some((query_profile.gap_bucket.min, query_profile.gap_bucket.max));
    let slope_filter = (fallback_level == NoReversalFallbackLevel::Exact)
        .then(|| query_profile.slope_bucket.clone());
    let mut stats = Vec::with_capacity(NO_REVERSAL_LOOKBACK_WINDOWS.len());
    for window in NO_REVERSAL_LOOKBACK_WINDOWS {
        let query = bot_infra::db::TradeBuilderAdverseMoveStatsQuery {
            asset: query_profile.asset.clone(),
            direction: query_profile.direction.clone(),
            current_market_slug: query_profile.market_slug.clone(),
            since: historical_until - ChronoDuration::hours(window.hours),
            until: historical_until,
            remaining_min_sec: query_profile.remaining_bucket.min,
            remaining_max_sec: query_profile.remaining_bucket.max,
            price_min: query_profile.price_bucket.min,
            price_max: query_profile.price_bucket.max,
            gap_min: gap_filter.map(|(min, _)| min),
            gap_max: gap_filter.map(|(_, max)| max),
            slope_bucket: slope_filter.clone(),
            quantile: query_profile.quantile,
        };
        let result = repo.trade_builder_adverse_move_stats(&query).await?;
        let valid = result.sample_count >= window.min_samples
            && result.market_count >= window.min_markets
            && result
                .adverse_quantile
                .is_some_and(|value| value.is_finite());
        stats.push(NoReversalLookbackStat {
            name: window.name,
            hours: window.hours,
            min_samples: window.min_samples,
            min_markets: window.min_markets,
            adverse_quantile: result.adverse_quantile,
            sample_count: result.sample_count,
            market_count: result.market_count,
            valid,
        });
    }
    Ok(stats)
}

async fn no_reversal_resolve_profile(
    repo: &PostgresRepository,
    config: &NoReversalEntryGuardConfig,
    query: &NoReversalProfileQuery,
    now: DateTime<Utc>,
) -> Result<NoReversalProfileLookup> {
    let mut last_stats = Vec::new();
    let mut last_fallback = NoReversalFallbackLevel::Exact;
    let feature_lookup = no_reversal_stats_by_fallback_from_features(repo, query, now).await?;
    let bulk_query_ms = feature_lookup.bulk_query_ms;
    for (fallback, stats) in feature_lookup.stats_by_fallback {
        last_fallback = fallback;
        if let Some(selection) = no_reversal_select_adverse(&stats, config) {
            let previous_key = no_reversal_previous_key_from_query(query);
            let previous = NO_REVERSAL_PREVIOUS_SELECTED
                .lock()
                .expect("no-reversal previous selected")
                .get(&previous_key)
                .copied();
            let (selected_adverse, clamp_applied, previous_selected) =
                no_reversal_apply_window_clamp(
                    selection.selected_adverse,
                    previous,
                    config.max_relax_pct_per_window,
                    config.max_tighten_pct_per_window,
                );
            NO_REVERSAL_PREVIOUS_SELECTED
                .lock()
                .expect("no-reversal previous selected")
                .insert(previous_key, selected_adverse);
            return Ok(NoReversalProfileLookup {
                profile: Some(NoReversalResolvedProfile {
                    selected_adverse,
                    raw_selected_adverse: selection.selected_adverse,
                    clamp_applied,
                    previous_selected,
                    selection,
                    fallback_level: fallback,
                    stats,
                }),
                last_stats: Vec::new(),
                last_fallback: fallback,
                stats_source: NO_REVERSAL_PROFILE_STATS_SOURCE_FEATURES,
                bulk_query_ms: Some(bulk_query_ms),
            });
        }
        last_stats = stats;
    }
    Ok(NoReversalProfileLookup {
        profile: None,
        last_stats,
        last_fallback,
        stats_source: NO_REVERSAL_PROFILE_STATS_SOURCE_FEATURES,
        bulk_query_ms: Some(bulk_query_ms),
    })
}

#[allow(clippy::too_many_arguments)]
fn no_reversal_decision_from_profile(
    config: &NoReversalEntryGuardConfig,
    input: &NoReversalEntryGuardInput<'_>,
    query: &NoReversalProfileQuery,
    ptb_floor: f64,
    source_buffer: f64,
    profile: &NoReversalResolvedProfile,
    cache_hit: bool,
    cache_age_ms: Option<i64>,
    query_ms: Option<i64>,
    profile_timeout: bool,
    profile_source: &'static str,
) -> NoReversalEntryGuardDecision {
    let worst_expected_gap = input.current_live_gap - profile.selected_adverse - source_buffer;
    let mut payload = json!({
        "enabled": config.enabled,
        "lookback_mode": config.lookback_mode,
        "selection_mode": "recent_max_with_session_and_baseline_floor",
        "market_slug": input.market_slug,
        "asset": input.asset,
        "direction": input.direction,
        "remaining_sec": input.remaining_sec,
        "effective_fill": input.effective_fill,
        "current_live_gap_usd": input.current_live_gap,
        "ptb_floor_usd": ptb_floor,
        "selected_adverse_usd": profile.selected_adverse,
        "raw_selected_adverse_usd": profile.raw_selected_adverse,
        "source_buffer_usd": source_buffer,
        "worst_expected_gap_usd": worst_expected_gap,
        "quantile": query.quantile,
        "high_late_profile": query.high_late,
        "fallback_level": no_reversal_fallback_label(profile.fallback_level),
        "profile_lookup_fallback_level": no_reversal_fallback_label(profile.fallback_level),
        "profile_lookup_status": "ready",
        "profile_lookup_key": no_reversal_profile_lookup_key_payload(query),
        "runtime_fallback_source": "historical_profile",
        "fallback_level_source": no_reversal_fallback_label(profile.fallback_level),
        "protection": "applied",
        "profile_source": profile_source,
        "cache_hit": cache_hit,
        "cache_age_ms": cache_age_ms,
        "cache_ttl_sec": config.cache_ttl_sec,
        "freeze_per_market": config.freeze_per_market,
        "no_reversal_profile_cache_hit": cache_hit,
        "no_reversal_profile_query_ms": query_ms,
        "no_reversal_profile_timeout": profile_timeout,
        "window_clamp": {
            "applied": profile.clamp_applied,
            "previous_selected_adverse_usd": profile.previous_selected,
            "max_relax_pct": config.max_relax_pct_per_window,
            "max_tighten_pct": config.max_tighten_pct_per_window,
        },
        "bucket": {
            "remaining_bucket": &query.remaining_bucket.label,
            "price_bucket": &query.price_bucket.label,
            "gap_bucket": &query.gap_bucket.label,
            "slope_bucket": &query.slope_bucket,
        },
        "lookbacks": no_reversal_stats_payload(&profile.stats),
        "selection": {
            "recent_risk": profile.selection.recent_risk,
            "session_risk": profile.selection.session_risk,
            "session_source": profile.selection.session_source,
            "baseline_floor": profile.selection.baseline_floor,
            "baseline_source": profile.selection.baseline_source,
        },
        "sources": {
            "historical": NO_REVERSAL_PROFILE_STATS_SOURCE_FEATURES,
            "live": "binance_live",
        },
    });
    if worst_expected_gap < ptb_floor {
        no_reversal_block("no_reversal_margin_too_low", payload)
    } else {
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("decision".to_string(), json!("pass"));
            obj.insert("reason".to_string(), json!("path_safe_gap_margin"));
            obj.insert("reason_code".to_string(), json!("path_safe_gap_margin"));
        }
        NoReversalEntryGuardDecision {
            passed: true,
            reason_code: "path_safe_gap_margin",
            payload,
        }
    }
}

fn no_reversal_unapplied_decision(
    config: &NoReversalEntryGuardConfig,
    reason_code: &'static str,
    payload: Value,
) -> NoReversalEntryGuardDecision {
    if config.soft_pass_on_insufficient_data {
        no_reversal_soft_pass(reason_code, payload)
    } else {
        no_reversal_block(reason_code, payload)
    }
}

fn no_reversal_local_path_primary_decision(
    config: &NoReversalEntryGuardConfig,
    input: &NoReversalEntryGuardInput<'_>,
    query: &NoReversalProfileQuery,
    ptb_floor: f64,
    source_buffer: f64,
) -> NoReversalEntryGuardDecision {
    let mut local_config = config.clone();
    local_config.precomputed_profiles_enabled = false;
    local_config.allow_cold_profile_query = false;
    local_config.soft_pass_on_insufficient_data = false;
    local_config.use_local_path_fallback_on_missing_profile = true;
    local_config.local_path_fallback_enabled = true;
    local_config.block_if_profile_missing_and_local_path_insufficient = true;

    let mut decision = no_reversal_local_path_decision(
        &local_config,
        input,
        query,
        ptb_floor,
        source_buffer,
        "disabled_by_local_path_only",
        "local_path_only",
    );
    if let Some(obj) = decision.payload.as_object_mut() {
        obj.insert(
            "decision_mode".to_string(),
            json!(NO_REVERSAL_DECISION_MODE_LOCAL_PATH_ONLY),
        );
        obj.insert(
            "profile_source".to_string(),
            json!("disabled_by_local_path_only"),
        );
        obj.insert("profile_reason".to_string(), json!("local_path_only"));
        obj.insert("cache_hit".to_string(), json!(false));
        obj.insert("no_reversal_profile_cache_hit".to_string(), json!(false));
        obj.insert("no_reversal_profile_query_ms".to_string(), Value::Null);
    }
    if decision.passed {
        decision.reason_code = "local_path_primary";
        if let Some(obj) = decision.payload.as_object_mut() {
            obj.insert("decision".to_string(), json!("pass"));
            obj.insert("reason".to_string(), json!("local_path_primary"));
            obj.insert("reason_code".to_string(), json!("local_path_primary"));
            obj.insert(
                "local_path_decision_reason".to_string(),
                json!("local_path_primary"),
            );
        }
    }
    decision
}

async fn evaluate_no_reversal_entry_guard(
    repo: &PostgresRepository,
    config: &NoReversalEntryGuardConfig,
    input: &NoReversalEntryGuardInput<'_>,
) -> NoReversalEntryGuardDecision {
    let now = Utc::now();
    let now_ms = now.timestamp_millis();
    let Some(ptb_floor) = config.ptb_floor_usd.filter(|value| value.is_finite()) else {
        return no_reversal_unapplied_decision(
            config,
            "ptb_floor_missing",
            json!({
                "enabled": config.enabled,
                "lookback_mode": config.lookback_mode,
                "market_slug": input.market_slug,
                "asset": input.asset,
                "direction": input.direction,
                "current_live_gap_usd": input.current_live_gap,
                "ptb_floor_usd": Value::Null,
                "no_reversal_profile_cache_hit": false,
                "no_reversal_profile_query_ms": Value::Null,
                "no_reversal_profile_timeout": false,
                "sources": { "historical": "chainlink_second_snapshot", "live": "binance_live" },
            }),
        );
    };
    let remaining_bucket = no_reversal_remaining_bucket(input.remaining_sec);
    let price_bucket = no_reversal_price_bucket(input.effective_fill);
    let gap_bucket = no_reversal_gap_bucket(input.current_live_gap);
    let high_late = no_reversal_high_late(input);
    let quantile = if high_late { 0.98 } else { 0.95 };
    let source_buffer = no_reversal_source_buffer(config, input.asset, ptb_floor, high_late);
    let target_window_start = MarketCycleId(input.market_slug.to_string()).start_time();
    let query = NoReversalProfileQuery {
        market_slug: input.market_slug.to_string(),
        target_window_start,
        definition_id: input.definition_id,
        node_key: input.node_key.to_string(),
        profile_config_hash: no_reversal_config_hash(config),
        asset: input.asset.to_string(),
        direction: input.direction.to_string(),
        slope_bucket: input.slope_bucket.to_string(),
        remaining_bucket,
        price_bucket,
        gap_bucket,
        quantile,
        high_late,
    };
    if config.decision_mode == NO_REVERSAL_DECISION_MODE_LOCAL_PATH_ONLY {
        return no_reversal_local_path_primary_decision(
            config,
            input,
            &query,
            ptb_floor,
            source_buffer,
        );
    }
    let cache_key = no_reversal_cache_key_from_query(config, &query);
    if let Some((profile, age_ms)) = no_reversal_cached_profile(&cache_key, config, now_ms) {
        return no_reversal_decision_from_profile(
            config,
            input,
            &query,
            ptb_floor,
            source_buffer,
            &profile,
            true,
            Some(age_ms),
            None,
            false,
            "memory_cache",
        );
    }

    if config.precomputed_profiles_enabled {
        let lookup_started = Instant::now();
        let profile_key = no_reversal_profile_key_from_query(&query);
        let table_lookup = if let Some(profile_key) = profile_key.as_ref() {
            tokio::time::timeout(
                Duration::from_millis(config.profile_lookup_timeout_ms as u64),
                repo.get_no_reversal_adverse_profile(&profile_key),
            )
            .await
        } else {
            Ok(Ok(None))
        };
        let lookup_ms = lookup_started.elapsed().as_millis().min(i64::MAX as u128) as i64;
        match table_lookup {
            Ok(Ok(Some(record))) if record.status == "ready" => {
                if let Some(profile) = no_reversal_profile_from_precomputed(&record) {
                    no_reversal_store_cached_profile(cache_key, now_ms, &profile);
                    let mut decision = no_reversal_decision_from_profile(
                        config,
                        input,
                        &query,
                        ptb_floor,
                        source_buffer,
                        &profile,
                        false,
                        None,
                        Some(lookup_ms),
                        false,
                        "precomputed_table",
                    );
                    if let Some(obj) = decision.payload.as_object_mut() {
                        obj.insert("profile_status".to_string(), json!(record.status));
                        obj.insert("prewarmer_status".to_string(), json!("ready"));
                        obj.insert("profile_as_of".to_string(), json!(record.profile_as_of));
                        obj.insert("profile_updated_at".to_string(), json!(record.updated_at));
                        obj.insert(
                            "no_reversal_profile_lookup_ms".to_string(),
                            json!(lookup_ms),
                        );
                        obj.insert("lookbacks".to_string(), record.lookbacks_json);
                        obj.insert("sample_count".to_string(), json!(record.sample_count));
                        obj.insert("market_count".to_string(), json!(record.market_count));
                    }
                    return decision;
                }
                no_reversal_spawn_profile_warmup(
                    repo.clone(),
                    config.clone(),
                    query.clone(),
                    cache_key.clone(),
                );
                let mut decision = no_reversal_local_path_decision(
                    config,
                    input,
                    &query,
                    ptb_floor,
                    source_buffer,
                    "error",
                    "precomputed_profile_invalid",
                );
                no_reversal_attach_prewarmer_status(
                    &mut decision.payload,
                    "expected_key_failed",
                );
                return decision;
            }
            Ok(Ok(Some(record))) => {
                no_reversal_spawn_profile_warmup(
                    repo.clone(),
                    config.clone(),
                    query.clone(),
                    cache_key.clone(),
                );
                let profile_source = match record.status.as_str() {
                    "insufficient" => "insufficient",
                    "stale" => "stale",
                    "timed_out" => "timeout",
                    "error" => "error",
                    _ => "error",
                };
                let profile_reason = match record.status.as_str() {
                    "insufficient" => "insufficient_historical_adverse_data",
                    "stale" => "precomputed_profile_stale",
                    "timed_out" => "prewarm_query_timeout",
                    "error" => "precomputed_profile_error",
                    _ => "precomputed_profile_error",
                };
                let mut decision = no_reversal_local_path_decision(
                    config,
                    input,
                    &query,
                    ptb_floor,
                    source_buffer,
                    profile_source,
                    profile_reason,
                );
                if let Some(obj) = decision.payload.as_object_mut() {
                    obj.insert(
                        "profile_status".to_string(),
                        json!(record.status.as_str()),
                    );
                    obj.insert(
                        "prewarmer_status".to_string(),
                        json!(no_reversal_prewarmer_status_from_record_status(
                            record.status.as_str()
                        )),
                    );
                    obj.insert("profile_as_of".to_string(), json!(record.profile_as_of));
                    obj.insert("profile_updated_at".to_string(), json!(record.updated_at));
                    obj.insert(
                        "no_reversal_profile_lookup_ms".to_string(),
                        json!(lookup_ms),
                    );
                    obj.insert("lookbacks".to_string(), record.lookbacks_json);
                    obj.insert("sample_count".to_string(), json!(record.sample_count));
                    obj.insert("market_count".to_string(), json!(record.market_count));
                    if let Some(error) = record.error {
                        obj.insert("profile_error".to_string(), json!(error));
                    }
                }
                return decision;
            }
            Ok(Ok(None)) => {
                no_reversal_spawn_profile_warmup(
                    repo.clone(),
                    config.clone(),
                    query.clone(),
                    cache_key.clone(),
                );
                let mut decision = no_reversal_local_path_decision(
                    config,
                    input,
                    &query,
                    ptb_floor,
                    source_buffer,
                    "missing",
                    "precomputed_profile_missing",
                );
                if let Some(obj) = decision.payload.as_object_mut() {
                    obj.insert(
                        "no_reversal_profile_lookup_ms".to_string(),
                        json!(lookup_ms),
                    );
                }
                if let Some(profile_key) = profile_key.as_ref() {
                    match repo.no_reversal_adverse_profile_diagnostics(profile_key).await {
                        Ok(diagnostics) => no_reversal_attach_prewarmer_diagnostics(
                            &mut decision.payload,
                            &diagnostics,
                        ),
                        Err(err) => {
                            no_reversal_attach_prewarmer_status(
                                &mut decision.payload,
                                "expected_key_failed",
                            );
                            if let Some(obj) = decision.payload.as_object_mut() {
                                obj.insert(
                                    "prewarmer_diagnostics_error".to_string(),
                                    json!(err.to_string()),
                                );
                            }
                        }
                    }
                } else {
                    no_reversal_attach_prewarmer_status(
                        &mut decision.payload,
                        "no_expected_key",
                    );
                }
                return decision;
            }
            Ok(Err(err)) => {
                no_reversal_spawn_profile_warmup(
                    repo.clone(),
                    config.clone(),
                    query.clone(),
                    cache_key.clone(),
                );
                let mut decision = no_reversal_local_path_decision(
                    config,
                    input,
                    &query,
                    ptb_floor,
                    source_buffer,
                    "error",
                    "precomputed_profile_error",
                );
                if let Some(obj) = decision.payload.as_object_mut() {
                    obj.insert("profile_error".to_string(), json!(err.to_string()));
                    obj.insert("prewarmer_status".to_string(), json!("expected_key_failed"));
                    obj.insert(
                        "no_reversal_profile_lookup_ms".to_string(),
                        json!(lookup_ms),
                    );
                }
                return decision;
            }
            Err(_) => {
                no_reversal_spawn_profile_warmup(
                    repo.clone(),
                    config.clone(),
                    query.clone(),
                    cache_key.clone(),
                );
                let mut decision = no_reversal_local_path_decision(
                    config,
                    input,
                    &query,
                    ptb_floor,
                    source_buffer,
                    "error",
                    "precomputed_profile_lookup_timeout",
                );
                if let Some(obj) = decision.payload.as_object_mut() {
                    obj.insert(
                        "no_reversal_profile_lookup_ms".to_string(),
                        json!(lookup_ms),
                    );
                }
                return decision;
            }
        }
    }

    if !config.allow_cold_profile_query {
        no_reversal_spawn_profile_warmup(repo.clone(), config.clone(), query.clone(), cache_key);
        return no_reversal_local_path_decision(
            config,
            input,
            &query,
            ptb_floor,
            source_buffer,
            "missing",
            "precomputed_profile_missing",
        );
    }

    let query_started = Instant::now();
    let lookup = tokio::time::timeout(
        Duration::from_millis(config.profile_query_timeout_ms as u64),
        no_reversal_resolve_profile(repo, config, &query, now),
    )
    .await;
    let query_ms = query_started.elapsed().as_millis().min(i64::MAX as u128) as i64;
    match lookup {
        Ok(Ok(lookup)) => {
            if let Some(profile) = lookup.profile {
                no_reversal_store_cached_profile(cache_key, now_ms, &profile);
                return no_reversal_decision_from_profile(
                    config,
                    input,
                    &query,
                    ptb_floor,
                    source_buffer,
                    &profile,
                    false,
                    None,
                    Some(query_ms),
                    false,
                    "cold_query",
                );
            }
            let payload = json!({
                "enabled": config.enabled,
                "lookback_mode": config.lookback_mode,
                "market_slug": input.market_slug,
                "asset": input.asset,
                "direction": input.direction,
                "remaining_sec": input.remaining_sec,
                "effective_fill": input.effective_fill,
                "current_live_gap_usd": input.current_live_gap,
                "ptb_floor_usd": ptb_floor,
                "source_buffer_usd": source_buffer,
                "quantile": query.quantile,
                "high_late_profile": query.high_late,
                "fallback_level": no_reversal_fallback_label(lookup.last_fallback),
                "protection": "not_applied",
                "cache_hit": false,
                "no_reversal_profile_cache_hit": false,
                "no_reversal_profile_query_ms": query_ms,
                "stats_source": lookup.stats_source,
                "bulk_query_ms": lookup.bulk_query_ms,
                "no_reversal_profile_timeout": false,
                "bucket": {
                    "remaining_bucket": &query.remaining_bucket.label,
                    "price_bucket": &query.price_bucket.label,
                    "gap_bucket": &query.gap_bucket.label,
                    "slope_bucket": &query.slope_bucket,
                },
                "lookbacks": no_reversal_stats_payload(&lookup.last_stats),
                "sources": { "historical": lookup.stats_source, "live": "binance_live" },
            });
            no_reversal_unapplied_decision(config, "insufficient_historical_adverse_data", payload)
        }
        Ok(Err(err)) => {
            let payload = json!({
                "enabled": config.enabled,
                "lookback_mode": config.lookback_mode,
                "market_slug": input.market_slug,
                "asset": input.asset,
                "direction": input.direction,
                "remaining_sec": input.remaining_sec,
                "effective_fill": input.effective_fill,
                "current_live_gap_usd": input.current_live_gap,
                "ptb_floor_usd": ptb_floor,
                "source_buffer_usd": source_buffer,
                "quantile": query.quantile,
                "high_late_profile": query.high_late,
                "protection": "not_applied",
                "cache_hit": false,
                "no_reversal_profile_cache_hit": false,
                "no_reversal_profile_query_ms": query_ms,
                "no_reversal_profile_timeout": false,
                "stats_source": NO_REVERSAL_PROFILE_STATS_SOURCE_FEATURES,
                "error": err.to_string(),
                "bucket": {
                    "remaining_bucket": &query.remaining_bucket.label,
                    "price_bucket": &query.price_bucket.label,
                    "gap_bucket": &query.gap_bucket.label,
                    "slope_bucket": &query.slope_bucket,
                },
                "sources": { "historical": NO_REVERSAL_PROFILE_STATS_SOURCE_FEATURES, "live": "binance_live" },
            });
            no_reversal_unapplied_decision(config, "historical_adverse_query_failed", payload)
        }
        Err(_) => {
            no_reversal_spawn_profile_warmup(
                repo.clone(),
                config.clone(),
                query.clone(),
                cache_key,
            );
            let payload = json!({
                "enabled": config.enabled,
                "lookback_mode": config.lookback_mode,
                "market_slug": input.market_slug,
                "asset": input.asset,
                "direction": input.direction,
                "remaining_sec": input.remaining_sec,
                "effective_fill": input.effective_fill,
                "current_live_gap_usd": input.current_live_gap,
                "ptb_floor_usd": ptb_floor,
                "source_buffer_usd": source_buffer,
                "quantile": query.quantile,
                "high_late_profile": query.high_late,
                "protection": "not_applied",
                "cache_hit": false,
                "no_reversal_profile_cache_hit": false,
                "no_reversal_profile_query_ms": query_ms,
                "no_reversal_profile_timeout": true,
                "profile_query_timeout_ms": config.profile_query_timeout_ms,
                "stats_source": NO_REVERSAL_PROFILE_STATS_SOURCE_FEATURES,
                "bucket": {
                    "remaining_bucket": &query.remaining_bucket.label,
                    "price_bucket": &query.price_bucket.label,
                    "gap_bucket": &query.gap_bucket.label,
                    "slope_bucket": &query.slope_bucket,
                },
                "sources": { "historical": NO_REVERSAL_PROFILE_STATS_SOURCE_FEATURES, "live": "binance_live" },
            });
            no_reversal_unapplied_decision(config, "historical_adverse_query_timeout", payload)
        }
    }
}
