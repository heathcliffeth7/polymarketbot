ALTER TABLE trade_builder_orders
  ADD COLUMN eligible_after_at TIMESTAMPTZ,
  ADD COLUMN eligible_before_at TIMESTAMPTZ;
