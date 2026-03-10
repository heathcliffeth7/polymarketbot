import { pool } from '@/lib/db';
import type {
  PaginatedResponse,
  TradeFlowDefinition,
  TradeFlowDefinitionDetail,
  TradeFlowEvent,
  TradeFlowGraph,
  TradeFlowRun,
  TradeFlowVersion,
} from '@/lib/types';
import { DEFAULT_GRAPH, type Queryable, type TradeFlowListFilters, type TradeFlowRunFilters, type CreateTradeFlowDefinitionInput, type UpdateTradeFlowDefinitionInput } from './shared';
import { normalizeTradeFlowGraph } from './graph';
import { migrateLegacyWorkflowsToFlows } from './legacy';
import { rotatePublishedFlowRunOnPublish } from './publish-runtime';
import { validateTradeFlowGraphWithRuntimeConfig } from './validation';

function mapDefinitionRow(row: Record<string, unknown>): TradeFlowDefinition {
  return {
    id: Number(row.id),
    name: String(row.name || ''),
    description: row.description == null ? null : String(row.description),
    status: String(row.status || 'draft') as TradeFlowDefinition['status'],
    draft_version_id: row.draft_version_id == null ? null : Number(row.draft_version_id),
    published_version_id: row.published_version_id == null ? null : Number(row.published_version_id),
    last_error: row.last_error == null ? null : String(row.last_error),
    created_at: String(row.created_at),
    updated_at: String(row.updated_at),
    legacy_workflow_id: row.legacy_workflow_id == null ? null : Number(row.legacy_workflow_id),
  };
}

function mapVersionRow(row: Record<string, unknown>): TradeFlowVersion {
  return {
    id: Number(row.id),
    definition_id: Number(row.definition_id),
    version_no: Number(row.version_no),
    status: String(row.status || 'draft') as TradeFlowVersion['status'],
    graph_json: normalizeTradeFlowGraph(row.graph_json),
    published_at: row.published_at == null ? null : String(row.published_at),
    created_at: String(row.created_at),
  };
}

function mapEventRow(row: Record<string, unknown>): TradeFlowEvent {
  return {
    id: Number(row.id),
    run_id: row.run_id == null ? null : Number(row.run_id),
    definition_id: Number(row.definition_id),
    version_id: row.version_id == null ? null : Number(row.version_id),
    definition_name: row.definition_name == null ? null : String(row.definition_name),
    event_type: String(row.event_type || ''),
    payload_json:
      row.payload_json && typeof row.payload_json === 'object' && !Array.isArray(row.payload_json)
        ? (row.payload_json as Record<string, unknown>)
        : {},
    created_at: String(row.created_at),
  };
}

const FLOW_STOP_REASON = 'flow_stopped_by_user';
const STOPPABLE_TRADE_BUILDER_ORDER_STATUSES = [
  'pending',
  'armed',
  'triggered',
  'open',
  'partially_filled',
  'blocked',
  'inventory_pending',
  'canceled_requested',
] as const;

async function fetchVersionById(queryable: Queryable, versionId: number | null): Promise<TradeFlowVersion | null> {
  if (!versionId) return null;
  const res = await queryable.query('SELECT * FROM trade_flow_versions WHERE id = $1 LIMIT 1', [versionId]);
  if ((res.rowCount ?? 0) === 0) return null;
  return mapVersionRow(res.rows[0] as Record<string, unknown>);
}

async function replaceVersionGraph(queryable: Queryable, versionId: number, graph: TradeFlowGraph): Promise<void> {
  await queryable.query('DELETE FROM trade_flow_nodes WHERE version_id = $1', [versionId]);
  await queryable.query('DELETE FROM trade_flow_edges WHERE version_id = $1', [versionId]);

  for (const node of graph.nodes) {
    await queryable.query(
      `INSERT INTO trade_flow_nodes (version_id, node_key, node_type, position_x, position_y, config_json, created_at)
       VALUES ($1, $2, $3, $4, $5, $6::jsonb, NOW())`,
      [
        versionId,
        node.key,
        node.type,
        node.positionX,
        node.positionY,
        JSON.stringify(node.config || {}),
      ]
    );
  }

  for (const edge of graph.edges) {
    await queryable.query(
      `INSERT INTO trade_flow_edges
         (version_id, edge_key, source_node_key, target_node_key, edge_type, condition_json, created_at)
       VALUES ($1, $2, $3, $4, $5, $6::jsonb, NOW())`,
      [
        versionId,
        edge.key,
        edge.source,
        edge.target,
        edge.type,
        edge.condition ? JSON.stringify(edge.condition) : null,
      ]
    );
  }
}

export async function createTradeFlowDefinition(
  input: CreateTradeFlowDefinitionInput
): Promise<TradeFlowDefinitionDetail> {
  const name = input.name.trim();
  if (!name) {
    throw new Error('Flow name is required');
  }

  const graph = normalizeTradeFlowGraph(input.graphJson);

  const client = await pool.connect();
  try {
    await client.query('BEGIN');

    if (input.legacyWorkflowId) {
      const legacyWorkflowRes = await client.query(
        `SELECT id
         FROM trade_builder_workflows
         WHERE id = $1 AND user_id = $2
         LIMIT 1`,
        [input.legacyWorkflowId, input.userId]
      );
      if ((legacyWorkflowRes.rowCount ?? 0) === 0) {
        throw new Error('Legacy workflow not found');
      }
    }

    const defRes = await client.query(
      `INSERT INTO trade_flow_definitions (user_id, name, description, status, created_at, updated_at)
       VALUES ($1, $2, $3, 'draft', NOW(), NOW())
       RETURNING *`,
      [input.userId, name, input.description ?? null]
    );
    const definition = defRes.rows[0] as Record<string, unknown>;

    const versionRes = await client.query(
      `INSERT INTO trade_flow_versions (definition_id, version_no, status, graph_json, created_at)
       VALUES ($1, 1, 'draft', $2::jsonb, NOW())
       RETURNING *`,
      [definition.id, JSON.stringify(graph)]
    );
    const draftVersion = versionRes.rows[0] as Record<string, unknown>;

    await replaceVersionGraph(client, Number(draftVersion.id), graph);

    await client.query(
      `UPDATE trade_flow_definitions
       SET draft_version_id = $2, updated_at = NOW()
       WHERE id = $1`,
      [definition.id, draftVersion.id]
    );

    if (input.legacyWorkflowId) {
      await client.query(
        `INSERT INTO trade_flow_legacy_mappings (legacy_workflow_id, definition_id, version_id, created_at, updated_at)
         VALUES ($1, $2, $3, NOW(), NOW())
         ON CONFLICT (legacy_workflow_id) DO UPDATE
         SET definition_id = EXCLUDED.definition_id,
             version_id = EXCLUDED.version_id,
             updated_at = NOW()`,
        [input.legacyWorkflowId, definition.id, draftVersion.id]
      );
    }

    await client.query('COMMIT');
    return (await getTradeFlowDefinitionById(input.userId, Number(definition.id))) as TradeFlowDefinitionDetail;
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function updateTradeFlowDefinitionDraft(
  userId: number,
  definitionId: number,
  updates: UpdateTradeFlowDefinitionInput
): Promise<TradeFlowDefinitionDetail> {
  const client = await pool.connect();
  try {
    await client.query('BEGIN');

    const defRes = await client.query(
      `SELECT *
       FROM trade_flow_definitions
       WHERE id = $1
         AND user_id = $2
       LIMIT 1
       FOR UPDATE`,
      [definitionId, userId]
    );
    if ((defRes.rowCount ?? 0) === 0) {
      throw new Error('Flow definition not found');
    }
    const definition = defRes.rows[0] as Record<string, unknown>;

    let draftVersionId = definition.draft_version_id == null ? null : Number(definition.draft_version_id);
    if (!draftVersionId) {
      const maxVersionRes = await client.query(
        `SELECT COALESCE(MAX(version_no), 0)::int AS max_version
         FROM trade_flow_versions
         WHERE definition_id = $1`,
        [definitionId]
      );
      const maxVersion = Number(maxVersionRes.rows[0]?.max_version || 0);
      const fallbackGraph =
        (await fetchVersionById(
          client,
          definition.published_version_id == null ? null : Number(definition.published_version_id)
        ))?.graph_json || DEFAULT_GRAPH;

      const insertDraftRes = await client.query(
        `INSERT INTO trade_flow_versions (definition_id, version_no, status, graph_json, created_at)
         VALUES ($1, $2, 'draft', $3::jsonb, NOW())
         RETURNING id`,
        [definitionId, maxVersion + 1, JSON.stringify(fallbackGraph)]
      );
      draftVersionId = Number(insertDraftRes.rows[0].id);
      await replaceVersionGraph(client, draftVersionId, fallbackGraph);
    }

    if (updates.graphJson !== undefined) {
      const normalizedGraph = normalizeTradeFlowGraph(updates.graphJson);

      await client.query(
        `UPDATE trade_flow_versions
         SET graph_json = $2::jsonb
         WHERE id = $1`,
        [draftVersionId, JSON.stringify(normalizedGraph)]
      );
      await replaceVersionGraph(client, draftVersionId, normalizedGraph);
    }

    const fields: string[] = ['updated_at = NOW()'];
    const params: unknown[] = [definitionId, userId];
    let idx = 3;

    if (updates.name !== undefined) {
      const nextName = updates.name.trim();
      if (!nextName) throw new Error('Flow name cannot be empty');
      fields.push(`name = $${idx++}`);
      params.push(nextName);
    }

    if (updates.description !== undefined) {
      fields.push(`description = $${idx++}`);
      params.push(updates.description ?? null);
    }

    if (draftVersionId !== (definition.draft_version_id == null ? null : Number(definition.draft_version_id))) {
      fields.push(`draft_version_id = $${idx++}`);
      params.push(draftVersionId);
    }

    await client.query(
      `UPDATE trade_flow_definitions
       SET ${fields.join(', ')}
       WHERE id = $1 AND user_id = $2`,
      params
    );

    await client.query('COMMIT');
    return (await getTradeFlowDefinitionById(userId, definitionId)) as TradeFlowDefinitionDetail;
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function publishTradeFlowDefinition(
  context: { userId: number; username: string },
  definitionId: number
): Promise<TradeFlowDefinitionDetail> {
  const client = await pool.connect();
  try {
    await client.query('BEGIN');

    const defRes = await client.query(
      `SELECT * FROM trade_flow_definitions WHERE id = $1 AND user_id = $2 LIMIT 1 FOR UPDATE`,
      [definitionId, context.userId]
    );
    if ((defRes.rowCount ?? 0) === 0) {
      throw new Error('Flow definition not found');
    }

    const def = defRes.rows[0] as Record<string, unknown>;
    const draftVersionId = def.draft_version_id == null ? null : Number(def.draft_version_id);
    if (!draftVersionId) {
      throw new Error('Draft version not found');
    }

    const draftVersion = await fetchVersionById(client, draftVersionId);
    if (!draftVersion) {
      throw new Error('Draft version payload not found');
    }

    const validation = await validateTradeFlowGraphWithRuntimeConfig(draftVersion.graph_json, context);
    if (!validation.valid) {
      throw new Error(
        validation.issues
          .filter((issue) => issue.severity === 'error')
          .map((issue) => issue.message)
          .join(' | ')
      );
    }

    const maxVersionRes = await client.query(
      `SELECT COALESCE(MAX(version_no), 0)::int AS max_version
       FROM trade_flow_versions
       WHERE definition_id = $1`,
      [definitionId]
    );
    const maxVersion = Number(maxVersionRes.rows[0]?.max_version || 0);

    await client.query(
      `UPDATE trade_flow_versions
       SET status = 'archived'
       WHERE definition_id = $1 AND status = 'published'`,
      [definitionId]
    );

    const publishedRes = await client.query(
      `INSERT INTO trade_flow_versions
         (definition_id, version_no, status, graph_json, published_at, created_at)
       VALUES
         ($1, $2, 'published', $3::jsonb, NOW(), NOW())
       RETURNING *`,
      [definitionId, maxVersion + 1, JSON.stringify(draftVersion.graph_json)]
    );
    const publishedVersionId = Number(publishedRes.rows[0].id);
    const publishedRow = publishedRes.rows[0] as Record<string, unknown>;
    const publishMarkerRaw = String(publishedRow.published_at ?? publishedRow.created_at ?? '').trim();
    const publishMarkerMs = Date.parse(publishMarkerRaw);
    const publishMarker = `${publishedVersionId}:${
      Number.isFinite(publishMarkerMs) ? publishMarkerMs : Date.now()
    }`;
    await replaceVersionGraph(client, publishedVersionId, draftVersion.graph_json);

    const newDraftRes = await client.query(
      `INSERT INTO trade_flow_versions
         (definition_id, version_no, status, graph_json, created_at)
       VALUES
         ($1, $2, 'draft', $3::jsonb, NOW())
       RETURNING id`,
      [definitionId, maxVersion + 2, JSON.stringify(draftVersion.graph_json)]
    );
    const newDraftVersionId = Number(newDraftRes.rows[0].id);
    await replaceVersionGraph(client, newDraftVersionId, draftVersion.graph_json);

    await client.query(
      `UPDATE trade_flow_definitions
       SET status = 'published',
           published_version_id = $2,
           draft_version_id = $3,
           updated_at = NOW(),
           last_error = NULL
       WHERE id = $1`,
      [definitionId, publishedVersionId, newDraftVersionId]
    );

    const cutover = await rotatePublishedFlowRunOnPublish(client, {
      definitionId,
      definitionName: String(def.name || ''),
      publishedVersionId,
      graph: draftVersion.graph_json,
      publishMarker,
    });

    await client.query(
      `INSERT INTO trade_flow_events (run_id, definition_id, version_id, event_type, payload_json, created_at)
       VALUES (NULL, $1, $2, 'flow_published', $3::jsonb, NOW())`,
      [
        definitionId,
        publishedVersionId,
        JSON.stringify({
          publishedVersionId,
          draftVersionId: newDraftVersionId,
          previousRunId: cutover.previousRunId,
          newRunId: cutover.newRunId,
          skippedQueuedSteps: cutover.skippedQueuedSteps,
          carriedState: cutover.carriedState,
        }),
      ]
    );

    await client.query('COMMIT');
    return (await getTradeFlowDefinitionById(context.userId, definitionId)) as TradeFlowDefinitionDetail;
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function archiveTradeFlowDefinition(
  userId: number,
  definitionId: number
): Promise<TradeFlowDefinitionDetail> {
  const client = await pool.connect();
  try {
    await client.query('BEGIN');

    const defRes = await client.query(
      `SELECT * FROM trade_flow_definitions WHERE id = $1 AND user_id = $2 LIMIT 1 FOR UPDATE`,
      [definitionId, userId]
    );
    if ((defRes.rowCount ?? 0) === 0) {
      throw new Error('Flow definition not found');
    }

    const current = defRes.rows[0] as Record<string, unknown>;
    const currentStatus = String(current.status || '');
    if (currentStatus === 'archived') {
      await client.query('COMMIT');
      return (await getTradeFlowDefinitionById(userId, definitionId)) as TradeFlowDefinitionDetail;
    }

    await client.query(
      `UPDATE trade_flow_runs
       SET status = 'canceled',
           ended_at = NOW(),
           updated_at = NOW(),
           last_error = COALESCE(last_error, 'definition_archived')
       WHERE definition_id = $1
         AND user_id = $2
         AND status = 'running'`,
      [definitionId, userId]
    );

    await client.query(
      `UPDATE trade_flow_definitions
       SET status = 'archived',
           updated_at = NOW()
       WHERE id = $1 AND user_id = $2`,
      [definitionId, userId]
    );

    await client.query(
      `INSERT INTO trade_flow_events
        (run_id, definition_id, version_id, event_type, payload_json, created_at)
       VALUES
        (NULL, $1, $2, 'flow_archived', $3::jsonb, NOW())`,
      [
        definitionId,
        current.published_version_id == null ? null : Number(current.published_version_id),
        JSON.stringify({ definitionId, archivedAt: new Date().toISOString() }),
      ]
    );

    await client.query('COMMIT');
    return (await getTradeFlowDefinitionById(userId, definitionId)) as TradeFlowDefinitionDetail;
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function stopTradeFlowDefinition(
  userId: number,
  definitionId: number
): Promise<TradeFlowDefinitionDetail> {
  const client = await pool.connect();
  try {
    await client.query('BEGIN');

    const defRes = await client.query(
      `SELECT *
       FROM trade_flow_definitions
       WHERE id = $1
         AND user_id = $2
       LIMIT 1
       FOR UPDATE`,
      [definitionId, userId]
    );
    if ((defRes.rowCount ?? 0) === 0) {
      throw new Error('Flow definition not found');
    }

    const definition = defRes.rows[0] as Record<string, unknown>;
    const currentStatus = String(definition.status || '');
    if (currentStatus !== 'published') {
      throw new Error('Flow definition is not published');
    }

    const eventTimestamp = new Date().toISOString();
    const publishedVersionId =
      definition.published_version_id == null ? null : Number(definition.published_version_id);

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
      [definitionId, userId, FLOW_STOP_REASON]
    );
    const affectedRuns = runRes.rows.map((row) => ({
      id: Number(row.id),
      versionId: row.version_id == null ? null : Number(row.version_id),
    }));
    const affectedRunIds = affectedRuns.map((run) => run.id);

    if (affectedRunIds.length > 0) {
      await client.query(
        `UPDATE trade_flow_run_steps
         SET status = 'canceled',
             error_text = COALESCE(error_text, $2),
             ended_at = NOW()
         WHERE run_id = ANY($1::bigint[])
           AND status IN ('queued', 'running')`,
        [affectedRunIds, FLOW_STOP_REASON]
      );

      for (const run of affectedRuns) {
        await client.query(
          `INSERT INTO trade_flow_events
            (run_id, definition_id, version_id, event_type, payload_json, created_at)
           VALUES
            ($1, $2, $3, 'run_canceled_by_user', $4::jsonb, NOW())`,
          [
            run.id,
            definitionId,
            run.versionId,
            JSON.stringify({
              reason: FLOW_STOP_REASON,
              stoppedAt: eventTimestamp,
            }),
          ]
        );
      }
    }

    const dualDcaRes = await client.query(
      `UPDATE trade_flow_dual_dca_jobs
       SET status = 'canceled',
           last_error = COALESCE(last_error, $2),
           updated_at = NOW()
       WHERE flow_definition_id = $1
         AND status IN ('active', 'paused')
       RETURNING id`,
      [definitionId, FLOW_STOP_REASON]
    );
    const dualDcaJobIds = dualDcaRes.rows.map((row) => Number(row.id));
    for (const jobId of dualDcaJobIds) {
      await client.query(
        `INSERT INTO trade_flow_dual_dca_events
          (job_id, leg_id, event_type, payload_json, created_at)
         VALUES
          ($1, NULL, 'job_canceled', $2::jsonb, NOW())`,
        [
          jobId,
          JSON.stringify({
            reason: FLOW_STOP_REASON,
            stoppedAt: eventTimestamp,
          }),
        ]
      );
    }

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
      [userId, STOPPABLE_TRADE_BUILDER_ORDER_STATUSES, definitionId, FLOW_STOP_REASON]
    );
    const affectedOrderRows = orderRes.rows.map((row) => ({
      id: Number(row.id),
      status: String(row.status || ''),
      activeExchangeOrderId:
        row.active_exchange_order_id == null ? null : String(row.active_exchange_order_id),
      originFlowRunId: row.origin_flow_run_id == null ? null : Number(row.origin_flow_run_id),
      originFlowNodeKey:
        row.origin_flow_node_key == null ? null : String(row.origin_flow_node_key),
    }));

    for (const order of affectedOrderRows) {
      await client.query(
        `INSERT INTO trade_builder_order_events
          (builder_order_id, event_type, payload_json, created_at)
         VALUES
          ($1, 'flow_stopped_by_user', $2::jsonb, NOW())`,
        [
          order.id,
          JSON.stringify({
            reason: FLOW_STOP_REASON,
            stoppedAt: eventTimestamp,
            nextStatus: order.status,
            cancelRequested: order.status === 'canceled_requested',
            activeExchangeOrderId: order.activeExchangeOrderId,
            originFlowRunId: order.originFlowRunId,
            originFlowNodeKey: order.originFlowNodeKey,
          }),
        ]
      );
    }

    await client.query(
      `UPDATE trade_flow_definitions
       SET status = 'draft',
           updated_at = NOW()
       WHERE id = $1
         AND user_id = $2`,
      [definitionId, userId]
    );

    await client.query(
      `INSERT INTO trade_flow_events
        (run_id, definition_id, version_id, event_type, payload_json, created_at)
       VALUES
        (NULL, $1, $2, 'flow_stopped_by_user', $3::jsonb, NOW())`,
      [
        definitionId,
        publishedVersionId,
        JSON.stringify({
          reason: FLOW_STOP_REASON,
          stoppedAt: eventTimestamp,
          canceledRunCount: affectedRuns.length,
          canceledDualDcaJobCount: dualDcaJobIds.length,
          canceledBuilderOrderCount: affectedOrderRows.length,
        }),
      ]
    );

    await client.query('COMMIT');
    return (await getTradeFlowDefinitionById(userId, definitionId)) as TradeFlowDefinitionDetail;
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function getTradeFlowDefinitionById(
  userId: number,
  definitionId: number
): Promise<TradeFlowDefinitionDetail | null> {
  const defRes = await pool.query(
    `SELECT d.*, m.legacy_workflow_id
     FROM trade_flow_definitions d
     LEFT JOIN trade_flow_legacy_mappings m ON m.definition_id = d.id
     WHERE d.id = $1
       AND d.user_id = $2
     LIMIT 1`,
    [definitionId, userId]
  );
  if ((defRes.rowCount ?? 0) === 0) return null;

  const definition = mapDefinitionRow(defRes.rows[0] as Record<string, unknown>);
  const [draftVersion, publishedVersion] = await Promise.all([
    fetchVersionById(pool, definition.draft_version_id),
    fetchVersionById(pool, definition.published_version_id),
  ]);

  return {
    definition,
    draftVersion,
    publishedVersion,
  };
}

export async function getTradeFlowDefinitions(
  filters: TradeFlowListFilters
): Promise<PaginatedResponse<TradeFlowDefinition>> {
  if (filters.autoMigrateLegacy !== false) {
    await migrateLegacyWorkflowsToFlows(filters.userId, 25);
  }

  const page = filters.page || 1;
  const limit = Math.min(filters.limit || 20, 100);
  const offset = (page - 1) * limit;

  const whereParts: string[] = ['d.user_id = $1'];
  const params: unknown[] = [filters.userId];
  let idx = 2;

  if (filters.status) {
    whereParts.push(`d.status = $${idx++}`);
    params.push(filters.status);
  }

  const where = whereParts.length ? `WHERE ${whereParts.join(' AND ')}` : '';

  const [countRes, dataRes] = await Promise.all([
    pool.query(`SELECT COUNT(*)::int AS total FROM trade_flow_definitions d ${where}`, params),
    pool.query(
      `SELECT d.*, m.legacy_workflow_id
       FROM trade_flow_definitions d
       LEFT JOIN trade_flow_legacy_mappings m ON m.definition_id = d.id
       ${where}
       ORDER BY d.updated_at DESC, d.id DESC
       LIMIT $${idx++} OFFSET $${idx++}`,
      [...params, limit, offset]
    ),
  ]);

  const total = Number(countRes.rows[0]?.total || 0);
  return {
    data: dataRes.rows.map((row) => mapDefinitionRow(row as Record<string, unknown>)),
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
  };
}

export async function getTradeFlowVersions(userId: number, definitionId: number): Promise<TradeFlowVersion[]> {
  const res = await pool.query(
    `SELECT v.*
     FROM trade_flow_versions v
     JOIN trade_flow_definitions d ON d.id = v.definition_id
     WHERE v.definition_id = $1 AND d.user_id = $2
     ORDER BY v.version_no DESC`,
    [definitionId, userId]
  );
  return res.rows.map((row) => mapVersionRow(row as Record<string, unknown>));
}

export async function getTradeFlowRuns(
  filters: TradeFlowRunFilters
): Promise<PaginatedResponse<TradeFlowRun>> {
  const page = filters.page || 1;
  const limit = Math.min(filters.limit || 20, 100);
  const offset = (page - 1) * limit;

  const whereParts: string[] = ['user_id = $1'];
  const params: unknown[] = [filters.userId];
  let idx = 2;

  if (filters.definitionId) {
    whereParts.push(`definition_id = $${idx++}`);
    params.push(filters.definitionId);
  }
  if (filters.status) {
    whereParts.push(`status = $${idx++}`);
    params.push(filters.status);
  }

  const where = whereParts.length ? `WHERE ${whereParts.join(' AND ')}` : '';

  const [countRes, dataRes] = await Promise.all([
    pool.query(`SELECT COUNT(*)::int AS total FROM trade_flow_runs ${where}`, params),
    pool.query(
      `SELECT * FROM trade_flow_runs ${where} ORDER BY created_at DESC LIMIT $${idx++} OFFSET $${idx++}`,
      [...params, limit, offset]
    ),
  ]);

  const total = Number(countRes.rows[0]?.total || 0);
  return {
    data: dataRes.rows as TradeFlowRun[],
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
  };
}

export async function getTradeFlowRunEvents(
  userId: number,
  runId: number,
  page = 1,
  limit = 50
): Promise<PaginatedResponse<TradeFlowEvent>> {
  const safeLimit = Math.min(Math.max(1, limit), 200);
  const safePage = Math.max(1, page);
  const offset = (safePage - 1) * safeLimit;

  const [countRes, dataRes] = await Promise.all([
    pool.query(
      `SELECT COUNT(*)::int AS total
       FROM trade_flow_events e
       JOIN trade_flow_runs r ON r.id = e.run_id
       WHERE e.run_id = $1 AND r.user_id = $2`,
      [runId, userId]
    ),
    pool.query(
      `SELECT e.*, d.name AS definition_name
       FROM trade_flow_events e
       JOIN trade_flow_runs r ON r.id = e.run_id
       JOIN trade_flow_definitions d ON d.id = e.definition_id
       WHERE e.run_id = $1 AND r.user_id = $2
       ORDER BY e.created_at DESC, e.id DESC
       LIMIT $3 OFFSET $4`,
      [runId, userId, safeLimit, offset]
    ),
  ]);

  const total = Number(countRes.rows[0]?.total || 0);
  return {
    data: dataRes.rows.map((row) => mapEventRow(row as Record<string, unknown>)),
    total,
    page: safePage,
    limit: safeLimit,
    totalPages: Math.ceil(total / safeLimit),
  };
}

export async function getRecentTradeFlowEvents(
  userId: number,
  status: TradeFlowRun['status'] | undefined = 'running',
  limit = 100
): Promise<TradeFlowEvent[]> {
  const safeLimit = Math.min(Math.max(1, limit), 200);
  const whereParts: string[] = ['r.user_id = $1'];
  const params: unknown[] = [userId];
  let idx = 2;

  if (status) {
    whereParts.push(`r.status = $${idx++}`);
    params.push(status);
  }

  const where = `WHERE ${whereParts.join(' AND ')}`;
  const res = await pool.query(
    `SELECT e.*, d.name AS definition_name
     FROM trade_flow_events e
     JOIN trade_flow_runs r ON r.id = e.run_id
     JOIN trade_flow_definitions d ON d.id = e.definition_id
     ${where}
     ORDER BY e.created_at DESC, e.id DESC
     LIMIT $${idx}`,
    [...params, safeLimit]
  );

  return res.rows.map((row) => mapEventRow(row as Record<string, unknown>));
}
