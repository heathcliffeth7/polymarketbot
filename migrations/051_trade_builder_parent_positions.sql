CREATE TABLE trade_builder_parent_positions (
  parent_builder_order_id BIGINT PRIMARY KEY REFERENCES trade_builder_orders(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL,
  source_trade_id BIGINT NOT NULL,
  market_slug TEXT NOT NULL,
  token_id TEXT NOT NULL,
  outcome_label TEXT NOT NULL,
  baseline_qty DOUBLE PRECISION NOT NULL,
  current_qty DOUBLE PRECISION NOT NULL,
  last_fill_qty DOUBLE PRECISION,
  last_fill_price DOUBLE PRECISION,
  qty_source TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_trade_builder_parent_positions_lookup
  ON trade_builder_parent_positions(user_id, market_slug, token_id);
