CREATE TABLE IF NOT EXISTS trade_builder_order_node_snapshots (
    id BIGSERIAL PRIMARY KEY,
    order_id BIGINT NOT NULL REFERENCES trade_builder_orders(id) ON DELETE CASCADE,
    root_order_id BIGINT NOT NULL,
    flow_run_id BIGINT,
    flow_definition_id BIGINT,
    flow_version_id BIGINT,
    node_key TEXT NOT NULL,
    node_type TEXT NOT NULL,
    node_config_hash TEXT NOT NULL,
    snapshot_json JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_trade_builder_order_node_snapshots_order
    ON trade_builder_order_node_snapshots (order_id);

CREATE INDEX IF NOT EXISTS idx_trade_builder_order_node_snapshots_root
    ON trade_builder_order_node_snapshots (root_order_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_trade_builder_order_node_snapshots_flow
    ON trade_builder_order_node_snapshots (flow_run_id, node_key)
    WHERE flow_run_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_trade_builder_order_node_snapshots_config_hash
    ON trade_builder_order_node_snapshots (node_config_hash);
