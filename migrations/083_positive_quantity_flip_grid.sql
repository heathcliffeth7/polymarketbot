CREATE TABLE IF NOT EXISTS trade_builder_positive_quantity_flip_grid_fills (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
  flow_definition_id BIGINT REFERENCES trade_flow_definitions(id) ON DELETE SET NULL,
  flow_run_id BIGINT REFERENCES trade_flow_runs(id) ON DELETE SET NULL,
  root_flow_node_key TEXT NOT NULL,
  market_slug TEXT NOT NULL,
  token_id TEXT NOT NULL,
  outcome_label TEXT NOT NULL,
  grid_side TEXT NOT NULL,
  order_side TEXT NOT NULL,
  builder_order_id BIGINT NOT NULL REFERENCES trade_builder_orders(id) ON DELETE CASCADE,
  parent_builder_order_id BIGINT REFERENCES trade_builder_orders(id) ON DELETE SET NULL,
  quantity DOUBLE PRECISION NOT NULL,
  execution_price DOUBLE PRECISION NOT NULL,
  notional_usdc DOUBLE PRECISION NOT NULL,
  payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT uq_positive_quantity_flip_grid_builder_order UNIQUE (builder_order_id),
  CONSTRAINT chk_positive_quantity_flip_grid_side CHECK (grid_side IN ('up', 'down')),
  CONSTRAINT chk_positive_quantity_flip_grid_order_side CHECK (order_side IN ('buy', 'sell')),
  CONSTRAINT chk_positive_quantity_flip_grid_qty CHECK (quantity >= 0),
  CONSTRAINT chk_positive_quantity_flip_grid_price CHECK (execution_price >= 0 AND execution_price <= 1),
  CONSTRAINT chk_positive_quantity_flip_grid_notional CHECK (notional_usdc >= 0)
);

CREATE INDEX IF NOT EXISTS idx_positive_quantity_flip_grid_state
  ON trade_builder_positive_quantity_flip_grid_fills
  (user_id, flow_definition_id, root_flow_node_key, market_slug, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_positive_quantity_flip_grid_parent
  ON trade_builder_positive_quantity_flip_grid_fills
  (parent_builder_order_id)
  WHERE parent_builder_order_id IS NOT NULL;
