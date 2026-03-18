use super::*;

impl PostgresRepository {
    pub async fn list_running_trade_flow_market_peers(
        &self,
        user_id: i64,
        market_slug: &str,
        exclude_run_id: i64,
    ) -> Result<Vec<RunningTradeFlowMarketPeer>> {
        let rows = sqlx::query(
            "SELECT r.id AS run_id, r.definition_id, d.name AS definition_name, \
                    CASE \
                        WHEN COALESCE(r.context_json #>> '{flowContext,sourceTradeId}', '') ~ '^[0-9]+$' \
                            THEN (r.context_json #>> '{flowContext,sourceTradeId}')::BIGINT \
                        ELSE NULL \
                    END AS source_trade_id \
             FROM trade_flow_runs r \
             JOIN trade_flow_definitions d ON d.id = r.definition_id \
             WHERE r.user_id = $1 \
               AND r.status = 'running' \
               AND r.id <> $2 \
               AND COALESCE(r.context_json #>> '{flowContext,marketSlug}', '') = $3 \
             ORDER BY r.created_at DESC \
             LIMIT 50",
        )
        .bind(user_id)
        .bind(exclude_run_id)
        .bind(market_slug)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| RunningTradeFlowMarketPeer {
                run_id: row.get("run_id"),
                definition_id: row.get("definition_id"),
                definition_name: row.get("definition_name"),
                source_trade_id: row.get("source_trade_id"),
            })
            .collect())
    }

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
}
