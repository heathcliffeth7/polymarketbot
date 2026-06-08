use super::iv_borderline_pump_book_lead::{
    PriceToBeatIvBorderlinePumpBookLeadConfig, PriceToBeatIvBorderlinePumpBookLeadEvaluation,
    PriceToBeatIvBorderlinePumpBookLeadInput, evaluate_price_to_beat_iv_borderline_pump_book_lead,
};
use super::iv_cex_open_gap::{
    PriceToBeatIvCexOpenGapConfig, PriceToBeatIvCexOpenGapEvaluation, PriceToBeatIvCexOpenGapInput,
    cex_open_gap_book_mismatch_reason, evaluate_price_to_beat_iv_cex_open_gap,
};
use super::iv_entry_quality::{
    IvEntryQualityConfig, IvEntryQualityDecision, IvEntryQualityInput, evaluate_iv_entry_quality,
};
use super::iv_execution_vwap::{
    PriceToBeatIvExecutionVwapConfig, PriceToBeatIvExecutionVwapEvaluation,
    PriceToBeatIvExecutionVwapInput, evaluate_price_to_beat_iv_execution_vwap,
};
use super::iv_gap_fail_cex_book_guard::{
    PriceToBeatIvGapFailCexBookGuardConfig, PriceToBeatIvGapFailCexBookGuardEvaluation,
    PriceToBeatIvGapFailCexBookGuardInput, evaluate_price_to_beat_iv_gap_fail_cex_book_guard,
};
use super::iv_mismatch_adaptive::{
    PriceToBeatIvAdaptiveConfig, PriceToBeatIvAdaptiveEvaluation, PriceToBeatIvAdaptiveInput,
    PriceToBeatIvAdaptiveVolumeInput, evaluate_price_to_beat_iv_adaptive_volume,
};
use super::iv_mismatch_binance::{evaluate_binance_disagreement_penalty, evaluate_binance_veto};
use super::iv_mismatch_depth::{PriceToBeatIvDepthEvaluation, evaluate_price_to_beat_iv_depth};
use super::iv_mismatch_edge_helpers::{
    previous_side_gap, side_gap, side_gap_at_or_before, sigma_since, time_normalized_price_deltas,
    valid_probability, zero_cross_count,
};
use super::iv_mismatch_expected_move::{
    PriceToBeatIvExpectedMoveFloorConfig, PriceToBeatIvExpectedMoveFloorEvaluation,
    PriceToBeatIvExpectedMoveFloorInput, PriceToBeatIvMinExpectedMoveMode,
    evaluate_expected_move_floor,
};
#[path = "iv_mismatch_edge_telemetry.rs"]
mod iv_mismatch_edge_telemetry;
use super::PriceToBeatSignalFormulaMarketInput;
#[cfg(test)]
use super::iv_mismatch_math::inverse_normal_cdf;
use super::iv_mismatch_math::{implied_volatility_ratio, normal_cdf, standard_deviation};
use super::iv_mismatch_protection::{
    PriceToBeatIvBookQuotes, PriceToBeatIvProtectionInput, PriceToBeatIvProtectionMode,
    evaluate_price_to_beat_iv_protection,
};
use super::iv_mismatch_ptb_chop::{
    PriceToBeatIvPtbChopConfig, PriceToBeatIvPtbChopEvaluation, PriceToBeatIvPtbChopInput,
    evaluate_price_to_beat_iv_ptb_chop,
};
use super::iv_mismatch_time_rule::PriceToBeatIvMismatchTimeRule;
use super::iv_oracle_lag_book_lead::{
    PriceToBeatIvOracleLagBookLeadConfig, PriceToBeatIvOracleLagBookLeadEvaluation,
    PriceToBeatIvOracleLagBookLeadInput, evaluate_price_to_beat_iv_oracle_lag_book_lead,
};
use super::iv_pump_shock::{
    PriceToBeatIvPumpShockConfig, PriceToBeatIvPumpShockEvaluation, PriceToBeatIvPumpShockInput,
    evaluate_price_to_beat_iv_pump_shock,
};
use super::signal_formula::signal_formula_taker_fee;
use crate::trade_flow::guards::chainlink_price::get_chainlink_price_samples;
use bot_infra::exchange::OrderBookSnapshot;
use chrono::{Duration as ChronoDuration, Utc};
use serde_json::Value;

const DEFAULT_VOL_WINDOW_SECS: i64 = 45;
const DEFAULT_MIN_VOL_SAMPLES: usize = 8;
const DEFAULT_CHAINLINK_STALE_MS: i64 = 3_000;
const DEFAULT_BUFFER: f64 = 0.005;
const DEFAULT_MAX_SPREAD: f64 = 0.04;
const DEFAULT_CHOP_ZERO_CROSS_LIMIT: usize = 3;
const DEFAULT_CHOP_VALUE_EDGE: f64 = 0.10;
const DEFAULT_IV_RATIO_BLOCK_FAVORITE_BELOW: f64 = 0.80;
const DEFAULT_NO_NEW_TRADE_OVER_SECS: f64 = 90.0;
const DEFAULT_EDGE_THRESHOLD_30_90_SECS: f64 = 0.06;
const DEFAULT_EDGE_THRESHOLD_15_30_SECS: f64 = 0.08;
const DEFAULT_EDGE_THRESHOLD_8_15_SECS: f64 = 0.10;
const DEFAULT_NO_NEW_TRADE_UNDER_SECS: f64 = 8.0;
const DEFAULT_FAST_VOL_WINDOW_SECS: i64 = 15;
const DEFAULT_FAST_VOL_MULTIPLIER: f64 = 1.25;
const DEFAULT_LATENCY_BUFFER_SECS: f64 = 0.75;
const DEFAULT_HIGH_PRICE_PENALTY_THRESHOLD: f64 = 0.75;
const DEFAULT_HIGH_PRICE_PENALTY: f64 = 0.02;
const DEFAULT_STALE_PENALTY_MS: i64 = 1_500;
const DEFAULT_STALE_PENALTY: f64 = 0.02;
const DEFAULT_DROP_PENALTY_START_Z: f64 = 0.5;
const DEFAULT_DROP_PENALTY_PER_Z: f64 = 0.04;
const DEFAULT_FALLING_KNIFE_DROP_Z: f64 = 1.0;
const DEFAULT_RECOVERY_EDGE_THRESHOLD: f64 = 0.12;
const DEFAULT_BINANCE_STALE_MS: i64 = 2_500;
const DEFAULT_BINANCE_Q_BUFFER: f64 = 0.03;
const DEFAULT_STALE_GAP_STRENGTH_PENALTY_MS: i64 = 1_500;
const DEFAULT_BINANCE_MISSING_ASK_THRESHOLD: f64 = 0.65;
const DEFAULT_BINANCE_MISSING_PENALTY: f64 = 0.0;
const DEFAULT_MIN_ADJUSTED_MARGIN: f64 = 0.0;
const DEFAULT_BINANCE_DISAGREEMENT_PENALTY: f64 = 0.0;
const DEFAULT_PROTECTION_BOOK_LEAD_UNDER_SEC: f64 = 120.0;
const DEFAULT_PROTECTION_BOOK_LEAD_MIN_MID_DIFF: f64 = 0.20;
const DEFAULT_PROTECTION_OPPOSITE_MID_BLOCK: f64 = 0.65;
const DEFAULT_PROTECTION_MODEL_BOOK_GAP_WARN: f64 = 0.30;
const DEFAULT_PROTECTION_MODEL_BOOK_GAP_HARD: f64 = 0.45;
const DEFAULT_PROTECTION_MODEL_BOOK_WARN_THRESHOLD_PENALTY: f64 = 0.02;
const DEFAULT_PROTECTION_MODEL_BOOK_WARN_GAP_PENALTY: f64 = 0.05;
const DEFAULT_DEPTH_MAX_SLIPPAGE: f64 = 0.03;
const DEFAULT_LATE_HIGH_PRICE_SOFT_UNDER_SEC: f64 = 60.0;
const DEFAULT_LATE_HIGH_PRICE_ASK: f64 = 0.65;
const DEFAULT_LATE_HIGH_PRICE_SELECTED_MID_SOFT: f64 = 0.75;
const DEFAULT_LATE_HIGH_PRICE_THRESHOLD_PENALTY: f64 = 0.03;
const DEFAULT_LATE_HIGH_PRICE_SELECTED_MID_HARD: f64 = 0.65;
const DEFAULT_LATE_HIGH_PRICE_MIN_GAP_USD: f64 = 20.0;
const DEFAULT_PARTICIPATION_AFTER_MINUTES: f64 = 60.0;
const DEFAULT_PARTICIPATION_LONG_AFTER_MINUTES: f64 = 180.0;
const DEFAULT_PARTICIPATION_CREDIT: f64 = 0.01;
const DEFAULT_PARTICIPATION_LONG_CREDIT: f64 = 0.02;
const DEFAULT_PARTICIPATION_MIN_THRESHOLD: f64 = 0.05;
const DEFAULT_PROTECTION_SOFT_THRESHOLD_PENALTY: f64 = 0.03;
const DEFAULT_PROTECTION_SOFT_GAP_STRENGTH_PENALTY: f64 = 0.10;
const DEFAULT_PROTECTION_DROP_Z_BLOCK_THRESHOLD: f64 = 0.80;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvMismatchEdgeConfig {
    pub(crate) market: PriceToBeatSignalFormulaMarketInput,
    pub(crate) vol_window_secs: i64,
    pub(crate) min_vol_samples: usize,
    pub(crate) chainlink_stale_ms: i64,
    pub(crate) buffer: f64,
    pub(crate) max_spread: f64,
    pub(crate) chop_zero_cross_limit: usize,
    pub(crate) chop_value_edge: f64,
    pub(crate) iv_ratio_block_favorite_below: f64,
    pub(crate) no_new_trade_over_secs: f64,
    pub(crate) edge_threshold_30_90_secs: f64,
    pub(crate) edge_threshold_15_30_secs: f64,
    pub(crate) edge_threshold_8_15_secs: f64,
    pub(crate) no_new_trade_under_secs: f64,
    pub(crate) fast_vol_window_secs: i64,
    pub(crate) fast_vol_multiplier: f64,
    pub(crate) latency_buffer_secs: f64,
    pub(crate) high_price_penalty_threshold: f64,
    pub(crate) high_price_penalty: f64,
    pub(crate) stale_penalty_ms: i64,
    pub(crate) stale_penalty: f64,
    pub(crate) drop_penalty_start_z: f64,
    pub(crate) drop_penalty_per_z: f64,
    pub(crate) falling_knife_drop_z: f64,
    pub(crate) recovery_edge_threshold: f64,
    pub(crate) binance_stale_ms: i64,
    pub(crate) binance_q_buffer: f64,
    pub(crate) binance_missing_ask_threshold: f64,
    pub(crate) binance_missing_penalty: f64,
    pub(crate) min_adjusted_margin: f64,
    pub(crate) min_final_q: Option<f64>,
    pub(crate) binance_disagreement_threshold: Option<f64>,
    pub(crate) binance_disagreement_penalty: f64,
    pub(crate) large_binance_disagreement_threshold: Option<f64>,
    pub(crate) large_binance_disagreement_penalty: f64,
    pub(crate) node_max_price: Option<f64>,
    pub(crate) time_rules: Vec<PriceToBeatIvMismatchTimeRule>,
    pub(crate) stale_gap_strength_penalty_ms: i64,
    pub(crate) stale_gap_strength_penalty: f64,
    pub(crate) negative_velocity_gap_strength_penalty: f64,
    pub(crate) protection_mode: PriceToBeatIvProtectionMode,
    pub(crate) book_quotes: Option<PriceToBeatIvBookQuotes>,
    pub(crate) book_lead_guard_enabled: bool,
    pub(crate) book_lead_under_sec: f64,
    pub(crate) book_lead_min_mid_diff: f64,
    pub(crate) opposite_mid_block: Option<f64>,
    pub(crate) block_on_opposite_book_lead: bool,
    pub(crate) require_binance_fresh_under_sec: Option<f64>,
    pub(crate) require_binance_same_direction: bool,
    pub(crate) too_good_to_be_true_gap: Option<f64>,
    pub(crate) model_book_gap_warn: Option<f64>,
    pub(crate) model_book_warn_threshold_penalty: f64,
    pub(crate) model_book_warn_gap_strength_penalty: f64,
    pub(crate) depth_guard_enabled: bool,
    pub(crate) depth_guard_hard_block_enabled: bool,
    pub(crate) depth_max_slippage: f64,
    pub(crate) depth_order_book: Option<OrderBookSnapshot>,
    pub(crate) depth_intended_qty: Option<f64>,
    pub(crate) late_high_price_soft_under_sec: f64,
    pub(crate) late_high_price_ask: f64,
    pub(crate) late_high_price_selected_mid_soft: f64,
    pub(crate) late_high_price_threshold_penalty: f64,
    pub(crate) late_high_price_selected_mid_hard: f64,
    pub(crate) late_high_price_min_gap_usd: f64,
    pub(crate) participation_credit_enabled: bool,
    pub(crate) participation_last_fill_age_minutes: Option<f64>,
    pub(crate) participation_after_minutes: f64,
    pub(crate) participation_long_after_minutes: f64,
    pub(crate) participation_credit: f64,
    pub(crate) participation_long_credit: f64,
    pub(crate) participation_min_threshold: f64,
    pub(crate) momentum_protection_enabled: bool,
    pub(crate) protection_drop_z_block_threshold: f64,
    pub(crate) protection_soft_threshold_penalty: f64,
    pub(crate) protection_soft_gap_strength_penalty: f64,
    pub(crate) entry_quality: IvEntryQualityConfig,
    pub(crate) cex_open_gap: PriceToBeatIvCexOpenGapConfig,
    pub(crate) execution_vwap_guard: PriceToBeatIvExecutionVwapConfig,
    pub(crate) gap_fail_cex_book: PriceToBeatIvGapFailCexBookGuardConfig,
    pub(crate) oracle_lag_book_lead: PriceToBeatIvOracleLagBookLeadConfig,
    pub(crate) borderline_pump_book_lead: PriceToBeatIvBorderlinePumpBookLeadConfig,
    pub(crate) pump_shock: PriceToBeatIvPumpShockConfig,
    pub(crate) expected_move_floor: PriceToBeatIvExpectedMoveFloorConfig,
    pub(crate) ptb_chop: PriceToBeatIvPtbChopConfig,
    pub(crate) adaptive: PriceToBeatIvAdaptiveConfig,
    pub(crate) adaptive_volume: Option<PriceToBeatIvAdaptiveVolumeInput>,
}

impl PriceToBeatIvMismatchEdgeConfig {
    pub(crate) fn crypto_defaults(market: PriceToBeatSignalFormulaMarketInput) -> Self {
        Self {
            market,
            vol_window_secs: DEFAULT_VOL_WINDOW_SECS,
            min_vol_samples: DEFAULT_MIN_VOL_SAMPLES,
            chainlink_stale_ms: DEFAULT_CHAINLINK_STALE_MS,
            buffer: DEFAULT_BUFFER,
            max_spread: DEFAULT_MAX_SPREAD,
            chop_zero_cross_limit: DEFAULT_CHOP_ZERO_CROSS_LIMIT,
            chop_value_edge: DEFAULT_CHOP_VALUE_EDGE,
            iv_ratio_block_favorite_below: DEFAULT_IV_RATIO_BLOCK_FAVORITE_BELOW,
            no_new_trade_over_secs: DEFAULT_NO_NEW_TRADE_OVER_SECS,
            edge_threshold_30_90_secs: DEFAULT_EDGE_THRESHOLD_30_90_SECS,
            edge_threshold_15_30_secs: DEFAULT_EDGE_THRESHOLD_15_30_SECS,
            edge_threshold_8_15_secs: DEFAULT_EDGE_THRESHOLD_8_15_SECS,
            no_new_trade_under_secs: DEFAULT_NO_NEW_TRADE_UNDER_SECS,
            fast_vol_window_secs: DEFAULT_FAST_VOL_WINDOW_SECS,
            fast_vol_multiplier: DEFAULT_FAST_VOL_MULTIPLIER,
            latency_buffer_secs: DEFAULT_LATENCY_BUFFER_SECS,
            high_price_penalty_threshold: DEFAULT_HIGH_PRICE_PENALTY_THRESHOLD,
            high_price_penalty: DEFAULT_HIGH_PRICE_PENALTY,
            stale_penalty_ms: DEFAULT_STALE_PENALTY_MS,
            stale_penalty: DEFAULT_STALE_PENALTY,
            drop_penalty_start_z: DEFAULT_DROP_PENALTY_START_Z,
            drop_penalty_per_z: DEFAULT_DROP_PENALTY_PER_Z,
            falling_knife_drop_z: DEFAULT_FALLING_KNIFE_DROP_Z,
            recovery_edge_threshold: DEFAULT_RECOVERY_EDGE_THRESHOLD,
            binance_stale_ms: DEFAULT_BINANCE_STALE_MS,
            binance_q_buffer: DEFAULT_BINANCE_Q_BUFFER,
            binance_missing_ask_threshold: DEFAULT_BINANCE_MISSING_ASK_THRESHOLD,
            binance_missing_penalty: DEFAULT_BINANCE_MISSING_PENALTY,
            min_adjusted_margin: DEFAULT_MIN_ADJUSTED_MARGIN,
            min_final_q: None,
            binance_disagreement_threshold: None,
            binance_disagreement_penalty: DEFAULT_BINANCE_DISAGREEMENT_PENALTY,
            large_binance_disagreement_threshold: None,
            large_binance_disagreement_penalty: DEFAULT_BINANCE_DISAGREEMENT_PENALTY,
            node_max_price: None,
            time_rules: Vec::new(),
            stale_gap_strength_penalty_ms: DEFAULT_STALE_GAP_STRENGTH_PENALTY_MS,
            stale_gap_strength_penalty: 0.0,
            negative_velocity_gap_strength_penalty: 0.0,
            protection_mode: PriceToBeatIvProtectionMode::Off,
            book_quotes: None,
            book_lead_guard_enabled: false,
            book_lead_under_sec: DEFAULT_PROTECTION_BOOK_LEAD_UNDER_SEC,
            book_lead_min_mid_diff: DEFAULT_PROTECTION_BOOK_LEAD_MIN_MID_DIFF,
            opposite_mid_block: Some(DEFAULT_PROTECTION_OPPOSITE_MID_BLOCK),
            block_on_opposite_book_lead: true,
            require_binance_fresh_under_sec: None,
            require_binance_same_direction: false,
            too_good_to_be_true_gap: Some(DEFAULT_PROTECTION_MODEL_BOOK_GAP_HARD),
            model_book_gap_warn: Some(DEFAULT_PROTECTION_MODEL_BOOK_GAP_WARN),
            model_book_warn_threshold_penalty: DEFAULT_PROTECTION_MODEL_BOOK_WARN_THRESHOLD_PENALTY,
            model_book_warn_gap_strength_penalty: DEFAULT_PROTECTION_MODEL_BOOK_WARN_GAP_PENALTY,
            depth_guard_enabled: true,
            depth_guard_hard_block_enabled: false,
            depth_max_slippage: DEFAULT_DEPTH_MAX_SLIPPAGE,
            depth_order_book: None,
            depth_intended_qty: None,
            late_high_price_soft_under_sec: DEFAULT_LATE_HIGH_PRICE_SOFT_UNDER_SEC,
            late_high_price_ask: DEFAULT_LATE_HIGH_PRICE_ASK,
            late_high_price_selected_mid_soft: DEFAULT_LATE_HIGH_PRICE_SELECTED_MID_SOFT,
            late_high_price_threshold_penalty: DEFAULT_LATE_HIGH_PRICE_THRESHOLD_PENALTY,
            late_high_price_selected_mid_hard: DEFAULT_LATE_HIGH_PRICE_SELECTED_MID_HARD,
            late_high_price_min_gap_usd: DEFAULT_LATE_HIGH_PRICE_MIN_GAP_USD,
            participation_credit_enabled: true,
            participation_last_fill_age_minutes: None,
            participation_after_minutes: DEFAULT_PARTICIPATION_AFTER_MINUTES,
            participation_long_after_minutes: DEFAULT_PARTICIPATION_LONG_AFTER_MINUTES,
            participation_credit: DEFAULT_PARTICIPATION_CREDIT,
            participation_long_credit: DEFAULT_PARTICIPATION_LONG_CREDIT,
            participation_min_threshold: DEFAULT_PARTICIPATION_MIN_THRESHOLD,
            momentum_protection_enabled: false,
            protection_drop_z_block_threshold: DEFAULT_PROTECTION_DROP_Z_BLOCK_THRESHOLD,
            protection_soft_threshold_penalty: DEFAULT_PROTECTION_SOFT_THRESHOLD_PENALTY,
            protection_soft_gap_strength_penalty: DEFAULT_PROTECTION_SOFT_GAP_STRENGTH_PENALTY,
            entry_quality: IvEntryQualityConfig::default(),
            cex_open_gap: PriceToBeatIvCexOpenGapConfig::default(),
            execution_vwap_guard: PriceToBeatIvExecutionVwapConfig::default(),
            gap_fail_cex_book: PriceToBeatIvGapFailCexBookGuardConfig::default(),
            oracle_lag_book_lead: PriceToBeatIvOracleLagBookLeadConfig::default(),
            borderline_pump_book_lead: PriceToBeatIvBorderlinePumpBookLeadConfig::default(),
            pump_shock: PriceToBeatIvPumpShockConfig::default(),
            expected_move_floor: PriceToBeatIvExpectedMoveFloorConfig::default(),
            ptb_chop: PriceToBeatIvPtbChopConfig::default(),
            adaptive: PriceToBeatIvAdaptiveConfig::default(),
            adaptive_volume: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvMismatchEdgeEvaluation {
    pub(crate) passed: bool,
    pub(crate) reason: &'static str,
    pub(crate) all_reasons: Vec<&'static str>,
    pub(crate) selected_side: Option<&'static str>,
    pub(crate) candidate_side: Option<&'static str>,
    pub(crate) seconds_left: Option<f64>,
    pub(crate) q: Option<f64>,
    pub(crate) q_up: Option<f64>,
    pub(crate) q_down: Option<f64>,
    pub(crate) cost: Option<f64>,
    pub(crate) edge: Option<f64>,
    pub(crate) sigma: Option<f64>,
    pub(crate) iv_ratio: Option<f64>,
    pub(crate) zero_cross_count: Option<usize>,
    pub(crate) chainlink_staleness_ms: Option<i64>,
    pub(crate) spread: Option<f64>,
    pub(crate) threshold: Option<f64>,
    pub(crate) ask: Option<f64>,
    pub(crate) bid: Option<f64>,
    pub(crate) node_max_price: Option<f64>,
    pub(crate) effective_max_price: Option<f64>,
    pub(crate) fee: Option<f64>,
    pub(crate) buffer: f64,
    pub(crate) sample_count: Option<usize>,
    pub(crate) delta_count: Option<usize>,
    pub(crate) expected_move: Option<f64>,
    pub(crate) z: Option<f64>,
    pub(crate) x_now: Option<f64>,
    pub(crate) x_prev: Option<f64>,
    pub(crate) gap_velocity: Option<f64>,
    pub(crate) latency_horizon_secs: Option<f64>,
    pub(crate) x_eff: Option<f64>,
    pub(crate) sigma_15: Option<f64>,
    pub(crate) sigma_eff: Option<f64>,
    pub(crate) expected_move_model: Option<f64>,
    pub(crate) expected_move_floor: Option<f64>,
    pub(crate) q_before_floor: Option<f64>,
    pub(crate) q_after_floor: Option<f64>,
    pub(crate) q_chain_adj: Option<f64>,
    pub(crate) binance_price: Option<f64>,
    pub(crate) binance_staleness_ms: Option<i64>,
    pub(crate) q_binance: Option<f64>,
    pub(crate) q_final: Option<f64>,
    pub(crate) edge_adj: Option<f64>,
    pub(crate) adjusted_margin: Option<f64>,
    pub(crate) min_adjusted_margin: Option<f64>,
    pub(crate) thin_margin_flag: Option<bool>,
    pub(crate) min_final_q: Option<f64>,
    pub(crate) q_disagreement: Option<f64>,
    pub(crate) q_disagreement_abs: Option<f64>,
    pub(crate) q_disagreement_bucket: Option<&'static str>,
    pub(crate) dynamic_threshold_before_participation: Option<f64>,
    pub(crate) dynamic_threshold: Option<f64>,
    pub(crate) participation_credit: Option<f64>,
    pub(crate) participation_last_fill_age_minutes: Option<f64>,
    pub(crate) high_price_penalty_applied: Option<f64>,
    pub(crate) stale_penalty_applied: Option<f64>,
    pub(crate) drop_penalty_applied: Option<f64>,
    pub(crate) binance_missing_penalty_applied: Option<f64>,
    pub(crate) binance_disagreement_penalty_applied: Option<f64>,
    pub(crate) confidence_score: Option<f64>,
    pub(crate) drop_z: Option<f64>,
    pub(crate) binance_veto_status: Option<String>,
    pub(crate) selected_time_rule_index: Option<usize>,
    pub(crate) selected_time_rule: Option<PriceToBeatIvMismatchTimeRule>,
    pub(crate) time_rule_max_price: Option<f64>,
    pub(crate) expected_move_eff: Option<f64>,
    pub(crate) gap_strength: Option<f64>,
    pub(crate) required_gap_strength: Option<f64>,
    pub(crate) required_gap_usd: Option<f64>,
    pub(crate) gap_strength_stale_penalty: Option<f64>,
    pub(crate) gap_strength_velocity_penalty: Option<f64>,
    pub(crate) protection_mode: Option<&'static str>,
    pub(crate) protection_result: Option<&'static str>,
    pub(crate) protection_reasons: Vec<&'static str>,
    pub(crate) protection_threshold_penalty: Option<f64>,
    pub(crate) protection_gap_strength_penalty: Option<f64>,
    pub(crate) up_bid: Option<f64>,
    pub(crate) up_ask: Option<f64>,
    pub(crate) down_bid: Option<f64>,
    pub(crate) down_ask: Option<f64>,
    pub(crate) depth: PriceToBeatIvDepthEvaluation,
    pub(crate) depth_guard_hard_block_enabled: bool,
    pub(crate) up_mid: Option<f64>,
    pub(crate) down_mid: Option<f64>,
    pub(crate) book_side: Option<&'static str>,
    pub(crate) book_mid_diff: Option<f64>,
    pub(crate) opposite_mid: Option<f64>,
    pub(crate) selected_mid: Option<f64>,
    pub(crate) selected_ask: Option<f64>,
    pub(crate) model_book_gap: Option<f64>,
    pub(crate) model_book_gap_warn_threshold: Option<f64>,
    pub(crate) too_good_threshold: Option<f64>,
    pub(crate) book_confirmation_result: Option<&'static str>,
    pub(crate) gap_strength_margin: Option<f64>,
    pub(crate) gap_usd_margin: Option<f64>,
    pub(crate) min_gap_strength_margin: Option<f64>,
    pub(crate) min_gap_usd_margin: Option<f64>,
    pub(crate) binance_same_direction: Option<bool>,
    pub(crate) falling_knife_flag: Option<bool>,
    pub(crate) entry_quality: Option<IvEntryQualityDecision>,
    pub(crate) cex_open_gap: PriceToBeatIvCexOpenGapEvaluation,
    pub(crate) execution_vwap: PriceToBeatIvExecutionVwapEvaluation,
    pub(crate) gap_fail_cex_book: PriceToBeatIvGapFailCexBookGuardEvaluation,
    pub(crate) oracle_lag_book_lead: PriceToBeatIvOracleLagBookLeadEvaluation,
    pub(crate) borderline_pump_book_lead: PriceToBeatIvBorderlinePumpBookLeadEvaluation,
    pub(crate) pump_shock: PriceToBeatIvPumpShockEvaluation,
    pub(crate) expected_move_floor_debug: PriceToBeatIvExpectedMoveFloorEvaluation,
    pub(crate) ptb_chop: PriceToBeatIvPtbChopEvaluation,
    pub(crate) adaptive: Option<PriceToBeatIvAdaptiveEvaluation>,
}

impl PriceToBeatIvMismatchEdgeEvaluation {
    fn new(config: &PriceToBeatIvMismatchEdgeConfig) -> Self {
        Self {
            passed: false,
            reason: "pending",
            all_reasons: Vec::new(),
            selected_side: None,
            candidate_side: None,
            seconds_left: None,
            q: None,
            q_up: None,
            q_down: None,
            cost: None,
            edge: None,
            sigma: None,
            iv_ratio: None,
            zero_cross_count: None,
            chainlink_staleness_ms: None,
            spread: None,
            threshold: None,
            ask: config.market.best_ask,
            bid: config.market.best_bid,
            node_max_price: config.node_max_price,
            effective_max_price: None,
            fee: None,
            buffer: config.buffer,
            sample_count: None,
            delta_count: None,
            expected_move: None,
            z: None,
            x_now: None,
            x_prev: None,
            gap_velocity: None,
            latency_horizon_secs: None,
            x_eff: None,
            sigma_15: None,
            sigma_eff: None,
            expected_move_model: None,
            expected_move_floor: None,
            q_before_floor: None,
            q_after_floor: None,
            q_chain_adj: None,
            binance_price: None,
            binance_staleness_ms: None,
            q_binance: None,
            q_final: None,
            edge_adj: None,
            adjusted_margin: None,
            min_adjusted_margin: None,
            thin_margin_flag: None,
            min_final_q: None,
            q_disagreement: None,
            q_disagreement_abs: None,
            q_disagreement_bucket: None,
            dynamic_threshold_before_participation: None,
            dynamic_threshold: None,
            participation_credit: None,
            participation_last_fill_age_minutes: config.participation_last_fill_age_minutes,
            high_price_penalty_applied: None,
            stale_penalty_applied: None,
            drop_penalty_applied: None,
            binance_missing_penalty_applied: None,
            binance_disagreement_penalty_applied: None,
            confidence_score: None,
            drop_z: None,
            binance_veto_status: None,
            selected_time_rule_index: None,
            selected_time_rule: None,
            time_rule_max_price: None,
            expected_move_eff: None,
            gap_strength: None,
            required_gap_strength: None,
            required_gap_usd: None,
            gap_strength_stale_penalty: None,
            gap_strength_velocity_penalty: None,
            protection_mode: Some(config.protection_mode.as_str()),
            protection_result: None,
            protection_reasons: Vec::new(),
            protection_threshold_penalty: None,
            protection_gap_strength_penalty: None,
            up_bid: config.book_quotes.and_then(|book| book.up_bid),
            up_ask: config.book_quotes.and_then(|book| book.up_ask),
            down_bid: config.book_quotes.and_then(|book| book.down_bid),
            down_ask: config.book_quotes.and_then(|book| book.down_ask),
            depth: PriceToBeatIvDepthEvaluation::off(),
            depth_guard_hard_block_enabled: config.depth_guard_hard_block_enabled,
            up_mid: None,
            down_mid: None,
            book_side: None,
            book_mid_diff: None,
            opposite_mid: None,
            selected_mid: None,
            selected_ask: None,
            model_book_gap: None,
            model_book_gap_warn_threshold: config.model_book_gap_warn,
            too_good_threshold: config.too_good_to_be_true_gap,
            book_confirmation_result: None,
            gap_strength_margin: None,
            gap_usd_margin: None,
            min_gap_strength_margin: None,
            min_gap_usd_margin: None,
            binance_same_direction: None,
            falling_knife_flag: None,
            entry_quality: None,
            cex_open_gap: PriceToBeatIvCexOpenGapEvaluation::default(),
            execution_vwap: PriceToBeatIvExecutionVwapEvaluation::default(),
            gap_fail_cex_book: PriceToBeatIvGapFailCexBookGuardEvaluation::default(),
            oracle_lag_book_lead: PriceToBeatIvOracleLagBookLeadEvaluation::default(),
            borderline_pump_book_lead: PriceToBeatIvBorderlinePumpBookLeadEvaluation::default(),
            pump_shock: PriceToBeatIvPumpShockEvaluation::default(),
            expected_move_floor_debug: PriceToBeatIvExpectedMoveFloorEvaluation::fixed(),
            ptb_chop: PriceToBeatIvPtbChopEvaluation::off(&config.ptb_chop),
            adaptive: None,
        }
    }

    fn finish(mut self, passed: bool, reason: &'static str) -> Self {
        self.passed = passed;
        self.reason = reason;
        if passed {
            self.selected_side = self.candidate_side;
        }
        self
    }

    pub(crate) fn to_value(&self) -> Value {
        iv_mismatch_edge_telemetry::to_value(self)
    }
}

pub(crate) fn evaluate_price_to_beat_iv_mismatch_edge(
    market_slug: &str,
    outcome_label: &str,
    asset: &str,
    current_price: f64,
    price_to_beat: f64,
    config: PriceToBeatIvMismatchEdgeConfig,
) -> PriceToBeatIvMismatchEdgeEvaluation {
    let mut evaluation = PriceToBeatIvMismatchEdgeEvaluation::new(&config);
    let Some(side) = iv_mismatch_side(outcome_label) else {
        return evaluation.finish(false, "unsupported_outcome_label");
    };
    evaluation.candidate_side = Some(side);

    let Some(seconds_left) = iv_mismatch_seconds_left(market_slug) else {
        return evaluation.finish(false, "market_window_unavailable");
    };
    evaluation.seconds_left = Some(seconds_left);
    let selected_time_rule = select_time_rule(seconds_left, &config);
    if !config.time_rules.is_empty() && selected_time_rule.is_none() {
        return evaluation.finish(false, "blocked_no_matching_time_rule");
    }
    if let Some((index, rule)) = selected_time_rule {
        evaluation.selected_time_rule_index = Some(index);
        evaluation.selected_time_rule = Some(rule);
        evaluation.time_rule_max_price = rule.max_price;
    }
    evaluation.effective_max_price = match (config.node_max_price, evaluation.time_rule_max_price) {
        (Some(node_max_price), Some(rule_max_price)) => Some(node_max_price.min(rule_max_price)),
        (Some(node_max_price), None) => Some(node_max_price),
        (None, Some(rule_max_price)) => Some(rule_max_price),
        (None, None) => None,
    };
    let threshold = match edge_threshold_for_seconds_left(seconds_left, &config, selected_time_rule)
    {
        Ok(threshold) => threshold,
        Err(reason) => return evaluation.finish(false, reason),
    };
    evaluation.threshold = Some(threshold);

    let Some(ask) = config
        .market
        .best_ask
        .filter(|value| valid_probability(*value))
    else {
        return evaluation.finish(false, "orderbook_unavailable");
    };
    let Some(bid) = config
        .market
        .best_bid
        .filter(|value| valid_probability(*value))
    else {
        return evaluation.finish(false, "orderbook_unavailable");
    };
    if ask < bid {
        evaluation.spread = Some(ask - bid);
        return evaluation.finish(false, "invalid_spread");
    }
    let spread = ask - bid;
    evaluation.spread = Some(spread);
    if spread > config.max_spread {
        return evaluation.finish(false, "blocked_spread_wide");
    }
    let time_rule_price_blocked = !config.entry_quality.eq77_risk_cap_enabled
        && evaluation
            .time_rule_max_price
            .map(|max_price| ask > max_price)
            .unwrap_or(false);

    let now_ms = Utc::now().timestamp_millis();
    let samples = match get_chainlink_price_samples(
        asset,
        now_ms - config.vol_window_secs.max(1) * 1_000,
        now_ms,
    ) {
        Ok(samples) => samples,
        Err(_) => return evaluation.finish(false, "blocked_insufficient_vol_samples"),
    };
    evaluation.sample_count = Some(samples.len());
    if samples.len() < config.min_vol_samples {
        return evaluation.finish(false, "blocked_insufficient_vol_samples");
    }
    let latest_timestamp_ms = samples
        .iter()
        .map(|sample| sample.timestamp_ms)
        .max()
        .unwrap_or(now_ms);
    let staleness_ms = now_ms.saturating_sub(latest_timestamp_ms);
    evaluation.chainlink_staleness_ms = Some(staleness_ms);
    if staleness_ms > config.chainlink_stale_ms {
        return evaluation.finish(false, "blocked_rtds_stale");
    }

    let deltas = time_normalized_price_deltas(&samples);
    evaluation.delta_count = Some(deltas.len());
    if deltas.len() + 1 < config.min_vol_samples {
        return evaluation.finish(false, "blocked_insufficient_vol_samples");
    }
    let sigma = standard_deviation(&deltas);
    if !sigma.is_finite() || sigma <= 0.0 {
        return evaluation.finish(false, "blocked_zero_sigma");
    }
    evaluation.sigma = Some(sigma);

    let zero_cross_count = zero_cross_count(&samples, price_to_beat);
    evaluation.zero_cross_count = Some(zero_cross_count);

    let gap = current_price - price_to_beat;
    let expected_move = sigma * seconds_left.sqrt();
    if !expected_move.is_finite() || expected_move <= 0.0 {
        return evaluation.finish(false, "blocked_zero_sigma");
    }
    let z = gap / expected_move;
    let q_up = normal_cdf(z);
    let q_down = 1.0 - q_up;
    let q = if side == "up" { q_up } else { q_down };
    let depth = evaluate_price_to_beat_iv_depth(
        config.depth_order_book.as_ref(),
        ask,
        config.depth_intended_qty,
        config.depth_max_slippage,
        config.depth_guard_enabled
            || config.execution_vwap_guard.enabled
            || config.execution_vwap_guard.limit_by_vwap_enabled,
    );
    let effective_fill_price = depth.estimated_avg_fill.unwrap_or(ask);
    let fee = signal_formula_taker_fee(effective_fill_price);
    let cost = effective_fill_price + fee + config.buffer.max(0.0);
    let edge = q - cost;
    let iv_ratio = implied_volatility_ratio(cost, gap.abs(), seconds_left, sigma);
    let sigma_15 = sigma_since(
        &samples,
        latest_timestamp_ms - config.fast_vol_window_secs.max(1) * 1_000,
    );
    let sigma_eff = sigma_15
        .map(|fast| sigma.max(config.fast_vol_multiplier.max(0.0) * fast))
        .unwrap_or(sigma);
    let x_now = side_gap(side, current_price, price_to_beat);
    let latency_horizon_secs =
        (staleness_ms as f64 / 1_000.0) + config.latency_buffer_secs.max(0.0);
    let (x_prev, gap_velocity) =
        previous_side_gap(&samples, side, price_to_beat, latest_timestamp_ms)
            .map(|(sample_ts, sample_gap)| {
                let dt_secs = ((latest_timestamp_ms - sample_ts) as f64 / 1_000.0).max(0.001);
                (Some(sample_gap), Some((x_now - sample_gap) / dt_secs))
            })
            .unwrap_or((None, None));
    let x_eff = x_now + gap_velocity.unwrap_or(0.0).min(0.0) * latency_horizon_secs;
    let expected_move_model = sigma_eff * seconds_left.sqrt();
    if !expected_move_model.is_finite() || expected_move_model <= 0.0 {
        return evaluation.finish(false, "blocked_zero_sigma");
    }
    let z_before_floor = x_eff / expected_move_model;
    let q_before_floor = normal_cdf(z_before_floor);
    let time_rule_expected_move_floor = selected_time_rule
        .and_then(|(_, rule)| rule.min_expected_move_usd)
        .filter(|value| value.is_finite() && *value > 0.0);
    let entry_quality_expected_move_floor = config
        .entry_quality
        .enabled
        .then(|| config.entry_quality.expected_move_floor(current_price))
        .filter(|value| value.is_finite() && *value > 0.0);
    let expected_move_floor = [
        time_rule_expected_move_floor,
        entry_quality_expected_move_floor,
    ]
    .into_iter()
    .flatten()
    .max_by(f64::total_cmp);
    let base_expected_move_eff = expected_move_floor
        .map(|floor| expected_move_model.max(floor))
        .unwrap_or(expected_move_model);
    let expected_move_floor_debug =
        if config.expected_move_floor.mode == PriceToBeatIvMinExpectedMoveMode::Adaptive {
            let base_q_chain_adj = normal_cdf(x_eff / base_expected_move_eff);
            let preliminary_binance_adjustment = evaluate_binance_veto(
                asset,
                side,
                price_to_beat,
                base_expected_move_eff,
                base_q_chain_adj,
                now_ms,
                &config,
            );
            let preliminary_disagreement = evaluate_binance_disagreement_penalty(
                base_q_chain_adj,
                preliminary_binance_adjustment.q_binance,
                &config,
            );
            evaluate_expected_move_floor(
                &config.expected_move_floor,
                PriceToBeatIvExpectedMoveFloorInput {
                    current_price,
                    spread,
                    source_staleness_ms: staleness_ms.max(
                        preliminary_binance_adjustment
                            .binance_staleness_ms
                            .unwrap_or(0),
                    ),
                    sigma_fast: sigma_15,
                    sigma_eff,
                    disagreement_abs: preliminary_disagreement.absolute,
                },
            )
        } else {
            PriceToBeatIvExpectedMoveFloorEvaluation::fixed()
        };
    let expected_move_floor = [expected_move_floor, expected_move_floor_debug.floor_usd]
        .into_iter()
        .flatten()
        .max_by(f64::total_cmp);
    let expected_move_eff = expected_move_floor
        .map(|floor| expected_move_model.max(floor))
        .unwrap_or(expected_move_model);
    let z_adj = x_eff / expected_move_eff;
    let q_after_floor = normal_cdf(z_adj);
    let q_chain_adj = q_after_floor;
    let gap_strength = x_now / expected_move_eff;
    let drop_z = side_gap_at_or_before(&samples, side, price_to_beat, latest_timestamp_ms - 3_000)
        .map(|x_ago| {
            let denominator = sigma_eff * 3.0_f64.sqrt();
            if denominator > 0.0 {
                (x_ago - x_now) / denominator
            } else {
                0.0
            }
        })
        .unwrap_or(0.0);
    let high_price_penalty = if ask >= config.high_price_penalty_threshold {
        config.high_price_penalty.max(0.0)
    } else {
        0.0
    };
    let stale_penalty = if staleness_ms > config.stale_penalty_ms {
        config.stale_penalty.max(0.0)
    } else {
        0.0
    };
    let drop_penalty =
        config.drop_penalty_per_z.max(0.0) * (drop_z - config.drop_penalty_start_z).max(0.0);
    let gap_strength_stale_penalty = if staleness_ms > config.stale_gap_strength_penalty_ms {
        config.stale_gap_strength_penalty.max(0.0)
    } else {
        0.0
    };
    let gap_strength_velocity_penalty = if gap_velocity.unwrap_or(0.0) < 0.0 {
        config.negative_velocity_gap_strength_penalty.max(0.0)
    } else {
        0.0
    };
    let raw_required_gap_strength = selected_time_rule
        .map(|(_, rule)| rule.min_gap_strength)
        .unwrap_or(0.0)
        + gap_strength_stale_penalty
        + gap_strength_velocity_penalty;
    let binance_adjustment = evaluate_binance_veto(
        asset,
        side,
        price_to_beat,
        expected_move_eff,
        q_chain_adj,
        now_ms,
        &config,
    );
    let binance_missing_penalty =
        if ask >= config.binance_missing_ask_threshold && binance_adjustment.is_missing() {
            config.binance_missing_penalty.max(0.0)
        } else {
            0.0
        };
    let binance_disagreement =
        evaluate_binance_disagreement_penalty(q_chain_adj, binance_adjustment.q_binance, &config);
    let base_dynamic_threshold = threshold
        + high_price_penalty
        + stale_penalty
        + drop_penalty
        + binance_missing_penalty
        + binance_disagreement.penalty;
    let mut cex_open_gap = evaluate_price_to_beat_iv_cex_open_gap(PriceToBeatIvCexOpenGapInput {
        config: config.cex_open_gap,
        market_slug,
        asset,
        selected_side: side,
        current_price,
        chainlink_signed_gap: x_now,
        expected_move_eff,
        q_final_before: binance_adjustment.q_final,
    });
    let cex_open_gap_block_reason = cex_open_gap.block_reason;
    let q_final = cex_open_gap
        .q_final_after_cex_consensus
        .unwrap_or(binance_adjustment.q_final);
    let binance_same_direction = binance_adjustment
        .binance_price
        .map(|price| side_gap(side, price, price_to_beat) > 0.0);
    let base_min_gap_strength_margin = selected_time_rule
        .and_then(|(_, rule)| rule.min_gap_strength_margin)
        .filter(|value| value.is_finite() && *value >= 0.0);
    let base_min_gap_usd_margin = selected_time_rule
        .and_then(|(_, rule)| rule.min_gap_usd_margin)
        .filter(|value| value.is_finite() && *value >= 0.0);
    let adaptive = (config.protection_mode == PriceToBeatIvProtectionMode::Adaptive).then(|| {
        evaluate_price_to_beat_iv_adaptive_volume(&PriceToBeatIvAdaptiveInput {
            selected_side: side,
            seconds_left,
            book_quotes: config.book_quotes,
            book_lead_guard_enabled: config.book_lead_guard_enabled,
            book_lead_under_sec: config.book_lead_under_sec,
            book_lead_min_mid_diff: config.book_lead_min_mid_diff,
            binance_same_direction,
            zero_cross_count,
            chop_zero_cross_limit: config.chop_zero_cross_limit,
            base_min_edge: base_dynamic_threshold,
            base_gap_strength: raw_required_gap_strength,
            base_gap_usd_margin: base_min_gap_usd_margin,
            volume: config.adaptive_volume.as_ref(),
            config: &config.adaptive,
        })
    });
    let adaptive_edge_delta = adaptive
        .as_ref()
        .map(|evaluation| evaluation.edge_delta)
        .unwrap_or(0.0);
    let adaptive_gap_strength_delta = adaptive
        .as_ref()
        .map(|evaluation| evaluation.gap_strength_delta)
        .unwrap_or(0.0);
    let adaptive_gap_usd_margin_delta = adaptive
        .as_ref()
        .map(|evaluation| evaluation.gap_usd_margin_delta)
        .unwrap_or(0.0);
    let base_required_gap_strength_before_chop =
        raw_required_gap_strength + adaptive_gap_strength_delta.max(-raw_required_gap_strength);
    let ptb_movement_model_book_dislocation = config
        .book_quotes
        .and_then(|quotes| selected_book_mid_for_ptb_movement(quotes, side))
        .map(|selected_mid| q_final - selected_mid);
    let ptb_chop = evaluate_price_to_beat_iv_ptb_chop(PriceToBeatIvPtbChopInput {
        config: &config.ptb_chop,
        asset,
        selected_side: side,
        samples: &samples,
        price_to_beat,
        current_price,
        latest_timestamp_ms,
        expected_move_eff,
        gap_strength,
        required_gap_strength: base_required_gap_strength_before_chop,
        cex_consensus: config
            .cex_open_gap
            .enabled
            .then_some(cex_open_gap.consensus),
        model_book_dislocation: ptb_movement_model_book_dislocation,
    });
    let base_required_gap_strength =
        base_required_gap_strength_before_chop + ptb_chop.gap_strength_penalty.max(0.0);
    let base_required_gap_usd = base_required_gap_strength * expected_move_eff;
    let min_gap_strength_margin = base_min_gap_strength_margin;
    let min_gap_usd_margin = base_min_gap_usd_margin
        .map(|value| (value + adaptive_gap_usd_margin_delta.max(-value)).max(0.0));
    let base_dynamic_threshold = base_dynamic_threshold + adaptive_edge_delta;
    let protection = evaluate_price_to_beat_iv_protection(&PriceToBeatIvProtectionInput {
        mode: config.protection_mode,
        selected_side: side,
        seconds_left,
        book_quotes: config.book_quotes,
        book_lead_guard_enabled: config.book_lead_guard_enabled,
        book_lead_under_sec: config.book_lead_under_sec,
        book_lead_min_mid_diff: config.book_lead_min_mid_diff,
        opposite_mid_block: config.opposite_mid_block,
        block_on_opposite_book_lead: config.block_on_opposite_book_lead,
        require_binance_fresh_under_sec: config.require_binance_fresh_under_sec,
        require_binance_same_direction: config.require_binance_same_direction,
        binance_fresh: binance_adjustment.is_fresh(),
        binance_same_direction,
        model_book_gap_warn: config.model_book_gap_warn,
        model_book_gap_hard: config.too_good_to_be_true_gap,
        model_book_warn_threshold_penalty: config.model_book_warn_threshold_penalty,
        model_book_warn_gap_strength_penalty: config.model_book_warn_gap_strength_penalty,
        depth_block_reason: depth.block_reason,
        late_high_price_soft_under_sec: config.late_high_price_soft_under_sec,
        late_high_price_ask: config.late_high_price_ask,
        late_high_price_selected_mid_soft: config.late_high_price_selected_mid_soft,
        late_high_price_threshold_penalty: config.late_high_price_threshold_penalty,
        late_high_price_selected_mid_hard: config.late_high_price_selected_mid_hard,
        late_high_price_min_gap_usd: config.late_high_price_min_gap_usd,
        q_final,
        gap_strength,
        required_gap_strength: base_required_gap_strength,
        directional_gap: x_now,
        required_gap_usd: base_required_gap_usd,
        min_gap_strength_margin,
        min_gap_usd_margin,
        momentum_enabled: config.momentum_protection_enabled,
        gap_velocity,
        drop_z,
        drop_z_block_threshold: config.protection_drop_z_block_threshold,
        soft_threshold_penalty_unit: config.protection_soft_threshold_penalty,
        soft_gap_strength_penalty_unit: config.protection_soft_gap_strength_penalty,
    });
    let required_gap_strength =
        base_required_gap_strength + protection.gap_strength_penalty.max(0.0);
    let required_gap_usd = required_gap_strength * expected_move_eff;
    let dynamic_threshold_before_participation =
        base_dynamic_threshold + protection.threshold_penalty.max(0.0);
    let participation_credit =
        super::iv_mismatch_participation::price_to_beat_iv_participation_threshold_credit(&config);
    let dynamic_threshold = if participation_credit > 0.0 {
        (dynamic_threshold_before_participation - participation_credit)
            .max(config.participation_min_threshold.max(0.0))
    } else {
        dynamic_threshold_before_participation
    };
    let edge_adj = q_final - cost;
    let adjusted_margin = edge_adj - dynamic_threshold;
    let min_adjusted_margin = config.min_adjusted_margin.max(0.0);
    let min_final_q = config
        .min_final_q
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0);
    let thin_margin_flag = adjusted_margin < min_adjusted_margin;
    let confidence_score = (adjusted_margin - min_adjusted_margin).max(0.0);

    evaluation.q = Some(q);
    evaluation.q_up = Some(q_up);
    evaluation.q_down = Some(q_down);
    evaluation.depth = depth;
    evaluation.fee = Some(fee);
    evaluation.cost = Some(cost);
    evaluation.edge = Some(edge);
    evaluation.iv_ratio = iv_ratio;
    evaluation.expected_move = Some(expected_move);
    evaluation.z = Some(z);
    evaluation.x_now = Some(x_now);
    evaluation.x_prev = x_prev;
    evaluation.gap_velocity = gap_velocity;
    evaluation.latency_horizon_secs = Some(latency_horizon_secs);
    evaluation.x_eff = Some(x_eff);
    evaluation.sigma_15 = sigma_15;
    evaluation.sigma_eff = Some(sigma_eff);
    evaluation.expected_move_model = Some(expected_move_model);
    evaluation.expected_move_floor = expected_move_floor;
    evaluation.expected_move_eff = Some(expected_move_eff);
    evaluation.q_before_floor = Some(q_before_floor);
    evaluation.q_after_floor = Some(q_after_floor);
    evaluation.q_chain_adj = Some(q_chain_adj);
    evaluation.binance_price = binance_adjustment.binance_price;
    evaluation.binance_staleness_ms = binance_adjustment.binance_staleness_ms;
    evaluation.q_binance = binance_adjustment.q_binance;
    evaluation.q_final = Some(q_final);
    evaluation.edge_adj = Some(edge_adj);
    evaluation.adjusted_margin = Some(adjusted_margin);
    evaluation.min_adjusted_margin = Some(min_adjusted_margin);
    evaluation.thin_margin_flag = Some(thin_margin_flag);
    evaluation.min_final_q = min_final_q;
    evaluation.q_disagreement = binance_disagreement.adverse;
    evaluation.q_disagreement_abs = binance_disagreement.absolute;
    evaluation.q_disagreement_bucket = binance_disagreement.bucket;
    evaluation.dynamic_threshold_before_participation =
        Some(dynamic_threshold_before_participation);
    evaluation.dynamic_threshold = Some(dynamic_threshold);
    evaluation.participation_credit = Some(participation_credit);
    evaluation.participation_last_fill_age_minutes = config.participation_last_fill_age_minutes;
    evaluation.high_price_penalty_applied = Some(high_price_penalty);
    evaluation.stale_penalty_applied = Some(stale_penalty);
    evaluation.drop_penalty_applied = Some(drop_penalty);
    evaluation.binance_missing_penalty_applied = Some(binance_missing_penalty);
    evaluation.binance_disagreement_penalty_applied = Some(binance_disagreement.penalty);
    evaluation.confidence_score = Some(confidence_score);
    evaluation.drop_z = Some(drop_z);
    evaluation.binance_veto_status = Some(binance_adjustment.status.clone());
    evaluation.gap_strength = Some(gap_strength);
    evaluation.required_gap_strength = Some(required_gap_strength);
    evaluation.required_gap_usd = Some(required_gap_usd);
    evaluation.gap_strength_stale_penalty = Some(gap_strength_stale_penalty);
    evaluation.gap_strength_velocity_penalty = Some(gap_strength_velocity_penalty);
    evaluation.protection_result = Some(protection.result);
    evaluation.protection_reasons = protection.reasons.clone();
    evaluation.protection_threshold_penalty = Some(protection.threshold_penalty);
    evaluation.protection_gap_strength_penalty = Some(protection.gap_strength_penalty);
    evaluation.up_mid = protection.up_mid;
    evaluation.down_mid = protection.down_mid;
    evaluation.book_side = protection.book_side;
    evaluation.book_mid_diff = protection.book_mid_diff;
    evaluation.opposite_mid = protection.opposite_mid;
    evaluation.selected_mid = protection.selected_mid;
    evaluation.selected_ask = protection.selected_ask;
    evaluation.model_book_gap = protection.model_book_gap;
    evaluation.book_confirmation_result = if protection
        .reasons
        .contains(&"blocked_model_book_not_confirmed")
    {
        Some("blocked_model_book_not_confirmed")
    } else if protection
        .reasons
        .contains(&"warn_model_book_not_confirmed")
    {
        Some("warn_model_book_not_confirmed")
    } else if protection.selected_mid.is_some() {
        Some("pass")
    } else {
        None
    };
    evaluation.gap_strength_margin = Some(gap_strength - required_gap_strength);
    evaluation.gap_usd_margin = Some(x_now - required_gap_usd);
    evaluation.min_gap_strength_margin = min_gap_strength_margin;
    evaluation.min_gap_usd_margin = min_gap_usd_margin;
    evaluation.binance_same_direction = binance_same_direction;
    evaluation.falling_knife_flag = protection.falling_knife_flag;
    evaluation.expected_move_floor_debug = expected_move_floor_debug;
    evaluation.ptb_chop = ptb_chop.clone();
    evaluation.adaptive = adaptive.clone();
    let execution_vwap =
        evaluate_price_to_beat_iv_execution_vwap(PriceToBeatIvExecutionVwapInput {
            config: config.execution_vwap_guard,
            time_rule_price_blocked,
            time_rule_max_price: evaluation.time_rule_max_price,
            model_ask: ask,
            depth: &evaluation.depth,
            effective_max_price: evaluation.effective_max_price,
            q_final,
            dynamic_threshold,
            safety_buffer: config.buffer,
        });
    let execution_vwap_block_reason = execution_vwap.block_reason;
    evaluation.execution_vwap = execution_vwap;
    let gap_fail_cex_book =
        evaluate_price_to_beat_iv_gap_fail_cex_book_guard(PriceToBeatIvGapFailCexBookGuardInput {
            config: config.gap_fail_cex_book,
            seconds_left,
            gap_strength,
            required_gap_strength,
            q_raw: q,
            book_confirmation_available: ptb_movement_model_book_dislocation.is_some(),
            cex_open_gap: &cex_open_gap,
            execution_vwap: &evaluation.execution_vwap,
        });
    let gap_fail_cex_book_block_reason = gap_fail_cex_book.block_reason;
    evaluation.gap_fail_cex_book = gap_fail_cex_book;
    let oracle_lag_book_lead =
        evaluate_price_to_beat_iv_oracle_lag_book_lead(PriceToBeatIvOracleLagBookLeadInput {
            config: config.oracle_lag_book_lead,
            seconds_left,
            q_final,
            execution_vwap: &evaluation.execution_vwap,
            spread: Some(spread),
            cex_consensus: config
                .cex_open_gap
                .enabled
                .then_some(cex_open_gap.consensus),
        });
    let oracle_lag_block_reason = oracle_lag_book_lead.block_reason;
    evaluation.oracle_lag_book_lead = oracle_lag_book_lead;
    let chainlink_cex_book_mismatch_block_reason = cex_open_gap_book_mismatch_reason(
        &config.cex_open_gap,
        &cex_open_gap,
        evaluation.oracle_lag_book_lead.dislocation,
    );
    cex_open_gap.chainlink_cex_book_mismatch_reason = chainlink_cex_book_mismatch_block_reason;
    let pump_hold_gap = side_gap_at_or_before(
        &samples,
        side,
        price_to_beat,
        latest_timestamp_ms - config.pump_shock.min_hold_ms.max(0),
    );
    let pump_shock = evaluate_price_to_beat_iv_pump_shock(PriceToBeatIvPumpShockInput {
        config: config.pump_shock,
        seconds_left,
        x_now,
        x_prev,
        expected_move_eff,
        same_side_gap_at_hold: pump_hold_gap,
        model_book_dislocation: evaluation.oracle_lag_book_lead.dislocation,
        dislocation_red: config.oracle_lag_book_lead.dislocation_red,
        cex_consensus: config
            .cex_open_gap
            .enabled
            .then_some(cex_open_gap.consensus),
        execution_ref_reliable: Some(
            evaluation.oracle_lag_book_lead.reference_status == "reliable",
        ),
        token_price_confirming: Some(gap_velocity.unwrap_or(0.0) >= 0.0),
        book_dislocation_improving: None,
    });
    let pump_shock_block_reason = pump_shock.block_reason;
    evaluation.pump_shock = pump_shock;
    evaluation.cex_open_gap = cex_open_gap;
    let borderline_pump_book_lead = evaluate_price_to_beat_iv_borderline_pump_book_lead(
        PriceToBeatIvBorderlinePumpBookLeadInput {
            config: config.borderline_pump_book_lead,
            seconds_left,
            gap_strength,
            required_gap_strength_raw: required_gap_strength,
            q_final,
            oracle_lag_book_lead: &evaluation.oracle_lag_book_lead,
            pump_shock: &evaluation.pump_shock,
        },
    );
    let borderline_pump_book_lead_block_reason = borderline_pump_book_lead.block_reason;
    evaluation.borderline_pump_book_lead = borderline_pump_book_lead;

    let mut eq77_entry_quality_gap_override = false;
    let mut entry_quality_block_reason = None;
    if config.entry_quality.enabled {
        let entry_quality = evaluate_iv_entry_quality(IvEntryQualityInput {
            config: &config.entry_quality,
            side,
            price_to_beat,
            current_price,
            samples: &samples,
            latest_timestamp_ms,
            seconds_left,
            ask,
            spread,
            chainlink_age_ms: Some(staleness_ms),
            expected_move_raw: expected_move_model,
            expected_move_eff,
            q_final: Some(q_final),
            fee,
            buffer: config.buffer,
            dynamic_threshold,
            configured_max_price: evaluation.effective_max_price,
            gap_velocity,
            cex_price: binance_adjustment.binance_price,
            cex_fresh: binance_adjustment.is_fresh(),
            cex_same_direction: binance_same_direction,
            rule_required_gap_strength: Some(required_gap_strength),
            rule_gap_strength_margin: min_gap_strength_margin,
        });
        let block_reason = entry_quality.primary_reason.map(|reason| reason.as_str());
        eq77_entry_quality_gap_override =
            config.entry_quality.eq77_risk_cap_enabled && entry_quality.allowed;
        evaluation.entry_quality = Some(entry_quality);
        entry_quality_block_reason = block_reason;
    }

    if time_rule_price_blocked {
        evaluation.all_reasons.push("blocked_time_rule_max_price");
        if let Some(reason) = cex_open_gap_block_reason {
            evaluation.all_reasons.push(reason);
        }
        for reason in &evaluation.gap_fail_cex_book.all_reasons {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = chainlink_cex_book_mismatch_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = oracle_lag_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = borderline_pump_book_lead_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = pump_shock_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = execution_vwap_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = entry_quality_block_reason {
            evaluation.all_reasons.push(reason);
        }
        if let Some(reason) = evaluation.ptb_chop.block_reason {
            evaluation.all_reasons.push(reason);
        }
        return evaluation.finish(false, "blocked_time_rule_max_price");
    }
    if let Some(reason) = cex_open_gap_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = gap_fail_cex_book_block_reason {
        for reason in &evaluation.gap_fail_cex_book.all_reasons {
            evaluation.all_reasons.push(reason);
        }
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = chainlink_cex_book_mismatch_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = oracle_lag_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = borderline_pump_book_lead_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = pump_shock_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = execution_vwap_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = entry_quality_block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if let Some(reason) = evaluation.ptb_chop.block_reason {
        evaluation.all_reasons.push(reason);
        return evaluation.finish(false, reason);
    }
    if config.depth_guard_enabled && config.depth_guard_hard_block_enabled {
        if let Some(reason) = evaluation.depth.block_reason {
            evaluation.all_reasons.push(reason);
            return evaluation.finish(false, reason);
        }
    }

    if let Some(reason) = protection.block_reason.filter(|reason| {
        matches!(
            *reason,
            "blocked_insufficient_depth"
                | "blocked_depth_guard_unavailable"
                | "blocked_model_book_not_confirmed"
                | "blocked_late_high_price_unconfirmed"
        )
    }) {
        return evaluation.finish(false, reason);
    }
    if gap_strength < required_gap_strength
        && (!eq77_entry_quality_gap_override || evaluation.ptb_chop.gap_strength_penalty > 0.0)
    {
        return evaluation.finish(false, "blocked_gap_strength_below_threshold");
    }
    if iv_ratio
        .map(|ratio| ratio < config.iv_ratio_block_favorite_below)
        .unwrap_or(false)
    {
        return evaluation.finish(false, "blocked_iv_ratio_low");
    }
    if zero_cross_count >= config.chop_zero_cross_limit && edge_adj < config.chop_value_edge {
        return evaluation.finish(false, "blocked_chop");
    }
    if drop_z > config.falling_knife_drop_z {
        return evaluation.finish(false, "blocked_falling_knife_drop");
    }
    if gap_velocity.unwrap_or(0.0) < 0.0
        && edge_adj < dynamic_threshold.max(config.recovery_edge_threshold)
    {
        return evaluation.finish(false, "blocked_waiting_recovery");
    }
    if let Some(reason) = adaptive.and_then(|evaluation| evaluation.block_reason) {
        return evaluation.finish(false, reason);
    }
    if min_final_q
        .map(|minimum| q_final < minimum)
        .unwrap_or(false)
    {
        return evaluation.finish(false, "blocked_low_final_q");
    }
    if edge_adj < dynamic_threshold {
        return evaluation.finish(false, "blocked_edge_below_threshold");
    }
    if adjusted_margin < min_adjusted_margin {
        return evaluation.finish(false, "blocked_thin_adjusted_margin");
    }
    if let Some(reason) = protection.block_reason {
        return evaluation.finish(false, reason);
    }

    evaluation.finish(true, "selected_edge_passed")
}

fn selected_book_mid_for_ptb_movement(
    book_quotes: PriceToBeatIvBookQuotes,
    selected_side: &str,
) -> Option<f64> {
    if selected_side == "up" {
        quote_mid_for_ptb_movement(book_quotes.up_bid, book_quotes.up_ask)
    } else {
        quote_mid_for_ptb_movement(book_quotes.down_bid, book_quotes.down_ask)
    }
}

fn quote_mid_for_ptb_movement(bid: Option<f64>, ask: Option<f64>) -> Option<f64> {
    let bid = bid.filter(|value| valid_probability(*value))?;
    let ask = ask.filter(|value| valid_probability(*value))?;
    (ask >= bid).then_some((bid + ask) / 2.0)
}

fn iv_mismatch_side(outcome_label: &str) -> Option<&'static str> {
    match outcome_label.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" | "long" | "bull" => Some("up"),
        "no" | "down" | "short" | "bear" => Some("down"),
        _ => None,
    }
}

fn iv_mismatch_seconds_left(market_slug: &str) -> Option<f64> {
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

fn edge_threshold_for_seconds_left(
    seconds_left: f64,
    config: &PriceToBeatIvMismatchEdgeConfig,
    selected_time_rule: Option<(usize, PriceToBeatIvMismatchTimeRule)>,
) -> Result<f64, &'static str> {
    if let Some((_, rule)) = selected_time_rule {
        return if rule.min_edge.is_finite() && rule.min_edge >= 0.0 {
            Ok(rule.min_edge)
        } else {
            Err("blocked_invalid_time_rule")
        };
    }
    if seconds_left <= config.no_new_trade_under_secs {
        Err("blocked_too_late")
    } else if seconds_left > config.no_new_trade_over_secs {
        Err("blocked_too_early")
    } else if seconds_left > 30.0 {
        Ok(config.edge_threshold_30_90_secs)
    } else if seconds_left > 15.0 {
        Ok(config.edge_threshold_15_30_secs)
    } else {
        Ok(config.edge_threshold_8_15_secs)
    }
}

fn select_time_rule(
    seconds_left: f64,
    config: &PriceToBeatIvMismatchEdgeConfig,
) -> Option<(usize, PriceToBeatIvMismatchTimeRule)> {
    config
        .time_rules
        .iter()
        .copied()
        .enumerate()
        .find(|(_, rule)| rule.matches_seconds_left(seconds_left))
}

#[cfg(test)]
static IV_MISMATCH_TEST_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

#[cfg(test)]
#[path = "iv_mismatch_edge_quality_tests.rs"]
mod quality_tests;

#[cfg(test)]
#[path = "iv_mismatch_edge_protection_tests.rs"]
mod protection_tests;

#[cfg(test)]
#[path = "iv_mismatch_edge_core_tests.rs"]
mod core_tests;

#[cfg(test)]
#[path = "iv_mismatch_edge_adaptive_tests.rs"]
mod adaptive_tests;
