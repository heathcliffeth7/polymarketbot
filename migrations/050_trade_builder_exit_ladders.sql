ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS tp_rules_json JSONB NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS sl_rules_json JSONB NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS time_exit_rules_json JSONB NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS exit_ladder_kind TEXT;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS exit_ladder_index INTEGER;

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS exit_ladder_size_pct DOUBLE PRECISION;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_tp_rules_json_array'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_tp_rules_json_array
      CHECK (jsonb_typeof(tp_rules_json) = 'array');
  END IF;

  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_sl_rules_json_array'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_sl_rules_json_array
      CHECK (jsonb_typeof(sl_rules_json) = 'array');
  END IF;

  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_time_exit_rules_json_array'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_time_exit_rules_json_array
      CHECK (jsonb_typeof(time_exit_rules_json) = 'array');
  END IF;

  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_exit_ladder_kind'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_exit_ladder_kind
      CHECK (
        exit_ladder_kind IS NULL
        OR exit_ladder_kind IN ('tp', 'sl')
      );
  END IF;

  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_exit_ladder_index'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_exit_ladder_index
      CHECK (exit_ladder_index IS NULL OR exit_ladder_index >= 0);
  END IF;

  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_exit_ladder_size_pct'
  ) THEN
    ALTER TABLE trade_builder_orders
      ADD CONSTRAINT chk_trade_builder_exit_ladder_size_pct
      CHECK (
        exit_ladder_size_pct IS NULL
        OR (exit_ladder_size_pct > 0 AND exit_ladder_size_pct <= 100)
      );
  END IF;
END
$$;
