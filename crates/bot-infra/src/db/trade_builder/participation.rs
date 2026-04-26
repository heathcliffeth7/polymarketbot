use super::super::*;

impl PostgresRepository {
    pub async fn latest_trade_builder_flow_entry_fill_at(
        &self,
        flow_definition_id: i64,
    ) -> Result<Option<DateTime<Utc>>> {
        let filled_at = sqlx::query_scalar::<_, Option<DateTime<Utc>>>(
            "SELECT MAX(e.created_at)
             FROM trade_builder_order_events e
             JOIN trade_builder_orders o ON o.id = e.builder_order_id
             WHERE o.origin_flow_definition_id = $1
               AND o.parent_order_id IS NULL
               AND o.side = 'buy'
               AND e.event_type = 'filled'",
        )
        .bind(flow_definition_id)
        .fetch_one(self.pool())
        .await?;

        Ok(filled_at)
    }
}
