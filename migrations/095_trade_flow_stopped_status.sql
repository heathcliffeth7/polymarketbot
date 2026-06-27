ALTER TABLE trade_flow_definitions
  DROP CONSTRAINT IF EXISTS chk_trade_flow_definition_status;

ALTER TABLE trade_flow_definitions
  ADD CONSTRAINT chk_trade_flow_definition_status
  CHECK (status IN ('draft', 'published', 'stopped', 'archived'));
