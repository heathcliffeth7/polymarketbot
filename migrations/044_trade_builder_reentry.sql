ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS reenter_on_sl_hit BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS reentry_max_attempts INTEGER NOT NULL DEFAULT 0;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS reentry_trigger_node_key TEXT;
