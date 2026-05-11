-- Fix: parent_step_id self-referencing FK (ON DELETE SET NULL) causes O(N²) sequential scan
-- during cascade deletes because there is no index on parent_step_id.
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trade_flow_run_steps_parent
ON trade_flow_run_steps (parent_step_id)
WHERE parent_step_id IS NOT NULL;
