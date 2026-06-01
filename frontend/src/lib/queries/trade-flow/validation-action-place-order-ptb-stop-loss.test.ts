import assert from 'node:assert/strict';
import test from 'node:test';

import { normalizeTradeFlowGraph } from '@/lib/queries/trade-flow/graph';

import {
  buildAutoScopeTrigger,
  collectActionIssues,
} from './validation-action-place-order.test-helpers';

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

test('validateActionPlaceOrderConfig accepts cent-based ptb stop-loss on supported auto-scope market', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_cent'),
      {
        key: 'ptb_stop_buy_cent',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          ptbStopLossEnabled: true,
          ptbStopLossGapUsd: 20,
          ptbStopLossGapUnit: 'cent',
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_ptb_cent', target: 'ptb_stop_buy_cent', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_stop_buy_cent');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig accepts ptb stop-loss current source override', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_source'),
      {
        key: 'ptb_stop_buy_source',
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
          ptbStopLossCurrentPriceSource: 'hyperliquid',
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_ptb_source', target: 'ptb_stop_buy_source', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_stop_buy_source');
  assert.equal(issues.length, 0);
});

test('validateActionPlaceOrderConfig rejects inactive ptb stop-loss current source override', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_source_inactive'),
      {
        key: 'ptb_stop_buy_source_inactive',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          ptbStopLossCurrentPriceSource: 'binance',
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_ptb_source_inactive', target: 'ptb_stop_buy_source_inactive', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_stop_buy_source_inactive');
  assert.ok(issues.some((issue) => issue.code === 'ptb_stop_loss_current_price_source_requires_ptb_stop_loss'));
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

test('validateActionPlaceOrderConfig accepts negative ptb stop-loss gap for counter-direction overshoot semantics', () => {
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
          ptbStopLossGapUsd: -10,
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
          ptbStopLossGapUnit: 'cent',
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

test('validateActionPlaceOrderConfig rejects invalid ptb stop-loss gap unit', () => {
  const graph = normalizeTradeFlowGraph({
    context: { sourceTradeId: 42 },
    nodes: [
      buildAutoScopeTrigger('trigger_ptb_bad_unit'),
      {
        key: 'ptb_bad_unit',
        type: 'action.place_order',
        positionX: 200,
        positionY: 0,
        config: {
          side: 'buy',
          executionMode: 'market',
          sizeMode: 'usdc',
          sizeUsdc: 10,
          ptbStopLossEnabled: true,
          ptbStopLossGapUsd: 20,
          ptbStopLossGapUnit: 'ticks',
        },
      },
    ],
    edges: [{ key: 'edge_1', source: 'trigger_ptb_bad_unit', target: 'ptb_bad_unit', type: 'default', condition: null }],
  });

  const issues = collectActionIssues(graph, 'ptb_bad_unit');
  assert.ok(issues.some((issue) => issue.code === 'invalid_ptb_stop_loss_gap_unit'));
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
