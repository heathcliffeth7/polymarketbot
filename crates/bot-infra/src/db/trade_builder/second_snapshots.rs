use super::super::*;

impl PostgresRepository {
    pub async fn upsert_trade_builder_market_second_snapshot(
        &self,
        input: &TradeBuilderMarketSecondSnapshotInput,
    ) -> Result<()> {
        let sample_count = input.sample_count.max(1);
        sqlx::query(
            "INSERT INTO market_price_second_snapshots \
             (market_slug, asset, window_start, window_end, second_ts, ptb_ref_price, chainlink_price, \
              yes_best_bid, yes_best_ask, yes_ask_depth_usdc, \
              no_best_bid, no_best_ask, no_ask_depth_usdc, sample_count, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NOW()) \
             ON CONFLICT (market_slug, second_ts) DO UPDATE SET \
               asset = EXCLUDED.asset, \
               window_start = EXCLUDED.window_start, \
               window_end = EXCLUDED.window_end, \
               ptb_ref_price = COALESCE(EXCLUDED.ptb_ref_price, market_price_second_snapshots.ptb_ref_price), \
               chainlink_price = COALESCE(EXCLUDED.chainlink_price, market_price_second_snapshots.chainlink_price), \
               yes_best_bid = COALESCE(EXCLUDED.yes_best_bid, market_price_second_snapshots.yes_best_bid), \
               yes_best_ask = COALESCE(EXCLUDED.yes_best_ask, market_price_second_snapshots.yes_best_ask), \
               yes_ask_depth_usdc = COALESCE(EXCLUDED.yes_ask_depth_usdc, market_price_second_snapshots.yes_ask_depth_usdc), \
               no_best_bid = COALESCE(EXCLUDED.no_best_bid, market_price_second_snapshots.no_best_bid), \
               no_best_ask = COALESCE(EXCLUDED.no_best_ask, market_price_second_snapshots.no_best_ask), \
               no_ask_depth_usdc = COALESCE(EXCLUDED.no_ask_depth_usdc, market_price_second_snapshots.no_ask_depth_usdc), \
               sample_count = market_price_second_snapshots.sample_count + EXCLUDED.sample_count"
        )
        .bind(&input.market_slug)
        .bind(&input.asset)
        .bind(input.window_start)
        .bind(input.window_end)
        .bind(input.second_ts)
        .bind(input.ptb_ref_price)
        .bind(input.chainlink_price)
        .bind(input.yes_best_bid)
        .bind(input.yes_best_ask)
        .bind(input.yes_ask_depth_usdc)
        .bind(input.no_best_bid)
        .bind(input.no_best_ask)
        .bind(input.no_ask_depth_usdc)
        .bind(sample_count)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_trade_builder_market_second_snapshots(
        &self,
        market_slugs: &[String],
    ) -> Result<Vec<TradeBuilderMarketSecondSnapshot>> {
        if market_slugs.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            "SELECT market_slug, asset, window_start, window_end, second_ts, ptb_ref_price, chainlink_price, \
                    yes_best_bid, yes_best_ask, yes_ask_depth_usdc, \
                    no_best_bid, no_best_ask, no_ask_depth_usdc, sample_count \
             FROM market_price_second_snapshots \
             WHERE market_slug = ANY($1) \
             ORDER BY market_slug ASC, second_ts ASC",
        )
        .bind(market_slugs)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeBuilderMarketSecondSnapshot {
                market_slug: row.get("market_slug"),
                asset: row.get("asset"),
                window_start: row.get("window_start"),
                window_end: row.get("window_end"),
                second_ts: row.get("second_ts"),
                ptb_ref_price: row.get("ptb_ref_price"),
                chainlink_price: row.get("chainlink_price"),
                yes_best_bid: row.get("yes_best_bid"),
                yes_best_ask: row.get("yes_best_ask"),
                yes_ask_depth_usdc: row.get("yes_ask_depth_usdc"),
                no_best_bid: row.get("no_best_bid"),
                no_best_ask: row.get("no_best_ask"),
                no_ask_depth_usdc: row.get("no_ask_depth_usdc"),
                sample_count: row.get("sample_count"),
            })
            .collect())
    }
}
