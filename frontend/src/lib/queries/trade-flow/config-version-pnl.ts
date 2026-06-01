import { pool } from '@/lib/db';

export interface ConfigVersionPnlRow {
  config_version: string | null;
  change_reason: string | null;
  changed_at: string | null;
  trade_count: number;
  total_pnl: number;
  realized_pnl: number;
  open_pnl: number;
  avg_pnl: number;
  win_count: number;
  loss_count: number;
  win_rate: number;
  avg_entry_ms: number | null;
  avg_fill_price: number | null;
  slow_fill_count: number;
  clean_win_count: number;
}

export async function getPnlByConfigVersion(
  userId: number,
  definitionId: number,
  since?: string,
  until?: string,
): Promise<ConfigVersionPnlRow[]> {
  const timeFilter = [];
  const params: unknown[] = [userId, definitionId];
  let paramIdx = 3;

  if (since) {
    timeFilter.push(`t.updated_at >= $${paramIdx}`);
    params.push(since);
    paramIdx++;
  }
  if (until) {
    timeFilter.push(`t.updated_at <= $${paramIdx}`);
    params.push(until);
    paramIdx++;
  }

  const whereClause = timeFilter.length > 0
    ? `AND ${timeFilter.join(' AND ')}`
    : '';

  const query = `
    SELECT
      ns.config_version,
      ccl.change_reason,
      ccl.created_at AS changed_at,
      COUNT(DISTINCT t.root_builder_order_id) AS trade_count,
      COALESCE(SUM(t.total_pnl_usdc), 0) AS total_pnl,
      COALESCE(SUM(t.realized_pnl_usdc), 0) AS realized_pnl,
      COALESCE(SUM(t.open_pnl_usdc), 0) AS open_pnl,
      COALESCE(AVG(t.total_pnl_usdc), 0) AS avg_pnl,
      COUNT(DISTINCT CASE WHEN t.total_pnl_usdc > 0 THEN t.root_builder_order_id END) AS win_count,
      COUNT(DISTINCT CASE WHEN t.total_pnl_usdc <= 0 THEN t.root_builder_order_id END) AS loss_count,
      CASE
        WHEN COUNT(DISTINCT t.root_builder_order_id) = 0 THEN 0
        ELSE COUNT(DISTINCT CASE WHEN t.total_pnl_usdc > 0 THEN t.root_builder_order_id END)::float
             / COUNT(DISTINCT t.root_builder_order_id)
      END AS win_rate,
      AVG(t.trigger_to_submit_ms) AS avg_entry_ms,
      AVG(t.entry_fill_price) AS avg_fill_price,
      COUNT(DISTINCT CASE WHEN t.primary_diagnosis_code = 'slow_fill' THEN t.root_builder_order_id END) AS slow_fill_count,
      COUNT(DISTINCT CASE WHEN t.primary_diagnosis_code = 'clean_win' THEN t.root_builder_order_id END) AS clean_win_count
    FROM trade_builder_order_node_snapshots ns
    LEFT JOIN config_change_log ccl ON ns.config_version = ccl.config_version
    JOIN trade_flow_auto_scope_trade_diagnostics t
      ON t.root_builder_order_id = ns.root_order_id
    WHERE t.user_id = $1
      AND t.definition_id = $2
      ${whereClause}
    GROUP BY ns.config_version, ccl.change_reason, ccl.created_at
    ORDER BY ccl.created_at ASC NULLS FIRST
  `;

  const result = await pool.query(query, params);
  return result.rows.map((row: Record<string, unknown>) => ({
    config_version: row.config_version as string | null,
    change_reason: row.change_reason as string | null,
    changed_at: row.changed_at as string | null,
    trade_count: parseInt(row.trade_count as string, 10),
    total_pnl: parseFloat(row.total_pnl as string),
    realized_pnl: parseFloat(row.realized_pnl as string),
    open_pnl: parseFloat(row.open_pnl as string),
    avg_pnl: parseFloat(row.avg_pnl as string),
    win_count: parseInt(row.win_count as string, 10),
    loss_count: parseInt(row.loss_count as string, 10),
    win_rate: parseFloat(row.win_rate as string),
    avg_entry_ms: row.avg_entry_ms ? parseFloat(row.avg_entry_ms as string) : null,
    avg_fill_price: row.avg_fill_price ? parseFloat(row.avg_fill_price as string) : null,
    slow_fill_count: parseInt(row.slow_fill_count as string, 10),
    clean_win_count: parseInt(row.clean_win_count as string, 10),
  }));
}

export interface ConfigChangeLogEntry {
  id: number;
  config_version: string;
  changed_by: string | null;
  change_reason: string | null;
  changed_fields: Record<string, unknown>;
  created_at: string;
}

export async function getConfigChangeLog(
  since?: string,
  limit = 50,
): Promise<ConfigChangeLogEntry[]> {
  const params: unknown[] = [limit];
  let whereClause = '';

  if (since) {
    whereClause = `WHERE created_at >= $2`;
    params.push(since);
  }

  const query = `
    SELECT id, config_version, changed_by, change_reason,
           changed_fields, created_at
    FROM config_change_log
    ${whereClause}
    ORDER BY created_at DESC
    LIMIT $1
  `;

  const result = await pool.query(query, params);
  return result.rows.map((row: Record<string, unknown>) => ({
    id: parseInt(row.id as string, 10),
    config_version: row.config_version as string,
    changed_by: row.changed_by as string | null,
    change_reason: row.change_reason as string | null,
    changed_fields: row.changed_fields as Record<string, unknown>,
    created_at: row.created_at as string,
  }));
}
