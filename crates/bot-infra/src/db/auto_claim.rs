use super::*;

impl PostgresRepository {
    pub async fn list_auto_claim_candidate_user_ids(&self) -> Result<Vec<i64>> {
        let user_ids = sqlx::query_scalar::<_, i64>(
            "SELECT DISTINCT us.user_id \
             FROM user_settings us \
             JOIN app_users u ON u.id = us.user_id \
             WHERE us.config_name = 'claim' \
               AND LOWER(COALESCE(us.payload_json ->> 'enabled', 'false')) IN ('true', '1', 'yes', 'on') \
             ORDER BY us.user_id ASC",
        )
        .fetch_all(self.pool())
        .await?;

        Ok(user_ids)
    }

    pub async fn upsert_auto_claim_job(
        &self,
        owner_address: &str,
        market_slug: Option<&str>,
        condition_id: &str,
        max_attempts: i32,
    ) -> Result<bool> {
        let inserted = sqlx::query_scalar::<_, bool>(
            "INSERT INTO auto_claim_jobs \
              (owner_address, market_slug, condition_id, status, attempts, max_attempts, next_attempt_at, tx_hash, last_error, claimed_at, submitted_at, last_seen_redeemable_at, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, 'pending', 0, $4, NOW(), NULL, NULL, NULL, NULL, NOW(), NOW(), NOW()) \
             ON CONFLICT (owner_address, condition_id) DO UPDATE SET \
               market_slug = COALESCE(EXCLUDED.market_slug, auto_claim_jobs.market_slug), \
               max_attempts = GREATEST(auto_claim_jobs.max_attempts, EXCLUDED.max_attempts), \
               last_seen_redeemable_at = NOW(), \
               updated_at = NOW(), \
               status = CASE \
                 WHEN auto_claim_jobs.status IN ('claimed', 'processing', 'submitted') THEN auto_claim_jobs.status \
                 ELSE 'pending' \
               END, \
               attempts = CASE \
                 WHEN auto_claim_jobs.status IN ('claimed', 'processing', 'submitted') THEN auto_claim_jobs.attempts \
                 ELSE 0 \
               END, \
               next_attempt_at = CASE \
                 WHEN auto_claim_jobs.status IN ('claimed', 'processing', 'submitted') THEN auto_claim_jobs.next_attempt_at \
                 ELSE NOW() \
               END, \
               last_error = CASE \
                 WHEN auto_claim_jobs.status IN ('claimed', 'processing', 'submitted') THEN auto_claim_jobs.last_error \
                 ELSE NULL \
               END \
             RETURNING (xmax = 0)",
        )
        .bind(owner_address)
        .bind(market_slug)
        .bind(condition_id)
        .bind(max_attempts)
        .fetch_one(self.pool())
        .await?;

        Ok(inserted)
    }

    pub async fn list_auto_claim_jobs_for_processing(
        &self,
        limit: i64,
    ) -> Result<Vec<AutoClaimJob>> {
        let rows = sqlx::query(
            "SELECT id, owner_address, market_slug, condition_id, status, attempts, max_attempts, next_attempt_at, tx_hash, last_error, claimed_at, submitted_at, last_seen_redeemable_at, created_at, updated_at \
             FROM auto_claim_jobs \
             WHERE status IN ('pending', 'retry') AND next_attempt_at <= NOW() \
             ORDER BY next_attempt_at ASC, id ASC \
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| AutoClaimJob {
                id: row.get("id"),
                owner_address: row.get("owner_address"),
                market_slug: row.get("market_slug"),
                condition_id: row.get("condition_id"),
                status: row.get("status"),
                attempts: row.get("attempts"),
                max_attempts: row.get("max_attempts"),
                next_attempt_at: row.get("next_attempt_at"),
                tx_hash: row.get("tx_hash"),
                last_error: row.get("last_error"),
                claimed_at: row.get("claimed_at"),
                submitted_at: row.get("submitted_at"),
                last_seen_redeemable_at: row.get("last_seen_redeemable_at"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn list_auto_claim_jobs_for_receipt_check(
        &self,
        limit: i64,
    ) -> Result<Vec<AutoClaimJob>> {
        let rows = sqlx::query(
            "SELECT id, owner_address, market_slug, condition_id, status, attempts, max_attempts, next_attempt_at, tx_hash, last_error, claimed_at, submitted_at, last_seen_redeemable_at, created_at, updated_at \
             FROM auto_claim_jobs \
             WHERE status = 'submitted' \
             ORDER BY submitted_at ASC NULLS FIRST, id ASC \
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| AutoClaimJob {
                id: row.get("id"),
                owner_address: row.get("owner_address"),
                market_slug: row.get("market_slug"),
                condition_id: row.get("condition_id"),
                status: row.get("status"),
                attempts: row.get("attempts"),
                max_attempts: row.get("max_attempts"),
                next_attempt_at: row.get("next_attempt_at"),
                tx_hash: row.get("tx_hash"),
                last_error: row.get("last_error"),
                claimed_at: row.get("claimed_at"),
                submitted_at: row.get("submitted_at"),
                last_seen_redeemable_at: row.get("last_seen_redeemable_at"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn mark_auto_claim_job_processing(&self, job_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE auto_claim_jobs \
             SET status = 'processing', updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(job_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn mark_auto_claim_job_submitted(&self, job_id: i64, tx_hash: &str) -> Result<()> {
        sqlx::query(
            "UPDATE auto_claim_jobs \
             SET status = 'submitted', tx_hash = $2, submitted_at = NOW(), last_error = NULL, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(job_id)
        .bind(tx_hash)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn mark_auto_claim_job_receipt_confirmed(&self, job_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE auto_claim_jobs \
             SET status = 'claimed', claimed_at = NOW(), last_error = NULL, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(job_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn mark_auto_claim_job_retry_or_fail(
        &self,
        job_id: i64,
        last_error: &str,
        retry_backoff_ms: i64,
    ) -> Result<String> {
        let status = sqlx::query_scalar::<_, String>(
            "UPDATE auto_claim_jobs \
             SET attempts = attempts + 1, \
                 status = CASE WHEN attempts + 1 >= max_attempts THEN 'failed' ELSE 'retry' END, \
                 next_attempt_at = CASE \
                   WHEN attempts + 1 >= max_attempts THEN NOW() \
                   ELSE NOW() + ($3 * INTERVAL '1 millisecond') \
                 END, \
                 last_error = $2, \
                 submitted_at = NULL, \
                 updated_at = NOW() \
             WHERE id = $1 \
             RETURNING status",
        )
        .bind(job_id)
        .bind(last_error)
        .bind(retry_backoff_ms)
        .fetch_one(self.pool())
        .await?;
        Ok(status)
    }

    pub async fn mark_auto_claim_job_failed(&self, job_id: i64, last_error: &str) -> Result<()> {
        sqlx::query(
            "UPDATE auto_claim_jobs \
             SET attempts = attempts + 1, \
                 status = 'failed', \
                 next_attempt_at = NOW(), \
                 tx_hash = NULL, \
                 last_error = $2, \
                 submitted_at = NULL, \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(job_id)
        .bind(last_error)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn count_auto_claim_jobs_submitted(&self) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM auto_claim_jobs WHERE status = 'submitted'",
        )
        .fetch_one(self.pool())
        .await?;
        Ok(count)
    }

    pub async fn recover_stale_processing_auto_claim_jobs(
        &self,
        stale_minutes: i64,
    ) -> Result<i64> {
        let rows = sqlx::query_scalar::<_, i64>(
            "UPDATE auto_claim_jobs \
             SET status = 'retry', next_attempt_at = NOW(), updated_at = NOW() \
             WHERE status = 'processing' \
               AND updated_at < NOW() - ($1 * INTERVAL '1 minute') \
             RETURNING id",
        )
        .bind(stale_minutes)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.len() as i64)
    }

    pub async fn append_auto_claim_event(
        &self,
        job_id: i64,
        event_type: &str,
        payload_json: &Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO auto_claim_events (job_id, event_type, payload_json, created_at) \
             VALUES ($1, $2, $3, NOW())",
        )
        .bind(job_id)
        .bind(event_type)
        .bind(payload_json)
        .execute(self.pool())
        .await?;
        Ok(())
    }
}
