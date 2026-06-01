use super::super::*;

impl PostgresRepository {
    pub async fn upsert_trade_builder_order_node_snapshot(
        &self,
        input: &TradeBuilderOrderNodeSnapshotInput,
        config_version: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO trade_builder_order_node_snapshots \
             (order_id, root_order_id, flow_run_id, flow_definition_id, flow_version_id, \
              node_key, node_type, node_config_hash, snapshot_json, config_version, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW(), NOW()) \
             ON CONFLICT (order_id) DO UPDATE SET \
               root_order_id = EXCLUDED.root_order_id, \
               flow_run_id = EXCLUDED.flow_run_id, \
               flow_definition_id = EXCLUDED.flow_definition_id, \
               flow_version_id = EXCLUDED.flow_version_id, \
               node_key = EXCLUDED.node_key, \
               node_type = EXCLUDED.node_type, \
               node_config_hash = EXCLUDED.node_config_hash, \
               snapshot_json = EXCLUDED.snapshot_json, \
               config_version = EXCLUDED.config_version, \
               updated_at = NOW()",
        )
        .bind(input.order_id)
        .bind(input.root_order_id)
        .bind(input.flow_run_id)
        .bind(input.flow_definition_id)
        .bind(input.flow_version_id)
        .bind(&input.node_key)
        .bind(&input.node_type)
        .bind(&input.node_config_hash)
        .bind(&input.snapshot_json)
        .bind(config_version)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn get_trade_builder_order_node_snapshot(
        &self,
        order_id: i64,
    ) -> Result<Option<TradeBuilderOrderNodeSnapshotRecord>> {
        let row = sqlx::query(
            "SELECT order_id, root_order_id, flow_run_id, flow_definition_id, flow_version_id, \
                    node_key, node_type, node_config_hash, snapshot_json, config_version, updated_at \
             FROM trade_builder_order_node_snapshots \
             WHERE order_id = $1 \
             LIMIT 1",
        )
        .bind(order_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(map_trade_builder_order_node_snapshot_row))
    }
}

fn map_trade_builder_order_node_snapshot_row(
    row: sqlx::postgres::PgRow,
) -> TradeBuilderOrderNodeSnapshotRecord {
    TradeBuilderOrderNodeSnapshotRecord {
        order_id: row.get("order_id"),
        root_order_id: row.get("root_order_id"),
        flow_run_id: row.get("flow_run_id"),
        flow_definition_id: row.get("flow_definition_id"),
        flow_version_id: row.get("flow_version_id"),
        node_key: row.get("node_key"),
        node_type: row.get("node_type"),
        node_config_hash: row.get("node_config_hash"),
        snapshot_json: row.get("snapshot_json"),
        config_version: row.get("config_version"),
        updated_at: row.get("updated_at"),
    }
}
