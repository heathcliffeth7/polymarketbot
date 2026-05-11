ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS ptb_stop_loss_rules_json JSONB NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE trade_builder_orders
  DROP CONSTRAINT IF EXISTS chk_trade_builder_ptb_stop_loss_rules_json_array;

ALTER TABLE trade_builder_orders
  ADD CONSTRAINT chk_trade_builder_ptb_stop_loss_rules_json_array
  CHECK (jsonb_typeof(ptb_stop_loss_rules_json) = 'array');
