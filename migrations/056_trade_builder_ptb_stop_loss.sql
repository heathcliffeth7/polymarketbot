ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS ptb_stop_loss_gap_usd DOUBLE PRECISION;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS ptb_reference_price DOUBLE PRECISION;

ALTER TABLE trade_builder_orders
  DROP CONSTRAINT IF EXISTS chk_trade_builder_ptb_stop_loss_gap_usd;

ALTER TABLE trade_builder_orders
  ADD CONSTRAINT chk_trade_builder_ptb_stop_loss_gap_usd
  CHECK (
    ptb_stop_loss_gap_usd IS NULL
    OR ptb_stop_loss_gap_usd >= 0
  );

ALTER TABLE trade_builder_orders
  DROP CONSTRAINT IF EXISTS chk_trade_builder_ptb_reference_price;

ALTER TABLE trade_builder_orders
  ADD CONSTRAINT chk_trade_builder_ptb_reference_price
  CHECK (
    ptb_reference_price IS NULL
    OR ptb_reference_price > 0
  );
