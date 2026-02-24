CREATE TABLE IF NOT EXISTS trade_builder_orders (
  id BIGSERIAL PRIMARY KEY,
  trade_id BIGINT NOT NULL REFERENCES trades(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  status TEXT NOT NULL,
  market_slug TEXT NOT NULL,
  token_id TEXT NOT NULL,
  outcome_label TEXT NOT NULL,
  side TEXT NOT NULL,
  trigger_condition TEXT,
  trigger_price DOUBLE PRECISION,
  size_usdc DOUBLE PRECISION NOT NULL,
  min_price_distance_cent DOUBLE PRECISION NOT NULL,
  expires_at TIMESTAMPTZ,
  max_triggers INTEGER NOT NULL DEFAULT 3,
  triggers_fired INTEGER NOT NULL DEFAULT 0,
  active_exchange_order_id TEXT,
  remaining_size DOUBLE PRECISION,
  working_price DOUBLE PRECISION,
  last_seen_price DOUBLE PRECISION,
  last_error TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT chk_trade_builder_kind CHECK (kind IN ('immediate', 'conditional')),
  CONSTRAINT chk_trade_builder_status CHECK (status IN (
    'pending',
    'armed',
    'triggered',
    'open',
    'partially_filled',
    'filled',
    'canceled_requested',
    'completed',
    'canceled',
    'expired',
    'blocked',
    'error'
  )),
  CONSTRAINT chk_trade_builder_side CHECK (side IN ('buy', 'sell')),
  CONSTRAINT chk_trade_builder_trigger_condition CHECK (
    trigger_condition IS NULL OR trigger_condition IN ('cross_above', 'cross_below')
  ),
  CONSTRAINT chk_trade_builder_trigger_price CHECK (
    trigger_price IS NULL OR (trigger_price >= 0 AND trigger_price <= 1)
  ),
  CONSTRAINT chk_trade_builder_size_usdc CHECK (size_usdc > 0),
  CONSTRAINT chk_trade_builder_min_distance CHECK (min_price_distance_cent > 0),
  CONSTRAINT chk_trade_builder_triggers CHECK (max_triggers BETWEEN 1 AND 20),
  CONSTRAINT chk_trade_builder_triggers_fired CHECK (triggers_fired >= 0)
);

CREATE INDEX IF NOT EXISTS idx_trade_builder_orders_status
  ON trade_builder_orders(status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_trade_builder_orders_expiry
  ON trade_builder_orders(expires_at);

CREATE INDEX IF NOT EXISTS idx_trade_builder_orders_market
  ON trade_builder_orders(market_slug, token_id);
