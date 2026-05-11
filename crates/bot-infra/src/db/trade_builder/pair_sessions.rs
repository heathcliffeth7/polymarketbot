use super::super::*;
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::Row;

fn map_trade_builder_pair_session_row(row: sqlx::postgres::PgRow) -> TradeBuilderPairSession {
    TradeBuilderPairSession {
        id: row.get("id"),
        user_id: row.get("user_id"),
        flow_definition_id: row.get("flow_definition_id"),
        flow_run_id: row.get("flow_run_id"),
        flow_node_key: row.get("flow_node_key"),
        market_slug: row.get("market_slug"),
        status: row.get("status"),
        pair_target_total_cent: row.get("pair_target_total_cent"),
        min_net_profit_usdc: row.get("min_net_profit_usdc"),
        profit_safety_buffer_usdc: row.get("profit_safety_buffer_usdc"),
        orphan_grace_ms: row.get("orphan_grace_ms"),
        ignore_stop_loss_after_locked: row.get("ignore_stop_loss_after_locked"),
        notify_on_pair_locked: row.get("notify_on_pair_locked"),
        notify_on_pair_unwind: row.get("notify_on_pair_unwind"),
        notify_on_pair_no_edge: row.get("notify_on_pair_no_edge"),
        primary_order_id: row.get("primary_order_id"),
        counter_order_id: row.get("counter_order_id"),
        lead_order_id: row.get("lead_order_id"),
        primary_fill_qty: row.get("primary_fill_qty"),
        primary_fill_fee_qty: row.get("primary_fill_fee_qty"),
        primary_net_qty: row.get("primary_net_qty"),
        primary_avg_fill_price: row.get("primary_avg_fill_price"),
        counter_fill_qty: row.get("counter_fill_qty"),
        counter_fill_fee_qty: row.get("counter_fill_fee_qty"),
        counter_net_qty: row.get("counter_net_qty"),
        counter_avg_fill_price: row.get("counter_avg_fill_price"),
        lead_filled_at: row.get("lead_filled_at"),
        locked_qty: row.get("locked_qty"),
        projected_net_profit_usdc: row.get("projected_net_profit_usdc"),
        last_error: row.get("last_error"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

impl PostgresRepository {
    pub async fn create_trade_builder_pair_session(
        &self,
        user_id: i64,
        flow_definition_id: Option<i64>,
        flow_run_id: Option<i64>,
        flow_node_key: Option<&str>,
        market_slug: &str,
        pair_target_total_cent: f64,
        min_net_profit_usdc: f64,
        profit_safety_buffer_usdc: f64,
        orphan_grace_ms: i64,
        ignore_stop_loss_after_locked: bool,
        notify_on_pair_locked: bool,
        notify_on_pair_unwind: bool,
        notify_on_pair_no_edge: bool,
    ) -> Result<i64> {
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO trade_builder_pair_sessions \
              (user_id, flow_definition_id, flow_run_id, flow_node_key, market_slug, status, pair_target_total_cent, min_net_profit_usdc, profit_safety_buffer_usdc, orphan_grace_ms, ignore_stop_loss_after_locked, notify_on_pair_locked, notify_on_pair_unwind, notify_on_pair_no_edge, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, $4, $5, 'working', $6, $7, $8, $9, $10, $11, $12, $13, NOW(), NOW()) \
             RETURNING id",
        )
        .bind(user_id)
        .bind(flow_definition_id)
        .bind(flow_run_id)
        .bind(flow_node_key)
        .bind(market_slug)
        .bind(pair_target_total_cent)
        .bind(min_net_profit_usdc)
        .bind(profit_safety_buffer_usdc)
        .bind(orphan_grace_ms)
        .bind(ignore_stop_loss_after_locked)
        .bind(notify_on_pair_locked)
        .bind(notify_on_pair_unwind)
        .bind(notify_on_pair_no_edge)
        .fetch_one(self.pool())
        .await?;
        Ok(id)
    }

    pub async fn get_trade_builder_pair_session(
        &self,
        pair_session_id: i64,
    ) -> Result<Option<TradeBuilderPairSession>> {
        let row = sqlx::query(
            "SELECT id, user_id, flow_definition_id, flow_run_id, flow_node_key, market_slug, \
                    status, pair_target_total_cent, min_net_profit_usdc, profit_safety_buffer_usdc, \
                    orphan_grace_ms, ignore_stop_loss_after_locked, notify_on_pair_locked, notify_on_pair_unwind, notify_on_pair_no_edge, primary_order_id, counter_order_id, lead_order_id, \
                    primary_fill_qty, primary_fill_fee_qty, primary_net_qty, primary_avg_fill_price, \
                    counter_fill_qty, counter_fill_fee_qty, counter_net_qty, counter_avg_fill_price, \
                    lead_filled_at, locked_qty, projected_net_profit_usdc, last_error, created_at, updated_at \
             FROM trade_builder_pair_sessions \
             WHERE id = $1",
        )
        .bind(pair_session_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(map_trade_builder_pair_session_row))
    }

    pub async fn attach_trade_builder_pair_session_orders(
        &self,
        pair_session_id: i64,
        primary_order_id: i64,
        counter_order_id: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_pair_sessions \
             SET primary_order_id = $2, counter_order_id = $3, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(pair_session_id)
        .bind(primary_order_id)
        .bind(counter_order_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn record_trade_builder_pair_session_fill(
        &self,
        pair_session_id: i64,
        pair_leg_role: &str,
        builder_order_id: i64,
        fill_qty: f64,
        fill_fee_qty: f64,
        net_qty: f64,
        avg_fill_price: f64,
        filled_at: DateTime<Utc>,
    ) -> Result<()> {
        let is_primary = pair_leg_role.eq_ignore_ascii_case("lead_candidate");
        let query = if is_primary {
            "UPDATE trade_builder_pair_sessions \
             SET primary_fill_qty = $2, \
                 primary_fill_fee_qty = $3, \
                 primary_net_qty = $4, \
                 primary_avg_fill_price = $5, \
                 lead_order_id = COALESCE(lead_order_id, $6), \
                 lead_filled_at = COALESCE(lead_filled_at, $7), \
                 updated_at = NOW() \
             WHERE id = $1"
        } else {
            "UPDATE trade_builder_pair_sessions \
             SET counter_fill_qty = $2, \
                 counter_fill_fee_qty = $3, \
                 counter_net_qty = $4, \
                 counter_avg_fill_price = $5, \
                 lead_order_id = COALESCE(lead_order_id, $6), \
                 lead_filled_at = COALESCE(lead_filled_at, $7), \
                 updated_at = NOW() \
             WHERE id = $1"
        };

        sqlx::query(query)
            .bind(pair_session_id)
            .bind(fill_qty)
            .bind(fill_fee_qty)
            .bind(net_qty)
            .bind(avg_fill_price)
            .bind(builder_order_id)
            .bind(filled_at)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn update_trade_builder_pair_session_state(
        &self,
        pair_session_id: i64,
        status: &str,
        locked_qty: Option<f64>,
        projected_net_profit_usdc: Option<f64>,
        last_error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_pair_sessions \
             SET status = $2, \
                 locked_qty = COALESCE($3, locked_qty), \
                 projected_net_profit_usdc = COALESCE($4, projected_net_profit_usdc), \
                 last_error = $5, \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(pair_session_id)
        .bind(status)
        .bind(locked_qty)
        .bind(projected_net_profit_usdc)
        .bind(last_error)
        .execute(self.pool())
        .await?;
        Ok(())
    }
}
