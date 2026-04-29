import assert from 'node:assert/strict';
import test from 'node:test';

import type { AutoScopeTradeAnalysisSummary } from '@/lib/types';
import { buildAutoScopeSummaryCardMetrics } from './auto-scope-summary-cards';

function baseSummary(): AutoScopeTradeAnalysisSummary {
  return {
    rowCount: 100,
    marketCount: 10,
    lossCount: 4,
    profitCount: 6,
    totalPnlUsdc: -748.68829,
    realizedPnlUsdc: -748.68829,
    openPnlUsdc: 0,
    lossUsdc: 30,
    profitUsdc: 20,
    buyFeeUsdc: 0,
    sellFeeUsdc: 0,
    totalFeeUsdc: 0,
    costBasisUsdc: 100,
    netValueUsdc: 20,
    profitFactor: 0.6666666667,
    winRatePct: 60.2780536246276,
    avgWinUsdc: 3.3333333333,
    avgLossUsdc: 7.5,
    largestLossUsdc: 12,
    feeDragUsdc: 0,
    diagnosisBreakdown: [],
    pnlSource: 'activity_cash',
    localCashFillPnlUsdc: -748.68829,
    referencePnlUsdc: -249.37611,
    referencePnlSource: 'polymarket_leaderboard',
    referenceDeltaUsdc: 499.31218,
    diagnosticPnlUsdc: -645.36,
    pendingInventoryValueUsdc: 0,
    pendingRedeemableValueUsdc: 0,
  };
}

test('summary cards show activity cash as the primary pnl and wallet pnl as reference', () => {
  const metrics = buildAutoScopeSummaryCardMetrics(baseSummary(), -13.08403);

  assert.deepEqual(
    metrics.slice(0, 4).map((metric) => [metric.label, metric.value]),
    [
      ['Activity Cash PnL', '-748.69 USDC'],
      ['Polymarket Reference PnL', '-249.38 USDC'],
      ['Diagnostic PnL', '-645.36 USDC'],
      ['Pending Inventory / Redeem', '0.00 USDC'],
    ]
  );
});

test('summary win rate is formatted without a pnl sign', () => {
  const metrics = buildAutoScopeSummaryCardMetrics(baseSummary(), -13.08403);
  const winRate = metrics.find((metric) => metric.label === 'Win Rate');

  assert.equal(winRate?.value, '60.28%');
});
