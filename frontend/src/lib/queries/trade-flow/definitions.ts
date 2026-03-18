import type { PoolClient } from 'pg';
import {
  compactTelemetryError,
  getPoolTelemetrySnapshot,
  isFlowTelemetryEnabled,
  pool,
} from '@/lib/db';
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
import { cancelFlowResources, recordCancellationEvents } from './cancel-resources';
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
const FLOW_DELETE_REASON = 'definition_deleted';
const FLOW_DUPLICATE_NAME_MESSAGE = 'Flow name is already in use';

function formatTelemetryDuration(startedAt: number, endedAt: number): string {
  if (startedAt <= 0 || endedAt <= 0 || endedAt < startedAt) return 'na';
  return `${Math.round(endedAt - startedAt)}ms`;
}

async function ensureUniqueActiveDefinitionName(
  queryable: Queryable,
  userId: number,
  name: string,
  excludeDefinitionId?: number
): Promise<void> {
  const params: unknown[] = [userId, name.trim().toLowerCase()];
  let query =
    `SELECT id
     FROM trade_flow_definitions
     WHERE user_id = $1
       AND LOWER(name) = $2
       AND status <> 'archived'`;

  if (excludeDefinitionId != null) {
    params.push(excludeDefinitionId);
    query += ` AND id <> $3`;
  }

  query += ' LIMIT 1';
  const res = await queryable.query(query, params);
  if ((res.rowCount ?? 0) > 0) {
    throw new Error(FLOW_DUPLICATE_NAME_MESSAGE);
  }
}

async function fetchVersionById(queryable: Queryable, versionId: number | null): Promise<TradeFlowVersion | null> {
  if (!versionId) return null;
  const res = await queryable.query('SELECT * FROM trade_flow_versions WHERE id = $1 LIMIT 1', [versionId]);
  if ((res.rowCount ?? 0) === 0) return null;
  return mapVersionRow(res.rows[0] as Record<string, unknown>);
}

async function fetchDefinitionDetailById(
  queryable: Queryable,
  userId: number,
  definitionId: number
): Promise<TradeFlowDefinitionDetail | null> {
  const defRes = await queryable.query(
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
    fetchVersionById(queryable, definition.draft_version_id),
    fetchVersionById(queryable, definition.published_version_id),
  ]);

  return {
    definition,
    draftVersion,
    publishedVersion,
  };
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

async function syncVersionGraph(queryable: Queryable, versionId: number, graph: TradeFlowGraph): Promise<void> {
  const [existingNodesRes, existingEdgesRes] = await Promise.all([
    queryable.query('SELECT node_key FROM trade_flow_nodes WHERE version_id = $1', [versionId]),
    queryable.query('SELECT edge_key FROM trade_flow_edges WHERE version_id = $1', [versionId]),
  ]);

  const existingNodeKeys = new Set((existingNodesRes.rows as { node_key: string }[]).map((r) => r.node_key));
  const existingEdgeKeys = new Set((existingEdgesRes.rows as { edge_key: string }[]).map((r) => r.edge_key));

  const incomingNodeKeys = new Set(graph.nodes.map((n) => n.key));
  const incomingEdgeKeys = new Set(graph.edges.map((e) => e.key));

  const removedNodeKeys = [...existingNodeKeys].filter((k) => !incomingNodeKeys.has(k));
  const removedEdgeKeys = [...existingEdgeKeys].filter((k) => !incomingEdgeKeys.has(k));

  const deletePromises: Promise<unknown>[] = [];
  if (removedNodeKeys.length > 0) {
    deletePromises.push(
      queryable.query('DELETE FROM trade_flow_nodes WHERE version_id = $1 AND node_key = ANY($2::text[])', [versionId, removedNodeKeys])
    );
  }
  if (removedEdgeKeys.length > 0) {
    deletePromises.push(
      queryable.query('DELETE FROM trade_flow_edges WHERE version_id = $1 AND edge_key = ANY($2::text[])', [versionId, removedEdgeKeys])
    );
  }
  if (deletePromises.length > 0) await Promise.all(deletePromises);

  if (graph.nodes.length > 0) {
    const nodeValues: unknown[] = [];
    const nodeRows: string[] = [];
    let idx = 1;
    for (const node of graph.nodes) {
      nodeRows.push(`($${idx++}, $${idx++}, $${idx++}, $${idx++}, $${idx++}, $${idx++}::jsonb, NOW())`);
      nodeValues.push(versionId, node.key, node.type, node.positionX, node.positionY, JSON.stringify(node.config || {}));
    }
    await queryable.query(
      `INSERT INTO trade_flow_nodes (version_id, node_key, node_type, position_x, position_y, config_json, created_at)
       VALUES ${nodeRows.join(', ')}
       ON CONFLICT (version_id, node_key) DO UPDATE SET
         node_type = EXCLUDED.node_type,
         position_x = EXCLUDED.position_x,
         position_y = EXCLUDED.position_y,
         config_json = EXCLUDED.config_json`,
      nodeValues
    );
  }

  if (graph.edges.length > 0) {
    const edgeValues: unknown[] = [];
    const edgeRows: string[] = [];
    let idx = 1;
    for (const edge of graph.edges) {
      edgeRows.push(`($${idx++}, $${idx++}, $${idx++}, $${idx++}, $${idx++}, $${idx++}::jsonb, NOW())`);
      edgeValues.push(versionId, edge.key, edge.source, edge.target, edge.type, edge.condition ? JSON.stringify(edge.condition) : null);
    }
    await queryable.query(
      `INSERT INTO trade_flow_edges (version_id, edge_key, source_node_key, target_node_key, edge_type, condition_json, created_at)
       VALUES ${edgeRows.join(', ')}
       ON CONFLICT (version_id, edge_key) DO UPDATE SET
         source_node_key = EXCLUDED.source_node_key,
         target_node_key = EXCLUDED.target_node_key,
         edge_type = EXCLUDED.edge_type,
         condition_json = EXCLUDED.condition_json`,
      edgeValues
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
    await ensureUniqueActiveDefinitionName(client, input.userId, name);

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
    return (await fetchDefinitionDetailById(client, input.userId, Number(definition.id))) as TradeFlowDefinitionDetail;
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
  const telemetryEnabled = isFlowTelemetryEnabled();
  const t0 = telemetryEnabled ? performance.now() : 0;
  let tConnect = 0;
  let tLock = 0;
  let tWrite = 0;
  let tCommit = 0;
  let tRead = 0;
  let transactionStarted = false;
  let client: PoolClient | null = null;
  try {
    client = await pool.connect();
    if (telemetryEnabled) {
      tConnect = performance.now();
    }

    await client.query('BEGIN');
    transactionStarted = true;
    await client.query("SET LOCAL statement_timeout = '15s'");
    await client.query("SET LOCAL lock_timeout = '5s'");

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
    if (telemetryEnabled) {
      tLock = performance.now();
    }
    const definition = defRes.rows[0] as Record<string, unknown>;

    const shouldSyncNormalizedTables = updates.syncNormalizedTables === true;
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
      if (shouldSyncNormalizedTables) {
        await replaceVersionGraph(client, draftVersionId, fallbackGraph);
      }
    }

    if (updates.graphJson !== undefined) {
      const normalizedGraph = normalizeTradeFlowGraph(updates.graphJson);

      await client.query(
        `UPDATE trade_flow_versions
         SET graph_json = $2::jsonb
         WHERE id = $1`,
        [draftVersionId, JSON.stringify(normalizedGraph)]
      );
      if (shouldSyncNormalizedTables) {
        await syncVersionGraph(client, draftVersionId, normalizedGraph);
      }
    }

    const fields: string[] = ['updated_at = NOW()'];
    const params: unknown[] = [definitionId, userId];
    let idx = 3;

    if (updates.name !== undefined) {
      const nextName = updates.name.trim();
      if (!nextName) throw new Error('Flow name cannot be empty');
      await ensureUniqueActiveDefinitionName(client, userId, nextName, definitionId);
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
    if (telemetryEnabled) {
      tWrite = performance.now();
    }

    await client.query('COMMIT');
    transactionStarted = false;
    if (telemetryEnabled) {
      tCommit = performance.now();
    }

    const result = (await fetchDefinitionDetailById(client, userId, definitionId)) as TradeFlowDefinitionDetail;
    if (telemetryEnabled) {
      tRead = performance.now();
      console.log(
        `[draft-save] outcome=ok def=${definitionId} pool=${getPoolTelemetrySnapshot()} connect=${formatTelemetryDuration(t0, tConnect)} lock=${formatTelemetryDuration(tConnect, tLock)} write=${formatTelemetryDuration(tLock, tWrite)} commit=${formatTelemetryDuration(tWrite, tCommit)} read=${formatTelemetryDuration(tCommit, tRead)} total=${formatTelemetryDuration(t0, tRead)}`
      );
    }
    return result;
  } catch (err) {
    if (transactionStarted && client) {
      try {
        await client.query('ROLLBACK');
      } catch (rollbackErr) {
        console.error('Trade flow draft rollback error:', rollbackErr);
      }
    }
    if (telemetryEnabled) {
      const tError = performance.now();
      console.log(
        `[draft-save] outcome=error def=${definitionId} pool=${getPoolTelemetrySnapshot()} connect=${formatTelemetryDuration(t0, tConnect)} lock=${formatTelemetryDuration(tConnect, tLock)} write=${formatTelemetryDuration(tLock, tWrite)} commit=${formatTelemetryDuration(tWrite, tCommit)} read=${formatTelemetryDuration(tCommit, tRead)} elapsed=${formatTelemetryDuration(t0, tError)} err=${compactTelemetryError(err)}`
      );
    }
    throw err;
  } finally {
    client?.release();
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
    return (await fetchDefinitionDetailById(client, context.userId, definitionId)) as TradeFlowDefinitionDetail;
  } catch (err) {
    await client.query('ROLLBACK');
    throw err;
  } finally {
    client.release();
  }
}

export async function hardDeleteTradeFlowDefinition(
  userId: number,
  definitionId: number
): Promise<void> {
  const client = await pool.connect();
  try {
    await client.query('BEGIN');
    await client.query("SET LOCAL statement_timeout = '45s'");

    const defRes = await client.query(
      `SELECT * FROM trade_flow_definitions WHERE id = $1 AND user_id = $2 LIMIT 1 FOR UPDATE`,
      [definitionId, userId]
    );
    if ((defRes.rowCount ?? 0) === 0) {
      throw new Error('Flow definition not found');
    }

    const eventTimestamp = new Date().toISOString();
    const cancelResult = await cancelFlowResources(client, userId, definitionId, FLOW_DELETE_REASON);
    await recordCancellationEvents(client, definitionId, cancelResult, FLOW_DELETE_REASON, eventTimestamp);

    // Clear self-referencing parent_step_id before cascade delete to avoid O(N²) FK checks
    await client.query(
      `UPDATE trade_flow_run_steps SET parent_step_id = NULL
       WHERE run_id IN (SELECT id FROM trade_flow_runs WHERE definition_id = $1 AND user_id = $2)
         AND parent_step_id IS NOT NULL`,
      [definitionId, userId]
    );

    await client.query(
      `DELETE FROM trade_flow_runs
       WHERE definition_id = $1 AND user_id = $2`,
      [definitionId, userId]
    );

    await client.query(
      `DELETE FROM trade_flow_definitions
       WHERE id = $1 AND user_id = $2`,
      [definitionId, userId]
    );

    await client.query('COMMIT');
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

    const cancelResult = await cancelFlowResources(client, userId, definitionId, FLOW_STOP_REASON);
    await recordCancellationEvents(client, definitionId, cancelResult, FLOW_STOP_REASON, eventTimestamp);

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
          canceledRunCount: cancelResult.affectedRuns.length,
          canceledDualDcaJobCount: cancelResult.canceledDualDcaJobIds.length,
          canceledBuilderOrderCount: cancelResult.canceledOrderRows.length,
        }),
      ]
    );

    await client.query('COMMIT');
    return (await fetchDefinitionDetailById(client, userId, definitionId)) as TradeFlowDefinitionDetail;
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
  const client = await pool.connect();
  try {
    return await fetchDefinitionDetailById(client, userId, definitionId);
  } finally {
    client.release();
  }
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
