import type { TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import type { PtbMode } from '@/lib/trade-flow-config-mappers/ptb-modes';
import { isRecord, toFiniteNumber } from './shared';
import { pushNodeError } from './validation-core';

interface ParsedIvTimeRule {
  startRemainingSec: number;
  endRemainingSec: number;
}

export function validateActionPlaceOrderPtbIvTimeRulesConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>,
  priceToBeatGuardEnabled: boolean | null,
  normalizedPriceToBeatMode: PtbMode | null
) {
  const hasIvConfig =
    config.priceToBeatIvTimeRules != null ||
    config.priceToBeatIvStalePenaltyMs != null ||
    config.priceToBeatIvStaleGapStrengthPenalty != null ||
    config.priceToBeatIvNegativeVelocityGapStrengthPenalty != null ||
    config.priceToBeatIvBinanceMissingAskThresholdCent != null ||
    config.priceToBeatIvBinanceMissingPenalty != null ||
    config.priceToBeatIvMinAdjustedMargin != null ||
    config.priceToBeatIvMinFinalQ != null ||
    config.priceToBeatIvBinanceDisagreementThreshold != null ||
    config.priceToBeatIvBinanceDisagreementPenalty != null ||
    config.priceToBeatIvLargeBinanceDisagreementThreshold != null ||
    config.priceToBeatIvLargeBinanceDisagreementPenalty != null ||
    config.priceToBeatIvProtectionMode != null ||
    config.priceToBeatIvBookLeadGuardEnabled != null ||
    config.priceToBeatIvBookLeadUnderSec != null ||
    config.priceToBeatIvBookLeadMinMidDiff != null ||
    config.priceToBeatIvOppositeMidBlockCent != null ||
    config.priceToBeatIvBlockOnOppositeBookLead != null ||
    config.priceToBeatIvTooGoodToBeTrueGap != null ||
    config.priceToBeatIvModelBookGapWarn != null ||
    config.priceToBeatIvModelBookGapHard != null ||
    config.priceToBeatIvModelBookWarnThresholdPenalty != null ||
    config.priceToBeatIvModelBookWarnGapPenalty != null ||
    config.priceToBeatIvDepthGuardEnabled != null ||
    config.priceToBeatIvDepthMaxSlippage != null ||
    config.priceToBeatIvLateUnconfirmedUnderSec != null ||
    config.priceToBeatIvLateUnconfirmedMinSelectedMid != null ||
    config.priceToBeatIvLateUnconfirmedMinQ != null ||
    config.priceToBeatIvLateHighPriceSoftUnderSec != null ||
    config.priceToBeatIvLateHighPriceAskCent != null ||
    config.priceToBeatIvLateHighPriceSelectedMidSoftCent != null ||
    config.priceToBeatIvLateHighPriceThresholdPenalty != null ||
    config.priceToBeatIvLateHighPriceSelectedMidHardCent != null ||
    config.priceToBeatIvLateHighPriceMinGapUsd != null ||
    config.priceToBeatIvParticipationCreditEnabled != null ||
    config.priceToBeatIvParticipationAfterMinutes != null ||
    config.priceToBeatIvParticipationLongAfterMinutes != null ||
    config.priceToBeatIvParticipationCredit != null ||
    config.priceToBeatIvParticipationLongCredit != null ||
    config.priceToBeatIvParticipationMinThreshold != null ||
    config.priceToBeatIvRequireBinanceFreshUnderSec != null ||
    config.priceToBeatIvBinanceMaxStaleMs != null ||
    config.priceToBeatIvRequireBinanceSameDirection != null ||
    config.priceToBeatIvMomentumProtectionEnabled != null ||
    config.priceToBeatIvDropZBlockThreshold != null ||
    config.priceToBeatIvProtectionSoftThresholdPenalty != null ||
    config.priceToBeatIvProtectionSoftGapStrengthPenalty != null ||
    config.priceToBeatIvVolumeBaselineMode != null ||
    config.priceToBeatIvVolumeBaselineLookbackDays != null ||
    config.priceToBeatIvVolumeWindowSec != null ||
    config.priceToBeatIvVolumeBaselineMinSamples != null ||
    config.priceToBeatIvLowHourlyVolumeRatio != null ||
    config.priceToBeatIvHighHourlyVolumeRatio != null ||
    config.priceToBeatIvExtremeHourlyVolumeRatio != null ||
    config.priceToBeatIvBookReliabilityThreshold != null ||
    config.priceToBeatIvAdaptiveGreenEdgeDelta != null ||
    config.priceToBeatIvAdaptiveGreenGapStrengthDelta != null ||
    config.priceToBeatIvAdaptiveOrangeEdgeDelta != null ||
    config.priceToBeatIvAdaptiveOrangeGapStrengthDelta != null ||
    config.priceToBeatIvAdaptiveOrangeGapUsdMarginDelta != null ||
    config.priceToBeatIvAdaptiveRedBlock != null;
  if (!hasIvConfig) return;

  if (priceToBeatGuardEnabled !== true) {
    pushNodeError(
      issues,
      node,
      'price_to_beat_iv_requires_guard',
      'action.place_order IV time rules require priceToBeatGuardEnabled=true.'
    );
  }
  if (normalizedPriceToBeatMode !== 'iv_mismatch_edge') {
    pushNodeError(
      issues,
      node,
      'price_to_beat_iv_requires_mode',
      'action.place_order IV time rules require priceToBeatMode=iv_mismatch_edge.'
    );
  }

  validateIvTimeRules(issues, node, config.priceToBeatIvTimeRules);
  validateOptionalNonNegativeInteger(
    issues,
    node,
    config.priceToBeatIvStalePenaltyMs,
    'priceToBeatIvStalePenaltyMs',
    'invalid_price_to_beat_iv_stale_penalty_ms'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvStaleGapStrengthPenalty,
    'priceToBeatIvStaleGapStrengthPenalty',
    'invalid_price_to_beat_iv_stale_gap_strength_penalty'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvNegativeVelocityGapStrengthPenalty,
    'priceToBeatIvNegativeVelocityGapStrengthPenalty',
    'invalid_price_to_beat_iv_negative_velocity_gap_strength_penalty'
  );
  validateOptionalCentPrice(
    issues,
    node,
    config.priceToBeatIvBinanceMissingAskThresholdCent,
    'priceToBeatIvBinanceMissingAskThresholdCent',
    'invalid_price_to_beat_iv_binance_missing_ask_threshold'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvBinanceMissingPenalty,
    'priceToBeatIvBinanceMissingPenalty',
    'invalid_price_to_beat_iv_binance_missing_penalty'
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvMinAdjustedMargin,
    'priceToBeatIvMinAdjustedMargin',
    'invalid_price_to_beat_iv_min_adjusted_margin',
    true
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvMinFinalQ,
    'priceToBeatIvMinFinalQ',
    'invalid_price_to_beat_iv_min_final_q',
    false
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvBinanceDisagreementThreshold,
    'priceToBeatIvBinanceDisagreementThreshold',
    'invalid_price_to_beat_iv_binance_disagreement_threshold',
    false
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvBinanceDisagreementPenalty,
    'priceToBeatIvBinanceDisagreementPenalty',
    'invalid_price_to_beat_iv_binance_disagreement_penalty',
    true
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvLargeBinanceDisagreementThreshold,
    'priceToBeatIvLargeBinanceDisagreementThreshold',
    'invalid_price_to_beat_iv_large_binance_disagreement_threshold',
    false
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvLargeBinanceDisagreementPenalty,
    'priceToBeatIvLargeBinanceDisagreementPenalty',
    'invalid_price_to_beat_iv_large_binance_disagreement_penalty',
    true
  );
  validateProtectionMode(issues, node, config.priceToBeatIvProtectionMode);
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvBookLeadGuardEnabled,
    'priceToBeatIvBookLeadGuardEnabled',
    'invalid_price_to_beat_iv_book_lead_guard_enabled'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvBookLeadUnderSec,
    'priceToBeatIvBookLeadUnderSec',
    'invalid_price_to_beat_iv_book_lead_under_sec'
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvBookLeadMinMidDiff,
    'priceToBeatIvBookLeadMinMidDiff',
    'invalid_price_to_beat_iv_book_lead_min_mid_diff',
    true
  );
  validateOptionalCentPrice(
    issues,
    node,
    config.priceToBeatIvOppositeMidBlockCent,
    'priceToBeatIvOppositeMidBlockCent',
    'invalid_price_to_beat_iv_opposite_mid_block'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvBlockOnOppositeBookLead,
    'priceToBeatIvBlockOnOppositeBookLead',
    'invalid_price_to_beat_iv_block_on_opposite_book_lead'
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvTooGoodToBeTrueGap,
    'priceToBeatIvTooGoodToBeTrueGap',
    'invalid_price_to_beat_iv_too_good_to_be_true_gap',
    true
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvModelBookGapWarn,
    'priceToBeatIvModelBookGapWarn',
    'invalid_price_to_beat_iv_model_book_gap_warn',
    true
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvModelBookGapHard,
    'priceToBeatIvModelBookGapHard',
    'invalid_price_to_beat_iv_model_book_gap_hard',
    true
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvModelBookWarnThresholdPenalty,
    'priceToBeatIvModelBookWarnThresholdPenalty',
    'invalid_price_to_beat_iv_model_book_warn_threshold_penalty',
    true
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvModelBookWarnGapPenalty,
    'priceToBeatIvModelBookWarnGapPenalty',
    'invalid_price_to_beat_iv_model_book_warn_gap_penalty'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvDepthGuardEnabled,
    'priceToBeatIvDepthGuardEnabled',
    'invalid_price_to_beat_iv_depth_guard_enabled'
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvDepthMaxSlippage,
    'priceToBeatIvDepthMaxSlippage',
    'invalid_price_to_beat_iv_depth_max_slippage',
    true
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvLateHighPriceSoftUnderSec,
    'priceToBeatIvLateHighPriceSoftUnderSec',
    'invalid_price_to_beat_iv_late_high_price_soft_under_sec'
  );
  validateOptionalCentPrice(
    issues,
    node,
    config.priceToBeatIvLateHighPriceAskCent,
    'priceToBeatIvLateHighPriceAskCent',
    'invalid_price_to_beat_iv_late_high_price_ask'
  );
  validateOptionalCentPrice(
    issues,
    node,
    config.priceToBeatIvLateHighPriceSelectedMidSoftCent,
    'priceToBeatIvLateHighPriceSelectedMidSoftCent',
    'invalid_price_to_beat_iv_late_high_price_selected_mid_soft'
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvLateHighPriceThresholdPenalty,
    'priceToBeatIvLateHighPriceThresholdPenalty',
    'invalid_price_to_beat_iv_late_high_price_threshold_penalty',
    true
  );
  validateOptionalCentPrice(
    issues,
    node,
    config.priceToBeatIvLateHighPriceSelectedMidHardCent,
    'priceToBeatIvLateHighPriceSelectedMidHardCent',
    'invalid_price_to_beat_iv_late_high_price_selected_mid_hard'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvLateHighPriceMinGapUsd,
    'priceToBeatIvLateHighPriceMinGapUsd',
    'invalid_price_to_beat_iv_late_high_price_min_gap_usd'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvParticipationCreditEnabled,
    'priceToBeatIvParticipationCreditEnabled',
    'invalid_price_to_beat_iv_participation_credit_enabled'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvParticipationAfterMinutes,
    'priceToBeatIvParticipationAfterMinutes',
    'invalid_price_to_beat_iv_participation_after_minutes'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvParticipationLongAfterMinutes,
    'priceToBeatIvParticipationLongAfterMinutes',
    'invalid_price_to_beat_iv_participation_long_after_minutes'
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvParticipationCredit,
    'priceToBeatIvParticipationCredit',
    'invalid_price_to_beat_iv_participation_credit',
    true
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvParticipationLongCredit,
    'priceToBeatIvParticipationLongCredit',
    'invalid_price_to_beat_iv_participation_long_credit',
    true
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvParticipationMinThreshold,
    'priceToBeatIvParticipationMinThreshold',
    'invalid_price_to_beat_iv_participation_min_threshold',
    true
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvRequireBinanceFreshUnderSec,
    'priceToBeatIvRequireBinanceFreshUnderSec',
    'invalid_price_to_beat_iv_require_binance_fresh_under_sec'
  );
  validateOptionalNonNegativeInteger(
    issues,
    node,
    config.priceToBeatIvBinanceMaxStaleMs,
    'priceToBeatIvBinanceMaxStaleMs',
    'invalid_price_to_beat_iv_binance_max_stale_ms'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvRequireBinanceSameDirection,
    'priceToBeatIvRequireBinanceSameDirection',
    'invalid_price_to_beat_iv_require_binance_same_direction'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvMomentumProtectionEnabled,
    'priceToBeatIvMomentumProtectionEnabled',
    'invalid_price_to_beat_iv_momentum_protection_enabled'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvDropZBlockThreshold,
    'priceToBeatIvDropZBlockThreshold',
    'invalid_price_to_beat_iv_drop_z_block_threshold'
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvProtectionSoftThresholdPenalty,
    'priceToBeatIvProtectionSoftThresholdPenalty',
    'invalid_price_to_beat_iv_protection_soft_threshold_penalty',
    true
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvProtectionSoftGapStrengthPenalty,
    'priceToBeatIvProtectionSoftGapStrengthPenalty',
    'invalid_price_to_beat_iv_protection_soft_gap_strength_penalty'
  );
  validateVolumeBaselineMode(issues, node, config.priceToBeatIvVolumeBaselineMode);
  validateOptionalNonNegativeInteger(
    issues,
    node,
    config.priceToBeatIvVolumeBaselineLookbackDays,
    'priceToBeatIvVolumeBaselineLookbackDays',
    'invalid_price_to_beat_iv_volume_baseline_lookback_days'
  );
  validateOptionalNonNegativeInteger(
    issues,
    node,
    config.priceToBeatIvVolumeWindowSec,
    'priceToBeatIvVolumeWindowSec',
    'invalid_price_to_beat_iv_volume_window_sec'
  );
  validateOptionalNonNegativeInteger(
    issues,
    node,
    config.priceToBeatIvVolumeBaselineMinSamples,
    'priceToBeatIvVolumeBaselineMinSamples',
    'invalid_price_to_beat_iv_volume_baseline_min_samples'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvLowHourlyVolumeRatio,
    'priceToBeatIvLowHourlyVolumeRatio',
    'invalid_price_to_beat_iv_low_hourly_volume_ratio'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvHighHourlyVolumeRatio,
    'priceToBeatIvHighHourlyVolumeRatio',
    'invalid_price_to_beat_iv_high_hourly_volume_ratio'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvExtremeHourlyVolumeRatio,
    'priceToBeatIvExtremeHourlyVolumeRatio',
    'invalid_price_to_beat_iv_extreme_hourly_volume_ratio'
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvBookReliabilityThreshold,
    'priceToBeatIvBookReliabilityThreshold',
    'invalid_price_to_beat_iv_book_reliability_threshold',
    true
  );
  validateOptionalFiniteNumber(
    issues,
    node,
    config.priceToBeatIvAdaptiveGreenEdgeDelta,
    'priceToBeatIvAdaptiveGreenEdgeDelta',
    'invalid_price_to_beat_iv_adaptive_green_edge_delta'
  );
  validateOptionalFiniteNumber(
    issues,
    node,
    config.priceToBeatIvAdaptiveGreenGapStrengthDelta,
    'priceToBeatIvAdaptiveGreenGapStrengthDelta',
    'invalid_price_to_beat_iv_adaptive_green_gap_strength_delta'
  );
  validateOptionalFiniteNumber(
    issues,
    node,
    config.priceToBeatIvAdaptiveOrangeEdgeDelta,
    'priceToBeatIvAdaptiveOrangeEdgeDelta',
    'invalid_price_to_beat_iv_adaptive_orange_edge_delta'
  );
  validateOptionalFiniteNumber(
    issues,
    node,
    config.priceToBeatIvAdaptiveOrangeGapStrengthDelta,
    'priceToBeatIvAdaptiveOrangeGapStrengthDelta',
    'invalid_price_to_beat_iv_adaptive_orange_gap_strength_delta'
  );
  validateOptionalFiniteNumber(
    issues,
    node,
    config.priceToBeatIvAdaptiveOrangeGapUsdMarginDelta,
    'priceToBeatIvAdaptiveOrangeGapUsdMarginDelta',
    'invalid_price_to_beat_iv_adaptive_orange_gap_usd_margin_delta'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvAdaptiveRedBlock,
    'priceToBeatIvAdaptiveRedBlock',
    'invalid_price_to_beat_iv_adaptive_red_block'
  );
}

function validateIvTimeRules(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  rawRules: unknown
) {
  if (rawRules == null) return;
  if (!Array.isArray(rawRules)) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_time_rules',
      'action.place_order priceToBeatIvTimeRules must be an array.'
    );
    return;
  }

  const parsedRules: ParsedIvTimeRule[] = [];
  rawRules.forEach((rawRule, index) => {
    if (!isRecord(rawRule)) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_iv_time_rule',
        `action.place_order priceToBeatIvTimeRules[${index}] must be an object.`
      );
      return;
    }
    const startRemainingSec = toFiniteNumber(rawRule.startRemainingSec);
    const endRemainingSec = toFiniteNumber(rawRule.endRemainingSec);
    const maxPriceCent = toFiniteNumber(rawRule.maxPriceCent);
    const minEdge = toFiniteNumber(rawRule.minEdge);
    const minGapStrength = toFiniteNumber(rawRule.minGapStrength);
    const minExpectedMoveUsd = toFiniteNumber(rawRule.minExpectedMoveUsd);
    const minGapStrengthMargin = toFiniteNumber(rawRule.minGapStrengthMargin);
    const minGapUsdMargin = toFiniteNumber(rawRule.minGapUsdMargin);
    if (startRemainingSec == null || startRemainingSec <= 0) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_iv_time_rule_start',
        `action.place_order priceToBeatIvTimeRules[${index}].startRemainingSec must be > 0.`
      );
      return;
    }
    if (endRemainingSec == null || endRemainingSec < 0) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_iv_time_rule_end',
        `action.place_order priceToBeatIvTimeRules[${index}].endRemainingSec must be >= 0.`
      );
      return;
    }
    if (startRemainingSec <= endRemainingSec) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_iv_time_rule_range',
        `action.place_order priceToBeatIvTimeRules[${index}] must have startRemainingSec > endRemainingSec.`
      );
      return;
    }
    if (rawRule.maxPriceCent != null && (maxPriceCent == null || maxPriceCent <= 0 || maxPriceCent > 100)) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_iv_time_rule_max_price',
        `action.place_order priceToBeatIvTimeRules[${index}].maxPriceCent must be in (0, 100].`
      );
      return;
    }
    if (minEdge == null || minEdge < 0 || minEdge > 1) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_iv_time_rule_min_edge',
        `action.place_order priceToBeatIvTimeRules[${index}].minEdge must be between 0 and 1.`
      );
      return;
    }
    if (minGapStrength == null || minGapStrength < 0) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_iv_time_rule_min_gap_strength',
        `action.place_order priceToBeatIvTimeRules[${index}].minGapStrength must be >= 0.`
      );
      return;
    }
    if (rawRule.minExpectedMoveUsd != null && (minExpectedMoveUsd == null || minExpectedMoveUsd <= 0)) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_iv_time_rule_min_expected_move',
        `action.place_order priceToBeatIvTimeRules[${index}].minExpectedMoveUsd must be > 0.`
      );
      return;
    }
    if (rawRule.minGapStrengthMargin != null && (minGapStrengthMargin == null || minGapStrengthMargin < 0)) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_iv_time_rule_min_gap_strength_margin',
        `action.place_order priceToBeatIvTimeRules[${index}].minGapStrengthMargin must be >= 0.`
      );
      return;
    }
    if (rawRule.minGapUsdMargin != null && (minGapUsdMargin == null || minGapUsdMargin < 0)) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_iv_time_rule_min_gap_usd_margin',
        `action.place_order priceToBeatIvTimeRules[${index}].minGapUsdMargin must be >= 0.`
      );
      return;
    }
    parsedRules.push({ startRemainingSec, endRemainingSec });
  });

  for (let left = 0; left < parsedRules.length; left += 1) {
    for (let right = left + 1; right < parsedRules.length; right += 1) {
      if (timeRulesOverlap(parsedRules[left], parsedRules[right])) {
        pushNodeError(
          issues,
          node,
          'overlapping_price_to_beat_iv_time_rules',
          'action.place_order priceToBeatIvTimeRules ranges must not overlap.'
        );
        return;
      }
    }
  }
}

function timeRulesOverlap(left: ParsedIvTimeRule, right: ParsedIvTimeRule): boolean {
  return Math.max(left.endRemainingSec, right.endRemainingSec) <
    Math.min(left.startRemainingSec, right.startRemainingSec);
}

function validateProtectionMode(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown
) {
  if (value == null) return;
  const normalized = String(value).trim().toLowerCase();
  if (!['off', 'soft', 'balanced', 'hard', 'adaptive'].includes(normalized)) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_protection_mode',
      'action.place_order priceToBeatIvProtectionMode must be off|soft|hard|adaptive.'
    );
  }
}

function validateVolumeBaselineMode(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown
) {
  if (value == null) return;
  const normalized = String(value).trim().toLowerCase();
  if (!['off', 'hourly'].includes(normalized)) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_volume_baseline_mode',
      'action.place_order priceToBeatIvVolumeBaselineMode must be off|hourly.'
    );
  }
}

function validateOptionalBoolean(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string,
  code: string
) {
  if (value == null) return;
  if (typeof value !== 'boolean') {
    pushNodeError(issues, node, code, `action.place_order ${key} must be boolean.`);
  }
}

function validateOptionalNonNegativeInteger(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string,
  code: string
) {
  if (value == null) return;
  const parsed = toFiniteNumber(value);
  if (parsed == null || !Number.isInteger(parsed) || parsed < 0) {
    pushNodeError(issues, node, code, `action.place_order ${key} must be an integer >= 0.`);
  }
}

function validateOptionalNonNegativeNumber(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string,
  code: string
) {
  if (value == null) return;
  const parsed = toFiniteNumber(value);
  if (parsed == null || parsed < 0) {
    pushNodeError(issues, node, code, `action.place_order ${key} must be >= 0.`);
  }
}

function validateOptionalProbability(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string,
  code: string,
  allowZero: boolean
) {
  if (value == null) return;
  const parsed = toFiniteNumber(value);
  const lowerOk = parsed != null && (allowZero ? parsed >= 0 : parsed > 0);
  if (parsed == null || !lowerOk || parsed > 1) {
    pushNodeError(
      issues,
      node,
      code,
      `action.place_order ${key} must be ${allowZero ? 'between 0 and 1' : 'in (0, 1]'}.`
    );
  }
}

function validateOptionalFiniteNumber(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string,
  code: string
) {
  if (value == null) return;
  const parsed = toFiniteNumber(value);
  if (parsed == null) {
    pushNodeError(issues, node, code, `action.place_order ${key} must be a finite number.`);
  }
}

function validateOptionalCentPrice(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string,
  code: string
) {
  if (value == null) return;
  const parsed = toFiniteNumber(value);
  if (parsed == null || parsed <= 0 || parsed > 100) {
    pushNodeError(issues, node, code, `action.place_order ${key} must be in (0, 100].`);
  }
}
