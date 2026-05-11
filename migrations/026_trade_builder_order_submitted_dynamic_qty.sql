ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS submitted_dynamic_qty DOUBLE PRECISION;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS submitted_dynamic_price DOUBLE PRECISION;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_submitted_dynamic_qty'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_submitted_dynamic_qty
      CHECK (submitted_dynamic_qty IS NULL OR submitted_dynamic_qty > 0);
  END IF;
END
$$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_submitted_dynamic_price'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_submitted_dynamic_price
      CHECK (submitted_dynamic_price IS NULL OR submitted_dynamic_price > 0);
  END IF;
END
$$;
