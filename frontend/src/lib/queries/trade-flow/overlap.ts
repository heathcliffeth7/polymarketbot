import { pool } from '@/lib/db';
import type { TradeFlowOverlapGroup, TradeFlowOverlapPeer } from '@/lib/types';

type OverlapRow = {
  market_slug: string;
  token_id: string;
  side: 'buy' | 'sell';
  definition_id: number;
  definition_name: string | null;
  run_id: number | null;
  node_key: string | null;
  source_trade_id: number;
  active_order_count: number;
};

function mapOverlapRow(row: Record<string, unknown>): OverlapRow {
  return {
    market_slug: String(row.market_slug || ''),
    token_id: String(row.token_id || ''),
    side: String(row.side || 'buy') === 'sell' ? 'sell' : 'buy',
    definition_id: Number(row.origin_flow_definition_id),
    definition_name: row.definition_name == null ? null : String(row.definition_name),
    run_id: row.origin_flow_run_id == null ? null : Number(row.origin_flow_run_id),
    node_key: row.origin_flow_node_key == null ? null : String(row.origin_flow_node_key),
    source_trade_id: Number(row.trade_id),
    active_order_count: Number(row.active_order_count || 0),
  };
}

function groupMarketOverlap(rows: OverlapRow[]): TradeFlowOverlapGroup | null {
  if (rows.length === 0) {
    return null;
  }

  const runIds = new Set(rows.map((row) => row.run_id).filter((value) => value != null));
  if (runIds.size > 1) {
    return null;
  }

  const nodeKeys = new Set<string>();
  for (const row of rows) {
    if (row.node_key) {
      nodeKeys.add(row.node_key);
    }
  }

  const intraFlow = nodeKeys.size > 1;
  if (!intraFlow) {
    return null;
  }

  const peers: TradeFlowOverlapPeer[] = rows.map((row) => ({
    market_slug: row.market_slug,
    token_id: row.token_id,
    side: row.side,
    definition_id: row.definition_id,
    definition_name: row.definition_name,
    run_id: row.run_id,
    node_key: row.node_key,
    source_trade_id: row.source_trade_id,
    active_order_count: row.active_order_count,
  }));

  return {
    market_slug: rows[0].market_slug,
    overlap_type: 'intra_flow',
    cross_flow: false,
    intra_flow: true,
    peers,
  };
}

export async function getTradeFlowOverlapSummary(
  userId: number
): Promise<TradeFlowOverlapGroup[]> {
  const res = await pool.query(
    `SELECT o.market_slug,
            o.token_id,
            o.side,
            o.trade_id,
            o.origin_flow_definition_id,
            d.name AS definition_name,
            o.origin_flow_run_id,
            o.origin_flow_node_key,
            COUNT(*)::int AS active_order_count
     FROM trade_builder_orders o
     LEFT JOIN trade_flow_definitions d ON d.id = o.origin_flow_definition_id
     WHERE o.user_id = $1
       AND o.origin_flow_definition_id IS NOT NULL
       AND o.status IN ('pending', 'armed', 'triggered', 'open', 'partially_filled')
     GROUP BY o.market_slug, o.token_id, o.side, o.trade_id, o.origin_flow_definition_id,
              d.name, o.origin_flow_run_id, o.origin_flow_node_key
     ORDER BY o.market_slug ASC, o.origin_flow_definition_id ASC, o.origin_flow_run_id ASC NULLS LAST,
              o.origin_flow_node_key ASC NULLS LAST`,
    [userId]
  );

  const rows = res.rows.map((row) => mapOverlapRow(row as Record<string, unknown>));
  const rowsByMarket = new Map<string, OverlapRow[]>();
  for (const row of rows) {
    if (row.run_id == null) {
      continue;
    }
    const groupKey = `${row.market_slug}::${row.run_id}`;
    const marketRows = rowsByMarket.get(groupKey) ?? [];
    marketRows.push(row);
    rowsByMarket.set(groupKey, marketRows);
  }

  return Array.from(rowsByMarket.values())
    .map((marketRows) => groupMarketOverlap(marketRows))
    .filter((group): group is TradeFlowOverlapGroup => group != null);
}
