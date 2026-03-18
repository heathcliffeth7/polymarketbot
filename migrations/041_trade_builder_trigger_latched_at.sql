ALTER TABLE trade_builder_orders ADD COLUMN trigger_latched_at TIMESTAMPTZ;
UPDATE trade_builder_orders SET trigger_latched_at = updated_at WHERE trigger_latched = TRUE;
