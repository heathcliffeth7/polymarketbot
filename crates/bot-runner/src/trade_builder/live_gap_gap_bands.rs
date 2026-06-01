#[derive(Debug, Clone, Copy, PartialEq)]
struct LiveGapDetailedGapBandsConfig {
    enabled: bool,
    ultra_clean_gap_usd: f64,
    low_clean_gap_usd: f64,
    mild_clean_gap_usd: f64,
    normal_gap_usd: f64,
    active_gap_usd: f64,
    high_gap_usd: f64,
    high_chop_gap_usd: f64,
    extreme_chop_gap_usd: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LiveGapDetailedGapBand {
    UltraClean,
    LowClean,
    MildClean,
    Normal,
    Active,
    High,
    HighChop,
    ExtremeChop,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LiveGapVolumeContext {
    volume_10s: Option<f64>,
    volume_30s: Option<f64>,
    volume_60s: Option<f64>,
    volume_90s: Option<f64>,
    volume_120s: Option<f64>,
    trade_count_10s: Option<i64>,
    trade_count_30s: Option<i64>,
    trade_count_60s: Option<i64>,
    trade_count_90s: Option<i64>,
    trade_count_120s: Option<i64>,
    baseline_30s: Option<f64>,
    baseline_sample_count: Option<i64>,
    volume_ratio_30s: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LiveGapBandPathContext {
    history_age_ms: i64,
    sample_count: usize,
    gap_drop_3s_usd: Option<f64>,
    gap_drop_5s_usd: Option<f64>,
    gap_slope_3s_usd_per_sec: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LiveGapBandSelection {
    band: LiveGapDetailedGapBand,
    old_regime: LiveGapCollectorRegime,
    detailed_enabled: bool,
    reason: &'static str,
    local_path_decision: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LiveGapRequiredGapEvaluation {
    band: LiveGapDetailedGapBand,
    old_regime: LiveGapCollectorRegime,
    detailed_enabled: bool,
    base_gap_usd: f64,
    price_adjustment_usd: f64,
    late_penalty_usd: f64,
    latency_buffer_usd: f64,
    required_gap_usd: f64,
    reason: &'static str,
    local_path_decision: &'static str,
}

impl LiveGapDetailedGapBand {
    fn label(self) -> &'static str {
        match self {
            Self::UltraClean => "ultra_clean",
            Self::LowClean => "low_clean",
            Self::MildClean => "mild_clean",
            Self::Normal => "normal",
            Self::Active => "active",
            Self::High => "high",
            Self::HighChop => "high_chop",
            Self::ExtremeChop => "extreme_chop",
        }
    }
}

impl LiveGapVolumeContext {
    fn unavailable() -> Self {
        Self {
            volume_10s: None,
            volume_30s: None,
            volume_60s: None,
            volume_90s: None,
            volume_120s: None,
            trade_count_10s: None,
            trade_count_30s: None,
            trade_count_60s: None,
            trade_count_90s: None,
            trade_count_120s: None,
            baseline_30s: None,
            baseline_sample_count: None,
            volume_ratio_30s: None,
        }
    }

    fn bucket(self) -> &'static str {
        let Some(ratio) = self
            .volume_ratio_30s
            .filter(|value| value.is_finite() && *value >= 0.0)
        else {
            return "unknown";
        };
        if ratio < 0.12 {
            "dead"
        } else if ratio < 0.50 {
            "thin"
        } else if ratio < 0.80 {
            "low"
        } else if ratio < 1.50 {
            "normal"
        } else if ratio < 2.50 {
            "active"
        } else if ratio < 4.00 {
            "hot"
        } else {
            "extreme"
        }
    }

    fn dead_activity_block_reason(self) -> Option<&'static str> {
        let ratio_dead = self
            .volume_ratio_30s
            .is_some_and(|ratio| ratio.is_finite() && ratio < 0.12);
        let count_dead = self.trade_count_120s.is_some_and(|count| count < 3);
        let zero_volume = self
            .volume_120s
            .is_some_and(|volume| volume.is_finite() && volume <= 0.0);
        (zero_volume || (ratio_dead && count_dead)).then_some("volume_dead_insufficient_activity")
    }
}

impl LiveGapDetailedGapBandsConfig {
    fn base_gap(self, band: LiveGapDetailedGapBand) -> f64 {
        match band {
            LiveGapDetailedGapBand::UltraClean => self.ultra_clean_gap_usd,
            LiveGapDetailedGapBand::LowClean => self.low_clean_gap_usd,
            LiveGapDetailedGapBand::MildClean => self.mild_clean_gap_usd,
            LiveGapDetailedGapBand::Normal => self.normal_gap_usd,
            LiveGapDetailedGapBand::Active => self.active_gap_usd,
            LiveGapDetailedGapBand::High => self.high_gap_usd,
            LiveGapDetailedGapBand::HighChop => self.high_chop_gap_usd,
            LiveGapDetailedGapBand::ExtremeChop => self.extreme_chop_gap_usd,
        }
    }
}

fn live_gap_sane_gap(value: Option<f64>, fallback: f64) -> f64 {
    value
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(fallback)
        .max(0.0)
}

fn resolve_live_gap_detailed_gap_bands_config(
    node: &TradeFlowNode,
    low_clean_gap_usd: f64,
    normal_gap_usd: f64,
    high_gap_usd: f64,
    high_chop_gap_usd: f64,
) -> LiveGapDetailedGapBandsConfig {
    LiveGapDetailedGapBandsConfig {
        enabled: node_config_bool(node, "liveGapCollectorDetailedGapBandsEnabled")
            .unwrap_or(false)
            && node_config_string(node, "liveGapCollectorGapBandMode")
                .map(|mode| mode.trim().eq_ignore_ascii_case("volume_volatility_v2"))
                .unwrap_or(true),
        ultra_clean_gap_usd: live_gap_sane_gap(
            node_config_f64(node, "liveGapCollectorUltraCleanGapUsd"),
            (low_clean_gap_usd * 0.85).max(0.0),
        ),
        low_clean_gap_usd,
        mild_clean_gap_usd: live_gap_sane_gap(
            node_config_f64(node, "liveGapCollectorMildCleanGapUsd"),
            (low_clean_gap_usd + normal_gap_usd) * 0.5,
        ),
        normal_gap_usd,
        active_gap_usd: live_gap_sane_gap(
            node_config_f64(node, "liveGapCollectorActiveGapUsd"),
            (normal_gap_usd + high_gap_usd) * 0.5,
        ),
        high_gap_usd,
        high_chop_gap_usd,
        extreme_chop_gap_usd: live_gap_sane_gap(
            node_config_f64(node, "liveGapCollectorExtremeChopGapUsd"),
            high_chop_gap_usd.max(high_gap_usd),
        ),
    }
}

fn live_gap_detailed_gap_bands_config_snapshot(
    config: &LiveGapDetailedGapBandsConfig,
) -> Value {
    json!({
        "enabled": config.enabled,
        "mode": "volume_volatility_v2",
        "ultraCleanGapUsd": config.ultra_clean_gap_usd,
        "lowCleanGapUsd": config.low_clean_gap_usd,
        "mildCleanGapUsd": config.mild_clean_gap_usd,
        "normalGapUsd": config.normal_gap_usd,
        "activeGapUsd": config.active_gap_usd,
        "highGapUsd": config.high_gap_usd,
        "highChopGapUsd": config.high_chop_gap_usd,
        "extremeChopGapUsd": config.extreme_chop_gap_usd,
    })
}

fn live_gap_detailed_gap_bands_config_from_metadata(
    metadata: &Value,
    low_clean_gap_usd: f64,
    normal_gap_usd: f64,
    high_gap_usd: f64,
    high_chop_gap_usd: f64,
) -> LiveGapDetailedGapBandsConfig {
    let bands = metadata.pointer("/resolved_guard_config/detailedGapBands");
    let f64_at = |key: &str, fallback: f64| {
        live_gap_sane_gap(bands.and_then(|value| value.get(key)).and_then(value_as_f64), fallback)
    };
    LiveGapDetailedGapBandsConfig {
        enabled: bands
            .and_then(|value| value.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && bands
                .and_then(|value| value.get("mode"))
                .and_then(Value::as_str)
                .map(|mode| mode.trim().eq_ignore_ascii_case("volume_volatility_v2"))
                .unwrap_or(true),
        ultra_clean_gap_usd: f64_at("ultraCleanGapUsd", (low_clean_gap_usd * 0.85).max(0.0)),
        low_clean_gap_usd,
        mild_clean_gap_usd: f64_at("mildCleanGapUsd", (low_clean_gap_usd + normal_gap_usd) * 0.5),
        normal_gap_usd,
        active_gap_usd: f64_at("activeGapUsd", (normal_gap_usd + high_gap_usd) * 0.5),
        high_gap_usd,
        high_chop_gap_usd,
        extreme_chop_gap_usd: f64_at("extremeChopGapUsd", high_chop_gap_usd.max(high_gap_usd)),
    }
}

async fn live_gap_collector_volume_context(
    repo: &PostgresRepository,
    market_slug: &str,
    asset: &str,
    now: DateTime<Utc>,
) -> LiveGapVolumeContext {
    let Ok(summary) = repo.market_trade_volume_summary(market_slug, now).await else {
        return LiveGapVolumeContext::unavailable();
    };
    let baseline = repo
        .market_trade_volume_bucket_median(asset, 30.0, 0.0, 7, 30, market_slug, now)
        .await
        .ok()
        .filter(|median| median.sample_count >= 20)
        .filter(|median| median.median_volume_usdc.is_finite() && median.median_volume_usdc > 0.0);
    let baseline_30s = baseline.as_ref().map(|median| median.median_volume_usdc);
    LiveGapVolumeContext {
        volume_10s: Some(summary.volume_10s),
        volume_30s: Some(summary.volume_30s),
        volume_60s: Some(summary.volume_60s),
        volume_90s: Some(summary.volume_90s),
        volume_120s: Some(summary.volume_120s),
        trade_count_10s: Some(summary.trade_count_10s),
        trade_count_30s: Some(summary.trade_count_30s),
        trade_count_60s: Some(summary.trade_count_60s),
        trade_count_90s: Some(summary.trade_count_90s),
        trade_count_120s: Some(summary.trade_count_120s),
        baseline_30s,
        baseline_sample_count: baseline.as_ref().map(|median| median.sample_count),
        volume_ratio_30s: baseline_30s.map(|baseline| summary.volume_30s / baseline),
    }
}

fn live_gap_band_path_context(
    market_slug: &str,
    token_id: &str,
    outcome_label: &str,
    now_ms: i64,
    effective_fill: f64,
    live_gap: f64,
) -> LiveGapBandPathContext {
    let metrics = pre_buy_collapse_guard_metrics(
        market_slug,
        token_id,
        outcome_label,
        now_ms,
        effective_fill,
        live_gap,
    );
    LiveGapBandPathContext {
        history_age_ms: metrics.history_age_ms,
        sample_count: metrics.sample_count,
        gap_drop_3s_usd: metrics.gap_drop_3s_usd,
        gap_drop_5s_usd: metrics.gap_drop_5s_usd,
        gap_slope_3s_usd_per_sec: metrics.gap_slope_3s_usd_per_sec,
    }
}

fn live_gap_band_path_decision(path: &LiveGapBandPathContext, normal_gap_usd: f64) -> &'static str {
    if path.history_age_ms < 3_000 || path.sample_count < 3 {
        return "unknown";
    }
    let normal = normal_gap_usd.max(0.0001);
    let broken = path
        .gap_drop_3s_usd
        .is_some_and(|drop| drop >= normal * 0.70)
        || path
            .gap_drop_5s_usd
            .is_some_and(|drop| drop >= normal)
        || path
            .gap_slope_3s_usd_per_sec
            .is_some_and(|slope| slope < -normal * 0.15);
    if broken {
        return "unstable";
    }
    let clean = path
        .gap_drop_3s_usd
        .is_some_and(|drop| drop <= normal * 0.20)
        && path
            .gap_drop_5s_usd
            .is_some_and(|drop| drop <= normal * 0.35)
        && path
            .gap_slope_3s_usd_per_sec
            .is_some_and(|slope| slope >= -normal * 0.05);
    if clean {
        "clean"
    } else {
        "mixed"
    }
}

fn live_gap_band_from_old_regime(regime: LiveGapCollectorRegime) -> LiveGapDetailedGapBand {
    match regime {
        LiveGapCollectorRegime::LowClean => LiveGapDetailedGapBand::LowClean,
        LiveGapCollectorRegime::Normal => LiveGapDetailedGapBand::Normal,
        LiveGapCollectorRegime::High => LiveGapDetailedGapBand::High,
        LiveGapCollectorRegime::HighChop | LiveGapCollectorRegime::Red => {
            LiveGapDetailedGapBand::HighChop
        }
    }
}

fn live_gap_select_detailed_gap_band(
    config: &ActionPlaceOrderLiveGapCollectorConfig,
    old_regime: LiveGapCollectorRegime,
    volume: &LiveGapVolumeContext,
    volatility_usd: Option<f64>,
    path: &LiveGapBandPathContext,
    fill_price: Option<f64>,
) -> LiveGapBandSelection {
    let local_path_decision = live_gap_band_path_decision(path, config.normal_gap_usd);
    if !config.detailed_gap_bands.enabled {
        return LiveGapBandSelection {
            band: live_gap_band_from_old_regime(old_regime),
            old_regime,
            detailed_enabled: false,
            reason: "detailed_gap_bands_disabled",
            local_path_decision,
        };
    }
    let normal = config.normal_gap_usd.max(0.0001);
    let volatility = volatility_usd.filter(|value| value.is_finite()).unwrap_or(0.0);
    let volume_ratio = volume
        .volume_ratio_30s
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(1.0);
    let fill_price = fill_price.filter(|value| value.is_finite()).unwrap_or(1.0);
    let (band, reason) = if local_path_decision == "unstable"
        && (matches!(
            old_regime,
            LiveGapCollectorRegime::High | LiveGapCollectorRegime::HighChop
        ) || volatility >= normal * 1.25
            || fill_price >= 0.90)
    {
        (
            LiveGapDetailedGapBand::ExtremeChop,
            "unstable_path_high_risk",
        )
    } else if local_path_decision == "unstable" {
        (LiveGapDetailedGapBand::HighChop, "unstable_path")
    } else if volume_ratio >= 4.0 || volatility >= normal * 2.0 {
        (LiveGapDetailedGapBand::ExtremeChop, "extreme_volume_or_volatility")
    } else if volume_ratio >= 2.5 || volatility >= normal * 1.25 {
        (LiveGapDetailedGapBand::HighChop, "hot_volume_or_chop")
    } else if matches!(old_regime, LiveGapCollectorRegime::High)
        || volatility >= normal * 0.80
    {
        (LiveGapDetailedGapBand::High, "elevated_volatility")
    } else if volume_ratio >= 1.5 || volatility >= normal * 0.45 {
        (LiveGapDetailedGapBand::Active, "active_volume_or_volatility")
    } else if local_path_decision == "clean" && fill_price <= 0.85 && volatility <= normal * 0.25
    {
        (LiveGapDetailedGapBand::UltraClean, "clean_path_low_price_low_volatility")
    } else if local_path_decision == "clean" && volume_ratio < 0.80 {
        (LiveGapDetailedGapBand::MildClean, "clean_path_low_volume")
    } else if local_path_decision == "clean" {
        (LiveGapDetailedGapBand::LowClean, "clean_path")
    } else if matches!(old_regime, LiveGapCollectorRegime::LowClean) {
        (LiveGapDetailedGapBand::MildClean, "low_clean_without_full_path_confirmation")
    } else {
        (LiveGapDetailedGapBand::Normal, "standard_conditions")
    };
    LiveGapBandSelection {
        band,
        old_regime,
        detailed_enabled: true,
        reason,
        local_path_decision,
    }
}

fn live_gap_required_gap_evaluation(
    config: &ActionPlaceOrderLiveGapCollectorConfig,
    selection: &LiveGapBandSelection,
    remaining_sec: i64,
    fill_price: Option<f64>,
) -> LiveGapRequiredGapEvaluation {
    let base_gap_usd = if selection.detailed_enabled {
        config.detailed_gap_bands.base_gap(selection.band)
    } else {
        match selection.old_regime {
            LiveGapCollectorRegime::LowClean => config.low_clean_gap_usd,
            LiveGapCollectorRegime::Normal => config.normal_gap_usd,
            LiveGapCollectorRegime::High => config.high_gap_usd,
            LiveGapCollectorRegime::HighChop => config.high_chop_gap_usd,
            LiveGapCollectorRegime::Red => f64::INFINITY,
        }
    };
    let price_adjustment_usd = live_gap_collector_price_adjustment(config, fill_price);
    let late_penalty_usd = if remaining_sec < config.strong_only_under_sec {
        config.strong_signal_extra_gap_usd
    } else {
        0.0
    };
    LiveGapRequiredGapEvaluation {
        band: selection.band,
        old_regime: selection.old_regime,
        detailed_enabled: selection.detailed_enabled,
        base_gap_usd,
        price_adjustment_usd,
        late_penalty_usd,
        latency_buffer_usd: config.latency_buffer_usd,
        required_gap_usd: (base_gap_usd
            + price_adjustment_usd
            + late_penalty_usd
            + config.latency_buffer_usd)
            .max(0.0),
        reason: selection.reason,
        local_path_decision: selection.local_path_decision,
    }
}

fn live_gap_append_volume_context(payload: &mut serde_json::Map<String, Value>, volume: &LiveGapVolumeContext) {
    payload.insert("volume_10s".to_string(), json!(volume.volume_10s));
    payload.insert("volume_30s".to_string(), json!(volume.volume_30s));
    payload.insert("volume_60s".to_string(), json!(volume.volume_60s));
    payload.insert("volume_90s".to_string(), json!(volume.volume_90s));
    payload.insert("volume_120s".to_string(), json!(volume.volume_120s));
    payload.insert("trade_count_10s".to_string(), json!(volume.trade_count_10s));
    payload.insert("trade_count_30s".to_string(), json!(volume.trade_count_30s));
    payload.insert("trade_count_60s".to_string(), json!(volume.trade_count_60s));
    payload.insert("trade_count_90s".to_string(), json!(volume.trade_count_90s));
    payload.insert(
        "trade_count_120s".to_string(),
        json!(volume.trade_count_120s),
    );
    payload.insert("volume_baseline_30s".to_string(), json!(volume.baseline_30s));
    payload.insert(
        "volume_baseline_sample_count".to_string(),
        json!(volume.baseline_sample_count),
    );
    payload.insert("volume_ratio".to_string(), json!(volume.volume_ratio_30s));
    payload.insert(
        "volume_ratio_30s".to_string(),
        json!(volume.volume_ratio_30s),
    );
    payload.insert("volume_bucket".to_string(), json!(volume.bucket()));
}

fn live_gap_append_required_gap_evaluation(
    payload: &mut serde_json::Map<String, Value>,
    evaluation: &LiveGapRequiredGapEvaluation,
) {
    payload.insert("gap_band".to_string(), json!(evaluation.band.label()));
    payload.insert(
        "old_regime".to_string(),
        json!(live_gap_collector_regime_label(evaluation.old_regime)),
    );
    payload.insert(
        "old_4_band_equivalent".to_string(),
        json!(live_gap_collector_regime_label(evaluation.old_regime)),
    );
    payload.insert(
        "detailed_gap_bands_enabled".to_string(),
        json!(evaluation.detailed_enabled),
    );
    payload.insert("base_required_gap_usd".to_string(), json!(evaluation.base_gap_usd));
    payload.insert("final_required_gap_usd".to_string(), json!(evaluation.required_gap_usd));
    payload.insert(
        "price_adjustment_usd".to_string(),
        json!(evaluation.price_adjustment_usd),
    );
    payload.insert("late_penalty_usd".to_string(), json!(evaluation.late_penalty_usd));
    payload.insert("latency_buffer_usd".to_string(), json!(evaluation.latency_buffer_usd));
    payload.insert("band_reason".to_string(), json!(evaluation.reason));
    payload.insert(
        "local_path_decision".to_string(),
        json!(evaluation.local_path_decision),
    );
}
