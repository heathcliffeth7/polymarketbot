use super::chainlink_price::get_chainlink_price_cached;
use super::polymarket_price_to_beat::{
    get_price_to_beat_cached, try_price_to_beat_cached_or_spawn, PriceToBeatLookup,
    PriceToBeatSource,
};
use anyhow::Result;
use serde_json::{json, Value};

mod auto_threshold;
mod current_price;
mod max_price_relax;
mod notification;
mod notification_state;
#[cfg(test)]
mod notification_tests;
mod resolution;
mod runtime;
#[cfg(test)]
mod tests;
mod threshold_math;
use self::auto_threshold::{
    resolve_auto_price_to_beat_threshold, AutoPriceToBeatThresholdResolution,
    AutoPriceToBeatThresholdStrategy,
};
#[cfg(test)]
use self::current_price::{map_current_price_error, resolve_current_price_result};
use self::current_price::{resolve_price_to_beat_current_price, CURRENT_PRICE_SOURCE_CHAINLINK};
pub(crate) use self::max_price_relax::note_max_price_relax_fill_market;
pub(crate) use self::notification::{
    build_price_to_beat_bump_increased_notification_message,
    build_price_to_beat_bump_max_reached_notification_message,
};
pub(crate) use self::notification_state::take_price_to_beat_guard_notification_seed;
pub(crate) use self::resolution::resolve_action_place_order_price_to_beat_guard_resolution;
pub(crate) use self::runtime::{
    evaluate_action_place_order_price_to_beat_guard_state,
    maybe_block_action_place_order_price_to_beat_guard, PriceToBeatGuardRuntimeContext,
};
pub(crate) use self::threshold_math::{
    apply_price_to_beat_risk_penalty, normalize_price_to_beat_threshold_usd,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PriceToBeatMode {
    Manual,
    AutoLast3AvgExcursion,
    AutoVolPct,
}

impl PriceToBeatMode {
    pub(crate) fn parse(raw: Option<&str>) -> Option<Self> {
        match raw
            .map(str::trim)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "" | "manual" => Some(Self::Manual),
            "auto_last_3_avg_excursion" => Some(Self::AutoLast3AvgExcursion),
            "auto_vol_pct" => Some(Self::AutoVolPct),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::AutoLast3AvgExcursion => "auto_last_3_avg_excursion",
            Self::AutoVolPct => "auto_vol_pct",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PriceToBeatDiffUnit {
    Usd,
    Cent,
}

impl PriceToBeatDiffUnit {
    pub(crate) fn parse(raw: Option<&str>) -> Option<Self> {
        match raw
            .map(str::trim)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "" | "usd" => Some(Self::Usd),
            "cent" => Some(Self::Cent),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Usd => "usd",
            Self::Cent => "cent",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PriceToBeatGuardEvaluation {
    pub(crate) passed: bool,
    pub(crate) reason_code: String,
    pub(crate) reason_detail: Option<String>,
    pub(crate) normalized_outcome_label: Option<String>,
    pub(crate) direction: Option<String>,
    pub(crate) market_slug: String,
    pub(crate) event_url: String,
    pub(crate) timeframe: Option<String>,
    pub(crate) asset: Option<String>,
    pub(crate) price_to_beat: Option<f64>,
    pub(crate) price_to_beat_status: Option<String>,
    pub(crate) price_to_beat_source: Option<String>,
    pub(crate) price_to_beat_source_latency_ms: Option<i64>,
    pub(crate) current_price: Option<f64>,
    pub(crate) current_price_source: &'static str,
    pub(crate) directional_gap: Option<f64>,
    pub(crate) gap_abs: Option<f64>,
    pub(crate) threshold_mode: String,
    pub(crate) configured_threshold_mode: Option<String>,
    pub(crate) base_threshold_value: Option<f64>,
    pub(crate) base_threshold_unit: Option<String>,
    pub(crate) base_threshold_usd: Option<f64>,
    pub(crate) current_effective_ptb_usd: Option<f64>,
    pub(crate) threshold_value: f64,
    pub(crate) threshold_unit: String,
    pub(crate) threshold_usd: f64,
    pub(crate) stop_loss_bump_count: i64,
    pub(crate) stop_loss_bump_applied_count: i64,
    pub(crate) stop_loss_bump_amount: Option<f64>,
    pub(crate) stop_loss_bump_max_value: Option<f64>,
    pub(crate) stop_loss_bump_unit: Option<String>,
    pub(crate) stop_loss_bump_raw_usd: f64,
    pub(crate) stop_loss_bump_usd: f64,
    pub(crate) stop_loss_bump_capped: bool,
    pub(crate) stop_loss_bump_max_reached: bool,
    pub(crate) stop_loss_bump_current_market_excluded: bool,
    pub(crate) stop_loss_bump_increment_usd: f64,
    pub(crate) reentry_generation: i64,
    pub(crate) reentry_override_active: bool,
    pub(crate) reentry_override_value: Option<f64>,
    pub(crate) reentry_override_unit: Option<String>,
    pub(crate) max_price_relax: Option<Value>,
    pub(crate) auto_threshold_usd: Option<f64>,
    pub(crate) lookback_windows_used: Option<usize>,
    pub(crate) current_windows_used: Option<usize>,
    pub(crate) avg_up_excursion_usd: Option<f64>,
    pub(crate) avg_down_excursion_usd: Option<f64>,
    pub(crate) lookback_market_slugs: Option<Vec<String>>,
    pub(crate) lookback_window_snapshots: Option<Vec<Value>>,
    pub(crate) baseline_pct: Option<f64>,
    pub(crate) current_pct: Option<f64>,
    pub(crate) vol_factor: Option<f64>,
    pub(crate) threshold_pct: Option<f64>,
    pub(crate) base_pct: Option<f64>,
    pub(crate) floor_usd: Option<f64>,
    pub(crate) ceiling_usd: Option<f64>,
    pub(crate) threshold_was_clamped: Option<bool>,
}

impl PriceToBeatGuardEvaluation {
    pub(crate) fn to_value(&self) -> Value {
        let mut value = json!({
            "passed": self.passed,
            "reason_code": self.reason_code,
            "reason_detail": self.reason_detail,
            "normalized_outcome_label": self.normalized_outcome_label,
            "direction": self.direction,
            "market_slug": self.market_slug,
            "event_url": self.event_url,
            "timeframe": self.timeframe,
            "asset": self.asset,
            "price_to_beat": self.price_to_beat,
            "price_to_beat_status": self.price_to_beat_status,
            "price_to_beat_source": self.price_to_beat_source,
            "price_to_beat_source_latency_ms": self.price_to_beat_source_latency_ms,
            "current_price": self.current_price,
            "current_price_source": self.current_price_source,
            "directional_gap": self.directional_gap,
            "gap_abs": self.gap_abs,
            "threshold_mode": self.threshold_mode,
            "configured_threshold_mode": self.configured_threshold_mode,
            "threshold_value": self.threshold_value,
            "threshold_unit": self.threshold_unit,
            "threshold_usd": self.threshold_usd,
            "reentry_generation": self.reentry_generation,
            "reentry_override_active": self.reentry_override_active,
            "reentry_override_value": self.reentry_override_value,
            "reentry_override_unit": self.reentry_override_unit,
            "auto_threshold_usd": self.auto_threshold_usd,
            "lookback_windows_used": self.lookback_windows_used,
            "current_windows_used": self.current_windows_used,
            "avg_up_excursion_usd": self.avg_up_excursion_usd,
            "avg_down_excursion_usd": self.avg_down_excursion_usd,
            "lookback_market_slugs": self.lookback_market_slugs,
            "lookback_window_snapshots": self.lookback_window_snapshots,
            "baseline_pct": self.baseline_pct,
            "current_pct": self.current_pct,
            "vol_factor": self.vol_factor,
            "threshold_pct": self.threshold_pct,
            "base_pct": self.base_pct,
            "floor_usd": self.floor_usd,
            "ceiling_usd": self.ceiling_usd,
            "threshold_was_clamped": self.threshold_was_clamped,
        });
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "base_threshold_value".to_string(),
                json!(self.base_threshold_value),
            );
            obj.insert(
                "base_threshold_unit".to_string(),
                json!(self.base_threshold_unit),
            );
            obj.insert(
                "base_threshold_usd".to_string(),
                json!(self.base_threshold_usd),
            );
            obj.insert(
                "current_effective_ptb_usd".to_string(),
                json!(self.current_effective_ptb_usd),
            );
            obj.insert(
                "stop_loss_bump_count".to_string(),
                json!(self.stop_loss_bump_count),
            );
            obj.insert(
                "stop_loss_bump_applied_count".to_string(),
                json!(self.stop_loss_bump_applied_count),
            );
            obj.insert(
                "stop_loss_bump_amount".to_string(),
                json!(self.stop_loss_bump_amount),
            );
            obj.insert(
                "stop_loss_bump_max_value".to_string(),
                json!(self.stop_loss_bump_max_value),
            );
            obj.insert(
                "stop_loss_bump_unit".to_string(),
                json!(self.stop_loss_bump_unit),
            );
            obj.insert(
                "stop_loss_bump_raw_usd".to_string(),
                json!(self.stop_loss_bump_raw_usd),
            );
            obj.insert(
                "stop_loss_bump_usd".to_string(),
                json!(self.stop_loss_bump_usd),
            );
            obj.insert(
                "stop_loss_bump_capped".to_string(),
                json!(self.stop_loss_bump_capped),
            );
            obj.insert(
                "stop_loss_bump_max_reached".to_string(),
                json!(self.stop_loss_bump_max_reached),
            );
            obj.insert(
                "stop_loss_bump_current_market_excluded".to_string(),
                json!(self.stop_loss_bump_current_market_excluded),
            );
            obj.insert(
                "stop_loss_bump_increment_usd".to_string(),
                json!(self.stop_loss_bump_increment_usd),
            );
            if let Some(max_price_relax) = self.max_price_relax.as_ref().and_then(Value::as_object)
            {
                for (key, value) in max_price_relax {
                    obj.insert(key.clone(), value.clone());
                }
            }
        }
        value
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DirectionalGapEvaluation {
    normalized_outcome_label: &'static str,
    direction: &'static str,
    directional_gap: f64,
    passed: bool,
}

fn normalize_outcome_direction(label: &str) -> Option<(&'static str, &'static str)> {
    match label.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "long" | "bull" => Some(("yes", "up")),
        "no" | "down" | "short" | "bear" => Some(("no", "down")),
        _ => None,
    }
}

fn evaluate_directional_gap(
    current_price: f64,
    price_to_beat: f64,
    threshold_usd: f64,
    outcome_label: &str,
) -> Option<DirectionalGapEvaluation> {
    let (normalized_outcome_label, direction) = normalize_outcome_direction(outcome_label)?;
    let directional_gap = if direction == "up" {
        current_price - price_to_beat
    } else {
        price_to_beat - current_price
    };
    Some(DirectionalGapEvaluation {
        normalized_outcome_label,
        direction,
        directional_gap,
        passed: directional_gap >= threshold_usd,
    })
}

#[derive(Debug, Clone)]
pub(crate) struct PriceToBeatTriggerGateResult {
    pub(crate) passed: bool,
    pub(crate) reason: &'static str,
    pub(crate) directional_gap: Option<f64>,
    pub(crate) price_to_beat: Option<f64>,
    pub(crate) price_to_beat_status: Option<String>,
    pub(crate) current_price: Option<f64>,
    pub(crate) threshold_mode: String,
    pub(crate) min_gap: f64,
    pub(crate) max_gap: Option<f64>,
    pub(crate) auto_threshold_usd: Option<f64>,
    pub(crate) lookback_windows_used: Option<usize>,
    pub(crate) current_windows_used: Option<usize>,
    pub(crate) avg_up_excursion_usd: Option<f64>,
    pub(crate) avg_down_excursion_usd: Option<f64>,
    pub(crate) lookback_market_slugs: Option<Vec<String>>,
    pub(crate) lookback_window_snapshots: Option<Vec<Value>>,
    pub(crate) baseline_pct: Option<f64>,
    pub(crate) current_pct: Option<f64>,
    pub(crate) vol_factor: Option<f64>,
    pub(crate) threshold_pct: Option<f64>,
    pub(crate) base_pct: Option<f64>,
    pub(crate) floor_usd: Option<f64>,
    pub(crate) ceiling_usd: Option<f64>,
    pub(crate) threshold_was_clamped: Option<bool>,
}

impl PriceToBeatTriggerGateResult {
    pub(crate) fn to_value(&self) -> Value {
        json!({
            "passed": self.passed,
            "reason": self.reason,
            "directional_gap": self.directional_gap,
            "price_to_beat": self.price_to_beat,
            "price_to_beat_status": self.price_to_beat_status,
            "current_price": self.current_price,
            "threshold_mode": self.threshold_mode,
            "min_gap": self.min_gap,
            "max_gap": self.max_gap,
            "auto_threshold_usd": self.auto_threshold_usd,
            "lookback_windows_used": self.lookback_windows_used,
            "current_windows_used": self.current_windows_used,
            "avg_up_excursion_usd": self.avg_up_excursion_usd,
            "avg_down_excursion_usd": self.avg_down_excursion_usd,
            "lookback_market_slugs": self.lookback_market_slugs,
            "lookback_window_snapshots": self.lookback_window_snapshots,
            "baseline_pct": self.baseline_pct,
            "current_pct": self.current_pct,
            "vol_factor": self.vol_factor,
            "threshold_pct": self.threshold_pct,
            "base_pct": self.base_pct,
            "floor_usd": self.floor_usd,
            "ceiling_usd": self.ceiling_usd,
            "threshold_was_clamped": self.threshold_was_clamped,
        })
    }
}

pub(crate) fn evaluate_price_to_beat_trigger_gate(
    market_slug: &str,
    outcome_label: &str,
    mode: PriceToBeatMode,
    min_gap: Option<f64>,
    max_gap: Option<f64>,
    unit: PriceToBeatDiffUnit,
) -> PriceToBeatTriggerGateResult {
    let mut min_gap_usd = 0.0;
    let mut max_gap_usd = None;
    let mut auto_threshold_usd = None;
    let mut lookback_windows_used = None;
    let mut current_windows_used = None;
    let mut avg_up_excursion_usd = None;
    let mut avg_down_excursion_usd = None;
    let mut lookback_market_slugs = None;
    let mut lookback_window_snapshots = None;
    let mut baseline_pct = None;
    let mut current_pct = None;
    let mut vol_factor = None;
    let mut threshold_pct = None;
    let mut base_pct = None;
    let mut floor_usd = None;
    let mut ceiling_usd = None;
    let mut threshold_was_clamped = None;
    let mut auto_threshold_snapshot = None;

    macro_rules! build_result {
        ($passed:expr, $reason:expr, $directional_gap:expr, $price_to_beat:expr, $price_to_beat_status:expr, $current_price:expr $(,)?) => {
            PriceToBeatTriggerGateResult {
                passed: $passed,
                reason: $reason,
                directional_gap: $directional_gap,
                price_to_beat: $price_to_beat,
                price_to_beat_status: $price_to_beat_status,
                current_price: $current_price,
                threshold_mode: mode.as_str().to_string(),
                min_gap: min_gap_usd,
                max_gap: max_gap_usd,
                auto_threshold_usd,
                lookback_windows_used,
                current_windows_used,
                avg_up_excursion_usd,
                avg_down_excursion_usd,
                lookback_market_slugs: lookback_market_slugs.clone(),
                lookback_window_snapshots: lookback_window_snapshots.clone(),
                baseline_pct,
                current_pct,
                vol_factor,
                threshold_pct,
                base_pct,
                floor_usd,
                ceiling_usd,
                threshold_was_clamped,
            }
        };
    }

    match mode {
        PriceToBeatMode::Manual => {
            let Some(min_gap) = min_gap else {
                return build_result!(false, "invalid_manual_threshold", None, None, None, None,);
            };
            min_gap_usd = normalize_price_to_beat_threshold_usd(min_gap, unit);
            max_gap_usd = max_gap.map(|value| normalize_price_to_beat_threshold_usd(value, unit));
        }
        PriceToBeatMode::AutoLast3AvgExcursion | PriceToBeatMode::AutoVolPct => {
            let strategy = match mode {
                PriceToBeatMode::AutoLast3AvgExcursion => {
                    AutoPriceToBeatThresholdStrategy::Last3AvgExcursion
                }
                PriceToBeatMode::AutoVolPct => AutoPriceToBeatThresholdStrategy::VolPct,
                PriceToBeatMode::Manual => unreachable!(),
            };
            match resolve_auto_price_to_beat_threshold(strategy, market_slug, outcome_label) {
                AutoPriceToBeatThresholdResolution::Ready(snapshot) => {
                    lookback_windows_used = Some(snapshot.lookback_windows_used);
                    current_windows_used = snapshot.current_windows_used;
                    avg_up_excursion_usd = snapshot.avg_up_excursion_usd;
                    avg_down_excursion_usd = snapshot.avg_down_excursion_usd;
                    lookback_market_slugs = Some(snapshot.lookback_market_slugs.clone());
                    lookback_window_snapshots = Some(snapshot.lookback_window_snapshots.clone());
                    baseline_pct = snapshot.baseline_pct;
                    current_pct = snapshot.current_pct;
                    vol_factor = snapshot.vol_factor;
                    threshold_pct = snapshot.threshold_pct;
                    base_pct = snapshot.base_pct;
                    floor_usd = snapshot.floor_usd;
                    ceiling_usd = snapshot.ceiling_usd;
                    if let Some(threshold) = snapshot.threshold_usd {
                        min_gap_usd = threshold;
                        auto_threshold_usd = Some(threshold);
                    }
                    auto_threshold_snapshot = Some(snapshot);
                }
                AutoPriceToBeatThresholdResolution::Pending(_) => {
                    return build_result!(false, "auto_threshold_pending", None, None, None, None);
                }
                AutoPriceToBeatThresholdResolution::Unsupported(detail) => {
                    lookback_market_slugs = Some(vec![market_slug.to_string()]);
                    let _ = detail;
                    return build_result!(false, "unsupported_market", None, None, None, None);
                }
            }
        }
    }

    let Some(snapshot) = get_price_to_beat_cached(market_slug) else {
        return build_result!(false, "price_to_beat_pending", None, None, None, None);
    };
    if let Some(auto_snapshot) = auto_threshold_snapshot.as_ref() {
        if auto_snapshot.threshold_usd.is_none() {
            if let Some((resolved_threshold_usd, was_clamped)) =
                auto_snapshot.resolved_threshold_usd(snapshot.price_to_beat)
            {
                min_gap_usd = resolved_threshold_usd;
                auto_threshold_usd = Some(resolved_threshold_usd);
                threshold_was_clamped = Some(was_clamped);
            }
        }
    }
    let current_price = match get_chainlink_price_cached(&snapshot.asset) {
        Ok(price) => price,
        Err(_) => {
            return build_result!(
                true,
                "chainlink_unavailable",
                None,
                Some(snapshot.price_to_beat),
                Some(snapshot.status().to_string()),
                None,
            );
        }
    };
    let Some(directional) = evaluate_directional_gap(
        current_price,
        snapshot.price_to_beat,
        min_gap_usd,
        outcome_label,
    ) else {
        return build_result!(
            false,
            "unsupported_outcome_label",
            None,
            Some(snapshot.price_to_beat),
            Some(snapshot.status().to_string()),
            Some(current_price),
        );
    };

    let passed = directional.directional_gap >= min_gap_usd
        && max_gap_usd
            .map(|threshold| directional.directional_gap <= threshold)
            .unwrap_or(true);
    let reason = if directional.directional_gap < min_gap_usd {
        "below_min_gap"
    } else if max_gap_usd
        .map(|threshold| directional.directional_gap > threshold)
        .unwrap_or(false)
    {
        "above_max_gap"
    } else {
        "in_range"
    };

    build_result!(
        passed,
        reason,
        Some(directional.directional_gap),
        Some(snapshot.price_to_beat),
        Some(snapshot.status().to_string()),
        Some(current_price),
    )
}

pub(crate) async fn evaluate_price_to_beat_guard(
    market_slug: &str,
    mode: PriceToBeatMode,
    threshold_value: Option<f64>,
    threshold_unit: PriceToBeatDiffUnit,
    outcome_label: &str,
) -> PriceToBeatGuardEvaluation {
    let mut resolved_threshold_value = threshold_value.unwrap_or_default();
    let mut resolved_threshold_unit = threshold_unit;
    let mut threshold_usd =
        normalize_price_to_beat_threshold_usd(resolved_threshold_value, resolved_threshold_unit);
    let mut auto_threshold_usd = None;
    let mut lookback_windows_used = None;
    let mut current_windows_used = None;
    let mut avg_up_excursion_usd = None;
    let mut avg_down_excursion_usd = None;
    let mut lookback_market_slugs = None;
    let mut lookback_window_snapshots = None;
    let mut baseline_pct = None;
    let mut current_pct = None;
    let mut vol_factor = None;
    let mut threshold_pct = None;
    let mut base_pct = None;
    let mut floor_usd = None;
    let mut ceiling_usd = None;
    let mut threshold_was_clamped = None;
    let mut auto_threshold_snapshot = None;

    let event_url = format!("https://polymarket.com/event/{market_slug}");
    match mode {
        PriceToBeatMode::Manual => {}
        PriceToBeatMode::AutoLast3AvgExcursion | PriceToBeatMode::AutoVolPct => {
            let strategy = match mode {
                PriceToBeatMode::AutoLast3AvgExcursion => {
                    AutoPriceToBeatThresholdStrategy::Last3AvgExcursion
                }
                PriceToBeatMode::AutoVolPct => AutoPriceToBeatThresholdStrategy::VolPct,
                PriceToBeatMode::Manual => unreachable!(),
            };
            match resolve_auto_price_to_beat_threshold(strategy, market_slug, outcome_label) {
                AutoPriceToBeatThresholdResolution::Ready(snapshot) => {
                    lookback_windows_used = Some(snapshot.lookback_windows_used);
                    current_windows_used = snapshot.current_windows_used;
                    avg_up_excursion_usd = snapshot.avg_up_excursion_usd;
                    avg_down_excursion_usd = snapshot.avg_down_excursion_usd;
                    lookback_market_slugs = Some(snapshot.lookback_market_slugs.clone());
                    lookback_window_snapshots = Some(snapshot.lookback_window_snapshots.clone());
                    baseline_pct = snapshot.baseline_pct;
                    current_pct = snapshot.current_pct;
                    vol_factor = snapshot.vol_factor;
                    threshold_pct = snapshot.threshold_pct;
                    base_pct = snapshot.base_pct;
                    floor_usd = snapshot.floor_usd;
                    ceiling_usd = snapshot.ceiling_usd;
                    if let Some(snapshot_threshold_usd) = snapshot.threshold_usd {
                        resolved_threshold_value = snapshot_threshold_usd;
                        resolved_threshold_unit = PriceToBeatDiffUnit::Usd;
                        threshold_usd = snapshot_threshold_usd;
                        auto_threshold_usd = Some(snapshot_threshold_usd);
                    } else {
                        resolved_threshold_value = 0.0;
                        resolved_threshold_unit = PriceToBeatDiffUnit::Usd;
                        threshold_usd = 0.0;
                    }
                    auto_threshold_snapshot = Some(snapshot);
                }
                AutoPriceToBeatThresholdResolution::Pending(detail) => {
                    return blocked_price_to_beat_guard_evaluation(
                        market_slug,
                        event_url,
                        mode,
                        resolved_threshold_value,
                        resolved_threshold_unit,
                        threshold_usd,
                        "auto_threshold_pending",
                        Some(detail),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        auto_threshold_usd,
                        lookback_windows_used,
                        current_windows_used,
                        avg_up_excursion_usd,
                        avg_down_excursion_usd,
                        lookback_market_slugs,
                        lookback_window_snapshots,
                        baseline_pct,
                        current_pct,
                        vol_factor,
                        threshold_pct,
                        base_pct,
                        floor_usd,
                        ceiling_usd,
                        threshold_was_clamped,
                    );
                }
                AutoPriceToBeatThresholdResolution::Unsupported(detail) => {
                    return blocked_price_to_beat_guard_evaluation(
                        market_slug,
                        event_url,
                        mode,
                        resolved_threshold_value,
                        resolved_threshold_unit,
                        threshold_usd,
                        "unsupported_market",
                        Some(detail),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        auto_threshold_usd,
                        lookback_windows_used,
                        current_windows_used,
                        avg_up_excursion_usd,
                        avg_down_excursion_usd,
                        lookback_market_slugs,
                        lookback_window_snapshots,
                        baseline_pct,
                        current_pct,
                        vol_factor,
                        threshold_pct,
                        base_pct,
                        floor_usd,
                        ceiling_usd,
                        threshold_was_clamped,
                    );
                }
            }
        }
    }
    let Some(scope) = crate::find_updown_scope_by_slug(market_slug) else {
        return blocked_price_to_beat_guard_evaluation(
            market_slug,
            event_url,
            mode,
            resolved_threshold_value,
            resolved_threshold_unit,
            threshold_usd,
            "unsupported_market",
            Some("market slug is not a supported 5m/15m updown scope".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            auto_threshold_usd,
            lookback_windows_used,
            current_windows_used,
            avg_up_excursion_usd,
            avg_down_excursion_usd,
            lookback_market_slugs,
            lookback_window_snapshots,
            baseline_pct,
            current_pct,
            vol_factor,
            threshold_pct,
            base_pct,
            floor_usd,
            ceiling_usd,
            threshold_was_clamped,
        );
    };
    if !matches!(scope.timeframe, "5m" | "15m") {
        return blocked_price_to_beat_guard_evaluation(
            market_slug,
            event_url,
            mode,
            resolved_threshold_value,
            resolved_threshold_unit,
            threshold_usd,
            "unsupported_market",
            Some(format!("unsupported timeframe: {}", scope.timeframe)),
            Some(scope.timeframe.to_string()),
            Some(scope.asset.to_string()),
            None,
            None,
            None,
            None,
            None,
            auto_threshold_usd,
            lookback_windows_used,
            current_windows_used,
            avg_up_excursion_usd,
            avg_down_excursion_usd,
            lookback_market_slugs,
            lookback_window_snapshots,
            baseline_pct,
            current_pct,
            vol_factor,
            threshold_pct,
            base_pct,
            floor_usd,
            ceiling_usd,
            threshold_was_clamped,
        );
    }

    let snapshot = match try_price_to_beat_cached_or_spawn(market_slug) {
        PriceToBeatLookup::Ready(snapshot) => snapshot,
        PriceToBeatLookup::Pending => {
            return blocked_price_to_beat_guard_evaluation(
                market_slug,
                event_url,
                mode,
                resolved_threshold_value,
                resolved_threshold_unit,
                threshold_usd,
                "price_to_beat_pending",
                None,
                Some(scope.timeframe.to_string()),
                Some(scope.asset.to_string()),
                None,
                None,
                None,
                None,
                None,
                auto_threshold_usd,
                lookback_windows_used,
                current_windows_used,
                avg_up_excursion_usd,
                avg_down_excursion_usd,
                lookback_market_slugs,
                lookback_window_snapshots,
                baseline_pct,
                current_pct,
                vol_factor,
                threshold_pct,
                base_pct,
                floor_usd,
                ceiling_usd,
                threshold_was_clamped,
            );
        }
        PriceToBeatLookup::Unavailable(detail) => {
            return blocked_price_to_beat_guard_evaluation(
                market_slug,
                event_url,
                mode,
                resolved_threshold_value,
                resolved_threshold_unit,
                threshold_usd,
                "price_to_beat_unavailable",
                Some(detail),
                Some(scope.timeframe.to_string()),
                Some(scope.asset.to_string()),
                None,
                None,
                None,
                None,
                None,
                auto_threshold_usd,
                lookback_windows_used,
                current_windows_used,
                avg_up_excursion_usd,
                avg_down_excursion_usd,
                lookback_market_slugs,
                lookback_window_snapshots,
                baseline_pct,
                current_pct,
                vol_factor,
                threshold_pct,
                base_pct,
                floor_usd,
                ceiling_usd,
                threshold_was_clamped,
            );
        }
    };
    if let Some(auto_snapshot) = auto_threshold_snapshot.as_ref() {
        if auto_snapshot.threshold_usd.is_none() {
            if let Some((resolved_auto_threshold_usd, was_clamped)) =
                auto_snapshot.resolved_threshold_usd(snapshot.price_to_beat)
            {
                resolved_threshold_value = resolved_auto_threshold_usd;
                resolved_threshold_unit = PriceToBeatDiffUnit::Usd;
                threshold_usd = resolved_auto_threshold_usd;
                auto_threshold_usd = Some(resolved_auto_threshold_usd);
                threshold_was_clamped = Some(was_clamped);
            }
        }
    }

    let (current_price, current_price_source) = match resolve_price_to_beat_current_price(
        snapshot.source,
        market_slug,
        &snapshot.asset,
        snapshot.source_latency_ms,
    )
    .await
    {
        Ok(resolved) => resolved,
        Err((reason_code, reason_detail)) => {
            return blocked_price_to_beat_guard_evaluation(
                market_slug,
                snapshot.event_url.clone(),
                mode,
                resolved_threshold_value,
                resolved_threshold_unit,
                threshold_usd,
                reason_code,
                Some(reason_detail),
                Some(scope.timeframe.to_string()),
                Some(scope.asset.to_string()),
                Some(snapshot.price_to_beat),
                Some(snapshot.status().to_string()),
                Some(snapshot.source.as_str().to_string()),
                snapshot.source_latency_ms,
                None,
                auto_threshold_usd,
                lookback_windows_used,
                current_windows_used,
                avg_up_excursion_usd,
                avg_down_excursion_usd,
                lookback_market_slugs,
                lookback_window_snapshots,
                baseline_pct,
                current_pct,
                vol_factor,
                threshold_pct,
                base_pct,
                floor_usd,
                ceiling_usd,
                threshold_was_clamped,
            );
        }
    };

    let gap_abs = (current_price - snapshot.price_to_beat).abs();
    let directional = match evaluate_directional_gap(
        current_price,
        snapshot.price_to_beat,
        threshold_usd,
        outcome_label,
    ) {
        Some(directional) => directional,
        None => {
            return blocked_price_to_beat_guard_evaluation(
                market_slug,
                snapshot.event_url.clone(),
                mode,
                resolved_threshold_value,
                resolved_threshold_unit,
                threshold_usd,
                "unsupported_outcome_label",
                Some(format!(
                    "outcome_label '{outcome_label}' is not a recognized direction"
                )),
                Some(scope.timeframe.to_string()),
                Some(scope.asset.to_string()),
                Some(snapshot.price_to_beat),
                Some(snapshot.status().to_string()),
                Some(snapshot.source.as_str().to_string()),
                snapshot.source_latency_ms,
                Some(current_price),
                auto_threshold_usd,
                lookback_windows_used,
                current_windows_used,
                avg_up_excursion_usd,
                avg_down_excursion_usd,
                lookback_market_slugs,
                lookback_window_snapshots,
                baseline_pct,
                current_pct,
                vol_factor,
                threshold_pct,
                base_pct,
                floor_usd,
                ceiling_usd,
                threshold_was_clamped,
            );
        }
    };
    let passed = directional.passed;
    let snapshot_status = snapshot.status().to_string();
    let snapshot_source = snapshot.source.as_str().to_string();
    PriceToBeatGuardEvaluation {
        passed,
        reason_code: if passed {
            "passed".to_string()
        } else {
            "price_to_beat_gap_below_threshold".to_string()
        },
        reason_detail: (!passed).then(|| {
            format!(
                "directional gap {:.8} (direction={}) is below threshold {:.8} {} (~{:.8} usd)",
                directional.directional_gap,
                directional.direction,
                resolved_threshold_value,
                resolved_threshold_unit.as_str(),
                threshold_usd
            )
        }),
        normalized_outcome_label: Some(directional.normalized_outcome_label.to_string()),
        direction: Some(directional.direction.to_string()),
        market_slug: market_slug.to_string(),
        event_url: snapshot.event_url,
        timeframe: Some(snapshot.timeframe),
        asset: Some(snapshot.asset),
        price_to_beat: Some(snapshot.price_to_beat),
        price_to_beat_status: Some(snapshot_status),
        price_to_beat_source: Some(snapshot_source),
        price_to_beat_source_latency_ms: snapshot.source_latency_ms,
        current_price: Some(current_price),
        current_price_source,
        directional_gap: Some(directional.directional_gap),
        gap_abs: Some(gap_abs),
        threshold_mode: mode.as_str().to_string(),
        configured_threshold_mode: None,
        base_threshold_value: None,
        base_threshold_unit: None,
        base_threshold_usd: None,
        current_effective_ptb_usd: None,
        threshold_value: resolved_threshold_value,
        threshold_unit: resolved_threshold_unit.as_str().to_string(),
        threshold_usd,
        stop_loss_bump_count: 0,
        stop_loss_bump_applied_count: 0,
        stop_loss_bump_amount: None,
        stop_loss_bump_max_value: None,
        stop_loss_bump_unit: None,
        stop_loss_bump_raw_usd: 0.0,
        stop_loss_bump_usd: 0.0,
        stop_loss_bump_capped: false,
        stop_loss_bump_max_reached: false,
        stop_loss_bump_current_market_excluded: false,
        stop_loss_bump_increment_usd: 0.0,
        reentry_generation: 0,
        reentry_override_active: false,
        reentry_override_value: None,
        reentry_override_unit: None,
        max_price_relax: None,
        auto_threshold_usd,
        lookback_windows_used,
        current_windows_used,
        avg_up_excursion_usd,
        avg_down_excursion_usd,
        lookback_market_slugs,
        lookback_window_snapshots,
        baseline_pct,
        current_pct,
        vol_factor,
        threshold_pct,
        base_pct,
        floor_usd,
        ceiling_usd,
        threshold_was_clamped,
    }
}

fn blocked_price_to_beat_guard_evaluation(
    market_slug: &str,
    event_url: String,
    mode: PriceToBeatMode,
    threshold_value: f64,
    threshold_unit: PriceToBeatDiffUnit,
    threshold_usd: f64,
    reason_code: &str,
    reason_detail: Option<String>,
    timeframe: Option<String>,
    asset: Option<String>,
    price_to_beat: Option<f64>,
    price_to_beat_status: Option<String>,
    price_to_beat_source: Option<String>,
    price_to_beat_source_latency_ms: Option<i64>,
    current_price: Option<f64>,
    auto_threshold_usd: Option<f64>,
    lookback_windows_used: Option<usize>,
    current_windows_used: Option<usize>,
    avg_up_excursion_usd: Option<f64>,
    avg_down_excursion_usd: Option<f64>,
    lookback_market_slugs: Option<Vec<String>>,
    lookback_window_snapshots: Option<Vec<Value>>,
    baseline_pct: Option<f64>,
    current_pct: Option<f64>,
    vol_factor: Option<f64>,
    threshold_pct: Option<f64>,
    base_pct: Option<f64>,
    floor_usd: Option<f64>,
    ceiling_usd: Option<f64>,
    threshold_was_clamped: Option<bool>,
) -> PriceToBeatGuardEvaluation {
    PriceToBeatGuardEvaluation {
        passed: false,
        reason_code: reason_code.to_string(),
        reason_detail,
        normalized_outcome_label: None,
        direction: None,
        market_slug: market_slug.to_string(),
        event_url,
        timeframe,
        asset,
        price_to_beat,
        price_to_beat_status,
        price_to_beat_source,
        price_to_beat_source_latency_ms,
        current_price,
        current_price_source: CURRENT_PRICE_SOURCE_CHAINLINK,
        directional_gap: None,
        gap_abs: None,
        threshold_mode: mode.as_str().to_string(),
        configured_threshold_mode: None,
        base_threshold_value: None,
        base_threshold_unit: None,
        base_threshold_usd: None,
        current_effective_ptb_usd: None,
        threshold_value,
        threshold_unit: threshold_unit.as_str().to_string(),
        threshold_usd,
        stop_loss_bump_count: 0,
        stop_loss_bump_applied_count: 0,
        stop_loss_bump_amount: None,
        stop_loss_bump_max_value: None,
        stop_loss_bump_unit: None,
        stop_loss_bump_raw_usd: 0.0,
        stop_loss_bump_usd: 0.0,
        stop_loss_bump_capped: false,
        stop_loss_bump_max_reached: false,
        stop_loss_bump_current_market_excluded: false,
        stop_loss_bump_increment_usd: 0.0,
        reentry_generation: 0,
        reentry_override_active: false,
        reentry_override_value: None,
        reentry_override_unit: None,
        max_price_relax: None,
        auto_threshold_usd,
        lookback_windows_used,
        current_windows_used,
        avg_up_excursion_usd,
        avg_down_excursion_usd,
        lookback_market_slugs,
        lookback_window_snapshots,
        baseline_pct,
        current_pct,
        vol_factor,
        threshold_pct,
        base_pct,
        floor_usd,
        ceiling_usd,
        threshold_was_clamped,
    }
}

pub(crate) async fn send_price_to_beat_guard_notification(
    repo: &crate::PostgresRepository,
    user_id: i64,
    message: &str,
) -> bool {
    let Ok(telegram) = crate::load_user_telegram_config(repo, user_id).await else {
        return false;
    };
    let bot_token = telegram.bot_token.trim();
    let chat_id = telegram.chat_id.trim();
    if bot_token.is_empty() || chat_id.is_empty() {
        return false;
    }

    let Ok(bot_token) = crate::decrypt_config_string_if_needed("telegram.bot_token", bot_token)
    else {
        return false;
    };
    if bot_token.is_empty() {
        return false;
    }

    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let result = crate::TELEGRAM_HTTP_CLIENT
        .post(&url)
        .json(&json!({
            "chat_id": chat_id,
            "text": message,
        }))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;

    match result {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(user_id, "PRICE_TO_BEAT_GUARD_NOTIFICATION_SENT");
            true
        }
        Ok(resp) => {
            tracing::warn!(
                user_id,
                http_status = resp.status().as_u16(),
                "PRICE_TO_BEAT_GUARD_NOTIFICATION_FAILED"
            );
            false
        }
        Err(err) => {
            tracing::warn!(
                user_id,
                error = %err,
                "PRICE_TO_BEAT_GUARD_NOTIFICATION_FAILED"
            );
            false
        }
    }
}
