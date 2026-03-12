ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS sl_trigger_price_mode TEXT;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_sl_trigger_price_mode'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_sl_trigger_price_mode
      CHECK (
        sl_trigger_price_mode IS NULL
        OR sl_trigger_price_mode IN ('best_bid', 'composite', 'last_trade')
      );
  END IF;
END
$$;
