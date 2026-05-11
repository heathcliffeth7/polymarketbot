import type { TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { isRecord, toBooleanish, toFiniteNumber } from './shared';
import { pushNodeError } from './validation-core';

const LEGACY_AUTO_TUNE_KEYS = [
  'autoTuneEnabled',
  'autoTuneMode',
  'autoTuneSampleMarkets',
  'autoTuneMinEligibleMarkets',
  'autoTuneCooldownMarketsAfterAdvice',
  'autoTuneDedupeSameAdviceForMarkets',
] as const;

export function validateActionPlaceOrderAutoTuneConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>
) {
  const source = readAutoTuneConfigSource(issues, node, config);
  if (!source) return;

  const enabled = toBooleanish(source.enabled);
  if (source.enabled != null && enabled == null) {
    pushNodeError(
      issues,
      node,
      'invalid_auto_tune_enabled',
      'action.place_order autoTune.enabled must be boolean (true/false).'
    );
  }

  const mode = String(source.mode ?? '').trim().toLowerCase();
  if (mode && mode !== 'advice') {
    pushNodeError(
      issues,
      node,
      'invalid_auto_tune_mode',
      'action.place_order autoTune.mode must be advice.'
    );
  }

  const sampleMarkets = validateAutoTuneInteger(
    issues,
    node,
    source.sampleMarkets,
    'sampleMarkets',
    false
  );
  const minEligibleMarkets = validateAutoTuneInteger(
    issues,
    node,
    source.minEligibleMarkets,
    'minEligibleMarkets',
    false
  );
  validateAutoTuneInteger(
    issues,
    node,
    source.cooldownMarketsAfterAdvice,
    'cooldownMarketsAfterAdvice',
    true
  );
  validateAutoTuneInteger(
    issues,
    node,
    source.dedupeSameAdviceForMarkets,
    'dedupeSameAdviceForMarkets',
    true
  );
  if (
    sampleMarkets != null &&
    minEligibleMarkets != null &&
    minEligibleMarkets > sampleMarkets
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_auto_tune_min_eligible_markets',
      'action.place_order autoTune.minEligibleMarkets must be <= autoTune.sampleMarkets.'
    );
  }
}

function readAutoTuneConfigSource(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>
): Record<string, unknown> | null {
  if (config.autoTune != null) {
    if (!isRecord(config.autoTune)) {
      pushNodeError(
        issues,
        node,
        'invalid_auto_tune_config',
        'action.place_order autoTune must be an object.'
      );
      return null;
    }
    return config.autoTune;
  }

  if (!LEGACY_AUTO_TUNE_KEYS.some((key) => config[key] != null)) {
    return null;
  }
  return {
    enabled: config.autoTuneEnabled,
    mode: config.autoTuneMode,
    sampleMarkets: config.autoTuneSampleMarkets,
    minEligibleMarkets: config.autoTuneMinEligibleMarkets,
    cooldownMarketsAfterAdvice: config.autoTuneCooldownMarketsAfterAdvice,
    dedupeSameAdviceForMarkets: config.autoTuneDedupeSameAdviceForMarkets,
  };
}

function validateAutoTuneInteger(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  raw: unknown,
  key: string,
  allowZero: boolean
): number | null {
  if (raw == null) return null;
  const value = toFiniteNumber(raw);
  const valid = value != null && Number.isInteger(value) && (allowZero ? value >= 0 : value > 0);
  if (!valid) {
    pushNodeError(
      issues,
      node,
      `invalid_auto_tune_${key}`,
      `action.place_order autoTune.${key} must be an integer ${allowZero ? '>= 0' : '> 0'}.`
    );
    return null;
  }
  return value;
}
