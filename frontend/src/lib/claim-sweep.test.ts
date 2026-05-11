import assert from 'node:assert/strict';
import test from 'node:test';

import { __claimSweepTestUtils } from './claim-sweep';

test('claim sweep value falls back to redeemable size when Data API price fields are zero', () => {
  const value = __claimSweepTestUtils.resolveCurrentValue({
    currentValue: 0,
    curPrice: 0,
    size: 249.9963,
    balance: null,
  });

  assert.equal(value, 249.9963);
});

test('claim sweep value falls back to balance when size is unavailable', () => {
  const value = __claimSweepTestUtils.resolveCurrentValue({
    currentValue: '0',
    curPrice: '0',
    size: null,
    balance: '3.125',
  });

  assert.equal(value, 3.125);
});

test('claim sweep value rejects zero token amounts', () => {
  const value = __claimSweepTestUtils.resolveCurrentValue({
    currentValue: 0,
    curPrice: 0,
    size: 0,
    balance: 0,
  });

  assert.equal(value, 0);
});
