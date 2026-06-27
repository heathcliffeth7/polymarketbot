use super::super::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const AVG_REBOUND_PAIRLOCK_RESCUE_LOCK_NAMESPACE: i32 = 42_091;
const AVG_REBOUND_PAIRLOCK_RESCUE_MODE: &str = "avg_rebound_pairlock_rescue_v1";

pub struct TradeBuilderAvgReboundPairlockRescueExecutionLock {
    conn: PoolConnection<Postgres>,
    lock_key1: i32,
    lock_key2: i32,
}

pub fn trade_builder_avg_rebound_pairlock_rescue_execution_lock_keys(
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
    (AVG_REBOUND_PAIRLOCK_RESCUE_LOCK_NAMESPACE, hash)
}

impl TradeBuilderAvgReboundPairlockRescueExecutionLock {
    pub async fn release(mut self) {
        let _ = sqlx::query("SELECT pg_advisory_unlock($1, $2)")
            .bind(self.lock_key1)
            .bind(self.lock_key2)
            .execute(&mut *self.conn)
            .await;
    }
}

impl PostgresRepository {
    pub async fn try_acquire_trade_builder_avg_rebound_pairlock_rescue_lock(
        &self,
        user_id: i64,
        flow_definition_id: i64,
        root_flow_node_key: &str,
        market_slug: &str,
    ) -> Result<Option<TradeBuilderAvgReboundPairlockRescueExecutionLock>> {
        let (lock_key1, lock_key2) = trade_builder_avg_rebound_pairlock_rescue_execution_lock_keys(
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
            Ok(Some(TradeBuilderAvgReboundPairlockRescueExecutionLock {
                conn,
                lock_key1,
                lock_key2,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_or_create_trade_builder_avg_rebound_pairlock_rescue_session(
        &self,
        input: &TradeBuilderAvgReboundPairlockRescueSessionInput,
    ) -> Result<TradeBuilderAvgReboundPairlockRescueSession> {
        if let Some(session) = self
            .load_active_trade_builder_avg_rebound_pairlock_rescue_session(
                input.user_id,
                input.flow_definition_id,
                &input.root_flow_node_key,
                &input.market_slug,
            )
            .await?
        {
            return Ok(session);
        }

        let row = sqlx::query(
             "INSERT INTO trade_builder_avg_rebound_pairlock_rescue_sessions \
              (user_id, flow_definition_id, flow_run_id, root_flow_node_key, market_slug, mode, status, primary_token_id, primary_outcome_label, opposite_token_id, opposite_outcome_label, payload_json, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, $4, $5, $6, 'BUILDING_PRIMARY', $7, $8, $9, $10, $11, NOW(), NOW()) \
             RETURNING id, status, primary_token_id, primary_outcome_label, opposite_token_id, opposite_outcome_label",
        )
        .bind(input.user_id)
        .bind(input.flow_definition_id)
        .bind(input.flow_run_id)
        .bind(&input.root_flow_node_key)
        .bind(&input.market_slug)
        .bind(AVG_REBOUND_PAIRLOCK_RESCUE_MODE)
        .bind(&input.primary_token_id)
        .bind(&input.primary_outcome_label)
        .bind(&input.opposite_token_id)
        .bind(&input.opposite_outcome_label)
        .bind(&input.payload_json)
        .fetch_one(self.pool())
        .await?;
        Ok(TradeBuilderAvgReboundPairlockRescueSession {
            id: row.get("id"),
            status: row.get("status"),
            primary_token_id: row.get("primary_token_id"),
            primary_outcome_label: row.get("primary_outcome_label"),
            opposite_token_id: row.get("opposite_token_id"),
            opposite_outcome_label: row.get("opposite_outcome_label"),
        })
    }

    pub async fn load_active_trade_builder_avg_rebound_pairlock_rescue_session(
        &self,
        user_id: i64,
        flow_definition_id: Option<i64>,
        root_flow_node_key: &str,
        market_slug: &str,
    ) -> Result<Option<TradeBuilderAvgReboundPairlockRescueSession>> {
        let row = sqlx::query(
            "SELECT id, status, primary_token_id, primary_outcome_label, opposite_token_id, opposite_outcome_label \
             FROM trade_builder_avg_rebound_pairlock_rescue_sessions \
             WHERE user_id = $1 \
               AND flow_definition_id IS NOT DISTINCT FROM $2 \
               AND root_flow_node_key = $3 \
               AND market_slug = $4 \
               AND mode = $5 \
               AND status IN ('BUILDING_PRIMARY', 'PROFIT_LOCKING', 'GUARD_EXIT', 'RESCUE_EXIT', 'LOCKED') \
             ORDER BY created_at DESC, id DESC \
             LIMIT 1",
        )
        .bind(user_id)
        .bind(flow_definition_id)
        .bind(root_flow_node_key)
        .bind(market_slug)
        .bind(AVG_REBOUND_PAIRLOCK_RESCUE_MODE)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|row| TradeBuilderAvgReboundPairlockRescueSession {
            id: row.get("id"),
            status: row.get("status"),
            primary_token_id: row.get("primary_token_id"),
            primary_outcome_label: row.get("primary_outcome_label"),
            opposite_token_id: row.get("opposite_token_id"),
            opposite_outcome_label: row.get("opposite_outcome_label"),
        }))
    }

    pub async fn mark_trade_builder_avg_rebound_pairlock_rescue_session_status(
        &self,
        session_id: i64,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_avg_rebound_pairlock_rescue_sessions \
             SET status = $2, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(session_id)
        .bind(status)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn has_active_trade_builder_avg_rebound_pairlock_rescue_order(
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
                 AND COALESCE((ns.snapshot_json->'action_node'->'config'->>'avgReboundPairlockRescueOrder')::boolean, FALSE) \
                 AND ns.snapshot_json->'action_node'->'config'->>'avgReboundPairlockRescueRootNodeKey' = $3 \
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

    pub async fn has_trade_builder_avg_rebound_pairlock_rescue_decision_order(
        &self,
        user_id: i64,
        flow_definition_id: Option<i64>,
        root_flow_node_key: &str,
        market_slug: &str,
        decision_id: &str,
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
                 AND o.status NOT IN ('error', 'canceled', 'expired') \
                 AND COALESCE((ns.snapshot_json->'action_node'->'config'->>'avgReboundPairlockRescueOrder')::boolean, FALSE) \
                 AND ns.snapshot_json->'action_node'->'config'->>'avgReboundPairlockRescueRootNodeKey' = $3 \
                 AND ns.snapshot_json->'action_node'->'config'->>'avgReboundPairlockRescueDecisionId' = $5 \
             ) AS exists",
        )
        .bind(user_id)
        .bind(flow_definition_id)
        .bind(root_flow_node_key)
        .bind(market_slug)
        .bind(decision_id)
        .fetch_one(self.pool())
        .await?;
        Ok(row.get("exists"))
    }

    pub async fn load_trade_builder_avg_rebound_pairlock_rescue_state(
        &self,
        user_id: i64,
        flow_definition_id: Option<i64>,
        root_flow_node_key: &str,
        market_slug: &str,
    ) -> Result<TradeBuilderAvgReboundPairlockRescueState> {
        let Some(session) = self
            .load_active_trade_builder_avg_rebound_pairlock_rescue_session(
                user_id,
                flow_definition_id,
                root_flow_node_key,
                market_slug,
            )
            .await?
        else {
            return Ok(TradeBuilderAvgReboundPairlockRescueState::default());
        };

        let row = sqlx::query(
            "WITH fills AS ( \
               SELECT * \
               FROM trade_builder_avg_rebound_pairlock_rescue_fills \
               WHERE session_id = $1 \
             ), totals AS ( \
               SELECT \
                 COALESCE(SUM(CASE WHEN leg_role = 'primary' THEN quantity ELSE 0 END), 0)::double precision AS primary_total_qty, \
                 COALESCE(SUM(CASE WHEN leg_role = 'primary' THEN notional_usdc ELSE 0 END), 0)::double precision AS primary_total_cost, \
                 COALESCE(SUM(CASE WHEN leg_role = 'opposite' THEN quantity ELSE 0 END), 0)::double precision AS opposite_filled_qty, \
                 COALESCE(SUM(CASE WHEN leg_role = 'opposite' THEN notional_usdc ELSE 0 END), 0)::double precision AS opposite_total_cost, \
                 COALESCE(BOOL_OR(leg_role = 'opposite' AND intent = 'PROFIT_PAIRLOCK'), FALSE) AS profit_started \
               FROM fills \
             ), ids AS ( \
               SELECT \
                 COALESCE(ARRAY_AGG(DISTINCT tier_or_leg_id) FILTER (WHERE leg_role = 'primary'), ARRAY[]::text[]) AS primary_tier_ids, \
                 COALESCE(ARRAY_AGG(DISTINCT tier_or_leg_id) FILTER (WHERE leg_role = 'opposite'), ARRAY[]::text[]) AS opposite_leg_ids \
               FROM fills \
             ) \
             SELECT totals.*, ids.primary_tier_ids, ids.opposite_leg_ids \
             FROM totals, ids",
        )
        .bind(session.id)
        .fetch_one(self.pool())
        .await?;

        let primary_total_qty: f64 = row.get("primary_total_qty");
        let primary_total_cost: f64 = row.get("primary_total_cost");
        let opposite_filled_qty: f64 = row.get("opposite_filled_qty");
        let opposite_total_cost: f64 = row.get("opposite_total_cost");
        let avg_primary_cost =
            (primary_total_qty > 0.0).then_some(primary_total_cost / primary_total_qty);
        let paired_primary_cost = avg_primary_cost.unwrap_or(0.0) * opposite_filled_qty;
        Ok(TradeBuilderAvgReboundPairlockRescueState {
            session_id: Some(session.id),
            session_status: Some(session.status),
            primary_total_qty,
            primary_total_cost,
            avg_primary_cost,
            opposite_filled_qty,
            opposite_total_cost,
            open_primary_qty: (primary_total_qty - opposite_filled_qty).max(0.0),
            locked_pnl: opposite_filled_qty - paired_primary_cost - opposite_total_cost,
            profit_started: row.get("profit_started"),
            primary_tier_ids: row.get("primary_tier_ids"),
            opposite_leg_ids: row.get("opposite_leg_ids"),
        })
    }

    pub async fn record_trade_builder_avg_rebound_pairlock_rescue_fill(
        &self,
        input: &TradeBuilderAvgReboundPairlockRescueFillInput,
    ) -> Result<bool> {
        let result = sqlx::query(
            "INSERT INTO trade_builder_avg_rebound_pairlock_rescue_fills \
              (session_id, user_id, flow_definition_id, flow_run_id, root_flow_node_key, market_slug, token_id, outcome_label, leg_role, intent, stage_id, tier_or_leg_id, decision_id, order_side, builder_order_id, quantity, execution_price, notional_usdc, payload_json, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, NOW(), NOW()) \
             ON CONFLICT (builder_order_id) DO NOTHING",
        )
        .bind(input.session_id)
        .bind(input.user_id)
        .bind(input.flow_definition_id)
        .bind(input.flow_run_id)
        .bind(&input.root_flow_node_key)
        .bind(&input.market_slug)
        .bind(&input.token_id)
        .bind(&input.outcome_label)
        .bind(&input.leg_role)
        .bind(&input.intent)
        .bind(&input.stage_id)
        .bind(&input.tier_or_leg_id)
        .bind(&input.decision_id)
        .bind(&input.order_side)
        .bind(input.builder_order_id)
        .bind(input.quantity)
        .bind(input.execution_price)
        .bind(input.notional_usdc)
        .bind(&input.payload_json)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
