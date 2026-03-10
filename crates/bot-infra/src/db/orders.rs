use super::*;

impl PostgresRepository {
    pub async fn append_order_event(
        &self,
        trade_id: i64,
        intent: &str,
        side: &str,
        price: f64,
        size: f64,
        status: &str,
    ) -> Result<i64> {
        self.append_order_event_with_client_id(trade_id, intent, side, price, size, status, None)
            .await
    }

    pub async fn append_order_event_with_client_id(
        &self,
        trade_id: i64,
        intent: &str,
        side: &str,
        price: f64,
        size: f64,
        status: &str,
        client_order_id: Option<&str>,
    ) -> Result<i64> {
        self.append_order_event_with_meta(
            trade_id,
            intent,
            side,
            price,
            size,
            status,
            client_order_id,
            None,
            None,
        )
        .await
    }

    pub async fn append_order_event_with_meta(
        &self,
        trade_id: i64,
        intent: &str,
        side: &str,
        price: f64,
        size: f64,
        status: &str,
        client_order_id: Option<&str>,
        leg_side: Option<&str>,
        token_id: Option<&str>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO orders (trade_id, exchange_order_id, intent, side, price, size, status, client_order_id, leg_side, token_id, last_exchange_status, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NOW(), NOW()) RETURNING id",
        )
        .bind(trade_id)
        .bind(Uuid::new_v4().to_string())
        .bind(intent)
        .bind(side)
        .bind(price)
        .bind(size)
        .bind(status)
        .bind(client_order_id)
        .bind(leg_side)
        .bind(token_id)
        .bind(status)
        .fetch_one(self.pool())
        .await?;
        Ok(id)
    }

    pub async fn append_fill_event(
        &self,
        order_id: i64,
        price: f64,
        size: f64,
        fee: f64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO fills (order_id, fill_id, price, size, fee, filled_at) VALUES ($1, gen_random_uuid()::text, $2, $3, $4, NOW())",
        )
        .bind(order_id)
        .bind(price)
        .bind(size)
        .bind(fee)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn upsert_order_by_exchange_id(
        &self,
        trade_id: i64,
        exchange_order_id: &str,
        client_order_id: Option<&str>,
        intent: &str,
        side: &str,
        price: f64,
        size: f64,
        status: &str,
        exchange_ts: Option<i64>,
        reject_reason: Option<&str>,
        raw_payload: &Value,
    ) -> Result<i64> {
        self.upsert_order_by_exchange_id_with_meta(
            trade_id,
            exchange_order_id,
            client_order_id,
            intent,
            side,
            price,
            size,
            status,
            exchange_ts,
            reject_reason,
            raw_payload,
            None,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_order_by_exchange_id_with_meta(
        &self,
        trade_id: i64,
        exchange_order_id: &str,
        client_order_id: Option<&str>,
        intent: &str,
        side: &str,
        price: f64,
        size: f64,
        status: &str,
        exchange_ts: Option<i64>,
        reject_reason: Option<&str>,
        raw_payload: &Value,
        leg_side: Option<&str>,
        token_id: Option<&str>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO orders (trade_id, exchange_order_id, client_order_id, intent, side, price, size, status, leg_side, token_id, last_exchange_status, exchange_ts, reject_reason, raw_payload_json, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $8, $11, $12, $13, NOW(), NOW()) \
             ON CONFLICT (exchange_order_id) DO UPDATE SET \
               client_order_id = EXCLUDED.client_order_id, \
               status = EXCLUDED.status, \
               leg_side = COALESCE(EXCLUDED.leg_side, orders.leg_side), \
               token_id = COALESCE(EXCLUDED.token_id, orders.token_id), \
               last_exchange_status = EXCLUDED.last_exchange_status, \
               exchange_ts = EXCLUDED.exchange_ts, \
               reject_reason = EXCLUDED.reject_reason, \
               raw_payload_json = EXCLUDED.raw_payload_json, \
               updated_at = NOW() \
             RETURNING id",
        )
        .bind(trade_id)
        .bind(exchange_order_id)
        .bind(client_order_id)
        .bind(intent)
        .bind(side)
        .bind(price)
        .bind(size)
        .bind(status)
        .bind(leg_side)
        .bind(token_id)
        .bind(exchange_ts)
        .bind(reject_reason)
        .bind(raw_payload)
        .fetch_one(self.pool())
        .await?;
        Ok(id)
    }

    pub async fn upsert_fill_by_exchange_fill_id(
        &self,
        order_id: i64,
        exchange_fill_id: &str,
        price: f64,
        size: f64,
        fee: f64,
        exchange_ts: Option<i64>,
        raw_payload: &Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO fills (order_id, fill_id, price, size, fee, exchange_ts, raw_payload_json, filled_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, NOW()) \
             ON CONFLICT (fill_id) DO UPDATE SET \
               price = EXCLUDED.price, \
               size = EXCLUDED.size, \
               fee = EXCLUDED.fee, \
               exchange_ts = EXCLUDED.exchange_ts, \
               raw_payload_json = EXCLUDED.raw_payload_json",
        )
        .bind(order_id)
        .bind(exchange_fill_id)
        .bind(price)
        .bind(size)
        .bind(fee)
        .bind(exchange_ts)
        .bind(raw_payload)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn mark_order_status(&self, exchange_order_id: &str, status: &str) -> Result<()> {
        sqlx::query(
            "UPDATE orders SET status = $2, last_exchange_status = $2, updated_at = NOW() WHERE exchange_order_id = $1",
        )
        .bind(exchange_order_id)
        .bind(status)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn open_exchange_order_ids_for_trade(&self, trade_id: i64) -> Result<Vec<String>> {
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT exchange_order_id FROM orders \
             WHERE trade_id = $1 \
               AND lower(status) NOT IN ('filled', 'canceled', 'cancelled', 'rejected', 'expired')",
        )
        .bind(trade_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows)
    }

    pub async fn internal_order_id_by_exchange_order_id(
        &self,
        exchange_order_id: &str,
    ) -> Result<Option<i64>> {
        let id = sqlx::query_scalar::<_, i64>("SELECT id FROM orders WHERE exchange_order_id = $1")
            .bind(exchange_order_id)
            .fetch_optional(self.pool())
            .await?;
        Ok(id)
    }

    pub async fn aggregate_fill_qty_by_exchange_order_id(
        &self,
        exchange_order_id: &str,
    ) -> Result<f64> {
        let filled_qty = sqlx::query_scalar::<_, f64>(
            "SELECT COALESCE(SUM(f.size), 0)::double precision \
             FROM fills f \
             JOIN orders o ON o.id = f.order_id \
             WHERE o.exchange_order_id = $1",
        )
        .bind(exchange_order_id)
        .fetch_one(self.pool())
        .await?;
        Ok(filled_qty)
    }

    pub async fn order_size_by_exchange_order_id(
        &self,
        exchange_order_id: &str,
    ) -> Result<Option<f64>> {
        let size =
            sqlx::query_scalar::<_, f64>("SELECT size FROM orders WHERE exchange_order_id = $1")
                .bind(exchange_order_id)
                .fetch_optional(self.pool())
                .await?;
        Ok(size)
    }

    pub async fn try_record_idempotency_key(&self, event_key: &str) -> Result<bool> {
        let rows = sqlx::query("INSERT INTO idempotency_keys (event_key) VALUES ($1) ON CONFLICT (event_key) DO NOTHING")
            .bind(event_key)
            .execute(self.pool())
            .await?
            .rows_affected();
        Ok(rows == 1)
    }

    pub async fn record_reconcile_run(
        &self,
        run_id: i64,
        market_slug: &str,
        status: &str,
        details: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO reconcile_runs (run_id, market_slug, status, details_json, created_at) VALUES ($1, $2, $3, $4, NOW())",
        )
        .bind(run_id)
        .bind(market_slug)
        .bind(status)
        .bind(details)
        .execute(self.pool())
        .await?;
        Ok(())
    }
}
