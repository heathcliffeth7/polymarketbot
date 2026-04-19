import assert from 'node:assert/strict';
import test from 'node:test';

import { normalizeTradeFlowGraph } from '@/lib/queries/trade-flow/graph';
import { validateActionPlaceOrderConfig } from '@/lib/queries/trade-flow/validation-action-place-order';
import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';

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
      bindingMode: 'pair_lock_only',
      priceMode: 'composite',
      repeatMode: 'once',
      outcomeConditions: [
        {
          outcomeLabel: 'Up',
          triggerCondition: 'level_above',
          triggerPriceCent: 70,
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

test('validateActionPlaceOrderConfig accepts valid pair_lock config', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair'),
      {
        key: 'pair_buy',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'manual',
          priceToBeatMaxDiff: 10,
          priceToBeatMaxDiffUnit: 'usd',
          pairMaxTotalCent: 90,
          pairOrphanGraceMs: 1500,
          pairSizingMode: 'manual',
          counterLegEnabled: true,
          counterLegSizeUsdc: 5,
          counterLegOutcomeLabel: 'opposite',
          counterLegTriggerCondition: 'level_below',
          counterLegTriggerPriceCent: 20,
          counterLegMaxPriceCent: 42,
          counterLegPriceToBeatGuardEnabled: true,
          counterLegPriceToBeatMode: 'manual',
          counterLegPriceToBeatMaxDiff: 10,
          counterLegPriceToBeatMaxDiffUnit: 'usd',
        },
      },
    ],
    edges: [{ key: 'edge_pair', source: 'trigger_pair', target: 'pair_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig accepts auto_remaining_budget pair_lock config', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_auto'),
      {
        key: 'pair_buy_auto',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          pairSizingMode: 'auto_remaining_budget',
          pairTotalBudgetUsdc: 14,
          pairOrphanGraceMs: 1500,
          counterLegEnabled: true,
          counterLegOutcomeLabel: 'opposite',
        },
      },
    ],
    edges: [{ key: 'edge_pair_auto', source: 'trigger_pair_auto', target: 'pair_buy_auto', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_auto');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig accepts pair_lock lead-leg hard/PTB stop-loss fields', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_sl'),
      {
        key: 'pair_buy_sl',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          kind: 'immediate',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          pairSizingMode: 'manual',
          counterLegEnabled: true,
          counterLegSizeUsdc: 5,
          counterLegOutcomeLabel: 'opposite',
          slEnabled: true,
          slPriceCent: 45,
          slTriggerPriceMode: 'composite_safe',
          ptbStopLossEnabled: true,
          ptbStopLossGapUsd: 0,
          ptbStopLossTimeDecayMode: 'tighten',
          notifyOnSlHit: true,
          reenterOnSlHit: true,
          reentryMaxAttempts: 2,
          reentryCooldownSec: 15,
        },
      },
    ],
    edges: [{ key: 'edge_pair_sl', source: 'trigger_pair_sl', target: 'pair_buy_sl', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_sl');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig accepts pair_lock with market execution', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_market'),
      {
        key: 'pair_buy_market',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          pairSizingMode: 'manual',
          counterLegEnabled: true,
          counterLegSizeUsdc: 5,
          counterLegOutcomeLabel: 'opposite',
        },
      },
    ],
    edges: [{ key: 'edge_pair_market', source: 'trigger_pair_market', target: 'pair_buy_market', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_market');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig rejects unsupported pair_lock execution mode', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_bad_exec'),
      {
        key: 'pair_buy_bad_exec',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'post_only',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          pairSizingMode: 'manual',
          counterLegEnabled: true,
          counterLegSizeUsdc: 5,
        },
      },
    ],
    edges: [{ key: 'edge_pair_bad_exec', source: 'trigger_pair_bad_exec', target: 'pair_buy_bad_exec', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_bad_exec');
  assert.ok(issues.some((issue) => issue.code === 'pair_lock_requires_supported_execution'));
});

test('validateActionPlaceOrderConfig rejects pair_lock without counter leg', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_invalid'),
      {
        key: 'pair_buy_invalid',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
        },
      },
    ],
    edges: [
      {
        key: 'edge_pair_invalid',
        source: 'trigger_pair_invalid',
        target: 'pair_buy_invalid',
        type: 'default',
        condition: null,
      },
    ],
  });

  const issues = collectActionIssues(graph, 'pair_buy_invalid');
  assert.ok(issues.some((issue) => issue.code === 'pair_lock_requires_counter_leg'));
});

test('validateActionPlaceOrderConfig rejects auto_remaining_budget pair_lock when total budget is not above primary size', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_bad_budget'),
      {
        key: 'pair_buy_bad_budget',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          pairSizingMode: 'auto_remaining_budget',
          pairTotalBudgetUsdc: 5,
          counterLegEnabled: true,
        },
      },
    ],
    edges: [{ key: 'edge_pair_bad_budget', source: 'trigger_pair_bad_budget', target: 'pair_buy_bad_budget', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_bad_budget');
  assert.ok(
    issues.some((issue) => issue.code === 'pair_lock_total_budget_must_exceed_primary_size')
  );
});

test('validateActionPlaceOrderConfig rejects pair_lock without direct trigger.market_price parent', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      {
        key: 'pair_buy_orphan',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          counterLegEnabled: true,
          counterLegSizeUsdc: 5,
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'pair_buy_orphan');
  assert.ok(issues.some((issue) => issue.code === 'pair_lock_requires_single_direct_trigger'));
});

test('validateActionPlaceOrderConfig rejects pair_lock when upstream trigger is not pair_lock_only', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      {
        ...buildAutoScopeTrigger('trigger_standard'),
        config: {
          ...buildAutoScopeTrigger('trigger_standard').config,
          bindingMode: 'standard',
        },
      },
      {
        key: 'pair_buy_standard',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          counterLegEnabled: true,
          counterLegSizeUsdc: 5,
        },
      },
    ],
    edges: [{ key: 'edge_pair_standard', source: 'trigger_standard', target: 'pair_buy_standard', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_standard');
  assert.ok(issues.some((issue) => issue.code === 'pair_lock_requires_pair_lock_only_trigger'));
});

test('validateActionPlaceOrderConfig rejects pair_lock with classic exit features', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_invalid_exit'),
      {
        key: 'pair_buy_invalid_exit',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          counterLegEnabled: true,
          counterLegSizeUsdc: 5,
          counterLegOutcomeLabel: 'opposite',
          tpEnabled: true,
          tpPriceCent: 95,
        },
      },
    ],
    edges: [
      {
        key: 'edge_pair_invalid_exit',
        source: 'trigger_pair_invalid_exit',
        target: 'pair_buy_invalid_exit',
        type: 'default',
        condition: null,
      },
    ],
  });

  const issues = collectActionIssues(graph, 'pair_buy_invalid_exit');
  assert.ok(issues.some((issue) => issue.code === 'pair_lock_disallows_exit_features'));
});

test('validateActionPlaceOrderConfig ignores zero-value reentry knobs in pair_lock mode', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_zero_reentry'),
      {
        key: 'pair_buy_zero_reentry',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          pairSizingMode: 'auto_remaining_budget',
          pairTotalBudgetUsdc: 10,
          counterLegEnabled: true,
          counterLegOutcomeLabel: 'opposite',
          reentryCooldownSec: 0,
          reentryMaxPriceTightenBps: 0,
        },
      },
    ],
    edges: [{ key: 'edge_pair_zero_reentry', source: 'trigger_pair_zero_reentry', target: 'pair_buy_zero_reentry', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_zero_reentry');
  assert.equal(
    issues.some((issue) => issue.code === 'pair_lock_disallows_exit_features'),
    false
  );
  assert.equal(
    issues.some((issue) => issue.code === 'reentry_max_price_tighten_bps_requires_reentry'),
    false
  );
});

test('validateActionPlaceOrderConfig still rejects non-zero reentry knobs in pair_lock mode', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_nonzero_reentry'),
      {
        key: 'pair_buy_nonzero_reentry',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          pairSizingMode: 'auto_remaining_budget',
          pairTotalBudgetUsdc: 10,
          counterLegEnabled: true,
          counterLegOutcomeLabel: 'opposite',
          reentryCooldownSec: 1,
          reentryMaxPriceTightenBps: 500,
        },
      },
    ],
    edges: [{ key: 'edge_pair_nonzero_reentry', source: 'trigger_pair_nonzero_reentry', target: 'pair_buy_nonzero_reentry', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_nonzero_reentry');
  assert.equal(
    issues.some((issue) => issue.code === 'pair_lock_disallows_exit_features'),
    true
  );
  assert.equal(
    issues.some((issue) => issue.code === 'reentry_max_price_tighten_bps_requires_reentry'),
    true
  );
});

test('validateActionPlaceOrderConfig rejects staged and advanced stop-loss extensions in pair_lock mode', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_advanced_exit'),
      {
        key: 'pair_buy_advanced_exit',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          kind: 'immediate',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          counterLegEnabled: true,
          counterLegSizeUsdc: 5,
          counterLegOutcomeLabel: 'opposite',
          slEnabled: true,
          slPriceCent: 45,
          reenterOnSlHit: true,
          reentryMaxAttempts: 2,
          reentryCooldownSec: 15,
          slRules: [{ priceCent: 40, sizePct: 100 }],
          reentryMinPriceCent: 35,
        },
      },
    ],
    edges: [{ key: 'edge_pair_advanced_exit', source: 'trigger_pair_advanced_exit', target: 'pair_buy_advanced_exit', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_advanced_exit');
  assert.ok(issues.some((issue) => issue.code === 'pair_lock_disallows_exit_features'));
});
