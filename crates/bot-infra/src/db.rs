use anyhow::Result;
use bot_core::{can_transition, LegSide, MarketCycleId, TradeState};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{pool::PoolConnection, postgres::PgPoolOptions, PgPool, Postgres, Row};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone)]
pub struct PostgresRepository {
    pool: PgPool,
}

pub struct RunnerSingletonDbLock {
    _conn: PoolConnection<Postgres>,
    lock_key: i64,
}

impl RunnerSingletonDbLock {
    pub fn lock_key(&self) -> i64 {
        self.lock_key
    }
}

#[derive(Debug, Clone)]
pub struct LegPositionSnapshot {
    pub leg_side: LegSide,
    pub token_id: String,
    pub qty: f64,
    pub avg_entry: f64,
    pub levels_filled: i32,
    pub last_fill_price: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct PositionExitRule {
    pub leg_side: LegSide,
    pub drop_sell_pct: f64,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct PressureSnapshot {
    pub trade_id: i64,
    pub pressure_score: f64,
    pub bid_ask_imbalance: Option<f64>,
    pub sell_ratio: Option<f64>,
    pub yes_price: Option<f64>,
    pub no_price: Option<f64>,
    pub trigger_reason: Option<String>,
    pub triggered: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeBuilderOrder {
    pub id: i64,
    pub trade_id: i64,
    pub user_id: i64,
    pub kind: String,
    pub status: String,
    pub market_slug: String,
    pub token_id: String,
    pub outcome_label: String,
    pub side: String,
    pub execution_mode: String,
    pub trigger_condition: Option<String>,
    pub trigger_price: Option<f64>,
    pub max_price: Option<f64>,
    pub size_basis: String,
    pub size_usdc: f64,
    pub target_qty: Option<f64>,
    pub min_price_distance_cent: f64,
    pub expires_at: Option<DateTime<Utc>>,
    pub max_triggers: i32,
    pub triggers_fired: i32,
    pub active_exchange_order_id: Option<String>,
    pub remaining_size: Option<f64>,
    pub remaining_qty: Option<f64>,
    pub working_price: Option<f64>,
    pub last_seen_price: Option<f64>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub parent_order_id: Option<i64>,
    pub tp_enabled: bool,
    pub tp_price: Option<f64>,
    pub sl_enabled: bool,
    pub sl_price: Option<f64>,
    pub filled_qty: f64,
    pub fee_rate_bps: i64,
    pub trigger_latched: bool,
    pub trigger_latched_reason: Option<String>,
    pub submitted_dynamic_qty: Option<f64>,
    pub submitted_dynamic_price: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct TradeBuilderInventoryObservationInput {
    pub parent_builder_order_id: i64,
    pub observer_builder_order_id: Option<i64>,
    pub user_id: i64,
    pub market_slug: String,
    pub token_id: String,
    pub outcome_label: String,
    pub exchange_order_id: Option<String>,
    pub observation_kind: String,
    pub qty_source: Option<String>,
    pub baseline_visible_qty: Option<f64>,
    pub submitted_dynamic_qty: Option<f64>,
    pub resolved_fill_qty: Option<f64>,
    pub expected_fee_qty: Option<f64>,
    pub expected_net_qty: Option<f64>,
    pub expected_visible_qty: Option<f64>,
    pub actual_visible_qty: Option<f64>,
    pub visible_delta_qty: Option<f64>,
    pub gap_vs_submit_qty: Option<f64>,
    pub gap_vs_fill_qty: Option<f64>,
    pub gap_vs_expected_qty: Option<f64>,
    pub reference_price: Option<f64>,
    pub fee_rate_bps: Option<i64>,
    pub fill_to_inventory_ms: Option<i64>,
    pub payload_json: Value,
}

#[derive(Debug, Clone)]
pub struct PendingTradeBuilderFirstVisibleInventoryObservation {
    pub parent_builder_order_id: i64,
    pub observer_builder_order_id: Option<i64>,
    pub user_id: i64,
    pub market_slug: String,
    pub token_id: String,
    pub outcome_label: String,
    pub exchange_order_id: Option<String>,
    pub baseline_visible_qty: Option<f64>,
    pub submitted_dynamic_qty: Option<f64>,
    pub resolved_fill_qty: Option<f64>,
    pub submit_reference_price: Option<f64>,
    pub fill_reference_price: Option<f64>,
    pub fill_qty_source: Option<String>,
    pub fee_rate_bps: i64,
    pub fill_observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeBuilderWorkflow {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub status: String,
    pub source_trade_id: i64,
    pub sell_target_pct: f64,
    pub buy_start_after_sell_progress_pct: f64,
    pub buy_trigger_mode: String,
    pub buy_allocation_pct: f64,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeBuilderWorkflowLeg {
    pub id: i64,
    pub workflow_id: i64,
    pub leg_type: String,
    pub market_slug: String,
    pub token_id: String,
    pub outcome_label: String,
    pub side: String,
    pub trigger_condition: Option<String>,
    pub trigger_price: Option<f64>,
    pub min_price_distance_cent: f64,
    pub status: String,
    pub builder_order_id: Option<i64>,
    pub target_notional_usdc: f64,
    pub allocated_notional_usdc: f64,
    pub filled_notional_usdc: f64,
    pub filled_qty: f64,
    pub last_seen_price: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowDefinitionRuntime {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub status: String,
    pub draft_version_id: Option<i64>,
    pub published_version_id: Option<i64>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowVersionRuntime {
    pub id: i64,
    pub definition_id: i64,
    pub version_no: i32,
    pub status: String,
    pub graph_json: Value,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowRun {
    pub id: i64,
    pub definition_id: i64,
    pub version_id: i64,
    pub user_id: i64,
    pub status: String,
    pub trigger_source: Option<String>,
    pub context_json: Value,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowRunStep {
    pub id: i64,
    pub run_id: i64,
    pub node_key: String,
    pub node_type: String,
    pub status: String,
    pub attempt: i32,
    pub input_json: Option<Value>,
    pub output_json: Option<Value>,
    pub error_text: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub available_at: DateTime<Utc>,
    pub parent_step_id: Option<i64>,
    pub idempotency_key: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowDualDcaJob {
    pub id: i64,
    pub flow_run_id: i64,
    pub flow_definition_id: i64,
    pub flow_version_id: Option<i64>,
    pub node_key: String,
    pub status: String,
    pub source_trade_id: Option<i64>,
    pub market_asset: String,
    pub market_timeframe: String,
    pub side_mode: String,
    pub base_sizing: String,
    pub base_shares: Option<f64>,
    pub base_usdc: Option<f64>,
    pub base_price_usdc: Option<f64>,
    pub dca_levels: i32,
    pub near_step: f64,
    pub step_mult: f64,
    pub size_mult: f64,
    pub min_price_distance_cent: f64,
    pub cutoff_min: i32,
    pub tp_profit_pct: f64,
    pub sl_loss_pct: f64,
    pub sl_spread_pct: f64,
    pub last_market_slug: Option<String>,
    pub last_market_started_at: Option<DateTime<Utc>>,
    pub last_market_ends_at: Option<DateTime<Utc>>,
    pub next_check_at: DateTime<Utc>,
    pub created_order_count: i32,
    pub consecutive_errors: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFlowDualDcaLeg {
    pub id: i64,
    pub job_id: i64,
    pub market_slug: String,
    pub token_id: String,
    pub outcome_label: String,
    pub side: String,
    pub level_index: i32,
    pub trigger_condition: Option<String>,
    pub trigger_price: Option<f64>,
    pub size_usdc: f64,
    pub reference_price: Option<f64>,
    pub builder_order_id: Option<i64>,
    pub status: String,
    pub active_exchange_order_id: Option<String>,
    pub client_order_id: Option<String>,
    pub filled_price: Option<f64>,
    pub filled_size: Option<f64>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub filled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct TradeFillTokenAggregate {
    pub token_id: String,
    pub buy_qty: f64,
    pub buy_notional_usdc: f64,
    pub sell_qty: f64,
    pub sell_notional_usdc: f64,
}

#[derive(Debug, Clone)]
pub struct AutoClaimJob {
    pub id: i64,
    pub owner_address: String,
    pub market_slug: Option<String>,
    pub condition_id: String,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub next_attempt_at: DateTime<Utc>,
    pub tx_hash: Option<String>,
    pub last_error: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub last_seen_redeemable_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PostgresRepository {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn try_acquire_runner_singleton_lock(
        &self,
        lock_key: i64,
    ) -> Result<Option<RunnerSingletonDbLock>> {
        let mut conn = self.pool.acquire().await?;
        let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
            .bind(lock_key)
            .fetch_one(&mut *conn)
            .await?;
        if acquired {
            Ok(Some(RunnerSingletonDbLock {
                _conn: conn,
                lock_key,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn record_run_start(&self, mode: &str, version: &str) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO bot_runs (mode, version, started_at) VALUES ($1, $2, NOW()) RETURNING id",
        )
        .bind(mode)
        .bind(version)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn record_run_stop(&self, run_id: i64, reason: &str) -> Result<()> {
        sqlx::query("UPDATE bot_runs SET stopped_at = NOW(), reason = $2 WHERE id = $1")
            .bind(run_id)
            .bind(reason)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn store_config_snapshot(
        &self,
        run_id: i64,
        config_hash: &str,
        payload: &Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO config_snapshots (run_id, config_hash, payload_json, created_at) VALUES ($1, $2, $3, NOW())",
        )
        .bind(run_id)
        .bind(config_hash)
        .bind(payload)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn upsert_market_cycle(&self, cycle: &MarketCycleId) -> Result<i64> {
        let starts_at = cycle.start_time().unwrap_or_else(Utc::now);
        let ends_at = starts_at + chrono::Duration::seconds(300);
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO markets (market_slug, starts_at, ends_at, status) VALUES ($1, $2, $3, 'open') \
             ON CONFLICT (market_slug) DO UPDATE SET status = EXCLUDED.status RETURNING id",
        )
        .bind(cycle.to_string())
        .bind(starts_at)
        .bind(ends_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn upsert_market_by_slug(
        &self,
        market_slug: &str,
        starts_at: DateTime<Utc>,
        ends_at: DateTime<Utc>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO markets (market_slug, starts_at, ends_at, status) VALUES ($1, $2, $3, 'open') \
             ON CONFLICT (market_slug) DO UPDATE SET \
               starts_at = COALESCE(markets.starts_at, EXCLUDED.starts_at), \
               ends_at = COALESCE(markets.ends_at, EXCLUDED.ends_at), \
               status = CASE WHEN markets.status = 'settled' THEN markets.status ELSE EXCLUDED.status END \
             RETURNING id",
        )
        .bind(market_slug)
        .bind(starts_at)
        .bind(ends_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn create_trade_stub_manual(
        &self,
        market_id: i64,
        notional: f64,
        reference_price: f64,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO trades (market_id, user_id, state, entry_price, notional_usdc, strategy_mode, opened_at) \
             VALUES ($1, (SELECT id FROM app_users ORDER BY id ASC LIMIT 1), $2, $3, $4, $5, NOW()) RETURNING id",
        )
        .bind(market_id)
        .bind(format!("{:?}", TradeState::Idle))
        .bind(reference_price)
        .bind(notional)
        .bind("manual_trade_builder")
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn find_latest_active_trade_by_market_token(
        &self,
        user_id: i64,
        market_slug: &str,
        token_id: &str,
    ) -> Result<Option<i64>> {
        let trade_id = sqlx::query_scalar::<_, i64>(
            "SELECT t.id
             FROM trades t
             JOIN markets m ON m.id = t.market_id
             LEFT JOIN leg_positions lp ON lp.trade_id = t.id
             WHERE LOWER(m.market_slug) = LOWER($1)
               AND LOWER(COALESCE(lp.token_id, '')) = LOWER($2)
               AND t.user_id = $3
               AND t.state NOT IN ('Settled', 'Halted')
             ORDER BY t.opened_at DESC NULLS LAST, t.id DESC
             LIMIT 1",
        )
        .bind(market_slug)
        .bind(token_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(trade_id)
    }

    pub async fn ensure_manual_builder_source_trade(
        &self,
        user_id: i64,
        market_slug: &str,
        token_id: &str,
        outcome_label: &str,
        reference_price: f64,
        notional_usdc: f64,
    ) -> Result<i64> {
        if let Some(existing_trade_id) = self
            .find_latest_active_trade_by_market_token(user_id, market_slug, token_id)
            .await?
        {
            return Ok(existing_trade_id);
        }

        let price = reference_price.clamp(0.01, 0.99);
        let notional = if notional_usdc.is_finite() && notional_usdc > 0.0 {
            notional_usdc.max(1.0)
        } else {
            1.0
        };
        let qty = (notional / price).max(0.0001);
        let leg_side = match outcome_label.trim().to_ascii_lowercase().as_str() {
            "no" | "false" | "0" => LegSide::No,
            _ => LegSide::Yes,
        };
        let starts_at = Utc::now() - chrono::Duration::hours(1);
        let ends_at = Utc::now() + chrono::Duration::days(30);

        let mut tx = self.pool.begin().await?;
        let market_id: i64 = sqlx::query_scalar(
            "INSERT INTO markets (market_slug, starts_at, ends_at, status)
             VALUES ($1, $2, $3, 'open')
             ON CONFLICT (market_slug) DO UPDATE SET
               starts_at = LEAST(markets.starts_at, EXCLUDED.starts_at),
               ends_at = GREATEST(markets.ends_at, EXCLUDED.ends_at),
               status = CASE WHEN markets.status = 'settled' THEN markets.status ELSE 'open' END
             RETURNING id",
        )
        .bind(market_slug)
        .bind(starts_at)
        .bind(ends_at)
        .fetch_one(&mut *tx)
        .await?;
        let trade_id: i64 = sqlx::query_scalar(
            "INSERT INTO trades (market_id, user_id, state, entry_price, notional_usdc, strategy_mode, opened_at)
             VALUES ($1, $2, $3, $4, $5, $6, NOW())
             RETURNING id",
        )
        .bind(market_id)
        .bind(user_id)
        .bind(format!("{:?}", TradeState::Idle))
        .bind(price)
        .bind(notional)
        .bind("manual_trade_builder")
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO leg_positions
               (trade_id, leg_side, token_id, qty, avg_entry, levels_filled, last_fill_price, updated_at)
             VALUES
               ($1, $2, $3, $4, $5, 1, $5, NOW())
             ON CONFLICT (trade_id, leg_side) DO UPDATE SET
               token_id = EXCLUDED.token_id,
               qty = EXCLUDED.qty,
               avg_entry = EXCLUDED.avg_entry,
               levels_filled = GREATEST(leg_positions.levels_filled, EXCLUDED.levels_filled),
               last_fill_price = EXCLUDED.last_fill_price,
               updated_at = NOW()",
        )
        .bind(trade_id)
        .bind(leg_side_to_db(leg_side))
        .bind(token_id)
        .bind(qty)
        .bind(price)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(trade_id)
    }

    pub async fn create_trade_stub(
        &self,
        market_id: i64,
        entry_price: f64,
        notional: f64,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO trades (market_id, user_id, state, entry_price, notional_usdc, opened_at) \
             VALUES ($1, (SELECT id FROM app_users ORDER BY id ASC LIMIT 1), $2, $3, $4, NOW()) RETURNING id",
        )
        .bind(market_id)
        .bind(format!("{:?}", TradeState::Idle))
        .bind(entry_price)
        .bind(notional)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn create_trade_stub_dual(
        &self,
        market_id: i64,
        notional: f64,
        strategy_mode: &str,
        basket_tp: f64,
        basket_sl: f64,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO trades (market_id, user_id, state, entry_price, notional_usdc, strategy_mode, basket_tp, basket_sl, opened_at) \
             VALUES ($1, (SELECT id FROM app_users ORDER BY id ASC LIMIT 1), $2, $3, $4, $5, $6, $7, NOW()) RETURNING id",
        )
        .bind(market_id)
        .bind(format!("{:?}", TradeState::Idle))
        .bind(0.50f64)
        .bind(notional)
        .bind(strategy_mode)
        .bind(basket_tp)
        .bind(basket_sl)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn trade_state(&self, trade_id: i64) -> Result<TradeState> {
        let raw: String = sqlx::query_scalar("SELECT state FROM trades WHERE id = $1")
            .bind(trade_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(parse_state(&raw).unwrap_or(TradeState::Idle))
    }

    pub async fn transition_trade_state(
        &self,
        trade_id: i64,
        from: TradeState,
        to: TradeState,
        reason: &str,
    ) -> Result<()> {
        can_transition(from, to)?;
        sqlx::query("UPDATE trades SET state = $2 WHERE id = $1")
            .bind(trade_id)
            .bind(format!("{:?}", to))
            .execute(&self.pool)
            .await?;
        self.record_risk_event(Some(trade_id), "state_transition", "allow", reason)
            .await?;
        Ok(())
    }

    pub async fn append_order_event(
        &self,
        trade_id: i64,
        intent: &str,
        side: &str,
        price: f64,
        size: f64,
        status: &str,
    ) -> Result<i64> {
        self.append_order_event_with_client_id(trade_id, intent, side, price, size, status, None)
            .await
    }

    pub async fn append_order_event_with_client_id(
        &self,
        trade_id: i64,
        intent: &str,
        side: &str,
        price: f64,
        size: f64,
        status: &str,
        client_order_id: Option<&str>,
    ) -> Result<i64> {
        self.append_order_event_with_meta(
            trade_id,
            intent,
            side,
            price,
            size,
            status,
            client_order_id,
            None,
            None,
        )
        .await
    }

    pub async fn append_order_event_with_meta(
        &self,
        trade_id: i64,
        intent: &str,
        side: &str,
        price: f64,
        size: f64,
        status: &str,
        client_order_id: Option<&str>,
        leg_side: Option<&str>,
        token_id: Option<&str>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO orders (trade_id, exchange_order_id, intent, side, price, size, status, client_order_id, leg_side, token_id, last_exchange_status, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NOW(), NOW()) RETURNING id",
        )
        .bind(trade_id)
        .bind(Uuid::new_v4().to_string())
        .bind(intent)
        .bind(side)
        .bind(price)
        .bind(size)
        .bind(status)
        .bind(client_order_id)
        .bind(leg_side)
        .bind(token_id)
        .bind(status)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn append_fill_event(
        &self,
        order_id: i64,
        price: f64,
        size: f64,
        fee: f64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO fills (order_id, fill_id, price, size, fee, filled_at) VALUES ($1, gen_random_uuid()::text, $2, $3, $4, NOW())",
        )
        .bind(order_id)
        .bind(price)
        .bind(size)
        .bind(fee)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn upsert_order_by_exchange_id(
        &self,
        trade_id: i64,
        exchange_order_id: &str,
        client_order_id: Option<&str>,
        intent: &str,
        side: &str,
        price: f64,
        size: f64,
        status: &str,
        exchange_ts: Option<i64>,
        reject_reason: Option<&str>,
        raw_payload: &Value,
    ) -> Result<i64> {
        self.upsert_order_by_exchange_id_with_meta(
            trade_id,
            exchange_order_id,
            client_order_id,
            intent,
            side,
            price,
            size,
            status,
            exchange_ts,
            reject_reason,
            raw_payload,
            None,
            None,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_order_by_exchange_id_with_meta(
        &self,
        trade_id: i64,
        exchange_order_id: &str,
        client_order_id: Option<&str>,
        intent: &str,
        side: &str,
        price: f64,
        size: f64,
        status: &str,
        exchange_ts: Option<i64>,
        reject_reason: Option<&str>,
        raw_payload: &Value,
        leg_side: Option<&str>,
        token_id: Option<&str>,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO orders (trade_id, exchange_order_id, client_order_id, intent, side, price, size, status, leg_side, token_id, last_exchange_status, exchange_ts, reject_reason, raw_payload_json, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $8, $11, $12, $13, NOW(), NOW()) \
             ON CONFLICT (exchange_order_id) DO UPDATE SET \
               client_order_id = EXCLUDED.client_order_id, \
               status = EXCLUDED.status, \
               leg_side = COALESCE(EXCLUDED.leg_side, orders.leg_side), \
               token_id = COALESCE(EXCLUDED.token_id, orders.token_id), \
               last_exchange_status = EXCLUDED.last_exchange_status, \
               exchange_ts = EXCLUDED.exchange_ts, \
               reject_reason = EXCLUDED.reject_reason, \
               raw_payload_json = EXCLUDED.raw_payload_json, \
               updated_at = NOW() \
             RETURNING id",
        )
        .bind(trade_id)
        .bind(exchange_order_id)
        .bind(client_order_id)
        .bind(intent)
        .bind(side)
        .bind(price)
        .bind(size)
        .bind(status)
        .bind(leg_side)
        .bind(token_id)
        .bind(exchange_ts)
        .bind(reject_reason)
        .bind(raw_payload)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn upsert_fill_by_exchange_fill_id(
        &self,
        order_id: i64,
        exchange_fill_id: &str,
        price: f64,
        size: f64,
        fee: f64,
        exchange_ts: Option<i64>,
        raw_payload: &Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO fills (order_id, fill_id, price, size, fee, exchange_ts, raw_payload_json, filled_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, NOW()) \
             ON CONFLICT (fill_id) DO UPDATE SET \
               price = EXCLUDED.price, \
               size = EXCLUDED.size, \
               fee = EXCLUDED.fee, \
               exchange_ts = EXCLUDED.exchange_ts, \
               raw_payload_json = EXCLUDED.raw_payload_json",
        )
        .bind(order_id)
        .bind(exchange_fill_id)
        .bind(price)
        .bind(size)
        .bind(fee)
        .bind(exchange_ts)
        .bind(raw_payload)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn record_risk_event(
        &self,
        trade_id: Option<i64>,
        event_type: &str,
        decision: &str,
        details: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO risk_events (trade_id, event_type, decision, details, created_at) VALUES ($1, $2, $3, $4, NOW())",
        )
        .bind(trade_id)
        .bind(event_type)
        .bind(decision)
        .bind(details)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn open_order_count(&self) -> Result<u32> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM orders WHERE status IN ('open', 'partially_filled')",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(count as u32)
    }

    pub async fn open_order_count_for_user(&self, user_id: i64) -> Result<u32> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM orders o
             JOIN trades t ON t.id = o.trade_id
             WHERE o.status IN ('open', 'partially_filled')
               AND t.user_id = $1",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count as u32)
    }

    pub async fn daily_realized_pnl(&self) -> Result<f64> {
        let pnl: Option<f64> = sqlx::query_scalar(
            "SELECT SUM(COALESCE(realized_pnl, 0.0)) FROM trades WHERE closed_at::date = CURRENT_DATE",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(pnl.unwrap_or(0.0))
    }

    pub async fn daily_realized_pnl_for_user(&self, user_id: i64) -> Result<f64> {
        let pnl: Option<f64> = sqlx::query_scalar(
            "SELECT SUM(COALESCE(realized_pnl, 0.0))
             FROM trades
             WHERE closed_at::date = CURRENT_DATE
               AND user_id = $1",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(pnl.unwrap_or(0.0))
    }

    pub async fn consecutive_losses(&self, limit: i64) -> Result<u32> {
        let rows: Vec<Option<f64>> = sqlx::query_scalar(
            "SELECT realized_pnl FROM trades WHERE closed_at IS NOT NULL ORDER BY closed_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut losses = 0u32;
        for pnl in rows {
            if pnl.unwrap_or(0.0) < 0.0 {
                losses += 1;
            } else {
                break;
            }
        }
        Ok(losses)
    }

    pub async fn consecutive_losses_for_user(&self, user_id: i64, limit: i64) -> Result<u32> {
        let rows: Vec<Option<f64>> = sqlx::query_scalar(
            "SELECT realized_pnl
             FROM trades
             WHERE closed_at IS NOT NULL
               AND user_id = $1
             ORDER BY closed_at DESC
             LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut losses = 0u32;
        for pnl in rows {
            if pnl.unwrap_or(0.0) < 0.0 {
                losses += 1;
            } else {
                break;
            }
        }
        Ok(losses)
    }

    pub async fn close_trade(
        &self,
        trade_id: i64,
        exit_price: f64,
        realized_pnl: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trades SET exit_price = $2, realized_pnl = $3, closed_at = NOW(), state = $4 WHERE id = $1",
        )
        .bind(trade_id)
        .bind(exit_price)
        .bind(realized_pnl)
        .bind(format!("{:?}", TradeState::Settled))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_order_status(&self, exchange_order_id: &str, status: &str) -> Result<()> {
        sqlx::query(
            "UPDATE orders SET status = $2, last_exchange_status = $2, updated_at = NOW() WHERE exchange_order_id = $1",
        )
        .bind(exchange_order_id)
        .bind(status)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn open_exchange_order_ids_for_trade(&self, trade_id: i64) -> Result<Vec<String>> {
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT exchange_order_id FROM orders \
             WHERE trade_id = $1 \
               AND lower(status) NOT IN ('filled', 'canceled', 'cancelled', 'rejected', 'expired')",
        )
        .bind(trade_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn internal_order_id_by_exchange_order_id(
        &self,
        exchange_order_id: &str,
    ) -> Result<Option<i64>> {
        let id = sqlx::query_scalar::<_, i64>("SELECT id FROM orders WHERE exchange_order_id = $1")
            .bind(exchange_order_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(id)
    }

    pub async fn aggregate_fill_qty_by_exchange_order_id(
        &self,
        exchange_order_id: &str,
    ) -> Result<f64> {
        let filled_qty = sqlx::query_scalar::<_, f64>(
            "SELECT COALESCE(SUM(f.size), 0)::double precision \
             FROM fills f \
             JOIN orders o ON o.id = f.order_id \
             WHERE o.exchange_order_id = $1",
        )
        .bind(exchange_order_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(filled_qty)
    }

    pub async fn order_size_by_exchange_order_id(
        &self,
        exchange_order_id: &str,
    ) -> Result<Option<f64>> {
        let size =
            sqlx::query_scalar::<_, f64>("SELECT size FROM orders WHERE exchange_order_id = $1")
                .bind(exchange_order_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(size)
    }

    pub async fn insert_trade_builder_inventory_observation_if_absent(
        &self,
        observation: &TradeBuilderInventoryObservationInput,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO trade_builder_inventory_observations \
              (parent_builder_order_id, observer_builder_order_id, user_id, market_slug, token_id, outcome_label, exchange_order_id, observation_kind, qty_source, baseline_visible_qty, submitted_dynamic_qty, resolved_fill_qty, expected_fee_qty, expected_net_qty, expected_visible_qty, actual_visible_qty, visible_delta_qty, gap_vs_submit_qty, gap_vs_fill_qty, gap_vs_expected_qty, reference_price, fee_rate_bps, fill_to_inventory_ms, payload_json, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, NOW(), NOW()) \
             ON CONFLICT (parent_builder_order_id, observation_kind) DO NOTHING",
        )
        .bind(observation.parent_builder_order_id)
        .bind(observation.observer_builder_order_id)
        .bind(observation.user_id)
        .bind(&observation.market_slug)
        .bind(&observation.token_id)
        .bind(&observation.outcome_label)
        .bind(&observation.exchange_order_id)
        .bind(&observation.observation_kind)
        .bind(&observation.qty_source)
        .bind(observation.baseline_visible_qty)
        .bind(observation.submitted_dynamic_qty)
        .bind(observation.resolved_fill_qty)
        .bind(observation.expected_fee_qty)
        .bind(observation.expected_net_qty)
        .bind(observation.expected_visible_qty)
        .bind(observation.actual_visible_qty)
        .bind(observation.visible_delta_qty)
        .bind(observation.gap_vs_submit_qty)
        .bind(observation.gap_vs_fill_qty)
        .bind(observation.gap_vs_expected_qty)
        .bind(observation.reference_price)
        .bind(observation.fee_rate_bps)
        .bind(observation.fill_to_inventory_ms)
        .bind(&observation.payload_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn upsert_trade_builder_inventory_observation(
        &self,
        observation: &TradeBuilderInventoryObservationInput,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO trade_builder_inventory_observations \
              (parent_builder_order_id, observer_builder_order_id, user_id, market_slug, token_id, outcome_label, exchange_order_id, observation_kind, qty_source, baseline_visible_qty, submitted_dynamic_qty, resolved_fill_qty, expected_fee_qty, expected_net_qty, expected_visible_qty, actual_visible_qty, visible_delta_qty, gap_vs_submit_qty, gap_vs_fill_qty, gap_vs_expected_qty, reference_price, fee_rate_bps, fill_to_inventory_ms, payload_json, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, NOW(), NOW()) \
             ON CONFLICT (parent_builder_order_id, observation_kind) DO UPDATE SET \
               observer_builder_order_id = EXCLUDED.observer_builder_order_id, \
               user_id = EXCLUDED.user_id, \
               market_slug = EXCLUDED.market_slug, \
               token_id = EXCLUDED.token_id, \
               outcome_label = EXCLUDED.outcome_label, \
               exchange_order_id = EXCLUDED.exchange_order_id, \
               qty_source = EXCLUDED.qty_source, \
               baseline_visible_qty = EXCLUDED.baseline_visible_qty, \
               submitted_dynamic_qty = EXCLUDED.submitted_dynamic_qty, \
               resolved_fill_qty = EXCLUDED.resolved_fill_qty, \
               expected_fee_qty = EXCLUDED.expected_fee_qty, \
               expected_net_qty = EXCLUDED.expected_net_qty, \
               expected_visible_qty = EXCLUDED.expected_visible_qty, \
               actual_visible_qty = EXCLUDED.actual_visible_qty, \
               visible_delta_qty = EXCLUDED.visible_delta_qty, \
               gap_vs_submit_qty = EXCLUDED.gap_vs_submit_qty, \
               gap_vs_fill_qty = EXCLUDED.gap_vs_fill_qty, \
               gap_vs_expected_qty = EXCLUDED.gap_vs_expected_qty, \
               reference_price = EXCLUDED.reference_price, \
               fee_rate_bps = EXCLUDED.fee_rate_bps, \
               fill_to_inventory_ms = EXCLUDED.fill_to_inventory_ms, \
               payload_json = EXCLUDED.payload_json, \
               updated_at = NOW()",
        )
        .bind(observation.parent_builder_order_id)
        .bind(observation.observer_builder_order_id)
        .bind(observation.user_id)
        .bind(&observation.market_slug)
        .bind(&observation.token_id)
        .bind(&observation.outcome_label)
        .bind(&observation.exchange_order_id)
        .bind(&observation.observation_kind)
        .bind(&observation.qty_source)
        .bind(observation.baseline_visible_qty)
        .bind(observation.submitted_dynamic_qty)
        .bind(observation.resolved_fill_qty)
        .bind(observation.expected_fee_qty)
        .bind(observation.expected_net_qty)
        .bind(observation.expected_visible_qty)
        .bind(observation.actual_visible_qty)
        .bind(observation.visible_delta_qty)
        .bind(observation.gap_vs_submit_qty)
        .bind(observation.gap_vs_fill_qty)
        .bind(observation.gap_vs_expected_qty)
        .bind(observation.reference_price)
        .bind(observation.fee_rate_bps)
        .bind(observation.fill_to_inventory_ms)
        .bind(&observation.payload_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_pending_trade_builder_first_visible_inventory_observations(
        &self,
        limit: i64,
    ) -> Result<Vec<PendingTradeBuilderFirstVisibleInventoryObservation>> {
        let rows = sqlx::query(
            "SELECT \
                fill_obs.parent_builder_order_id, \
                fill_obs.observer_builder_order_id, \
                fill_obs.user_id, \
                fill_obs.market_slug, \
                fill_obs.token_id, \
                fill_obs.outcome_label, \
                COALESCE(fill_obs.exchange_order_id, submit.exchange_order_id) AS exchange_order_id, \
                baseline.baseline_visible_qty AS baseline_visible_qty, \
                COALESCE(submit.submitted_dynamic_qty, o.submitted_dynamic_qty) AS submitted_dynamic_qty, \
                fill_obs.resolved_fill_qty AS resolved_fill_qty, \
                COALESCE(submit.reference_price, o.submitted_dynamic_price) AS submit_reference_price, \
                fill_obs.reference_price AS fill_reference_price, \
                fill_obs.qty_source AS fill_qty_source, \
                COALESCE(fill_obs.fee_rate_bps, submit.fee_rate_bps, o.fee_rate_bps, 0) AS fee_rate_bps, \
                fill_obs.created_at AS fill_observed_at \
             FROM trade_builder_inventory_observations fill_obs \
             JOIN trade_builder_orders o \
               ON o.id = fill_obs.parent_builder_order_id \
             LEFT JOIN trade_builder_inventory_observations baseline \
               ON baseline.parent_builder_order_id = fill_obs.parent_builder_order_id \
              AND baseline.observation_kind = 'buy_inventory_baseline' \
             LEFT JOIN trade_builder_inventory_observations submit \
               ON submit.parent_builder_order_id = fill_obs.parent_builder_order_id \
              AND submit.observation_kind = 'buy_submit_dynamic_qty' \
             LEFT JOIN trade_builder_inventory_observations first_visible \
               ON first_visible.parent_builder_order_id = fill_obs.parent_builder_order_id \
              AND first_visible.observation_kind = 'first_visible_inventory' \
             WHERE fill_obs.observation_kind = 'buy_fill_resolution' \
               AND first_visible.id IS NULL \
             ORDER BY fill_obs.updated_at ASC \
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| PendingTradeBuilderFirstVisibleInventoryObservation {
                parent_builder_order_id: row.get("parent_builder_order_id"),
                observer_builder_order_id: row.get("observer_builder_order_id"),
                user_id: row.get("user_id"),
                market_slug: row.get("market_slug"),
                token_id: row.get("token_id"),
                outcome_label: row.get("outcome_label"),
                exchange_order_id: row.get("exchange_order_id"),
                baseline_visible_qty: row.get("baseline_visible_qty"),
                submitted_dynamic_qty: row.get("submitted_dynamic_qty"),
                resolved_fill_qty: row.get("resolved_fill_qty"),
                submit_reference_price: row.get("submit_reference_price"),
                fill_reference_price: row.get("fill_reference_price"),
                fill_qty_source: row.get("fill_qty_source"),
                fee_rate_bps: row.get("fee_rate_bps"),
                fill_observed_at: row.get("fill_observed_at"),
            })
            .collect())
    }

    pub async fn try_record_idempotency_key(&self, event_key: &str) -> Result<bool> {
        let rows = sqlx::query("INSERT INTO idempotency_keys (event_key) VALUES ($1) ON CONFLICT (event_key) DO NOTHING")
            .bind(event_key)
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(rows == 1)
    }

    pub async fn record_reconcile_run(
        &self,
        run_id: i64,
        market_slug: &str,
        status: &str,
        details: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO reconcile_runs (run_id, market_slug, status, details, created_at) VALUES ($1, $2, $3, $4, NOW())",
        )
        .bind(run_id)
        .bind(market_slug)
        .bind(status)
        .bind(details)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn upsert_leg_position(
        &self,
        trade_id: i64,
        leg_side: LegSide,
        token_id: &str,
        qty: f64,
        avg_entry: f64,
        levels_filled: i32,
        last_fill_price: Option<f64>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO leg_positions (trade_id, leg_side, token_id, qty, avg_entry, levels_filled, last_fill_price, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, NOW()) \
             ON CONFLICT (trade_id, leg_side) DO UPDATE SET \
               token_id = EXCLUDED.token_id, \
               qty = EXCLUDED.qty, \
               avg_entry = EXCLUDED.avg_entry, \
               levels_filled = EXCLUDED.levels_filled, \
               last_fill_price = EXCLUDED.last_fill_price, \
               updated_at = NOW()",
        )
        .bind(trade_id)
        .bind(leg_side_to_db(leg_side))
        .bind(token_id)
        .bind(qty)
        .bind(avg_entry)
        .bind(levels_filled)
        .bind(last_fill_price)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_leg_positions(&self, trade_id: i64) -> Result<Vec<LegPositionSnapshot>> {
        let rows = sqlx::query_as::<_, (String, String, f64, f64, i32, Option<f64>)>(
            "SELECT leg_side, token_id, qty, avg_entry, levels_filled, last_fill_price \
             FROM leg_positions WHERE trade_id = $1",
        )
        .bind(trade_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(
                |(leg_side_raw, token_id, qty, avg_entry, levels_filled, last_fill_price)| {
                    let leg_side = db_to_leg_side(&leg_side_raw)?;
                    Some(LegPositionSnapshot {
                        leg_side,
                        token_id,
                        qty,
                        avg_entry,
                        levels_filled,
                        last_fill_price,
                    })
                },
            )
            .collect())
    }

    pub async fn ensure_position_exit_rule_defaults(
        &self,
        trade_id: i64,
        default_drop_sell_pct: f64,
    ) -> Result<()> {
        for leg_side in [LegSide::Yes, LegSide::No] {
            sqlx::query(
                "INSERT INTO position_exit_rules (trade_id, leg_side, drop_sell_pct, enabled, updated_at) \
                 VALUES ($1, $2, $3, TRUE, NOW()) \
                 ON CONFLICT (trade_id, leg_side) DO NOTHING",
            )
            .bind(trade_id)
            .bind(leg_side_to_db(leg_side))
            .bind(default_drop_sell_pct)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn upsert_position_exit_rule(
        &self,
        trade_id: i64,
        leg_side: LegSide,
        drop_sell_pct: f64,
        enabled: bool,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO position_exit_rules (trade_id, leg_side, drop_sell_pct, enabled, updated_at) \
             VALUES ($1, $2, $3, $4, NOW()) \
             ON CONFLICT (trade_id, leg_side) DO UPDATE SET \
               drop_sell_pct = EXCLUDED.drop_sell_pct, \
               enabled = EXCLUDED.enabled, \
               updated_at = NOW()",
        )
        .bind(trade_id)
        .bind(leg_side_to_db(leg_side))
        .bind(drop_sell_pct)
        .bind(enabled)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_position_exit_rules(&self, trade_id: i64) -> Result<Vec<PositionExitRule>> {
        let rows = sqlx::query_as::<_, (String, f64, bool)>(
            "SELECT leg_side, drop_sell_pct, enabled FROM position_exit_rules WHERE trade_id = $1",
        )
        .bind(trade_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(raw_leg_side, drop_sell_pct, enabled)| {
                let leg_side = db_to_leg_side(&raw_leg_side)?;
                Some(PositionExitRule {
                    leg_side,
                    drop_sell_pct,
                    enabled,
                })
            })
            .collect())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_pressure_snapshot(
        &self,
        trade_id: i64,
        pressure_score: f64,
        bid_ask_imbalance: Option<f64>,
        sell_ratio: Option<f64>,
        yes_price: Option<f64>,
        no_price: Option<f64>,
        trigger_reason: Option<&str>,
        triggered: bool,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO pressure_snapshots (trade_id, pressure_score, bid_ask_imbalance, sell_ratio, yes_price, no_price, trigger_reason, triggered, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW()) \
             ON CONFLICT (trade_id) DO UPDATE SET \
               pressure_score = EXCLUDED.pressure_score, \
               bid_ask_imbalance = EXCLUDED.bid_ask_imbalance, \
               sell_ratio = EXCLUDED.sell_ratio, \
               yes_price = EXCLUDED.yes_price, \
               no_price = EXCLUDED.no_price, \
               trigger_reason = EXCLUDED.trigger_reason, \
               triggered = EXCLUDED.triggered, \
               updated_at = NOW()",
        )
        .bind(trade_id)
        .bind(pressure_score)
        .bind(bid_ask_imbalance)
        .bind(sell_ratio)
        .bind(yes_price)
        .bind(no_price)
        .bind(trigger_reason)
        .bind(triggered)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_pressure_snapshot(&self, trade_id: i64) -> Result<Option<PressureSnapshot>> {
        let row = sqlx::query_as::<
            _,
            (
                i64,
                f64,
                Option<f64>,
                Option<f64>,
                Option<f64>,
                Option<f64>,
                Option<String>,
                bool,
                DateTime<Utc>,
            ),
        >(
            "SELECT trade_id, pressure_score, bid_ask_imbalance, sell_ratio, yes_price, no_price, trigger_reason, triggered, updated_at \
             FROM pressure_snapshots WHERE trade_id = $1",
        )
        .bind(trade_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(
            |(
                trade_id,
                pressure_score,
                bid_ask_imbalance,
                sell_ratio,
                yes_price,
                no_price,
                trigger_reason,
                triggered,
                updated_at,
            )| PressureSnapshot {
                trade_id,
                pressure_score,
                bid_ask_imbalance,
                sell_ratio,
                yes_price,
                no_price,
                trigger_reason,
                triggered,
                updated_at,
            },
        ))
    }

    pub async fn create_trade_builder_order(
        &self,
        trade_id: i64,
        kind: &str,
        status: &str,
        market_slug: &str,
        token_id: &str,
        outcome_label: &str,
        side: &str,
        execution_mode: &str,
        trigger_condition: Option<&str>,
        trigger_price: Option<f64>,
        max_price: Option<f64>,
        size_basis: &str,
        size_usdc: f64,
        target_qty: Option<f64>,
        remaining_qty: Option<f64>,
        min_price_distance_cent: f64,
        expires_at: Option<DateTime<Utc>>,
        max_triggers: i32,
        parent_order_id: Option<i64>,
        tp_enabled: bool,
        tp_price: Option<f64>,
        sl_enabled: bool,
        sl_price: Option<f64>,
        fee_rate_bps: i64,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO trade_builder_orders \
              (trade_id, user_id, kind, status, market_slug, token_id, outcome_label, side, execution_mode, trigger_condition, trigger_price, max_price, size_basis, size_usdc, target_qty, remaining_qty, min_price_distance_cent, expires_at, max_triggers, triggers_fired, parent_order_id, tp_enabled, tp_price, sl_enabled, sl_price, fee_rate_bps, created_at, updated_at) \
             VALUES \
              ($1, (SELECT user_id FROM trades WHERE id = $1), $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, 0, $19, $20, $21, $22, $23, $24, NOW(), NOW()) \
             RETURNING id",
        )
        .bind(trade_id)
        .bind(kind)
        .bind(status)
        .bind(market_slug)
        .bind(token_id)
        .bind(outcome_label)
        .bind(side)
        .bind(execution_mode)
        .bind(trigger_condition)
        .bind(trigger_price)
        .bind(max_price)
        .bind(size_basis)
        .bind(size_usdc)
        .bind(target_qty)
        .bind(remaining_qty)
        .bind(min_price_distance_cent)
        .bind(expires_at)
        .bind(max_triggers)
        .bind(parent_order_id)
        .bind(tp_enabled)
        .bind(tp_price)
        .bind(sl_enabled)
        .bind(sl_price)
        .bind(fee_rate_bps)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn unblock_next_trade_builder_order(
        &self,
        trade_id: i64,
        token_id: &str,
    ) -> Result<Option<i64>> {
        let row: Option<(i64,)> = sqlx::query_as(
            "UPDATE trade_builder_orders SET status = 'pending', updated_at = NOW() \
             WHERE id = (SELECT id FROM trade_builder_orders \
                         WHERE trade_id = $1 AND token_id = $2 AND status = 'blocked' \
                         ORDER BY id ASC LIMIT 1) \
             RETURNING id",
        )
        .bind(trade_id)
        .bind(token_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(id,)| id))
    }

    pub async fn list_trade_builder_orders_for_processing(
        &self,
        limit: i64,
    ) -> Result<Vec<TradeBuilderOrder>> {
        let rows = sqlx::query(
            "SELECT id, trade_id, user_id, kind, status, market_slug, token_id, outcome_label, side, execution_mode, trigger_condition, trigger_price, max_price, size_basis, size_usdc, target_qty, min_price_distance_cent, expires_at, max_triggers, triggers_fired, active_exchange_order_id, remaining_size, remaining_qty, working_price, last_seen_price, last_error, created_at, updated_at, parent_order_id, tp_enabled, tp_price, sl_enabled, sl_price, filled_qty, fee_rate_bps, trigger_latched, trigger_latched_reason, submitted_dynamic_qty, submitted_dynamic_price \
             FROM trade_builder_orders \
             WHERE status IN ('pending', 'armed', 'triggered', 'open', 'partially_filled', 'canceled_requested', 'inventory_pending') \
                OR (status = 'error' AND trigger_latched = TRUE AND trigger_latched_reason = 'stop_loss') \
             ORDER BY created_at ASC \
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeBuilderOrder {
                id: row.get("id"),
                trade_id: row.get("trade_id"),
                user_id: row.get("user_id"),
                kind: row.get("kind"),
                status: row.get("status"),
                market_slug: row.get("market_slug"),
                token_id: row.get("token_id"),
                outcome_label: row.get("outcome_label"),
                side: row.get("side"),
                execution_mode: row.get("execution_mode"),
                trigger_condition: row.get("trigger_condition"),
                trigger_price: row.get("trigger_price"),
                max_price: row.get("max_price"),
                size_basis: row.get("size_basis"),
                size_usdc: row.get("size_usdc"),
                target_qty: row.get("target_qty"),
                min_price_distance_cent: row.get("min_price_distance_cent"),
                expires_at: row.get("expires_at"),
                max_triggers: row.get("max_triggers"),
                triggers_fired: row.get("triggers_fired"),
                active_exchange_order_id: row.get("active_exchange_order_id"),
                remaining_size: row.get("remaining_size"),
                remaining_qty: row.get("remaining_qty"),
                working_price: row.get("working_price"),
                last_seen_price: row.get("last_seen_price"),
                last_error: row.get("last_error"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                parent_order_id: row.get("parent_order_id"),
                tp_enabled: row.get("tp_enabled"),
                tp_price: row.get("tp_price"),
                sl_enabled: row.get("sl_enabled"),
                sl_price: row.get("sl_price"),
                filled_qty: row.get("filled_qty"),
                fee_rate_bps: row.get("fee_rate_bps"),
                trigger_latched: row.get("trigger_latched"),
                trigger_latched_reason: row.get("trigger_latched_reason"),
                submitted_dynamic_qty: row.get("submitted_dynamic_qty"),
                submitted_dynamic_price: row.get("submitted_dynamic_price"),
            })
            .collect())
    }

    pub async fn list_active_dual_dca_conditional_orders(
        &self,
        job_id: i64,
        market_slug: Option<&str>,
    ) -> Result<Vec<TradeBuilderOrder>> {
        let rows = sqlx::query(
            "SELECT DISTINCT \
                o.id, o.trade_id, o.user_id, o.kind, o.status, o.market_slug, o.token_id, o.outcome_label, o.side, \
                o.execution_mode, o.trigger_condition, o.trigger_price, o.max_price, o.size_basis, o.size_usdc, o.target_qty, o.min_price_distance_cent, o.expires_at, \
                o.max_triggers, o.triggers_fired, o.active_exchange_order_id, o.remaining_size, o.remaining_qty, o.working_price, \
                o.last_seen_price, o.last_error, o.created_at, o.updated_at, \
                o.parent_order_id, o.tp_enabled, o.tp_price, o.sl_enabled, o.sl_price, \
                o.filled_qty, o.fee_rate_bps, o.trigger_latched, o.trigger_latched_reason, o.submitted_dynamic_qty, o.submitted_dynamic_price \
             FROM trade_builder_orders o \
             JOIN trade_flow_dual_dca_legs l ON l.builder_order_id = o.id \
             WHERE l.job_id = $1 \
               AND ($2::text IS NULL OR l.market_slug = $2) \
               AND o.kind = 'conditional' \
               AND o.status IN ('pending', 'armed', 'triggered', 'open', 'partially_filled', 'inventory_pending') \
             ORDER BY o.id ASC",
        )
        .bind(job_id)
        .bind(market_slug)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeBuilderOrder {
                id: row.get("id"),
                trade_id: row.get("trade_id"),
                user_id: row.get("user_id"),
                kind: row.get("kind"),
                status: row.get("status"),
                market_slug: row.get("market_slug"),
                token_id: row.get("token_id"),
                outcome_label: row.get("outcome_label"),
                side: row.get("side"),
                execution_mode: row.get("execution_mode"),
                trigger_condition: row.get("trigger_condition"),
                trigger_price: row.get("trigger_price"),
                max_price: row.get("max_price"),
                size_basis: row.get("size_basis"),
                size_usdc: row.get("size_usdc"),
                target_qty: row.get("target_qty"),
                min_price_distance_cent: row.get("min_price_distance_cent"),
                expires_at: row.get("expires_at"),
                max_triggers: row.get("max_triggers"),
                triggers_fired: row.get("triggers_fired"),
                active_exchange_order_id: row.get("active_exchange_order_id"),
                remaining_size: row.get("remaining_size"),
                remaining_qty: row.get("remaining_qty"),
                working_price: row.get("working_price"),
                last_seen_price: row.get("last_seen_price"),
                last_error: row.get("last_error"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                parent_order_id: row.get("parent_order_id"),
                tp_enabled: row.get("tp_enabled"),
                tp_price: row.get("tp_price"),
                sl_enabled: row.get("sl_enabled"),
                sl_price: row.get("sl_price"),
                filled_qty: row.get("filled_qty"),
                fee_rate_bps: row.get("fee_rate_bps"),
                trigger_latched: row.get("trigger_latched"),
                trigger_latched_reason: row.get("trigger_latched_reason"),
                submitted_dynamic_qty: row.get("submitted_dynamic_qty"),
                submitted_dynamic_price: row.get("submitted_dynamic_price"),
            })
            .collect())
    }

    pub async fn append_trade_builder_order_event(
        &self,
        builder_order_id: i64,
        event_type: &str,
        payload: &Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO trade_builder_order_events (builder_order_id, event_type, payload_json, created_at) \
             VALUES ($1, $2, $3, NOW())",
        )
        .bind(builder_order_id)
        .bind(event_type)
        .bind(payload)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_trade_builder_order_trigger_plan(
        &self,
        builder_order_id: i64,
    ) -> Result<Option<(Option<String>, Vec<f64>)>> {
        let payload = sqlx::query_scalar::<_, Value>(
            "SELECT payload_json
             FROM trade_builder_order_events
             WHERE builder_order_id = $1
               AND event_type = 'flow_created'
             ORDER BY id DESC
             LIMIT 1",
        )
        .bind(builder_order_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(payload) = payload else {
            return Ok(None);
        };

        let size_mode = payload
            .get("size_mode")
            .and_then(Value::as_str)
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| value == "usdc" || value == "pct");

        let trigger_sizes = payload
            .get("trigger_sizes")
            .and_then(Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|item| match item {
                        Value::Number(v) => v.as_f64(),
                        Value::String(v) => v.parse::<f64>().ok(),
                        _ => None,
                    })
                    .filter(|value| value.is_finite() && *value > 0.0)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(Some((size_mode, trigger_sizes)))
    }

    pub async fn set_trade_builder_order_status(
        &self,
        builder_order_id: i64,
        status: &str,
        last_error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders SET status = $2, last_error = $3, updated_at = NOW() WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(status)
        .bind(last_error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_fee_rate_bps(
        &self,
        builder_order_id: i64,
        fee_rate_bps: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders SET fee_rate_bps = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(fee_rate_bps)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_filled_qty(
        &self,
        builder_order_id: i64,
        filled_qty: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders SET filled_qty = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(filled_qty)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_submitted_dynamic(
        &self,
        builder_order_id: i64,
        submitted_dynamic_qty: Option<f64>,
        submitted_dynamic_price: Option<f64>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET submitted_dynamic_qty = $2, submitted_dynamic_price = $3, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(submitted_dynamic_qty)
        .bind(submitted_dynamic_price)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_trigger_latched(
        &self,
        builder_order_id: i64,
        trigger_latched: bool,
        trigger_latched_reason: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET trigger_latched = $2, trigger_latched_reason = $3, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(trigger_latched)
        .bind(trigger_latched_reason)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_last_seen_price(
        &self,
        builder_order_id: i64,
        last_seen_price: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders SET last_seen_price = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(last_seen_price)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_working_state(
        &self,
        builder_order_id: i64,
        active_exchange_order_id: Option<&str>,
        working_price: Option<f64>,
        remaining_size: Option<f64>,
        remaining_qty: Option<f64>,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET active_exchange_order_id = $2, working_price = $3, remaining_size = $4, remaining_qty = $5, status = $6, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(active_exchange_order_id)
        .bind(working_price)
        .bind(remaining_size)
        .bind(remaining_qty)
        .bind(status)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_trade_builder_order_sizing_and_state(
        &self,
        builder_order_id: i64,
        size_basis: &str,
        size_usdc: f64,
        target_qty: Option<f64>,
        remaining_size: Option<f64>,
        remaining_qty: Option<f64>,
        status: &str,
        last_error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET size_basis = $2, size_usdc = $3, target_qty = $4, remaining_size = $5, remaining_qty = $6, \
                 active_exchange_order_id = NULL, working_price = NULL, status = $7, last_error = $8, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(size_basis)
        .bind(size_usdc)
        .bind(target_qty)
        .bind(remaining_size)
        .bind(remaining_qty)
        .bind(status)
        .bind(last_error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_order_retry_state(
        &self,
        builder_order_id: i64,
        status: &str,
        last_error: Option<&str>,
        remaining_size: Option<f64>,
        remaining_qty: Option<f64>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET active_exchange_order_id = NULL, working_price = NULL, remaining_size = $2, remaining_qty = $3, status = $4, last_error = $5, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(remaining_size)
        .bind(remaining_qty)
        .bind(status)
        .bind(last_error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn increment_trade_builder_trigger_count(&self, builder_order_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders SET triggers_fired = triggers_fired + 1, updated_at = NOW() WHERE id = $1",
        )
        .bind(builder_order_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear_trade_builder_active_exchange_order(
        &self,
        builder_order_id: i64,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET active_exchange_order_id = NULL, remaining_size = NULL, remaining_qty = NULL, working_price = NULL, status = $2, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(status)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear_trade_builder_active_exchange_order_preserve_sizing(
        &self,
        builder_order_id: i64,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET active_exchange_order_id = NULL, working_price = NULL, status = $2, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(status)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_trade_builder_order(
        &self,
        builder_order_id: i64,
    ) -> Result<Option<TradeBuilderOrder>> {
        let row = sqlx::query(
            "SELECT id, trade_id, user_id, kind, status, market_slug, token_id, outcome_label, side, execution_mode, trigger_condition, trigger_price, max_price, size_basis, size_usdc, target_qty, min_price_distance_cent, expires_at, max_triggers, triggers_fired, active_exchange_order_id, remaining_size, remaining_qty, working_price, last_seen_price, last_error, created_at, updated_at, parent_order_id, tp_enabled, tp_price, sl_enabled, sl_price, filled_qty, fee_rate_bps, trigger_latched, trigger_latched_reason, submitted_dynamic_qty, submitted_dynamic_price \
             FROM trade_builder_orders WHERE id = $1",
        )
        .bind(builder_order_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| TradeBuilderOrder {
            id: row.get("id"),
            trade_id: row.get("trade_id"),
            user_id: row.get("user_id"),
            kind: row.get("kind"),
            status: row.get("status"),
            market_slug: row.get("market_slug"),
            token_id: row.get("token_id"),
            outcome_label: row.get("outcome_label"),
            side: row.get("side"),
            execution_mode: row.get("execution_mode"),
            trigger_condition: row.get("trigger_condition"),
            trigger_price: row.get("trigger_price"),
            max_price: row.get("max_price"),
            size_basis: row.get("size_basis"),
            size_usdc: row.get("size_usdc"),
            target_qty: row.get("target_qty"),
            min_price_distance_cent: row.get("min_price_distance_cent"),
            expires_at: row.get("expires_at"),
            max_triggers: row.get("max_triggers"),
            triggers_fired: row.get("triggers_fired"),
            active_exchange_order_id: row.get("active_exchange_order_id"),
            remaining_size: row.get("remaining_size"),
            remaining_qty: row.get("remaining_qty"),
            working_price: row.get("working_price"),
            last_seen_price: row.get("last_seen_price"),
            last_error: row.get("last_error"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            parent_order_id: row.get("parent_order_id"),
            tp_enabled: row.get("tp_enabled"),
            tp_price: row.get("tp_price"),
            sl_enabled: row.get("sl_enabled"),
            sl_price: row.get("sl_price"),
            filled_qty: row.get("filled_qty"),
            fee_rate_bps: row.get("fee_rate_bps"),
            trigger_latched: row.get("trigger_latched"),
            trigger_latched_reason: row.get("trigger_latched_reason"),
            submitted_dynamic_qty: row.get("submitted_dynamic_qty"),
            submitted_dynamic_price: row.get("submitted_dynamic_price"),
        }))
    }

    pub async fn list_trade_builder_child_orders_by_parent(
        &self,
        parent_id: i64,
        exclude_order_id: Option<i64>,
    ) -> Result<Vec<TradeBuilderOrder>> {
        let rows = sqlx::query(
            "SELECT id, trade_id, user_id, kind, status, market_slug, token_id, outcome_label, side, execution_mode, trigger_condition, trigger_price, max_price, size_basis, size_usdc, target_qty, min_price_distance_cent, expires_at, max_triggers, triggers_fired, active_exchange_order_id, remaining_size, remaining_qty, working_price, last_seen_price, last_error, created_at, updated_at, parent_order_id, tp_enabled, tp_price, sl_enabled, sl_price, filled_qty, fee_rate_bps, trigger_latched, trigger_latched_reason, submitted_dynamic_qty, submitted_dynamic_price \
             FROM trade_builder_orders \
             WHERE parent_order_id = $1
               AND ($2::bigint IS NULL OR id <> $2)
             ORDER BY id ASC",
        )
        .bind(parent_id)
        .bind(exclude_order_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeBuilderOrder {
                id: row.get("id"),
                trade_id: row.get("trade_id"),
                user_id: row.get("user_id"),
                kind: row.get("kind"),
                status: row.get("status"),
                market_slug: row.get("market_slug"),
                token_id: row.get("token_id"),
                outcome_label: row.get("outcome_label"),
                side: row.get("side"),
                execution_mode: row.get("execution_mode"),
                trigger_condition: row.get("trigger_condition"),
                trigger_price: row.get("trigger_price"),
                max_price: row.get("max_price"),
                size_basis: row.get("size_basis"),
                size_usdc: row.get("size_usdc"),
                target_qty: row.get("target_qty"),
                min_price_distance_cent: row.get("min_price_distance_cent"),
                expires_at: row.get("expires_at"),
                max_triggers: row.get("max_triggers"),
                triggers_fired: row.get("triggers_fired"),
                active_exchange_order_id: row.get("active_exchange_order_id"),
                remaining_size: row.get("remaining_size"),
                remaining_qty: row.get("remaining_qty"),
                working_price: row.get("working_price"),
                last_seen_price: row.get("last_seen_price"),
                last_error: row.get("last_error"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                parent_order_id: row.get("parent_order_id"),
                tp_enabled: row.get("tp_enabled"),
                tp_price: row.get("tp_price"),
                sl_enabled: row.get("sl_enabled"),
                sl_price: row.get("sl_price"),
                filled_qty: row.get("filled_qty"),
                fee_rate_bps: row.get("fee_rate_bps"),
                trigger_latched: row.get("trigger_latched"),
                trigger_latched_reason: row.get("trigger_latched_reason"),
                submitted_dynamic_qty: row.get("submitted_dynamic_qty"),
                submitted_dynamic_price: row.get("submitted_dynamic_price"),
            })
            .collect())
    }

    pub async fn cancel_child_orders_by_parent(&self, parent_id: i64) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE trade_builder_orders SET status = 'canceled', updated_at = NOW() \
             WHERE parent_order_id = $1 AND status NOT IN ('completed', 'canceled', 'expired', 'filled')",
        )
        .bind(parent_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_trade_builder_workflows_for_processing(
        &self,
        limit: i64,
    ) -> Result<Vec<TradeBuilderWorkflow>> {
        let rows = sqlx::query(
            "SELECT id, user_id, name, status, source_trade_id, sell_target_pct, buy_start_after_sell_progress_pct, buy_trigger_mode, buy_allocation_pct, expires_at, last_error, created_at, updated_at \
             FROM trade_builder_workflows \
             WHERE status IN ('armed', 'running') \
             ORDER BY created_at ASC \
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeBuilderWorkflow {
                id: row.get("id"),
                user_id: row.get("user_id"),
                name: row.get("name"),
                status: row.get("status"),
                source_trade_id: row.get("source_trade_id"),
                sell_target_pct: row.get("sell_target_pct"),
                buy_start_after_sell_progress_pct: row.get("buy_start_after_sell_progress_pct"),
                buy_trigger_mode: row.get("buy_trigger_mode"),
                buy_allocation_pct: row.get("buy_allocation_pct"),
                expires_at: row.get("expires_at"),
                last_error: row.get("last_error"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn load_trade_builder_workflow_legs(
        &self,
        workflow_id: i64,
    ) -> Result<Vec<TradeBuilderWorkflowLeg>> {
        let rows = sqlx::query(
            "SELECT id, workflow_id, leg_type, market_slug, token_id, outcome_label, side, trigger_condition, trigger_price, min_price_distance_cent, status, builder_order_id, target_notional_usdc, allocated_notional_usdc, filled_notional_usdc, filled_qty, last_seen_price, created_at, updated_at \
             FROM trade_builder_workflow_legs \
             WHERE workflow_id = $1 \
             ORDER BY leg_type ASC, id ASC",
        )
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeBuilderWorkflowLeg {
                id: row.get("id"),
                workflow_id: row.get("workflow_id"),
                leg_type: row.get("leg_type"),
                market_slug: row.get("market_slug"),
                token_id: row.get("token_id"),
                outcome_label: row.get("outcome_label"),
                side: row.get("side"),
                trigger_condition: row.get("trigger_condition"),
                trigger_price: row.get("trigger_price"),
                min_price_distance_cent: row.get("min_price_distance_cent"),
                status: row.get("status"),
                builder_order_id: row.get("builder_order_id"),
                target_notional_usdc: row.get("target_notional_usdc"),
                allocated_notional_usdc: row.get("allocated_notional_usdc"),
                filled_notional_usdc: row.get("filled_notional_usdc"),
                filled_qty: row.get("filled_qty"),
                last_seen_price: row.get("last_seen_price"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn set_trade_builder_workflow_status(
        &self,
        workflow_id: i64,
        status: &str,
        last_error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_workflows \
             SET status = $2, last_error = $3, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(workflow_id)
        .bind(status)
        .bind(last_error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn append_trade_builder_workflow_event(
        &self,
        workflow_id: i64,
        leg_id: Option<i64>,
        event_type: &str,
        payload: &Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO trade_builder_workflow_events (workflow_id, leg_id, event_type, payload_json, created_at) \
             VALUES ($1, $2, $3, $4, NOW())",
        )
        .bind(workflow_id)
        .bind(leg_id)
        .bind(event_type)
        .bind(payload)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_workflow_leg_status(
        &self,
        leg_id: i64,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_workflow_legs SET status = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(leg_id)
        .bind(status)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_workflow_leg_builder_order(
        &self,
        leg_id: i64,
        builder_order_id: Option<i64>,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_workflow_legs \
             SET builder_order_id = $2, status = $3, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(leg_id)
        .bind(builder_order_id)
        .bind(status)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_workflow_leg_last_seen_price(
        &self,
        leg_id: i64,
        price: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_workflow_legs \
             SET last_seen_price = $2, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(leg_id)
        .bind(price)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn add_trade_builder_workflow_leg_allocated_notional(
        &self,
        leg_id: i64,
        delta_usdc: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_workflow_legs \
             SET allocated_notional_usdc = GREATEST(0, allocated_notional_usdc + $2), updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(leg_id)
        .bind(delta_usdc)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_trade_builder_workflow_leg_filled_metrics(
        &self,
        leg_id: i64,
        filled_notional_usdc: f64,
        filled_qty: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_workflow_legs \
             SET filled_notional_usdc = GREATEST(0, $2), \
                 filled_qty = GREATEST(0, $3), \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(leg_id)
        .bind(filled_notional_usdc)
        .bind(filled_qty)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn aggregate_trade_builder_workflow_leg_fills(
        &self,
        leg_id: i64,
    ) -> Result<(f64, f64)> {
        let (filled_notional_usdc, filled_qty) = sqlx::query_as::<_, (f64, f64)>(
            "WITH child_orders AS (
               SELECT DISTINCT (wfe.payload_json->>'builder_order_id')::bigint AS builder_order_id
               FROM trade_builder_workflow_events wfe
               WHERE wfe.leg_id = $1
                 AND wfe.payload_json ? 'builder_order_id'
                 AND (wfe.payload_json->>'builder_order_id') ~ '^[0-9]+$'
             ),
             exchange_ids AS (
               SELECT DISTINCT x.exchange_order_id
               FROM (
                 SELECT tboe.payload_json->>'exchange_order_id' AS exchange_order_id
                 FROM trade_builder_order_events tboe
                 JOIN child_orders co ON co.builder_order_id = tboe.builder_order_id
                 UNION ALL
                 SELECT tboe.payload_json->>'new_exchange_order_id' AS exchange_order_id
                 FROM trade_builder_order_events tboe
                 JOIN child_orders co ON co.builder_order_id = tboe.builder_order_id
                 UNION ALL
                 SELECT tboe.payload_json->>'prev_exchange_order_id' AS exchange_order_id
                 FROM trade_builder_order_events tboe
                 JOIN child_orders co ON co.builder_order_id = tboe.builder_order_id
               ) x
               WHERE x.exchange_order_id IS NOT NULL
                 AND x.exchange_order_id <> ''
             ),
             internal_orders AS (
               SELECT DISTINCT o.id
               FROM orders o
               JOIN exchange_ids e ON e.exchange_order_id = o.exchange_order_id
             )
             SELECT
               COALESCE(SUM(f.price * f.size), 0)::double precision AS filled_notional_usdc,
               COALESCE(SUM(f.size), 0)::double precision AS filled_qty
             FROM fills f
             JOIN internal_orders io ON io.id = f.order_id",
        )
        .bind(leg_id)
        .fetch_one(&self.pool)
        .await?;

        Ok((filled_notional_usdc, filled_qty))
    }

    pub async fn aggregate_trade_fills(&self, trade_id: i64) -> Result<(f64, f64)> {
        let (filled_notional_usdc, filled_qty) = sqlx::query_as::<_, (f64, f64)>(
            "SELECT \
               COALESCE(SUM(f.price * f.size), 0)::double precision AS filled_notional_usdc, \
               COALESCE(SUM(f.size), 0)::double precision AS filled_qty \
             FROM fills f \
             JOIN orders o ON o.id = f.order_id \
             WHERE o.trade_id = $1",
        )
        .bind(trade_id)
        .fetch_one(&self.pool)
        .await?;
        Ok((filled_notional_usdc, filled_qty))
    }

    pub async fn aggregate_trade_fill_by_token(
        &self,
        trade_id: i64,
        token_ids: &[String],
    ) -> Result<Vec<TradeFillTokenAggregate>> {
        if token_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            "SELECT \
               o.token_id AS token_id, \
               COALESCE(SUM(CASE WHEN lower(o.side) = 'buy' THEN f.size ELSE 0 END), 0)::double precision AS buy_qty, \
               COALESCE(SUM(CASE WHEN lower(o.side) = 'buy' THEN f.price * f.size ELSE 0 END), 0)::double precision AS buy_notional_usdc, \
               COALESCE(SUM(CASE WHEN lower(o.side) = 'sell' THEN f.size ELSE 0 END), 0)::double precision AS sell_qty, \
               COALESCE(SUM(CASE WHEN lower(o.side) = 'sell' THEN f.price * f.size ELSE 0 END), 0)::double precision AS sell_notional_usdc \
             FROM fills f \
             JOIN orders o ON o.id = f.order_id \
             WHERE o.trade_id = $1 \
               AND o.token_id = ANY($2::text[]) \
             GROUP BY o.token_id",
        )
        .bind(trade_id)
        .bind(token_ids)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeFillTokenAggregate {
                token_id: row.get("token_id"),
                buy_qty: row.get("buy_qty"),
                buy_notional_usdc: row.get("buy_notional_usdc"),
                sell_qty: row.get("sell_qty"),
                sell_notional_usdc: row.get("sell_notional_usdc"),
            })
            .collect())
    }

    pub async fn trade_notional_usdc(&self, trade_id: i64) -> Result<Option<f64>> {
        let value = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT notional_usdc FROM trades WHERE id = $1 LIMIT 1",
        )
        .bind(trade_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(value)
    }

    pub async fn list_published_trade_flow_definitions(
        &self,
        limit: i64,
    ) -> Result<Vec<TradeFlowDefinitionRuntime>> {
        let rows = sqlx::query(
            "SELECT id, user_id, name, status, draft_version_id, published_version_id, last_error, created_at, updated_at \
             FROM trade_flow_definitions \
             WHERE status = 'published' AND published_version_id IS NOT NULL \
             ORDER BY updated_at ASC \
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeFlowDefinitionRuntime {
                id: row.get("id"),
                user_id: row.get("user_id"),
                name: row.get("name"),
                status: row.get("status"),
                draft_version_id: row.get("draft_version_id"),
                published_version_id: row.get("published_version_id"),
                last_error: row.get("last_error"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn has_active_trade_flow_auto_claim_enabled(&self) -> Result<bool> {
        let enabled = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
               SELECT 1
               FROM trade_flow_definitions d
               LEFT JOIN trade_flow_versions draft_v ON draft_v.id = d.draft_version_id
               LEFT JOIN trade_flow_versions published_v ON published_v.id = d.published_version_id
               WHERE d.status <> 'archived'
                 AND LOWER(
                   COALESCE(
                     CASE
                       WHEN d.draft_version_id IS NOT NULL
                         THEN draft_v.graph_json #>> '{context,autoClaimEnabled}'
                       WHEN d.published_version_id IS NOT NULL
                         THEN published_v.graph_json #>> '{context,autoClaimEnabled}'
                       ELSE NULL
                     END,
                     'false'
                   )
                 )
                     IN ('true', '1', 'yes', 'on')
             )",
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(enabled)
    }

    pub async fn get_trade_flow_definition(
        &self,
        definition_id: i64,
    ) -> Result<Option<TradeFlowDefinitionRuntime>> {
        let row = sqlx::query(
            "SELECT id, user_id, name, status, draft_version_id, published_version_id, last_error, created_at, updated_at \
             FROM trade_flow_definitions \
             WHERE id = $1 \
             LIMIT 1",
        )
        .bind(definition_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| TradeFlowDefinitionRuntime {
            id: row.get("id"),
            user_id: row.get("user_id"),
            name: row.get("name"),
            status: row.get("status"),
            draft_version_id: row.get("draft_version_id"),
            published_version_id: row.get("published_version_id"),
            last_error: row.get("last_error"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    pub async fn get_trade_flow_version(
        &self,
        version_id: i64,
    ) -> Result<Option<TradeFlowVersionRuntime>> {
        let row = sqlx::query(
            "SELECT id, definition_id, version_no, status, graph_json, published_at, created_at \
             FROM trade_flow_versions \
             WHERE id = $1 \
             LIMIT 1",
        )
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| TradeFlowVersionRuntime {
            id: row.get("id"),
            definition_id: row.get("definition_id"),
            version_no: row.get("version_no"),
            status: row.get("status"),
            graph_json: row.get("graph_json"),
            published_at: row.get("published_at"),
            created_at: row.get("created_at"),
        }))
    }

    pub async fn archive_trade_flow_definition(&self, definition_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE trade_flow_definitions \
             SET status = 'archived', updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(definition_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_active_trade_flow_run(
        &self,
        definition_id: i64,
    ) -> Result<Option<TradeFlowRun>> {
        let row = sqlx::query(
            "SELECT id, definition_id, version_id, user_id, status, trigger_source, context_json, started_at, ended_at, last_error, created_at, updated_at \
             FROM trade_flow_runs \
             WHERE definition_id = $1 AND status = 'running' \
             ORDER BY created_at DESC \
             LIMIT 1",
        )
        .bind(definition_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| TradeFlowRun {
            id: row.get("id"),
            definition_id: row.get("definition_id"),
            version_id: row.get("version_id"),
            user_id: row.get("user_id"),
            status: row.get("status"),
            trigger_source: row.get("trigger_source"),
            context_json: row.get("context_json"),
            started_at: row.get("started_at"),
            ended_at: row.get("ended_at"),
            last_error: row.get("last_error"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    pub async fn create_trade_flow_run(
        &self,
        definition_id: i64,
        version_id: i64,
        trigger_source: Option<&str>,
        context_json: &Value,
    ) -> Result<TradeFlowRun> {
        let row = sqlx::query(
            "INSERT INTO trade_flow_runs \
              (definition_id, version_id, user_id, status, trigger_source, context_json, started_at, created_at, updated_at) \
             VALUES \
              ($1, $2, (SELECT user_id FROM trade_flow_definitions WHERE id = $1), 'running', $3, $4, NOW(), NOW(), NOW()) \
             RETURNING id, definition_id, version_id, user_id, status, trigger_source, context_json, started_at, ended_at, last_error, created_at, updated_at",
        )
        .bind(definition_id)
        .bind(version_id)
        .bind(trigger_source)
        .bind(context_json)
        .fetch_one(&self.pool)
        .await?;

        Ok(TradeFlowRun {
            id: row.get("id"),
            definition_id: row.get("definition_id"),
            version_id: row.get("version_id"),
            user_id: row.get("user_id"),
            status: row.get("status"),
            trigger_source: row.get("trigger_source"),
            context_json: row.get("context_json"),
            started_at: row.get("started_at"),
            ended_at: row.get("ended_at"),
            last_error: row.get("last_error"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    pub async fn get_trade_flow_run(&self, run_id: i64) -> Result<Option<TradeFlowRun>> {
        let row = sqlx::query(
            "SELECT id, definition_id, version_id, user_id, status, trigger_source, context_json, started_at, ended_at, last_error, created_at, updated_at \
             FROM trade_flow_runs \
             WHERE id = $1 \
             LIMIT 1",
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| TradeFlowRun {
            id: row.get("id"),
            definition_id: row.get("definition_id"),
            version_id: row.get("version_id"),
            user_id: row.get("user_id"),
            status: row.get("status"),
            trigger_source: row.get("trigger_source"),
            context_json: row.get("context_json"),
            started_at: row.get("started_at"),
            ended_at: row.get("ended_at"),
            last_error: row.get("last_error"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    pub async fn load_user_settings_payloads(
        &self,
        user_id: i64,
    ) -> Result<HashMap<String, Value>> {
        let rows = sqlx::query(
            "SELECT config_name, payload_json
             FROM user_settings
             WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        let mut out = HashMap::new();
        for row in rows {
            let name: String = row.get("config_name");
            let payload: Value = row.get("payload_json");
            out.insert(name, payload);
        }
        Ok(out)
    }

    pub async fn set_trade_flow_run_status(
        &self,
        run_id: i64,
        status: &str,
        last_error: Option<&str>,
    ) -> Result<()> {
        let ended_at_clause = if matches!(status, "completed" | "failed" | "canceled") {
            "ended_at = NOW(),"
        } else {
            ""
        };
        let query = format!(
            "UPDATE trade_flow_runs \
             SET status = $2, {ended_at_clause} last_error = $3, updated_at = NOW() \
             WHERE id = $1"
        );
        sqlx::query(&query)
            .bind(run_id)
            .bind(status)
            .bind(last_error)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_trade_flow_run_context(
        &self,
        run_id: i64,
        context_json: &Value,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_flow_runs \
             SET context_json = $2, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(run_id)
        .bind(context_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn append_trade_flow_event(
        &self,
        run_id: Option<i64>,
        definition_id: i64,
        version_id: Option<i64>,
        event_type: &str,
        payload_json: &Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO trade_flow_events \
              (run_id, definition_id, version_id, event_type, payload_json, created_at) \
             VALUES \
              ($1, $2, $3, $4, $5, NOW())",
        )
        .bind(run_id)
        .bind(definition_id)
        .bind(version_id)
        .bind(event_type)
        .bind(payload_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn enqueue_trade_flow_step(
        &self,
        run_id: i64,
        node_key: &str,
        node_type: &str,
        attempt: i32,
        input_json: Option<&Value>,
        available_at: DateTime<Utc>,
        parent_step_id: Option<i64>,
        idempotency_key: Option<&str>,
    ) -> Result<Option<i64>> {
        let row = sqlx::query(
            "INSERT INTO trade_flow_run_steps \
              (run_id, node_key, node_type, status, attempt, input_json, output_json, error_text, started_at, ended_at, available_at, parent_step_id, idempotency_key, created_at) \
             VALUES \
              ($1, $2, $3, 'queued', $4, $5, NULL, NULL, NULL, NULL, $6, $7, $8, NOW()) \
             ON CONFLICT (run_id, idempotency_key) WHERE idempotency_key IS NOT NULL \
             DO NOTHING \
             RETURNING id",
        )
        .bind(run_id)
        .bind(node_key)
        .bind(node_type)
        .bind(attempt)
        .bind(input_json)
        .bind(available_at)
        .bind(parent_step_id)
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| row.get("id")))
    }

    pub async fn list_ready_trade_flow_steps(&self, limit: i64) -> Result<Vec<TradeFlowRunStep>> {
        let rows = sqlx::query(
            "SELECT id, run_id, node_key, node_type, status, attempt, input_json, output_json, error_text, started_at, ended_at, available_at, parent_step_id, idempotency_key, created_at \
             FROM trade_flow_run_steps \
             WHERE status = 'queued' AND available_at <= NOW() \
             ORDER BY available_at ASC, id ASC \
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeFlowRunStep {
                id: row.get("id"),
                run_id: row.get("run_id"),
                node_key: row.get("node_key"),
                node_type: row.get("node_type"),
                status: row.get("status"),
                attempt: row.get("attempt"),
                input_json: row.get("input_json"),
                output_json: row.get("output_json"),
                error_text: row.get("error_text"),
                started_at: row.get("started_at"),
                ended_at: row.get("ended_at"),
                available_at: row.get("available_at"),
                parent_step_id: row.get("parent_step_id"),
                idempotency_key: row.get("idempotency_key"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn claim_ready_trade_flow_steps(&self, limit: i64) -> Result<Vec<TradeFlowRunStep>> {
        let rows = sqlx::query(
            "WITH claimable AS (
               SELECT id
               FROM trade_flow_run_steps
               WHERE status = 'queued' AND available_at <= NOW()
               ORDER BY available_at ASC, id ASC
               LIMIT $1
               FOR UPDATE SKIP LOCKED
             )
             UPDATE trade_flow_run_steps s
             SET status = 'running', started_at = NOW()
             FROM claimable
             WHERE s.id = claimable.id
             RETURNING s.id, s.run_id, s.node_key, s.node_type, s.status, s.attempt, s.input_json, s.output_json, s.error_text, s.started_at, s.ended_at, s.available_at, s.parent_step_id, s.idempotency_key, s.created_at",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeFlowRunStep {
                id: row.get("id"),
                run_id: row.get("run_id"),
                node_key: row.get("node_key"),
                node_type: row.get("node_type"),
                status: row.get("status"),
                attempt: row.get("attempt"),
                input_json: row.get("input_json"),
                output_json: row.get("output_json"),
                error_text: row.get("error_text"),
                started_at: row.get("started_at"),
                ended_at: row.get("ended_at"),
                available_at: row.get("available_at"),
                parent_step_id: row.get("parent_step_id"),
                idempotency_key: row.get("idempotency_key"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn mark_trade_flow_step_running(&self, step_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE trade_flow_run_steps \
             SET status = 'running', started_at = NOW() \
             WHERE id = $1",
        )
        .bind(step_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_trade_flow_step_completed(
        &self,
        step_id: i64,
        output_json: Option<&Value>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_flow_run_steps \
             SET status = 'completed', output_json = $2, error_text = NULL, ended_at = NOW() \
             WHERE id = $1",
        )
        .bind(step_id)
        .bind(output_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_trade_flow_step_failed(
        &self,
        step_id: i64,
        output_json: Option<&Value>,
        error_text: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_flow_run_steps \
             SET status = 'failed', output_json = $2, error_text = $3, ended_at = NOW() \
             WHERE id = $1",
        )
        .bind(step_id)
        .bind(output_json)
        .bind(error_text)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_trade_flow_step_skipped(
        &self,
        step_id: i64,
        output_json: Option<&Value>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_flow_run_steps \
             SET status = 'skipped', output_json = $2, error_text = NULL, ended_at = NOW() \
             WHERE id = $1",
        )
        .bind(step_id)
        .bind(output_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_trade_flow_dual_dca_job(
        &self,
        flow_run_id: i64,
        flow_definition_id: i64,
        flow_version_id: Option<i64>,
        node_key: &str,
        source_trade_id: Option<i64>,
        market_asset: &str,
        market_timeframe: &str,
        side_mode: &str,
        base_sizing: &str,
        base_shares: Option<f64>,
        base_usdc: Option<f64>,
        base_price_usdc: Option<f64>,
        dca_levels: i32,
        near_step: f64,
        step_mult: f64,
        size_mult: f64,
        min_price_distance_cent: f64,
        cutoff_min: i32,
        tp_profit_pct: f64,
        sl_loss_pct: f64,
        sl_spread_pct: f64,
    ) -> Result<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO trade_flow_dual_dca_jobs \
              (flow_run_id, flow_definition_id, flow_version_id, node_key, status, source_trade_id, \
               market_asset, market_timeframe, side_mode, base_sizing, base_shares, base_usdc, base_price_usdc, \
               dca_levels, near_step, step_mult, size_mult, min_price_distance_cent, cutoff_min, \
               tp_profit_pct, sl_loss_pct, sl_spread_pct, next_check_at, last_error, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, $4, 'active', $5, $6, $7, $8, $9, $10, $11, $12, \
               $13, $14, $15, $16, $17, $18, $19, $20, $21, NOW(), NULL, NOW(), NOW()) \
             ON CONFLICT (flow_run_id, node_key) DO UPDATE SET \
               flow_definition_id = EXCLUDED.flow_definition_id, \
               flow_version_id = EXCLUDED.flow_version_id, \
               source_trade_id = COALESCE(EXCLUDED.source_trade_id, trade_flow_dual_dca_jobs.source_trade_id), \
               market_asset = EXCLUDED.market_asset, \
               market_timeframe = EXCLUDED.market_timeframe, \
               side_mode = EXCLUDED.side_mode, \
               base_sizing = EXCLUDED.base_sizing, \
               base_shares = EXCLUDED.base_shares, \
               base_usdc = EXCLUDED.base_usdc, \
               base_price_usdc = EXCLUDED.base_price_usdc, \
               dca_levels = EXCLUDED.dca_levels, \
               near_step = EXCLUDED.near_step, \
               step_mult = EXCLUDED.step_mult, \
               size_mult = EXCLUDED.size_mult, \
               min_price_distance_cent = EXCLUDED.min_price_distance_cent, \
               cutoff_min = EXCLUDED.cutoff_min, \
               tp_profit_pct = EXCLUDED.tp_profit_pct, \
               sl_loss_pct = EXCLUDED.sl_loss_pct, \
               sl_spread_pct = EXCLUDED.sl_spread_pct, \
               status = CASE \
                 WHEN trade_flow_dual_dca_jobs.status IN ('paused', 'completed', 'canceled') THEN trade_flow_dual_dca_jobs.status \
                 ELSE 'active' \
               END, \
               next_check_at = NOW(), \
               last_error = NULL, \
               updated_at = NOW() \
             RETURNING id",
        )
        .bind(flow_run_id)
        .bind(flow_definition_id)
        .bind(flow_version_id)
        .bind(node_key)
        .bind(source_trade_id)
        .bind(market_asset)
        .bind(market_timeframe)
        .bind(side_mode)
        .bind(base_sizing)
        .bind(base_shares)
        .bind(base_usdc)
        .bind(base_price_usdc)
        .bind(dca_levels)
        .bind(near_step)
        .bind(step_mult)
        .bind(size_mult)
        .bind(min_price_distance_cent)
        .bind(cutoff_min)
        .bind(tp_profit_pct)
        .bind(sl_loss_pct)
        .bind(sl_spread_pct)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn list_trade_flow_dual_dca_jobs_for_processing(
        &self,
        limit: i64,
    ) -> Result<Vec<TradeFlowDualDcaJob>> {
        let rows = sqlx::query(
            "SELECT id, flow_run_id, flow_definition_id, flow_version_id, node_key, status, source_trade_id, \
                    market_asset, market_timeframe, side_mode, base_sizing, base_shares, base_usdc, base_price_usdc, \
                    dca_levels, near_step, step_mult, size_mult, min_price_distance_cent, cutoff_min, \
                    tp_profit_pct, sl_loss_pct, sl_spread_pct, last_market_slug, last_market_started_at, \
                    last_market_ends_at, next_check_at, created_order_count, consecutive_errors, last_error, created_at, updated_at \
             FROM trade_flow_dual_dca_jobs \
             WHERE status = 'active' AND next_check_at <= NOW() \
             ORDER BY next_check_at ASC, id ASC \
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| TradeFlowDualDcaJob {
                id: row.get("id"),
                flow_run_id: row.get("flow_run_id"),
                flow_definition_id: row.get("flow_definition_id"),
                flow_version_id: row.get("flow_version_id"),
                node_key: row.get("node_key"),
                status: row.get("status"),
                source_trade_id: row.get("source_trade_id"),
                market_asset: row.get("market_asset"),
                market_timeframe: row.get("market_timeframe"),
                side_mode: row.get("side_mode"),
                base_sizing: row.get("base_sizing"),
                base_shares: row.get("base_shares"),
                base_usdc: row.get("base_usdc"),
                base_price_usdc: row.get("base_price_usdc"),
                dca_levels: row.get("dca_levels"),
                near_step: row.get("near_step"),
                step_mult: row.get("step_mult"),
                size_mult: row.get("size_mult"),
                min_price_distance_cent: row.get("min_price_distance_cent"),
                cutoff_min: row.get("cutoff_min"),
                tp_profit_pct: row.get("tp_profit_pct"),
                sl_loss_pct: row.get("sl_loss_pct"),
                sl_spread_pct: row.get("sl_spread_pct"),
                last_market_slug: row.get("last_market_slug"),
                last_market_started_at: row.get("last_market_started_at"),
                last_market_ends_at: row.get("last_market_ends_at"),
                next_check_at: row.get("next_check_at"),
                created_order_count: row.get("created_order_count"),
                consecutive_errors: row.get("consecutive_errors"),
                last_error: row.get("last_error"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn update_trade_flow_dual_dca_job_market_state(
        &self,
        job_id: i64,
        last_market_slug: Option<&str>,
        last_market_started_at: Option<DateTime<Utc>>,
        last_market_ends_at: Option<DateTime<Utc>>,
        next_check_at: DateTime<Utc>,
        created_order_delta: i32,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_flow_dual_dca_jobs \
             SET last_market_slug = $2, \
                 last_market_started_at = $3, \
                 last_market_ends_at = $4, \
                 next_check_at = $5, \
                 created_order_count = GREATEST(0, created_order_count + $6), \
                 consecutive_errors = 0, \
                 last_error = NULL, \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(job_id)
        .bind(last_market_slug)
        .bind(last_market_started_at)
        .bind(last_market_ends_at)
        .bind(next_check_at)
        .bind(created_order_delta)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn schedule_trade_flow_dual_dca_job_check(
        &self,
        job_id: i64,
        next_check_at: DateTime<Utc>,
        last_error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_flow_dual_dca_jobs \
             SET next_check_at = $2, \
                 last_error = $3, \
                 consecutive_errors = consecutive_errors + 1, \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(job_id)
        .bind(next_check_at)
        .bind(last_error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_trade_flow_dual_dca_job_status(
        &self,
        job_id: i64,
        status: &str,
        last_error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_flow_dual_dca_jobs \
             SET status = $2, last_error = $3, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(job_id)
        .bind(status)
        .bind(last_error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    fn row_to_dual_dca_leg(r: &sqlx::postgres::PgRow) -> TradeFlowDualDcaLeg {
        use sqlx::Row;
        TradeFlowDualDcaLeg {
            id: r.get("id"),
            job_id: r.get("job_id"),
            market_slug: r.get("market_slug"),
            token_id: r.get("token_id"),
            outcome_label: r.get("outcome_label"),
            side: r.get("side"),
            level_index: r.get("level_index"),
            trigger_condition: r.get("trigger_condition"),
            trigger_price: r.get("trigger_price"),
            size_usdc: r.get("size_usdc"),
            reference_price: r.get("reference_price"),
            builder_order_id: r.get("builder_order_id"),
            status: r.get("status"),
            active_exchange_order_id: r.get("active_exchange_order_id"),
            client_order_id: r.get("client_order_id"),
            filled_price: r.get("filled_price"),
            filled_size: r.get("filled_size"),
            submitted_at: r.get("submitted_at"),
            filled_at: r.get("filled_at"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_trade_flow_dual_dca_leg(
        &self,
        job_id: i64,
        market_slug: &str,
        token_id: &str,
        outcome_label: &str,
        side: &str,
        level_index: i32,
        trigger_condition: Option<&str>,
        trigger_price: Option<f64>,
        size_usdc: f64,
        reference_price: Option<f64>,
        builder_order_id: Option<i64>,
        status: &str,
    ) -> Result<TradeFlowDualDcaLeg> {
        let row = sqlx::query(
            "INSERT INTO trade_flow_dual_dca_legs \
              (job_id, market_slug, token_id, outcome_label, side, level_index, trigger_condition, trigger_price, \
               size_usdc, reference_price, builder_order_id, status, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW(), NOW()) \
             ON CONFLICT (job_id, market_slug, outcome_label, level_index) DO UPDATE SET \
               token_id = EXCLUDED.token_id, \
               side = EXCLUDED.side, \
               trigger_condition = EXCLUDED.trigger_condition, \
               trigger_price = EXCLUDED.trigger_price, \
               size_usdc = EXCLUDED.size_usdc, \
               reference_price = EXCLUDED.reference_price, \
               builder_order_id = EXCLUDED.builder_order_id, \
               status = EXCLUDED.status, \
               updated_at = NOW() \
             RETURNING id, job_id, market_slug, token_id, outcome_label, side, level_index, trigger_condition, \
                       trigger_price, size_usdc, reference_price, builder_order_id, status, \
                       active_exchange_order_id, client_order_id, filled_price, filled_size, \
                       submitted_at, filled_at, created_at, updated_at",
        )
        .bind(job_id)
        .bind(market_slug)
        .bind(token_id)
        .bind(outcome_label)
        .bind(side)
        .bind(level_index)
        .bind(trigger_condition)
        .bind(trigger_price)
        .bind(size_usdc)
        .bind(reference_price)
        .bind(builder_order_id)
        .bind(status)
        .fetch_one(&self.pool)
        .await?;

        Ok(Self::row_to_dual_dca_leg(&row))
    }

    pub async fn get_trade_flow_dual_dca_leg(
        &self,
        job_id: i64,
        market_slug: &str,
        outcome_label: &str,
        level_index: i32,
    ) -> Result<Option<TradeFlowDualDcaLeg>> {
        let row = sqlx::query(
            "SELECT id, job_id, market_slug, token_id, outcome_label, side, level_index, \
             trigger_condition, trigger_price, size_usdc, reference_price, builder_order_id, \
             status, active_exchange_order_id, client_order_id, filled_price, filled_size, \
             submitted_at, filled_at, created_at, updated_at \
             FROM trade_flow_dual_dca_legs \
             WHERE job_id = $1 AND market_slug = $2 AND outcome_label = $3 AND level_index = $4 \
             LIMIT 1",
        )
        .bind(job_id)
        .bind(market_slug)
        .bind(outcome_label)
        .bind(level_index)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Self::row_to_dual_dca_leg(&r)))
    }

    /// Returns all legs for a given job + market, ordered by outcome then level.
    pub async fn list_dual_dca_legs_for_job(
        &self,
        job_id: i64,
        market_slug: &str,
    ) -> Result<Vec<TradeFlowDualDcaLeg>> {
        let rows = sqlx::query(
            "SELECT id, job_id, market_slug, token_id, outcome_label, side, level_index, \
             trigger_condition, trigger_price, size_usdc, reference_price, builder_order_id, \
             status, active_exchange_order_id, client_order_id, filled_price, filled_size, \
             submitted_at, filled_at, created_at, updated_at \
             FROM trade_flow_dual_dca_legs \
             WHERE job_id = $1 AND market_slug = $2 \
             ORDER BY outcome_label ASC, level_index ASC",
        )
        .bind(job_id)
        .bind(market_slug)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(Self::row_to_dual_dca_leg).collect())
    }

    /// Returns the lowest-level pending leg for a specific outcome.
    pub async fn next_pending_dual_dca_leg(
        &self,
        job_id: i64,
        market_slug: &str,
        outcome_label: &str,
    ) -> Result<Option<TradeFlowDualDcaLeg>> {
        let row = sqlx::query(
            "SELECT id, job_id, market_slug, token_id, outcome_label, side, level_index, \
             trigger_condition, trigger_price, size_usdc, reference_price, builder_order_id, \
             status, active_exchange_order_id, client_order_id, filled_price, filled_size, \
             submitted_at, filled_at, created_at, updated_at \
             FROM trade_flow_dual_dca_legs \
             WHERE job_id = $1 AND market_slug = $2 AND outcome_label = $3 AND status = 'pending' \
             ORDER BY level_index ASC \
             LIMIT 1",
        )
        .bind(job_id)
        .bind(market_slug)
        .bind(outcome_label)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| Self::row_to_dual_dca_leg(&r)))
    }

    /// Marks a leg as submitted with the CLOB exchange order ID.
    pub async fn set_dual_dca_leg_submitted(
        &self,
        leg_id: i64,
        exchange_order_id: &str,
        client_order_id: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_flow_dual_dca_legs \
             SET status = 'submitted', active_exchange_order_id = $2, client_order_id = $3, \
                 submitted_at = NOW(), updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(leg_id)
        .bind(exchange_order_id)
        .bind(client_order_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Marks a leg as filled with execution details.
    pub async fn set_dual_dca_leg_filled(
        &self,
        leg_id: i64,
        filled_price: f64,
        filled_size: f64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_flow_dual_dca_legs \
             SET status = 'filled', filled_price = $2, filled_size = $3, \
                 filled_at = NOW(), active_exchange_order_id = NULL, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(leg_id)
        .bind(filled_price)
        .bind(filled_size)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Cancels all active legs (pending/submitted/open) for a job+market.
    /// Returns (leg_id, exchange_order_id) for legs that had active CLOB orders.
    pub async fn cancel_dual_dca_active_legs(
        &self,
        job_id: i64,
        market_slug: Option<&str>,
    ) -> Result<Vec<(i64, Option<String>)>> {
        let rows = sqlx::query(
            "UPDATE trade_flow_dual_dca_legs \
             SET status = 'canceled', active_exchange_order_id = NULL, updated_at = NOW() \
             WHERE job_id = $1 \
               AND ($2::text IS NULL OR market_slug = $2) \
               AND status IN ('pending', 'submitted', 'open') \
             RETURNING id, active_exchange_order_id",
        )
        .bind(job_id)
        .bind(market_slug)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                use sqlx::Row;
                (
                    r.get::<i64, _>("id"),
                    r.get::<Option<String>, _>("active_exchange_order_id"),
                )
            })
            .collect())
    }

    /// Returns all legs with active CLOB orders (submitted or open).
    pub async fn list_dual_dca_legs_with_active_orders(
        &self,
        job_id: i64,
        market_slug: &str,
    ) -> Result<Vec<TradeFlowDualDcaLeg>> {
        let rows = sqlx::query(
            "SELECT id, job_id, market_slug, token_id, outcome_label, side, level_index, \
             trigger_condition, trigger_price, size_usdc, reference_price, builder_order_id, \
             status, active_exchange_order_id, client_order_id, filled_price, filled_size, \
             submitted_at, filled_at, created_at, updated_at \
             FROM trade_flow_dual_dca_legs \
             WHERE job_id = $1 AND market_slug = $2 \
               AND active_exchange_order_id IS NOT NULL \
               AND status IN ('submitted', 'open') \
             ORDER BY level_index ASC",
        )
        .bind(job_id)
        .bind(market_slug)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(Self::row_to_dual_dca_leg).collect())
    }

    /// Resets a leg back to pending (for retry after cancel/error).
    pub async fn reset_dual_dca_leg_to_pending(&self, leg_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE trade_flow_dual_dca_legs \
             SET status = 'pending', active_exchange_order_id = NULL, client_order_id = NULL, \
                 submitted_at = NULL, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(leg_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn append_trade_flow_dual_dca_event(
        &self,
        job_id: i64,
        leg_id: Option<i64>,
        event_type: &str,
        payload_json: &Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO trade_flow_dual_dca_events (job_id, leg_id, event_type, payload_json, created_at) \
             VALUES ($1, $2, $3, $4, NOW())",
        )
        .bind(job_id)
        .bind(leg_id)
        .bind(event_type)
        .bind(payload_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn upsert_auto_claim_job(
        &self,
        owner_address: &str,
        market_slug: Option<&str>,
        condition_id: &str,
        max_attempts: i32,
    ) -> Result<bool> {
        let inserted = sqlx::query_scalar::<_, bool>(
            "INSERT INTO auto_claim_jobs \
              (owner_address, market_slug, condition_id, status, attempts, max_attempts, next_attempt_at, tx_hash, last_error, claimed_at, last_seen_redeemable_at, created_at, updated_at) \
             VALUES \
              ($1, $2, $3, 'pending', 0, $4, NOW(), NULL, NULL, NULL, NOW(), NOW(), NOW()) \
             ON CONFLICT (owner_address, condition_id) DO UPDATE SET \
               market_slug = COALESCE(EXCLUDED.market_slug, auto_claim_jobs.market_slug), \
               max_attempts = GREATEST(auto_claim_jobs.max_attempts, EXCLUDED.max_attempts), \
               last_seen_redeemable_at = NOW(), \
               updated_at = NOW(), \
               status = CASE \
                 WHEN auto_claim_jobs.status IN ('claimed', 'processing') THEN auto_claim_jobs.status \
                 WHEN auto_claim_jobs.status = 'failed' AND auto_claim_jobs.attempts >= auto_claim_jobs.max_attempts THEN auto_claim_jobs.status \
                 ELSE 'pending' \
               END, \
               next_attempt_at = CASE \
                 WHEN auto_claim_jobs.status IN ('claimed', 'processing') THEN auto_claim_jobs.next_attempt_at \
                 WHEN auto_claim_jobs.status = 'failed' AND auto_claim_jobs.attempts >= auto_claim_jobs.max_attempts THEN auto_claim_jobs.next_attempt_at \
                 ELSE NOW() \
               END, \
               last_error = CASE \
                 WHEN auto_claim_jobs.status IN ('claimed', 'processing') THEN auto_claim_jobs.last_error \
                 WHEN auto_claim_jobs.status = 'failed' AND auto_claim_jobs.attempts >= auto_claim_jobs.max_attempts THEN auto_claim_jobs.last_error \
                 ELSE NULL \
               END \
             RETURNING (xmax = 0)",
        )
        .bind(owner_address)
        .bind(market_slug)
        .bind(condition_id)
        .bind(max_attempts)
        .fetch_one(&self.pool)
        .await?;

        Ok(inserted)
    }

    pub async fn list_auto_claim_jobs_for_processing(
        &self,
        limit: i64,
    ) -> Result<Vec<AutoClaimJob>> {
        let rows = sqlx::query(
            "SELECT id, owner_address, market_slug, condition_id, status, attempts, max_attempts, next_attempt_at, tx_hash, last_error, claimed_at, last_seen_redeemable_at, created_at, updated_at \
             FROM auto_claim_jobs \
             WHERE status IN ('pending', 'retry') AND next_attempt_at <= NOW() \
             ORDER BY next_attempt_at ASC, id ASC \
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| AutoClaimJob {
                id: row.get("id"),
                owner_address: row.get("owner_address"),
                market_slug: row.get("market_slug"),
                condition_id: row.get("condition_id"),
                status: row.get("status"),
                attempts: row.get("attempts"),
                max_attempts: row.get("max_attempts"),
                next_attempt_at: row.get("next_attempt_at"),
                tx_hash: row.get("tx_hash"),
                last_error: row.get("last_error"),
                claimed_at: row.get("claimed_at"),
                last_seen_redeemable_at: row.get("last_seen_redeemable_at"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn mark_auto_claim_job_processing(&self, job_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE auto_claim_jobs \
             SET status = 'processing', updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(job_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_auto_claim_job_claimed(&self, job_id: i64, tx_hash: &str) -> Result<()> {
        sqlx::query(
            "UPDATE auto_claim_jobs \
             SET status = 'claimed', tx_hash = $2, claimed_at = NOW(), last_error = NULL, updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(job_id)
        .bind(tx_hash)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_auto_claim_job_retry_or_fail(
        &self,
        job_id: i64,
        last_error: &str,
        retry_backoff_ms: i64,
    ) -> Result<String> {
        let status = sqlx::query_scalar::<_, String>(
            "UPDATE auto_claim_jobs \
             SET attempts = attempts + 1, \
                 status = CASE WHEN attempts + 1 >= max_attempts THEN 'failed' ELSE 'retry' END, \
                 next_attempt_at = CASE \
                   WHEN attempts + 1 >= max_attempts THEN NOW() \
                   ELSE NOW() + ($3 * INTERVAL '1 millisecond') \
                 END, \
                 last_error = $2, \
                 updated_at = NOW() \
             WHERE id = $1 \
             RETURNING status",
        )
        .bind(job_id)
        .bind(last_error)
        .bind(retry_backoff_ms)
        .fetch_one(&self.pool)
        .await?;
        Ok(status)
    }

    pub async fn append_auto_claim_event(
        &self,
        job_id: i64,
        event_type: &str,
        payload_json: &Value,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO auto_claim_events (job_id, event_type, payload_json, created_at) \
             VALUES ($1, $2, $3, NOW())",
        )
        .bind(job_id)
        .bind(event_type)
        .bind(payload_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_trade_builder_order_params(
        &self,
        builder_order_id: i64,
        min_price_distance_cent: Option<f64>,
        max_triggers: Option<i32>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE trade_builder_orders \
             SET min_price_distance_cent = COALESCE($2, min_price_distance_cent), \
                 max_triggers = COALESCE($3, max_triggers), \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(builder_order_id)
        .bind(min_price_distance_cent)
        .bind(max_triggers)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn parse_state(raw: &str) -> Option<TradeState> {
    match raw {
        "Idle" => Some(TradeState::Idle),
        "WaitingEntry" => Some(TradeState::WaitingEntry),
        "EntryPlaced" => Some(TradeState::EntryPlaced),
        "EntryPartiallyFilled" => Some(TradeState::EntryPartiallyFilled),
        "EntryFilled" => Some(TradeState::EntryFilled),
        "TpPlaced" => Some(TradeState::TpPlaced),
        "SlArmed" => Some(TradeState::SlArmed),
        "ExitPartiallyFilled" => Some(TradeState::ExitPartiallyFilled),
        "ExitFilled" => Some(TradeState::ExitFilled),
        "Settled" => Some(TradeState::Settled),
        "Halted" => Some(TradeState::Halted),
        _ => None,
    }
}

fn leg_side_to_db(leg_side: LegSide) -> &'static str {
    match leg_side {
        LegSide::Yes => "yes",
        LegSide::No => "no",
    }
}

fn db_to_leg_side(raw: &str) -> Option<LegSide> {
    match raw {
        "yes" | "YES" => Some(LegSide::Yes),
        "no" | "NO" => Some(LegSide::No),
        _ => None,
    }
}
