import assert from 'node:assert/strict';
import test from 'node:test';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

import { PairLockSummarySection } from './pair-lock-binding-section';

test('PairLockSummarySection explains custom range comes from trigger node', () => {
  const html = renderToStaticMarkup(
    React.createElement(PairLockSummarySection, {
      visible: true,
      primaryOutcomeLabel: 'Up',
      counterOutcomePreview: 'Down',
      upstreamPairLockTrigger: {
        nodeKey: 'trigger_market',
        bindingMode: 'pair_lock_only',
        marketMode: 'auto_scope',
        marketSource: 'btc_5m_updown',
        cycleWindowMode: 'custom_range',
        cycleWindowSecs: '',
        cycleWindowStartSec: '230',
        cycleWindowEndSec: '290',
      },
    })
  );

  assert.match(html, /cycleWindow/);
  assert.match(html, /230-290s/);
  assert.match(html, /Pair lock icin ozel aralik varsa/);
  assert.match(html, /trigger\.market_price/);
  assert.match(html, /ayri yonetilebilir/);
  assert.match(html, /standalone devam eder/);
  assert.match(html, /Retry acik guard/);
});
