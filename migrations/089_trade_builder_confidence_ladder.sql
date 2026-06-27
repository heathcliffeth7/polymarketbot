CREATE TABLE IF NOT EXISTS trade_builder_confidence_ladder_fills (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
  flow_definition_id BIGINT REFERENCES trade_flow_definitions(id) ON DELETE SET NULL,
  flow_run_id BIGINT REFERENCES trade_flow_runs(id) ON DELETE SET NULL,
  root_flow_node_key TEXT NOT NULL,
  market_slug TEXT NOT NULL,
  token_id TEXT NOT NULL,
  outcome_label TEXT NOT NULL,
  ladder_side TEXT NOT NULL,
  intent TEXT NOT NULL,
  order_side TEXT NOT NULL,
  builder_order_id BIGINT NOT NULL REFERENCES trade_builder_orders(id) ON DELETE CASCADE,
  parent_builder_order_id BIGINT REFERENCES trade_builder_orders(id) ON DELETE SET NULL,
  quantity DOUBLE PRECISION NOT NULL,
  execution_price DOUBLE PRECISION NOT NULL,
  notional_usdc DOUBLE PRECISION NOT NULL,
  model_probability DOUBLE PRECISION,
  edge DOUBLE PRECISION,
  payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT uq_trade_builder_confidence_ladder_builder_order UNIQUE (builder_order_id),
  CONSTRAINT chk_trade_builder_confidence_ladder_side CHECK (ladder_side IN ('up', 'down')),
  CONSTRAINT chk_trade_builder_confidence_ladder_order_side CHECK (order_side = 'buy'),
  CONSTRAINT chk_trade_builder_confidence_ladder_qty CHECK (quantity >= 0),
  CONSTRAINT chk_trade_builder_confidence_ladder_price CHECK (execution_price >= 0 AND execution_price <= 1),
  CONSTRAINT chk_trade_builder_confidence_ladder_notional CHECK (notional_usdc >= 0),
  CONSTRAINT chk_trade_builder_confidence_ladder_probability CHECK (model_probability IS NULL OR (model_probability >= 0 AND model_probability <= 1))
);

CREATE INDEX IF NOT EXISTS idx_trade_builder_confidence_ladder_state
  ON trade_builder_confidence_ladder_fills
  (user_id, flow_definition_id, root_flow_node_key, market_slug, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_trade_builder_confidence_ladder_parent
  ON trade_builder_confidence_ladder_fills (parent_builder_order_id)
  WHERE parent_builder_order_id IS NOT NULL;
