use super::super::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const REVENGE_FLIP_EXECUTION_LOCK_NAMESPACE: i32 = 42_086;
const REVENGE_FLIP_POSITION_EPSILON: f64 = 0.0001;
const REVENGE_FLIP_DUST_CLOSE_QTY: f64 = 0.2;

pub struct TradeBuilderRevengeFlipExecutionLock {
    conn: PoolConnection<Postgres>,
    lock_key1: i32,
    lock_key2: i32,
}

pub fn trade_builder_revenge_flip_execution_lock_keys(
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
    (REVENGE_FLIP_EXECUTION_LOCK_NAMESPACE, hash)
}

impl TradeBuilderRevengeFlipExecutionLock {
    pub async fn release(mut self) {
        let _ = sqlx::query("SELECT pg_advisory_unlock($1, $2)")
            .bind(self.lock_key1)
            .bind(self.lock_key2)
            .execute(&mut *self.conn)
            .await;
    }
}

fn revenge_flip_opposite_side(side: &str) -> Option<&'static str> {
    match side {
        "up" => Some("down"),
        "down" => Some("up"),
        _ => None,
    }
}

fn row_to_revenge_flip_state(row: sqlx::postgres::PgRow) -> TradeBuilderRevengeFlipState {
    TradeBuilderRevengeFlipState {
        user_id: row.get("user_id"),
        flow_definition_id: row.get("flow_definition_id"),
        root_flow_node_key: row.get("root_flow_node_key"),
        market_slug: row.get("market_slug"),
        current_side: row.get("current_side"),
        next_entry_side: row.get("next_entry_side"),
        position_qty: row.get("position_qty"),
        position_avg_cost: row.get("position_avg_cost"),
        position_entry_price: row.get("position_entry_price"),
        position_stop_loss_enabled: row.get("position_stop_loss_enabled"),
        position_stop_loss_pct: row.get("position_stop_loss_pct"),
        position_source_trade_id: row.get("position_source_trade_id"),
        position_builder_order_id: row.get("position_builder_order_id"),
        flip_count: row.get("flip_count"),
        total_loss_usdc: row.get("total_loss_usdc"),
        realized_pnl_usdc: row.get("realized_pnl_usdc"),
        total_buy_cost: row.get("total_buy_cost"),
        total_sell_revenue: row.get("total_sell_revenue"),
        ptb_bump_count: row.get("ptb_bump_count"),
        ptb_bump_total_usdc: row.get("ptb_bump_total_usdc"),
        last_intent: row.get("last_intent"),
        last_builder_order_id: row.get("last_builder_order_id"),
        last_action_json: row.get("last_action_json"),
    }
}

fn revenge_flip_apply_fill_to_state(
    state: &TradeBuilderRevengeFlipState,
    input: &TradeBuilderRevengeFlipFillInput,
) -> TradeBuilderRevengeFlipState {
    let qty = input.quantity.max(0.0);
    let notional = input
        .notional_usdc
        .max(qty * input.execution_price)
        .max(0.0);
    let mut next_state = state.clone();
    next_state.position_qty = next_state.position_qty.max(0.0);
    next_state.position_avg_cost = next_state.position_avg_cost.max(0.0);
    next_state.position_entry_price = next_state.position_entry_price.max(0.0);
    next_state.last_intent = Some(input.intent.clone());
    next_state.last_builder_order_id = Some(input.builder_order_id);
    next_state.last_action_json = input.payload_json.clone();

    match input.order_side.as_str() {
        "buy" => {
            let existing_cost = next_state.position_qty * next_state.position_avg_cost;
            next_state.position_qty += qty;
            if next_state.position_qty > REVENGE_FLIP_POSITION_EPSILON {
                next_state.position_avg_cost = (existing_cost + notional) / next_state.position_qty;
                next_state.position_entry_price = input.execution_price;
            }
            if let Some(stop_loss_pct) = input
                .stop_loss_pct
                .filter(|value| value.is_finite() && *value > 0.0 && *value < 1.0)
            {
                next_state.position_stop_loss_pct = stop_loss_pct;
            }
            if let Some(stop_loss_enabled) = input.stop_loss_enabled {
                next_state.position_stop_loss_enabled = stop_loss_enabled;
            }
            next_state.current_side = Some(input.revenge_side.clone());
            next_state.next_entry_side = None;
            next_state.position_source_trade_id = input.source_trade_id;
            next_state.position_builder_order_id = Some(input.builder_order_id);
            next_state.total_buy_cost += notional;
        }
        "sell" => {
            let sold_qty = if next_state.position_qty > REVENGE_FLIP_POSITION_EPSILON {
                qty.min(next_state.position_qty)
            } else {
                qty
            };
            let cost_basis = sold_qty * next_state.position_avg_cost;
            let mut close_realized = notional - cost_basis;
            next_state.realized_pnl_usdc += close_realized;
            next_state.total_sell_revenue += notional;
            if next_state.position_qty > REVENGE_FLIP_POSITION_EPSILON {
                next_state.position_qty = (next_state.position_qty - sold_qty).max(0.0);
            }
            let dust_close = input.intent == "stop_loss_sell"
                && next_state.position_qty > REVENGE_FLIP_POSITION_EPSILON
                && next_state.position_qty < REVENGE_FLIP_DUST_CLOSE_QTY;
            if dust_close {
                let dust_cost_basis = next_state.position_qty * next_state.position_avg_cost;
                close_realized -= dust_cost_basis;
                next_state.realized_pnl_usdc -= dust_cost_basis;
            }
            if next_state.position_qty <= REVENGE_FLIP_POSITION_EPSILON || dust_close {
                let closed_side = next_state
                    .current_side
                    .as_deref()
                    .unwrap_or(input.revenge_side.as_str())
                    .to_string();
                next_state.current_side = None;
                next_state.next_entry_side =
                    revenge_flip_opposite_side(&closed_side).map(str::to_string);
                next_state.position_qty = 0.0;
                next_state.position_avg_cost = 0.0;
                next_state.position_entry_price = 0.0;
                next_state.position_stop_loss_enabled = true;
                next_state.position_stop_loss_pct = 0.20;
                next_state.position_source_trade_id = None;
                next_state.position_builder_order_id = None;
                next_state.flip_count += 1;
                let loss = (-close_realized).max(0.0);
                if loss > 0.0 {
                    next_state.total_loss_usdc += loss;
                    next_state.ptb_bump_count += 1;
                    next_state.ptb_bump_total_usdc += loss;
                }
            }
        }
        _ => {}
    }

    next_state
}

impl PostgresRepository {
    pub async fn try_acquire_revenge_flip_execution_lock(
        &self,
        user_id: i64,
        flow_definition_id: i64,
        root_flow_node_key: &str,
        market_slug: &str,
    ) -> Result<Option<TradeBuilderRevengeFlipExecutionLock>> {
        let (lock_key1, lock_key2) = trade_builder_revenge_flip_execution_lock_keys(
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
            Ok(Some(TradeBuilderRevengeFlipExecutionLock {
                conn,
                lock_key1,
                lock_key2,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn has_active_trade_builder_revenge_flip_order(
        &self,
        user_id: i64,
        flow_definition_id: i64,
        root_flow_node_key: &str,
        market_slug: &str,
    ) -> Result<bool> {
        let row = sqlx::query(
            "SELECT EXISTS ( \
               SELECT 1 \
               FROM trade_builder_orders o \
               JOIN trade_builder_order_node_snapshots ns ON ns.order_id = o.id \
               WHERE o.user_id = $1 \
                 AND o.origin_flow_definition_id = $2 \
                 AND o.origin_flow_node_key = $3 \
                 AND o.market_slug = $4 \
                 AND o.status IN ('pending', 'armed', 'triggered', 'open', 'partially_filled') \
                 AND COALESCE((ns.snapshot_json->'action_node'->'config'->>'revengeFlipOrder')::boolean, FALSE) \
                 AND ns.snapshot_json->'action_node'->'config'->>'revengeFlipRootNodeKey' = $3 \
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

    pub async fn load_trade_builder_revenge_flip_state(
        &self,
        user_id: i64,
        flow_definition_id: i64,
        root_flow_node_key: &str,
        market_slug: &str,
    ) -> Result<TradeBuilderRevengeFlipState> {
        let row = sqlx::query(
            "SELECT \
               user_id, flow_definition_id, root_flow_node_key, market_slug, \
               current_side, next_entry_side, position_qty::double precision AS position_qty, \
               position_avg_cost::double precision AS position_avg_cost, \
               position_entry_price::double precision AS position_entry_price, \
               position_stop_loss_enabled, \
               position_stop_loss_pct::double precision AS position_stop_loss_pct, \
               position_source_trade_id, position_builder_order_id, flip_count, \
               total_loss_usdc::double precision AS total_loss_usdc, \
               realized_pnl_usdc::double precision AS realized_pnl_usdc, \
               total_buy_cost::double precision AS total_buy_cost, \
               total_sell_revenue::double precision AS total_sell_revenue, \
               ptb_bump_count, ptb_bump_total_usdc::double precision AS ptb_bump_total_usdc, \
               last_intent, last_builder_order_id, last_action_json \
             FROM trade_builder_revenge_flip_state \
             WHERE user_id = $1 \
               AND flow_definition_id = $2 \
               AND root_flow_node_key = $3 \
               AND market_slug = $4",
        )
        .bind(user_id)
        .bind(flow_definition_id)
        .bind(root_flow_node_key)
        .bind(market_slug)
        .fetch_optional(self.pool())
        .await?;
        Ok(row
            .map(row_to_revenge_flip_state)
            .unwrap_or_else(|| TradeBuilderRevengeFlipState {
                user_id,
                flow_definition_id,
                root_flow_node_key: root_flow_node_key.to_string(),
                market_slug: market_slug.to_string(),
                ..TradeBuilderRevengeFlipState::default()
            }))
    }

    pub async fn record_trade_builder_revenge_flip_fill(
        &self,
        input: &TradeBuilderRevengeFlipFillInput,
    ) -> Result<bool> {
        let mut tx = self.pool().begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO trade_builder_revenge_flip_fills \
              (user_id, flow_definition_id, flow_run_id, root_flow_node_key, market_slug, token_id, outcome_label, revenge_side, intent, order_side, builder_order_id, parent_builder_order_id, source_trade_id, quantity, execution_price, notional_usdc, stop_loss_enabled, stop_loss_pct, payload_json, created_at) \
             VALUES \
              ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, NOW()) \
             ON CONFLICT (builder_order_id) DO NOTHING \
             RETURNING id",
        )
        .bind(input.user_id)
        .bind(input.flow_definition_id)
        .bind(input.flow_run_id)
        .bind(&input.root_flow_node_key)
        .bind(&input.market_slug)
        .bind(&input.token_id)
        .bind(&input.outcome_label)
        .bind(&input.revenge_side)
        .bind(&input.intent)
        .bind(&input.order_side)
        .bind(input.builder_order_id)
        .bind(input.parent_builder_order_id)
        .bind(input.source_trade_id)
        .bind(input.quantity)
        .bind(input.execution_price)
        .bind(input.notional_usdc)
        .bind(input.stop_loss_enabled)
        .bind(input.stop_loss_pct)
        .bind(&input.payload_json)
        .fetch_optional(&mut *tx)
        .await?;
        if inserted.is_none() {
            tx.commit().await?;
            return Ok(false);
        }

        sqlx::query(
            "INSERT INTO trade_builder_revenge_flip_state \
              (user_id, flow_definition_id, root_flow_node_key, market_slug, current_side, next_entry_side, last_action_json, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, NULL, NULL, '{}'::jsonb, NOW(), NOW()) \
             ON CONFLICT (user_id, flow_definition_id, root_flow_node_key, market_slug) DO NOTHING",
        )
        .bind(input.user_id)
        .bind(input.flow_definition_id)
        .bind(&input.root_flow_node_key)
        .bind(&input.market_slug)
        .execute(&mut *tx)
        .await?;

        let row = sqlx::query(
            "SELECT \
               user_id, flow_definition_id, root_flow_node_key, market_slug, \
               current_side, next_entry_side, position_qty::double precision AS position_qty, \
               position_avg_cost::double precision AS position_avg_cost, \
               position_entry_price::double precision AS position_entry_price, \
               position_stop_loss_enabled, \
               position_stop_loss_pct::double precision AS position_stop_loss_pct, \
               position_source_trade_id, position_builder_order_id, flip_count, \
               total_loss_usdc::double precision AS total_loss_usdc, \
               realized_pnl_usdc::double precision AS realized_pnl_usdc, \
               total_buy_cost::double precision AS total_buy_cost, \
               total_sell_revenue::double precision AS total_sell_revenue, \
               ptb_bump_count, ptb_bump_total_usdc::double precision AS ptb_bump_total_usdc, \
               last_intent, last_builder_order_id, last_action_json \
             FROM trade_builder_revenge_flip_state \
             WHERE user_id = $1 \
               AND flow_definition_id = $2 \
               AND root_flow_node_key = $3 \
               AND market_slug = $4 \
             FOR UPDATE",
        )
        .bind(input.user_id)
        .bind(input.flow_definition_id)
        .bind(&input.root_flow_node_key)
        .bind(&input.market_slug)
        .fetch_one(&mut *tx)
        .await?;
        let state = row_to_revenge_flip_state(row);

        let next_state = revenge_flip_apply_fill_to_state(&state, input);

        sqlx::query(
            "UPDATE trade_builder_revenge_flip_state SET \
               current_side = $5, \
               next_entry_side = $6, \
               position_qty = $7, \
               position_avg_cost = $8, \
               position_entry_price = $9, \
               position_stop_loss_enabled = $10, \
               position_stop_loss_pct = $11, \
               position_source_trade_id = $12, \
               position_builder_order_id = $13, \
               flip_count = $14, \
               total_loss_usdc = $15, \
               realized_pnl_usdc = $16, \
               total_buy_cost = $17, \
               total_sell_revenue = $18, \
               ptb_bump_count = $19, \
               ptb_bump_total_usdc = $20, \
               last_intent = $21, \
               last_builder_order_id = $22, \
               last_action_json = $23, \
               updated_at = NOW() \
             WHERE user_id = $1 \
               AND flow_definition_id = $2 \
               AND root_flow_node_key = $3 \
               AND market_slug = $4",
        )
        .bind(input.user_id)
        .bind(input.flow_definition_id)
        .bind(&input.root_flow_node_key)
        .bind(&input.market_slug)
        .bind(next_state.current_side)
        .bind(next_state.next_entry_side)
        .bind(next_state.position_qty)
        .bind(next_state.position_avg_cost)
        .bind(next_state.position_entry_price)
        .bind(next_state.position_stop_loss_enabled)
        .bind(next_state.position_stop_loss_pct)
        .bind(next_state.position_source_trade_id)
        .bind(next_state.position_builder_order_id)
        .bind(next_state.flip_count)
        .bind(next_state.total_loss_usdc)
        .bind(next_state.realized_pnl_usdc)
        .bind(next_state.total_buy_cost)
        .bind(next_state.total_sell_revenue)
        .bind(next_state.ptb_bump_count)
        .bind(next_state.ptb_bump_total_usdc)
        .bind(next_state.last_intent)
        .bind(next_state.last_builder_order_id)
        .bind(&next_state.last_action_json)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn revenge_fill_input(quantity: f64, notional_usdc: f64) -> TradeBuilderRevengeFlipFillInput {
        TradeBuilderRevengeFlipFillInput {
            user_id: 1,
            flow_definition_id: 4328,
            flow_run_id: Some(868),
            root_flow_node_key: "action_revenge_flip".to_string(),
            market_slug: "btc-updown-5m-1780266600".to_string(),
            token_id: "token".to_string(),
            outcome_label: "Down".to_string(),
            revenge_side: "down".to_string(),
            intent: "stop_loss_sell".to_string(),
            order_side: "sell".to_string(),
            builder_order_id: 18362,
            parent_builder_order_id: Some(18361),
            source_trade_id: Some(108248),
            quantity,
            execution_price: if quantity > 0.0 {
                notional_usdc / quantity
            } else {
                0.0
            },
            notional_usdc,
            stop_loss_enabled: None,
            stop_loss_pct: None,
            payload_json: json!({ "test": true }),
        }
    }

    fn revenge_state(position_qty: f64) -> TradeBuilderRevengeFlipState {
        TradeBuilderRevengeFlipState {
            user_id: 1,
            flow_definition_id: 4328,
            root_flow_node_key: "action_revenge_flip".to_string(),
            market_slug: "btc-updown-5m-1780266600".to_string(),
            current_side: Some("down".to_string()),
            position_qty,
            position_avg_cost: 0.67,
            position_entry_price: 0.67,
            position_stop_loss_enabled: false,
            position_stop_loss_pct: 0.4,
            position_source_trade_id: Some(108248),
            position_builder_order_id: Some(18361),
            total_buy_cost: position_qty * 0.67,
            ..TradeBuilderRevengeFlipState::default()
        }
    }

    #[test]
    fn stop_loss_sell_dust_closes_revenge_position() {
        let next = revenge_flip_apply_fill_to_state(
            &revenge_state(7.46),
            &revenge_fill_input(7.44, 2.8272),
        );

        assert_eq!(next.current_side, None);
        assert_eq!(next.next_entry_side, Some("up".to_string()));
        assert_eq!(next.position_qty, 0.0);
        assert_eq!(next.position_avg_cost, 0.0);
        assert_eq!(next.position_source_trade_id, None);
        assert_eq!(next.position_builder_order_id, None);
        assert_eq!(next.flip_count, 1);
        assert!((next.realized_pnl_usdc + 2.171).abs() < 0.000001);
        assert!((next.total_loss_usdc - 2.171).abs() < 0.000001);
    }

    #[test]
    fn stop_loss_sell_keeps_exact_dust_boundary_open() {
        let next =
            revenge_flip_apply_fill_to_state(&revenge_state(0.2), &revenge_fill_input(0.0, 0.0));

        assert_eq!(next.current_side, Some("down".to_string()));
        assert_eq!(next.next_entry_side, None);
        assert_eq!(next.position_qty, 0.2);
        assert_eq!(next.flip_count, 0);
    }
}
