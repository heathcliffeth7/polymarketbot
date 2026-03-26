import assert from 'node:assert/strict';
import test from 'node:test';

import { normalizeTradeFlowGraph } from '@/lib/queries/trade-flow/graph';
import { validateActionPlaceOrderConfig } from '@/lib/queries/trade-flow/validation-action-place-order';
import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';

function buildGenericPresetAction(nodeKey: string, refKey = 'preset_place_order'): TradeFlowNode {
  return {
    key: nodeKey,
    type: 'action.place_order',
    positionX: 200,
    positionY: 0,
    config: {
      presetKind: 'place_order',
      refKey,
      side: 'buy',
      executionMode: 'market',
      sizeMode: 'usdc',
      sizeUsdc: 5,
      marketSlug: 'nba-lal-orl-2026-03-21',
      tokenId: 'stale-token',
      outcomeLabel: 'Stale Outcome',
    },
  };
}

function buildFixedTrigger(
  key: string,
  tokenId: string,
  outcomeLabel: string,
  triggerPriceCent: number
): TradeFlowNode {
  return {
    key,
    type: 'trigger.market_price',
    positionX: 0,
    positionY: 0,
    config: {
      marketMode: 'fixed',
      priceMode: 'composite',
      marketSlug: 'nba-lal-orl-2026-03-21',
      repeatMode: 'once',
      outcomeConditions: [
        {
          tokenId,
          outcomeLabel,
          triggerCondition: 'level_above',
          triggerPriceCent,
        },
      ],
    },
  };
}

function buildAutoScopeTrigger(key: string): TradeFlowNode {
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
      outcomeConditions: [
        {
          triggerCondition: 'level_above',
          triggerPriceCent: 30,
        },
      ],
    },
  };
}

function collectActionIssues(graph: TradeFlowGraph, nodeKey: string): TradeFlowValidationIssue[] {
  const node = graph.nodes.find((item) => item.key === nodeKey);
  assert.ok(node, `node ${nodeKey} should exist`);
  const issues: TradeFlowValidationIssue[] = [];
  validateActionPlaceOrderConfig(issues, node, graph);
  return issues;
}

function buildReentryAction(
  key: string,
  overrides: Record<string, unknown> = {}
): TradeFlowNode {
  return {
    key,
    type: 'action.place_order',
    positionX: 200,
    positionY: 0,
    config: {
      side: 'buy',
      executionMode: 'market',
      sizeMode: 'usdc',
      sizeUsdc: 10,
      marketSlug: 'nba-lal-orl-2026-03-21',
      tokenId: 'magic-token',
      outcomeLabel: 'Moneyline: Magic',
      slEnabled: true,
      slPriceCent: 45,
      reenterOnSlHit: true,
      reentryMaxAttempts: 2,
      ...overrides,
    },
  };
}

test('validateActionPlaceOrderConfig errors when generic preset place_order has no unique upstream fixed trigger outcome', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [buildGenericPresetAction('action_k2p4un')],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'action_k2p4un');
  assert.ok(
    issues.some((issue) => issue.code === 'missing_unique_upstream_fixed_trigger_seed'),
    'expected preset seed validation error'
  );
});

test('validateActionPlaceOrderConfig accepts generic preset place_order when unique upstream fixed trigger outcome exists', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildFixedTrigger('trigger_eyh0vk', 'magic-token', 'Moneyline: Magic', 30),
      buildGenericPresetAction('action_k2p4un'),
    ],
    edges: [{ key: 'edge_1', source: 'trigger_eyh0vk', target: 'action_k2p4un', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'action_k2p4un');
  assert.equal(
    issues.some((issue) => issue.code === 'missing_unique_upstream_fixed_trigger_seed'),
    false
  );
});

test('validateActionPlaceOrderConfig accepts generic preset place_order when upstream auto_scope trigger exists', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [buildAutoScopeTrigger('trigger_auto'), buildGenericPresetAction('action_auto')],
    edges: [{ key: 'edge_1', source: 'trigger_auto', target: 'action_auto', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'action_auto');
  assert.equal(
    issues.some((issue) => issue.code === 'missing_unique_upstream_fixed_trigger_seed'),
    false
  );
});

test('validateActionPlaceOrderConfig accepts generic preset place_order when multiple upstream auto_scope triggers exist', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_auto_1'),
      buildAutoScopeTrigger('trigger_auto_2'),
      buildGenericPresetAction('action_auto'),
    ],
    edges: [
      { key: 'edge_1', source: 'trigger_auto_1', target: 'action_auto', type: 'default', condition: null },
      { key: 'edge_2', source: 'trigger_auto_2', target: 'action_auto', type: 'default', condition: null },
    ],
  });

  const issues = collectActionIssues(graph, 'action_auto');
  assert.equal(
    issues.some((issue) => issue.code === 'missing_unique_upstream_fixed_trigger_seed'),
    false
  );
});

test('validateActionPlaceOrderConfig accepts valid tp/sl/time ladders on buy action', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'ladder_buy',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'nba-lal-orl-2026-03-21',
          tokenId: 'magic-token',
          outcomeLabel: 'Moneyline: Magic',
          tpRules: [
            { priceCent: 65, sizePct: 35 },
            { priceCent: 75, sizePct: 65 },
          ],
          slRules: [
            { priceCent: 45, sizePct: 40 },
            { priceCent: 35, sizePct: 60 },
          ],
          timeExitRules: [
            { elapsedMinutes: 12, remainingPct: 20 },
            { elapsedMinutes: 20, remainingPct: 30 },
            { elapsedMinutes: 60, remainingPct: 100 },
          ],
          slTriggerPriceMode: 'composite_safe',
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'ladder_buy');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig accepts hard and staged tp/sl exits together', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'hard_and_staged_buy',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'nba-lal-orl-2026-03-21',
          tokenId: 'magic-token',
          outcomeLabel: 'Moneyline: Magic',
          tpEnabled: true,
          tpPriceCent: 92,
          tpRules: [
            { priceCent: 65, sizePct: 35 },
            { priceCent: 75, sizePct: 65 },
          ],
          slEnabled: true,
          slPriceCent: 30,
          slRules: [
            { priceCent: 45, sizePct: 40 },
            { priceCent: 35, sizePct: 60 },
          ],
          slTriggerPriceMode: 'composite_safe',
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'hard_and_staged_buy');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig rejects invalid ladder sums and ordering', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'ladder_buy',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'nba-lal-orl-2026-03-21',
          tokenId: 'magic-token',
          outcomeLabel: 'Moneyline: Magic',
          tpRules: [
            { priceCent: 70, sizePct: 60 },
            { priceCent: 65, sizePct: 30 },
          ],
          slRules: [
            { priceCent: 40, sizePct: 50 },
            { priceCent: 45, sizePct: 40 },
          ],
          timeExitRules: [
            { elapsedMinutes: 20, remainingPct: 30 },
            { elapsedMinutes: 12, remainingPct: 20 },
          ],
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'ladder_buy');
  assert.ok(issues.some((issue) => issue.code === 'invalid_tp_rules_sum'));
  assert.ok(issues.some((issue) => issue.code === 'invalid_tp_rules_order'));
  assert.ok(issues.some((issue) => issue.code === 'invalid_sl_rules_sum'));
  assert.ok(issues.some((issue) => issue.code === 'invalid_sl_rules_order'));
  assert.ok(issues.some((issue) => issue.code === 'invalid_time_exit_rules_order'));
});

test('validateActionPlaceOrderConfig rejects reentry price fields when reentry is disabled', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildReentryAction('reentry_buy', {
        reenterOnSlHit: false,
        reentryMaxAttempts: undefined,
        reentryMinPriceCent: 60,
        reentryMaxPriceCent: 85,
      }),
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'reentry_buy');
  assert.ok(issues.some((issue) => issue.code === 'reentry_min_price_requires_reentry'));
  assert.ok(issues.some((issue) => issue.code === 'reentry_max_price_requires_reentry'));
});

test('validateActionPlaceOrderConfig rejects invalid reentry price band ordering', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildFixedTrigger('trigger_eyh0vk', 'magic-token', 'Moneyline: Magic', 30),
      buildReentryAction('reentry_buy', {
        reentryMinPriceCent: 90,
        reentryMaxPriceCent: 85,
      }),
    ],
    edges: [{ key: 'edge_1', source: 'trigger_eyh0vk', target: 'reentry_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'reentry_buy');
  assert.ok(issues.some((issue) => issue.code === 'invalid_reentry_price_band'));
});

test('validateActionPlaceOrderConfig accepts valid reentry price band', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildFixedTrigger('trigger_eyh0vk', 'magic-token', 'Moneyline: Magic', 30),
      buildReentryAction('reentry_buy', {
        reentryMinPriceCent: 60,
        reentryMaxPriceCent: 85,
      }),
    ],
    edges: [{ key: 'edge_1', source: 'trigger_eyh0vk', target: 'reentry_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'reentry_buy');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig accepts execution floor override without upstream trigger', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      {
        key: 'floor_override_buy',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'nba-lal-orl-2026-03-21',
          tokenId: 'magic-token',
          outcomeLabel: 'Moneyline: Magic',
          executionFloorGuardEnabled: true,
          executionFloorPriceCent: 82,
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'floor_override_buy');
  assert.equal(
    issues.some((issue) => issue.code === 'missing_upstream_execution_floor_trigger_price'),
    false
  );
  assert.equal(
    issues.some((issue) => issue.code === 'invalid_execution_floor_price_cent'),
    false
  );
});

test('validateActionPlaceOrderConfig rejects execution floor guard with no upstream or local floor', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      {
        key: 'floor_missing_buy',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'nba-lal-orl-2026-03-21',
          tokenId: 'magic-token',
          outcomeLabel: 'Moneyline: Magic',
          executionFloorGuardEnabled: true,
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'floor_missing_buy');
  assert.ok(
    issues.some((issue) => issue.code === 'missing_upstream_execution_floor_trigger_price')
  );
});

test('validateActionPlaceOrderConfig rejects invalid execution floor override price', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildFixedTrigger('trigger_eyh0vk', 'magic-token', 'Moneyline: Magic', 30),
      {
        key: 'floor_invalid_buy',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'nba-lal-orl-2026-03-21',
          tokenId: 'magic-token',
          outcomeLabel: 'Moneyline: Magic',
          executionFloorGuardEnabled: true,
          executionFloorPriceCent: 101,
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_eyh0vk', target: 'floor_invalid_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'floor_invalid_buy');
  assert.ok(issues.some((issue) => issue.code === 'invalid_execution_floor_price_cent'));
});
