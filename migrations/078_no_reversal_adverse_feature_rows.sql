CREATE TABLE IF NOT EXISTS no_reversal_adverse_feature_rows (
  market_slug TEXT NOT NULL,
  direction TEXT NOT NULL,
  second_ts TIMESTAMPTZ NOT NULL,
  asset TEXT NOT NULL,
  window_start TIMESTAMPTZ NOT NULL,
  window_end TIMESTAMPTZ NOT NULL,
  remaining_sec DOUBLE PRECISION NOT NULL,
  entry_ask DOUBLE PRECISION,
  directional_gap DOUBLE PRECISION NOT NULL,
  slope_delta_3s DOUBLE PRECISION,
  slope_bucket TEXT NOT NULL,
  adverse_move DOUBLE PRECISION NOT NULL,
  computed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (market_slug, direction, second_ts),
  CONSTRAINT chk_no_reversal_adverse_feature_direction
    CHECK (direction IN ('up', 'down')),
  CONSTRAINT chk_no_reversal_adverse_feature_slope_bucket
    CHECK (slope_bucket IN ('negative', 'non_negative', 'unknown'))
);

CREATE INDEX IF NOT EXISTS idx_no_reversal_adverse_feature_asset_direction_second
  ON no_reversal_adverse_feature_rows (asset, direction, second_ts)
  INCLUDE (
    market_slug,
    remaining_sec,
    entry_ask,
    directional_gap,
    slope_bucket,
    adverse_move
  );

CREATE INDEX IF NOT EXISTS idx_no_reversal_adverse_feature_market
  ON no_reversal_adverse_feature_rows (market_slug, direction, second_ts);
