use super::super::*;

const TRADE_BUILDER_ADVERSE_MOVE_STATS_SQL: &str = "WITH raw AS ( \
                SELECT market_slug, second_ts, window_end, \
                       EXTRACT(EPOCH FROM (window_end - second_ts))::DOUBLE PRECISION AS remaining_sec, \
                       CASE WHEN $2 = 'down' THEN ptb_ref_price - chainlink_price ELSE chainlink_price - ptb_ref_price END AS directional_gap, \
                       CASE WHEN $2 = 'down' THEN no_best_ask ELSE yes_best_ask END AS entry_ask \
                FROM market_price_second_snapshots \
                WHERE asset = $1 \
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
             FROM candidates";

const NO_REVERSAL_ADVERSE_FEATURE_REFRESH_SQL: &str = r#"
WITH source AS (
  SELECT
    market_slug,
    asset,
    window_start,
    window_end,
    second_ts,
    ptb_ref_price,
    chainlink_price,
    yes_best_ask,
    no_best_ask
  FROM market_price_second_snapshots
  WHERE asset = $1
    AND window_end >= $2
    AND window_end <= $3
    AND second_ts <= window_end
    AND ptb_ref_price IS NOT NULL
    AND chainlink_price IS NOT NULL
), directional AS (
  SELECT
    market_slug,
    asset,
    window_start,
    window_end,
    second_ts,
    'up'::TEXT AS direction,
    EXTRACT(EPOCH FROM (window_end - second_ts))::DOUBLE PRECISION AS remaining_sec,
    yes_best_ask AS entry_ask,
    chainlink_price - ptb_ref_price AS directional_gap
  FROM source
  UNION ALL
  SELECT
    market_slug,
    asset,
    window_start,
    window_end,
    second_ts,
    'down'::TEXT AS direction,
    EXTRACT(EPOCH FROM (window_end - second_ts))::DOUBLE PRECISION AS remaining_sec,
    no_best_ask AS entry_ask,
    ptb_ref_price - chainlink_price AS directional_gap
  FROM source
), scored AS (
  SELECT
    market_slug,
    asset,
    window_start,
    window_end,
    second_ts,
    direction,
    remaining_sec,
    entry_ask,
    directional_gap,
    directional_gap
      - LAG(directional_gap, 3) OVER (
          PARTITION BY market_slug, direction
          ORDER BY second_ts ASC
        ) AS slope_delta_3s,
    MIN(directional_gap) OVER (
      PARTITION BY market_slug, direction
      ORDER BY second_ts ASC
      ROWS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING
    ) AS future_min_gap
  FROM directional
), feature_rows AS (
  SELECT
    market_slug,
    direction,
    second_ts,
    asset,
    window_start,
    window_end,
    remaining_sec,
    entry_ask,
    directional_gap,
    slope_delta_3s,
    CASE
      WHEN slope_delta_3s IS NULL THEN 'unknown'
      WHEN slope_delta_3s < 0.0 THEN 'negative'
      ELSE 'non_negative'
    END AS slope_bucket,
    GREATEST(0.0, directional_gap - future_min_gap) AS adverse_move
  FROM scored
)
INSERT INTO no_reversal_adverse_feature_rows (
  market_slug,
  direction,
  second_ts,
  asset,
  window_start,
  window_end,
  remaining_sec,
  entry_ask,
  directional_gap,
  slope_delta_3s,
  slope_bucket,
  adverse_move,
  computed_at
)
SELECT
  market_slug,
  direction,
  second_ts,
  asset,
  window_start,
  window_end,
  remaining_sec,
  entry_ask,
  directional_gap,
  slope_delta_3s,
  slope_bucket,
  adverse_move,
  NOW()
FROM feature_rows
ON CONFLICT (market_slug, direction, second_ts) DO UPDATE SET
  asset = EXCLUDED.asset,
  window_start = EXCLUDED.window_start,
  window_end = EXCLUDED.window_end,
  remaining_sec = EXCLUDED.remaining_sec,
  entry_ask = EXCLUDED.entry_ask,
  directional_gap = EXCLUDED.directional_gap,
  slope_delta_3s = EXCLUDED.slope_delta_3s,
  slope_bucket = EXCLUDED.slope_bucket,
  adverse_move = EXCLUDED.adverse_move,
  computed_at = NOW()
"#;

const TRADE_BUILDER_ADVERSE_MOVE_STATS_BULK_FEATURE_SQL: &str = r#"
WITH requests AS (
  SELECT *
  FROM jsonb_to_recordset($8::jsonb) AS request(
    fallback_level TEXT,
    lookback_name TEXT,
    hours BIGINT,
    min_samples BIGINT,
    min_markets BIGINT,
    since_ts TIMESTAMPTZ,
    until_ts TIMESTAMPTZ,
    gap_min DOUBLE PRECISION,
    gap_max DOUBLE PRECISION,
    slope_bucket TEXT,
    quantile DOUBLE PRECISION
  )
), bounds AS (
  SELECT MIN(since_ts) AS min_since_ts, MAX(until_ts) AS max_until_ts
  FROM requests
), candidates AS (
  SELECT
    request.fallback_level,
    request.lookback_name,
    features.market_slug,
    features.adverse_move
  FROM requests request
  JOIN bounds ON TRUE
  JOIN no_reversal_adverse_feature_rows features
    ON features.asset = $1
   AND features.direction = $2
   AND features.second_ts >= bounds.min_since_ts
   AND features.second_ts <= bounds.max_until_ts
  WHERE features.market_slug <> $3
    AND features.second_ts >= request.since_ts
    AND features.second_ts <= request.until_ts
    AND features.remaining_sec >= $4
    AND features.remaining_sec < $5
    AND features.entry_ask >= $6
    AND features.entry_ask < $7
    AND (request.gap_min IS NULL OR features.directional_gap >= request.gap_min)
    AND (request.gap_max IS NULL OR features.directional_gap < request.gap_max)
    AND (request.slope_bucket IS NULL OR features.slope_bucket = request.slope_bucket)
)
SELECT
  request.fallback_level,
  request.lookback_name,
  request.hours,
  request.min_samples,
  request.min_markets,
  percentile_cont(request.quantile) WITHIN GROUP (ORDER BY candidates.adverse_move) AS adverse_quantile,
  COUNT(candidates.adverse_move)::BIGINT AS sample_count,
  COUNT(DISTINCT candidates.market_slug)::BIGINT AS market_count
FROM requests request
LEFT JOIN candidates
  ON candidates.fallback_level = request.fallback_level
 AND candidates.lookback_name = request.lookback_name
GROUP BY
  request.fallback_level,
  request.lookback_name,
  request.hours,
  request.min_samples,
  request.min_markets,
  request.quantile
ORDER BY request.fallback_level ASC, request.hours ASC
"#;

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
        let row = sqlx::query(TRADE_BUILDER_ADVERSE_MOVE_STATS_SQL)
            .bind(input.asset.trim().to_ascii_lowercase())
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

    pub async fn refresh_no_reversal_adverse_features(
        &self,
        asset: &str,
        since: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<u64> {
        let result = sqlx::query(NO_REVERSAL_ADVERSE_FEATURE_REFRESH_SQL)
            .bind(asset.trim().to_ascii_lowercase())
            .bind(since)
            .bind(until)
            .execute(self.pool())
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn trade_builder_adverse_move_stats_bulk_from_features(
        &self,
        input: &TradeBuilderAdverseMoveStatsBulkFromFeaturesQuery,
    ) -> Result<Vec<TradeBuilderAdverseMoveStatsBulkRow>> {
        if input.lookbacks.is_empty() {
            return Ok(Vec::new());
        }

        let lookbacks = serde_json::Value::Array(
            input
                .lookbacks
                .iter()
                .map(|lookback| {
                    serde_json::json!({
                        "fallback_level": lookback.fallback_level,
                        "lookback_name": lookback.lookback_name,
                        "hours": lookback.hours,
                        "min_samples": lookback.min_samples,
                        "min_markets": lookback.min_markets,
                        "since_ts": lookback.since,
                        "until_ts": lookback.until,
                        "gap_min": lookback.gap_min,
                        "gap_max": lookback.gap_max,
                        "slope_bucket": lookback.slope_bucket,
                        "quantile": lookback.quantile.clamp(0.0, 1.0),
                    })
                })
                .collect(),
        );

        let rows = sqlx::query(TRADE_BUILDER_ADVERSE_MOVE_STATS_BULK_FEATURE_SQL)
            .bind(input.asset.trim().to_ascii_lowercase())
            .bind(input.direction.trim().to_ascii_lowercase())
            .bind(&input.current_market_slug)
            .bind(input.remaining_min_sec)
            .bind(input.remaining_max_sec)
            .bind(input.price_min)
            .bind(input.price_max)
            .bind(lookbacks)
            .fetch_all(self.pool())
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeBuilderAdverseMoveStatsBulkRow {
                fallback_level: row.get("fallback_level"),
                lookback_name: row.get("lookback_name"),
                hours: row.get("hours"),
                min_samples: row.get("min_samples"),
                min_markets: row.get("min_markets"),
                adverse_quantile: row.get("adverse_quantile"),
                sample_count: row.get("sample_count"),
                market_count: row.get("market_count"),
            })
            .collect())
    }

    pub async fn upsert_no_reversal_adverse_profile(
        &self,
        input: &NoReversalAdverseProfileInput,
    ) -> Result<()> {
        let key = &input.key;
        sqlx::query(
            "INSERT INTO no_reversal_adverse_profiles \
             (target_market_slug, target_window_start, definition_id, node_key, profile_config_hash, \
              asset, direction, remaining_bucket, price_bucket, gap_bucket, slope_bucket, quantile, high_late, \
              status, selected_adverse, raw_selected_adverse, fallback_level, lookbacks_json, \
              sample_count, market_count, profile_as_of, error, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, \
                     $14, $15, $16, $17, $18, $19, $20, $21, $22, NOW(), NOW()) \
             ON CONFLICT (target_market_slug, target_window_start, definition_id, node_key, profile_config_hash, \
                          asset, direction, remaining_bucket, price_bucket, gap_bucket, slope_bucket, quantile, high_late) \
             DO UPDATE SET \
               status = EXCLUDED.status, \
               selected_adverse = EXCLUDED.selected_adverse, \
               raw_selected_adverse = EXCLUDED.raw_selected_adverse, \
               fallback_level = EXCLUDED.fallback_level, \
               lookbacks_json = EXCLUDED.lookbacks_json, \
               sample_count = EXCLUDED.sample_count, \
               market_count = EXCLUDED.market_count, \
               profile_as_of = EXCLUDED.profile_as_of, \
               error = EXCLUDED.error, \
               updated_at = NOW()",
        )
        .bind(&key.target_market_slug)
        .bind(key.target_window_start)
        .bind(key.definition_id)
        .bind(&key.node_key)
        .bind(&key.profile_config_hash)
        .bind(&key.asset)
        .bind(&key.direction)
        .bind(&key.remaining_bucket)
        .bind(&key.price_bucket)
        .bind(&key.gap_bucket)
        .bind(&key.slope_bucket)
        .bind(key.quantile)
        .bind(key.high_late)
        .bind(&input.status)
        .bind(input.selected_adverse)
        .bind(input.raw_selected_adverse)
        .bind(&input.fallback_level)
        .bind(&input.lookbacks_json)
        .bind(input.sample_count)
        .bind(input.market_count)
        .bind(input.profile_as_of)
        .bind(&input.error)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn get_no_reversal_adverse_profile(
        &self,
        key: &NoReversalAdverseProfileKey,
    ) -> Result<Option<NoReversalAdverseProfileRecord>> {
        let row = sqlx::query(
            "SELECT status, selected_adverse, raw_selected_adverse, fallback_level, lookbacks_json, \
                    sample_count, market_count, profile_as_of, updated_at, error \
             FROM no_reversal_adverse_profiles \
             WHERE target_market_slug = $1 \
               AND target_window_start = $2 \
               AND definition_id = $3 \
               AND node_key = $4 \
               AND profile_config_hash = $5 \
               AND LOWER(asset) = LOWER($6) \
               AND LOWER(direction) = LOWER($7) \
               AND remaining_bucket = $8 \
               AND price_bucket = $9 \
               AND gap_bucket = $10 \
               AND slope_bucket = $11 \
               AND quantile = $12 \
               AND high_late = $13",
        )
        .bind(&key.target_market_slug)
        .bind(key.target_window_start)
        .bind(key.definition_id)
        .bind(&key.node_key)
        .bind(&key.profile_config_hash)
        .bind(&key.asset)
        .bind(&key.direction)
        .bind(&key.remaining_bucket)
        .bind(&key.price_bucket)
        .bind(&key.gap_bucket)
        .bind(&key.slope_bucket)
        .bind(key.quantile)
        .bind(key.high_late)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|row| NoReversalAdverseProfileRecord {
            status: row.get("status"),
            selected_adverse: row.get("selected_adverse"),
            raw_selected_adverse: row.get("raw_selected_adverse"),
            fallback_level: row.get("fallback_level"),
            lookbacks_json: row.get("lookbacks_json"),
            sample_count: row.get("sample_count"),
            market_count: row.get("market_count"),
            profile_as_of: row.get("profile_as_of"),
            updated_at: row.get("updated_at"),
            error: row.get("error"),
        }))
    }

    pub async fn no_reversal_adverse_profile_diagnostics(
        &self,
        key: &NoReversalAdverseProfileKey,
    ) -> Result<NoReversalAdverseProfileDiagnostics> {
        let table = sqlx::query(
            "SELECT \
                    COUNT(*)::BIGINT AS total_rows, \
                    COUNT(*) FILTER (WHERE profile_config_hash = $5)::BIGINT AS same_hash_rows, \
                    COUNT(*) FILTER (WHERE profile_config_hash <> $5)::BIGINT AS different_hash_rows, \
                    COUNT(*) FILTER (WHERE profile_config_hash = $5 \
                                      AND remaining_bucket = $8 \
                                      AND price_bucket = $9 \
                                      AND gap_bucket = $10 \
                                      AND slope_bucket = $11 \
                                      AND quantile = $12 \
                                      AND high_late = $13)::BIGINT AS same_hash_bucket_rows, \
                    COUNT(*) FILTER (WHERE remaining_bucket = $8 \
                                      AND price_bucket = $9 \
                                      AND gap_bucket = $10 \
                                      AND slope_bucket = $11 \
                                      AND quantile = $12 \
                                      AND high_late = $13)::BIGINT AS same_bucket_rows, \
                    COUNT(*) FILTER (WHERE status = 'ready')::BIGINT AS ready_rows, \
                    COUNT(*) FILTER (WHERE status = 'insufficient')::BIGINT AS insufficient_rows, \
                    COUNT(*) FILTER (WHERE status = 'timed_out')::BIGINT AS timed_out_rows, \
                    COUNT(*) FILTER (WHERE status = 'error')::BIGINT AS error_rows, \
                    MAX(updated_at) AS latest_updated_at \
             FROM no_reversal_adverse_profiles \
             WHERE target_market_slug = $1 \
               AND target_window_start = $2 \
               AND definition_id = $3 \
               AND node_key = $4 \
               AND LOWER(asset) = LOWER($6) \
               AND LOWER(direction) = LOWER($7)",
        )
        .bind(&key.target_market_slug)
        .bind(key.target_window_start)
        .bind(key.definition_id)
        .bind(&key.node_key)
        .bind(&key.profile_config_hash)
        .bind(&key.asset)
        .bind(&key.direction)
        .bind(&key.remaining_bucket)
        .bind(&key.price_bucket)
        .bind(&key.gap_bucket)
        .bind(&key.slope_bucket)
        .bind(key.quantile)
        .bind(key.high_late)
        .fetch_one(self.pool())
        .await?;

        let events = sqlx::query(
            "SELECT \
                    COUNT(*)::BIGINT AS event_rows, \
                    COUNT(*) FILTER (WHERE event_type IN ('no_reversal_profile_expected_key', 'no_reversal_profile_prewarm_started'))::BIGINT AS expected_rows, \
                    COUNT(*) FILTER (WHERE event_type = 'no_reversal_profile_prewarm_started')::BIGINT AS started_rows, \
                    COUNT(*) FILTER (WHERE event_type IN ('no_reversal_profile_timed_out', 'no_reversal_profile_prewarm_timed_out') \
                                      OR payload_json->>'status' = 'timed_out')::BIGINT AS timed_out_event_rows, \
                    COUNT(*) FILTER (WHERE event_type IN ('no_reversal_profile_failed', 'no_reversal_profile_prewarm_failed') \
                                      OR payload_json->>'status' IN ('lookup_error', 'error'))::BIGINT AS failed_event_rows, \
                    COUNT(*) FILTER (WHERE event_type = 'no_reversal_profile_capacity_limited' \
                                      OR payload_json->>'status' = 'queued_capacity_limited')::BIGINT AS capacity_limited_event_rows, \
                    MAX(created_at) FILTER (WHERE event_type = 'no_reversal_profile_expected_key') AS latest_expected_at, \
                    MAX(created_at) FILTER (WHERE event_type = 'no_reversal_profile_prewarm_started') AS latest_started_at, \
                    (ARRAY_AGG(payload_json->>'priority' ORDER BY created_at DESC) FILTER (WHERE payload_json ? 'priority'))[1] AS latest_priority, \
                    (ARRAY_AGG(payload_json->>'slot_status' ORDER BY created_at DESC) FILTER (WHERE payload_json ? 'slot_status'))[1] AS latest_slot_status, \
                    (EXTRACT(EPOCH FROM (NOW() - MAX(created_at) FILTER (WHERE event_type = 'no_reversal_profile_prewarm_started'))) * 1000)::BIGINT AS prewarm_age_ms, \
                    MAX(created_at) AS latest_event_at \
             FROM trade_flow_events \
             WHERE event_type LIKE 'no_reversal_profile%' \
               AND payload_json->>'target_market_slug' = $1 \
               AND (payload_json->>'target_window_start')::TIMESTAMPTZ = $2 \
               AND definition_id = $3 \
               AND payload_json->>'node_key' = $4 \
               AND payload_json->>'profile_config_hash' = $5 \
               AND LOWER(payload_json #>> '{profile_lookup_key,asset}') = LOWER($6) \
               AND LOWER(payload_json->>'direction') = LOWER($7) \
               AND payload_json #>> '{profile_lookup_key,remaining_bucket}' = $8 \
               AND payload_json #>> '{profile_lookup_key,price_bucket}' = $9 \
               AND payload_json #>> '{profile_lookup_key,gap_bucket}' = $10 \
               AND payload_json #>> '{profile_lookup_key,slope_bucket}' = $11 \
               AND (payload_json #>> '{profile_lookup_key,quantile}')::DOUBLE PRECISION = $12 \
               AND (payload_json #>> '{profile_lookup_key,high_late}')::BOOLEAN = $13",
        )
        .bind(&key.target_market_slug)
        .bind(key.target_window_start)
        .bind(key.definition_id)
        .bind(&key.node_key)
        .bind(&key.profile_config_hash)
        .bind(&key.asset)
        .bind(&key.direction)
        .bind(&key.remaining_bucket)
        .bind(&key.price_bucket)
        .bind(&key.gap_bucket)
        .bind(&key.slope_bucket)
        .bind(key.quantile)
        .bind(key.high_late)
        .fetch_one(self.pool())
        .await?;

        let total_rows: i64 = table.get("total_rows");
        let same_hash_rows: i64 = table.get("same_hash_rows");
        let different_hash_rows: i64 = table.get("different_hash_rows");
        let same_hash_bucket_rows: i64 = table.get("same_hash_bucket_rows");
        let same_bucket_rows: i64 = table.get("same_bucket_rows");
        let ready_rows: i64 = table.get("ready_rows");
        let insufficient_rows: i64 = table.get("insufficient_rows");
        let timed_out_rows: i64 = table.get("timed_out_rows");
        let error_rows: i64 = table.get("error_rows");
        let latest_updated_at: Option<DateTime<Utc>> = table.get("latest_updated_at");

        let event_rows: i64 = events.get("event_rows");
        let expected_rows: i64 = events.get("expected_rows");
        let started_rows: i64 = events.get("started_rows");
        let timed_out_event_rows: i64 = events.get("timed_out_event_rows");
        let failed_event_rows: i64 = events.get("failed_event_rows");
        let capacity_limited_event_rows: i64 = events.get("capacity_limited_event_rows");
        let latest_expected_at: Option<DateTime<Utc>> = events.get("latest_expected_at");
        let latest_started_at: Option<DateTime<Utc>> = events.get("latest_started_at");
        let latest_priority: Option<String> = events.get("latest_priority");
        let latest_slot_status: Option<String> = events.get("latest_slot_status");
        let prewarm_age_ms: Option<i64> = events.get("prewarm_age_ms");
        let latest_event_at: Option<DateTime<Utc>> = events.get("latest_event_at");

        let prewarmer_status = if timed_out_event_rows > 0 || timed_out_rows > 0 {
            "expected_key_timed_out"
        } else if failed_event_rows > 0 || error_rows > 0 {
            "expected_key_failed"
        } else if capacity_limited_event_rows > 0 {
            "queued_capacity_limited"
        } else if same_bucket_rows > 0 && same_hash_bucket_rows == 0 {
            "completed_but_hash_mismatch"
        } else if same_hash_rows > 0 {
            "completed_but_bucket_mismatch"
        } else if different_hash_rows > 0 {
            "completed_but_hash_mismatch"
        } else if expected_rows > 0 {
            "expected_key_pending"
        } else {
            "no_expected_key"
        }
        .to_string();

        Ok(NoReversalAdverseProfileDiagnostics {
            prewarmer_status,
            summary_json: serde_json::json!({
                "table": {
                    "total_rows": total_rows,
                    "same_hash_rows": same_hash_rows,
                    "different_hash_rows": different_hash_rows,
                    "same_hash_bucket_rows": same_hash_bucket_rows,
                    "same_bucket_rows": same_bucket_rows,
                    "ready_rows": ready_rows,
                    "insufficient_rows": insufficient_rows,
                    "timed_out_rows": timed_out_rows,
                    "error_rows": error_rows,
                    "latest_updated_at": latest_updated_at,
                },
                "events": {
                    "event_rows": event_rows,
                    "expected_rows": expected_rows,
                    "started_rows": started_rows,
                    "timed_out_event_rows": timed_out_event_rows,
                    "failed_event_rows": failed_event_rows,
                    "capacity_limited_event_rows": capacity_limited_event_rows,
                    "latest_expected_at": latest_expected_at,
                    "latest_started_at": latest_started_at,
                    "latest_priority": latest_priority,
                    "latest_slot_status": latest_slot_status,
                    "prewarm_age_ms": prewarm_age_ms,
                    "latest_event_at": latest_event_at,
                },
            }),
        })
    }
}

#[cfg(test)]
mod no_reversal_profile_query_tests {
    use super::*;

    #[test]
    fn adverse_move_stats_sql_uses_normalized_asset_index_filter() {
        assert!(TRADE_BUILDER_ADVERSE_MOVE_STATS_SQL.contains("WHERE asset = $1"));
        assert!(!TRADE_BUILDER_ADVERSE_MOVE_STATS_SQL.contains("LOWER(asset)"));
    }

    #[test]
    fn no_reversal_adverse_feature_sql_builds_up_and_down_rows() {
        assert!(NO_REVERSAL_ADVERSE_FEATURE_REFRESH_SQL
            .contains("INSERT INTO no_reversal_adverse_feature_rows"));
        assert!(NO_REVERSAL_ADVERSE_FEATURE_REFRESH_SQL.contains("'up'::TEXT AS direction"));
        assert!(NO_REVERSAL_ADVERSE_FEATURE_REFRESH_SQL.contains("'down'::TEXT AS direction"));
        assert!(NO_REVERSAL_ADVERSE_FEATURE_REFRESH_SQL.contains("future_min_gap"));
        assert!(NO_REVERSAL_ADVERSE_FEATURE_REFRESH_SQL.contains("slope_delta_3s"));
    }

    #[test]
    fn no_reversal_bulk_feature_stats_sql_uses_feature_index_shape() {
        assert!(TRADE_BUILDER_ADVERSE_MOVE_STATS_BULK_FEATURE_SQL
            .contains("no_reversal_adverse_feature_rows"));
        assert!(TRADE_BUILDER_ADVERSE_MOVE_STATS_BULK_FEATURE_SQL.contains("features.asset = $1"));
        assert!(
            TRADE_BUILDER_ADVERSE_MOVE_STATS_BULK_FEATURE_SQL.contains("features.direction = $2")
        );
        assert!(!TRADE_BUILDER_ADVERSE_MOVE_STATS_BULK_FEATURE_SQL
            .contains("market_price_second_snapshots"));
    }
}
