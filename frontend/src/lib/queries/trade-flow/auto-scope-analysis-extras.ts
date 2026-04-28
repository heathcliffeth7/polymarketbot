import { pool } from '@/lib/db';
import type {
  AutoScopeTradeAnalysisRow,
  AutoScopeTradeBlockedSignal,
  AutoScopeTradeDiagnostic,
  AutoScopeTradeExecutionTelemetry,
  AutoScopeTradeForensicEvent,
  AutoScopeTradeForensicSummary,
  AutoScopeNoOrderTelemetry,
  AutoScopeTradePositionLegSnapshot,
  AutoScopeTradePositionSnapshot,
  AutoScopeTradeRiskFlags,
  AutoScopeTradeScenarioPnl,
  AutoScopeTradeSignalQuality,
  AutoScopeTradeTpStatus,
} from '@/lib/types';

type JsonObject = Record<string, unknown>;

interface AutoScopeAnalysisExtra {
  signalQuality: AutoScopeTradeSignalQuality | null;
  riskFlags: AutoScopeTradeRiskFlags;
  scenarioPnl: AutoScopeTradeScenarioPnl;
  executionTelemetry: AutoScopeTradeExecutionTelemetry | null;
  positionSnapshot: AutoScopeTradePositionSnapshot;
  tpStatus: AutoScopeTradeTpStatus;
  forensic: AutoScopeTradeForensicSummary;
}

interface AnalysisExtraRowDb {
  root_builder_order_id: number;
  run_id: number;
  market_slug: string;
  outcome_label: string;
  row_type: 'sell_exit' | 'settled_payout' | 'open_position';
  exit_reason: string;
  row_qty: number;
  row_pnl_usdc: number;
  cost_basis_usdc: number | null;
  valuation_kind: 'realized' | 'settled' | 'mark_to_market' | null;
  triggered_at: string | null;
  buy_filled_at: string | null;
  sell_filled_at: string | null;
  mark_price_captured_at: string | null;
  updated_at: string;
}

interface GuardEventDb {
  builder_order_id: number;
  payload_json: JsonObject;
}

interface ExecutionTelemetryEventDb {
  builder_order_id: number;
  event_type: 'submitted' | 'filled';
  payload_json: JsonObject;
}

interface DecisionLogEventDb {
  event_id: string;
  event_type: string;
  event_ts: string;
  created_at: string;
  decision_id: string | null;
  sl_event_id: string | null;
  fill_event_id: string | null;
  order_id: string | null;
  root_order_id: string | null;
  payload: JsonObject;
}

interface NodeSnapshotDb {
  root_order_id: number;
  order_id: number;
  node_key: string;
  node_type: string;
  node_config_hash: string;
  snapshot_json: JsonObject;
}

interface TpOrderDb {
  parent_order_id: number;
  status: string;
  trigger_price: number | null;
  working_price: number | null;
  last_seen_price: number | null;
  target_qty: number | null;
  remaining_qty: number | null;
  filled_qty: number | null;
}

interface BlockedSignalEventDb {
  event_type: string;
  payload_json: JsonObject;
  created_at: string;
}

const EMPTY_RISK_FLAGS: AutoScopeTradeRiskFlags = {
  highPrice: false,
  stale: false,
  fallingKnife: false,
  chop: false,
  reasons: [],
};

function roundMetric(value: number): number {
  return Math.round(value * 10_000) / 10_000;
}

function finiteNumber(value: unknown): number | null {
  if (typeof value === 'number') return Number.isFinite(value) ? value : null;
  if (typeof value === 'string' && value.trim() !== '') {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function textValue(value: unknown): string | null {
  return typeof value === 'string' && value.trim() !== '' ? value.trim() : null;
}

function boolValue(value: unknown): boolean | null {
  return typeof value === 'boolean' ? value : null;
}

function objectValue(value: unknown): JsonObject | null {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as JsonObject)
    : null;
}

function nestedObject(payload: JsonObject | null | undefined, path: string[]): JsonObject | null {
  let current: unknown = payload;
  for (const key of path) {
    current = objectValue(current)?.[key];
  }
  return objectValue(current);
}

function nestedNumber(payload: JsonObject | null | undefined, path: string[]): number | null {
  let current: unknown = payload;
  for (const key of path) {
    current = objectValue(current)?.[key];
  }
  return finiteNumber(current);
}

function nestedText(payload: JsonObject | null | undefined, path: string[]): string | null {
  let current: unknown = payload;
  for (const key of path) {
    current = objectValue(current)?.[key];
  }
  return textValue(current);
}

function nestedBool(payload: JsonObject | null | undefined, path: string[]): boolean | null {
  let current: unknown = payload;
  for (const key of path) {
    current = objectValue(current)?.[key];
  }
  return boolValue(current);
}

function firstNumber(payload: JsonObject | null | undefined, paths: string[][]): number | null {
  for (const path of paths) {
    const value = nestedNumber(payload, path);
    if (value != null) return value;
  }
  return null;
}

function firstText(payload: JsonObject | null | undefined, paths: string[][]): string | null {
  for (const path of paths) {
    const value = nestedText(payload, path);
    if (value) return value;
  }
  return null;
}

function guardPayloadFromEventPayload(payload: JsonObject | null | undefined): JsonObject | null {
  return (
    nestedObject(payload, ['price_to_beat_guard']) ??
    nestedObject(payload, ['price_to_beat_trigger_gate']) ??
    nestedObject(payload, ['primary_selection', 'selected_candidate', 'price_to_beat_guard']) ??
    null
  );
}

function buildExecutionTelemetry(
  submittedPayload: JsonObject | null,
  fillPayload: JsonObject | null
): AutoScopeTradeExecutionTelemetry | null {
  if (!submittedPayload && !fillPayload) return null;
  return {
    submittedBestAsk: firstNumber(submittedPayload, [['submitted_best_ask'], ['best_ask']]),
    submittedEstimatedAvgFill: firstNumber(submittedPayload, [
      ['submitted_estimated_avg_fill'],
      ['execution_price'],
    ]),
    submittedVwapSlippage: firstNumber(submittedPayload, [['submitted_vwap_slippage']]),
    submittedTargetQty: firstNumber(submittedPayload, [['submitted_target_qty'], ['target_qty'], ['size']]),
    submittedEstimatedNotional: firstNumber(submittedPayload, [['submitted_estimated_notional']]),
    submittedQFinal: firstNumber(submittedPayload, [['submitted_q_final']]),
    submittedModelBookGap: firstNumber(submittedPayload, [['submitted_model_book_gap']]),
    submittedModelBookZone: firstText(submittedPayload, [['submitted_model_book_zone']]),
    submittedParticipationCredit: firstNumber(submittedPayload, [['submitted_participation_credit']]),
    fillActualPrice: firstNumber(fillPayload, [['fill_actual_price'], ['execution_price']]),
    fillActualQty: firstNumber(fillPayload, [['fill_actual_qty'], ['actual_fill_qty'], ['canonical_entry_qty']]),
    fillActualNotional: firstNumber(fillPayload, [['fill_actual_notional']]),
    fillSlippageVsVwap: firstNumber(fillPayload, [['fill_slippage_vs_vwap']]),
    fillSlippageVsBestAsk: firstNumber(fillPayload, [['fill_slippage_vs_best_ask']]),
    fillSource: firstText(fillPayload, [['fill_source']]),
  };
}

function signalPayloadFromGuard(guard: JsonObject | null): JsonObject | null {
  return (
    nestedObject(guard, ['iv_mismatch_edge']) ??
    nestedObject(guard, ['signal_formula']) ??
    guard
  );
}

function signalMode(guard: JsonObject | null, signal: JsonObject | null): string | null {
  if (nestedObject(guard, ['iv_mismatch_edge'])) return 'iv_mismatch_edge';
  if (nestedObject(guard, ['signal_formula'])) return 'signal_formula';
  return firstText(guard ?? signal, [['threshold_mode'], ['configured_threshold_mode']]);
}

export function buildAutoScopeSignalQualityFromGuard(
  guard: JsonObject | null
): AutoScopeTradeSignalQuality | null {
  const signal = signalPayloadFromGuard(guard);
  if (!signal) return null;

  const q = firstNumber(signal, [['q_final'], ['q'], ['q_side']]);
  const qUp = firstNumber(signal, [['q_up']]);
  const qDown = firstNumber(signal, [['q_down']]);
  const cost = firstNumber(signal, [['cost']]);
  const threshold = firstNumber(signal, [['threshold'], ['edge_threshold']]);
  const dynamicThreshold = firstNumber(signal, [['dynamic_threshold']]) ?? threshold;
  const requiredQ =
    cost != null && dynamicThreshold != null ? roundMetric(cost + dynamicThreshold) : null;
  const qMargin = q != null && requiredQ != null ? roundMetric(q - requiredQ) : null;

  return {
    mode: signalMode(guard, signal),
    decisionReason: firstText(signal, [['decision_reason'], ['reason']]) ??
      firstText(guard, [['reason_code'], ['reason']]),
    passed: nestedBool(signal, ['passed']) ?? nestedBool(guard, ['passed']),
    selectedSide: firstText(signal, [['selected_side'], ['side']]),
    candidateSide: firstText(signal, [['candidate_side']]),
    q,
    qUp,
    qDown,
    cost,
    threshold,
    dynamicThreshold,
    requiredQ,
    qMargin,
    edge: firstNumber(signal, [['edge']]),
    edgeAdjusted: firstNumber(signal, [['edge_adj']]),
    secondsLeft: firstNumber(signal, [['seconds_left']]),
  };
}

function pushReason(reasons: string[], reason: string) {
  if (!reasons.includes(reason)) reasons.push(reason);
}

export function buildAutoScopeRiskFlagsFromGuard(
  guard: JsonObject | null
): AutoScopeTradeRiskFlags {
  const signal = signalPayloadFromGuard(guard);
  if (!signal) return { ...EMPTY_RISK_FLAGS, reasons: [] };

  const decisionReason = firstText(signal, [['decision_reason'], ['reason']]) ??
    firstText(guard, [['reason_code'], ['reason']]);
  const highPricePenalty = firstNumber(signal, [['high_price_penalty']]) ?? 0;
  const stalePenalty = firstNumber(signal, [['stale_penalty']]) ?? 0;
  const dropPenalty = firstNumber(signal, [['drop_penalty']]) ?? 0;
  const reasons: string[] = [];

  const highPrice = highPricePenalty > 0;
  const stale = stalePenalty > 0 || decisionReason === 'blocked_rtds_stale';
  const fallingKnife =
    dropPenalty > 0 ||
    decisionReason === 'blocked_falling_knife_drop' ||
    decisionReason === 'blocked_waiting_recovery';
  const chop =
    decisionReason === 'blocked_chop' || decisionReason === 'chop_filter_blocked';

  if (highPrice) pushReason(reasons, 'high_price');
  if (stale) pushReason(reasons, 'stale');
  if (fallingKnife) pushReason(reasons, 'falling_knife');
  if (chop) pushReason(reasons, 'chop');
  if (decisionReason?.startsWith('blocked_')) pushReason(reasons, decisionReason);

  return { highPrice, stale, fallingKnife, chop, reasons };
}

function outcomeSide(outcomeLabel: string): 'up' | 'down' | null {
  const normalized = outcomeLabel.trim().toLowerCase();
  if (['up', 'yes', 'long', 'bull'].includes(normalized)) return 'up';
  if (['down', 'no', 'short', 'bear'].includes(normalized)) return 'down';
  if (/\bup\b/.test(normalized)) return 'up';
  if (/\bdown\b/.test(normalized)) return 'down';
  return null;
}

function rowTimeMs(row: AnalysisExtraRowDb): number {
  const raw =
    row.buy_filled_at ??
    row.triggered_at ??
    row.sell_filled_at ??
    row.mark_price_captured_at ??
    row.updated_at;
  const parsed = new Date(raw).getTime();
  return Number.isFinite(parsed) ? parsed : 0;
}

function aggregateOpenPosition(rows: AnalysisExtraRowDb[]): AutoScopeTradePositionLegSnapshot {
  let upQty = 0;
  let downQty = 0;
  let costUsdc = 0;

  for (const row of rows) {
    if (row.row_type !== 'open_position') continue;
    const side = outcomeSide(row.outcome_label);
    if (side === 'up') upQty += Number(row.row_qty || 0);
    if (side === 'down') downQty += Number(row.row_qty || 0);
    costUsdc += Number(row.cost_basis_usdc || 0);
  }

  const floorQty = Math.min(upQty, downQty);
  return {
    upQty: roundMetric(upQty),
    downQty: roundMetric(downQty),
    costUsdc: roundMetric(costUsdc),
    floorQty: roundMetric(floorQty),
    floorPnlUsdc: roundMetric(floorQty - costUsdc),
  };
}

export function buildAutoScopeScenarioPnl(
  rows: AnalysisExtraRowDb[],
  signalQuality: AutoScopeTradeSignalQuality | null
): AutoScopeTradeScenarioPnl {
  const realizedPnlUsdc = roundMetric(
    rows
      .filter((row) => row.row_type === 'sell_exit' || row.row_type === 'settled_payout')
      .reduce((sum, row) => sum + Number(row.row_pnl_usdc || 0), 0)
  );
  const markPnlUsdc = roundMetric(
    rows
      .filter((row) => row.row_type === 'open_position')
      .reduce((sum, row) => sum + Number(row.row_pnl_usdc || 0), 0)
  );
  const openRows = rows.filter((row) => row.row_type === 'open_position');
  const openCostUsdc = roundMetric(
    openRows.reduce((sum, row) => sum + Number(row.cost_basis_usdc || 0), 0)
  );
  let openUpQty = 0;
  let openDownQty = 0;
  for (const row of openRows) {
    const side = outcomeSide(row.outcome_label);
    if (side === 'up') openUpQty += Number(row.row_qty || 0);
    if (side === 'down') openDownQty += Number(row.row_qty || 0);
  }

  const ifUpUsdc = roundMetric(realizedPnlUsdc + openUpQty - openCostUsdc);
  const ifDownUsdc = roundMetric(realizedPnlUsdc + openDownQty - openCostUsdc);
  const qUp = signalQuality?.qUp ?? null;
  const qDown = signalQuality?.qDown ?? null;
  const evUsdc =
    qUp != null && qDown != null
      ? roundMetric(qUp * ifUpUsdc + qDown * ifDownUsdc)
      : null;

  return {
    ifUpUsdc,
    ifDownUsdc,
    evUsdc,
    worstUsdc: roundMetric(Math.min(ifUpUsdc, ifDownUsdc)),
    realizedPnlUsdc,
    markPnlUsdc,
    openCostUsdc,
    qUp,
    qDown,
  };
}

function buildPositionSnapshot(
  rootOrderId: number,
  rootRows: AnalysisExtraRowDb[],
  runMarketRows: AnalysisExtraRowDb[]
): AutoScopeTradePositionSnapshot {
  const selectedTime = rootRows.reduce(
    (earliest, row) => Math.min(earliest, rowTimeMs(row)),
    Number.POSITIVE_INFINITY
  );
  const beforeRows = runMarketRows.filter(
    (row) =>
      row.root_builder_order_id !== rootOrderId &&
      rowTimeMs(row) < (Number.isFinite(selectedTime) ? selectedTime : Number.MAX_SAFE_INTEGER)
  );

  return {
    before: aggregateOpenPosition(beforeRows),
    after: aggregateOpenPosition([...beforeRows, ...rootRows]),
    basis: 'current_analysis_rows',
  };
}

function buildTpStatus(rootRows: AnalysisExtraRowDb[], tpOrders: TpOrderDb[]): AutoScopeTradeTpStatus {
  const orderCount = tpOrders.length;
  const openStatuses = new Set(['pending', 'armed', 'triggered', 'open', 'partially_filled', 'guard_blocked']);
  const filledQty = roundMetric(tpOrders.reduce((sum, order) => sum + Number(order.filled_qty || 0), 0));
  const targetQty = tpOrders.some((order) => order.target_qty != null)
    ? roundMetric(tpOrders.reduce((sum, order) => sum + Number(order.target_qty || 0), 0))
    : null;
  const remainingQty = tpOrders.some((order) => order.remaining_qty != null)
    ? roundMetric(tpOrders.reduce((sum, order) => sum + Number(order.remaining_qty || 0), 0))
    : null;
  const openOrderCount = tpOrders.filter((order) => openStatuses.has(order.status)).length;
  const realizedPnlUsdc = roundMetric(
    rootRows
      .filter((row) => row.row_type === 'sell_exit' && row.exit_reason === 'tp')
      .reduce((sum, row) => sum + Number(row.row_pnl_usdc || 0), 0)
  );
  const averagePrice =
    filledQty > 0
      ? roundMetric(
          tpOrders.reduce((sum, order) => {
            const price = order.last_seen_price ?? order.working_price ?? order.trigger_price ?? 0;
            return sum + price * Number(order.filled_qty || 0);
          }, 0) / filledQty
        )
      : null;
  const hasTerminalFill = filledQty > 0 && openOrderCount === 0;
  const allCanceled =
    orderCount > 0 && tpOrders.every((order) => order.status === 'canceled');
  const allExpired =
    orderCount > 0 && tpOrders.every((order) => order.status === 'expired');
  const partial =
    filledQty > 0 && (openOrderCount > 0 || (targetQty != null && filledQty < targetQty));

  return {
    configured: orderCount > 0,
    status:
      orderCount === 0
        ? 'disabled'
        : partial
          ? 'partial'
          : hasTerminalFill
            ? 'filled'
            : openOrderCount > 0
              ? 'open'
              : allCanceled
                ? 'canceled'
                : allExpired
                  ? 'expired'
                  : 'unknown',
    orderCount,
    openOrderCount,
    filledQty,
    targetQty,
    remainingQty,
    averagePrice,
    realizedPnlUsdc,
  };
}

function mapDecisionLogEvent(row: DecisionLogEventDb): AutoScopeTradeForensicEvent {
  return {
    eventId: row.event_id,
    eventType: row.event_type,
    eventTs: row.event_ts,
    createdAt: row.created_at,
    decisionId: row.decision_id,
    slEventId: row.sl_event_id,
    fillEventId: row.fill_event_id,
    orderId: row.order_id,
    payload: row.payload ?? {},
  };
}

function latestPayload(
  events: AutoScopeTradeForensicEvent[],
  eventTypes: string[]
): Record<string, unknown> | null {
  const wanted = new Set(eventTypes);
  for (let index = events.length - 1; index >= 0; index -= 1) {
    const event = events[index];
    if (wanted.has(event.eventType)) return event.payload;
  }
  return null;
}

function buildForensicSummary(
  events: AutoScopeTradeForensicEvent[],
  fallbackNodeSnapshot: JsonObject | null
): AutoScopeTradeForensicSummary {
  const orderLifecycleTypes = new Set([
    'ORDER_SUBMITTED',
    'ORDER_FILLED',
    'ORDER_PARTIALLY_FILLED',
    'ORDER_EXPIRED',
    'ORDER_CANCELED',
    'ORDER_ERROR',
    'ORDER_REPLACED',
    'ORDER_REARMED',
  ]);
  const entry = latestPayload(events, ['ENTRY_EVALUATED']);
  const nodeSnapshot = objectValue(entry?.node_snapshot) ?? fallbackNodeSnapshot;
  return {
    entryDecision: entry,
    entryStopLossPlan: objectValue(entry?.stop_loss_config_at_entry),
    nodeSnapshot,
    entryNodeKey:
      firstText(nodeSnapshot, [['node_key'], ['action_node', 'key']]) ??
      firstText(entry, [['workflow']]),
    entryNodeType: firstText(nodeSnapshot, [['node_type'], ['action_node', 'type']]),
    entryNodeConfigHash: firstText(nodeSnapshot, [['node_config_hash']]),
    orderLifecycle: events.filter((event) => orderLifecycleTypes.has(event.eventType)),
    stopLossTrigger: latestPayload(events, ['PTB_STOP_LOSS_TRIGGERED']),
    postSlRecovery: latestPayload(events, [
      'POST_SL_RESOLUTION_FINAL',
      'POST_SL_MARKET_END',
      'POST_SL_CHECK',
    ]),
    slClassification: latestPayload(events, ['POST_SL_RESOLUTION_FINAL']),
    rawEvents: events,
  };
}

function buildExtra(
  rootOrderId: number,
  rootRows: AnalysisExtraRowDb[],
  runMarketRows: AnalysisExtraRowDb[],
  guardPayload: JsonObject | null,
  tpOrders: TpOrderDb[],
  executionTelemetry: AutoScopeTradeExecutionTelemetry | null,
  forensicEvents: AutoScopeTradeForensicEvent[],
  nodeSnapshot: JsonObject | null
): AutoScopeAnalysisExtra {
  const signalQuality = buildAutoScopeSignalQualityFromGuard(guardPayload);
  return {
    signalQuality,
    riskFlags: buildAutoScopeRiskFlagsFromGuard(guardPayload),
    scenarioPnl: buildAutoScopeScenarioPnl(rootRows, signalQuality),
    executionTelemetry,
    positionSnapshot: buildPositionSnapshot(rootOrderId, rootRows, runMarketRows),
    tpStatus: buildTpStatus(rootRows, tpOrders),
    forensic: buildForensicSummary(forensicEvents, nodeSnapshot),
  };
}

function attachExtraToRow(
  row: AutoScopeTradeAnalysisRow,
  extra: AutoScopeAnalysisExtra | undefined
): AutoScopeTradeAnalysisRow {
  return extra ? { ...row, ...extra } : row;
}

export function attachExtraToDiagnostic(
  diagnostic: AutoScopeTradeDiagnostic,
  extra: AutoScopeAnalysisExtra | undefined
): AutoScopeTradeDiagnostic {
  return extra ? { ...diagnostic, ...extra } : diagnostic;
}

export function attachExtrasToRows(
  rows: AutoScopeTradeAnalysisRow[],
  extras: Map<number, AutoScopeAnalysisExtra>
): AutoScopeTradeAnalysisRow[] {
  return rows.map((row) => attachExtraToRow(row, extras.get(row.rootOrderId)));
}

export async function getAutoScopeTradeAnalysisExtrasForRoots(
  userId: number,
  rootOrderIds: number[]
): Promise<Map<number, AutoScopeAnalysisExtra>> {
  const ids = Array.from(new Set(rootOrderIds.filter((id) => Number.isFinite(id) && id > 0)));
  if (ids.length === 0) return new Map();

  const [
    rootRowsRes,
    runMarketRowsRes,
    guardEventsRes,
    telemetryEventsRes,
    tpOrdersRes,
    decisionLogsRes,
    nodeSnapshotsRes,
  ] = await Promise.all([
    pool.query<AnalysisExtraRowDb>(
      `SELECT
         root_builder_order_id,
         run_id,
         market_slug,
         outcome_label,
         row_type,
         exit_reason,
         row_qty,
         row_pnl_usdc,
         cost_basis_usdc,
         valuation_kind,
         triggered_at::text,
         buy_filled_at::text,
         sell_filled_at::text,
         mark_price_captured_at::text,
         updated_at::text
       FROM trade_flow_auto_scope_analysis_rows
       WHERE user_id = $1
         AND root_builder_order_id = ANY($2::bigint[])`,
      [userId, ids]
    ),
    pool.query<AnalysisExtraRowDb>(
      `WITH selected_scope AS (
         SELECT DISTINCT run_id, market_slug
         FROM trade_flow_auto_scope_analysis_rows
         WHERE user_id = $1
           AND root_builder_order_id = ANY($2::bigint[])
       )
       SELECT
         s.root_builder_order_id,
         s.run_id,
         s.market_slug,
         s.outcome_label,
         s.row_type,
         s.exit_reason,
         s.row_qty,
         s.row_pnl_usdc,
         s.cost_basis_usdc,
         s.valuation_kind,
         s.triggered_at::text,
         s.buy_filled_at::text,
         s.sell_filled_at::text,
         s.mark_price_captured_at::text,
         s.updated_at::text
       FROM trade_flow_auto_scope_analysis_rows s
       JOIN selected_scope x ON x.run_id = s.run_id AND x.market_slug = s.market_slug
       WHERE s.user_id = $1`,
      [userId, ids]
    ),
    pool.query<GuardEventDb>(
      `SELECT DISTINCT ON (e.builder_order_id)
         e.builder_order_id,
         e.payload_json
       FROM trade_builder_order_events e
       JOIN trade_builder_orders o ON o.id = e.builder_order_id
       WHERE o.user_id = $1
         AND e.builder_order_id = ANY($2::bigint[])
         AND e.payload_json ? 'price_to_beat_guard'
       ORDER BY e.builder_order_id, e.created_at DESC, e.id DESC`,
      [userId, ids]
    ),
    pool.query<ExecutionTelemetryEventDb>(
      `SELECT DISTINCT ON (e.builder_order_id, e.event_type)
         e.builder_order_id,
         e.event_type,
         e.payload_json
       FROM trade_builder_order_events e
       JOIN trade_builder_orders o ON o.id = e.builder_order_id
       WHERE o.user_id = $1
         AND e.builder_order_id = ANY($2::bigint[])
         AND e.event_type IN ('submitted', 'filled')
       ORDER BY e.builder_order_id, e.event_type, e.created_at DESC, e.id DESC`,
      [userId, ids]
    ),
    pool.query<TpOrderDb>(
      `SELECT
         parent_order_id,
         status,
         trigger_price,
         working_price,
         last_seen_price,
         target_qty,
         remaining_qty,
         filled_qty
       FROM trade_builder_orders
       WHERE user_id = $1
         AND parent_order_id = ANY($2::bigint[])
         AND side = 'sell'
         AND (exit_ladder_kind = 'tp' OR trigger_condition = 'cross_above')`,
      [userId, ids]
    ),
    pool.query<DecisionLogEventDb>(
      `SELECT
         l.event_id::text,
         l.event_type,
         l.event_ts::text,
         l.created_at::text,
         l.decision_id,
         l.sl_event_id,
         l.fill_event_id,
         l.order_id,
         l.root_order_id,
         l.payload
       FROM bot_decision_logs l
       JOIN trade_builder_orders o ON o.id::text = l.root_order_id
       WHERE o.user_id = $1
         AND l.root_order_id = ANY($2::text[])
       ORDER BY l.root_order_id ASC, l.event_ts ASC, l.created_at ASC, l.id ASC`,
      [userId, ids.map(String)]
    ),
    pool.query<NodeSnapshotDb>(
      `SELECT DISTINCT ON (s.root_order_id)
         s.root_order_id,
         s.order_id,
         s.node_key,
         s.node_type,
         s.node_config_hash,
         s.snapshot_json
       FROM trade_builder_order_node_snapshots s
       JOIN trade_builder_orders o ON o.id = s.root_order_id
       WHERE o.user_id = $1
         AND s.root_order_id = ANY($2::bigint[])
       ORDER BY s.root_order_id, s.updated_at DESC, s.id DESC`,
      [userId, ids]
    ),
  ]);

  const rootRowsById = new Map<number, AnalysisExtraRowDb[]>();
  for (const row of rootRowsRes.rows) {
    const rootId = Number(row.root_builder_order_id);
    rootRowsById.set(rootId, [...(rootRowsById.get(rootId) ?? []), row]);
  }

  const runMarketRowsByKey = new Map<string, AnalysisExtraRowDb[]>();
  for (const row of runMarketRowsRes.rows) {
    const key = `${row.run_id}:${row.market_slug}`;
    runMarketRowsByKey.set(key, [...(runMarketRowsByKey.get(key) ?? []), row]);
  }

  const guardByRootId = new Map<number, JsonObject | null>();
  for (const row of guardEventsRes.rows) {
    guardByRootId.set(Number(row.builder_order_id), guardPayloadFromEventPayload(row.payload_json));
  }

  const tpOrdersByRootId = new Map<number, TpOrderDb[]>();
  for (const order of tpOrdersRes.rows) {
    const rootId = Number(order.parent_order_id);
    tpOrdersByRootId.set(rootId, [...(tpOrdersByRootId.get(rootId) ?? []), order]);
  }

  const submittedByRootId = new Map<number, JsonObject>();
  const fillByRootId = new Map<number, JsonObject>();
  for (const event of telemetryEventsRes.rows) {
    const rootId = Number(event.builder_order_id);
    if (event.event_type === 'submitted') submittedByRootId.set(rootId, event.payload_json);
    if (event.event_type === 'filled') fillByRootId.set(rootId, event.payload_json);
  }

  const decisionLogsByRootId = new Map<number, AutoScopeTradeForensicEvent[]>();
  for (const row of decisionLogsRes.rows) {
    const rootId = Number(row.root_order_id);
    if (!Number.isFinite(rootId)) continue;
    decisionLogsByRootId.set(rootId, [
      ...(decisionLogsByRootId.get(rootId) ?? []),
      mapDecisionLogEvent(row),
    ]);
  }

  const nodeSnapshotByRootId = new Map<number, JsonObject>();
  for (const row of nodeSnapshotsRes.rows) {
    nodeSnapshotByRootId.set(Number(row.root_order_id), row.snapshot_json);
  }

  const extras = new Map<number, AutoScopeAnalysisExtra>();
  for (const rootId of ids) {
    const rootRows = rootRowsById.get(rootId) ?? [];
    const firstRow = rootRows[0];
    if (!firstRow) continue;
    const runMarketRows = runMarketRowsByKey.get(`${firstRow.run_id}:${firstRow.market_slug}`) ?? rootRows;
    extras.set(
      rootId,
      buildExtra(
        rootId,
        rootRows,
        runMarketRows,
        guardByRootId.get(rootId) ?? null,
        tpOrdersByRootId.get(rootId) ?? [],
        buildExecutionTelemetry(
          submittedByRootId.get(rootId) ?? null,
          fillByRootId.get(rootId) ?? null
        ),
        decisionLogsByRootId.get(rootId) ?? [],
        nodeSnapshotByRootId.get(rootId) ?? null
      )
    );
  }

  return extras;
}

function blockedReason(payload: JsonObject, guard: JsonObject | null): string | null {
  const signal = signalPayloadFromGuard(guard);
  return (
    firstText(signal, [['decision_reason'], ['reason']]) ??
    firstText(guard, [['reason_code'], ['reason']]) ??
    firstText(payload, [['reason_code'], ['reason'], ['blocked_by']])
  );
}

function noOrderTelemetryFromPayload(payload: JsonObject): AutoScopeNoOrderTelemetry | null {
  const finalActionStatus = firstText(payload, [['final_action_status']]);
  const orderNotCreated = boolValue(payload.order_not_created);
  if (finalActionStatus !== 'NO_ORDER' && orderNotCreated !== true) return null;

  return {
    orderCreated: boolValue(payload.order_created),
    orderSubmitted: boolValue(payload.order_submitted),
    orderFilled: boolValue(payload.order_filled),
    finalActionStatus,
    lastGuardName: firstText(payload, [['last_guard_name']]),
    lastGuardCode: firstText(payload, [['last_guard_code'], ['reason_code']]),
    lastGuardState: firstText(payload, [['last_guard_state']]),
    executionFloor: firstNumber(payload, [['execution_floor']]),
    bestAskAtWindowEnd: firstNumber(payload, [['best_ask_at_window_end'], ['best_ask_at_block']]),
    floorDistance: firstNumber(payload, [['floor_distance']]),
    floorWaitMs: firstNumber(payload, [['floor_wait_ms']]),
    liquidityRegime: firstText(payload, [['liquidity_regime']]),
    hourlyVolumeRatio: firstNumber(payload, [['hourly_volume_ratio']]),
    volume30s: firstNumber(payload, [['volume_30s']]),
    tradeCount60s: firstNumber(payload, [['trade_count_60s']]),
    quoteSnapshotSource: firstText(payload, [['quote_snapshot_source']]),
    bookDataStatus: firstText(payload, [['book_data_status']]),
    quoteMissingReason: firstText(payload, [['quote_missing_reason']]),
    selectedBid: firstNumber(payload, [['selected_bid']]),
    selectedAsk: firstNumber(payload, [['selected_ask']]),
    selectedMid: firstNumber(payload, [['selected_mid']]),
    upBid: firstNumber(payload, [['up_bid']]),
    upAsk: firstNumber(payload, [['up_ask']]),
    downBid: firstNumber(payload, [['down_bid']]),
    downAsk: firstNumber(payload, [['down_ask']]),
    bookSide: firstText(payload, [['book_side']]),
    upMid: firstNumber(payload, [['up_mid']]),
    downMid: firstNumber(payload, [['down_mid']]),
    bookMidDiff: firstNumber(payload, [['book_mid_diff']]),
    whyNoOrderSummary: firstText(payload, [['why_no_order_summary']]),
    humanReadableReason: firstText(payload, [['human_readable_reason']]),
  };
}

function mapBlockedSignal(row: BlockedSignalEventDb): AutoScopeTradeBlockedSignal {
  const payload = row.payload_json ?? {};
  const guard = guardPayloadFromEventPayload(payload);
  return {
    eventType: row.event_type,
    createdAt: row.created_at,
    nodeKey: firstText(payload, [['node_key']]),
    marketSlug: firstText(payload, [['market_slug'], ['resolved_market_slug']]),
    outcomeLabel: firstText(payload, [['outcome_label']]),
    reasonCode: blockedReason(payload, guard),
    reasonDetail:
      firstText(guard, [['reason_detail']]) ?? firstText(payload, [['reason_detail']]),
    signalQuality: buildAutoScopeSignalQualityFromGuard(guard),
    riskFlags: buildAutoScopeRiskFlagsFromGuard(guard),
    noOrderTelemetry: noOrderTelemetryFromPayload(payload),
  };
}

export async function getAutoScopeBlockedSignalsForRun(params: {
  userId: number;
  runId: number | null;
  limit?: number;
}): Promise<AutoScopeTradeBlockedSignal[]> {
  if (!params.runId) return [];
  const limit = Math.min(Math.max(params.limit ?? 100, 1), 200);
  const res = await pool.query<BlockedSignalEventDb>(
    `SELECT
       e.event_type,
       e.payload_json,
       e.created_at::text
     FROM trade_flow_events e
     JOIN trade_flow_runs r ON r.id = e.run_id
     WHERE r.user_id = $1
       AND e.run_id = $2
       AND (
         e.event_type ILIKE '%blocked%'
         OR e.event_type IN (
           'pre_order_price_to_beat_blocked',
           'trigger_ws_price_to_beat_gate_blocked',
           'trigger_cycle_window_price_to_beat_gate_blocked',
           'missed_market_order_not_filled_notification_sent'
         )
       )
     ORDER BY e.created_at DESC, e.id DESC
     LIMIT $3`,
    [params.userId, params.runId, limit]
  );
  return res.rows.map(mapBlockedSignal);
}

export async function getAutoScopeNoOrderSignalsForRun(params: {
  userId: number;
  runId: number;
  limit?: number;
}): Promise<AutoScopeTradeBlockedSignal[]> {
  return getAutoScopeNoOrderSignalsForExport({
    userId: params.userId,
    runId: params.runId,
    limit: params.limit,
  });
}

export async function getAutoScopeNoOrderSignalsForExport(params: {
  userId: number;
  runId?: number | null;
  from?: string | null;
  to?: string | null;
  limit?: number;
}): Promise<AutoScopeTradeBlockedSignal[]> {
  const limit = Math.min(Math.max(params.limit ?? 1000, 1), 2000);
  const values: Array<number | string> = [params.userId];
  const clauses = [
    'r.user_id = $1',
    "e.event_type = 'missed_market_order_not_filled_notification_sent'",
  ];
  if (params.runId) {
    values.push(params.runId);
    clauses.push(`e.run_id = $${values.length}`);
  }
  if (params.from) {
    values.push(params.from);
    clauses.push(`e.created_at >= $${values.length}::timestamptz`);
  }
  if (params.to) {
    values.push(params.to);
    clauses.push(`e.created_at <= $${values.length}::timestamptz`);
  }
  values.push(limit);
  const res = await pool.query<BlockedSignalEventDb>(
    `SELECT
       e.event_type,
       e.payload_json,
       e.created_at::text
     FROM trade_flow_events e
     JOIN trade_flow_runs r ON r.id = e.run_id
     WHERE ${clauses.join(' AND ')}
     ORDER BY e.created_at DESC, e.id DESC
     LIMIT $${values.length}`,
    values
  );
  return res.rows.map(mapBlockedSignal).filter((signal) => signal.noOrderTelemetry);
}

function csvField(value: string | number | boolean | null): string {
  if (value == null) return '';
  const text = String(value);
  if (!/[",\r\n]/.test(text)) return text;
  return `"${text.replaceAll('"', '""')}"`;
}

export function buildAutoScopeNoOrderSignalsCsv(
  signals: AutoScopeTradeBlockedSignal[]
): string {
  const headers = [
    'created_at',
    'event_type',
    'node_key',
    'market_slug',
    'outcome_label',
    'order_created',
    'order_submitted',
    'order_filled',
    'final_action_status',
    'last_guard_name',
    'last_guard_code',
    'last_guard_state',
    'execution_floor',
    'best_ask_at_window_end',
    'floor_distance',
    'floor_wait_ms',
    'liquidity_regime',
    'hourly_volume_ratio',
    'volume_30s',
    'trade_count_60s',
    'quote_snapshot_source',
    'book_data_status',
    'quote_missing_reason',
    'selected_bid',
    'selected_ask',
    'selected_mid',
    'up_bid',
    'up_ask',
    'down_bid',
    'down_ask',
    'book_side',
    'up_mid',
    'down_mid',
    'book_mid_diff',
    'why_no_order_summary',
    'human_readable_reason',
  ];
  const lines = [headers.map(csvField).join(',')];

  for (const signal of signals) {
    const noOrder = signal.noOrderTelemetry;
    if (!noOrder) continue;
    lines.push(
      [
        signal.createdAt,
        signal.eventType,
        signal.nodeKey,
        signal.marketSlug,
        signal.outcomeLabel,
        noOrder.orderCreated,
        noOrder.orderSubmitted,
        noOrder.orderFilled,
        noOrder.finalActionStatus,
        noOrder.lastGuardName,
        noOrder.lastGuardCode,
        noOrder.lastGuardState,
        noOrder.executionFloor,
        noOrder.bestAskAtWindowEnd,
        noOrder.floorDistance,
        noOrder.floorWaitMs,
        noOrder.liquidityRegime,
        noOrder.hourlyVolumeRatio,
        noOrder.volume30s,
        noOrder.tradeCount60s,
        noOrder.quoteSnapshotSource,
        noOrder.bookDataStatus,
        noOrder.quoteMissingReason,
        noOrder.selectedBid,
        noOrder.selectedAsk,
        noOrder.selectedMid,
        noOrder.upBid,
        noOrder.upAsk,
        noOrder.downBid,
        noOrder.downAsk,
        noOrder.bookSide,
        noOrder.upMid,
        noOrder.downMid,
        noOrder.bookMidDiff,
        noOrder.whyNoOrderSummary,
        noOrder.humanReadableReason,
      ]
        .map(csvField)
        .join(',')
    );
  }

  return `${lines.join('\n')}\n`;
}

export const __autoScopeAnalysisExtrasTestUtils = {
  buildAutoScopeSignalQualityFromGuard,
  buildAutoScopeRiskFlagsFromGuard,
  buildAutoScopeScenarioPnl,
};
