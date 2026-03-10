use super::*;

impl PostgresRepository {
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
        .fetch_one(self.pool())
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
        .fetch_optional(self.pool())
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

        let mut tx = self.pool().begin().await?;
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
        .fetch_one(self.pool())
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
        .fetch_one(self.pool())
        .await?;
        Ok(id)
    }

    pub async fn trade_state(&self, trade_id: i64) -> Result<TradeState> {
        let raw: String = sqlx::query_scalar("SELECT state FROM trades WHERE id = $1")
            .bind(trade_id)
            .fetch_one(self.pool())
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
            .execute(self.pool())
            .await?;
        self.record_risk_event(Some(trade_id), "state_transition", "allow", reason)
            .await?;
        Ok(())
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
        .execute(self.pool())
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
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn load_leg_positions(&self, trade_id: i64) -> Result<Vec<LegPositionSnapshot>> {
        let rows = sqlx::query_as::<_, (String, String, f64, f64, i32, Option<f64>)>(
            "SELECT leg_side, token_id, qty, avg_entry, levels_filled, last_fill_price \
             FROM leg_positions WHERE trade_id = $1",
        )
        .bind(trade_id)
        .fetch_all(self.pool())
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
            .execute(self.pool())
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
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn load_position_exit_rules(&self, trade_id: i64) -> Result<Vec<PositionExitRule>> {
        let rows = sqlx::query_as::<_, (String, f64, bool)>(
            "SELECT leg_side, drop_sell_pct, enabled FROM position_exit_rules WHERE trade_id = $1",
        )
        .bind(trade_id)
        .fetch_all(self.pool())
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
        .execute(self.pool())
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
        .fetch_optional(self.pool())
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
        .fetch_one(self.pool())
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
        .fetch_all(self.pool())
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
        .fetch_one(self.pool())
        .await?;
        Ok(value)
    }
}
