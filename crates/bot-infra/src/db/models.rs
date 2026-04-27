use super::*;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeBuilderPriceExitRule {
    pub price: f64,
    pub size_pct: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeBuilderTimeExitRule {
    pub elapsed_minutes: i32,
    pub remaining_pct: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeBuilderPtbStopLossRule {
    pub gap_usd: f64,
    pub size_pct: f64,
}

#[derive(Debug, Clone)]
pub struct TradeBuilderMarketSecondSnapshotInput {
    pub market_slug: String,
    pub asset: String,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub second_ts: DateTime<Utc>,
    pub ptb_ref_price: Option<f64>,
    pub chainlink_price: Option<f64>,
    pub yes_best_bid: Option<f64>,
    pub yes_best_ask: Option<f64>,
    pub yes_ask_depth_usdc: Option<f64>,
    pub no_best_bid: Option<f64>,
    pub no_best_ask: Option<f64>,
    pub no_ask_depth_usdc: Option<f64>,
    pub sample_count: i32,
}

#[derive(Debug, Clone)]
pub struct TradeBuilderMarketSecondSnapshot {
    pub market_slug: String,
    pub asset: String,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub second_ts: DateTime<Utc>,
    pub ptb_ref_price: Option<f64>,
    pub chainlink_price: Option<f64>,
    pub yes_best_bid: Option<f64>,
    pub yes_best_ask: Option<f64>,
    pub yes_ask_depth_usdc: Option<f64>,
    pub no_best_bid: Option<f64>,
    pub no_best_ask: Option<f64>,
    pub no_ask_depth_usdc: Option<f64>,
    pub sample_count: i32,
}

#[derive(Debug, Clone)]
pub struct TradeBuilderMarketTradeTickInput {
    pub market_slug: String,
    pub asset: String,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub token_id: String,
    pub outcome_side: String,
    pub event_ts: DateTime<Utc>,
    pub price: f64,
    pub size: f64,
    pub notional_usdc: f64,
    pub side: Option<String>,
    pub dedupe_key: String,
}

#[derive(Debug, Clone)]
pub struct MarketTradeHourlyVolumeMedian {
    pub hour_utc: i32,
    pub median_volume_usdc: f64,
    pub sample_count: i64,
}

#[derive(Debug, Clone)]
pub struct MarketTradeVolumeMedian {
    pub median_volume_usdc: f64,
    pub sample_count: i64,
}

#[derive(Debug, Clone)]
pub struct MarketTradeVolumeSummary {
    pub volume_10s: f64,
    pub volume_30s: f64,
    pub volume_60s: f64,
    pub trade_count_60s: i64,
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
    pub pair_session_id: Option<i64>,
    pub pair_leg_role: Option<String>,
    pub tp_enabled: bool,
    pub tp_price: Option<f64>,
    pub tp_rules_json: Vec<TradeBuilderPriceExitRule>,
    pub sl_enabled: bool,
    pub sl_price: Option<f64>,
    pub sl_rules_json: Vec<TradeBuilderPriceExitRule>,
    pub time_exit_rules_json: Vec<TradeBuilderTimeExitRule>,
    pub filled_qty: f64,
    pub fee_rate_bps: i64,
    pub trigger_latched: bool,
    pub trigger_latched_reason: Option<String>,
    pub trigger_latched_at: Option<DateTime<Utc>>,
    pub submitted_dynamic_qty: Option<f64>,
    pub submitted_dynamic_price: Option<f64>,
    pub runtime_snapshot_json: Option<Value>,
    pub fresh_submit_lease_until: Option<DateTime<Utc>>,
    pub guard_trigger_price: Option<f64>,
    pub best_ask_floor_price: Option<f64>,
    pub retry_on_trigger_guard_block: bool,
    pub retry_on_execution_floor_guard_block: bool,
    pub retry_on_max_price_block: bool,
    pub ptb_stop_loss_gap_usd: Option<f64>,
    pub ptb_reference_price: Option<f64>,
    pub ptb_stop_loss_rules_json: Vec<TradeBuilderPtbStopLossRule>,
    pub ptb_stop_loss_time_decay_mode: Option<String>,
    pub staged_sl_retry_only_dust: bool,
    pub staged_sl_retry_dust_metric: Option<String>,
    pub staged_sl_retry_dust_value: Option<f64>,
    pub staged_sl_reentry_use_sold_notional: bool,
    pub staged_sl_reentry_only_after_all_stages: bool,
    pub sl_trigger_price_mode: Option<String>,
    pub reenter_on_sl_hit: bool,
    pub reentry_max_attempts: i32,
    pub reentry_trigger_node_key: Option<String>,
    pub notify_on_order_submitted: bool,
    pub notify_on_fill: bool,
    pub notify_on_order_not_filled: bool,
    pub notify_on_trigger_guard_blocked: bool,
    pub notify_on_execution_floor_blocked: bool,
    pub notify_on_tp_hit: bool,
    pub notify_on_sl_hit: bool,
    pub notify_on_max_price_blocked: bool,
    pub last_guard_notification_reason: Option<String>,
    pub exit_ladder_kind: Option<String>,
    pub exit_ladder_index: Option<i32>,
    pub exit_ladder_size_pct: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct TradeBuilderPairSession {
    pub id: i64,
    pub user_id: i64,
    pub flow_definition_id: Option<i64>,
    pub flow_run_id: Option<i64>,
    pub flow_node_key: Option<String>,
    pub market_slug: String,
    pub status: String,
    pub pair_target_total_cent: f64,
    pub min_net_profit_usdc: f64,
    pub profit_safety_buffer_usdc: f64,
    pub orphan_grace_ms: i64,
    pub ignore_stop_loss_after_locked: bool,
    pub notify_on_pair_locked: bool,
    pub notify_on_pair_unwind: bool,
    pub notify_on_pair_no_edge: bool,
    pub primary_order_id: Option<i64>,
    pub counter_order_id: Option<i64>,
    pub lead_order_id: Option<i64>,
    pub primary_fill_qty: Option<f64>,
    pub primary_fill_fee_qty: Option<f64>,
    pub primary_net_qty: Option<f64>,
    pub primary_avg_fill_price: Option<f64>,
    pub counter_fill_qty: Option<f64>,
    pub counter_fill_fee_qty: Option<f64>,
    pub counter_net_qty: Option<f64>,
    pub counter_avg_fill_price: Option<f64>,
    pub lead_filled_at: Option<DateTime<Utc>>,
    pub locked_qty: Option<f64>,
    pub projected_net_profit_usdc: Option<f64>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
pub struct TradeBuilderParentPosition {
    pub parent_builder_order_id: i64,
    pub user_id: i64,
    pub source_trade_id: i64,
    pub market_slug: String,
    pub token_id: String,
    pub outcome_label: String,
    pub baseline_qty: f64,
    pub current_qty: f64,
    pub last_fill_qty: Option<f64>,
    pub last_fill_price: Option<f64>,
    pub qty_source: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeBuilderParentPositionInput {
    pub parent_builder_order_id: i64,
    pub user_id: i64,
    pub source_trade_id: i64,
    pub market_slug: String,
    pub token_id: String,
    pub outcome_label: String,
    pub baseline_qty: f64,
    pub current_qty: f64,
    pub last_fill_qty: Option<f64>,
    pub last_fill_price: Option<f64>,
    pub qty_source: String,
}

#[derive(Debug, Clone)]
pub struct TradeBuilderParentPositionSeed {
    pub actual_visible_qty: Option<f64>,
    pub expected_visible_qty: Option<f64>,
    pub reference_price: Option<f64>,
    pub qty_source: Option<String>,
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
pub struct TradeBuilderOrderEventRecord {
    pub builder_order_id: i64,
    pub event_type: String,
    pub payload_json: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowEventRecord {
    pub run_id: i64,
    pub event_type: String,
    pub payload_json: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeBuilderExchangeFillSummary {
    pub exchange_order_id: String,
    pub filled_qty: f64,
    pub filled_notional_usdc: f64,
    pub fee_usdc: f64,
    pub first_filled_at: Option<DateTime<Utc>>,
    pub last_filled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowAutoScopeAnalysisRowInput {
    pub row_key: String,
    pub user_id: i64,
    pub definition_id: i64,
    pub run_id: i64,
    pub root_builder_order_id: i64,
    pub exit_builder_order_id: Option<i64>,
    pub row_type: String,
    pub market_slug: String,
    pub token_id: String,
    pub outcome_label: String,
    pub exit_reason: String,
    pub market_open_at: Option<DateTime<Utc>>,
    pub triggered_at: Option<DateTime<Utc>>,
    pub buy_filled_at: Option<DateTime<Utc>>,
    pub sell_filled_at: Option<DateTime<Utc>>,
    pub open_to_trigger_ms: Option<i64>,
    pub trigger_to_buy_fill_ms: Option<i64>,
    pub buy_avg_price: Option<f64>,
    pub mark_or_sell_price: Option<f64>,
    pub mark_price_captured_at: Option<DateTime<Utc>>,
    pub row_qty: f64,
    pub remaining_qty_after_exit: f64,
    pub row_pnl_usdc: f64,
    pub buy_notional_usdc: Option<f64>,
    pub buy_fee_usdc: Option<f64>,
    pub cost_basis_usdc: Option<f64>,
    pub sell_notional_usdc: Option<f64>,
    pub sell_fee_usdc: Option<f64>,
    pub mark_value_usdc: Option<f64>,
    pub net_value_usdc: Option<f64>,
    pub pnl_pct: Option<f64>,
    pub valuation_kind: String,
}

#[derive(Debug, Clone)]
pub struct TradeFlowAutoScopeTradeDiagnosticInput {
    pub root_builder_order_id: i64,
    pub user_id: i64,
    pub definition_id: i64,
    pub run_id: i64,
    pub market_slug: String,
    pub token_id: String,
    pub outcome_label: String,
    pub total_pnl_usdc: f64,
    pub realized_pnl_usdc: f64,
    pub open_pnl_usdc: f64,
    pub pnl_pct: Option<f64>,
    pub fee_drag_usdc: f64,
    pub cost_basis_usdc: f64,
    pub net_value_usdc: f64,
    pub entry_trigger_price: Option<f64>,
    pub entry_submit_price: Option<f64>,
    pub entry_fill_price: Option<f64>,
    pub entry_reference_price: Option<f64>,
    pub entry_slippage_usdc: Option<f64>,
    pub entry_quality_score: Option<f64>,
    pub exit_reason: Option<String>,
    pub exit_price: Option<f64>,
    pub best_price_during_hold: Option<f64>,
    pub worst_price_during_hold: Option<f64>,
    pub max_favorable_usdc: Option<f64>,
    pub max_adverse_usdc: Option<f64>,
    pub gave_back_usdc: Option<f64>,
    pub exit_quality_score: Option<f64>,
    pub open_to_trigger_ms: Option<i64>,
    pub trigger_to_buy_fill_ms: Option<i64>,
    pub trigger_to_submit_ms: Option<i64>,
    pub submit_to_fill_ms: Option<i64>,
    pub hold_ms: Option<i64>,
    pub snapshot_age_ms: Option<i64>,
    pub runtime_price_fetch_ms: Option<i64>,
    pub guard_eval_ms: Option<i64>,
    pub place_http_ms: Option<i64>,
    pub primary_diagnosis_code: String,
    pub secondary_diagnosis_code: Option<String>,
    pub diagnosis_label: String,
    pub diagnosis_detail: String,
    pub data_quality_flags: Vec<String>,
    pub compact_metrics_json: Value,
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
pub struct TradeFlowNodeRuntimeSnapshotInput {
    pub run_id: i64,
    pub definition_id: i64,
    pub version_id: Option<i64>,
    pub node_key: String,
    pub node_type: String,
    pub status: String,
    pub state_kind: String,
    pub market_slug: Option<String>,
    pub token_id: Option<String>,
    pub snapshot_json: Value,
}

#[derive(Debug, Clone)]
pub struct TradeFlowNodeRuntimeSnapshotRecord {
    pub run_id: i64,
    pub definition_id: i64,
    pub version_id: Option<i64>,
    pub node_key: String,
    pub node_type: String,
    pub status: String,
    pub state_kind: String,
    pub market_slug: Option<String>,
    pub token_id: Option<String>,
    pub snapshot_json: Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowAutoTuneMarketSummaryInput {
    pub definition_id: i64,
    pub version_id: i64,
    pub flow_run_id: Option<i64>,
    pub node_key: String,
    pub market_scope: String,
    pub market_slug: String,
    pub window_start: Option<DateTime<Utc>>,
    pub window_end: Option<DateTime<Utc>>,
    pub completed_at: DateTime<Utc>,
    pub trigger_passed: bool,
    pub action_started: bool,
    pub builder_order_created: bool,
    pub order_submitted: bool,
    pub order_filled: bool,
    pub first_terminal_guard_scope: Option<String>,
    pub first_terminal_guard_code: Option<String>,
    pub first_terminal_guard_node: Option<String>,
    pub first_terminal_guard_at: Option<DateTime<Utc>>,
    pub last_guard_scope: Option<String>,
    pub last_guard_code: Option<String>,
    pub max_price_block: bool,
    pub execution_floor_block: bool,
    pub ptb_block: bool,
    pub pair_total_block: bool,
    pub counter_max_block: bool,
    pub counter_floor_block: bool,
    pub risk_block: bool,
    pub data_problem_block: bool,
    pub best_ask_at_block: Option<f64>,
    pub max_price_effective: Option<f64>,
    pub execution_floor_effective: Option<f64>,
    pub pair_total_effective: Option<f64>,
    pub counter_price_effective: Option<f64>,
    pub iv_edge_margin: Option<f64>,
    pub iv_dynamic_threshold: Option<f64>,
    pub gap_strength: Option<f64>,
    pub required_gap_strength: Option<f64>,
    pub binance_stale_ms: Option<i64>,
    pub binance_same_direction: Option<bool>,
    pub depth_ok: Option<bool>,
    pub floor_recovered_once: bool,
    pub max_best_ask_after_block: Option<f64>,
    pub tradable_seconds_count: Option<i64>,
    pub depth_ok_seconds_count: Option<i64>,
    pub pair_session_id: Option<i64>,
    pub pair_locked: bool,
    pub locked_qty: Option<f64>,
    pub unpaired_qty: Option<f64>,
    pub locked_profit_per_share: Option<f64>,
    pub orphan_detected: bool,
    pub protective_unwind_triggered: bool,
    pub sl_hit: bool,
    pub tp_hit: bool,
    pub realized_pnl_usdc: Option<f64>,
    pub metrics_json: Value,
}

#[derive(Debug, Clone)]
pub struct TradeFlowAutoTuneMarketSummaryRecord {
    pub id: i64,
    pub definition_id: i64,
    pub version_id: i64,
    pub flow_run_id: Option<i64>,
    pub node_key: String,
    pub market_scope: String,
    pub market_slug: String,
    pub window_start: Option<DateTime<Utc>>,
    pub window_end: Option<DateTime<Utc>>,
    pub completed_at: DateTime<Utc>,
    pub trigger_passed: bool,
    pub action_started: bool,
    pub builder_order_created: bool,
    pub order_submitted: bool,
    pub order_filled: bool,
    pub first_terminal_guard_scope: Option<String>,
    pub first_terminal_guard_code: Option<String>,
    pub first_terminal_guard_node: Option<String>,
    pub first_terminal_guard_at: Option<DateTime<Utc>>,
    pub last_guard_scope: Option<String>,
    pub last_guard_code: Option<String>,
    pub max_price_block: bool,
    pub execution_floor_block: bool,
    pub ptb_block: bool,
    pub pair_total_block: bool,
    pub counter_max_block: bool,
    pub counter_floor_block: bool,
    pub risk_block: bool,
    pub data_problem_block: bool,
    pub best_ask_at_block: Option<f64>,
    pub max_price_effective: Option<f64>,
    pub execution_floor_effective: Option<f64>,
    pub pair_total_effective: Option<f64>,
    pub counter_price_effective: Option<f64>,
    pub iv_edge_margin: Option<f64>,
    pub binance_stale_ms: Option<i64>,
    pub binance_same_direction: Option<bool>,
    pub depth_ok: Option<bool>,
    pub floor_recovered_once: bool,
    pub max_best_ask_after_block: Option<f64>,
    pub tradable_seconds_count: Option<i64>,
    pub pair_session_id: Option<i64>,
    pub pair_locked: bool,
    pub locked_qty: Option<f64>,
    pub unpaired_qty: Option<f64>,
    pub locked_profit_per_share: Option<f64>,
    pub orphan_detected: bool,
    pub protective_unwind_triggered: bool,
    pub sl_hit: bool,
    pub tp_hit: bool,
    pub realized_pnl_usdc: Option<f64>,
    pub metrics_json: Value,
}

#[derive(Debug, Clone)]
pub struct TradeFlowAutoTuneAdviceInput {
    pub definition_id: i64,
    pub version_id: i64,
    pub node_key: String,
    pub market_scope: String,
    pub sample_start_market_slug: Option<String>,
    pub sample_end_market_slug: Option<String>,
    pub markets_seen: i64,
    pub eligible_markets: i64,
    pub order_created_count: i64,
    pub filled_count: i64,
    pub pair_locked_count: i64,
    pub orphan_count: i64,
    pub sl_count: i64,
    pub advice_kind: String,
    pub advice_action: String,
    pub target_key_path: Option<String>,
    pub current_value_json: Option<Value>,
    pub suggested_value_json: Option<Value>,
    pub clamped: bool,
    pub hard_cap_min_json: Option<Value>,
    pub hard_cap_max_json: Option<Value>,
    pub reason_code: String,
    pub reason_text: String,
    pub dominant_blocker: Option<String>,
    pub metrics_json: Value,
    pub dedupe_key: String,
}

#[derive(Debug, Clone, Default)]
pub struct TradeFlowAutoTuneOrderRollup {
    pub builder_order_created: bool,
    pub order_submitted: bool,
    pub order_filled: bool,
    pub pair_session_id: Option<i64>,
    pub pair_locked: bool,
    pub locked_qty: Option<f64>,
    pub unpaired_qty: Option<f64>,
    pub locked_profit_per_share: Option<f64>,
    pub orphan_detected: bool,
    pub protective_unwind_triggered: bool,
    pub sl_hit: bool,
    pub tp_hit: bool,
    pub realized_pnl_usdc: Option<f64>,
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
