CREATE TABLE IF NOT EXISTS trade_flow_dual_dca_jobs (
  id BIGSERIAL PRIMARY KEY,
  flow_run_id BIGINT NOT NULL REFERENCES trade_flow_runs(id) ON DELETE CASCADE,
  flow_definition_id BIGINT NOT NULL REFERENCES trade_flow_definitions(id) ON DELETE CASCADE,
  flow_version_id BIGINT REFERENCES trade_flow_versions(id) ON DELETE SET NULL,
  node_key TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  source_trade_id BIGINT REFERENCES trades(id) ON DELETE SET NULL,
  market_asset TEXT NOT NULL,
  market_timeframe TEXT NOT NULL,
  side_mode TEXT NOT NULL DEFAULT 'all',
  base_sizing TEXT NOT NULL DEFAULT 'shares',
  base_shares DOUBLE PRECISION,
  base_usdc DOUBLE PRECISION,
  base_price_usdc DOUBLE PRECISION,
  dca_levels INTEGER NOT NULL DEFAULT 1,
  near_step DOUBLE PRECISION NOT NULL DEFAULT 0.05,
  step_mult DOUBLE PRECISION NOT NULL DEFAULT 1.0,
  size_mult DOUBLE PRECISION NOT NULL DEFAULT 1.0,
  min_price_distance_cent DOUBLE PRECISION NOT NULL DEFAULT 1.0,
  cutoff_min INTEGER NOT NULL DEFAULT 3,
  tp_profit_pct DOUBLE PRECISION NOT NULL DEFAULT 0,
  sl_loss_pct DOUBLE PRECISION NOT NULL DEFAULT 0,
  sl_spread_pct DOUBLE PRECISION NOT NULL DEFAULT 0,
  last_market_slug TEXT,
  last_market_started_at TIMESTAMPTZ,
  last_market_ends_at TIMESTAMPTZ,
  next_check_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_order_count INTEGER NOT NULL DEFAULT 0,
  last_error TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT uq_tf_dual_dca_job UNIQUE (flow_run_id, node_key),
  CONSTRAINT chk_tf_dual_dca_status CHECK (status IN ('active', 'paused', 'completed', 'canceled', 'error')),
  CONSTRAINT chk_tf_dual_dca_asset CHECK (market_asset IN ('btc', 'eth', 'sol', 'xrp')),
  CONSTRAINT chk_tf_dual_dca_timeframe CHECK (market_timeframe IN ('5m', '15m')),
  CONSTRAINT chk_tf_dual_dca_side_mode CHECK (side_mode IN ('up', 'down', 'all')),
  CONSTRAINT chk_tf_dual_dca_base_sizing CHECK (base_sizing IN ('shares', 'usdc')),
  CONSTRAINT chk_tf_dual_dca_base_shares CHECK (base_shares IS NULL OR base_shares > 0),
  CONSTRAINT chk_tf_dual_dca_base_usdc CHECK (base_usdc IS NULL OR base_usdc > 0),
  CONSTRAINT chk_tf_dual_dca_base_price CHECK (
    base_price_usdc IS NULL OR (base_price_usdc >= 0.01 AND base_price_usdc <= 0.99)
  ),
  CONSTRAINT chk_tf_dual_dca_levels CHECK (dca_levels BETWEEN 1 AND 20),
  CONSTRAINT chk_tf_dual_dca_near_step CHECK (near_step > 0 AND near_step < 1),
  CONSTRAINT chk_tf_dual_dca_step_mult CHECK (step_mult >= 1),
  CONSTRAINT chk_tf_dual_dca_size_mult CHECK (size_mult > 0),
  CONSTRAINT chk_tf_dual_dca_min_distance CHECK (min_price_distance_cent > 0),
  CONSTRAINT chk_tf_dual_dca_cutoff CHECK (cutoff_min >= 0),
  CONSTRAINT chk_tf_dual_dca_order_count CHECK (created_order_count >= 0),
  CONSTRAINT chk_tf_dual_dca_risk_tp CHECK (tp_profit_pct >= 0),
  CONSTRAINT chk_tf_dual_dca_risk_sl CHECK (sl_loss_pct >= 0),
  CONSTRAINT chk_tf_dual_dca_risk_spread CHECK (sl_spread_pct >= 0)
);

CREATE INDEX IF NOT EXISTS idx_tf_dual_dca_jobs_status_next
  ON trade_flow_dual_dca_jobs(status, next_check_at, id);

CREATE INDEX IF NOT EXISTS idx_tf_dual_dca_jobs_run_node
  ON trade_flow_dual_dca_jobs(flow_run_id, node_key);

CREATE TABLE IF NOT EXISTS trade_flow_dual_dca_legs (
  id BIGSERIAL PRIMARY KEY,
  job_id BIGINT NOT NULL REFERENCES trade_flow_dual_dca_jobs(id) ON DELETE CASCADE,
  market_slug TEXT NOT NULL,
  token_id TEXT NOT NULL,
  outcome_label TEXT NOT NULL,
  side TEXT NOT NULL DEFAULT 'buy',
  level_index INTEGER NOT NULL,
  trigger_condition TEXT,
  trigger_price DOUBLE PRECISION,
  size_usdc DOUBLE PRECISION NOT NULL,
  reference_price DOUBLE PRECISION,
  builder_order_id BIGINT REFERENCES trade_builder_orders(id) ON DELETE SET NULL,
  status TEXT NOT NULL DEFAULT 'created',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT uq_tf_dual_dca_leg UNIQUE (job_id, market_slug, outcome_label, level_index),
  CONSTRAINT chk_tf_dual_dca_leg_side CHECK (side IN ('buy', 'sell')),
  CONSTRAINT chk_tf_dual_dca_leg_level CHECK (level_index >= 0),
  CONSTRAINT chk_tf_dual_dca_leg_trigger_condition CHECK (
    trigger_condition IS NULL OR trigger_condition IN ('cross_above', 'cross_below')
  ),
  CONSTRAINT chk_tf_dual_dca_leg_trigger_price CHECK (
    trigger_price IS NULL OR (trigger_price >= 0.01 AND trigger_price <= 0.99)
  ),
  CONSTRAINT chk_tf_dual_dca_leg_size CHECK (size_usdc > 0),
  CONSTRAINT chk_tf_dual_dca_leg_reference CHECK (
    reference_price IS NULL OR (reference_price >= 0.01 AND reference_price <= 0.99)
  ),
  CONSTRAINT chk_tf_dual_dca_leg_status CHECK (status IN ('created', 'submitted', 'completed', 'canceled', 'error'))
);

CREATE INDEX IF NOT EXISTS idx_tf_dual_dca_legs_job
  ON trade_flow_dual_dca_legs(job_id, level_index);

CREATE INDEX IF NOT EXISTS idx_tf_dual_dca_legs_builder_order
  ON trade_flow_dual_dca_legs(builder_order_id);

CREATE TABLE IF NOT EXISTS trade_flow_dual_dca_events (
  id BIGSERIAL PRIMARY KEY,
  job_id BIGINT NOT NULL REFERENCES trade_flow_dual_dca_jobs(id) ON DELETE CASCADE,
  leg_id BIGINT REFERENCES trade_flow_dual_dca_legs(id) ON DELETE SET NULL,
  event_type TEXT NOT NULL,
  payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_tf_dual_dca_events_job_time
  ON trade_flow_dual_dca_events(job_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_tf_dual_dca_events_leg_time
  ON trade_flow_dual_dca_events(leg_id, created_at DESC);
