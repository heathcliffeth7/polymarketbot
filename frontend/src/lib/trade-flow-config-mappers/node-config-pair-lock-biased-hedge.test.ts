import assert from 'node:assert/strict';
import test from 'node:test';

import { buildNodeConfigFromForm, parseNodeConfigToForm } from './node-config';

test('action.place_order biased_hedge_v1 round-trips nested config', () => {
  const form = parseNodeConfigToForm('action.place_order', {
    mode: 'pair_lock',
    pairLockStrategy: 'biased_hedge_v1',
    side: 'buy',
    executionMode: 'market',
    sizeMode: 'usdc',
    sizeUsdc: 2,
    maxPriceCent: 75,
    counterLegEnabled: true,
    pairProtectiveUnwindEnabled: true,
    pairOrphanGraceMs: 1500,
    priceToBeatGuardEnabled: true,
    priceToBeatMode: 'iv_mismatch_edge',
    biasedHedgeMaxPairedEffectiveCostCent: 95,
    biasedHedge: {
      primaryBudgetUsdc: 2,
      hedgeBudgetUsdc: 0.5,
      minDominantShare: 0.75,
      maxHedgeSpendRatio: 0.25,
      primaryMinEdge: 0.08,
      primaryMinFinalQ: 0.72,
      highPriceCent: 70,
      highPriceMinFinalQ: 0.82,
      highPriceMinEdge: 0.10,
      hedgeOnlyIfPrimaryFilled: true,
      hedgeMinPriceCent: 3,
      hedgeMaxPriceCent: 25,
      disableNewPrimaryAfterSec: 180,
      disableAnyBuyAfterSec: 240,
      maxSideSwitchCount: 0,
    },
    biasedHedgeStop: {
      biasInvalidationEnabled: true,
      minQFinalToHold: 0.55,
      minEdgeToHold: 0,
      exitPctOnInvalidation: 100,
      ptbStopLossEnabled: true,
      ptbStopLossGapUsd: -3,
      ptbStopLossTimeDecayMode: 'tighten',
      timeExitRules: [
        { elapsedSec: 90, remainingPct: 60 },
        { elapsedSec: 150, remainingPct: 0 },
      ],
    },
  });

  assert.equal(form.fields.pairLockStrategy, 'biased_hedge_v1');
  assert.equal(form.fields.biasedHedgePrimaryBudgetUsdc, '2');
  assert.equal(form.fields.biasedHedgeStopTimeExitRulesJson, '[{"elapsedSec":90,"remainingPct":60},{"elapsedSec":150,"remainingPct":0}]');

  const rebuilt = buildNodeConfigFromForm('action.place_order', form);
  assert.equal(rebuilt.pairLockStrategy, 'biased_hedge_v1');
  assert.equal(rebuilt.priceToBeatGuardEnabled, true);
  assert.equal(rebuilt.priceToBeatMode, 'iv_mismatch_edge');
  assert.equal(rebuilt.tpEnabled, false);
  assert.equal(rebuilt.biasedHedge.primaryBudgetUsdc, 2);
  assert.equal(rebuilt.biasedHedge.hedgeBudgetUsdc, 0.5);
  assert.equal(rebuilt.biasedHedge.hedgeOnlyIfPrimaryFilled, true);
  assert.equal(rebuilt.biasedHedge.maxSideSwitchCount, 0);
  assert.equal(rebuilt.biasedHedgeStop.biasInvalidationEnabled, true);
  assert.deepEqual(rebuilt.biasedHedgeStop.timeExitRules, [
    { elapsedSec: 90, remainingPct: 60 },
    { elapsedSec: 150, remainingPct: 0 },
  ]);
  assert.equal('counterLegSizeUsdc' in rebuilt, false);
  assert.equal('pairMaxTotalCent' in rebuilt, false);
});
