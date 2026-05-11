use super::*;

impl PostgresRepository {
    pub async fn list_completed_place_order_blocked_steps_for_nodes_market_token(
        &self,
        run_id: i64,
        action_node_keys: &[String],
        market_slug: &str,
        token_id: &str,
    ) -> Result<Vec<TradeFlowRunStep>> {
        if action_node_keys.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            "SELECT id, run_id, node_key, node_type, status, attempt, input_json, output_json, \
                    error_text, started_at, ended_at, available_at, parent_step_id, \
                    idempotency_key, created_at
             FROM trade_flow_run_steps
             WHERE run_id = $1
               AND node_key = ANY($2::text[])
               AND node_type = 'action.place_order'
               AND status = 'completed'
               AND output_json IS NOT NULL
               AND output_json->>'blocked' = 'true'
               AND (
                    output_json->>'market_slug' = $3
                 OR output_json->>'trigger_node_market_slug' = $3
                 OR output_json #>> '{primary_selection,trigger_node_market_slug}' = $3
               )
               AND (
                    output_json->>'token_id' = $4
                 OR output_json #>> '{no_candidate_guard,token_id}' = $4
                 OR output_json #>> '{yes_candidate_guard,token_id}' = $4
                 OR output_json #>> '{primary_selection,no_candidate_guard,token_id}' = $4
                 OR output_json #>> '{primary_selection,yes_candidate_guard,token_id}' = $4
               )
             ORDER BY ended_at ASC NULLS LAST, created_at ASC, id ASC
             LIMIT 250",
        )
        .bind(run_id)
        .bind(action_node_keys)
        .bind(market_slug)
        .bind(token_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeFlowRunStep {
                id: row.get("id"),
                run_id: row.get("run_id"),
                node_key: row.get("node_key"),
                node_type: row.get("node_type"),
                status: row.get("status"),
                attempt: row.get("attempt"),
                input_json: row.get("input_json"),
                output_json: row.get("output_json"),
                error_text: row.get("error_text"),
                started_at: row.get("started_at"),
                ended_at: row.get("ended_at"),
                available_at: row.get("available_at"),
                parent_step_id: row.get("parent_step_id"),
                idempotency_key: row.get("idempotency_key"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn list_failed_place_order_steps_for_nodes_market_token(
        &self,
        run_id: i64,
        action_node_keys: &[String],
        market_slug: &str,
        token_id: &str,
    ) -> Result<Vec<TradeFlowRunStep>> {
        if action_node_keys.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            "SELECT id, run_id, node_key, node_type, status, attempt, input_json, output_json, \
                    error_text, started_at, ended_at, available_at, parent_step_id, \
                    idempotency_key, created_at
             FROM trade_flow_run_steps
             WHERE run_id = $1
               AND node_key = ANY($2::text[])
               AND node_type = 'action.place_order'
               AND status = 'failed'
               AND (
                    input_json->>'market_slug' = $3
                 OR input_json->>'trigger_node_market_slug' = $3
                 OR output_json->>'market_slug' = $3
                 OR output_json->>'trigger_node_market_slug' = $3
               )
               AND (
                    input_json->>'token_id' = $4
                 OR input_json->>'yesTokenId' = $4
                 OR input_json->>'noTokenId' = $4
                 OR input_json->>'yes_token_id' = $4
                 OR input_json->>'no_token_id' = $4
                 OR output_json->>'token_id' = $4
               )
             ORDER BY ended_at ASC NULLS LAST, created_at ASC, id ASC
             LIMIT 250",
        )
        .bind(run_id)
        .bind(action_node_keys)
        .bind(market_slug)
        .bind(token_id)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeFlowRunStep {
                id: row.get("id"),
                run_id: row.get("run_id"),
                node_key: row.get("node_key"),
                node_type: row.get("node_type"),
                status: row.get("status"),
                attempt: row.get("attempt"),
                input_json: row.get("input_json"),
                output_json: row.get("output_json"),
                error_text: row.get("error_text"),
                started_at: row.get("started_at"),
                ended_at: row.get("ended_at"),
                available_at: row.get("available_at"),
                parent_step_id: row.get("parent_step_id"),
                idempotency_key: row.get("idempotency_key"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn find_latest_completed_place_order_blocked_output_for_nodes_market_token(
        &self,
        run_id: i64,
        action_node_keys: &[String],
        market_slug: &str,
        token_id: &str,
    ) -> Result<Option<Value>> {
        if action_node_keys.is_empty() {
            return Ok(None);
        }

        let row = sqlx::query(
            "SELECT output_json
             FROM trade_flow_run_steps
             WHERE run_id = $1
               AND node_key = ANY($2::text[])
               AND node_type = 'action.place_order'
               AND status = 'completed'
               AND output_json IS NOT NULL
               AND output_json->>'blocked' = 'true'
               AND (
                    output_json->>'market_slug' = $3
                 OR output_json->>'trigger_node_market_slug' = $3
                 OR output_json #>> '{primary_selection,trigger_node_market_slug}' = $3
               )
               AND (
                    output_json->>'token_id' = $4
                 OR output_json #>> '{no_candidate_guard,token_id}' = $4
                 OR output_json #>> '{yes_candidate_guard,token_id}' = $4
                 OR output_json #>> '{primary_selection,no_candidate_guard,token_id}' = $4
                 OR output_json #>> '{primary_selection,yes_candidate_guard,token_id}' = $4
               )
             ORDER BY ended_at DESC NULLS LAST, created_at DESC, id DESC
             LIMIT 1",
        )
        .bind(run_id)
        .bind(action_node_keys)
        .bind(market_slug)
        .bind(token_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.and_then(|row| {
            row.try_get::<Option<Value>, _>("output_json")
                .ok()
                .flatten()
        }))
    }
}
