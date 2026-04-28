import { pool } from '@/lib/db';
import type { AutoScopeTradeAnalysisRow, AutoScopeTradeAnalysisSummary } from '@/lib/types';

export const AUTO_SCOPE_CASH_PNL_CSV_HEADERS = [
  'cash_fill_pnl_usdc',
  'cash_pnl_source',
  'local_fallback_cash_fill_pnl_usdc',
];

export function autoScopeCashPnlCsvValues(
  row: AutoScopeTradeAnalysisRow
): Array<number | string | null> {
  return [
    row.cashFillPnlUsdc,
    row.cashPnlSource,
    row.localFallbackCashFillPnlUsdc,
  ];
}

export interface AutoScopeCashMetrics {
  cashFillPnlUsdc: number | null;
  cashPnlSource: string | null;
  localFallbackCashFillPnlUsdc: number | null;
  diagnosticPnlUsdc: number | null;
  economicPnlUsdc: number | null;
  cashBuyUsdc: number | null;
  cashSellUsdc: number | null;
  cashRedeemUsdc: number | null;
  officialRootPnlUsdc: number | null;
  officialPnlSource: string | null;
  officialBuyUsdc: number | null;
  officialSellUsdc: number | null;
  officialRedeemUsdc: number | null;
  officialDeltaUsdc: number | null;
  pendingInventoryQty: number | null;
  pendingInventoryValueUsdc: number | null;
  pendingRedeemableValueUsdc: number | null;
  cashStatus: string | null;
}

interface AutoScopeCashSummaryDb {
  root_count: number;
  cash_metric_count: number;
  local_cash_fill_pnl_usdc: number | null;
  diagnostic_pnl_usdc: number | null;
  economic_pnl_usdc: number | null;
  pending_inventory_value_usdc: number | null;
  pending_redeemable_value_usdc: number | null;
}

function compactNumber(
  compact: Record<string, unknown> | null,
  key: string
): number | null {
  const value = compact?.[key];
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function compactString(
  compact: Record<string, unknown> | null,
  key: string
): string | null {
  const value = compact?.[key];
  return typeof value === 'string' && value.trim() ? value : null;
}

function numberOrZero(value: number | null | undefined): number {
  return value == null ? 0 : Number(value);
}

export function mapAutoScopeCashMetrics(
  compact: Record<string, unknown> | null
): AutoScopeCashMetrics {
  return {
    cashFillPnlUsdc: compactNumber(compact, 'cash_fill_pnl_usdc'),
    cashPnlSource: compactString(compact, 'cash_pnl_source'),
    localFallbackCashFillPnlUsdc: compactNumber(compact, 'local_fallback_cash_fill_pnl_usdc'),
    diagnosticPnlUsdc: compactNumber(compact, 'diagnostic_pnl_usdc'),
    economicPnlUsdc: compactNumber(compact, 'economic_pnl_usdc'),
    cashBuyUsdc: compactNumber(compact, 'cash_buy_notional_usdc'),
    cashSellUsdc: compactNumber(compact, 'cash_sell_notional_usdc'),
    cashRedeemUsdc: compactNumber(compact, 'cash_redeem_usdc'),
    officialRootPnlUsdc: compactNumber(compact, 'official_pnl_usdc'),
    officialPnlSource: compactString(compact, 'official_pnl_source'),
    officialBuyUsdc: compactNumber(compact, 'official_buy_notional_usdc'),
    officialSellUsdc: compactNumber(compact, 'official_sell_notional_usdc'),
    officialRedeemUsdc: compactNumber(compact, 'official_redeem_usdc'),
    officialDeltaUsdc: compactNumber(compact, 'official_delta_usdc'),
    pendingInventoryQty: compactNumber(compact, 'pending_inventory_qty'),
    pendingInventoryValueUsdc: compactNumber(compact, 'pending_inventory_value_usdc'),
    pendingRedeemableValueUsdc: compactNumber(compact, 'pending_redeemable_value_usdc'),
    cashStatus: compactString(compact, 'cash_status'),
  };
}

export async function getAutoScopeCashMetricsSummaryForWhere({
  whereClause,
  params,
}: {
  whereClause: string;
  params: Array<number | string | null>;
}): Promise<Partial<AutoScopeTradeAnalysisSummary>> {
  const result = await pool.query<AutoScopeCashSummaryDb>(
    `WITH filtered_roots AS (
       SELECT DISTINCT s.root_builder_order_id
       FROM trade_flow_auto_scope_analysis_rows s
       WHERE ${whereClause}
     )
     SELECT
       COUNT(*)::int AS root_count,
       COUNT(*) FILTER (WHERE dg.compact_metrics_json ? 'cash_fill_pnl_usdc')::int AS cash_metric_count,
       COALESCE(SUM(COALESCE((dg.compact_metrics_json->>'cash_fill_pnl_usdc')::double precision, 0)), 0)::double precision AS local_cash_fill_pnl_usdc,
       COALESCE(SUM(COALESCE((dg.compact_metrics_json->>'diagnostic_pnl_usdc')::double precision, 0)), 0)::double precision AS diagnostic_pnl_usdc,
       COALESCE(SUM(COALESCE((dg.compact_metrics_json->>'economic_pnl_usdc')::double precision, 0)), 0)::double precision AS economic_pnl_usdc,
       COALESCE(SUM(COALESCE((dg.compact_metrics_json->>'pending_inventory_value_usdc')::double precision, 0)), 0)::double precision AS pending_inventory_value_usdc,
       COALESCE(SUM(COALESCE((dg.compact_metrics_json->>'pending_redeemable_value_usdc')::double precision, 0)), 0)::double precision AS pending_redeemable_value_usdc
     FROM filtered_roots r
     JOIN trade_flow_auto_scope_trade_diagnostics dg
       ON dg.root_builder_order_id = r.root_builder_order_id`,
    params
  );
  const row = result.rows[0];
  if (!row || Number(row.root_count || 0) !== Number(row.cash_metric_count || 0)) {
    return {};
  }
  return {
    localCashFillPnlUsdc: numberOrZero(row?.local_cash_fill_pnl_usdc),
    diagnosticPnlUsdc: numberOrZero(row?.diagnostic_pnl_usdc),
    economicPnlUsdc: numberOrZero(row?.economic_pnl_usdc),
    pendingInventoryValueUsdc: numberOrZero(row?.pending_inventory_value_usdc),
    pendingRedeemableValueUsdc: numberOrZero(row?.pending_redeemable_value_usdc),
  };
}
