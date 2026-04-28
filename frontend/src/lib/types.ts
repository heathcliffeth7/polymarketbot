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
export type TradeBuilderExitLadderKind = 'tp' | 'sl' | 'ptb_sl';
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
  max_price_relax_enabled: boolean;
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
  runtime_snapshot_json?: unknown;
  fresh_submit_lease_until?: string | null;
  created_at: string;
  updated_at: string;
  parent_order_id: number | null;
  origin_flow_definition_id: number | null;
  origin_flow_run_id: number | null;
  origin_flow_node_key: string | null;
  pair_session_id?: number | null;
  pair_leg_role?: 'lead_candidate' | 'counter_candidate' | 'completion_buy' | 'orphan_unwind_sell' | null;
  tp_enabled: boolean;
  tp_price: number | null;
  tp_rules_json?: Array<{
    price: number;
    size_pct: number;
  }>;
  sl_enabled: boolean;
  sl_price: number | null;
  sl_rules_json?: Array<{
    price: number;
    size_pct: number;
  }>;
  ptb_stop_loss_rules_json?: Array<{
    gap_usd: number;
    size_pct: number;
  }>;
  staged_sl_retry_only_dust?: boolean;
  staged_sl_retry_dust_metric?: 'notional' | 'qty' | null;
  staged_sl_retry_dust_value?: number | null;
  staged_sl_reentry_use_sold_notional?: boolean;
  staged_sl_reentry_only_after_all_stages?: boolean;
  time_exit_rules_json?: Array<{
    elapsed_minutes: number;
    remaining_pct: number;
  }>;
  exit_ladder_kind?: TradeBuilderExitLadderKind | null;
  exit_ladder_index?: number | null;
  exit_ladder_size_pct?: number | null;
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
  legSide: 'yes' | 'no';
  feeRateBps?: number | null;
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
  draftCustomRangeSnapshots: TradeFlowCustomRangeSnapshot[];
  publishedCustomRangeSnapshots: TradeFlowCustomRangeSnapshot[];
}

export interface TradeFlowCustomRangeSnapshot {
  nodeKey: string;
  startSec: number;
  endSec: number;
  autoSellOnWindowEnd: boolean;
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
  version_no?: number | null;
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

export type AutoScopeTradeAnalysisRowType =
  | 'sell_exit'
  | 'settled_payout'
  | 'open_position'
  | 'pending_analysis';
export type AutoScopeTradeAnalysisPositionState =
  | 'open'
  | 'pending_analysis'
  | 'closed_market_ended'
  | 'closed_exit';
export type AutoScopeTradeAnalysisSortBy = 'default' | 'pnl';
export type AutoScopeTradeAnalysisSortDirection = 'asc' | 'desc';
export type AutoScopeTradeAnalysisPnlFilter = 'all' | 'loss' | 'profit';
export type AutoScopeTradeAnalysisPositionFilter = 'all' | 'realized' | 'open';
export type AutoScopeTradeAnalysisTimeRange =
  | 'all'
  | '3h'
  | '6h'
  | '12h'
  | '24h'
  | '1w'
  | '1m'
  | 'custom';
export type AutoScopeTradeAnalysisValuationKind = 'realized' | 'settled' | 'mark_to_market';
export type AutoScopeTradeDiagnosisCode =
  | 'bad_entry_price'
  | 'late_entry'
  | 'slow_fill'
  | 'thin_liquidity'
  | 'stop_loss_expected'
  | 'exit_too_late'
  | 'market_reversal'
  | 'fee_drag'
  | 'unrealized_mark_loss'
  | 'take_profit_success'
  | 'clean_win'
  | 'unknown';

export type AutoScopeTradeAnalysisExitReason =
  | 'tp'
  | 'sl'
  | 'window_end_auto_sell'
  | 'other'
  | 'open_position'
  | 'pending_analysis';

export interface AutoScopeTradeDiagnostic {
  rootOrderId: number;
  userId: number;
  definitionId: number;
  runId: number;
  marketSlug: string;
  tokenId: string;
  outcomeLabel: string;
  totalPnlUsdc: number;
  realizedPnlUsdc: number;
  openPnlUsdc: number;
  pnlPct: number | null;
  feeDragUsdc: number;
  costBasisUsdc: number;
  netValueUsdc: number;
  cashFillPnlUsdc?: number | null;
  diagnosticPnlUsdc?: number | null;
  economicPnlUsdc?: number | null;
  cashBuyUsdc?: number | null;
  cashSellUsdc?: number | null;
  cashRedeemUsdc?: number | null;
  officialRootPnlUsdc?: number | null;
  officialPnlSource?: string | null;
  officialBuyUsdc?: number | null;
  officialSellUsdc?: number | null;
  officialRedeemUsdc?: number | null;
  officialDeltaUsdc?: number | null;
  polymarketPositionPnlUsdc?: number | null;
  polymarketPositionSource?: string | null;
  polymarketTotalBetUsdc?: number | null;
  polymarketAmountReturnedUsdc?: number | null;
  polymarketRealizedPnlUsdc?: number | null;
  polymarketCashPnlUsdc?: number | null;
  pendingInventoryQty?: number | null;
  pendingInventoryValueUsdc?: number | null;
  pendingRedeemableValueUsdc?: number | null;
  cashStatus?: string | null;
  entryTriggerPrice: number | null;
  entrySubmitPrice: number | null;
  entryFillPrice: number | null;
  entryReferencePrice: number | null;
  entrySlippageUsdc: number | null;
  entryQualityScore: number | null;
  exitReason: AutoScopeTradeAnalysisExitReason | string | null;
  exitPrice: number | null;
  bestPriceDuringHold: number | null;
  worstPriceDuringHold: number | null;
  maxFavorableUsdc: number | null;
  maxAdverseUsdc: number | null;
  gaveBackUsdc: number | null;
  exitQualityScore: number | null;
  openToTriggerMs: number | null;
  triggerToBuyFillMs: number | null;
  triggerToSubmitMs: number | null;
  submitToFillMs: number | null;
  holdMs: number | null;
  snapshotAgeMs: number | null;
  runtimePriceFetchMs: number | null;
  guardEvalMs: number | null;
  placeHttpMs: number | null;
  primaryDiagnosisCode: AutoScopeTradeDiagnosisCode;
  secondaryDiagnosisCode: AutoScopeTradeDiagnosisCode | null;
  diagnosisLabel: string;
  diagnosisDetail: string;
  dataQualityFlags: string[];
  compactMetrics: Record<string, unknown>;
  signalQuality?: AutoScopeTradeSignalQuality | null;
  riskFlags?: AutoScopeTradeRiskFlags;
  scenarioPnl?: AutoScopeTradeScenarioPnl;
  executionTelemetry?: AutoScopeTradeExecutionTelemetry | null;
  positionSnapshot?: AutoScopeTradePositionSnapshot;
  tpStatus?: AutoScopeTradeTpStatus;
  forensic?: AutoScopeTradeForensicSummary;
  updatedAt: string;
}

export interface AutoScopeTradeSignalQuality {
  mode: string | null;
  decisionReason: string | null;
  passed: boolean | null;
  selectedSide: string | null;
  candidateSide: string | null;
  q: number | null;
  qUp: number | null;
  qDown: number | null;
  cost: number | null;
  threshold: number | null;
  dynamicThreshold: number | null;
  requiredQ: number | null;
  qMargin: number | null;
  edge: number | null;
  edgeAdjusted: number | null;
  secondsLeft: number | null;
}

export interface AutoScopeTradeRiskFlags {
  highPrice: boolean;
  stale: boolean;
  fallingKnife: boolean;
  chop: boolean;
  reasons: string[];
}

export interface AutoScopeTradeScenarioPnl {
  ifUpUsdc: number | null;
  ifDownUsdc: number | null;
  evUsdc: number | null;
  worstUsdc: number | null;
  realizedPnlUsdc: number;
  markPnlUsdc: number;
  openCostUsdc: number;
  qUp: number | null;
  qDown: number | null;
}

export interface AutoScopeTradeExecutionTelemetry {
  submittedBestAsk: number | null;
  submittedEstimatedAvgFill: number | null;
  submittedVwapSlippage: number | null;
  submittedTargetQty: number | null;
  submittedEstimatedNotional: number | null;
  submittedQFinal: number | null;
  submittedModelBookGap: number | null;
  submittedModelBookZone: string | null;
  submittedParticipationCredit: number | null;
  fillActualPrice: number | null;
  fillActualQty: number | null;
  fillActualNotional: number | null;
  fillSlippageVsVwap: number | null;
  fillSlippageVsBestAsk: number | null;
  fillSource: string | null;
}

export interface AutoScopeTradeForensicEvent {
  eventId: string;
  eventType: string;
  eventTs: string;
  createdAt: string;
  decisionId: string | null;
  slEventId: string | null;
  fillEventId: string | null;
  orderId: string | null;
  payload: Record<string, unknown>;
}

export interface AutoScopeTradeForensicSummary {
  entryDecision: Record<string, unknown> | null;
  entryStopLossPlan: Record<string, unknown> | null;
  nodeSnapshot: Record<string, unknown> | null;
  entryNodeKey: string | null;
  entryNodeType: string | null;
  entryNodeConfigHash: string | null;
  orderLifecycle: AutoScopeTradeForensicEvent[];
  stopLossTrigger: Record<string, unknown> | null;
  postSlRecovery: Record<string, unknown> | null;
  slClassification: Record<string, unknown> | null;
  rawEvents: AutoScopeTradeForensicEvent[];
}

export interface AutoScopeTradePositionLegSnapshot {
  upQty: number;
  downQty: number;
  costUsdc: number;
  floorQty: number;
  floorPnlUsdc: number;
}

export interface AutoScopeTradePositionSnapshot {
  before: AutoScopeTradePositionLegSnapshot;
  after: AutoScopeTradePositionLegSnapshot;
  basis: 'current_analysis_rows';
}

export type AutoScopeTradeTpLifecycleStatus =
  | 'disabled'
  | 'open'
  | 'partial'
  | 'filled'
  | 'canceled'
  | 'expired'
  | 'unknown';

export interface AutoScopeTradeTpStatus {
  configured: boolean;
  status: AutoScopeTradeTpLifecycleStatus;
  orderCount: number;
  openOrderCount: number;
  filledQty: number;
  targetQty: number | null;
  remainingQty: number | null;
  averagePrice: number | null;
  realizedPnlUsdc: number;
}

export interface AutoScopeTradeBlockedSignal {
  eventType: string;
  createdAt: string;
  nodeKey: string | null;
  marketSlug: string | null;
  outcomeLabel: string | null;
  reasonCode: string | null;
  reasonDetail: string | null;
  signalQuality: AutoScopeTradeSignalQuality | null;
  riskFlags: AutoScopeTradeRiskFlags;
  noOrderTelemetry: AutoScopeNoOrderTelemetry | null;
}

export interface AutoScopeNoOrderTelemetry {
  orderCreated: boolean | null;
  orderSubmitted: boolean | null;
  orderFilled: boolean | null;
  finalActionStatus: string | null;
  lastGuardName: string | null;
  lastGuardCode: string | null;
  lastGuardState: string | null;
  executionFloor: number | null;
  bestAskAtWindowEnd: number | null;
  floorDistance: number | null;
  floorWaitMs: number | null;
  liquidityRegime: string | null;
  hourlyVolumeRatio: number | null;
  volume30s: number | null;
  tradeCount60s: number | null;
  quoteSnapshotSource: string | null;
  bookDataStatus: string | null;
  quoteMissingReason: string | null;
  selectedBid: number | null;
  selectedAsk: number | null;
  selectedMid: number | null;
  upBid: number | null;
  upAsk: number | null;
  downBid: number | null;
  downAsk: number | null;
  bookSide: string | null;
  upMid: number | null;
  downMid: number | null;
  bookMidDiff: number | null;
  whyNoOrderSummary: string | null;
  humanReadableReason: string | null;
}

export interface AutoScopeTradeAnalysisDiagnosisBreakdown {
  code: AutoScopeTradeDiagnosisCode;
  label: string;
  count: number;
  pnlUsdc: number;
}

export interface AutoScopeTradeAnalysisRow {
  rowId: string;
  rowType: AutoScopeTradeAnalysisRowType;
  positionState: AutoScopeTradeAnalysisPositionState;
  definitionId: number;
  definitionName: string | null;
  runId: number;
  rootOrderId: number;
  exitOrderId: number | null;
  marketSlug: string;
  tokenId: string;
  outcomeLabel: string;
  exitReason: AutoScopeTradeAnalysisExitReason;
  marketEndAt: string | null;
  marketOpenAt: string | null;
  triggeredAt: string | null;
  buyFilledAt: string | null;
  sellFilledAt: string | null;
  markPriceCapturedAt: string | null;
  openToTriggerMs: number | null;
  triggerToBuyFillMs: number | null;
  buyAvgPrice: number | null;
  sellOrLivePrice: number | null;
  rowQty: number;
  remainingQtyAfterExit: number;
  rowPnlUsdc: number;
  buyNotionalUsdc: number | null;
  buyFeeUsdc: number | null;
  costBasisUsdc: number | null;
  sellNotionalUsdc: number | null;
  sellFeeUsdc: number | null;
  markValueUsdc: number | null;
  netValueUsdc: number | null;
  pnlPct: number | null;
  cashFillPnlUsdc: number | null;
  diagnosticPnlUsdc: number | null;
  economicPnlUsdc: number | null;
  cashBuyUsdc: number | null;
  cashSellUsdc: number | null;
  cashRedeemUsdc: number | null;
  officialRootPnlUsdc: number | null;
  officialPnlSource: string | null;
  officialBuyUsdc: number | null;
  officialSellUsdc: number | null;
  officialRedeemUsdc: number | null;
  officialDeltaUsdc: number | null;
  polymarketPositionPnlUsdc?: number | null;
  polymarketPositionSource?: string | null;
  polymarketTotalBetUsdc?: number | null;
  polymarketAmountReturnedUsdc?: number | null;
  polymarketRealizedPnlUsdc?: number | null;
  polymarketCashPnlUsdc?: number | null;
  pendingInventoryQty: number | null;
  pendingInventoryValueUsdc: number | null;
  pendingRedeemableValueUsdc: number | null;
  cashStatus: string | null;
  valuationKind: AutoScopeTradeAnalysisValuationKind | null;
  primaryDiagnosisCode: AutoScopeTradeDiagnosisCode | null;
  diagnosisLabel: string | null;
  entryQualityScore: number | null;
  exitQualityScore: number | null;
  signalQuality?: AutoScopeTradeSignalQuality | null;
  riskFlags?: AutoScopeTradeRiskFlags;
  scenarioPnl?: AutoScopeTradeScenarioPnl;
  executionTelemetry?: AutoScopeTradeExecutionTelemetry | null;
  positionSnapshot?: AutoScopeTradePositionSnapshot;
  tpStatus?: AutoScopeTradeTpStatus;
  forensic?: AutoScopeTradeForensicSummary;
}

export interface AutoScopeTradeAnalysisSummary {
  rowCount: number;
  marketCount: number;
  lossCount: number;
  profitCount: number;
  totalPnlUsdc: number;
  realizedPnlUsdc: number;
  openPnlUsdc: number;
  lossUsdc: number;
  profitUsdc: number;
  buyFeeUsdc: number;
  sellFeeUsdc: number;
  totalFeeUsdc: number;
  costBasisUsdc: number;
  netValueUsdc: number;
  profitFactor: number | null;
  winRatePct: number | null;
  avgWinUsdc: number | null;
  avgLossUsdc: number | null;
  largestLossUsdc: number | null;
  feeDragUsdc: number;
  diagnosisBreakdown: AutoScopeTradeAnalysisDiagnosisBreakdown[];
  pnlSource?:
    | 'analysis_rows'
    | 'official_market_activity'
    | 'polymarket_leaderboard'
    | 'polymarket_user_pnl_history'
    | 'official_activity_window';
  officialBuyUsdc?: number;
  officialSellUsdc?: number;
  officialRedeemUsdc?: number;
  rootRowsPnlUsdc?: number;
  officialDeltaUsdc?: number;
  localCashFillPnlUsdc?: number;
  diagnosticPnlUsdc?: number;
  economicPnlUsdc?: number;
  pendingInventoryValueUsdc?: number;
  pendingRedeemableValueUsdc?: number;
}

export interface AutoScopeTradeAnalysisResponse
  extends PaginatedResponse<AutoScopeTradeAnalysisRow> {
  refreshedAt: string;
  sortBy: AutoScopeTradeAnalysisSortBy;
  sortDirection: AutoScopeTradeAnalysisSortDirection;
  pnlFilter: AutoScopeTradeAnalysisPnlFilter;
  positionFilter: AutoScopeTradeAnalysisPositionFilter;
  from: string | null;
  to: string | null;
  summary: AutoScopeTradeAnalysisSummary;
}

export interface AutoScopeTradeDiagnosticResponse {
  diagnostic: AutoScopeTradeDiagnostic | null;
  rows: AutoScopeTradeAnalysisRow[];
  blockedSignals: AutoScopeTradeBlockedSignal[];
  refreshedAt: string;
}

export interface TradeFlowPtbStateRow {
  builderOrderId: number;
  runId: number | null;
  nodeKey: string | null;
  marketSlug: string;
  outcomeLabel: string;
  baseThresholdUsd: number | null;
  bumpUsd: number | null;
  bumpIncrementUsd: number | null;
  relaxCreditUsd: number | null;
  effectiveThresholdUsd: number | null;
  guardMissReason: string | null;
  maxPriceMiss: boolean;
  firstTradableSecond: string | null;
  firstTradableGapUsd: number | null;
  tradableSecondsCount: number;
  priceOkDepthFailCount: number;
  maxFillabilityScore: number | null;
  qualityScore: number | null;
  refreshedAt: string;
}

export interface TradeFlowPtbStateResponse extends PaginatedResponse<TradeFlowPtbStateRow> {
  refreshedAt: string;
}

export interface TradeFlowNodeRuntimeRow {
  runId: number;
  definitionId: number;
  versionId: number | null;
  nodeKey: string;
  nodeType: string;
  status: string;
  stateKind: string;
  marketSlug: string | null;
  tokenId: string | null;
  snapshotJson: Record<string, unknown>;
  updatedAt: string;
}

export interface TradeFlowNodeRuntimeResponse
  extends PaginatedResponse<TradeFlowNodeRuntimeRow> {
  refreshedAt: string;
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
