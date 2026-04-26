ALTER TABLE trade_builder_orders
  DROP CONSTRAINT IF EXISTS chk_trade_builder_ptb_stop_loss_gap_usd;

ALTER TABLE trade_builder_orders
  ADD CONSTRAINT chk_trade_builder_ptb_stop_loss_gap_usd
  CHECK (
    ptb_stop_loss_gap_usd IS NULL
    OR (
      ptb_stop_loss_gap_usd > '-Infinity'::DOUBLE PRECISION
      AND ptb_stop_loss_gap_usd < 'Infinity'::DOUBLE PRECISION
    )
  );
