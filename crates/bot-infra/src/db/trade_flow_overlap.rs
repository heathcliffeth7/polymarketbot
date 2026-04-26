use super::*;

impl PostgresRepository {
    pub async fn list_active_trade_flow_run_market_orders(
        &self,
        run_id: i64,
        market_slug: &str,
    ) -> Result<Vec<ActiveTradeFlowRunOrderPeer>> {
        let rows = sqlx::query(
            "SELECT id AS builder_order_id, trade_id AS source_trade_id, origin_flow_node_key \
             FROM trade_builder_orders \
             WHERE origin_flow_run_id = $1 \
               AND market_slug = $2 \
               AND status IN ('pending', 'armed', 'triggered', 'open', 'partially_filled') \
             ORDER BY created_at DESC \
             LIMIT 50",
        )
        .bind(run_id)
        .bind(market_slug)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| ActiveTradeFlowRunOrderPeer {
                builder_order_id: row.get("builder_order_id"),
                source_trade_id: row.get("source_trade_id"),
                origin_flow_node_key: row.get("origin_flow_node_key"),
            })
            .collect())
    }

    pub async fn has_trade_flow_run_builder_order_for_node_market_token(
        &self,
        run_id: i64,
        node_key: &str,
        market_slug: &str,
        token_id: &str,
    ) -> Result<bool> {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
               SELECT 1
               FROM trade_builder_orders
               WHERE origin_flow_run_id = $1
                 AND origin_flow_node_key = $2
                 AND market_slug = $3
                 AND token_id = $4
               LIMIT 1
             )",
        )
        .bind(run_id)
        .bind(node_key)
        .bind(market_slug)
        .bind(token_id)
        .fetch_one(self.pool())
        .await?;

        Ok(exists)
    }
}
