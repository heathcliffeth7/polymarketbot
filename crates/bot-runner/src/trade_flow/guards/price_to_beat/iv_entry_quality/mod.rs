use crate::trade_flow::guards::chainlink_price::ChainlinkPriceSample;
use serde_json::{json, Value};

const DEFAULT_NORMAL_MAX_PRICE: f64 = 0.94;
const DEFAULT_PREMIUM_MAX_PRICE: f64 = 0.96;
const DEFAULT_NO_NEW_ENTRY_BELOW_SECONDS: f64 = 8.0;
const DEFAULT_MIN_EXPECTED_MOVE_BPS: f64 = 2.0;
const DEFAULT_GAP_STRENGTH_60_TO_45: f64 = 2.5;
const DEFAULT_GAP_STRENGTH_45_TO_25: f64 = 2.2;
const DEFAULT_GAP_STRENGTH_25_TO_10: f64 = 1.9;
const DEFAULT_GAP_STRENGTH_10_TO_8: f64 = 2.0;
const DEFAULT_BUFFER_RETAIN_5S: f64 = 0.85;
const DEFAULT_BUFFER_RETAIN_10S: f64 = 0.70;
const DEFAULT_PREMIUM_BUFFER_RETAIN_5S: f64 = 0.90;
const DEFAULT_PREMIUM_BUFFER_RETAIN_10S: f64 = 0.75;
const DEFAULT_SPIKE_MULTIPLIER: f64 = 2.5;
const DEFAULT_SPIKE_RETRACE_RATIO: f64 = 0.20;
const DEFAULT_PREMIUM_MAX_SPREAD: f64 = 0.02;
const DEFAULT_PREMIUM_MAX_CHAINLINK_AGE_MS: i64 = 2_500;
const DEFAULT_CEX_ALIGN_MAX_BPS: f64 = 5.0;
const DEFAULT_RISK_SCORE_CLEAN_MAX: f64 = 20.0;
const DEFAULT_RISK_SCORE_MODERATE_MAX: f64 = 45.0;
const DEFAULT_RISK_SCORE_HIGH_MAX: f64 = 70.0;
const DEFAULT_MODERATE_RISK_MAX_PRICE: f64 = 0.74;
const DEFAULT_HIGH_RISK_MAX_PRICE: f64 = 0.70;
const DEFAULT_DEEP_VALUE_MAX_PRICE: f64 = 0.64;
const DEFAULT_MAX_RISK_HAIRCUT_CENT: f64 = 8.0;
const DEFAULT_ODDS_MAX_SPREAD: f64 = 0.05;
const DEFAULT_CEX_UNCONFIRMED_RISK_POINTS: f64 = 10.0;
const DEFAULT_CEX_CONFLICT_RISK_POINTS: f64 = 10.0;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct IvEntryQualityConfig {
    pub(crate) enabled: bool,
    pub(crate) normal_max_price: f64,
    pub(crate) premium_max_price: f64,
    pub(crate) no_new_entry_below_seconds: f64,
    pub(crate) min_expected_move_bps: f64,
    pub(crate) min_expected_move_usd: f64,
    pub(crate) gap_strength_min_60_to_45: f64,
    pub(crate) gap_strength_min_45_to_25: f64,
    pub(crate) gap_strength_min_25_to_10: f64,
    pub(crate) gap_strength_min_10_to_8: f64,
    pub(crate) buffer_trend_guard_enabled: bool,
    pub(crate) buffer_retain_5s: f64,
    pub(crate) buffer_retain_10s: f64,
    pub(crate) premium_buffer_retain_5s: f64,
    pub(crate) premium_buffer_retain_10s: f64,
    pub(crate) spike_fade_guard_enabled: bool,
    pub(crate) spike_multiplier: f64,
    pub(crate) spike_retrace_ratio: f64,
    pub(crate) premium_max_spread: f64,
    pub(crate) premium_max_chainlink_age_ms: i64,
    pub(crate) cex_align_max_usd: Option<f64>,
    pub(crate) cex_align_max_bps: Option<f64>,
    pub(crate) eq77_risk_cap_enabled: bool,
    pub(crate) risk_score_clean_max: f64,
    pub(crate) risk_score_moderate_max: f64,
    pub(crate) risk_score_high_max: f64,
    pub(crate) moderate_risk_max_price: f64,
    pub(crate) high_risk_max_price: f64,
    pub(crate) deep_value_max_price: f64,
    pub(crate) max_risk_haircut_cent: f64,
    pub(crate) wait_for_price_enabled: bool,
    pub(crate) recheck_before_submit: bool,
    pub(crate) odds_max_spread: f64,
    pub(crate) cex_unconfirmed_risk_points: f64,
    pub(crate) cex_conflict_risk_points: f64,
    pub(crate) passive_bid_enabled: bool,
    pub(crate) passive_bid_ttl_ms: i64,
}

impl Default for IvEntryQualityConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            normal_max_price: DEFAULT_NORMAL_MAX_PRICE,
            premium_max_price: DEFAULT_PREMIUM_MAX_PRICE,
            no_new_entry_below_seconds: DEFAULT_NO_NEW_ENTRY_BELOW_SECONDS,
            min_expected_move_bps: DEFAULT_MIN_EXPECTED_MOVE_BPS,
            min_expected_move_usd: 0.0,
            gap_strength_min_60_to_45: DEFAULT_GAP_STRENGTH_60_TO_45,
            gap_strength_min_45_to_25: DEFAULT_GAP_STRENGTH_45_TO_25,
            gap_strength_min_25_to_10: DEFAULT_GAP_STRENGTH_25_TO_10,
            gap_strength_min_10_to_8: DEFAULT_GAP_STRENGTH_10_TO_8,
            buffer_trend_guard_enabled: true,
            buffer_retain_5s: DEFAULT_BUFFER_RETAIN_5S,
            buffer_retain_10s: DEFAULT_BUFFER_RETAIN_10S,
            premium_buffer_retain_5s: DEFAULT_PREMIUM_BUFFER_RETAIN_5S,
            premium_buffer_retain_10s: DEFAULT_PREMIUM_BUFFER_RETAIN_10S,
            spike_fade_guard_enabled: true,
            spike_multiplier: DEFAULT_SPIKE_MULTIPLIER,
            spike_retrace_ratio: DEFAULT_SPIKE_RETRACE_RATIO,
            premium_max_spread: DEFAULT_PREMIUM_MAX_SPREAD,
            premium_max_chainlink_age_ms: DEFAULT_PREMIUM_MAX_CHAINLINK_AGE_MS,
            cex_align_max_usd: None,
            cex_align_max_bps: Some(DEFAULT_CEX_ALIGN_MAX_BPS),
            eq77_risk_cap_enabled: false,
            risk_score_clean_max: DEFAULT_RISK_SCORE_CLEAN_MAX,
            risk_score_moderate_max: DEFAULT_RISK_SCORE_MODERATE_MAX,
            risk_score_high_max: DEFAULT_RISK_SCORE_HIGH_MAX,
            moderate_risk_max_price: DEFAULT_MODERATE_RISK_MAX_PRICE,
            high_risk_max_price: DEFAULT_HIGH_RISK_MAX_PRICE,
            deep_value_max_price: DEFAULT_DEEP_VALUE_MAX_PRICE,
            max_risk_haircut_cent: DEFAULT_MAX_RISK_HAIRCUT_CENT,
            wait_for_price_enabled: true,
            recheck_before_submit: true,
            odds_max_spread: DEFAULT_ODDS_MAX_SPREAD,
            cex_unconfirmed_risk_points: DEFAULT_CEX_UNCONFIRMED_RISK_POINTS,
            cex_conflict_risk_points: DEFAULT_CEX_CONFLICT_RISK_POINTS,
            passive_bid_enabled: false,
            passive_bid_ttl_ms: 1_500,
        }
    }
}

impl IvEntryQualityConfig {
    pub(crate) fn expected_move_floor(&self, current_price: f64) -> f64 {
        let bps_floor = if current_price.is_finite() && self.min_expected_move_bps.is_finite() {
            current_price.abs() * self.min_expected_move_bps.max(0.0) / 10_000.0
        } else {
            0.0
        };
        bps_floor.max(self.min_expected_move_usd.max(0.0))
    }

    pub(crate) fn required_gap_strength(&self, seconds_left: f64) -> f64 {
        if seconds_left > 45.0 {
            self.gap_strength_min_60_to_45
        } else if seconds_left > 25.0 {
            self.gap_strength_min_45_to_25
        } else if seconds_left > 10.0 {
            self.gap_strength_min_25_to_10
        } else {
            self.gap_strength_min_10_to_8
        }
        .max(0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IvEntryQualityReason {
    BelowNoNewEntryWindow,
    GapStrengthTooLow,
    BufferCollapse5s,
    BufferCollapse10s,
    MissingSameSideHistoryForPremium,
    NegativeVelocityPremium,
    SpikeFade,
    PremiumSpreadTooWide,
    PremiumChainlinkStale,
    PremiumCexMisaligned,
    PremiumEvMissing,
    PremiumEvInsufficient,
    PriceAbovePremiumMax,
    PriceAboveEffectiveMax,
    RiskCapHardBlock,
}

impl IvEntryQualityReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::BelowNoNewEntryWindow => "blocked_below_no_new_entry_window",
            Self::GapStrengthTooLow => "blocked_entry_quality_gap_strength_low",
            Self::BufferCollapse5s => "blocked_buffer_collapse_5s",
            Self::BufferCollapse10s => "blocked_buffer_collapse_10s",
            Self::MissingSameSideHistoryForPremium => {
                "blocked_missing_same_side_history_for_premium"
            }
            Self::NegativeVelocityPremium => "blocked_negative_velocity_premium",
            Self::SpikeFade => "blocked_spike_fade",
            Self::PremiumSpreadTooWide => "blocked_premium_spread_too_wide",
            Self::PremiumChainlinkStale => "blocked_premium_chainlink_stale",
            Self::PremiumCexMisaligned => "blocked_premium_cex_misaligned",
            Self::PremiumEvMissing => "blocked_premium_ev_missing",
            Self::PremiumEvInsufficient => "blocked_premium_ev_insufficient",
            Self::PriceAbovePremiumMax => "blocked_price_above_premium_max",
            Self::PriceAboveEffectiveMax => "blocked_price_above_effective_max",
            Self::RiskCapHardBlock => "blocked_eq77_risk_cap_hard_red_flag",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct IvEntryQualityDecision {
    pub(crate) allowed: bool,
    pub(crate) primary_reason: Option<IvEntryQualityReason>,
    pub(crate) all_reasons: Vec<IvEntryQualityReason>,
    pub(crate) expected_move_raw: f64,
    pub(crate) expected_move_eff: f64,
    pub(crate) expected_move_floor_applied: bool,
    pub(crate) required_gap_strength: f64,
    pub(crate) required_gap_strength_with_margin: f64,
    pub(crate) gap_strength: f64,
    pub(crate) gap_strength_hard_floor: Option<f64>,
    pub(crate) gap_strength_deficit: Option<f64>,
    pub(crate) gap_strength_soft_low_ratio: Option<f64>,
    pub(crate) gap_soft_low_risk_points: Option<f64>,
    pub(crate) gap_strength_soft_low: Option<bool>,
    pub(crate) eq77_lite_profile: Option<&'static str>,
    pub(crate) buffer_5s_ago: Option<f64>,
    pub(crate) buffer_10s_ago: Option<f64>,
    pub(crate) same_side_history_5s: Option<bool>,
    pub(crate) same_side_history_10s: Option<bool>,
    pub(crate) buffer_retain_5s: Option<f64>,
    pub(crate) buffer_retain_10s: Option<f64>,
    pub(crate) spike_ratio: Option<f64>,
    pub(crate) spike_retrace_usd: Option<f64>,
    pub(crate) premium_ev_pass: Option<bool>,
    pub(crate) premium_ev_required_price: Option<f64>,
    pub(crate) premium_ev_margin_cent: Option<f64>,
    pub(crate) premium_price_allowed: bool,
    pub(crate) effective_max_buy_price: Option<f64>,
    pub(crate) fair_probability: Option<f64>,
    pub(crate) fee_buffer: Option<f64>,
    pub(crate) min_edge: Option<f64>,
    pub(crate) entry_quality_price_source: &'static str,
    pub(crate) history_price_source: &'static str,
    pub(crate) entry_action: &'static str,
    pub(crate) hard_block: bool,
    pub(crate) deferred: bool,
    pub(crate) signal_recheck_required: bool,
    pub(crate) risk_cap_price_cent: Option<f64>,
    pub(crate) ask_over_cap_cent: Option<f64>,
    pub(crate) risk_score: Option<f64>,
    pub(crate) cap_haircut_cent: Option<f64>,
    pub(crate) risk_level: Option<&'static str>,
    pub(crate) lane: Option<&'static str>,
    pub(crate) size_multiplier: Option<f64>,
    pub(crate) risk_components: Vec<Value>,
    pub(crate) cap_components: Vec<Value>,
}

impl IvEntryQualityDecision {
    pub(crate) fn to_value(&self) -> Value {
        let mut value = json!({
            "allowed": self.allowed,
            "primary_reason": self.primary_reason.map(IvEntryQualityReason::as_str),
            "all_reasons": self.all_reasons.iter().map(|reason| reason.as_str()).collect::<Vec<_>>(),
            "expected_move_raw": self.expected_move_raw,
            "expected_move_eff": self.expected_move_eff,
            "expected_move_floor_applied": self.expected_move_floor_applied,
            "required_gap_strength": self.required_gap_strength,
            "gap_strength": self.gap_strength,
            "buffer_5s_ago": self.buffer_5s_ago,
            "buffer_10s_ago": self.buffer_10s_ago,
            "same_side_history_5s": self.same_side_history_5s,
            "same_side_history_10s": self.same_side_history_10s,
            "buffer_retain_5s": self.buffer_retain_5s,
            "buffer_retain_10s": self.buffer_retain_10s,
            "spike_ratio": self.spike_ratio,
            "spike_retrace_usd": self.spike_retrace_usd,
            "premium_ev_pass": self.premium_ev_pass,
            "premium_ev_required_price": self.premium_ev_required_price,
            "premium_ev_margin_cent": self.premium_ev_margin_cent,
            "premium_price_allowed": self.premium_price_allowed,
            "effective_max_buy_price": self.effective_max_buy_price,
            "fair_probability": self.fair_probability,
            "fee_buffer": self.fee_buffer,
            "min_edge": self.min_edge,
            "entry_quality_price_source": self.entry_quality_price_source,
            "history_price_source": self.history_price_source,
            "entry_action": self.entry_action,
            "hard_block": self.hard_block,
            "deferred": self.deferred,
            "signal_recheck_required": self.signal_recheck_required,
            "risk_cap_price_cent": self.risk_cap_price_cent,
            "ask_over_cap_cent": self.ask_over_cap_cent,
            "risk_score": self.risk_score,
            "cap_haircut_cent": self.cap_haircut_cent,
            "risk_level": self.risk_level,
            "lane": self.lane,
            "size_multiplier": self.size_multiplier,
            "risk_components": self.risk_components,
            "cap_components": self.cap_components,
        });
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "gap_strength_required".to_string(),
                json!(self.required_gap_strength),
            );
            obj.insert(
                "gap_strength_required_with_margin".to_string(),
                json!(self.required_gap_strength_with_margin),
            );
            obj.insert(
                "gap_strength_hard_floor".to_string(),
                json!(self.gap_strength_hard_floor),
            );
            obj.insert(
                "gap_strength_deficit".to_string(),
                json!(self.gap_strength_deficit),
            );
            obj.insert(
                "gap_strength_soft_low_ratio".to_string(),
                json!(self.gap_strength_soft_low_ratio),
            );
            obj.insert(
                "gap_soft_low_risk_points".to_string(),
                json!(self.gap_soft_low_risk_points),
            );
            obj.insert(
                "gap_strength_soft_low".to_string(),
                json!(self.gap_strength_soft_low),
            );
            obj.insert(
                "eq77_lite_profile".to_string(),
                json!(self.eq77_lite_profile),
            );
        }
        value
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct IvEntryQualityInput<'a> {
    pub(crate) config: &'a IvEntryQualityConfig,
    pub(crate) side: &'a str,
    pub(crate) price_to_beat: f64,
    pub(crate) current_price: f64,
    pub(crate) samples: &'a [ChainlinkPriceSample],
    pub(crate) latest_timestamp_ms: i64,
    pub(crate) seconds_left: f64,
    pub(crate) ask: f64,
    pub(crate) spread: f64,
    pub(crate) chainlink_age_ms: Option<i64>,
    pub(crate) expected_move_raw: f64,
    pub(crate) expected_move_eff: f64,
    pub(crate) q_final: Option<f64>,
    pub(crate) fee: f64,
    pub(crate) buffer: f64,
    pub(crate) dynamic_threshold: f64,
    pub(crate) configured_max_price: Option<f64>,
    pub(crate) gap_velocity: Option<f64>,
    pub(crate) cex_price: Option<f64>,
    pub(crate) cex_fresh: bool,
    pub(crate) cex_same_direction: Option<bool>,
    pub(crate) rule_required_gap_strength: Option<f64>,
    pub(crate) rule_gap_strength_margin: Option<f64>,
}

pub(crate) fn evaluate_iv_entry_quality(input: IvEntryQualityInput<'_>) -> IvEntryQualityDecision {
    let config = input.config;
    let gap_now = side_gap(input.side, input.current_price, input.price_to_beat);
    let required_gap_strength = input
        .rule_required_gap_strength
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or_else(|| config.required_gap_strength(input.seconds_left));
    let gap_strength = gap_now / input.expected_move_eff.max(f64::EPSILON);
    let history_5s = history_gap(input, 5_000);
    let history_10s = history_gap(input, 10_000);
    let buffer_retain_5s = history_5s
        .filter(|history| history.same_side)
        .and_then(|history| retain_ratio(gap_now, history.gap));
    let buffer_retain_10s = history_10s
        .filter(|history| history.same_side)
        .and_then(|history| retain_ratio(gap_now, history.gap));
    let spike = config
        .spike_fade_guard_enabled
        .then(|| spike_fade(input))
        .flatten();
    let ask_cent = input.ask * 100.0;
    let is_premium = ask_cent > config.normal_max_price * 100.0;
    let fair_probability = input.q_final.map(|q| q * 100.0);
    let fee_buffer = Some((input.fee + input.buffer.max(0.0)) * 100.0);
    let min_edge = Some(input.dynamic_threshold.max(0.0) * 100.0);
    let premium_ev_required_price =
        Some(ask_cent + fee_buffer.unwrap_or(0.0) + min_edge.unwrap_or(0.0));
    let premium_ev_margin_cent = fair_probability
        .zip(premium_ev_required_price)
        .map(|(fair_probability, required)| fair_probability - required);
    let premium_ev_pass = premium_ev_margin_cent.map(|margin| margin >= 0.0);
    let premium_price_allowed = !is_premium
        || premium_static_checks_pass(input, history_5s, history_10s)
            && premium_ev_pass == Some(true)
            && input.gap_velocity.unwrap_or(0.0) >= 0.0
            && spike.is_none();
    let quality_cap = if is_premium && premium_price_allowed {
        config.premium_max_price * 100.0
    } else {
        config.normal_max_price * 100.0
    };
    let ev_cap =
        fair_probability.map(|fair| fair - fee_buffer.unwrap_or(0.0) - min_edge.unwrap_or(0.0));
    let configured_cap = input
        .configured_max_price
        .filter(|value| value.is_finite())
        .map(|value| value * 100.0);
    let effective_max_buy_price = min_cap(configured_cap, quality_cap, ev_cap);

    if config.eq77_risk_cap_enabled {
        return evaluate_eq77_risk_cap(
            input,
            gap_now,
            required_gap_strength,
            gap_strength,
            history_5s,
            history_10s,
            buffer_retain_5s,
            buffer_retain_10s,
            spike,
            fair_probability,
            fee_buffer,
            min_edge,
            configured_cap,
        );
    }

    let mut all_reasons = Vec::new();
    if input.seconds_left < config.no_new_entry_below_seconds {
        all_reasons.push(IvEntryQualityReason::BelowNoNewEntryWindow);
    }
    if gap_strength < required_gap_strength {
        all_reasons.push(IvEntryQualityReason::GapStrengthTooLow);
    }
    if config.buffer_trend_guard_enabled {
        if buffer_retain_5s
            .map(|retain| retain < config.buffer_retain_5s)
            .unwrap_or(false)
        {
            all_reasons.push(IvEntryQualityReason::BufferCollapse5s);
        }
        if buffer_retain_10s
            .map(|retain| retain < config.buffer_retain_10s)
            .unwrap_or(false)
        {
            all_reasons.push(IvEntryQualityReason::BufferCollapse10s);
        }
    }
    if spike.is_some() {
        all_reasons.push(IvEntryQualityReason::SpikeFade);
    }
    if is_premium {
        append_premium_reasons(
            &mut all_reasons,
            input,
            history_5s,
            history_10s,
            buffer_retain_5s,
            buffer_retain_10s,
            premium_ev_pass,
            effective_max_buy_price,
        );
    } else if ask_cent > effective_max_buy_price.unwrap_or(f64::INFINITY) {
        all_reasons.push(IvEntryQualityReason::PriceAboveEffectiveMax);
    }

    let primary_reason = all_reasons.first().copied();
    IvEntryQualityDecision {
        allowed: primary_reason.is_none(),
        primary_reason,
        all_reasons,
        expected_move_raw: input.expected_move_raw,
        expected_move_eff: input.expected_move_eff,
        expected_move_floor_applied: input.expected_move_eff > input.expected_move_raw,
        required_gap_strength,
        required_gap_strength_with_margin: required_gap_strength,
        gap_strength,
        gap_strength_hard_floor: None,
        gap_strength_deficit: None,
        gap_strength_soft_low_ratio: None,
        gap_soft_low_risk_points: None,
        gap_strength_soft_low: None,
        eq77_lite_profile: None,
        buffer_5s_ago: history_5s.map(|history| history.gap),
        buffer_10s_ago: history_10s.map(|history| history.gap),
        same_side_history_5s: history_5s.map(|history| history.same_side),
        same_side_history_10s: history_10s.map(|history| history.same_side),
        buffer_retain_5s,
        buffer_retain_10s,
        spike_ratio: spike.map(|spike| spike.ratio),
        spike_retrace_usd: spike.map(|spike| spike.retrace),
        premium_ev_pass,
        premium_ev_required_price,
        premium_ev_margin_cent,
        premium_price_allowed,
        effective_max_buy_price,
        fair_probability,
        fee_buffer,
        min_edge,
        entry_quality_price_source: "iv_mismatch_current_price",
        history_price_source: "chainlink_live_data_ws",
        entry_action: if primary_reason.is_none() {
            "submit_order"
        } else {
            "hard_block"
        },
        hard_block: primary_reason.is_some(),
        deferred: false,
        signal_recheck_required: false,
        risk_cap_price_cent: None,
        ask_over_cap_cent: None,
        risk_score: None,
        cap_haircut_cent: None,
        risk_level: None,
        lane: None,
        size_multiplier: None,
        risk_components: Vec::new(),
        cap_components: Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_eq77_risk_cap(
    input: IvEntryQualityInput<'_>,
    gap_now: f64,
    required_gap_strength: f64,
    gap_strength: f64,
    history_5s: Option<HistoryGap>,
    history_10s: Option<HistoryGap>,
    buffer_retain_5s: Option<f64>,
    buffer_retain_10s: Option<f64>,
    spike: Option<SpikeFade>,
    fair_probability: Option<f64>,
    fee_buffer: Option<f64>,
    min_edge: Option<f64>,
    configured_cap: Option<f64>,
) -> IvEntryQualityDecision {
    let config = input.config;
    let ask_cent = input.ask * 100.0;
    let history_3s = history_gap(input, 3_000);
    let impulse_ratio = history_10s
        .map(|history| {
            if history.same_side {
                history.gap.max(0.0)
            } else {
                0.0
            }
        })
        .map(|previous_gap| {
            (gap_now - previous_gap).max(0.0) / input.expected_move_eff.max(f64::EPSILON)
        });
    let stop_cushion = stop_cushion_strength(input.seconds_left, gap_now, input.expected_move_eff);
    let ev_cap =
        fair_probability.map(|fair| fair - fee_buffer.unwrap_or(0.0) - min_edge.unwrap_or(0.0));
    let mut all_reasons = Vec::new();
    let mut risk_components = Vec::new();
    let mut hard_red_flag = false;
    let mut risk_score = 0.0;
    let required_gap_strength_with_margin = required_gap_strength
        + input
            .rule_gap_strength_margin
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or_else(|| risk_gap_margin(input.seconds_left));
    let gap_strength_hard_floor = eq77_gap_strength_hard_floor(input.seconds_left);
    let mut gap_strength_deficit = None;
    let mut gap_strength_soft_low_ratio = None;
    let mut gap_soft_low_risk_points = None;
    let mut gap_strength_soft_low = false;

    if input.seconds_left < config.no_new_entry_below_seconds {
        all_reasons.push(IvEntryQualityReason::BelowNoNewEntryWindow);
        hard_red_flag = true;
    }
    if gap_now <= 0.0 || gap_strength < gap_strength_hard_floor {
        all_reasons.push(IvEntryQualityReason::GapStrengthTooLow);
        hard_red_flag = true;
    } else if gap_strength < required_gap_strength_with_margin {
        let deficit = required_gap_strength_with_margin - gap_strength;
        let soft_range =
            (required_gap_strength_with_margin - gap_strength_hard_floor).max(f64::EPSILON);
        let ratio = (deficit / soft_range).clamp(0.0, 1.0);
        let points = eq77_gap_soft_low_risk_points(ratio);
        risk_score += points;
        gap_strength_deficit = Some(deficit);
        gap_strength_soft_low_ratio = Some(ratio);
        gap_soft_low_risk_points = Some(points);
        gap_strength_soft_low = true;
        push_risk_component(&mut risk_components, "gap_strength_soft_low", points, 0.0);
    }
    if stop_cushion.is_some_and(|value| value < stop_cushion_hard_threshold(input.seconds_left)) {
        push_risk_component(&mut risk_components, "stop_cushion_hard_fail", 35.0, 0.0);
        all_reasons.push(IvEntryQualityReason::RiskCapHardBlock);
        hard_red_flag = true;
    }
    if spike.is_some() && impulse_ratio.unwrap_or(0.0) >= 2.20 {
        push_risk_component(&mut risk_components, "strong_spike_fade", 35.0, 0.0);
        all_reasons.push(IvEntryQualityReason::SpikeFade);
        hard_red_flag = true;
    }
    if buffer_retain_5s.unwrap_or(1.0) < 0.45 && input.gap_velocity.unwrap_or(0.0) < 0.0 {
        push_risk_component(
            &mut risk_components,
            "severe_buffer_decay_negative_velocity",
            30.0,
            0.0,
        );
        all_reasons.push(IvEntryQualityReason::BufferCollapse5s);
        hard_red_flag = true;
    }

    if history_5s.map(|history| !history.same_side).unwrap_or(true) {
        risk_score += 8.0;
        push_risk_component(
            &mut risk_components,
            "missing_same_side_history_5s",
            8.0,
            0.0,
        );
    }
    if history_10s
        .map(|history| !history.same_side)
        .unwrap_or(true)
    {
        risk_score += 4.0;
        push_risk_component(
            &mut risk_components,
            "missing_same_side_history_10s",
            4.0,
            0.0,
        );
    }
    add_threshold_risk(
        &mut risk_score,
        &mut risk_components,
        "buffer_retain_5s",
        buffer_retain_5s,
        config.buffer_retain_5s,
        (config.buffer_retain_5s - 0.20).max(0.45),
        8.0,
        18.0,
    );
    add_threshold_risk(
        &mut risk_score,
        &mut risk_components,
        "buffer_retain_10s",
        buffer_retain_10s,
        config.buffer_retain_10s,
        (config.buffer_retain_10s - 0.15).max(0.25),
        6.0,
        14.0,
    );
    if let Some(ratio) = impulse_ratio {
        if ratio >= 1.70 {
            risk_score += 18.0;
            push_risk_component(&mut risk_components, "impulse_ratio_strong", 18.0, 2.0);
        } else if ratio >= 1.30 {
            risk_score += 10.0;
            push_risk_component(&mut risk_components, "impulse_ratio_mild", 10.0, 1.0);
        }
    }
    if history_3s
        .filter(|history| history.same_side)
        .is_some_and(|history| gap_now < history.gap)
    {
        risk_score += 10.0;
        push_risk_component(&mut risk_components, "deceleration_after_pump", 10.0, 1.0);
    }
    match (input.cex_fresh, input.cex_same_direction) {
        (true, Some(true)) => {}
        (true, Some(false)) => {
            risk_score += config.cex_conflict_risk_points;
            push_risk_component(
                &mut risk_components,
                "cex_chainlink_conflict",
                config.cex_conflict_risk_points,
                2.0,
            );
        }
        _ => {
            risk_score += config.cex_unconfirmed_risk_points;
            push_risk_component(
                &mut risk_components,
                "cex_unconfirmed",
                config.cex_unconfirmed_risk_points,
                0.0,
            );
        }
    }
    if input.spread > config.odds_max_spread {
        risk_score += 8.0;
        push_risk_component(&mut risk_components, "odds_spread_low_confidence", 8.0, 0.0);
    }
    if let Some(cushion) = stop_cushion {
        let min = stop_cushion_min_threshold(input.seconds_left);
        let good = stop_cushion_good_threshold(input.seconds_left);
        if cushion < min {
            risk_score += 20.0;
            push_risk_component(&mut risk_components, "stop_cushion_weak", 20.0, 2.0);
        } else if cushion < good {
            risk_score += 10.0;
            push_risk_component(&mut risk_components, "stop_cushion_moderate", 10.0, 1.0);
        }
    }
    let gap_margin = gap_strength - required_gap_strength;
    if !gap_strength_soft_low && gap_margin < risk_gap_margin(input.seconds_left) {
        risk_score += 8.0;
        push_risk_component(&mut risk_components, "borderline_gap_margin", 8.0, 1.0);
    }
    if input.ask * 100.0 > config.normal_max_price * 100.0 && fair_probability.is_none() {
        all_reasons.push(IvEntryQualityReason::PremiumEvMissing);
        hard_red_flag = true;
    }

    let deep_value = ask_cent <= config.deep_value_max_price * 100.0 && !hard_red_flag;
    if risk_score > config.risk_score_high_max && !deep_value {
        all_reasons.push(IvEntryQualityReason::RiskCapHardBlock);
        hard_red_flag = true;
    }

    let lane = if deep_value {
        "deep_value"
    } else if risk_score <= config.risk_score_clean_max {
        "clean"
    } else if risk_score <= config.risk_score_moderate_max {
        "moderate"
    } else {
        "high"
    };
    let (risk_level, lane_max_cent, size_multiplier): (&'static str, f64, f64) = match lane {
        "deep_value" => ("deep_value", config.deep_value_max_price * 100.0, 0.50),
        "clean" => ("clean", config.normal_max_price * 100.0, 1.0),
        "moderate" => ("moderate", config.moderate_risk_max_price * 100.0, 0.70),
        _ => ("high", config.high_risk_max_price * 100.0, 0.50),
    };
    let cap_haircut_cent = 0.0;
    let risk_cap_price_cent = min_cap(configured_cap, lane_max_cent, None);
    let effective_max_buy_price = min_cap(risk_cap_price_cent, f64::INFINITY, ev_cap);
    let ask_over_cap_cent = effective_max_buy_price
        .map(|cap| ask_cent - cap)
        .filter(|value| *value > 0.0);
    if ask_over_cap_cent.is_some() && !hard_red_flag {
        all_reasons.push(IvEntryQualityReason::PriceAboveEffectiveMax);
    }
    let hard_block = hard_red_flag;
    let deferred = !hard_block && ask_over_cap_cent.is_some() && config.wait_for_price_enabled;
    let entry_action = if hard_block {
        "hard_block"
    } else if deferred {
        "wait_for_price"
    } else if config.passive_bid_enabled && ask_over_cap_cent.is_some() {
        "place_passive_bid"
    } else {
        "submit_order"
    };
    let primary_reason = all_reasons.first().copied();

    IvEntryQualityDecision {
        allowed: primary_reason.is_none(),
        primary_reason,
        all_reasons,
        expected_move_raw: input.expected_move_raw,
        expected_move_eff: input.expected_move_eff,
        expected_move_floor_applied: input.expected_move_eff > input.expected_move_raw,
        required_gap_strength,
        required_gap_strength_with_margin,
        gap_strength,
        gap_strength_hard_floor: Some(gap_strength_hard_floor),
        gap_strength_deficit,
        gap_strength_soft_low_ratio,
        gap_soft_low_risk_points,
        gap_strength_soft_low: Some(gap_strength_soft_low),
        eq77_lite_profile: Some("lite_v1"),
        buffer_5s_ago: history_5s.map(|history| history.gap),
        buffer_10s_ago: history_10s.map(|history| history.gap),
        same_side_history_5s: history_5s.map(|history| history.same_side),
        same_side_history_10s: history_10s.map(|history| history.same_side),
        buffer_retain_5s,
        buffer_retain_10s,
        spike_ratio: spike.map(|spike| spike.ratio),
        spike_retrace_usd: spike.map(|spike| spike.retrace),
        premium_ev_pass: fair_probability
            .zip(effective_max_buy_price)
            .map(|(fair, cap)| fair >= cap),
        premium_ev_required_price: effective_max_buy_price,
        premium_ev_margin_cent: fair_probability
            .zip(effective_max_buy_price)
            .map(|(fair, cap)| fair - cap),
        premium_price_allowed: ask_over_cap_cent.is_none() && !hard_block,
        effective_max_buy_price,
        fair_probability,
        fee_buffer,
        min_edge,
        entry_quality_price_source: "iv_mismatch_current_price",
        history_price_source: "chainlink_live_data_ws",
        entry_action,
        hard_block,
        deferred,
        signal_recheck_required: deferred || config.recheck_before_submit,
        risk_cap_price_cent,
        ask_over_cap_cent,
        risk_score: Some(risk_score.min(100.0)),
        cap_haircut_cent: Some(cap_haircut_cent),
        risk_level: Some(risk_level),
        lane: Some(lane),
        size_multiplier: Some(size_multiplier),
        risk_components,
        cap_components: vec![
            json!({"name": "lane_max", "cap_cent": lane_max_cent}),
            json!({"name": "risk_haircut", "haircut_cent": cap_haircut_cent}),
            json!({"name": "risk_cap", "cap_cent": risk_cap_price_cent}),
            json!({"name": "ev_cap", "cap_cent": ev_cap}),
        ],
    }
}

fn append_premium_reasons(
    all_reasons: &mut Vec<IvEntryQualityReason>,
    input: IvEntryQualityInput<'_>,
    history_5s: Option<HistoryGap>,
    history_10s: Option<HistoryGap>,
    buffer_retain_5s: Option<f64>,
    buffer_retain_10s: Option<f64>,
    premium_ev_pass: Option<bool>,
    effective_max_buy_price: Option<f64>,
) {
    let config = input.config;
    let ask_cent = input.ask * 100.0;
    if ask_cent > config.premium_max_price * 100.0 {
        all_reasons.push(IvEntryQualityReason::PriceAbovePremiumMax);
    }
    if history_5s.map(|history| !history.same_side).unwrap_or(true)
        || history_10s
            .map(|history| !history.same_side)
            .unwrap_or(true)
    {
        all_reasons.push(IvEntryQualityReason::MissingSameSideHistoryForPremium);
    }
    if buffer_retain_5s
        .map(|retain| retain < config.premium_buffer_retain_5s)
        .unwrap_or(false)
    {
        all_reasons.push(IvEntryQualityReason::BufferCollapse5s);
    }
    if buffer_retain_10s
        .map(|retain| retain < config.premium_buffer_retain_10s)
        .unwrap_or(false)
    {
        all_reasons.push(IvEntryQualityReason::BufferCollapse10s);
    }
    if input.gap_velocity.unwrap_or(0.0) < 0.0 {
        all_reasons.push(IvEntryQualityReason::NegativeVelocityPremium);
    }
    if input.spread > config.premium_max_spread {
        all_reasons.push(IvEntryQualityReason::PremiumSpreadTooWide);
    }
    if input
        .chainlink_age_ms
        .map(|age| age > config.premium_max_chainlink_age_ms)
        .unwrap_or(true)
    {
        all_reasons.push(IvEntryQualityReason::PremiumChainlinkStale);
    }
    if !cex_aligned(input) {
        all_reasons.push(IvEntryQualityReason::PremiumCexMisaligned);
    }
    match premium_ev_pass {
        Some(true) => {}
        Some(false) => all_reasons.push(IvEntryQualityReason::PremiumEvInsufficient),
        None => all_reasons.push(IvEntryQualityReason::PremiumEvMissing),
    }
    if ask_cent > effective_max_buy_price.unwrap_or(f64::NEG_INFINITY) {
        all_reasons.push(IvEntryQualityReason::PriceAboveEffectiveMax);
    }
}

fn premium_static_checks_pass(
    input: IvEntryQualityInput<'_>,
    history_5s: Option<HistoryGap>,
    history_10s: Option<HistoryGap>,
) -> bool {
    history_5s.map(|history| history.same_side).unwrap_or(false)
        && history_10s
            .map(|history| history.same_side)
            .unwrap_or(false)
        && input.spread <= input.config.premium_max_spread
        && input
            .chainlink_age_ms
            .map(|age| age <= input.config.premium_max_chainlink_age_ms)
            .unwrap_or(false)
        && cex_aligned(input)
}

fn cex_aligned(input: IvEntryQualityInput<'_>) -> bool {
    if !input.cex_fresh || input.cex_same_direction != Some(true) {
        return false;
    }
    let Some(cex_price) = input.cex_price.filter(|price| price.is_finite()) else {
        return false;
    };
    let gap_usd = (input.current_price - cex_price).abs();
    let bps_cap = input
        .config
        .cex_align_max_bps
        .filter(|bps| bps.is_finite() && *bps >= 0.0)
        .map(|bps| input.current_price.abs() * bps / 10_000.0);
    let cap = match (input.config.cex_align_max_usd, bps_cap) {
        (Some(usd), Some(bps)) => usd.max(bps),
        (Some(usd), None) => usd,
        (None, Some(bps)) => bps,
        (None, None) => return false,
    };
    gap_usd <= cap.max(0.0)
}

fn min_cap(configured_cap: Option<f64>, quality_cap: f64, ev_cap: Option<f64>) -> Option<f64> {
    let mut cap = quality_cap;
    if let Some(configured_cap) = configured_cap {
        cap = cap.min(configured_cap);
    }
    if let Some(ev_cap) = ev_cap {
        cap = cap.min(ev_cap);
    }
    cap.is_finite().then_some(cap)
}

fn push_risk_component(
    components: &mut Vec<Value>,
    name: &'static str,
    risk_points: f64,
    haircut_cent: f64,
) {
    components.push(json!({
        "name": name,
        "risk_points": risk_points,
        "haircut_cent": haircut_cent,
    }));
}

#[allow(clippy::too_many_arguments)]
fn add_threshold_risk(
    risk_score: &mut f64,
    components: &mut Vec<Value>,
    name: &'static str,
    value: Option<f64>,
    mild_threshold: f64,
    strong_threshold: f64,
    mild_points: f64,
    strong_points: f64,
) {
    let Some(value) = value else {
        return;
    };
    if value < strong_threshold {
        *risk_score += strong_points;
        push_risk_component(components, name, strong_points, 2.0);
    } else if value < mild_threshold {
        *risk_score += mild_points;
        push_risk_component(components, name, mild_points, 1.0);
    }
}

fn eq77_gap_strength_hard_floor(seconds_left: f64) -> f64 {
    if seconds_left > 60.0 {
        0.85
    } else if seconds_left > 25.0 {
        0.75
    } else {
        0.65
    }
}

fn eq77_gap_soft_low_risk_points(ratio: f64) -> f64 {
    if ratio < 0.25 {
        6.0
    } else if ratio < 0.50 {
        12.0
    } else if ratio < 0.75 {
        20.0
    } else {
        30.0
    }
}

fn stop_cushion_strength(seconds_left: f64, gap_now: f64, expected_move_eff: f64) -> Option<f64> {
    let soft_stop_gap = if seconds_left > 60.0 {
        -7.0
    } else if seconds_left > 25.0 {
        -5.0
    } else {
        -4.0
    };
    let denominator = expected_move_eff.max(f64::EPSILON);
    ((gap_now - soft_stop_gap).is_finite() && denominator.is_finite())
        .then_some((gap_now - soft_stop_gap) / denominator)
}

fn stop_cushion_good_threshold(seconds_left: f64) -> f64 {
    if seconds_left > 60.0 {
        0.65
    } else if seconds_left > 25.0 {
        0.50
    } else {
        0.40
    }
}

fn stop_cushion_min_threshold(seconds_left: f64) -> f64 {
    if seconds_left > 60.0 {
        0.40
    } else if seconds_left > 25.0 {
        0.30
    } else {
        0.25
    }
}

fn stop_cushion_hard_threshold(seconds_left: f64) -> f64 {
    if seconds_left > 60.0 {
        0.25
    } else if seconds_left > 25.0 {
        0.20
    } else {
        0.15
    }
}

fn risk_gap_margin(seconds_left: f64) -> f64 {
    if seconds_left > 60.0 {
        0.08
    } else if seconds_left > 25.0 {
        0.06
    } else {
        0.04
    }
}

#[derive(Debug, Clone, Copy)]
struct HistoryGap {
    gap: f64,
    same_side: bool,
}

fn history_gap(input: IvEntryQualityInput<'_>, lookback_ms: i64) -> Option<HistoryGap> {
    let target_ms = input.latest_timestamp_ms - lookback_ms;
    let sample = input
        .samples
        .iter()
        .rev()
        .find(|sample| sample.timestamp_ms <= target_ms)?;
    let gap = side_gap(input.side, sample.price, input.price_to_beat);
    Some(HistoryGap {
        gap,
        same_side: gap > 0.0,
    })
}

fn retain_ratio(now_gap: f64, previous_gap: f64) -> Option<f64> {
    (now_gap > 0.0 && previous_gap > 0.0).then_some(now_gap / previous_gap)
}

#[derive(Debug, Clone, Copy)]
struct SpikeFade {
    ratio: f64,
    retrace: f64,
}

fn spike_fade(input: IvEntryQualityInput<'_>) -> Option<SpikeFade> {
    let price_15s_ago = price_at_or_before(input.samples, input.latest_timestamp_ms - 15_000)?;
    let directional_move = if input.side == "up" {
        input.current_price - price_15s_ago
    } else {
        price_15s_ago - input.current_price
    };
    if directional_move <= 0.0 {
        return None;
    }
    let typical_move = typical_15s_move(input.samples, input.latest_timestamp_ms - 120_000)?;
    if typical_move <= 0.0 {
        return None;
    }
    let ratio = directional_move / typical_move;
    let (extreme_price, extreme_ts) = directional_extreme(
        input.samples,
        input.side,
        input.latest_timestamp_ms - 15_000,
    )?;
    let retrace = if input.side == "up" {
        extreme_price - input.current_price
    } else {
        input.current_price - extreme_price
    };
    let no_new_extreme_last_3s = extreme_ts < input.latest_timestamp_ms - 3_000;
    (ratio >= input.config.spike_multiplier
        && retrace >= input.config.spike_retrace_ratio * directional_move
        && no_new_extreme_last_3s)
        .then_some(SpikeFade { ratio, retrace })
}

fn price_at_or_before(samples: &[ChainlinkPriceSample], target_ms: i64) -> Option<f64> {
    samples
        .iter()
        .rev()
        .find(|sample| sample.timestamp_ms <= target_ms)
        .map(|sample| sample.price)
}

fn directional_extreme(
    samples: &[ChainlinkPriceSample],
    side: &str,
    start_ms: i64,
) -> Option<(f64, i64)> {
    samples
        .iter()
        .filter(|sample| sample.timestamp_ms >= start_ms)
        .max_by(|left, right| {
            if side == "up" {
                left.price.total_cmp(&right.price)
            } else {
                right.price.total_cmp(&left.price)
            }
        })
        .map(|sample| (sample.price, sample.timestamp_ms))
}

fn typical_15s_move(samples: &[ChainlinkPriceSample], start_ms: i64) -> Option<f64> {
    let mut moves = Vec::new();
    for sample in samples
        .iter()
        .filter(|sample| sample.timestamp_ms >= start_ms)
    {
        if let Some(previous) = price_at_or_before(samples, sample.timestamp_ms - 15_000) {
            let movement = (sample.price - previous).abs();
            if movement.is_finite() && movement > 0.0 {
                moves.push(movement);
            }
        }
    }
    if moves.is_empty() {
        return None;
    }
    moves.sort_by(f64::total_cmp);
    Some(moves[moves.len() / 2])
}

fn side_gap(side: &str, price: f64, price_to_beat: f64) -> f64 {
    if side == "up" {
        price - price_to_beat
    } else {
        price_to_beat - price
    }
}

#[cfg(test)]
mod tests;
