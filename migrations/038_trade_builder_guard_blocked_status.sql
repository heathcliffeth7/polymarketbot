-- Add 'guard_blocked' to chk_trade_builder_status constraint.
-- The Rust code uses "guard_blocked" status via set_trade_builder_guard_blocked_state()
-- but the constraint was never updated to include it.

DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'chk_trade_builder_status'
  ) THEN
    ALTER TABLE trade_builder_orders DROP CONSTRAINT chk_trade_builder_status;
  END IF;
END
$$;

ALTER TABLE trade_builder_orders
  ADD CONSTRAINT chk_trade_builder_status
  CHECK (
    status = ANY (
      ARRAY[
        'pending'::text,
        'armed'::text,
        'triggered'::text,
        'open'::text,
        'partially_filled'::text,
        'filled'::text,
        'canceled_requested'::text,
        'completed'::text,
        'canceled'::text,
        'expired'::text,
        'blocked'::text,
        'inventory_pending'::text,
        'error'::text,
        'guard_blocked'::text
      ]
    )
  );
