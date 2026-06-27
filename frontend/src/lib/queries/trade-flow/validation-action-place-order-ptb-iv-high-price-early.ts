import type { TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { toFiniteNumber } from './shared';
import { pushNodeError } from './validation-core';

const BOOLEAN_KEYS = [
  'priceToBeatIvHighPriceEarlyReversalGuardEnabled',
  'priceToBeatIvHighPriceEarlyQExtremeRequireBinanceQ',
  'priceToBeatIvHighPriceEarlyQExtremeRequireCleanStrongCex',
];

const CENT_KEYS = [
  'priceToBeatIvHighPriceEarlyRefCent',
  'priceToBeatIvHighPriceEarlyQExtremeCent',
];

const NON_NEGATIVE_NUMBER_KEYS = [
  'priceToBeatIvHighPriceEarlyRemainingSec',
  'priceToBeatIvHighPriceEarlyStaleGapAdd',
  'priceToBeatIvHighPriceEarlyBinanceMissingGapAdd',
  'priceToBeatIvHighPriceEarlyQExtremeMinGapStrength',
];

const NON_NEGATIVE_INTEGER_KEYS = [
  'priceToBeatIvHighPriceEarlyMaxStaleMs',
  'priceToBeatIvHighPriceEarlyQExtremeMaxStaleMs',
];

export function hasPtbIvHighPriceEarlyConfig(config: Record<string, unknown>): boolean {
  return [
    ...BOOLEAN_KEYS,
    ...CENT_KEYS,
    ...NON_NEGATIVE_NUMBER_KEYS,
    ...NON_NEGATIVE_INTEGER_KEYS,
  ].some((key) => config[key] != null);
}

export function validatePtbIvHighPriceEarlyConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>
) {
  for (const key of BOOLEAN_KEYS) {
    validateOptionalBoolean(issues, node, config[key], key, `invalid_${toSnakeCase(key)}`);
  }
  for (const key of CENT_KEYS) {
    validateOptionalCentPriceAllowZero(issues, node, config[key], key, `invalid_${toSnakeCase(key)}`);
  }
  for (const key of NON_NEGATIVE_NUMBER_KEYS) {
    validateOptionalNonNegativeNumber(
      issues,
      node,
      config[key],
      key,
      `invalid_${toSnakeCase(key)}`
    );
  }
  for (const key of NON_NEGATIVE_INTEGER_KEYS) {
    validateOptionalNonNegativeInteger(
      issues,
      node,
      config[key],
      key,
      `invalid_${toSnakeCase(key)}`
    );
  }
}

function validateOptionalBoolean(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string,
  code: string
) {
  if (value == null) return;
  if (typeof value !== 'boolean') {
    pushNodeError(issues, node, code, `action.place_order ${key} must be boolean.`);
  }
}

function validateOptionalCentPriceAllowZero(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string,
  code: string
) {
  if (value == null) return;
  const parsed = toFiniteNumber(value);
  if (parsed == null || parsed < 0 || parsed > 100) {
    pushNodeError(issues, node, code, `action.place_order ${key} must be between 0 and 100.`);
  }
}

function validateOptionalNonNegativeNumber(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string,
  code: string
) {
  if (value == null) return;
  const parsed = toFiniteNumber(value);
  if (parsed == null || parsed < 0) {
    pushNodeError(issues, node, code, `action.place_order ${key} must be >= 0.`);
  }
}

function validateOptionalNonNegativeInteger(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string,
  code: string
) {
  if (value == null) return;
  const parsed = toFiniteNumber(value);
  if (parsed == null || !Number.isInteger(parsed) || parsed < 0) {
    pushNodeError(issues, node, code, `action.place_order ${key} must be an integer >= 0.`);
  }
}

function toSnakeCase(value: string): string {
  return value.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`).replace(/^_/, '');
}
