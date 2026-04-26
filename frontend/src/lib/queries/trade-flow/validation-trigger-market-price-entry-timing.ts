import type { TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import type { PtbMode } from '@/lib/trade-flow-config-mappers/ptb-modes';
import { hasProvidedValue, isRecord, toFiniteNumber, toTrimmedString } from './shared';
import { pushNodeError } from './validation-core';

interface ParsedEntryTimingProfileRange {
  startRemainingSec: number;
  endRemainingSec: number;
}

function profileRangesOverlap(
  left: ParsedEntryTimingProfileRange,
  right: ParsedEntryTimingProfileRange
): boolean {
  return (
    Math.min(left.startRemainingSec, right.startRemainingSec) >
    Math.max(left.endRemainingSec, right.endRemainingSec)
  );
}

export function validateTriggerMarketPriceEntryTimingProfiles(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>,
  {
    autoScope,
    repeatMode,
    priceToBeatTriggerEnabled,
    normalizedPtbMode,
  }: {
    autoScope: boolean;
    repeatMode: string;
    priceToBeatTriggerEnabled: boolean;
    normalizedPtbMode: PtbMode | null;
  }
) {
  const rawProfiles = config.entryTimingProfiles;
  if (rawProfiles == null) return;

  if (!Array.isArray(rawProfiles)) {
    pushNodeError(
      issues,
      node,
      'invalid_entry_timing_profiles',
      'trigger.market_price entryTimingProfiles must be an array.'
    );
    return;
  }

  if (!autoScope) {
    pushNodeError(
      issues,
      node,
      'invalid_entry_timing_profiles_scope',
      'trigger.market_price entryTimingProfiles are only valid when marketMode is auto_scope.'
    );
  }
  if (repeatMode !== 'once') {
    pushNodeError(
      issues,
      node,
      'invalid_entry_timing_profiles_repeat_mode',
      'trigger.market_price entryTimingProfiles require repeatMode=once.'
    );
  }
  if (toTrimmedString(config.cycleWindowMode).toLowerCase() && toTrimmedString(config.cycleWindowMode).toLowerCase() !== 'off') {
    pushNodeError(
      issues,
      node,
      'entry_timing_profiles_disallow_cycle_window',
      'trigger.market_price entryTimingProfiles cannot be combined with cycleWindowMode in v1.'
    );
  }

  const parsedRanges: ParsedEntryTimingProfileRange[] = [];
  for (const [index, item] of rawProfiles.entries()) {
    if (!isRecord(item)) {
      pushNodeError(
        issues,
        node,
        'invalid_entry_timing_profile_row',
        `trigger.market_price entryTimingProfiles[${index}] must be an object.`
      );
      continue;
    }

    const startRemainingSec = toFiniteNumber(item.startRemainingSec);
    const endRemainingSec = toFiniteNumber(item.endRemainingSec);
    if (
      startRemainingSec == null ||
      !Number.isInteger(startRemainingSec) ||
      startRemainingSec <= 0
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_entry_timing_profile_start',
        `trigger.market_price entryTimingProfiles[${index}].startRemainingSec must be an integer > 0.`
      );
      continue;
    }
    if (
      endRemainingSec == null ||
      !Number.isInteger(endRemainingSec) ||
      endRemainingSec < 0
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_entry_timing_profile_end',
        `trigger.market_price entryTimingProfiles[${index}].endRemainingSec must be an integer >= 0.`
      );
      continue;
    }
    if (startRemainingSec <= endRemainingSec) {
      pushNodeError(
        issues,
        node,
        'invalid_entry_timing_profile_range',
        `trigger.market_price entryTimingProfiles[${index}] requires startRemainingSec > endRemainingSec.`
      );
      continue;
    }

    const maxPriceCent = toFiniteNumber(item.maxPriceCent);
    if (
      hasProvidedValue(item.maxPriceCent) &&
      (maxPriceCent == null || maxPriceCent <= 0 || maxPriceCent > 100)
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_entry_timing_profile_max_price_cent',
        `trigger.market_price entryTimingProfiles[${index}].maxPriceCent must be in (0, 100].`
      );
    }

    const sizeUsdc = toFiniteNumber(item.sizeUsdc);
    if (hasProvidedValue(item.sizeUsdc) && (sizeUsdc == null || sizeUsdc <= 0)) {
      pushNodeError(
        issues,
        node,
        'invalid_entry_timing_profile_size_usdc',
        `trigger.market_price entryTimingProfiles[${index}].sizeUsdc must be > 0.`
      );
    }

    const minGap = toFiniteNumber(item.priceToBeatTriggerMinGap);
    const maxGap = toFiniteNumber(item.priceToBeatTriggerMaxGap);
    const hasPtbOverride =
      hasProvidedValue(item.priceToBeatTriggerMinGap) ||
      hasProvidedValue(item.priceToBeatTriggerMaxGap);
    if (hasPtbOverride && !priceToBeatTriggerEnabled) {
      pushNodeError(
        issues,
        node,
        'entry_timing_profiles_require_ptb_gate',
        'trigger.market_price entryTimingProfiles PTB gap overrides require priceToBeatTriggerEnabled=true.'
      );
    }
    if (hasPtbOverride && normalizedPtbMode !== 'manual') {
      pushNodeError(
        issues,
        node,
        'entry_timing_profiles_require_manual_ptb',
        'trigger.market_price entryTimingProfiles PTB gap overrides require priceToBeatMode=manual.'
      );
    }
    if (
      hasProvidedValue(item.priceToBeatTriggerMinGap) &&
      (minGap == null || minGap <= 0)
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_entry_timing_profile_min_gap',
        `trigger.market_price entryTimingProfiles[${index}].priceToBeatTriggerMinGap must be > 0.`
      );
    }
    if (
      hasProvidedValue(item.priceToBeatTriggerMaxGap) &&
      (maxGap == null || maxGap <= 0 || (minGap != null && maxGap < minGap))
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_entry_timing_profile_max_gap',
        `trigger.market_price entryTimingProfiles[${index}].priceToBeatTriggerMaxGap must be >= min gap when provided.`
      );
    }

    parsedRanges.push({
      startRemainingSec,
      endRemainingSec,
    });
  }

  for (let index = 0; index < parsedRanges.length; index += 1) {
    for (let otherIndex = index + 1; otherIndex < parsedRanges.length; otherIndex += 1) {
      if (!profileRangesOverlap(parsedRanges[index], parsedRanges[otherIndex])) continue;
      pushNodeError(
        issues,
        node,
        'overlapping_entry_timing_profiles',
        'trigger.market_price entryTimingProfiles windows must not overlap.'
      );
      return;
    }
  }
}
