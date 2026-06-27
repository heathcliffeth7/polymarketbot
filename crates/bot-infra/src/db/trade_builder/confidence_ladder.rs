use super::super::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const CONFIDENCE_LADDER_EXECUTION_LOCK_NAMESPACE: i32 = 42_089;

pub struct TradeBuilderConfidenceLadderExecutionLock {
    conn: PoolConnection<Postgres>,
    lock_key1: i32,
    lock_key2: i32,
}

pub fn trade_builder_confidence_ladder_execution_lock_keys(
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
    (CONFIDENCE_LADDER_EXECUTION_LOCK_NAMESPACE, hash)
}

impl TradeBuilderConfidenceLadderExecutionLock {
    pub async fn release(mut self) {
        let _ = sqlx::query("SELECT pg_advisory_unlock($1, $2)")
            .bind(self.lock_key1)
            .bind(self.lock_key2)
            .execute(&mut *self.conn)
            .await;
    }
}

impl PostgresRepository {
    pub async fn try_acquire_trade_builder_confidence_ladder_lock(
        &self,
        user_id: i64,
        flow_definition_id: i64,
        root_flow_node_key: &str,
        market_slug: &str,
    ) -> Result<Option<TradeBuilderConfidenceLadderExecutionLock>> {
        let (lock_key1, lock_key2) = trade_builder_confidence_ladder_execution_lock_keys(
            user_id,
            flow_definition_id,
            root_flow_node_key,
            market_slug,
        );
        let mut conn = self.pool().acquire().await?;
        let row = sqlx::query("SELECT pg_try_advisory_lock($1, $2) AS acquired")
            .bind(lock_key1)
            .bind(lock_key2)
            .fetch_one(&mut *conn)
            .await?;
        if row.get::<bool, _>("acquired") {
            Ok(Some(TradeBuilderConfidenceLadderExecutionLock {
                conn,
                lock_key1,
                lock_key2,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn has_active_trade_builder_confidence_ladder_order(
        &self,
        user_id: i64,
        flow_definition_id: Option<i64>,
        root_flow_node_key: &str,
        market_slug: &str,
    ) -> Result<bool> {
        let row = sqlx::query(
            "SELECT EXISTS ( \
               SELECT 1 \
               FROM trade_builder_orders o \
               JOIN trade_builder_order_node_snapshots ns ON ns.order_id = o.id \
               WHERE o.user_id = $1 \
                 AND o.origin_flow_definition_id IS NOT DISTINCT FROM $2 \
                 AND o.origin_flow_node_key = $3 \
                 AND o.market_slug = $4 \
                 AND o.status IN ('pending', 'armed', 'triggered', 'open', 'partially_filled') \
                 AND COALESCE((ns.snapshot_json->'action_node'->'config'->>'confidenceLadderOrder')::boolean, FALSE) \
                 AND ns.snapshot_json->'action_node'->'config'->>'confidenceLadderRootNodeKey' = $3 \
             ) AS active",
        )
        .bind(user_id)
        .bind(flow_definition_id)
        .bind(root_flow_node_key)
        .bind(market_slug)
        .fetch_one(self.pool())
        .await?;
        Ok(row.get("active"))
    }

    pub async fn record_trade_builder_confidence_ladder_fill(
        &self,
        input: &TradeBuilderConfidenceLadderFillInput,
    ) -> Result<bool> {
        let result = sqlx::query(
            "INSERT INTO trade_builder_confidence_ladder_fills \
              (user_id, flow_definition_id, flow_run_id, root_flow_node_key, market_slug, token_id, outcome_label, ladder_side, intent, order_side, builder_order_id, parent_builder_order_id, quantity, execution_price, notional_usdc, model_probability, edge, payload_json, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, NOW(), NOW()) \
             ON CONFLICT (builder_order_id) DO NOTHING",
        )
        .bind(input.user_id)
        .bind(input.flow_definition_id)
        .bind(input.flow_run_id)
        .bind(&input.root_flow_node_key)
        .bind(&input.market_slug)
        .bind(&input.token_id)
        .bind(&input.outcome_label)
        .bind(&input.ladder_side)
        .bind(&input.intent)
        .bind(&input.order_side)
        .bind(input.builder_order_id)
        .bind(input.parent_builder_order_id)
        .bind(input.quantity)
        .bind(input.execution_price)
        .bind(input.notional_usdc)
        .bind(input.model_probability)
        .bind(input.edge)
        .bind(&input.payload_json)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn load_trade_builder_confidence_ladder_state(
        &self,
        user_id: i64,
        flow_definition_id: Option<i64>,
        root_flow_node_key: &str,
        market_slug: &str,
    ) -> Result<TradeBuilderConfidenceLadderState> {
        let row = sqlx::query(
            "WITH fills AS ( \
               SELECT * \
               FROM trade_builder_confidence_ladder_fills \
               WHERE user_id = $1 \
                 AND flow_definition_id IS NOT DISTINCT FROM $2 \
                 AND root_flow_node_key = $3 \
                 AND market_slug = $4 \
             ), totals AS ( \
               SELECT \
                 COALESCE(SUM(CASE WHEN ladder_side = 'up' THEN quantity ELSE 0 END), 0)::double precision AS up_qty, \
                 COALESCE(SUM(CASE WHEN ladder_side = 'down' THEN quantity ELSE 0 END), 0)::double precision AS down_qty, \
                 COALESCE(SUM(CASE WHEN ladder_side = 'up' THEN notional_usdc ELSE 0 END), 0)::double precision AS up_cost_usdc, \
                 COALESCE(SUM(CASE WHEN ladder_side = 'down' THEN notional_usdc ELSE 0 END), 0)::double precision AS down_cost_usdc, \
                 COALESCE(SUM(notional_usdc), 0)::double precision AS total_cost_usdc, \
                 COALESCE(COUNT(*), 0)::bigint AS buy_count \
               FROM fills \
             ), side_switches AS ( \
               SELECT COALESCE(SUM(CASE WHEN prev_side IS NOT NULL AND prev_side <> ladder_side THEN 1 ELSE 0 END), 0)::bigint AS side_switch_count \
               FROM ( \
                 SELECT ladder_side, LAG(ladder_side) OVER (ORDER BY created_at, id) AS prev_side \
                 FROM fills \
               ) ordered \
             ), last_buy AS ( \
               SELECT ladder_side \
               FROM fills \
               ORDER BY created_at DESC, id DESC \
               LIMIT 1 \
             ) \
             SELECT totals.*, side_switches.side_switch_count, last_buy.ladder_side AS last_buy_side \
             FROM totals \
             CROSS JOIN side_switches \
             LEFT JOIN last_buy ON TRUE",
        )
        .bind(user_id)
        .bind(flow_definition_id)
        .bind(root_flow_node_key)
        .bind(market_slug)
        .fetch_one(self.pool())
        .await?;

        let up_qty: f64 = row.get("up_qty");
        let down_qty: f64 = row.get("down_qty");
        let up_cost_usdc: f64 = row.get("up_cost_usdc");
        let down_cost_usdc: f64 = row.get("down_cost_usdc");
        let total_cost_usdc: f64 = row.get("total_cost_usdc");
        let if_up_wins_pnl = up_qty - total_cost_usdc;
        let if_down_wins_pnl = down_qty - total_cost_usdc;
        Ok(TradeBuilderConfidenceLadderState {
            up_qty,
            down_qty,
            total_cost_usdc,
            up_cost_usdc,
            down_cost_usdc,
            up_avg_cost: (up_qty > 0.0).then_some(up_cost_usdc / up_qty),
            down_avg_cost: (down_qty > 0.0).then_some(down_cost_usdc / down_qty),
            if_up_wins_pnl,
            if_down_wins_pnl,
            worst_case_pnl: if_up_wins_pnl.min(if_down_wins_pnl),
            buy_count: row.get("buy_count"),
            side_switch_count: row.get("side_switch_count"),
            last_buy_side: row.get("last_buy_side"),
        })
    }
}
