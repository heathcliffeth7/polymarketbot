import assert from 'node:assert/strict';
import test from 'node:test';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

import { parseNodeConfigToForm, type NodeConfigFormState } from '@/lib/trade-flow-config-mappers';
import { PTB_CURRENT_PRICE_SOURCE_OPTIONS } from '@/lib/trade-flow-config-mappers/ptb-modes';
import { NodeInspectorPanel } from './node-inspector-panel';
import type { NodeInspectorActions, NodeInspectorPanelProps } from './types';

const noop = () => {};

function createActions(): NodeInspectorActions {
  return {
    onNodeKeyChange: noop,
    onNodeTypeChange: noop,
    onTabChange: noop,
    onFormChange: noop,
    onUpdateField: noop,
    onUpdateTriggerSizeRow: noop,
    onCreateNode: noop,
    onUpdateNode: noop,
    onDeleteNode: noop,
    onCreateFromAdvanced: noop,
    onUpdateFromAdvanced: noop,
    onApplyOpenPosition: noop,
    onUpdateExpressionRow: noop,
    onAddExpressionRow: noop,
    onRemoveExpressionRow: noop,
    onUpdateStatePatchRow: noop,
    onAddStatePatchRow: noop,
    onRemoveStatePatchRow: noop,
    onAddOutcomeCondition: noop,
    onRemoveOutcomeCondition: noop,
    onUpdateOutcomeCondition: noop,
    onAddDrawdownRule: noop,
    onRemoveDrawdownRule: noop,
    onUpdateDrawdownRule: noop,
  };
}

function createPairLockForm(overrides: Record<string, unknown> = {}): NodeConfigFormState {
  return parseNodeConfigToForm('action.place_order', {
    kind: 'immediate',
    mode: 'pair_lock',
    side: 'buy',
    refKey: 'action_4z6ivq',
    sizeMode: 'usdc',
    sizeUsdc: 5,
    slEnabled: true,
    tpEnabled: true,
    presetKind: 'place_order',
    maxTriggers: 1,
    slPriceCent: 44,
    tpPriceCent: 99,
    maxPriceCent: 55,
    executionMode: 'market',
    pairSizingMode: 'auto_remaining_budget',
    pairTotalBudgetUsdc: 10,
    pairMaxTotalCent: 97,
    counterLegEnabled: true,
    counterLegOutcomeLabel: 'opposite',
    ...overrides,
  });
}

function renderInspector(form: NodeConfigFormState) {
  const props: NodeInspectorPanelProps = {
    form,
    nodeKeyDraft: 'action_4z6ivq',
    nodeTypeDraft: 'action.place_order',
    tab: 'basic',
    openPositions: [],
    openPositionsMeta: null,
    openPositionsLoading: false,
    openPositionApplyingKey: null,
    canApplyOpenPosition: () => false,
    marketOutcomes: [],
    marketOutcomesLoading: false,
    upstreamAutoScope: true,
    upstreamHasTriggerPrice: false,
    upstreamMaxPriceResolution: {
      kind: 'none',
      maxPriceCent: null,
      distinctMaxPriceCents: [],
    },
    upstreamPairLockTrigger: null,
    userTelegramBotTokenMasked: null,
    userTelegramDefaultChatId: null,
    actions: createActions(),
  };

  return renderToStaticMarkup(React.createElement(NodeInspectorPanel, props));
}

function countPrimaryPtbStopLossHeadings(html: string): number {
  return (html.match(/>PTB Gap Stop-Loss<\/label>/g) ?? []).length;
}

test('NodeInspectorPanel shows pair-lock PTB guard near top when PTB config is empty', () => {
  const html = renderInspector(createPairLockForm());
  const modeIndex = html.indexOf('Çalışma Modu');
  const executionModeIndex = html.indexOf('Islem Modu');
  const ptbIndex = html.indexOf('Price to Beat Korumasi');
  const ptbStopLossIndex = html.indexOf('PTB Gap Stop-Loss');
  const amountIndex = html.indexOf('Tutar (USDC)');

  assert.ok(modeIndex > -1);
  assert.ok(ptbIndex > -1);
  assert.ok(executionModeIndex > -1);
  assert.ok(ptbStopLossIndex > -1);
  assert.ok(amountIndex > -1);
  assert.ok(modeIndex < executionModeIndex);
  assert.ok(executionModeIndex < ptbIndex);
  assert.ok(ptbIndex < ptbStopLossIndex);
  assert.ok(ptbStopLossIndex < amountIndex);
  assert.equal(countPrimaryPtbStopLossHeadings(html), 1);
  assert.doesNotMatch(html, /PTB Current Kaynagi/);
});

test('NodeInspectorPanel shows dedicated place-order mode selector and live gap fields only in live mode', () => {
  const singleHtml = renderInspector(
    parseNodeConfigToForm('action.place_order', {
      mode: 'single',
      side: 'buy',
      executionMode: 'market',
      sizeMode: 'usdc',
      sizeUsdc: 5,
    })
  );
  const liveHtml = renderInspector(
    parseNodeConfigToForm('action.place_order', {
      mode: 'live_gap_collector_v1',
      side: 'buy',
      executionMode: 'market',
      sizeMode: 'usdc',
      sizeUsdc: 5,
      marketSlug: 'btc-updown-5m-1773319200',
      tokenId: 'tok-up',
      outcomeLabel: 'Up',
    })
  );

  assert.match(singleHtml, /Çalışma Modu/);
  assert.match(singleHtml, /Live Gap Collector/);
  assert.doesNotMatch(singleHtml, /Live Gap Start/);
  assert.doesNotMatch(singleHtml, /Live Gap Karar Bildirimi/);
  assert.match(liveHtml, /Çalışma Modu/);
  assert.match(liveHtml, /Live Gap Start/);
  assert.match(liveHtml, /Live Gap Karar Bildirimi/);
});

test('NodeInspectorPanel renders RevengeFlip mode fields', () => {
  const html = renderInspector(
    parseNodeConfigToForm('action.place_order', {
      mode: 'revenge_flip_v1',
      revengeFlip: {
        initialOrderUsdc: 5,
        profitTargetUsdc: 0.25,
        stopLossPct: 0.2,
        stopLossRules: [{ minFlip: 1, stopLossPct: 0.15 }],
        reentrySideMode: 'rule_match',
        entryPtbRules: [{ minFlip: 0, maxFlip: 0, sideMode: 'any', priceToBeatMaxDiff: 10, priceToBeatMaxDiffUnit: 'usd', maxPriceCent: 80 }],
        ptbStopLossEnabled: true,
        ptbStopLossGapUsd: -4,
        ptbStopLossGapUnit: 'cent',
      },
      triggerPrice: { enabled: true, minCent: 40, maxCent: 65 },
      priceToBeatCurrentPriceSource: 'chainlink',
    })
  );

  assert.match(html, /Initial USDC/);
  assert.match(html, /Profit USDC/);
  assert.match(html, /Classic Stop-Loss/);
  assert.match(html, /Stop Loss Rules/);
  assert.match(html, /Kural Ekle/);
  assert.match(html, /Entry PTB Rules/);
  assert.match(html, /PTB Source/);
  assert.deepEqual(
    PTB_CURRENT_PRICE_SOURCE_OPTIONS.map((option) => option.value),
    ['chainlink', 'binance', 'coinbase', 'hyperliquid', 'bybit'],
  );
  assert.match(html, /PTB Kurali Ekle/);
  assert.match(html, /Min PTB Diff/);
  assert.match(html, /PTB Unit/);
  assert.match(html, /value="10"/);
  assert.match(html, /value="80"/);
  assert.match(html, /Max Price/);
  assert.match(html, /Re-entry Mode/);
  assert.match(html, /PTB Gap Stop-Loss/);
  assert.match(html, /Side/);
  assert.match(html, /Time Rules JSON/);
});

test('NodeInspectorPanel shows PTB current source when pair-lock PTB guard is enabled', () => {
  const html = renderInspector(
    createPairLockForm({
      priceToBeatGuardEnabled: true,
      priceToBeatMode: 'manual',
      priceToBeatCurrentPriceSource: 'binance',
    })
  );

  assert.match(html, /Price to Beat Korumasi/);
  assert.match(html, /PTB Current Kaynagi/);
});

test('NodeInspectorPanel shows PTB current source when pair-lock PTB stop-loss is enabled', () => {
  const html = renderInspector(
    createPairLockForm({
      ptbStopLossEnabled: true,
      ptbStopLossGapUsd: 0,
      priceToBeatCurrentPriceSource: 'coinbase',
    })
  );

  assert.match(html, /PTB Gap Stop-Loss/);
  assert.match(html, /PTB Current Kaynagi/);
  assert.match(html, /PTB SL Current Kaynagi/);
  assert.match(html, /Gap Eşiği/);
  assert.match(html, /Satış Yüzdesi/);
  assert.match(html, /Entry PTB kaynagi ile ayni/i);
  assert.equal(countPrimaryPtbStopLossHeadings(html), 1);
});

test('NodeInspectorPanel renders selected PTB stop-loss current source override', () => {
  const html = renderInspector(
    createPairLockForm({
      ptbStopLossEnabled: true,
      ptbStopLossGapUsd: 0,
      priceToBeatCurrentPriceSource: 'binance',
      ptbStopLossCurrentPriceSource: 'coinbase',
    })
  );

  assert.match(html, /PTB SL Current Kaynagi/);
  assert.match(html, /Coinbase/);
});

test('NodeInspectorPanel hides PTB guard section for sell place-order nodes', () => {
  const html = renderInspector(
    parseNodeConfigToForm('action.place_order', {
      mode: 'single',
      side: 'sell',
      executionMode: 'market',
      presetKind: 'place_order',
    })
  );

  assert.doesNotMatch(html, /Price to Beat Korumasi/);
  assert.doesNotMatch(html, /PTB Current Kaynagi/);
});

test('NodeInspectorPanel keeps SL and PTB stop-loss visible for positive grid while hiding classic TP', () => {
  const html = renderInspector(
    parseNodeConfigToForm('action.place_order', {
      mode: 'positive_quantity_flip_grid_v1',
      side: 'buy',
      executionMode: 'market',
      sizeMode: 'usdc',
      sizeUsdc: 1,
      slEnabled: true,
      slPriceCent: 42,
      ptbStopLossEnabled: true,
      ptbStopLossGapUsd: 0,
      positiveQuantityFlipGrid: {
        ptbCurrentPriceSource: 'binance',
      },
    })
  );

  assert.match(html, /Stop Loss/);
  assert.match(html, /SL Fiyat/);
  assert.match(html, /PTB Gap Stop-Loss/);
  assert.match(html, /Entry PTB kaynagi ile ayni/i);
  assert.match(html, /Binance/);
  assert.doesNotMatch(html, />Take Profit<\/label>/);
  assert.doesNotMatch(html, /TP Fiyat/);
  assert.doesNotMatch(html, /Price to Beat Korumasi/);
});

test('NodeInspectorPanel labels positive grid entry PTB guard by normal and rescue buy scope', () => {
  const guardOnHtml = renderInspector(
    parseNodeConfigToForm('action.place_order', {
      mode: 'positive_quantity_flip_grid_v1',
      side: 'buy',
      executionMode: 'market',
      sizeMode: 'usdc',
      sizeUsdc: 1,
      positiveQuantityFlipGrid: {
        rescueBuyEnabled: true,
        ptbGuardEnabled: true,
        ptbMinDiff: 80,
        ptbRescueMinDiff: 40,
        ptbDiffUnit: 'usd',
        ptbCurrentPriceSource: 'binance',
      },
    })
  );

  assert.match(guardOnHtml, /Entry PTB Min Diff \(Normal Buy\)/);
  assert.match(guardOnHtml, /Entry PTB Min Diff \(Rescue Buy\)/);
  assert.match(
    guardOnHtml,
    /Rescue entry PTB bos birakilirsa normal entry PTB kullanilir/i,
  );

  const guardOffHtml = renderInspector(
    parseNodeConfigToForm('action.place_order', {
      mode: 'positive_quantity_flip_grid_v1',
      side: 'buy',
      executionMode: 'market',
      sizeMode: 'usdc',
      sizeUsdc: 1,
      positiveQuantityFlipGrid: {
        rescueBuyEnabled: true,
        ptbGuardEnabled: false,
        ptbMinDiff: 80,
        ptbRescueMinDiff: 40,
      },
    })
  );

  assert.match(guardOffHtml, /Entry PTB Min Diff \(Normal Buy\)/);
  assert.doesNotMatch(guardOffHtml, /Entry PTB Min Diff \(Rescue Buy\)/);
});
