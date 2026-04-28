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
               AND (
                 NOT EXISTS (
                   SELECT 1
                   FROM trade_flow_auto_scope_analysis_rows s
                   WHERE s.root_builder_order_id = o.id
                 )
                 OR NOT EXISTS (
                   SELECT 1
                   FROM trade_flow_auto_scope_trade_diagnostics d
                   WHERE d.root_builder_order_id = o.id
                 )
                 OR EXISTS (
                   SELECT 1
                   FROM trade_flow_auto_scope_analysis_rows s
                   WHERE s.root_builder_order_id = o.id
                     AND s.valuation_kind IS NULL
                 )
                 OR EXISTS (
                   SELECT 1
                   FROM trade_flow_auto_scope_trade_diagnostics d
                   WHERE d.root_builder_order_id = o.id
                     AND COALESCE((d.compact_metrics_json->>'pnl_model_version')::int, 0) < 4
                 )
               )
             ORDER BY o.created_at DESC, o.id DESC
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.pool())
        .await?;

        Ok(ids)
    }

    pub async fn count_trade_builder_filled_roots_for_market_token(
        &self,
        user_id: i64,
        market_slug: &str,
        token_id: &str,
    ) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)
             FROM trade_builder_orders o
             WHERE o.user_id = $1
               AND o.side = 'buy'
               AND o.parent_order_id IS NULL
               AND o.origin_flow_run_id IS NOT NULL
               AND LOWER(o.market_slug) = LOWER($2)
               AND o.token_id = $3
               AND (
                 COALESCE(o.filled_qty, 0) > 0
                 OR EXISTS (
                   SELECT 1
                   FROM trade_builder_order_events e
                   WHERE e.builder_order_id = o.id
                     AND e.event_type = 'filled'
                 )
               )",
        )
        .bind(user_id)
        .bind(market_slug)
        .bind(token_id)
        .fetch_one(self.pool())
        .await?;

        Ok(count)
    }

    pub async fn count_trade_builder_filled_roots_for_market(
        &self,
        user_id: i64,
        market_slug: &str,
    ) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)
             FROM trade_builder_orders o
             WHERE o.user_id = $1
               AND o.side = 'buy'
               AND o.parent_order_id IS NULL
               AND o.origin_flow_run_id IS NOT NULL
               AND LOWER(o.market_slug) = LOWER($2)
               AND (
                 COALESCE(o.filled_qty, 0) > 0
                 OR EXISTS (
                   SELECT 1
                   FROM trade_builder_order_events e
                   WHERE e.builder_order_id = o.id
                     AND e.event_type = 'filled'
                 )
               )",
        )
        .bind(user_id)
        .bind(market_slug)
        .fetch_one(self.pool())
        .await?;

        Ok(count)
    }

    pub async fn delete_trade_flow_auto_scope_analysis_rows_for_root(
        &self,
        root_builder_order_id: i64,
    ) -> Result<()> {
        sqlx::query(
            "DELETE FROM trade_flow_auto_scope_trade_diagnostics
             WHERE root_builder_order_id = $1",
        )
        .bind(root_builder_order_id)
        .execute(self.pool())
        .await?;

        sqlx::query(
            "DELETE FROM trade_flow_auto_scope_analysis_rows
             WHERE root_builder_order_id = $1",
        )
        .bind(root_builder_order_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn upsert_trade_flow_auto_scope_trade_diagnostic(
        &self,
        input: &TradeFlowAutoScopeTradeDiagnosticInput,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO trade_flow_auto_scope_trade_diagnostics
              (root_builder_order_id, user_id, definition_id, run_id, market_slug, token_id,
               outcome_label, total_pnl_usdc, realized_pnl_usdc, open_pnl_usdc, pnl_pct,
               fee_drag_usdc, cost_basis_usdc, net_value_usdc, entry_trigger_price,
               entry_submit_price, entry_fill_price, entry_reference_price,
               entry_slippage_usdc, entry_quality_score, exit_reason, exit_price,
               best_price_during_hold, worst_price_during_hold, max_favorable_usdc,
               max_adverse_usdc, gave_back_usdc, exit_quality_score, open_to_trigger_ms,
               trigger_to_buy_fill_ms, trigger_to_submit_ms, submit_to_fill_ms, hold_ms,
               snapshot_age_ms, runtime_price_fetch_ms, guard_eval_ms, place_http_ms,
               primary_diagnosis_code, secondary_diagnosis_code, diagnosis_label,
               diagnosis_detail, data_quality_flags, compact_metrics_json, updated_at)
             VALUES
              ($1, $2, $3, $4, $5, $6,
               $7, $8, $9, $10, $11,
               $12, $13, $14, $15,
               $16, $17, $18,
               $19, $20, $21, $22,
               $23, $24, $25,
               $26, $27, $28, $29,
               $30, $31, $32, $33,
               $34, $35, $36, $37,
               $38, $39, $40,
               $41, $42, $43, NOW())
             ON CONFLICT (root_builder_order_id) DO UPDATE SET
               user_id = EXCLUDED.user_id,
               definition_id = EXCLUDED.definition_id,
               run_id = EXCLUDED.run_id,
               market_slug = EXCLUDED.market_slug,
               token_id = EXCLUDED.token_id,
               outcome_label = EXCLUDED.outcome_label,
               total_pnl_usdc = EXCLUDED.total_pnl_usdc,
               realized_pnl_usdc = EXCLUDED.realized_pnl_usdc,
               open_pnl_usdc = EXCLUDED.open_pnl_usdc,
               pnl_pct = EXCLUDED.pnl_pct,
               fee_drag_usdc = EXCLUDED.fee_drag_usdc,
               cost_basis_usdc = EXCLUDED.cost_basis_usdc,
               net_value_usdc = EXCLUDED.net_value_usdc,
               entry_trigger_price = EXCLUDED.entry_trigger_price,
               entry_submit_price = EXCLUDED.entry_submit_price,
               entry_fill_price = EXCLUDED.entry_fill_price,
               entry_reference_price = EXCLUDED.entry_reference_price,
               entry_slippage_usdc = EXCLUDED.entry_slippage_usdc,
               entry_quality_score = EXCLUDED.entry_quality_score,
               exit_reason = EXCLUDED.exit_reason,
               exit_price = EXCLUDED.exit_price,
               best_price_during_hold = EXCLUDED.best_price_during_hold,
               worst_price_during_hold = EXCLUDED.worst_price_during_hold,
               max_favorable_usdc = EXCLUDED.max_favorable_usdc,
               max_adverse_usdc = EXCLUDED.max_adverse_usdc,
               gave_back_usdc = EXCLUDED.gave_back_usdc,
               exit_quality_score = EXCLUDED.exit_quality_score,
               open_to_trigger_ms = EXCLUDED.open_to_trigger_ms,
               trigger_to_buy_fill_ms = EXCLUDED.trigger_to_buy_fill_ms,
               trigger_to_submit_ms = EXCLUDED.trigger_to_submit_ms,
               submit_to_fill_ms = EXCLUDED.submit_to_fill_ms,
               hold_ms = EXCLUDED.hold_ms,
               snapshot_age_ms = EXCLUDED.snapshot_age_ms,
               runtime_price_fetch_ms = EXCLUDED.runtime_price_fetch_ms,
               guard_eval_ms = EXCLUDED.guard_eval_ms,
               place_http_ms = EXCLUDED.place_http_ms,
               primary_diagnosis_code = EXCLUDED.primary_diagnosis_code,
               secondary_diagnosis_code = EXCLUDED.secondary_diagnosis_code,
               diagnosis_label = EXCLUDED.diagnosis_label,
               diagnosis_detail = EXCLUDED.diagnosis_detail,
               data_quality_flags = EXCLUDED.data_quality_flags,
               compact_metrics_json = EXCLUDED.compact_metrics_json,
               updated_at = NOW()",
        )
        .bind(input.root_builder_order_id)
        .bind(input.user_id)
        .bind(input.definition_id)
        .bind(input.run_id)
        .bind(&input.market_slug)
        .bind(&input.token_id)
        .bind(&input.outcome_label)
        .bind(input.total_pnl_usdc)
        .bind(input.realized_pnl_usdc)
        .bind(input.open_pnl_usdc)
        .bind(input.pnl_pct)
        .bind(input.fee_drag_usdc)
        .bind(input.cost_basis_usdc)
        .bind(input.net_value_usdc)
        .bind(input.entry_trigger_price)
        .bind(input.entry_submit_price)
        .bind(input.entry_fill_price)
        .bind(input.entry_reference_price)
        .bind(input.entry_slippage_usdc)
        .bind(input.entry_quality_score)
        .bind(&input.exit_reason)
        .bind(input.exit_price)
        .bind(input.best_price_during_hold)
        .bind(input.worst_price_during_hold)
        .bind(input.max_favorable_usdc)
        .bind(input.max_adverse_usdc)
        .bind(input.gave_back_usdc)
        .bind(input.exit_quality_score)
        .bind(input.open_to_trigger_ms)
        .bind(input.trigger_to_buy_fill_ms)
        .bind(input.trigger_to_submit_ms)
        .bind(input.submit_to_fill_ms)
        .bind(input.hold_ms)
        .bind(input.snapshot_age_ms)
        .bind(input.runtime_price_fetch_ms)
        .bind(input.guard_eval_ms)
        .bind(input.place_http_ms)
        .bind(&input.primary_diagnosis_code)
        .bind(&input.secondary_diagnosis_code)
        .bind(&input.diagnosis_label)
        .bind(&input.diagnosis_detail)
        .bind(&input.data_quality_flags)
        .bind(&input.compact_metrics_json)
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
                   buy_notional_usdc, buy_fee_usdc, cost_basis_usdc, sell_notional_usdc,
                   sell_fee_usdc, mark_value_usdc, net_value_usdc, pnl_pct, valuation_kind,
                   updated_at)
                 VALUES
                  ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                   $11, $12, $13, $14, $15, $16, $17, $18, $19,
                   $20, $21, $22, $23, $24, $25, $26, $27, $28,
                   $29, $30, $31, $32, NOW())
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
                   buy_notional_usdc = EXCLUDED.buy_notional_usdc,
                   buy_fee_usdc = EXCLUDED.buy_fee_usdc,
                   cost_basis_usdc = EXCLUDED.cost_basis_usdc,
                   sell_notional_usdc = EXCLUDED.sell_notional_usdc,
                   sell_fee_usdc = EXCLUDED.sell_fee_usdc,
                   mark_value_usdc = EXCLUDED.mark_value_usdc,
                   net_value_usdc = EXCLUDED.net_value_usdc,
                   pnl_pct = EXCLUDED.pnl_pct,
                   valuation_kind = EXCLUDED.valuation_kind,
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
            .bind(row.buy_notional_usdc)
            .bind(row.buy_fee_usdc)
            .bind(row.cost_basis_usdc)
            .bind(row.sell_notional_usdc)
            .bind(row.sell_fee_usdc)
            .bind(row.mark_value_usdc)
            .bind(row.net_value_usdc)
            .bind(row.pnl_pct)
            .bind(&row.valuation_kind)
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
