CREATE TABLE IF NOT EXISTS bot_decision_logs (
    id BIGSERIAL PRIMARY KEY,
    event_id UUID NOT NULL,
    idempotency_key TEXT,
    schema_version INT NOT NULL DEFAULT 1,
    event_type TEXT NOT NULL,
    event_ts TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    decision_id TEXT,
    sl_event_id TEXT,
    fill_event_id TEXT,
    market_slug TEXT,
    root_order_id TEXT,
    order_id TEXT,
    exchange_order_id TEXT,
    parent_order_id TEXT,
    child_order_id TEXT,
    source_trade_id TEXT,
    flow_run_id TEXT,
    flow_definition_id TEXT,
    pair_session_id TEXT,
    asset TEXT,
    workflow TEXT,
    outcome TEXT,
    outcome_token_id TEXT,
    opposite_token_id TEXT,
    payload JSONB NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_bot_decision_logs_event_id
    ON bot_decision_logs (event_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_bot_decision_logs_idempotency
    ON bot_decision_logs (idempotency_key)
    WHERE idempotency_key IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_bot_decision_logs_event_type_created
    ON bot_decision_logs (event_type, created_at);

CREATE INDEX IF NOT EXISTS idx_bot_decision_logs_decision_created
    ON bot_decision_logs (decision_id, created_at);

CREATE INDEX IF NOT EXISTS idx_bot_decision_logs_root_created
    ON bot_decision_logs (root_order_id, created_at);

CREATE INDEX IF NOT EXISTS idx_bot_decision_logs_sl_event_created
    ON bot_decision_logs (sl_event_id, created_at);

CREATE INDEX IF NOT EXISTS idx_bot_decision_logs_order_created
    ON bot_decision_logs (order_id, created_at);

CREATE INDEX IF NOT EXISTS idx_bot_decision_logs_market_created
    ON bot_decision_logs (market_slug, created_at);
