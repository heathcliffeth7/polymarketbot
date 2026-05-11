ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS origin_flow_definition_id BIGINT,
  ADD COLUMN IF NOT EXISTS origin_flow_run_id BIGINT,
  ADD COLUMN IF NOT EXISTS origin_flow_node_key TEXT;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'fk_trade_builder_orders_origin_flow_definition'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT fk_trade_builder_orders_origin_flow_definition
      FOREIGN KEY (origin_flow_definition_id) REFERENCES trade_flow_definitions(id) ON DELETE SET NULL;
  END IF;
END $$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'fk_trade_builder_orders_origin_flow_run'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT fk_trade_builder_orders_origin_flow_run
      FOREIGN KEY (origin_flow_run_id) REFERENCES trade_flow_runs(id) ON DELETE SET NULL;
  END IF;
END $$;

WITH latest_flow_origin AS (
  SELECT DISTINCT ON (e.builder_order_id)
    e.builder_order_id,
    NULLIF(e.payload_json->>'flow_run_id', '')::BIGINT AS flow_run_id,
    NULLIF(e.payload_json->>'node_key', '') AS flow_node_key
  FROM trade_builder_order_events e
  WHERE e.event_type IN ('flow_created', 'flow_rearmed')
    AND (e.payload_json->>'flow_run_id') ~ '^[0-9]+$'
  ORDER BY e.builder_order_id, e.created_at DESC, e.id DESC
)
UPDATE trade_builder_orders o
SET origin_flow_run_id = latest.flow_run_id,
    origin_flow_definition_id = runs.definition_id,
    origin_flow_node_key = COALESCE(latest.flow_node_key, o.origin_flow_node_key)
FROM latest_flow_origin latest
JOIN trade_flow_runs runs ON runs.id = latest.flow_run_id
WHERE o.id = latest.builder_order_id
  AND (
    o.origin_flow_run_id IS NULL
    OR o.origin_flow_definition_id IS NULL
    OR o.origin_flow_node_key IS NULL
  );

UPDATE trade_builder_orders child
SET origin_flow_definition_id = parent.origin_flow_definition_id,
    origin_flow_run_id = parent.origin_flow_run_id,
    origin_flow_node_key = COALESCE(child.origin_flow_node_key, parent.origin_flow_node_key)
FROM trade_builder_orders parent
WHERE child.parent_order_id = parent.id
  AND child.origin_flow_definition_id IS NULL
  AND parent.origin_flow_definition_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_trade_builder_orders_origin_flow_definition
  ON trade_builder_orders (origin_flow_definition_id, status, id)
  WHERE origin_flow_definition_id IS NOT NULL;
