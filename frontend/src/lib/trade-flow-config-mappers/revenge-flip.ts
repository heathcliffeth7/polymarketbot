import { isRecord, safeJsonStringify, toStringValue } from './utils';
import { normalizeOptionalPtbStopLossCurrentPriceSource } from './ptb-modes';

export const REVENGE_FLIP_MODE = 'revenge_flip_v1';
export const REVENGE_FLIP_BINDING_MODE = 'revenge_flip_only';

export const REVENGE_FLIP_INITIAL_ORDER_USDC_FIELD = 'revengeFlipInitialOrderUsdc';
export const REVENGE_FLIP_PROFIT_TARGET_USDC_FIELD = 'revengeFlipProfitTargetUsdc';
export const REVENGE_FLIP_CLASSIC_STOP_LOSS_ENABLED_FIELD =
  'revengeFlipClassicStopLossEnabled';
export const REVENGE_FLIP_STOP_LOSS_PCT_FIELD = 'revengeFlipStopLossPct';
export const REVENGE_FLIP_STOP_LOSS_RULES_FIELD = 'revengeFlipStopLossRulesJson';
export const REVENGE_FLIP_ENTRY_PTB_RULES_FIELD = 'revengeFlipEntryPtbRulesJson';
export const REVENGE_FLIP_REENTRY_SIDE_MODE_FIELD = 'revengeFlipReentrySideMode';
export const REVENGE_FLIP_MAX_FLIP_FIELD = 'revengeFlipMaxFlip';
export const REVENGE_FLIP_MIN_REENTRY_SHARES_FIELD = 'revengeFlipMinReentryShares';
export const REVENGE_FLIP_POST_STOP_LOSS_IV_MISMATCH_ENABLED_FIELD =
  'revengeFlipPostStopLossIvMismatchEnabled';
export const REVENGE_FLIP_LOT_LIMIT_PCT_FIELD = 'revengeFlipLotLimitPct';
export const REVENGE_FLIP_CLOSE_ONLY_SEC_FIELD = 'revengeFlipCloseOnlySec';
export const REVENGE_FLIP_TRIGGER_ENABLED_FIELD = 'revengeFlipTriggerPriceEnabled';
export const REVENGE_FLIP_TRIGGER_MIN_CENT_FIELD = 'revengeFlipTriggerPriceMinCent';
export const REVENGE_FLIP_TRIGGER_MAX_CENT_FIELD = 'revengeFlipTriggerPriceMaxCent';
export const REVENGE_FLIP_TIME_RULES_FIELD = 'revengeFlipTimeRulesJson';
export const REVENGE_FLIP_PTB_BUMP_ENABLED_FIELD = 'revengeFlipPtbStopLossBumpEnabled';
export const REVENGE_FLIP_PTB_BUMP_AMOUNT_FIELD = 'revengeFlipPtbStopLossBumpAmount';
export const REVENGE_FLIP_PTB_BUMP_UNIT_FIELD = 'revengeFlipPtbStopLossBumpUnit';
export const REVENGE_FLIP_PTB_BUMP_MAX_FIELD = 'revengeFlipPtbStopLossBumpMax';
export const REVENGE_FLIP_PTB_BUMP_MAX_UNIT_FIELD = 'revengeFlipPtbStopLossBumpMaxUnit';
export const REVENGE_FLIP_PTB_STOP_LOSS_ENABLED_FIELD = 'revengeFlipPtbStopLossEnabled';
export const REVENGE_FLIP_PTB_STOP_LOSS_GAP_FIELD = 'revengeFlipPtbStopLossGapUsd';
export const REVENGE_FLIP_PTB_STOP_LOSS_GAP_UNIT_FIELD = 'revengeFlipPtbStopLossGapUnit';
export const REVENGE_FLIP_PTB_STOP_LOSS_CURRENT_SOURCE_FIELD =
  'revengeFlipPtbStopLossCurrentPriceSource';
export const REVENGE_FLIP_PTB_STOP_LOSS_TIME_DECAY_FIELD =
  'revengeFlipPtbStopLossTimeDecayMode';

const REVENGE_FLIP_DEFAULT_IV_TIME_RULES = [
  {
    startRemainingSec: 45,
    endRemainingSec: 30,
    minEdge: 0.03,
    minGapStrength: 0.5,
    maxPriceCent: 92,
  },
  {
    startRemainingSec: 30,
    endRemainingSec: 15,
    minEdge: 0.05,
    minGapStrength: 0.75,
    maxPriceCent: 92,
  },
  {
    startRemainingSec: 15,
    endRemainingSec: 8,
    minEdge: 0.07,
    minGapStrength: 1,
    maxPriceCent: 92,
  },
];

const REVENGE_FLIP_IV_ENTRY_QUALITY_DEFAULTS: Record<string, string | number | boolean> = {
  priceToBeatIvEntryQualityPolicy: true,
  priceToBeatIvNormalMaxPriceCent: 94,
  priceToBeatIvPremiumMaxPriceCent: 96,
  priceToBeatIvNoNewEntryBelowSeconds: 8,
  priceToBeatIvMinExpectedMoveBps: 2,
  priceToBeatIvMinExpectedMoveUsd: 0,
  priceToBeatIvGapStrengthMin60To45: 2.5,
  priceToBeatIvGapStrengthMin45To25: 2.2,
  priceToBeatIvGapStrengthMin25To10: 1.9,
  priceToBeatIvGapStrengthMin10To8: 2,
  priceToBeatIvBufferTrendGuardEnabled: true,
  priceToBeatIvBufferRetain5s: 0.85,
  priceToBeatIvBufferRetain10s: 0.7,
  priceToBeatIvPremiumBufferRetain5s: 0.9,
  priceToBeatIvPremiumBufferRetain10s: 0.75,
  priceToBeatIvSpikeFadeGuardEnabled: true,
  priceToBeatIvSpikeMultiplier: 2.5,
  priceToBeatIvSpikeRetraceRatio: 0.2,
  priceToBeatIvPremiumMaxSpreadCent: 2,
  priceToBeatIvPremiumMaxChainlinkAgeMs: 2500,
  priceToBeatIvCexAlignMaxBps: 5,
  priceToBeatIvEntryQualityChainlinkMaxAgeMs: 2500,
  priceToBeatIvEntryQualityHighRiskUnderSec: 30,
  priceToBeatIvEntryQualityHighRiskAskCent: 85,
  priceToBeatIvEntryQualityHighPriceMaxSpreadCent: 2,
  priceToBeatIvEntryQualityMaxSpreadCent: 3,
  priceToBeatIvEntryQualityNeutralEdgePenalty: 0.03,
  priceToBeatIvEntryQualityNeutralGapStrengthPenalty: 0.25,
  priceToBeatIvEntryQualityStaleEdgePenalty: 0.03,
  priceToBeatIvEntryQualityStaleGapStrengthPenalty: 0.25,
};

const REVENGE_FLIP_IV_PTB_CHOP_DEFAULTS: Record<string, string | number | boolean> = {
  priceToBeatIvPtbChopGuardEnabled: true,
  priceToBeatIvPtbChopLookbackSeconds: 10,
  priceToBeatIvPtbChopExtendedLookbackSeconds: 15,
  priceToBeatIvPtbChopDeadbandBps: 0.5,
  priceToBeatIvPtbChopDeadbandMinUsdBtc: 5,
  priceToBeatIvPtbChopDeadbandMinUsdEth: 0.3,
  priceToBeatIvPtbChopDeadbandMinUsdSol: 0.03,
  priceToBeatIvPtbChopZeroCrossBlock10s: 2,
  priceToBeatIvPtbChopZeroCrossBlock15s: 3,
  priceToBeatIvPtbChopPathZWarn: 1.25,
  priceToBeatIvPtbChopPathZBlock: 1.75,
  priceToBeatIvPtbChopEfficiencyWarn: 0.25,
  priceToBeatIvPtbChopEfficiencyBlock: 0.15,
  priceToBeatIvPtbChopOppositeDepthZWarn: 0.5,
  priceToBeatIvPtbChopOppositeDepthZBlock: 0.9,
  priceToBeatIvPtbChopMaxGapStrengthPenalty: 0.35,
};

const REVENGE_FLIP_IV_HIGH_PRICE_EARLY_DEFAULTS: Record<string, string | number | boolean> = {
  priceToBeatIvHighPriceEarlyReversalGuardEnabled: true,
  priceToBeatIvHighPriceEarlyRefCent: 77,
  priceToBeatIvHighPriceEarlyRemainingSec: 120,
  priceToBeatIvHighPriceEarlyMaxStaleMs: 2000,
  priceToBeatIvHighPriceEarlyStaleGapAdd: 0.3,
  priceToBeatIvHighPriceEarlyBinanceMissingGapAdd: 0.35,
  priceToBeatIvHighPriceEarlyQExtremeCent: 98.5,
  priceToBeatIvHighPriceEarlyQExtremeMinGapStrength: 3.5,
  priceToBeatIvHighPriceEarlyQExtremeMaxStaleMs: 1500,
  priceToBeatIvHighPriceEarlyQExtremeRequireBinanceQ: true,
  priceToBeatIvHighPriceEarlyQExtremeRequireCleanStrongCex: true,
};

function toFiniteNumber(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  const text = toStringValue(value).trim();
  if (!text) return null;
  const parsed = Number(text);
  return Number.isFinite(parsed) ? parsed : null;
}

function toBooleanString(value: unknown, fallback: boolean): string {
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  const text = toStringValue(value).trim().toLowerCase();
  if (['true', '1', 'yes', 'on'].includes(text)) return 'true';
  if (['false', '0', 'no', 'off'].includes(text)) return 'false';
  return fallback ? 'true' : 'false';
}

function numberField(
  fields: Record<string, string>,
  key: string,
  fallback: number,
): number {
  return toFiniteNumber(fields[key]) ?? fallback;
}

function normalizeUnit(value: unknown, fallback = 'usd'): 'usd' | 'cent' {
  const unit = toStringValue(value).trim().toLowerCase();
  if (unit === 'cent' || unit === 'cents') return 'cent';
  return fallback === 'cent' ? 'cent' : 'usd';
}

function normalizeReentrySideMode(value: unknown): 'opposite' | 'rule_match' {
  const normalized = toStringValue(value).trim().toLowerCase();
  return normalized === 'rule_match' ? 'rule_match' : 'opposite';
}

function normalizeTimeDecayMode(value: unknown): 'tighten' | 'relax' | 'none' {
  const normalized = toStringValue(value).trim().toLowerCase();
  if (normalized === 'relax' || normalized === 'none') return normalized;
  return 'tighten';
}

function parseJsonArray(raw: unknown): unknown[] {
  if (Array.isArray(raw)) return raw;
  const text = toStringValue(raw).trim();
  if (!text) return [];
  try {
    const parsed = JSON.parse(text);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function applyStringDefaults(
  target: Record<string, string>,
  defaults: Record<string, string | number | boolean>,
): void {
  for (const [key, value] of Object.entries(defaults)) {
    if (!toStringValue(target[key]).trim()) target[key] = toStringValue(value);
  }
}

function applyConfigDefaults(
  target: Record<string, unknown>,
  defaults: Record<string, string | number | boolean>,
): void {
  for (const [key, value] of Object.entries(defaults)) {
    if (!toStringValue(target[key]).trim()) target[key] = value;
  }
}

function normalizePtbMinDiffRules(raw: unknown): unknown[] {
  const rows = Array.isArray(raw) ? raw : [];
  return rows.map((item) => {
    if (!isRecord(item)) return item;
    const next: Record<string, unknown> = { ...item };
    if (next.priceToBeatMinDiff == null && next.priceToBeatMaxDiff != null) {
      next.priceToBeatMinDiff = next.priceToBeatMaxDiff;
    }
    if (next.priceToBeatMinDiffUnit == null && next.priceToBeatMaxDiffUnit != null) {
      next.priceToBeatMinDiffUnit = next.priceToBeatMaxDiffUnit;
    }
    delete next.priceToBeatMaxDiff;
    delete next.priceToBeatMaxDiffUnit;
    return next;
  });
}

export function applyRevengeFlipFormDefaults(
  fields: Record<string, string>,
  config: Record<string, unknown>,
) {
  const revenge = isRecord(config.revengeFlip) ? config.revengeFlip : {};
  if (toStringValue(config.mode).trim().toLowerCase() === REVENGE_FLIP_MODE) {
    if (!toStringValue(config.priceToBeatMode).trim()) {
      fields.priceToBeatMode = 'iv_mismatch_edge';
    }
    if (!toStringValue(config.priceToBeatCurrentPriceSource).trim()) {
      fields.priceToBeatCurrentPriceSource = 'chainlink';
    }
    if (!toStringValue(config.cexDirectionGuardEnabled).trim()) {
      fields.cexDirectionGuardEnabled = 'true';
    }
    if (!toStringValue(config.cexDirectionGuardMode).trim()) {
      fields.cexDirectionGuardMode = 'bybit_plus_one';
    }
    if (!toStringValue(config.cexDirectionGuardFailClosed).trim()) {
      fields.cexDirectionGuardFailClosed = 'false';
    }
    applyStringDefaults(fields, REVENGE_FLIP_IV_ENTRY_QUALITY_DEFAULTS);
    applyStringDefaults(fields, REVENGE_FLIP_IV_PTB_CHOP_DEFAULTS);
    applyStringDefaults(fields, REVENGE_FLIP_IV_HIGH_PRICE_EARLY_DEFAULTS);
  }
  const triggerPrice = isRecord(config.triggerPrice) ? config.triggerPrice : {};
  fields[REVENGE_FLIP_INITIAL_ORDER_USDC_FIELD] = toStringValue(
    revenge.initialOrderUsdc ?? config.initialOrderUsdc ?? 5,
  );
  fields[REVENGE_FLIP_PROFIT_TARGET_USDC_FIELD] = toStringValue(
    revenge.profitTargetUsdc ?? config.profitTargetUsdc ?? 0.25,
  );
  fields[REVENGE_FLIP_CLASSIC_STOP_LOSS_ENABLED_FIELD] = toBooleanString(
    revenge.classicStopLossEnabled ?? config.classicStopLossEnabled,
    true,
  );
  fields[REVENGE_FLIP_STOP_LOSS_PCT_FIELD] = toStringValue(
    revenge.stopLossPct ?? config.stopLossPct ?? 0.2,
  );
  fields[REVENGE_FLIP_STOP_LOSS_RULES_FIELD] = safeJsonStringify(
    Array.isArray(revenge.stopLossRules) ? revenge.stopLossRules : config.stopLossRules ?? [],
  );
  fields[REVENGE_FLIP_ENTRY_PTB_RULES_FIELD] = safeJsonStringify(
    normalizePtbMinDiffRules(
      Array.isArray(revenge.entryPtbRules) ? revenge.entryPtbRules : config.entryPtbRules ?? [],
    ),
  );
  fields[REVENGE_FLIP_REENTRY_SIDE_MODE_FIELD] = normalizeReentrySideMode(
    revenge.reentrySideMode ?? config.reentrySideMode,
  );
  fields[REVENGE_FLIP_MAX_FLIP_FIELD] = toStringValue(
    revenge.maxFlip ?? config.maxFlip ?? 0,
  );
  fields[REVENGE_FLIP_MIN_REENTRY_SHARES_FIELD] = toStringValue(
    revenge.minReentryShares ?? config.minReentryShares ?? 0,
  );
  fields[REVENGE_FLIP_POST_STOP_LOSS_IV_MISMATCH_ENABLED_FIELD] = toBooleanString(
    revenge.postStopLossIvMismatchEnabled ?? config.postStopLossIvMismatchEnabled,
    true,
  );
  fields[REVENGE_FLIP_LOT_LIMIT_PCT_FIELD] = toStringValue(
    revenge.lotLimitPct ?? config.lotLimitPct ?? 0.98,
  );
  fields[REVENGE_FLIP_CLOSE_ONLY_SEC_FIELD] = toStringValue(
    revenge.closeOnlySec ?? config.closeOnlySec ?? 12,
  );
  fields[REVENGE_FLIP_TRIGGER_ENABLED_FIELD] = toBooleanString(
    triggerPrice.enabled ?? revenge.triggerPriceEnabled ?? config.triggerPriceEnabled,
    false,
  );
  fields[REVENGE_FLIP_TRIGGER_MIN_CENT_FIELD] = toStringValue(
    triggerPrice.minCent ?? revenge.triggerPriceMinCent ?? config.triggerPriceMinCent ?? 0,
  );
  fields[REVENGE_FLIP_TRIGGER_MAX_CENT_FIELD] = toStringValue(
    triggerPrice.maxCent ?? revenge.triggerPriceMaxCent ?? config.triggerPriceMaxCent ?? 100,
  );
  fields[REVENGE_FLIP_TIME_RULES_FIELD] = safeJsonStringify(
    Array.isArray(revenge.timeRules) ? revenge.timeRules : config.timeRules ?? [],
  );
  fields[REVENGE_FLIP_PTB_BUMP_ENABLED_FIELD] = toBooleanString(
    revenge.ptbStopLossBumpEnabled ??
      config.priceToBeatStopLossBumpEnabled ??
      config.ptbStopLossBumpEnabled,
    false,
  );
  fields[REVENGE_FLIP_PTB_BUMP_AMOUNT_FIELD] = toStringValue(
    revenge.ptbStopLossBumpAmount ??
      config.priceToBeatStopLossBumpAmount ??
      config.ptbStopLossBumpAmount ??
      '',
  );
  fields[REVENGE_FLIP_PTB_BUMP_UNIT_FIELD] = normalizeUnit(
    revenge.ptbStopLossBumpUnit ??
      config.priceToBeatStopLossBumpUnit ??
      config.ptbStopLossBumpUnit,
  );
  fields[REVENGE_FLIP_PTB_BUMP_MAX_FIELD] = toStringValue(
    revenge.ptbStopLossBumpMax ??
      config.priceToBeatStopLossBumpMax ??
      config.ptbStopLossBumpMax ??
      '',
  );
  fields[REVENGE_FLIP_PTB_BUMP_MAX_UNIT_FIELD] = normalizeUnit(
    revenge.ptbStopLossBumpMaxUnit ??
      config.priceToBeatStopLossBumpMaxUnit ??
      config.ptbStopLossBumpMaxUnit,
  );
  fields[REVENGE_FLIP_PTB_STOP_LOSS_ENABLED_FIELD] = toBooleanString(
    revenge.ptbStopLossEnabled ?? config.ptbStopLossEnabled,
    false,
  );
  fields[REVENGE_FLIP_PTB_STOP_LOSS_GAP_FIELD] = toStringValue(
    revenge.ptbStopLossGapUsd ?? config.ptbStopLossGapUsd ?? '',
  );
  fields[REVENGE_FLIP_PTB_STOP_LOSS_GAP_UNIT_FIELD] = normalizeUnit(
    revenge.ptbStopLossGapUnit ?? config.ptbStopLossGapUnit,
  );
  fields[REVENGE_FLIP_PTB_STOP_LOSS_CURRENT_SOURCE_FIELD] =
    normalizeOptionalPtbStopLossCurrentPriceSource(
      revenge.ptbStopLossCurrentPriceSource ?? config.ptbStopLossCurrentPriceSource,
    );
  fields[REVENGE_FLIP_PTB_STOP_LOSS_TIME_DECAY_FIELD] = normalizeTimeDecayMode(
    revenge.ptbStopLossTimeDecayMode ?? config.ptbStopLossTimeDecayMode,
  );
}

export function normalizeRevengeFlipBuildConfig(
  config: Record<string, unknown>,
  fields: Record<string, string>,
): boolean {
  if (toStringValue(config.mode).trim().toLowerCase() !== REVENGE_FLIP_MODE) {
    return false;
  }
  config.mode = REVENGE_FLIP_MODE;
  config.revengeFlip = {
    initialOrderUsdc: numberField(fields, REVENGE_FLIP_INITIAL_ORDER_USDC_FIELD, 5),
    profitTargetUsdc: numberField(fields, REVENGE_FLIP_PROFIT_TARGET_USDC_FIELD, 0.25),
    classicStopLossEnabled:
      toBooleanString(fields[REVENGE_FLIP_CLASSIC_STOP_LOSS_ENABLED_FIELD], true) === 'true',
    stopLossPct: numberField(fields, REVENGE_FLIP_STOP_LOSS_PCT_FIELD, 0.2),
    stopLossRules: parseJsonArray(fields[REVENGE_FLIP_STOP_LOSS_RULES_FIELD]),
    entryPtbRules: normalizePtbMinDiffRules(parseJsonArray(fields[REVENGE_FLIP_ENTRY_PTB_RULES_FIELD])),
    reentrySideMode: normalizeReentrySideMode(fields[REVENGE_FLIP_REENTRY_SIDE_MODE_FIELD]),
    maxFlip: Math.max(0, Math.trunc(numberField(fields, REVENGE_FLIP_MAX_FLIP_FIELD, 0))),
    minReentryShares: numberField(fields, REVENGE_FLIP_MIN_REENTRY_SHARES_FIELD, 0),
    postStopLossIvMismatchEnabled:
      toBooleanString(fields[REVENGE_FLIP_POST_STOP_LOSS_IV_MISMATCH_ENABLED_FIELD], true) ===
      'true',
    lotLimitPct: numberField(fields, REVENGE_FLIP_LOT_LIMIT_PCT_FIELD, 0.98),
    closeOnlySec: Math.max(0, Math.trunc(numberField(fields, REVENGE_FLIP_CLOSE_ONLY_SEC_FIELD, 12))),
    ptbStopLossBumpEnabled:
      toBooleanString(fields[REVENGE_FLIP_PTB_BUMP_ENABLED_FIELD], false) === 'true',
    ptbStopLossBumpAmount: numberField(fields, REVENGE_FLIP_PTB_BUMP_AMOUNT_FIELD, 0),
    ptbStopLossBumpUnit: normalizeUnit(fields[REVENGE_FLIP_PTB_BUMP_UNIT_FIELD]),
    ptbStopLossBumpMax:
      toFiniteNumber(fields[REVENGE_FLIP_PTB_BUMP_MAX_FIELD]) ?? undefined,
    ptbStopLossBumpMaxUnit: normalizeUnit(fields[REVENGE_FLIP_PTB_BUMP_MAX_UNIT_FIELD]),
    ptbStopLossEnabled:
      toBooleanString(fields[REVENGE_FLIP_PTB_STOP_LOSS_ENABLED_FIELD], false) === 'true',
    ptbStopLossGapUsd: toFiniteNumber(fields[REVENGE_FLIP_PTB_STOP_LOSS_GAP_FIELD]) ?? undefined,
    ptbStopLossGapUnit: normalizeUnit(fields[REVENGE_FLIP_PTB_STOP_LOSS_GAP_UNIT_FIELD]),
    ptbStopLossCurrentPriceSource: normalizeOptionalPtbStopLossCurrentPriceSource(
      fields[REVENGE_FLIP_PTB_STOP_LOSS_CURRENT_SOURCE_FIELD],
    ),
    ptbStopLossTimeDecayMode: normalizeTimeDecayMode(
      fields[REVENGE_FLIP_PTB_STOP_LOSS_TIME_DECAY_FIELD],
    ),
    timeRules: parseJsonArray(fields[REVENGE_FLIP_TIME_RULES_FIELD]),
  };
  config.triggerPrice = {
    enabled: toBooleanString(fields[REVENGE_FLIP_TRIGGER_ENABLED_FIELD], false) === 'true',
    minCent: numberField(fields, REVENGE_FLIP_TRIGGER_MIN_CENT_FIELD, 0),
    maxCent: numberField(fields, REVENGE_FLIP_TRIGGER_MAX_CENT_FIELD, 100),
  };
  if (config.priceToBeatGuardEnabled == null) config.priceToBeatGuardEnabled = true;
  if (!toStringValue(config.priceToBeatMode).trim()) config.priceToBeatMode = 'iv_mismatch_edge';
  if (!toStringValue(config.priceToBeatMinDiffUnit ?? config.priceToBeatMaxDiffUnit).trim()) {
    config.priceToBeatMinDiffUnit = 'usd';
  } else if (config.priceToBeatMinDiffUnit == null && config.priceToBeatMaxDiffUnit != null) {
    config.priceToBeatMinDiffUnit = config.priceToBeatMaxDiffUnit;
  }
  if (config.priceToBeatMinDiff == null && config.priceToBeatMaxDiff != null) {
    config.priceToBeatMinDiff = config.priceToBeatMaxDiff;
  }
  if (config.priceToBeatMinDiff == null || toStringValue(config.priceToBeatMinDiff).trim() === '') {
    config.priceToBeatMinDiff = 0.01;
  }
  delete config.priceToBeatMaxDiff;
  delete config.priceToBeatMaxDiffUnit;
  if (!toStringValue(config.priceToBeatCurrentPriceSource).trim()) {
    config.priceToBeatCurrentPriceSource = 'chainlink';
  }
  if (!Array.isArray(config.priceToBeatIvTimeRules)) {
    config.priceToBeatIvTimeRules = REVENGE_FLIP_DEFAULT_IV_TIME_RULES.map((rule) => ({
      ...rule,
    }));
  }
  applyConfigDefaults(config, REVENGE_FLIP_IV_PTB_CHOP_DEFAULTS);
  applyConfigDefaults(config, REVENGE_FLIP_IV_HIGH_PRICE_EARLY_DEFAULTS);
  if (!toStringValue(config.priceToBeatIvEntryQualityPolicy).trim()) {
    config.priceToBeatIvEntryQualityPolicy = true;
  }
  if (!toStringValue(config.priceToBeatIvNormalMaxPriceCent).trim()) {
    config.priceToBeatIvNormalMaxPriceCent = 94;
  }
  if (!toStringValue(config.priceToBeatIvPremiumMaxPriceCent).trim()) {
    config.priceToBeatIvPremiumMaxPriceCent = 96;
  }
  if (!toStringValue(config.priceToBeatIvNoNewEntryBelowSeconds).trim()) {
    config.priceToBeatIvNoNewEntryBelowSeconds = 8;
  }
  if (!toStringValue(config.priceToBeatIvMinExpectedMoveBps).trim()) {
    config.priceToBeatIvMinExpectedMoveBps = 2;
  }
  if (!toStringValue(config.priceToBeatIvMinExpectedMoveUsd).trim()) {
    config.priceToBeatIvMinExpectedMoveUsd = 0;
  }
  if (!toStringValue(config.priceToBeatIvGapStrengthMin60To45).trim()) {
    config.priceToBeatIvGapStrengthMin60To45 = 2.5;
  }
  if (!toStringValue(config.priceToBeatIvGapStrengthMin45To25).trim()) {
    config.priceToBeatIvGapStrengthMin45To25 = 2.2;
  }
  if (!toStringValue(config.priceToBeatIvGapStrengthMin25To10).trim()) {
    config.priceToBeatIvGapStrengthMin25To10 = 1.9;
  }
  if (!toStringValue(config.priceToBeatIvGapStrengthMin10To8).trim()) {
    config.priceToBeatIvGapStrengthMin10To8 = 2;
  }
  if (!toStringValue(config.priceToBeatIvBufferTrendGuardEnabled).trim()) {
    config.priceToBeatIvBufferTrendGuardEnabled = true;
  }
  if (!toStringValue(config.priceToBeatIvBufferRetain5s).trim()) {
    config.priceToBeatIvBufferRetain5s = 0.85;
  }
  if (!toStringValue(config.priceToBeatIvBufferRetain10s).trim()) {
    config.priceToBeatIvBufferRetain10s = 0.7;
  }
  if (!toStringValue(config.priceToBeatIvPremiumBufferRetain5s).trim()) {
    config.priceToBeatIvPremiumBufferRetain5s = 0.9;
  }
  if (!toStringValue(config.priceToBeatIvPremiumBufferRetain10s).trim()) {
    config.priceToBeatIvPremiumBufferRetain10s = 0.75;
  }
  if (!toStringValue(config.priceToBeatIvSpikeFadeGuardEnabled).trim()) {
    config.priceToBeatIvSpikeFadeGuardEnabled = true;
  }
  if (!toStringValue(config.priceToBeatIvSpikeMultiplier).trim()) {
    config.priceToBeatIvSpikeMultiplier = 2.5;
  }
  if (!toStringValue(config.priceToBeatIvSpikeRetraceRatio).trim()) {
    config.priceToBeatIvSpikeRetraceRatio = 0.2;
  }
  if (!toStringValue(config.priceToBeatIvPremiumMaxSpreadCent).trim()) {
    config.priceToBeatIvPremiumMaxSpreadCent = 2;
  }
  if (!toStringValue(config.priceToBeatIvPremiumMaxChainlinkAgeMs).trim()) {
    config.priceToBeatIvPremiumMaxChainlinkAgeMs = 2500;
  }
  if (!toStringValue(config.priceToBeatIvCexAlignMaxBps).trim()) {
    config.priceToBeatIvCexAlignMaxBps = 5;
  }
  if (!toStringValue(config.priceToBeatIvEntryQualityChainlinkMaxAgeMs).trim()) {
    config.priceToBeatIvEntryQualityChainlinkMaxAgeMs = 2500;
  }
  if (!toStringValue(config.priceToBeatIvEntryQualityHighRiskUnderSec).trim()) {
    config.priceToBeatIvEntryQualityHighRiskUnderSec = 30;
  }
  if (!toStringValue(config.priceToBeatIvEntryQualityHighRiskAskCent).trim()) {
    config.priceToBeatIvEntryQualityHighRiskAskCent = 85;
  }
  if (!toStringValue(config.priceToBeatIvEntryQualityHighPriceMaxSpreadCent).trim()) {
    config.priceToBeatIvEntryQualityHighPriceMaxSpreadCent = 2;
  }
  if (!toStringValue(config.priceToBeatIvEntryQualityMaxSpreadCent).trim()) {
    config.priceToBeatIvEntryQualityMaxSpreadCent = 3;
  }
  if (!toStringValue(config.priceToBeatIvEntryQualityNeutralEdgePenalty).trim()) {
    config.priceToBeatIvEntryQualityNeutralEdgePenalty = 0.03;
  }
  if (!toStringValue(config.priceToBeatIvEntryQualityNeutralGapStrengthPenalty).trim()) {
    config.priceToBeatIvEntryQualityNeutralGapStrengthPenalty = 0.25;
  }
  if (!toStringValue(config.priceToBeatIvEntryQualityStaleEdgePenalty).trim()) {
    config.priceToBeatIvEntryQualityStaleEdgePenalty = 0.03;
  }
  if (!toStringValue(config.priceToBeatIvEntryQualityStaleGapStrengthPenalty).trim()) {
    config.priceToBeatIvEntryQualityStaleGapStrengthPenalty = 0.25;
  }
  if (!toStringValue(config.cexDirectionGuardEnabled).trim()) {
    config.cexDirectionGuardEnabled = true;
  }
  if (!toStringValue(config.cexDirectionGuardMode).trim()) {
    config.cexDirectionGuardMode = 'bybit_plus_one';
  }
  if (!toStringValue(config.cexDirectionGuardFailClosed).trim()) {
    config.cexDirectionGuardFailClosed = false;
  }
  return true;
}
