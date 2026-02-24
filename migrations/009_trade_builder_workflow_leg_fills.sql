ALTER TABLE trade_builder_workflow_legs
  ADD COLUMN IF NOT EXISTS filled_notional_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS filled_qty DOUBLE PRECISION NOT NULL DEFAULT 0;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_tb_workflow_leg_filled_notional'
  ) THEN
    ALTER TABLE trade_builder_workflow_legs
      ADD CONSTRAINT chk_tb_workflow_leg_filled_notional
      CHECK (filled_notional_usdc >= 0);
  END IF;
END$$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_tb_workflow_leg_filled_qty'
  ) THEN
    ALTER TABLE trade_builder_workflow_legs
      ADD CONSTRAINT chk_tb_workflow_leg_filled_qty
      CHECK (filled_qty >= 0);
  END IF;
END$$;
