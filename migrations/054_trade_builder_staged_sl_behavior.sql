ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS staged_sl_retry_only_dust BOOLEAN NOT NULL DEFAULT FALSE,
  ADD COLUMN IF NOT EXISTS staged_sl_retry_dust_metric TEXT,
  ADD COLUMN IF NOT EXISTS staged_sl_retry_dust_value DOUBLE PRECISION,
  ADD COLUMN IF NOT EXISTS staged_sl_reentry_use_sold_notional BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE trade_builder_orders
  DROP CONSTRAINT IF EXISTS chk_trade_builder_staged_sl_retry_dust_metric;

ALTER TABLE trade_builder_orders
  ADD CONSTRAINT chk_trade_builder_staged_sl_retry_dust_metric
  CHECK (
    staged_sl_retry_dust_metric IS NULL
    OR staged_sl_retry_dust_metric = ANY (ARRAY['notional', 'qty'])
  );

ALTER TABLE trade_builder_orders
  DROP CONSTRAINT IF EXISTS chk_trade_builder_staged_sl_retry_dust_config;

ALTER TABLE trade_builder_orders
  ADD CONSTRAINT chk_trade_builder_staged_sl_retry_dust_config
  CHECK (
    (
      staged_sl_retry_only_dust = FALSE
      AND staged_sl_retry_dust_metric IS NULL
      AND staged_sl_retry_dust_value IS NULL
    )
    OR (
      staged_sl_retry_only_dust = TRUE
      AND staged_sl_retry_dust_metric = ANY (ARRAY['notional', 'qty'])
      AND staged_sl_retry_dust_value IS NOT NULL
      AND staged_sl_retry_dust_value > 0
    )
  );
