ALTER TABLE trade_builder_inventory_observations
  DROP CONSTRAINT IF EXISTS chk_trade_builder_inventory_observation_kind;

ALTER TABLE trade_builder_inventory_observations
  ADD CONSTRAINT chk_trade_builder_inventory_observation_kind
  CHECK (
    observation_kind IN (
      'buy_inventory_baseline',
      'buy_submit_dynamic_qty',
      'buy_fill_resolution',
      'first_visible_inventory',
      'first_visible_inventory_terminal_not_visible'
    )
  );
