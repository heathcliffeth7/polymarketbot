use super::{evaluate_directional_gap, normalize_outcome_direction, PriceToBeatGuardEvaluation};
use anyhow::Result;
use async_trait::async_trait;
use bot_infra::db::{TradeBuilderMarketSecondSnapshot, TradeFlowNodeRuntimeSnapshotRecord};
use serde_json::{json, Value};
use std::collections::HashMap;

const DEFAULT_MAX_PRICE_RELAX_MISS_COUNT: i64 = 5;
const DEFAULT_MAX_PRICE_RELAX_HISTORY_COUNT: usize = 5;
const DEFAULT_MAX_PRICE_RELAX_STEP_PERCENT: f64 = 25.0;
const MAX_PRICE_RELAX_MIN_COUNT: i64 = 1;
const MAX_PRICE_RELAX_MAX_COUNT: i64 = 20;
const MAX_PRICE_RELAX_NOTIFY_MIN_CHANGE_USD: f64 = 0.01;
const NODE_STATE_TRACKING_START_MARKET_SLUG: &str =
    "ptb_max_price_relax_tracking_start_market_slug";
const NODE_STATE_LAST_FILL_MARKET_SLUG: &str = "ptb_max_price_relax_last_fill_market_slug";
const NODE_STATE_LAST_NOTIFIED_THRESHOLD_USD: &str =
    "ptb_max_price_relax_last_notified_threshold_usd";
const NODE_STATE_LAST_NOTIFIED_MARKET_SLUG: &str = "ptb_max_price_relax_last_notified_market_slug";

#[derive(Debug, Clone, PartialEq)]
pub(super) struct ActionPlaceOrderMaxPriceRelaxation {
    pub(super) applied: bool,
    pub(super) target_threshold_usd: Option<f64>,
    pub(super) raw_target_threshold_usd: Option<f64>,
    pub(super) effective_target_threshold_usd: Option<f64>,
    pub(super) min_gap_usd: Option<f64>,
    pub(super) selected_gap_usd: Option<f64>,
    pub(super) relax_credit_usd: f64,
    pub(super) miss_reason: Option<String>,
    pub(super) tradable_seconds_count: i64,
    pub(super) max_fillability_score: Option<f64>,
    pub(super) quality_score: Option<f64>,
    pub(super) buffer_usd: f64,
    pub(super) floor_usd: f64,
    pub(super) miss_streak: i64,
    pub(super) config_miss_count: i64,
    pub(super) config_history_count: usize,
    pub(super) config_step_mode: String,
    pub(super) config_step_value: f64,
    pub(super) config_step_unit: Option<String>,
    pub(super) qualified_market_slugs: Vec<String>,
    pub(super) first_tradable_market_slug: Option<String>,
    pub(super) first_tradable_second_ts: Option<String>,
    pub(super) price_ok_depth_fail_count: i64,
    pub(super) notification_sent: bool,
    pub(super) previous_threshold_usd: Option<f64>,
}

impl ActionPlaceOrderMaxPriceRelaxation {
    fn to_value(&self) -> Value {
        json!({
            "max_price_relax_applied": self.applied,
            "max_price_relax_target_usd": self.target_threshold_usd,
            "max_price_relax_raw_target_usd": self.raw_target_threshold_usd,
            "max_price_relax_effective_target_usd": self.effective_target_threshold_usd,
            "max_price_relax_min_gap_usd": self.min_gap_usd,
            "max_price_relax_selected_gap_usd": self.selected_gap_usd,
            "max_price_relax_relax_credit_usd": self.relax_credit_usd,
            "max_price_relax_miss_reason": self.miss_reason,
            "max_price_relax_tradable_seconds_count": self.tradable_seconds_count,
            "max_price_relax_max_fillability_score": self.max_fillability_score,
            "max_price_relax_quality_score": self.quality_score,
            "max_price_relax_buffer_usd": self.buffer_usd,
            "max_price_relax_floor_usd": self.floor_usd,
            "max_price_relax_miss_streak": self.miss_streak,
            "max_price_relax_config_miss_count": self.config_miss_count,
            "max_price_relax_config_history_count": self.config_history_count,
            "max_price_relax_config_step_mode": self.config_step_mode,
            "max_price_relax_config_step_value": self.config_step_value,
            "max_price_relax_config_step_unit": self.config_step_unit,
            "max_price_relax_qualified_market_slugs": self.qualified_market_slugs,
            "max_price_relax_first_tradable_market_slug": self.first_tradable_market_slug,
            "max_price_relax_first_tradable_second_ts": self.first_tradable_second_ts,
            "max_price_relax_price_ok_depth_fail_count": self.price_ok_depth_fail_count,
            "max_price_relax_notification_sent": self.notification_sent,
            "max_price_relax_previous_threshold_usd": self.previous_threshold_usd,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MaxPriceRelaxationConfig {
    miss_count: i64,
    history_count: usize,
    floor_usd: f64,
    min_depth_usdc: f64,
    target_notional_usdc: f64,
    step_mode: MaxPriceRelaxationStepMode,
    step_value: f64,
    step_unit: Option<crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaxPriceRelaxationStepMode {
    Percent,
    Absolute,
}

impl MaxPriceRelaxationStepMode {
    fn parse(raw: Option<&str>) -> Option<Self> {
        match raw
            .map(str::trim)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "" | "percent" => Some(Self::Percent),
            "absolute" => Some(Self::Absolute),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Percent => "percent",
            Self::Absolute => "absolute",
        }
    }
}

#[async_trait]
trait MaxPriceRelaxationDataSource {
    async fn load_market_second_snapshots(
        &self,
        market_slugs: &[String],
    ) -> Result<HashMap<String, Vec<TradeBuilderMarketSecondSnapshot>>>;
    async fn load_market_runtime_snapshots(
        &self,
        run_id: i64,
        node_key: &str,
        market_slugs: &[String],
    ) -> Result<HashMap<String, TradeFlowNodeRuntimeSnapshotRecord>>;
}

struct LiveMaxPriceRelaxationDataSource<'a> {
    repo: &'a crate::PostgresRepository,
}

#[async_trait]
impl<'a> MaxPriceRelaxationDataSource for LiveMaxPriceRelaxationDataSource<'a> {
    async fn load_market_second_snapshots(
        &self,
        market_slugs: &[String],
    ) -> Result<HashMap<String, Vec<TradeBuilderMarketSecondSnapshot>>> {
        let snapshots = self
            .repo
            .list_trade_builder_market_second_snapshots(market_slugs)
            .await?;
        let mut grouped = HashMap::new();
        for snapshot in snapshots {
            grouped
                .entry(snapshot.market_slug.clone())
                .or_insert_with(Vec::new)
                .push(snapshot);
        }
        Ok(grouped)
    }

    async fn load_market_runtime_snapshots(
        &self,
        run_id: i64,
        node_key: &str,
        market_slugs: &[String],
    ) -> Result<HashMap<String, TradeFlowNodeRuntimeSnapshotRecord>> {
        let rows = self
            .repo
            .list_trade_flow_node_runtime_snapshots_for_markets(run_id, node_key, market_slugs)
            .await?;
        let mut grouped = HashMap::new();
        for row in rows {
            if let Some(market_slug) = row.market_slug.clone() {
                grouped.entry(market_slug).or_insert(row);
            }
        }
        Ok(grouped)
    }
}

fn node_state_market_slug(context: &Value, node_key: &str, state_key: &str) -> Option<String> {
    context
        .get("nodeState")
        .and_then(|value| value.get(node_key))
        .and_then(|value| value.get(state_key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(str::to_string)
}

fn node_state_f64(context: &Value, node_key: &str, state_key: &str) -> Option<f64> {
    context
        .get("nodeState")
        .and_then(|value| value.get(node_key))
        .and_then(|value| value.get(state_key))
        .and_then(crate::value_as_f64)
        .filter(|value| value.is_finite())
}

fn market_cycle_scope(
    market_slug: &str,
) -> Option<(
    &'static str,
    &'static str,
    &'static str,
    chrono::DateTime<chrono::Utc>,
    i64,
)> {
    let scope = crate::find_updown_scope_by_slug(market_slug)?;
    let start = crate::MarketCycleId(market_slug.to_string()).start_time()?;
    Some((
        scope.scope,
        scope.asset,
        scope.slug_prefix,
        start,
        crate::updown_scope_window_seconds(scope),
    ))
}

fn market_slug_matches_scope(candidate: &str, current_market_slug: &str) -> bool {
    let Some((candidate_scope, _, _, _, _)) = market_cycle_scope(candidate) else {
        return false;
    };
    let Some((current_scope, _, _, _, _)) = market_cycle_scope(current_market_slug) else {
        return false;
    };
    candidate_scope == current_scope
}

fn market_windows_since(previous_market_slug: &str, current_market_slug: &str) -> Option<i64> {
    let (_, _, _, previous_start, previous_window_seconds) =
        market_cycle_scope(previous_market_slug)?;
    let (_, _, _, current_start, current_window_seconds) = market_cycle_scope(current_market_slug)?;
    if previous_window_seconds != current_window_seconds {
        return None;
    }
    let delta_seconds = current_start
        .signed_duration_since(previous_start)
        .num_seconds();
    if delta_seconds < 0 {
        return None;
    }
    Some(delta_seconds / previous_window_seconds)
}

fn resolve_fill_less_completed_market_streak(
    context: &Value,
    node_key: &str,
    current_market_slug: &str,
) -> i64 {
    let tracking_start_market_slug =
        node_state_market_slug(context, node_key, NODE_STATE_TRACKING_START_MARKET_SLUG)
            .filter(|slug| market_slug_matches_scope(slug, current_market_slug));
    let last_fill_market_slug =
        node_state_market_slug(context, node_key, NODE_STATE_LAST_FILL_MARKET_SLUG)
            .filter(|slug| market_slug_matches_scope(slug, current_market_slug));

    if let Some(last_fill_market_slug) = last_fill_market_slug.as_deref() {
        return market_windows_since(last_fill_market_slug, current_market_slug)
            .map(|windows| windows.saturating_sub(1))
            .unwrap_or(0)
            .max(0);
    }

    if let Some(tracking_start_market_slug) = tracking_start_market_slug.as_deref() {
        return market_windows_since(tracking_start_market_slug, current_market_slug)
            .unwrap_or(0)
            .max(0);
    }

    0
}

fn recent_fill_less_completed_market_slugs(
    current_market_slug: &str,
    miss_streak: i64,
    limit: usize,
) -> Vec<String> {
    let Some((_, _, slug_prefix, current_start, window_seconds)) =
        market_cycle_scope(current_market_slug)
    else {
        return Vec::new();
    };
    let count = miss_streak.max(0).min(limit as i64) as usize;
    let mut market_slugs = Vec::with_capacity(count);
    for offset in 1..=count {
        let start = current_start - crate::ChronoDuration::seconds(window_seconds * offset as i64);
        market_slugs.push(format!("{slug_prefix}{}", start.timestamp()));
    }
    market_slugs
}

fn resolve_effective_max_price(
    node: &crate::TradeFlowNode,
    context: &Value,
    reentry_generation: i64,
) -> Option<f64> {
    let base_max_price = crate::node_config_f64(node, "maxPriceCent")
        .map(|value| value / 100.0)
        .or_else(|| crate::node_config_f64(node, "maxPrice"))
        .or_else(|| {
            context
                .get("flowContext")
                .and_then(|value| value.get("maxPrice"))
                .and_then(crate::value_as_f64)
        })
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .map(crate::clamp_probability);
    let configured_reentry_max_price = crate::node_config_f64(node, "reentryMaxPriceCent")
        .map(|value| value / 100.0)
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .map(crate::clamp_probability);

    if reentry_generation > 0 {
        configured_reentry_max_price.or(base_max_price)
    } else {
        base_max_price
    }
}

fn resolve_relax_buffer_usd(node: &crate::TradeFlowNode) -> f64 {
    let bump_enabled =
        crate::node_config_bool(node, "priceToBeatStopLossBumpEnabled").unwrap_or(false);
    if !bump_enabled {
        return 0.0;
    }
    let amount = crate::node_config_f64(node, "priceToBeatStopLossBumpAmount").unwrap_or(0.0);
    if !amount.is_finite() || amount <= 0.0 {
        return 0.0;
    }
    let unit = crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::parse(
        crate::node_config_string(node, "priceToBeatStopLossBumpUnit").as_deref(),
    )
    .unwrap_or(crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Usd);
    super::normalize_price_to_beat_threshold_usd(amount, unit)
}

fn resolve_relax_floor_usd(node: &crate::TradeFlowNode, buffer_usd: f64) -> f64 {
    let min_value = crate::node_config_f64(node, "priceToBeatMaxPriceRelaxMinValue")
        .filter(|value| value.is_finite() && *value > 0.0);
    let min_unit = crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::parse(
        crate::node_config_string(node, "priceToBeatMaxPriceRelaxMinUnit").as_deref(),
    )
    .unwrap_or(crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Usd);

    min_value
        .map(|value| super::normalize_price_to_beat_threshold_usd(value, min_unit))
        .unwrap_or(buffer_usd)
        .max(0.0)
}

fn clamp_relax_count(value: Option<i64>, default_value: i64) -> i64 {
    value
        .unwrap_or(default_value)
        .clamp(MAX_PRICE_RELAX_MIN_COUNT, MAX_PRICE_RELAX_MAX_COUNT)
}

fn resolve_max_price_relaxation_config(node: &crate::TradeFlowNode) -> MaxPriceRelaxationConfig {
    let buffer_usd = resolve_relax_buffer_usd(node);
    let step_mode = MaxPriceRelaxationStepMode::parse(
        crate::node_config_string(node, "priceToBeatMaxPriceRelaxStepMode").as_deref(),
    )
    .unwrap_or(MaxPriceRelaxationStepMode::Percent);
    let miss_count = clamp_relax_count(
        crate::node_config_i64(node, "priceToBeatMaxPriceRelaxMissCount"),
        DEFAULT_MAX_PRICE_RELAX_MISS_COUNT,
    );
    let history_count = clamp_relax_count(
        crate::node_config_i64(node, "priceToBeatMaxPriceRelaxHistoryCount"),
        DEFAULT_MAX_PRICE_RELAX_HISTORY_COUNT as i64,
    ) as usize;
    let min_depth_usdc = crate::node_config_f64(node, "priceToBeatMaxPriceRelaxMinDepthUsd")
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(5.0);
    let target_notional_usdc = crate::node_config_f64(node, "sizeUsdc")
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(5.0);
    let step_value = crate::node_config_f64(node, "priceToBeatMaxPriceRelaxStepValue")
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| {
            if step_mode == MaxPriceRelaxationStepMode::Percent {
                value.min(100.0)
            } else {
                value
            }
        })
        .unwrap_or(if step_mode == MaxPriceRelaxationStepMode::Percent {
            DEFAULT_MAX_PRICE_RELAX_STEP_PERCENT
        } else {
            0.0
        });
    let step_unit = (step_mode == MaxPriceRelaxationStepMode::Absolute).then(|| {
        crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::parse(
            crate::node_config_string(node, "priceToBeatMaxPriceRelaxStepUnit").as_deref(),
        )
        .unwrap_or(crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Usd)
    });
    MaxPriceRelaxationConfig {
        miss_count,
        history_count,
        floor_usd: resolve_relax_floor_usd(node, buffer_usd),
        min_depth_usdc,
        target_notional_usdc,
        step_mode,
        step_value,
        step_unit,
    }
}

fn previous_notified_relax_threshold(context: &Value, node_key: &str) -> Option<f64> {
    node_state_f64(context, node_key, NODE_STATE_LAST_NOTIFIED_THRESHOLD_USD)
}

fn previous_notified_relax_market_slug(context: &Value, node_key: &str) -> Option<String> {
    node_state_market_slug(context, node_key, NODE_STATE_LAST_NOTIFIED_MARKET_SLUG)
}

fn snapshot_best_ask_and_depth(
    snapshot: &TradeBuilderMarketSecondSnapshot,
    outcome_label: &str,
) -> Option<(f64, f64)> {
    let (normalized_outcome_label, _) = normalize_outcome_direction(outcome_label)?;
    let (best_ask, ask_depth_usdc) = if normalized_outcome_label == "yes" {
        (snapshot.yes_best_ask, snapshot.yes_ask_depth_usdc)
    } else {
        (snapshot.no_best_ask, snapshot.no_ask_depth_usdc)
    };
    let best_ask = best_ask.filter(|value| value.is_finite() && *value > 0.0)?;
    let ask_depth_usdc = ask_depth_usdc.filter(|value| value.is_finite() && *value > 0.0)?;
    Some((best_ask, ask_depth_usdc))
}

fn snapshot_directional_gap_usd(
    snapshot: &TradeBuilderMarketSecondSnapshot,
    outcome_label: &str,
) -> Option<f64> {
    let chainlink_price = snapshot
        .chainlink_price
        .filter(|value| value.is_finite() && *value > 0.0)?;
    let ptb_ref_price = snapshot
        .ptb_ref_price
        .filter(|value| value.is_finite() && *value > 0.0)?;
    let direction_evaluation =
        evaluate_directional_gap(chainlink_price, ptb_ref_price, 0.0, outcome_label)?;
    direction_evaluation
        .passed
        .then_some(direction_evaluation.directional_gap)
}

fn percentile_gap(candidate_gaps: &[f64], percentile: f64) -> Option<f64> {
    if candidate_gaps.is_empty() {
        return None;
    }
    let mut sorted = candidate_gaps
        .iter()
        .copied()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .collect::<Vec<_>>();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(|left, right| left.total_cmp(right));
    let rank = ((sorted.len() - 1) as f64 * percentile.clamp(0.0, 1.0)).round() as usize;
    sorted.get(rank).copied()
}

fn relax_extra_miss_count(miss_streak: i64, miss_count: i64) -> f64 {
    miss_streak.saturating_sub(miss_count).saturating_add(1) as f64
}

fn relax_credit_usd(
    relax_config: MaxPriceRelaxationConfig,
    current_threshold_usd: f64,
    target_threshold_usd: f64,
    miss_streak: i64,
) -> f64 {
    let relaxable_gap_usd = (current_threshold_usd - target_threshold_usd).max(0.0);
    if relaxable_gap_usd <= 0.0 {
        return 0.0;
    }

    let extra_miss_count = relax_extra_miss_count(miss_streak, relax_config.miss_count);
    let raw_credit_usd = match relax_config.step_mode {
        MaxPriceRelaxationStepMode::Percent => {
            relaxable_gap_usd * (relax_config.step_value / 100.0) * extra_miss_count
        }
        MaxPriceRelaxationStepMode::Absolute => {
            let step_unit = relax_config
                .step_unit
                .unwrap_or(crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit::Usd);
            super::normalize_price_to_beat_threshold_usd(relax_config.step_value, step_unit)
                * extra_miss_count
        }
    };

    raw_credit_usd.clamp(0.0, relaxable_gap_usd)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelaxMissReason {
    GuardMiss,
    MaxPriceMiss,
    DepthMiss,
    SnapshotMissing,
}

impl RelaxMissReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::GuardMiss => "guard_miss",
            Self::MaxPriceMiss => "max_price_miss",
            Self::DepthMiss => "depth_miss",
            Self::SnapshotMissing => "snapshot_missing",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct HistoricalRelaxCandidate {
    market_slug: String,
    miss_reason: RelaxMissReason,
    first_tradable_second_ts: Option<String>,
    first_tradable_gap_usd: Option<f64>,
    tradable_seconds_count: i64,
    price_ok_depth_fail_count: i64,
    max_fillability_score: f64,
    quality_score: f64,
    qualifies_for_relax: bool,
}

fn runtime_snapshot_indicates_guard_miss(
    snapshot: Option<&TradeFlowNodeRuntimeSnapshotRecord>,
) -> bool {
    snapshot
        .and_then(|row| row.snapshot_json.get("output"))
        .and_then(Value::as_object)
        .and_then(|output| output.get("price_to_beat_guard"))
        .and_then(Value::as_object)
        .and_then(|guard| guard.get("passed"))
        .and_then(Value::as_bool)
        == Some(false)
}

fn historical_relax_candidate_from_snapshots(
    market_slug: &str,
    snapshots: Option<&Vec<TradeBuilderMarketSecondSnapshot>>,
    runtime_snapshot: Option<&TradeFlowNodeRuntimeSnapshotRecord>,
    outcome_label: &str,
    max_price: f64,
    relax_config: MaxPriceRelaxationConfig,
) -> HistoricalRelaxCandidate {
    let Some(snapshots) = snapshots else {
        return HistoricalRelaxCandidate {
            market_slug: market_slug.to_string(),
            miss_reason: RelaxMissReason::SnapshotMissing,
            first_tradable_second_ts: None,
            first_tradable_gap_usd: None,
            tradable_seconds_count: 0,
            price_ok_depth_fail_count: 0,
            max_fillability_score: 0.0,
            quality_score: 0.0,
            qualifies_for_relax: false,
        };
    };

    if runtime_snapshot_indicates_guard_miss(runtime_snapshot) {
        return HistoricalRelaxCandidate {
            market_slug: market_slug.to_string(),
            miss_reason: RelaxMissReason::GuardMiss,
            first_tradable_second_ts: None,
            first_tradable_gap_usd: None,
            tradable_seconds_count: 0,
            price_ok_depth_fail_count: 0,
            max_fillability_score: 0.0,
            quality_score: 0.0,
            qualifies_for_relax: false,
        };
    }

    let mut first_tradable_second_ts = None;
    let mut first_tradable_gap_usd = None;
    let mut tradable_seconds_count = 0_i64;
    let mut price_ok_depth_fail_count = 0_i64;
    let mut price_ok_seconds_count = 0_i64;
    let mut max_fillability_score = 0.0_f64;

    for snapshot in snapshots {
        let Some((best_ask, ask_depth_usdc)) = snapshot_best_ask_and_depth(snapshot, outcome_label)
        else {
            continue;
        };
        if best_ask > max_price {
            continue;
        }
        price_ok_seconds_count += 1;
        let fillability_score =
            (ask_depth_usdc / relax_config.target_notional_usdc).clamp(0.0, 1.0);
        max_fillability_score = max_fillability_score.max(fillability_score);
        if ask_depth_usdc < relax_config.min_depth_usdc {
            price_ok_depth_fail_count += 1;
            continue;
        }
        let Some(directional_gap) = snapshot_directional_gap_usd(snapshot, outcome_label) else {
            continue;
        };
        tradable_seconds_count += 1;
        if first_tradable_second_ts.is_none() {
            first_tradable_second_ts = Some(snapshot.second_ts.to_rfc3339());
            first_tradable_gap_usd = Some(directional_gap);
        }
    }

    let quality_score =
        (0.5 * (tradable_seconds_count as f64 / 10.0).min(1.0)) + (0.5 * max_fillability_score);
    let miss_reason = if tradable_seconds_count >= 2 {
        RelaxMissReason::MaxPriceMiss
    } else if price_ok_depth_fail_count > 0 || price_ok_seconds_count > 0 {
        RelaxMissReason::DepthMiss
    } else {
        RelaxMissReason::MaxPriceMiss
    };
    let qualifies_for_relax = miss_reason == RelaxMissReason::MaxPriceMiss
        && tradable_seconds_count >= 2
        && max_fillability_score >= 0.5;

    HistoricalRelaxCandidate {
        market_slug: market_slug.to_string(),
        miss_reason,
        first_tradable_second_ts,
        first_tradable_gap_usd,
        tradable_seconds_count,
        price_ok_depth_fail_count,
        max_fillability_score,
        quality_score,
        qualifies_for_relax,
    }
}

async fn evaluate_relaxation_with_data_source<D>(
    data_source: &D,
    node: &crate::TradeFlowNode,
    context: &Value,
    run_id: i64,
    market_slug: &str,
    outcome_label: &str,
    current_threshold_usd: f64,
    reentry_generation: i64,
) -> Result<ActionPlaceOrderMaxPriceRelaxation>
where
    D: MaxPriceRelaxationDataSource + Send + Sync,
{
    let buffer_usd = resolve_relax_buffer_usd(node);
    let relax_config = resolve_max_price_relaxation_config(node);
    let effective_max_price = resolve_effective_max_price(node, context, reentry_generation);
    let miss_streak = resolve_fill_less_completed_market_streak(context, &node.key, market_slug);
    let mut result = ActionPlaceOrderMaxPriceRelaxation {
        applied: false,
        target_threshold_usd: None,
        raw_target_threshold_usd: None,
        effective_target_threshold_usd: None,
        min_gap_usd: None,
        selected_gap_usd: None,
        relax_credit_usd: 0.0,
        miss_reason: None,
        tradable_seconds_count: 0,
        max_fillability_score: None,
        quality_score: None,
        buffer_usd,
        floor_usd: relax_config.floor_usd,
        miss_streak,
        config_miss_count: relax_config.miss_count,
        config_history_count: relax_config.history_count,
        config_step_mode: relax_config.step_mode.as_str().to_string(),
        config_step_value: relax_config.step_value,
        config_step_unit: relax_config.step_unit.map(|unit| unit.as_str().to_string()),
        qualified_market_slugs: Vec::new(),
        first_tradable_market_slug: None,
        first_tradable_second_ts: None,
        price_ok_depth_fail_count: 0,
        notification_sent: false,
        previous_threshold_usd: None,
    };

    let Some(max_price) = effective_max_price else {
        return Ok(result);
    };
    if miss_streak < relax_config.miss_count {
        return Ok(result);
    }

    let candidate_market_slugs = recent_fill_less_completed_market_slugs(
        market_slug,
        miss_streak,
        relax_config.history_count,
    );
    if candidate_market_slugs.is_empty() {
        return Ok(result);
    }
    let historical_snapshots = data_source
        .load_market_second_snapshots(&candidate_market_slugs)
        .await?;
    let historical_runtime_snapshots = data_source
        .load_market_runtime_snapshots(run_id, &node.key, &candidate_market_slugs)
        .await?;
    let mut min_gap_usd: Option<f64> = None;
    let mut candidate_gaps = Vec::new();
    let mut selected_candidate: Option<HistoricalRelaxCandidate> = None;
    let mut first_observed_miss_reason: Option<RelaxMissReason> = None;

    for historical_market_slug in candidate_market_slugs {
        let candidate = historical_relax_candidate_from_snapshots(
            &historical_market_slug,
            historical_snapshots.get(&historical_market_slug),
            historical_runtime_snapshots.get(&historical_market_slug),
            outcome_label,
            max_price,
            relax_config,
        );
        if first_observed_miss_reason.is_none() {
            first_observed_miss_reason = Some(candidate.miss_reason);
        }
        if matches!(candidate.miss_reason, RelaxMissReason::DepthMiss) {
            result.price_ok_depth_fail_count += candidate.price_ok_depth_fail_count;
        }
        if candidate.qualifies_for_relax {
            result
                .qualified_market_slugs
                .push(historical_market_slug.clone());
            if let Some(gap) = candidate.first_tradable_gap_usd {
                candidate_gaps.push(gap);
                min_gap_usd = Some(
                    min_gap_usd
                        .map(|current_min| current_min.min(gap))
                        .unwrap_or(gap),
                );
            }
            if selected_candidate.is_none() {
                selected_candidate = Some(candidate);
            }
        }
    }

    result.min_gap_usd = min_gap_usd;
    result.selected_gap_usd = percentile_gap(&candidate_gaps, 0.25).or(min_gap_usd);
    result.raw_target_threshold_usd = result
        .selected_gap_usd
        .map(|selected_gap| (selected_gap + buffer_usd).max(0.0));
    result.target_threshold_usd = result
        .raw_target_threshold_usd
        .map(|target| target.max(relax_config.floor_usd));
    result.relax_credit_usd = result
        .target_threshold_usd
        .map(|target| relax_credit_usd(relax_config, current_threshold_usd, target, miss_streak))
        .unwrap_or(0.0);
    result.effective_target_threshold_usd = result
        .target_threshold_usd
        .map(|target| (current_threshold_usd - result.relax_credit_usd).max(target));
    if let Some(selected_candidate) = selected_candidate {
        result.miss_reason = Some(selected_candidate.miss_reason.as_str().to_string());
        result.first_tradable_market_slug = Some(selected_candidate.market_slug);
        result.first_tradable_second_ts = selected_candidate.first_tradable_second_ts;
        result.tradable_seconds_count = selected_candidate.tradable_seconds_count;
        result.max_fillability_score = Some(selected_candidate.max_fillability_score);
        result.quality_score = Some(selected_candidate.quality_score);
    } else if let Some(miss_reason) = first_observed_miss_reason {
        result.miss_reason = Some(miss_reason.as_str().to_string());
    }
    result.applied = result
        .effective_target_threshold_usd
        .map(|target| target < current_threshold_usd)
        .unwrap_or(false);
    Ok(result)
}

pub(super) fn ensure_max_price_relax_tracking_market(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
) {
    let existing = node_state_market_slug(context, node_key, NODE_STATE_TRACKING_START_MARKET_SLUG);
    if existing
        .as_deref()
        .map(|slug| market_slug_matches_scope(slug, market_slug))
        == Some(true)
    {
        return;
    }
    crate::set_flow_node_state(
        context,
        node_key,
        NODE_STATE_TRACKING_START_MARKET_SLUG,
        json!(market_slug),
    );
}

pub(crate) fn note_max_price_relax_fill_market(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
) {
    ensure_max_price_relax_tracking_market(context, node_key, market_slug);
    crate::set_flow_node_state(
        context,
        node_key,
        NODE_STATE_LAST_FILL_MARKET_SLUG,
        json!(market_slug),
    );
}

fn update_evaluation_after_relaxation(
    evaluation: &mut PriceToBeatGuardEvaluation,
    relaxation: &ActionPlaceOrderMaxPriceRelaxation,
) {
    if relaxation.applied {
        if let Some(target_threshold_usd) = relaxation.effective_target_threshold_usd {
            evaluation.threshold_usd = target_threshold_usd;
            evaluation.threshold_value = if evaluation.threshold_unit == "cent" {
                target_threshold_usd * 100.0
            } else {
                target_threshold_usd
            };
            evaluation.auto_threshold_usd = Some(target_threshold_usd);
            if let (Some(current_price), Some(price_to_beat), Some(outcome_label)) = (
                evaluation.current_price,
                evaluation.price_to_beat,
                evaluation.normalized_outcome_label.as_deref(),
            ) {
                if let Some(direction_evaluation) = evaluate_directional_gap(
                    current_price,
                    price_to_beat,
                    target_threshold_usd,
                    outcome_label,
                ) {
                    evaluation.passed = direction_evaluation.passed;
                    evaluation.directional_gap = Some(direction_evaluation.directional_gap);
                    evaluation.gap_abs = Some((current_price - price_to_beat).abs());
                    evaluation.reason_code = if direction_evaluation.passed {
                        "passed".to_string()
                    } else {
                        "price_to_beat_gap_below_threshold".to_string()
                    };
                    evaluation.reason_detail = (!direction_evaluation.passed).then(|| {
                        format!(
                            "directional gap {:.8} (direction={}) is below threshold {:.8} {} (~{:.8} usd)",
                            direction_evaluation.directional_gap,
                            direction_evaluation.direction,
                            target_threshold_usd,
                            "usd",
                            target_threshold_usd
                        )
                    });
                }
            }
        }
    }
}

fn should_notify_relax_threshold_change(
    previous_market_slug: Option<&str>,
    previous_threshold_usd: Option<f64>,
    market_slug: &str,
    next_threshold_usd: f64,
) -> bool {
    if previous_market_slug != Some(market_slug) {
        return true;
    }
    previous_threshold_usd
        .map(|previous| {
            (previous - next_threshold_usd).abs() >= MAX_PRICE_RELAX_NOTIFY_MIN_CHANGE_USD
        })
        .unwrap_or(true)
}

fn set_last_notified_relax_threshold(
    context: &mut Value,
    node_key: &str,
    market_slug: &str,
    threshold_usd: f64,
) {
    crate::set_flow_node_state(
        context,
        node_key,
        NODE_STATE_LAST_NOTIFIED_THRESHOLD_USD,
        json!(threshold_usd),
    );
    crate::set_flow_node_state(
        context,
        node_key,
        NODE_STATE_LAST_NOTIFIED_MARKET_SLUG,
        json!(market_slug),
    );
}

async fn maybe_notify_relax_threshold_change(
    repo: &crate::PostgresRepository,
    user_id: i64,
    context: &mut Value,
    node: &crate::TradeFlowNode,
    market_slug: &str,
    evaluation: &PriceToBeatGuardEvaluation,
    relaxation: &mut ActionPlaceOrderMaxPriceRelaxation,
) -> Result<()> {
    let Some(next_threshold_usd) = relaxation.effective_target_threshold_usd else {
        return Ok(());
    };
    let previous_market_slug = previous_notified_relax_market_slug(context, &node.key);
    let previous_threshold_usd = previous_notified_relax_threshold(context, &node.key);
    relaxation.previous_threshold_usd = previous_threshold_usd;

    if !relaxation.applied
        || !should_notify_relax_threshold_change(
            previous_market_slug.as_deref(),
            previous_threshold_usd,
            market_slug,
            next_threshold_usd,
        )
    {
        return Ok(());
    }

    let message = super::notification::build_price_to_beat_relax_changed_notification_message(
        evaluation,
        previous_threshold_usd,
        relaxation.raw_target_threshold_usd,
        next_threshold_usd,
        relaxation.min_gap_usd,
        relaxation.buffer_usd,
        relaxation.floor_usd,
        relaxation.miss_streak,
        &relaxation.qualified_market_slugs,
    );
    let sent = super::send_price_to_beat_guard_notification(repo, user_id, &message).await;
    relaxation.notification_sent = sent;
    if sent {
        set_last_notified_relax_threshold(context, &node.key, market_slug, next_threshold_usd);
    }
    Ok(())
}

pub(super) async fn maybe_apply_action_place_order_max_price_relaxation(
    repo: &crate::PostgresRepository,
    user_id: i64,
    context: &mut Value,
    node: &crate::TradeFlowNode,
    run_id: i64,
    market_slug: &str,
    outcome_label: &str,
    _cfg: &crate::AppConfig,
    _client: Option<&dyn crate::OrderExecutor>,
    evaluation: &mut PriceToBeatGuardEvaluation,
) -> Result<()> {
    if evaluation.reason_code != "price_to_beat_gap_below_threshold" {
        return Ok(());
    }
    let data_source = LiveMaxPriceRelaxationDataSource { repo };
    let mut relaxation = evaluate_relaxation_with_data_source(
        &data_source,
        node,
        context,
        run_id,
        market_slug,
        outcome_label,
        evaluation.threshold_usd,
        evaluation.reentry_generation,
    )
    .await?;
    update_evaluation_after_relaxation(evaluation, &relaxation);
    maybe_notify_relax_threshold_change(
        repo,
        user_id,
        context,
        node,
        market_slug,
        evaluation,
        &mut relaxation,
    )
    .await?;
    evaluation.max_price_relax = Some(relaxation.to_value());
    Ok(())
}

#[cfg(test)]
mod step_tests;

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDataSource {
        snapshots: HashMap<String, Vec<TradeBuilderMarketSecondSnapshot>>,
        runtime_snapshots: HashMap<String, TradeFlowNodeRuntimeSnapshotRecord>,
    }

    #[async_trait]
    impl MaxPriceRelaxationDataSource for MockDataSource {
        async fn load_market_second_snapshots(
            &self,
            market_slugs: &[String],
        ) -> Result<HashMap<String, Vec<TradeBuilderMarketSecondSnapshot>>> {
            Ok(market_slugs
                .iter()
                .filter_map(|market_slug| {
                    self.snapshots
                        .get(market_slug)
                        .cloned()
                        .map(|rows| (market_slug.clone(), rows))
                })
                .collect())
        }

        async fn load_market_runtime_snapshots(
            &self,
            _run_id: i64,
            _node_key: &str,
            market_slugs: &[String],
        ) -> Result<HashMap<String, TradeFlowNodeRuntimeSnapshotRecord>> {
            Ok(market_slugs
                .iter()
                .filter_map(|market_slug| {
                    self.runtime_snapshots
                        .get(market_slug)
                        .cloned()
                        .map(|row| (market_slug.clone(), row))
                })
                .collect())
        }
    }

    fn test_node(config: Value) -> crate::TradeFlowNode {
        crate::TradeFlowNode {
            key: "action_1".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    fn second_snapshot(
        market_slug: &str,
        second_offset: i64,
        best_ask: f64,
        ask_depth_usdc: f64,
        chainlink_price: f64,
        ptb_ref_price: f64,
        outcome_side: &str,
    ) -> TradeBuilderMarketSecondSnapshot {
        let (window_start, window_end) =
            crate::trade_builder_second_snapshot_window(market_slug).expect("window");
        let second_ts = window_start + crate::ChronoDuration::seconds(second_offset);
        TradeBuilderMarketSecondSnapshot {
            market_slug: market_slug.to_string(),
            asset: "eth".to_string(),
            window_start,
            window_end,
            second_ts,
            ptb_ref_price: Some(ptb_ref_price),
            chainlink_price: Some(chainlink_price),
            yes_best_bid: None,
            yes_best_ask: (outcome_side == "yes").then_some(best_ask),
            yes_ask_depth_usdc: (outcome_side == "yes").then_some(ask_depth_usdc),
            no_best_bid: None,
            no_best_ask: (outcome_side == "no").then_some(best_ask),
            no_ask_depth_usdc: (outcome_side == "no").then_some(ask_depth_usdc),
            sample_count: 1,
        }
    }

    fn assert_close_option(actual: Option<f64>, expected: f64, tolerance: f64) {
        let actual = actual.expect("expected Some(f64)");
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {actual} to be within {tolerance} of {expected}"
        );
    }

    #[tokio::test]
    async fn max_price_relax_applies_after_five_fill_less_markets() {
        let _guard = super::super::tests::lock_price_to_beat_test_state();
        let current_market_slug = "eth-updown-5m-1774117900";
        let node = test_node(json!({
            "priceToBeatGuardEnabled": true,
            "priceToBeatMode": "auto_last_3_avg_excursion",
            "maxPriceCent": 80,
            "priceToBeatStopLossBumpEnabled": true,
            "priceToBeatStopLossBumpAmount": 10,
            "priceToBeatStopLossBumpUnit": "cent",
        }));
        let context = json!({
            "nodeState": {
                "action_1": {
                    "ptb_max_price_relax_tracking_start_market_slug": "eth-updown-5m-1774116100"
                }
            }
        });

        let current_start = crate::MarketCycleId(current_market_slug.to_string())
            .start_time()
            .expect("current start");
        let window_ms = 300_000_i64;
        let mut snapshots = HashMap::new();
        for offset in 1..=5 {
            let market_start =
                current_start - crate::ChronoDuration::milliseconds(window_ms * offset);
            let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
            let gap = match offset {
                1 => 1.25,
                2 => 1.05,
                3 => 1.30,
                4 => 1.10,
                _ => 1.40,
            };
            snapshots.insert(
                market_slug.clone(),
                vec![
                    second_snapshot(&market_slug, 120, 0.79, 10.0, 2_000.0 + gap, 2_000.0, "yes"),
                    second_snapshot(
                        &market_slug,
                        121,
                        0.79,
                        10.0,
                        2_000.0 + gap + 0.01,
                        2_000.0,
                        "yes",
                    ),
                ],
            );
        }

        let relaxation = evaluate_relaxation_with_data_source(
            &MockDataSource {
                snapshots,
                runtime_snapshots: HashMap::new(),
            },
            &node,
            &context,
            1,
            current_market_slug,
            "Up",
            1.80,
            0,
        )
        .await
        .expect("relaxation");

        assert!(relaxation.applied);
        assert_eq!(relaxation.miss_streak, 6);
        assert_close_option(relaxation.min_gap_usd, 1.05, 1e-9);
        assert_close_option(relaxation.selected_gap_usd, 1.10, 1e-9);
        assert_eq!(relaxation.buffer_usd, 0.10);
        assert_close_option(relaxation.target_threshold_usd, 1.20, 1e-9);
        assert_close_option(relaxation.effective_target_threshold_usd, 1.50, 1e-9);
        assert!(
            (relaxation.relax_credit_usd - 0.30).abs() <= 1e-9,
            "expected relax credit to be 0.30, got {}",
            relaxation.relax_credit_usd
        );
        assert_eq!(relaxation.qualified_market_slugs.len(), 5);
    }

    #[tokio::test]
    async fn max_price_relax_skips_when_no_qualified_market_exists() {
        let _guard = super::super::tests::lock_price_to_beat_test_state();
        let current_market_slug = "eth-updown-5m-1774117900";
        let node = test_node(json!({
            "priceToBeatGuardEnabled": true,
            "priceToBeatMode": "auto_vol_pct",
            "maxPriceCent": 80,
            "priceToBeatStopLossBumpEnabled": true,
            "priceToBeatStopLossBumpAmount": 10,
            "priceToBeatStopLossBumpUnit": "cent",
        }));
        let context = json!({
            "nodeState": {
                "action_1": {
                    "ptb_max_price_relax_tracking_start_market_slug": "eth-updown-5m-1774116100"
                }
            }
        });

        let current_start = crate::MarketCycleId(current_market_slug.to_string())
            .start_time()
            .expect("current start");
        let window_ms = 300_000_i64;
        let mut snapshots = HashMap::new();
        for offset in 1..=5 {
            let market_start =
                current_start - crate::ChronoDuration::milliseconds(window_ms * offset);
            let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
            snapshots.insert(
                market_slug.clone(),
                vec![second_snapshot(
                    &market_slug,
                    120,
                    0.92,
                    10.0,
                    2_101.0,
                    2_100.0,
                    "yes",
                )],
            );
        }

        let relaxation = evaluate_relaxation_with_data_source(
            &MockDataSource {
                snapshots,
                runtime_snapshots: HashMap::new(),
            },
            &node,
            &context,
            1,
            current_market_slug,
            "Up",
            1.80,
            0,
        )
        .await
        .expect("relaxation");

        assert!(!relaxation.applied);
        assert_eq!(relaxation.min_gap_usd, None);
        assert_eq!(relaxation.target_threshold_usd, None);
        assert!(relaxation.qualified_market_slugs.is_empty());
    }

    #[tokio::test]
    async fn max_price_relax_uses_configured_miss_and_history_counts() {
        let _guard = super::super::tests::lock_price_to_beat_test_state();
        let current_market_slug = "eth-updown-5m-1774117900";
        let node = test_node(json!({
            "priceToBeatGuardEnabled": true,
            "priceToBeatMode": "auto_vol_pct",
            "maxPriceCent": 80,
            "priceToBeatStopLossBumpEnabled": true,
            "priceToBeatStopLossBumpAmount": 10,
            "priceToBeatStopLossBumpUnit": "cent",
            "priceToBeatMaxPriceRelaxMissCount": 3,
            "priceToBeatMaxPriceRelaxHistoryCount": 3
        }));
        let context = json!({
            "nodeState": {
                "action_1": {
                    "ptb_max_price_relax_tracking_start_market_slug": "eth-updown-5m-1774116100"
                }
            }
        });

        let current_start = crate::MarketCycleId(current_market_slug.to_string())
            .start_time()
            .expect("current start");
        let window_ms = 300_000_i64;
        let mut snapshots = HashMap::new();
        for offset in 1..=5 {
            let market_start =
                current_start - crate::ChronoDuration::milliseconds(window_ms * offset);
            let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
            let gap = 1.0 + (offset as f64 * 0.1);
            snapshots.insert(
                market_slug.clone(),
                vec![
                    second_snapshot(&market_slug, 120, 0.79, 10.0, 2_500.0 + gap, 2_500.0, "yes"),
                    second_snapshot(
                        &market_slug,
                        121,
                        0.79,
                        10.0,
                        2_500.0 + gap + 0.01,
                        2_500.0,
                        "yes",
                    ),
                ],
            );
        }

        let relaxation = evaluate_relaxation_with_data_source(
            &MockDataSource {
                snapshots,
                runtime_snapshots: HashMap::new(),
            },
            &node,
            &context,
            1,
            current_market_slug,
            "Up",
            1.80,
            0,
        )
        .await
        .expect("relaxation");

        assert!(relaxation.applied);
        assert_eq!(relaxation.config_miss_count, 3);
        assert_eq!(relaxation.config_history_count, 3);
        assert_eq!(relaxation.qualified_market_slugs.len(), 3);
    }

    #[tokio::test]
    async fn max_price_relax_allows_manual_ptb_mode() {
        let _guard = super::super::tests::lock_price_to_beat_test_state();
        let current_market_slug = "eth-updown-5m-1774117900";
        let node = test_node(json!({
            "priceToBeatGuardEnabled": true,
            "priceToBeatMode": "manual",
            "priceToBeatMaxDiff": 80,
            "priceToBeatMaxDiffUnit": "cent",
            "maxPriceCent": 80,
            "priceToBeatMaxPriceRelaxMissCount": 3,
            "priceToBeatMaxPriceRelaxHistoryCount": 3,
            "priceToBeatMaxPriceRelaxMinValue": 15,
            "priceToBeatMaxPriceRelaxMinUnit": "cent"
        }));
        let context = json!({
            "nodeState": {
                "action_1": {
                    "ptb_max_price_relax_tracking_start_market_slug": "eth-updown-5m-1774116100"
                }
            }
        });

        let current_start = crate::MarketCycleId(current_market_slug.to_string())
            .start_time()
            .expect("current start");
        let window_ms = 300_000_i64;
        let mut snapshots = HashMap::new();
        for offset in 1..=3 {
            let market_start =
                current_start - crate::ChronoDuration::milliseconds(window_ms * offset);
            let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
            snapshots.insert(
                market_slug.clone(),
                vec![
                    second_snapshot(&market_slug, 120, 0.79, 10.0, 2_601.1, 2_600.0, "yes"),
                    second_snapshot(&market_slug, 121, 0.79, 10.0, 2_601.11, 2_600.0, "yes"),
                ],
            );
        }

        let relaxation = evaluate_relaxation_with_data_source(
            &MockDataSource {
                snapshots,
                runtime_snapshots: HashMap::new(),
            },
            &node,
            &context,
            1,
            current_market_slug,
            "Up",
            2.00,
            0,
        )
        .await
        .expect("relaxation");

        assert!(relaxation.applied);
        assert_eq!(relaxation.config_miss_count, 3);
    }

    #[tokio::test]
    async fn max_price_relax_applies_floor_to_effective_target() {
        let _guard = super::super::tests::lock_price_to_beat_test_state();
        let current_market_slug = "eth-updown-5m-1774117900";
        let node = test_node(json!({
            "priceToBeatGuardEnabled": true,
            "priceToBeatMode": "auto_last_3_avg_excursion",
            "maxPriceCent": 80,
            "priceToBeatStopLossBumpEnabled": true,
            "priceToBeatStopLossBumpAmount": 10,
            "priceToBeatStopLossBumpUnit": "cent",
            "priceToBeatMaxPriceRelaxMinValue": 1.20,
            "priceToBeatMaxPriceRelaxMinUnit": "usd"
        }));
        let context = json!({
            "nodeState": {
                "action_1": {
                    "ptb_max_price_relax_tracking_start_market_slug": "eth-updown-5m-1774116100"
                }
            }
        });

        let current_start = crate::MarketCycleId(current_market_slug.to_string())
            .start_time()
            .expect("current start");
        let window_ms = 300_000_i64;
        let mut snapshots = HashMap::new();
        for offset in 1..=5 {
            let market_start =
                current_start - crate::ChronoDuration::milliseconds(window_ms * offset);
            let market_slug = format!("eth-updown-5m-{}", market_start.timestamp());
            snapshots.insert(
                market_slug.clone(),
                vec![
                    second_snapshot(&market_slug, 120, 0.79, 10.0, 2_801.05, 2_800.0, "yes"),
                    second_snapshot(&market_slug, 121, 0.79, 10.0, 2_801.06, 2_800.0, "yes"),
                ],
            );
        }

        let relaxation = evaluate_relaxation_with_data_source(
            &MockDataSource {
                snapshots,
                runtime_snapshots: HashMap::new(),
            },
            &node,
            &context,
            1,
            current_market_slug,
            "Up",
            1.80,
            0,
        )
        .await
        .expect("relaxation");

        assert_close_option(relaxation.raw_target_threshold_usd, 1.15, 1e-9);
        assert_close_option(relaxation.target_threshold_usd, 1.20, 1e-9);
        assert_close_option(relaxation.effective_target_threshold_usd, 1.50, 1e-9);
        assert_eq!(relaxation.floor_usd, 1.20);
    }

    #[test]
    fn fill_less_miss_streak_resets_from_last_fill_market() {
        let current_market_slug = "eth-updown-5m-1774117900";
        let context = json!({
            "nodeState": {
                "action_1": {
                    "ptb_max_price_relax_tracking_start_market_slug": "eth-updown-5m-1774116100",
                    "ptb_max_price_relax_last_fill_market_slug": "eth-updown-5m-1774117300"
                }
            }
        });

        assert_eq!(
            resolve_fill_less_completed_market_streak(&context, "action_1", current_market_slug),
            1
        );
    }

    #[test]
    fn relax_notification_requires_meaningful_same_market_change() {
        assert!(!should_notify_relax_threshold_change(
            Some("eth-updown-5m-1"),
            Some(1.00),
            "eth-updown-5m-1",
            1.009
        ));
        assert!(should_notify_relax_threshold_change(
            Some("eth-updown-5m-1"),
            Some(1.00),
            "eth-updown-5m-1",
            1.01
        ));
        assert!(should_notify_relax_threshold_change(
            Some("eth-updown-5m-1"),
            Some(1.00),
            "eth-updown-5m-2",
            1.001
        ));
    }
}
