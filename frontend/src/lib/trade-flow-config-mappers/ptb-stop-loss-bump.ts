import { createEmptyPtbStopLossBumpLossRuleRow } from './drafts';
import type {
  NodeConfigFormState,
  PtbStopLossBumpLossRuleRow,
  PtbStopLossBumpMode,
} from './types';
import { normalizePtbMode } from './ptb-modes';
import { isRecord, toStringValue } from './utils';

interface SerializedPtbStopLossBumpLossRule {
  lossUsd: number;
  bumpValue: number;
}

export function normalizePtbStopLossBumpMode(
  value: unknown,
  fallback: PtbStopLossBumpMode = 'fixed'
): PtbStopLossBumpMode {
  const normalized = toStringValue(value).trim().toLowerCase();
  if (normalized === 'fixed' || normalized === 'loss_table') {
    return normalized;
  }
  return fallback;
}

export function normalizePtbStopLossBumpUnit(
  value: unknown,
  fallback: 'usd' | 'cent' = 'usd'
): 'usd' | 'cent' {
  const normalized = toStringValue(value).trim().toLowerCase();
  if (normalized === 'usd' || normalized === 'cent') {
    return normalized;
  }
  return fallback;
}

export function normalizePtbStopLossBumpScope(
  value: unknown,
  fallback: 'global' | 'per_scope' = 'per_scope'
): 'global' | 'per_scope' {
  return toStringValue(value).trim().toLowerCase() === 'global' ? 'global' : fallback;
}

export function parsePtbStopLossBumpLossRuleRows(
  cfg: Record<string, unknown>
): PtbStopLossBumpLossRuleRow[] {
  const rows: PtbStopLossBumpLossRuleRow[] = [];
  if (!Array.isArray(cfg.priceToBeatStopLossBumpLossRules)) {
    return rows;
  }

  for (const item of cfg.priceToBeatStopLossBumpLossRules as Record<string, unknown>[]) {
    if (!isRecord(item)) continue;
    rows.push({
      ...createEmptyPtbStopLossBumpLossRuleRow(),
      lossUsd: toStringValue(item.lossUsd),
      bumpValue: toStringValue(item.bumpValue),
    });
  }

  return rows;
}

function clearPtbStopLossBumpFormState(
  fields: Record<string, string>,
  rows: PtbStopLossBumpLossRuleRow[]
): void {
  fields.priceToBeatStopLossBumpEnabled = '';
  fields.priceToBeatStopLossBumpMode = '';
  fields.priceToBeatStopLossBumpAmount = '';
  fields.priceToBeatStopLossBumpMaxValue = '';
  fields.priceToBeatStopLossBumpUnit = '';
  fields.priceToBeatStopLossBumpScope = '';
  fields.priceToBeatStopLossBumpDecayWindows = '';
  rows.length = 0;
}

export function applyPtbStopLossBumpFormDefaults(
  fields: Record<string, string>,
  cfg: Record<string, unknown>,
  rows: PtbStopLossBumpLossRuleRow[]
): void {
  const explicitlyDisabled =
    (fields.priceToBeatStopLossBumpEnabled ?? '').trim().toLowerCase() === 'false' ||
    cfg.priceToBeatStopLossBumpEnabled === false;
  if (explicitlyDisabled) {
    clearPtbStopLossBumpFormState(fields, rows);
    return;
  }

  const enabled =
    (fields.priceToBeatStopLossBumpEnabled ?? '').trim().toLowerCase() === 'true' ||
    cfg.priceToBeatStopLossBumpEnabled === true ||
    rows.length > 0 ||
    toStringValue(cfg.priceToBeatStopLossBumpAmount).trim().length > 0 ||
    toStringValue(cfg.priceToBeatStopLossBumpMaxValue).trim().length > 0 ||
    toStringValue(cfg.priceToBeatStopLossBumpMode).trim().length > 0;
  if (!enabled) {
    return;
  }

  fields.priceToBeatStopLossBumpEnabled = 'true';
  const inferredMode: PtbStopLossBumpMode =
    toStringValue(cfg.priceToBeatStopLossBumpMode).trim().length > 0
      ? normalizePtbStopLossBumpMode(cfg.priceToBeatStopLossBumpMode)
      : rows.length > 0
        ? 'loss_table'
        : 'fixed';
  fields.priceToBeatStopLossBumpMode = normalizePtbStopLossBumpMode(
    fields.priceToBeatStopLossBumpMode || cfg.priceToBeatStopLossBumpMode,
    inferredMode
  );

  const fallbackUnit = normalizePtbStopLossBumpUnit(
    fields.priceToBeatMaxDiffUnit,
    'cent'
  );
  fields.priceToBeatStopLossBumpUnit = normalizePtbStopLossBumpUnit(
    fields.priceToBeatStopLossBumpUnit || cfg.priceToBeatStopLossBumpUnit,
    fallbackUnit
  );
  fields.priceToBeatStopLossBumpScope = normalizePtbStopLossBumpScope(
    fields.priceToBeatStopLossBumpScope || cfg.priceToBeatStopLossBumpScope
  );
}

export function buildPtbStopLossBumpLossRules(
  rows: PtbStopLossBumpLossRuleRow[]
): SerializedPtbStopLossBumpLossRule[] {
  return rows
    .map((row) => {
      const lossUsd = Number(row.lossUsd.trim());
      const bumpValue = Number(row.bumpValue.trim());
      if (!Number.isFinite(lossUsd) || lossUsd <= 0) return null;
      if (!Number.isFinite(bumpValue) || bumpValue <= 0) return null;
      return { lossUsd, bumpValue };
    })
    .filter((item): item is SerializedPtbStopLossBumpLossRule => item != null);
}

function clearPtbStopLossBumpBuildConfig(config: Record<string, unknown>) {
  delete config.priceToBeatStopLossBumpEnabled;
  delete config.priceToBeatStopLossBumpMode;
  delete config.priceToBeatStopLossBumpAmount;
  delete config.priceToBeatStopLossBumpLossRules;
  delete config.priceToBeatStopLossBumpMaxValue;
  delete config.priceToBeatStopLossBumpUnit;
  delete config.priceToBeatStopLossBumpScope;
  delete config.priceToBeatStopLossBumpDecayWindows;
}

function clearPrimaryPriceToBeatGuardBuildConfig(config: Record<string, unknown>) {
  delete config.priceToBeatMode;
  delete config.priceToBeatMaxDiff;
  delete config.priceToBeatMaxDiffUnit;
  clearPtbStopLossBumpBuildConfig(config);
  delete config.priceToBeatMaxPriceRelaxEnabled;
  delete config.priceToBeatMaxPriceRelaxMissCount;
  delete config.priceToBeatMaxPriceRelaxHistoryCount;
  delete config.priceToBeatMaxPriceRelaxMinValue;
  delete config.priceToBeatMaxPriceRelaxMinUnit;
  delete config.priceToBeatMaxPriceRelaxMinDepthUsd;
  delete config.priceToBeatMaxPriceRelaxStepMode;
  delete config.priceToBeatMaxPriceRelaxStepValue;
  delete config.priceToBeatMaxPriceRelaxStepUnit;
  delete config.reentryPriceToBeatMaxDiff;
  delete config.reentryPriceToBeatMaxDiffUnit;
  delete config.reentryThresholdDecay;
  delete config.notifyOnPriceToBeatGapBlocked;
  delete config.retryOnPriceToBeatGuardBlock;
}

export function normalizePtbStopLossBumpBuildConfig(
  config: Record<string, unknown>,
  form: NodeConfigFormState
): void {
  if (config.priceToBeatStopLossBumpEnabled !== true) {
    clearPtbStopLossBumpBuildConfig(config);
    return;
  }

  const configuredRows = buildPtbStopLossBumpLossRules(
    form.ptbStopLossBumpLossRuleRows || []
  );
  const inferredMode: PtbStopLossBumpMode =
    configuredRows.length > 0 ? 'loss_table' : 'fixed';
  const configuredMode = normalizePtbStopLossBumpMode(
    config.priceToBeatStopLossBumpMode,
    inferredMode
  );
  config.priceToBeatStopLossBumpMode = configuredMode;

  const bumpMaxValue = Number(toStringValue(config.priceToBeatStopLossBumpMaxValue).trim());
  if (Number.isFinite(bumpMaxValue) && bumpMaxValue > 0) {
    config.priceToBeatStopLossBumpMaxValue = bumpMaxValue;
  } else {
    delete config.priceToBeatStopLossBumpMaxValue;
  }

  const fallbackUnit =
    config.priceToBeatMode === 'manual' &&
    (config.priceToBeatMaxDiffUnit === 'usd' || config.priceToBeatMaxDiffUnit === 'cent')
      ? (config.priceToBeatMaxDiffUnit as 'usd' | 'cent')
      : 'usd';
  config.priceToBeatStopLossBumpUnit = normalizePtbStopLossBumpUnit(
    config.priceToBeatStopLossBumpUnit,
    fallbackUnit
  );
  config.priceToBeatStopLossBumpScope = normalizePtbStopLossBumpScope(
    config.priceToBeatStopLossBumpScope
  );

  const bumpDecayWindows = Number(
    toStringValue(config.priceToBeatStopLossBumpDecayWindows).trim()
  );
  if (Number.isInteger(bumpDecayWindows) && bumpDecayWindows > 0) {
    config.priceToBeatStopLossBumpDecayWindows = bumpDecayWindows;
  } else {
    delete config.priceToBeatStopLossBumpDecayWindows;
  }

  if (configuredMode === 'loss_table') {
    delete config.priceToBeatStopLossBumpAmount;
    if (configuredRows.length > 0) {
      config.priceToBeatStopLossBumpLossRules = configuredRows;
    } else {
      delete config.priceToBeatStopLossBumpLossRules;
    }
  } else {
    const bumpAmount = Number(toStringValue(config.priceToBeatStopLossBumpAmount).trim());
    if (Number.isFinite(bumpAmount) && bumpAmount > 0) {
      config.priceToBeatStopLossBumpAmount = bumpAmount;
    } else {
      delete config.priceToBeatStopLossBumpAmount;
    }
    delete config.priceToBeatStopLossBumpLossRules;
  }

  if (config.priceToBeatMode !== 'manual') {
    delete config.priceToBeatStopLossBumpMaxValue;
  }
}

export function normalizePrimaryPriceToBeatGuardBuildConfig(
  config: Record<string, unknown>,
  form: NodeConfigFormState
): void {
  if (config.priceToBeatGuardEnabled !== true) {
    clearPrimaryPriceToBeatGuardBuildConfig(config);
    return;
  }

  config.priceToBeatMode = normalizePtbMode(config.priceToBeatMode);
  if (config.priceToBeatMode === 'manual') {
    const priceToBeatUnitRaw = toStringValue(config.priceToBeatMaxDiffUnit).trim().toLowerCase();
    config.priceToBeatMaxDiffUnit = priceToBeatUnitRaw === 'cent' ? 'cent' : 'usd';
  } else {
    delete config.priceToBeatMaxDiff;
    delete config.priceToBeatMaxDiffUnit;
  }

  const relaxMissCount = Number(
    toStringValue(config.priceToBeatMaxPriceRelaxMissCount).trim()
  );
  if (Number.isInteger(relaxMissCount) && relaxMissCount > 0) {
    config.priceToBeatMaxPriceRelaxMissCount = relaxMissCount;
  } else {
    delete config.priceToBeatMaxPriceRelaxMissCount;
  }

  const relaxHistoryCount = Number(
    toStringValue(config.priceToBeatMaxPriceRelaxHistoryCount).trim()
  );
  if (Number.isInteger(relaxHistoryCount) && relaxHistoryCount > 0) {
    config.priceToBeatMaxPriceRelaxHistoryCount = relaxHistoryCount;
  } else {
    delete config.priceToBeatMaxPriceRelaxHistoryCount;
  }

  const relaxMinValue = Number(
    toStringValue(config.priceToBeatMaxPriceRelaxMinValue).trim()
  );
  if (Number.isFinite(relaxMinValue) && relaxMinValue > 0) {
    config.priceToBeatMaxPriceRelaxMinValue = relaxMinValue;
    const relaxMinUnitRaw = toStringValue(config.priceToBeatMaxPriceRelaxMinUnit)
      .trim()
      .toLowerCase();
    config.priceToBeatMaxPriceRelaxMinUnit =
      relaxMinUnitRaw === 'usd' || relaxMinUnitRaw === 'cent' ? relaxMinUnitRaw : 'usd';
  } else {
    delete config.priceToBeatMaxPriceRelaxMinValue;
    delete config.priceToBeatMaxPriceRelaxMinUnit;
  }

  const relaxMinDepthUsd = Number(
    toStringValue(config.priceToBeatMaxPriceRelaxMinDepthUsd).trim()
  );
  if (Number.isFinite(relaxMinDepthUsd) && relaxMinDepthUsd > 0) {
    config.priceToBeatMaxPriceRelaxMinDepthUsd = relaxMinDepthUsd;
  } else {
    delete config.priceToBeatMaxPriceRelaxMinDepthUsd;
  }

  const relaxStepModeRaw = toStringValue(config.priceToBeatMaxPriceRelaxStepMode)
    .trim()
    .toLowerCase();
  config.priceToBeatMaxPriceRelaxStepMode =
    relaxStepModeRaw === 'absolute' ? 'absolute' : 'percent';

  const relaxStepValue = Number(
    toStringValue(config.priceToBeatMaxPriceRelaxStepValue).trim()
  );
  if (
    Number.isFinite(relaxStepValue) &&
    relaxStepValue > 0 &&
    (config.priceToBeatMaxPriceRelaxStepMode !== 'percent' || relaxStepValue <= 100)
  ) {
    config.priceToBeatMaxPriceRelaxStepValue = relaxStepValue;
  } else {
    config.priceToBeatMaxPriceRelaxStepMode = 'percent';
    config.priceToBeatMaxPriceRelaxStepValue = 25;
  }

  if (config.priceToBeatMaxPriceRelaxStepMode === 'absolute') {
    const relaxStepUnitRaw = toStringValue(config.priceToBeatMaxPriceRelaxStepUnit)
      .trim()
      .toLowerCase();
    config.priceToBeatMaxPriceRelaxStepUnit = relaxStepUnitRaw === 'cent' ? 'cent' : 'usd';
  } else {
    delete config.priceToBeatMaxPriceRelaxStepUnit;
  }

  normalizePtbStopLossBumpBuildConfig(config, form);
}
