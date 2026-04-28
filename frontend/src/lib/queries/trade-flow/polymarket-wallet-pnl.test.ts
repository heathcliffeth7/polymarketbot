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
});
