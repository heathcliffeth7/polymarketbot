use super::iv_cex_open_gap::CexDecisionGapFallback;
use super::iv_mismatch_edge::PriceToBeatIvMismatchEdgeConfig;
use super::iv_mismatch_expected_move::PriceToBeatIvMinExpectedMoveMode;
use super::iv_mismatch_protection::PriceToBeatIvProtectionMode;

pub(crate) fn apply_action_place_order_iv_mismatch_risk_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.protection_mode = PriceToBeatIvProtectionMode::parse(
        crate::node_config_string(node, "priceToBeatIvProtectionMode").as_deref(),
    )
    .unwrap_or(PriceToBeatIvProtectionMode::Off);
    config.book_lead_guard_enabled =
        crate::node_config_bool(node, "priceToBeatIvBookLeadGuardEnabled").unwrap_or(false);
    if let Some(seconds) = crate::node_config_f64(node, "priceToBeatIvBookLeadUnderSec")
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        config.book_lead_under_sec = seconds;
    }
    if let Some(diff) = crate::node_config_f64(node, "priceToBeatIvBookLeadMinMidDiff")
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.book_lead_min_mid_diff = diff;
    }
    config.opposite_mid_block = crate::node_config_f64(node, "priceToBeatIvOppositeMidBlockCent")
        .map(|value| value / 100.0)
        .or_else(|| crate::node_config_f64(node, "priceToBeatIvOppositeMidBlock"))
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .or(config.opposite_mid_block);
    config.block_on_opposite_book_lead =
        crate::node_config_bool(node, "priceToBeatIvBlockOnOppositeBookLead").unwrap_or(true);
    apply_model_book_config(node, config);
    apply_depth_config(node, config);
    config.execution_vwap_guard.enabled =
        crate::node_config_bool(node, "priceToBeatIvExecutionVwapCostGuardEnabled")
            .unwrap_or(false);
    config.execution_vwap_guard.required_on_high_dislocation =
        crate::node_config_bool(node, "priceToBeatIvExecutionVwapRequiredOnHighDislocation")
            .unwrap_or(false);
    config.execution_vwap_guard.limit_by_vwap_enabled =
        crate::node_config_bool(node, "priceToBeatIvExecutionLimitByVwapEnabled").unwrap_or(false);
    if let Some(value) = cent_config(node, "priceToBeatIvExecutionVwapMaxSlippageCent")
        .filter(|value| *value >= 0.0 && *value <= 1.0)
    {
        config.execution_vwap_guard.max_slippage = value;
    }
    apply_cex_open_gap_config(node, config);
    apply_cex_magnitude_config(node, config);
    apply_gap_fail_cex_book_config(node, config);
    apply_oracle_lag_book_lead_config(node, config);
    config.price_band_guard =
        super::iv_price_band_guard::PriceToBeatIvPriceBandGuardConfig::from_node(node);
    apply_borderline_pump_book_lead_config(node, config);
    apply_pump_shock_config(node, config);
    apply_oracle_tick_jump_config(node, config);
    apply_ptb_chop_config(node, config);
    apply_medium_chop_margin_config(node, config);
    apply_high_price_early_reversal_config(node, config);
    apply_late_high_price_config(node, config);
    apply_participation_config(node, config);
    apply_expected_move_floor_config(node, config);
    apply_chainlink_stale_strong_gap_exception_config(node, config);
    apply_entry_quality_config(node, config);
    apply_token_crash_cooldown_config(node, config);
    config.require_binance_fresh_under_sec =
        crate::node_config_f64(node, "priceToBeatIvRequireBinanceFreshUnderSec")
            .filter(|value| value.is_finite() && *value >= 0.0);
    config.entry_quality_chainlink_max_age_ms =
        Some(super::entry_quality_policy::entry_quality_chainlink_max_age_ms(node));
    if let Some(stale_ms) =
        crate::node_config_i64(node, "priceToBeatIvChainlinkStaleMs").filter(|value| *value >= 0)
    {
        config.chainlink_stale_ms = stale_ms;
        config.chainlink_stale_override_source = "node_config";
    }
    if let Some(stale_ms) = crate::node_config_i64(node, "priceToBeatIvBinanceMaxStaleMs") {
        config.binance_stale_ms = stale_ms.max(0);
    }
    config.binance_hard_block_stale =
        crate::node_config_bool(node, "priceToBeatIvBinanceHardBlockStale").unwrap_or(false);
    config.require_binance_same_direction =
        crate::node_config_bool(node, "priceToBeatIvRequireBinanceSameDirection").unwrap_or(false);
    config.momentum_protection_enabled =
        crate::node_config_bool(node, "priceToBeatIvMomentumProtectionEnabled").unwrap_or(false);
    if let Some(drop_z) = crate::node_config_f64(node, "priceToBeatIvDropZBlockThreshold")
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        config.protection_drop_z_block_threshold = drop_z;
    }
    if let Some(penalty) =
        crate::node_config_f64(node, "priceToBeatIvProtectionSoftThresholdPenalty")
            .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.protection_soft_threshold_penalty = penalty;
    }
    if let Some(penalty) =
        crate::node_config_f64(node, "priceToBeatIvProtectionSoftGapStrengthPenalty")
            .filter(|value| value.is_finite() && *value >= 0.0)
    {
        config.protection_soft_gap_strength_penalty = penalty;
    }
}

fn apply_chainlink_stale_strong_gap_exception_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.chainlink_stale_strong_gap_exception.enabled =
        crate::node_config_bool(node, "priceToBeatIvChainlinkStaleStrongGapExceptionEnabled")
            .unwrap_or(false);
    if let Some(value) =
        non_negative_config(node, "priceToBeatIvChainlinkStaleStrongGapMinGapStrength")
    {
        config.chainlink_stale_strong_gap_exception.min_gap_strength = value;
    }
    if let Some(value) =
        crate::node_config_i64(node, "priceToBeatIvChainlinkStaleStrongGapMaxOracleAgeMs")
            .filter(|value| *value >= 0)
    {
        config
            .chainlink_stale_strong_gap_exception
            .max_oracle_age_ms = value;
    }
    config
        .chainlink_stale_strong_gap_exception
        .require_cex_confirmed = crate::node_config_bool(
        node,
        "priceToBeatIvChainlinkStaleStrongGapRequireCexConfirmed",
    )
    .unwrap_or(true);
    config
        .chainlink_stale_strong_gap_exception
        .require_bybit_hit =
        crate::node_config_bool(node, "priceToBeatIvChainlinkStaleStrongGapRequireBybitHit")
            .unwrap_or(true);
    config
        .chainlink_stale_strong_gap_exception
        .require_secondary_cex = crate::node_config_bool(
        node,
        "priceToBeatIvChainlinkStaleStrongGapRequireSecondaryCex",
    )
    .unwrap_or(true);
}

fn apply_high_price_early_reversal_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.high_price_early_reversal.enabled =
        crate::node_config_bool(node, "priceToBeatIvHighPriceEarlyReversalGuardEnabled")
            .unwrap_or(false);
    if let Some(value) = cent_config(node, "priceToBeatIvHighPriceEarlyRefCent")
        .filter(|value| *value >= 0.0 && *value <= 1.0)
    {
        config.high_price_early_reversal.ref_threshold = value;
    }
    if let Some(value) = crate::node_config_f64(node, "priceToBeatIvHighPriceEarlyRemainingSec")
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        config.high_price_early_reversal.remaining_sec = value;
    }
    if let Some(value) = crate::node_config_i64(node, "priceToBeatIvHighPriceEarlyMaxStaleMs")
        .filter(|value| *value >= 0)
    {
        config.high_price_early_reversal.max_stale_ms = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvHighPriceEarlyStaleGapAdd") {
        config.high_price_early_reversal.stale_gap_add = value;
    }
    if let Some(value) =
        non_negative_config(node, "priceToBeatIvHighPriceEarlyBinanceMissingGapAdd")
    {
        config.high_price_early_reversal.binance_missing_gap_add = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvHighPriceEarlyQExtremeCent")
        .filter(|value| *value >= 0.0 && *value <= 1.0)
    {
        config.high_price_early_reversal.q_extreme = value;
    }
    if let Some(value) =
        non_negative_config(node, "priceToBeatIvHighPriceEarlyQExtremeMinGapStrength")
    {
        config.high_price_early_reversal.q_extreme_min_gap_strength = value;
    }
    if let Some(value) =
        crate::node_config_i64(node, "priceToBeatIvHighPriceEarlyQExtremeMaxStaleMs")
            .filter(|value| *value >= 0)
    {
        config.high_price_early_reversal.q_extreme_max_stale_ms = value;
    }
    config.high_price_early_reversal.q_extreme_require_binance_q =
        crate::node_config_bool(node, "priceToBeatIvHighPriceEarlyQExtremeRequireBinanceQ")
            .unwrap_or(true);
    config
        .high_price_early_reversal
        .q_extreme_require_clean_strong_cex = crate::node_config_bool(
        node,
        "priceToBeatIvHighPriceEarlyQExtremeRequireCleanStrongCex",
    )
    .unwrap_or(true);
}

fn apply_ptb_chop_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.ptb_chop.enabled =
        crate::node_config_bool(node, "priceToBeatIvPtbChopGuardEnabled").unwrap_or(false);
    if let Some(value) = crate::node_config_i64(node, "priceToBeatIvPtbChopLookbackSeconds")
        .filter(|value| *value > 0)
    {
        config.ptb_chop.lookback_seconds = value;
    }
    if let Some(value) = crate::node_config_i64(node, "priceToBeatIvPtbChopExtendedLookbackSeconds")
        .filter(|value| *value > 0)
    {
        config.ptb_chop.extended_lookback_seconds = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvPtbChopDeadbandBps") {
        config.ptb_chop.deadband_bps = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvPtbChopDeadbandMinUsdBtc") {
        config.ptb_chop.deadband_min_usd_btc = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvPtbChopDeadbandMinUsdEth") {
        config.ptb_chop.deadband_min_usd_eth = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvPtbChopDeadbandMinUsdSol") {
        config.ptb_chop.deadband_min_usd_sol = value;
    }
    // CEX/chainlink sigma blend agirligi (0.0..=1.0). Drift removal ile birlikte sisirmeyi keser.
    if let Some(value) = unit_config(node, "priceToBeatIvCexSigmaBlendWeight") {
        config.cex_sigma_blend_weight = value;
    }
    // Asset-bazli required_gap_usd tavani (tail guard). None => cap devre disi.
    if let Some(value) = non_negative_config(node, "priceToBeatIvRequiredGapUsdMaxBtc") {
        config.required_gap_usd_max_btc = Some(value);
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvRequiredGapUsdMaxEth") {
        config.required_gap_usd_max_eth = Some(value);
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvRequiredGapUsdMaxSol") {
        config.required_gap_usd_max_sol = Some(value);
    }
    if let Some(value) = crate::node_config_i64(node, "priceToBeatIvPtbChopZeroCrossBlock10s")
        .filter(|value| *value >= 0)
    {
        config.ptb_chop.zero_cross_block_10s = value as usize;
    }
    if let Some(value) = crate::node_config_i64(node, "priceToBeatIvPtbChopZeroCrossBlock15s")
        .filter(|value| *value >= 0)
    {
        config.ptb_chop.zero_cross_block_15s = value as usize;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvPtbChopPathZWarn") {
        config.ptb_chop.path_z_warn = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvPtbChopPathZBlock") {
        config.ptb_chop.path_z_block = value;
    }
    if let Some(value) = unit_config(node, "priceToBeatIvPtbChopEfficiencyWarn") {
        config.ptb_chop.efficiency_warn = value;
    }
    if let Some(value) = unit_config(node, "priceToBeatIvPtbChopEfficiencyBlock") {
        config.ptb_chop.efficiency_block = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvPtbChopOppositeDepthZWarn") {
        config.ptb_chop.opposite_depth_z_warn = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvPtbChopOppositeDepthZBlock") {
        config.ptb_chop.opposite_depth_z_block = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvPtbChopMaxGapStrengthPenalty") {
        config.ptb_chop.max_gap_strength_penalty = value;
    }
}

fn apply_medium_chop_margin_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    if let Some(value) = crate::node_config_f64(node, "priceToBeatIvMediumChopMinAdjMargin")
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.medium_chop_margin.min_adj_margin = value;
    }
    if let Some(value) =
        crate::node_config_f64(node, "priceToBeatIvMediumChopHighPriceMinAdjMargin")
            .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.medium_chop_margin.high_price_min_adj_margin = value;
    }
    if let Some(value) = crate::node_config_f64(node, "priceToBeatIvMediumChopHighPriceRefCent")
        .map(|value| value / 100.0)
        .or_else(|| crate::node_config_f64(node, "priceToBeatIvMediumChopHighPriceRef"))
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.medium_chop_margin.high_price_ref = value;
    }
    if let Some(value) =
        crate::node_config_f64(node, "priceToBeatIvMediumChopBinanceFailOpenMarginAdd")
            .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.medium_chop_margin.binance_fail_open_margin_add = value;
    }
    if let Some(value) =
        crate::node_config_i64(node, "priceToBeatIvMediumChopStaleMs").filter(|value| *value >= 0)
    {
        config.medium_chop_margin.stale_ms = value;
    }
    if let Some(value) = crate::node_config_f64(node, "priceToBeatIvMediumChopStaleMarginAdd")
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.medium_chop_margin.stale_margin_add = value;
    }
}

fn apply_cex_open_gap_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.cex_open_gap.enabled =
        crate::node_config_bool(node, "priceToBeatIvCexOpenGapConsensusGuardEnabled")
            .or_else(|| crate::node_config_bool(node, "priceToBeatIvCexOpenGapGuardEnabled"))
            .unwrap_or(false);
    if let Some(value) = non_negative_config(node, "priceToBeatIvCexOpenGapMinUsd") {
        config.cex_open_gap.min_usd = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvCexOpenGapMinZ") {
        config.cex_open_gap.min_z = value;
    }
    if let Some(value) = crate::node_config_i64(node, "priceToBeatIvCexOpenGapMaxStaleMs")
        .filter(|value| *value >= 0)
    {
        config.cex_open_gap.max_stale_ms = value;
    }
    config.cex_open_gap.apply_negative_conservative_cap =
        crate::node_config_bool(node, "priceToBeatIvCexOpenGapApplyNegativeConservativeCap")
            .unwrap_or(false);
    config.cex_open_gap.lag_guard_enabled =
        crate::node_config_bool(node, "priceToBeatIvChainlinkCexLagGuardEnabled").unwrap_or(false);
    if let Some(value) = non_negative_config(node, "priceToBeatIvChainlinkCexDiffZBlock") {
        config.cex_open_gap.diff_z_block = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvChainlinkCexMaxDiffUsd") {
        config.cex_open_gap.max_diff_usd = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvChainlinkCexMaxDiffBps") {
        config.cex_open_gap.max_diff_bps = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvChainlinkCexBookMismatchDislocationCent") {
        config.cex_open_gap.book_mismatch_dislocation = value;
    }
    config.cex_open_gap.decision_gap_enabled =
        crate::node_config_bool(node, "priceToBeatIvCexDecisionGapEnabled").unwrap_or(true);
    config.cex_open_gap.decision_gap_fallback = CexDecisionGapFallback::parse(
        crate::node_config_string(node, "priceToBeatIvCexDecisionGapFallback").as_deref(),
    )
    .unwrap_or(CexDecisionGapFallback::Block);
    config.cex_open_gap.divergence_hard_block_enabled =
        crate::node_config_bool(node, "priceToBeatIvOracleCexDivergenceHardBlockEnabled")
            .unwrap_or(true);
    if let Some(value) = non_negative_config(node, "priceToBeatIvOracleCexDivergenceBlockZ") {
        config.cex_open_gap.divergence_block_z = value;
    }
    config.cex_open_gap.cex_lead_override_enabled =
        crate::node_config_bool(node, "priceToBeatIvCexLeadOverrideEnabled").unwrap_or(false);
}

fn apply_cex_magnitude_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.cex_magnitude.enabled =
        crate::node_config_bool(node, "priceToBeatIvCexMagnitudeGuardEnabled").unwrap_or(false);
    if let Some(value) = non_negative_config(node, "priceToBeatIvCexMagnitudeShallowRatio") {
        config.cex_magnitude.shallow_ratio = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvCexMagnitudeModerateRatio") {
        config.cex_magnitude.moderate_ratio = value;
    }
}

fn apply_gap_fail_cex_book_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.gap_fail_cex_book.enabled =
        crate::node_config_bool(node, "priceToBeatIvGapFailCexBookMismatchGuardEnabled")
            .unwrap_or(false);
    if let Some(value) = cent_config(node, "priceToBeatIvGapFailBookMaxExecutionRefCent")
        .filter(|value| *value >= 0.0 && *value <= 1.0)
    {
        config.gap_fail_cex_book.book_max_execution_ref = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvGapFailRawBookDislocationCent")
        .filter(|value| *value >= 0.0 && *value <= 1.0)
    {
        config.gap_fail_cex_book.raw_book_dislocation = value;
    }
    config.gap_fail_cex_book.mixed_cex_guard_enabled =
        crate::node_config_bool(node, "priceToBeatIvGapFailMixedCexGuardEnabled").unwrap_or(false);
    if let Some(value) = non_negative_config(node, "priceToBeatIvGapFailMixedCexMaxSeconds") {
        config.gap_fail_cex_book.mixed_cex_max_seconds = value;
    }
    config
        .gap_fail_cex_book
        .late_expensive_mixed_cex_guard_enabled =
        crate::node_config_bool(node, "priceToBeatIvLateExpensiveMixedCexGuardEnabled")
            .unwrap_or(false);
    if let Some(value) = non_negative_config(node, "priceToBeatIvLateExpensiveMixedCexSeconds") {
        config.gap_fail_cex_book.late_expensive_mixed_cex_seconds = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvLateExpensiveMixedCexMinVwapCent")
        .filter(|value| *value >= 0.0 && *value <= 1.0)
    {
        config.gap_fail_cex_book.late_expensive_mixed_cex_min_vwap = value;
    }
    config
        .gap_fail_cex_book
        .late_expensive_mixed_cex_require_gap_fail_or_lag_high = crate::node_config_bool(
        node,
        "priceToBeatIvLateExpensiveMixedCexRequireGapFailOrLagHigh",
    )
    .unwrap_or(true);
    config
        .gap_fail_cex_book
        .chainlink_cex_lag_no_book_guard_enabled =
        crate::node_config_bool(node, "priceToBeatIvChainlinkCexLagNoBookGuardEnabled")
            .unwrap_or(false);
    if let Some(value) = non_negative_config(node, "priceToBeatIvChainlinkCexLagNoBookMaxSeconds") {
        config
            .gap_fail_cex_book
            .chainlink_cex_lag_no_book_max_seconds = value;
    }
    config
        .gap_fail_cex_book
        .chainlink_cex_lag_no_book_require_non_strong_cex = crate::node_config_bool(
        node,
        "priceToBeatIvChainlinkCexLagNoBookRequireNonStrongCex",
    )
    .unwrap_or(true);
}

fn apply_oracle_lag_book_lead_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.oracle_lag_book_lead.enabled =
        crate::node_config_bool(node, "priceToBeatIvOracleLagBookLeadGuardEnabled")
            .unwrap_or(false);
    if let Some(value) = non_negative_config(node, "priceToBeatIvOracleLagEarlySeconds") {
        config.oracle_lag_book_lead.early_seconds = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvOracleLagQHighCent") {
        config.oracle_lag_book_lead.q_high = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvOracleLagCheapTokenCent") {
        config.oracle_lag_book_lead.cheap_token = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvOracleLagQExtremeCent") {
        config.oracle_lag_book_lead.q_extreme = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvOracleLagCheapTokenExtremeCent") {
        config.oracle_lag_book_lead.cheap_token_extreme = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvOracleLagConsensusMismatchQCent") {
        config.oracle_lag_book_lead.q_consensus_mismatch = value;
    }
    if let Some(value) = cent_config(
        node,
        "priceToBeatIvOracleLagConsensusMismatchCheapTokenCent",
    ) {
        config.oracle_lag_book_lead.cheap_token_consensus_mismatch = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvModelBookDislocationWarnCent") {
        config.oracle_lag_book_lead.dislocation_warn = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvModelBookDislocationHighCent") {
        config.oracle_lag_book_lead.dislocation_high = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvModelBookDislocationRedCent") {
        config.oracle_lag_book_lead.dislocation_red = value;
    }
    if let Some(value) = cent_config(
        node,
        "priceToBeatIvOracleLagConsensusMismatchDislocationCent",
    ) {
        config.oracle_lag_book_lead.dislocation_consensus_mismatch = value;
    }
    if let Some(value) = crate::node_config_i64(node, "priceToBeatIvOracleLagMaxBookAgeMs")
        .filter(|value| *value >= 0)
    {
        config.oracle_lag_book_lead.max_book_age_ms = value;
    }
    if let Some(value) = unit_config(node, "priceToBeatIvOracleLagMinDepthCoverage") {
        config.oracle_lag_book_lead.min_depth_coverage = value;
    }
    config.oracle_lag_book_lead.use_best_ask_fallback =
        crate::node_config_bool(node, "priceToBeatIvOracleLagUseBestAskFallback").unwrap_or(false);
    if let Some(value) = cent_config(node, "priceToBeatIvOracleLagBestAskFallbackMaxSpreadCent") {
        config.oracle_lag_book_lead.best_ask_fallback_max_spread = value;
    }
}

fn apply_token_crash_cooldown_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.token_crash_cooldown.enabled =
        crate::node_config_bool(node, "priceToBeatIvTokenCrashCooldownEnabled").unwrap_or(false);
    if let Some(value) = crate::node_config_f64(node, "priceToBeatIvTokenCrashCooldownHighBidCent")
        .filter(|v| v.is_finite() && *v >= 0.0 && *v <= 100.0)
    {
        config.token_crash_cooldown.high_bid_cent = value;
    }
    if let Some(value) = crate::node_config_f64(node, "priceToBeatIvTokenCrashCooldownLowBidCent")
        .filter(|v| v.is_finite() && *v >= 0.0 && *v <= 100.0)
    {
        config.token_crash_cooldown.low_bid_cent = value;
    }
    if let Some(value) = crate::node_config_f64(node, "priceToBeatIvTokenCrashCooldownWindowSec")
        .filter(|v| v.is_finite() && *v >= 0.0)
    {
        config.token_crash_cooldown.window_sec = value;
    }
    if let Some(value) = crate::node_config_f64(node, "priceToBeatIvTokenCrashCooldownLookbackSec")
        .filter(|v| v.is_finite() && *v >= 0.0)
    {
        config.token_crash_cooldown.lookback_sec = value;
    }
}

fn apply_borderline_pump_book_lead_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.borderline_pump_book_lead.enabled =
        crate::node_config_bool(node, "priceToBeatIvBorderlinePumpBookLeadGuardEnabled")
            .unwrap_or(false);
    if let Some(value) = non_negative_config(node, "priceToBeatIvBorderlineGapMarginEarly") {
        config.borderline_pump_book_lead.gap_margin_early = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvBorderlinePumpShockRatio") {
        config.borderline_pump_book_lead.pump_shock_ratio = value;
    }
    if let Some(value) = cent_value_config(node, "priceToBeatIvBorderlineBookLeadQMinCent") {
        config.borderline_pump_book_lead.q_min_cent = value;
    }
    if let Some(value) = cent_value_config(node, "priceToBeatIvBorderlineBookLeadCheapTokenCent") {
        config.borderline_pump_book_lead.cheap_token_cent = value;
    }
    if let Some(value) = cent_value_config(node, "priceToBeatIvBorderlineBookLeadDislocationCent") {
        config.borderline_pump_book_lead.dislocation_cent = value;
    }
}

fn apply_pump_shock_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.pump_shock.enabled =
        crate::node_config_bool(node, "priceToBeatIvPumpShockGuardEnabled").unwrap_or(false);
    if let Some(value) = non_negative_config(node, "priceToBeatIvPumpShockGapGrowthRatio") {
        config.pump_shock.gap_growth_ratio = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvPumpShockHardRatio") {
        config.pump_shock.hard_ratio = value;
    }
    if let Some(value) =
        crate::node_config_i64(node, "priceToBeatIvPumpShockMinHoldMs").filter(|value| *value >= 0)
    {
        config.pump_shock.min_hold_ms = value;
    }
    if let Some(value) = unit_config(node, "priceToBeatIvPumpShockMinBufferRetain") {
        config.pump_shock.min_buffer_retain = value;
    }
}

fn apply_expected_move_floor_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.expected_move_floor.mode = PriceToBeatIvMinExpectedMoveMode::parse(
        crate::node_config_string(node, "priceToBeatIvMinExpectedMoveMode").as_deref(),
    )
    .unwrap_or(config.expected_move_floor.mode);
    if let Some(value) = non_negative_config(node, "priceToBeatIvAdaptiveMinExpectedMoveBaseBps") {
        config.expected_move_floor.base_bps = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvAdaptiveMinExpectedMoveMinBps") {
        config.expected_move_floor.min_bps = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvAdaptiveMinExpectedMoveMaxBps") {
        config.expected_move_floor.max_bps = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvAdaptiveDisagreementBpsAdd") {
        config.expected_move_floor.disagreement_add_bps = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvAdaptiveStrongDisagreementBpsAdd")
    {
        config.expected_move_floor.strong_disagreement_add_bps = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvAdaptiveSpreadBpsAdd") {
        config.expected_move_floor.spread_add_bps = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvAdaptiveWideSpreadBpsAdd") {
        config.expected_move_floor.wide_spread_add_bps = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvAdaptiveStaleBpsAdd") {
        config.expected_move_floor.stale_add_bps = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvAdaptiveNoiseBpsAdd") {
        config.expected_move_floor.noise_add_bps = value;
    }
}

fn apply_entry_quality_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.entry_quality.enabled =
        crate::node_config_bool(node, "priceToBeatIvEntryQualityPolicy").unwrap_or(false);
    if let Some(value) = cent_config(node, "priceToBeatIvNormalMaxPriceCent")
        .filter(|value| *value >= 0.0 && *value <= 1.0)
    {
        config.entry_quality.normal_max_price = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvPremiumMaxPriceCent")
        .filter(|value| *value >= 0.0 && *value <= 1.0)
    {
        config.entry_quality.premium_max_price = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvNoNewEntryBelowSeconds") {
        config.entry_quality.no_new_entry_below_seconds = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvMinExpectedMoveBps") {
        config.entry_quality.min_expected_move_bps = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvMinExpectedMoveUsd") {
        config.entry_quality.min_expected_move_usd = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvGapStrengthMin60To45") {
        config.entry_quality.gap_strength_min_60_to_45 = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvGapStrengthMin45To25") {
        config.entry_quality.gap_strength_min_45_to_25 = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvGapStrengthMin25To10") {
        config.entry_quality.gap_strength_min_25_to_10 = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvGapStrengthMin10To8") {
        config.entry_quality.gap_strength_min_10_to_8 = value;
    }
    config.entry_quality.buffer_trend_guard_enabled =
        crate::node_config_bool(node, "priceToBeatIvBufferTrendGuardEnabled").unwrap_or(true);
    if let Some(value) = unit_config(node, "priceToBeatIvBufferRetain5s") {
        config.entry_quality.buffer_retain_5s = value;
    }
    if let Some(value) = unit_config(node, "priceToBeatIvBufferRetain10s") {
        config.entry_quality.buffer_retain_10s = value;
    }
    if let Some(value) = unit_config(node, "priceToBeatIvPremiumBufferRetain5s") {
        config.entry_quality.premium_buffer_retain_5s = value;
    }
    if let Some(value) = unit_config(node, "priceToBeatIvPremiumBufferRetain10s") {
        config.entry_quality.premium_buffer_retain_10s = value;
    }
    config.entry_quality.spike_fade_guard_enabled =
        crate::node_config_bool(node, "priceToBeatIvSpikeFadeGuardEnabled").unwrap_or(true);
    if let Some(value) = crate::node_config_f64(node, "priceToBeatIvSpikeMultiplier")
        .filter(|value| value.is_finite() && *value > 1.0)
    {
        config.entry_quality.spike_multiplier = value;
    }
    if let Some(value) = unit_config(node, "priceToBeatIvSpikeRetraceRatio") {
        config.entry_quality.spike_retrace_ratio = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvPremiumMaxSpreadCent")
        .filter(|value| *value >= 0.0 && *value <= 1.0)
    {
        config.entry_quality.premium_max_spread = value;
    }
    if let Some(value) = crate::node_config_i64(node, "priceToBeatIvPremiumMaxChainlinkAgeMs")
        .filter(|value| *value > 0)
    {
        config.entry_quality.premium_max_chainlink_age_ms = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvCexAlignMaxUsd") {
        config.entry_quality.cex_align_max_usd = Some(value);
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvCexAlignMaxBps") {
        config.entry_quality.cex_align_max_bps = Some(value);
    }
    config.entry_quality.eq77_risk_cap_enabled =
        crate::node_config_bool(node, "priceToBeatIvEq77RiskCapEnabled").unwrap_or(false);
    if let Some(value) = non_negative_config(node, "priceToBeatIvRiskScoreCleanMax") {
        config.entry_quality.risk_score_clean_max = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvRiskScoreModerateMax") {
        config.entry_quality.risk_score_moderate_max = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvRiskScoreHighMax") {
        config.entry_quality.risk_score_high_max = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvModerateRiskMaxPriceCent") {
        config.entry_quality.moderate_risk_max_price = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvHighRiskMaxPriceCent") {
        config.entry_quality.high_risk_max_price = value;
    }
    if let Some(value) = cent_config(node, "priceToBeatIvDeepValueMaxPriceCent") {
        config.entry_quality.deep_value_max_price = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvMaxRiskHaircutCent") {
        config.entry_quality.max_risk_haircut_cent = value;
    }
    config.entry_quality.wait_for_price_enabled =
        crate::node_config_bool(node, "priceToBeatIvWaitForPriceEnabled").unwrap_or(true);
    config.entry_quality.recheck_before_submit =
        crate::node_config_bool(node, "priceToBeatIvRecheckBeforeSubmit").unwrap_or(true);
    if let Some(value) = cent_config(node, "priceToBeatIvOddsMaxSpreadCent") {
        config.entry_quality.odds_max_spread = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvCexUnconfirmedRiskPoints") {
        config.entry_quality.cex_unconfirmed_risk_points = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvCexConflictRiskPoints") {
        config.entry_quality.cex_conflict_risk_points = value;
    }
    config.entry_quality.passive_bid_enabled =
        crate::node_config_bool(node, "priceToBeatIvPassiveBidEnabled").unwrap_or(false);
    if let Some(value) =
        crate::node_config_i64(node, "priceToBeatIvPassiveBidTtlMs").filter(|value| *value > 0)
    {
        config.entry_quality.passive_bid_ttl_ms = value;
    }
}

fn cent_config(node: &crate::TradeFlowNode, key: &str) -> Option<f64> {
    crate::node_config_f64(node, key)
        .map(|value| value / 100.0)
        .filter(|value| value.is_finite())
}

fn cent_value_config(node: &crate::TradeFlowNode, key: &str) -> Option<f64> {
    crate::node_config_f64(node, key)
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 100.0)
}

fn non_negative_config(node: &crate::TradeFlowNode, key: &str) -> Option<f64> {
    crate::node_config_f64(node, key).filter(|value| value.is_finite() && *value >= 0.0)
}

fn unit_config(node: &crate::TradeFlowNode, key: &str) -> Option<f64> {
    crate::node_config_f64(node, key)
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
}

fn apply_model_book_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.model_book_gap_warn = crate::node_config_f64(node, "priceToBeatIvModelBookGapWarn")
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
        .or(config.model_book_gap_warn);
    config.too_good_to_be_true_gap = crate::node_config_f64(node, "priceToBeatIvModelBookGapHard")
        .or_else(|| crate::node_config_f64(node, "priceToBeatIvTooGoodToBeTrueGap"))
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
        .or(config.too_good_to_be_true_gap);
    if let Some(penalty) =
        crate::node_config_f64(node, "priceToBeatIvModelBookWarnThresholdPenalty")
            .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.model_book_warn_threshold_penalty = penalty;
    }
    if let Some(penalty) = crate::node_config_f64(node, "priceToBeatIvModelBookWarnGapPenalty")
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        config.model_book_warn_gap_strength_penalty = penalty;
    }
}

fn apply_depth_config(node: &crate::TradeFlowNode, config: &mut PriceToBeatIvMismatchEdgeConfig) {
    config.depth_guard_enabled =
        crate::node_config_bool(node, "priceToBeatIvDepthGuardEnabled").unwrap_or(true);
    config.depth_guard_hard_block_enabled =
        crate::node_config_bool(node, "priceToBeatIvDepthGuardHardBlockEnabled").unwrap_or(false);
    config.reject_no_opp_depth =
        crate::node_config_bool(node, "priceToBeatIvRejectNoOppDepth").unwrap_or(false);
    if let Some(slippage) = crate::node_config_f64(node, "priceToBeatIvDepthMaxSlippage")
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.depth_max_slippage = slippage;
    }
}

fn apply_late_high_price_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    if let Some(seconds) = crate::node_config_f64(node, "priceToBeatIvLateHighPriceSoftUnderSec")
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        config.late_high_price_soft_under_sec = seconds;
    }
    if let Some(price) = crate::node_config_f64(node, "priceToBeatIvLateHighPriceAskCent")
        .map(|value| value / 100.0)
        .or_else(|| crate::node_config_f64(node, "priceToBeatIvLateHighPriceAsk"))
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.late_high_price_ask = price;
    }
    if let Some(mid) = crate::node_config_f64(node, "priceToBeatIvLateHighPriceSelectedMidSoftCent")
        .map(|value| value / 100.0)
        .or_else(|| crate::node_config_f64(node, "priceToBeatIvLateHighPriceSelectedMidSoft"))
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.late_high_price_selected_mid_soft = mid;
    }
    if let Some(penalty) =
        crate::node_config_f64(node, "priceToBeatIvLateHighPriceThresholdPenalty")
            .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.late_high_price_threshold_penalty = penalty;
    }
    if let Some(mid) = crate::node_config_f64(node, "priceToBeatIvLateHighPriceSelectedMidHardCent")
        .map(|value| value / 100.0)
        .or_else(|| crate::node_config_f64(node, "priceToBeatIvLateHighPriceSelectedMidHard"))
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.late_high_price_selected_mid_hard = mid;
    }
    if let Some(gap) = crate::node_config_f64(node, "priceToBeatIvLateHighPriceMinGapUsd")
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        config.late_high_price_min_gap_usd = gap;
    }
}

fn apply_participation_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.participation_credit_enabled =
        crate::node_config_bool(node, "priceToBeatIvParticipationCreditEnabled").unwrap_or(true);
    if let Some(minutes) = crate::node_config_f64(node, "priceToBeatIvParticipationAfterMinutes")
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        config.participation_after_minutes = minutes;
    }
    if let Some(minutes) =
        crate::node_config_f64(node, "priceToBeatIvParticipationLongAfterMinutes")
            .filter(|value| value.is_finite() && *value >= 0.0)
    {
        config.participation_long_after_minutes = minutes;
    }
    if let Some(credit) = crate::node_config_f64(node, "priceToBeatIvParticipationCredit")
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.participation_credit = credit;
    }
    if let Some(credit) = crate::node_config_f64(node, "priceToBeatIvParticipationLongCredit")
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.participation_long_credit = credit;
    }
    if let Some(floor) = crate::node_config_f64(node, "priceToBeatIvParticipationMinThreshold")
        .filter(|value| value.is_finite() && *value >= 0.0 && *value <= 1.0)
    {
        config.participation_min_threshold = floor;
    }
}

fn apply_oracle_tick_jump_config(
    node: &crate::TradeFlowNode,
    config: &mut PriceToBeatIvMismatchEdgeConfig,
) {
    config.oracle_tick_jump.enabled =
        crate::node_config_bool(node, "priceToBeatIvOracleTickJumpCooldownEnabled").unwrap_or(true);
    if let Some(value) = non_negative_config(node, "priceToBeatIvOracleTickJumpRatio") {
        config.oracle_tick_jump.jump_ratio = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvOracleTickJumpEmMult") {
        config.oracle_tick_jump.jump_em_mult = value;
    }
    if let Some(value) = non_negative_config(node, "priceToBeatIvOracleTickJumpCexConfirmRatio") {
        config.oracle_tick_jump.cex_confirm_ratio = value;
    }
    if let Some(value) =
        crate::node_config_i64(node, "priceToBeatIvOracleTickJumpCooldownMs").filter(|v| *v >= 0)
    {
        config.oracle_tick_jump.cooldown_ms = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade_flow::guards::price_to_beat::PriceToBeatSignalFormulaMarketInput;
    use serde_json::json;

    fn test_node(config: serde_json::Value) -> crate::TradeFlowNode {
        crate::TradeFlowNode {
            key: "action_1".to_string(),
            node_type: "action.place_order".to_string(),
            config,
        }
    }

    fn base_config() -> PriceToBeatIvMismatchEdgeConfig {
        PriceToBeatIvMismatchEdgeConfig::crypto_defaults(PriceToBeatSignalFormulaMarketInput {
            best_bid: Some(0.42),
            best_ask: Some(0.44),
        })
    }

    #[test]
    fn chainlink_stale_override_defaults_to_3000ms() {
        let node = test_node(json!({}));
        let mut config = base_config();

        apply_action_place_order_iv_mismatch_risk_config(&node, &mut config);

        assert_eq!(config.chainlink_stale_ms, 3_000);
        assert_eq!(config.chainlink_stale_override_source, "default");
        assert_eq!(config.entry_quality_chainlink_max_age_ms, Some(3_000));
    }

    #[test]
    fn chainlink_stale_override_reads_node_threshold_pair() {
        let node = test_node(json!({
            "priceToBeatIvChainlinkStaleMs": 3_500,
            "priceToBeatIvEntryQualityChainlinkMaxAgeMs": 3_500,
        }));
        let mut config = base_config();

        apply_action_place_order_iv_mismatch_risk_config(&node, &mut config);

        assert_eq!(config.chainlink_stale_ms, 3_500);
        assert_eq!(config.chainlink_stale_override_source, "node_config");
        assert_eq!(config.entry_quality_chainlink_max_age_ms, Some(3_500));
    }
}
