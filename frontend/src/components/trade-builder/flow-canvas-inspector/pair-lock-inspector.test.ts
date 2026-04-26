import assert from 'node:assert/strict';
import test from 'node:test';

import {
  estimatePairLockAutoRemainingBudgetPreview,
  isPairLockField,
  resolvePairLockStopLossFieldVisibility,
  resolvePairLockSizingFieldVisibility,
  resolvePairLockTakeProfitFieldVisibility,
} from './pair-lock-inspector';

test('estimatePairLockAutoRemainingBudgetPreview computes remaining budget and profit', () => {
  const preview = estimatePairLockAutoRemainingBudgetPreview(
    {
      pairSizingMode: 'auto_remaining_budget',
      sizeUsdc: '5',
      pairTotalBudgetUsdc: '14',
      pairMaxTotalCent: '100',
      tokenId: 'tok-up',
      outcomeLabel: 'Up',
      counterLegOutcomeLabel: 'opposite',
    },
    [
      { token_id: 'tok-up', label: 'Up', price: 0.7, legSide: 'yes', feeRateBps: 0 },
      { token_id: 'tok-down', label: 'Down', price: 0.3, legSide: 'no', feeRateBps: 0 },
    ]
  );

  assert.ok(preview);
  assert.equal(preview?.blockedReason, null);
  assert.equal(preview?.remainingBudgetUsdc, 9);
  assert.ok((preview?.commonQty ?? 0) > 7);
  assert.ok((preview?.projectedNetProfitUsdc ?? 0) >= 0);

  const secondPreview = estimatePairLockAutoRemainingBudgetPreview(
    {
      pairSizingMode: 'auto_remaining_budget',
      sizeUsdc: '5',
      pairTotalBudgetUsdc: '14',
      pairMaxTotalCent: '95',
      tokenId: 'tok-yes',
      outcomeLabel: 'Yes',
      counterLegOutcomeLabel: 'opposite',
    },
    [
      { token_id: 'tok-yes', label: 'Yes', price: 0.53, legSide: 'yes', feeRateBps: 0 },
      { token_id: 'tok-no', label: 'No', price: 0.37, legSide: 'no', feeRateBps: 0 },
    ]
  );

  assert.equal(secondPreview?.blockedReason, null);
  assert.ok((secondPreview?.projectedNetProfitUsdc ?? 0) > 0);
});

test('estimatePairLockAutoRemainingBudgetPreview blocks over-max-total pairs', () => {
  const preview = estimatePairLockAutoRemainingBudgetPreview(
    {
      pairSizingMode: 'auto_remaining_budget',
      sizeUsdc: '5',
      pairTotalBudgetUsdc: '14',
      pairMaxTotalCent: '80',
      tokenId: 'tok-up',
      outcomeLabel: 'Up',
      counterLegOutcomeLabel: 'opposite',
    },
    [
      { token_id: 'tok-up', label: 'Up', price: 0.42, legSide: 'yes', feeRateBps: 0 },
      { token_id: 'tok-down', label: 'Down', price: 0.48, legSide: 'no', feeRateBps: 0 },
    ]
  );

  assert.equal(preview?.blockedReason, 'above_max_total');
});

test('resolvePairLockSizingFieldVisibility toggles manual and auto fields', () => {
  assert.equal(
    resolvePairLockSizingFieldVisibility('counterLegSizeUsdc', true, { pairSizingMode: 'manual' }),
    true
  );
  assert.equal(
    resolvePairLockSizingFieldVisibility('counterLegSizeUsdc', true, {
      pairSizingMode: 'auto_remaining_budget',
    }),
    false
  );
  assert.equal(
    resolvePairLockSizingFieldVisibility('pairTotalBudgetUsdc', true, {
      pairSizingMode: 'auto_remaining_budget',
    }),
    true
  );
});

test('resolvePairLockSizingFieldVisibility shows edge strategy qty fields', () => {
  const fields = { pairLockStrategy: 'edge_pairlock_v1', pairSizingMode: 'manual' };
  assert.equal(resolvePairLockSizingFieldVisibility('pairLockStrategy', true, fields), true);
  assert.equal(resolvePairLockSizingFieldVisibility('pairLockDecisionQty', true, fields), true);
  assert.equal(
    resolvePairLockSizingFieldVisibility('pairLockSingleEdgeThreshold', true, fields),
    true
  );
  assert.equal(resolvePairLockSizingFieldVisibility('pairLockCostBuffer', true, fields), true);
  assert.equal(resolvePairLockSizingFieldVisibility('counterLegSizeUsdc', true, fields), false);
});

test('isPairLockField includes locked pair stop-loss ignore toggle', () => {
  assert.equal(isPairLockField('pairIgnoreStopLossAfterLocked'), true);
  assert.equal(isPairLockField('pairLockStrategy'), true);
});

test('resolvePairLockStopLossFieldVisibility allows supported lead-leg stop-loss fields', () => {
  const fields = {
    counterLegEnabled: 'true',
    slEnabled: 'true',
    ptbStopLossEnabled: 'true',
    counterLegSlEnabled: 'true',
    counterLegPtbStopLossEnabled: 'true',
    reenterOnSlHit: 'true',
  };

  assert.equal(
    resolvePairLockStopLossFieldVisibility('slEnabled', true, fields),
    true
  );
  assert.equal(
    resolvePairLockStopLossFieldVisibility('slPriceCent', true, fields),
    true
  );
  assert.equal(
    resolvePairLockStopLossFieldVisibility('ptbStopLossGapUsd', true, fields),
    true
  );
  assert.equal(
    resolvePairLockStopLossFieldVisibility('ptbStopLossGapUnit', true, fields),
    true
  );
  assert.equal(
    resolvePairLockStopLossFieldVisibility('reentryCooldownSec', true, fields),
    true
  );
  assert.equal(
    resolvePairLockStopLossFieldVisibility('counterLegSlPriceCent', true, fields),
    true
  );
  assert.equal(
    resolvePairLockStopLossFieldVisibility('counterLegPtbStopLossGapUsd', true, fields),
    true
  );
  assert.equal(
    resolvePairLockStopLossFieldVisibility('counterLegPtbStopLossGapUnit', true, fields),
    true
  );
  assert.equal(
    resolvePairLockStopLossFieldVisibility('counterLegNotifyOnSlHit', true, fields),
    true
  );
  assert.equal(
    resolvePairLockStopLossFieldVisibility('reentryMinPriceCent', true, fields),
    null
  );
});

test('resolvePairLockTakeProfitFieldVisibility gates counter TP fields behind counter leg and TP toggle', () => {
  assert.equal(
    resolvePairLockTakeProfitFieldVisibility('counterLegTpEnabled', true, {
      counterLegEnabled: 'true',
    }),
    true
  );
  assert.equal(
    resolvePairLockTakeProfitFieldVisibility('counterLegTpPriceCent', true, {
      counterLegEnabled: 'true',
      counterLegTpEnabled: 'false',
    }),
    false
  );
  assert.equal(
    resolvePairLockTakeProfitFieldVisibility('counterLegTpPriceCent', true, {
      counterLegEnabled: 'true',
      counterLegTpEnabled: 'true',
    }),
    true
  );
  assert.equal(
    resolvePairLockTakeProfitFieldVisibility('counterLegNotifyOnTpHit', false, {
      counterLegEnabled: 'true',
      counterLegTpEnabled: 'true',
    }),
    false
  );
});
