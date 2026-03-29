import assert from 'node:assert/strict';
import test from 'node:test';

import { __analyticsTestUtils } from '@/lib/queries/trade-flow/analytics';

test('deriveMarketEndAtFromSlug resolves 5m market end', () => {
  const endedAt = __analyticsTestUtils.deriveMarketEndAtFromSlug(
    'btc-updown-5m-1772296200'
  );
  assert.equal(endedAt, '2026-02-28T16:35:00.000Z');
});

test('deriveMarketEndAtFromSlug resolves 15m market end', () => {
  const endedAt = __analyticsTestUtils.deriveMarketEndAtFromSlug(
    'btc-updown-15m-1772296200'
  );
  assert.equal(endedAt, '2026-02-28T16:45:00.000Z');
});

test('derivePositionState treats ended open positions as closed_market_ended', () => {
  const state = __analyticsTestUtils.derivePositionState(
    'open_position',
    '2026-03-29T10:00:00.000Z',
    '2026-03-29T10:05:00.000Z'
  );
  assert.equal(state, 'closed_market_ended');
});

test('derivePositionState keeps future open positions as open', () => {
  const state = __analyticsTestUtils.derivePositionState(
    'open_position',
    '2026-03-29T17:00:00.000Z',
    '2026-03-29T16:05:00.000Z'
  );
  assert.equal(state, 'open');
});

test('derivePositionState always marks sell_exit rows as closed_exit', () => {
  const state = __analyticsTestUtils.derivePositionState(
    'sell_exit',
    null,
    '2026-03-29T16:05:00.000Z'
  );
  assert.equal(state, 'closed_exit');
});

test('buildOrderByClause returns pnl ascending order when requested', () => {
  const clause = __analyticsTestUtils.buildOrderByClause('pnl', 'asc');
  assert.match(clause, /row_pnl_usdc ASC/);
});

test('buildOrderByClause keeps default ordering when sortBy=default', () => {
  const clause = __analyticsTestUtils.buildOrderByClause('default', 'desc');
  assert.match(clause, /triggered_at DESC/);
  assert.doesNotMatch(clause, /row_pnl_usdc ASC/);
});
