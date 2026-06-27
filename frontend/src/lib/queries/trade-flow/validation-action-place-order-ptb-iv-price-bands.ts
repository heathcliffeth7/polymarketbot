import type { TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { pushNodeError } from './validation-core';

interface ParsedPriceBand {
  minPriceCent: number;
  maxPriceCent: number;
}

interface ParsedTimeRule {
  startRemainingSec: number;
  endRemainingSec: number;
}

const PRICE_BAND_SOURCE_VALUES = new Set(['execution_vwap']);
const PRICE_BAND_COMBINE_MODE_VALUES = new Set(['strictest']);
const BAND_BOOLEAN_KEYS = [
  'requireCleanCex',
  'requireCexWithDirection',
  'requireBookConfirmation',
  'requireNoChainlinkStalePenalty',
  'requireNoMixedCex',
] as const;
const BAND_NUMERIC_KEYS = [
  'minQCent',
  'minFairEdgeCent',
  'maxSpreadCent',
] as const;
const TIME_RULE_BOOLEAN_KEYS = [
  'requireBookConfirmation',
  'requireNoChainlinkStalePenalty',
  'requireNoMixedCex',
] as const;
const TIME_RULE_NUMERIC_KEYS = [
  'minQCent',
  'minFairEdgeCent',
  'maxSpreadCent',
] as const;

export function hasPtbIvPriceBandGuardConfig(config: Record<string, unknown>): boolean {
  return (
    config.priceToBeatIvPriceBandGuardEnabled != null ||
    config.priceToBeatIvPriceBandSource != null ||
    config.priceToBeatIvPriceBandCombineMode != null ||
    config.priceToBeatIvPriceBands != null
  );
}

export function validatePtbIvPriceBandGuardConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>
): void {
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvPriceBandGuardEnabled,
    'priceToBeatIvPriceBandGuardEnabled'
  );
  validateOptionalEnum(
    issues,
    node,
    config.priceToBeatIvPriceBandSource,
    PRICE_BAND_SOURCE_VALUES,
    'invalid_price_to_beat_iv_price_band_source',
    'priceToBeatIvPriceBandSource'
  );
  validateOptionalEnum(
    issues,
    node,
    config.priceToBeatIvPriceBandCombineMode,
    PRICE_BAND_COMBINE_MODE_VALUES,
    'invalid_price_to_beat_iv_price_band_combine_mode',
    'priceToBeatIvPriceBandCombineMode'
  );
  validatePriceBands(issues, node, config.priceToBeatIvPriceBands);
}

function validatePriceBands(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown
): void {
  if (value == null) return;
  if (!Array.isArray(value)) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_price_bands',
      'action.place_order priceToBeatIvPriceBands must be an array.'
    );
    return;
  }

  const parsedBands: ParsedPriceBand[] = [];
  value.forEach((item, index) => {
    if (!isRecord(item)) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_iv_price_band',
        `action.place_order priceToBeatIvPriceBands[${index}] must be an object.`
      );
      return;
    }
    const parsed = validatePriceBand(issues, node, item, index);
    if (parsed) parsedBands.push(parsed);
  });

  for (let left = 0; left < parsedBands.length; left += 1) {
    for (let right = left + 1; right < parsedBands.length; right += 1) {
      if (priceBandsOverlap(parsedBands[left], parsedBands[right])) {
        pushNodeError(
          issues,
          node,
          'overlapping_price_to_beat_iv_price_bands',
          'action.place_order priceToBeatIvPriceBands ranges must not overlap.'
        );
      }
    }
  }
}

function validatePriceBand(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  band: Record<string, unknown>,
  index: number
): ParsedPriceBand | null {
  const minPriceCent = toFiniteNumber(band.minPriceCent);
  const maxPriceCent = toFiniteNumber(band.maxPriceCent);
  if (minPriceCent == null || minPriceCent < 0 || minPriceCent > 100) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_price_band_min_price',
      `action.place_order priceToBeatIvPriceBands[${index}].minPriceCent must be between 0 and 100.`
    );
  }
  if (maxPriceCent == null || maxPriceCent <= 0 || maxPriceCent > 100) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_price_band_max_price',
      `action.place_order priceToBeatIvPriceBands[${index}].maxPriceCent must be in (0, 100].`
    );
  }
  if (minPriceCent != null && maxPriceCent != null && minPriceCent >= maxPriceCent) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_price_band_range',
      `action.place_order priceToBeatIvPriceBands[${index}] must have minPriceCent < maxPriceCent.`
    );
  }
  for (const key of BAND_NUMERIC_KEYS) validateOptionalCent(issues, node, band[key], index, key);
  for (const key of BAND_BOOLEAN_KEYS) validateNestedBoolean(issues, node, band[key], index, key);
  validateTimeRules(issues, node, band.timeRules, index);

  if (minPriceCent == null || maxPriceCent == null || minPriceCent >= maxPriceCent) return null;
  return { minPriceCent, maxPriceCent };
}

function validateTimeRules(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  bandIndex: number
): void {
  if (value == null) return;
  if (!Array.isArray(value)) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_price_band_time_rules',
      `action.place_order priceToBeatIvPriceBands[${bandIndex}].timeRules must be an array.`
    );
    return;
  }

  const parsedRules: ParsedTimeRule[] = [];
  value.forEach((item, index) => {
    if (!isRecord(item)) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_iv_price_band_time_rule',
        `action.place_order priceToBeatIvPriceBands[${bandIndex}].timeRules[${index}] must be an object.`
      );
      return;
    }
    const parsed = validateTimeRule(issues, node, item, bandIndex, index);
    if (parsed) parsedRules.push(parsed);
  });

  for (let left = 0; left < parsedRules.length; left += 1) {
    for (let right = left + 1; right < parsedRules.length; right += 1) {
      if (timeRulesOverlap(parsedRules[left], parsedRules[right])) {
        pushNodeError(
          issues,
          node,
          'overlapping_price_to_beat_iv_price_band_time_rules',
          `action.place_order priceToBeatIvPriceBands[${bandIndex}].timeRules ranges must not overlap.`
        );
      }
    }
  }
}

function validateTimeRule(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  rule: Record<string, unknown>,
  bandIndex: number,
  index: number
): ParsedTimeRule | null {
  const startRemainingSec = toFiniteNumber(rule.startRemainingSec);
  const endRemainingSec = toFiniteNumber(rule.endRemainingSec);
  if (startRemainingSec == null || startRemainingSec <= 0) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_price_band_time_rule_start',
      `action.place_order priceToBeatIvPriceBands[${bandIndex}].timeRules[${index}].startRemainingSec must be > 0.`
    );
  }
  if (endRemainingSec == null || endRemainingSec < 0) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_price_band_time_rule_end',
      `action.place_order priceToBeatIvPriceBands[${bandIndex}].timeRules[${index}].endRemainingSec must be >= 0.`
    );
  }
  if (
    startRemainingSec != null &&
    endRemainingSec != null &&
    startRemainingSec <= endRemainingSec
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_price_band_time_rule_range',
      `action.place_order priceToBeatIvPriceBands[${bandIndex}].timeRules[${index}] must have startRemainingSec > endRemainingSec.`
    );
  }
  validateOptionalNonNegativeNumber(
    issues,
    node,
    rule.minGapStrength,
    bandIndex,
    `timeRules[${index}].minGapStrength`
  );
  for (const key of TIME_RULE_NUMERIC_KEYS) {
    validateOptionalCent(issues, node, rule[key], bandIndex, `timeRules[${index}].${key}`);
  }
  for (const key of TIME_RULE_BOOLEAN_KEYS) {
    validateNestedBoolean(issues, node, rule[key], bandIndex, `timeRules[${index}].${key}`);
  }

  if (
    startRemainingSec == null ||
    endRemainingSec == null ||
    startRemainingSec <= endRemainingSec
  ) {
    return null;
  }
  return { startRemainingSec, endRemainingSec };
}

function validateOptionalEnum(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  allowed: Set<string>,
  code: string,
  key: string
): void {
  if (value == null) return;
  const normalized = String(value).trim().toLowerCase();
  if (!allowed.has(normalized)) {
    pushNodeError(issues, node, code, `action.place_order ${key} is not supported.`);
  }
}

function validateOptionalBoolean(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string
): void {
  if (value == null) return;
  if (typeof value !== 'boolean') {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_price_band_guard_enabled',
      `action.place_order ${key} must be boolean.`
    );
  }
}

function validateNestedBoolean(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  bandIndex: number,
  key: string
): void {
  if (value == null) return;
  if (typeof value !== 'boolean') {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_price_band_boolean',
      `action.place_order priceToBeatIvPriceBands[${bandIndex}].${key} must be boolean.`
    );
  }
}

function validateOptionalCent(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  bandIndex: number,
  key: string
): void {
  if (value == null) return;
  const parsed = toFiniteNumber(value);
  if (parsed == null || parsed < 0 || parsed > 100) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_price_band_numeric',
      `action.place_order priceToBeatIvPriceBands[${bandIndex}].${key} must be between 0 and 100.`
    );
  }
}

function validateOptionalNonNegativeNumber(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  bandIndex: number,
  key: string
): void {
  if (value == null) return;
  const parsed = toFiniteNumber(value);
  if (parsed == null || parsed < 0) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_iv_price_band_numeric',
      `action.place_order priceToBeatIvPriceBands[${bandIndex}].${key} must be >= 0.`
    );
  }
}

function priceBandsOverlap(left: ParsedPriceBand, right: ParsedPriceBand): boolean {
  return Math.max(left.minPriceCent, right.minPriceCent) <
    Math.min(left.maxPriceCent, right.maxPriceCent);
}

function timeRulesOverlap(left: ParsedTimeRule, right: ParsedTimeRule): boolean {
  return Math.max(left.endRemainingSec, right.endRemainingSec) <
    Math.min(left.startRemainingSec, right.startRemainingSec);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

function toFiniteNumber(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return null;
}
