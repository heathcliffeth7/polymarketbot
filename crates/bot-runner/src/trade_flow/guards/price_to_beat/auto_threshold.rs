use super::normalize_outcome_direction;
use crate::trade_flow::guards::chainlink_price::get_chainlink_price_window_stats;
use anyhow::{anyhow, Result};

const AUTO_PRICE_TO_BEAT_LOOKBACK_WINDOWS: usize = 3;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct AutoPriceToBeatThresholdSnapshot {
    pub(super) threshold_usd: f64,
    pub(super) lookback_windows_used: usize,
    pub(super) avg_up_excursion_usd: f64,
    pub(super) avg_down_excursion_usd: f64,
    pub(super) lookback_market_slugs: Vec<String>,
}

pub(super) fn resolve_auto_price_to_beat_threshold(
    market_slug: &str,
    outcome_label: &str,
) -> Result<AutoPriceToBeatThresholdSnapshot> {
    let (_, direction) = normalize_outcome_direction(outcome_label)
        .ok_or_else(|| anyhow!("unsupported outcome label for auto price-to-beat: {outcome_label}"))?;
    let scope = crate::find_updown_scope_by_slug(market_slug)
        .ok_or_else(|| anyhow!("unsupported updown market slug for auto price-to-beat: {market_slug}"))?;
    let current_start = crate::MarketCycleId(market_slug.to_string())
        .start_time()
        .ok_or_else(|| anyhow!("failed to parse cycle start from market slug: {market_slug}"))?;
    let window_ms = crate::updown_scope_window_seconds(scope) * 1_000;

    let mut up_excursions = Vec::with_capacity(AUTO_PRICE_TO_BEAT_LOOKBACK_WINDOWS);
    let mut down_excursions = Vec::with_capacity(AUTO_PRICE_TO_BEAT_LOOKBACK_WINDOWS);
    let mut lookback_market_slugs = Vec::with_capacity(AUTO_PRICE_TO_BEAT_LOOKBACK_WINDOWS);

    for offset in 1..=AUTO_PRICE_TO_BEAT_LOOKBACK_WINDOWS {
        let window_start = current_start - crate::ChronoDuration::milliseconds(window_ms * offset as i64);
        let window_end = window_start + crate::ChronoDuration::milliseconds(window_ms);
        let lookback_market_slug = format!("{}{}", scope.slug_prefix, window_start.timestamp());
        let stats = get_chainlink_price_window_stats(
            scope.asset,
            window_start.timestamp_millis(),
            window_end.timestamp_millis(),
        )
        .map_err(|err| {
            anyhow!(
                "auto price-to-beat history pending for {}: {}",
                lookback_market_slug,
                err
            )
        })?;
        up_excursions.push((stats.high_price - stats.open_price).max(0.0));
        down_excursions.push((stats.open_price - stats.low_price).max(0.0));
        lookback_market_slugs.push(lookback_market_slug);
    }

    let lookback_windows_used = lookback_market_slugs.len();
    if lookback_windows_used < AUTO_PRICE_TO_BEAT_LOOKBACK_WINDOWS {
        return Err(anyhow!(
            "auto price-to-beat history pending: expected {} windows, got {}",
            AUTO_PRICE_TO_BEAT_LOOKBACK_WINDOWS,
            lookback_windows_used
        ));
    }

    let avg_up_excursion_usd = average(&up_excursions);
    let avg_down_excursion_usd = average(&down_excursions);
    let threshold_usd = if direction == "up" {
        avg_up_excursion_usd
    } else {
        avg_down_excursion_usd
    };

    Ok(AutoPriceToBeatThresholdSnapshot {
        threshold_usd,
        lookback_windows_used,
        avg_up_excursion_usd,
        avg_down_excursion_usd,
        lookback_market_slugs,
    })
}

fn average(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}
