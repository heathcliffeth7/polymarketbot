ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS filled_qty DOUBLE PRECISION NOT NULL DEFAULT 0;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS fee_rate_bps BIGINT NOT NULL DEFAULT 0;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS trigger_latched BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS trigger_latched_reason TEXT;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_filled_qty'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_filled_qty
      CHECK (filled_qty >= 0);
  END IF;
END
$$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_fee_rate_bps'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_fee_rate_bps
      CHECK (fee_rate_bps >= 0);
  END IF;
END
$$;
