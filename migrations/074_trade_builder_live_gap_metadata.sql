ALTER TABLE trade_builder_orders
    ADD COLUMN IF NOT EXISTS live_gap_metadata_json JSONB;
