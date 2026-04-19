CREATE TABLE IF NOT EXISTS trade_builder_pair_sessions (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
  flow_definition_id BIGINT REFERENCES trade_flow_definitions(id) ON DELETE SET NULL,
  flow_run_id BIGINT REFERENCES trade_flow_runs(id) ON DELETE SET NULL,
  flow_node_key TEXT,
  market_slug TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'working',
  pair_target_total_cent DOUBLE PRECISION NOT NULL,
  min_net_profit_usdc DOUBLE PRECISION NOT NULL,
  profit_safety_buffer_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
  orphan_grace_ms BIGINT NOT NULL DEFAULT 1500,
  notify_on_pair_locked BOOLEAN NOT NULL DEFAULT FALSE,
  notify_on_pair_unwind BOOLEAN NOT NULL DEFAULT FALSE,
  notify_on_pair_no_edge BOOLEAN NOT NULL DEFAULT FALSE,
  primary_order_id BIGINT,
  counter_order_id BIGINT,
  lead_order_id BIGINT,
  primary_fill_qty DOUBLE PRECISION,
  primary_fill_fee_qty DOUBLE PRECISION,
  primary_net_qty DOUBLE PRECISION,
  primary_avg_fill_price DOUBLE PRECISION,
  counter_fill_qty DOUBLE PRECISION,
  counter_fill_fee_qty DOUBLE PRECISION,
  counter_net_qty DOUBLE PRECISION,
  counter_avg_fill_price DOUBLE PRECISION,
  lead_filled_at TIMESTAMPTZ,
  locked_qty DOUBLE PRECISION,
  projected_net_profit_usdc DOUBLE PRECISION,
  last_error TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT chk_trade_builder_pair_sessions_status
    CHECK (status = ANY (ARRAY['working', 'locked', 'unwinding', 'completed', 'expired', 'error'])),
  CONSTRAINT chk_trade_builder_pair_sessions_pair_target_total_cent
    CHECK (pair_target_total_cent > 0 AND pair_target_total_cent < 100),
  CONSTRAINT chk_trade_builder_pair_sessions_min_net_profit
    CHECK (min_net_profit_usdc >= 0),
  CONSTRAINT chk_trade_builder_pair_sessions_profit_safety_buffer
    CHECK (profit_safety_buffer_usdc >= 0),
  CONSTRAINT chk_trade_builder_pair_sessions_orphan_grace
    CHECK (orphan_grace_ms >= 0)
);

CREATE INDEX IF NOT EXISTS idx_trade_builder_pair_sessions_status_updated
  ON trade_builder_pair_sessions (status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_trade_builder_pair_sessions_flow_run
  ON trade_builder_pair_sessions (flow_run_id, flow_node_key, status);

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS pair_session_id BIGINT REFERENCES trade_builder_pair_sessions(id) ON DELETE SET NULL,
  ADD COLUMN IF NOT EXISTS pair_leg_role TEXT;

ALTER TABLE trade_builder_orders
  DROP CONSTRAINT IF EXISTS chk_trade_builder_pair_leg_role;

ALTER TABLE trade_builder_orders
  ADD CONSTRAINT chk_trade_builder_pair_leg_role
  CHECK (
    pair_leg_role IS NULL
    OR pair_leg_role = ANY (ARRAY['lead_candidate', 'counter_candidate', 'completion_buy', 'orphan_unwind_sell'])
  );

CREATE INDEX IF NOT EXISTS idx_trade_builder_orders_pair_session
  ON trade_builder_orders (pair_session_id, pair_leg_role, status);
