import assert from 'node:assert/strict';
import test from 'node:test';

import {
  __analyticsTestUtils,
  buildAutoScopeTradeAnalysisCsv,
} from '@/lib/queries/trade-flow/analytics';
import type { AutoScopeTradeAnalysisRow } from '@/lib/types';

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

test('buildAutoScopeTradeAnalysisCsv escapes commas and includes pnl breakdown', () => {
  const rows: AutoScopeTradeAnalysisRow[] = [
    {
      rowId: 'sell:1',
      rowType: 'sell_exit',
      positionState: 'closed_exit',
      definitionId: 10,
      definitionName: 'Flow, A',
      runId: 20,
      rootOrderId: 30,
      exitOrderId: 31,
      marketSlug: 'btc-updown-5m-1772296200',
      tokenId: 'token-1',
      outcomeLabel: 'Up',
      exitReason: 'sl',
      marketEndAt: '2026-02-28T16:35:00.000Z',
      marketOpenAt: '2026-02-28T16:30:00.000Z',
      triggeredAt: '2026-02-28T16:31:00.000Z',
      buyFilledAt: '2026-02-28T16:31:01.000Z',
      sellFilledAt: '2026-02-28T16:32:00.000Z',
      markPriceCapturedAt: '2026-02-28T16:32:00.000Z',
      openToTriggerMs: 60_000,
      triggerToBuyFillMs: 1000,
      buyAvgPrice: 0.4,
      sellOrLivePrice: 0.35,
      rowQty: 10,
      remainingQtyAfterExit: 0,
      rowPnlUsdc: -0.6,
      buyNotionalUsdc: 4,
      buyFeeUsdc: 0.1,
      costBasisUsdc: 4.1,
      sellNotionalUsdc: 3.6,
      sellFeeUsdc: 0.1,
      markValueUsdc: null,
      netValueUsdc: 3.5,
      pnlPct: -14.63,
      valuationKind: 'realized',
    },
  ];

  const csv = buildAutoScopeTradeAnalysisCsv(rows);

  assert.match(csv, /^workflow,definition_id,/);
  assert.match(csv, /"Flow, A"/);
  assert.match(csv, /buy_fee_usdc/);
  assert.match(csv, /-14.63/);
});
