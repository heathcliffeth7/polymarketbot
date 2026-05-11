use super::iv_mismatch_edge::PriceToBeatIvMismatchEdgeConfig;
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
    apply_late_high_price_config(node, config);
    apply_participation_config(node, config);
    config.require_binance_fresh_under_sec =
        crate::node_config_f64(node, "priceToBeatIvRequireBinanceFreshUnderSec")
            .filter(|value| value.is_finite() && *value >= 0.0);
    if let Some(stale_ms) = crate::node_config_i64(node, "priceToBeatIvBinanceMaxStaleMs") {
        config.binance_stale_ms = stale_ms.max(0);
    }
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
