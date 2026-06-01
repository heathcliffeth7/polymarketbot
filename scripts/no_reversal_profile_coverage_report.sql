WITH profile_events AS (
  SELECT
    payload_json->>'target_market_slug' AS target_market_slug,
    (payload_json->>'target_window_start')::TIMESTAMPTZ AS target_window_start,
    definition_id,
    payload_json->>'node_key' AS node_key,
    payload_json->>'profile_config_hash' AS profile_config_hash,
    LOWER(payload_json #>> '{profile_lookup_key,asset}') AS asset,
    LOWER(payload_json #>> '{profile_lookup_key,direction}') AS direction,
    payload_json #>> '{profile_lookup_key,remaining_bucket}' AS remaining_bucket,
    payload_json #>> '{profile_lookup_key,price_bucket}' AS price_bucket,
    payload_json #>> '{profile_lookup_key,gap_bucket}' AS gap_bucket,
    payload_json #>> '{profile_lookup_key,slope_bucket}' AS slope_bucket,
    (payload_json #>> '{profile_lookup_key,quantile}')::DOUBLE PRECISION AS quantile,
    (payload_json #>> '{profile_lookup_key,high_late}')::BOOLEAN AS high_late,
    COALESCE(payload_json->>'priority', 'unknown') AS priority,
    event_type,
    payload_json->>'status' AS status
  FROM trade_flow_events
  WHERE event_type LIKE 'no_reversal_profile%'
    AND created_at >= NOW() - INTERVAL '2 hours'
    AND payload_json ? 'profile_lookup_key'
),
expected AS (
  SELECT
    target_market_slug,
    target_window_start,
    definition_id,
    node_key,
    profile_config_hash,
    asset,
    direction,
    remaining_bucket,
    price_bucket,
    gap_bucket,
    slope_bucket,
    quantile,
    high_late,
    priority,
    COUNT(*) FILTER (WHERE event_type = 'no_reversal_profile_expected_key') AS expected_events,
    COUNT(*) FILTER (WHERE event_type = 'no_reversal_profile_prewarm_started') AS started_events,
    COUNT(*) FILTER (WHERE event_type = 'no_reversal_profile_capacity_limited') AS capacity_events
  FROM profile_events
  GROUP BY
    target_market_slug,
    target_window_start,
    definition_id,
    node_key,
    profile_config_hash,
    asset,
    direction,
    remaining_bucket,
    price_bucket,
    gap_bucket,
    slope_bucket,
    quantile,
    high_late,
    priority
),
joined AS (
  SELECT
    e.target_market_slug,
    e.target_window_start,
    e.definition_id,
    e.node_key,
    e.profile_config_hash,
    e.asset,
    e.direction,
    e.priority,
    e.started_events,
    e.capacity_events,
    p.status
  FROM expected e
  LEFT JOIN no_reversal_adverse_profiles p
    ON p.target_market_slug = e.target_market_slug
   AND p.target_window_start = e.target_window_start
   AND p.definition_id = e.definition_id
   AND p.node_key = e.node_key
   AND p.profile_config_hash = e.profile_config_hash
   AND LOWER(p.asset) = e.asset
   AND LOWER(p.direction) = e.direction
   AND p.remaining_bucket = e.remaining_bucket
   AND p.price_bucket = e.price_bucket
   AND p.gap_bucket = e.gap_bucket
   AND p.slope_bucket = e.slope_bucket
   AND p.quantile = e.quantile
   AND p.high_late = e.high_late
)
SELECT
  target_window_start,
  target_market_slug,
  definition_id,
  node_key,
  profile_config_hash,
  asset,
  direction,
  priority,
  COUNT(*) AS expected_profile_rows,
  COUNT(*) FILTER (WHERE started_events > 0) AS started_rows,
  COUNT(*) FILTER (WHERE status = 'ready') AS ready_rows,
  COUNT(*) FILTER (WHERE status = 'insufficient') AS insufficient_rows,
  COUNT(*) FILTER (WHERE status = 'timed_out') AS timed_out_rows,
  COUNT(*) FILTER (WHERE status = 'error') AS error_rows,
  COUNT(*) FILTER (WHERE capacity_events > 0 AND started_events = 0) AS queued_capacity_limited_rows,
  COUNT(*) FILTER (WHERE status IS NULL) AS missing_rows,
  ROUND(
    100.0 * COUNT(*) FILTER (WHERE status = 'ready') / NULLIF(COUNT(*), 0),
    1
  ) AS ready_coverage_pct
FROM joined
GROUP BY
  target_window_start,
  target_market_slug,
  definition_id,
  node_key,
  profile_config_hash,
  asset,
  direction,
  priority
ORDER BY target_window_start DESC, node_key, asset, direction, priority;
