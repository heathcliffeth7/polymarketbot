use super::*;

impl PostgresRepository {
    pub async fn record_run_start(&self, mode: &str, version: &str) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO bot_runs (mode, version, started_at) VALUES ($1, $2, NOW()) RETURNING id",
        )
        .bind(mode)
        .bind(version)
        .fetch_one(self.pool())
        .await?;
        Ok(id)
    }

    pub async fn record_run_stop(&self, run_id: i64, reason: &str) -> Result<()> {
        sqlx::query("UPDATE bot_runs SET stopped_at = NOW(), reason = $2 WHERE id = $1")
            .bind(run_id)
            .bind(reason)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn store_config_snapshot(
        &self,
        run_id: i64,
        config_hash: &str,
        payload: &Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO config_snapshots (run_id, config_hash, payload_json, created_at) VALUES ($1, $2, $3, NOW())",
        )
        .bind(run_id)
        .bind(config_hash)
        .bind(payload)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn upsert_market_cycle(&self, cycle: &MarketCycleId) -> Result<i64> {
        let starts_at = cycle.start_time().unwrap_or_else(Utc::now);
        let ends_at = starts_at + chrono::Duration::seconds(300);
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO markets (market_slug, starts_at, ends_at, status) VALUES ($1, $2, $3, 'open') \
             ON CONFLICT (market_slug) DO UPDATE SET status = EXCLUDED.status RETURNING id",
        )
        .bind(cycle.to_string())
        .bind(starts_at)
        .bind(ends_at)
        .fetch_one(self.pool())
        .await?;
        Ok(id)
    }

    pub async fn upsert_market_by_slug(
        &self,
        market_slug: &str,
        starts_at: DateTime<Utc>,
        ends_at: DateTime<Utc>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO markets (market_slug, starts_at, ends_at, status) VALUES ($1, $2, $3, 'open') \
             ON CONFLICT (market_slug) DO UPDATE SET \
               starts_at = COALESCE(markets.starts_at, EXCLUDED.starts_at), \
               ends_at = COALESCE(markets.ends_at, EXCLUDED.ends_at), \
               status = CASE WHEN markets.status = 'settled' THEN markets.status ELSE EXCLUDED.status END \
             RETURNING id",
        )
        .bind(market_slug)
        .bind(starts_at)
        .bind(ends_at)
        .fetch_one(self.pool())
        .await?;
        Ok(id)
    }
}
