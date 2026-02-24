CREATE TABLE IF NOT EXISTS trade_flow_definitions (
  id BIGSERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  status TEXT NOT NULL DEFAULT 'draft',
  draft_version_id BIGINT,
  published_version_id BIGINT,
  last_error TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT chk_trade_flow_definition_status CHECK (status IN ('draft', 'published', 'archived'))
);

CREATE INDEX IF NOT EXISTS idx_trade_flow_definitions_status_updated
  ON trade_flow_definitions(status, updated_at DESC);

CREATE TABLE IF NOT EXISTS trade_flow_versions (
  id BIGSERIAL PRIMARY KEY,
  definition_id BIGINT NOT NULL REFERENCES trade_flow_definitions(id) ON DELETE CASCADE,
  version_no INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'draft',
  graph_json JSONB NOT NULL DEFAULT '{"nodes":[],"edges":[]}'::jsonb,
  published_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT uq_trade_flow_version UNIQUE (definition_id, version_no),
  CONSTRAINT chk_trade_flow_version_status CHECK (status IN ('draft', 'published', 'archived'))
);

CREATE INDEX IF NOT EXISTS idx_trade_flow_versions_definition
  ON trade_flow_versions(definition_id, version_no DESC);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'fk_trade_flow_definitions_draft'
  ) THEN
    ALTER TABLE trade_flow_definitions
      ADD CONSTRAINT fk_trade_flow_definitions_draft
      FOREIGN KEY (draft_version_id) REFERENCES trade_flow_versions(id) ON DELETE SET NULL;
  END IF;
END $$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'fk_trade_flow_definitions_published'
  ) THEN
    ALTER TABLE trade_flow_definitions
      ADD CONSTRAINT fk_trade_flow_definitions_published
      FOREIGN KEY (published_version_id) REFERENCES trade_flow_versions(id) ON DELETE SET NULL;
  END IF;
END $$;

CREATE TABLE IF NOT EXISTS trade_flow_nodes (
  id BIGSERIAL PRIMARY KEY,
  version_id BIGINT NOT NULL REFERENCES trade_flow_versions(id) ON DELETE CASCADE,
  node_key TEXT NOT NULL,
  node_type TEXT NOT NULL,
  position_x DOUBLE PRECISION,
  position_y DOUBLE PRECISION,
  config_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT uq_trade_flow_node_key UNIQUE (version_id, node_key)
);

CREATE INDEX IF NOT EXISTS idx_trade_flow_nodes_version
  ON trade_flow_nodes(version_id);

CREATE TABLE IF NOT EXISTS trade_flow_edges (
  id BIGSERIAL PRIMARY KEY,
  version_id BIGINT NOT NULL REFERENCES trade_flow_versions(id) ON DELETE CASCADE,
  edge_key TEXT NOT NULL,
  source_node_key TEXT NOT NULL,
  target_node_key TEXT NOT NULL,
  edge_type TEXT NOT NULL DEFAULT 'default',
  condition_json JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT uq_trade_flow_edge_key UNIQUE (version_id, edge_key)
);

CREATE INDEX IF NOT EXISTS idx_trade_flow_edges_version
  ON trade_flow_edges(version_id);

CREATE TABLE IF NOT EXISTS trade_flow_runs (
  id BIGSERIAL PRIMARY KEY,
  definition_id BIGINT NOT NULL REFERENCES trade_flow_definitions(id) ON DELETE CASCADE,
  version_id BIGINT NOT NULL REFERENCES trade_flow_versions(id) ON DELETE RESTRICT,
  status TEXT NOT NULL DEFAULT 'queued',
  trigger_source TEXT,
  context_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  started_at TIMESTAMPTZ,
  ended_at TIMESTAMPTZ,
  last_error TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT chk_trade_flow_run_status CHECK (status IN ('queued', 'running', 'completed', 'failed', 'canceled'))
);

CREATE INDEX IF NOT EXISTS idx_trade_flow_runs_status_updated
  ON trade_flow_runs(status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_trade_flow_runs_definition
  ON trade_flow_runs(definition_id, created_at DESC);

CREATE TABLE IF NOT EXISTS trade_flow_run_steps (
  id BIGSERIAL PRIMARY KEY,
  run_id BIGINT NOT NULL REFERENCES trade_flow_runs(id) ON DELETE CASCADE,
  node_key TEXT NOT NULL,
  node_type TEXT NOT NULL,
  status TEXT NOT NULL,
  attempt INTEGER NOT NULL DEFAULT 1,
  input_json JSONB,
  output_json JSONB,
  error_text TEXT,
  started_at TIMESTAMPTZ,
  ended_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT chk_trade_flow_step_status CHECK (status IN ('queued', 'running', 'completed', 'failed', 'skipped', 'canceled'))
);

CREATE INDEX IF NOT EXISTS idx_trade_flow_run_steps_run
  ON trade_flow_run_steps(run_id, created_at DESC);

CREATE TABLE IF NOT EXISTS trade_flow_events (
  id BIGSERIAL PRIMARY KEY,
  run_id BIGINT REFERENCES trade_flow_runs(id) ON DELETE CASCADE,
  definition_id BIGINT NOT NULL REFERENCES trade_flow_definitions(id) ON DELETE CASCADE,
  version_id BIGINT REFERENCES trade_flow_versions(id) ON DELETE SET NULL,
  event_type TEXT NOT NULL,
  payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_trade_flow_events_run_time
  ON trade_flow_events(run_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_trade_flow_events_definition_time
  ON trade_flow_events(definition_id, created_at DESC);

CREATE TABLE IF NOT EXISTS trade_flow_legacy_mappings (
  legacy_workflow_id BIGINT PRIMARY KEY REFERENCES trade_builder_workflows(id) ON DELETE CASCADE,
  definition_id BIGINT NOT NULL REFERENCES trade_flow_definitions(id) ON DELETE CASCADE,
  version_id BIGINT REFERENCES trade_flow_versions(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT uq_trade_flow_legacy_definition UNIQUE (definition_id)
);

CREATE INDEX IF NOT EXISTS idx_trade_flow_legacy_definition
  ON trade_flow_legacy_mappings(definition_id);
