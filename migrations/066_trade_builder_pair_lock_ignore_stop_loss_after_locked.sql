ALTER TABLE trade_builder_pair_sessions
  ADD COLUMN IF NOT EXISTS ignore_stop_loss_after_locked BOOLEAN NOT NULL DEFAULT FALSE;
