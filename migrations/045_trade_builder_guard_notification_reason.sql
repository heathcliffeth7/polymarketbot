ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS last_guard_notification_reason TEXT;
