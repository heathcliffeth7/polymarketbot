CREATE UNIQUE INDEX CONCURRENTLY IF NOT EXISTS uq_trade_flow_definitions_user_name_active
  ON trade_flow_definitions (user_id, LOWER(name))
  WHERE status <> 'archived';
