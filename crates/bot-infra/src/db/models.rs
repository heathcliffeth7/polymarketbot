use super::*;

pub struct LegPositionSnapshot {
    pub leg_side: LegSide,
    pub token_id: String,
    pub qty: f64,
    pub avg_entry: f64,
    pub levels_filled: i32,
    pub last_fill_price: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct PositionExitRule {
    pub leg_side: LegSide,
    pub drop_sell_pct: f64,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct PressureSnapshot {
    pub trade_id: i64,
    pub pressure_score: f64,
    pub bid_ask_imbalance: Option<f64>,
    pub sell_ratio: Option<f64>,
    pub yes_price: Option<f64>,
    pub no_price: Option<f64>,
    pub trigger_reason: Option<String>,
    pub triggered: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeBuilderOrder {
    pub id: i64,
    pub trade_id: i64,
    pub user_id: i64,
    pub kind: String,
    pub status: String,
    pub market_slug: String,
    pub token_id: String,
    pub outcome_label: String,
    pub side: String,
    pub execution_mode: String,
    pub trigger_condition: Option<String>,
    pub trigger_price: Option<f64>,
    pub max_price: Option<f64>,
    pub size_basis: String,
    pub size_usdc: f64,
    pub target_qty: Option<f64>,
    pub min_price_distance_cent: f64,
    pub expires_at: Option<DateTime<Utc>>,
    pub eligible_after_at: Option<DateTime<Utc>>,
    pub eligible_before_at: Option<DateTime<Utc>>,
    pub max_triggers: i32,
    pub triggers_fired: i32,
    pub active_exchange_order_id: Option<String>,
    pub remaining_size: Option<f64>,
    pub remaining_qty: Option<f64>,
    pub working_price: Option<f64>,
    pub last_seen_price: Option<f64>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub parent_order_id: Option<i64>,
    pub origin_flow_definition_id: Option<i64>,
    pub origin_flow_run_id: Option<i64>,
    pub origin_flow_node_key: Option<String>,
    pub tp_enabled: bool,
    pub tp_price: Option<f64>,
    pub sl_enabled: bool,
    pub sl_price: Option<f64>,
    pub filled_qty: f64,
    pub fee_rate_bps: i64,
    pub trigger_latched: bool,
    pub trigger_latched_reason: Option<String>,
    pub trigger_latched_at: Option<DateTime<Utc>>,
    pub submitted_dynamic_qty: Option<f64>,
    pub submitted_dynamic_price: Option<f64>,
    pub guard_trigger_price: Option<f64>,
    pub best_ask_floor_price: Option<f64>,
    pub retry_on_trigger_guard_block: bool,
    pub retry_on_execution_floor_guard_block: bool,
    pub retry_on_max_price_block: bool,
    pub sl_trigger_price_mode: Option<String>,
    pub notify_on_fill: bool,
    pub notify_on_trigger_guard_blocked: bool,
    pub notify_on_execution_floor_blocked: bool,
    pub notify_on_tp_hit: bool,
    pub notify_on_sl_hit: bool,
    pub notify_on_max_price_blocked: bool,
}

#[derive(Debug, Clone)]
pub struct RunningTradeFlowMarketPeer {
    pub run_id: i64,
    pub definition_id: i64,
    pub definition_name: String,
    pub source_trade_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ActiveTradeFlowRunOrderPeer {
    pub builder_order_id: i64,
    pub source_trade_id: i64,
    pub origin_flow_node_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TradeBuilderInventoryObservationInput {
    pub parent_builder_order_id: i64,
    pub observer_builder_order_id: Option<i64>,
    pub user_id: i64,
    pub market_slug: String,
    pub token_id: String,
    pub outcome_label: String,
    pub exchange_order_id: Option<String>,
    pub observation_kind: String,
    pub qty_source: Option<String>,
    pub baseline_visible_qty: Option<f64>,
    pub submitted_dynamic_qty: Option<f64>,
    pub resolved_fill_qty: Option<f64>,
    pub expected_fee_qty: Option<f64>,
    pub expected_net_qty: Option<f64>,
    pub expected_visible_qty: Option<f64>,
    pub actual_visible_qty: Option<f64>,
    pub visible_delta_qty: Option<f64>,
    pub gap_vs_submit_qty: Option<f64>,
    pub gap_vs_fill_qty: Option<f64>,
    pub gap_vs_expected_qty: Option<f64>,
    pub reference_price: Option<f64>,
    pub fee_rate_bps: Option<i64>,
    pub fill_to_inventory_ms: Option<i64>,
    pub payload_json: Value,
}

#[derive(Debug, Clone)]
pub struct PendingTradeBuilderFirstVisibleInventoryObservation {
    pub parent_builder_order_id: i64,
    pub observer_builder_order_id: Option<i64>,
    pub user_id: i64,
    pub market_slug: String,
    pub token_id: String,
    pub outcome_label: String,
    pub exchange_order_id: Option<String>,
    pub baseline_visible_qty: Option<f64>,
    pub submitted_dynamic_qty: Option<f64>,
    pub resolved_fill_qty: Option<f64>,
    pub submit_reference_price: Option<f64>,
    pub fill_reference_price: Option<f64>,
    pub fill_qty_source: Option<String>,
    pub fee_rate_bps: i64,
    pub fill_observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeBuilderWorkflow {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub status: String,
    pub source_trade_id: i64,
    pub sell_target_pct: f64,
    pub buy_start_after_sell_progress_pct: f64,
    pub buy_trigger_mode: String,
    pub buy_allocation_pct: f64,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeBuilderWorkflowLeg {
    pub id: i64,
    pub workflow_id: i64,
    pub leg_type: String,
    pub market_slug: String,
    pub token_id: String,
    pub outcome_label: String,
    pub side: String,
    pub trigger_condition: Option<String>,
    pub trigger_price: Option<f64>,
    pub min_price_distance_cent: f64,
    pub status: String,
    pub builder_order_id: Option<i64>,
    pub target_notional_usdc: f64,
    pub allocated_notional_usdc: f64,
    pub filled_notional_usdc: f64,
    pub filled_qty: f64,
    pub last_seen_price: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowDefinitionRuntime {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub status: String,
    pub draft_version_id: Option<i64>,
    pub published_version_id: Option<i64>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowVersionRuntime {
    pub id: i64,
    pub definition_id: i64,
    pub version_no: i32,
    pub status: String,
    pub graph_json: Value,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowRun {
    pub id: i64,
    pub definition_id: i64,
    pub version_id: i64,
    pub user_id: i64,
    pub status: String,
    pub trigger_source: Option<String>,
    pub context_json: Value,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowRunStep {
    pub id: i64,
    pub run_id: i64,
    pub node_key: String,
    pub node_type: String,
    pub status: String,
    pub attempt: i32,
    pub input_json: Option<Value>,
    pub output_json: Option<Value>,
    pub error_text: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub available_at: DateTime<Utc>,
    pub parent_step_id: Option<i64>,
    pub idempotency_key: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowDualDcaJob {
    pub id: i64,
    pub flow_run_id: i64,
    pub flow_definition_id: i64,
    pub flow_version_id: Option<i64>,
    pub node_key: String,
    pub status: String,
    pub source_trade_id: Option<i64>,
    pub market_asset: String,
    pub market_timeframe: String,
    pub side_mode: String,
    pub base_sizing: String,
    pub base_shares: Option<f64>,
    pub base_usdc: Option<f64>,
    pub base_price_usdc: Option<f64>,
    pub dca_levels: i32,
    pub near_step: f64,
    pub step_mult: f64,
    pub size_mult: f64,
    pub min_price_distance_cent: f64,
    pub cutoff_min: i32,
    pub tp_profit_pct: f64,
    pub sl_loss_pct: f64,
    pub sl_spread_pct: f64,
    pub last_market_slug: Option<String>,
    pub last_market_started_at: Option<DateTime<Utc>>,
    pub last_market_ends_at: Option<DateTime<Utc>>,
    pub next_check_at: DateTime<Utc>,
    pub created_order_count: i32,
    pub consecutive_errors: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowDualDcaLeg {
    pub id: i64,
    pub job_id: i64,
    pub market_slug: String,
    pub token_id: String,
    pub outcome_label: String,
    pub side: String,
    pub level_index: i32,
    pub trigger_condition: Option<String>,
    pub trigger_price: Option<f64>,
    pub size_usdc: f64,
    pub reference_price: Option<f64>,
    pub builder_order_id: Option<i64>,
    pub status: String,
    pub active_exchange_order_id: Option<String>,
    pub client_order_id: Option<String>,
    pub filled_price: Option<f64>,
    pub filled_size: Option<f64>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub filled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFillTokenAggregate {
    pub token_id: String,
    pub buy_qty: f64,
    pub buy_notional_usdc: f64,
    pub sell_qty: f64,
    pub sell_notional_usdc: f64,
}

#[derive(Debug, Clone)]
pub struct AutoClaimJob {
    pub id: i64,
    pub owner_address: String,
    pub market_slug: Option<String>,
    pub condition_id: String,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub next_attempt_at: DateTime<Utc>,
    pub tx_hash: Option<String>,
    pub last_error: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub last_seen_redeemable_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
