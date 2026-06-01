ALTER TABLE trade_builder_revenge_flip_state
  ADD COLUMN IF NOT EXISTS position_stop_loss_enabled BOOLEAN NOT NULL DEFAULT TRUE;

ALTER TABLE trade_builder_revenge_flip_fills
  ADD COLUMN IF NOT EXISTS stop_loss_enabled BOOLEAN;
