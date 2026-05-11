-- Stop Loss support for action.place_order
-- When a buy order fills and sl_enabled=true, a conditional IOC sell is auto-created

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS sl_enabled BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS sl_price DOUBLE PRECISION;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_sl_price'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_sl_price
      CHECK (sl_price IS NULL OR (sl_price > 0 AND sl_price <= 1));
  END IF;
END
$$;
