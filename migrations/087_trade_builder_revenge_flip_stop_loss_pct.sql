ALTER TABLE trade_builder_revenge_flip_state
  ADD COLUMN IF NOT EXISTS position_stop_loss_pct DOUBLE PRECISION NOT NULL DEFAULT 0.2;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_revenge_flip_stop_pct'
  ) THEN
    ALTER TABLE trade_builder_revenge_flip_state
      ADD CONSTRAINT chk_trade_builder_revenge_flip_stop_pct
      CHECK (position_stop_loss_pct > 0 AND position_stop_loss_pct < 1);
  END IF;
END $$;

ALTER TABLE trade_builder_revenge_flip_fills
  ADD COLUMN IF NOT EXISTS stop_loss_pct DOUBLE PRECISION;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_revenge_flip_fill_stop_pct'
  ) THEN
    ALTER TABLE trade_builder_revenge_flip_fills
      ADD CONSTRAINT chk_trade_builder_revenge_flip_fill_stop_pct
      CHECK (stop_loss_pct IS NULL OR (stop_loss_pct > 0 AND stop_loss_pct < 1));
  END IF;
END $$;
