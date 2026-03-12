ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS best_ask_floor_price DOUBLE PRECISION;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_best_ask_floor_price'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_best_ask_floor_price
      CHECK (
        best_ask_floor_price IS NULL
        OR (best_ask_floor_price > 0 AND best_ask_floor_price <= 1)
      );
  END IF;
END $$;
