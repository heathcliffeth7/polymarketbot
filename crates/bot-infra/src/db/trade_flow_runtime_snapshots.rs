use super::*;
use serde_json::json;

fn runtime_snapshot_key(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("")
        .to_string()
}

fn row_to_trade_flow_node_runtime_snapshot_record(
    row: sqlx::postgres::PgRow,
) -> TradeFlowNodeRuntimeSnapshotRecord {
    TradeFlowNodeRuntimeSnapshotRecord {
        run_id: row.get("run_id"),
        definition_id: row.get("definition_id"),
        version_id: row.get("version_id"),
        node_key: row.get("node_key"),
        node_type: row.get("node_type"),
        status: row.get("status"),
        state_kind: row.get("state_kind"),
        market_slug: row.get("market_slug"),
        token_id: row.get("token_id"),
        snapshot_json: row.get("snapshot_json"),
        updated_at: row.get("updated_at"),
    }
}

fn snapshot_string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn resolve_runtime_snapshot_identity(
    input_json: Option<&Value>,
    output_json: Option<&Value>,
) -> (Option<String>, Option<String>) {
    let market_slug = output_json
        .and_then(|value| snapshot_string_field(value, "market_slug"))
        .or_else(|| output_json.and_then(|value| snapshot_string_field(value, "resolved_market_slug")))
        .or_else(|| input_json.and_then(|value| snapshot_string_field(value, "market_slug")))
        .or_else(|| input_json.and_then(|value| snapshot_string_field(value, "marketSlug")));
    let token_id = output_json
        .and_then(|value| snapshot_string_field(value, "token_id"))
        .or_else(|| output_json.and_then(|value| snapshot_string_field(value, "resolved_token_id")))
        .or_else(|| input_json.and_then(|value| snapshot_string_field(value, "token_id")))
        .or_else(|| input_json.and_then(|value| snapshot_string_field(value, "tokenId")));
    (market_slug, token_id)
}

fn build_runtime_snapshot_payload(
    status: &str,
    input_json: Option<&Value>,
    output_json: Option<&Value>,
    error_text: Option<&str>,
) -> Value {
    let mut payload = serde_json::Map::new();
    payload.insert("status".to_string(), json!(status));
    if let Some(input_json) = input_json {
        payload.insert("input".to_string(), input_json.clone());
    }
    if let Some(output_json) = output_json {
        payload.insert("output".to_string(), output_json.clone());
    }
    if let Some(error_text) = error_text {
        payload.insert("error_text".to_string(), json!(error_text));
    }
    Value::Object(payload)
}

impl PostgresRepository {
    pub async fn upsert_trade_flow_node_runtime_snapshot(
        &self,
        input: &TradeFlowNodeRuntimeSnapshotInput,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO trade_flow_node_runtime_snapshots \
              (run_id, definition_id, version_id, node_key, node_type, status, state_kind, \
               market_slug, token_id, market_slug_key, token_id_key, snapshot_json, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW(), NOW()) \
             ON CONFLICT (run_id, node_key, market_slug_key, token_id_key) DO UPDATE SET \
               definition_id = EXCLUDED.definition_id, \
               version_id = EXCLUDED.version_id, \
               node_type = EXCLUDED.node_type, \
               status = EXCLUDED.status, \
               state_kind = EXCLUDED.state_kind, \
               market_slug = EXCLUDED.market_slug, \
               token_id = EXCLUDED.token_id, \
               snapshot_json = EXCLUDED.snapshot_json, \
               updated_at = NOW()",
        )
        .bind(input.run_id)
        .bind(input.definition_id)
        .bind(input.version_id)
        .bind(&input.node_key)
        .bind(&input.node_type)
        .bind(&input.status)
        .bind(&input.state_kind)
        .bind(input.market_slug.as_deref())
        .bind(input.token_id.as_deref())
        .bind(runtime_snapshot_key(input.market_slug.as_deref()))
        .bind(runtime_snapshot_key(input.token_id.as_deref()))
        .bind(&input.snapshot_json)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn list_trade_flow_node_runtime_snapshots(
        &self,
        user_id: i64,
        run_id: i64,
        node_key: Option<&str>,
        node_type: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<TradeFlowNodeRuntimeSnapshotRecord>> {
        let rows = sqlx::query(
            "SELECT s.run_id, s.definition_id, s.version_id, s.node_key, s.node_type, s.status, \
                    s.state_kind, s.market_slug, s.token_id, s.snapshot_json, s.updated_at \
             FROM trade_flow_node_runtime_snapshots s \
             JOIN trade_flow_runs r ON r.id = s.run_id \
             WHERE r.user_id = $1 \
               AND s.run_id = $2 \
               AND ($3::text IS NULL OR s.node_key = $3) \
               AND ($4::text IS NULL OR s.node_type = $4) \
             ORDER BY s.updated_at DESC, s.node_key ASC \
             LIMIT $5 OFFSET $6",
        )
        .bind(user_id)
        .bind(run_id)
        .bind(node_key)
        .bind(node_type)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(row_to_trade_flow_node_runtime_snapshot_record)
            .collect())
    }

    pub async fn count_trade_flow_node_runtime_snapshots(
        &self,
        user_id: i64,
        run_id: i64,
        node_key: Option<&str>,
        node_type: Option<&str>,
    ) -> Result<i64> {
        let total = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint \
             FROM trade_flow_node_runtime_snapshots s \
             JOIN trade_flow_runs r ON r.id = s.run_id \
             WHERE r.user_id = $1 \
               AND s.run_id = $2 \
               AND ($3::text IS NULL OR s.node_key = $3) \
               AND ($4::text IS NULL OR s.node_type = $4)",
        )
        .bind(user_id)
        .bind(run_id)
        .bind(node_key)
        .bind(node_type)
        .fetch_one(self.pool())
        .await?;
        Ok(total)
    }

    pub async fn list_trade_flow_node_runtime_snapshots_for_markets(
        &self,
        run_id: i64,
        node_key: &str,
        market_slugs: &[String],
    ) -> Result<Vec<TradeFlowNodeRuntimeSnapshotRecord>> {
        if market_slugs.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query(
            "SELECT run_id, definition_id, version_id, node_key, node_type, status, state_kind, \
                    market_slug, token_id, snapshot_json, updated_at \
             FROM trade_flow_node_runtime_snapshots \
             WHERE run_id = $1 \
               AND node_key = $2 \
               AND market_slug = ANY($3) \
             ORDER BY updated_at DESC",
        )
        .bind(run_id)
        .bind(node_key)
        .bind(market_slugs)
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(row_to_trade_flow_node_runtime_snapshot_record)
            .collect())
    }

    pub async fn sync_trade_flow_node_runtime_snapshot_for_step(
        &self,
        step_id: i64,
    ) -> Result<()> {
        let row = sqlx::query(
            "SELECT s.run_id, r.definition_id, r.version_id, s.node_key, s.node_type, s.status, \
                    s.input_json, s.output_json, s.error_text \
             FROM trade_flow_run_steps s \
             JOIN trade_flow_runs r ON r.id = s.run_id \
             WHERE s.id = $1 \
             LIMIT 1",
        )
        .bind(step_id)
        .fetch_optional(self.pool())
        .await?;
        let Some(row) = row else {
            return Ok(());
        };

        let node_type: String = row.get("node_type");
        if node_type == "trigger.market_price" {
            return Ok(());
        }

        let status: String = row.get("status");
        let input_json: Option<Value> = row.get("input_json");
        let output_json: Option<Value> = row.get("output_json");
        let error_text: Option<String> = row.get("error_text");
        let (market_slug, token_id) =
            resolve_runtime_snapshot_identity(input_json.as_ref(), output_json.as_ref());
        let snapshot_json = build_runtime_snapshot_payload(
            &status,
            input_json.as_ref(),
            output_json.as_ref(),
            error_text.as_deref(),
        );

        let input = TradeFlowNodeRuntimeSnapshotInput {
            run_id: row.get("run_id"),
            definition_id: row.get("definition_id"),
            version_id: row.get("version_id"),
            node_key: row.get("node_key"),
            node_type,
            status: status.clone(),
            state_kind: status,
            market_slug,
            token_id,
            snapshot_json,
        };
        self.upsert_trade_flow_node_runtime_snapshot(&input).await
    }
}
