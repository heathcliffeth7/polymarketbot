use super::iv_cex_open_gap::CexOpenGapConsensus;
use crate::trade_flow::guards::chainlink_price::ChainlinkPriceSample;

const DEFAULT_LOOKBACK_SECONDS: i64 = 10;
const DEFAULT_EXTENDED_LOOKBACK_SECONDS: i64 = 15;
const DEFAULT_DEADBAND_BPS: f64 = 0.5;
const DEFAULT_DEADBAND_MIN_USD_BTC: f64 = 5.0;
const DEFAULT_DEADBAND_MIN_USD_ETH: f64 = 0.30;
const DEFAULT_DEADBAND_MIN_USD_SOL: f64 = 0.03;
const DEFAULT_ZERO_CROSS_BLOCK_10S: usize = 2;
const DEFAULT_ZERO_CROSS_BLOCK_15S: usize = 3;
const DEFAULT_PATH_Z_WARN: f64 = 1.25;
const DEFAULT_PATH_Z_BLOCK: f64 = 1.75;
const DEFAULT_EFFICIENCY_WARN: f64 = 0.25;
const DEFAULT_EFFICIENCY_BLOCK: f64 = 0.15;
const DEFAULT_OPPOSITE_DEPTH_Z_WARN: f64 = 0.50;
const DEFAULT_OPPOSITE_DEPTH_Z_BLOCK: f64 = 0.90;
const DEFAULT_MAX_GAP_STRENGTH_PENALTY: f64 = 0.35;
const DEFAULT_MAX_1S_JUMP_Z_WARN: f64 = 0.50;
const DEFAULT_ONE_WAY_SPIKE_JUMP_Z: f64 = 0.40;
const DEFAULT_ONE_WAY_SPIKE_MAX_SAME_SIDE_AGE_SECONDS: f64 = 3.0;
const DEFAULT_ONE_WAY_SPIKE_BOOK_WAIT_DISLOCATION: f64 = 0.20;
const DEFAULT_ONE_WAY_SPIKE_BOOK_BLOCK_DISLOCATION: f64 = 0.25;
const DEFAULT_ONE_WAY_SPIKE_BLOCK_GAP_STRENGTH_BUFFER: f64 = 0.50;
const CLEAN_TREND_MIN_EFFICIENCY: f64 = 0.65;
const REQUIRED_GAP_STRENGTH_BLOCK_BUFFER: f64 = 0.75;
const DEAD_BAND_EXPECTED_MOVE_MULTIPLIER: f64 = 0.05;
const GAP_PATH_EPSILON: f64 = 1e-9;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvPtbChopConfig {
    pub(crate) enabled: bool,
    pub(crate) lookback_seconds: i64,
    pub(crate) extended_lookback_seconds: i64,
    pub(crate) deadband_bps: f64,
    pub(crate) deadband_min_usd_btc: f64,
    pub(crate) deadband_min_usd_eth: f64,
    pub(crate) deadband_min_usd_sol: f64,
    pub(crate) zero_cross_block_10s: usize,
    pub(crate) zero_cross_block_15s: usize,
    pub(crate) path_z_warn: f64,
    pub(crate) path_z_block: f64,
    pub(crate) efficiency_warn: f64,
    pub(crate) efficiency_block: f64,
    pub(crate) opposite_depth_z_warn: f64,
    pub(crate) opposite_depth_z_block: f64,
    pub(crate) max_gap_strength_penalty: f64,
}

impl Default for PriceToBeatIvPtbChopConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            lookback_seconds: DEFAULT_LOOKBACK_SECONDS,
            extended_lookback_seconds: DEFAULT_EXTENDED_LOOKBACK_SECONDS,
            deadband_bps: DEFAULT_DEADBAND_BPS,
            deadband_min_usd_btc: DEFAULT_DEADBAND_MIN_USD_BTC,
            deadband_min_usd_eth: DEFAULT_DEADBAND_MIN_USD_ETH,
            deadband_min_usd_sol: DEFAULT_DEADBAND_MIN_USD_SOL,
            zero_cross_block_10s: DEFAULT_ZERO_CROSS_BLOCK_10S,
            zero_cross_block_15s: DEFAULT_ZERO_CROSS_BLOCK_15S,
            path_z_warn: DEFAULT_PATH_Z_WARN,
            path_z_block: DEFAULT_PATH_Z_BLOCK,
            efficiency_warn: DEFAULT_EFFICIENCY_WARN,
            efficiency_block: DEFAULT_EFFICIENCY_BLOCK,
            opposite_depth_z_warn: DEFAULT_OPPOSITE_DEPTH_Z_WARN,
            opposite_depth_z_block: DEFAULT_OPPOSITE_DEPTH_Z_BLOCK,
            max_gap_strength_penalty: DEFAULT_MAX_GAP_STRENGTH_PENALTY,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvPtbChopEvaluation {
    pub(crate) enabled: bool,
    pub(crate) risk: &'static str,
    pub(crate) action: &'static str,
    pub(crate) block_reason: Option<&'static str>,
    pub(crate) movement_mode: &'static str,
    pub(crate) movement_action: &'static str,
    pub(crate) movement_reason: Option<&'static str>,
    pub(crate) gap_strength_penalty: f64,
    pub(crate) deadband_usd: Option<f64>,
    pub(crate) lookback_seconds: i64,
    pub(crate) extended_lookback_seconds: i64,
    pub(crate) zero_cross_count_10s: Option<usize>,
    pub(crate) zero_cross_count_15s: Option<usize>,
    pub(crate) gap_path_10s: Option<f64>,
    pub(crate) gap_path_z_10s: Option<f64>,
    pub(crate) net_gap_change_10s: Option<f64>,
    pub(crate) efficiency_ratio_10s: Option<f64>,
    pub(crate) max_1s_jump_10s: Option<f64>,
    pub(crate) max_1s_jump_z_10s: Option<f64>,
    pub(crate) opposite_depth_usd_10s: Option<f64>,
    pub(crate) opposite_depth_z_10s: Option<f64>,
    pub(crate) same_side_age_seconds: Option<f64>,
    pub(crate) cex_consensus: Option<&'static str>,
    pub(crate) model_book_dislocation: Option<f64>,
}

impl PriceToBeatIvPtbChopEvaluation {
    pub(crate) fn off(config: &PriceToBeatIvPtbChopConfig) -> Self {
        Self {
            enabled: false,
            risk: "off",
            action: "off",
            block_reason: None,
            movement_mode: "off",
            movement_action: "off",
            movement_reason: None,
            gap_strength_penalty: 0.0,
            deadband_usd: None,
            lookback_seconds: config.lookback_seconds,
            extended_lookback_seconds: config.extended_lookback_seconds,
            zero_cross_count_10s: None,
            zero_cross_count_15s: None,
            gap_path_10s: None,
            gap_path_z_10s: None,
            net_gap_change_10s: None,
            efficiency_ratio_10s: None,
            max_1s_jump_10s: None,
            max_1s_jump_z_10s: None,
            opposite_depth_usd_10s: None,
            opposite_depth_z_10s: None,
            same_side_age_seconds: None,
            cex_consensus: None,
            model_book_dislocation: None,
        }
    }
}

pub(crate) struct PriceToBeatIvPtbChopInput<'a> {
    pub(crate) config: &'a PriceToBeatIvPtbChopConfig,
    pub(crate) asset: &'a str,
    pub(crate) selected_side: &'static str,
    pub(crate) samples: &'a [ChainlinkPriceSample],
    pub(crate) price_to_beat: f64,
    pub(crate) current_price: f64,
    pub(crate) latest_timestamp_ms: i64,
    pub(crate) expected_move_eff: f64,
    pub(crate) gap_strength: f64,
    pub(crate) required_gap_strength: f64,
    pub(crate) cex_consensus: Option<CexOpenGapConsensus>,
    pub(crate) model_book_dislocation: Option<f64>,
}

#[derive(Debug, Clone)]
struct ChopWindowMetrics {
    zero_cross_count: usize,
    gap_path: f64,
    gap_path_z: f64,
    net_gap_change: f64,
    efficiency_ratio: f64,
    max_1s_jump: f64,
    max_1s_jump_z: f64,
    opposite_depth_usd: f64,
    opposite_depth_z: f64,
    same_side_age_seconds: Option<f64>,
}

pub(crate) fn evaluate_price_to_beat_iv_ptb_chop(
    input: PriceToBeatIvPtbChopInput<'_>,
) -> PriceToBeatIvPtbChopEvaluation {
    let config = input.config;
    if !config.enabled {
        return PriceToBeatIvPtbChopEvaluation::off(config);
    }
    let mut evaluation = PriceToBeatIvPtbChopEvaluation {
        enabled: true,
        risk: "unknown",
        action: "none",
        block_reason: None,
        movement_mode: "unknown",
        movement_action: "none",
        movement_reason: None,
        gap_strength_penalty: 0.0,
        deadband_usd: None,
        lookback_seconds: config.lookback_seconds,
        extended_lookback_seconds: config.extended_lookback_seconds,
        ..PriceToBeatIvPtbChopEvaluation::off(config)
    };
    let deadband_usd = resolve_deadband_usd(
        config,
        input.asset,
        input.current_price,
        input.expected_move_eff,
    );
    evaluation.deadband_usd = Some(deadband_usd);
    evaluation.cex_consensus = input.cex_consensus.map(CexOpenGapConsensus::as_str);
    evaluation.model_book_dislocation = input.model_book_dislocation;
    if !input.expected_move_eff.is_finite() || input.expected_move_eff <= 0.0 {
        evaluation.movement_mode = "insufficient_samples";
        return evaluation;
    }

    let short = chop_window_metrics(
        input.samples,
        input.selected_side,
        input.price_to_beat,
        input.latest_timestamp_ms,
        config.lookback_seconds,
        deadband_usd,
        input.expected_move_eff,
    );
    let extended = chop_window_metrics(
        input.samples,
        input.selected_side,
        input.price_to_beat,
        input.latest_timestamp_ms,
        config.extended_lookback_seconds,
        deadband_usd,
        input.expected_move_eff,
    );
    if let Some(metrics) = short.as_ref() {
        evaluation.zero_cross_count_10s = Some(metrics.zero_cross_count);
        evaluation.gap_path_10s = Some(metrics.gap_path);
        evaluation.gap_path_z_10s = Some(metrics.gap_path_z);
        evaluation.net_gap_change_10s = Some(metrics.net_gap_change);
        evaluation.efficiency_ratio_10s = Some(metrics.efficiency_ratio);
        evaluation.max_1s_jump_10s = Some(metrics.max_1s_jump);
        evaluation.max_1s_jump_z_10s = Some(metrics.max_1s_jump_z);
        evaluation.opposite_depth_usd_10s = Some(metrics.opposite_depth_usd);
        evaluation.opposite_depth_z_10s = Some(metrics.opposite_depth_z);
        evaluation.same_side_age_seconds = metrics.same_side_age_seconds;
    }
    evaluation.zero_cross_count_15s = extended.as_ref().map(|metrics| metrics.zero_cross_count);

    let short = match short {
        Some(metrics) => metrics,
        None => {
            evaluation.movement_mode = "insufficient_samples";
            return evaluation;
        }
    };
    let extended_crosses = extended
        .as_ref()
        .map(|metrics| metrics.zero_cross_count)
        .unwrap_or(short.zero_cross_count);
    let toxic_chop = short.zero_cross_count >= config.zero_cross_block_10s
        || extended_crosses >= config.zero_cross_block_15s
        || (short.gap_path_z >= config.path_z_block
            && short.efficiency_ratio <= config.efficiency_block)
        || (short.opposite_depth_z >= config.opposite_depth_z_block
            && input.gap_strength
                < input.required_gap_strength + REQUIRED_GAP_STRENGTH_BLOCK_BUFFER);
    if toxic_chop {
        evaluation.risk = "toxic";
        evaluation.action = "block";
        evaluation.block_reason = Some("blocked_ptb_chop_volatility");
        evaluation.movement_mode = "toxic_chop";
        evaluation.movement_action = "block";
        evaluation.movement_reason = Some("blocked_ptb_chop_volatility");
        return evaluation;
    }

    let one_way_spike = short.zero_cross_count == 0
        && short.max_1s_jump_z >= DEFAULT_ONE_WAY_SPIKE_JUMP_Z
        && short.same_side_age_seconds.unwrap_or(0.0)
            < DEFAULT_ONE_WAY_SPIKE_MAX_SAME_SIDE_AGE_SECONDS;
    let cex_not_strong = input
        .cex_consensus
        .map(|consensus| consensus != CexOpenGapConsensus::Strong)
        .unwrap_or(true);
    let book_dislocation = input
        .model_book_dislocation
        .filter(|value| value.is_finite())
        .unwrap_or(0.0);
    if one_way_spike
        && cex_not_strong
        && book_dislocation >= DEFAULT_ONE_WAY_SPIKE_BOOK_BLOCK_DISLOCATION
        && input.gap_strength
            < input.required_gap_strength + DEFAULT_ONE_WAY_SPIKE_BLOCK_GAP_STRENGTH_BUFFER
    {
        evaluation.risk = "toxic";
        evaluation.action = "block";
        evaluation.block_reason = Some("blocked_ptb_spike_unconfirmed");
        evaluation.movement_mode = "unconfirmed_spike";
        evaluation.movement_action = "block";
        evaluation.movement_reason = Some("blocked_ptb_spike_unconfirmed");
        return evaluation;
    }
    if one_way_spike
        && cex_not_strong
        && book_dislocation >= DEFAULT_ONE_WAY_SPIKE_BOOK_WAIT_DISLOCATION
    {
        evaluation.risk = "medium";
        evaluation.action = "wait";
        evaluation.block_reason = Some("wait_ptb_spike_persistence");
        evaluation.movement_mode = "one_way_spike";
        evaluation.movement_action = "wait";
        evaluation.movement_reason = Some("wait_ptb_spike_persistence");
        return evaluation;
    }

    let clean_trend = short.zero_cross_count == 0
        && short.efficiency_ratio >= CLEAN_TREND_MIN_EFFICIENCY
        && short.same_side_age_seconds.unwrap_or(0.0)
            >= DEFAULT_ONE_WAY_SPIKE_MAX_SAME_SIDE_AGE_SECONDS;
    let mut penalty: f64 = 0.0;
    if short.zero_cross_count == 1 {
        penalty += 0.15;
    }
    if !clean_trend && short.gap_path_z >= config.path_z_warn {
        penalty += 0.15;
    }
    if short.efficiency_ratio <= config.efficiency_warn {
        penalty += 0.10;
    }
    if !clean_trend && short.max_1s_jump_z >= DEFAULT_MAX_1S_JUMP_Z_WARN {
        penalty += 0.10;
    }
    if short.opposite_depth_z >= config.opposite_depth_z_warn {
        penalty += 0.10;
    }
    evaluation.gap_strength_penalty = penalty.min(config.max_gap_strength_penalty.max(0.0));
    if evaluation.gap_strength_penalty > 0.0 {
        evaluation.risk = "medium";
        evaluation.action = "penalty";
        evaluation.movement_mode = if one_way_spike {
            "one_way_spike"
        } else {
            "medium_chop"
        };
        evaluation.movement_action = "penalty";
        evaluation.movement_reason = Some("ptb_movement_penalty");
    } else {
        evaluation.risk = "clean";
        evaluation.movement_mode = "clean_trend";
        evaluation.movement_action = "none";
    }
    evaluation
}

fn resolve_deadband_usd(
    config: &PriceToBeatIvPtbChopConfig,
    asset: &str,
    current_price: f64,
    expected_move_eff: f64,
) -> f64 {
    let asset_floor = match asset.trim().to_ascii_lowercase().as_str() {
        "btc" => config.deadband_min_usd_btc,
        "eth" => config.deadband_min_usd_eth,
        "sol" => config.deadband_min_usd_sol,
        _ => config.deadband_min_usd_sol,
    };
    [
        asset_floor,
        current_price * config.deadband_bps.max(0.0) / 10_000.0,
        expected_move_eff.max(0.0) * DEAD_BAND_EXPECTED_MOVE_MULTIPLIER,
    ]
    .into_iter()
    .filter(|value| value.is_finite() && *value >= 0.0)
    .max_by(f64::total_cmp)
    .unwrap_or(0.0)
}

fn chop_window_metrics(
    samples: &[ChainlinkPriceSample],
    selected_side: &'static str,
    price_to_beat: f64,
    latest_timestamp_ms: i64,
    lookback_seconds: i64,
    deadband_usd: f64,
    expected_move_eff: f64,
) -> Option<ChopWindowMetrics> {
    let window_start_ms = latest_timestamp_ms - lookback_seconds.max(1) * 1_000;
    let window = samples
        .iter()
        .filter(|sample| sample.timestamp_ms >= window_start_ms)
        .collect::<Vec<_>>();
    if window.len() < 2 {
        return None;
    }
    let side_sign = if selected_side == "down" { -1.0 } else { 1.0 };
    let gaps = window
        .iter()
        .map(|sample| sample.price - price_to_beat)
        .collect::<Vec<_>>();
    let first_gap = *gaps.first()?;
    let last_gap = *gaps.last()?;
    let mut gap_path = 0.0;
    let mut max_1s_jump = 0.0;
    let mut zero_cross_count = 0;
    let mut previous_band_sign = band_sign(first_gap, deadband_usd);
    let mut opposite_depth_usd = 0.0;
    for (index, gap) in gaps.iter().enumerate() {
        let selected_gap = gap * side_sign;
        if selected_gap < 0.0 {
            opposite_depth_usd = f64::max(opposite_depth_usd, -selected_gap);
        }
        let sign = band_sign(*gap, deadband_usd);
        if sign != 0 {
            if previous_band_sign != 0 && sign != previous_band_sign {
                zero_cross_count += 1;
            }
            previous_band_sign = sign;
        }
        if index == 0 {
            continue;
        }
        let delta = (gap - gaps[index - 1]).abs();
        gap_path += delta;
        max_1s_jump = f64::max(max_1s_jump, delta);
    }
    let net_gap_change = (last_gap - first_gap).abs();
    let efficiency_ratio = if gap_path > GAP_PATH_EPSILON {
        (net_gap_change / gap_path).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let same_side_age_seconds = same_side_age_seconds(
        &window,
        side_sign,
        price_to_beat,
        deadband_usd,
        latest_timestamp_ms,
    );
    Some(ChopWindowMetrics {
        zero_cross_count,
        gap_path,
        gap_path_z: gap_path / expected_move_eff,
        net_gap_change,
        efficiency_ratio,
        max_1s_jump,
        max_1s_jump_z: max_1s_jump / expected_move_eff,
        opposite_depth_usd,
        opposite_depth_z: opposite_depth_usd / expected_move_eff,
        same_side_age_seconds,
    })
}

fn same_side_age_seconds(
    samples: &[&ChainlinkPriceSample],
    side_sign: f64,
    price_to_beat: f64,
    deadband_usd: f64,
    latest_timestamp_ms: i64,
) -> Option<f64> {
    let latest = samples.last()?;
    let latest_selected_gap = (latest.price - price_to_beat) * side_sign;
    if latest_selected_gap <= deadband_usd {
        return Some(0.0);
    }
    let last_not_same_side = samples
        .iter()
        .rev()
        .skip(1)
        .find(|sample| (sample.price - price_to_beat) * side_sign <= deadband_usd);
    let start_ms = last_not_same_side
        .map(|sample| sample.timestamp_ms)
        .unwrap_or_else(|| {
            samples
                .first()
                .map(|sample| sample.timestamp_ms)
                .unwrap_or(latest.timestamp_ms)
        });
    Some(((latest_timestamp_ms - start_ms) as f64 / 1_000.0).max(0.0))
}

fn band_sign(gap: f64, deadband_usd: f64) -> i8 {
    if gap > deadband_usd {
        1
    } else if gap < -deadband_usd {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(prices: &[f64]) -> Vec<ChainlinkPriceSample> {
        let start_ms = 1_000_000;
        prices
            .iter()
            .enumerate()
            .map(|(index, price)| ChainlinkPriceSample {
                price: *price,
                timestamp_ms: start_ms + index as i64 * 1_000,
            })
            .collect()
    }

    fn eval(prices: &[f64], price_to_beat: f64) -> PriceToBeatIvPtbChopEvaluation {
        let samples = samples(prices);
        evaluate_price_to_beat_iv_ptb_chop(PriceToBeatIvPtbChopInput {
            config: &PriceToBeatIvPtbChopConfig {
                enabled: true,
                deadband_min_usd_btc: 0.5,
                ..PriceToBeatIvPtbChopConfig::default()
            },
            asset: "btc",
            selected_side: "up",
            samples: &samples,
            price_to_beat,
            current_price: *prices.last().unwrap_or(&price_to_beat),
            latest_timestamp_ms: samples
                .last()
                .map(|sample| sample.timestamp_ms)
                .unwrap_or(0),
            expected_move_eff: 10.0,
            gap_strength: 1.0,
            required_gap_strength: 1.0,
            cex_consensus: None,
            model_book_dislocation: None,
        })
    }

    fn eval_with_context(
        prices: &[f64],
        price_to_beat: f64,
        cex_consensus: Option<CexOpenGapConsensus>,
        model_book_dislocation: Option<f64>,
        gap_strength: f64,
    ) -> PriceToBeatIvPtbChopEvaluation {
        let samples = samples(prices);
        evaluate_price_to_beat_iv_ptb_chop(PriceToBeatIvPtbChopInput {
            config: &PriceToBeatIvPtbChopConfig {
                enabled: true,
                deadband_min_usd_btc: 0.5,
                ..PriceToBeatIvPtbChopConfig::default()
            },
            asset: "btc",
            selected_side: "up",
            samples: &samples,
            price_to_beat,
            current_price: *prices.last().unwrap_or(&price_to_beat),
            latest_timestamp_ms: samples
                .last()
                .map(|sample| sample.timestamp_ms)
                .unwrap_or(0),
            expected_move_eff: 10.0,
            gap_strength,
            required_gap_strength: 1.0,
            cex_consensus,
            model_book_dislocation,
        })
    }

    #[test]
    fn clean_trend_does_not_block() {
        let evaluation = eval(&[102.0, 105.0, 108.0, 112.0, 115.0], 100.0);

        assert_eq!(evaluation.risk, "clean");
        assert_eq!(evaluation.movement_mode, "clean_trend");
        assert_eq!(evaluation.block_reason, None);
        assert_eq!(evaluation.zero_cross_count_10s, Some(0));
    }

    #[test]
    fn toxic_chop_blocks_on_deadband_crosses() {
        let evaluation = eval(&[105.0, 96.0, 106.0, 97.0, 104.0], 100.0);

        assert_eq!(evaluation.risk, "toxic");
        assert_eq!(evaluation.action, "block");
        assert_eq!(evaluation.movement_mode, "toxic_chop");
        assert_eq!(evaluation.block_reason, Some("blocked_ptb_chop_volatility"));
    }

    #[test]
    fn high_path_clean_direction_does_not_block() {
        let evaluation = eval(&[105.0, 115.0, 125.0, 135.0], 100.0);

        assert_ne!(evaluation.block_reason, Some("blocked_ptb_chop_volatility"));
        assert_eq!(evaluation.zero_cross_count_10s, Some(0));
        assert!(evaluation.efficiency_ratio_10s.unwrap_or_default() > 0.9);
    }

    #[test]
    fn deadband_ignores_tiny_crosses() {
        let evaluation = eval(&[100.1, 99.9, 100.2, 99.8, 100.3], 100.0);

        assert_eq!(evaluation.zero_cross_count_10s, Some(0));
        assert_eq!(evaluation.block_reason, None);
    }

    #[test]
    fn medium_chop_applies_penalty_without_blocking() {
        let evaluation = eval(&[101.0, 103.5, 101.5, 103.0, 102.0], 100.0);

        assert_eq!(evaluation.risk, "medium");
        assert_eq!(evaluation.action, "penalty");
        assert!(evaluation.gap_strength_penalty > 0.0);
        assert_eq!(evaluation.block_reason, None);
    }

    #[test]
    fn one_way_spike_applies_penalty_without_confirmation_failure() {
        let evaluation = eval(&[100.1, 100.2, 108.5, 109.0], 100.0);

        assert_eq!(evaluation.movement_mode, "one_way_spike");
        assert_eq!(evaluation.movement_action, "penalty");
        assert_eq!(evaluation.block_reason, None);
        assert!(evaluation.max_1s_jump_z_10s.unwrap_or_default() >= 0.75);
        assert!(evaluation.gap_strength_penalty > 0.0);
    }

    #[test]
    fn one_way_spike_waits_when_cex_and_book_do_not_confirm() {
        let evaluation = eval_with_context(
            &[100.1, 100.2, 108.5, 109.0],
            100.0,
            Some(CexOpenGapConsensus::Mixed),
            Some(0.24),
            2.0,
        );

        assert_eq!(evaluation.movement_mode, "one_way_spike");
        assert_eq!(evaluation.movement_action, "wait");
        assert_eq!(evaluation.block_reason, Some("wait_ptb_spike_persistence"));
    }

    #[test]
    fn one_way_spike_blocks_when_unconfirmed_and_gap_strength_is_weak() {
        let evaluation = eval_with_context(
            &[100.1, 100.2, 108.5, 109.0],
            100.0,
            Some(CexOpenGapConsensus::Mixed),
            Some(0.30),
            1.0,
        );

        assert_eq!(evaluation.movement_mode, "unconfirmed_spike");
        assert_eq!(evaluation.movement_action, "block");
        assert_eq!(
            evaluation.block_reason,
            Some("blocked_ptb_spike_unconfirmed")
        );
    }
}
