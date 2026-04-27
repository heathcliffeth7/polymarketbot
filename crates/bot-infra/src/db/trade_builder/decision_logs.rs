use super::super::*;

impl PostgresRepository {
    pub async fn append_bot_decision_log(&self, input: &BotDecisionLogInput) -> Result<bool> {
        let result = sqlx::query(
            "INSERT INTO bot_decision_logs \
             (event_id, idempotency_key, schema_version, event_type, event_ts, decision_id, \
              sl_event_id, fill_event_id, market_slug, root_order_id, order_id, exchange_order_id, \
              parent_order_id, child_order_id, source_trade_id, flow_run_id, flow_definition_id, \
              pair_session_id, asset, workflow, outcome, outcome_token_id, opposite_token_id, payload) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, \
                     $17, $18, $19, $20, $21, $22, $23, $24) \
             ON CONFLICT DO NOTHING",
        )
        .bind(input.event_id)
        .bind(&input.idempotency_key)
        .bind(input.schema_version)
        .bind(&input.event_type)
        .bind(input.event_ts)
        .bind(&input.decision_id)
        .bind(&input.sl_event_id)
        .bind(&input.fill_event_id)
        .bind(&input.market_slug)
        .bind(&input.root_order_id)
        .bind(&input.order_id)
        .bind(&input.exchange_order_id)
        .bind(&input.parent_order_id)
        .bind(&input.child_order_id)
        .bind(&input.source_trade_id)
        .bind(&input.flow_run_id)
        .bind(&input.flow_definition_id)
        .bind(&input.pair_session_id)
        .bind(&input.asset)
        .bind(&input.workflow)
        .bind(&input.outcome)
        .bind(&input.outcome_token_id)
        .bind(&input.opposite_token_id)
        .bind(&input.payload)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_bot_decision_logs_for_roots(
        &self,
        root_order_ids: &[String],
    ) -> Result<Vec<BotDecisionLogRecord>> {
        if root_order_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            "SELECT event_id, idempotency_key, schema_version, event_type, event_ts, created_at, \
                    decision_id, sl_event_id, fill_event_id, market_slug, root_order_id, order_id, \
                    exchange_order_id, parent_order_id, child_order_id, source_trade_id, flow_run_id, \
                    flow_definition_id, pair_session_id, asset, workflow, outcome, outcome_token_id, \
                    opposite_token_id, payload \
             FROM bot_decision_logs \
             WHERE root_order_id = ANY($1::text[]) \
             ORDER BY root_order_id ASC, event_ts ASC, created_at ASC, id ASC",
        )
        .bind(root_order_ids)
        .fetch_all(self.pool())
        .await?;

        Ok(rows.into_iter().map(map_bot_decision_log_row).collect())
    }
}

fn map_bot_decision_log_row(row: sqlx::postgres::PgRow) -> BotDecisionLogRecord {
    BotDecisionLogRecord {
        event_id: row.get("event_id"),
        idempotency_key: row.get("idempotency_key"),
        schema_version: row.get("schema_version"),
        event_type: row.get("event_type"),
        event_ts: row.get("event_ts"),
        created_at: row.get("created_at"),
        decision_id: row.get("decision_id"),
        sl_event_id: row.get("sl_event_id"),
        fill_event_id: row.get("fill_event_id"),
        market_slug: row.get("market_slug"),
        root_order_id: row.get("root_order_id"),
        order_id: row.get("order_id"),
        exchange_order_id: row.get("exchange_order_id"),
        parent_order_id: row.get("parent_order_id"),
        child_order_id: row.get("child_order_id"),
        source_trade_id: row.get("source_trade_id"),
        flow_run_id: row.get("flow_run_id"),
        flow_definition_id: row.get("flow_definition_id"),
        pair_session_id: row.get("pair_session_id"),
        asset: row.get("asset"),
        workflow: row.get("workflow"),
        outcome: row.get("outcome"),
        outcome_token_id: row.get("outcome_token_id"),
        opposite_token_id: row.get("opposite_token_id"),
        payload: row.get("payload"),
    }
}
