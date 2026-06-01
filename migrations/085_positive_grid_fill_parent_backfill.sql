UPDATE trade_builder_positive_quantity_flip_grid_fills f
SET parent_builder_order_id = f.builder_order_id,
    updated_at = NOW()
WHERE f.parent_builder_order_id IS NULL
  AND f.order_side = 'buy'
  AND EXISTS (
    SELECT 1
    FROM trade_builder_parent_positions p
    WHERE p.parent_builder_order_id = f.builder_order_id
  );
