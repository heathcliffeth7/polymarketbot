import { pool } from '@/lib/db';

interface DecisionLogExportRow {
  created_at: string;
  event_ts: string;
  event_type: string;
  decision_id: string | null;
  sl_event_id: string | null;
  fill_event_id: string | null;
  root_order_id: string | null;
  order_id: string | null;
  market_slug: string | null;
  asset: string | null;
  workflow: string | null;
  outcome: string | null;
  payload_json: unknown;
}

function csvField(value: string | number | null): string {
  if (value == null) return '';
  const text = String(value);
  if (!/[",\r\n]/.test(text)) return text;
  return `"${text.replaceAll('"', '""')}"`;
}

export async function buildDecisionLogsRawCsv(params: {
  userId: number;
  from?: string | null;
  to?: string | null;
}): Promise<string> {
  const queryParams: unknown[] = [params.userId];
  const clauses = ['o.user_id = $1'];

  if (params.from) {
    queryParams.push(params.from);
    clauses.push(`l.event_ts >= $${queryParams.length}::timestamptz`);
  }
  if (params.to) {
    queryParams.push(params.to);
    clauses.push(`l.event_ts <= $${queryParams.length}::timestamptz`);
  }

  const res = await pool.query<DecisionLogExportRow>(
    `SELECT
       l.created_at::text,
       l.event_ts::text,
       l.event_type,
       l.decision_id,
       l.sl_event_id,
       l.fill_event_id,
       l.root_order_id,
       l.order_id,
       l.market_slug,
       l.asset,
       l.workflow,
       l.outcome,
       l.payload AS payload_json
     FROM bot_decision_logs l
     JOIN trade_builder_orders o ON o.id::text = COALESCE(l.root_order_id, l.order_id)
     WHERE ${clauses.join(' AND ')}
     ORDER BY l.event_ts ASC, l.created_at ASC, l.id ASC`,
    queryParams
  );

  const headers = [
    'created_at',
    'event_ts',
    'event_type',
    'decision_id',
    'sl_event_id',
    'fill_event_id',
    'root_order_id',
    'order_id',
    'market_slug',
    'asset',
    'workflow',
    'outcome',
    'payload_json',
  ];
  const lines = [headers.map(csvField).join(',')];
  for (const row of res.rows) {
    lines.push(
      [
        row.created_at,
        row.event_ts,
        row.event_type,
        row.decision_id,
        row.sl_event_id,
        row.fill_event_id,
        row.root_order_id,
        row.order_id,
        row.market_slug,
        row.asset,
        row.workflow,
        row.outcome,
        JSON.stringify(row.payload_json ?? {}),
      ]
        .map((value) => csvField(value == null ? null : String(value)))
        .join(',')
    );
  }
  return `${lines.join('\n')}\n`;
}
