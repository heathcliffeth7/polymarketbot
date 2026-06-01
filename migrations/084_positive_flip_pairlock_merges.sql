CREATE TABLE IF NOT EXISTS trade_builder_positive_quantity_flip_grid_merges (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
  flow_definition_id BIGINT REFERENCES trade_flow_definitions(id) ON DELETE SET NULL,
  flow_run_id BIGINT REFERENCES trade_flow_runs(id) ON DELETE SET NULL,
  root_flow_node_key TEXT NOT NULL,
  market_slug TEXT NOT NULL,
  condition_id TEXT NOT NULL,
  quantity DOUBLE PRECISION NOT NULL,
  returned_usdc DOUBLE PRECISION NOT NULL,
  tx_hash TEXT NOT NULL,
  submission_mode TEXT NOT NULL,
  payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT uq_positive_flip_pairlock_merge_tx UNIQUE (tx_hash),
  CONSTRAINT chk_positive_flip_pairlock_merge_qty CHECK (quantity >= 0),
  CONSTRAINT chk_positive_flip_pairlock_merge_return CHECK (returned_usdc >= 0)
);

CREATE INDEX IF NOT EXISTS idx_positive_flip_pairlock_merge_state
  ON trade_builder_positive_quantity_flip_grid_merges
  (user_id, flow_definition_id, root_flow_node_key, market_slug, created_at DESC);
