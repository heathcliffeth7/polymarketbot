ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS notify_on_order_submitted BOOLEAN NOT NULL DEFAULT false;
