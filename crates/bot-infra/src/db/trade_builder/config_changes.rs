use super::super::*;

impl PostgresRepository {
    pub async fn insert_config_change_log(&self, input: &ConfigChangeLogInput) -> Result<i64> {
        let row = sqlx::query(
            "INSERT INTO config_change_log \
             (config_version, changed_by, change_reason, changed_fields, full_config_snapshot) \
             VALUES ($1, $2, $3, $4, $5) \
             RETURNING id",
        )
        .bind(&input.config_version)
        .bind(&input.changed_by)
        .bind(&input.change_reason)
        .bind(&input.changed_fields)
        .bind(&input.full_config_snapshot)
        .fetch_one(self.pool())
        .await?;

        Ok(row.get("id"))
    }

    pub async fn get_active_config_version(&self) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT config_version \
             FROM config_change_log \
             ORDER BY created_at DESC \
             LIMIT 1",
        )
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|r| r.get("config_version")))
    }

    pub async fn list_config_changes_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<ConfigChangeLogRecord>> {
        let rows = sqlx::query(
            "SELECT id, config_version, changed_by, change_reason, \
                    changed_fields, full_config_snapshot, created_at \
             FROM config_change_log \
             WHERE created_at >= $1 \
             ORDER BY created_at ASC",
        )
        .bind(since)
        .fetch_all(self.pool())
        .await?;

        Ok(rows.into_iter().map(map_config_change_log_row).collect())
    }

    pub async fn get_config_change_by_version(
        &self,
        version: &str,
    ) -> Result<Option<ConfigChangeLogRecord>> {
        let row = sqlx::query(
            "SELECT id, config_version, changed_by, change_reason, \
                    changed_fields, full_config_snapshot, created_at \
             FROM config_change_log \
             WHERE config_version = $1 \
             LIMIT 1",
        )
        .bind(version)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(map_config_change_log_row))
    }
}

fn map_config_change_log_row(row: sqlx::postgres::PgRow) -> ConfigChangeLogRecord {
    ConfigChangeLogRecord {
        id: row.get("id"),
        config_version: row.get("config_version"),
        changed_by: row.get("changed_by"),
        change_reason: row.get("change_reason"),
        changed_fields: row.get("changed_fields"),
        full_config_snapshot: row.get("full_config_snapshot"),
        created_at: row.get("created_at"),
    }
}
