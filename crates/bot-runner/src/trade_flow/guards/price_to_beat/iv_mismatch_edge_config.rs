use super::iv_borderline_pump_book_lead::PriceToBeatIvBorderlinePumpBookLeadConfig;
use super::iv_cex_open_gap::PriceToBeatIvCexOpenGapConfig;
use super::iv_chainlink_stale_strong_gap_exception::{
    ChainlinkStaleStrongGapExceptionConfig, ChainlinkStaleStrongGapRuntimeContext,
};
use super::iv_depth_diagnostics::PriceToBeatIvDepthRuntimeDiagnostics;
use super::iv_entry_quality::IvEntryQualityConfig;
use super::iv_execution_vwap::PriceToBeatIvExecutionVwapConfig;
use super::iv_gap_fail_cex_book_guard::PriceToBeatIvGapFailCexBookGuardConfig;
use super::iv_high_price_early_reversal::PriceToBeatIvHighPriceEarlyReversalConfig;
use super::iv_mismatch_adaptive::{PriceToBeatIvAdaptiveConfig, PriceToBeatIvAdaptiveVolumeInput};
use super::iv_mismatch_edge::iv_cex_magnitude_guard::PriceToBeatIvCexMagnitudeConfig;
use super::iv_mismatch_edge::iv_mismatch_medium_chop_margin::PriceToBeatIvMediumChopMarginConfig;
use super::iv_mismatch_expected_move::PriceToBeatIvExpectedMoveFloorConfig;
use super::iv_mismatch_protection::{PriceToBeatIvBookQuotes, PriceToBeatIvProtectionMode};
use super::iv_mismatch_ptb_chop::PriceToBeatIvPtbChopConfig;
use super::iv_mismatch_time_rule::PriceToBeatIvMismatchTimeRule;
use super::iv_oracle_lag_book_lead::PriceToBeatIvOracleLagBookLeadConfig;
use super::iv_oracle_tick_jump::PriceToBeatIvOracleTickJumpConfig;
use super::iv_price_band_guard::PriceToBeatIvPriceBandGuardConfig;
use super::iv_pump_shock::PriceToBeatIvPumpShockConfig;
use super::iv_token_crash_cooldown::PriceToBeatIvTokenCrashCooldownConfig;
use super::PriceToBeatSignalFormulaMarketInput;
use bot_infra::exchange::OrderBookSnapshot;

pub(crate) const DEFAULT_VOL_WINDOW_SECS: i64 = 45;
pub(crate) const DEFAULT_MIN_VOL_SAMPLES: usize = 8;
pub(crate) const DEFAULT_CHAINLINK_STALE_MS: i64 = 3_000;
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
pub(crate) const DEFAULT_FAST_VOL_WINDOW_SECS: i64 = 15;
const DEFAULT_FAST_VOL_MULTIPLIER: f64 = 1.25;
// CEX sigma'nin blended sigma_eff icindeki agirligi. 0.0 => saf chainlink,
// 1.0 => saf cex_median (eski floor davranisi). 0.4 onerilir: SOL gibi trend'li
// CEX serilerinde sisirmeyi keser ama cex sinyalini de gozardi etmez.
const DEFAULT_CEX_SIGMA_BLEND_WEIGHT: f64 = 0.4;
const DEFAULT_LATENCY_BUFFER_SECS: f64 = 0.75;
const DEFAULT_HIGH_PRICE_PENALTY_THRESHOLD: f64 = 0.75;
const DEFAULT_HIGH_PRICE_PENALTY: f64 = 0.02;
const DEFAULT_STALE_PENALTY_MS: i64 = 1_500;
const DEFAULT_STALE_PENALTY: f64 = 0.02;
const DEFAULT_DROP_PENALTY_START_Z: f64 = 0.5;
const DEFAULT_DROP_PENALTY_PER_Z: f64 = 0.04;
const DEFAULT_FALLING_KNIFE_DROP_Z: f64 = 1.0;
const DEFAULT_RISING_KNIFE_DROP_Z: f64 = 2.5;
const DEFAULT_RISING_DROP_PENALTY_START_Z: f64 = 1.5;
const DEFAULT_RECOVERY_EDGE_THRESHOLD: f64 = 0.12;
const DEFAULT_BINANCE_STALE_MS: i64 = 2_500;
pub(crate) const DEFAULT_BINANCE_Q_BUFFER: f64 = 0.03;
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
    pub(crate) chainlink_stale_override_source: &'static str,
    pub(crate) entry_quality_chainlink_max_age_ms: Option<i64>,
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
    // CEX/chainlink sigma blend agirligi (0.0..=1.0). Drift removal (venue_mid_sigma)
    // ile birlikte sisirmeyi keser. 0.4 => w_chainlink=0.6, w_cex=0.4.
    pub(crate) cex_sigma_blend_weight: f64,
    // Asset-bazli required_gap_usd tavani (tail guard). None => cap devre disi.
    // SOL gibi sigma patlamasi yasanan assetlerde sisik gap'i sinirlar.
    pub(crate) required_gap_usd_max_btc: Option<f64>,
    pub(crate) required_gap_usd_max_eth: Option<f64>,
    pub(crate) required_gap_usd_max_sol: Option<f64>,
    pub(crate) latency_buffer_secs: f64,
    pub(crate) high_price_penalty_threshold: f64,
    pub(crate) high_price_penalty: f64,
    pub(crate) stale_penalty_ms: i64,
    pub(crate) stale_penalty: f64,
    pub(crate) drop_penalty_start_z: f64,
    pub(crate) drop_penalty_per_z: f64,
    pub(crate) falling_knife_drop_z: f64,
    pub(crate) rising_knife_drop_z: f64,
    pub(crate) rising_drop_penalty_start_z: f64,
    pub(crate) recovery_edge_threshold: f64,
    pub(crate) binance_stale_ms: i64,
    pub(crate) binance_hard_block_stale: bool,
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
    pub(crate) reject_no_opp_depth: bool,
    pub(crate) depth_order_book: Option<OrderBookSnapshot>,
    pub(crate) depth_intended_qty: Option<f64>,
    pub(crate) depth_runtime_diagnostics: PriceToBeatIvDepthRuntimeDiagnostics,
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
    pub(crate) cex_magnitude: PriceToBeatIvCexMagnitudeConfig,
    pub(crate) execution_vwap_guard: PriceToBeatIvExecutionVwapConfig,
    pub(crate) gap_fail_cex_book: PriceToBeatIvGapFailCexBookGuardConfig,
    pub(crate) oracle_lag_book_lead: PriceToBeatIvOracleLagBookLeadConfig,
    pub(crate) price_band_guard: PriceToBeatIvPriceBandGuardConfig,
    pub(crate) borderline_pump_book_lead: PriceToBeatIvBorderlinePumpBookLeadConfig,
    pub(crate) oracle_tick_jump: PriceToBeatIvOracleTickJumpConfig,
    pub(crate) pump_shock: PriceToBeatIvPumpShockConfig,
    pub(crate) expected_move_floor: PriceToBeatIvExpectedMoveFloorConfig,
    pub(crate) ptb_chop: PriceToBeatIvPtbChopConfig,
    pub(crate) medium_chop_margin: PriceToBeatIvMediumChopMarginConfig,
    pub(crate) high_price_early_reversal: PriceToBeatIvHighPriceEarlyReversalConfig,
    pub(crate) adaptive: PriceToBeatIvAdaptiveConfig,
    pub(crate) adaptive_volume: Option<PriceToBeatIvAdaptiveVolumeInput>,
    pub(crate) chainlink_stale_strong_gap_exception: ChainlinkStaleStrongGapExceptionConfig,
    pub(crate) chainlink_stale_strong_gap_context: Option<ChainlinkStaleStrongGapRuntimeContext>,
    pub(crate) token_crash_cooldown: PriceToBeatIvTokenCrashCooldownConfig,
}

impl PriceToBeatIvMismatchEdgeConfig {
    pub(crate) fn crypto_defaults(market: PriceToBeatSignalFormulaMarketInput) -> Self {
        Self {
            market,
            vol_window_secs: DEFAULT_VOL_WINDOW_SECS,
            min_vol_samples: DEFAULT_MIN_VOL_SAMPLES,
            chainlink_stale_ms: DEFAULT_CHAINLINK_STALE_MS,
            chainlink_stale_override_source: "default",
            entry_quality_chainlink_max_age_ms: Some(
                super::entry_quality_policy::DEFAULT_ENTRY_QUALITY_CHAINLINK_MAX_AGE_MS,
            ),
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
            cex_sigma_blend_weight: DEFAULT_CEX_SIGMA_BLEND_WEIGHT,
            required_gap_usd_max_btc: None,
            required_gap_usd_max_eth: None,
            required_gap_usd_max_sol: None,
            latency_buffer_secs: DEFAULT_LATENCY_BUFFER_SECS,
            high_price_penalty_threshold: DEFAULT_HIGH_PRICE_PENALTY_THRESHOLD,
            high_price_penalty: DEFAULT_HIGH_PRICE_PENALTY,
            stale_penalty_ms: DEFAULT_STALE_PENALTY_MS,
            stale_penalty: DEFAULT_STALE_PENALTY,
            drop_penalty_start_z: DEFAULT_DROP_PENALTY_START_Z,
            drop_penalty_per_z: DEFAULT_DROP_PENALTY_PER_Z,
            falling_knife_drop_z: DEFAULT_FALLING_KNIFE_DROP_Z,
            rising_knife_drop_z: DEFAULT_RISING_KNIFE_DROP_Z,
            rising_drop_penalty_start_z: DEFAULT_RISING_DROP_PENALTY_START_Z,
            recovery_edge_threshold: DEFAULT_RECOVERY_EDGE_THRESHOLD,
            binance_stale_ms: DEFAULT_BINANCE_STALE_MS,
            binance_hard_block_stale: false,
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
            reject_no_opp_depth: false,
            depth_order_book: None,
            depth_intended_qty: None,
            depth_runtime_diagnostics: PriceToBeatIvDepthRuntimeDiagnostics::default(),
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
            cex_magnitude: PriceToBeatIvCexMagnitudeConfig::default(),
            execution_vwap_guard: PriceToBeatIvExecutionVwapConfig::default(),
            gap_fail_cex_book: PriceToBeatIvGapFailCexBookGuardConfig::default(),
            oracle_lag_book_lead: PriceToBeatIvOracleLagBookLeadConfig::default(),
            price_band_guard: PriceToBeatIvPriceBandGuardConfig::default(),
            borderline_pump_book_lead: PriceToBeatIvBorderlinePumpBookLeadConfig::default(),
            oracle_tick_jump: PriceToBeatIvOracleTickJumpConfig::default(),
            pump_shock: PriceToBeatIvPumpShockConfig::default(),
            expected_move_floor: PriceToBeatIvExpectedMoveFloorConfig::default(),
            ptb_chop: PriceToBeatIvPtbChopConfig::default(),
            medium_chop_margin: PriceToBeatIvMediumChopMarginConfig::default(),
            high_price_early_reversal: PriceToBeatIvHighPriceEarlyReversalConfig::default(),
            adaptive: PriceToBeatIvAdaptiveConfig::default(),
            adaptive_volume: None,
            chainlink_stale_strong_gap_exception: ChainlinkStaleStrongGapExceptionConfig::default(),
            chainlink_stale_strong_gap_context: None,
            token_crash_cooldown: PriceToBeatIvTokenCrashCooldownConfig::default(),
        }
    }
}
