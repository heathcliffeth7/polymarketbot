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
  fields[REVENGE_FLIP_LOT_LIMIT_PCT_FIELD] = toStringValue(
    revenge.lotLimitPct ?? config.lotLimitPct ?? 0.98,
  );
  fields[REVENGE_FLIP_CLOSE_ONLY_SEC_FIELD] = toStringValue(
    revenge.closeOnlySec ?? config.closeOnlySec ?? 10,
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
    lotLimitPct: numberField(fields, REVENGE_FLIP_LOT_LIMIT_PCT_FIELD, 0.98),
    closeOnlySec: Math.max(0, Math.trunc(numberField(fields, REVENGE_FLIP_CLOSE_ONLY_SEC_FIELD, 10))),
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
  if (!toStringValue(config.priceToBeatMode).trim()) config.priceToBeatMode = 'manual';
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
  return true;
}
