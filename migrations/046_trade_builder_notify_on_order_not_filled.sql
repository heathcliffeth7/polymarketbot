ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS notify_on_order_not_filled BOOLEAN NOT NULL DEFAULT FALSE;
