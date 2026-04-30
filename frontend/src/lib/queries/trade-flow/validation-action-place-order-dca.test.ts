import assert from 'node:assert/strict';
import test from 'node:test';

import { normalizeTradeFlowGraph } from '@/lib/queries/trade-flow/graph';
import type { TradeFlowNode } from '@/lib/types';
import { collectActionIssues } from './validation-action-place-order.test-helpers';

function buildDcaTrigger(
  key: string,
  bindingMode = 'dca_live_only'
): TradeFlowNode {
  return {
    key,
    type: 'trigger.market_price',
    positionX: 0,
    positionY: 0,
    config: {
      marketMode: 'auto_scope',
      marketScope: 'btc_5m_updown',
      marketSelection: 'latest_by_slug',
      priceMode: 'composite',
      repeatMode: 'once',
      bindingMode,
      outcomeConditions: [],
      priceToBeatTriggerEnabled: false,
    },
  };
}

function buildDcaAction(key: string): TradeFlowNode {
  return {
    key,
    type: 'action.place_order',
    positionX: 400,
    positionY: 0,
    config: {
      mode: 'dca_live_v1',
      side: 'buy',
      executionMode: 'limit',
      marketSelectionMode: 'manual_slug',
      manualSlug: 'btc-updown-5m-1777493700',
      sideMode: 'one_sided',
      selectedOutcomes: [
        { slug: 'btc-updown-5m-1777493700', outcomeLabel: 'Up', tokenId: 'up-token' },
      ],
      initialOrderShares: 1,
      maxTotalCostPerSlugUsdc: 2,
      maxTotalCostAllSlugsUsdc: 2,
    },
  };
}

test('validateActionPlaceOrderConfig accepts dca_live_v1 behind a logic guard', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildDcaTrigger('trigger_dca'),
      {
        key: 'logic_guard',
        type: 'logic.if',
        positionX: 200,
        positionY: 0,
        config: { expression: { var: 'guard_ok' } },
      },
      buildDcaAction('dca_buy'),
    ],
    edges: [
      { key: 'edge_guard', source: 'trigger_dca', target: 'logic_guard', type: 'default', condition: null },
      { key: 'edge_dca', source: 'logic_guard', target: 'dca_buy', type: 'default', condition: null },
    ],
  });

  const issues = collectActionIssues(graph, 'dca_buy');
  assert.equal(issues.some((issue) => issue.code === 'dca_live_requires_single_market_price_binding'), false);
  assert.equal(issues.some((issue) => issue.code === 'dca_live_requires_dca_binding_mode'), false);
});

test('validateActionPlaceOrderConfig rejects dca_live_v1 behind standard trigger binding', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [buildDcaTrigger('trigger_standard', 'standard'), buildDcaAction('dca_buy')],
    edges: [
      { key: 'edge_dca', source: 'trigger_standard', target: 'dca_buy', type: 'default', condition: null },
    ],
  });

  const issues = collectActionIssues(graph, 'dca_buy');
  assert.ok(issues.some((issue) => issue.code === 'dca_live_requires_dca_binding_mode'));
});

test('validateActionPlaceOrderConfig rejects dca_live_v1 with multiple upstream DCA triggers', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildDcaTrigger('trigger_dca_1'),
      buildDcaTrigger('trigger_dca_2'),
      buildDcaAction('dca_buy'),
    ],
    edges: [
      { key: 'edge_dca_1', source: 'trigger_dca_1', target: 'dca_buy', type: 'default', condition: null },
      { key: 'edge_dca_2', source: 'trigger_dca_2', target: 'dca_buy', type: 'default', condition: null },
    ],
  });

  const issues = collectActionIssues(graph, 'dca_buy');
  assert.ok(issues.some((issue) => issue.code === 'dca_live_requires_single_market_price_binding'));
});
