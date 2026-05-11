ALTER TABLE trades ADD COLUMN IF NOT EXISTS strategy_mode TEXT;
ALTER TABLE trades ADD COLUMN IF NOT EXISTS basket_tp DOUBLE PRECISION;
ALTER TABLE trades ADD COLUMN IF NOT EXISTS basket_sl DOUBLE PRECISION;

ALTER TABLE orders ADD COLUMN IF NOT EXISTS leg_side TEXT;
ALTER TABLE orders ADD COLUMN IF NOT EXISTS token_id TEXT;

CREATE INDEX IF NOT EXISTS idx_orders_trade_leg_side ON orders(trade_id, leg_side);
CREATE INDEX IF NOT EXISTS idx_orders_token_id ON orders(token_id);

CREATE TABLE IF NOT EXISTS leg_positions (
  id BIGSERIAL PRIMARY KEY,
  trade_id BIGINT NOT NULL REFERENCES trades(id),
  leg_side TEXT NOT NULL,
  token_id TEXT NOT NULL,
  qty DOUBLE PRECISION NOT NULL,
  avg_entry DOUBLE PRECISION NOT NULL,
  levels_filled INTEGER NOT NULL DEFAULT 0,
  last_fill_price DOUBLE PRECISION,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(trade_id, leg_side)
);

CREATE INDEX IF NOT EXISTS idx_leg_positions_trade ON leg_positions(trade_id, updated_at);
