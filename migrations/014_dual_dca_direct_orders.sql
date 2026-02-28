-- DCA Direct Market Orders: Add columns to track CLOB orders directly on legs
-- instead of going through trade_builder_orders intermediary.

ALTER TABLE trade_flow_dual_dca_legs
  ADD COLUMN IF NOT EXISTS active_exchange_order_id TEXT,
  ADD COLUMN IF NOT EXISTS client_order_id TEXT,
  ADD COLUMN IF NOT EXISTS filled_price DOUBLE PRECISION,
  ADD COLUMN IF NOT EXISTS filled_size DOUBLE PRECISION,
  ADD COLUMN IF NOT EXISTS submitted_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS filled_at TIMESTAMPTZ;

-- Update status constraint to include new states for direct order lifecycle
ALTER TABLE trade_flow_dual_dca_legs
  DROP CONSTRAINT IF EXISTS chk_tf_dual_dca_leg_status;
ALTER TABLE trade_flow_dual_dca_legs
  ADD CONSTRAINT chk_tf_dual_dca_leg_status
  CHECK (status IN ('created', 'pending', 'submitted', 'open', 'filled', 'canceled', 'error'));

-- Index for quickly finding legs with active CLOB orders
CREATE INDEX IF NOT EXISTS idx_tf_dual_dca_legs_active_order
  ON trade_flow_dual_dca_legs(active_exchange_order_id)
  WHERE active_exchange_order_id IS NOT NULL;

-- Index for finding next pending leg per job+market+outcome
CREATE INDEX IF NOT EXISTS idx_tf_dual_dca_legs_pending
  ON trade_flow_dual_dca_legs(job_id, market_slug, outcome_label, level_index)
  WHERE status = 'pending';
