ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS guard_trigger_price DOUBLE PRECISION;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_guard_trigger_price'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_guard_trigger_price
      CHECK (guard_trigger_price IS NULL OR (guard_trigger_price > 0 AND guard_trigger_price <= 1));
  END IF;
END
$$;
