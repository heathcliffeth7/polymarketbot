import type { TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import type { PtbMode } from '@/lib/trade-flow-config-mappers/ptb-modes';
import { isRecord, toFiniteNumber } from './shared';
import { pushNodeError } from './validation-core';
import { validateEq77RiskCap } from './validation-action-place-order-ptb-iv-risk-guards';

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
    config.priceToBeatIvEntryQualityPolicy != null ||
    config.priceToBeatIvNormalMaxPriceCent != null ||
    config.priceToBeatIvPremiumMaxPriceCent != null ||
    config.priceToBeatIvNoNewEntryBelowSeconds != null ||
    config.priceToBeatIvMinExpectedMoveBps != null ||
    config.priceToBeatIvMinExpectedMoveUsd != null ||
    config.priceToBeatIvMinExpectedMoveMode != null ||
    config.priceToBeatIvAdaptiveMinExpectedMoveBaseBps != null ||
    config.priceToBeatIvAdaptiveMinExpectedMoveMinBps != null ||
    config.priceToBeatIvAdaptiveMinExpectedMoveMaxBps != null ||
    config.priceToBeatIvAdaptiveDisagreementBpsAdd != null ||
    config.priceToBeatIvAdaptiveStrongDisagreementBpsAdd != null ||
    config.priceToBeatIvAdaptiveSpreadBpsAdd != null ||
    config.priceToBeatIvAdaptiveWideSpreadBpsAdd != null ||
    config.priceToBeatIvAdaptiveStaleBpsAdd != null ||
    config.priceToBeatIvAdaptiveNoiseBpsAdd != null ||
    config.priceToBeatIvGapStrengthMin60To45 != null ||
    config.priceToBeatIvGapStrengthMin45To25 != null ||
    config.priceToBeatIvGapStrengthMin25To10 != null ||
    config.priceToBeatIvGapStrengthMin10To8 != null ||
    config.priceToBeatIvBufferTrendGuardEnabled != null ||
    config.priceToBeatIvBufferRetain5s != null ||
    config.priceToBeatIvBufferRetain10s != null ||
    config.priceToBeatIvPremiumBufferRetain5s != null ||
    config.priceToBeatIvPremiumBufferRetain10s != null ||
    config.priceToBeatIvSpikeFadeGuardEnabled != null ||
    config.priceToBeatIvSpikeMultiplier != null ||
    config.priceToBeatIvSpikeRetraceRatio != null ||
    config.priceToBeatIvPremiumMaxSpreadCent != null ||
    config.priceToBeatIvPremiumMaxChainlinkAgeMs != null ||
    config.priceToBeatIvCexAlignMaxUsd != null ||
    config.priceToBeatIvCexAlignMaxBps != null ||
    config.priceToBeatIvCexOpenGapConsensusGuardEnabled != null ||
    config.priceToBeatIvCexOpenGapMinUsd != null ||
    config.priceToBeatIvCexOpenGapMinZ != null ||
    config.priceToBeatIvCexOpenGapMaxStaleMs != null ||
    config.priceToBeatIvCexOpenGapApplyNegativeConservativeCap != null ||
    config.priceToBeatIvChainlinkCexLagGuardEnabled != null ||
    config.priceToBeatIvChainlinkCexDiffZBlock != null ||
    config.priceToBeatIvChainlinkCexMaxDiffUsd != null ||
    config.priceToBeatIvChainlinkCexMaxDiffBps != null ||
    config.priceToBeatIvChainlinkCexBookMismatchDislocationCent != null ||
    config.priceToBeatIvGapFailMixedCexGuardEnabled != null ||
    config.priceToBeatIvGapFailMixedCexMaxSeconds != null ||
    config.priceToBeatIvLateExpensiveMixedCexGuardEnabled != null ||
    config.priceToBeatIvLateExpensiveMixedCexSeconds != null ||
    config.priceToBeatIvLateExpensiveMixedCexMinVwapCent != null ||
    config.priceToBeatIvLateExpensiveMixedCexRequireGapFailOrLagHigh != null ||
    config.priceToBeatIvChainlinkCexLagNoBookGuardEnabled != null ||
    config.priceToBeatIvChainlinkCexLagNoBookMaxSeconds != null ||
    config.priceToBeatIvChainlinkCexLagNoBookRequireNonStrongCex != null ||
    config.priceToBeatIvEq77RiskCapEnabled != null ||
    config.priceToBeatIvRiskScoreCleanMax != null ||
    config.priceToBeatIvRiskScoreModerateMax != null ||
    config.priceToBeatIvRiskScoreHighMax != null ||
    config.priceToBeatIvModerateRiskMaxPriceCent != null ||
    config.priceToBeatIvHighRiskMaxPriceCent != null ||
    config.priceToBeatIvDeepValueMaxPriceCent != null ||
    config.priceToBeatIvMaxRiskHaircutCent != null ||
    config.priceToBeatIvWaitForPriceEnabled != null ||
    config.priceToBeatIvRecheckBeforeSubmit != null ||
    config.priceToBeatIvOddsMaxSpreadCent != null ||
    config.priceToBeatIvCexUnconfirmedRiskPoints != null ||
    config.priceToBeatIvCexConflictRiskPoints != null ||
    config.priceToBeatIvPassiveBidEnabled != null ||
    config.priceToBeatIvPassiveBidTtlMs != null ||
    config.priceToBeatIvWaitRepriceGuardEnabled != null ||
    config.priceToBeatIvWaitMaxAgeMsEarly != null ||
    config.priceToBeatIvWaitMaxAgeMsMid != null ||
    config.priceToBeatIvWaitMaxAgeMsLate != null ||
    config.priceToBeatIvWaitInitialAskMaxOverCapCent != null ||
    config.priceToBeatIvFallingIntoCapGuardEnabled != null ||
    config.priceToBeatIvFallingIntoCapDropCentEarly != null ||
    config.priceToBeatIvFallingIntoCapDropCentMid != null ||
    config.priceToBeatIvFallingIntoCapDropCentLate != null ||
    config.priceToBeatIvLateExpensiveEntryGuardEnabled != null ||
    config.priceToBeatIvLateExpensiveSeconds != null ||
    config.priceToBeatIvLateExpensiveVwapCent != null ||
    config.priceToBeatIvLateExpensiveMinQCent != null ||
    config.priceToBeatIvLateExpensiveMinGapStrengthExtra != null ||
    config.priceToBeatIvBorderlinePumpBookLeadGuardEnabled != null ||
    config.priceToBeatIvBorderlineGapMarginEarly != null ||
    config.priceToBeatIvBorderlinePumpShockRatio != null ||
    config.priceToBeatIvBorderlineBookLeadQMinCent != null ||
    config.priceToBeatIvBorderlineBookLeadCheapTokenCent != null ||
    config.priceToBeatIvBorderlineBookLeadDislocationCent != null ||
    Object.keys(config).some((key) => key.startsWith('priceToBeatIvPtbChop')) ||
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
    config.priceToBeatIvOracleLagBookLeadGuardEnabled != null ||
    config.priceToBeatIvOracleLagUseBestAskFallback != null ||
    config.priceToBeatIvOracleLagQExtremeCent != null ||
    config.priceToBeatIvOracleLagCheapTokenExtremeCent != null ||
    config.priceToBeatIvOracleLagConsensusMismatchQCent != null ||
    config.priceToBeatIvOracleLagConsensusMismatchCheapTokenCent != null ||
    config.priceToBeatIvOracleLagConsensusMismatchDislocationCent != null ||
    config.priceToBeatIvModelBookDislocationRedCent != null ||
    config.priceToBeatIvExecutionVwapCostGuardEnabled != null ||
    config.priceToBeatIvExecutionVwapRequiredOnHighDislocation != null ||
    config.priceToBeatIvExecutionLimitByVwapEnabled != null ||
    config.priceToBeatIvExecutionVwapMaxSlippageCent != null ||
    config.priceToBeatIvModelBookWarnThresholdPenalty != null ||
    config.priceToBeatIvModelBookWarnGapPenalty != null ||
    config.priceToBeatIvDepthGuardEnabled != null ||
    config.priceToBeatIvDepthGuardHardBlockEnabled != null ||
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
  for (const key of [
    'priceToBeatIvOracleLagBookLeadGuardEnabled',
    'priceToBeatIvOracleLagUseBestAskFallback',
    'priceToBeatIvExecutionVwapCostGuardEnabled',
    'priceToBeatIvExecutionVwapRequiredOnHighDislocation',
    'priceToBeatIvExecutionLimitByVwapEnabled',
  ]) {
    validateOptionalBoolean(issues, node, config[key], key, `invalid_${toSnakeCase(key)}`);
  }
  for (const key of [
    'priceToBeatIvOracleLagQHighCent',
    'priceToBeatIvOracleLagCheapTokenCent',
    'priceToBeatIvOracleLagQExtremeCent',
    'priceToBeatIvOracleLagCheapTokenExtremeCent',
    'priceToBeatIvOracleLagConsensusMismatchQCent',
    'priceToBeatIvOracleLagConsensusMismatchCheapTokenCent',
    'priceToBeatIvOracleLagConsensusMismatchDislocationCent',
    'priceToBeatIvModelBookDislocationHighCent',
    'priceToBeatIvModelBookDislocationRedCent',
    'priceToBeatIvExecutionVwapMaxSlippageCent',
  ]) {
    validateOptionalCentPriceAllowZero(
      issues,
      node,
      config[key],
      key,
      `invalid_${toSnakeCase(key)}`
    );
  }
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvBorderlinePumpBookLeadGuardEnabled,
    'priceToBeatIvBorderlinePumpBookLeadGuardEnabled',
    'invalid_price_to_beat_iv_borderline_pump_book_lead_guard_enabled'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvBorderlineGapMarginEarly,
    'priceToBeatIvBorderlineGapMarginEarly',
    'invalid_price_to_beat_iv_borderline_gap_margin_early'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvBorderlinePumpShockRatio,
    'priceToBeatIvBorderlinePumpShockRatio',
    'invalid_price_to_beat_iv_borderline_pump_shock_ratio'
  );
  validateOptionalCentPrice(
    issues,
    node,
    config.priceToBeatIvBorderlineBookLeadQMinCent,
    'priceToBeatIvBorderlineBookLeadQMinCent',
    'invalid_price_to_beat_iv_borderline_book_lead_q_min_cent'
  );
  validateOptionalCentPrice(
    issues,
    node,
    config.priceToBeatIvBorderlineBookLeadCheapTokenCent,
    'priceToBeatIvBorderlineBookLeadCheapTokenCent',
    'invalid_price_to_beat_iv_borderline_book_lead_cheap_token_cent'
  );
  validateOptionalCentPrice(
    issues,
    node,
    config.priceToBeatIvBorderlineBookLeadDislocationCent,
    'priceToBeatIvBorderlineBookLeadDislocationCent',
    'invalid_price_to_beat_iv_borderline_book_lead_dislocation_cent'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvPtbChopGuardEnabled,
    'priceToBeatIvPtbChopGuardEnabled',
    'invalid_price_to_beat_iv_ptb_chop_guard_enabled'
  );
  for (const key of [
    'priceToBeatIvPtbChopLookbackSeconds',
    'priceToBeatIvPtbChopExtendedLookbackSeconds',
    'priceToBeatIvPtbChopZeroCrossBlock10s',
    'priceToBeatIvPtbChopZeroCrossBlock15s',
  ]) {
    validateOptionalNonNegativeInteger(issues, node, config[key], key, `invalid_${toSnakeCase(key)}`);
  }
  for (const key of [
    'priceToBeatIvPtbChopDeadbandBps',
    'priceToBeatIvPtbChopDeadbandMinUsdBtc',
    'priceToBeatIvPtbChopDeadbandMinUsdEth',
    'priceToBeatIvPtbChopDeadbandMinUsdSol',
    'priceToBeatIvPtbChopPathZWarn',
    'priceToBeatIvPtbChopPathZBlock',
    'priceToBeatIvPtbChopOppositeDepthZWarn',
    'priceToBeatIvPtbChopOppositeDepthZBlock',
    'priceToBeatIvPtbChopMaxGapStrengthPenalty',
  ]) {
    validateOptionalNonNegativeNumber(issues, node, config[key], key, `invalid_${toSnakeCase(key)}`);
  }
  for (const key of [
    'priceToBeatIvPtbChopEfficiencyWarn',
    'priceToBeatIvPtbChopEfficiencyBlock',
  ]) {
    validateOptionalProbability(issues, node, config[key], key, `invalid_${toSnakeCase(key)}`, true);
  }
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvDepthGuardEnabled,
    'priceToBeatIvDepthGuardEnabled',
    'invalid_price_to_beat_iv_depth_guard_enabled'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvDepthGuardHardBlockEnabled,
    'priceToBeatIvDepthGuardHardBlockEnabled',
    'invalid_price_to_beat_iv_depth_guard_hard_block_enabled'
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
  validateEntryQualityV2(issues, node, config);
}

function validateEntryQualityV2(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>
) {
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvEntryQualityPolicy,
    'priceToBeatIvEntryQualityPolicy',
    'invalid_price_to_beat_iv_entry_quality_policy'
  );
  validateOptionalCentPrice(
    issues,
    node,
    config.priceToBeatIvNormalMaxPriceCent,
    'priceToBeatIvNormalMaxPriceCent',
    'invalid_price_to_beat_iv_normal_max_price'
  );
  validateOptionalCentPrice(
    issues,
    node,
    config.priceToBeatIvPremiumMaxPriceCent,
    'priceToBeatIvPremiumMaxPriceCent',
    'invalid_price_to_beat_iv_premium_max_price'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvNoNewEntryBelowSeconds,
    'priceToBeatIvNoNewEntryBelowSeconds',
    'invalid_price_to_beat_iv_no_new_entry_below_seconds'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvMinExpectedMoveBps,
    'priceToBeatIvMinExpectedMoveBps',
    'invalid_price_to_beat_iv_min_expected_move_bps'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvMinExpectedMoveUsd,
    'priceToBeatIvMinExpectedMoveUsd',
    'invalid_price_to_beat_iv_min_expected_move_usd'
  );
  validateAdaptiveExpectedMoveFloor(issues, node, config);
  for (const key of [
    'priceToBeatIvGapStrengthMin60To45',
    'priceToBeatIvGapStrengthMin45To25',
    'priceToBeatIvGapStrengthMin25To10',
    'priceToBeatIvGapStrengthMin10To8',
  ]) {
    validateOptionalNonNegativeNumber(
      issues,
      node,
      config[key],
      key,
      `invalid_${toSnakeCase(key)}`
    );
  }
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvBufferTrendGuardEnabled,
    'priceToBeatIvBufferTrendGuardEnabled',
    'invalid_price_to_beat_iv_buffer_trend_guard_enabled'
  );
  for (const key of [
    'priceToBeatIvBufferRetain5s',
    'priceToBeatIvBufferRetain10s',
    'priceToBeatIvPremiumBufferRetain5s',
    'priceToBeatIvPremiumBufferRetain10s',
  ]) {
    validateOptionalProbability(issues, node, config[key], key, `invalid_${toSnakeCase(key)}`, true);
  }
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvSpikeFadeGuardEnabled,
    'priceToBeatIvSpikeFadeGuardEnabled',
    'invalid_price_to_beat_iv_spike_fade_guard_enabled'
  );
  validateOptionalGreaterThan(
    issues,
    node,
    config.priceToBeatIvSpikeMultiplier,
    'priceToBeatIvSpikeMultiplier',
    'invalid_price_to_beat_iv_spike_multiplier',
    1
  );
  validateOptionalProbability(
    issues,
    node,
    config.priceToBeatIvSpikeRetraceRatio,
    'priceToBeatIvSpikeRetraceRatio',
    'invalid_price_to_beat_iv_spike_retrace_ratio',
    true
  );
  validateOptionalCentPriceAllowZero(
    issues,
    node,
    config.priceToBeatIvPremiumMaxSpreadCent,
    'priceToBeatIvPremiumMaxSpreadCent',
    'invalid_price_to_beat_iv_premium_max_spread'
  );
  validateOptionalPositiveInteger(
    issues,
    node,
    config.priceToBeatIvPremiumMaxChainlinkAgeMs,
    'priceToBeatIvPremiumMaxChainlinkAgeMs',
    'invalid_price_to_beat_iv_premium_max_chainlink_age'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvCexAlignMaxUsd,
    'priceToBeatIvCexAlignMaxUsd',
    'invalid_price_to_beat_iv_cex_align_max_usd'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvCexAlignMaxBps,
    'priceToBeatIvCexAlignMaxBps',
    'invalid_price_to_beat_iv_cex_align_max_bps'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvCexOpenGapConsensusGuardEnabled,
    'priceToBeatIvCexOpenGapConsensusGuardEnabled',
    'invalid_price_to_beat_iv_cex_open_gap_consensus_guard_enabled'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvCexOpenGapApplyNegativeConservativeCap,
    'priceToBeatIvCexOpenGapApplyNegativeConservativeCap',
    'invalid_price_to_beat_iv_cex_open_gap_apply_negative_cap'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvChainlinkCexLagGuardEnabled,
    'priceToBeatIvChainlinkCexLagGuardEnabled',
    'invalid_price_to_beat_iv_chainlink_cex_lag_guard_enabled'
  );
  for (const key of [
    'priceToBeatIvCexOpenGapMinUsd',
    'priceToBeatIvCexOpenGapMinZ',
    'priceToBeatIvChainlinkCexDiffZBlock',
    'priceToBeatIvChainlinkCexMaxDiffUsd',
    'priceToBeatIvChainlinkCexMaxDiffBps',
  ]) {
    validateOptionalNonNegativeNumber(issues, node, config[key], key, `invalid_${toSnakeCase(key)}`);
  }
  validateOptionalNonNegativeInteger(
    issues,
    node,
    config.priceToBeatIvCexOpenGapMaxStaleMs,
    'priceToBeatIvCexOpenGapMaxStaleMs',
    'invalid_price_to_beat_iv_cex_open_gap_max_stale_ms'
  );
  validateOptionalCentPriceAllowZero(
    issues,
    node,
    config.priceToBeatIvChainlinkCexBookMismatchDislocationCent,
    'priceToBeatIvChainlinkCexBookMismatchDislocationCent',
    'invalid_price_to_beat_iv_chainlink_cex_book_mismatch_dislocation'
  );
  validateEq77RiskCap(issues, node, config);
  validateEntryQualityRelations(issues, node, config);
}

function validateAdaptiveExpectedMoveFloor(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>
) {
  const mode = config.priceToBeatIvMinExpectedMoveMode;
  if (mode != null && mode !== 'fixed' && mode !== 'adaptive') {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_min_expected_move_mode',
      'action.place_order priceToBeatIvMinExpectedMoveMode must be fixed or adaptive.'
    );
  }
  for (const key of [
    'priceToBeatIvAdaptiveMinExpectedMoveBaseBps',
    'priceToBeatIvAdaptiveMinExpectedMoveMinBps',
    'priceToBeatIvAdaptiveMinExpectedMoveMaxBps',
    'priceToBeatIvAdaptiveDisagreementBpsAdd',
    'priceToBeatIvAdaptiveStrongDisagreementBpsAdd',
    'priceToBeatIvAdaptiveSpreadBpsAdd',
    'priceToBeatIvAdaptiveWideSpreadBpsAdd',
    'priceToBeatIvAdaptiveStaleBpsAdd',
    'priceToBeatIvAdaptiveNoiseBpsAdd',
  ]) {
    validateOptionalNonNegativeNumber(
      issues,
      node,
      config[key],
      key,
      `invalid_${toSnakeCase(key)}`
    );
  }

  const min = toFiniteNumber(config.priceToBeatIvAdaptiveMinExpectedMoveMinBps);
  const base = toFiniteNumber(config.priceToBeatIvAdaptiveMinExpectedMoveBaseBps);
  const max = toFiniteNumber(config.priceToBeatIvAdaptiveMinExpectedMoveMaxBps);
  if (max != null && max > 5.0) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_adaptive_min_expected_move_max_bps',
      'action.place_order priceToBeatIvAdaptiveMinExpectedMoveMaxBps must be <= 5.'
    );
  }
  if (min != null && base != null && min > base) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_adaptive_min_expected_move_bps_order',
      'action.place_order adaptive expected move min bps must be <= base bps.'
    );
  }
  if (base != null && max != null && base > max) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_adaptive_min_expected_move_bps_order',
      'action.place_order adaptive expected move base bps must be <= max bps.'
    );
  }
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

function validateEntryQualityRelations(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>
) {
  const normalMax = toFiniteNumber(config.priceToBeatIvNormalMaxPriceCent);
  const premiumMax = toFiniteNumber(config.priceToBeatIvPremiumMaxPriceCent);
  if (
    normalMax != null &&
    premiumMax != null &&
    normalMax >= premiumMax
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_price_caps_relation',
      'action.place_order priceToBeatIvNormalMaxPriceCent must be smaller than priceToBeatIvPremiumMaxPriceCent.'
    );
  }

  const retain5 = toFiniteNumber(config.priceToBeatIvBufferRetain5s);
  const premiumRetain5 = toFiniteNumber(config.priceToBeatIvPremiumBufferRetain5s);
  if (retain5 != null && premiumRetain5 != null && premiumRetain5 < retain5) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_premium_retain_5s_relation',
      'action.place_order priceToBeatIvPremiumBufferRetain5s must be >= priceToBeatIvBufferRetain5s.'
    );
  }

  const retain10 = toFiniteNumber(config.priceToBeatIvBufferRetain10s);
  const premiumRetain10 = toFiniteNumber(config.priceToBeatIvPremiumBufferRetain10s);
  if (retain10 != null && premiumRetain10 != null && premiumRetain10 < retain10) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_premium_retain_10s_relation',
      'action.place_order priceToBeatIvPremiumBufferRetain10s must be >= priceToBeatIvBufferRetain10s.'
    );
  }

  const clean = toFiniteNumber(config.priceToBeatIvRiskScoreCleanMax);
  const moderate = toFiniteNumber(config.priceToBeatIvRiskScoreModerateMax);
  const high = toFiniteNumber(config.priceToBeatIvRiskScoreHighMax);
  if (clean != null && moderate != null && clean >= moderate) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_risk_score_order',
      'action.place_order risk score clean max must be smaller than moderate max.'
    );
  }
  if (moderate != null && high != null && moderate >= high) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_risk_score_order',
      'action.place_order risk score moderate max must be smaller than high max.'
    );
  }

  const moderateCap = toFiniteNumber(config.priceToBeatIvModerateRiskMaxPriceCent);
  const highCap = toFiniteNumber(config.priceToBeatIvHighRiskMaxPriceCent);
  const deepCap = toFiniteNumber(config.priceToBeatIvDeepValueMaxPriceCent);
  if (normalMax != null && moderateCap != null && moderateCap > normalMax) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_risk_cap_order',
      'action.place_order moderate risk max price must be <= normal max price.'
    );
  }
  if (moderateCap != null && highCap != null && highCap > moderateCap) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_risk_cap_order',
      'action.place_order high risk max price must be <= moderate risk max price.'
    );
  }
  if (highCap != null && deepCap != null && deepCap > highCap) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_risk_cap_order',
      'action.place_order deep value max price must be <= high risk max price.'
    );
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

function validateOptionalPositiveInteger(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string,
  code: string
) {
  if (value == null) return;
  const parsed = toFiniteNumber(value);
  if (parsed == null || !Number.isInteger(parsed) || parsed <= 0) {
    pushNodeError(issues, node, code, `action.place_order ${key} must be an integer > 0.`);
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

function validateOptionalGreaterThan(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string,
  code: string,
  minExclusive: number
) {
  if (value == null) return;
  const parsed = toFiniteNumber(value);
  if (parsed == null || parsed <= minExclusive) {
    pushNodeError(
      issues,
      node,
      code,
      `action.place_order ${key} must be > ${minExclusive}.`
    );
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

function validateOptionalCentPriceAllowZero(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string,
  code: string
) {
  if (value == null) return;
  const parsed = toFiniteNumber(value);
  if (parsed == null || parsed < 0 || parsed > 100) {
    pushNodeError(issues, node, code, `action.place_order ${key} must be between 0 and 100.`);
  }
}

function toSnakeCase(value: string): string {
  return value.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`).replace(/^_/, '');
}
