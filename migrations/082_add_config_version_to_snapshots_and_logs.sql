ALTER TABLE trade_builder_order_node_snapshots
    ADD COLUMN IF NOT EXISTS config_version TEXT;

CREATE INDEX IF NOT EXISTS idx_node_snapshots_config_version
    ON trade_builder_order_node_snapshots (config_version);

ALTER TABLE bot_decision_logs
    ADD COLUMN IF NOT EXISTS config_version TEXT;

CREATE INDEX IF NOT EXISTS idx_decision_logs_config_version
    ON bot_decision_logs (config_version);
