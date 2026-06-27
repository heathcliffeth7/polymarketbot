import type { TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { toFiniteNumber } from './shared';
import { pushNodeError } from './validation-core';

export function validateEq77RiskCap(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>
) {
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvEq77RiskCapEnabled,
    'priceToBeatIvEq77RiskCapEnabled',
    'invalid_price_to_beat_iv_eq77_risk_cap_enabled'
  );
  for (const key of [
    'priceToBeatIvRiskScoreCleanMax',
    'priceToBeatIvRiskScoreModerateMax',
    'priceToBeatIvRiskScoreHighMax',
    'priceToBeatIvMaxRiskHaircutCent',
    'priceToBeatIvCexUnconfirmedRiskPoints',
    'priceToBeatIvCexConflictRiskPoints',
  ]) {
    validateOptionalNonNegativeNumber(issues, node, config[key], key, `invalid_${toSnakeCase(key)}`);
  }
  for (const key of [
    'priceToBeatIvModerateRiskMaxPriceCent',
    'priceToBeatIvHighRiskMaxPriceCent',
    'priceToBeatIvDeepValueMaxPriceCent',
  ]) {
    validateOptionalCentPrice(issues, node, config[key], key, `invalid_${toSnakeCase(key)}`);
  }
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvWaitForPriceEnabled,
    'priceToBeatIvWaitForPriceEnabled',
    'invalid_price_to_beat_iv_wait_for_price_enabled'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvRecheckBeforeSubmit,
    'priceToBeatIvRecheckBeforeSubmit',
    'invalid_price_to_beat_iv_recheck_before_submit'
  );
  validateOptionalCentPriceAllowZero(
    issues,
    node,
    config.priceToBeatIvOddsMaxSpreadCent,
    'priceToBeatIvOddsMaxSpreadCent',
    'invalid_price_to_beat_iv_odds_max_spread'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvPassiveBidEnabled,
    'priceToBeatIvPassiveBidEnabled',
    'invalid_price_to_beat_iv_passive_bid_enabled'
  );
  validateOptionalPositiveInteger(
    issues,
    node,
    config.priceToBeatIvPassiveBidTtlMs,
    'priceToBeatIvPassiveBidTtlMs',
    'invalid_price_to_beat_iv_passive_bid_ttl'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvWaitRepriceGuardEnabled,
    'priceToBeatIvWaitRepriceGuardEnabled',
    'invalid_price_to_beat_iv_wait_reprice_guard_enabled'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvLowQualityEdgeRecheckEnabled,
    'priceToBeatIvLowQualityEdgeRecheckEnabled',
    'invalid_price_to_beat_iv_low_quality_edge_recheck_enabled'
  );
  for (const key of ['priceToBeatIvLowQualityGapMargin', 'priceToBeatIvLowQualityEdgeMarginCent']) {
    validateOptionalNonNegativeNumber(issues, node, config[key], key, `invalid_${toSnakeCase(key)}`);
  }
  for (const key of [
    'priceToBeatIvWaitMaxAgeMsEarly',
    'priceToBeatIvWaitMaxAgeMsMid',
    'priceToBeatIvWaitMaxAgeMsLate',
  ]) {
    validateOptionalPositiveInteger(issues, node, config[key], key, `invalid_${toSnakeCase(key)}`);
  }
  for (const key of [
    'priceToBeatIvWaitInitialAskMaxOverCapCent',
    'priceToBeatIvFallingIntoCapDropCentEarly',
    'priceToBeatIvFallingIntoCapDropCentMid',
    'priceToBeatIvFallingIntoCapDropCentLate',
    'priceToBeatIvLateExpensiveVwapCent',
    'priceToBeatIvLateExpensiveMinQCent',
  ]) {
    validateOptionalCentPriceAllowZero(
      issues,
      node,
      config[key],
      key,
      `invalid_${toSnakeCase(key)}`
    );
  }
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvFallingIntoCapGuardEnabled,
    'priceToBeatIvFallingIntoCapGuardEnabled',
    'invalid_price_to_beat_iv_falling_into_cap_guard_enabled'
  );
  validateOptionalBoolean(
    issues,
    node,
    config.priceToBeatIvLateExpensiveEntryGuardEnabled,
    'priceToBeatIvLateExpensiveEntryGuardEnabled',
    'invalid_price_to_beat_iv_late_expensive_entry_guard_enabled'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvLateExpensiveSeconds,
    'priceToBeatIvLateExpensiveSeconds',
    'invalid_price_to_beat_iv_late_expensive_seconds'
  );
  validateOptionalNonNegativeNumber(
    issues,
    node,
    config.priceToBeatIvLateExpensiveMinGapStrengthExtra,
    'priceToBeatIvLateExpensiveMinGapStrengthExtra',
    'invalid_price_to_beat_iv_late_expensive_min_gap_strength_extra'
  );
  validateMixedCexGapFailGuards(issues, node, config);
}

function validateMixedCexGapFailGuards(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>
) {
  for (const key of [
    'priceToBeatIvGapFailMixedCexGuardEnabled',
    'priceToBeatIvLateExpensiveMixedCexGuardEnabled',
    'priceToBeatIvLateExpensiveMixedCexRequireGapFailOrLagHigh',
    'priceToBeatIvChainlinkCexLagNoBookGuardEnabled',
    'priceToBeatIvChainlinkCexLagNoBookRequireNonStrongCex',
  ]) {
    validateOptionalBoolean(issues, node, config[key], key, `invalid_${toSnakeCase(key)}`);
  }
  for (const key of [
    'priceToBeatIvGapFailMixedCexMaxSeconds',
    'priceToBeatIvLateExpensiveMixedCexSeconds',
    'priceToBeatIvChainlinkCexLagNoBookMaxSeconds',
  ]) {
    validateOptionalNonNegativeNumber(issues, node, config[key], key, `invalid_${toSnakeCase(key)}`);
  }
  validateOptionalCentPriceAllowZero(
    issues,
    node,
    config.priceToBeatIvLateExpensiveMixedCexMinVwapCent,
    'priceToBeatIvLateExpensiveMixedCexMinVwapCent',
    'invalid_price_to_beat_iv_late_expensive_mixed_cex_min_vwap_cent'
  );
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

function validateOptionalPositiveInteger(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string,
  code: string
) {
  if (value == null) return;
  const parsed = toFiniteNumber(value);
  if (parsed == null || !Number.isInteger(parsed) || parsed <= 0) {
    pushNodeError(issues, node, code, `action.place_order ${key} must be an integer > 0.`);
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

function validateOptionalCentPrice(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
  key: string,
  code: string
) {
  if (value == null) return;
  const parsed = toFiniteNumber(value);
  if (parsed == null || parsed <= 0 || parsed > 100) {
    pushNodeError(issues, node, code, `action.place_order ${key} must be in (0, 100].`);
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

function toSnakeCase(value: string): string {
  return value.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`).replace(/^_/, '');
}
