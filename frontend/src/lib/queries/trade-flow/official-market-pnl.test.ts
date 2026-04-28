import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildOfficialMarketLedgersFromActivity,
  buildOfficialMarketPnlSummaryFromLedgers,
  type OfficialMarketActivity,
} from '@/lib/queries/trade-flow/official-market-pnl';

const MARKET_SLUGS = new Set(['btc-updown-5m-1777336200', 'btc-updown-5m-1777336800']);

test('official market pnl uses wallet cash ledger values', () => {
  const ledgers = buildOfficialMarketLedgersFromActivity({
    activity: [
      activity('TRADE', 'BUY', 'btc-updown-5m-1777336800', 10.02238, 4.8974),
      activity('TRADE', 'SELL', 'btc-updown-5m-1777336800', 9, 2.739),
      activity('REDEEM', null, 'btc-updown-5m-1777336800', 1.02238, 1.02238),
    ],
    marketSlugs: MARKET_SLUGS,
    pnlFilter: 'all',
  });
  const summary = buildOfficialMarketPnlSummaryFromLedgers({
    ledgers,
    rootRowsPnlUsdc: -1.6,
  });

  assert.equal(summary.totalPnlUsdc, -1.13602);
  assert.equal(summary.realizedPnlUsdc, -1.13602);
  assert.equal(summary.officialBuyUsdc, 4.8974);
  assert.equal(summary.officialSellUsdc, 2.739);
  assert.equal(summary.officialRedeemUsdc, 1.02238);
  assert.equal(summary.officialDeltaUsdc, 0.46398);
});

test('official market pnl includes counter buy cash in same market', () => {
  const ledgers = buildOfficialMarketLedgersFromActivity({
    activity: [
      activity('TRADE', 'BUY', 'btc-updown-5m-1777336200', 9.09, 9.9258),
      activity('TRADE', 'BUY', 'btc-updown-5m-1777336200', 8.84, 3.2708),
      activity('TRADE', 'SELL', 'btc-updown-5m-1777336200', 9.09, 12.066308),
    ],
    marketSlugs: MARKET_SLUGS,
    pnlFilter: 'all',
  });

  assert.equal(ledgers[0]?.buyUsdc, 13.1966);
  assert.equal(ledgers[0]?.sellUsdc, 12.06631);
  assert.equal(ledgers[0]?.pnlUsdc, -1.13029);
});

test('official market pnl filters by activity timestamp instead of row updated time', () => {
  const ledgers = buildOfficialMarketLedgersFromActivity({
    activity: [
      activity(
        'TRADE',
        'BUY',
        'btc-updown-5m-1777336800',
        10,
        5,
        Date.parse('2026-04-27T11:00:00.000Z') / 1000
      ),
      activity(
        'TRADE',
        'SELL',
        'btc-updown-5m-1777336800',
        10,
        6,
        Date.parse('2026-04-27T13:00:00.000Z') / 1000
      ),
    ],
    marketSlugs: MARKET_SLUGS,
    from: '2026-04-27T12:00:00.000Z',
    to: '2026-04-27T14:00:00.000Z',
    pnlFilter: 'all',
  });

  assert.equal(ledgers[0]?.buyUsdc, 0);
  assert.equal(ledgers[0]?.sellUsdc, 6);
  assert.equal(ledgers[0]?.pnlUsdc, 6);
});

function activity(
  activityType: string,
  side: string | null,
  slug: string,
  size: number,
  usdcSize: number,
  timestamp = 1
): OfficialMarketActivity {
  return {
    activityType,
    side,
    slug,
    size,
    usdcSize,
    timestamp,
  };
}
