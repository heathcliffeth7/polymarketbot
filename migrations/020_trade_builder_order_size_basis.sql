-- Share-basis sizing for trade_builder exit orders (TP/SL) and inventory-aware retries

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS size_basis TEXT NOT NULL DEFAULT 'notional_usdc';

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS target_qty DOUBLE PRECISION;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS remaining_qty DOUBLE PRECISION;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_order_size_basis'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_order_size_basis
      CHECK (size_basis IN ('notional_usdc', 'shares'));
  END IF;
END
$$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_order_target_qty'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_order_target_qty
      CHECK (target_qty IS NULL OR target_qty > 0);
  END IF;
END
$$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_order_remaining_qty'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_order_remaining_qty
      CHECK (remaining_qty IS NULL OR remaining_qty >= 0);
  END IF;
END
$$;

