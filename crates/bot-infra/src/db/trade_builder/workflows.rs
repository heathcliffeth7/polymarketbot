use super::super::*;

impl PostgresRepository {
    pub async fn list_trade_builder_workflows_for_processing(
        &self,
        limit: i64,
    ) -> Result<Vec<TradeBuilderWorkflow>> {
        let rows = sqlx::query(
            "SELECT id, user_id, name, status, source_trade_id, sell_target_pct, buy_start_after_sell_progress_pct, buy_trigger_mode, buy_allocation_pct, expires_at, last_error, created_at, updated_at \
             FROM trade_builder_workflows \
             WHERE status IN ('armed', 'running') \
             ORDER BY created_at ASC \
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeBuilderWorkflow {
                id: row.get("id"),
                user_id: row.get("user_id"),
                name: row.get("name"),
                status: row.get("status"),
                source_trade_id: row.get("source_trade_id"),
                sell_target_pct: row.get("sell_target_pct"),
                buy_start_after_sell_progress_pct: row.get("buy_start_after_sell_progress_pct"),
                buy_trigger_mode: row.get("buy_trigger_mode"),
                buy_allocation_pct: row.get("buy_allocation_pct"),
                expires_at: row.get("expires_at"),
                last_error: row.get("last_error"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn load_trade_builder_workflow_legs(
        &self,
        workflow_id: i64,
    ) -> Result<Vec<TradeBuilderWorkflowLeg>> {
        let rows = sqlx::query(
            "SELECT id, workflow_id, leg_type, market_slug, token_id, outcome_label, side, trigger_condition, trigger_price, min_price_distance_cent, status, builder_order_id, target_notional_usdc, allocated_notional_usdc, filled_notional_usdc, filled_qty, last_seen_price, created_at, updated_at \
             FROM trade_builder_workflow_legs \
             WHERE workflow_id = $1 \
             ORDER BY leg_type ASC, id ASC",
        )
        .bind(workflow_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeBuilderWorkflowLeg {
                id: row.get("id"),
                workflow_id: row.get("workflow_id"),
                leg_type: row.get("leg_type"),
                market_slug: row.get("market_slug"),
                token_id: row.get("token_id"),
                outcome_label: row.get("outcome_label"),
                side: row.get("side"),
                trigger_condition: row.get("trigger_condition"),
                trigger_price: row.get("trigger_price"),
                min_price_distance_cent: row.get("min_price_distance_cent"),
                status: row.get("status"),
                builder_order_id: row.get("builder_order_id"),
                target_notional_usdc: row.get("target_notional_usdc"),
                allocated_notional_usdc: row.get("allocated_notional_usdc"),
                filled_notional_usdc: row.get("filled_notional_usdc"),
                filled_qty: row.get("filled_qty"),
                last_seen_price: row.get("last_seen_price"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn set_trade_builder_workflow_status(
        &self,
        workflow_id: i64,
        status: &str,
        last_error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_workflows \
             SET status = $2, last_error = $3, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(workflow_id)
        .bind(status)
        .bind(last_error)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn append_trade_builder_workflow_event(
        &self,
        workflow_id: i64,
        leg_id: Option<i64>,
        event_type: &str,
        payload: &Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO trade_builder_workflow_events (workflow_id, leg_id, event_type, payload_json, created_at) \
             VALUES ($1, $2, $3, $4, NOW())",
        )
        .bind(workflow_id)
        .bind(leg_id)
        .bind(event_type)
        .bind(payload)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_workflow_leg_status(
        &self,
        leg_id: i64,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_workflow_legs SET status = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(leg_id)
        .bind(status)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_workflow_leg_builder_order(
        &self,
        leg_id: i64,
        builder_order_id: Option<i64>,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_workflow_legs \
             SET builder_order_id = $2, status = $3, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(leg_id)
        .bind(builder_order_id)
        .bind(status)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_workflow_leg_last_seen_price(
        &self,
        leg_id: i64,
        price: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_workflow_legs \
             SET last_seen_price = $2, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(leg_id)
        .bind(price)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn add_trade_builder_workflow_leg_allocated_notional(
        &self,
        leg_id: i64,
        delta_usdc: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_workflow_legs \
             SET allocated_notional_usdc = GREATEST(0, allocated_notional_usdc + $2), updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(leg_id)
        .bind(delta_usdc)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_workflow_leg_filled_metrics(
        &self,
        leg_id: i64,
        filled_notional_usdc: f64,
        filled_qty: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_workflow_legs \
             SET filled_notional_usdc = GREATEST(0, $2), \
                 filled_qty = GREATEST(0, $3), \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(leg_id)
        .bind(filled_notional_usdc)
        .bind(filled_qty)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn aggregate_trade_builder_workflow_leg_fills(
        &self,
        leg_id: i64,
    ) -> Result<(f64, f64)> {
        let (filled_notional_usdc, filled_qty) = sqlx::query_as::<_, (f64, f64)>(
            "WITH child_orders AS (
               SELECT DISTINCT (wfe.payload_json->>'builder_order_id')::bigint AS builder_order_id
               FROM trade_builder_workflow_events wfe
               WHERE wfe.leg_id = $1
                 AND wfe.payload_json ? 'builder_order_id'
                 AND (wfe.payload_json->>'builder_order_id') ~ '^[0-9]+$'
             ),
             exchange_ids AS (
               SELECT DISTINCT x.exchange_order_id
               FROM (
                 SELECT tboe.payload_json->>'exchange_order_id' AS exchange_order_id
                 FROM trade_builder_order_events tboe
                 JOIN child_orders co ON co.builder_order_id = tboe.builder_order_id
                 UNION ALL
                 SELECT tboe.payload_json->>'new_exchange_order_id' AS exchange_order_id
                 FROM trade_builder_order_events tboe
                 JOIN child_orders co ON co.builder_order_id = tboe.builder_order_id
                 UNION ALL
                 SELECT tboe.payload_json->>'prev_exchange_order_id' AS exchange_order_id
                 FROM trade_builder_order_events tboe
                 JOIN child_orders co ON co.builder_order_id = tboe.builder_order_id
               ) x
               WHERE x.exchange_order_id IS NOT NULL
                 AND x.exchange_order_id <> ''
             ),
             internal_orders AS (
               SELECT DISTINCT o.id
               FROM orders o
               JOIN exchange_ids e ON e.exchange_order_id = o.exchange_order_id
             )
             SELECT
               COALESCE(SUM(f.price * f.size), 0)::double precision AS filled_notional_usdc,
               COALESCE(SUM(f.size), 0)::double precision AS filled_qty
             FROM fills f
             JOIN internal_orders io ON io.id = f.order_id",
        )
        .bind(leg_id)
        .fetch_one(self.pool())
        .await?;

        Ok((filled_notional_usdc, filled_qty))
    }
}
