import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildNodeConfigFromForm,
  parseNodeConfigToForm,
} from '@/lib/trade-flow-config-mappers';

test('trigger.market_price dca_live_only binding round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('trigger.market_price', {
    marketMode: 'auto_scope',
    marketScope: 'btc_5m_updown',
    marketSelection: 'latest_by_slug',
    repeatMode: 'once',
    bindingMode: 'dca_live_only',
  });

  assert.equal(form.fields.bindingMode, 'dca_live_only');

  const rebuilt = buildNodeConfigFromForm('trigger.market_price', form);
  assert.equal(rebuilt.bindingMode, 'dca_live_only');
});

test('trigger.market_price dca_live_only strips outcome and PTB trigger config on rebuild', () => {
  const form = parseNodeConfigToForm('trigger.market_price', {
    marketMode: 'auto_scope',
    marketScope: 'btc_5m_updown',
    marketSelection: 'latest_by_slug',
    repeatMode: 'once',
    bindingMode: 'dca_live_only',
    priceToBeatTriggerEnabled: true,
    priceToBeatMode: 'manual',
    priceToBeatTriggerMinGap: 10,
    outcomeConditions: [
      {
        tokenId: 'btc-up-token',
        outcomeLabel: 'Up',
        triggerCondition: 'level_above',
        triggerPriceCent: 70,
      },
    ],
  });

  const rebuilt = buildNodeConfigFromForm('trigger.market_price', form);
  assert.equal(rebuilt.bindingMode, 'dca_live_only');
  assert.equal('outcomeConditions' in rebuilt, false);
  assert.equal('priceToBeatTriggerEnabled' in rebuilt, false);
  assert.equal('priceToBeatMode' in rebuilt, false);
  assert.equal('priceToBeatTriggerMinGap' in rebuilt, false);
});
