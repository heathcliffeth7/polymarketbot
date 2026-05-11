CREATE TABLE IF NOT EXISTS market_trade_ticks (
  id BIGSERIAL PRIMARY KEY,
  market_slug TEXT NOT NULL,
  asset TEXT NOT NULL,
  window_start TIMESTAMPTZ NOT NULL,
  window_end TIMESTAMPTZ NOT NULL,
  token_id TEXT NOT NULL,
  outcome_side TEXT NOT NULL,
  event_ts TIMESTAMPTZ NOT NULL,
  price DOUBLE PRECISION NOT NULL,
  size DOUBLE PRECISION NOT NULL,
  notional_usdc DOUBLE PRECISION NOT NULL,
  side TEXT,
  dedupe_key TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (dedupe_key)
);

CREATE INDEX IF NOT EXISTS idx_market_trade_ticks_market_event
  ON market_trade_ticks (market_slug, event_ts DESC);

CREATE INDEX IF NOT EXISTS idx_market_trade_ticks_asset_event
  ON market_trade_ticks (asset, event_ts DESC);

CREATE INDEX IF NOT EXISTS idx_market_trade_ticks_asset_window
  ON market_trade_ticks (asset, window_end, event_ts);
