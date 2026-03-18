DELETE FROM trade_flow_runs
WHERE definition_id IN (
  SELECT id
  FROM trade_flow_definitions
  WHERE status = 'archived'
);

DELETE FROM trade_flow_definitions
WHERE status = 'archived';
