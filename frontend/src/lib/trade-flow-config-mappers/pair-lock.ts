import { toStringValue } from './utils';
import { normalizePtbStopLossGapUnit } from './ptb-stop-loss';
import { normalizePtbMode } from './ptb-modes';

const ADAPTIVE_MAX_PRICE_CONFIG_KEYS = [
  'adaptiveMaxPriceMissCount',
  'adaptiveMaxPriceRequiredGoodMissCount',
  'adaptiveMaxPriceRelaxCreditCent',
  'adaptiveMaxPriceMaxRelaxCreditCent',
  'adaptiveMaxPriceHardCapCent',
  'adaptiveMaxPriceExtraBufferCent',
  'adaptiveMaxPricePairBufferCent',
  'adaptiveMaxPriceSizeMultiplier',
  'adaptiveMaxPriceWindowStartSec',
  'adaptiveMaxPriceWindowEndSec',
  'adaptiveMaxPriceLateRelaxCutoffS',
  'adaptiveMaxPriceLateRiskEnabled',
  'adaptiveMaxPriceLateRiskAfterSec',
  'adaptiveMaxPriceLateExtraBufferCent',
  'adaptiveMaxPriceLateSizeMultiplier',
  'adaptiveMaxPriceSlCooldownMarkets',
  'notifyOnAdaptiveMaxPriceEvaluated',
  'notifyOnAdaptiveMaxPriceRelax',
  'notifyOnAdaptiveMaxPriceRelaxSl',
  'notifyOnAdaptiveMaxPriceNoRelaxImportant',
  'notifyOnAdaptiveMaxPriceMissResolved',
  'notifyOnAdaptiveMaxPriceCooldown',
  'notifyOnAdaptiveMaxPriceSummary',
  'notifyOnAdaptiveMaxPriceAllNoRelax',
  'adaptiveMaxPriceNotifyMinIntervalSec',
  'adaptiveMaxPriceNotifyIncludePayload',
  'adaptiveMaxPriceSummaryEveryMarkets',
] as const;

export const PAIR_LOCK_CONFIG_KEYS = [
  'pairLockStrategy',
  'pairLockDecisionQty',
  'pairLockSingleEdgeThreshold',
  'pairLockCostBuffer',
  ...ADAPTIVE_MAX_PRICE_CONFIG_KEYS,
  'biasedHedge',
  'biasedHedgeStop',
  'biasedHedgePrimaryBudgetUsdc',
  'biasedHedgeHedgeBudgetUsdc',
  'biasedHedgeMinDominantShare',
  'biasedHedgeMaxHedgeSpendRatio',
  'biasedHedgePrimaryMinEdge',
  'biasedHedgePrimaryMinFinalQ',
  'biasedHedgeMaxPriceCent',
  'biasedHedgeHighPriceCent',
  'biasedHedgeHighPriceMinFinalQ',
  'biasedHedgeHighPriceMinEdge',
  'biasedHedgeHedgeOnlyIfPrimaryFilled',
  'biasedHedgeHedgeMaxPriceCent',
  'biasedHedgeHedgeMinPriceCent',
  'biasedHedgeDisableNewPrimaryAfterSec',
  'biasedHedgeDisableAnyBuyAfterSec',
  'biasedHedgeMaxSideSwitchCount',
  'biasedHedgeMaxPairedEffectiveCostCent',
  'biasedHedgeStopBiasInvalidationEnabled',
  'biasedHedgeStopMinQFinalToHold',
  'biasedHedgeStopMinEdgeToHold',
  'biasedHedgeStopExitPctOnInvalidation',
  'biasedHedgeStopPtbStopLossEnabled',
  'biasedHedgeStopPtbStopLossGapUsd',
  'biasedHedgeStopPtbStopLossTimeDecayMode',
  'biasedHedgeStopTimeExitRulesJson',
  'pairMaxTotalCent',
  'pairTargetTotalCent',
  'pairSizingMode',
  'pairTotalBudgetUsdc',
  'pairMinNetProfitUsdc',
  'pairProfitSafetyBufferUsdc',
  'pairOrphanGraceMs',
  'pairProtectiveUnwindEnabled',
  'pairIgnoreStopLossAfterLocked',
  'notifyOnPairLocked',
  'notifyOnPairUnwind',
  'notifyOnPairNoEdge',
  'counterLegEnabled',
  'counterLegSizeUsdc',
  'counterLegOutcomeLabel',
  'counterLegTriggerCondition',
  'counterLegTriggerPriceCent',
  'counterLegMaxPriceCent',
  'counterLegPriceToBeatGuardEnabled',
  'counterLegPriceToBeatMode',
  'counterLegPriceToBeatMaxDiff',
  'counterLegPriceToBeatMaxDiffUnit',
  'counterLegExecutionFloorGuardEnabled',
  'counterLegExecutionFloorPriceCent',
  'counterLegRetryOnPriceToBeatGuardBlock',
  'counterLegRetryOnExecutionFloorGuardBlock',
  'counterLegRetryOnMaxPriceBlock',
  'counterLegTpEnabled',
  'counterLegTpPriceCent',
  'counterLegTpRules',
  'counterLegNotifyOnTpHit',
  'counterLegSlEnabled',
  'counterLegSlPriceCent',
  'counterLegSlTriggerPriceMode',
  'counterLegPtbStopLossEnabled',
  'counterLegPtbStopLossGapUsd',
  'counterLegPtbStopLossGapUnit',
  'counterLegPtbStopLossTimeDecayMode',
  'counterLegNotifyOnSlHit',
] as const;

export const PAIR_LOCK_SUPPORTED_STOP_LOSS_FIELD_KEYS = [
  'slEnabled',
  'slPriceCent',
  'slTriggerPriceMode',
  'ptbStopLossEnabled',
  'ptbStopLossGapUsd',
  'ptbStopLossGapUnit',
  'ptbStopLossTimeDecayMode',
  'notifyOnSlHit',
  'reenterOnSlHit',
  'reentryMaxAttempts',
  'reentryCooldownSec',
  'counterLegSlEnabled',
  'counterLegSlPriceCent',
  'counterLegSlTriggerPriceMode',
  'counterLegPtbStopLossEnabled',
  'counterLegPtbStopLossGapUsd',
  'counterLegPtbStopLossGapUnit',
  'counterLegPtbStopLossTimeDecayMode',
  'counterLegNotifyOnSlHit',
] as const;

export const PAIR_LOCK_UNSUPPORTED_EXIT_FIELD_KEYS = [
  'timeExitRules',
  'stagedSlReentryOnlyAfterAllStages',
  'reentryMinPriceCent',
  'reentryMaxPriceCent',
  'reentrySkipCurrentWindow',
  'reentryPriceToBeatMaxDiff',
  'reentryPriceToBeatMaxDiffUnit',
  'reentryThresholdDecay',
  'reentryMaxPriceTightenBps',
  'counterLegSlRules',
  'counterLegPtbStopLossRules',
  'counterLegReenterOnSlHit',
  'counterLegReentryMaxAttempts',
  'counterLegReentryCooldownSec',
] as const;

function applyExplicitCounterStopLossFormDefaults(fields: Record<string, string>): void {
  if (
    (fields.counterLegSlEnabled ?? '').trim().toLowerCase() === 'true' &&
    !(fields.counterLegSlTriggerPriceMode ?? '').trim()
  ) {
    fields.counterLegSlTriggerPriceMode = 'best_bid';
  }
  if (
    (fields.counterLegPtbStopLossEnabled ?? '').trim().toLowerCase() === 'true' &&
    !(fields.counterLegPtbStopLossTimeDecayMode ?? '').trim()
  ) {
    fields.counterLegPtbStopLossTimeDecayMode = 'tighten';
  }
}

export function isPairLockSupportedStopLossField(key: string): boolean {
  return (
    PAIR_LOCK_SUPPORTED_STOP_LOSS_FIELD_KEYS as readonly string[]
  ).includes(key);
}

export function normalizePairLockSizingMode(value: string): 'manual' | 'auto_remaining_budget' {
  return value.trim().toLowerCase() === 'auto_remaining_budget'
    ? 'auto_remaining_budget'
    : 'manual';
}

export type PairLockStrategy =
  | 'legacy'
  | 'edge_pairlock_v1'
  | 'biased_hedge_v1'
  | 'adaptive_max_price_v1';

export function normalizePairLockStrategy(value: string): PairLockStrategy {
  const normalized = value.trim().toLowerCase();
  if (normalized === 'edge_pairlock_v1') return 'edge_pairlock_v1';
  if (normalized === 'biased_hedge_v1') return 'biased_hedge_v1';
  if (normalized === 'adaptive_max_price_v1') return 'adaptive_max_price_v1';
  return 'legacy';
}

function nestedRecord(value: unknown): Record<string, unknown> {
  return typeof value === 'object' && value != null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

function setDefault(fields: Record<string, string>, key: string, value: string): void {
  if (!(fields[key] ?? '').trim()) fields[key] = value;
}

function readNestedField(
  fields: Record<string, string>,
  source: Record<string, unknown>,
  sourceKey: string,
  fieldKey: string,
  fallback?: string
): void {
  const value = toStringValue(source[sourceKey]).trim();
  if (value) fields[fieldKey] = value;
  else if (fallback != null) setDefault(fields, fieldKey, fallback);
}

function parseBiasedHedgeTimeExitRulesJson(value: unknown): Array<{ elapsedSec: number; remainingPct: number }> {
  if (Array.isArray(value)) {
    return value
      .map((item) => nestedRecord(item))
      .map((item) => ({
        elapsedSec: Number(toStringValue(item.elapsedSec).trim()),
        remainingPct: Number(toStringValue(item.remainingPct).trim()),
      }))
      .filter((item) => Number.isFinite(item.elapsedSec) && item.elapsedSec > 0 && Number.isFinite(item.remainingPct) && item.remainingPct >= 0 && item.remainingPct <= 100);
  }
  if (typeof value !== 'string' || !value.trim()) return [];
  try {
    return parseBiasedHedgeTimeExitRulesJson(JSON.parse(value));
  } catch {
    return [];
  }
}

function applyBiasedHedgeFormDefaults(fields: Record<string, string>, cfg: Record<string, unknown>): void {
  const biased = nestedRecord(cfg.biasedHedge);
  const stop = nestedRecord(cfg.biasedHedgeStop);
  fields.priceToBeatGuardEnabled = 'true';
  fields.priceToBeatMode = 'iv_mismatch_edge';
  fields.pairSizingMode = 'manual';
  fields.counterLegEnabled = 'true';
  fields.tpEnabled = 'false';
  fields.pairProtectiveUnwindEnabled = 'true';
  fields.pairOrphanGraceMs = (fields.pairOrphanGraceMs ?? '').trim() || '1500';
  fields.reentryMaxAttempts = (fields.reentryMaxAttempts ?? '').trim() || '0';
  readNestedField(fields, biased, 'primaryBudgetUsdc', 'biasedHedgePrimaryBudgetUsdc', toStringValue(cfg.sizeUsdc).trim() || '2');
  readNestedField(fields, biased, 'hedgeBudgetUsdc', 'biasedHedgeHedgeBudgetUsdc', '0.5');
  readNestedField(fields, biased, 'minDominantShare', 'biasedHedgeMinDominantShare', '0.75');
  readNestedField(fields, biased, 'maxHedgeSpendRatio', 'biasedHedgeMaxHedgeSpendRatio', '0.25');
  readNestedField(fields, biased, 'primaryMinEdge', 'biasedHedgePrimaryMinEdge', '0.08');
  readNestedField(fields, biased, 'primaryMinFinalQ', 'biasedHedgePrimaryMinFinalQ', '0.72');
  readNestedField(fields, biased, 'maxPriceCent', 'biasedHedgeMaxPriceCent', toStringValue(cfg.maxPriceCent).trim() || '75');
  readNestedField(fields, biased, 'highPriceCent', 'biasedHedgeHighPriceCent', '70');
  readNestedField(fields, biased, 'highPriceMinFinalQ', 'biasedHedgeHighPriceMinFinalQ', '0.82');
  readNestedField(fields, biased, 'highPriceMinEdge', 'biasedHedgeHighPriceMinEdge', '0.10');
  fields.biasedHedgeHedgeOnlyIfPrimaryFilled = toStringValue(biased.hedgeOnlyIfPrimaryFilled ?? 'true');
  readNestedField(fields, biased, 'hedgeMinPriceCent', 'biasedHedgeHedgeMinPriceCent', '3');
  readNestedField(fields, biased, 'hedgeMaxPriceCent', 'biasedHedgeHedgeMaxPriceCent', '25');
  readNestedField(fields, biased, 'disableNewPrimaryAfterSec', 'biasedHedgeDisableNewPrimaryAfterSec', '180');
  readNestedField(fields, biased, 'disableAnyBuyAfterSec', 'biasedHedgeDisableAnyBuyAfterSec', '240');
  readNestedField(fields, biased, 'maxSideSwitchCount', 'biasedHedgeMaxSideSwitchCount', '0');
  readNestedField(fields, cfg, 'biasedHedgeMaxPairedEffectiveCostCent', 'biasedHedgeMaxPairedEffectiveCostCent', '95');
  fields.biasedHedgeStopBiasInvalidationEnabled = toStringValue(stop.biasInvalidationEnabled ?? 'true');
  readNestedField(fields, stop, 'minQFinalToHold', 'biasedHedgeStopMinQFinalToHold', '0.55');
  readNestedField(fields, stop, 'minEdgeToHold', 'biasedHedgeStopMinEdgeToHold', '0');
  readNestedField(fields, stop, 'exitPctOnInvalidation', 'biasedHedgeStopExitPctOnInvalidation', '100');
  fields.biasedHedgeStopPtbStopLossEnabled = toStringValue(stop.ptbStopLossEnabled ?? 'true');
  readNestedField(fields, stop, 'ptbStopLossGapUsd', 'biasedHedgeStopPtbStopLossGapUsd', '-3');
  fields.biasedHedgeStopPtbStopLossTimeDecayMode = toStringValue(stop.ptbStopLossTimeDecayMode ?? 'tighten');
  const timeExitRules = parseBiasedHedgeTimeExitRulesJson(stop.timeExitRules);
  fields.biasedHedgeStopTimeExitRulesJson = JSON.stringify(
    timeExitRules.length > 0
      ? timeExitRules
      : [{ elapsedSec: 90, remainingPct: 60 }, { elapsedSec: 150, remainingPct: 0 }]
  );
}

export function applyPairLockFormDefaults(
  fields: Record<string, string>,
  cfg: Record<string, unknown>
): void {
  const modeRaw = toStringValue(cfg.mode).trim().toLowerCase();
  fields.mode = modeRaw === 'pair_lock' ? 'pair_lock' : 'single';
  if (fields.mode !== 'pair_lock') {
    return;
  }

  fields.pairLockStrategy = normalizePairLockStrategy(toStringValue(cfg.pairLockStrategy));
  if (fields.pairLockStrategy === 'edge_pairlock_v1') {
    fields.priceToBeatGuardEnabled = 'true';
    fields.priceToBeatMode = 'iv_mismatch_edge';
    if (!(fields.pairLockDecisionQty ?? '').trim()) fields.pairLockDecisionQty = '5';
    if (!(fields.pairLockSingleEdgeThreshold ?? '').trim()) fields.pairLockSingleEdgeThreshold = '0.10';
    if (!(fields.pairLockCostBuffer ?? '').trim()) fields.pairLockCostBuffer = '0.005';
    if (!(fields.pairMaxTotalCent ?? '').trim()) fields.pairMaxTotalCent = '95';
  } else if (fields.pairLockStrategy === 'adaptive_max_price_v1') {
    fields.priceToBeatGuardEnabled = 'true';
    fields.priceToBeatMode = 'iv_mismatch_edge';
    setDefault(fields, 'adaptiveMaxPriceMissCount', '3');
    setDefault(fields, 'adaptiveMaxPriceRequiredGoodMissCount', '2');
    setDefault(fields, 'adaptiveMaxPriceRelaxCreditCent', '2');
    setDefault(fields, 'adaptiveMaxPriceMaxRelaxCreditCent', '5');
    setDefault(fields, 'adaptiveMaxPriceHardCapCent', '76');
    setDefault(fields, 'adaptiveMaxPriceExtraBufferCent', '1');
    setDefault(fields, 'adaptiveMaxPricePairBufferCent', '1');
    setDefault(fields, 'adaptiveMaxPriceSizeMultiplier', '0.5');
    setDefault(fields, 'adaptiveMaxPriceLateRiskEnabled', 'true');
    setDefault(fields, 'adaptiveMaxPriceLateRiskAfterSec', '210');
    setDefault(fields, 'adaptiveMaxPriceLateExtraBufferCent', '1');
    setDefault(fields, 'adaptiveMaxPriceLateSizeMultiplier', '0.35');
    setDefault(fields, 'adaptiveMaxPriceSlCooldownMarkets', '3');
    setDefault(fields, 'notifyOnAdaptiveMaxPriceEvaluated', 'false');
    setDefault(fields, 'notifyOnAdaptiveMaxPriceRelax', 'true');
    setDefault(fields, 'notifyOnAdaptiveMaxPriceRelaxSl', 'true');
    setDefault(fields, 'notifyOnAdaptiveMaxPriceNoRelaxImportant', 'true');
    setDefault(fields, 'notifyOnAdaptiveMaxPriceMissResolved', 'true');
    setDefault(fields, 'notifyOnAdaptiveMaxPriceCooldown', 'true');
    setDefault(fields, 'notifyOnAdaptiveMaxPriceSummary', 'true');
    setDefault(fields, 'notifyOnAdaptiveMaxPriceAllNoRelax', 'false');
    setDefault(fields, 'adaptiveMaxPriceNotifyMinIntervalSec', '30');
    setDefault(fields, 'adaptiveMaxPriceNotifyIncludePayload', 'false');
    setDefault(fields, 'adaptiveMaxPriceSummaryEveryMarkets', '5');
  } else if (fields.pairLockStrategy === 'biased_hedge_v1') {
    applyBiasedHedgeFormDefaults(fields, cfg);
  }
  if (!(fields.pairMaxTotalCent ?? '').trim()) {
    fields.pairMaxTotalCent = toStringValue(cfg.pairMaxTotalCent ?? cfg.pairTargetTotalCent).trim();
  }
  if (!(fields.counterLegOutcomeLabel ?? '').trim()) {
    fields.counterLegOutcomeLabel = 'opposite';
  }
  if (!(fields.pairOrphanGraceMs ?? '').trim()) {
    fields.pairOrphanGraceMs = '1500';
  }
  if (!(fields.pairProtectiveUnwindEnabled ?? '').trim()) {
    fields.pairProtectiveUnwindEnabled = 'true';
  }
  fields.pairSizingMode = normalizePairLockSizingMode(toStringValue(cfg.pairSizingMode));
  if (
    fields.pairSizingMode === 'auto_remaining_budget' &&
    !(fields.pairTotalBudgetUsdc ?? '').trim()
  ) {
    fields.pairTotalBudgetUsdc = toStringValue(cfg.pairTotalBudgetUsdc).trim();
  }
  applyExplicitCounterStopLossFormDefaults(fields);
}

function normalizePositiveNumberField(
  config: Record<string, unknown>,
  key: string,
  fallback: number
): void {
  const value = Number(toStringValue(config[key]).trim());
  config[key] = Number.isFinite(value) && value > 0 ? value : fallback;
}

function normalizeNonNegativeNumberField(
  config: Record<string, unknown>,
  key: string,
  fallback: number
): void {
  const value = Number(toStringValue(config[key]).trim());
  config[key] = Number.isFinite(value) && value >= 0 ? value : fallback;
}

function normalizeOptionalIntegerField(
  config: Record<string, unknown>,
  key: string,
  min: number,
  max: number
): void {
  const raw = toStringValue(config[key]).trim();
  if (!raw) {
    delete config[key];
    return;
  }
  const value = Number(raw);
  if (Number.isInteger(value) && value >= min && value <= max) {
    config[key] = value;
  } else {
    delete config[key];
  }
}

function normalizeIntegerField(
  config: Record<string, unknown>,
  key: string,
  fallback: number,
  min: number,
  max: number
): void {
  const value = Number(toStringValue(config[key]).trim());
  config[key] = Number.isInteger(value) && value >= min && value <= max ? value : fallback;
}

function normalizePositiveIntegerField(
  config: Record<string, unknown>,
  key: string,
  fallback: number
): void {
  const value = Number(toStringValue(config[key]).trim());
  config[key] = Number.isInteger(value) && value > 0 ? value : fallback;
}

function deleteAdaptiveMaxPriceConfig(config: Record<string, unknown>): void {
  for (const key of ADAPTIVE_MAX_PRICE_CONFIG_KEYS) {
    delete config[key];
  }
}

function positiveNumber(config: Record<string, unknown>, key: string, fallback: number): number {
  const value = Number(toStringValue(config[key]).trim());
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function nonNegativeNumber(config: Record<string, unknown>, key: string, fallback: number): number {
  const value = Number(toStringValue(config[key]).trim());
  return Number.isFinite(value) && value >= 0 ? value : fallback;
}

function booleanValue(config: Record<string, unknown>, key: string, fallback: boolean): boolean {
  const value = toStringValue(config[key]).trim().toLowerCase();
  if (value === 'true') return true;
  if (value === 'false') return false;
  if (typeof config[key] === 'boolean') return config[key] as boolean;
  return fallback;
}

function finiteNumber(config: Record<string, unknown>, key: string, fallback: number): number {
  const value = Number(toStringValue(config[key]).trim());
  return Number.isFinite(value) ? value : fallback;
}

function normalizeBiasedHedgeBuildConfig(config: Record<string, unknown>): void {
  config.pairLockStrategy = 'biased_hedge_v1';
  config.priceToBeatGuardEnabled = true;
  config.priceToBeatMode = 'iv_mismatch_edge';
  config.pairSizingMode = 'manual';
  config.counterLegEnabled = true;
  config.tpEnabled = false;
  config.pairProtectiveUnwindEnabled = true;
  const primaryBudgetUsdc = positiveNumber(config, 'biasedHedgePrimaryBudgetUsdc', positiveNumber(config, 'sizeUsdc', 2));
  const hedgeBudgetUsdc = positiveNumber(config, 'biasedHedgeHedgeBudgetUsdc', 0.5);
  config.biasedHedge = {
    primaryBudgetUsdc,
    hedgeBudgetUsdc,
    minDominantShare: positiveNumber(config, 'biasedHedgeMinDominantShare', 0.75),
    maxHedgeSpendRatio: positiveNumber(config, 'biasedHedgeMaxHedgeSpendRatio', 0.25),
    primaryMinEdge: nonNegativeNumber(config, 'biasedHedgePrimaryMinEdge', 0.08),
    primaryMinFinalQ: positiveNumber(config, 'biasedHedgePrimaryMinFinalQ', 0.72),
    maxPriceCent: positiveNumber(config, 'biasedHedgeMaxPriceCent', positiveNumber(config, 'maxPriceCent', 75)),
    highPriceCent: positiveNumber(config, 'biasedHedgeHighPriceCent', 70),
    highPriceMinFinalQ: positiveNumber(config, 'biasedHedgeHighPriceMinFinalQ', 0.82),
    highPriceMinEdge: nonNegativeNumber(config, 'biasedHedgeHighPriceMinEdge', 0.10),
    hedgeOnlyIfPrimaryFilled: booleanValue(config, 'biasedHedgeHedgeOnlyIfPrimaryFilled', true),
    hedgeMinPriceCent: positiveNumber(config, 'biasedHedgeHedgeMinPriceCent', 3),
    hedgeMaxPriceCent: positiveNumber(config, 'biasedHedgeHedgeMaxPriceCent', 25),
    disableNewPrimaryAfterSec: Math.floor(positiveNumber(config, 'biasedHedgeDisableNewPrimaryAfterSec', 180)),
    disableAnyBuyAfterSec: Math.floor(positiveNumber(config, 'biasedHedgeDisableAnyBuyAfterSec', 240)),
    maxSideSwitchCount: Math.floor(nonNegativeNumber(config, 'biasedHedgeMaxSideSwitchCount', 0)),
  };
  config.biasedHedgeMaxPairedEffectiveCostCent = positiveNumber(config, 'biasedHedgeMaxPairedEffectiveCostCent', 95);
  const timeExitRules = parseBiasedHedgeTimeExitRulesJson(config.biasedHedgeStopTimeExitRulesJson);
  config.biasedHedgeStop = {
    biasInvalidationEnabled: booleanValue(config, 'biasedHedgeStopBiasInvalidationEnabled', true),
    minQFinalToHold: positiveNumber(config, 'biasedHedgeStopMinQFinalToHold', 0.55),
    minEdgeToHold: nonNegativeNumber(config, 'biasedHedgeStopMinEdgeToHold', 0),
    exitPctOnInvalidation: positiveNumber(config, 'biasedHedgeStopExitPctOnInvalidation', 100),
    ptbStopLossEnabled: booleanValue(config, 'biasedHedgeStopPtbStopLossEnabled', true),
    ptbStopLossGapUsd: finiteNumber(config, 'biasedHedgeStopPtbStopLossGapUsd', -3),
    ptbStopLossTimeDecayMode: ['none', 'relax'].includes(toStringValue(config.biasedHedgeStopPtbStopLossTimeDecayMode).trim().toLowerCase())
      ? toStringValue(config.biasedHedgeStopPtbStopLossTimeDecayMode).trim().toLowerCase()
      : 'tighten',
    timeExitRules: timeExitRules.length > 0
      ? timeExitRules
      : [{ elapsedSec: 90, remainingPct: 60 }, { elapsedSec: 150, remainingPct: 0 }],
  };
  for (const key of PAIR_LOCK_CONFIG_KEYS) {
    if (
      key.startsWith('biasedHedge') &&
      key !== 'biasedHedge' &&
      key !== 'biasedHedgeStop' &&
      key !== 'biasedHedgeMaxPairedEffectiveCostCent'
    ) {
      delete config[key];
    }
  }
  delete config.pairLockDecisionQty;
  delete config.pairLockSingleEdgeThreshold;
  delete config.pairLockCostBuffer;
  deleteAdaptiveMaxPriceConfig(config);
  delete config.pairTotalBudgetUsdc;
  delete config.counterLegSizeUsdc;
}

export function normalizePairLockBuildConfig(config: Record<string, unknown>): void {
  const pairLockMode = toStringValue(config.mode).trim().toLowerCase() === 'pair_lock';
  if (!pairLockMode) {
    delete config.mode;
    for (const key of PAIR_LOCK_CONFIG_KEYS) {
      delete config[key];
    }
    return;
  }

  const pairLockStrategy = normalizePairLockStrategy(toStringValue(config.pairLockStrategy));
  if (pairLockStrategy === 'edge_pairlock_v1') {
    config.pairLockStrategy = 'edge_pairlock_v1';
    config.priceToBeatGuardEnabled = true;
    config.priceToBeatMode = 'iv_mismatch_edge';
    normalizePositiveNumberField(config, 'pairLockDecisionQty', 5);
    normalizeNonNegativeNumberField(config, 'pairLockSingleEdgeThreshold', 0.10);
    normalizeNonNegativeNumberField(config, 'pairLockCostBuffer', 0.005);
    deleteAdaptiveMaxPriceConfig(config);
  } else if (pairLockStrategy === 'adaptive_max_price_v1') {
    config.pairLockStrategy = 'adaptive_max_price_v1';
    config.priceToBeatGuardEnabled = true;
    config.priceToBeatMode = 'iv_mismatch_edge';
    normalizePositiveNumberField(config, 'adaptiveMaxPriceMissCount', 3);
    normalizePositiveNumberField(config, 'adaptiveMaxPriceRequiredGoodMissCount', 2);
    normalizePositiveNumberField(config, 'adaptiveMaxPriceRelaxCreditCent', 2);
    normalizePositiveNumberField(config, 'adaptiveMaxPriceMaxRelaxCreditCent', 5);
    normalizePositiveNumberField(config, 'adaptiveMaxPriceHardCapCent', 76);
    normalizePositiveNumberField(config, 'adaptiveMaxPriceExtraBufferCent', 1);
    normalizeNonNegativeNumberField(config, 'adaptiveMaxPricePairBufferCent', 1);
    normalizePositiveNumberField(config, 'adaptiveMaxPriceSizeMultiplier', 0.5);
    normalizeOptionalIntegerField(config, 'adaptiveMaxPriceWindowStartSec', 0, 300);
    normalizeOptionalIntegerField(config, 'adaptiveMaxPriceWindowEndSec', 0, 300);
    delete config.adaptiveMaxPriceLateRelaxCutoffS;
    config.adaptiveMaxPriceLateRiskEnabled = booleanValue(config, 'adaptiveMaxPriceLateRiskEnabled', true);
    normalizeIntegerField(config, 'adaptiveMaxPriceLateRiskAfterSec', 210, 0, 300);
    normalizeNonNegativeNumberField(config, 'adaptiveMaxPriceLateExtraBufferCent', 1);
    normalizePositiveNumberField(config, 'adaptiveMaxPriceLateSizeMultiplier', 0.35);
    normalizeNonNegativeNumberField(config, 'adaptiveMaxPriceSlCooldownMarkets', 3);
    config.notifyOnAdaptiveMaxPriceEvaluated = booleanValue(config, 'notifyOnAdaptiveMaxPriceEvaluated', false);
    config.notifyOnAdaptiveMaxPriceRelax = booleanValue(config, 'notifyOnAdaptiveMaxPriceRelax', true);
    config.notifyOnAdaptiveMaxPriceRelaxSl = booleanValue(config, 'notifyOnAdaptiveMaxPriceRelaxSl', true);
    config.notifyOnAdaptiveMaxPriceNoRelaxImportant = booleanValue(config, 'notifyOnAdaptiveMaxPriceNoRelaxImportant', true);
    config.notifyOnAdaptiveMaxPriceMissResolved = booleanValue(config, 'notifyOnAdaptiveMaxPriceMissResolved', true);
    config.notifyOnAdaptiveMaxPriceCooldown = booleanValue(config, 'notifyOnAdaptiveMaxPriceCooldown', true);
    config.notifyOnAdaptiveMaxPriceSummary = booleanValue(config, 'notifyOnAdaptiveMaxPriceSummary', true);
    config.notifyOnAdaptiveMaxPriceAllNoRelax = booleanValue(config, 'notifyOnAdaptiveMaxPriceAllNoRelax', false);
    normalizeIntegerField(config, 'adaptiveMaxPriceNotifyMinIntervalSec', 30, 0, Number.MAX_SAFE_INTEGER);
    config.adaptiveMaxPriceNotifyIncludePayload = booleanValue(config, 'adaptiveMaxPriceNotifyIncludePayload', false);
    normalizePositiveIntegerField(config, 'adaptiveMaxPriceSummaryEveryMarkets', 5);
    delete config.pairLockDecisionQty;
    delete config.pairLockSingleEdgeThreshold;
    delete config.pairLockCostBuffer;
  } else if (pairLockStrategy === 'biased_hedge_v1') {
    normalizeBiasedHedgeBuildConfig(config);
  } else {
    delete config.pairLockStrategy;
    delete config.pairLockDecisionQty;
    delete config.pairLockSingleEdgeThreshold;
    delete config.pairLockCostBuffer;
    deleteAdaptiveMaxPriceConfig(config);
    delete config.biasedHedge;
    delete config.biasedHedgeStop;
    delete config.biasedHedgeMaxPairedEffectiveCostCent;
  }

  const pairMaxTotalCent = Number(toStringValue(config.pairMaxTotalCent ?? config.pairTargetTotalCent).trim());
  if (Number.isFinite(pairMaxTotalCent) && pairMaxTotalCent > 0 && pairMaxTotalCent < 100) {
    config.pairMaxTotalCent = pairMaxTotalCent;
  } else {
    delete config.pairMaxTotalCent;
  }
  delete config.pairTargetTotalCent;
  delete config.pairMinNetProfitUsdc;
  delete config.pairProfitSafetyBufferUsdc;
  delete config.notifyOnPairNoEdge;

  const pairOrphanGraceMs = Number(toStringValue(config.pairOrphanGraceMs).trim());
  if (Number.isFinite(pairOrphanGraceMs) && pairOrphanGraceMs >= 0) {
    config.pairOrphanGraceMs = Math.floor(pairOrphanGraceMs);
  } else {
    delete config.pairOrphanGraceMs;
  }

  if (!toStringValue(config.counterLegOutcomeLabel).trim()) {
    config.counterLegOutcomeLabel = 'opposite';
  }

  config.pairSizingMode = normalizePairLockSizingMode(toStringValue(config.pairSizingMode));
  if (pairLockStrategy === 'edge_pairlock_v1' || pairLockStrategy === 'biased_hedge_v1') {
    config.pairSizingMode = 'manual';
    delete config.pairTotalBudgetUsdc;
    delete config.counterLegSizeUsdc;
  } else if (config.pairSizingMode === 'auto_remaining_budget') {
    const pairTotalBudgetUsdc = Number(toStringValue(config.pairTotalBudgetUsdc).trim());
    if (Number.isFinite(pairTotalBudgetUsdc) && pairTotalBudgetUsdc > 0) {
      config.pairTotalBudgetUsdc = pairTotalBudgetUsdc;
    } else {
      delete config.pairTotalBudgetUsdc;
    }
    delete config.counterLegSizeUsdc;
  } else {
    delete config.pairTotalBudgetUsdc;
  }

  if (config.counterLegPriceToBeatGuardEnabled === true) {
    config.counterLegPriceToBeatMode = normalizePtbMode(config.counterLegPriceToBeatMode);
    if (config.counterLegPriceToBeatMode === 'manual') {
      const counterLegPriceToBeatUnitRaw = toStringValue(config.counterLegPriceToBeatMaxDiffUnit).trim().toLowerCase();
      config.counterLegPriceToBeatMaxDiffUnit = counterLegPriceToBeatUnitRaw === 'cent' ? 'cent' : 'usd';
    } else {
      delete config.counterLegPriceToBeatMaxDiff;
      delete config.counterLegPriceToBeatMaxDiffUnit;
    }
  } else {
    delete config.counterLegPriceToBeatMode;
    delete config.counterLegPriceToBeatMaxDiff;
    delete config.counterLegPriceToBeatMaxDiffUnit;
  }
}

export function normalizePairLockStopLossBuildConfig(
  config: Record<string, unknown>
): void {
  const hardSlEnabled = config.slEnabled === true;
  const ptbStopLossEnabled = config.ptbStopLossEnabled === true;
  const hasStagedPrimarySl = Array.isArray(config.slRules) && config.slRules.length > 0;
  const hasStagedPrimaryPtbStopLoss =
    Array.isArray(config.ptbStopLossRules) && config.ptbStopLossRules.length > 0;
  const anyStopLossEnabled =
    hardSlEnabled ||
    ptbStopLossEnabled ||
    hasStagedPrimarySl ||
    hasStagedPrimaryPtbStopLoss;
  const counterSlEnabled = config.counterLegSlEnabled === true;
  const counterPtbStopLossEnabled = config.counterLegPtbStopLossEnabled === true;
  const anyCounterStopLossEnabled = counterSlEnabled || counterPtbStopLossEnabled;

  for (const key of PAIR_LOCK_UNSUPPORTED_EXIT_FIELD_KEYS) {
    delete config[key];
  }

  if (!hardSlEnabled) {
    delete config.slEnabled;
    delete config.slPriceCent;
    delete config.slPrice;
    delete config.slTriggerPriceMode;
  }

  if (!ptbStopLossEnabled) {
    delete config.ptbStopLossEnabled;
    delete config.ptbStopLossGapUsd;
    delete config.ptbStopLossGapUnit;
    delete config.ptbStopLossRules;
    delete config.ptbStopLossTimeDecayMode;
  } else {
    config.ptbStopLossGapUnit = normalizePtbStopLossGapUnit(config.ptbStopLossGapUnit);
    const ptbStopLossTimeDecayModeRaw = toStringValue(config.ptbStopLossTimeDecayMode)
      .trim()
      .toLowerCase();
    config.ptbStopLossTimeDecayMode =
      ptbStopLossTimeDecayModeRaw === 'none' || ptbStopLossTimeDecayModeRaw === 'relax'
        ? ptbStopLossTimeDecayModeRaw
        : 'tighten';
  }

  if (!counterSlEnabled) {
    delete config.counterLegSlPriceCent;
    delete config.counterLegSlTriggerPriceMode;
  } else {
    const counterLegSlTriggerPriceModeRaw = toStringValue(config.counterLegSlTriggerPriceMode)
      .trim()
      .toLowerCase();
    config.counterLegSlTriggerPriceMode =
      counterLegSlTriggerPriceModeRaw === 'composite' ||
      counterLegSlTriggerPriceModeRaw === 'composite_safe' ||
      counterLegSlTriggerPriceModeRaw === 'composite_fast' ||
      counterLegSlTriggerPriceModeRaw === 'last_trade'
        ? counterLegSlTriggerPriceModeRaw
        : 'best_bid';
  }

  if (!counterPtbStopLossEnabled) {
    delete config.counterLegPtbStopLossGapUsd;
    delete config.counterLegPtbStopLossGapUnit;
    delete config.counterLegPtbStopLossTimeDecayMode;
  } else {
    config.counterLegPtbStopLossGapUnit = normalizePtbStopLossGapUnit(
      config.counterLegPtbStopLossGapUnit,
      config.ptbStopLossGapUnit === 'cent' ? 'cent' : 'usd'
    );
    const counterLegPtbStopLossTimeDecayModeRaw = toStringValue(
      config.counterLegPtbStopLossTimeDecayMode
    )
      .trim()
      .toLowerCase();
    config.counterLegPtbStopLossTimeDecayMode =
      counterLegPtbStopLossTimeDecayModeRaw === 'none' ||
      counterLegPtbStopLossTimeDecayModeRaw === 'relax'
        ? counterLegPtbStopLossTimeDecayModeRaw
        : 'tighten';
  }

  if (!anyStopLossEnabled) {
    delete config.notifyOnSlHit;
    delete config.reenterOnSlHit;
    delete config.reentryMaxAttempts;
    delete config.reentryCooldownSec;
    return;
  }

  if (!anyCounterStopLossEnabled) {
    delete config.counterLegNotifyOnSlHit;
  }

  if (config.reenterOnSlHit !== true) {
    delete config.reentryMaxAttempts;
    delete config.reentryCooldownSec;
    return;
  }

  const reentryMaxAttempts = Number(toStringValue(config.reentryMaxAttempts).trim());
  if (
    Number.isInteger(reentryMaxAttempts) &&
    reentryMaxAttempts >= 1 &&
    reentryMaxAttempts <= 10
  ) {
    config.reentryMaxAttempts = reentryMaxAttempts;
  } else {
    delete config.reentryMaxAttempts;
  }

  const reentryCooldownSec = Number(toStringValue(config.reentryCooldownSec).trim());
  if (Number.isInteger(reentryCooldownSec) && reentryCooldownSec >= 0) {
    config.reentryCooldownSec = reentryCooldownSec;
  } else {
    delete config.reentryCooldownSec;
  }
}

export function normalizePairLockTakeProfitBuildConfig(
  config: Record<string, unknown>
): void {
  const pairLockStrategy = normalizePairLockStrategy(toStringValue(config.pairLockStrategy));
  const primaryTpEnabled =
    config.tpEnabled === true ||
    (Array.isArray(config.tpRules) && config.tpRules.length > 0);
  const counterTpEnabled =
    config.counterLegEnabled === true &&
    (config.counterLegTpEnabled === true ||
      (Array.isArray(config.counterLegTpRules) && config.counterLegTpRules.length > 0));

  if (!primaryTpEnabled) {
    if (pairLockStrategy === 'biased_hedge_v1') {
      config.tpEnabled = false;
    } else {
      delete config.tpEnabled;
    }
    delete config.tpPriceCent;
    delete config.tpPrice;
    delete config.tpRules;
    delete config.notifyOnTpHit;
  }

  if (!counterTpEnabled) {
    delete config.counterLegTpEnabled;
    delete config.counterLegTpPriceCent;
    delete config.counterLegTpRules;
    delete config.counterLegNotifyOnTpHit;
  }
}
