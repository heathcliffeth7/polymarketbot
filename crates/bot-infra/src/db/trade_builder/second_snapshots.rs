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

    pub async fn trade_builder_adverse_move_stats(
        &self,
        input: &TradeBuilderAdverseMoveStatsQuery,
    ) -> Result<TradeBuilderAdverseMoveStats> {
        let slope_bucket = input
            .slope_bucket
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let row = sqlx::query(
            "WITH raw AS ( \
                SELECT market_slug, second_ts, window_end, \
                       EXTRACT(EPOCH FROM (window_end - second_ts))::DOUBLE PRECISION AS remaining_sec, \
                       CASE WHEN $2 = 'down' THEN ptb_ref_price - chainlink_price ELSE chainlink_price - ptb_ref_price END AS directional_gap, \
                       CASE WHEN $2 = 'down' THEN no_best_ask ELSE yes_best_ask END AS entry_ask \
                FROM market_price_second_snapshots \
                WHERE LOWER(asset) = LOWER($1) \
                  AND market_slug <> $3 \
                  AND second_ts >= $4 \
                  AND second_ts <= $5 \
                  AND second_ts <= window_end \
                  AND ptb_ref_price IS NOT NULL \
                  AND chainlink_price IS NOT NULL \
             ), scored AS ( \
                SELECT market_slug, second_ts, remaining_sec, directional_gap, entry_ask, \
                       MIN(directional_gap) OVER (PARTITION BY market_slug ORDER BY second_ts ASC ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING) AS future_min_gap, \
                       LAG(directional_gap, 3) OVER (PARTITION BY market_slug ORDER BY second_ts ASC) AS prior_gap_3s \
                FROM raw \
             ), candidates AS ( \
                SELECT market_slug, GREATEST(0.0, directional_gap - future_min_gap) AS adverse_move \
                FROM scored \
                WHERE remaining_sec >= $6 \
                  AND remaining_sec < $7 \
                  AND entry_ask >= $8 \
                  AND entry_ask < $9 \
                  AND ($10::DOUBLE PRECISION IS NULL OR directional_gap >= $10) \
                  AND ($11::DOUBLE PRECISION IS NULL OR directional_gap < $11) \
                  AND ( \
                    $12::TEXT IS NULL \
                    OR ($12 = 'negative' AND prior_gap_3s IS NOT NULL AND directional_gap - prior_gap_3s < 0.0) \
                    OR ($12 = 'non_negative' AND prior_gap_3s IS NOT NULL AND directional_gap - prior_gap_3s >= 0.0) \
                    OR ($12 = 'unknown' AND prior_gap_3s IS NULL) \
                  ) \
             ) \
             SELECT percentile_cont($13) WITHIN GROUP (ORDER BY adverse_move) AS adverse_quantile, \
                    COUNT(*)::BIGINT AS sample_count, \
                    COUNT(DISTINCT market_slug)::BIGINT AS market_count \
             FROM candidates",
        )
        .bind(&input.asset)
        .bind(input.direction.trim().to_ascii_lowercase())
        .bind(&input.current_market_slug)
        .bind(input.since)
        .bind(input.until)
        .bind(input.remaining_min_sec)
        .bind(input.remaining_max_sec)
        .bind(input.price_min)
        .bind(input.price_max)
        .bind(input.gap_min)
        .bind(input.gap_max)
        .bind(slope_bucket)
        .bind(input.quantile.clamp(0.0, 1.0))
        .fetch_one(self.pool())
        .await?;

        Ok(TradeBuilderAdverseMoveStats {
            adverse_quantile: row.get("adverse_quantile"),
            sample_count: row.get("sample_count"),
            market_count: row.get("market_count"),
        })
    }
}
