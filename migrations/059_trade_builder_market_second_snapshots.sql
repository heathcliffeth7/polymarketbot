ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS ptb_stop_loss_time_decay_mode TEXT;

CREATE TABLE IF NOT EXISTS market_price_second_snapshots (
  id BIGSERIAL PRIMARY KEY,
  market_slug TEXT NOT NULL,
  asset TEXT NOT NULL,
  window_start TIMESTAMPTZ NOT NULL,
  window_end TIMESTAMPTZ NOT NULL,
  second_ts TIMESTAMPTZ NOT NULL,
  ptb_ref_price DOUBLE PRECISION,
  chainlink_price DOUBLE PRECISION,
  yes_best_bid DOUBLE PRECISION,
  yes_best_ask DOUBLE PRECISION,
  yes_ask_depth_usdc DOUBLE PRECISION,
  no_best_bid DOUBLE PRECISION,
  no_best_ask DOUBLE PRECISION,
  no_ask_depth_usdc DOUBLE PRECISION,
  sample_count INTEGER NOT NULL DEFAULT 1,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (market_slug, second_ts)
);

CREATE INDEX IF NOT EXISTS idx_market_price_second_snapshots_market_second
  ON market_price_second_snapshots (market_slug, second_ts);
