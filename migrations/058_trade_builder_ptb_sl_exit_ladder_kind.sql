ALTER TABLE trade_builder_orders
  DROP CONSTRAINT IF EXISTS chk_trade_builder_exit_ladder_kind;

ALTER TABLE trade_builder_orders
  ADD CONSTRAINT chk_trade_builder_exit_ladder_kind
  CHECK (
    exit_ladder_kind IS NULL
    OR exit_ladder_kind IN ('tp', 'sl', 'ptb_sl')
  );
