use super::iv_mismatch_math::root_mean_square;
use super::iv_mismatch_time_rule::PriceToBeatIvMismatchTimeRule;
use crate::trade_flow::guards::chainlink_price::ChainlinkPriceSample;
use chrono::{Duration as ChronoDuration, Utc};

pub(crate) fn valid_probability(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value < 1.0
}

pub(crate) fn iv_mismatch_side(outcome_label: &str) -> Option<&'static str> {
    match outcome_label.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "long" | "bull" => Some("up"),
        "no" | "down" | "short" | "bear" => Some("down"),
        _ => None,
    }
}

pub(crate) fn iv_mismatch_seconds_left(market_slug: &str) -> Option<f64> {
    let scope = crate::find_updown_scope_by_slug(market_slug)?;
    let start = crate::MarketCycleId(market_slug.to_string()).start_time()?;
    let end = start + ChronoDuration::seconds(crate::updown_scope_window_seconds(scope));
    Some(
        end.signed_duration_since(Utc::now())
            .num_milliseconds()
            .max(0) as f64
            / 1_000.0,
    )
}

pub(crate) fn edge_threshold_for_seconds_left(
    seconds_left: f64,
    selected_time_rule: Option<(usize, PriceToBeatIvMismatchTimeRule)>,
    no_new_trade_under_secs: f64,
    no_new_trade_over_secs: f64,
    edge_threshold_30_90_secs: f64,
    edge_threshold_15_30_secs: f64,
    edge_threshold_8_15_secs: f64,
) -> Result<f64, &'static str> {
    if let Some((_, rule)) = selected_time_rule {
        return if rule.min_edge.is_finite() && rule.min_edge >= 0.0 {
            Ok(rule.min_edge)
        } else {
            Err("blocked_invalid_time_rule")
        };
    }
    if seconds_left <= no_new_trade_under_secs {
        Err("blocked_too_late")
    } else if seconds_left > no_new_trade_over_secs {
        Err("blocked_too_early")
    } else if seconds_left > 30.0 {
        Ok(edge_threshold_30_90_secs)
    } else if seconds_left > 15.0 {
        Ok(edge_threshold_15_30_secs)
    } else {
        Ok(edge_threshold_8_15_secs)
    }
}

pub(crate) fn side_gap(side: &str, price: f64, price_to_beat: f64) -> f64 {
    if side == "up" {
        price - price_to_beat
    } else {
        price_to_beat - price
    }
}

pub(crate) fn previous_side_gap(
    samples: &[ChainlinkPriceSample],
    side: &str,
    price_to_beat: f64,
    latest_timestamp_ms: i64,
) -> Option<(i64, f64)> {
    let target_ms = latest_timestamp_ms - 1_000;
    samples
        .iter()
        .rev()
        .find(|sample| sample.timestamp_ms <= target_ms)
        .or_else(|| {
            samples
                .iter()
                .rev()
                .find(|sample| sample.timestamp_ms < latest_timestamp_ms)
        })
        .map(|sample| {
            (
                sample.timestamp_ms,
                side_gap(side, sample.price, price_to_beat),
            )
        })
}

pub(crate) fn side_gap_at_or_before(
    samples: &[ChainlinkPriceSample],
    side: &str,
    price_to_beat: f64,
    target_ms: i64,
) -> Option<f64> {
    samples
        .iter()
        .rev()
        .find(|sample| sample.timestamp_ms <= target_ms)
        .map(|sample| side_gap(side, sample.price, price_to_beat))
}

pub(crate) fn sigma_since(samples: &[ChainlinkPriceSample], start_ms: i64) -> Option<f64> {
    let deltas = time_normalized_price_deltas_since(samples, start_ms);
    if deltas.len() < 2 {
        return None;
    }
    // RMS (mean dahil): tek yonlu spike'lar da volatilite sayilsin diye std yerine kullaniliyor
    let sigma = root_mean_square(&deltas);
    (sigma.is_finite() && sigma > 0.0).then_some(sigma)
}

pub(crate) fn time_normalized_price_deltas(samples: &[ChainlinkPriceSample]) -> Vec<f64> {
    time_normalized_price_deltas_since(samples, i64::MIN)
}

fn time_normalized_price_deltas_since(samples: &[ChainlinkPriceSample], start_ms: i64) -> Vec<f64> {
    let mut deltas = Vec::new();
    let filtered = samples
        .iter()
        .filter(|sample| sample.timestamp_ms >= start_ms)
        .collect::<Vec<_>>();
    for pair in filtered.windows(2) {
        let prev = &pair[0];
        let next = &pair[1];
        let dt_secs = (next.timestamp_ms - prev.timestamp_ms) as f64 / 1_000.0;
        if dt_secs <= 0.0 {
            continue;
        }
        let delta = (next.price - prev.price) / dt_secs.sqrt();
        if delta.is_finite() {
            deltas.push(delta);
        }
    }
    deltas
}

pub(crate) fn zero_cross_count(samples: &[ChainlinkPriceSample], price_to_beat: f64) -> usize {
    let mut previous = None;
    let mut count = 0;
    for sample in samples {
        let sign = gap_sign(sample.price - price_to_beat);
        if let Some(previous_sign) = previous {
            if sign != previous_sign {
                count += 1;
            }
        }
        previous = Some(sign);
    }
    count
}

fn gap_sign(gap: f64) -> i8 {
    if gap > 0.0 {
        1
    } else if gap < 0.0 {
        -1
    } else {
        0
    }
}
