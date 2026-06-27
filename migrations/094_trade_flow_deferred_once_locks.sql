CREATE TABLE IF NOT EXISTS trade_flow_deferred_once_locks (
  id BIGSERIAL PRIMARY KEY,
  run_id BIGINT NOT NULL REFERENCES trade_flow_runs(id) ON DELETE CASCADE,
  definition_id BIGINT NOT NULL REFERENCES trade_flow_definitions(id) ON DELETE CASCADE,
  version_id BIGINT NOT NULL REFERENCES trade_flow_versions(id) ON DELETE CASCADE,
  trigger_node_key TEXT NOT NULL,
  action_node_key TEXT NOT NULL,
  market_slug TEXT NOT NULL,
  token_id TEXT NOT NULL,
  outcome_label TEXT NOT NULL,
  once_scope_market BOOLEAN NOT NULL DEFAULT TRUE,
  lock_key TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'pending',
  expires_at TIMESTAMPTZ NOT NULL,
  builder_order_id BIGINT REFERENCES trade_builder_orders(id) ON DELETE SET NULL,
  release_reason TEXT,
  consumed_at TIMESTAMPTZ,
  released_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT chk_trade_flow_deferred_once_locks_state
    CHECK (state IN ('pending', 'consumed', 'released', 'expired'))
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_trade_flow_deferred_once_locks_pending_key
  ON trade_flow_deferred_once_locks(lock_key)
  WHERE state = 'pending';

CREATE INDEX IF NOT EXISTS idx_trade_flow_deferred_once_locks_run_state
  ON trade_flow_deferred_once_locks(run_id, state, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_trade_flow_deferred_once_locks_expires
  ON trade_flow_deferred_once_locks(state, expires_at)
  WHERE state = 'pending';
