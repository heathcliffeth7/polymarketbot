use super::*;

impl PostgresRepository {
    pub async fn record_risk_event(
        &self,
        trade_id: Option<i64>,
        event_type: &str,
        decision: &str,
        details: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO risk_events (trade_id, event_type, decision, details, created_at) VALUES ($1, $2, $3, $4, NOW())",
        )
        .bind(trade_id)
        .bind(event_type)
        .bind(decision)
        .bind(details)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn open_order_count(&self) -> Result<u32> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM orders WHERE status IN ('open', 'partially_filled')",
        )
        .fetch_one(self.pool())
        .await?;
        Ok(count as u32)
    }

    pub async fn open_order_count_for_user(&self, user_id: i64) -> Result<u32> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM orders o
             JOIN trades t ON t.id = o.trade_id
             WHERE o.status IN ('open', 'partially_filled')
               AND t.user_id = $1",
        )
        .bind(user_id)
        .fetch_one(self.pool())
        .await?;
        Ok(count as u32)
    }

    pub async fn daily_realized_pnl(&self) -> Result<f64> {
        let pnl: Option<f64> = sqlx::query_scalar(
            "SELECT SUM(COALESCE(realized_pnl, 0.0)) FROM trades WHERE closed_at::date = CURRENT_DATE",
        )
        .fetch_one(self.pool())
        .await?;
        Ok(pnl.unwrap_or(0.0))
    }

    pub async fn daily_realized_pnl_for_user(&self, user_id: i64) -> Result<f64> {
        let pnl: Option<f64> = sqlx::query_scalar(
            "SELECT SUM(COALESCE(realized_pnl, 0.0))
             FROM trades
             WHERE closed_at::date = CURRENT_DATE
               AND user_id = $1",
        )
        .bind(user_id)
        .fetch_one(self.pool())
        .await?;
        Ok(pnl.unwrap_or(0.0))
    }

    pub async fn consecutive_losses(&self, limit: i64) -> Result<u32> {
        let rows: Vec<Option<f64>> = sqlx::query_scalar(
            "SELECT realized_pnl FROM trades WHERE closed_at IS NOT NULL ORDER BY closed_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.pool())
        .await?;

        let mut losses = 0u32;
        for pnl in rows {
            if pnl.unwrap_or(0.0) < 0.0 {
                losses += 1;
            } else {
                break;
            }
        }
        Ok(losses)
    }

    pub async fn consecutive_losses_for_user(&self, user_id: i64, limit: i64) -> Result<u32> {
        let rows: Vec<Option<f64>> = sqlx::query_scalar(
            "SELECT realized_pnl
             FROM trades
             WHERE closed_at IS NOT NULL
               AND user_id = $1
             ORDER BY closed_at DESC
             LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(self.pool())
        .await?;

        let mut losses = 0u32;
        for pnl in rows {
            if pnl.unwrap_or(0.0) < 0.0 {
                losses += 1;
            } else {
                break;
            }
        }
        Ok(losses)
    }
}
