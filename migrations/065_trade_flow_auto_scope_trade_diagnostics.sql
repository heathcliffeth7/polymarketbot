CREATE TABLE IF NOT EXISTS trade_flow_auto_scope_trade_diagnostics (
  root_builder_order_id BIGINT PRIMARY KEY,
  user_id BIGINT NOT NULL,
  definition_id BIGINT NOT NULL,
  run_id BIGINT NOT NULL,
  market_slug TEXT NOT NULL,
  token_id TEXT NOT NULL,
  outcome_label TEXT NOT NULL,
  total_pnl_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
  realized_pnl_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
  open_pnl_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
  pnl_pct DOUBLE PRECISION,
  fee_drag_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
  cost_basis_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
  net_value_usdc DOUBLE PRECISION NOT NULL DEFAULT 0,
  entry_trigger_price DOUBLE PRECISION,
  entry_submit_price DOUBLE PRECISION,
  entry_fill_price DOUBLE PRECISION,
  entry_reference_price DOUBLE PRECISION,
  entry_slippage_usdc DOUBLE PRECISION,
  entry_quality_score DOUBLE PRECISION,
  exit_reason TEXT,
  exit_price DOUBLE PRECISION,
  best_price_during_hold DOUBLE PRECISION,
  worst_price_during_hold DOUBLE PRECISION,
  max_favorable_usdc DOUBLE PRECISION,
  max_adverse_usdc DOUBLE PRECISION,
  gave_back_usdc DOUBLE PRECISION,
  exit_quality_score DOUBLE PRECISION,
  open_to_trigger_ms BIGINT,
  trigger_to_buy_fill_ms BIGINT,
  trigger_to_submit_ms BIGINT,
  submit_to_fill_ms BIGINT,
  hold_ms BIGINT,
  snapshot_age_ms BIGINT,
  runtime_price_fetch_ms BIGINT,
  guard_eval_ms BIGINT,
  place_http_ms BIGINT,
  primary_diagnosis_code TEXT NOT NULL DEFAULT 'unknown',
  secondary_diagnosis_code TEXT,
  diagnosis_label TEXT NOT NULL DEFAULT 'Veri yetersiz',
  diagnosis_detail TEXT NOT NULL DEFAULT '',
  data_quality_flags TEXT[] NOT NULL DEFAULT '{}'::text[],
  compact_metrics_json JSONB NOT NULL DEFAULT '{}'::jsonb,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_tf_auto_scope_diagnostic_primary_code'
  ) THEN
    ALTER TABLE trade_flow_auto_scope_trade_diagnostics
      ADD CONSTRAINT chk_tf_auto_scope_diagnostic_primary_code
      CHECK (
        primary_diagnosis_code IN (
          'bad_entry_price',
          'late_entry',
          'slow_fill',
          'thin_liquidity',
          'stop_loss_expected',
          'exit_too_late',
          'market_reversal',
          'fee_drag',
          'unrealized_mark_loss',
          'take_profit_success',
          'clean_win',
          'unknown'
        )
      );
  END IF;

  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_tf_auto_scope_diagnostic_secondary_code'
  ) THEN
    ALTER TABLE trade_flow_auto_scope_trade_diagnostics
      ADD CONSTRAINT chk_tf_auto_scope_diagnostic_secondary_code
      CHECK (
        secondary_diagnosis_code IS NULL
        OR secondary_diagnosis_code IN (
          'bad_entry_price',
          'late_entry',
          'slow_fill',
          'thin_liquidity',
          'stop_loss_expected',
          'exit_too_late',
          'market_reversal',
          'fee_drag',
          'unrealized_mark_loss',
          'take_profit_success',
          'clean_win',
          'unknown'
        )
      );
  END IF;
END
$$;

CREATE INDEX IF NOT EXISTS idx_tf_auto_scope_diagnostics_user_updated
  ON trade_flow_auto_scope_trade_diagnostics(user_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_tf_auto_scope_diagnostics_user_diagnosis
  ON trade_flow_auto_scope_trade_diagnostics(user_id, primary_diagnosis_code, total_pnl_usdc);

CREATE INDEX IF NOT EXISTS idx_tf_auto_scope_diagnostics_run
  ON trade_flow_auto_scope_trade_diagnostics(run_id);
