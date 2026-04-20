import { createEmptyPtbStopLossRuleRow } from './drafts';
import type { PtbGapUnit, PtbStopLossRuleRow } from './types';
import { isRecord, toStringValue } from './utils';

interface SerializedPtbStopLossRule {
  gapUsd: number;
  sizePct: number;
}

export function normalizePtbStopLossGapUnit(
  value: unknown,
  fallback: PtbGapUnit = 'usd'
): PtbGapUnit {
  const normalized = toStringValue(value).trim().toLowerCase();
  if (normalized === 'usd' || normalized === 'cent') {
    return normalized;
  }
  return fallback;
}

export function applyPtbStopLossFormDefaults(
  fields: Record<string, string>,
  cfg: Record<string, unknown>
): void {
  const primaryFallback = normalizePtbStopLossGapUnit(cfg.ptbStopLossGapUnit);
  const primaryConfigured =
    (fields.ptbStopLossEnabled ?? '').trim().toLowerCase() === 'true' ||
    Array.isArray(cfg.ptbStopLossRules) ||
    toStringValue(cfg.ptbStopLossGapUsd).trim().length > 0;
  if (primaryConfigured || (fields.ptbStopLossGapUnit ?? '').trim().length > 0) {
    fields.ptbStopLossGapUnit = normalizePtbStopLossGapUnit(
      fields.ptbStopLossGapUnit || cfg.ptbStopLossGapUnit,
      primaryFallback
    );
  }

  const counterFallback = normalizePtbStopLossGapUnit(
    cfg.counterLegPtbStopLossGapUnit,
    fields.ptbStopLossGapUnit === 'cent' ? 'cent' : 'usd'
  );
  const counterConfigured =
    (fields.counterLegPtbStopLossEnabled ?? '').trim().toLowerCase() === 'true' ||
    toStringValue(cfg.counterLegPtbStopLossGapUsd).trim().length > 0;
  if (counterConfigured || (fields.counterLegPtbStopLossGapUnit ?? '').trim().length > 0) {
    fields.counterLegPtbStopLossGapUnit = normalizePtbStopLossGapUnit(
      fields.counterLegPtbStopLossGapUnit || cfg.counterLegPtbStopLossGapUnit,
      counterFallback
    );
  }
}

export function parsePtbStopLossRuleRows(
  cfg: Record<string, unknown>,
  pairLockMode: boolean
): PtbStopLossRuleRow[] {
  const rows: PtbStopLossRuleRow[] = [];
  if (Array.isArray(cfg.ptbStopLossRules)) {
    for (const item of cfg.ptbStopLossRules as Record<string, unknown>[]) {
      if (!isRecord(item)) continue;
      rows.push({
        ...createEmptyPtbStopLossRuleRow(),
        gapUsd: toStringValue(item.gapUsd),
        sizePct: toStringValue(item.sizePct),
      });
    }
  }

  if (
    !pairLockMode &&
    rows.length === 0 &&
    (cfg.ptbStopLossEnabled === true ||
      (typeof cfg.ptbStopLossGapUsd === 'number' && Number.isFinite(cfg.ptbStopLossGapUsd)))
  ) {
    const legacyGapUsd = Number(cfg.ptbStopLossGapUsd);
    if (Number.isFinite(legacyGapUsd)) {
      rows.push({
        ...createEmptyPtbStopLossRuleRow(),
        gapUsd: String(legacyGapUsd),
        sizePct: '100',
      });
    }
  }

  return rows;
}

export function shouldEnablePtbStopLossFromConfig(
  cfg: Record<string, unknown>,
  rows: PtbStopLossRuleRow[]
): boolean {
  return rows.length > 0 || toStringValue(cfg.ptbStopLossGapUsd).trim().length > 0;
}

export function buildPtbStopLossRules(
  rows: PtbStopLossRuleRow[]
): SerializedPtbStopLossRule[] {
  return rows
    .map((row) => {
      const gapUsd = Number(row.gapUsd.trim());
      const sizePct = Number(row.sizePct.trim());
      if (!Number.isFinite(gapUsd)) return null;
      if (!Number.isFinite(sizePct) || sizePct <= 0 || sizePct > 100) return null;
      return { gapUsd, sizePct };
    })
    .filter((item): item is SerializedPtbStopLossRule => item != null);
}
