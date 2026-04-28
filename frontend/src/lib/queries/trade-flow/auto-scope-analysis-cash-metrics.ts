import { pool } from '@/lib/db';
import type {
  AutoScopeTradeAnalysisPnlSourceStatus,
  AutoScopeTradeAnalysisRow,
  AutoScopeTradeAnalysisSummary,
} from '@/lib/types';

export const AUTO_SCOPE_CASH_PNL_CSV_HEADERS = [
  'cash_fill_pnl_usdc',
  'cash_pnl_source',
  'local_fallback_cash_fill_pnl_usdc',
];

const OFFICIAL_MARKET_ACTIVITY_FLAG_SQL = `(dg.data_quality_flags @> ARRAY['official_activity_ambiguous']::text[]
                  OR dg.data_quality_flags @> ARRAY['official_market_scope_required']::text[])`;
const OFFICIAL_MARKET_ACTIVITY_EVIDENCE_SQL = `(COALESCE((dg.compact_metrics_json->>'official_market_buy_usdc')::double precision, 0)
                  + COALESCE((dg.compact_metrics_json->>'official_market_sell_usdc')::double precision, 0)
                  + COALESCE((dg.compact_metrics_json->>'official_market_redeem_usdc')::double precision, 0)) > 0`;

export const AUTO_SCOPE_EFFECTIVE_CASH_PNL_SQL_EXPR = `CASE
              WHEN ${OFFICIAL_MARKET_ACTIVITY_FLAG_SQL}
                AND ${OFFICIAL_MARKET_ACTIVITY_EVIDENCE_SQL}
                AND (dg.compact_metrics_json->>'official_market_pnl_usdc') IS NOT NULL
              THEN (dg.compact_metrics_json->>'official_market_pnl_usdc')::double precision
              ELSE COALESCE((dg.compact_metrics_json->>'cash_fill_pnl_usdc')::double precision, s.row_pnl_usdc)
            END`;

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
  officialMarketPnlUsdc: number | null;
  officialMarketBuyUsdc: number | null;
  officialMarketSellUsdc: number | null;
  officialMarketRedeemUsdc: number | null;
  officialVsRootDeltaUsdc: number | null;
  activityMarketPnlUsdc: number | null;
  positionMarketPnlUsdc: number | null;
  localMarketPnlUsdc: number | null;
  pnlSourceStatus: AutoScopeTradeAnalysisPnlSourceStatus | null;
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

function hasDataQualityFlag(
  flags: readonly string[] | null | undefined,
  flag: string
): boolean {
  return Array.isArray(flags) && flags.includes(flag);
}

function hasOfficialMarketScopeFlag(flags: readonly string[] | null | undefined): boolean {
  return (
    hasDataQualityFlag(flags, 'official_activity_ambiguous') ||
    hasDataQualityFlag(flags, 'official_market_scope_required')
  );
}

function hasOfficialMarketActivityEvidence(compact: Record<string, unknown> | null): boolean {
  return (
    (compactNumber(compact, 'official_market_buy_usdc') ?? 0) +
      (compactNumber(compact, 'official_market_sell_usdc') ?? 0) +
      (compactNumber(compact, 'official_market_redeem_usdc') ?? 0) >
    0
  );
}

export function mapAutoScopeCashMetrics(
  compact: Record<string, unknown> | null,
  dataQualityFlags: readonly string[] | null = []
): AutoScopeCashMetrics {
  const officialMarketPnlUsdc = compactNumber(compact, 'official_market_pnl_usdc');
  const hasMarketScopeFlag = hasOfficialMarketScopeFlag(dataQualityFlags);
  const hasMarketActivityEvidence = hasOfficialMarketActivityEvidence(compact);
  const useOfficialMarketPnl =
    hasMarketScopeFlag && hasMarketActivityEvidence && officialMarketPnlUsdc != null;
  const pnlSourceStatus: AutoScopeTradeAnalysisPnlSourceStatus =
    useOfficialMarketPnl
      ? 'activity_market'
      : officialMarketPnlUsdc != null && !hasMarketActivityEvidence
        ? 'local_fallback_no_activity_evidence'
        : 'local_fallback';
  const cashFillPnlUsdc =
    useOfficialMarketPnl
      ? officialMarketPnlUsdc
      : compactNumber(compact, 'cash_fill_pnl_usdc');
  return {
    cashFillPnlUsdc,
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
    officialMarketPnlUsdc,
    officialMarketBuyUsdc: compactNumber(compact, 'official_market_buy_usdc'),
    officialMarketSellUsdc: compactNumber(compact, 'official_market_sell_usdc'),
    officialMarketRedeemUsdc: compactNumber(compact, 'official_market_redeem_usdc'),
    officialVsRootDeltaUsdc: compactNumber(compact, 'official_vs_root_delta_usdc'),
    activityMarketPnlUsdc:
      hasMarketActivityEvidence && officialMarketPnlUsdc != null ? officialMarketPnlUsdc : null,
    positionMarketPnlUsdc: null,
    localMarketPnlUsdc: null,
    pnlSourceStatus,
    pendingInventoryQty: compactNumber(compact, 'pending_inventory_qty'),
    pendingInventoryValueUsdc: compactNumber(compact, 'pending_inventory_value_usdc'),
    pendingRedeemableValueUsdc: compactNumber(compact, 'pending_redeemable_value_usdc'),
    cashStatus: compactString(compact, 'cash_status'),
  };
}

function buildAutoScopeCashMetricsSummarySql(whereClause: string): string {
  return `WITH filtered_roots AS (
       SELECT DISTINCT s.root_builder_order_id, s.market_slug
       FROM trade_flow_auto_scope_analysis_rows s
       WHERE ${whereClause}
     ),
     root_metrics AS (
       SELECT
         r.market_slug,
         dg.root_builder_order_id,
         dg.compact_metrics_json,
         dg.data_quality_flags,
         COALESCE((dg.compact_metrics_json->>'cash_fill_pnl_usdc')::double precision, 0) AS cash_fill_pnl_usdc,
         (dg.compact_metrics_json->>'official_market_pnl_usdc')::double precision AS official_market_pnl_usdc,
         CASE
           WHEN ${OFFICIAL_MARKET_ACTIVITY_FLAG_SQL}
            AND ${OFFICIAL_MARKET_ACTIVITY_EVIDENCE_SQL}
            AND (dg.compact_metrics_json->>'official_market_pnl_usdc') IS NOT NULL
           THEN true
           ELSE false
         END AS use_market_pnl
       FROM filtered_roots r
       JOIN trade_flow_auto_scope_trade_diagnostics dg
         ON dg.root_builder_order_id = r.root_builder_order_id
     ),
     market_effective AS (
       SELECT
         market_slug,
         BOOL_OR(use_market_pnl) AS use_market_pnl,
         MAX(official_market_pnl_usdc) FILTER (WHERE use_market_pnl) AS official_market_pnl_usdc,
         SUM(cash_fill_pnl_usdc) AS cash_fill_pnl_usdc
       FROM root_metrics
       GROUP BY market_slug
     )
     SELECT
       COUNT(*)::int AS root_count,
       COUNT(*) FILTER (WHERE compact_metrics_json ? 'cash_fill_pnl_usdc')::int AS cash_metric_count,
       (SELECT COALESCE(SUM(
          CASE
            WHEN use_market_pnl AND official_market_pnl_usdc IS NOT NULL
            THEN official_market_pnl_usdc
            ELSE cash_fill_pnl_usdc
          END
        ), 0)::double precision FROM market_effective) AS local_cash_fill_pnl_usdc,
       COALESCE(SUM(COALESCE((compact_metrics_json->>'diagnostic_pnl_usdc')::double precision, 0)), 0)::double precision AS diagnostic_pnl_usdc,
       COALESCE(SUM(COALESCE((compact_metrics_json->>'economic_pnl_usdc')::double precision, 0)), 0)::double precision AS economic_pnl_usdc,
       COALESCE(SUM(COALESCE((compact_metrics_json->>'pending_inventory_value_usdc')::double precision, 0)), 0)::double precision AS pending_inventory_value_usdc,
       COALESCE(SUM(COALESCE((compact_metrics_json->>'pending_redeemable_value_usdc')::double precision, 0)), 0)::double precision AS pending_redeemable_value_usdc
     FROM root_metrics`;
}

export async function getAutoScopeCashMetricsSummaryForWhere({
  whereClause,
  params,
}: {
  whereClause: string;
  params: Array<number | string | null>;
}): Promise<Partial<AutoScopeTradeAnalysisSummary>> {
  const result = await pool.query<AutoScopeCashSummaryDb>(
    buildAutoScopeCashMetricsSummarySql(whereClause),
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

export const __autoScopeCashMetricsTestUtils = {
  buildAutoScopeCashMetricsSummarySql,
};
