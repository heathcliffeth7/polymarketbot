CREATE TABLE IF NOT EXISTS auto_claim_jobs (
  id BIGSERIAL PRIMARY KEY,
  owner_address TEXT NOT NULL,
  market_slug TEXT,
  condition_id TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  attempts INTEGER NOT NULL DEFAULT 0,
  max_attempts INTEGER NOT NULL DEFAULT 5,
  next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  tx_hash TEXT,
  last_error TEXT,
  claimed_at TIMESTAMPTZ,
  last_seen_redeemable_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT uq_auto_claim_owner_condition UNIQUE (owner_address, condition_id),
  CONSTRAINT chk_auto_claim_status
    CHECK (status IN ('pending', 'processing', 'retry', 'claimed', 'failed', 'skipped')),
  CONSTRAINT chk_auto_claim_attempts CHECK (attempts >= 0),
  CONSTRAINT chk_auto_claim_max_attempts CHECK (max_attempts > 0)
);

CREATE INDEX IF NOT EXISTS idx_auto_claim_jobs_status_next
  ON auto_claim_jobs(status, next_attempt_at, id);

CREATE INDEX IF NOT EXISTS idx_auto_claim_jobs_owner_updated
  ON auto_claim_jobs(owner_address, updated_at DESC);

CREATE TABLE IF NOT EXISTS auto_claim_events (
  id BIGSERIAL PRIMARY KEY,
  job_id BIGINT NOT NULL REFERENCES auto_claim_jobs(id) ON DELETE CASCADE,
  event_type TEXT NOT NULL,
  payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_auto_claim_events_job_time
  ON auto_claim_events(job_id, created_at DESC);
