ALTER TABLE orders ADD COLUMN IF NOT EXISTS exchange_ts BIGINT;
ALTER TABLE orders ADD COLUMN IF NOT EXISTS reject_reason TEXT;
ALTER TABLE orders ADD COLUMN IF NOT EXISTS raw_payload_json JSONB;

ALTER TABLE fills ADD COLUMN IF NOT EXISTS exchange_ts BIGINT;
ALTER TABLE fills ADD COLUMN IF NOT EXISTS raw_payload_json JSONB;

CREATE TABLE IF NOT EXISTS reconcile_runs (
  id BIGSERIAL PRIMARY KEY,
  run_id BIGINT REFERENCES bot_runs(id),
  market_slug TEXT,
  status TEXT NOT NULL,
  details TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_reconcile_runs_run_id ON reconcile_runs(run_id, created_at);
