import type { Queryable } from './shared';

const STOPPABLE_TRADE_BUILDER_ORDER_STATUSES = [
  'pending',
  'armed',
  'triggered',
  'open',
  'partially_filled',
  'blocked',
  'guard_blocked',
  'inventory_pending',
  'canceled_requested',
] as const;

export interface CancelFlowResourcesResult {
  affectedRuns: Array<{ id: number; versionId: number | null }>;
  canceledDualDcaJobIds: number[];
  canceledOrderRows: Array<{
    id: number;
    status: string;
    activeExchangeOrderId: string | null;
    originFlowRunId: number | null;
    originFlowNodeKey: string | null;
  }>;
}

export async function cancelFlowResources(
  client: Queryable,
  userId: number,
  definitionId: number,
  reason: string
): Promise<CancelFlowResourcesResult> {
  // 1. Cancel running runs
  const runRes = await client.query(
    `UPDATE trade_flow_runs
     SET status = 'canceled',
         ended_at = COALESCE(ended_at, NOW()),
         updated_at = NOW(),
         last_error = COALESCE(last_error, $3)
     WHERE definition_id = $1
       AND user_id = $2
       AND status = 'running'
     RETURNING id, version_id`,
    [definitionId, userId, reason]
  );
  const affectedRuns = runRes.rows.map((row) => ({
    id: Number(row.id),
    versionId: row.version_id == null ? null : Number(row.version_id),
  }));
  const affectedRunIds = affectedRuns.map((run) => run.id);

  // 2. Cancel queued/running steps for affected runs
  if (affectedRunIds.length > 0) {
    await client.query(
      `UPDATE trade_flow_run_steps
       SET status = 'canceled',
           error_text = COALESCE(error_text, $2),
           ended_at = NOW()
       WHERE run_id = ANY($1::bigint[])
         AND status IN ('queued', 'running')`,
      [affectedRunIds, reason]
    );
  }

  // 3. Cancel active/paused DCA jobs
  const dualDcaRes = await client.query(
    `UPDATE trade_flow_dual_dca_jobs
     SET status = 'canceled',
         last_error = COALESCE(last_error, $2),
         updated_at = NOW()
     WHERE flow_definition_id = $1
       AND status IN ('active', 'paused')
     RETURNING id`,
    [definitionId, reason]
  );
  const canceledDualDcaJobIds = dualDcaRes.rows.map((row) => Number(row.id));

  // 4. Cancel trade builder orders (flow-owned + child + DCA leg orders)
  const orderRes = await client.query(
    `WITH flow_owned_orders AS (
       SELECT DISTINCT o.id
       FROM trade_builder_orders o
       WHERE o.origin_flow_definition_id = $3
       UNION
       SELECT DISTINCT child.id
       FROM trade_builder_orders child
       JOIN trade_builder_orders parent ON parent.id = child.parent_order_id
       WHERE parent.origin_flow_definition_id = $3
       UNION
       SELECT DISTINCT l.builder_order_id
       FROM trade_flow_dual_dca_legs l
       JOIN trade_flow_dual_dca_jobs j ON j.id = l.job_id
       WHERE j.flow_definition_id = $3
         AND l.builder_order_id IS NOT NULL
     )
     UPDATE trade_builder_orders o
     SET status = CASE
           WHEN o.status = 'canceled_requested' THEN 'canceled_requested'
           WHEN o.active_exchange_order_id IS NULL THEN 'canceled'
           ELSE 'canceled_requested'
         END,
         last_error = COALESCE(o.last_error, $4),
         updated_at = NOW()
     WHERE o.user_id = $1
       AND o.status = ANY($2::text[])
       AND o.id IN (SELECT id FROM flow_owned_orders)
     RETURNING o.id, o.status, o.active_exchange_order_id, o.origin_flow_run_id, o.origin_flow_node_key`,
    [userId, STOPPABLE_TRADE_BUILDER_ORDER_STATUSES, definitionId, reason]
  );
  const canceledOrderRows = orderRes.rows.map((row) => ({
    id: Number(row.id),
    status: String(row.status || ''),
    activeExchangeOrderId:
      row.active_exchange_order_id == null ? null : String(row.active_exchange_order_id),
    originFlowRunId: row.origin_flow_run_id == null ? null : Number(row.origin_flow_run_id),
    originFlowNodeKey:
      row.origin_flow_node_key == null ? null : String(row.origin_flow_node_key),
  }));

  return { affectedRuns, canceledDualDcaJobIds, canceledOrderRows };
}

export async function recordCancellationEvents(
  client: Queryable,
  definitionId: number,
  result: CancelFlowResourcesResult,
  reason: string,
  eventTimestamp: string
): Promise<void> {
  const payloadBase = JSON.stringify({ reason, stoppedAt: eventTimestamp });

  // Run cancellation events (bulk)
  if (result.affectedRuns.length > 0) {
    const runIds: number[] = [];
    const versionIds: (number | null)[] = [];
    const payloads: string[] = [];
    for (const run of result.affectedRuns) {
      runIds.push(run.id);
      versionIds.push(run.versionId);
      payloads.push(payloadBase);
    }
    await client.query(
      `INSERT INTO trade_flow_events
        (run_id, definition_id, version_id, event_type, payload_json, created_at)
       SELECT unnest($1::bigint[]), $2, unnest($3::bigint[]), 'run_canceled_by_user', unnest($4::jsonb[]), NOW()`,
      [runIds, definitionId, versionIds, payloads]
    );
  }

  // DCA job cancellation events (bulk)
  if (result.canceledDualDcaJobIds.length > 0) {
    const payloads = result.canceledDualDcaJobIds.map(() => payloadBase);
    await client.query(
      `INSERT INTO trade_flow_dual_dca_events
        (job_id, leg_id, event_type, payload_json, created_at)
       SELECT unnest($1::bigint[]), NULL, 'job_canceled', unnest($2::jsonb[]), NOW()`,
      [result.canceledDualDcaJobIds, payloads]
    );
  }

  // Order cancellation events (bulk)
  if (result.canceledOrderRows.length > 0) {
    const orderIds: number[] = [];
    const eventTypes: string[] = [];
    const payloads: string[] = [];
    for (const order of result.canceledOrderRows) {
      orderIds.push(order.id);
      eventTypes.push(reason);
      payloads.push(JSON.stringify({
        reason,
        stoppedAt: eventTimestamp,
        nextStatus: order.status,
        cancelRequested: order.status === 'canceled_requested',
        activeExchangeOrderId: order.activeExchangeOrderId,
        originFlowRunId: order.originFlowRunId,
        originFlowNodeKey: order.originFlowNodeKey,
      }));
    }
    await client.query(
      `INSERT INTO trade_builder_order_events
        (builder_order_id, event_type, payload_json, created_at)
       SELECT unnest($1::bigint[]), unnest($2::text[]), unnest($3::jsonb[]), NOW()`,
      [orderIds, eventTypes, payloads]
    );
  }
}
