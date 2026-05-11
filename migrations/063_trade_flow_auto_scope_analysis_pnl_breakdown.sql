ALTER TABLE trade_flow_auto_scope_analysis_rows
  ADD COLUMN IF NOT EXISTS buy_notional_usdc DOUBLE PRECISION,
  ADD COLUMN IF NOT EXISTS buy_fee_usdc DOUBLE PRECISION,
  ADD COLUMN IF NOT EXISTS cost_basis_usdc DOUBLE PRECISION,
  ADD COLUMN IF NOT EXISTS sell_notional_usdc DOUBLE PRECISION,
  ADD COLUMN IF NOT EXISTS sell_fee_usdc DOUBLE PRECISION,
  ADD COLUMN IF NOT EXISTS mark_value_usdc DOUBLE PRECISION,
  ADD COLUMN IF NOT EXISTS net_value_usdc DOUBLE PRECISION,
  ADD COLUMN IF NOT EXISTS pnl_pct DOUBLE PRECISION,
  ADD COLUMN IF NOT EXISTS valuation_kind TEXT;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_tf_auto_scope_analysis_valuation_kind'
  ) THEN
    ALTER TABLE trade_flow_auto_scope_analysis_rows
      ADD CONSTRAINT chk_tf_auto_scope_analysis_valuation_kind
      CHECK (
        valuation_kind IS NULL
        OR valuation_kind IN ('realized', 'mark_to_market')
      );
  END IF;
END
$$;

CREATE INDEX IF NOT EXISTS idx_tf_auto_scope_analysis_user_pnl
  ON trade_flow_auto_scope_analysis_rows(user_id, row_pnl_usdc);
