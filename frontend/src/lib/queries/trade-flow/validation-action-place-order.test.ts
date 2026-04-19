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

function buildAutoScopeTrigger(key: string, marketScope = 'btc_5m_updown'): TradeFlowNode {
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

test('validateActionPlaceOrderConfig accepts auto_vol_pct on supported explicit market', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'auto_vol_buy',
        type: 'action.place_order',
        positionX: 0,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          marketSlug: 'eth-updown-5m-1774013100',
          tokenId: 'eth-up-token',
          outcomeLabel: 'Up',
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'auto_vol_pct',
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'auto_vol_buy');
  assert.equal(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_mode'),
    false
  );
  assert.equal(
    issues.some((issue) => issue.code === 'unsupported_price_to_beat_auto_vol_pct_asset'),
    false
  );
});

test('validateActionPlaceOrderConfig rejects auto_vol_pct on upstream xrp auto_scope trigger', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_auto_xrp', 'xrp_5m_updown'),
      {
        key: 'action_auto_xrp',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'auto_vol_pct',
        },
      },
    ],
    edges: [
      {
        key: 'edge_auto_xrp',
        source: 'trigger_auto_xrp',
        target: 'action_auto_xrp',
        type: 'default',
        condition: null,
      },
    ],
  });

  const issues = collectActionIssues(graph, 'action_auto_xrp');
  assert.equal(
    issues.some((issue) => issue.code === 'unsupported_price_to_beat_auto_vol_pct_asset'),
    true
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

test('validateActionPlaceOrderConfig accepts manual reentry PTB override', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildFixedTrigger('trigger_ptb_manual', 'magic-token', 'Moneyline: Magic', 30),
      buildReentryAction('reentry_ptb_manual', {
        marketSlug: 'btc-updown-5m-1774013100',
        outcomeLabel: 'Up',
        priceToBeatGuardEnabled: true,
        priceToBeatMode: 'manual',
        priceToBeatMaxDiff: 10,
        priceToBeatMaxDiffUnit: 'usd',
        reentryPriceToBeatMaxDiff: 5,
      }),
    ],
    edges: [
      {
        key: 'edge_ptb_manual',
        source: 'trigger_ptb_manual',
        target: 'reentry_ptb_manual',
        type: 'default',
        condition: null,
      },
    ],
  });

  const issues = collectActionIssues(graph, 'reentry_ptb_manual');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig accepts manual reentry PTB override with explicit unit', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildFixedTrigger('trigger_ptb_manual_unit', 'magic-token', 'Moneyline: Magic', 30),
      buildReentryAction('reentry_ptb_manual_unit', {
        marketSlug: 'btc-updown-5m-1774013100',
        outcomeLabel: 'Up',
        priceToBeatGuardEnabled: true,
        priceToBeatMode: 'manual',
        priceToBeatMaxDiff: 10,
        priceToBeatMaxDiffUnit: 'usd',
        reentryPriceToBeatMaxDiff: 5,
        reentryPriceToBeatMaxDiffUnit: 'cent',
      }),
    ],
    edges: [
      {
        key: 'edge_ptb_manual_unit',
        source: 'trigger_ptb_manual_unit',
        target: 'reentry_ptb_manual_unit',
        type: 'default',
        condition: null,
      },
    ],
  });

  const issues = collectActionIssues(graph, 'reentry_ptb_manual_unit');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig rejects invalid reentry PTB override unit', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildFixedTrigger('trigger_ptb_invalid_unit', 'magic-token', 'Moneyline: Magic', 30),
      buildReentryAction('reentry_ptb_invalid_unit', {
        marketSlug: 'btc-updown-5m-1774013100',
        outcomeLabel: 'Up',
        priceToBeatGuardEnabled: true,
        priceToBeatMode: 'manual',
        priceToBeatMaxDiff: 10,
        priceToBeatMaxDiffUnit: 'usd',
        reentryPriceToBeatMaxDiff: 5,
        reentryPriceToBeatMaxDiffUnit: 'bps',
      }),
    ],
    edges: [
      {
        key: 'edge_ptb_invalid_unit',
        source: 'trigger_ptb_invalid_unit',
        target: 'reentry_ptb_invalid_unit',
        type: 'default',
        condition: null,
      },
    ],
  });

  const issues = collectActionIssues(graph, 'reentry_ptb_invalid_unit');
  assert.ok(issues.some((issue) => issue.code === 'invalid_reentry_price_to_beat_max_diff_unit'));
});

test('validateActionPlaceOrderConfig accepts manual PTB stop-loss bump', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_bump'),
      {
        key: 'ptb_bump_buy',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'manual',
          priceToBeatMaxDiff: 80,
          priceToBeatMaxDiffUnit: 'cent',
          priceToBeatStopLossBumpEnabled: true,
          priceToBeatStopLossBumpAmount: 10,
          priceToBeatStopLossBumpMaxValue: 30,
          priceToBeatStopLossBumpUnit: 'cent',
        },
      },
    ],
    edges: [{ key: 'edge_ptb_bump', source: 'trigger_ptb_bump', target: 'ptb_bump_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_bump_buy');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig rejects invalid PTB stop-loss bump config', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_bump_invalid'),
      {
        key: 'ptb_bump_invalid',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'auto_vol_pct',
          priceToBeatStopLossBumpEnabled: true,
          priceToBeatStopLossBumpAmount: 0,
          priceToBeatStopLossBumpMaxValue: 0,
          priceToBeatStopLossBumpUnit: 'shares',
        },
      },
    ],
    edges: [{ key: 'edge_ptb_bump_invalid', source: 'trigger_ptb_bump_invalid', target: 'ptb_bump_invalid', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_bump_invalid');
  assert.ok(issues.some((issue) => issue.code === 'invalid_price_to_beat_stop_loss_bump_amount'));
  assert.ok(issues.some((issue) => issue.code === 'invalid_price_to_beat_stop_loss_bump_max_value'));
  assert.ok(issues.some((issue) => issue.code === 'invalid_price_to_beat_stop_loss_bump_unit'));
});

test('validateActionPlaceOrderConfig rejects PTB stop-loss bump max below amount', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_bump_max_small'),
      {
        key: 'ptb_bump_max_small',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'manual',
          priceToBeatMaxDiff: 80,
          priceToBeatMaxDiffUnit: 'cent',
          priceToBeatStopLossBumpEnabled: true,
          priceToBeatStopLossBumpAmount: 10,
          priceToBeatStopLossBumpMaxValue: 5,
          priceToBeatStopLossBumpUnit: 'cent',
        },
      },
    ],
    edges: [{ key: 'edge_ptb_bump_max_small', source: 'trigger_ptb_bump_max_small', target: 'ptb_bump_max_small', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_bump_max_small');
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_stop_loss_bump_max_value_range')
  );
});

test('validateActionPlaceOrderConfig accepts auto PTB stop-loss bump config', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_bump_auto'),
      {
        key: 'ptb_bump_auto',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'auto_vol_pct',
          priceToBeatStopLossBumpEnabled: true,
          priceToBeatStopLossBumpAmount: 10,
          priceToBeatStopLossBumpUnit: 'cent',
        },
      },
    ],
    edges: [{ key: 'edge_ptb_bump_auto', source: 'trigger_ptb_bump_auto', target: 'ptb_bump_auto', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_bump_auto');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig accepts auto PTB relax config', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_relax_auto'),
      {
        key: 'ptb_relax_auto',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'auto_vol_pct',
          priceToBeatMaxPriceRelaxMissCount: 3,
          priceToBeatMaxPriceRelaxHistoryCount: 4,
          priceToBeatMaxPriceRelaxMinValue: 15,
          priceToBeatMaxPriceRelaxMinUnit: 'cent',
          priceToBeatMaxPriceRelaxStepMode: 'percent',
          priceToBeatMaxPriceRelaxStepValue: 25,
        },
      },
    ],
    edges: [{ key: 'edge_ptb_relax_auto', source: 'trigger_ptb_relax_auto', target: 'ptb_relax_auto', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_relax_auto');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig rejects invalid auto PTB relax config', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_relax_invalid'),
      {
        key: 'ptb_relax_invalid',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'auto_vol_pct',
          priceToBeatMaxPriceRelaxMissCount: 0,
          priceToBeatMaxPriceRelaxHistoryCount: 'x',
          priceToBeatMaxPriceRelaxMinValue: -1,
          priceToBeatMaxPriceRelaxMinUnit: 'shares',
          priceToBeatMaxPriceRelaxStepMode: 'loud',
          priceToBeatMaxPriceRelaxStepValue: 150,
          priceToBeatMaxPriceRelaxStepUnit: 'shares',
        },
      },
    ],
    edges: [{ key: 'edge_ptb_relax_invalid', source: 'trigger_ptb_relax_invalid', target: 'ptb_relax_invalid', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_relax_invalid');
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_max_price_relax_miss_count')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_max_price_relax_history_count')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_max_price_relax_min_value')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_max_price_relax_min_unit')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_max_price_relax_step_mode')
  );
});

test('validateActionPlaceOrderConfig rejects percent PTB relax step above 100', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_relax_percent_invalid'),
      {
        key: 'ptb_relax_percent_invalid',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'auto_vol_pct',
          priceToBeatMaxPriceRelaxStepMode: 'percent',
          priceToBeatMaxPriceRelaxStepValue: 150,
        },
      },
    ],
    edges: [{ key: 'edge_ptb_relax_percent_invalid', source: 'trigger_ptb_relax_percent_invalid', target: 'ptb_relax_percent_invalid', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_relax_percent_invalid');
  assert.ok(
    issues.some(
      (issue) => issue.code === 'invalid_price_to_beat_max_price_relax_step_percent_value'
    )
  );
});

test('validateActionPlaceOrderConfig accepts manual PTB relax config', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_relax_manual'),
      {
        key: 'ptb_relax_manual',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'manual',
          priceToBeatMaxDiff: 80,
          priceToBeatMaxDiffUnit: 'cent',
          priceToBeatMaxPriceRelaxMissCount: 3,
          priceToBeatMaxPriceRelaxHistoryCount: 4,
          priceToBeatMaxPriceRelaxMinValue: 15,
          priceToBeatMaxPriceRelaxMinUnit: 'cent',
          priceToBeatMaxPriceRelaxStepMode: 'absolute',
          priceToBeatMaxPriceRelaxStepValue: 10,
          priceToBeatMaxPriceRelaxStepUnit: 'cent',
        },
      },
    ],
    edges: [{ key: 'edge_ptb_relax_manual', source: 'trigger_ptb_relax_manual', target: 'ptb_relax_manual', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_relax_manual');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig rejects invalid absolute PTB relax step config', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_relax_absolute_invalid'),
      {
        key: 'ptb_relax_absolute_invalid',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'auto_vol_pct',
          priceToBeatMaxPriceRelaxStepMode: 'absolute',
          priceToBeatMaxPriceRelaxStepValue: 0,
        },
      },
    ],
    edges: [{ key: 'edge_ptb_relax_absolute_invalid', source: 'trigger_ptb_relax_absolute_invalid', target: 'ptb_relax_absolute_invalid', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_relax_absolute_invalid');
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_price_to_beat_max_price_relax_step_value')
  );
  assert.ok(
    issues.some((issue) => issue.code === 'price_to_beat_max_price_relax_step_unit_required')
  );
});

test('validateActionPlaceOrderConfig accepts auto PTB reentry override with explicit unit', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_auto'),
      buildReentryAction('reentry_ptb_auto', {
        marketSlug: 'btc-updown-5m-1774013100',
        outcomeLabel: 'Up',
        priceToBeatGuardEnabled: true,
        priceToBeatMode: 'auto_vol_pct',
        reentryPriceToBeatMaxDiff: 5,
        reentryPriceToBeatMaxDiffUnit: 'usd',
      }),
    ],
    edges: [
      {
        key: 'edge_ptb_auto',
        source: 'trigger_ptb_auto',
        target: 'reentry_ptb_auto',
        type: 'default',
        condition: null,
      },
    ],
  });

  const issues = collectActionIssues(graph, 'reentry_ptb_auto');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig rejects auto PTB reentry override without unit', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_auto_missing_unit'),
      buildReentryAction('reentry_ptb_auto_missing_unit', {
        marketSlug: 'btc-updown-5m-1774013100',
        outcomeLabel: 'Up',
        priceToBeatGuardEnabled: true,
        priceToBeatMode: 'auto_vol_pct',
        reentryPriceToBeatMaxDiff: 5,
      }),
    ],
    edges: [
      {
        key: 'edge_ptb_auto_missing_unit',
        source: 'trigger_ptb_auto_missing_unit',
        target: 'reentry_ptb_auto_missing_unit',
        type: 'default',
        condition: null,
      },
    ],
  });

  const issues = collectActionIssues(graph, 'reentry_ptb_auto_missing_unit');
  assert.ok(issues.some((issue) => issue.code === 'missing_reentry_price_to_beat_max_diff_unit'));
});

test('validateActionPlaceOrderConfig rejects reentry PTB override when reentry is disabled', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildReentryAction('reentry_ptb_disabled', {
        reenterOnSlHit: false,
        reentryMaxAttempts: undefined,
        reentryPriceToBeatMaxDiff: 5,
      }),
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'reentry_ptb_disabled');
  assert.ok(
    issues.some((issue) => issue.code === 'reentry_price_to_beat_max_diff_requires_reentry')
  );
});

test('validateActionPlaceOrderConfig rejects reentry PTB override when PTB guard is disabled', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildFixedTrigger('trigger_ptb_guard_disabled', 'magic-token', 'Moneyline: Magic', 30),
      buildReentryAction('reentry_ptb_guard_disabled', {
        marketSlug: 'btc-updown-5m-1774013100',
        outcomeLabel: 'Up',
        reentryPriceToBeatMaxDiff: 5,
      }),
    ],
    edges: [
      {
        key: 'edge_ptb_guard_disabled',
        source: 'trigger_ptb_guard_disabled',
        target: 'reentry_ptb_guard_disabled',
        type: 'default',
        condition: null,
      },
    ],
  });

  const issues = collectActionIssues(graph, 'reentry_ptb_guard_disabled');
  assert.ok(
    issues.some((issue) => issue.code === 'reentry_price_to_beat_max_diff_requires_guard')
  );
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

test('validateActionPlaceOrderConfig accepts ptb stop-loss on supported auto-scope market', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb'),
      {
        key: 'ptb_stop_buy',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          ptbStopLossEnabled: true,
          ptbStopLossGapUsd: 0,
          notifyOnSlHit: true,
          reenterOnSlHit: true,
          reentryMaxAttempts: 2,
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_ptb', target: 'ptb_stop_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_stop_buy');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig accepts ptb-only stop-loss when slEnabled remains true', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_only'),
      {
        key: 'ptb_only_with_sl_toggle',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          slEnabled: true,
          ptbStopLossEnabled: true,
          ptbStopLossGapUsd: 0,
          notifyOnSlHit: true,
          reenterOnSlHit: true,
          reentryMaxAttempts: 2,
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_ptb_only', target: 'ptb_only_with_sl_toggle', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_only_with_sl_toggle');
  assert.equal(
    issues.some((issue) => issue.code === 'missing_sl_price'),
    false
  );
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig rejects ptb stop-loss without gap value', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb'),
      {
        key: 'ptb_stop_missing_gap',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          ptbStopLossEnabled: true,
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_ptb', target: 'ptb_stop_missing_gap', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_stop_missing_gap');
  assert.ok(issues.some((issue) => issue.code === 'missing_ptb_stop_loss_config'));
});

test('validateActionPlaceOrderConfig accepts negative ptb stop-loss gap', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb'),
      {
        key: 'ptb_stop_negative_gap',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          ptbStopLossEnabled: true,
          ptbStopLossGapUsd: -1,
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_ptb', target: 'ptb_stop_negative_gap', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_stop_negative_gap');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig still requires classic sl price when ptb stop-loss is disabled', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_classic_sl'),
      {
        key: 'classic_sl_missing_price',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          slEnabled: true,
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_classic_sl', target: 'classic_sl_missing_price', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'classic_sl_missing_price');
  assert.ok(issues.some((issue) => issue.code === 'missing_sl_price'));
});

test('validateActionPlaceOrderConfig accepts staged ptb stop-loss on supported auto-scope market', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_staged'),
      {
        key: 'ptb_staged_buy',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          ptbStopLossEnabled: true,
          ptbStopLossRules: [
            { gapUsd: 12.5, sizePct: 25 },
            { gapUsd: 3, sizePct: 75 },
          ],
          reenterOnSlHit: true,
          reentryMaxAttempts: 2,
          stagedSlReentryOnlyAfterAllStages: true,
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_ptb_staged', target: 'ptb_staged_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_staged_buy');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig accepts negative staged ptb stop-loss gaps when strictly decreasing', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_staged_negative'),
      {
        key: 'ptb_staged_negative',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          ptbStopLossEnabled: true,
          ptbStopLossRules: [
            { gapUsd: 20, sizePct: 25 },
            { gapUsd: 0, sizePct: 25 },
            { gapUsd: -20, sizePct: 50 },
          ],
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_ptb_staged_negative', target: 'ptb_staged_negative', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_staged_negative');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig rejects staged ptb stop-loss when master toggle is off', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_staged_toggle'),
      {
        key: 'ptb_staged_toggle_off',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          ptbStopLossRules: [{ gapUsd: 3, sizePct: 100 }],
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_ptb_staged_toggle', target: 'ptb_staged_toggle_off', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_staged_toggle_off');
  assert.ok(issues.some((issue) => issue.code === 'ptb_stop_loss_toggle_required'));
});

test('validateActionPlaceOrderConfig rejects empty ptb master toggle without hard gap or rules', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_empty_toggle'),
      {
        key: 'ptb_empty_toggle',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          ptbStopLossEnabled: true,
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_ptb_empty_toggle', target: 'ptb_empty_toggle', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_empty_toggle');
  assert.ok(issues.some((issue) => issue.code === 'missing_ptb_stop_loss_config'));
});

test('validateActionPlaceOrderConfig rejects staged ptb stop-loss when sizePct total is not 100', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_staged_sum'),
      {
        key: 'ptb_staged_invalid_sum',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          ptbStopLossRules: [
            { gapUsd: 12.5, sizePct: 25 },
            { gapUsd: 3, sizePct: 50 },
          ],
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_ptb_staged_sum', target: 'ptb_staged_invalid_sum', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_staged_invalid_sum');
  assert.ok(issues.some((issue) => issue.code === 'invalid_ptb_stop_loss_rules_sum'));
});

test('validateActionPlaceOrderConfig rejects staged ptb stop-loss when gaps are not strictly decreasing', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_staged_order'),
      {
        key: 'ptb_staged_invalid_order',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          ptbStopLossRules: [
            { gapUsd: 3, sizePct: 50 },
            { gapUsd: 3, sizePct: 50 },
          ],
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_ptb_staged_order', target: 'ptb_staged_invalid_order', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_staged_invalid_order');
  assert.ok(issues.some((issue) => issue.code === 'invalid_ptb_stop_loss_rules_order'));
});

test('validateActionPlaceOrderConfig accepts classic sl together with ptb stop-loss', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_both_sl'),
      {
        key: 'classic_and_ptb_sl',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          slEnabled: true,
          slPriceCent: 30,
          ptbStopLossEnabled: true,
          ptbStopLossGapUsd: 0,
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_both_sl', target: 'classic_and_ptb_sl', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'classic_and_ptb_sl');
  assert.equal(issues.length, 0);
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

test('validateActionPlaceOrderConfig accepts staged sl reentry-after-all-stages config', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildFixedTrigger('trigger_reentry', 'magic-token', 'Moneyline: Magic', 30),
      {
        key: 'staged_sl_buy',
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
          slRules: [
            { priceCent: 45, sizePct: 40 },
            { priceCent: 35, sizePct: 60 },
          ],
          reenterOnSlHit: true,
          reentryMaxAttempts: 2,
          stagedSlReentryOnlyAfterAllStages: true,
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_reentry', target: 'staged_sl_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'staged_sl_buy');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig rejects staged sl reentry-after-all-stages without slRules', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'staged_sl_invalid',
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
          reenterOnSlHit: true,
          reentryMaxAttempts: 2,
          stagedSlReentryOnlyAfterAllStages: true,
        },
      },
    ],
    edges: [],
  });

  const issues = collectActionIssues(graph, 'staged_sl_invalid');
  assert.ok(
    issues.some(
      (issue) =>
        issue.code === 'staged_sl_reentry_only_after_all_stages_requires_sl_rules'
    )
  );
});
