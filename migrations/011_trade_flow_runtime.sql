ALTER TABLE trade_flow_run_steps
  ADD COLUMN IF NOT EXISTS available_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

ALTER TABLE trade_flow_run_steps
  ADD COLUMN IF NOT EXISTS parent_step_id BIGINT;

ALTER TABLE trade_flow_run_steps
  ADD COLUMN IF NOT EXISTS idempotency_key TEXT;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'fk_trade_flow_run_steps_parent'
  ) THEN
    ALTER TABLE trade_flow_run_steps
      ADD CONSTRAINT fk_trade_flow_run_steps_parent
      FOREIGN KEY (parent_step_id) REFERENCES trade_flow_run_steps(id) ON DELETE SET NULL;
  END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_trade_flow_run_steps_ready
  ON trade_flow_run_steps(run_id, status, available_at, id);

CREATE UNIQUE INDEX IF NOT EXISTS uq_trade_flow_run_steps_idempotency
  ON trade_flow_run_steps(run_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS uq_trade_flow_runs_single_active
  ON trade_flow_runs(definition_id)
  WHERE status = 'running';
