ALTER TABLE trade_builder_orders
    DROP CONSTRAINT IF EXISTS trade_builder_orders_ptb_current_price_source_check;

ALTER TABLE trade_builder_orders
    ADD CONSTRAINT trade_builder_orders_ptb_current_price_source_check
    CHECK (
        ptb_current_price_source IN (
            'chainlink',
            'binance',
            'coinbase',
            'hyperliquid',
            'binance_hyperliquid',
            'cex_consensus',
            'chainlink_cex_consensus',
            'chainlink_cex_consensus_confirmed',
            'cex_median_fast'
        )
    );
