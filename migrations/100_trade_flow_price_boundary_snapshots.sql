CREATE TABLE IF NOT EXISTS trade_flow_price_boundary_snapshots (
  market_slug TEXT PRIMARY KEY,
  asset TEXT NOT NULL,
  timeframe TEXT NOT NULL,
  window_start TIMESTAMPTZ NOT NULL,
  window_end TIMESTAMPTZ NOT NULL,
  open_price DOUBLE PRECISION,
  open_ts TIMESTAMPTZ,
  high_price DOUBLE PRECISION,
  low_price DOUBLE PRECISION,
  close_price DOUBLE PRECISION,
  close_ts TIMESTAMPTZ,
  sample_count INTEGER NOT NULL DEFAULT 0,
  source TEXT NOT NULL DEFAULT 'chainlink_rtds',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT chk_trade_flow_price_boundary_sample_count
    CHECK (sample_count >= 0)
);

CREATE INDEX IF NOT EXISTS idx_trade_flow_price_boundary_asset_window
  ON trade_flow_price_boundary_snapshots (asset, timeframe, window_start DESC);
