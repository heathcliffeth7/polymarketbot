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
      cycleWindowMode: 'custom_range',
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

function buildAutoScopeTriggerWithConfig(
  key: string,
  config: Record<string, unknown>
): TradeFlowNode {
  const trigger = buildAutoScopeTrigger(key);
  trigger.config = { ...trigger.config, ...config };
  return trigger;
}

function buildBiasedHedgeConfig(
  overrides: Record<string, unknown> = {}
): Record<string, unknown> {
  const biasedOverrides =
    overrides.biasedHedge && typeof overrides.biasedHedge === 'object'
      ? (overrides.biasedHedge as Record<string, unknown>)
      : {};
  const stopOverrides =
    overrides.biasedHedgeStop && typeof overrides.biasedHedgeStop === 'object'
      ? (overrides.biasedHedgeStop as Record<string, unknown>)
      : {};
  const base = {
    mode: 'pair_lock',
    pairLockStrategy: 'biased_hedge_v1',
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 2,
    maxPriceCent: 75,
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'iv_mismatch_edge',
    counterLegEnabled: true,
    counterLegOutcomeLabel: 'opposite',
    pairProtectiveUnwindEnabled: true,
    pairOrphanGraceMs: 1500,
    tpEnabled: false,
    reentryMaxAttempts: 0,
    biasedHedgeMaxPairedEffectiveCostCent: 95,
    biasedHedge: {
      primaryBudgetUsdc: 2,
      hedgeBudgetUsdc: 0.5,
      minDominantShare: 0.75,
      maxHedgeSpendRatio: 0.25,
      primaryMinEdge: 0.08,
      primaryMinFinalQ: 0.72,
      maxPriceCent: 75,
      highPriceCent: 70,
      highPriceMinFinalQ: 0.82,
      highPriceMinEdge: 0.10,
      hedgeOnlyIfPrimaryFilled: true,
      hedgeMinPriceCent: 3,
      hedgeMaxPriceCent: 25,
      disableNewPrimaryAfterSec: 180,
      disableAnyBuyAfterSec: 240,
      maxSideSwitchCount: 0,
      ...biasedOverrides,
    },
    biasedHedgeStop: {
      biasInvalidationEnabled: true,
      minQFinalToHold: 0.55,
      minEdgeToHold: 0,
      exitPctOnInvalidation: 100,
      ptbStopLossEnabled: true,
      ptbStopLossGapUsd: -3,
      ptbStopLossTimeDecayMode: 'tighten',
      timeExitRules: [
        { elapsedSec: 90, remainingPct: 60 },
        { elapsedSec: 150, remainingPct: 0 },
      ],
      ...stopOverrides,
    },
  };
  const result: Record<string, unknown> = { ...base, ...overrides, biasedHedge: base.biasedHedge, biasedHedgeStop: base.biasedHedgeStop };
  if ('biasedHedge' in overrides && overrides.biasedHedge == null) {
    result.biasedHedge = overrides.biasedHedge;
  }
  if ('biasedHedgeStop' in overrides && overrides.biasedHedgeStop == null) {
    result.biasedHedgeStop = overrides.biasedHedgeStop;
  }
  return result;
}

function buildBiasedHedgeGraph(
  actionOverrides: Record<string, unknown> = {},
  triggerOverrides: Record<string, unknown> = {}
): TradeFlowGraph {
  return normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTriggerWithConfig('trigger_biased', {
        cycleWindowStartSec: 30,
        cycleWindowEndSec: 180,
        ...triggerOverrides,
      }),
      {
        key: 'pair_buy_biased',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: buildBiasedHedgeConfig(actionOverrides),
      },
    ],
    edges: [{ key: 'edge_biased', source: 'trigger_biased', target: 'pair_buy_biased', type: 'default', condition: null }],
  });
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
          pairProtectiveUnwindEnabled: false,
          pairIgnoreStopLossAfterLocked: true,
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

test('validateActionPlaceOrderConfig accepts edge_pairlock_v1 share qty config', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_edge'),
      {
        key: 'pair_buy_edge',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          pairLockStrategy: 'edge_pairlock_v1',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          sizeUsdc: 5,
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'iv_mismatch_edge',
          pairMaxTotalCent: 95,
          pairLockDecisionQty: 5,
          pairLockSingleEdgeThreshold: 0.10,
          pairLockCostBuffer: 0.005,
          counterLegEnabled: true,
          counterLegOutcomeLabel: 'opposite',
        },
      },
    ],
    edges: [{ key: 'edge_pair_edge', source: 'trigger_pair_edge', target: 'pair_buy_edge', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_edge');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig accepts biased_hedge_v1 smoke config', () => {
  const graph = buildBiasedHedgeGraph();

  const issues = collectActionIssues(graph, 'pair_buy_biased');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig accepts biased_hedge_v1 explicit early IV time rule', () => {
  const graph = buildBiasedHedgeGraph({
    priceToBeatIvTimeRules: [
      {
        startRemainingSec: 270,
        endRemainingSec: 120,
        maxPriceCent: 75,
        minEdge: 0.08,
        minGapStrength: 0,
      },
    ],
  });

  const issues = collectActionIssues(graph, 'pair_buy_biased');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig rejects biased_hedge_v1 IV time rules outside entry window', () => {
  const graph = buildBiasedHedgeGraph({
    priceToBeatIvTimeRules: [
      {
        startRemainingSec: 90,
        endRemainingSec: 30,
        maxPriceCent: 75,
        minEdge: 0.08,
        minGapStrength: 0,
      },
    ],
  });

  const issues = collectActionIssues(graph, 'pair_buy_biased');
  assert.ok(
    issues.some((issue) => issue.code === 'biased_hedge_iv_time_rules_no_entry_overlap')
  );
});

test('validateActionPlaceOrderConfig rejects biased_hedge_v1 without stop config', () => {
  const graph = buildBiasedHedgeGraph({ biasedHedgeStop: null });

  const issues = collectActionIssues(graph, 'pair_buy_biased');
  assert.ok(issues.some((issue) => issue.code === 'biased_hedge_stop_required'));
  assert.ok(issues.some((issue) => issue.code === 'biased_hedge_time_exit_required'));
});

test('validateActionPlaceOrderConfig rejects biased_hedge_v1 hedge before primary fill', () => {
  const graph = buildBiasedHedgeGraph({
    biasedHedge: { hedgeOnlyIfPrimaryFilled: false },
  });

  const issues = collectActionIssues(graph, 'pair_buy_biased');
  assert.ok(issues.some((issue) => issue.code === 'biased_hedge_requires_primary_fill_before_hedge'));
});

test('validateActionPlaceOrderConfig rejects biased_hedge_v1 weak high-price guard', () => {
  const graph = buildBiasedHedgeGraph({
    biasedHedge: { highPriceMinFinalQ: 0.77 },
  });

  const issues = collectActionIssues(graph, 'pair_buy_biased');
  assert.ok(issues.some((issue) => issue.code === 'biased_hedge_high_price_min_q_too_low'));
});

test('validateActionPlaceOrderConfig rejects biased_hedge_v1 dominance-breaking hedge ratio', () => {
  const graph = buildBiasedHedgeGraph({
    biasedHedge: { minDominantShare: 0.80, maxHedgeSpendRatio: 0.30 },
  });

  const issues = collectActionIssues(graph, 'pair_buy_biased');
  assert.ok(issues.some((issue) => issue.code === 'biased_hedge_ratio_breaks_dominance'));
});

test('validateActionPlaceOrderConfig rejects biased_hedge_v1 late cycle window', () => {
  const graph = buildBiasedHedgeGraph({}, { cycleWindowEndSec: 260 });

  const issues = collectActionIssues(graph, 'pair_buy_biased');
  assert.ok(issues.some((issue) => issue.code === 'biased_hedge_cycle_window_end_too_late'));
});

test('validateActionPlaceOrderConfig accepts pair_lock primary PTB bump loss table config', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_bump_loss_table'),
      {
        key: 'pair_buy_bump_loss_table',
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
          pairSizingMode: 'manual',
          counterLegEnabled: true,
          counterLegSizeUsdc: 5,
          counterLegOutcomeLabel: 'opposite',
          priceToBeatGuardEnabled: true,
          priceToBeatMode: 'auto_vol_pct',
          priceToBeatStopLossBumpEnabled: true,
          priceToBeatStopLossBumpMode: 'loss_table',
          priceToBeatStopLossBumpUnit: 'cent',
          priceToBeatStopLossBumpLossRules: [
            { lossUsd: 1, bumpValue: 25 },
            { lossUsd: 2, bumpValue: 50 },
          ],
        },
      },
    ],
    edges: [{ key: 'edge_pair_bump_loss_table', source: 'trigger_pair_bump_loss_table', target: 'pair_buy_bump_loss_table', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_bump_loss_table');
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

test('validateActionPlaceOrderConfig rejects pair_lock shares sizing even with targetQty', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_shares'),
      {
        key: 'pair_buy_shares',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'shares',
          targetQty: 5,
          pairMaxTotalCent: 90,
          pairSizingMode: 'auto_remaining_budget',
          pairTotalBudgetUsdc: 10,
          counterLegEnabled: true,
        },
      },
    ],
    edges: [{ key: 'edge_pair_shares', source: 'trigger_pair_shares', target: 'pair_buy_shares', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_shares');
  assert.ok(issues.some((issue) => issue.code === 'pair_lock_requires_usdc_sizing'));
  assert.ok(issues.some((issue) => issue.code === 'pair_lock_requires_size_usdc'));
});

test('validateActionPlaceOrderConfig rejects pair_lock without primary USDC sizing', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_missing_size'),
      {
        key: 'pair_buy_missing_size',
        type: 'action.place_order',
        positionX: 240,
        positionY: 0,
        config: {
          mode: 'pair_lock',
          side: 'buy',
          executionMode: 'limit',
          sizeMode: 'usdc',
          targetQty: 5,
          pairMaxTotalCent: 90,
          pairSizingMode: 'auto_remaining_budget',
          pairTotalBudgetUsdc: 10,
          counterLegEnabled: true,
        },
      },
    ],
    edges: [{ key: 'edge_pair_missing_size', source: 'trigger_pair_missing_size', target: 'pair_buy_missing_size', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_missing_size');
  assert.ok(issues.some((issue) => issue.code === 'pair_lock_requires_size_usdc'));
});

test('validateActionPlaceOrderConfig accepts pair_lock lead-leg hard, staged, and PTB stop-loss fields', () => {
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
          slRules: [
            { priceCent: 45, sizePct: 60 },
            { priceCent: 40, sizePct: 40 },
          ],
          ptbStopLossEnabled: true,
          ptbStopLossGapUsd: 0,
          ptbStopLossGapUnit: 'usd',
          ptbStopLossTimeDecayMode: 'relax',
          ptbStopLossRules: [
            { gapUsd: 7, sizePct: 60 },
            { gapUsd: 0, sizePct: 40 },
          ],
          notifyOnSlHit: true,
          counterLegSlEnabled: true,
          counterLegSlPriceCent: 38,
          counterLegSlTriggerPriceMode: 'best_bid',
          counterLegPtbStopLossEnabled: true,
          counterLegPtbStopLossGapUsd: -2,
          counterLegPtbStopLossGapUnit: 'cent',
          counterLegPtbStopLossTimeDecayMode: 'relax',
          counterLegNotifyOnSlHit: true,
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

test('validateActionPlaceOrderConfig rejects invalid counter leg ptb stop-loss gap unit', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_counter_bad_unit'),
      {
        key: 'pair_buy_counter_bad_unit',
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
          counterLegPtbStopLossEnabled: true,
          counterLegPtbStopLossGapUsd: -2,
          counterLegPtbStopLossGapUnit: 'ticks',
        },
      },
    ],
    edges: [{ key: 'edge_pair_counter_bad_unit', source: 'trigger_pair_counter_bad_unit', target: 'pair_buy_counter_bad_unit', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_counter_bad_unit');
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_counter_leg_ptb_stop_loss_gap_unit')
  );
});

test('validateActionPlaceOrderConfig rejects counter leg stop-loss without required thresholds', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_counter_sl'),
      {
        key: 'pair_buy_counter_sl',
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
          counterLegSlEnabled: true,
          counterLegPtbStopLossEnabled: true,
        },
      },
    ],
    edges: [{ key: 'edge_pair_counter_sl', source: 'trigger_pair_counter_sl', target: 'pair_buy_counter_sl', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_counter_sl');
  assert.ok(issues.some((issue) => issue.code === 'invalid_counter_leg_sl_price_cent'));
  assert.ok(issues.some((issue) => issue.code === 'invalid_counter_leg_ptb_stop_loss_gap_usd'));
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

test('validateActionPlaceOrderConfig accepts pair_lock with primary take profit', () => {
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
          notifyOnTpHit: true,
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
  assert.equal(
    issues.some((issue) => issue.code === 'pair_lock_disallows_exit_features'),
    false
  );
  assert.equal(
    issues.some((issue) => issue.code === 'notify_on_tp_hit_requires_tp'),
    false
  );
});

test('validateActionPlaceOrderConfig accepts pair_lock with counter take profit', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_counter_tp'),
      {
        key: 'pair_buy_counter_tp',
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
          counterLegTpEnabled: true,
          counterLegTpPriceCent: 82,
          counterLegTpRules: [{ priceCent: 82, sizePct: 100 }],
          counterLegNotifyOnTpHit: true,
        },
      },
    ],
    edges: [
      {
        key: 'edge_pair_counter_tp',
        source: 'trigger_pair_counter_tp',
        target: 'pair_buy_counter_tp',
        type: 'default',
        condition: null,
      },
    ],
  });

  const issues = collectActionIssues(graph, 'pair_buy_counter_tp');
  assert.equal(
    issues.some((issue) => issue.code === 'pair_lock_disallows_exit_features'),
    false
  );
  assert.equal(
    issues.some((issue) => issue.code === 'counter_leg_notify_on_tp_hit_requires_take_profit'),
    false
  );
});

test('validateActionPlaceOrderConfig rejects counter TP without price or rules', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_counter_tp_missing'),
      {
        key: 'pair_buy_counter_tp_missing',
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
          counterLegTpEnabled: true,
        },
      },
    ],
    edges: [
      {
        key: 'edge_pair_counter_tp_missing',
        source: 'trigger_pair_counter_tp_missing',
        target: 'pair_buy_counter_tp_missing',
        type: 'default',
        condition: null,
      },
    ],
  });

  const issues = collectActionIssues(graph, 'pair_buy_counter_tp_missing');
  assert.ok(issues.some((issue) => issue.code === 'counter_leg_tp_requires_price_or_rules'));
});

test('validateActionPlaceOrderConfig rejects counter TP notify without TP', () => {
  const graph = normalizeTradeFlowGraph({
    context: {},
    nodes: [
      buildAutoScopeTrigger('trigger_pair_counter_tp_notify'),
      {
        key: 'pair_buy_counter_tp_notify',
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
          counterLegNotifyOnTpHit: true,
        },
      },
    ],
    edges: [
      {
        key: 'edge_pair_counter_tp_notify',
        source: 'trigger_pair_counter_tp_notify',
        target: 'pair_buy_counter_tp_notify',
        type: 'default',
        condition: null,
      },
    ],
  });

  const issues = collectActionIssues(graph, 'pair_buy_counter_tp_notify');
  assert.ok(
    issues.some((issue) => issue.code === 'counter_leg_notify_on_tp_hit_requires_take_profit')
  );
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

test('validateActionPlaceOrderConfig rejects counter staged and advanced stop-loss extensions in pair_lock mode', () => {
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
          counterLegSlRules: [{ priceCent: 40, sizePct: 100 }],
          reentryMinPriceCent: 35,
        },
      },
    ],
    edges: [{ key: 'edge_pair_advanced_exit', source: 'trigger_pair_advanced_exit', target: 'pair_buy_advanced_exit', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'pair_buy_advanced_exit');
  assert.ok(issues.some((issue) => issue.code === 'pair_lock_disallows_exit_features'));
});
