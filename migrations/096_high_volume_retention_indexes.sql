CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trade_flow_run_steps_created_at_retention
  ON trade_flow_run_steps (created_at);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_trade_flow_events_created_at_retention
  ON trade_flow_events (created_at);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_market_trade_ticks_event_ts_retention
  ON market_trade_ticks (event_ts);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_market_price_second_snapshots_second_ts_retention
  ON market_price_second_snapshots (second_ts);
