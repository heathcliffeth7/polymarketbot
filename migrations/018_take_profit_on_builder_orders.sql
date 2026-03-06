-- Take Profit support for action.place_order
-- When a buy order fills and tp_enabled=true, a conditional IOC sell is auto-created

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS parent_order_id BIGINT REFERENCES trade_builder_orders(id);

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS tp_enabled BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS tp_price DOUBLE PRECISION;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_tp_price'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_tp_price
      CHECK (tp_price IS NULL OR (tp_price > 0 AND tp_price <= 1));
  END IF;
END
$$;

CREATE INDEX IF NOT EXISTS idx_trade_builder_orders_parent
  ON trade_builder_orders(parent_order_id)
  WHERE parent_order_id IS NOT NULL;
