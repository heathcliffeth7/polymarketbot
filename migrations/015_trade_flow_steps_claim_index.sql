CREATE INDEX IF NOT EXISTS idx_trade_flow_run_steps_claim_queue
  ON trade_flow_run_steps(status, available_at, id)
  WHERE status = 'queued';
