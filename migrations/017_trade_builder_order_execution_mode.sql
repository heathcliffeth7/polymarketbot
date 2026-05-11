ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS execution_mode TEXT;

UPDATE trade_builder_orders
SET execution_mode = 'market'
WHERE execution_mode IS NULL OR btrim(execution_mode) = '';

ALTER TABLE trade_builder_orders
  ALTER COLUMN execution_mode SET DEFAULT 'market';

ALTER TABLE trade_builder_orders
  ALTER COLUMN execution_mode SET NOT NULL;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_execution_mode'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_execution_mode
      CHECK (execution_mode IN ('limit', 'market'));
  END IF;
END
$$;
