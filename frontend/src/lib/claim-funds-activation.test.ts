import assert from 'node:assert/strict';
import test from 'node:test';

import { buildClaimFundsActivationTransactions } from './claim-relayer';
import { formatClaimErrorForDisplay } from './claim-error-format';

const SAFE = '0xb98776BAbF48478304b8959556d7205A9Ff105b6';
const USDCE = '0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174';
const ONRAMP = '0x93070a847efEf7F70739046A929D47a521F5B8ee';

test('funds activation builds approve and wrap when allowance is low', () => {
  const plan = buildClaimFundsActivationTransactions({
    safeAddress: SAFE,
    usdceTokenAddress: USDCE,
    collateralOnrampAddress: ONRAMP,
    usdcEBalanceRaw: 2_000_000n,
    allowanceRaw: 0n,
    minUsdc: 0.01,
  });

  assert.equal(plan.status, 'ready');
  assert.equal(plan.needsApproval, true);
  assert.equal(plan.transactions.length, 2);
  assert.equal(plan.transactions[0].to, USDCE);
  assert.equal(plan.transactions[1].to, ONRAMP);
});

test('funds activation skips approve when allowance covers balance', () => {
  const plan = buildClaimFundsActivationTransactions({
    safeAddress: SAFE,
    usdceTokenAddress: USDCE,
    collateralOnrampAddress: ONRAMP,
    usdcEBalanceRaw: 2_000_000n,
    allowanceRaw: 2_000_000n,
    minUsdc: 0.01,
  });

  assert.equal(plan.status, 'ready');
  assert.equal(plan.needsApproval, false);
  assert.equal(plan.transactions.length, 1);
  assert.equal(plan.transactions[0].to, ONRAMP);
});

test('funds activation skips zero and below-threshold balances', () => {
  const plan = buildClaimFundsActivationTransactions({
    safeAddress: SAFE,
    usdceTokenAddress: USDCE,
    collateralOnrampAddress: ONRAMP,
    usdcEBalanceRaw: 9_000n,
    allowanceRaw: 9_000n,
    minUsdc: 0.01,
  });

  assert.equal(plan.status, 'skipped');
  assert.equal(plan.transactions.length, 0);
});

test('claim error formatter hides raw activation and HTML relayer errors', () => {
  assert.match(
    formatClaimErrorForDisplay('relayer_wallet_activation_required: Activate Funds') ?? '',
    /funds activation/
  );
  assert.match(
    formatClaimErrorForDisplay('<!DOCTYPE html><html><body>oops</body></html>') ?? '',
    /HTML hata/
  );
});
