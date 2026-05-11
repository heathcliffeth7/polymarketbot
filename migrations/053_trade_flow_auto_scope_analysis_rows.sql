CREATE TABLE IF NOT EXISTS trade_flow_auto_scope_analysis_rows (
  row_key TEXT PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
  definition_id BIGINT NOT NULL REFERENCES trade_flow_definitions(id) ON DELETE CASCADE,
  run_id BIGINT NOT NULL REFERENCES trade_flow_runs(id) ON DELETE CASCADE,
  root_builder_order_id BIGINT NOT NULL REFERENCES trade_builder_orders(id) ON DELETE CASCADE,
  exit_builder_order_id BIGINT REFERENCES trade_builder_orders(id) ON DELETE CASCADE,
  row_type TEXT NOT NULL,
  market_slug TEXT NOT NULL,
  token_id TEXT NOT NULL,
  outcome_label TEXT NOT NULL,
  exit_reason TEXT NOT NULL,
  market_open_at TIMESTAMPTZ,
  triggered_at TIMESTAMPTZ,
  buy_filled_at TIMESTAMPTZ,
  sell_filled_at TIMESTAMPTZ,
  open_to_trigger_ms BIGINT,
  trigger_to_buy_fill_ms BIGINT,
  buy_avg_price DOUBLE PRECISION,
  mark_or_sell_price DOUBLE PRECISION,
  mark_price_captured_at TIMESTAMPTZ,
  row_qty DOUBLE PRECISION NOT NULL,
  remaining_qty_after_exit DOUBLE PRECISION NOT NULL,
  row_pnl_usdc DOUBLE PRECISION NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT chk_tf_auto_scope_analysis_row_type CHECK (
    row_type IN ('sell_exit', 'open_position')
  ),
  CONSTRAINT chk_tf_auto_scope_analysis_exit_reason CHECK (
    exit_reason IN ('tp', 'sl', 'window_end_auto_sell', 'other', 'open_position')
  )
);

CREATE INDEX IF NOT EXISTS idx_tf_auto_scope_analysis_user_triggered
  ON trade_flow_auto_scope_analysis_rows(user_id, triggered_at DESC, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_tf_auto_scope_analysis_root
  ON trade_flow_auto_scope_analysis_rows(root_builder_order_id);

CREATE INDEX IF NOT EXISTS idx_tf_auto_scope_analysis_run
  ON trade_flow_auto_scope_analysis_rows(run_id);
