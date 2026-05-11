import { createEmptyEntryTimingProfileRow } from './drafts';
import type { EntryTimingProfileRow } from './types';
import { isRecord, toStringValue } from './utils';

interface SerializedEntryTimingProfile {
  startRemainingSec: number;
  endRemainingSec: number;
  maxPriceCent?: number;
  priceToBeatTriggerMinGap?: number;
  priceToBeatTriggerMaxGap?: number;
  sizeUsdc?: number;
}

function parsePositiveInteger(value: string): number | null {
  const parsed = Number(value.trim());
  return Number.isInteger(parsed) && parsed > 0 ? parsed : null;
}

function parseNonNegativeInteger(value: string): number | null {
  const parsed = Number(value.trim());
  return Number.isInteger(parsed) && parsed >= 0 ? parsed : null;
}

function parsePositiveFinite(value: string): number | null {
  const parsed = Number(value.trim());
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
}

export function parseTriggerMarketEntryTimingProfileRows(
  cfg: Record<string, unknown>
): EntryTimingProfileRow[] {
  const rows: EntryTimingProfileRow[] = [];
  if (!Array.isArray(cfg.entryTimingProfiles)) {
    return rows;
  }

  for (const item of cfg.entryTimingProfiles) {
    if (!isRecord(item)) continue;
    rows.push({
      ...createEmptyEntryTimingProfileRow(),
      startRemainingSec: toStringValue(item.startRemainingSec),
      endRemainingSec: toStringValue(item.endRemainingSec),
      maxPriceCent: toStringValue(item.maxPriceCent),
      priceToBeatTriggerMinGap: toStringValue(item.priceToBeatTriggerMinGap),
      priceToBeatTriggerMaxGap: toStringValue(item.priceToBeatTriggerMaxGap),
      sizeUsdc: toStringValue(item.sizeUsdc),
    });
  }

  return rows;
}

export function buildTriggerMarketEntryTimingProfiles(
  rows: EntryTimingProfileRow[]
): SerializedEntryTimingProfile[] {
  return rows
    .map((row) => {
      const startRemainingSec = parsePositiveInteger(row.startRemainingSec);
      const endRemainingSec = parseNonNegativeInteger(row.endRemainingSec);
      if (
        startRemainingSec == null ||
        endRemainingSec == null ||
        startRemainingSec <= endRemainingSec
      ) {
        return null;
      }

      const profile: SerializedEntryTimingProfile = {
        startRemainingSec,
        endRemainingSec,
      };

      const maxPriceCent = parsePositiveFinite(row.maxPriceCent);
      if (maxPriceCent != null && maxPriceCent <= 100) {
        profile.maxPriceCent = maxPriceCent;
      }

      const minGap = parsePositiveFinite(row.priceToBeatTriggerMinGap);
      if (minGap != null) {
        profile.priceToBeatTriggerMinGap = minGap;
      }

      const maxGap = parsePositiveFinite(row.priceToBeatTriggerMaxGap);
      if (
        maxGap != null &&
        (profile.priceToBeatTriggerMinGap == null ||
          maxGap >= profile.priceToBeatTriggerMinGap)
      ) {
        profile.priceToBeatTriggerMaxGap = maxGap;
      }

      const sizeUsdc = parsePositiveFinite(row.sizeUsdc);
      if (sizeUsdc != null) {
        profile.sizeUsdc = sizeUsdc;
      }

      return profile;
    })
    .filter((profile): profile is SerializedEntryTimingProfile => profile != null);
}

export function isEntryTimingProfileRowEmpty(row: EntryTimingProfileRow): boolean {
  return (
    row.startRemainingSec.trim().length === 0 &&
    row.endRemainingSec.trim().length === 0 &&
    row.maxPriceCent.trim().length === 0 &&
    row.priceToBeatTriggerMinGap.trim().length === 0 &&
    row.priceToBeatTriggerMaxGap.trim().length === 0 &&
    row.sizeUsdc.trim().length === 0
  );
}

export function isEntryTimingProfileRowComplete(row: EntryTimingProfileRow): boolean {
  return (
    row.startRemainingSec.trim().length > 0 &&
    row.endRemainingSec.trim().length > 0 &&
    parsePositiveInteger(row.startRemainingSec) != null &&
    parseNonNegativeInteger(row.endRemainingSec) != null
  );
}
