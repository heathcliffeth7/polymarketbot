CREATE TABLE IF NOT EXISTS trade_builder_inventory_observations (
  id BIGSERIAL PRIMARY KEY,
  parent_builder_order_id BIGINT NOT NULL REFERENCES trade_builder_orders(id) ON DELETE CASCADE,
  observer_builder_order_id BIGINT REFERENCES trade_builder_orders(id) ON DELETE SET NULL,
  user_id BIGINT NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
  market_slug TEXT NOT NULL,
  token_id TEXT NOT NULL,
  outcome_label TEXT NOT NULL,
  exchange_order_id TEXT,
  observation_kind TEXT NOT NULL,
  qty_source TEXT,
  baseline_visible_qty DOUBLE PRECISION,
  submitted_dynamic_qty DOUBLE PRECISION,
  resolved_fill_qty DOUBLE PRECISION,
  expected_fee_qty DOUBLE PRECISION,
  expected_net_qty DOUBLE PRECISION,
  expected_visible_qty DOUBLE PRECISION,
  actual_visible_qty DOUBLE PRECISION,
  visible_delta_qty DOUBLE PRECISION,
  gap_vs_submit_qty DOUBLE PRECISION,
  gap_vs_fill_qty DOUBLE PRECISION,
  gap_vs_expected_qty DOUBLE PRECISION,
  reference_price DOUBLE PRECISION,
  fee_rate_bps BIGINT,
  fill_to_inventory_ms BIGINT,
  payload_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_trade_builder_inventory_observations_parent_kind
  ON trade_builder_inventory_observations (parent_builder_order_id, observation_kind);

CREATE INDEX IF NOT EXISTS idx_trade_builder_inventory_observations_kind_created
  ON trade_builder_inventory_observations (observation_kind, created_at);

CREATE INDEX IF NOT EXISTS idx_trade_builder_inventory_observations_user_token
  ON trade_builder_inventory_observations (user_id, token_id, created_at);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_inventory_observation_kind'
  ) THEN
    ALTER TABLE trade_builder_inventory_observations
      ADD CONSTRAINT chk_trade_builder_inventory_observation_kind
      CHECK (
        observation_kind IN (
          'buy_inventory_baseline',
          'buy_submit_dynamic_qty',
          'buy_fill_resolution',
          'first_visible_inventory'
        )
      );
  END IF;
END
$$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_inventory_observation_fee_rate_bps'
  ) THEN
    ALTER TABLE trade_builder_inventory_observations
      ADD CONSTRAINT chk_trade_builder_inventory_observation_fee_rate_bps
      CHECK (fee_rate_bps IS NULL OR fee_rate_bps >= 0);
  END IF;
END
$$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_trade_builder_inventory_observation_fill_to_inventory_ms'
  ) THEN
    ALTER TABLE trade_builder_inventory_observations
      ADD CONSTRAINT chk_trade_builder_inventory_observation_fill_to_inventory_ms
      CHECK (fill_to_inventory_ms IS NULL OR fill_to_inventory_ms >= 0);
  END IF;
END
$$;
