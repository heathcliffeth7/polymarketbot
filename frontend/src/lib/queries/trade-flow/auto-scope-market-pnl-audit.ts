import { pool } from '@/lib/db';
import type { AutoScopeTradeAnalysisRow } from '@/lib/types';

interface LocalMarketPnlRow {
  market_slug: string;
  local_market_pnl_usdc: number | null;
}

const LOCAL_MARKET_PNL_SQL = `WITH target_markets AS (
       SELECT DISTINCT lower(btrim(raw.market_slug)) AS market_slug
       FROM unnest($2::text[]) AS raw(market_slug)
       WHERE btrim(raw.market_slug) <> ''
     ),
     root_metrics AS (
       SELECT DISTINCT ON (lower(btrim(dg.market_slug)), dg.root_builder_order_id)
         lower(btrim(dg.market_slug)) AS market_slug,
         dg.root_builder_order_id,
         COALESCE(
           NULLIF(dg.compact_metrics_json->>'local_fallback_cash_fill_pnl_usdc', '')::double precision,
           NULLIF(dg.compact_metrics_json->>'cash_fill_pnl_usdc', '')::double precision,
           dg.total_pnl_usdc,
           0
         ) AS root_local_pnl_usdc
       FROM trade_flow_auto_scope_trade_diagnostics dg
       JOIN target_markets tm
         ON lower(btrim(dg.market_slug)) = tm.market_slug
       WHERE dg.user_id = $1
       ORDER BY lower(btrim(dg.market_slug)), dg.root_builder_order_id, dg.updated_at DESC
     )
     SELECT
       market_slug,
       COALESCE(SUM(root_local_pnl_usdc), 0)::double precision AS local_market_pnl_usdc
     FROM root_metrics
     GROUP BY market_slug`;

function normalizeMarketSlug(marketSlug: string): string {
  return marketSlug.trim().toLowerCase();
}

export async function enrichRowsWithAutoScopeMarketPnlAudit({
  userId,
  rows,
}: {
  userId: number;
  rows: AutoScopeTradeAnalysisRow[];
}): Promise<AutoScopeTradeAnalysisRow[]> {
  const marketSlugs = Array.from(
    new Set(rows.map((row) => normalizeMarketSlug(row.marketSlug)).filter(Boolean))
  );
  if (marketSlugs.length === 0) return rows;

  try {
    const result = await pool.query<LocalMarketPnlRow>(LOCAL_MARKET_PNL_SQL, [
      userId,
      marketSlugs,
    ]);
    const byMarket = new Map(
      result.rows.map((row) => [
        normalizeMarketSlug(row.market_slug),
        row.local_market_pnl_usdc == null ? null : Number(row.local_market_pnl_usdc),
      ])
    );

    return rows.map((row) => ({
      ...row,
      localMarketPnlUsdc:
        byMarket.get(normalizeMarketSlug(row.marketSlug)) ?? row.localMarketPnlUsdc,
    }));
  } catch (err) {
    console.error('Auto-scope market PnL audit enrichment failed:', err);
    return rows;
  }
}

export const __autoScopeMarketPnlAuditTestUtils = {
  localMarketPnlSql: LOCAL_MARKET_PNL_SQL,
};
