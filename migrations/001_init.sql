CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS markets (
  id BIGSERIAL PRIMARY KEY,
  market_slug TEXT UNIQUE NOT NULL,
  starts_at TIMESTAMPTZ NOT NULL,
  ends_at TIMESTAMPTZ NOT NULL,
  status TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS trades (
  id BIGSERIAL PRIMARY KEY,
  market_id BIGINT NOT NULL REFERENCES markets(id),
  state TEXT NOT NULL,
  entry_price DOUBLE PRECISION,
  exit_price DOUBLE PRECISION,
  notional_usdc DOUBLE PRECISION NOT NULL,
  realized_pnl DOUBLE PRECISION,
  opened_at TIMESTAMPTZ,
  closed_at TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS orders (
  id BIGSERIAL PRIMARY KEY,
  trade_id BIGINT NOT NULL REFERENCES trades(id),
  exchange_order_id TEXT UNIQUE NOT NULL,
  intent TEXT NOT NULL,
  side TEXT NOT NULL,
  price DOUBLE PRECISION NOT NULL,
  size DOUBLE PRECISION NOT NULL,
  status TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS fills (
  id BIGSERIAL PRIMARY KEY,
  order_id BIGINT NOT NULL REFERENCES orders(id),
  fill_id TEXT UNIQUE NOT NULL,
  price DOUBLE PRECISION NOT NULL,
  size DOUBLE PRECISION NOT NULL,
  fee DOUBLE PRECISION NOT NULL,
  filled_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS positions (
  id BIGSERIAL PRIMARY KEY,
  trade_id BIGINT NOT NULL REFERENCES trades(id),
  token_id TEXT,
  qty DOUBLE PRECISION,
  avg_price DOUBLE PRECISION,
  status TEXT
);

CREATE TABLE IF NOT EXISTS risk_events (
  id BIGSERIAL PRIMARY KEY,
  trade_id BIGINT REFERENCES trades(id),
  event_type TEXT NOT NULL,
  decision TEXT NOT NULL,
  details TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS bot_runs (
  id BIGSERIAL PRIMARY KEY,
  mode TEXT NOT NULL,
  version TEXT NOT NULL,
  started_at TIMESTAMPTZ NOT NULL,
  stopped_at TIMESTAMPTZ,
  reason TEXT
);

CREATE TABLE IF NOT EXISTS config_snapshots (
  id BIGSERIAL PRIMARY KEY,
  run_id BIGINT NOT NULL REFERENCES bot_runs(id),
  config_hash TEXT NOT NULL,
  payload_json JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_orders_trade_status ON orders(trade_id, status);
CREATE INDEX IF NOT EXISTS idx_fills_order_time ON fills(order_id, filled_at);
CREATE INDEX IF NOT EXISTS idx_trades_market_state ON trades(market_id, state);
CREATE INDEX IF NOT EXISTS idx_risk_events_time_type ON risk_events(created_at, event_type);
