use super::super::*;

impl PostgresRepository {
    pub async fn insert_trade_builder_market_trade_tick(
        &self,
        input: &TradeBuilderMarketTradeTickInput,
    ) -> Result<bool> {
        let result = sqlx::query(
            "INSERT INTO market_trade_ticks \
             (market_slug, asset, window_start, window_end, token_id, outcome_side, event_ts, \
              price, size, notional_usdc, side, dedupe_key, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW()) \
             ON CONFLICT (dedupe_key) DO NOTHING",
        )
        .bind(&input.market_slug)
        .bind(&input.asset)
        .bind(input.window_start)
        .bind(input.window_end)
        .bind(&input.token_id)
        .bind(&input.outcome_side)
        .bind(input.event_ts)
        .bind(input.price)
        .bind(input.size)
        .bind(input.notional_usdc)
        .bind(&input.side)
        .bind(&input.dedupe_key)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn sum_market_trade_notional_usdc(
        &self,
        market_slug: &str,
        now: DateTime<Utc>,
        window_sec: i64,
    ) -> Result<f64> {
        let start_at = now - chrono::Duration::seconds(window_sec.max(1));
        let value = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT COALESCE(SUM(notional_usdc), 0.0) \
             FROM market_trade_ticks \
             WHERE market_slug = $1 AND event_ts >= $2 AND event_ts <= $3",
        )
        .bind(market_slug)
        .bind(start_at)
        .bind(now)
        .fetch_one(self.pool())
        .await?;
        Ok(value.unwrap_or(0.0).max(0.0))
    }

    pub async fn market_trade_volume_summary(
        &self,
        market_slug: &str,
        now: DateTime<Utc>,
    ) -> Result<MarketTradeVolumeSummary> {
        let start_10s = now - chrono::Duration::seconds(10);
        let start_30s = now - chrono::Duration::seconds(30);
        let start_60s = now - chrono::Duration::seconds(60);
        let row = sqlx::query(
            "SELECT \
                COALESCE(SUM(notional_usdc) FILTER (WHERE event_ts >= $2), 0.0)::DOUBLE PRECISION AS volume_10s, \
                COALESCE(SUM(notional_usdc) FILTER (WHERE event_ts >= $3), 0.0)::DOUBLE PRECISION AS volume_30s, \
                COALESCE(SUM(notional_usdc) FILTER (WHERE event_ts >= $4), 0.0)::DOUBLE PRECISION AS volume_60s, \
                COUNT(*) FILTER (WHERE event_ts >= $2)::BIGINT AS trade_count_10s, \
                COUNT(*) FILTER (WHERE event_ts >= $3)::BIGINT AS trade_count_30s, \
                COUNT(*) FILTER (WHERE event_ts >= $4)::BIGINT AS trade_count_60s \
             FROM market_trade_ticks \
             WHERE market_slug = $1 AND event_ts >= $4 AND event_ts <= $5",
        )
        .bind(market_slug)
        .bind(start_10s)
        .bind(start_30s)
        .bind(start_60s)
        .bind(now)
        .fetch_one(self.pool())
        .await?;

        Ok(MarketTradeVolumeSummary {
            volume_10s: row.get::<f64, _>("volume_10s").max(0.0),
            volume_30s: row.get::<f64, _>("volume_30s").max(0.0),
            volume_60s: row.get::<f64, _>("volume_60s").max(0.0),
            trade_count_10s: row.get("trade_count_10s"),
            trade_count_30s: row.get("trade_count_30s"),
            trade_count_60s: row.get("trade_count_60s"),
        })
    }

    pub async fn list_market_trade_hourly_volume_medians(
        &self,
        asset: &str,
        hours_utc: &[i32],
        bucket_start_sec: f64,
        bucket_end_sec: f64,
        lookback_days: i64,
        volume_window_sec: i64,
        exclude_market_slug: &str,
        now: DateTime<Utc>,
    ) -> Result<Vec<MarketTradeHourlyVolumeMedian>> {
        if hours_utc.is_empty() {
            return Ok(Vec::new());
        }
        let start_at = now - chrono::Duration::days(lookback_days.max(1));
        let rows = sqlx::query(
            "WITH bucketed AS ( \
               SELECT \
                 EXTRACT(HOUR FROM event_ts AT TIME ZONE 'UTC')::INT AS hour_utc, \
                 market_slug, \
                 FLOOR(EXTRACT(EPOCH FROM event_ts) / $6)::BIGINT AS volume_bucket, \
                 SUM(notional_usdc) AS volume_usdc \
               FROM market_trade_ticks \
               WHERE asset = $1 \
                 AND event_ts >= $2 AND event_ts < $3 \
                 AND market_slug <> $4 \
                 AND EXTRACT(HOUR FROM event_ts AT TIME ZONE 'UTC')::INT = ANY($5) \
                 AND EXTRACT(EPOCH FROM (window_end - event_ts)) <= $7 \
                 AND EXTRACT(EPOCH FROM (window_end - event_ts)) > $8 \
               GROUP BY hour_utc, market_slug, volume_bucket \
             ) \
             SELECT hour_utc, \
                    percentile_cont(0.5) WITHIN GROUP (ORDER BY volume_usdc)::DOUBLE PRECISION AS median_volume_usdc, \
                    COUNT(*)::BIGINT AS sample_count \
             FROM bucketed \
             GROUP BY hour_utc \
             ORDER BY hour_utc ASC",
        )
        .bind(asset)
        .bind(start_at)
        .bind(now)
        .bind(exclude_market_slug)
        .bind(hours_utc)
        .bind(volume_window_sec.max(1))
        .bind(bucket_start_sec)
        .bind(bucket_end_sec)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| MarketTradeHourlyVolumeMedian {
                hour_utc: row.get("hour_utc"),
                median_volume_usdc: row.get("median_volume_usdc"),
                sample_count: row.get("sample_count"),
            })
            .collect())
    }

    pub async fn market_trade_volume_bucket_median(
        &self,
        asset: &str,
        bucket_start_sec: f64,
        bucket_end_sec: f64,
        lookback_days: i64,
        volume_window_sec: i64,
        exclude_market_slug: &str,
        now: DateTime<Utc>,
    ) -> Result<MarketTradeVolumeMedian> {
        let start_at = now - chrono::Duration::days(lookback_days.max(1));
        let row = sqlx::query(
            "WITH bucketed AS ( \
               SELECT \
                 market_slug, \
                 FLOOR(EXTRACT(EPOCH FROM event_ts) / $5)::BIGINT AS volume_bucket, \
                 SUM(notional_usdc) AS volume_usdc \
               FROM market_trade_ticks \
               WHERE asset = $1 \
                 AND event_ts >= $2 AND event_ts < $3 \
                 AND market_slug <> $4 \
                 AND EXTRACT(EPOCH FROM (window_end - event_ts)) <= $6 \
                 AND EXTRACT(EPOCH FROM (window_end - event_ts)) > $7 \
               GROUP BY market_slug, volume_bucket \
             ) \
             SELECT COALESCE(percentile_cont(0.5) WITHIN GROUP (ORDER BY volume_usdc), 0.0)::DOUBLE PRECISION AS median_volume_usdc, \
                    COUNT(*)::BIGINT AS sample_count \
             FROM bucketed",
        )
        .bind(asset)
        .bind(start_at)
        .bind(now)
        .bind(exclude_market_slug)
        .bind(volume_window_sec.max(1))
        .bind(bucket_start_sec)
        .bind(bucket_end_sec)
        .fetch_one(self.pool())
        .await?;

        Ok(MarketTradeVolumeMedian {
            median_volume_usdc: row.get("median_volume_usdc"),
            sample_count: row.get("sample_count"),
        })
    }
}
