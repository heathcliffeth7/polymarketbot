CREATE TABLE IF NOT EXISTS trade_flow_node_runtime_snapshots (
  id BIGSERIAL PRIMARY KEY,
  run_id BIGINT NOT NULL REFERENCES trade_flow_runs(id) ON DELETE CASCADE,
  definition_id BIGINT NOT NULL REFERENCES trade_flow_definitions(id) ON DELETE CASCADE,
  version_id BIGINT REFERENCES trade_flow_versions(id) ON DELETE SET NULL,
  node_key TEXT NOT NULL,
  node_type TEXT NOT NULL,
  status TEXT NOT NULL,
  state_kind TEXT NOT NULL,
  market_slug TEXT,
  token_id TEXT,
  market_slug_key TEXT NOT NULL DEFAULT '',
  token_id_key TEXT NOT NULL DEFAULT '',
  snapshot_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_trade_flow_node_runtime_snapshots_unique
  ON trade_flow_node_runtime_snapshots (run_id, node_key, market_slug_key, token_id_key);

CREATE INDEX IF NOT EXISTS idx_trade_flow_node_runtime_snapshots_run_updated
  ON trade_flow_node_runtime_snapshots (run_id, updated_at DESC);
