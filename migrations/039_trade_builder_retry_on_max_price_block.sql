-- Allow max-price guard to move orders to guard_blocked / waiting instead of
-- canceling immediately, mirroring the existing trigger-price and
-- execution-floor retry behaviour.

ALTER TABLE trade_builder_orders
  ADD COLUMN IF NOT EXISTS retry_on_max_price_block BOOLEAN NOT NULL DEFAULT FALSE;
