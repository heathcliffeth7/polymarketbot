CREATE TABLE IF NOT EXISTS trade_builder_avg_rebound_pairlock_rescue_sessions (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
  flow_definition_id BIGINT REFERENCES trade_flow_definitions(id) ON DELETE SET NULL,
  flow_run_id BIGINT REFERENCES trade_flow_runs(id) ON DELETE SET NULL,
  root_flow_node_key TEXT NOT NULL,
  market_slug TEXT NOT NULL,
  mode TEXT NOT NULL DEFAULT 'avg_rebound_pairlock_rescue_v1',
  status TEXT NOT NULL DEFAULT 'BUILDING_PRIMARY',
  primary_token_id TEXT NOT NULL,
  primary_outcome_label TEXT NOT NULL,
  opposite_token_id TEXT NOT NULL,
  opposite_outcome_label TEXT NOT NULL,
  payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT chk_trade_builder_avg_rebound_status CHECK (
    status IN (
      'BUILDING_PRIMARY',
      'PROFIT_LOCKING',
      'GUARD_EXIT',
      'RESCUE_EXIT',
      'LOCKED',
      'CLOSED',
      'FAILED'
    )
  )
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_trade_builder_avg_rebound_active_session
  ON trade_builder_avg_rebound_pairlock_rescue_sessions
  (user_id, flow_definition_id, root_flow_node_key, market_slug, mode)
  WHERE status IN ('BUILDING_PRIMARY', 'PROFIT_LOCKING', 'GUARD_EXIT', 'RESCUE_EXIT', 'LOCKED');

CREATE TABLE IF NOT EXISTS trade_builder_avg_rebound_pairlock_rescue_fills (
  id BIGSERIAL PRIMARY KEY,
  session_id BIGINT NOT NULL REFERENCES trade_builder_avg_rebound_pairlock_rescue_sessions(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
  flow_definition_id BIGINT REFERENCES trade_flow_definitions(id) ON DELETE SET NULL,
  flow_run_id BIGINT REFERENCES trade_flow_runs(id) ON DELETE SET NULL,
  root_flow_node_key TEXT NOT NULL,
  market_slug TEXT NOT NULL,
  token_id TEXT NOT NULL,
  outcome_label TEXT NOT NULL,
  leg_role TEXT NOT NULL,
  intent TEXT NOT NULL,
  stage_id TEXT,
  tier_or_leg_id TEXT NOT NULL,
  decision_id TEXT NOT NULL,
  order_side TEXT NOT NULL,
  builder_order_id BIGINT NOT NULL REFERENCES trade_builder_orders(id) ON DELETE CASCADE,
  quantity DOUBLE PRECISION NOT NULL,
  execution_price DOUBLE PRECISION NOT NULL,
  notional_usdc DOUBLE PRECISION NOT NULL,
  payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT uq_trade_builder_avg_rebound_builder_order UNIQUE (builder_order_id),
  CONSTRAINT chk_trade_builder_avg_rebound_leg_role CHECK (leg_role IN ('primary', 'opposite')),
  CONSTRAINT chk_trade_builder_avg_rebound_order_side CHECK (order_side = 'buy'),
  CONSTRAINT chk_trade_builder_avg_rebound_qty CHECK (quantity >= 0),
  CONSTRAINT chk_trade_builder_avg_rebound_price CHECK (execution_price >= 0 AND execution_price <= 1),
  CONSTRAINT chk_trade_builder_avg_rebound_notional CHECK (notional_usdc >= 0)
);

CREATE INDEX IF NOT EXISTS idx_trade_builder_avg_rebound_state
  ON trade_builder_avg_rebound_pairlock_rescue_fills
  (session_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_trade_builder_avg_rebound_decision
  ON trade_builder_avg_rebound_pairlock_rescue_fills
  (session_id, intent, stage_id, tier_or_leg_id);
