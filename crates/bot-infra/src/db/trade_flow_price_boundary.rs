use super::*;

#[derive(Debug, Clone)]
pub struct TradeFlowPriceBoundarySnapshotInput {
    pub market_slug: String,
    pub asset: String,
    pub timeframe: String,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub open_price: Option<f64>,
    pub open_ts: Option<DateTime<Utc>>,
    pub high_price: Option<f64>,
    pub low_price: Option<f64>,
    pub close_price: Option<f64>,
    pub close_ts: Option<DateTime<Utc>>,
    pub sample_count: i32,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct TradeFlowPriceBoundarySnapshot {
    pub market_slug: String,
    pub asset: String,
    pub timeframe: String,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub open_price: Option<f64>,
    pub open_ts: Option<DateTime<Utc>>,
    pub high_price: Option<f64>,
    pub low_price: Option<f64>,
    pub close_price: Option<f64>,
    pub close_ts: Option<DateTime<Utc>>,
    pub sample_count: i32,
    pub source: String,
    pub updated_at: DateTime<Utc>,
}

impl PostgresRepository {
    pub async fn upsert_trade_flow_price_boundary_snapshot(
        &self,
        input: &TradeFlowPriceBoundarySnapshotInput,
    ) -> Result<TradeFlowPriceBoundarySnapshot> {
        let row = sqlx::query(
            "INSERT INTO trade_flow_price_boundary_snapshots \
             (market_slug, asset, timeframe, window_start, window_end, \
              open_price, open_ts, high_price, low_price, close_price, close_ts, sample_count, source) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) \
             ON CONFLICT (market_slug) DO UPDATE SET \
               asset = EXCLUDED.asset, \
               timeframe = EXCLUDED.timeframe, \
               window_start = EXCLUDED.window_start, \
               window_end = EXCLUDED.window_end, \
               open_price = COALESCE(EXCLUDED.open_price, trade_flow_price_boundary_snapshots.open_price), \
               open_ts = COALESCE(EXCLUDED.open_ts, trade_flow_price_boundary_snapshots.open_ts), \
               high_price = COALESCE(EXCLUDED.high_price, trade_flow_price_boundary_snapshots.high_price), \
               low_price = COALESCE(EXCLUDED.low_price, trade_flow_price_boundary_snapshots.low_price), \
               close_price = COALESCE(EXCLUDED.close_price, trade_flow_price_boundary_snapshots.close_price), \
               close_ts = COALESCE(EXCLUDED.close_ts, trade_flow_price_boundary_snapshots.close_ts), \
               sample_count = GREATEST(trade_flow_price_boundary_snapshots.sample_count, EXCLUDED.sample_count), \
               source = EXCLUDED.source, \
               updated_at = NOW() \
             RETURNING market_slug, asset, timeframe, window_start, window_end, \
               open_price, open_ts, high_price, low_price, close_price, close_ts, sample_count, source, updated_at",
        )
        .bind(&input.market_slug)
        .bind(&input.asset)
        .bind(&input.timeframe)
        .bind(input.window_start)
        .bind(input.window_end)
        .bind(input.open_price)
        .bind(input.open_ts)
        .bind(input.high_price)
        .bind(input.low_price)
        .bind(input.close_price)
        .bind(input.close_ts)
        .bind(input.sample_count.max(0))
        .bind(&input.source)
        .fetch_one(self.pool())
        .await?;
        Ok(Self::trade_flow_price_boundary_snapshot_from_row(row))
    }

    pub async fn get_trade_flow_price_boundary_snapshot(
        &self,
        market_slug: &str,
    ) -> Result<Option<TradeFlowPriceBoundarySnapshot>> {
        let row = sqlx::query(
            "SELECT market_slug, asset, timeframe, window_start, window_end, \
                    open_price, open_ts, high_price, low_price, close_price, close_ts, sample_count, source, updated_at \
             FROM trade_flow_price_boundary_snapshots \
             WHERE market_slug = $1",
        )
        .bind(market_slug)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(Self::trade_flow_price_boundary_snapshot_from_row))
    }

    fn trade_flow_price_boundary_snapshot_from_row(
        row: sqlx::postgres::PgRow,
    ) -> TradeFlowPriceBoundarySnapshot {
        TradeFlowPriceBoundarySnapshot {
            market_slug: row.get("market_slug"),
            asset: row.get("asset"),
            timeframe: row.get("timeframe"),
            window_start: row.get("window_start"),
            window_end: row.get("window_end"),
            open_price: row.get("open_price"),
            open_ts: row.get("open_ts"),
            high_price: row.get("high_price"),
            low_price: row.get("low_price"),
            close_price: row.get("close_price"),
            close_ts: row.get("close_ts"),
            sample_count: row.get("sample_count"),
            source: row.get("source"),
            updated_at: row.get("updated_at"),
        }
    }
}
