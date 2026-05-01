import assert from 'node:assert/strict';
import test from 'node:test';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

import { TriggerMarketFiringModeSection } from './trigger-market-firing-mode-section';

function renderSection(args: {
  fields?: Record<string, string>;
  triggerCondition?: string;
} = {}) {
  return renderToStaticMarkup(
    React.createElement(TriggerMarketFiringModeSection, {
      fields: args.fields ?? { repeatMode: 'once', onceScope: 'market' },
      outcomeConditionRows: [
        {
          id: 'oc_1',
          tokenId: 'token',
          outcomeLabel: 'Up',
          triggerCondition: args.triggerCondition ?? 'level_above',
          triggerPriceCent: '71',
          maxPriceCent: '',
        },
      ],
      onUpdateField: () => {},
    })
  );
}

test('TriggerMarketFiringModeSection shows market-once mode and level warning', () => {
  const html = renderSection();

  assert.match(html, /Tetik Calisma Modu/);
  assert.match(html, /Her markette bir kez/);
  assert.match(html, /Run boyunca bir kez/);
  assert.match(html, /Dongu/);
  assert.match(html, /level_above\/level_below/);
  assert.match(html, /repeatMode=once/);
  assert.match(html, /onceScope=market/);
});

test('TriggerMarketFiringModeSection allows loop copy when no level trigger is active', () => {
  const html = renderSection({
    fields: { repeatMode: 'loop', onceScope: '' },
    triggerCondition: 'cross_above',
  });

  assert.match(html, /Dongu/);
  assert.doesNotMatch(html, /dongu modunda publish edilemez/);
  assert.doesNotMatch(html, /repeatMode=once/);
});
