import { pool } from '@/lib/db';
import type {
  AutoScopeTradeAnalysisPositionState,
  AutoScopeTradeAnalysisResponse,
  AutoScopeTradeAnalysisRow,
  AutoScopeTradeAnalysisSortBy,
  AutoScopeTradeAnalysisSortDirection,
} from '@/lib/types';

interface AutoScopeTradeAnalysisFilters {
  userId: number;
  page?: number;
  limit?: number;
  sortBy?: AutoScopeTradeAnalysisSortBy;
  sortDirection?: AutoScopeTradeAnalysisSortDirection;
}

interface AutoScopeTradeAnalysisRowDb {
  row_key: string;
  definition_id: number;
  definition_name: string | null;
  run_id: number;
  root_builder_order_id: number;
  exit_builder_order_id: number | null;
  row_type: 'sell_exit' | 'open_position';
  market_slug: string;
  token_id: string;
  outcome_label: string;
  exit_reason: 'tp' | 'sl' | 'window_end_auto_sell' | 'other' | 'open_position';
  market_end_at: string | null;
  market_open_at: string | null;
  triggered_at: string | null;
  buy_filled_at: string | null;
  sell_filled_at: string | null;
  open_to_trigger_ms: number | null;
  trigger_to_buy_fill_ms: number | null;
  buy_avg_price: number | null;
  mark_or_sell_price: number | null;
  row_qty: number;
  remaining_qty_after_exit: number;
  row_pnl_usdc: number;
  updated_at: string;
}

function parseDate(value: string | null): Date | null {
  if (!value) return null;
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

function deriveMarketEndAtFromSlug(marketSlug: string): string | null {
  const trimmed = marketSlug.trim().toLowerCase();
  const unixSuffix = trimmed.split('-').at(-1);
  if (!unixSuffix || !/^\d{9,13}$/.test(unixSuffix)) return null;

  const raw = Number(unixSuffix);
  if (!Number.isFinite(raw) || raw <= 0) return null;

  const startMs = raw > 10_000_000_000 ? raw : raw * 1000;
  const durationMs = trimmed.includes('-15m-')
    ? 15 * 60 * 1000
    : trimmed.includes('-5m-')
      ? 5 * 60 * 1000
      : null;
  if (durationMs == null) return null;

  return new Date(startMs + durationMs).toISOString();
}

function derivePositionState(
  rowType: AutoScopeTradeAnalysisRowDb['row_type'],
  marketEndAt: string | null,
  nowIso: string
): AutoScopeTradeAnalysisPositionState {
  if (rowType === 'sell_exit') return 'closed_exit';
  const marketEnd = parseDate(marketEndAt);
  const now = parseDate(nowIso);
  if (marketEnd && now && now >= marketEnd) {
    return 'closed_market_ended';
  }
  return 'open';
}

function mapAnalysisRow(row: AutoScopeTradeAnalysisRowDb): AutoScopeTradeAnalysisRow {
  const marketEndAt = row.market_end_at ?? deriveMarketEndAtFromSlug(row.market_slug);
  const nowIso = new Date().toISOString();
  return {
    rowId: row.row_key,
    rowType: row.row_type,
    positionState: derivePositionState(row.row_type, marketEndAt, nowIso),
    definitionId: Number(row.definition_id),
    definitionName: row.definition_name,
    runId: Number(row.run_id),
    rootOrderId: Number(row.root_builder_order_id),
    exitOrderId:
      row.exit_builder_order_id == null ? null : Number(row.exit_builder_order_id),
    marketSlug: row.market_slug,
    tokenId: row.token_id,
    outcomeLabel: row.outcome_label,
    exitReason: row.exit_reason,
    marketEndAt,
    marketOpenAt: row.market_open_at,
    triggeredAt: row.triggered_at,
    buyFilledAt: row.buy_filled_at,
    sellFilledAt: row.sell_filled_at,
    openToTriggerMs:
      row.open_to_trigger_ms == null ? null : Number(row.open_to_trigger_ms),
    triggerToBuyFillMs:
      row.trigger_to_buy_fill_ms == null ? null : Number(row.trigger_to_buy_fill_ms),
    buyAvgPrice: row.buy_avg_price == null ? null : Number(row.buy_avg_price),
    sellOrLivePrice:
      row.mark_or_sell_price == null ? null : Number(row.mark_or_sell_price),
    rowQty: Number(row.row_qty),
    remainingQtyAfterExit: Number(row.remaining_qty_after_exit),
    rowPnlUsdc: Number(row.row_pnl_usdc),
  };
}

function normalizeSortBy(value: string | undefined): AutoScopeTradeAnalysisSortBy {
  return value === 'pnl' ? 'pnl' : 'default';
}

function normalizeSortDirection(value: string | undefined): AutoScopeTradeAnalysisSortDirection {
  return value === 'asc' ? 'asc' : 'desc';
}

function buildOrderByClause(
  sortBy: AutoScopeTradeAnalysisSortBy,
  sortDirection: AutoScopeTradeAnalysisSortDirection
): string {
  if (sortBy === 'pnl') {
    const pnlDirection = sortDirection === 'asc' ? 'ASC' : 'DESC';
    return `s.row_pnl_usdc ${pnlDirection},
            s.triggered_at DESC NULLS LAST,
            s.sell_filled_at DESC NULLS LAST,
            s.root_builder_order_id DESC,
            s.row_key ASC`;
  }

  return `s.triggered_at DESC NULLS LAST,
          s.sell_filled_at DESC NULLS LAST,
          s.root_builder_order_id DESC,
          s.row_key ASC`;
}

export async function getAutoScopeTradeAnalysis(
  filters: AutoScopeTradeAnalysisFilters
): Promise<AutoScopeTradeAnalysisResponse> {
  const page = Math.max(1, filters.page || 1);
  const limit = Math.min(Math.max(1, filters.limit || 50), 100);
  const offset = (page - 1) * limit;
  const sortBy = normalizeSortBy(filters.sortBy);
  const sortDirection = normalizeSortDirection(filters.sortDirection);
  const orderByClause = buildOrderByClause(sortBy, sortDirection);

  const [countRes, refreshedAtRes, dataRes] = await Promise.all([
    pool.query<{ total: number }>(
      `SELECT COUNT(*)::int AS total
       FROM trade_flow_auto_scope_analysis_rows
       WHERE user_id = $1`,
      [filters.userId]
    ),
    pool.query<{ refreshed_at: string | null }>(
      `SELECT MAX(updated_at)::text AS refreshed_at
       FROM trade_flow_auto_scope_analysis_rows
       WHERE user_id = $1`,
      [filters.userId]
    ),
    pool.query<AutoScopeTradeAnalysisRowDb>(
      `SELECT
         s.row_key,
         s.definition_id,
         d.name AS definition_name,
         s.run_id,
         s.root_builder_order_id,
         s.exit_builder_order_id,
         s.row_type,
         s.market_slug,
         s.token_id,
         s.outcome_label,
         s.exit_reason,
         m.ends_at::text AS market_end_at,
         s.market_open_at::text,
         s.triggered_at::text,
         s.buy_filled_at::text,
         s.sell_filled_at::text,
         s.open_to_trigger_ms,
         s.trigger_to_buy_fill_ms,
         s.buy_avg_price,
         s.mark_or_sell_price,
         s.row_qty,
         s.remaining_qty_after_exit,
         s.row_pnl_usdc,
         s.updated_at::text
       FROM trade_flow_auto_scope_analysis_rows s
       LEFT JOIN trade_flow_definitions d ON d.id = s.definition_id
       LEFT JOIN markets m ON m.market_slug = s.market_slug
       WHERE s.user_id = $1
       ORDER BY ${orderByClause}
       LIMIT $2 OFFSET $3`,
      [filters.userId, limit, offset]
    ),
  ]);

  const total = Number(countRes.rows[0]?.total || 0);

  return {
    data: dataRes.rows.map(mapAnalysisRow),
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
    sortBy,
    sortDirection,
    refreshedAt:
      refreshedAtRes.rows[0]?.refreshed_at ?? new Date().toISOString(),
  };
}

export const __analyticsTestUtils = {
  deriveMarketEndAtFromSlug,
  derivePositionState,
  buildOrderByClause,
};
