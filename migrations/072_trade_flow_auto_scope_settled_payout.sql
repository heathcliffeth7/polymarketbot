DO $$
BEGIN
  IF EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_tf_auto_scope_analysis_row_type'
  ) THEN
    ALTER TABLE trade_flow_auto_scope_analysis_rows
      DROP CONSTRAINT chk_tf_auto_scope_analysis_row_type;
  END IF;

  ALTER TABLE trade_flow_auto_scope_analysis_rows
    ADD CONSTRAINT chk_tf_auto_scope_analysis_row_type
    CHECK (
      row_type IN ('sell_exit', 'open_position', 'settled_payout')
    );

  IF EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'chk_tf_auto_scope_analysis_valuation_kind'
  ) THEN
    ALTER TABLE trade_flow_auto_scope_analysis_rows
      DROP CONSTRAINT chk_tf_auto_scope_analysis_valuation_kind;
  END IF;

  ALTER TABLE trade_flow_auto_scope_analysis_rows
    ADD CONSTRAINT chk_tf_auto_scope_analysis_valuation_kind
    CHECK (
      valuation_kind IS NULL
      OR valuation_kind IN ('realized', 'mark_to_market', 'settled')
    );
END
$$;
