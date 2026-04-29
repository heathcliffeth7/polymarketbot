import assert from 'node:assert/strict';
import test from 'node:test';

import {
  __analyticsTestUtils,
  buildAutoScopeTradeAnalysisCsv,
} from '@/lib/queries/trade-flow/analytics';
import {
  __autoScopeAnalysisExtrasTestUtils,
  buildAutoScopeNoOrderSignalsCsv,
} from '@/lib/queries/trade-flow/auto-scope-analysis-extras';
import {
  __autoScopeCashMetricsTestUtils,
  mapAutoScopeCashMetrics,
} from '@/lib/queries/trade-flow/auto-scope-analysis-cash-metrics';
import { __autoScopeMarketPnlAuditTestUtils } from '@/lib/queries/trade-flow/auto-scope-market-pnl-audit';
import type { AutoScopeTradeAnalysisRow, AutoScopeTradeBlockedSignal } from '@/lib/types';

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

test('derivePositionState marks settled payout rows as closed_exit', () => {
  const state = __analyticsTestUtils.derivePositionState(
    'settled_payout',
    null,
    '2026-03-29T16:05:00.000Z'
  );
  assert.equal(state, 'closed_exit');
});

test('buildOrderByClause returns pnl ascending order when requested', () => {
  const clause = __analyticsTestUtils.buildOrderByClause('pnl', 'asc');
  const analysisTimeExpr = __analyticsTestUtils.analysisFilterTimeExpr;
  const effectiveCashPnlExpr = __analyticsTestUtils.effectiveCashPnlExpr;
  assert.ok(clause.includes(`${effectiveCashPnlExpr} ASC NULLS LAST`));
  assert.ok(
    clause.indexOf(`${effectiveCashPnlExpr} ASC NULLS LAST`) <
      clause.indexOf(`${analysisTimeExpr} DESC NULLS LAST`)
  );
});

test('buildOrderByClause keeps default ordering by analysis time fallback', () => {
  const clause = __analyticsTestUtils.buildOrderByClause('default', 'desc');
  const analysisTimeExpr = __analyticsTestUtils.analysisFilterTimeExpr;
  assert.ok(clause.startsWith(`${analysisTimeExpr} DESC NULLS LAST`));
  assert.match(clause, /s\.buy_filled_at/);
  assert.doesNotMatch(clause, /row_pnl_usdc ASC/);
});

test('buildOrderByClause lets null-trigger rows sort by buy fill time', () => {
  const clause = __analyticsTestUtils.buildOrderByClause('default', 'desc');
  assert.match(clause, /COALESCE\(s\.triggered_at, s\.buy_filled_at,/);
  assert.doesNotMatch(clause, /^s\.triggered_at DESC NULLS LAST/);
});

test('analysis relative filters do not use updated_at fallback', () => {
  assert.doesNotMatch(__analyticsTestUtils.analysisFilterTimeExpr, /updated_at/);
  assert.match(__analyticsTestUtils.analysisFilterTimeExpr, /mark_price_captured_at/);
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
      cashFillPnlUsdc: -0.4,
      cashPnlSource: 'data_api_activity',
      localFallbackCashFillPnlUsdc: -0.5,
      diagnosticPnlUsdc: -0.6,
      economicPnlUsdc: -0.4,
      cashBuyUsdc: 4,
      cashSellUsdc: 3.6,
      cashRedeemUsdc: 0,
      officialRootPnlUsdc: -1.13602,
      officialPnlSource: 'data_api_activity',
      officialBuyUsdc: 4.8974,
      officialSellUsdc: 2.739,
      officialRedeemUsdc: 1.02238,
      officialDeltaUsdc: -1.72268,
      officialMarketPnlUsdc: 0.877,
      officialMarketBuyUsdc: 13.4182,
      officialMarketSellUsdc: 14.2952,
      officialMarketRedeemUsdc: 0,
      officialVsRootDeltaUsdc: 9.357,
      activityMarketPnlUsdc: 0.877,
      positionMarketPnlUsdc: -0.5974,
      localMarketPnlUsdc: -3.83,
      pnlSourceStatus: 'pnl_source_mismatch',
      polymarketPositionPnlUsdc: -0.5974,
      polymarketPositionSource: 'closed_positions',
      polymarketTotalBetUsdc: 4.8974,
      polymarketAmountReturnedUsdc: 4.3,
      polymarketRealizedPnlUsdc: -0.5974,
      polymarketCashPnlUsdc: null,
      pendingInventoryQty: 0,
      pendingInventoryValueUsdc: 0,
      pendingRedeemableValueUsdc: null,
      cashStatus: 'lost_unclaimed_or_unredeemed',
      valuationKind: 'realized',
      primaryDiagnosisCode: 'bad_entry_price',
      diagnosisLabel: 'Kotu giris fiyati',
      entryQualityScore: 72,
      exitQualityScore: 81,
      executionTelemetry: {
        submittedBestAsk: 0.58,
        submittedEstimatedAvgFill: 0.61,
        submittedVwapSlippage: 0.03,
        submittedTargetQty: 5,
        submittedEstimatedNotional: 3.05,
        submittedQFinal: 0.9474,
        submittedModelBookGap: 0.3724,
        submittedModelBookZone: 'WARNING',
        submittedParticipationCredit: 0.01,
        fillActualPrice: 0.61,
        fillActualQty: 5,
        fillActualNotional: 3.05,
        fillSlippageVsVwap: 0,
        fillSlippageVsBestAsk: 0.03,
        fillSource: 'fills_aggregate',
      },
    },
  ];

  const csv = buildAutoScopeTradeAnalysisCsv(rows);

  assert.match(csv, /^workflow,definition_id,/);
  assert.match(csv, /"Flow, A"/);
  assert.match(csv, /buy_fee_usdc/);
  assert.match(csv, /cash_fill_pnl_usdc/);
  assert.match(csv, /official_root_pnl_usdc/);
  assert.match(csv, /official_market_pnl_usdc/);
  assert.match(csv, /activity_market_pnl_usdc/);
  assert.match(csv, /position_market_pnl_usdc/);
  assert.match(csv, /local_market_pnl_usdc/);
  assert.doesNotMatch(csv, /polymarket_ui_market_pnl_usdc/);
  assert.doesNotMatch(csv, /display_pnl_usdc/);
  assert.match(csv, /pnl_source_status/);
  assert.match(csv, /pnl_source_mismatch/);
  assert.match(csv, /polymarket_position_pnl_usdc/);
  assert.match(csv, /closed_positions/);
  assert.match(csv, /data_api_activity/);
  assert.match(csv, /-1.13602/);
  assert.match(csv, /0.877/);
  assert.match(csv, /diagnostic_pnl_usdc/);
  assert.match(csv, /lost_unclaimed_or_unredeemed/);
  assert.match(csv, /diagnosis_code/);
  assert.match(csv, /required_q/);
  assert.match(csv, /submitted_estimated_avg_fill/);
  assert.match(csv, /fill_slippage_vs_best_ask/);
  assert.match(csv, /cash_pnl_source/);
  assert.match(csv, /local_fallback_cash_fill_pnl_usdc/);
  assert.match(csv, /fills_aggregate/);
  assert.match(csv, /bad_entry_price/);
  assert.match(csv, /-14.63/);
});

test('mapAutoScopeCashMetrics separates cash diagnostic and pending values', () => {
  const metrics = mapAutoScopeCashMetrics({
    cash_fill_pnl_usdc: -24.59,
    cash_pnl_source: 'data_api_activity',
    local_fallback_cash_fill_pnl_usdc: -27.51,
    diagnostic_pnl_usdc: 27.51,
    economic_pnl_usdc: -4.59,
    cash_buy_notional_usdc: 169.98,
    cash_sell_notional_usdc: 145.39,
    cash_redeem_usdc: 0,
    official_pnl_usdc: -1.13602,
    official_pnl_source: 'data_api_activity',
    official_buy_notional_usdc: 4.8974,
    official_sell_notional_usdc: 2.739,
    official_redeem_usdc: 1.02238,
    official_delta_usdc: -1.72268,
    official_market_pnl_usdc: 0.877,
    official_market_buy_usdc: 13.4182,
    official_market_sell_usdc: 14.2952,
    official_market_redeem_usdc: 0,
    official_vs_root_delta_usdc: 9.357,
    pending_inventory_qty: 35,
    pending_inventory_value_usdc: 20,
    pending_redeemable_value_usdc: null,
    cash_status: 'pending_inventory_or_redeem',
  });

  assert.equal(metrics.cashFillPnlUsdc, -24.59);
  assert.equal(metrics.cashPnlSource, 'data_api_activity');
  assert.equal(metrics.localFallbackCashFillPnlUsdc, -27.51);
  assert.equal(metrics.diagnosticPnlUsdc, 27.51);
  assert.equal(metrics.cashBuyUsdc, 169.98);
  assert.equal(metrics.officialRootPnlUsdc, -1.13602);
  assert.equal(metrics.officialPnlSource, 'data_api_activity');
  assert.equal(metrics.officialRedeemUsdc, 1.02238);
  assert.equal(metrics.officialMarketPnlUsdc, 0.877);
  assert.equal(metrics.officialMarketSellUsdc, 14.2952);
  assert.equal(metrics.activityMarketPnlUsdc, 0.877);
  assert.equal(metrics.positionMarketPnlUsdc, null);
  assert.equal(metrics.localMarketPnlUsdc, null);
  assert.equal(metrics.pnlSourceStatus, 'local_fallback');
  assert.equal(metrics.cashStatus, 'pending_inventory_or_redeem');
});

test('mapAutoScopeCashMetrics uses official market pnl for ambiguous activity rows', () => {
  const metrics = mapAutoScopeCashMetrics(
    {
      cash_fill_pnl_usdc: -3.83,
      cash_pnl_source: 'local_fallback',
      local_fallback_cash_fill_pnl_usdc: -3.83,
      official_market_pnl_usdc: 0.877,
      official_market_buy_usdc: 13.4182,
      official_market_sell_usdc: 14.2952,
      official_market_redeem_usdc: 0,
      official_vs_root_delta_usdc: 4.707,
    },
    ['official_activity_ambiguous']
  );

  assert.equal(metrics.cashFillPnlUsdc, 0.877);
  assert.equal(metrics.localFallbackCashFillPnlUsdc, -3.83);
  assert.equal(metrics.officialMarketBuyUsdc, 13.4182);
  assert.equal(metrics.activityMarketPnlUsdc, 0.877);
  assert.equal(metrics.pnlSourceStatus, 'activity_market');
});

test('mapAutoScopeCashMetrics uses official market pnl for market-scope rows', () => {
  const metrics = mapAutoScopeCashMetrics(
    {
      cash_fill_pnl_usdc: -1,
      cash_pnl_source: 'data_api_activity',
      local_fallback_cash_fill_pnl_usdc: -1.81,
      official_market_pnl_usdc: -3.24722,
      official_market_buy_usdc: 9.8982,
      official_market_sell_usdc: 5.6286,
      official_market_redeem_usdc: 1.02238,
    },
    ['official_market_scope_required']
  );

  assert.equal(metrics.cashFillPnlUsdc, -3.24722);
  assert.equal(metrics.localFallbackCashFillPnlUsdc, -1.81);
  assert.equal(metrics.activityMarketPnlUsdc, -3.24722);
  assert.equal(metrics.pnlSourceStatus, 'activity_market');
});

test('mapAutoScopeCashMetrics keeps fallback when official market activity has no evidence', () => {
  const metrics = mapAutoScopeCashMetrics(
    {
      cash_fill_pnl_usdc: -6,
      cash_pnl_source: 'local_fallback',
      local_fallback_cash_fill_pnl_usdc: -6,
      official_market_pnl_usdc: 0,
      official_market_buy_usdc: 0,
      official_market_sell_usdc: 0,
      official_market_redeem_usdc: 0,
    },
    ['official_market_scope_required']
  );

  assert.equal(metrics.cashFillPnlUsdc, -6);
  assert.equal(metrics.officialMarketPnlUsdc, 0);
  assert.equal(metrics.activityMarketPnlUsdc, null);
  assert.equal(metrics.pnlSourceStatus, 'local_fallback_no_activity_evidence');
});

test('local market pnl audit SQL sums distinct root diagnostic fallback by market', () => {
  const sql = __autoScopeMarketPnlAuditTestUtils.localMarketPnlSql;
  assert.match(sql, /DISTINCT ON \(lower\(btrim\(dg\.market_slug\)\), dg\.root_builder_order_id\)/);
  assert.match(sql, /local_fallback_cash_fill_pnl_usdc/);
  assert.match(sql, /cash_fill_pnl_usdc/);
  assert.match(sql, /dg\.total_pnl_usdc/);
  assert.match(sql, /GROUP BY market_slug/);
});

test('cash metrics summary SQL counts ambiguous market pnl once per market', () => {
  const sql = __autoScopeCashMetricsTestUtils.buildAutoScopeCashMetricsSummarySql('s.user_id = $1');
  assert.match(sql, /market_effective AS/);
  assert.match(sql, /GROUP BY market_slug/);
  assert.match(sql, /BOOL_OR\(use_market_pnl\)/);
  assert.match(sql, /official_market_scope_required/);
  assert.match(sql, /official_market_buy_usdc/);
  assert.match(sql, /effective_pnl_usdc/);
  assert.match(sql, /profit_count/);
  assert.match(sql, /loss_count/);
  assert.match(sql, /largest_loss_usdc/);
  assert.doesNotMatch(sql, /SUM\(COALESCE\(\(dg\.compact_metrics_json->>'cash_fill_pnl_usdc'\)/);
});

test('buildAutoScopeNoOrderSignalsCsv includes quote status telemetry', () => {
  const signals: AutoScopeTradeBlockedSignal[] = [
    {
      eventType: 'missed_market_order_not_filled_notification_sent',
      createdAt: '2026-02-28T16:35:00.000Z',
      nodeKey: 'trigger_market',
      marketSlug: 'btc-updown-5m-1772296200',
      outcomeLabel: 'Down',
      reasonCode: 'no_matching_block_event',
      reasonDetail: null,
      signalQuality: null,
      riskFlags: { highPrice: false, stale: false, fallingKnife: false, chop: false, reasons: [] },
      noOrderTelemetry: {
        orderCreated: false,
        orderSubmitted: false,
        orderFilled: false,
        finalActionStatus: 'NO_ORDER',
        lastGuardName: 'Trigger Condition',
        lastGuardCode: 'no_matching_block_event',
        lastGuardState: 'blocked',
        executionFloor: null,
        bestAskAtWindowEnd: null,
        floorDistance: null,
        floorWaitMs: null,
        liquidityRegime: 'UNKNOWN',
        hourlyVolumeRatio: null,
        volume30s: null,
        tradeCount60s: 0,
        quoteSnapshotSource: 'final_fetch',
        bookDataStatus: 'selected_side_only',
        quoteMissingReason: 'Up quote missing',
        selectedBid: 0.06,
        selectedAsk: 0.07,
        selectedMid: 0.065,
        upBid: null,
        upAsk: null,
        downBid: 0.06,
        downAsk: 0.07,
        bookSide: null,
        upMid: null,
        downMid: 0.065,
        bookMidDiff: null,
        whyNoOrderSummary: 'Down guard condition did not pass before the market window ended.',
        humanReadableReason: 'Trigger Condition guard stayed blocked before window end.',
      },
    },
  ];

  const csv = buildAutoScopeNoOrderSignalsCsv(signals);

  assert.match(csv, /quote_snapshot_source/);
  assert.match(csv, /book_data_status/);
  assert.match(csv, /selected_side_only/);
  assert.match(csv, /Up quote missing/);
});

test('auto-scope signal quality derives required q and q margin', () => {
  const quality = __autoScopeAnalysisExtrasTestUtils.buildAutoScopeSignalQualityFromGuard({
    threshold_mode: 'iv_mismatch_edge',
    iv_mismatch_edge: {
      passed: true,
      decision_reason: 'selected_edge_passed',
      selected_side: 'up',
      q_final: 0.7,
      q_up: 0.72,
      q_down: 0.28,
      cost: 0.52,
      threshold: 0.06,
      dynamic_threshold: 0.08,
      edge_adj: 0.18,
    },
  });

  assert.equal(quality?.mode, 'iv_mismatch_edge');
  assert.equal(quality?.requiredQ, 0.6);
  assert.equal(quality?.qMargin, 0.1);
});

test('auto-scope risk flags map penalties and blocked reasons', () => {
  const flags = __autoScopeAnalysisExtrasTestUtils.buildAutoScopeRiskFlagsFromGuard({
    threshold_mode: 'iv_mismatch_edge',
    iv_mismatch_edge: {
      decision_reason: 'blocked_falling_knife_drop',
      high_price_penalty: 0.02,
      stale_penalty: 0.02,
      drop_penalty: 0.03,
    },
  });

  assert.equal(flags.highPrice, true);
  assert.equal(flags.stale, true);
  assert.equal(flags.fallingKnife, true);
  assert.equal(flags.chop, false);
  assert.ok(flags.reasons.includes('blocked_falling_knife_drop'));
});

test('auto-scope scenario pnl separates up down ev and worst cases', () => {
  const rows = [
    {
      root_builder_order_id: 1,
      run_id: 10,
      market_slug: 'btc-updown-5m-1772296200',
      outcome_label: 'Up',
      row_type: 'sell_exit',
      exit_reason: 'tp',
      row_qty: 2,
      row_pnl_usdc: 0.4,
      cost_basis_usdc: 1,
      valuation_kind: 'realized',
      triggered_at: null,
      buy_filled_at: null,
      sell_filled_at: null,
      mark_price_captured_at: null,
      updated_at: '2026-02-28T16:32:00.000Z',
    },
    {
      root_builder_order_id: 1,
      run_id: 10,
      market_slug: 'btc-updown-5m-1772296200',
      outcome_label: 'Up',
      row_type: 'settled_payout',
      exit_reason: 'other',
      row_qty: 1,
      row_pnl_usdc: 0.1,
      cost_basis_usdc: 0.5,
      valuation_kind: 'settled',
      triggered_at: null,
      buy_filled_at: null,
      sell_filled_at: null,
      mark_price_captured_at: null,
      updated_at: '2026-02-28T16:35:00.000Z',
    },
    {
      root_builder_order_id: 1,
      run_id: 10,
      market_slug: 'btc-updown-5m-1772296200',
      outcome_label: 'Down',
      row_type: 'open_position',
      exit_reason: 'open_position',
      row_qty: 3,
      row_pnl_usdc: -0.2,
      cost_basis_usdc: 1.5,
      valuation_kind: 'mark_to_market',
      triggered_at: null,
      buy_filled_at: null,
      sell_filled_at: null,
      mark_price_captured_at: null,
      updated_at: '2026-02-28T16:33:00.000Z',
    },
  ] as Parameters<typeof __autoScopeAnalysisExtrasTestUtils.buildAutoScopeScenarioPnl>[0];

  const scenario = __autoScopeAnalysisExtrasTestUtils.buildAutoScopeScenarioPnl(rows, {
    mode: 'iv_mismatch_edge',
    decisionReason: 'selected_edge_passed',
    passed: true,
    selectedSide: 'down',
    candidateSide: 'down',
    q: 0.65,
    qUp: 0.35,
    qDown: 0.65,
    cost: 0.52,
    threshold: 0.08,
    dynamicThreshold: 0.08,
    requiredQ: 0.6,
    qMargin: 0.05,
    edge: 0.13,
    edgeAdjusted: 0.13,
    secondsLeft: 30,
  });

  assert.equal(scenario.realizedPnlUsdc, 0.5);
  assert.equal(scenario.markPnlUsdc, -0.2);
  assert.equal(scenario.ifUpUsdc, -1.0);
  assert.equal(scenario.ifDownUsdc, 2.0);
  assert.equal(scenario.worstUsdc, -1.0);
  assert.equal(scenario.evUsdc, 0.95);
});
