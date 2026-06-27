import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildNodeConfigFromForm,
  parseNodeConfigToForm,
} from '@/lib/trade-flow-config-mappers';

test('action.place_order PTB IV wait reprice guard round-trips through mapper form state', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 5,
    marketSlug: 'btc-updown-5m-1780762500',
    tokenId: 'btc-up-token',
    outcomeLabel: 'Up',
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'iv_mismatch_edge',
    priceToBeatIvWaitRepriceGuardEnabled: true,
    priceToBeatIvWaitMaxAgeMsEarly: 8000,
    priceToBeatIvWaitMaxAgeMsMid: 5000,
    priceToBeatIvWaitMaxAgeMsLate: 3000,
    priceToBeatIvWaitInitialAskMaxOverCapCent: 10,
    priceToBeatIvFallingIntoCapGuardEnabled: true,
    priceToBeatIvFallingIntoCapDropCentEarly: 15,
    priceToBeatIvFallingIntoCapDropCentMid: 12,
    priceToBeatIvFallingIntoCapDropCentLate: 8,
    priceToBeatIvLateExpensiveEntryGuardEnabled: true,
    priceToBeatIvLateExpensiveSeconds: 45,
    priceToBeatIvLateExpensiveVwapCent: 70,
    priceToBeatIvLateExpensiveMinQCent: 92,
    priceToBeatIvLateExpensiveMinGapStrengthExtra: 0.5,
  });

  assert.equal(form.fields.priceToBeatIvWaitRepriceGuardEnabled, 'true');
  assert.equal(form.fields.priceToBeatIvWaitMaxAgeMsEarly, '8000');
  assert.equal(form.fields.priceToBeatIvFallingIntoCapDropCentLate, '8');
  assert.equal(form.fields.priceToBeatIvLateExpensiveMinGapStrengthExtra, '0.5');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.priceToBeatIvWaitRepriceGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatIvWaitMaxAgeMsEarly, 8000);
  assert.equal(rebuilt.priceToBeatIvWaitMaxAgeMsMid, 5000);
  assert.equal(rebuilt.priceToBeatIvWaitMaxAgeMsLate, 3000);
  assert.equal(rebuilt.priceToBeatIvWaitInitialAskMaxOverCapCent, 10);
  assert.equal(rebuilt.priceToBeatIvFallingIntoCapGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatIvFallingIntoCapDropCentEarly, 15);
  assert.equal(rebuilt.priceToBeatIvFallingIntoCapDropCentMid, 12);
  assert.equal(rebuilt.priceToBeatIvFallingIntoCapDropCentLate, 8);
  assert.equal(rebuilt.priceToBeatIvLateExpensiveEntryGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatIvLateExpensiveSeconds, 45);
  assert.equal(rebuilt.priceToBeatIvLateExpensiveVwapCent, 70);
  assert.equal(rebuilt.priceToBeatIvLateExpensiveMinQCent, 92);
  assert.equal(rebuilt.priceToBeatIvLateExpensiveMinGapStrengthExtra, 0.5);
});
