CREATE TABLE IF NOT EXISTS no_reversal_adverse_profiles (
  id BIGSERIAL PRIMARY KEY,
  target_market_slug TEXT NOT NULL,
  target_window_start TIMESTAMPTZ NOT NULL,
  definition_id BIGINT NOT NULL,
  node_key TEXT NOT NULL,
  profile_config_hash TEXT NOT NULL,
  asset TEXT NOT NULL,
  direction TEXT NOT NULL,
  remaining_bucket TEXT NOT NULL,
  price_bucket TEXT NOT NULL,
  gap_bucket TEXT NOT NULL,
  slope_bucket TEXT NOT NULL,
  quantile DOUBLE PRECISION NOT NULL,
  high_late BOOLEAN NOT NULL,
  status TEXT NOT NULL,
  selected_adverse DOUBLE PRECISION,
  raw_selected_adverse DOUBLE PRECISION,
  fallback_level TEXT,
  lookbacks_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  sample_count BIGINT NOT NULL DEFAULT 0,
  market_count BIGINT NOT NULL DEFAULT 0,
  profile_as_of TIMESTAMPTZ NOT NULL,
  error TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT chk_no_reversal_adverse_profiles_status
    CHECK (status IN ('ready', 'insufficient', 'error', 'stale')),
  CONSTRAINT chk_no_reversal_adverse_profiles_quantile
    CHECK (quantile >= 0.0 AND quantile <= 1.0),
  CONSTRAINT chk_no_reversal_adverse_profiles_ready_values
    CHECK (
      status <> 'ready'
      OR (
        selected_adverse IS NOT NULL
        AND raw_selected_adverse IS NOT NULL
        AND fallback_level IS NOT NULL
      )
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_no_reversal_adverse_profiles_key
  ON no_reversal_adverse_profiles (
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
    high_late
  );

CREATE INDEX IF NOT EXISTS idx_no_reversal_adverse_profiles_lookup
  ON no_reversal_adverse_profiles (
    target_market_slug,
    definition_id,
    node_key,
    asset,
    direction,
    updated_at DESC
  );
