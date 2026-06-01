#!/usr/bin/env bash
set -euo pipefail

ASSETS="btc,eth,sol"
LOOKBACK="14 days"
DRY_RUN=0
SAFETY_DELAY="90 seconds"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --assets)
      ASSETS="${2:-}"
      shift 2
      ;;
    --lookback)
      LOOKBACK="${2:-}"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --safety-delay)
      SAFETY_DELAY="${2:-}"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "DATABASE_URL is required" >&2
  exit 2
fi

IFS=',' read -r -a ASSET_LIST <<< "$ASSETS"

for raw_asset in "${ASSET_LIST[@]}"; do
  asset="$(echo "$raw_asset" | tr '[:upper:]' '[:lower:]' | xargs)"
  [[ -n "$asset" ]] || continue
  echo "no-reversal adverse feature backfill: asset=$asset lookback=$LOOKBACK dry_run=$DRY_RUN"

  if [[ "$DRY_RUN" -eq 1 ]]; then
    psql "$DATABASE_URL" -X -v ON_ERROR_STOP=1 \
      -v asset="$asset" \
      -v lookback="$LOOKBACK" \
      -v safety_delay="$SAFETY_DELAY" <<'SQL'
SELECT
  :'asset' AS asset,
  COUNT(DISTINCT market_slug) AS source_markets,
  COUNT(*) AS source_rows
FROM market_price_second_snapshots
WHERE asset = :'asset'
  AND window_end >= NOW() - (:'lookback')::INTERVAL
  AND window_end <= NOW() - (:'safety_delay')::INTERVAL
  AND second_ts <= window_end
  AND ptb_ref_price IS NOT NULL
  AND chainlink_price IS NOT NULL;
SQL
    continue
  fi

  psql "$DATABASE_URL" -X -v ON_ERROR_STOP=1 \
    -v asset="$asset" \
    -v lookback="$LOOKBACK" \
    -v safety_delay="$SAFETY_DELAY" <<'SQL'
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
  WHERE asset = :'asset'
    AND window_end >= NOW() - (:'lookback')::INTERVAL
    AND window_end <= NOW() - (:'safety_delay')::INTERVAL
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
), upserted AS (
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
  RETURNING 1
)
SELECT :'asset' AS asset, COUNT(*) AS affected_feature_rows
FROM upserted;
SQL
done
