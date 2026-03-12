ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS retry_on_trigger_guard_block BOOLEAN NOT NULL DEFAULT FALSE,
  ADD COLUMN IF NOT EXISTS retry_on_execution_floor_guard_block BOOLEAN NOT NULL DEFAULT FALSE;
