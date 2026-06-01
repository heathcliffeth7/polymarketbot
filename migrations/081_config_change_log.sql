CREATE TABLE IF NOT EXISTS config_change_log (
    id BIGSERIAL PRIMARY KEY,
    config_version TEXT NOT NULL,
    changed_by TEXT,
    change_reason TEXT,
    changed_fields JSONB NOT NULL DEFAULT '{}'::jsonb,
    full_config_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_config_change_log_version
    ON config_change_log (config_version);

CREATE INDEX IF NOT EXISTS idx_config_change_log_created
    ON config_change_log (created_at DESC);
