import { pool } from '@/lib/db';
import type {
  AutoScopeTradeAnalysisPnlFilter,
  AutoScopeTradeAnalysisPositionFilter,
  AutoScopeTradeAnalysisRow,
} from '@/lib/types';
import type { AutoScopeTradeAnalysisFilters } from './analytics';

interface PendingAutoScopeTradeAnalysisRowDb {
  definition_id: number;
  definition_name: string | null;
  run_id: number;
  root_builder_order_id: number;
  market_slug: string;
  token_id: string;
  outcome_label: string;
  filled_qty: number;
  working_price: number | null;
  last_seen_price: number | null;
  filled_at: string | null;
  updated_at: string;
}

function numberOrNull(value: number | null): number | null {
  return value == null ? null : Number(value);
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
  return durationMs == null ? null : new Date(startMs + durationMs).toISOString();
}

function mapPendingAnalysisRow(
  row: PendingAutoScopeTradeAnalysisRowDb
): AutoScopeTradeAnalysisRow {
  return {
    rowId: `pending:${row.root_builder_order_id}`,
    rowType: 'pending_analysis',
    positionState: 'pending_analysis',
    definitionId: Number(row.definition_id),
    definitionName: row.definition_name,
    runId: Number(row.run_id),
    rootOrderId: Number(row.root_builder_order_id),
    exitOrderId: null,
    marketSlug: row.market_slug,
    tokenId: row.token_id,
    outcomeLabel: row.outcome_label,
    exitReason: 'pending_analysis',
    marketEndAt: deriveMarketEndAtFromSlug(row.market_slug),
    marketOpenAt: null,
    triggeredAt: null,
    buyFilledAt: row.filled_at,
    sellFilledAt: null,
    markPriceCapturedAt: row.updated_at,
    openToTriggerMs: null,
    triggerToBuyFillMs: null,
    buyAvgPrice: null,
    sellOrLivePrice: numberOrNull(row.last_seen_price) ?? numberOrNull(row.working_price),
    rowQty: Number(row.filled_qty || 0),
    remainingQtyAfterExit: Number(row.filled_qty || 0),
    rowPnlUsdc: 0,
    buyNotionalUsdc: null,
    buyFeeUsdc: null,
    costBasisUsdc: null,
    sellNotionalUsdc: null,
    sellFeeUsdc: null,
    markValueUsdc: null,
    netValueUsdc: null,
    pnlPct: null,
    valuationKind: null,
    primaryDiagnosisCode: 'unknown',
    diagnosisLabel: 'Pending Analysis',
    entryQualityScore: null,
    exitQualityScore: null,
  };
}

export async function getPendingAutoScopeAnalysisRows(
  filters: AutoScopeTradeAnalysisFilters,
  limit: number,
  pnlFilter: AutoScopeTradeAnalysisPnlFilter,
  positionFilter: AutoScopeTradeAnalysisPositionFilter
): Promise<AutoScopeTradeAnalysisRow[]> {
  if (limit <= 0 || pnlFilter !== 'all' || positionFilter === 'realized') {
    return [];
  }

  const params: Array<number | string> = [filters.userId];
  const conditions = [
    'o.user_id = $1',
    "o.side = 'buy'",
    'o.parent_order_id IS NULL',
    'o.origin_flow_run_id IS NOT NULL',
    'o.origin_flow_definition_id IS NOT NULL',
    `(
       COALESCE(o.filled_qty, 0) > 0
       OR fill_log.last_fill_ts IS NOT NULL
       OR EXISTS (
         SELECT 1
         FROM trade_builder_order_events e
         WHERE e.builder_order_id = o.id
           AND e.event_type = 'filled'
       )
     )`,
    `NOT EXISTS (
       SELECT 1
       FROM trade_flow_auto_scope_analysis_rows s
       WHERE s.root_builder_order_id = o.id
     )`,
  ];

  if (filters.from) {
    params.push(filters.from);
    conditions.push(
      `COALESCE(fill_log.last_fill_ts, o.updated_at, o.created_at) >= $${params.length}::timestamptz`
    );
  }
  if (filters.to) {
    params.push(filters.to);
    conditions.push(
      `COALESCE(fill_log.last_fill_ts, o.updated_at, o.created_at) <= $${params.length}::timestamptz`
    );
  }
  params.push(limit);

  const rows = await pool.query<PendingAutoScopeTradeAnalysisRowDb>(
    `WITH fill_log AS (
       SELECT
         root_order_id::bigint AS root_builder_order_id,
         MAX(event_ts) AS last_fill_ts
       FROM bot_decision_logs
       WHERE event_type = 'ORDER_FILLED'
         AND root_order_id ~ '^[0-9]+$'
       GROUP BY root_order_id::bigint
     )
     SELECT
       o.origin_flow_definition_id AS definition_id,
       d.name AS definition_name,
       o.origin_flow_run_id AS run_id,
       o.id AS root_builder_order_id,
       o.market_slug,
       o.token_id,
       o.outcome_label,
       COALESCE(o.filled_qty, 0)::double precision AS filled_qty,
       o.working_price::double precision AS working_price,
       o.last_seen_price::double precision AS last_seen_price,
       fill_log.last_fill_ts::text AS filled_at,
       COALESCE(fill_log.last_fill_ts, o.updated_at, o.created_at)::text AS updated_at
     FROM trade_builder_orders o
     LEFT JOIN fill_log ON fill_log.root_builder_order_id = o.id
     LEFT JOIN trade_flow_definitions d ON d.id = o.origin_flow_definition_id
     WHERE ${conditions.join('\n       AND ')}
     ORDER BY COALESCE(fill_log.last_fill_ts, o.updated_at, o.created_at) DESC, o.id DESC
     LIMIT $${params.length}`,
    params
  );

  return rows.rows.map(mapPendingAnalysisRow);
}
