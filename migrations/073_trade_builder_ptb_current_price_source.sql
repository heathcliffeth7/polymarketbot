ALTER TABLE trade_builder_orders
    ADD COLUMN IF NOT EXISTS ptb_current_price_source TEXT NOT NULL DEFAULT 'chainlink';

ALTER TABLE trade_builder_orders
    DROP CONSTRAINT IF EXISTS trade_builder_orders_ptb_current_price_source_check;

ALTER TABLE trade_builder_orders
    ADD CONSTRAINT trade_builder_orders_ptb_current_price_source_check
    CHECK (ptb_current_price_source IN ('chainlink', 'binance', 'coinbase'));
