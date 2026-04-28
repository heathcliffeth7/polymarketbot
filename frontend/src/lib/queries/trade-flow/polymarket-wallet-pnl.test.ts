import assert from 'node:assert/strict';
import test from 'node:test';

import { __polymarketWalletPnlTestUtils } from './polymarket-wallet-pnl';

test('leaderboard pnl is extracted as all-time wallet pnl', () => {
  const pnl = __polymarketWalletPnlTestUtils.extractLeaderboardPnl({
    data: [{ pnl: '-249.37610998891682' }],
  });

  assert.equal(pnl, -249.37611);
});

test('user-pnl history delta uses last minus first point', () => {
  const pnl = __polymarketWalletPnlTestUtils.buildUserPnlDelta([
    { t: 1777381200, p: -249.37611 },
    { t: 1777294800, p: -252.43118 },
  ]);

  assert.equal(pnl, 3.05507);
});

test('3h trade analysis filter maps to supported Polymarket 6h history window', () => {
  const request = __polymarketWalletPnlTestUtils.mapTimeRangeToUserPnlRequest('3h');

  assert.deepEqual(request, { interval: '6h', fidelity: '1h' });
});

test('closed position realized pnl becomes Polymarket position pnl', () => {
  const stats = __polymarketWalletPnlTestUtils.buildPolymarketPositionStats({
    closedRows: [
      {
        slug: 'btc-updown-5m-1777336800',
        asset: 'token-up',
        outcome: 'Up',
        realizedPnl: -0.5974,
        totalBought: 10.42,
        avgPrice: 0.47,
      },
    ],
    openRows: [],
  });
  const row = stats.index.get('btc-updown-5m-1777336800|asset:token-up');

  assert.equal(row?.pnlUsdc, -0.5974);
  assert.equal(row?.source, 'closed_positions');
  assert.equal(row?.totalBetUsdc, 4.8974);
  assert.equal(stats.marketPnlIndex.get('btc-updown-5m-1777336800'), -0.5974);
});

test('redeemable lost open position is treated as synthetic closed loss', () => {
  const stats = __polymarketWalletPnlTestUtils.buildPolymarketPositionStats({
    closedRows: [],
    openRows: [
      {
        slug: 'btc-updown-5m-1777336800',
        asset: 'token-down',
        outcome: 'Down',
        redeemable: true,
        currentValue: 0,
        initialValue: 5,
      },
    ],
  });
  const row = stats.index.get('btc-updown-5m-1777336800|asset:token-down');

  assert.equal(row?.pnlUsdc, -5);
  assert.equal(row?.source, 'positions_redeemable_lost');
  assert.equal(row?.amountReturnedUsdc, 0);
  assert.equal(stats.marketPnlIndex.get('btc-updown-5m-1777336800'), -5);
});

test('redeemable lost position uses cash pnl before zero realized pnl', () => {
  const stats = __polymarketWalletPnlTestUtils.buildPolymarketPositionStats({
    closedRows: [],
    openRows: [
      {
        slug: 'btc-updown-5m-1777335600',
        asset: 'token-up',
        outcome: 'Up',
        redeemable: true,
        currentValue: 0,
        curPrice: 0,
        initialValue: 3.3832,
        cashPnl: -3.3832,
        realizedPnl: 0,
      },
    ],
  });
  const row = stats.index.get('btc-updown-5m-1777335600|asset:token-up');

  assert.equal(row?.pnlUsdc, -3.3832);
  assert.equal(row?.source, 'positions_redeemable_lost');
  assert.equal(stats.marketPnlIndex.get('btc-updown-5m-1777335600'), -3.3832);
});

test('sold down and lost up market keeps activity pnl separate from position audit', () => {
  const stats = __polymarketWalletPnlTestUtils.buildPolymarketPositionStats({
    closedRows: [
      {
        slug: 'btc-updown-5m-1777335600',
        asset: 'token-down',
        outcome: 'Down',
        realizedPnl: 4.003,
        totalBought: 9.09,
        avgPrice: 0.55,
      },
    ],
    openRows: [
      {
        slug: 'btc-updown-5m-1777335600',
        asset: 'token-up',
        outcome: 'Up',
        redeemable: true,
        currentValue: 0,
        curPrice: 0,
        initialValue: 3.3832,
        cashPnl: -3.3832,
        realizedPnl: 0,
      },
    ],
  });
  const status = __polymarketWalletPnlTestUtils.resolvePnlSourceStatus({
    baseStatus: 'activity_market',
    activityMarketPnlUsdc: 0.127,
    positionMarketPnlUsdc: stats.marketPnlIndex.get('btc-updown-5m-1777335600') ?? null,
  });

  assert.equal(stats.marketPnlIndex.get('btc-updown-5m-1777335600'), 0.6198);
  assert.equal(status, 'pnl_source_mismatch');
});

test('position stats aggregate positions and closed positions by market slug', () => {
  const stats = __polymarketWalletPnlTestUtils.buildPolymarketPositionStats({
    closedRows: [
      {
        slug: 'btc-updown-5m-1777336200',
        asset: 'token-down',
        outcome: 'Down',
        realizedPnl: 1.2,
        totalBet: 3,
      },
    ],
    openRows: [
      {
        slug: 'btc-updown-5m-1777336200',
        asset: 'token-up',
        outcome: 'Up',
        cashPnl: -3.1939,
        realizedPnl: -0.7297,
      },
    ],
  });

  assert.equal(stats.marketPnlIndex.get('btc-updown-5m-1777336200'), -2.7236);
});

test('activity versus position market pnl mismatch updates source status', () => {
  const status = __polymarketWalletPnlTestUtils.resolvePnlSourceStatus({
    baseStatus: 'activity_market',
    activityMarketPnlUsdc: -0.9717,
    positionMarketPnlUsdc: -3.9236,
  });

  assert.equal(status, 'pnl_source_mismatch');
});

test('small activity versus position pnl drift keeps source status', () => {
  const status = __polymarketWalletPnlTestUtils.resolvePnlSourceStatus({
    baseStatus: 'activity_market',
    activityMarketPnlUsdc: 0.877,
    positionMarketPnlUsdc: 0.865,
  });

  assert.equal(status, 'activity_market');
});
