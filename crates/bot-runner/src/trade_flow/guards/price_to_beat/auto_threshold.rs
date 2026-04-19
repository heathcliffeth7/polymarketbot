use super::normalize_outcome_direction;
use crate::trade_flow::guards::chainlink_price::{
    get_chainlink_price_window_stats, ChainlinkPriceWindowStats,
};
use anyhow::anyhow;
use serde_json::{json, Value};

const AUTO_LAST_3_LOOKBACK_WINDOWS: usize = 3;
const AUTO_VOL_PCT_BASELINE_WINDOWS: usize = 20;
const AUTO_VOL_PCT_CURRENT_WINDOWS: usize = 3;
const AUTO_VOL_PCT_EWMA_SPAN: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AutoPriceToBeatThresholdStrategy {
    Last3AvgExcursion,
    VolPct,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum AutoPriceToBeatThresholdResolution {
    Ready(AutoPriceToBeatThresholdSnapshot),
    Pending(String),
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct AutoPriceToBeatThresholdSnapshot {
    pub(super) threshold_usd: Option<f64>,
    pub(super) threshold_pct: Option<f64>,
    pub(super) lookback_windows_used: usize,
    pub(super) current_windows_used: Option<usize>,
    pub(super) avg_up_excursion_usd: Option<f64>,
    pub(super) avg_down_excursion_usd: Option<f64>,
    pub(super) lookback_market_slugs: Vec<String>,
    pub(super) lookback_window_snapshots: Vec<Value>,
    pub(super) baseline_pct: Option<f64>,
    pub(super) current_pct: Option<f64>,
    pub(super) vol_factor: Option<f64>,
    pub(super) base_pct: Option<f64>,
    pub(super) floor_usd: Option<f64>,
    pub(super) ceiling_usd: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AutoVolPctAssetConfig {
    base_pct: f64,
    floor_usd: f64,
    ceiling_usd: f64,
    lookback_baseline: usize,
    lookback_current: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct AutoPriceToBeatWindowHistoryEntry {
    market_slug: String,
    start_ts: i64,
    end_ts: i64,
    stats: ChainlinkPriceWindowStats,
}

impl AutoPriceToBeatWindowHistoryEntry {
    fn up_excursion(&self) -> f64 {
        (self.stats.high_price - self.stats.open_price).max(0.0)
    }

    fn down_excursion(&self) -> f64 {
        (self.stats.open_price - self.stats.low_price).max(0.0)
    }

    fn range_pct(&self) -> f64 {
        if self.stats.open_price <= 0.0 {
            return 0.0;
        }
        ((self.stats.high_price - self.stats.low_price).max(0.0)) / self.stats.open_price
    }

    fn to_value(&self) -> Value {
        json!({
            "market_slug": self.market_slug,
            "start_ts": self.start_ts,
            "end_ts": self.end_ts,
            "open_price": self.stats.open_price,
            "high_price": self.stats.high_price,
            "low_price": self.stats.low_price,
            "close_price": self.stats.close_price,
            "up_excursion_usd": self.up_excursion(),
            "down_excursion_usd": self.down_excursion(),
            "range_pct": self.range_pct(),
            "sample_count": self.stats.sample_count,
        })
    }
}

impl AutoPriceToBeatThresholdSnapshot {
    pub(super) fn resolved_threshold_usd(&self, price_to_beat: f64) -> Option<(f64, bool)> {
        if let Some(threshold_usd) = self.threshold_usd {
            return Some((threshold_usd, false));
        }
        let threshold_pct = self.threshold_pct?;
        let floor_usd = self.floor_usd?;
        let ceiling_usd = self.ceiling_usd?;
        let raw_threshold_usd = price_to_beat * threshold_pct;
        let threshold_usd = raw_threshold_usd.clamp(floor_usd, ceiling_usd);
        Some((
            threshold_usd,
            (raw_threshold_usd - threshold_usd).abs() > f64::EPSILON,
        ))
    }
}

pub(super) fn resolve_auto_price_to_beat_threshold(
    strategy: AutoPriceToBeatThresholdStrategy,
    market_slug: &str,
    outcome_label: &str,
) -> AutoPriceToBeatThresholdResolution {
    match strategy {
        AutoPriceToBeatThresholdStrategy::Last3AvgExcursion => {
            match resolve_last_3_avg_excursion_threshold(market_slug, outcome_label) {
                Ok(snapshot) => AutoPriceToBeatThresholdResolution::Ready(snapshot),
                Err(err) => AutoPriceToBeatThresholdResolution::Pending(err.to_string()),
            }
        }
        AutoPriceToBeatThresholdStrategy::VolPct => {
            match resolve_auto_vol_pct_threshold(market_slug) {
                Ok(snapshot) => AutoPriceToBeatThresholdResolution::Ready(snapshot),
                Err(AutoPriceToBeatThresholdResolution::Pending(detail)) => {
                    AutoPriceToBeatThresholdResolution::Pending(detail)
                }
                Err(AutoPriceToBeatThresholdResolution::Unsupported(detail)) => {
                    AutoPriceToBeatThresholdResolution::Unsupported(detail)
                }
                Err(AutoPriceToBeatThresholdResolution::Ready(_)) => {
                    AutoPriceToBeatThresholdResolution::Pending(
                        "unexpected auto_vol_pct resolution state".to_string(),
                    )
                }
            }
        }
    }
}

fn resolve_last_3_avg_excursion_threshold(
    market_slug: &str,
    outcome_label: &str,
) -> anyhow::Result<AutoPriceToBeatThresholdSnapshot> {
    let (_, direction) = normalize_outcome_direction(outcome_label).ok_or_else(|| {
        anyhow!("unsupported outcome label for auto price-to-beat: {outcome_label}")
    })?;
    let scope = crate::find_updown_scope_by_slug(market_slug).ok_or_else(|| {
        anyhow!("unsupported updown market slug for auto price-to-beat: {market_slug}")
    })?;
    let current_start = crate::MarketCycleId(market_slug.to_string())
        .start_time()
        .ok_or_else(|| anyhow!("failed to parse cycle start from market slug: {market_slug}"))?;
    let history = collect_contiguous_window_history(
        scope.asset,
        scope.slug_prefix,
        current_start,
        crate::updown_scope_window_seconds(scope),
        AUTO_LAST_3_LOOKBACK_WINDOWS,
    );
    if history.len() < AUTO_LAST_3_LOOKBACK_WINDOWS {
        return Err(anyhow!(
            "auto price-to-beat history pending: expected {} windows, got {}",
            AUTO_LAST_3_LOOKBACK_WINDOWS,
            history.len()
        ));
    }

    let up_excursions: Vec<f64> = history
        .iter()
        .map(AutoPriceToBeatWindowHistoryEntry::up_excursion)
        .collect();
    let down_excursions: Vec<f64> = history
        .iter()
        .map(AutoPriceToBeatWindowHistoryEntry::down_excursion)
        .collect();
    let threshold_usd = if direction == "up" {
        average(&up_excursions)
    } else {
        average(&down_excursions)
    };

    Ok(AutoPriceToBeatThresholdSnapshot {
        threshold_usd: Some(threshold_usd),
        threshold_pct: None,
        lookback_windows_used: history.len(),
        current_windows_used: None,
        avg_up_excursion_usd: Some(average(&up_excursions)),
        avg_down_excursion_usd: Some(average(&down_excursions)),
        lookback_market_slugs: history
            .iter()
            .map(|entry| entry.market_slug.clone())
            .collect(),
        lookback_window_snapshots: history
            .iter()
            .map(AutoPriceToBeatWindowHistoryEntry::to_value)
            .collect(),
        baseline_pct: None,
        current_pct: None,
        vol_factor: None,
        base_pct: None,
        floor_usd: None,
        ceiling_usd: None,
    })
}

fn resolve_auto_vol_pct_threshold(
    market_slug: &str,
) -> Result<AutoPriceToBeatThresholdSnapshot, AutoPriceToBeatThresholdResolution> {
    let scope = crate::find_updown_scope_by_slug(market_slug).ok_or_else(|| {
        AutoPriceToBeatThresholdResolution::Unsupported(format!(
            "unsupported updown market slug for auto_vol_pct: {market_slug}"
        ))
    })?;
    let config = auto_vol_pct_config(scope.asset).ok_or_else(|| {
        AutoPriceToBeatThresholdResolution::Unsupported(format!(
            "auto_vol_pct supports only btc/eth/sol assets, got {}",
            scope.asset
        ))
    })?;
    let current_start = crate::MarketCycleId(market_slug.to_string())
        .start_time()
        .ok_or_else(|| {
            AutoPriceToBeatThresholdResolution::Unsupported(format!(
                "failed to parse cycle start from market slug: {market_slug}"
            ))
        })?;
    let history = collect_contiguous_window_history(
        scope.asset,
        scope.slug_prefix,
        current_start,
        crate::updown_scope_window_seconds(scope),
        config.lookback_baseline,
    );
    if history.len() < config.lookback_current {
        return Err(AutoPriceToBeatThresholdResolution::Pending(format!(
            "auto_vol_pct history pending: need at least {} contiguous windows, got {}",
            config.lookback_current,
            history.len()
        )));
    }

    let range_pcts: Vec<f64> = history
        .iter()
        .map(AutoPriceToBeatWindowHistoryEntry::range_pct)
        .collect();
    let current_windows_used = range_pcts.len().min(config.lookback_current);
    let baseline_pct = ewma(&range_pcts, AUTO_VOL_PCT_EWMA_SPAN);
    let current_pct = weighted_recent_mean(&range_pcts, current_windows_used);
    let vol_factor = safe_vol_factor(current_pct, baseline_pct);
    let threshold_pct = config.base_pct * vol_factor;

    Ok(AutoPriceToBeatThresholdSnapshot {
        threshold_usd: None,
        threshold_pct: Some(threshold_pct),
        lookback_windows_used: history.len(),
        current_windows_used: Some(current_windows_used),
        avg_up_excursion_usd: None,
        avg_down_excursion_usd: None,
        lookback_market_slugs: history
            .iter()
            .map(|entry| entry.market_slug.clone())
            .collect(),
        lookback_window_snapshots: history
            .iter()
            .map(AutoPriceToBeatWindowHistoryEntry::to_value)
            .collect(),
        baseline_pct: Some(baseline_pct),
        current_pct: Some(current_pct),
        vol_factor: Some(vol_factor),
        base_pct: Some(config.base_pct),
        floor_usd: Some(config.floor_usd),
        ceiling_usd: Some(config.ceiling_usd),
    })
}

fn auto_vol_pct_config(asset: &str) -> Option<AutoVolPctAssetConfig> {
    match asset {
        "eth" => Some(AutoVolPctAssetConfig {
            base_pct: 0.0012,
            floor_usd: 1.0,
            ceiling_usd: 5.0,
            lookback_baseline: AUTO_VOL_PCT_BASELINE_WINDOWS,
            lookback_current: AUTO_VOL_PCT_CURRENT_WINDOWS,
        }),
        "btc" => Some(AutoVolPctAssetConfig {
            base_pct: 0.0009,
            floor_usd: 10.0,
            ceiling_usd: 150.0,
            lookback_baseline: AUTO_VOL_PCT_BASELINE_WINDOWS,
            lookback_current: AUTO_VOL_PCT_CURRENT_WINDOWS,
        }),
        "sol" => Some(AutoVolPctAssetConfig {
            base_pct: 0.0005,
            floor_usd: 0.02,
            ceiling_usd: 0.05,
            lookback_baseline: AUTO_VOL_PCT_BASELINE_WINDOWS,
            lookback_current: AUTO_VOL_PCT_CURRENT_WINDOWS,
        }),
        _ => None,
    }
}

fn collect_contiguous_window_history(
    asset: &str,
    slug_prefix: &str,
    current_start: chrono::DateTime<chrono::Utc>,
    window_seconds: i64,
    target_windows: usize,
) -> Vec<AutoPriceToBeatWindowHistoryEntry> {
    let window_ms = window_seconds * 1_000;
    let mut windows = Vec::with_capacity(target_windows);

    for offset in 1..=target_windows {
        let window_start =
            current_start - crate::ChronoDuration::milliseconds(window_ms * offset as i64);
        let window_end = window_start + crate::ChronoDuration::milliseconds(window_ms);
        let lookback_market_slug = format!("{slug_prefix}{}", window_start.timestamp());
        let stats = match get_chainlink_price_window_stats(
            asset,
            window_start.timestamp_millis(),
            window_end.timestamp_millis(),
        ) {
            Ok(stats) => stats,
            Err(_) => break,
        };
        windows.push(AutoPriceToBeatWindowHistoryEntry {
            market_slug: lookback_market_slug,
            start_ts: window_start.timestamp(),
            end_ts: window_end.timestamp(),
            stats,
        });
    }

    windows.reverse();
    windows
}

fn average(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn ewma(values: &[f64], span: usize) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let alpha = 2.0 / (span as f64 + 1.0);
    let mut acc = values[0];
    for value in &values[1..] {
        acc = alpha * *value + (1.0 - alpha) * acc;
    }
    acc
}

fn weighted_recent_mean(values: &[f64], current_windows_used: usize) -> f64 {
    if values.is_empty() || current_windows_used == 0 {
        return 0.0;
    }
    let weights = [0.2_f64, 0.3, 0.5];
    let start = values.len().saturating_sub(current_windows_used);
    let recent_values = &values[start..];
    let recent_weights = &weights[weights.len() - recent_values.len()..];
    let weight_sum = recent_weights.iter().sum::<f64>();
    recent_values
        .iter()
        .zip(recent_weights.iter())
        .map(|(value, weight)| value * (weight / weight_sum))
        .sum()
}

fn safe_vol_factor(current_pct: f64, baseline_pct: f64) -> f64 {
    if baseline_pct <= 0.0 || current_pct <= 0.0 {
        return 1.0;
    }
    (current_pct / baseline_pct).clamp(0.1, 10.0).sqrt()
}
