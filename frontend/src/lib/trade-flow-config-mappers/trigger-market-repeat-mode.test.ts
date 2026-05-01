import assert from 'node:assert/strict';
import test from 'node:test';

import { buildNodeConfigFromForm, parseNodeConfigToForm } from './node-config';

test('trigger.market_price level trigger drafts parse back as once per market', () => {
  const form = parseNodeConfigToForm('trigger.market_price', {
    marketMode: 'auto_scope',
    marketScope: 'eth_5m_updown',
    repeatMode: 'loop',
    outcomeConditions: [
      {
        tokenId: 'token',
        outcomeLabel: 'Up',
        triggerCondition: 'level_above',
        triggerPriceCent: 71,
      },
    ],
  });

  assert.equal(form.fields.repeatMode, 'once');
  assert.equal(form.fields.onceScope, 'market');

  const rebuilt = buildNodeConfigFromForm('trigger.market_price', form);
  assert.equal(rebuilt.repeatMode, 'once');
  assert.equal(rebuilt.onceScope, 'market');
  assert.equal(rebuilt.onceScopeVersion, 2);
});

test('trigger.market_price cross trigger drafts keep loop mode', () => {
  const form = parseNodeConfigToForm('trigger.market_price', {
    marketMode: 'auto_scope',
    marketScope: 'eth_5m_updown',
    repeatMode: 'loop',
    outcomeConditions: [
      {
        tokenId: 'token',
        outcomeLabel: 'Up',
        triggerCondition: 'cross_above',
        triggerPriceCent: 71,
      },
    ],
  });

  assert.equal(form.fields.repeatMode, 'loop');

  const rebuilt = buildNodeConfigFromForm('trigger.market_price', form);
  assert.equal(rebuilt.repeatMode, 'loop');
  assert.equal('onceScope' in rebuilt, false);
  assert.equal('onceScopeVersion' in rebuilt, false);
});
