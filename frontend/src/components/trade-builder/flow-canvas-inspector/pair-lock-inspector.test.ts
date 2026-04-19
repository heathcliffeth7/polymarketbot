import assert from 'node:assert/strict';
import test from 'node:test';

import {
  estimatePairLockAutoRemainingBudgetPreview,
  resolvePairLockStopLossFieldVisibility,
  resolvePairLockSizingFieldVisibility,
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

test('resolvePairLockStopLossFieldVisibility allows supported lead-leg stop-loss fields', () => {
  const fields = {
    slEnabled: 'true',
    ptbStopLossEnabled: 'true',
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
    resolvePairLockStopLossFieldVisibility('reentryCooldownSec', true, fields),
    true
  );
  assert.equal(
    resolvePairLockStopLossFieldVisibility('reentryMinPriceCent', true, fields),
    null
  );
});
