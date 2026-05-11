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

const PTB_IV_CONFIG_KEYS = [
  'priceToBeatIvTimeRules',
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

  const rules = buildPtbIvTimeRules(form.ptbIvTimeRuleRows || []);
  if (rules.length > 0) {
    config.priceToBeatIvTimeRules = rules;
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

function normalizeOptionalFiniteNumber(config: Record<string, unknown>, key: string): void {
  const value = Number(toStringValue(config[key]).trim());
  if (Number.isFinite(value)) {
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
