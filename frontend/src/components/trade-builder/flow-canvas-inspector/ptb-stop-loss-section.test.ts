import assert from 'node:assert/strict';
import test from 'node:test';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

import { PtbStopLossRuleSection } from './exit-sections';
import { PtbStopLossSection } from './ptb-stop-loss-section';

test('PtbStopLossSection explains that PTB stop-loss uses directional gap instead of counter token price', () => {
  const html = renderToStaticMarkup(
    React.createElement(PtbStopLossSection, {
      enabled: true,
      unit: 'usd',
      timeDecayMode: 'tighten',
      rows: [{ id: 'ptb-1', gapUsd: '-10', sizePct: '100' }],
      onUpdateField: () => {},
      onUpdateRows: () => {},
    })
  );

  assert.match(html, /karsi token fiyati degil/i);
  assert.match(html, /Up\/Yes icin PTB referansinin 10 altini/i);
  assert.match(html, /Down\/No icin PTB referansinin 10 ustunu bekler/i);
  assert.match(html, /Negatif esik, karsi yone overshoot bekler/i);
});

test('PtbStopLossRuleSection explains staged negative rows with directional gap semantics', () => {
  const html = renderToStaticMarkup(
    React.createElement(PtbStopLossRuleSection, {
      unit: 'usd',
      rows: [{ id: 'ptb-1', gapUsd: '-10', sizePct: '100' }],
      onAdd: () => {},
      onUpdate: () => {},
      onRemove: () => {},
    })
  );

  assert.match(html, /karsi token fiyati degil, directional gap esigidir/i);
  assert.match(html, /Up\/Yes icin current &lt;= PTB - 10/i);
  assert.match(html, /Down\/No icin current &gt;= PTB \+ 10/i);
});
