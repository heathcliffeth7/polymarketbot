ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS max_price DOUBLE PRECISION;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_max_price'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_max_price
      CHECK (max_price IS NULL OR (max_price > 0 AND max_price <= 1));
  END IF;
END $$;
