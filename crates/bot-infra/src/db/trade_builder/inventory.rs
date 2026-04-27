use super::super::*;

impl PostgresRepository {
    pub async fn get_trade_builder_buy_inventory_baseline_qty(
        &self,
        parent_builder_order_id: i64,
    ) -> Result<Option<f64>> {
        let row = sqlx::query(
            "SELECT baseline_visible_qty \
             FROM trade_builder_inventory_observations \
             WHERE parent_builder_order_id = $1 \
               AND observation_kind = 'buy_inventory_baseline' \
             ORDER BY created_at DESC \
             LIMIT 1",
        )
        .bind(parent_builder_order_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.and_then(|row| row.get("baseline_visible_qty")))
    }

    pub async fn insert_trade_builder_inventory_observation_if_absent(
        &self,
        observation: &TradeBuilderInventoryObservationInput,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO trade_builder_inventory_observations \
              (parent_builder_order_id, observer_builder_order_id, user_id, market_slug, token_id, outcome_label, exchange_order_id, observation_kind, qty_source, baseline_visible_qty, submitted_dynamic_qty, resolved_fill_qty, expected_fee_qty, expected_net_qty, expected_visible_qty, actual_visible_qty, visible_delta_qty, gap_vs_submit_qty, gap_vs_fill_qty, gap_vs_expected_qty, reference_price, fee_rate_bps, fill_to_inventory_ms, payload_json, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, NOW(), NOW()) \
             ON CONFLICT (parent_builder_order_id, observation_kind) DO NOTHING",
        )
        .bind(observation.parent_builder_order_id)
        .bind(observation.observer_builder_order_id)
        .bind(observation.user_id)
        .bind(&observation.market_slug)
        .bind(&observation.token_id)
        .bind(&observation.outcome_label)
        .bind(&observation.exchange_order_id)
        .bind(&observation.observation_kind)
        .bind(&observation.qty_source)
        .bind(observation.baseline_visible_qty)
        .bind(observation.submitted_dynamic_qty)
        .bind(observation.resolved_fill_qty)
        .bind(observation.expected_fee_qty)
        .bind(observation.expected_net_qty)
        .bind(observation.expected_visible_qty)
        .bind(observation.actual_visible_qty)
        .bind(observation.visible_delta_qty)
        .bind(observation.gap_vs_submit_qty)
        .bind(observation.gap_vs_fill_qty)
        .bind(observation.gap_vs_expected_qty)
        .bind(observation.reference_price)
        .bind(observation.fee_rate_bps)
        .bind(observation.fill_to_inventory_ms)
        .bind(&observation.payload_json)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn upsert_trade_builder_inventory_observation(
        &self,
        observation: &TradeBuilderInventoryObservationInput,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO trade_builder_inventory_observations \
              (parent_builder_order_id, observer_builder_order_id, user_id, market_slug, token_id, outcome_label, exchange_order_id, observation_kind, qty_source, baseline_visible_qty, submitted_dynamic_qty, resolved_fill_qty, expected_fee_qty, expected_net_qty, expected_visible_qty, actual_visible_qty, visible_delta_qty, gap_vs_submit_qty, gap_vs_fill_qty, gap_vs_expected_qty, reference_price, fee_rate_bps, fill_to_inventory_ms, payload_json, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, NOW(), NOW()) \
             ON CONFLICT (parent_builder_order_id, observation_kind) DO UPDATE SET \
               observer_builder_order_id = EXCLUDED.observer_builder_order_id, \
               user_id = EXCLUDED.user_id, \
               market_slug = EXCLUDED.market_slug, \
               token_id = EXCLUDED.token_id, \
               outcome_label = EXCLUDED.outcome_label, \
               exchange_order_id = EXCLUDED.exchange_order_id, \
               qty_source = EXCLUDED.qty_source, \
               baseline_visible_qty = EXCLUDED.baseline_visible_qty, \
               submitted_dynamic_qty = EXCLUDED.submitted_dynamic_qty, \
               resolved_fill_qty = EXCLUDED.resolved_fill_qty, \
               expected_fee_qty = EXCLUDED.expected_fee_qty, \
               expected_net_qty = EXCLUDED.expected_net_qty, \
               expected_visible_qty = EXCLUDED.expected_visible_qty, \
               actual_visible_qty = EXCLUDED.actual_visible_qty, \
               visible_delta_qty = EXCLUDED.visible_delta_qty, \
               gap_vs_submit_qty = EXCLUDED.gap_vs_submit_qty, \
               gap_vs_fill_qty = EXCLUDED.gap_vs_fill_qty, \
               gap_vs_expected_qty = EXCLUDED.gap_vs_expected_qty, \
               reference_price = EXCLUDED.reference_price, \
               fee_rate_bps = EXCLUDED.fee_rate_bps, \
               fill_to_inventory_ms = EXCLUDED.fill_to_inventory_ms, \
               payload_json = EXCLUDED.payload_json, \
               updated_at = NOW()",
        )
        .bind(observation.parent_builder_order_id)
        .bind(observation.observer_builder_order_id)
        .bind(observation.user_id)
        .bind(&observation.market_slug)
        .bind(&observation.token_id)
        .bind(&observation.outcome_label)
        .bind(&observation.exchange_order_id)
        .bind(&observation.observation_kind)
        .bind(&observation.qty_source)
        .bind(observation.baseline_visible_qty)
        .bind(observation.submitted_dynamic_qty)
        .bind(observation.resolved_fill_qty)
        .bind(observation.expected_fee_qty)
        .bind(observation.expected_net_qty)
        .bind(observation.expected_visible_qty)
        .bind(observation.actual_visible_qty)
        .bind(observation.visible_delta_qty)
        .bind(observation.gap_vs_submit_qty)
        .bind(observation.gap_vs_fill_qty)
        .bind(observation.gap_vs_expected_qty)
        .bind(observation.reference_price)
        .bind(observation.fee_rate_bps)
        .bind(observation.fill_to_inventory_ms)
        .bind(&observation.payload_json)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn list_pending_trade_builder_first_visible_inventory_observations(
        &self,
        limit: i64,
    ) -> Result<Vec<PendingTradeBuilderFirstVisibleInventoryObservation>> {
        let rows = sqlx::query(
            "SELECT \
                fill_obs.parent_builder_order_id, \
                fill_obs.observer_builder_order_id, \
                fill_obs.user_id, \
                fill_obs.market_slug, \
                fill_obs.token_id, \
                fill_obs.outcome_label, \
                COALESCE(fill_obs.exchange_order_id, submit.exchange_order_id) AS exchange_order_id, \
                baseline.baseline_visible_qty AS baseline_visible_qty, \
                COALESCE(submit.submitted_dynamic_qty, o.submitted_dynamic_qty) AS submitted_dynamic_qty, \
                fill_obs.resolved_fill_qty AS resolved_fill_qty, \
                COALESCE(submit.reference_price, o.submitted_dynamic_price) AS submit_reference_price, \
                fill_obs.reference_price AS fill_reference_price, \
                fill_obs.qty_source AS fill_qty_source, \
                COALESCE(fill_obs.fee_rate_bps, submit.fee_rate_bps, o.fee_rate_bps, 0) AS fee_rate_bps, \
                fill_obs.created_at AS fill_observed_at \
             FROM trade_builder_inventory_observations fill_obs \
             JOIN trade_builder_orders o \
               ON o.id = fill_obs.parent_builder_order_id \
             LEFT JOIN trade_builder_inventory_observations baseline \
               ON baseline.parent_builder_order_id = fill_obs.parent_builder_order_id \
              AND baseline.observation_kind = 'buy_inventory_baseline' \
             LEFT JOIN trade_builder_inventory_observations submit \
               ON submit.parent_builder_order_id = fill_obs.parent_builder_order_id \
              AND submit.observation_kind = 'buy_submit_dynamic_qty' \
             LEFT JOIN trade_builder_inventory_observations first_visible \
               ON first_visible.parent_builder_order_id = fill_obs.parent_builder_order_id \
              AND first_visible.observation_kind = 'first_visible_inventory' \
             WHERE fill_obs.observation_kind = 'buy_fill_resolution' \
               AND first_visible.id IS NULL \
             ORDER BY fill_obs.updated_at ASC \
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| PendingTradeBuilderFirstVisibleInventoryObservation {
                parent_builder_order_id: row.get("parent_builder_order_id"),
                observer_builder_order_id: row.get("observer_builder_order_id"),
                user_id: row.get("user_id"),
                market_slug: row.get("market_slug"),
                token_id: row.get("token_id"),
                outcome_label: row.get("outcome_label"),
                exchange_order_id: row.get("exchange_order_id"),
                baseline_visible_qty: row.get("baseline_visible_qty"),
                submitted_dynamic_qty: row.get("submitted_dynamic_qty"),
                resolved_fill_qty: row.get("resolved_fill_qty"),
                submit_reference_price: row.get("submit_reference_price"),
                fill_reference_price: row.get("fill_reference_price"),
                fill_qty_source: row.get("fill_qty_source"),
                fee_rate_bps: row.get("fee_rate_bps"),
                fill_observed_at: row.get("fill_observed_at"),
            })
            .collect())
    }
}
