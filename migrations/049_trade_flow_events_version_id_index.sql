CREATE INDEX IF NOT EXISTS idx_trade_flow_events_version_id_not_null
  ON trade_flow_events (version_id)
  WHERE version_id IS NOT NULL;
