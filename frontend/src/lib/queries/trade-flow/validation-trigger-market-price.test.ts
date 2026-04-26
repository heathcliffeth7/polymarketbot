import assert from 'node:assert/strict';
import test from 'node:test';

import { normalizeTradeFlowGraph } from '@/lib/queries/trade-flow/graph';
import { validateTriggerMarketPriceNodeConfig } from '@/lib/queries/trade-flow/validation-trigger-market-price';
import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';

function collectTriggerIssues(graph: TradeFlowGraph, nodeKey: string): TradeFlowValidationIssue[] {
  const node = graph.nodes.find((item) => item.key === nodeKey);
  assert.ok(node, `node ${nodeKey} should exist`);
  const issues: TradeFlowValidationIssue[] = [];
  validateTriggerMarketPriceNodeConfig(issues, node, graph);
  return issues;
}

function buildAutoScopeTrigger(
  key: string,
  marketScope: string,
  priceToBeatMode: string,
  overrides: Record<string, unknown> = {}
): TradeFlowNode {
  return {
    key,
    type: 'trigger.market_price',
    positionX: 0,
    positionY: 0,
    config: {
      marketMode: 'auto_scope',
      marketScope,
      marketSelection: 'latest_by_slug',
      priceMode: 'composite',
      repeatMode: 'once',
      priceToBeatTriggerEnabled: true,
      priceToBeatMode,
      outcomeConditions: [
        {
          tokenId: 'token-up',
          outcomeLabel: 'Up',
        },
      ],
      ...overrides,
    },
  };
}

test('validateTriggerMarketPriceNodeConfig accepts auto_vol_pct on supported auto_scope asset', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [buildAutoScopeTrigger('trigger_eth', 'eth_5m_updown', 'auto_vol_pct')],
    edges: [],
  });

  const issues = collectTriggerIssues(graph, 'trigger_eth');
  assert.equal(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_mode'),
    false
  );
  assert.equal(
    issues.some((issue) => issue.code === 'unsupported_price_to_beat_auto_vol_pct_asset'),
    false
  );
});

test('validateTriggerMarketPriceNodeConfig accepts valid entry timing profiles on auto_scope once trigger', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_entry_profiles', 'btc_5m_updown', 'manual', {
        priceToBeatTriggerMinGap: 10,
        priceToBeatTriggerUnit: 'usd',
        entryTimingProfiles: [
          {
            startRemainingSec: 90,
            endRemainingSec: 45,
            maxPriceCent: 60,
            priceToBeatTriggerMinGap: 10,
            sizeUsdc: 1.5,
          },
          {
            startRemainingSec: 45,
            endRemainingSec: 20,
            maxPriceCent: 67,
            priceToBeatTriggerMinGap: 18,
            sizeUsdc: 1,
          },
        ],
      }),
    ],
    edges: [],
  });

  const issues = collectTriggerIssues(graph, 'trigger_entry_profiles');
  assert.equal(
    issues.some((issue) => issue.code.startsWith('invalid_entry_timing_profile')),
    false
  );
  assert.equal(
    issues.some((issue) => issue.code === 'entry_timing_profiles_disallow_cycle_window'),
    false
  );
});

test('validateTriggerMarketPriceNodeConfig rejects entry timing profiles combined with cycleWindowMode', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_entry_cycle', 'btc_5m_updown', 'manual', {
        priceToBeatTriggerMinGap: 10,
        cycleWindowMode: 'custom_range',
        cycleWindowStartSec: 230,
        cycleWindowEndSec: 290,
        entryTimingProfiles: [
          {
            startRemainingSec: 90,
            endRemainingSec: 45,
            maxPriceCent: 60,
          },
        ],
      }),
    ],
    edges: [],
  });

  const issues = collectTriggerIssues(graph, 'trigger_entry_cycle');
  assert.equal(
    issues.some((issue) => issue.code === 'entry_timing_profiles_disallow_cycle_window'),
    true
  );
});

test('validateTriggerMarketPriceNodeConfig rejects overlapping entry timing profile windows', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_entry_overlap', 'btc_5m_updown', 'manual', {
        priceToBeatTriggerMinGap: 10,
        entryTimingProfiles: [
          { startRemainingSec: 90, endRemainingSec: 30 },
          { startRemainingSec: 45, endRemainingSec: 20 },
        ],
      }),
    ],
    edges: [],
  });

  const issues = collectTriggerIssues(graph, 'trigger_entry_overlap');
  assert.equal(
    issues.some((issue) => issue.code === 'overlapping_entry_timing_profiles'),
    true
  );
});

test('validateTriggerMarketPriceNodeConfig rejects auto_vol_pct on xrp auto_scope asset', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [buildAutoScopeTrigger('trigger_xrp', 'xrp_5m_updown', 'auto_vol_pct')],
    edges: [],
  });

  const issues = collectTriggerIssues(graph, 'trigger_xrp');
  assert.equal(
    issues.some((issue) => issue.code === 'unsupported_price_to_beat_auto_vol_pct_asset'),
    true
  );
});

test('validateTriggerMarketPriceNodeConfig accepts pair_lock_only with single downstream pair_lock', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair', 'btc_5m_updown', 'auto_vol_pct', {
        bindingMode: 'pair_lock_only',
        priceToBeatTriggerEnabled: false,
        outcomeConditions: [],
      }),
      {
        key: 'pair_buy',
        type: 'action.place_order',
        positionX: 220,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          counterLegEnabled: true,
        },
      },
    ],
    edges: [{ key: 'edge_pair', source: 'trigger_pair', target: 'pair_buy', type: 'default', condition: null }],
  });

  const issues = collectTriggerIssues(graph, 'trigger_pair');
  assert.equal(issues.length, 0);
});

test('validateTriggerMarketPriceNodeConfig accepts pair_lock_only custom_range on auto_scope trigger', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_window', 'btc_5m_updown', 'auto_vol_pct', {
        bindingMode: 'pair_lock_only',
        priceToBeatTriggerEnabled: false,
        outcomeConditions: [],
        cycleWindowMode: 'custom_range',
        cycleWindowStartSec: 230,
        cycleWindowEndSec: 290,
      }),
      {
        key: 'pair_buy',
        type: 'action.place_order',
        positionX: 220,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          counterLegEnabled: true,
        },
      },
    ],
    edges: [{ key: 'edge_pair_window', source: 'trigger_pair_window', target: 'pair_buy', type: 'default', condition: null }],
  });

  const issues = collectTriggerIssues(graph, 'trigger_pair_window');
  assert.equal(issues.length, 0);
});

test('validateTriggerMarketPriceNodeConfig accepts pair_lock_only with pair_lock and action.notify downstream', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_notify', 'btc_5m_updown', 'auto_vol_pct', {
        bindingMode: 'pair_lock_only',
        priceToBeatTriggerEnabled: false,
        outcomeConditions: [],
      }),
      {
        key: 'pair_buy',
        type: 'action.place_order',
        positionX: 220,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          counterLegEnabled: true,
        },
      },
      {
        key: 'notify_ui',
        type: 'action.notify',
        positionX: 220,
        positionY: 80,
        config: { channel: 'ui', message: 'tetiklendi' },
      },
    ],
    edges: [
      { key: 'edge_pair', source: 'trigger_pair_notify', target: 'pair_buy', type: 'default', condition: null },
      { key: 'edge_notify', source: 'trigger_pair_notify', target: 'notify_ui', type: 'default', condition: null },
    ],
  });

  const issues = collectTriggerIssues(graph, 'trigger_pair_notify');
  assert.equal(issues.length, 0);
});

test('validateTriggerMarketPriceNodeConfig accepts pair_lock_only with pair_lock and action.telegram_notify downstream', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_telegram', 'btc_5m_updown', 'auto_vol_pct', {
        bindingMode: 'pair_lock_only',
        priceToBeatTriggerEnabled: false,
        outcomeConditions: [],
      }),
      {
        key: 'pair_buy',
        type: 'action.place_order',
        positionX: 220,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          counterLegEnabled: true,
        },
      },
      {
        key: 'notify_tg',
        type: 'action.telegram_notify',
        positionX: 220,
        positionY: 80,
        config: { message: 'tetiklendi' },
      },
    ],
    edges: [
      { key: 'edge_pair', source: 'trigger_pair_telegram', target: 'pair_buy', type: 'default', condition: null },
      { key: 'edge_tg', source: 'trigger_pair_telegram', target: 'notify_tg', type: 'default', condition: null },
    ],
  });

  const issues = collectTriggerIssues(graph, 'trigger_pair_telegram');
  assert.equal(issues.length, 0);
});

test('validateTriggerMarketPriceNodeConfig accepts pair_lock_only with pair_lock and two notification actions', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_double_notify', 'btc_5m_updown', 'auto_vol_pct', {
        bindingMode: 'pair_lock_only',
        priceToBeatTriggerEnabled: false,
        outcomeConditions: [],
      }),
      {
        key: 'pair_buy',
        type: 'action.place_order',
        positionX: 220,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          counterLegEnabled: true,
        },
      },
      {
        key: 'notify_ui',
        type: 'action.notify',
        positionX: 220,
        positionY: 80,
        config: { channel: 'ui', message: 'tetiklendi' },
      },
      {
        key: 'notify_tg',
        type: 'action.telegram_notify',
        positionX: 220,
        positionY: 160,
        config: { message: 'tetiklendi' },
      },
    ],
    edges: [
      { key: 'edge_pair', source: 'trigger_pair_double_notify', target: 'pair_buy', type: 'default', condition: null },
      { key: 'edge_notify', source: 'trigger_pair_double_notify', target: 'notify_ui', type: 'default', condition: null },
      { key: 'edge_tg', source: 'trigger_pair_double_notify', target: 'notify_tg', type: 'default', condition: null },
    ],
  });

  const issues = collectTriggerIssues(graph, 'trigger_pair_double_notify');
  assert.equal(issues.length, 0);
});

test('validateTriggerMarketPriceNodeConfig rejects pair_lock_only with multiple pair_lock downstream nodes', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_multi', 'btc_5m_updown', 'auto_vol_pct', {
        bindingMode: 'pair_lock_only',
        priceToBeatTriggerEnabled: false,
        outcomeConditions: [],
      }),
      {
        key: 'pair_buy_1',
        type: 'action.place_order',
        positionX: 220,
        positionY: 0,
        config: { mode: 'pair_lock', side: 'buy', executionMode: 'limit', sizeMode: 'usdc', sizeUsdc: 5, pairMaxTotalCent: 90, counterLegEnabled: true },
      },
      {
        key: 'pair_buy_2',
        type: 'action.place_order',
        positionX: 220,
        positionY: 80,
        config: { mode: 'pair_lock', side: 'buy', executionMode: 'limit', sizeMode: 'usdc', sizeUsdc: 5, pairMaxTotalCent: 90, counterLegEnabled: true },
      },
    ],
    edges: [
      { key: 'edge_pair_1', source: 'trigger_pair_multi', target: 'pair_buy_1', type: 'default', condition: null },
      { key: 'edge_pair_2', source: 'trigger_pair_multi', target: 'pair_buy_2', type: 'default', condition: null },
    ],
  });

  const issues = collectTriggerIssues(graph, 'trigger_pair_multi');
  assert.ok(
    issues.some((issue) => issue.code === 'pair_lock_only_requires_single_pair_lock_downstream')
  );
});

test('validateTriggerMarketPriceNodeConfig rejects pair_lock_only with notification-only downstream', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_notify_only', 'btc_5m_updown', 'auto_vol_pct', {
        bindingMode: 'pair_lock_only',
        priceToBeatTriggerEnabled: false,
        outcomeConditions: [],
      }),
      {
        key: 'notify_ui',
        type: 'action.notify',
        positionX: 220,
        positionY: 0,
        config: { channel: 'ui', message: 'tetiklendi' },
      },
    ],
    edges: [{ key: 'edge_notify', source: 'trigger_pair_notify_only', target: 'notify_ui', type: 'default', condition: null }],
  });

  const issues = collectTriggerIssues(graph, 'trigger_pair_notify_only');
  assert.ok(
    issues.some((issue) => issue.code === 'pair_lock_only_requires_single_pair_lock_downstream')
  );
});

test('validateTriggerMarketPriceNodeConfig rejects pair_lock_only with non-notification parallel downstream', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_logic', 'btc_5m_updown', 'auto_vol_pct', {
        bindingMode: 'pair_lock_only',
        priceToBeatTriggerEnabled: false,
        outcomeConditions: [],
      }),
      {
        key: 'pair_buy',
        type: 'action.place_order',
        positionX: 220,
        positionY: 80,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          counterLegEnabled: true,
        },
      },
      {
        key: 'logic_if',
        type: 'logic.if',
        positionX: 220,
        positionY: 0,
        config: { expression: { '>': [{ var: 'market_price' }, 0.5] } },
      },
    ],
    edges: [
      { key: 'edge_pair', source: 'trigger_pair_logic', target: 'pair_buy', type: 'default', condition: null },
      { key: 'edge_logic', source: 'trigger_pair_logic', target: 'logic_if', type: 'default', condition: null },
    ],
  });

  const issues = collectTriggerIssues(graph, 'trigger_pair_logic');
  assert.ok(
    issues.some((issue) => issue.code === 'pair_lock_only_disallows_non_notification_downstream')
  );
});

test('validateTriggerMarketPriceNodeConfig rejects pair_lock_only with outcome rows', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_outcome', 'btc_5m_updown', 'auto_vol_pct', {
        bindingMode: 'pair_lock_only',
      }),
      {
        key: 'pair_buy',
        type: 'action.place_order',
        positionX: 220,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          pairMaxTotalCent: 90,
          counterLegEnabled: true,
        },
      },
    ],
    edges: [{ key: 'edge_pair', source: 'trigger_pair_outcome', target: 'pair_buy', type: 'default', condition: null }],
  });

  const issues = collectTriggerIssues(graph, 'trigger_pair_outcome');
  assert.ok(issues.some((issue) => issue.code === 'pair_lock_only_disallows_outcome_conditions'));
});
