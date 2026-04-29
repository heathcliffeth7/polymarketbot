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

test('action.place_order adaptive max price strategy round-trips guarded config', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: 'pair_lock',
    pairLockStrategy: 'adaptive_max_price_v1',
    pairSizingMode: 'manual',
    side: 'buy',
    executionMode: 'limit',
    sizeMode: 'usdc',
    sizeUsdc: 5,
    maxPriceCent: 70,
    pairMaxTotalCent: 96,
    counterLegEnabled: true,
    counterLegOutcomeLabel: 'opposite',
    counterLegSizeUsdc: 5,
    priceToBeatGuardEnabled: false,
    priceToBeatMode: 'manual',
    adaptiveMaxPriceMissCount: 3,
    adaptiveMaxPriceRequiredGoodMissCount: 2,
    adaptiveMaxPriceRelaxCreditCent: 2,
    adaptiveMaxPriceMaxRelaxCreditCent: 5,
    adaptiveMaxPriceHardCapCent: 76,
    adaptiveMaxPriceExtraBufferCent: 1,
    adaptiveMaxPricePairBufferCent: 1,
    adaptiveMaxPriceSizeMultiplier: 0.5,
    adaptiveMaxPriceWindowStartSec: 120,
    adaptiveMaxPriceWindowEndSec: 290,
    adaptiveMaxPriceLateRiskEnabled: true,
    adaptiveMaxPriceLateRiskAfterSec: 210,
    adaptiveMaxPriceLateExtraBufferCent: 1,
    adaptiveMaxPriceLateSizeMultiplier: 0.35,
    adaptiveMaxPriceSlCooldownMarkets: 3,
    pairLockDecisionQty: 9,
  });

  assert.equal(form.fields.pairLockStrategy, 'adaptive_max_price_v1');
  assert.equal(form.fields.priceToBeatGuardEnabled, 'true');
  assert.equal(form.fields.priceToBeatMode, 'iv_mismatch_edge');
  assert.equal(form.fields.adaptiveMaxPriceHardCapCent, '76');
  assert.equal(form.fields.adaptiveMaxPriceSizeMultiplier, '0.5');
  assert.equal(form.fields.adaptiveMaxPriceWindowStartSec, '120');
  assert.equal(form.fields.adaptiveMaxPriceWindowEndSec, '290');
  assert.equal(form.fields.adaptiveMaxPriceLateRiskEnabled, 'true');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.pairLockStrategy, 'adaptive_max_price_v1');
  assert.equal(rebuilt.priceToBeatGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatMode, 'iv_mismatch_edge');
  assert.equal(rebuilt.adaptiveMaxPriceMissCount, 3);
  assert.equal(rebuilt.adaptiveMaxPriceRequiredGoodMissCount, 2);
  assert.equal(rebuilt.adaptiveMaxPriceRelaxCreditCent, 2);
  assert.equal(rebuilt.adaptiveMaxPriceMaxRelaxCreditCent, 5);
  assert.equal(rebuilt.adaptiveMaxPriceHardCapCent, 76);
  assert.equal(rebuilt.adaptiveMaxPriceExtraBufferCent, 1);
  assert.equal(rebuilt.adaptiveMaxPricePairBufferCent, 1);
  assert.equal(rebuilt.adaptiveMaxPriceSizeMultiplier, 0.5);
  assert.equal(rebuilt.adaptiveMaxPriceWindowStartSec, 120);
  assert.equal(rebuilt.adaptiveMaxPriceWindowEndSec, 290);
  assert.equal(rebuilt.adaptiveMaxPriceLateRiskEnabled, true);
  assert.equal(rebuilt.adaptiveMaxPriceLateRiskAfterSec, 210);
  assert.equal(rebuilt.adaptiveMaxPriceLateExtraBufferCent, 1);
  assert.equal(rebuilt.adaptiveMaxPriceLateSizeMultiplier, 0.35);
  assert.equal(rebuilt.adaptiveMaxPriceSlCooldownMarkets, 3);
  assert.equal('pairLockDecisionQty' in rebuilt, false);
  assert.equal('adaptiveMaxPriceLateRelaxCutoffS' in rebuilt, false);
});

test('action.place_order adaptive max price strategy leaves blank window fields optional', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: 'pair_lock',
    pairLockStrategy: 'adaptive_max_price_v1',
    side: 'buy',
    executionMode: 'limit',
    sizeMode: 'usdc',
    sizeUsdc: 5,
    maxPriceCent: 70,
    pairMaxTotalCent: 96,
    counterLegEnabled: true,
    counterLegOutcomeLabel: 'opposite',
    counterLegSizeUsdc: 5,
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'iv_mismatch_edge',
    adaptiveMaxPriceWindowStartSec: '',
    adaptiveMaxPriceWindowEndSec: '',
  });

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal('adaptiveMaxPriceWindowStartSec' in rebuilt, false);
  assert.equal('adaptiveMaxPriceWindowEndSec' in rebuilt, false);
  assert.equal(rebuilt.adaptiveMaxPriceLateRiskEnabled, true);
  assert.equal(rebuilt.adaptiveMaxPriceLateRiskAfterSec, 210);
});
