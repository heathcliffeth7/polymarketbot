export type TradeState =
  | 'Idle'
  | 'WaitingEntry'
  | 'EntryPlaced'
  | 'EntryPartiallyFilled'
  | 'EntryFilled'
  | 'TpPlaced'
  | 'SlArmed'
  | 'ExitPartiallyFilled'
  | 'ExitFilled'
  | 'Settled'
  | 'Halted';

export type OrderIntent = 'entry' | 'tp' | 'sl' | 'renewal';
export type OrderStatus = 'pending' | 'open' | 'partially_filled' | 'filled' | 'canceled' | 'rejected' | 'expired';
export type RiskDecision = 'allow' | 'block' | 'halt';
export type KillSwitchMode = 'disabled' | 'manual_only' | 'manual_or_policy';
export type ExecutionMode = 'paper' | 'live';
export type MarketDiscoveryState = 'ready' | 'waiting_for_market' | 'error';
export type BuilderOrderKind = 'immediate' | 'conditional';
export type BuilderOrderStatus =
  | 'pending'
  | 'armed'
  | 'triggered'
  | 'open'
  | 'partially_filled'
  | 'filled'
  | 'canceled_requested'
  | 'completed'
  | 'canceled'
  | 'expired'
  | 'blocked'
  | 'guard_blocked'
  | 'error';
export type TriggerCondition = 'cross_above' | 'cross_below';
export type BuilderWorkflowStatus =
  | 'draft'
  | 'armed'
  | 'running'
  | 'completed'
  | 'canceled'
  | 'expired'
  | 'error';
export type BuilderWorkflowLegType = 'sell' | 'buy';
export type BuilderWorkflowLegStatus =
  | 'pending'
  | 'armed'
  | 'waiting_sell_progress'
  | 'open'
  | 'partially_filled'
  | 'completed'
  | 'blocked'
  | 'canceled'
  | 'expired'
  | 'error';
export type BuyTriggerMode =
  | 'sell_progress_only'
  | 'price_only'
  | 'sell_progress_and_price';
export type TradeFlowDefinitionStatus = 'draft' | 'published' | 'archived';
export type TradeFlowVersionStatus = 'draft' | 'published' | 'archived';
export type TradeFlowRunStatus = 'queued' | 'running' | 'completed' | 'failed' | 'canceled';
export type TradeFlowStepStatus = 'queued' | 'running' | 'completed' | 'failed' | 'skipped' | 'canceled';

export interface Market {
  id: number;
  market_slug: string;
  starts_at: string;
  ends_at: string;
  status: string;
}

export interface Trade {
  id: number;
  market_id: number;
  market_slug?: string;
  state: TradeState;
  entry_price: number | null;
  exit_price: number | null;
  notional_usdc: number;
  realized_pnl: number | null;
  opened_at: string | null;
  closed_at: string | null;
}

export interface Order {
  id: number;
  trade_id: number;
  exchange_order_id: string;
  client_order_id: string | null;
  intent: string;
  side: string;
  price: number;
  size: number;
  status: string;
  last_exchange_status: string | null;
  reject_reason: string | null;
  created_at: string;
  updated_at: string | null;
}

export interface Fill {
  id: number;
  order_id: number;
  fill_id: string;
  price: number;
  size: number;
  fee: number;
  filled_at: string;
}

export interface RiskEvent {
  id: number;
  trade_id: number | null;
  event_type: string;
  decision: string;
  details: string;
  created_at: string;
}

export interface BotRun {
  id: number;
  mode: string;
  version: string;
  started_at: string;
  stopped_at: string | null;
  reason: string | null;
}

export interface ClaimSweepQueueStatus {
  pending: number;
  retry: number;
  processing: number;
  submitted: number;
  failed: number;
  claimed: number;
}

export interface ClaimSweepStatus {
  thresholdUsdc: number;
  walletAddress: string | null;
  executionMode: 'direct' | 'builder_relayer' | 'relayer_api_key';
  claimEnabled: boolean;
  publishedAutoClaimFlow: boolean;
  canSweep: boolean;
  disabledReasonCode: string | null;
  disabledReason: string | null;
  eligibleCount: number;
  eligibleTotalUsdc: number;
  queue: ClaimSweepQueueStatus;
  lastError: string | null;
  refreshedAt: string | null;
}

export interface ClaimSweepRunResult {
  thresholdUsdc: number;
  walletAddress: string | null;
  eligibleCount: number;
  eligibleTotalUsdc: number;
  queuedNewCount: number;
  rearmedCount: number;
  alreadyTrackedCount: number;
  queue: ClaimSweepQueueStatus;
  refreshedAt: string;
}

export interface DashboardData {
  botStatus: {
    serviceActive: boolean;
    lastRun: BotRun | null;
    controlAvailable?: boolean;
    controlReason?: string | null;
    controlReasonCode?: string | null;
    marketDiscoveryState?: MarketDiscoveryState;
    selectedMarketSlug?: string | null;
    marketDiscoveryMessage?: string | null;
  };
  activeTrade: Trade | null;
  dailyPnl: {
    totalPnl: number;
    tradeCount: number;
    winCount: number;
    lossCount: number;
  };
  recentTrades: Trade[];
  riskSummary: {
    openOrders: number;
    consecutiveLosses: number;
    haltCount: number;
    killSwitchActive: boolean;
  };
  activePosition?: {
    tradeId: number;
    marketSlug: string;
    legs: Array<{
      legSide: 'yes' | 'no';
      tokenId: string;
      qty: number;
      avgEntry: number;
      levelsFilled: number;
      lastFillPrice: number | null;
      updatedAt: string | null;
    }>;
  } | null;
  pressure?: {
    tradeId: number;
    pressureScore: number;
    bidAskImbalance: number | null;
    sellRatio: number | null;
    yesPrice: number | null;
    noPrice: number | null;
    triggerReason: string | null;
    triggered: boolean;
    updatedAt: string | null;
  } | null;
  positionExitRules?: Array<{
    legSide: 'yes' | 'no';
    dropSellPct: number;
    enabled: boolean;
    updatedAt: string | null;
  }>;
  claimSweep: ClaimSweepStatus;
}

export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface BotStatusResponse {
  serviceActive: boolean;
  lastRun: BotRun | null;
  controlAvailable: boolean;
  controlReason: string | null;
  controlReasonCode: string | null;
  marketDiscoveryState: MarketDiscoveryState;
  selectedMarketSlug: string | null;
  marketDiscoveryMessage: string | null;
}

export interface StrategyConfig {
  entry_price: number;
  tp_pct: number;
  base_sl_pct: number;
  aggressive_sl_pct: number;
  entry_window_sec: number;
  max_hold_sec: number;
  sl_renew_interval_ms: number;
}

export interface RiskConfig {
  max_daily_loss_usdc: number;
  max_consecutive_losses: number;
  max_notional_per_market_usdc: number;
  max_open_orders: number;
  max_stale_data_ms: number;
  kill_switch_mode: KillSwitchMode;
  manual_kill_switch_active: boolean;
}

export interface ExecutionConfig {
  order_type: string;
  time_in_force: string;
  retry_count: number;
  retry_backoff_ms: number;
  reconcile_interval_ms: number;
}

export interface BotConfig {
  mode: ExecutionMode;
  market_scope: string;
  market_slug_override: string;
  loop_interval_ms: number;
  market_discovery_retry_interval_ms: number;
  market_discovery_timeout_sec: number;
  market_selection: string;
}

export interface TradeBuilderOrder {
  id: number;
  trade_id: number;
  kind: BuilderOrderKind;
  status: BuilderOrderStatus;
  market_slug: string;
  token_id: string;
  outcome_label: string;
  side: 'buy' | 'sell';
  execution_mode: 'limit' | 'market';
  trigger_condition: TriggerCondition | null;
  trigger_price: number | null;
  max_price: number | null;
  guard_trigger_price: number | null;
  best_ask_floor_price: number | null;
  size_usdc: number;
  min_price_distance_cent: number;
  expires_at: string | null;
  max_triggers: number;
  triggers_fired: number;
  active_exchange_order_id: string | null;
  remaining_size: number | null;
  working_price: number | null;
  last_seen_price: number | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
  parent_order_id: number | null;
  origin_flow_definition_id: number | null;
  origin_flow_run_id: number | null;
  origin_flow_node_key: string | null;
  tp_enabled: boolean;
  tp_price: number | null;
  sl_enabled: boolean;
  sl_price: number | null;
}

export interface TradeBuilderOrderEvent {
  id: number;
  builder_order_id: number;
  event_type: string;
  payload_json: unknown;
  created_at: string;
}

export interface TradeBuilderOrderDiagnosticSummary {
  buy_created: boolean;
  processing_started: boolean;
  guards_ran: boolean;
  builder_guards_ran: boolean;
  price_to_beat_ran: boolean;
  price_to_beat_decision: string | null;
  price_to_beat_reason_code: string | null;
  last_guard_scope: string | null;
  last_guard_decision: string | null;
  last_guard_reason_code: string | null;
  submit_attempted: boolean;
  effective_outcome: string;
  effective_reason_code: string | null;
}

export interface TradeBuilderOrderEventsResponse
  extends PaginatedResponse<TradeBuilderOrderEvent> {
  diagnostic_summary: TradeBuilderOrderDiagnosticSummary;
}

export interface TradeBuilderWorkflow {
  id: number;
  name: string;
  status: BuilderWorkflowStatus;
  source_trade_id: number;
  sell_target_pct: number;
  buy_start_after_sell_progress_pct: number;
  buy_trigger_mode: BuyTriggerMode;
  buy_allocation_pct: number;
  expires_at: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface TradeBuilderWorkflowLeg {
  id: number;
  workflow_id: number;
  leg_type: BuilderWorkflowLegType;
  market_slug: string;
  token_id: string;
  outcome_label: string;
  side: 'buy' | 'sell';
  trigger_condition: TriggerCondition | null;
  trigger_price: number | null;
  min_price_distance_cent: number;
  status: BuilderWorkflowLegStatus;
  builder_order_id: number | null;
  target_notional_usdc: number;
  allocated_notional_usdc: number;
  filled_notional_usdc: number;
  filled_qty: number;
  last_seen_price: number | null;
  created_at: string;
  updated_at: string;
}

export interface TradeBuilderWorkflowEvent {
  id: number;
  workflow_id: number;
  leg_id: number | null;
  event_type: string;
  payload_json: unknown;
  created_at: string;
}

export interface TradeBuilderWorkflowDetail {
  workflow: TradeBuilderWorkflow;
  legs: TradeBuilderWorkflowLeg[];
}

export interface TradeBuilderMarketSearchItem {
  slug: string;
  title: string;
  endDate: string | null;
  active: boolean;
}

export interface TradeBuilderOutcome {
  token_id: string;
  label: string;
  price: number | null;
}

export interface TradeFlowOpenPositionOption {
  positionKey: string;
  marketTitle: string;
  marketSlug: string;
  tokenId: string;
  outcomeLabel: string;
  size: number;
  avgPrice: number | null;
  currentValue: number | null;
  unrealizedPnl: number | null;
  walletAddress: string;
  matchedTradeId: number | null;
  matchConfidence: 'exact' | 'market_token' | 'none';
}

export interface TradeFlowOpenPositionsMeta {
  walletAddressUsed: string;
  count: number;
  minCurrentValueUsd: number;
  fetchedAt: string;
}

export interface TradeFlowOpenPositionsResponse {
  data: TradeFlowOpenPositionOption[];
  meta: TradeFlowOpenPositionsMeta;
}

export interface TradeFlowEnsureSourceTradeRequest {
  marketSlug: string;
  tokenId: string;
  outcomeLabel: string;
  marketTitle?: string | null;
  size?: number | null;
  avgPrice?: number | null;
  currentValue?: number | null;
}

export interface TradeFlowEnsureSourceTradeResult {
  sourceTradeId: number;
  created: boolean;
}

export interface TradeFlowEnsureDualDcaSourceTradeRequest {
  asset: 'btc' | 'eth' | 'sol' | 'xrp';
  timeframe: '5m' | '15m';
  definitionId?: number | null;
  nodeKey?: string | null;
}

export interface TradeFlowEnsureDualDcaSourceTradeResult {
  sourceTradeId: number;
  created: boolean;
}

export interface TradeFlowNode {
  key: string;
  type: string;
  positionX: number | null;
  positionY: number | null;
  config: Record<string, unknown>;
}

export interface TradeFlowEdge {
  key: string;
  source: string;
  target: string;
  type: string;
  condition: Record<string, unknown> | null;
}

export interface TradeFlowGraph {
  context: Record<string, unknown>;
  nodes: TradeFlowNode[];
  edges: TradeFlowEdge[];
}

export interface TradeFlowVersion {
  id: number;
  definition_id: number;
  version_no: number;
  status: TradeFlowVersionStatus;
  graph_json: TradeFlowGraph;
  published_at: string | null;
  created_at: string;
}

export interface TradeFlowDefinition {
  id: number;
  name: string;
  description: string | null;
  status: TradeFlowDefinitionStatus;
  draft_version_id: number | null;
  published_version_id: number | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
  legacy_workflow_id: number | null;
}

export interface TradeFlowDefinitionDetail {
  definition: TradeFlowDefinition;
  draftVersion: TradeFlowVersion | null;
  publishedVersion: TradeFlowVersion | null;
}

export interface TradeFlowValidationIssue {
  severity: 'error' | 'warning';
  code: string;
  message: string;
  nodeKey?: string;
  edgeKey?: string;
}

export interface TradeFlowValidationResult {
  valid: boolean;
  issues: TradeFlowValidationIssue[];
  stats: {
    nodes: number;
    edges: number;
    triggers: number;
    actions: number;
  };
}

export interface TradeFlowRun {
  id: number;
  definition_id: number;
  version_id: number;
  status: TradeFlowRunStatus;
  trigger_source: string | null;
  context_json: Record<string, unknown>;
  started_at: string | null;
  ended_at: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface TradeFlowRunStep {
  id: number;
  run_id: number;
  node_key: string;
  node_type: string;
  status: TradeFlowStepStatus;
  attempt: number;
  input_json: Record<string, unknown> | null;
  output_json: Record<string, unknown> | null;
  error_text: string | null;
  started_at: string | null;
  ended_at: string | null;
  created_at: string;
}

export interface TradeFlowEvent {
  id: number;
  run_id: number | null;
  definition_id: number;
  version_id: number | null;
  definition_name?: string | null;
  event_type: string;
  payload_json: Record<string, unknown>;
  created_at: string;
}

export interface TradeFlowOverlapPeer {
  market_slug: string;
  token_id: string;
  side: 'buy' | 'sell';
  definition_id: number;
  definition_name: string | null;
  run_id: number | null;
  node_key: string | null;
  source_trade_id: number;
  active_order_count: number;
}

export interface TradeFlowOverlapGroup {
  market_slug: string;
  overlap_type: 'cross_flow' | 'intra_flow' | 'both';
  cross_flow: boolean;
  intra_flow: boolean;
  peers: TradeFlowOverlapPeer[];
}

export interface TradeFlowRealtimePriceTick {
  kind: 'price_tick';
  run_id: number;
  definition_id: number;
  version_id: number;
  node_key: string;
  node_type: string;
  market_slug: string | null;
  token_id: string;
  outcome_label: string | null;
  price: number;
  best_bid: number | null;
  best_ask: number | null;
  last_trade_price: number | null;
  price_mode: string;
  price_source: string;
  price_source_detail: string | null;
  evaluation_mode: string | null;
  snapshot_age_ms: number | null;
  event_ts: number | null;
  created_at: string;
}

export interface TradeFlowRealtimeHeartbeat {
  kind: 'heartbeat';
  now: string;
}

export interface TradeFlowRealtimeReady {
  kind: 'ready';
  connected_at: string;
}

export type TradeFlowRealtimePayload =
  | TradeFlowEvent
  | TradeFlowRealtimePriceTick
  | TradeFlowRealtimeHeartbeat
  | TradeFlowRealtimeReady;

export type NodeExecutionStatus = 'idle' | 'running' | 'completed' | 'failed' | 'skipped';

export interface NodeExecutionState {
  nodeKey: string;
  status: NodeExecutionStatus;
  startedAt: string | null;
  endedAt: string | null;
  error: string | null;
}

export type ExpressionGroupOperator = 'and' | 'or';

export interface ExpressionLeaf {
  type: 'leaf';
  leftVar: string;
  operator: '>' | '>=' | '<' | '<=' | '==' | '!=' | 'in' | 'contains' | 'between';
  rightValue: unknown;
  rightType: 'number' | 'string' | 'boolean';
}

export interface ExpressionGroup {
  type: 'group';
  operator: ExpressionGroupOperator;
  children: Array<ExpressionLeaf | ExpressionGroup>;
}
