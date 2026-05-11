use super::iv_mismatch_protection::PriceToBeatIvBookQuotes;
use serde_json::{json, Map, Value};

pub(crate) const DEFAULT_ADAPTIVE_LOOKBACK_DAYS: i64 = 7;
pub(crate) const DEFAULT_ADAPTIVE_VOLUME_WINDOW_SEC: i64 = 30;
pub(crate) const DEFAULT_ADAPTIVE_VOLUME_MIN_SAMPLES: i64 = 20;
pub(crate) const DEFAULT_LOW_HOURLY_VOLUME_RATIO: f64 = 0.70;
pub(crate) const DEFAULT_HIGH_HOURLY_VOLUME_RATIO: f64 = 1.50;
pub(crate) const DEFAULT_EXTREME_HOURLY_VOLUME_RATIO: f64 = 3.00;
pub(crate) const DEFAULT_BOOK_RELIABILITY_THRESHOLD: f64 = 0.60;
pub(crate) const DEFAULT_ADAPTIVE_GREEN_EDGE_DELTA: f64 = -0.01;
pub(crate) const DEFAULT_ADAPTIVE_GREEN_GAP_STRENGTH_DELTA: f64 = -0.03;
pub(crate) const DEFAULT_ADAPTIVE_ORANGE_EDGE_DELTA: f64 = 0.03;
pub(crate) const DEFAULT_ADAPTIVE_ORANGE_GAP_STRENGTH_DELTA: f64 = 0.15;
pub(crate) const DEFAULT_ADAPTIVE_ORANGE_GAP_USD_MARGIN_DELTA: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PriceToBeatIvVolumeBaselineMode {
    Off,
    Hourly,
}

impl PriceToBeatIvVolumeBaselineMode {
    pub(crate) fn parse(raw: Option<&str>) -> Option<Self> {
        match raw
            .map(str::trim)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "" | "off" => Some(Self::Off),
            "hourly" => Some(Self::Hourly),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvAdaptiveConfig {
    pub(crate) volume_baseline_mode: PriceToBeatIvVolumeBaselineMode,
    pub(crate) volume_baseline_lookback_days: i64,
    pub(crate) volume_window_sec: i64,
    pub(crate) volume_baseline_min_samples: i64,
    pub(crate) low_hourly_volume_ratio: f64,
    pub(crate) high_hourly_volume_ratio: f64,
    pub(crate) extreme_hourly_volume_ratio: f64,
    pub(crate) book_reliability_threshold: f64,
    pub(crate) green_edge_delta: f64,
    pub(crate) green_gap_strength_delta: f64,
    pub(crate) orange_edge_delta: f64,
    pub(crate) orange_gap_strength_delta: f64,
    pub(crate) orange_gap_usd_margin_delta: f64,
    pub(crate) red_block: bool,
}

impl Default for PriceToBeatIvAdaptiveConfig {
    fn default() -> Self {
        Self {
            volume_baseline_mode: PriceToBeatIvVolumeBaselineMode::Off,
            volume_baseline_lookback_days: DEFAULT_ADAPTIVE_LOOKBACK_DAYS,
            volume_window_sec: DEFAULT_ADAPTIVE_VOLUME_WINDOW_SEC,
            volume_baseline_min_samples: DEFAULT_ADAPTIVE_VOLUME_MIN_SAMPLES,
            low_hourly_volume_ratio: DEFAULT_LOW_HOURLY_VOLUME_RATIO,
            high_hourly_volume_ratio: DEFAULT_HIGH_HOURLY_VOLUME_RATIO,
            extreme_hourly_volume_ratio: DEFAULT_EXTREME_HOURLY_VOLUME_RATIO,
            book_reliability_threshold: DEFAULT_BOOK_RELIABILITY_THRESHOLD,
            green_edge_delta: DEFAULT_ADAPTIVE_GREEN_EDGE_DELTA,
            green_gap_strength_delta: DEFAULT_ADAPTIVE_GREEN_GAP_STRENGTH_DELTA,
            orange_edge_delta: DEFAULT_ADAPTIVE_ORANGE_EDGE_DELTA,
            orange_gap_strength_delta: DEFAULT_ADAPTIVE_ORANGE_GAP_STRENGTH_DELTA,
            orange_gap_usd_margin_delta: DEFAULT_ADAPTIVE_ORANGE_GAP_USD_MARGIN_DELTA,
            red_block: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvAdaptiveVolumeInput {
    pub(crate) baseline_mode: PriceToBeatIvVolumeBaselineMode,
    pub(crate) volume_window_sec: i64,
    pub(crate) current_volume_usdc: Option<f64>,
    pub(crate) baseline_median_usdc: Option<f64>,
    pub(crate) baseline_sample_count: Option<i64>,
    pub(crate) baseline_key: Option<String>,
    pub(crate) baseline_status: &'static str,
}

impl PriceToBeatIvAdaptiveVolumeInput {
    pub(crate) fn neutral(
        baseline_mode: PriceToBeatIvVolumeBaselineMode,
        volume_window_sec: i64,
        baseline_key: Option<String>,
        baseline_status: &'static str,
    ) -> Self {
        Self {
            baseline_mode,
            volume_window_sec,
            current_volume_usdc: None,
            baseline_median_usdc: None,
            baseline_sample_count: None,
            baseline_key,
            baseline_status,
        }
    }

    fn hourly_volume_ratio(&self) -> Option<f64> {
        if self.baseline_mode != PriceToBeatIvVolumeBaselineMode::Hourly {
            return None;
        }
        let current = self.current_volume_usdc?;
        let baseline = self.baseline_median_usdc?;
        (current.is_finite() && current >= 0.0 && baseline.is_finite() && baseline >= 0.0)
            .then_some((current + 1.0) / (baseline + 1.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvAdaptiveInput<'a> {
    pub(crate) selected_side: &'a str,
    pub(crate) seconds_left: f64,
    pub(crate) book_quotes: Option<PriceToBeatIvBookQuotes>,
    pub(crate) book_lead_guard_enabled: bool,
    pub(crate) book_lead_under_sec: f64,
    pub(crate) book_lead_min_mid_diff: f64,
    pub(crate) binance_same_direction: Option<bool>,
    pub(crate) zero_cross_count: usize,
    pub(crate) chop_zero_cross_limit: usize,
    pub(crate) base_min_edge: f64,
    pub(crate) base_gap_strength: f64,
    pub(crate) base_gap_usd_margin: Option<f64>,
    pub(crate) volume: Option<&'a PriceToBeatIvAdaptiveVolumeInput>,
    pub(crate) config: &'a PriceToBeatIvAdaptiveConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvAdaptiveEvaluation {
    pub(crate) regime: &'static str,
    pub(crate) reason: &'static str,
    pub(crate) block_reason: Option<&'static str>,
    pub(crate) edge_delta: f64,
    pub(crate) gap_strength_delta: f64,
    pub(crate) gap_usd_margin_delta: f64,
    pub(crate) hourly_volume_ratio: Option<f64>,
    pub(crate) volume_baseline_key: Option<String>,
    pub(crate) volume_baseline_median: Option<f64>,
    pub(crate) volume_baseline_sample_count: Option<i64>,
    pub(crate) current_volume: Option<f64>,
    pub(crate) volume_window_sec: Option<i64>,
    pub(crate) volume_baseline_status: Option<&'static str>,
    pub(crate) book_reliability: Option<f64>,
    pub(crate) spread_score: Option<f64>,
    pub(crate) volume_score: Option<f64>,
    pub(crate) risk_score: f64,
    pub(crate) base_min_edge: f64,
    pub(crate) adaptive_min_edge: f64,
    pub(crate) base_gap_strength: f64,
    pub(crate) adaptive_gap_strength: f64,
    pub(crate) base_gap_usd_margin: Option<f64>,
    pub(crate) adaptive_gap_usd_margin: Option<f64>,
}

impl PriceToBeatIvAdaptiveEvaluation {
    fn neutral(input: &PriceToBeatIvAdaptiveInput<'_>, reason: &'static str) -> Self {
        let base_gap_usd_margin = input.base_gap_usd_margin;
        Self {
            regime: "yellow",
            reason,
            block_reason: None,
            edge_delta: 0.0,
            gap_strength_delta: 0.0,
            gap_usd_margin_delta: 0.0,
            hourly_volume_ratio: None,
            volume_baseline_key: None,
            volume_baseline_median: None,
            volume_baseline_sample_count: None,
            current_volume: None,
            volume_window_sec: None,
            volume_baseline_status: None,
            book_reliability: None,
            spread_score: None,
            volume_score: None,
            risk_score: 0.0,
            base_min_edge: input.base_min_edge,
            adaptive_min_edge: input.base_min_edge,
            base_gap_strength: input.base_gap_strength,
            adaptive_gap_strength: input.base_gap_strength,
            base_gap_usd_margin,
            adaptive_gap_usd_margin: base_gap_usd_margin,
        }
    }

    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert(
            "hourly_volume_ratio".to_string(),
            json!(self.hourly_volume_ratio),
        );
        obj.insert(
            "volume_baseline_key".to_string(),
            json!(self.volume_baseline_key),
        );
        obj.insert(
            "volume_baseline_median".to_string(),
            json!(self.volume_baseline_median),
        );
        obj.insert(
            "volume_baseline_sample_count".to_string(),
            json!(self.volume_baseline_sample_count),
        );
        obj.insert("current_volume_30s".to_string(), json!(self.current_volume));
        obj.insert(
            "volume_window_sec".to_string(),
            json!(self.volume_window_sec),
        );
        obj.insert(
            "volume_baseline_status".to_string(),
            json!(self.volume_baseline_status),
        );
        obj.insert("book_reliability".to_string(), json!(self.book_reliability));
        obj.insert("spread_score".to_string(), json!(self.spread_score));
        obj.insert("volume_score".to_string(), json!(self.volume_score));
        obj.insert("adaptive_regime".to_string(), json!(self.regime));
        obj.insert("risk_score".to_string(), json!(self.risk_score));
        obj.insert("base_min_edge".to_string(), json!(self.base_min_edge));
        obj.insert(
            "adaptive_min_edge".to_string(),
            json!(self.adaptive_min_edge),
        );
        obj.insert(
            "base_gap_strength".to_string(),
            json!(self.base_gap_strength),
        );
        obj.insert(
            "adaptive_gap_strength".to_string(),
            json!(self.adaptive_gap_strength),
        );
        obj.insert(
            "base_gap_usd_margin".to_string(),
            json!(self.base_gap_usd_margin),
        );
        obj.insert(
            "adaptive_gap_usd_margin".to_string(),
            json!(self.adaptive_gap_usd_margin),
        );
        obj.insert("adaptive_reason".to_string(), json!(self.reason));
        obj.insert(
            "adaptive_block_reason".to_string(),
            json!(self.block_reason),
        );
    }
}

pub(crate) fn evaluate_price_to_beat_iv_adaptive_volume(
    input: &PriceToBeatIvAdaptiveInput<'_>,
) -> PriceToBeatIvAdaptiveEvaluation {
    let Some(volume) = input.volume else {
        return PriceToBeatIvAdaptiveEvaluation::neutral(input, "adaptive_volume_unavailable");
    };
    let mut evaluation = PriceToBeatIvAdaptiveEvaluation::neutral(input, volume.baseline_status);
    evaluation.volume_baseline_key = volume.baseline_key.clone();
    evaluation.volume_baseline_median = volume.baseline_median_usdc;
    evaluation.volume_baseline_sample_count = volume.baseline_sample_count;
    evaluation.current_volume = volume.current_volume_usdc;
    evaluation.volume_window_sec = Some(volume.volume_window_sec);
    evaluation.volume_baseline_status = Some(volume.baseline_status);
    evaluation.hourly_volume_ratio = volume.hourly_volume_ratio();

    let Some(volume_ratio) = evaluation.hourly_volume_ratio else {
        return evaluation;
    };
    let volume_score = (volume_ratio / 2.0).clamp(0.0, 1.0);
    evaluation.volume_score = Some(volume_score);

    let book_active =
        input.book_lead_guard_enabled && input.seconds_left <= input.book_lead_under_sec.max(0.0);
    let (book_side, spread_score) = if book_active {
        book_side_and_spread_score(input.book_quotes, input.book_lead_min_mid_diff)
    } else {
        (None, None)
    };
    evaluation.spread_score = spread_score;
    evaluation.book_reliability = spread_score.map(|score| score * volume_score);

    let book_same = matches!(
        (book_side, input.selected_side),
        (Some("up"), "up") | (Some("down"), "down")
    );
    let book_opposite = matches!(
        (book_side, input.selected_side),
        (Some("up"), "down") | (Some("down"), "up")
    );
    let reliable_book = evaluation
        .book_reliability
        .map(|value| value >= input.config.book_reliability_threshold)
        .unwrap_or(false);
    let high_volume = volume_ratio >= input.config.high_hourly_volume_ratio;
    let low_volume = volume_ratio < input.config.low_hourly_volume_ratio;
    let extreme_volume = volume_ratio >= input.config.extreme_hourly_volume_ratio;
    let choppy = input.zero_cross_count >= input.chop_zero_cross_limit;

    if input.config.red_block && high_volume && reliable_book && book_opposite {
        return with_regime(
            evaluation,
            input,
            "red",
            "high_volume_book_opposite",
            Some("blocked_adaptive_high_volume_book_opposite"),
            1.0,
        );
    }
    if input.config.red_block && extreme_volume && choppy {
        return with_regime(
            evaluation,
            input,
            "red",
            "extreme_volume_chop",
            Some("blocked_adaptive_extreme_volume_chop"),
            1.0,
        );
    }
    if book_same
        && input.binance_same_direction == Some(true)
        && !choppy
        && volume_ratio >= input.config.low_hourly_volume_ratio
        && !extreme_volume
    {
        return with_deltas(
            evaluation,
            input,
            "green",
            "volume_confirms_selected_side",
            input.config.green_edge_delta,
            input.config.green_gap_strength_delta,
            0.0,
            0.0,
        );
    }
    if book_opposite {
        let reason = if low_volume || !reliable_book {
            "low_reliability_book_opposite"
        } else {
            "book_opposite_soft_risk"
        };
        return with_deltas(
            evaluation,
            input,
            "orange",
            reason,
            input.config.orange_edge_delta,
            input.config.orange_gap_strength_delta,
            input.config.orange_gap_usd_margin_delta,
            0.60,
        );
    }
    if extreme_volume && choppy {
        return with_deltas(
            evaluation,
            input,
            "orange",
            "extreme_volume_chop_soft",
            input.config.orange_edge_delta,
            input.config.orange_gap_strength_delta,
            input.config.orange_gap_usd_margin_delta,
            0.60,
        );
    }

    evaluation.reason = "adaptive_neutral";
    evaluation
}

fn with_regime(
    mut evaluation: PriceToBeatIvAdaptiveEvaluation,
    input: &PriceToBeatIvAdaptiveInput<'_>,
    regime: &'static str,
    reason: &'static str,
    block_reason: Option<&'static str>,
    risk_score: f64,
) -> PriceToBeatIvAdaptiveEvaluation {
    evaluation.regime = regime;
    evaluation.reason = reason;
    evaluation.block_reason = block_reason;
    evaluation.risk_score = risk_score;
    evaluation.adaptive_min_edge = input.base_min_edge + evaluation.edge_delta;
    evaluation.adaptive_gap_strength = input.base_gap_strength + evaluation.gap_strength_delta;
    evaluation.adaptive_gap_usd_margin = input
        .base_gap_usd_margin
        .map(|value| value + evaluation.gap_usd_margin_delta);
    evaluation
}

fn with_deltas(
    mut evaluation: PriceToBeatIvAdaptiveEvaluation,
    input: &PriceToBeatIvAdaptiveInput<'_>,
    regime: &'static str,
    reason: &'static str,
    edge_delta: f64,
    gap_strength_delta: f64,
    gap_usd_margin_delta: f64,
    risk_score: f64,
) -> PriceToBeatIvAdaptiveEvaluation {
    evaluation.edge_delta = edge_delta;
    evaluation.gap_strength_delta = gap_strength_delta;
    evaluation.gap_usd_margin_delta = gap_usd_margin_delta;
    with_regime(evaluation, input, regime, reason, None, risk_score)
}

fn book_side_and_spread_score(
    book_quotes: Option<PriceToBeatIvBookQuotes>,
    min_mid_diff: f64,
) -> (Option<&'static str>, Option<f64>) {
    let Some(book_quotes) = book_quotes else {
        return (None, None);
    };
    let Some((up_bid, up_ask)) = valid_quote(book_quotes.up_bid, book_quotes.up_ask) else {
        return (None, None);
    };
    let Some((down_bid, down_ask)) = valid_quote(book_quotes.down_bid, book_quotes.down_ask) else {
        return (None, None);
    };
    let up_mid = (up_bid + up_ask) / 2.0;
    let down_mid = (down_bid + down_ask) / 2.0;
    let diff = up_mid - down_mid;
    let side = if diff >= min_mid_diff.max(0.0) {
        Some("up")
    } else if -diff >= min_mid_diff.max(0.0) {
        Some("down")
    } else {
        Some("neutral")
    };
    let max_spread = (up_ask - up_bid).max(down_ask - down_bid);
    let spread_score = if max_spread <= 0.02 {
        1.0
    } else if max_spread <= 0.04 {
        0.6
    } else {
        0.2
    };
    (side, Some(spread_score))
}

fn valid_quote(bid: Option<f64>, ask: Option<f64>) -> Option<(f64, f64)> {
    let bid = bid.filter(|value| value.is_finite() && *value > 0.0 && *value < 1.0)?;
    let ask = ask.filter(|value| value.is_finite() && *value > 0.0 && *value < 1.0)?;
    (ask >= bid).then_some((bid, ask))
}
