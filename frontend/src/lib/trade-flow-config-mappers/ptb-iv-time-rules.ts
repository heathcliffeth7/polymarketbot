import type { NodeConfigFormState, PtbIvTimeRuleRow } from './types';
import { createEmptyPtbIvTimeRuleRow } from './drafts';
import { isRecord, toCentStringValue, toStringValue } from './utils';

interface SerializedPtbIvTimeRule {
  startRemainingSec: number;
  endRemainingSec: number;
  maxPriceCent?: number;
  minEdge: number;
  minGapStrength: number;
  minExpectedMoveUsd?: number;
  minGapStrengthMargin?: number;
  minGapUsdMargin?: number;
}

const REVENGE_FLIP_MODE = 'revenge_flip_v1';
const REVENGE_FLIP_DEFAULT_IV_TIME_RULES: SerializedPtbIvTimeRule[] = [
  {
    startRemainingSec: 45,
    endRemainingSec: 30,
    maxPriceCent: 92,
    minEdge: 0.03,
    minGapStrength: 0.5,
  },
  {
    startRemainingSec: 30,
    endRemainingSec: 15,
    maxPriceCent: 92,
    minEdge: 0.05,
    minGapStrength: 0.75,
  },
  {
    startRemainingSec: 15,
    endRemainingSec: 8,
    maxPriceCent: 92,
    minEdge: 0.07,
    minGapStrength: 1,
  },
];

const PTB_IV_CONFIG_KEYS = [
  'priceToBeatIvTimeRules',
  'priceToBeatIvEntryQualityPolicy',
  'priceToBeatIvNormalMaxPriceCent',
  'priceToBeatIvPremiumMaxPriceCent',
  'priceToBeatIvNoNewEntryBelowSeconds',
  'priceToBeatIvMinExpectedMoveBps',
  'priceToBeatIvMinExpectedMoveUsd',
  'priceToBeatIvMinExpectedMoveMode',
  'priceToBeatIvAdaptiveMinExpectedMoveBaseBps',
  'priceToBeatIvAdaptiveMinExpectedMoveMinBps',
  'priceToBeatIvAdaptiveMinExpectedMoveMaxBps',
  'priceToBeatIvAdaptiveDisagreementBpsAdd',
  'priceToBeatIvAdaptiveStrongDisagreementBpsAdd',
  'priceToBeatIvAdaptiveSpreadBpsAdd',
  'priceToBeatIvAdaptiveWideSpreadBpsAdd',
  'priceToBeatIvAdaptiveStaleBpsAdd',
  'priceToBeatIvAdaptiveNoiseBpsAdd',
  'priceToBeatIvGapStrengthMin60To45',
  'priceToBeatIvGapStrengthMin45To25',
  'priceToBeatIvGapStrengthMin25To10',
  'priceToBeatIvGapStrengthMin10To8',
  'priceToBeatIvBufferTrendGuardEnabled',
  'priceToBeatIvBufferRetain5s',
  'priceToBeatIvBufferRetain10s',
  'priceToBeatIvPremiumBufferRetain5s',
  'priceToBeatIvPremiumBufferRetain10s',
  'priceToBeatIvSpikeFadeGuardEnabled',
  'priceToBeatIvSpikeMultiplier',
  'priceToBeatIvSpikeRetraceRatio',
  'priceToBeatIvPremiumMaxSpreadCent',
  'priceToBeatIvPremiumMaxChainlinkAgeMs',
  'priceToBeatIvCexAlignMaxUsd',
  'priceToBeatIvCexAlignMaxBps',
  'priceToBeatIvCexOpenGapConsensusGuardEnabled',
  'priceToBeatIvCexOpenGapMinUsd',
  'priceToBeatIvCexOpenGapMinZ',
  'priceToBeatIvCexOpenGapMaxStaleMs',
  'priceToBeatIvCexOpenGapApplyNegativeConservativeCap',
  'priceToBeatIvChainlinkCexLagGuardEnabled',
  'priceToBeatIvChainlinkCexDiffZBlock',
  'priceToBeatIvChainlinkCexMaxDiffUsd',
  'priceToBeatIvChainlinkCexMaxDiffBps',
  'priceToBeatIvChainlinkCexBookMismatchDislocationCent',
  'priceToBeatIvGapFailMixedCexGuardEnabled',
  'priceToBeatIvGapFailMixedCexMaxSeconds',
  'priceToBeatIvLateExpensiveMixedCexGuardEnabled',
  'priceToBeatIvLateExpensiveMixedCexSeconds',
  'priceToBeatIvLateExpensiveMixedCexMinVwapCent',
  'priceToBeatIvLateExpensiveMixedCexRequireGapFailOrLagHigh',
  'priceToBeatIvChainlinkCexLagNoBookGuardEnabled',
  'priceToBeatIvChainlinkCexLagNoBookMaxSeconds',
  'priceToBeatIvChainlinkCexLagNoBookRequireNonStrongCex',
  'priceToBeatIvEq77RiskCapEnabled',
  'priceToBeatIvRiskScoreCleanMax',
  'priceToBeatIvRiskScoreModerateMax',
  'priceToBeatIvRiskScoreHighMax',
  'priceToBeatIvModerateRiskMaxPriceCent',
  'priceToBeatIvHighRiskMaxPriceCent',
  'priceToBeatIvDeepValueMaxPriceCent',
  'priceToBeatIvMaxRiskHaircutCent',
  'priceToBeatIvWaitForPriceEnabled',
  'priceToBeatIvRecheckBeforeSubmit',
  'priceToBeatIvOddsMaxSpreadCent',
  'priceToBeatIvCexUnconfirmedRiskPoints',
  'priceToBeatIvCexConflictRiskPoints',
  'priceToBeatIvPassiveBidEnabled',
  'priceToBeatIvPassiveBidTtlMs',
  'priceToBeatIvWaitRepriceGuardEnabled',
  'priceToBeatIvWaitMaxAgeMsEarly',
  'priceToBeatIvWaitMaxAgeMsMid',
  'priceToBeatIvWaitMaxAgeMsLate',
  'priceToBeatIvWaitInitialAskMaxOverCapCent',
  'priceToBeatIvFallingIntoCapGuardEnabled',
  'priceToBeatIvFallingIntoCapDropCentEarly',
  'priceToBeatIvFallingIntoCapDropCentMid',
  'priceToBeatIvFallingIntoCapDropCentLate',
  'priceToBeatIvLateExpensiveEntryGuardEnabled',
  'priceToBeatIvLateExpensiveSeconds',
  'priceToBeatIvLateExpensiveVwapCent',
  'priceToBeatIvLateExpensiveMinQCent',
  'priceToBeatIvLateExpensiveMinGapStrengthExtra',
  'priceToBeatIvOracleLagBookLeadGuardEnabled',
  'priceToBeatIvOracleLagUseBestAskFallback',
  'priceToBeatIvOracleLagQExtremeCent',
  'priceToBeatIvOracleLagCheapTokenExtremeCent',
  'priceToBeatIvOracleLagConsensusMismatchQCent',
  'priceToBeatIvOracleLagConsensusMismatchCheapTokenCent',
  'priceToBeatIvOracleLagConsensusMismatchDislocationCent',
  'priceToBeatIvModelBookDislocationRedCent',
  'priceToBeatIvExecutionVwapCostGuardEnabled',
  'priceToBeatIvExecutionVwapRequiredOnHighDislocation',
  'priceToBeatIvExecutionLimitByVwapEnabled',
  'priceToBeatIvExecutionVwapMaxSlippageCent',
  'priceToBeatIvBorderlinePumpBookLeadGuardEnabled',
  'priceToBeatIvBorderlineGapMarginEarly',
  'priceToBeatIvBorderlinePumpShockRatio',
  'priceToBeatIvBorderlineBookLeadQMinCent',
  'priceToBeatIvBorderlineBookLeadCheapTokenCent',
  'priceToBeatIvBorderlineBookLeadDislocationCent',
  'priceToBeatIvPumpShockGuardEnabled',
  'priceToBeatIvPumpShockGapGrowthRatio',
  'priceToBeatIvPumpShockHardRatio',
  'priceToBeatIvPumpShockMinHoldMs',
  'priceToBeatIvPumpShockMinBufferRetain',
  'priceToBeatIvPtbChopGuardEnabled',
  'priceToBeatIvPtbChopLookbackSeconds',
  'priceToBeatIvPtbChopExtendedLookbackSeconds',
  'priceToBeatIvPtbChopDeadbandBps',
  'priceToBeatIvPtbChopDeadbandMinUsdBtc',
  'priceToBeatIvPtbChopDeadbandMinUsdEth',
  'priceToBeatIvPtbChopDeadbandMinUsdSol',
  'priceToBeatIvPtbChopZeroCrossBlock10s',
  'priceToBeatIvPtbChopZeroCrossBlock15s',
  'priceToBeatIvPtbChopPathZWarn',
  'priceToBeatIvPtbChopPathZBlock',
  'priceToBeatIvPtbChopEfficiencyWarn',
  'priceToBeatIvPtbChopEfficiencyBlock',
  'priceToBeatIvPtbChopOppositeDepthZWarn',
  'priceToBeatIvPtbChopOppositeDepthZBlock',
  'priceToBeatIvPtbChopMaxGapStrengthPenalty',
  'priceToBeatIvEntryQualityChainlinkMaxAgeMs',
  'priceToBeatIvEntryQualityHighRiskUnderSec',
  'priceToBeatIvEntryQualityHighRiskAskCent',
  'priceToBeatIvEntryQualityHighPriceMaxSpreadCent',
  'priceToBeatIvEntryQualityMaxSpreadCent',
  'priceToBeatIvEntryQualityNeutralEdgePenalty',
  'priceToBeatIvEntryQualityNeutralGapStrengthPenalty',
  'priceToBeatIvEntryQualityStaleEdgePenalty',
  'priceToBeatIvEntryQualityStaleGapStrengthPenalty',
  'priceToBeatIvStalePenaltyMs',
  'priceToBeatIvStaleGapStrengthPenaltyMs',
  'priceToBeatIvStaleGapStrengthPenalty',
  'priceToBeatIvNegativeVelocityGapStrengthPenalty',
  'priceToBeatIvBinanceMissingAskThresholdCent',
  'priceToBeatIvBinanceMissingPenalty',
  'priceToBeatIvMinAdjustedMargin',
  'priceToBeatIvMinFinalQ',
  'priceToBeatIvBinanceDisagreementThreshold',
  'priceToBeatIvBinanceDisagreementPenalty',
  'priceToBeatIvLargeBinanceDisagreementThreshold',
  'priceToBeatIvLargeBinanceDisagreementPenalty',
  'priceToBeatIvProtectionMode',
  'priceToBeatIvBookLeadGuardEnabled',
  'priceToBeatIvBookLeadUnderSec',
  'priceToBeatIvBookLeadMinMidDiff',
  'priceToBeatIvOppositeMidBlockCent',
  'priceToBeatIvBlockOnOppositeBookLead',
  'priceToBeatIvTooGoodToBeTrueGap',
  'priceToBeatIvModelBookGapWarn',
  'priceToBeatIvModelBookGapHard',
  'priceToBeatIvModelBookWarnThresholdPenalty',
  'priceToBeatIvModelBookWarnGapPenalty',
  'priceToBeatIvDepthGuardEnabled',
  'priceToBeatIvDepthGuardHardBlockEnabled',
  'priceToBeatIvDepthMaxSlippage',
  'priceToBeatIvLateUnconfirmedUnderSec',
  'priceToBeatIvLateUnconfirmedMinSelectedMid',
  'priceToBeatIvLateUnconfirmedMinQ',
  'priceToBeatIvLateHighPriceSoftUnderSec',
  'priceToBeatIvLateHighPriceAskCent',
  'priceToBeatIvLateHighPriceSelectedMidSoftCent',
  'priceToBeatIvLateHighPriceThresholdPenalty',
  'priceToBeatIvLateHighPriceSelectedMidHardCent',
  'priceToBeatIvLateHighPriceMinGapUsd',
  'priceToBeatIvParticipationCreditEnabled',
  'priceToBeatIvParticipationAfterMinutes',
  'priceToBeatIvParticipationLongAfterMinutes',
  'priceToBeatIvParticipationCredit',
  'priceToBeatIvParticipationLongCredit',
  'priceToBeatIvParticipationMinThreshold',
  'priceToBeatIvRequireBinanceFreshUnderSec',
  'priceToBeatIvBinanceMaxStaleMs',
  'priceToBeatIvRequireBinanceSameDirection',
  'priceToBeatIvMomentumProtectionEnabled',
  'priceToBeatIvDropZBlockThreshold',
  'priceToBeatIvProtectionSoftThresholdPenalty',
  'priceToBeatIvProtectionSoftGapStrengthPenalty',
  'priceToBeatIvVolumeBaselineMode',
  'priceToBeatIvVolumeBaselineLookbackDays',
  'priceToBeatIvVolumeWindowSec',
  'priceToBeatIvVolumeBaselineMinSamples',
  'priceToBeatIvLowHourlyVolumeRatio',
  'priceToBeatIvHighHourlyVolumeRatio',
  'priceToBeatIvExtremeHourlyVolumeRatio',
  'priceToBeatIvBookReliabilityThreshold',
  'priceToBeatIvAdaptiveGreenEdgeDelta',
  'priceToBeatIvAdaptiveGreenGapStrengthDelta',
  'priceToBeatIvAdaptiveOrangeEdgeDelta',
  'priceToBeatIvAdaptiveOrangeGapStrengthDelta',
  'priceToBeatIvAdaptiveOrangeGapUsdMarginDelta',
  'priceToBeatIvAdaptiveRedBlock',
] as const;

export function parsePtbIvTimeRuleRows(cfg: Record<string, unknown>): PtbIvTimeRuleRow[] {
  if (!Array.isArray(cfg.priceToBeatIvTimeRules)) return [];
  return cfg.priceToBeatIvTimeRules
    .filter(isRecord)
    .map((item) => ({
      ...createEmptyPtbIvTimeRuleRow(),
      startRemainingSec: toStringValue(item.startRemainingSec ?? item.start_remaining_secs),
      endRemainingSec: toStringValue(item.endRemainingSec ?? item.end_remaining_secs),
      maxPriceCent: toCentStringValue(
        item.maxPriceCent ?? item.max_price_cent,
        item.maxPrice ?? item.max_price
      ),
      minEdge: toStringValue(item.minEdge ?? item.min_edge),
      minGapStrength: toStringValue(item.minGapStrength ?? item.min_gap_strength),
      minExpectedMoveUsd: toStringValue(
        item.minExpectedMoveUsd ?? item.min_expected_move_usd
      ),
      minGapStrengthMargin: toStringValue(
        item.minGapStrengthMargin ?? item.min_gap_strength_margin
      ),
      minGapUsdMargin: toStringValue(item.minGapUsdMargin ?? item.min_gap_usd_margin),
    }));
}

export function copyPtbIvConfigFields(
  fields: Record<string, string>,
  cfg: Record<string, unknown>
): void {
  for (const key of PTB_IV_CONFIG_KEYS) {
    if (key === 'priceToBeatIvTimeRules') continue;
    if (cfg[key] != null) fields[key] = toStringValue(cfg[key]);
  }
}

export function buildPtbIvTimeRules(rows: PtbIvTimeRuleRow[]): SerializedPtbIvTimeRule[] {
  return rows
    .map((row) => {
      const startRemainingSec = Number(row.startRemainingSec.trim());
      const endRemainingSec = Number(row.endRemainingSec.trim());
      const maxPriceCentRaw = row.maxPriceCent.trim();
      const maxPriceCent = maxPriceCentRaw ? Number(maxPriceCentRaw) : undefined;
      const minEdge = Number(row.minEdge.trim());
      const minGapStrength = Number(row.minGapStrength.trim());
      const minExpectedMoveUsdRaw = row.minExpectedMoveUsd.trim();
      const minExpectedMoveUsd = minExpectedMoveUsdRaw
        ? Number(minExpectedMoveUsdRaw)
        : undefined;
      const minGapStrengthMarginRaw = row.minGapStrengthMargin.trim();
      const minGapStrengthMargin = minGapStrengthMarginRaw
        ? Number(minGapStrengthMarginRaw)
        : undefined;
      const minGapUsdMarginRaw = row.minGapUsdMargin.trim();
      const minGapUsdMargin = minGapUsdMarginRaw ? Number(minGapUsdMarginRaw) : undefined;
      if (!Number.isFinite(startRemainingSec) || startRemainingSec <= 0) return null;
      if (!Number.isFinite(endRemainingSec) || endRemainingSec < 0) return null;
      if (startRemainingSec <= endRemainingSec) return null;
      if (maxPriceCentRaw && (maxPriceCent == null || !Number.isFinite(maxPriceCent) || maxPriceCent <= 0 || maxPriceCent > 100)) {
        return null;
      }
      if (!Number.isFinite(minEdge) || minEdge < 0 || minEdge > 1) return null;
      if (!Number.isFinite(minGapStrength) || minGapStrength < 0) return null;
      if (
        minExpectedMoveUsdRaw &&
        (minExpectedMoveUsd == null || !Number.isFinite(minExpectedMoveUsd) || minExpectedMoveUsd <= 0)
      ) {
        return null;
      }
      if (
        minGapStrengthMarginRaw &&
        (minGapStrengthMargin == null || !Number.isFinite(minGapStrengthMargin) || minGapStrengthMargin < 0)
      ) {
        return null;
      }
      if (
        minGapUsdMarginRaw &&
        (minGapUsdMargin == null || !Number.isFinite(minGapUsdMargin) || minGapUsdMargin < 0)
      ) {
        return null;
      }
      return {
        startRemainingSec,
        endRemainingSec,
        ...(maxPriceCent != null ? { maxPriceCent } : {}),
        minEdge,
        minGapStrength,
        ...(minExpectedMoveUsd != null ? { minExpectedMoveUsd } : {}),
        ...(minGapStrengthMargin != null ? { minGapStrengthMargin } : {}),
        ...(minGapUsdMargin != null ? { minGapUsdMargin } : {}),
      };
    })
    .filter((item): item is SerializedPtbIvTimeRule => item != null);
}

export function clearPtbIvTimeRuleBuildConfig(config: Record<string, unknown>): void {
  for (const key of PTB_IV_CONFIG_KEYS) delete config[key];
}

export function normalizePtbIvTimeRuleBuildConfig(
  config: Record<string, unknown>,
  form: NodeConfigFormState
): void {
  if (config.priceToBeatGuardEnabled !== true || config.priceToBeatMode !== 'iv_mismatch_edge') {
    clearPtbIvTimeRuleBuildConfig(config);
    return;
  }

  for (const key of PTB_IV_CONFIG_KEYS) {
    if (key === 'priceToBeatIvTimeRules') continue;
    const raw = toStringValue(form.fields[key]).trim();
    if (raw) config[key] = raw;
  }

  const rules = buildPtbIvTimeRules(form.ptbIvTimeRuleRows || []);
  if (rules.length > 0) {
    config.priceToBeatIvTimeRules = rules;
  } else if (config.mode === REVENGE_FLIP_MODE) {
    config.priceToBeatIvTimeRules = REVENGE_FLIP_DEFAULT_IV_TIME_RULES.map((rule) => ({
      ...rule,
    }));
  } else {
    delete config.priceToBeatIvTimeRules;
  }

  const staleMs = Number(toStringValue(config.priceToBeatIvStalePenaltyMs).trim());
  if (Number.isInteger(staleMs) && staleMs >= 0) {
    config.priceToBeatIvStalePenaltyMs = staleMs;
  } else {
    delete config.priceToBeatIvStalePenaltyMs;
  }

  const stalePenalty = Number(toStringValue(config.priceToBeatIvStaleGapStrengthPenalty).trim());
  if (Number.isFinite(stalePenalty) && stalePenalty >= 0) {
    config.priceToBeatIvStaleGapStrengthPenalty = stalePenalty;
  } else {
    delete config.priceToBeatIvStaleGapStrengthPenalty;
  }

  const velocityPenalty = Number(
    toStringValue(config.priceToBeatIvNegativeVelocityGapStrengthPenalty).trim()
  );
  if (Number.isFinite(velocityPenalty) && velocityPenalty >= 0) {
    config.priceToBeatIvNegativeVelocityGapStrengthPenalty = velocityPenalty;
  } else {
    delete config.priceToBeatIvNegativeVelocityGapStrengthPenalty;
  }

  const binanceMissingAskThresholdCent = Number(
    toStringValue(config.priceToBeatIvBinanceMissingAskThresholdCent).trim()
  );
  if (
    Number.isFinite(binanceMissingAskThresholdCent) &&
    binanceMissingAskThresholdCent > 0 &&
    binanceMissingAskThresholdCent <= 100
  ) {
    config.priceToBeatIvBinanceMissingAskThresholdCent = binanceMissingAskThresholdCent;
  } else {
    delete config.priceToBeatIvBinanceMissingAskThresholdCent;
  }

  const binanceMissingPenalty = Number(
    toStringValue(config.priceToBeatIvBinanceMissingPenalty).trim()
  );
  if (Number.isFinite(binanceMissingPenalty) && binanceMissingPenalty >= 0) {
    config.priceToBeatIvBinanceMissingPenalty = binanceMissingPenalty;
  } else {
    delete config.priceToBeatIvBinanceMissingPenalty;
  }

  normalizeOptionalProbability(config, 'priceToBeatIvMinAdjustedMargin', true);
  normalizeOptionalProbability(config, 'priceToBeatIvMinFinalQ', false);
  normalizeOptionalProbability(config, 'priceToBeatIvBinanceDisagreementThreshold', false);
  normalizeOptionalProbability(config, 'priceToBeatIvBinanceDisagreementPenalty', true);
  normalizeOptionalProbability(config, 'priceToBeatIvLargeBinanceDisagreementThreshold', false);
  normalizeOptionalProbability(config, 'priceToBeatIvLargeBinanceDisagreementPenalty', true);
  normalizeProtectionMode(config);
  normalizeOptionalBoolean(config, 'priceToBeatIvBookLeadGuardEnabled');
  normalizeOptionalNumber(config, 'priceToBeatIvBookLeadUnderSec', 0);
  normalizeOptionalProbability(config, 'priceToBeatIvBookLeadMinMidDiff', true);
  normalizeOptionalCent(config, 'priceToBeatIvOppositeMidBlockCent');
  normalizeOptionalBoolean(config, 'priceToBeatIvBlockOnOppositeBookLead');
  normalizeOptionalProbability(config, 'priceToBeatIvTooGoodToBeTrueGap', true);
  normalizeOptionalProbability(config, 'priceToBeatIvModelBookGapWarn', true);
  normalizeOptionalProbability(config, 'priceToBeatIvModelBookGapHard', true);
  normalizeOptionalProbability(config, 'priceToBeatIvModelBookWarnThresholdPenalty', true);
  normalizeOptionalNumber(config, 'priceToBeatIvModelBookWarnGapPenalty', 0);
  normalizeOptionalBoolean(config, 'priceToBeatIvDepthGuardEnabled');
  normalizeOptionalBoolean(config, 'priceToBeatIvDepthGuardHardBlockEnabled');
  normalizeOptionalProbability(config, 'priceToBeatIvDepthMaxSlippage', true);
  normalizeOptionalNumber(config, 'priceToBeatIvLateHighPriceSoftUnderSec', 0);
  normalizeOptionalCent(config, 'priceToBeatIvLateHighPriceAskCent');
  normalizeOptionalCent(config, 'priceToBeatIvLateHighPriceSelectedMidSoftCent');
  normalizeOptionalProbability(config, 'priceToBeatIvLateHighPriceThresholdPenalty', true);
  normalizeOptionalCent(config, 'priceToBeatIvLateHighPriceSelectedMidHardCent');
  normalizeOptionalNumber(config, 'priceToBeatIvLateHighPriceMinGapUsd', 0);
  normalizeOptionalBoolean(config, 'priceToBeatIvParticipationCreditEnabled');
  normalizeOptionalNumber(config, 'priceToBeatIvParticipationAfterMinutes', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvParticipationLongAfterMinutes', 0);
  normalizeOptionalProbability(config, 'priceToBeatIvParticipationCredit', true);
  normalizeOptionalProbability(config, 'priceToBeatIvParticipationLongCredit', true);
  normalizeOptionalProbability(config, 'priceToBeatIvParticipationMinThreshold', true);
  delete config.priceToBeatIvLateUnconfirmedUnderSec;
  delete config.priceToBeatIvLateUnconfirmedMinSelectedMid;
  delete config.priceToBeatIvLateUnconfirmedMinQ;
  normalizeOptionalNumber(config, 'priceToBeatIvRequireBinanceFreshUnderSec', 0);
  normalizeOptionalInteger(config, 'priceToBeatIvBinanceMaxStaleMs', 0);
  normalizeOptionalBoolean(config, 'priceToBeatIvRequireBinanceSameDirection');
  normalizeOptionalBoolean(config, 'priceToBeatIvMomentumProtectionEnabled');
  normalizeOptionalNumber(config, 'priceToBeatIvDropZBlockThreshold', 0);
  normalizeOptionalProbability(config, 'priceToBeatIvProtectionSoftThresholdPenalty', true);
  normalizeOptionalNumber(config, 'priceToBeatIvProtectionSoftGapStrengthPenalty', 0);
  normalizeVolumeBaselineMode(config);
  normalizeOptionalInteger(config, 'priceToBeatIvVolumeBaselineLookbackDays', 1);
  normalizeOptionalInteger(config, 'priceToBeatIvVolumeWindowSec', 1);
  normalizeOptionalInteger(config, 'priceToBeatIvVolumeBaselineMinSamples', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvLowHourlyVolumeRatio', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvHighHourlyVolumeRatio', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvExtremeHourlyVolumeRatio', 0);
  normalizeOptionalProbability(config, 'priceToBeatIvBookReliabilityThreshold', true);
  normalizeOptionalFiniteNumber(config, 'priceToBeatIvAdaptiveGreenEdgeDelta');
  normalizeOptionalFiniteNumber(config, 'priceToBeatIvAdaptiveGreenGapStrengthDelta');
  normalizeOptionalFiniteNumber(config, 'priceToBeatIvAdaptiveOrangeEdgeDelta');
  normalizeOptionalFiniteNumber(config, 'priceToBeatIvAdaptiveOrangeGapStrengthDelta');
  normalizeOptionalFiniteNumber(config, 'priceToBeatIvAdaptiveOrangeGapUsdMarginDelta');
  normalizeOptionalBoolean(config, 'priceToBeatIvAdaptiveRedBlock');
  normalizeOptionalBoolean(config, 'priceToBeatIvPtbChopGuardEnabled');
  normalizeOptionalInteger(config, 'priceToBeatIvPtbChopLookbackSeconds', 1);
  normalizeOptionalInteger(config, 'priceToBeatIvPtbChopExtendedLookbackSeconds', 1);
  normalizeOptionalNumber(config, 'priceToBeatIvPtbChopDeadbandBps', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvPtbChopDeadbandMinUsdBtc', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvPtbChopDeadbandMinUsdEth', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvPtbChopDeadbandMinUsdSol', 0);
  normalizeOptionalInteger(config, 'priceToBeatIvPtbChopZeroCrossBlock10s', 0);
  normalizeOptionalInteger(config, 'priceToBeatIvPtbChopZeroCrossBlock15s', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvPtbChopPathZWarn', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvPtbChopPathZBlock', 0);
  normalizeOptionalProbability(config, 'priceToBeatIvPtbChopEfficiencyWarn', true);
  normalizeOptionalProbability(config, 'priceToBeatIvPtbChopEfficiencyBlock', true);
  normalizeOptionalNumber(config, 'priceToBeatIvPtbChopOppositeDepthZWarn', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvPtbChopOppositeDepthZBlock', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvPtbChopMaxGapStrengthPenalty', 0);
  normalizeOptionalBoolean(config, 'priceToBeatIvEntryQualityPolicy');
  normalizeOptionalCent(config, 'priceToBeatIvNormalMaxPriceCent');
  normalizeOptionalCent(config, 'priceToBeatIvPremiumMaxPriceCent');
  normalizeOptionalNumber(config, 'priceToBeatIvNoNewEntryBelowSeconds', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvMinExpectedMoveBps', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvMinExpectedMoveUsd', 0);
  normalizeMinExpectedMoveMode(config);
  normalizeOptionalNumber(config, 'priceToBeatIvAdaptiveMinExpectedMoveBaseBps', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvAdaptiveMinExpectedMoveMinBps', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvAdaptiveMinExpectedMoveMaxBps', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvAdaptiveDisagreementBpsAdd', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvAdaptiveStrongDisagreementBpsAdd', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvAdaptiveSpreadBpsAdd', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvAdaptiveWideSpreadBpsAdd', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvAdaptiveStaleBpsAdd', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvAdaptiveNoiseBpsAdd', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvGapStrengthMin60To45', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvGapStrengthMin45To25', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvGapStrengthMin25To10', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvGapStrengthMin10To8', 0);
  normalizeOptionalBoolean(config, 'priceToBeatIvBufferTrendGuardEnabled');
  normalizeOptionalProbability(config, 'priceToBeatIvBufferRetain5s', true);
  normalizeOptionalProbability(config, 'priceToBeatIvBufferRetain10s', true);
  normalizeOptionalProbability(config, 'priceToBeatIvPremiumBufferRetain5s', true);
  normalizeOptionalProbability(config, 'priceToBeatIvPremiumBufferRetain10s', true);
  normalizeOptionalBoolean(config, 'priceToBeatIvSpikeFadeGuardEnabled');
  normalizeOptionalGreaterThan(config, 'priceToBeatIvSpikeMultiplier', 1);
  normalizeOptionalProbability(config, 'priceToBeatIvSpikeRetraceRatio', true);
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvPremiumMaxSpreadCent');
  normalizeOptionalInteger(config, 'priceToBeatIvPremiumMaxChainlinkAgeMs', 1);
  normalizeOptionalNumber(config, 'priceToBeatIvCexAlignMaxUsd', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvCexAlignMaxBps', 0);
  normalizeOptionalBoolean(config, 'priceToBeatIvCexOpenGapConsensusGuardEnabled');
  normalizeOptionalNumber(config, 'priceToBeatIvCexOpenGapMinUsd', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvCexOpenGapMinZ', 0);
  normalizeOptionalInteger(config, 'priceToBeatIvCexOpenGapMaxStaleMs', 0);
  normalizeOptionalBoolean(config, 'priceToBeatIvCexOpenGapApplyNegativeConservativeCap');
  normalizeOptionalBoolean(config, 'priceToBeatIvChainlinkCexLagGuardEnabled');
  normalizeOptionalNumber(config, 'priceToBeatIvChainlinkCexDiffZBlock', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvChainlinkCexMaxDiffUsd', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvChainlinkCexMaxDiffBps', 0);
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvChainlinkCexBookMismatchDislocationCent');
  normalizeOptionalBoolean(config, 'priceToBeatIvGapFailMixedCexGuardEnabled');
  normalizeOptionalNumber(config, 'priceToBeatIvGapFailMixedCexMaxSeconds', 0);
  normalizeOptionalBoolean(config, 'priceToBeatIvLateExpensiveMixedCexGuardEnabled');
  normalizeOptionalNumber(config, 'priceToBeatIvLateExpensiveMixedCexSeconds', 0);
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvLateExpensiveMixedCexMinVwapCent');
  normalizeOptionalBoolean(config, 'priceToBeatIvLateExpensiveMixedCexRequireGapFailOrLagHigh');
  normalizeOptionalBoolean(config, 'priceToBeatIvChainlinkCexLagNoBookGuardEnabled');
  normalizeOptionalNumber(config, 'priceToBeatIvChainlinkCexLagNoBookMaxSeconds', 0);
  normalizeOptionalBoolean(config, 'priceToBeatIvChainlinkCexLagNoBookRequireNonStrongCex');
  normalizeOptionalBoolean(config, 'priceToBeatIvEq77RiskCapEnabled');
  normalizeOptionalNumber(config, 'priceToBeatIvRiskScoreCleanMax', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvRiskScoreModerateMax', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvRiskScoreHighMax', 0);
  normalizeOptionalCent(config, 'priceToBeatIvModerateRiskMaxPriceCent');
  normalizeOptionalCent(config, 'priceToBeatIvHighRiskMaxPriceCent');
  normalizeOptionalCent(config, 'priceToBeatIvDeepValueMaxPriceCent');
  normalizeOptionalNumber(config, 'priceToBeatIvMaxRiskHaircutCent', 0);
  normalizeOptionalBoolean(config, 'priceToBeatIvWaitForPriceEnabled');
  normalizeOptionalBoolean(config, 'priceToBeatIvRecheckBeforeSubmit');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvOddsMaxSpreadCent');
  normalizeOptionalNumber(config, 'priceToBeatIvCexUnconfirmedRiskPoints', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvCexConflictRiskPoints', 0);
  normalizeOptionalBoolean(config, 'priceToBeatIvPassiveBidEnabled');
  normalizeOptionalInteger(config, 'priceToBeatIvPassiveBidTtlMs', 1);
  normalizeOptionalBoolean(config, 'priceToBeatIvWaitRepriceGuardEnabled');
  normalizeOptionalInteger(config, 'priceToBeatIvWaitMaxAgeMsEarly', 1);
  normalizeOptionalInteger(config, 'priceToBeatIvWaitMaxAgeMsMid', 1);
  normalizeOptionalInteger(config, 'priceToBeatIvWaitMaxAgeMsLate', 1);
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvWaitInitialAskMaxOverCapCent');
  normalizeOptionalBoolean(config, 'priceToBeatIvFallingIntoCapGuardEnabled');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvFallingIntoCapDropCentEarly');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvFallingIntoCapDropCentMid');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvFallingIntoCapDropCentLate');
  normalizeOptionalBoolean(config, 'priceToBeatIvLateExpensiveEntryGuardEnabled');
  normalizeOptionalNumber(config, 'priceToBeatIvLateExpensiveSeconds', 0);
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvLateExpensiveVwapCent');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvLateExpensiveMinQCent');
  normalizeOptionalNumber(config, 'priceToBeatIvLateExpensiveMinGapStrengthExtra', 0);
  normalizeOptionalBoolean(config, 'priceToBeatIvOracleLagBookLeadGuardEnabled');
  normalizeOptionalBoolean(config, 'priceToBeatIvOracleLagUseBestAskFallback');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvOracleLagQHighCent');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvOracleLagCheapTokenCent');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvOracleLagQExtremeCent');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvOracleLagCheapTokenExtremeCent');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvOracleLagConsensusMismatchQCent');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvOracleLagConsensusMismatchCheapTokenCent');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvOracleLagConsensusMismatchDislocationCent');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvModelBookDislocationHighCent');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvModelBookDislocationRedCent');
  normalizeOptionalBoolean(config, 'priceToBeatIvExecutionVwapCostGuardEnabled');
  normalizeOptionalBoolean(config, 'priceToBeatIvExecutionVwapRequiredOnHighDislocation');
  normalizeOptionalBoolean(config, 'priceToBeatIvExecutionLimitByVwapEnabled');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvExecutionVwapMaxSlippageCent');
  normalizeOptionalBoolean(config, 'priceToBeatIvBorderlinePumpBookLeadGuardEnabled');
  normalizeOptionalNumber(config, 'priceToBeatIvBorderlineGapMarginEarly', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvBorderlinePumpShockRatio', 0);
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvBorderlineBookLeadQMinCent');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvBorderlineBookLeadCheapTokenCent');
  normalizeOptionalCentAllowZero(config, 'priceToBeatIvBorderlineBookLeadDislocationCent');
  normalizeOptionalBoolean(config, 'priceToBeatIvPumpShockGuardEnabled');
  normalizeOptionalNumber(config, 'priceToBeatIvPumpShockGapGrowthRatio', 0);
  normalizeOptionalNumber(config, 'priceToBeatIvPumpShockHardRatio', 0);
  normalizeOptionalInteger(config, 'priceToBeatIvPumpShockMinHoldMs', 0);
  normalizeOptionalProbability(config, 'priceToBeatIvPumpShockMinBufferRetain', true);
  normalizeOptionalInteger(config, 'priceToBeatIvEntryQualityChainlinkMaxAgeMs', 1);
  normalizeOptionalNumber(config, 'priceToBeatIvEntryQualityHighRiskUnderSec', 0);
  normalizeOptionalCent(config, 'priceToBeatIvEntryQualityHighRiskAskCent');
  normalizeOptionalCent(config, 'priceToBeatIvEntryQualityHighPriceMaxSpreadCent');
  normalizeOptionalCent(config, 'priceToBeatIvEntryQualityMaxSpreadCent');
  normalizeOptionalProbability(config, 'priceToBeatIvEntryQualityNeutralEdgePenalty', true);
  normalizeOptionalNumber(config, 'priceToBeatIvEntryQualityNeutralGapStrengthPenalty', 0);
  normalizeOptionalProbability(config, 'priceToBeatIvEntryQualityStaleEdgePenalty', true);
  normalizeOptionalNumber(config, 'priceToBeatIvEntryQualityStaleGapStrengthPenalty', 0);

  delete config.priceToBeatIvStaleGapStrengthPenaltyMs;
}

function normalizeProtectionMode(config: Record<string, unknown>): void {
  const value = toStringValue(config.priceToBeatIvProtectionMode).trim().toLowerCase();
  if (
    value === 'off' ||
    value === 'soft' ||
    value === 'balanced' ||
    value === 'hard' ||
    value === 'adaptive'
  ) {
    config.priceToBeatIvProtectionMode = value;
  } else {
    delete config.priceToBeatIvProtectionMode;
  }
}

function normalizeVolumeBaselineMode(config: Record<string, unknown>): void {
  const value = toStringValue(config.priceToBeatIvVolumeBaselineMode).trim().toLowerCase();
  if (value === 'off' || value === 'hourly') {
    config.priceToBeatIvVolumeBaselineMode = value;
  } else {
    delete config.priceToBeatIvVolumeBaselineMode;
  }
}

function normalizeMinExpectedMoveMode(config: Record<string, unknown>): void {
  const value = toStringValue(config.priceToBeatIvMinExpectedMoveMode).trim().toLowerCase();
  if (value === 'fixed' || value === 'adaptive') {
    config.priceToBeatIvMinExpectedMoveMode = value;
  } else {
    delete config.priceToBeatIvMinExpectedMoveMode;
  }
}

function normalizeOptionalBoolean(config: Record<string, unknown>, key: string): void {
  const value = config[key];
  if (value === true || value === 'true') {
    config[key] = true;
  } else if (value === false || value === 'false') {
    config[key] = false;
  } else {
    delete config[key];
  }
}

function normalizeOptionalNumber(
  config: Record<string, unknown>,
  key: string,
  minValue: number
): void {
  const value = Number(toStringValue(config[key]).trim());
  if (Number.isFinite(value) && value >= minValue) {
    config[key] = value;
  } else {
    delete config[key];
  }
}

function normalizeOptionalInteger(
  config: Record<string, unknown>,
  key: string,
  minValue: number
): void {
  const value = Number(toStringValue(config[key]).trim());
  if (Number.isInteger(value) && value >= minValue) {
    config[key] = value;
  } else {
    delete config[key];
  }
}

function normalizeOptionalCent(config: Record<string, unknown>, key: string): void {
  const value = Number(toStringValue(config[key]).trim());
  if (Number.isFinite(value) && value > 0 && value <= 100) {
    config[key] = value;
  } else {
    delete config[key];
  }
}

function normalizeOptionalCentAllowZero(config: Record<string, unknown>, key: string): void {
  const value = Number(toStringValue(config[key]).trim());
  if (Number.isFinite(value) && value >= 0 && value <= 100) {
    config[key] = value;
  } else {
    delete config[key];
  }
}

function normalizeOptionalFiniteNumber(config: Record<string, unknown>, key: string): void {
  const value = Number(toStringValue(config[key]).trim());
  if (Number.isFinite(value)) {
    config[key] = value;
  } else {
    delete config[key];
  }
}

function normalizeOptionalGreaterThan(
  config: Record<string, unknown>,
  key: string,
  minExclusive: number
): void {
  const value = Number(toStringValue(config[key]).trim());
  if (Number.isFinite(value) && value > minExclusive) {
    config[key] = value;
  } else {
    delete config[key];
  }
}

function normalizeOptionalProbability(
  config: Record<string, unknown>,
  key: string,
  allowZero: boolean
): void {
  const value = Number(toStringValue(config[key]).trim());
  const lowerOk = allowZero ? value >= 0 : value > 0;
  if (Number.isFinite(value) && lowerOk && value <= 1) {
    config[key] = value;
  } else {
    delete config[key];
  }
}
