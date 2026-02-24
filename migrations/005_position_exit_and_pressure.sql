CREATE TABLE IF NOT EXISTS position_exit_rules (
  id BIGSERIAL PRIMARY KEY,
  trade_id BIGINT NOT NULL REFERENCES trades(id) ON DELETE CASCADE,
  leg_side TEXT NOT NULL,
  drop_sell_pct DOUBLE PRECISION NOT NULL,
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE(trade_id, leg_side),
  CONSTRAINT chk_position_exit_leg_side CHECK (leg_side IN ('yes', 'no')),
  CONSTRAINT chk_position_exit_drop_pct CHECK (drop_sell_pct > 0 AND drop_sell_pct <= 100)
);

CREATE INDEX IF NOT EXISTS idx_position_exit_rules_trade_id
  ON position_exit_rules(trade_id);

CREATE TABLE IF NOT EXISTS pressure_snapshots (
  trade_id BIGINT PRIMARY KEY REFERENCES trades(id) ON DELETE CASCADE,
  pressure_score DOUBLE PRECISION NOT NULL DEFAULT 0,
  bid_ask_imbalance DOUBLE PRECISION,
  sell_ratio DOUBLE PRECISION,
  yes_price DOUBLE PRECISION,
  no_price DOUBLE PRECISION,
  trigger_reason TEXT,
  triggered BOOLEAN NOT NULL DEFAULT FALSE,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pressure_snapshots_updated_at
  ON pressure_snapshots(updated_at DESC);
