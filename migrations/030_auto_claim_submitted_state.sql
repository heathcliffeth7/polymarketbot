ALTER TABLE auto_claim_jobs
  DROP CONSTRAINT IF EXISTS chk_auto_claim_status;

ALTER TABLE auto_claim_jobs
  ADD CONSTRAINT chk_auto_claim_status
    CHECK (status IN ('pending', 'processing', 'submitted', 'retry', 'claimed', 'failed', 'skipped'));

ALTER TABLE auto_claim_jobs
  ADD COLUMN IF NOT EXISTS submitted_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_auto_claim_jobs_submitted
  ON auto_claim_jobs(submitted_at, id)
  WHERE status = 'submitted';

UPDATE auto_claim_jobs
SET status = 'pending',
    attempts = 0,
    next_attempt_at = NOW(),
    last_error = NULL,
    tx_hash = NULL,
    claimed_at = NULL,
    submitted_at = NULL,
    updated_at = NOW()
WHERE status = 'failed'
  AND last_error LIKE '%safe execTransaction send failed%';
