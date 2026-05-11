ALTER TABLE trade_builder_orders
  DROP CONSTRAINT IF EXISTS chk_trade_builder_sl_trigger_price_mode;

ALTER TABLE trade_builder_orders
  ADD CONSTRAINT chk_trade_builder_sl_trigger_price_mode
  CHECK (
    sl_trigger_price_mode IS NULL
    OR sl_trigger_price_mode = ANY (
      ARRAY[
        'best_bid'::TEXT,
        'composite'::TEXT,
        'composite_safe'::TEXT,
        'composite_fast'::TEXT,
        'last_trade'::TEXT
      ]
    )
  );
