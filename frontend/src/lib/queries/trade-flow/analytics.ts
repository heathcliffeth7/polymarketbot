import { pool } from '@/lib/db';
import type {
  AutoScopeTradeAnalysisPositionState,
  AutoScopeTradeAnalysisResponse,
  AutoScopeTradeAnalysisRow,
  AutoScopeTradeAnalysisSortBy,
  AutoScopeTradeAnalysisSortDirection,
  TradeFlowNodeRuntimeResponse,
  TradeFlowNodeRuntimeRow,
  TradeFlowPtbStateResponse,
  TradeFlowPtbStateRow,
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
