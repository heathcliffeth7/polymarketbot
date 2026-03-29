use super::super::*;

impl PostgresRepository {
    pub async fn list_trade_builder_root_orders_missing_auto_scope_analysis(
        &self,
        limit: i64,
    ) -> Result<Vec<i64>> {
        let ids = sqlx::query_scalar::<_, i64>(
            "SELECT o.id
             FROM trade_builder_orders o
             WHERE o.user_id IS NOT NULL
               AND o.side = 'buy'
               AND o.parent_order_id IS NULL
               AND o.origin_flow_run_id IS NOT NULL
               AND (
                 COALESCE(o.filled_qty, 0) > 0
                 OR EXISTS (
                   SELECT 1
                   FROM trade_builder_order_events e
                   WHERE e.builder_order_id = o.id
                     AND e.event_type = 'filled'
                 )
               )
               AND NOT EXISTS (
                 SELECT 1
                 FROM trade_flow_auto_scope_analysis_rows s
                 WHERE s.root_builder_order_id = o.id
               )
             ORDER BY o.created_at ASC, o.id ASC
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.pool())
        .await?;

        Ok(ids)
    }

    pub async fn delete_trade_flow_auto_scope_analysis_rows_for_root(
        &self,
        root_builder_order_id: i64,
    ) -> Result<()> {
        sqlx::query(
            "DELETE FROM trade_flow_auto_scope_analysis_rows
             WHERE root_builder_order_id = $1",
        )
        .bind(root_builder_order_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn upsert_trade_flow_auto_scope_analysis_rows(
        &self,
        rows: &[TradeFlowAutoScopeAnalysisRowInput],
    ) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;
        for row in rows {
            sqlx::query(
                "INSERT INTO trade_flow_auto_scope_analysis_rows
                  (row_key, user_id, definition_id, run_id, root_builder_order_id,
                   exit_builder_order_id, row_type, market_slug, token_id, outcome_label,
                   exit_reason, market_open_at, triggered_at, buy_filled_at, sell_filled_at,
                   open_to_trigger_ms, trigger_to_buy_fill_ms, buy_avg_price, mark_or_sell_price,
                   mark_price_captured_at, row_qty, remaining_qty_after_exit, row_pnl_usdc,
                   updated_at)
                 VALUES
                  ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                   $11, $12, $13, $14, $15, $16, $17, $18, $19,
                   $20, $21, $22, $23, NOW())
                 ON CONFLICT (row_key) DO UPDATE SET
                   user_id = EXCLUDED.user_id,
                   definition_id = EXCLUDED.definition_id,
                   run_id = EXCLUDED.run_id,
                   root_builder_order_id = EXCLUDED.root_builder_order_id,
                   exit_builder_order_id = EXCLUDED.exit_builder_order_id,
                   row_type = EXCLUDED.row_type,
                   market_slug = EXCLUDED.market_slug,
                   token_id = EXCLUDED.token_id,
                   outcome_label = EXCLUDED.outcome_label,
                   exit_reason = EXCLUDED.exit_reason,
                   market_open_at = EXCLUDED.market_open_at,
                   triggered_at = EXCLUDED.triggered_at,
                   buy_filled_at = EXCLUDED.buy_filled_at,
                   sell_filled_at = EXCLUDED.sell_filled_at,
                   open_to_trigger_ms = EXCLUDED.open_to_trigger_ms,
                   trigger_to_buy_fill_ms = EXCLUDED.trigger_to_buy_fill_ms,
                   buy_avg_price = EXCLUDED.buy_avg_price,
                   mark_or_sell_price = EXCLUDED.mark_or_sell_price,
                   mark_price_captured_at = EXCLUDED.mark_price_captured_at,
                   row_qty = EXCLUDED.row_qty,
                   remaining_qty_after_exit = EXCLUDED.remaining_qty_after_exit,
                   row_pnl_usdc = EXCLUDED.row_pnl_usdc,
                   updated_at = NOW()",
            )
            .bind(&row.row_key)
            .bind(row.user_id)
            .bind(row.definition_id)
            .bind(row.run_id)
            .bind(row.root_builder_order_id)
            .bind(row.exit_builder_order_id)
            .bind(&row.row_type)
            .bind(&row.market_slug)
            .bind(&row.token_id)
            .bind(&row.outcome_label)
            .bind(&row.exit_reason)
            .bind(row.market_open_at)
            .bind(row.triggered_at)
            .bind(row.buy_filled_at)
            .bind(row.sell_filled_at)
            .bind(row.open_to_trigger_ms)
            .bind(row.trigger_to_buy_fill_ms)
            .bind(row.buy_avg_price)
            .bind(row.mark_or_sell_price)
            .bind(row.mark_price_captured_at)
            .bind(row.row_qty)
            .bind(row.remaining_qty_after_exit)
            .bind(row.row_pnl_usdc)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn list_trade_builder_order_events_for_orders(
        &self,
        builder_order_ids: &[i64],
    ) -> Result<Vec<TradeBuilderOrderEventRecord>> {
        if builder_order_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            "SELECT builder_order_id, event_type, payload_json, created_at
             FROM trade_builder_order_events
             WHERE builder_order_id = ANY($1::bigint[])
             ORDER BY builder_order_id ASC, created_at ASC, id ASC",
        )
        .bind(builder_order_ids)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeBuilderOrderEventRecord {
                builder_order_id: row.get("builder_order_id"),
                event_type: row.get("event_type"),
                payload_json: row.get("payload_json"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn list_trade_flow_events_for_run_types(
        &self,
        run_id: i64,
        event_types: &[&str],
    ) -> Result<Vec<TradeFlowEventRecord>> {
        if event_types.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            "SELECT run_id, event_type, payload_json, created_at
             FROM trade_flow_events
             WHERE run_id = $1
               AND event_type = ANY($2::text[])
             ORDER BY created_at DESC, id DESC",
        )
        .bind(run_id)
        .bind(event_types)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeFlowEventRecord {
                run_id: row.get("run_id"),
                event_type: row.get("event_type"),
                payload_json: row.get("payload_json"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn list_trade_builder_fill_summaries_by_exchange_order_ids(
        &self,
        exchange_order_ids: &[String],
    ) -> Result<Vec<TradeBuilderExchangeFillSummary>> {
        if exchange_order_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            "SELECT
               o.exchange_order_id,
               COALESCE(SUM(f.size), 0)::double precision AS filled_qty,
               COALESCE(SUM(f.price * f.size), 0)::double precision AS filled_notional_usdc,
               COALESCE(SUM(f.fee), 0)::double precision AS fee_usdc,
               MIN(f.filled_at) AS first_filled_at,
               MAX(f.filled_at) AS last_filled_at
             FROM orders o
             LEFT JOIN fills f ON f.order_id = o.id
             WHERE o.exchange_order_id = ANY($1::text[])
             GROUP BY o.exchange_order_id",
        )
        .bind(exchange_order_ids)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeBuilderExchangeFillSummary {
                exchange_order_id: row.get("exchange_order_id"),
                filled_qty: row.get("filled_qty"),
                filled_notional_usdc: row.get("filled_notional_usdc"),
                fee_usdc: row.get("fee_usdc"),
                first_filled_at: row.get("first_filled_at"),
                last_filled_at: row.get("last_filled_at"),
            })
            .collect())
    }
}
