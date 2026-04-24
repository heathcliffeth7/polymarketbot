import { pool } from '@/lib/db';
import type {
  AutoScopeTradeAnalysisPnlFilter,
  AutoScopeTradeAnalysisPositionState,
  AutoScopeTradeAnalysisPositionFilter,
  AutoScopeTradeAnalysisResponse,
  AutoScopeTradeAnalysisRow,
  AutoScopeTradeAnalysisSortBy,
  AutoScopeTradeAnalysisSortDirection,
  AutoScopeTradeAnalysisSummary,
  TradeFlowNodeRuntimeResponse,
  TradeFlowNodeRuntimeRow,
  TradeFlowPtbStateResponse,
  TradeFlowPtbStateRow,
} from '@/lib/types';

export interface AutoScopeTradeAnalysisFilters {
  userId: number;
  page?: number;
  limit?: number;
  sortBy?: AutoScopeTradeAnalysisSortBy;
  sortDirection?: AutoScopeTradeAnalysisSortDirection;
  pnl?: AutoScopeTradeAnalysisPnlFilter;
  position?: AutoScopeTradeAnalysisPositionFilter;
  from?: string | null;
  to?: string | null;
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
  mark_price_captured_at: string | null;
  open_to_trigger_ms: number | null;
  trigger_to_buy_fill_ms: number | null;
  buy_avg_price: number | null;
  mark_or_sell_price: number | null;
  row_qty: number;
  remaining_qty_after_exit: number;
  row_pnl_usdc: number;
  buy_notional_usdc: number | null;
  buy_fee_usdc: number | null;
  cost_basis_usdc: number | null;
  sell_notional_usdc: number | null;
  sell_fee_usdc: number | null;
  mark_value_usdc: number | null;
  net_value_usdc: number | null;
  pnl_pct: number | null;
  valuation_kind: 'realized' | 'mark_to_market' | null;
  updated_at: string;
}

interface AutoScopeTradeAnalysisSummaryDb {
  row_count: number;
  market_count: number;
  loss_count: number;
  profit_count: number;
  total_pnl_usdc: number | null;
  realized_pnl_usdc: number | null;
  open_pnl_usdc: number | null;
  loss_usdc: number | null;
  profit_usdc: number | null;
  buy_fee_usdc: number | null;
  sell_fee_usdc: number | null;
  total_fee_usdc: number | null;
  cost_basis_usdc: number | null;
  net_value_usdc: number | null;
}

interface TradeFlowPtbStateRowDb {
  builder_order_id: number;
  run_id: number | null;
  node_key: string | null;
  market_slug: string;
  outcome_label: string;
  base_threshold_usd: number | null;
  bump_usd: number | null;
  bump_increment_usd: number | null;
  relax_credit_usd: number | null;
  effective_threshold_usd: number | null;
  guard_miss_reason: string | null;
  max_price_miss: boolean | null;
  first_tradable_second: string | null;
  first_tradable_gap_usd: number | null;
  tradable_seconds_count: number | null;
  price_ok_depth_fail_count: number | null;
  max_fillability_score: number | null;
  quality_score: number | null;
  refreshed_at: string;
}

interface TradeFlowNodeRuntimeRowDb {
  run_id: number;
  definition_id: number;
  version_id: number | null;
  node_key: string;
  node_type: string;
  status: string;
  state_kind: string;
  market_slug: string | null;
  token_id: string | null;
  snapshot_json: Record<string, unknown>;
  updated_at: string;
}

const ANALYSIS_FILTER_TIME_EXPR =
  'COALESCE(s.triggered_at, s.buy_filled_at, s.sell_filled_at, s.updated_at)';

function parseDate(value: string | null): Date | null {
  if (!value) return null;
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

function numberOrNull(value: number | null): number | null {
  return value == null ? null : Number(value);
}

function normalizePnlFilter(value: string | undefined): AutoScopeTradeAnalysisPnlFilter {
  return value === 'loss' || value === 'profit' ? value : 'all';
}

function normalizePositionFilter(
  value: string | undefined
): AutoScopeTradeAnalysisPositionFilter {
  return value === 'realized' || value === 'open' ? value : 'all';
}

function buildAnalysisWhereClause(filters: AutoScopeTradeAnalysisFilters): {
  whereClause: string;
  params: Array<number | string | null>;
} {
  const params: Array<number | string | null> = [filters.userId];
  const conditions = ['s.user_id = $1'];
  const pnlFilter = normalizePnlFilter(filters.pnl);
  const positionFilter = normalizePositionFilter(filters.position);

  if (pnlFilter === 'loss') {
    conditions.push('s.row_pnl_usdc < 0');
  } else if (pnlFilter === 'profit') {
    conditions.push('s.row_pnl_usdc > 0');
  }

  if (positionFilter === 'realized') {
    conditions.push("s.row_type = 'sell_exit'");
  } else if (positionFilter === 'open') {
    conditions.push("s.row_type = 'open_position'");
  }

  if (filters.from) {
    params.push(filters.from);
    conditions.push(`${ANALYSIS_FILTER_TIME_EXPR} >= $${params.length}::timestamptz`);
  }
  if (filters.to) {
    params.push(filters.to);
    conditions.push(`${ANALYSIS_FILTER_TIME_EXPR} <= $${params.length}::timestamptz`);
  }

  return {
    whereClause: conditions.join('\n         AND '),
    params,
  };
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
    markPriceCapturedAt: row.mark_price_captured_at,
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
    buyNotionalUsdc: numberOrNull(row.buy_notional_usdc),
    buyFeeUsdc: numberOrNull(row.buy_fee_usdc),
    costBasisUsdc: numberOrNull(row.cost_basis_usdc),
    sellNotionalUsdc: numberOrNull(row.sell_notional_usdc),
    sellFeeUsdc: numberOrNull(row.sell_fee_usdc),
    markValueUsdc: numberOrNull(row.mark_value_usdc),
    netValueUsdc: numberOrNull(row.net_value_usdc),
    pnlPct: numberOrNull(row.pnl_pct),
    valuationKind: row.valuation_kind,
  };
}

function mapAnalysisSummary(row: AutoScopeTradeAnalysisSummaryDb | undefined): AutoScopeTradeAnalysisSummary {
  return {
    rowCount: Number(row?.row_count || 0),
    marketCount: Number(row?.market_count || 0),
    lossCount: Number(row?.loss_count || 0),
    profitCount: Number(row?.profit_count || 0),
    totalPnlUsdc: Number(row?.total_pnl_usdc || 0),
    realizedPnlUsdc: Number(row?.realized_pnl_usdc || 0),
    openPnlUsdc: Number(row?.open_pnl_usdc || 0),
    lossUsdc: Number(row?.loss_usdc || 0),
    profitUsdc: Number(row?.profit_usdc || 0),
    buyFeeUsdc: Number(row?.buy_fee_usdc || 0),
    sellFeeUsdc: Number(row?.sell_fee_usdc || 0),
    totalFeeUsdc: Number(row?.total_fee_usdc || 0),
    costBasisUsdc: Number(row?.cost_basis_usdc || 0),
    netValueUsdc: Number(row?.net_value_usdc || 0),
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

const ANALYSIS_ROW_SELECT = `SELECT
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
         s.mark_price_captured_at::text,
         s.open_to_trigger_ms,
         s.trigger_to_buy_fill_ms,
         s.buy_avg_price,
         s.mark_or_sell_price,
         s.row_qty,
         s.remaining_qty_after_exit,
         s.row_pnl_usdc,
         s.buy_notional_usdc,
         s.buy_fee_usdc,
         s.cost_basis_usdc,
         s.sell_notional_usdc,
         s.sell_fee_usdc,
         s.mark_value_usdc,
         s.net_value_usdc,
         s.pnl_pct,
         s.valuation_kind,
         s.updated_at::text
       FROM trade_flow_auto_scope_analysis_rows s
       LEFT JOIN trade_flow_definitions d ON d.id = s.definition_id
       LEFT JOIN markets m ON m.market_slug = s.market_slug`;

export async function getAutoScopeTradeAnalysis(
  filters: AutoScopeTradeAnalysisFilters
): Promise<AutoScopeTradeAnalysisResponse> {
  const page = Math.max(1, filters.page || 1);
  const limit = Math.min(Math.max(1, filters.limit || 50), 100);
  const offset = (page - 1) * limit;
  const sortBy = normalizeSortBy(filters.sortBy);
  const sortDirection = normalizeSortDirection(filters.sortDirection);
  const pnlFilter = normalizePnlFilter(filters.pnl);
  const positionFilter = normalizePositionFilter(filters.position);
  const orderByClause = buildOrderByClause(sortBy, sortDirection);
  const where = buildAnalysisWhereClause({
    ...filters,
    pnl: pnlFilter,
    position: positionFilter,
  });
  const limitParam = where.params.length + 1;
  const offsetParam = where.params.length + 2;

  const [countRes, refreshedAtRes, summaryRes, dataRes] = await Promise.all([
    pool.query<{ total: number }>(
      `SELECT COUNT(*)::int AS total
       FROM trade_flow_auto_scope_analysis_rows s
       WHERE ${where.whereClause}`,
      where.params
    ),
    pool.query<{ refreshed_at: string | null }>(
      `SELECT MAX(updated_at)::text AS refreshed_at
       FROM trade_flow_auto_scope_analysis_rows s
       WHERE ${where.whereClause}`,
      where.params
    ),
    pool.query<AutoScopeTradeAnalysisSummaryDb>(
      `SELECT
         COUNT(*)::int AS row_count,
         COUNT(DISTINCT (s.market_slug, s.root_builder_order_id))::int AS market_count,
         COUNT(*) FILTER (WHERE s.row_pnl_usdc < 0)::int AS loss_count,
         COUNT(*) FILTER (WHERE s.row_pnl_usdc > 0)::int AS profit_count,
         COALESCE(SUM(s.row_pnl_usdc), 0)::double precision AS total_pnl_usdc,
         COALESCE(SUM(s.row_pnl_usdc) FILTER (WHERE s.row_type = 'sell_exit'), 0)::double precision AS realized_pnl_usdc,
         COALESCE(SUM(s.row_pnl_usdc) FILTER (WHERE s.row_type = 'open_position'), 0)::double precision AS open_pnl_usdc,
         ABS(COALESCE(SUM(s.row_pnl_usdc) FILTER (WHERE s.row_pnl_usdc < 0), 0))::double precision AS loss_usdc,
         COALESCE(SUM(s.row_pnl_usdc) FILTER (WHERE s.row_pnl_usdc > 0), 0)::double precision AS profit_usdc,
         COALESCE(SUM(s.buy_fee_usdc), 0)::double precision AS buy_fee_usdc,
         COALESCE(SUM(s.sell_fee_usdc), 0)::double precision AS sell_fee_usdc,
         COALESCE(SUM(COALESCE(s.buy_fee_usdc, 0) + COALESCE(s.sell_fee_usdc, 0)), 0)::double precision AS total_fee_usdc,
         COALESCE(SUM(s.cost_basis_usdc), 0)::double precision AS cost_basis_usdc,
         COALESCE(SUM(s.net_value_usdc), 0)::double precision AS net_value_usdc
       FROM trade_flow_auto_scope_analysis_rows s
       WHERE ${where.whereClause}`,
      where.params
    ),
    pool.query<AutoScopeTradeAnalysisRowDb>(
      `${ANALYSIS_ROW_SELECT}
       WHERE ${where.whereClause}
       ORDER BY ${orderByClause}
       LIMIT $${limitParam} OFFSET $${offsetParam}`,
      [...where.params, limit, offset]
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
    pnlFilter,
    positionFilter,
    from: filters.from ?? null,
    to: filters.to ?? null,
    summary: mapAnalysisSummary(summaryRes.rows[0]),
    refreshedAt:
      refreshedAtRes.rows[0]?.refreshed_at ?? new Date().toISOString(),
  };
}

export async function getAutoScopeTradeAnalysisRowsForExport(
  filters: AutoScopeTradeAnalysisFilters
): Promise<AutoScopeTradeAnalysisRow[]> {
  const sortBy = normalizeSortBy(filters.sortBy);
  const sortDirection = normalizeSortDirection(filters.sortDirection);
  const pnlFilter = normalizePnlFilter(filters.pnl);
  const positionFilter = normalizePositionFilter(filters.position);
  const where = buildAnalysisWhereClause({
    ...filters,
    pnl: pnlFilter,
    position: positionFilter,
  });

  const rows = await pool.query<AutoScopeTradeAnalysisRowDb>(
    `${ANALYSIS_ROW_SELECT}
     WHERE ${where.whereClause}
     ORDER BY ${buildOrderByClause(sortBy, sortDirection)}`,
    where.params
  );

  return rows.rows.map(mapAnalysisRow);
}

function csvField(value: string | number | null): string {
  if (value == null) return '';
  const text = String(value);
  if (!/[",\r\n]/.test(text)) return text;
  return `"${text.replaceAll('"', '""')}"`;
}

export function buildAutoScopeTradeAnalysisCsv(
  rows: AutoScopeTradeAnalysisRow[]
): string {
  const headers = [
    'workflow',
    'definition_id',
    'run_id',
    'root_order_id',
    'exit_order_id',
    'row_type',
    'position_state',
    'valuation_kind',
    'market_slug',
    'token_id',
    'outcome_label',
    'exit_reason',
    'market_open_at',
    'market_end_at',
    'triggered_at',
    'buy_filled_at',
    'sell_filled_at',
    'mark_price_captured_at',
    'open_to_trigger_ms',
    'trigger_to_buy_fill_ms',
    'buy_avg_price',
    'sell_or_live_price',
    'row_qty',
    'remaining_qty_after_exit',
    'buy_notional_usdc',
    'buy_fee_usdc',
    'cost_basis_usdc',
    'sell_notional_usdc',
    'sell_fee_usdc',
    'mark_value_usdc',
    'net_value_usdc',
    'row_pnl_usdc',
    'pnl_pct',
  ];
  const lines = [headers.map(csvField).join(',')];

  for (const row of rows) {
    lines.push(
      [
        row.definitionName ?? '',
        row.definitionId,
        row.runId,
        row.rootOrderId,
        row.exitOrderId,
        row.rowType,
        row.positionState,
        row.valuationKind,
        row.marketSlug,
        row.tokenId,
        row.outcomeLabel,
        row.exitReason,
        row.marketOpenAt,
        row.marketEndAt,
        row.triggeredAt,
        row.buyFilledAt,
        row.sellFilledAt,
        row.markPriceCapturedAt,
        row.openToTriggerMs,
        row.triggerToBuyFillMs,
        row.buyAvgPrice,
        row.sellOrLivePrice,
        row.rowQty,
        row.remainingQtyAfterExit,
        row.buyNotionalUsdc,
        row.buyFeeUsdc,
        row.costBasisUsdc,
        row.sellNotionalUsdc,
        row.sellFeeUsdc,
        row.markValueUsdc,
        row.netValueUsdc,
        row.rowPnlUsdc,
        row.pnlPct,
      ]
        .map(csvField)
        .join(',')
    );
  }

  return `${lines.join('\n')}\n`;
}

function mapPtbStateRow(row: TradeFlowPtbStateRowDb): TradeFlowPtbStateRow {
  return {
    builderOrderId: Number(row.builder_order_id),
    runId: row.run_id == null ? null : Number(row.run_id),
    nodeKey: row.node_key,
    marketSlug: row.market_slug,
    outcomeLabel: row.outcome_label,
    baseThresholdUsd:
      row.base_threshold_usd == null ? null : Number(row.base_threshold_usd),
    bumpUsd: row.bump_usd == null ? null : Number(row.bump_usd),
    bumpIncrementUsd:
      row.bump_increment_usd == null ? null : Number(row.bump_increment_usd),
    relaxCreditUsd:
      row.relax_credit_usd == null ? null : Number(row.relax_credit_usd),
    effectiveThresholdUsd:
      row.effective_threshold_usd == null ? null : Number(row.effective_threshold_usd),
    guardMissReason: row.guard_miss_reason,
    maxPriceMiss: row.max_price_miss === true,
    firstTradableSecond: row.first_tradable_second,
    firstTradableGapUsd:
      row.first_tradable_gap_usd == null ? null : Number(row.first_tradable_gap_usd),
    tradableSecondsCount: Number(row.tradable_seconds_count || 0),
    priceOkDepthFailCount: Number(row.price_ok_depth_fail_count || 0),
    maxFillabilityScore:
      row.max_fillability_score == null ? null : Number(row.max_fillability_score),
    qualityScore: row.quality_score == null ? null : Number(row.quality_score),
    refreshedAt: row.refreshed_at,
  };
}

export async function getTradeFlowPtbState(params: {
  userId: number;
  runId?: number | null;
  page?: number;
  limit?: number;
}): Promise<TradeFlowPtbStateResponse> {
  const page = Math.max(1, params.page || 1);
  const limit = Math.min(Math.max(1, params.limit || 50), 100);
  const offset = (page - 1) * limit;
  const runId = params.runId ?? null;

  const [countRes, dataRes] = await Promise.all([
    pool.query<{ total: number }>(
      `WITH latest AS (
         SELECT DISTINCT ON (e.builder_order_id)
           e.builder_order_id,
           o.origin_flow_run_id AS run_id,
           o.origin_flow_node_key AS node_key,
           o.market_slug,
           o.outcome_label,
           e.payload_json,
           e.created_at
         FROM trade_builder_order_events e
         JOIN trade_builder_orders o ON o.id = e.builder_order_id
         WHERE o.user_id = $1
           AND ($2::bigint IS NULL OR o.origin_flow_run_id = $2)
           AND e.event_type IN ('flow_created', 'flow_rearmed')
         ORDER BY e.builder_order_id, e.created_at DESC, e.id DESC
       )
       SELECT COUNT(*)::int AS total FROM latest`,
      [params.userId, runId]
    ),
    pool.query<TradeFlowPtbStateRowDb>(
      `WITH latest AS (
         SELECT DISTINCT ON (e.builder_order_id)
           e.builder_order_id,
           o.origin_flow_run_id AS run_id,
           o.origin_flow_node_key AS node_key,
           o.market_slug,
           o.outcome_label,
           e.payload_json,
           e.created_at
         FROM trade_builder_order_events e
         JOIN trade_builder_orders o ON o.id = e.builder_order_id
         WHERE o.user_id = $1
           AND ($2::bigint IS NULL OR o.origin_flow_run_id = $2)
           AND e.event_type IN ('flow_created', 'flow_rearmed')
         ORDER BY e.builder_order_id, e.created_at DESC, e.id DESC
       )
       SELECT
         builder_order_id,
         run_id,
         node_key,
         market_slug,
         outcome_label,
         NULLIF(payload_json->'price_to_beat_guard'->>'base_threshold_usd', '')::double precision AS base_threshold_usd,
         NULLIF(payload_json->'price_to_beat_guard'->>'stop_loss_bump_usd', '')::double precision AS bump_usd,
         NULLIF(payload_json->'price_to_beat_guard'->>'stop_loss_bump_increment_usd', '')::double precision AS bump_increment_usd,
         NULLIF(payload_json->'price_to_beat_guard'->'max_price_relax'->>'max_price_relax_relax_credit_usd', '')::double precision AS relax_credit_usd,
         NULLIF(payload_json->'price_to_beat_guard'->>'threshold_usd', '')::double precision AS effective_threshold_usd,
         COALESCE(
           payload_json->'price_to_beat_guard'->'max_price_relax'->>'max_price_relax_miss_reason',
           payload_json->'price_to_beat_guard'->>'reason_code'
         ) AS guard_miss_reason,
         COALESCE(
           payload_json->'price_to_beat_guard'->'max_price_relax'->>'max_price_relax_miss_reason' = 'max_price_miss',
           false
         ) AS max_price_miss,
         payload_json->'price_to_beat_guard'->'max_price_relax'->>'max_price_relax_first_tradable_second_ts' AS first_tradable_second,
         NULLIF(payload_json->'price_to_beat_guard'->'max_price_relax'->>'max_price_relax_selected_gap_usd', '')::double precision AS first_tradable_gap_usd,
         COALESCE((payload_json->'price_to_beat_guard'->'max_price_relax'->>'max_price_relax_tradable_seconds_count')::int, 0) AS tradable_seconds_count,
         COALESCE((payload_json->'price_to_beat_guard'->'max_price_relax'->>'max_price_relax_price_ok_depth_fail_count')::int, 0) AS price_ok_depth_fail_count,
         NULLIF(payload_json->'price_to_beat_guard'->'max_price_relax'->>'max_price_relax_max_fillability_score', '')::double precision AS max_fillability_score,
         NULLIF(payload_json->'price_to_beat_guard'->'max_price_relax'->>'max_price_relax_quality_score', '')::double precision AS quality_score,
         created_at::text AS refreshed_at
       FROM latest
       ORDER BY created_at DESC, builder_order_id DESC
       LIMIT $3 OFFSET $4`,
      [params.userId, runId, limit, offset]
    ),
  ]);

  const total = Number(countRes.rows[0]?.total || 0);
  return {
    data: dataRes.rows.map(mapPtbStateRow),
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
    refreshedAt: dataRes.rows[0]?.refreshed_at ?? new Date().toISOString(),
  };
}

function mapNodeRuntimeRow(row: TradeFlowNodeRuntimeRowDb): TradeFlowNodeRuntimeRow {
  return {
    runId: Number(row.run_id),
    definitionId: Number(row.definition_id),
    versionId: row.version_id == null ? null : Number(row.version_id),
    nodeKey: row.node_key,
    nodeType: row.node_type,
    status: row.status,
    stateKind: row.state_kind,
    marketSlug: row.market_slug,
    tokenId: row.token_id,
    snapshotJson: row.snapshot_json ?? {},
    updatedAt: row.updated_at,
  };
}

export async function getTradeFlowNodeRuntime(params: {
  userId: number;
  runId: number;
  nodeKey?: string;
  nodeType?: string;
  page?: number;
  limit?: number;
}): Promise<TradeFlowNodeRuntimeResponse> {
  const page = Math.max(1, params.page || 1);
  const limit = Math.min(Math.max(1, params.limit || 50), 100);
  const offset = (page - 1) * limit;

  const [countRes, dataRes] = await Promise.all([
    pool.query<{ total: number }>(
      `SELECT COUNT(*)::int AS total
       FROM trade_flow_node_runtime_snapshots s
       JOIN trade_flow_runs r ON r.id = s.run_id
       WHERE r.user_id = $1
         AND s.run_id = $2
         AND ($3::text IS NULL OR s.node_key = $3)
         AND ($4::text IS NULL OR s.node_type = $4)`,
      [params.userId, params.runId, params.nodeKey ?? null, params.nodeType ?? null]
    ),
    pool.query<TradeFlowNodeRuntimeRowDb>(
      `SELECT
         s.run_id,
         s.definition_id,
         s.version_id,
         s.node_key,
         s.node_type,
         s.status,
         s.state_kind,
         s.market_slug,
         s.token_id,
         s.snapshot_json,
         s.updated_at::text
       FROM trade_flow_node_runtime_snapshots s
       JOIN trade_flow_runs r ON r.id = s.run_id
       WHERE r.user_id = $1
         AND s.run_id = $2
         AND ($3::text IS NULL OR s.node_key = $3)
         AND ($4::text IS NULL OR s.node_type = $4)
       ORDER BY s.updated_at DESC, s.node_key ASC
       LIMIT $5 OFFSET $6`,
      [
        params.userId,
        params.runId,
        params.nodeKey ?? null,
        params.nodeType ?? null,
        limit,
        offset,
      ]
    ),
  ]);

  const total = Number(countRes.rows[0]?.total || 0);
  return {
    data: dataRes.rows.map(mapNodeRuntimeRow),
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
    refreshedAt: dataRes.rows[0]?.updated_at ?? new Date().toISOString(),
  };
}

export const __analyticsTestUtils = {
  deriveMarketEndAtFromSlug,
  derivePositionState,
  buildOrderByClause,
};
