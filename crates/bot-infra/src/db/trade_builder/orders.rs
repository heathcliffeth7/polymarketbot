use super::super::*;
use serde_json::json;

const TRADE_BUILDER_ORDER_SELECT_COLUMNS: &str =
    "id, trade_id, user_id, kind, status, market_slug, token_id, outcome_label, side, \
     execution_mode, trigger_condition, trigger_price, max_price, guard_trigger_price, \
     best_ask_floor_price, size_basis, size_usdc, target_qty, min_price_distance_cent, \
     expires_at, eligible_after_at, eligible_before_at, max_triggers, triggers_fired, \
     active_exchange_order_id, remaining_size, remaining_qty, working_price, last_seen_price, \
     last_error, created_at, updated_at, parent_order_id, origin_flow_definition_id, \
     origin_flow_run_id, origin_flow_node_key, pair_session_id, pair_leg_role, tp_enabled, tp_price, tp_rules_json, sl_enabled, \
     sl_price, sl_rules_json, time_exit_rules_json, filled_qty, fee_rate_bps, trigger_latched, \
     trigger_latched_reason, trigger_latched_at, submitted_dynamic_qty, submitted_dynamic_price, \
     runtime_snapshot_json, fresh_submit_lease_until, retry_on_trigger_guard_block, \
     retry_on_execution_floor_guard_block, retry_on_max_price_block, \
     ptb_stop_loss_gap_usd, ptb_reference_price, ptb_stop_loss_rules_json, ptb_stop_loss_time_decay_mode, \
     ptb_current_price_source, \
     staged_sl_retry_only_dust, staged_sl_retry_dust_metric, staged_sl_retry_dust_value, \
     staged_sl_reentry_use_sold_notional, staged_sl_reentry_only_after_all_stages, \
     sl_trigger_price_mode, reenter_on_sl_hit, reentry_max_attempts, reentry_trigger_node_key, \
     notify_on_order_submitted, notify_on_fill, notify_on_order_not_filled, \
     notify_on_trigger_guard_blocked, notify_on_execution_floor_blocked, notify_on_tp_hit, notify_on_sl_hit, \
     notify_on_max_price_blocked, last_guard_notification_reason, exit_ladder_kind, \
     exit_ladder_index, exit_ladder_size_pct";

const TRADE_BUILDER_ORDER_SELECT_COLUMNS_O_ALIAS: &str =
    "o.id, o.trade_id, o.user_id, o.kind, o.status, o.market_slug, o.token_id, o.outcome_label, \
     o.side, o.execution_mode, o.trigger_condition, o.trigger_price, o.max_price, \
     o.guard_trigger_price, o.best_ask_floor_price, o.size_basis, o.size_usdc, o.target_qty, \
     o.min_price_distance_cent, o.expires_at, o.eligible_after_at, o.eligible_before_at, \
     o.max_triggers, o.triggers_fired, o.active_exchange_order_id, o.remaining_size, \
     o.remaining_qty, o.working_price, o.last_seen_price, o.last_error, o.created_at, \
     o.updated_at, o.parent_order_id, o.origin_flow_definition_id, o.origin_flow_run_id, \
     o.origin_flow_node_key, o.pair_session_id, o.pair_leg_role, o.tp_enabled, o.tp_price, o.tp_rules_json, o.sl_enabled, \
     o.sl_price, o.sl_rules_json, o.time_exit_rules_json, o.filled_qty, o.fee_rate_bps, \
     o.trigger_latched, o.trigger_latched_reason, o.trigger_latched_at, o.submitted_dynamic_qty, \
     o.submitted_dynamic_price, o.runtime_snapshot_json, o.fresh_submit_lease_until, \
     o.retry_on_trigger_guard_block, \
     o.retry_on_execution_floor_guard_block, o.retry_on_max_price_block, \
     o.ptb_stop_loss_gap_usd, o.ptb_reference_price, o.ptb_stop_loss_rules_json, \
     o.ptb_stop_loss_time_decay_mode, o.ptb_current_price_source, \
     o.staged_sl_retry_only_dust, o.staged_sl_retry_dust_metric, \
     o.staged_sl_retry_dust_value, o.staged_sl_reentry_use_sold_notional, \
     o.staged_sl_reentry_only_after_all_stages, \
     o.sl_trigger_price_mode, \
     o.reenter_on_sl_hit, o.reentry_max_attempts, o.reentry_trigger_node_key, \
     o.notify_on_order_submitted, o.notify_on_fill, o.notify_on_order_not_filled, o.notify_on_trigger_guard_blocked, \
     o.notify_on_execution_floor_blocked, o.notify_on_tp_hit, o.notify_on_sl_hit, \
     o.notify_on_max_price_blocked, o.last_guard_notification_reason, o.exit_ladder_kind, \
     o.exit_ladder_index, o.exit_ladder_size_pct";

fn trade_builder_price_exit_rules_to_json(rules: Option<&[TradeBuilderPriceExitRule]>) -> Value {
    rules
        .map(serde_json::to_value)
        .transpose()
        .ok()
        .flatten()
        .unwrap_or_else(|| json!([]))
}

fn trade_builder_time_exit_rules_to_json(rules: Option<&[TradeBuilderTimeExitRule]>) -> Value {
    rules
        .map(serde_json::to_value)
        .transpose()
        .ok()
        .flatten()
        .unwrap_or_else(|| json!([]))
}

fn trade_builder_ptb_stop_loss_rules_to_json(
    rules: Option<&[TradeBuilderPtbStopLossRule]>,
) -> Value {
    rules
        .map(serde_json::to_value)
        .transpose()
        .ok()
        .flatten()
        .unwrap_or_else(|| json!([]))
}

fn trade_builder_parse_rules<T>(value: Value) -> Vec<T>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(value).unwrap_or_default()
}

fn trade_builder_order_uses_ws_fast_path(order: &TradeBuilderOrder) -> bool {
    order.parent_order_id.is_some()
        && order.side == "sell"
        && order.kind == "conditional"
        && matches!(order.status.as_str(), "armed" | "triggered")
        && order.trigger_condition.is_some()
        && order.trigger_price.is_some()
}

fn trade_builder_order_requires_housekeeping(order: &TradeBuilderOrder) -> bool {
    if order.status == "error" {
        return order.trigger_latched
            && order.trigger_latched_reason.as_deref() == Some("stop_loss");
    }

    if trade_builder_order_uses_ws_fast_path(order) {
        return false;
    }

    matches!(
        order.status.as_str(),
        "pending"
            | "armed"
            | "triggered"
            | "open"
            | "partially_filled"
            | "canceled_requested"
            | "inventory_pending"
            | "guard_blocked"
    )
}

fn map_trade_builder_order_row(row: sqlx::postgres::PgRow) -> TradeBuilderOrder {
    TradeBuilderOrder {
        id: row.get("id"),
        trade_id: row.get("trade_id"),
        user_id: row.get("user_id"),
        kind: row.get("kind"),
        status: row.get("status"),
        market_slug: row.get("market_slug"),
        token_id: row.get("token_id"),
        outcome_label: row.get("outcome_label"),
        side: row.get("side"),
        execution_mode: row.get("execution_mode"),
        trigger_condition: row.get("trigger_condition"),
        trigger_price: row.get("trigger_price"),
        max_price: row.get("max_price"),
        guard_trigger_price: row.get("guard_trigger_price"),
        best_ask_floor_price: row.get("best_ask_floor_price"),
        size_basis: row.get("size_basis"),
        size_usdc: row.get("size_usdc"),
        target_qty: row.get("target_qty"),
        min_price_distance_cent: row.get("min_price_distance_cent"),
        expires_at: row.get("expires_at"),
        eligible_after_at: row.get("eligible_after_at"),
        eligible_before_at: row.get("eligible_before_at"),
        max_triggers: row.get("max_triggers"),
        triggers_fired: row.get("triggers_fired"),
        active_exchange_order_id: row.get("active_exchange_order_id"),
        remaining_size: row.get("remaining_size"),
        remaining_qty: row.get("remaining_qty"),
        working_price: row.get("working_price"),
        last_seen_price: row.get("last_seen_price"),
        last_error: row.get("last_error"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        parent_order_id: row.get("parent_order_id"),
        origin_flow_definition_id: row.get("origin_flow_definition_id"),
        origin_flow_run_id: row.get("origin_flow_run_id"),
        origin_flow_node_key: row.get("origin_flow_node_key"),
        pair_session_id: row.get("pair_session_id"),
        pair_leg_role: row.get("pair_leg_role"),
        tp_enabled: row.get("tp_enabled"),
        tp_price: row.get("tp_price"),
        tp_rules_json: trade_builder_parse_rules(row.get("tp_rules_json")),
        sl_enabled: row.get("sl_enabled"),
        sl_price: row.get("sl_price"),
        sl_rules_json: trade_builder_parse_rules(row.get("sl_rules_json")),
        time_exit_rules_json: trade_builder_parse_rules(row.get("time_exit_rules_json")),
        filled_qty: row.get("filled_qty"),
        fee_rate_bps: row.get("fee_rate_bps"),
        trigger_latched: row.get("trigger_latched"),
        trigger_latched_reason: row.get("trigger_latched_reason"),
        trigger_latched_at: row.get("trigger_latched_at"),
        submitted_dynamic_qty: row.get("submitted_dynamic_qty"),
        submitted_dynamic_price: row.get("submitted_dynamic_price"),
        runtime_snapshot_json: row.get("runtime_snapshot_json"),
        fresh_submit_lease_until: row.get("fresh_submit_lease_until"),
        retry_on_trigger_guard_block: row.get("retry_on_trigger_guard_block"),
        retry_on_execution_floor_guard_block: row.get("retry_on_execution_floor_guard_block"),
        retry_on_max_price_block: row.get("retry_on_max_price_block"),
        ptb_stop_loss_gap_usd: row.get("ptb_stop_loss_gap_usd"),
        ptb_reference_price: row.get("ptb_reference_price"),
        ptb_stop_loss_rules_json: trade_builder_parse_rules(row.get("ptb_stop_loss_rules_json")),
        ptb_stop_loss_time_decay_mode: row.get("ptb_stop_loss_time_decay_mode"),
        ptb_current_price_source: row.get("ptb_current_price_source"),
        staged_sl_retry_only_dust: row.get("staged_sl_retry_only_dust"),
        staged_sl_retry_dust_metric: row.get("staged_sl_retry_dust_metric"),
        staged_sl_retry_dust_value: row.get("staged_sl_retry_dust_value"),
        staged_sl_reentry_use_sold_notional: row.get("staged_sl_reentry_use_sold_notional"),
        staged_sl_reentry_only_after_all_stages: row.get("staged_sl_reentry_only_after_all_stages"),
        sl_trigger_price_mode: row.get("sl_trigger_price_mode"),
        reenter_on_sl_hit: row.get("reenter_on_sl_hit"),
        reentry_max_attempts: row.get("reentry_max_attempts"),
        reentry_trigger_node_key: row.get("reentry_trigger_node_key"),
        notify_on_order_submitted: row.get("notify_on_order_submitted"),
        notify_on_fill: row.get("notify_on_fill"),
        notify_on_order_not_filled: row.get("notify_on_order_not_filled"),
        notify_on_trigger_guard_blocked: row.get("notify_on_trigger_guard_blocked"),
        notify_on_execution_floor_blocked: row.get("notify_on_execution_floor_blocked"),
        notify_on_tp_hit: row.get("notify_on_tp_hit"),
        notify_on_sl_hit: row.get("notify_on_sl_hit"),
        notify_on_max_price_blocked: row.get("notify_on_max_price_blocked"),
        last_guard_notification_reason: row.get("last_guard_notification_reason"),
        exit_ladder_kind: row.get("exit_ladder_kind"),
        exit_ladder_index: row.get("exit_ladder_index"),
        exit_ladder_size_pct: row.get("exit_ladder_size_pct"),
    }
}

impl PostgresRepository {
    #[allow(clippy::too_many_arguments)]
    pub async fn create_trade_builder_order_with_exit_ladders(
        &self,
        trade_id: i64,
        kind: &str,
        status: &str,
        market_slug: &str,
        token_id: &str,
        outcome_label: &str,
        side: &str,
        execution_mode: &str,
        trigger_condition: Option<&str>,
        trigger_price: Option<f64>,
        max_price: Option<f64>,
        guard_trigger_price: Option<f64>,
        best_ask_floor_price: Option<f64>,
        size_basis: &str,
        size_usdc: f64,
        target_qty: Option<f64>,
        remaining_qty: Option<f64>,
        min_price_distance_cent: f64,
        expires_at: Option<DateTime<Utc>>,
        eligible_after_at: Option<DateTime<Utc>>,
        eligible_before_at: Option<DateTime<Utc>>,
        max_triggers: i32,
        parent_order_id: Option<i64>,
        tp_enabled: bool,
        tp_price: Option<f64>,
        tp_rules_json: Option<&[TradeBuilderPriceExitRule]>,
        sl_enabled: bool,
        sl_price: Option<f64>,
        sl_rules_json: Option<&[TradeBuilderPriceExitRule]>,
        time_exit_rules_json: Option<&[TradeBuilderTimeExitRule]>,
        fee_rate_bps: i64,
        origin_flow_definition_id: Option<i64>,
        origin_flow_run_id: Option<i64>,
        origin_flow_node_key: Option<&str>,
        ptb_stop_loss_gap_usd: Option<f64>,
        ptb_reference_price: Option<f64>,
        ptb_stop_loss_rules_json: Option<&[TradeBuilderPtbStopLossRule]>,
        ptb_stop_loss_time_decay_mode: Option<&str>,
        ptb_current_price_source: Option<&str>,
        staged_sl_retry_only_dust: bool,
        staged_sl_retry_dust_metric: Option<&str>,
        staged_sl_retry_dust_value: Option<f64>,
        staged_sl_reentry_use_sold_notional: bool,
        staged_sl_reentry_only_after_all_stages: bool,
        sl_trigger_price_mode: Option<&str>,
        reenter_on_sl_hit: bool,
        reentry_max_attempts: i32,
        reentry_trigger_node_key: Option<&str>,
        notify_on_order_submitted: bool,
        notify_on_fill: bool,
        notify_on_order_not_filled: bool,
        notify_on_trigger_guard_blocked: bool,
        notify_on_execution_floor_blocked: bool,
        notify_on_tp_hit: bool,
        notify_on_sl_hit: bool,
        notify_on_max_price_blocked: bool,
        last_guard_notification_reason: Option<&str>,
        retry_on_trigger_guard_block: bool,
        retry_on_execution_floor_guard_block: bool,
        retry_on_max_price_block: bool,
        exit_ladder_kind: Option<&str>,
        exit_ladder_index: Option<i32>,
        exit_ladder_size_pct: Option<f64>,
    ) -> Result<i64> {
        let tp_rules_json = trade_builder_price_exit_rules_to_json(tp_rules_json);
        let sl_rules_json = trade_builder_price_exit_rules_to_json(sl_rules_json);
        let time_exit_rules_json = trade_builder_time_exit_rules_to_json(time_exit_rules_json);
        let ptb_stop_loss_rules_json =
            trade_builder_ptb_stop_loss_rules_to_json(ptb_stop_loss_rules_json);
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO trade_builder_orders \
              (trade_id, user_id, kind, status, market_slug, token_id, outcome_label, side, execution_mode, trigger_condition, trigger_price, max_price, guard_trigger_price, best_ask_floor_price, size_basis, size_usdc, target_qty, remaining_qty, min_price_distance_cent, expires_at, eligible_after_at, eligible_before_at, max_triggers, triggers_fired, parent_order_id, tp_enabled, tp_price, tp_rules_json, sl_enabled, sl_price, sl_rules_json, time_exit_rules_json, fee_rate_bps, origin_flow_definition_id, origin_flow_run_id, origin_flow_node_key, ptb_stop_loss_gap_usd, ptb_reference_price, ptb_stop_loss_rules_json, ptb_stop_loss_time_decay_mode, ptb_current_price_source, staged_sl_retry_only_dust, staged_sl_retry_dust_metric, staged_sl_retry_dust_value, staged_sl_reentry_use_sold_notional, staged_sl_reentry_only_after_all_stages, sl_trigger_price_mode, reenter_on_sl_hit, reentry_max_attempts, reentry_trigger_node_key, notify_on_order_submitted, notify_on_fill, notify_on_order_not_filled, notify_on_trigger_guard_blocked, notify_on_execution_floor_blocked, notify_on_tp_hit, notify_on_sl_hit, notify_on_max_price_blocked, last_guard_notification_reason, retry_on_trigger_guard_block, retry_on_execution_floor_guard_block, retry_on_max_price_block, exit_ladder_kind, exit_ladder_index, exit_ladder_size_pct, created_at, updated_at) \
             VALUES \
              ($1, (SELECT user_id FROM trades WHERE id = $1), $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, 0, $23, $24, $25, $26, $27, $28, $29, $30, \
               $31, \
               COALESCE($32, CASE WHEN $23 IS NOT NULL THEN (SELECT origin_flow_definition_id FROM trade_builder_orders WHERE id = $23) ELSE NULL END), \
               COALESCE($33, CASE WHEN $23 IS NOT NULL THEN (SELECT origin_flow_run_id FROM trade_builder_orders WHERE id = $23) ELSE NULL END), \
               COALESCE($34, CASE WHEN $23 IS NOT NULL THEN (SELECT origin_flow_node_key FROM trade_builder_orders WHERE id = $23) ELSE NULL END), \
               $35, $36, $37, $38, COALESCE($39, 'chainlink'), $40, $41, $42, $43, $44, $45, $46, $47, $48, $49, $50, $51, $52, $53, $54, $55, $56, $57, $58, $59, $60, $61, $62, $63, \
               NOW(), NOW()) \
             RETURNING id",
        )
        .bind(trade_id)
        .bind(kind)
        .bind(status)
        .bind(market_slug)
        .bind(token_id)
        .bind(outcome_label)
        .bind(side)
        .bind(execution_mode)
        .bind(trigger_condition)
        .bind(trigger_price)
        .bind(max_price)
        .bind(guard_trigger_price)
        .bind(best_ask_floor_price)
        .bind(size_basis)
        .bind(size_usdc)
        .bind(target_qty)
        .bind(remaining_qty)
        .bind(min_price_distance_cent)
        .bind(expires_at)
        .bind(eligible_after_at)
        .bind(eligible_before_at)
        .bind(max_triggers)
        .bind(parent_order_id)
        .bind(tp_enabled)
        .bind(tp_price)
        .bind(tp_rules_json)
        .bind(sl_enabled)
        .bind(sl_price)
        .bind(sl_rules_json)
        .bind(time_exit_rules_json)
        .bind(fee_rate_bps)
        .bind(origin_flow_definition_id)
        .bind(origin_flow_run_id)
        .bind(origin_flow_node_key)
        .bind(ptb_stop_loss_gap_usd)
        .bind(ptb_reference_price)
        .bind(ptb_stop_loss_rules_json)
        .bind(ptb_stop_loss_time_decay_mode)
        .bind(ptb_current_price_source)
        .bind(staged_sl_retry_only_dust)
        .bind(staged_sl_retry_dust_metric)
        .bind(staged_sl_retry_dust_value)
        .bind(staged_sl_reentry_use_sold_notional)
        .bind(staged_sl_reentry_only_after_all_stages)
        .bind(sl_trigger_price_mode)
        .bind(reenter_on_sl_hit)
        .bind(reentry_max_attempts)
        .bind(reentry_trigger_node_key)
        .bind(notify_on_order_submitted)
        .bind(notify_on_fill)
        .bind(notify_on_order_not_filled)
        .bind(notify_on_trigger_guard_blocked)
        .bind(notify_on_execution_floor_blocked)
        .bind(notify_on_tp_hit)
        .bind(notify_on_sl_hit)
        .bind(notify_on_max_price_blocked)
        .bind(last_guard_notification_reason)
        .bind(retry_on_trigger_guard_block)
        .bind(retry_on_execution_floor_guard_block)
        .bind(retry_on_max_price_block)
        .bind(exit_ladder_kind)
        .bind(exit_ladder_index)
        .bind(exit_ladder_size_pct)
        .fetch_one(self.pool())
        .await?;
        Ok(id)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_trade_builder_order(
        &self,
        trade_id: i64,
        kind: &str,
        status: &str,
        market_slug: &str,
        token_id: &str,
        outcome_label: &str,
        side: &str,
        execution_mode: &str,
        trigger_condition: Option<&str>,
        trigger_price: Option<f64>,
        max_price: Option<f64>,
        guard_trigger_price: Option<f64>,
        best_ask_floor_price: Option<f64>,
        size_basis: &str,
        size_usdc: f64,
        target_qty: Option<f64>,
        remaining_qty: Option<f64>,
        min_price_distance_cent: f64,
        expires_at: Option<DateTime<Utc>>,
        eligible_after_at: Option<DateTime<Utc>>,
        eligible_before_at: Option<DateTime<Utc>>,
        max_triggers: i32,
        parent_order_id: Option<i64>,
        tp_enabled: bool,
        tp_price: Option<f64>,
        sl_enabled: bool,
        sl_price: Option<f64>,
        fee_rate_bps: i64,
        origin_flow_definition_id: Option<i64>,
        origin_flow_run_id: Option<i64>,
        origin_flow_node_key: Option<&str>,
        sl_trigger_price_mode: Option<&str>,
        reenter_on_sl_hit: bool,
        reentry_max_attempts: i32,
        reentry_trigger_node_key: Option<&str>,
        ptb_stop_loss_time_decay_mode: Option<&str>,
        notify_on_order_submitted: bool,
        notify_on_fill: bool,
        notify_on_order_not_filled: bool,
        notify_on_trigger_guard_blocked: bool,
        notify_on_execution_floor_blocked: bool,
        notify_on_tp_hit: bool,
        notify_on_sl_hit: bool,
        notify_on_max_price_blocked: bool,
        last_guard_notification_reason: Option<&str>,
        retry_on_trigger_guard_block: bool,
        retry_on_execution_floor_guard_block: bool,
        retry_on_max_price_block: bool,
    ) -> Result<i64> {
        self.create_trade_builder_order_with_exit_ladders(
            trade_id,
            kind,
            status,
            market_slug,
            token_id,
            outcome_label,
            side,
            execution_mode,
            trigger_condition,
            trigger_price,
            max_price,
            guard_trigger_price,
            best_ask_floor_price,
            size_basis,
            size_usdc,
            target_qty,
            remaining_qty,
            min_price_distance_cent,
            expires_at,
            eligible_after_at,
            eligible_before_at,
            max_triggers,
            parent_order_id,
            tp_enabled,
            tp_price,
            None,
            sl_enabled,
            sl_price,
            None,
            None,
            fee_rate_bps,
            origin_flow_definition_id,
            origin_flow_run_id,
            origin_flow_node_key,
            None,
            None,
            None,
            ptb_stop_loss_time_decay_mode,
            None,
            false,
            None,
            None,
            false,
            false,
            sl_trigger_price_mode,
            reenter_on_sl_hit,
            reentry_max_attempts,
            reentry_trigger_node_key,
            notify_on_order_submitted,
            notify_on_fill,
            notify_on_order_not_filled,
            notify_on_trigger_guard_blocked,
            notify_on_execution_floor_blocked,
            notify_on_tp_hit,
            notify_on_sl_hit,
            notify_on_max_price_blocked,
            last_guard_notification_reason,
            retry_on_trigger_guard_block,
            retry_on_execution_floor_guard_block,
            retry_on_max_price_block,
            None,
            None,
            None,
        )
        .await
    }

    pub async fn unblock_next_trade_builder_order(
        &self,
        trade_id: i64,
        token_id: &str,
    ) -> Result<Option<i64>> {
        let row: Option<(i64,)> = sqlx::query_as(
            "UPDATE trade_builder_orders SET status = 'pending', updated_at = NOW() \
             WHERE id = (SELECT id FROM trade_builder_orders \
                         WHERE trade_id = $1 AND token_id = $2 AND status = 'blocked' \
                         ORDER BY id ASC LIMIT 1) \
             RETURNING id",
        )
        .bind(trade_id)
        .bind(token_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|(id,)| id))
    }

    pub async fn list_trade_builder_orders_for_processing(
        &self,
        limit: i64,
    ) -> Result<Vec<TradeBuilderOrder>> {
        let rows = sqlx::query(&format!(
            "SELECT {TRADE_BUILDER_ORDER_SELECT_COLUMNS} \
             FROM trade_builder_orders \
             WHERE (
                    status IN ('pending', 'open', 'partially_filled', 'canceled_requested', 'inventory_pending', 'guard_blocked') \
                OR (status IN ('armed', 'triggered') \
                    AND NOT (
                        parent_order_id IS NOT NULL \
                        AND side = 'sell' \
                        AND kind = 'conditional' \
                        AND trigger_condition IS NOT NULL \
                        AND trigger_price IS NOT NULL
                    )) \
                OR (status = 'error' AND trigger_latched = TRUE AND trigger_latched_reason = 'stop_loss') \
                 ) \
               AND (fresh_submit_lease_until IS NULL OR fresh_submit_lease_until <= NOW()) \
             ORDER BY \
                CASE \
                    WHEN trigger_latched = TRUE AND trigger_latched_reason = 'stop_loss' THEN 0 \
                    WHEN parent_order_id IS NOT NULL AND side = 'sell' AND trigger_condition = 'cross_below' THEN 1 \
                    WHEN parent_order_id IS NOT NULL AND side = 'sell' AND trigger_condition = 'cross_above' THEN 2 \
                    ELSE 3 \
                END ASC, \
                created_at ASC \
             LIMIT $1"
        ))
        .bind(limit)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(map_trade_builder_order_row)
            .filter(trade_builder_order_requires_housekeeping)
            .collect())
    }

    pub async fn list_armed_tp_sl_child_builder_orders(&self) -> Result<Vec<TradeBuilderOrder>> {
        let rows = sqlx::query(&format!(
            "SELECT {TRADE_BUILDER_ORDER_SELECT_COLUMNS} \
             FROM trade_builder_orders \
             WHERE parent_order_id IS NOT NULL \
               AND side = 'sell' \
               AND kind = 'conditional' \
               AND status IN ('armed', 'triggered') \
               AND trigger_condition IS NOT NULL \
               AND (trigger_price IS NOT NULL OR ptb_stop_loss_gap_usd IS NOT NULL) \
             ORDER BY created_at ASC"
        ))
        .fetch_all(self.pool())
        .await?;

        Ok(rows.into_iter().map(map_trade_builder_order_row).collect())
    }

    pub async fn list_active_dual_dca_conditional_orders(
        &self,
        job_id: i64,
        market_slug: Option<&str>,
    ) -> Result<Vec<TradeBuilderOrder>> {
        let rows = sqlx::query(&format!(
            "SELECT DISTINCT {TRADE_BUILDER_ORDER_SELECT_COLUMNS_O_ALIAS} \
             FROM trade_builder_orders o \
             JOIN trade_flow_dual_dca_legs l ON l.builder_order_id = o.id \
             WHERE l.job_id = $1 \
               AND ($2::text IS NULL OR l.market_slug = $2) \
               AND o.kind = 'conditional' \
               AND o.status IN ('pending', 'armed', 'triggered', 'open', 'partially_filled', 'inventory_pending', 'guard_blocked') \
             ORDER BY o.id ASC"
        ))
        .bind(job_id)
        .bind(market_slug)
        .fetch_all(self.pool())
        .await?;

        Ok(rows.into_iter().map(map_trade_builder_order_row).collect())
    }

    pub async fn append_trade_builder_order_event(
        &self,
        builder_order_id: i64,
        event_type: &str,
        payload: &Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO trade_builder_order_events (builder_order_id, event_type, payload_json, created_at) \
             VALUES ($1, $2, $3, NOW())",
        )
        .bind(builder_order_id)
        .bind(event_type)
        .bind(payload)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_live_gap_metadata(
        &self,
        builder_order_id: i64,
        metadata: Option<&Value>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET live_gap_metadata_json = $2, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(metadata)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn load_trade_builder_order_live_gap_metadata(
        &self,
        builder_order_id: i64,
    ) -> Result<Option<Value>> {
        let metadata = sqlx::query_scalar::<_, Option<Value>>(
            "SELECT live_gap_metadata_json FROM trade_builder_orders WHERE id = $1",
        )
        .bind(builder_order_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(metadata.flatten().filter(|value| !value.is_null()))
    }

    pub async fn load_trade_builder_order_trigger_plan(
        &self,
        builder_order_id: i64,
    ) -> Result<Option<(Option<String>, Vec<f64>)>> {
        let payload = sqlx::query_scalar::<_, Value>(
            "SELECT payload_json
             FROM trade_builder_order_events
             WHERE builder_order_id = $1
               AND event_type = 'flow_created'
             ORDER BY id DESC
             LIMIT 1",
        )
        .bind(builder_order_id)
        .fetch_optional(self.pool())
        .await?;

        let Some(payload) = payload else {
            return Ok(None);
        };

        let size_mode = payload
            .get("size_mode")
            .and_then(Value::as_str)
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| value == "usdc" || value == "pct");

        let trigger_sizes = payload
            .get("trigger_sizes")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|item| match item {
                        Value::Number(v) => v.as_f64(),
                        Value::String(v) => v.parse::<f64>().ok(),
                        _ => None,
                    })
                    .filter(|value| value.is_finite() && *value > 0.0)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(Some((size_mode, trigger_sizes)))
    }

    pub async fn load_trade_builder_order_flow_created_payload(
        &self,
        builder_order_id: i64,
    ) -> Result<Option<Value>> {
        let payload = sqlx::query_scalar::<_, Value>(
            "SELECT payload_json
             FROM trade_builder_order_events
             WHERE builder_order_id = $1
               AND event_type = 'flow_created'
             ORDER BY id DESC
             LIMIT 1",
        )
        .bind(builder_order_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(payload)
    }

    pub async fn set_trade_builder_order_status(
        &self,
        builder_order_id: i64,
        status: &str,
        last_error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders SET status = $2, last_error = $3, updated_at = NOW() WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(status)
        .bind(last_error)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_last_error(
        &self,
        builder_order_id: i64,
        last_error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders SET last_error = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(last_error)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_fee_rate_bps(
        &self,
        builder_order_id: i64,
        fee_rate_bps: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders SET fee_rate_bps = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(fee_rate_bps)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_filled_qty(
        &self,
        builder_order_id: i64,
        filled_qty: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders SET filled_qty = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(filled_qty)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_submitted_dynamic(
        &self,
        builder_order_id: i64,
        submitted_dynamic_qty: Option<f64>,
        submitted_dynamic_price: Option<f64>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET submitted_dynamic_qty = $2, submitted_dynamic_price = $3, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(submitted_dynamic_qty)
        .bind(submitted_dynamic_price)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_runtime_snapshot(
        &self,
        builder_order_id: i64,
        runtime_snapshot_json: Option<&Value>,
        fresh_submit_lease_until: Option<DateTime<Utc>>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET runtime_snapshot_json = $2, fresh_submit_lease_until = $3, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(runtime_snapshot_json)
        .bind(fresh_submit_lease_until)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_notification_flags(
        &self,
        builder_order_id: i64,
        notify_on_order_submitted: bool,
        notify_on_fill: bool,
        notify_on_order_not_filled: bool,
        notify_on_trigger_guard_blocked: bool,
        notify_on_execution_floor_blocked: bool,
        notify_on_tp_hit: bool,
        notify_on_sl_hit: bool,
        notify_on_max_price_blocked: bool,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET notify_on_fill = $2, \
                 notify_on_order_submitted = $3, \
                 notify_on_order_not_filled = $4, \
                 notify_on_trigger_guard_blocked = $5, \
                 notify_on_execution_floor_blocked = $6, \
                 notify_on_tp_hit = $7, \
                 notify_on_sl_hit = $8, \
                 notify_on_max_price_blocked = $9, \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(notify_on_fill)
        .bind(notify_on_order_submitted)
        .bind(notify_on_order_not_filled)
        .bind(notify_on_trigger_guard_blocked)
        .bind(notify_on_execution_floor_blocked)
        .bind(notify_on_tp_hit)
        .bind(notify_on_sl_hit)
        .bind(notify_on_max_price_blocked)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn update_trade_builder_guard_notification_reason(
        &self,
        builder_order_id: i64,
        reason: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET last_guard_notification_reason = $2, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(reason)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_guard_retry_flags(
        &self,
        builder_order_id: i64,
        retry_on_trigger_guard_block: bool,
        retry_on_execution_floor_guard_block: bool,
        retry_on_max_price_block: bool,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET retry_on_trigger_guard_block = $2, \
                 retry_on_execution_floor_guard_block = $3, \
                 retry_on_max_price_block = $4, \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(retry_on_trigger_guard_block)
        .bind(retry_on_execution_floor_guard_block)
        .bind(retry_on_max_price_block)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_staged_sl_behavior(
        &self,
        builder_order_id: i64,
        staged_sl_reentry_only_after_all_stages: bool,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET staged_sl_retry_only_dust = FALSE, \
                 staged_sl_retry_dust_metric = NULL, \
                 staged_sl_retry_dust_value = NULL, \
                 staged_sl_reentry_use_sold_notional = FALSE, \
                 staged_sl_reentry_only_after_all_stages = $2, \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(staged_sl_reentry_only_after_all_stages)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_ptb_stop_loss(
        &self,
        builder_order_id: i64,
        ptb_stop_loss_gap_usd: Option<f64>,
        ptb_reference_price: Option<f64>,
        ptb_stop_loss_rules_json: Option<&[TradeBuilderPtbStopLossRule]>,
        ptb_stop_loss_time_decay_mode: Option<&str>,
        ptb_current_price_source: Option<&str>,
    ) -> Result<()> {
        let ptb_stop_loss_rules_json =
            trade_builder_ptb_stop_loss_rules_to_json(ptb_stop_loss_rules_json);
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET ptb_stop_loss_gap_usd = $2, \
                 ptb_reference_price = $3, \
                 ptb_stop_loss_rules_json = $4, \
                 ptb_stop_loss_time_decay_mode = $5, \
                 ptb_current_price_source = COALESCE($6, 'chainlink'), \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(ptb_stop_loss_gap_usd)
        .bind(ptb_reference_price)
        .bind(ptb_stop_loss_rules_json)
        .bind(ptb_stop_loss_time_decay_mode)
        .bind(ptb_current_price_source)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_ptb_reference_price(
        &self,
        builder_order_id: i64,
        ptb_reference_price: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET ptb_reference_price = $2, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(ptb_reference_price)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn list_guard_blocked_immediate_buy_builder_orders(
        &self,
    ) -> Result<Vec<TradeBuilderOrder>> {
        let rows = sqlx::query(&format!(
            "SELECT {TRADE_BUILDER_ORDER_SELECT_COLUMNS} \
             FROM trade_builder_orders \
             WHERE side = 'buy' \
               AND kind = 'immediate' \
               AND status = 'guard_blocked' \
               AND active_exchange_order_id IS NULL \
             ORDER BY created_at ASC"
        ))
        .fetch_all(self.pool())
        .await?;

        Ok(rows.into_iter().map(map_trade_builder_order_row).collect())
    }

    pub async fn set_trade_builder_order_trigger_latched(
        &self,
        builder_order_id: i64,
        trigger_latched: bool,
        trigger_latched_reason: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET trigger_latched = $2, trigger_latched_reason = $3, trigger_latched_at = CASE WHEN $2 = TRUE THEN NOW() ELSE NULL END, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(trigger_latched)
        .bind(trigger_latched_reason)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_last_seen_price(
        &self,
        builder_order_id: i64,
        last_seen_price: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders SET last_seen_price = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(last_seen_price)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_working_state(
        &self,
        builder_order_id: i64,
        active_exchange_order_id: Option<&str>,
        working_price: Option<f64>,
        remaining_size: Option<f64>,
        remaining_qty: Option<f64>,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET active_exchange_order_id = $2, working_price = $3, remaining_size = $4, remaining_qty = $5, status = $6, last_error = NULL, fresh_submit_lease_until = NULL, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(active_exchange_order_id)
        .bind(working_price)
        .bind(remaining_size)
        .bind(remaining_qty)
        .bind(status)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn update_trade_builder_order_sizing_and_state(
        &self,
        builder_order_id: i64,
        size_basis: &str,
        size_usdc: f64,
        target_qty: Option<f64>,
        remaining_size: Option<f64>,
        remaining_qty: Option<f64>,
        status: &str,
        last_error: Option<&str>,
        eligible_after_at: Option<DateTime<Utc>>,
        eligible_before_at: Option<DateTime<Utc>>,
        origin_flow_definition_id: Option<i64>,
        origin_flow_run_id: Option<i64>,
        origin_flow_node_key: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET size_basis = $2, size_usdc = $3, target_qty = $4, remaining_size = $5, remaining_qty = $6, \
                 active_exchange_order_id = NULL, working_price = NULL, status = $7, last_error = $8, \
                 eligible_after_at = $9, eligible_before_at = $10, \
                 origin_flow_definition_id = COALESCE($11, origin_flow_definition_id), \
                 origin_flow_run_id = COALESCE($12, origin_flow_run_id), \
                 origin_flow_node_key = COALESCE($13, origin_flow_node_key), \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(size_basis)
        .bind(size_usdc)
        .bind(target_qty)
        .bind(remaining_size)
        .bind(remaining_qty)
        .bind(status)
        .bind(last_error)
        .bind(eligible_after_at)
        .bind(eligible_before_at)
        .bind(origin_flow_definition_id)
        .bind(origin_flow_run_id)
        .bind(origin_flow_node_key)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_retry_state(
        &self,
        builder_order_id: i64,
        status: &str,
        last_error: Option<&str>,
        remaining_size: Option<f64>,
        remaining_qty: Option<f64>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET active_exchange_order_id = NULL, working_price = NULL, remaining_size = $2, remaining_qty = $3, status = $4, last_error = $5, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(remaining_size)
        .bind(remaining_qty)
        .bind(status)
        .bind(last_error)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_guard_blocked_state(
        &self,
        builder_order_id: i64,
        reason: &str,
        remaining_size: Option<f64>,
        remaining_qty: Option<f64>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET active_exchange_order_id = NULL, \
                 working_price = NULL, \
                 remaining_size = $3, \
                 remaining_qty = $4, \
                 status = 'guard_blocked', \
                 last_error = $2, \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(reason)
        .bind(remaining_size)
        .bind(remaining_qty)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn increment_trade_builder_trigger_count(&self, builder_order_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders SET triggers_fired = triggers_fired + 1, updated_at = NOW() WHERE id = $1",
        )
        .bind(builder_order_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn clear_trade_builder_active_exchange_order(
        &self,
        builder_order_id: i64,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET active_exchange_order_id = NULL, remaining_size = NULL, remaining_qty = NULL, working_price = NULL, status = $2, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(status)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn clear_trade_builder_active_exchange_order_preserve_sizing(
        &self,
        builder_order_id: i64,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET active_exchange_order_id = NULL, working_price = NULL, status = $2, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(status)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn get_trade_builder_order(
        &self,
        builder_order_id: i64,
    ) -> Result<Option<TradeBuilderOrder>> {
        let row = sqlx::query(&format!(
            "SELECT {TRADE_BUILDER_ORDER_SELECT_COLUMNS} \
             FROM trade_builder_orders WHERE id = $1"
        ))
        .bind(builder_order_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(map_trade_builder_order_row))
    }

    pub async fn list_trade_builder_child_orders_by_parent(
        &self,
        parent_id: i64,
        exclude_order_id: Option<i64>,
    ) -> Result<Vec<TradeBuilderOrder>> {
        let rows = sqlx::query(&format!(
            "SELECT {TRADE_BUILDER_ORDER_SELECT_COLUMNS} \
             FROM trade_builder_orders \
             WHERE parent_order_id = $1
               AND ($2::bigint IS NULL OR id <> $2)
             ORDER BY id ASC"
        ))
        .bind(parent_id)
        .bind(exclude_order_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows.into_iter().map(map_trade_builder_order_row).collect())
    }

    pub async fn list_trade_builder_orders_by_pair_session(
        &self,
        pair_session_id: i64,
    ) -> Result<Vec<TradeBuilderOrder>> {
        let rows = sqlx::query(&format!(
            "SELECT {TRADE_BUILDER_ORDER_SELECT_COLUMNS} \
             FROM trade_builder_orders \
             WHERE pair_session_id = $1
             ORDER BY id ASC"
        ))
        .bind(pair_session_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows.into_iter().map(map_trade_builder_order_row).collect())
    }

    pub async fn cancel_child_orders_by_parent(&self, parent_id: i64) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE trade_builder_orders SET status = 'canceled', updated_at = NOW() \
             WHERE parent_order_id = $1 AND status NOT IN ('completed', 'canceled', 'expired', 'filled')",
        )
        .bind(parent_id)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn update_trade_builder_order_params(
        &self,
        builder_order_id: i64,
        min_price_distance_cent: Option<f64>,
        max_triggers: Option<i32>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET min_price_distance_cent = COALESCE($2, min_price_distance_cent), \
                 max_triggers = COALESCE($3, max_triggers), \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(min_price_distance_cent)
        .bind(max_triggers)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_pair_session(
        &self,
        builder_order_id: i64,
        pair_session_id: Option<i64>,
        pair_leg_role: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET pair_session_id = $2, pair_leg_role = $3, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(pair_session_id)
        .bind(pair_leg_role)
        .execute(self.pool())
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn test_trade_builder_order(status: &str) -> TradeBuilderOrder {
        TradeBuilderOrder {
            id: 1,
            trade_id: 77,
            user_id: 1,
            kind: "immediate".to_string(),
            status: status.to_string(),
            market_slug: "eth-updown-5m-1".to_string(),
            token_id: "tok-up".to_string(),
            outcome_label: "Up".to_string(),
            side: "buy".to_string(),
            execution_mode: "market".to_string(),
            trigger_condition: None,
            trigger_price: None,
            max_price: None,
            guard_trigger_price: None,
            best_ask_floor_price: None,
            size_basis: "notional_usdc".to_string(),
            size_usdc: 5.0,
            target_qty: None,
            min_price_distance_cent: 1.0,
            expires_at: None,
            eligible_after_at: None,
            eligible_before_at: None,
            max_triggers: 1,
            triggers_fired: 0,
            active_exchange_order_id: None,
            remaining_size: None,
            remaining_qty: None,
            working_price: None,
            last_seen_price: None,
            last_error: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            parent_order_id: None,
            origin_flow_definition_id: None,
            origin_flow_run_id: None,
            origin_flow_node_key: None,
            pair_session_id: None,
            pair_leg_role: None,
            tp_enabled: false,
            tp_price: None,
            tp_rules_json: Vec::new(),
            sl_enabled: false,
            sl_price: None,
            sl_rules_json: Vec::new(),
            time_exit_rules_json: Vec::new(),
            filled_qty: 0.0,
            fee_rate_bps: 0,
            trigger_latched: false,
            trigger_latched_reason: None,
            trigger_latched_at: None,
            submitted_dynamic_qty: None,
            submitted_dynamic_price: None,
            runtime_snapshot_json: None,
            fresh_submit_lease_until: None,
            retry_on_trigger_guard_block: false,
            retry_on_execution_floor_guard_block: false,
            retry_on_max_price_block: false,
            ptb_stop_loss_gap_usd: None,
            ptb_reference_price: None,
            ptb_stop_loss_rules_json: Vec::new(),
            ptb_stop_loss_time_decay_mode: None,
            ptb_current_price_source: "chainlink".to_string(),
            staged_sl_retry_only_dust: false,
            staged_sl_retry_dust_metric: None,
            staged_sl_retry_dust_value: None,
            staged_sl_reentry_use_sold_notional: false,
            staged_sl_reentry_only_after_all_stages: false,
            sl_trigger_price_mode: None,
            reenter_on_sl_hit: false,
            reentry_max_attempts: 0,
            reentry_trigger_node_key: None,
            notify_on_order_submitted: false,
            notify_on_fill: false,
            notify_on_order_not_filled: false,
            notify_on_trigger_guard_blocked: false,
            notify_on_execution_floor_blocked: false,
            notify_on_tp_hit: false,
            notify_on_sl_hit: false,
            notify_on_max_price_blocked: false,
            last_guard_notification_reason: None,
            exit_ladder_kind: None,
            exit_ladder_index: None,
            exit_ladder_size_pct: None,
        }
    }

    #[test]
    fn housekeeping_excludes_armed_and_triggered_child_exit_sells() {
        let mut armed_child = test_trade_builder_order("armed");
        armed_child.parent_order_id = Some(9);
        armed_child.side = "sell".to_string();
        armed_child.kind = "conditional".to_string();
        armed_child.trigger_condition = Some("cross_below".to_string());
        armed_child.trigger_price = Some(0.60);

        let mut triggered_child = armed_child.clone();
        triggered_child.status = "triggered".to_string();

        assert!(!trade_builder_order_requires_housekeeping(&armed_child));
        assert!(!trade_builder_order_requires_housekeeping(&triggered_child));
    }

    #[test]
    fn housekeeping_excludes_armed_and_triggered_child_take_profit_sells() {
        let mut armed_child = test_trade_builder_order("armed");
        armed_child.parent_order_id = Some(9);
        armed_child.side = "sell".to_string();
        armed_child.kind = "conditional".to_string();
        armed_child.trigger_condition = Some("cross_above".to_string());
        armed_child.trigger_price = Some(0.84);

        let mut triggered_child = armed_child.clone();
        triggered_child.status = "triggered".to_string();

        assert!(!trade_builder_order_requires_housekeeping(&armed_child));
        assert!(!trade_builder_order_requires_housekeeping(&triggered_child));
    }

    #[test]
    fn housekeeping_keeps_repair_and_fallback_statuses() {
        for status in [
            "pending",
            "open",
            "partially_filled",
            "canceled_requested",
            "inventory_pending",
            "guard_blocked",
        ] {
            let mut child = test_trade_builder_order(status);
            child.parent_order_id = Some(9);
            child.side = "sell".to_string();
            child.kind = "conditional".to_string();
            child.trigger_condition = Some("cross_below".to_string());
            child.trigger_price = Some(0.60);
            assert!(
                trade_builder_order_requires_housekeeping(&child),
                "expected {status} to remain in housekeeping"
            );
        }
    }

    #[test]
    fn housekeeping_keeps_latched_stop_loss_errors_only() {
        let mut latched_error = test_trade_builder_order("error");
        latched_error.trigger_latched = true;
        latched_error.trigger_latched_reason = Some("stop_loss".to_string());

        let plain_error = test_trade_builder_order("error");

        assert!(trade_builder_order_requires_housekeeping(&latched_error));
        assert!(!trade_builder_order_requires_housekeeping(&plain_error));
    }
}
