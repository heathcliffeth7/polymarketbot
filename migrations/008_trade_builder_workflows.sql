CREATE TABLE IF NOT EXISTS trade_builder_workflows (
  id BIGSERIAL PRIMARY KEY,
  name TEXT NOT NULL DEFAULT 'workflow',
  status TEXT NOT NULL DEFAULT 'armed',
  source_trade_id BIGINT NOT NULL REFERENCES trades(id) ON DELETE CASCADE,
  sell_target_pct DOUBLE PRECISION NOT NULL,
  buy_start_after_sell_progress_pct DOUBLE PRECISION NOT NULL DEFAULT 0,
  buy_trigger_mode TEXT NOT NULL DEFAULT 'sell_progress_and_price',
  buy_allocation_pct DOUBLE PRECISION NOT NULL DEFAULT 100,
  expires_at TIMESTAMPTZ,
  last_error TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT chk_tb_workflow_status CHECK (status IN (
    'draft',
    'armed',
    'running',
    'completed',
    'canceled',
    'expired',
    'error'
  )),
  CONSTRAINT chk_tb_workflow_sell_target CHECK (sell_target_pct > 0 AND sell_target_pct <= 100),
  CONSTRAINT chk_tb_workflow_buy_start CHECK (
    buy_start_after_sell_progress_pct >= 0
    AND buy_start_after_sell_progress_pct <= 100
  ),
  CONSTRAINT chk_tb_workflow_trigger_mode CHECK (buy_trigger_mode IN (
    'sell_progress_only',
    'price_only',
    'sell_progress_and_price'
  )),
  CONSTRAINT chk_tb_workflow_buy_alloc CHECK (buy_allocation_pct > 0 AND buy_allocation_pct <= 100)
);

CREATE INDEX IF NOT EXISTS idx_tb_workflows_status_updated
  ON trade_builder_workflows(status, updated_at DESC);

CREATE TABLE IF NOT EXISTS trade_builder_workflow_legs (
  id BIGSERIAL PRIMARY KEY,
  workflow_id BIGINT NOT NULL REFERENCES trade_builder_workflows(id) ON DELETE CASCADE,
  leg_type TEXT NOT NULL,
  market_slug TEXT NOT NULL,
  token_id TEXT NOT NULL,
  outcome_label TEXT NOT NULL,
  side TEXT NOT NULL,
  trigger_condition TEXT,
  trigger_price DOUBLE PRECISION,
  min_price_distance_cent DOUBLE PRECISION NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  builder_order_id BIGINT REFERENCES trade_builder_orders(id) ON DELETE SET NULL,
  target_notional_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
  allocated_notional_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
  last_seen_price DOUBLE PRECISION,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT uq_tb_workflow_leg UNIQUE (workflow_id, leg_type),
  CONSTRAINT chk_tb_workflow_leg_type CHECK (leg_type IN ('sell', 'buy')),
  CONSTRAINT chk_tb_workflow_leg_side CHECK (side IN ('buy', 'sell')),
  CONSTRAINT chk_tb_workflow_leg_trigger_condition CHECK (
    trigger_condition IS NULL OR trigger_condition IN ('cross_above', 'cross_below')
  ),
  CONSTRAINT chk_tb_workflow_leg_trigger_price CHECK (
    trigger_price IS NULL OR (trigger_price >= 0 AND trigger_price <= 1)
  ),
  CONSTRAINT chk_tb_workflow_leg_min_dist CHECK (min_price_distance_cent > 0),
  CONSTRAINT chk_tb_workflow_leg_status CHECK (status IN (
    'pending',
    'armed',
    'waiting_sell_progress',
    'open',
    'partially_filled',
    'completed',
    'blocked',
    'canceled',
    'expired',
    'error'
  )),
  CONSTRAINT chk_tb_workflow_leg_target_usdc CHECK (target_notional_usdc >= 0),
  CONSTRAINT chk_tb_workflow_leg_alloc_usdc CHECK (allocated_notional_usdc >= 0)
);

CREATE INDEX IF NOT EXISTS idx_tb_workflow_legs_workflow
  ON trade_builder_workflow_legs(workflow_id, leg_type);

CREATE INDEX IF NOT EXISTS idx_tb_workflow_legs_builder_order
  ON trade_builder_workflow_legs(builder_order_id);

CREATE TABLE IF NOT EXISTS trade_builder_workflow_events (
  id BIGSERIAL PRIMARY KEY,
  workflow_id BIGINT NOT NULL REFERENCES trade_builder_workflows(id) ON DELETE CASCADE,
  leg_id BIGINT REFERENCES trade_builder_workflow_legs(id) ON DELETE CASCADE,
  event_type TEXT NOT NULL,
  payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_tb_workflow_events_flow_time
  ON trade_builder_workflow_events(workflow_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_tb_workflow_events_leg_time
  ON trade_builder_workflow_events(leg_id, created_at DESC);
