import type { TradeFlowGraph, TradeFlowNode } from '@/lib/types';
import { collectRootNodeKeys } from './graph';
import { isRecord, type Queryable } from './shared';

const FLOW_STATE_PUBLISH_MARKER = '__publish_marker';
const FLOW_NODE_STATE_ONCE_FIRED = 'once_fired';
const FLOW_NODE_STATE_ONCE_FIRED_AT = 'once_fired_at';
const FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG = 'once_fired_market_slug';
const FLOW_NODE_STATE_ONCE_BLOCK_LOGGED = 'once_blocked_logged';
const FLOW_NODE_STATE_PUBLISH_AUTO_SCOPE_LOCK_MARKET_SLUG =
  'publish_auto_scope_locked_market_slug';
const FLOW_NODE_STATE_CYCLE_WINDOW_BOUNDARY_MARKER_PREFIX = 'cycle_window_boundary_marker_';
const FLOW_NODE_STATE_CYCLE_WINDOW_LAST_EVAL_PREFIX = 'cycle_window_last_eval_';
const TRIGGER_MARKET_ONCE_SCOPE_VERSION_CURRENT = 2;
const FIXED_ONCE_TRIGGER_PUBLISH_RESET_EXACT_KEYS = [
  FLOW_NODE_STATE_ONCE_FIRED,
  FLOW_NODE_STATE_ONCE_FIRED_AT,
  FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG,
  FLOW_NODE_STATE_ONCE_BLOCK_LOGGED,
  FLOW_NODE_STATE_PUBLISH_AUTO_SCOPE_LOCK_MARKET_SLUG,
  'last_price',
  'last_ws_market_slug',
  'previous_price',
] as const;
const FIXED_ONCE_TRIGGER_PUBLISH_RESET_PREFIXES = [
  'previous_price_',
  'price_samples_',
  'cross_pending_at_',
  'cross_pending_price_',
  'cross_pending_prev_',
  FLOW_NODE_STATE_CYCLE_WINDOW_BOUNDARY_MARKER_PREFIX,
  FLOW_NODE_STATE_CYCLE_WINDOW_LAST_EVAL_PREFIX,
] as const;

interface ActiveTradeFlowRunRow {
  id: number;
  versionId: number;
  contextJson: unknown;
}

export interface PublishRunCutoverResult {
  previousRunId: number | null;
  newRunId: number;
  skippedQueuedSteps: number;
  carriedState: boolean;
}

function cloneJson<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function ensureMutableRecord(value: unknown): Record<string, unknown> {
  return isRecord(value) ? cloneJson(value) : {};
}

function ensureNestedRecord(root: Record<string, unknown>, key: string): Record<string, unknown> {
  const current = root[key];
  if (!isRecord(current)) {
    root[key] = {};
  }
  return root[key] as Record<string, unknown>;
}

function truthy(value: unknown): boolean {
  if (typeof value === 'boolean') return value;
  if (typeof value === 'number') return value !== 0;
  if (typeof value === 'string') {
    const normalized = value.trim().toLowerCase();
    return normalized === 'true' || normalized === '1' || normalized === 'yes' || normalized === 'on';
  }
  return false;
}

function nodeRepeatMode(node: TradeFlowNode): 'once' | 'loop' {
  return String(node.config.repeatMode ?? '').trim().toLowerCase() === 'once' ? 'once' : 'loop';
}

function nodeMarketMode(node: TradeFlowNode): 'auto_scope' | 'fixed' {
  return String(node.config.marketMode ?? '').trim().toLowerCase() === 'auto_scope'
    ? 'auto_scope'
    : 'fixed';
}

function nodeOnceScope(node: TradeFlowNode): 'market' | 'run' {
  const marketMode = nodeMarketMode(node);
  const onceScopeVersion = Number(node.config.onceScopeVersion ?? 0);
  if (
    node.type === 'trigger.market_price' &&
    nodeRepeatMode(node) === 'once' &&
    marketMode === 'auto_scope' &&
    Number.isFinite(onceScopeVersion) &&
    onceScopeVersion < TRIGGER_MARKET_ONCE_SCOPE_VERSION_CURRENT
  ) {
    return 'market';
  }
  return String(node.config.onceScope ?? '').trim().toLowerCase() === 'market' ? 'market' : 'run';
}

function isFixedOnceMarketPriceNode(node: TradeFlowNode): boolean {
  return (
    node.type === 'trigger.market_price' &&
    nodeRepeatMode(node) === 'once' &&
    nodeMarketMode(node) === 'fixed'
  );
}

function resetFixedOnceMarketPriceStateForPublish(stateForNode: Record<string, unknown>): boolean {
  let changed = false;

  for (const key of FIXED_ONCE_TRIGGER_PUBLISH_RESET_EXACT_KEYS) {
    if (!(key in stateForNode)) continue;
    delete stateForNode[key];
    changed = true;
  }

  for (const key of Object.keys(stateForNode)) {
    if (!FIXED_ONCE_TRIGGER_PUBLISH_RESET_PREFIXES.some((prefix) => key.startsWith(prefix))) {
      continue;
    }
    delete stateForNode[key];
    changed = true;
  }

  return changed;
}

function buildInitialTradeFlowContext(graphContext: Record<string, unknown>): Record<string, unknown> {
  return {
    flowContext: cloneJson(graphContext),
    vars: {},
    state: {},
    refs: {},
    nodeState: {},
  };
}

function normalizeTradeFlowContextForPublish(
  graph: TradeFlowGraph,
  existingContext: unknown,
  publishMarker: string
): Record<string, unknown> {
  const context = ensureMutableRecord(existingContext);
  const flowContext = ensureNestedRecord(context, 'flowContext');
  const vars = ensureNestedRecord(context, 'vars');
  const state = ensureNestedRecord(context, 'state');
  const refs = ensureNestedRecord(context, 'refs');
  const nodeState = ensureNestedRecord(context, 'nodeState');
  void vars;
  void refs;

  if (Object.keys(flowContext).length === 0) {
    context.flowContext = cloneJson(graph.context);
  }

  const nodeKeySet = new Set(graph.nodes.map((node) => node.key));
  for (const nodeKey of Object.keys(nodeState)) {
    if (!nodeKeySet.has(nodeKey) || !isRecord(nodeState[nodeKey])) {
      delete nodeState[nodeKey];
    }
  }

  for (const node of graph.nodes) {
    if (!isRecord(nodeState[node.key])) {
      continue;
    }
    const stateForNode = nodeState[node.key] as Record<string, unknown>;
    if (isFixedOnceMarketPriceNode(node)) {
      resetFixedOnceMarketPriceStateForPublish(stateForNode);
      continue;
    }
    if (
      node.type === 'trigger.market_price' &&
      nodeRepeatMode(node) === 'once' &&
      nodeMarketMode(node) === 'auto_scope' &&
      nodeOnceScope(node) === 'run' &&
      truthy(stateForNode[FLOW_NODE_STATE_ONCE_FIRED])
    ) {
      const currentMarketSlug = String(
        flowContext.marketSlug ?? stateForNode[FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG] ?? ''
      ).trim();
      if (currentMarketSlug) {
        stateForNode[FLOW_NODE_STATE_PUBLISH_AUTO_SCOPE_LOCK_MARKET_SLUG] = currentMarketSlug;
        delete stateForNode[FLOW_NODE_STATE_ONCE_FIRED];
        delete stateForNode[FLOW_NODE_STATE_ONCE_FIRED_AT];
        delete stateForNode[FLOW_NODE_STATE_ONCE_BLOCK_LOGGED];
        delete stateForNode[FLOW_NODE_STATE_ONCE_FIRED_MARKET_SLUG];
      }
    }
  }

  state[FLOW_STATE_PUBLISH_MARKER] = publishMarker;
  return context;
}

function selectTradeFlowSeedNodes(graph: TradeFlowGraph): TradeFlowNode[] {
  const triggerNodes = graph.nodes.filter((node) => node.type.startsWith('trigger.'));
  if (triggerNodes.length > 0) return triggerNodes;

  const rootNodeKeys = collectRootNodeKeys(graph.nodes, graph.edges);
  const invalidRoots = graph.nodes.filter(
    (node) => rootNodeKeys.has(node.key) && node.type !== 'action.dual_dca'
  );
  if (invalidRoots.length > 0) {
    throw new Error('flow_invalid_roots_without_trigger');
  }
  const dualRoots = graph.nodes.filter(
    (node) => node.type === 'action.dual_dca' && rootNodeKeys.has(node.key)
  );
  if (dualRoots.length === 0) {
    throw new Error('flow_missing_trigger');
  }
  return dualRoots;
}

async function appendTradeFlowEvent(
  queryable: Queryable,
  runId: number | null,
  definitionId: number,
  versionId: number | null,
  eventType: string,
  payloadJson: Record<string, unknown>
): Promise<void> {
  await queryable.query(
    `INSERT INTO trade_flow_events
       (run_id, definition_id, version_id, event_type, payload_json, created_at)
     VALUES ($1, $2, $3, $4, $5::jsonb, NOW())`,
    [runId, definitionId, versionId, eventType, JSON.stringify(payloadJson)]
  );
}

async function getActiveTradeFlowRunForUpdate(
  queryable: Queryable,
  definitionId: number
): Promise<ActiveTradeFlowRunRow | null> {
  const res = await queryable.query(
    `SELECT id, version_id, context_json
     FROM trade_flow_runs
     WHERE definition_id = $1 AND status = 'running'
     ORDER BY created_at DESC
     LIMIT 1
     FOR UPDATE`,
    [definitionId]
  );
  if ((res.rowCount ?? 0) === 0) return null;
  const row = res.rows[0] as Record<string, unknown>;
  return {
    id: Number(row.id),
    versionId: Number(row.version_id),
    contextJson: row.context_json,
  };
}

async function skipQueuedStepsForRun(
  queryable: Queryable,
  runId: number,
  publishedVersionId: number
): Promise<number> {
  const res = await queryable.query(
    `UPDATE trade_flow_run_steps
     SET status = 'skipped',
         output_json = $2::jsonb,
         error_text = NULL,
         ended_at = NOW()
     WHERE run_id = $1 AND status = 'queued'
     RETURNING id`,
    [
      runId,
      JSON.stringify({
        reason: 'version_changed_on_publish',
        next_version_id: publishedVersionId,
      }),
    ]
  );
  return res.rowCount ?? 0;
}

async function cancelRunForPublish(
  queryable: Queryable,
  runId: number
): Promise<void> {
  await queryable.query(
    `UPDATE trade_flow_runs
     SET status = 'canceled',
         ended_at = NOW(),
         last_error = $2,
         updated_at = NOW()
     WHERE id = $1`,
    [runId, 'version_changed']
  );
}

async function createTradeFlowRunForPublish(
  queryable: Queryable,
  definitionId: number,
  versionId: number,
  triggerSource: string,
  contextJson: Record<string, unknown>
): Promise<number> {
  const res = await queryable.query(
    `INSERT INTO trade_flow_runs
       (definition_id, version_id, user_id, status, trigger_source, context_json, started_at, created_at, updated_at)
     VALUES
       ($1, $2, (SELECT user_id FROM trade_flow_definitions WHERE id = $1), 'running', $3, $4::jsonb, NOW(), NOW(), NOW())
     RETURNING id`,
    [definitionId, versionId, triggerSource, JSON.stringify(contextJson)]
  );
  return Number((res.rows[0] as Record<string, unknown>).id);
}

async function enqueueInitialSeedSteps(
  queryable: Queryable,
  runId: number,
  nodes: TradeFlowNode[]
): Promise<number> {
  let seeded = 0;
  for (const node of nodes) {
    const res = await queryable.query(
      `INSERT INTO trade_flow_run_steps
         (run_id, node_key, node_type, status, attempt, input_json, output_json, error_text, started_at, ended_at, available_at, parent_step_id, idempotency_key, created_at)
       VALUES
         ($1, $2, $3, 'queued', 1, NULL, NULL, NULL, NULL, NULL, NOW(), NULL, $4, NOW())
       ON CONFLICT (run_id, idempotency_key) WHERE idempotency_key IS NOT NULL
       DO NOTHING
       RETURNING id`,
      [runId, node.key, node.type, `seed:${runId}:${node.key}`]
    );
    if ((res.rowCount ?? 0) > 0) seeded += 1;
  }
  return seeded;
}

export async function rotatePublishedFlowRunOnPublish(
  queryable: Queryable,
  input: {
    definitionId: number;
    definitionName: string;
    publishedVersionId: number;
    graph: TradeFlowGraph;
    publishMarker: string;
  }
): Promise<PublishRunCutoverResult> {
  const activeRun = await getActiveTradeFlowRunForUpdate(queryable, input.definitionId);
  const carriedState = !!activeRun;
  const previousRunId = activeRun?.id ?? null;

  const nextContext = activeRun
    ? normalizeTradeFlowContextForPublish(input.graph, activeRun.contextJson, input.publishMarker)
    : buildInitialTradeFlowContext(input.graph.context);

  let skippedQueuedSteps = 0;
  if (activeRun) {
    skippedQueuedSteps = await skipQueuedStepsForRun(
      queryable,
      activeRun.id,
      input.publishedVersionId
    );
    await cancelRunForPublish(queryable, activeRun.id);
    await appendTradeFlowEvent(
      queryable,
      activeRun.id,
      input.definitionId,
      activeRun.versionId,
      'run_canceled_version_changed',
      {
        previous_version_id: activeRun.versionId,
        next_version_id: input.publishedVersionId,
        reason: 'publish_cutover',
        skipped_queued_steps: skippedQueuedSteps,
      }
    );
  }

  const newRunId = await createTradeFlowRunForPublish(
    queryable,
    input.definitionId,
    input.publishedVersionId,
    activeRun ? 'publish_cutover' : 'publish_start',
    nextContext
  );
  const seedNodes = selectTradeFlowSeedNodes(input.graph);
  const seededStepCount = await enqueueInitialSeedSteps(queryable, newRunId, seedNodes);

  await appendTradeFlowEvent(
    queryable,
    newRunId,
    input.definitionId,
    input.publishedVersionId,
    'run_started',
    {
      run_id: newRunId,
      version_id: input.publishedVersionId,
      definition_name: input.definitionName,
      trigger_source: activeRun ? 'publish_cutover' : 'publish_start',
      previous_run_id: previousRunId,
      carried_state: carriedState,
      seeded_step_count: seededStepCount,
    }
  );

  return {
    previousRunId,
    newRunId,
    skippedQueuedSteps,
    carriedState,
  };
}
