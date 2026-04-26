import assert from 'node:assert/strict';
import test from 'node:test';

import { buildNodeConfigFromForm, parseNodeConfigToForm } from './node-config';

test('action.place_order edge pair_lock strategy round-trips share qty config', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: 'pair_lock',
    pairLockStrategy: 'edge_pairlock_v1',
    pairSizingMode: 'manual',
    side: 'buy',
    executionMode: 'limit',
    sizeMode: 'usdc',
    sizeUsdc: 5,
    marketSlug: 'btc-updown-5m-1774013100',
    pairMaxTotalCent: 95,
    pairLockDecisionQty: 5,
    pairLockSingleEdgeThreshold: 0.10,
    pairLockCostBuffer: 0.005,
    counterLegEnabled: true,
    counterLegOutcomeLabel: 'opposite',
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'iv_mismatch_edge',
    counterLegSizeUsdc: 5,
  });

  assert.equal(form.fields.pairLockStrategy, 'edge_pairlock_v1');
  assert.equal(form.fields.pairLockDecisionQty, '5');
  assert.equal(form.fields.pairLockSingleEdgeThreshold, '0.1');
  assert.equal(form.fields.pairLockCostBuffer, '0.005');
  assert.equal(form.fields.priceToBeatGuardEnabled, 'true');
  assert.equal(form.fields.priceToBeatMode, 'iv_mismatch_edge');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.pairLockStrategy, 'edge_pairlock_v1');
  assert.equal(rebuilt.pairLockDecisionQty, 5);
  assert.equal(rebuilt.pairLockSingleEdgeThreshold, 0.1);
  assert.equal(rebuilt.pairLockCostBuffer, 0.005);
  assert.equal(rebuilt.priceToBeatGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatMode, 'iv_mismatch_edge');
  assert.equal(rebuilt.pairSizingMode, 'manual');
  assert.equal('counterLegSizeUsdc' in rebuilt, false);
});
