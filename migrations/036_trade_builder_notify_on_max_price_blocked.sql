ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS notify_on_max_price_blocked BOOLEAN NOT NULL DEFAULT FALSE;
