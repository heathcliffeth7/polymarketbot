use super::super::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const POSITIVE_GRID_BUY_EXECUTION_LOCK_NAMESPACE: i32 = 42_077;

pub struct PositiveQuantityFlipGridBuyExecutionLock {
    conn: PoolConnection<Postgres>,
    lock_key1: i32,
    lock_key2: i32,
}

pub fn positive_quantity_flip_grid_buy_execution_lock_keys(
    user_id: i64,
    flow_definition_id: i64,
    root_flow_node_key: &str,
    market_slug: &str,
) -> (i32, i32) {
    let mut hasher = DefaultHasher::new();
    user_id.hash(&mut hasher);
    flow_definition_id.hash(&mut hasher);
    root_flow_node_key.hash(&mut hasher);
    market_slug.hash(&mut hasher);
    let hash = (hasher.finish() & i32::MAX as u64) as i32;
    (POSITIVE_GRID_BUY_EXECUTION_LOCK_NAMESPACE, hash)
}

impl PositiveQuantityFlipGridBuyExecutionLock {
    pub async fn release(mut self) {
        let _ = sqlx::query("SELECT pg_advisory_unlock($1, $2)")
            .bind(self.lock_key1)
            .bind(self.lock_key2)
            .execute(&mut *self.conn)
            .await;
    }
}

const POSITIVE_QUANTITY_FLIP_GRID_POSITION_COLUMNS: &str = "\
p.parent_builder_order_id, \
p.user_id, \
p.source_trade_id, \
p.market_slug, \
p.token_id, \
p.outcome_label, \
p.baseline_qty, \
p.current_qty, \
p.last_fill_qty, \
p.last_fill_price, \
p.qty_source, \
p.created_at, \
p.updated_at";

impl PostgresRepository {
    pub async fn record_positive_quantity_flip_grid_fill(
        &self,
        input: &TradeBuilderPositiveQuantityFlipGridFillInput,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO trade_builder_positive_quantity_flip_grid_fills \
              (user_id, flow_definition_id, flow_run_id, root_flow_node_key, market_slug, token_id, outcome_label, grid_side, order_side, builder_order_id, parent_builder_order_id, quantity, execution_price, notional_usdc, payload_json, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, NOW(), NOW()) \
             ON CONFLICT (builder_order_id) DO UPDATE SET \
               quantity = EXCLUDED.quantity, \
               execution_price = EXCLUDED.execution_price, \
               notional_usdc = EXCLUDED.notional_usdc, \
               payload_json = EXCLUDED.payload_json, \
               updated_at = NOW()",
        )
        .bind(input.user_id)
        .bind(input.flow_definition_id)
        .bind(input.flow_run_id)
        .bind(&input.root_flow_node_key)
        .bind(&input.market_slug)
        .bind(&input.token_id)
        .bind(&input.outcome_label)
        .bind(&input.grid_side)
        .bind(&input.order_side)
        .bind(input.builder_order_id)
        .bind(input.parent_builder_order_id)
        .bind(input.quantity)
        .bind(input.execution_price)
        .bind(input.notional_usdc)
        .bind(&input.payload_json)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn load_positive_quantity_flip_grid_state(
        &self,
        user_id: i64,
        flow_definition_id: Option<i64>,
        root_flow_node_key: &str,
        market_slug: &str,
    ) -> Result<TradeBuilderPositiveQuantityFlipGridState> {
        let row = sqlx::query(
            "WITH fill_totals AS ( \
               SELECT \
                 COALESCE(SUM(CASE WHEN grid_side = 'up' THEN CASE WHEN order_side = 'buy' THEN quantity ELSE -quantity END ELSE 0 END), 0)::double precision AS fill_up_qty, \
                 COALESCE(SUM(CASE WHEN grid_side = 'down' THEN CASE WHEN order_side = 'buy' THEN quantity ELSE -quantity END ELSE 0 END), 0)::double precision AS fill_down_qty, \
                 COALESCE(SUM(CASE WHEN order_side = 'buy' THEN notional_usdc ELSE 0 END), 0)::double precision AS total_buy_cost, \
                 COALESCE(SUM(CASE WHEN order_side = 'sell' THEN notional_usdc ELSE 0 END), 0)::double precision AS total_sell_revenue, \
                 COALESCE(COUNT(*) FILTER (WHERE order_side = 'buy'), 0)::bigint AS buy_count \
               FROM trade_builder_positive_quantity_flip_grid_fills \
               WHERE user_id = $1 \
                 AND flow_definition_id IS NOT DISTINCT FROM $2 \
                 AND root_flow_node_key = $3 \
                 AND market_slug = $4 \
             ), merge_totals AS ( \
               SELECT \
                 COALESCE(SUM(quantity), 0)::double precision AS merged_qty, \
                 COALESCE(SUM(returned_usdc), 0)::double precision AS total_merge_return \
               FROM trade_builder_positive_quantity_flip_grid_merges \
               WHERE user_id = $1 \
                 AND flow_definition_id IS NOT DISTINCT FROM $2 \
                 AND root_flow_node_key = $3 \
                 AND market_slug = $4 \
             ), latest_balance_failure AS ( \
               SELECT order_id, grid_side \
               FROM ( \
                 SELECT \
                   o.id AS order_id, \
                   ns.snapshot_json->'action_node'->'config'->>'positiveQuantityFlipGridSide' AS grid_side, \
                   COALESCE(o.updated_at, o.created_at) AS sort_at \
                 FROM trade_builder_orders o \
                 JOIN trade_builder_order_node_snapshots ns ON ns.order_id = o.id \
                 WHERE o.user_id = $1 \
                   AND o.origin_flow_definition_id IS NOT DISTINCT FROM $2 \
                   AND o.origin_flow_node_key = $3 \
                   AND o.market_slug = $4 \
                   AND o.side = 'buy' \
                   AND o.status = 'error' \
                   AND COALESCE((ns.snapshot_json->'action_node'->'config'->>'positiveQuantityFlipGridOrder')::boolean, FALSE) \
                   AND ns.snapshot_json->'action_node'->'config'->>'positiveQuantityFlipGridRootNodeKey' = $3 \
                   AND ns.snapshot_json->'action_node'->'config'->>'positiveQuantityFlipGridSide' IN ('up', 'down') \
                   AND ( \
                     LOWER(COALESCE(o.last_error, '')) LIKE '%not enough balance / allowance%' \
                     OR LOWER(COALESCE(o.last_error, '')) LIKE '%balance is not enough%' \
                   ) \
               ) failures \
               ORDER BY sort_at DESC, order_id DESC \
               LIMIT 1 \
             ) \
             SELECT \
               fill_up_qty - merged_qty AS up_qty, \
               fill_down_qty - merged_qty AS down_qty, \
               total_buy_cost, \
               total_sell_revenue, \
               total_merge_return, \
               buy_count, \
               (SELECT recent.grid_side \
                FROM ( \
                  SELECT f.grid_side, f.created_at, f.id \
                  FROM trade_builder_positive_quantity_flip_grid_fills f \
                  WHERE f.user_id = $1 \
                    AND f.flow_definition_id IS NOT DISTINCT FROM $2 \
                    AND f.root_flow_node_key = $3 \
                    AND f.market_slug = $4 \
                    AND f.order_side = 'buy' \
                  UNION ALL \
                  SELECT ns.snapshot_json->'action_node'->'config'->>'positiveQuantityFlipGridSide' AS grid_side, o.created_at, o.id \
                  FROM trade_builder_orders o \
                  JOIN trade_builder_order_node_snapshots ns ON ns.order_id = o.id \
                  WHERE o.user_id = $1 \
                    AND o.origin_flow_definition_id IS NOT DISTINCT FROM $2 \
                    AND o.origin_flow_node_key = $3 \
                    AND o.market_slug = $4 \
                    AND o.side = 'buy' \
                    AND o.status IN ('pending', 'armed', 'triggered', 'open', 'partially_filled', 'filled', 'completed') \
                    AND COALESCE((ns.snapshot_json->'action_node'->'config'->>'positiveQuantityFlipGridOrder')::boolean, FALSE) \
                    AND ns.snapshot_json->'action_node'->'config'->>'positiveQuantityFlipGridRootNodeKey' = $3 \
                    AND ns.snapshot_json->'action_node'->'config'->>'positiveQuantityFlipGridIntent' IN ('buy', 'core_positive', 'flip_positive', 'pairlock_compression_buy') \
                    AND ns.snapshot_json->'action_node'->'config'->>'positiveQuantityFlipGridSide' IN ('up', 'down') \
                ) recent \
                ORDER BY recent.created_at DESC, recent.id DESC \
                LIMIT 1) AS last_buy_grid_side, \
               (SELECT order_id FROM latest_balance_failure) AS last_balance_failure_order_id, \
               (SELECT grid_side FROM latest_balance_failure) AS last_balance_failure_grid_side \
             FROM fill_totals, merge_totals",
        )
        .bind(user_id)
        .bind(flow_definition_id)
        .bind(root_flow_node_key)
        .bind(market_slug)
        .fetch_one(self.pool())
        .await?;

        let total_buy_cost: f64 = row.get("total_buy_cost");
        let total_sell_revenue: f64 = row.get("total_sell_revenue");
        let total_merge_return: f64 = row.get("total_merge_return");
        Ok(TradeBuilderPositiveQuantityFlipGridState {
            up_qty: f64::max(row.get("up_qty"), 0.0),
            down_qty: f64::max(row.get("down_qty"), 0.0),
            total_buy_cost,
            total_sell_revenue,
            total_merge_return,
            net_cost: total_buy_cost - total_sell_revenue - total_merge_return,
            buy_count: row.get("buy_count"),
            last_buy_grid_side: row.get("last_buy_grid_side"),
            last_balance_failure_order_id: row.get("last_balance_failure_order_id"),
            last_balance_failure_grid_side: row.get("last_balance_failure_grid_side"),
        })
    }

    pub async fn list_open_positive_quantity_flip_grid_lots(
        &self,
        user_id: i64,
        flow_definition_id: Option<i64>,
        root_flow_node_key: &str,
        market_slug: &str,
    ) -> Result<Vec<TradeBuilderPositiveQuantityFlipGridLot>> {
        let rows = sqlx::query(
            "SELECT \
               p.parent_builder_order_id, \
               f.market_slug, \
               f.token_id, \
               f.outcome_label, \
               f.grid_side, \
               COALESCE(f.payload_json->>'intent', 'buy') AS intent, \
               p.current_qty::double precision AS quantity, \
               f.execution_price::double precision AS execution_price, \
               f.notional_usdc::double precision AS notional_usdc, \
               f.created_at \
             FROM trade_builder_positive_quantity_flip_grid_fills f \
             JOIN trade_builder_parent_positions p \
               ON p.parent_builder_order_id = COALESCE(f.parent_builder_order_id, f.builder_order_id) \
              AND p.current_qty > 0.0001 \
             WHERE f.user_id = $1 \
               AND f.flow_definition_id IS NOT DISTINCT FROM $2 \
               AND f.root_flow_node_key = $3 \
               AND f.market_slug = $4 \
               AND f.order_side = 'buy' \
             ORDER BY f.created_at ASC, f.id ASC",
        )
        .bind(user_id)
        .bind(flow_definition_id)
        .bind(root_flow_node_key)
        .bind(market_slug)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeBuilderPositiveQuantityFlipGridLot {
                parent_builder_order_id: row.get("parent_builder_order_id"),
                market_slug: row.get("market_slug"),
                token_id: row.get("token_id"),
                outcome_label: row.get("outcome_label"),
                grid_side: row.get("grid_side"),
                intent: row.get("intent"),
                quantity: row.get("quantity"),
                execution_price: row.get("execution_price"),
                notional_usdc: row.get("notional_usdc"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn list_active_positive_quantity_flip_grid_buys(
        &self,
        user_id: i64,
        flow_definition_id: Option<i64>,
        root_flow_node_key: &str,
        market_slug: &str,
    ) -> Result<Vec<TradeBuilderPositiveQuantityFlipGridActiveBuy>> {
        let rows = sqlx::query(
            "SELECT \
               o.id AS order_id, \
               o.status, \
               ns.snapshot_json->'action_node'->'config'->>'positiveQuantityFlipGridSide' AS grid_side, \
               o.outcome_label, \
               COALESCE(o.size_usdc, 0)::double precision AS size_usdc, \
               COALESCE(o.target_qty, 0)::double precision AS target_qty, \
               o.created_at \
             FROM trade_builder_orders o \
             JOIN trade_builder_order_node_snapshots ns ON ns.order_id = o.id \
             WHERE o.user_id = $1 \
               AND o.origin_flow_definition_id IS NOT DISTINCT FROM $2 \
               AND o.origin_flow_node_key = $3 \
               AND o.market_slug = $4 \
               AND o.side = 'buy' \
               AND o.status IN ('pending', 'armed', 'triggered', 'open', 'partially_filled') \
               AND COALESCE((ns.snapshot_json->'action_node'->'config'->>'positiveQuantityFlipGridOrder')::boolean, FALSE) \
               AND ns.snapshot_json->'action_node'->'config'->>'positiveQuantityFlipGridRootNodeKey' = $3 \
               AND ns.snapshot_json->'action_node'->'config'->>'positiveQuantityFlipGridSide' IN ('up', 'down') \
             ORDER BY o.created_at ASC, o.id ASC",
        )
        .bind(user_id)
        .bind(flow_definition_id)
        .bind(root_flow_node_key)
        .bind(market_slug)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeBuilderPositiveQuantityFlipGridActiveBuy {
                order_id: row.get("order_id"),
                status: row.get("status"),
                grid_side: row.get("grid_side"),
                outcome_label: row.get("outcome_label"),
                size_usdc: row.get("size_usdc"),
                target_qty: row.get("target_qty"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn record_positive_quantity_flip_grid_merge(
        &self,
        input: &TradeBuilderPositiveQuantityFlipGridMergeInput,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO trade_builder_positive_quantity_flip_grid_merges \
              (user_id, flow_definition_id, flow_run_id, root_flow_node_key, market_slug, condition_id, quantity, returned_usdc, tx_hash, submission_mode, payload_json, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NOW(), NOW()) \
             ON CONFLICT (tx_hash) DO NOTHING \
             RETURNING id",
        )
        .bind(input.user_id)
        .bind(input.flow_definition_id)
        .bind(input.flow_run_id)
        .bind(&input.root_flow_node_key)
        .bind(&input.market_slug)
        .bind(&input.condition_id)
        .bind(input.quantity)
        .bind(input.returned_usdc)
        .bind(&input.tx_hash)
        .bind(&input.submission_mode)
        .bind(&input.payload_json)
        .fetch_optional(&mut *tx)
        .await?;
        if inserted.is_none() {
            tx.commit().await?;
            return Ok(());
        }

        for leg in input.up_legs.iter().chain(input.down_legs.iter()) {
            sqlx::query(
                "UPDATE trade_builder_parent_positions \
                 SET current_qty = GREATEST(0, current_qty - $2), \
                     last_fill_qty = $2, \
                     last_fill_price = NULL, \
                     qty_source = 'positive_flip_pairlock_merge', \
                     updated_at = NOW() \
                 WHERE parent_builder_order_id = $1",
            )
            .bind(leg.parent_builder_order_id)
            .bind(leg.quantity.max(0.0))
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn list_open_positive_quantity_flip_grid_positions(
        &self,
        user_id: i64,
        flow_definition_id: Option<i64>,
        root_flow_node_key: &str,
        market_slug: &str,
        grid_side: &str,
    ) -> Result<Vec<TradeBuilderParentPosition>> {
        let rows = sqlx::query(&format!(
            "SELECT DISTINCT {cols} \
             FROM trade_builder_parent_positions p \
             JOIN trade_builder_positive_quantity_flip_grid_fills f \
               ON f.builder_order_id = p.parent_builder_order_id \
              AND f.order_side = 'buy' \
             WHERE f.user_id = $1 \
               AND f.flow_definition_id IS NOT DISTINCT FROM $2 \
               AND f.root_flow_node_key = $3 \
               AND f.market_slug = $4 \
               AND f.grid_side = $5 \
               AND p.current_qty > 0.0001 \
             ORDER BY p.updated_at ASC",
            cols = POSITIVE_QUANTITY_FLIP_GRID_POSITION_COLUMNS,
        ))
        .bind(user_id)
        .bind(flow_definition_id)
        .bind(root_flow_node_key)
        .bind(market_slug)
        .bind(grid_side)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeBuilderParentPosition {
                parent_builder_order_id: row.get("parent_builder_order_id"),
                user_id: row.get("user_id"),
                source_trade_id: row.get("source_trade_id"),
                market_slug: row.get("market_slug"),
                token_id: row.get("token_id"),
                outcome_label: row.get("outcome_label"),
                baseline_qty: row.get("baseline_qty"),
                current_qty: row.get("current_qty"),
                last_fill_qty: row.get("last_fill_qty"),
                last_fill_price: row.get("last_fill_price"),
                qty_source: row.get("qty_source"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn positive_quantity_flip_grid_open_market_usage(
        &self,
        user_id: i64,
        flow_definition_id: Option<i64>,
        root_flow_node_key: &str,
        market_slug: &str,
    ) -> Result<(i64, bool)> {
        let row = sqlx::query(
            "WITH open_markets AS ( \
               SELECT DISTINCT f.market_slug \
               FROM trade_builder_positive_quantity_flip_grid_fills f \
               JOIN trade_builder_parent_positions p \
                 ON p.parent_builder_order_id = f.builder_order_id \
                AND p.current_qty > 0.0001 \
               WHERE f.user_id = $1 \
                 AND f.flow_definition_id IS NOT DISTINCT FROM $2 \
                 AND f.root_flow_node_key = $3 \
                 AND f.order_side = 'buy' \
             ), active_window_markets AS ( \
               SELECT market_slug \
               FROM open_markets \
               WHERE COALESCE( \
                 ((regexp_match(market_slug, '([0-9]{10})$'))[1])::double precision + 300.0 > EXTRACT(EPOCH FROM NOW()), \
                 TRUE \
               ) \
             ) \
             SELECT \
               COUNT(DISTINCT market_slug)::bigint AS active_market_count, \
               COALESCE(BOOL_OR(market_slug = $4), FALSE) AS current_market_active \
             FROM active_window_markets",
        )
        .bind(user_id)
        .bind(flow_definition_id)
        .bind(root_flow_node_key)
        .bind(market_slug)
        .fetch_one(self.pool())
        .await?;

        Ok((
            row.get("active_market_count"),
            row.get("current_market_active"),
        ))
    }

    pub async fn try_acquire_positive_quantity_flip_grid_buy_execution_lock(
        &self,
        user_id: i64,
        flow_definition_id: i64,
        root_flow_node_key: &str,
        market_slug: &str,
    ) -> Result<Option<PositiveQuantityFlipGridBuyExecutionLock>> {
        let (lock_key1, lock_key2) = positive_quantity_flip_grid_buy_execution_lock_keys(
            user_id,
            flow_definition_id,
            root_flow_node_key,
            market_slug,
        );
        let mut conn = self.pool().acquire().await?;
        let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1, $2)")
            .bind(lock_key1)
            .bind(lock_key2)
            .fetch_one(&mut *conn)
            .await?;
        if acquired {
            Ok(Some(PositiveQuantityFlipGridBuyExecutionLock {
                conn,
                lock_key1,
                lock_key2,
            }))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_quantity_flip_grid_position_columns_are_qualified() {
        for column in POSITIVE_QUANTITY_FLIP_GRID_POSITION_COLUMNS
            .split(',')
            .map(str::trim)
        {
            assert!(
                column.starts_with("p."),
                "open-position column must be selected from parent position alias: {column}"
            );
        }
        assert!(POSITIVE_QUANTITY_FLIP_GRID_POSITION_COLUMNS.contains("p.user_id"));
    }

    #[test]
    fn buy_execution_lock_keys_are_stable_and_market_scoped() {
        let same = positive_quantity_flip_grid_buy_execution_lock_keys(
            1,
            4327,
            "action_positive_grid_buy",
            "btc-updown-5m-1779973200",
        );
        let repeat = positive_quantity_flip_grid_buy_execution_lock_keys(
            1,
            4327,
            "action_positive_grid_buy",
            "btc-updown-5m-1779973200",
        );
        let other_market = positive_quantity_flip_grid_buy_execution_lock_keys(
            1,
            4327,
            "action_positive_grid_buy",
            "btc-updown-5m-1779971700",
        );
        assert_eq!(same, repeat);
        assert_ne!(same, other_market);
        assert_eq!(same.0, POSITIVE_GRID_BUY_EXECUTION_LOCK_NAMESPACE);
    }
}
