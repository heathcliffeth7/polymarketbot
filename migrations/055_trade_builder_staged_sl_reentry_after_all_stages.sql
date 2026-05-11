ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS staged_sl_reentry_only_after_all_stages BOOLEAN NOT NULL DEFAULT FALSE;
