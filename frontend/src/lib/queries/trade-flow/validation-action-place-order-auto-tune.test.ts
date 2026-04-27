import assert from 'node:assert/strict';
import test from 'node:test';

import { validateActionPlaceOrderConfig } from '@/lib/queries/trade-flow/validation-action-place-order';
import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';

test('validateActionPlaceOrderConfig accepts valid autoTune advice config', () => {
  const issues = validateAutoTuneConfig({
    enabled: true,
    mode: 'advice',
    sampleMarkets: 6,
    minEligibleMarkets: 4,
    cooldownMarketsAfterAdvice: 2,
    dedupeSameAdviceForMarkets: 4,
  });

  assert.deepEqual(autoTuneIssueCodes(issues), []);
});

test('validateActionPlaceOrderConfig rejects invalid autoTune mode', () => {
  const issues = validateAutoTuneConfig({
    enabled: true,
    mode: 'auto',
  });

  assert.deepEqual(autoTuneIssueCodes(issues), ['invalid_auto_tune_mode']);
});

test('validateActionPlaceOrderConfig rejects invalid autoTune numeric fields', () => {
  const issues = validateAutoTuneConfig({
    enabled: true,
    mode: 'advice',
    sampleMarkets: 0,
    minEligibleMarkets: 1.5,
    cooldownMarketsAfterAdvice: -1,
    dedupeSameAdviceForMarkets: 'x',
  });

  assert.deepEqual(autoTuneIssueCodes(issues), [
    'invalid_auto_tune_sampleMarkets',
    'invalid_auto_tune_minEligibleMarkets',
    'invalid_auto_tune_cooldownMarketsAfterAdvice',
    'invalid_auto_tune_dedupeSameAdviceForMarkets',
  ]);
});

test('validateActionPlaceOrderConfig rejects minEligibleMarkets above sampleMarkets', () => {
  const issues = validateAutoTuneConfig({
    enabled: true,
    mode: 'advice',
    sampleMarkets: 3,
    minEligibleMarkets: 4,
  });

  assert.deepEqual(autoTuneIssueCodes(issues), ['invalid_auto_tune_min_eligible_markets']);
});

test('validateActionPlaceOrderConfig rejects non-object autoTune config', () => {
  const issues = validateActionConfig({ autoTune: '' });

  assert.deepEqual(autoTuneIssueCodes(issues), ['invalid_auto_tune_config']);
});

function validateAutoTuneConfig(autoTune: unknown): TradeFlowValidationIssue[] {
  return validateActionConfig({ autoTune });
}

function validateActionConfig(config: Record<string, unknown>): TradeFlowValidationIssue[] {
  const action: TradeFlowNode = {
    key: 'action_test',
    type: 'action.place_order',
    positionX: 0,
    positionY: 0,
    config: {
      side: 'buy',
      executionMode: 'market',
      sizeMode: 'usdc',
      sizeUsdc: 5,
      marketSlug: 'btc-updown-5m-1777305900',
      tokenId: 'token-up',
      outcomeLabel: 'Up',
      ...config,
    },
  };
  const graph: TradeFlowGraph = {
    context: {},
    nodes: [action],
    edges: [],
  };
  const issues: TradeFlowValidationIssue[] = [];
  validateActionPlaceOrderConfig(issues, action, graph);
  return issues;
}

function autoTuneIssueCodes(issues: TradeFlowValidationIssue[]): string[] {
  return issues
    .map((issue) => issue.code)
    .filter((code) => code.includes('auto_tune'));
}
