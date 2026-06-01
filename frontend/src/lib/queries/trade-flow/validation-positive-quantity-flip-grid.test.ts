import assert from 'node:assert/strict';
import test from 'node:test';

import { normalizeTradeFlowGraph } from '@/lib/queries/trade-flow/graph';

import { collectActionIssues } from './validation-action-place-order.test-helpers';

function positiveGridGraph(overrides: Record<string, unknown> = {}) {
  return normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      {
        key: 'positive_grid_trigger',
        type: 'trigger.market_price',
        positionX: 0,
        positionY: 0,
        config: {
          marketMode: 'auto_scope',
          marketScope: 'btc_5m_updown',
          marketSelection: 'latest_by_slug',
          bindingMode: 'positive_quantity_flip_grid_only',
          priceMode: 'composite',
          repeatMode: 'once',
        },
      },
      {
        key: 'positive_grid_buy',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          mode: 'positive_quantity_flip_grid_v1',
          side: 'buy',
          kind: 'immediate',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 1,
          positiveQuantityFlipGrid: {
            entryBandMinCent: 50,
            entryBandMaxCent: 60,
          },
          ...overrides,
        },
      },
    ],
    edges: [
      {
        key: 'edge_positive_grid',
        source: 'positive_grid_trigger',
        target: 'positive_grid_buy',
        type: 'default',
        condition: null,
      },
    ],
  });
}

test('positive grid accepts classic SL and PTB stop-loss for normal buy config', () => {
  const graph = positiveGridGraph({
    slEnabled: true,
    slPriceCent: 42,
    slRules: [
      { priceCent: 44, sizePct: 60 },
      { priceCent: 36, sizePct: 40 },
    ],
    ptbStopLossEnabled: true,
    ptbStopLossRules: [
      { gapUsd: 20, sizePct: 60 },
      { gapUsd: 0, sizePct: 40 },
    ],
    ptbStopLossGapUnit: 'cent',
    ptbStopLossCurrentPriceSource: 'hyperliquid',
    ptbStopLossTimeDecayMode: 'relax',
  });

  assert.deepEqual(collectActionIssues(graph, 'positive_grid_buy'), []);
});

test('positive grid accepts separate normal and rescue entry PTB guard config', () => {
  const graph = positiveGridGraph({
    positiveQuantityFlipGrid: {
      entryBandMinCent: 50,
      entryBandMaxCent: 60,
      rescueBuyEnabled: true,
      ptbGuardEnabled: true,
      ptbMinDiff: 80,
      ptbRescueMinDiff: 40,
      ptbDiffUnit: 'usd',
      ptbCurrentPriceSource: 'binance',
    },
  });

  assert.deepEqual(collectActionIssues(graph, 'positive_grid_buy'), []);
});

test('positive grid rejects non-positive rescue entry PTB guard diff', () => {
  const graph = positiveGridGraph({
    positiveQuantityFlipGrid: {
      entryBandMinCent: 50,
      entryBandMaxCent: 60,
      rescueBuyEnabled: true,
      ptbGuardEnabled: true,
      ptbMinDiff: 80,
      ptbRescueMinDiff: 0,
    },
  });

  const issues = collectActionIssues(graph, 'positive_grid_buy');
  assert.ok(
    issues.some((issue) => issue.code === 'invalid_positive_grid_ptb_rescue_min_diff'),
  );
});

test('positive grid still rejects classic TP config', () => {
  const issues = collectActionIssues(
    positiveGridGraph({ tpEnabled: true, tpPriceCent: 98 }),
    'positive_grid_buy',
  );

  assert.ok(
    issues.some((issue) => issue.code === 'positive_grid_disallows_classic_exits'),
  );
});

test('positive grid requires a classic SL price when hard SL is enabled', () => {
  const issues = collectActionIssues(
    positiveGridGraph({ slEnabled: true }),
    'positive_grid_buy',
  );

  assert.ok(issues.some((issue) => issue.code === 'missing_sl_price'));
});

test('positive grid validates staged SL rule ordering', () => {
  const issues = collectActionIssues(
    positiveGridGraph({
      slRules: [
        { priceCent: 35, sizePct: 50 },
        { priceCent: 40, sizePct: 50 },
      ],
    }),
    'positive_grid_buy',
  );

  assert.ok(issues.some((issue) => issue.code === 'invalid_sl_rules_order'));
});

test('positive grid validates PTB stop-loss unit', () => {
  const issues = collectActionIssues(
    positiveGridGraph({
      ptbStopLossEnabled: true,
      ptbStopLossGapUsd: 0,
      ptbStopLossGapUnit: 'ticks',
    }),
    'positive_grid_buy',
  );

  assert.ok(issues.some((issue) => issue.code === 'invalid_ptb_stop_loss_gap_unit'));
});
