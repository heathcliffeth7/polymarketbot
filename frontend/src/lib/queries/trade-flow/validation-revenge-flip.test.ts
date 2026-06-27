import assert from 'node:assert/strict';
import test from 'node:test';

import { normalizeTradeFlowGraph } from '@/lib/queries/trade-flow/graph';
import type { TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { validateTriggerMarketPriceNodeConfig } from './validation-trigger-market-price';
import { collectActionIssues } from './validation-action-place-order.test-helpers';

function trigger(bindingMode = 'revenge_flip_only'): TradeFlowNode {
  return {
    key: 'trigger_rf',
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

function action(overrides: Record<string, unknown> = {}): TradeFlowNode {
  return {
    key: 'revenge_buy',
    type: 'action.place_order',
    positionX: 300,
    positionY: 0,
    config: {
      mode: 'revenge_flip_v1',
      revengeFlip: {
        initialOrderUsdc: 5,
        profitTargetUsdc: 0.25,
        stopLossPct: 0.2,
        stopLossRules: [
          { minFlip: 0, maxFlip: 0, stopLossPct: 0.2 },
          { minFlip: 1, stopLossPct: 0.15 },
        ],
        reentrySideMode: 'rule_match',
        entryPtbRules: [
          { minFlip: 0, maxFlip: 0, sideMode: 'any', minRemainingSec: 0, maxRemainingSec: 300, priceToBeatMaxDiff: 5, priceToBeatMaxDiffUnit: 'cent', maxPriceCent: 80 },
          { minFlip: 1, sideMode: 'same', priceToBeatMaxDiff: 2, priceToBeatMaxDiffUnit: 'cent', maxPriceCent: 90 },
        ],
        ptbStopLossEnabled: true,
        ptbStopLossGapUsd: -4,
        ptbStopLossGapUnit: 'cent',
        ptbStopLossCurrentPriceSource: 'chainlink_cex_consensus',
        ptbStopLossTimeDecayMode: 'none',
        postStopLossIvMismatchEnabled: true,
        lotLimitPct: 0.98,
        closeOnlySec: 10,
        maxFlip: 0,
      },
      triggerPrice: { enabled: true, minCent: 40, maxCent: 65 },
      priceToBeatGuardEnabled: true,
      priceToBeatMaxDiff: 0.01,
      priceToBeatMaxDiffUnit: 'usd',
      ...overrides,
    },
  };
}

test('validateActionPlaceOrderConfig accepts revenge_flip_v1 with revenge binding', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [trigger(), action()],
    edges: [{ key: 'edge_rf', source: 'trigger_rf', target: 'revenge_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'revenge_buy');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig accepts revenge_flip_v1 with PTB-only stop-loss', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      trigger(),
      action({
        revengeFlip: {
          initialOrderUsdc: 2,
          profitTargetUsdc: 0.25,
          classicStopLossEnabled: false,
          ptbStopLossEnabled: true,
          ptbStopLossGapUsd: 1,
          ptbStopLossGapUnit: 'usd',
          ptbStopLossTimeDecayMode: 'none',
          stopLossRules: [{ minFlip: 2, maxFlip: 1, stopLossPct: 0.1 }],
          lotLimitPct: 0.98,
          closeOnlySec: 10,
          maxFlip: 0,
        },
      }),
    ],
    edges: [{ key: 'edge_rf', source: 'trigger_rf', target: 'revenge_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'revenge_buy');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig accepts negative revenge target pnl', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      trigger(),
      action({
        revengeFlip: {
          initialOrderUsdc: 5,
          profitTargetUsdc: -1,
          stopLossPct: 0.2,
          lotLimitPct: 0.98,
          closeOnlySec: 10,
          maxFlip: 0,
        },
      }),
    ],
    edges: [{ key: 'edge_rf', source: 'trigger_rf', target: 'revenge_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'revenge_buy');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig rejects invalid revenge post-stop-loss IV mismatch toggle', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      trigger(),
      action({
        revengeFlip: {
          initialOrderUsdc: 5,
          profitTargetUsdc: 0.25,
          stopLossPct: 0.2,
          postStopLossIvMismatchEnabled: 'maybe',
          lotLimitPct: 0.98,
          closeOnlySec: 10,
          maxFlip: 0,
        },
      }),
    ],
    edges: [{ key: 'edge_rf', source: 'trigger_rf', target: 'revenge_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'revenge_buy');
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_revenge_flip_post_stop_loss_iv_mismatch')
  );
});

test('validateActionPlaceOrderConfig rejects revenge PTB-only mode without PTB stop-loss', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      trigger(),
      action({
        revengeFlip: {
          initialOrderUsdc: 2,
          profitTargetUsdc: 0.25,
          classicStopLossEnabled: false,
          ptbStopLossEnabled: false,
          lotLimitPct: 0.98,
          closeOnlySec: 10,
          maxFlip: 0,
        },
      }),
    ],
    edges: [{ key: 'edge_rf', source: 'trigger_rf', target: 'revenge_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'revenge_buy');
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_revenge_flip_classic_stop_loss_disabled')
  );
});

test('validateActionPlaceOrderConfig rejects invalid revenge entry PTB rule', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      trigger(),
      action({
        revengeFlip: {
          initialOrderUsdc: 5,
          profitTargetUsdc: 0.25,
          stopLossPct: 0.2,
          entryPtbRules: [{ minFlip: 1, maxFlip: 0, priceToBeatMaxDiff: 2, priceToBeatMaxDiffUnit: 'cent' }],
          lotLimitPct: 0.98,
          closeOnlySec: 10,
          maxFlip: 0,
        },
      }),
    ],
    edges: [{ key: 'edge_rf', source: 'trigger_rf', target: 'revenge_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'revenge_buy');
  assert.ok(issues.some((issue) => issue.code === 'invalid_revenge_flip_entry_ptb_rule_flip_range'));
});

test('validateActionPlaceOrderConfig rejects invalid revenge entry max price', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      trigger(),
      action({
        revengeFlip: {
          initialOrderUsdc: 5,
          profitTargetUsdc: 0.25,
          stopLossPct: 0.2,
          entryPtbRules: [{ minFlip: 0, priceToBeatMaxDiff: 10, priceToBeatMaxDiffUnit: 'cent', maxPriceCent: 101 }],
          lotLimitPct: 0.98,
          closeOnlySec: 10,
          maxFlip: 0,
        },
      }),
    ],
    edges: [{ key: 'edge_rf', source: 'trigger_rf', target: 'revenge_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'revenge_buy');
  assert.ok(issues.some((issue) => issue.code === 'invalid_revenge_flip_entry_ptb_rule_max_price'));
});

test('validateActionPlaceOrderConfig rejects invalid revenge entry side mode', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      trigger(),
      action({
        revengeFlip: {
          initialOrderUsdc: 5,
          profitTargetUsdc: 0.25,
          stopLossPct: 0.2,
          entryPtbRules: [{ minFlip: 0, sideMode: 'sideways', priceToBeatMaxDiff: 10 }],
          lotLimitPct: 0.98,
          closeOnlySec: 10,
          maxFlip: 0,
        },
      }),
    ],
    edges: [{ key: 'edge_rf', source: 'trigger_rf', target: 'revenge_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'revenge_buy');
  assert.ok(issues.some((issue) => issue.code === 'invalid_revenge_flip_entry_ptb_rule_side_mode'));
});

test('validateActionPlaceOrderConfig rejects invalid revenge reentry and PTB stop-loss config', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      trigger(),
      action({
        revengeFlip: {
          initialOrderUsdc: 5,
          profitTargetUsdc: 0.25,
          stopLossPct: 0.2,
          reentrySideMode: 'both',
          ptbStopLossEnabled: true,
          ptbStopLossGapUnit: 'points',
          ptbStopLossCurrentPriceSource: 'kraken',
          ptbStopLossTimeDecayMode: 'fast',
          lotLimitPct: 0.98,
          closeOnlySec: 10,
          maxFlip: 0,
        },
      }),
    ],
    edges: [{ key: 'edge_rf', source: 'trigger_rf', target: 'revenge_buy', type: 'default', condition: null }],
  });

  const codes = collectActionIssues(graph, 'revenge_buy').map((issue) => issue.code);
  assert.ok(codes.includes('invalid_revenge_flip_reentry_side_mode'));
  assert.ok(codes.includes('invalid_revenge_flip_ptb_stop_loss_gap_unit'));
  assert.ok(codes.includes('invalid_revenge_flip_ptb_stop_loss_current_source'));
  assert.ok(codes.includes('invalid_revenge_flip_ptb_stop_loss_time_mode'));
  assert.ok(codes.includes('invalid_revenge_flip_ptb_stop_loss_gap'));
});

test('validateActionPlaceOrderConfig rejects invalid revenge min re-entry shares', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      trigger(),
      action({
        revengeFlip: {
          initialOrderUsdc: 5,
          profitTargetUsdc: 0.25,
          stopLossPct: 0.2,
          minReentryShares: -1,
          lotLimitPct: 0.98,
          closeOnlySec: 10,
          maxFlip: 0,
        },
      }),
    ],
    edges: [{ key: 'edge_rf', source: 'trigger_rf', target: 'revenge_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'revenge_buy');
  assert.ok(issues.some((issue) => issue.code === 'invalid_revenge_flip_min_reentry_shares'));
});

test('validateActionPlaceOrderConfig rejects invalid revenge stop-loss rule', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      trigger(),
      action({
        revengeFlip: {
          initialOrderUsdc: 5,
          profitTargetUsdc: 0.25,
          stopLossPct: 0.2,
          stopLossRules: [{ minFlip: 2, maxFlip: 1, stopLossPct: 0.1 }],
          lotLimitPct: 0.98,
          closeOnlySec: 10,
          maxFlip: 0,
        },
      }),
    ],
    edges: [{ key: 'edge_rf', source: 'trigger_rf', target: 'revenge_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'revenge_buy');
  assert.ok(issues.some((issue) => issue.code === 'invalid_revenge_flip_stop_loss_rule_range'));
});

test('validateActionPlaceOrderConfig rejects revenge_flip_v1 without revenge binding', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [trigger('standard'), action()],
    edges: [{ key: 'edge_rf', source: 'trigger_rf', target: 'revenge_buy', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'revenge_buy');
  assert.ok(issues.some((issue) => issue.code === 'revenge_flip_requires_revenge_flip_only_trigger'));
});

test('validateTriggerMarketPriceNodeConfig accepts revenge_flip_only with single downstream action', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [trigger(), action()],
    edges: [{ key: 'edge_rf', source: 'trigger_rf', target: 'revenge_buy', type: 'default', condition: null }],
  });
  const issues: TradeFlowValidationIssue[] = [];

  validateTriggerMarketPriceNodeConfig(issues, graph.nodes[0], graph);
  assert.equal(issues.length, 0);
});

test('validateTriggerMarketPriceNodeConfig rejects revenge_flip_only outcome rows', () => {
  const badTrigger = trigger();
  badTrigger.config = {
    ...(badTrigger.config as Record<string, unknown>),
    outcomeConditions: [{ triggerCondition: 'level_above', triggerPriceCent: 50 }],
  };
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [badTrigger, action()],
    edges: [{ key: 'edge_rf', source: 'trigger_rf', target: 'revenge_buy', type: 'default', condition: null }],
  });
  const issues: TradeFlowValidationIssue[] = [];

  validateTriggerMarketPriceNodeConfig(issues, graph.nodes[0], graph);
  assert.ok(issues.some((issue) => issue.code === 'revenge_flip_only_disallows_outcome_conditions'));
});
