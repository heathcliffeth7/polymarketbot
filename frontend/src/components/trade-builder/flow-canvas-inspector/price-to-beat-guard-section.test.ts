import assert from 'node:assert/strict';
import test from 'node:test';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

import { PriceToBeatGuardSection } from './price-to-beat-guard-section';

function renderSection(overrides: {
  checked?: boolean;
  currentSourceVisible?: boolean;
  fields?: Record<string, string>;
} = {}) {
  return renderToStaticMarkup(
    React.createElement(PriceToBeatGuardSection, {
      checked: overrides.checked ?? false,
      retryChecked: false,
      mode: 'manual',
      unit: 'usd',
      currentSource: 'chainlink',
      currentSourceVisible: overrides.currentSourceVisible ?? false,
      fields: overrides.fields ?? {},
      stopLossBumpUi: {
        checked: false,
        mode: 'fixed',
        scope: 'global',
        unit: 'usd',
      },
      stopLossBumpLossRuleRows: [],
      ivTimeRuleRows: [],
      maxPriceRelaxMinUnit: 'usd',
      maxPriceRelaxStepMode: 'absolute',
      maxPriceRelaxStepUnit: 'usd',
      onUpdateField: () => {},
      onUpdateStopLossBumpLossRuleRows: () => {},
      onUpdateIvTimeRuleRows: () => {},
    })
  );
}

test('PriceToBeatGuardSection shows current source when PTB guard is enabled', () => {
  const html = renderSection({ checked: true });

  assert.match(html, /PTB Current Kaynagi/);
  assert.match(html, /Market BTC ise/);
  assert.match(html, /Binance BTCUSDT/);
  assert.match(html, /Coinbase BTC-USD/);
});

test('PriceToBeatGuardSection shows only current source when PTB stop-loss needs it', () => {
  const html = renderSection({ checked: false, currentSourceVisible: true });

  assert.match(html, /PTB Current Kaynagi/);
  assert.doesNotMatch(html, /PTB Modu/);
});

test('PriceToBeatGuardSection hides current source when PTB guard and stop-loss are inactive', () => {
  const html = renderSection({ checked: false, currentSourceVisible: false });

  assert.match(html, /Price to Beat Korumasi/);
  assert.doesNotMatch(html, /PTB Current Kaynagi/);
});
