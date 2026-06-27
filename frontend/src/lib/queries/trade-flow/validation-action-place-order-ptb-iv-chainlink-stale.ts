import type { TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { toFiniteNumber } from './shared';
import { pushNodeError, pushNodeWarning } from './validation-core';

export function validatePtbIvChainlinkStaleConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>
) {
  validateOptionalNonNegativeInteger(
    issues,
    node,
    config.priceToBeatIvChainlinkStaleMs,
    'priceToBeatIvChainlinkStaleMs',
    'invalid_price_to_beat_iv_chainlink_stale_ms'
  );
  const hasIvThreshold = config.priceToBeatIvChainlinkStaleMs != null;
  const hasEntryQualityThreshold = config.priceToBeatIvEntryQualityChainlinkMaxAgeMs != null;
  if (hasIvThreshold === hasEntryQualityThreshold) return;
  pushNodeWarning(
    issues,
    node,
    'price_to_beat_iv_chainlink_stale_threshold_pair_incomplete',
    'action.place_order Chainlink stale thresholds should be set together: priceToBeatIvChainlinkStaleMs and priceToBeatIvEntryQualityChainlinkMaxAgeMs.'
  );
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
