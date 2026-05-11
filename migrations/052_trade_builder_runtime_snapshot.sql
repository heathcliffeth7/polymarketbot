ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS runtime_snapshot_json JSONB,
  ADD COLUMN IF NOT EXISTS fresh_submit_lease_until TIMESTAMPTZ;
