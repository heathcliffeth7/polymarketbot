ALTER TABLE bot_runs
  ADD COLUMN IF NOT EXISTS package_version TEXT,
  ADD COLUMN IF NOT EXISTS git_sha TEXT,
  ADD COLUMN IF NOT EXISTS build_time TEXT,
  ADD COLUMN IF NOT EXISTS process_start_time TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS config_hash TEXT;

CREATE INDEX IF NOT EXISTS idx_bot_runs_config_hash
  ON bot_runs (config_hash);
