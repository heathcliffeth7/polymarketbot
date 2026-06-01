CREATE TABLE IF NOT EXISTS trade_builder_revenge_flip_state (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
  flow_definition_id BIGINT NOT NULL REFERENCES trade_flow_definitions(id) ON DELETE CASCADE,
  root_flow_node_key TEXT NOT NULL,
  market_slug TEXT NOT NULL,
  current_side TEXT,
  next_entry_side TEXT,
  position_qty DOUBLE PRECISION NOT NULL DEFAULT 0,
  position_avg_cost DOUBLE PRECISION NOT NULL DEFAULT 0,
  position_entry_price DOUBLE PRECISION NOT NULL DEFAULT 0,
  position_stop_loss_pct DOUBLE PRECISION NOT NULL DEFAULT 0.2,
  position_source_trade_id BIGINT,
  position_builder_order_id BIGINT REFERENCES trade_builder_orders(id) ON DELETE SET NULL,
  flip_count BIGINT NOT NULL DEFAULT 0,
  total_loss_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
  realized_pnl_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
  total_buy_cost DOUBLE PRECISION NOT NULL DEFAULT 0,
  total_sell_revenue DOUBLE PRECISION NOT NULL DEFAULT 0,
  ptb_bump_count BIGINT NOT NULL DEFAULT 0,
  ptb_bump_total_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
  last_intent TEXT,
  last_builder_order_id BIGINT REFERENCES trade_builder_orders(id) ON DELETE SET NULL,
  last_action_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT uq_trade_builder_revenge_flip_state UNIQUE
    (user_id, flow_definition_id, root_flow_node_key, market_slug),
  CONSTRAINT chk_trade_builder_revenge_flip_current_side
    CHECK (current_side IS NULL OR current_side IN ('up', 'down')),
  CONSTRAINT chk_trade_builder_revenge_flip_next_side
    CHECK (next_entry_side IS NULL OR next_entry_side IN ('up', 'down')),
  CONSTRAINT chk_trade_builder_revenge_flip_qty CHECK (position_qty >= 0),
  CONSTRAINT chk_trade_builder_revenge_flip_avg CHECK (position_avg_cost >= 0 AND position_avg_cost <= 1),
  CONSTRAINT chk_trade_builder_revenge_flip_entry CHECK (position_entry_price >= 0 AND position_entry_price <= 1),
  CONSTRAINT chk_trade_builder_revenge_flip_stop_pct CHECK (position_stop_loss_pct > 0 AND position_stop_loss_pct < 1),
  CONSTRAINT chk_trade_builder_revenge_flip_flip_count CHECK (flip_count >= 0),
  CONSTRAINT chk_trade_builder_revenge_flip_loss CHECK (total_loss_usdc >= 0),
  CONSTRAINT chk_trade_builder_revenge_flip_bump CHECK (ptb_bump_count >= 0 AND ptb_bump_total_usdc >= 0)
);

CREATE INDEX IF NOT EXISTS idx_trade_builder_revenge_flip_state_lookup
  ON trade_builder_revenge_flip_state
  (user_id, flow_definition_id, root_flow_node_key, market_slug);

CREATE TABLE IF NOT EXISTS trade_builder_revenge_flip_fills (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
  flow_definition_id BIGINT NOT NULL REFERENCES trade_flow_definitions(id) ON DELETE CASCADE,
  flow_run_id BIGINT REFERENCES trade_flow_runs(id) ON DELETE SET NULL,
  root_flow_node_key TEXT NOT NULL,
  market_slug TEXT NOT NULL,
  token_id TEXT NOT NULL,
  outcome_label TEXT NOT NULL,
  revenge_side TEXT NOT NULL,
  intent TEXT NOT NULL,
  order_side TEXT NOT NULL,
  builder_order_id BIGINT NOT NULL REFERENCES trade_builder_orders(id) ON DELETE CASCADE,
  parent_builder_order_id BIGINT REFERENCES trade_builder_orders(id) ON DELETE SET NULL,
  source_trade_id BIGINT,
  quantity DOUBLE PRECISION NOT NULL,
  execution_price DOUBLE PRECISION NOT NULL,
  notional_usdc DOUBLE PRECISION NOT NULL,
  stop_loss_pct DOUBLE PRECISION,
  payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT uq_trade_builder_revenge_flip_builder_order UNIQUE (builder_order_id),
  CONSTRAINT chk_trade_builder_revenge_flip_fill_side CHECK (revenge_side IN ('up', 'down')),
  CONSTRAINT chk_trade_builder_revenge_flip_fill_order_side CHECK (order_side IN ('buy', 'sell')),
  CONSTRAINT chk_trade_builder_revenge_flip_fill_qty CHECK (quantity >= 0),
  CONSTRAINT chk_trade_builder_revenge_flip_fill_price CHECK (execution_price >= 0 AND execution_price <= 1),
  CONSTRAINT chk_trade_builder_revenge_flip_fill_notional CHECK (notional_usdc >= 0),
  CONSTRAINT chk_trade_builder_revenge_flip_fill_stop_pct CHECK (stop_loss_pct IS NULL OR (stop_loss_pct > 0 AND stop_loss_pct < 1))
);

CREATE INDEX IF NOT EXISTS idx_trade_builder_revenge_flip_fills_state
  ON trade_builder_revenge_flip_fills
  (user_id, flow_definition_id, root_flow_node_key, market_slug, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_trade_builder_revenge_flip_fills_parent
  ON trade_builder_revenge_flip_fills (parent_builder_order_id)
  WHERE parent_builder_order_id IS NOT NULL;
