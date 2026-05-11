use super::*;

impl PostgresRepository {
    fn should_broadcast_trade_flow_event(event_type: &str) -> bool {
        matches!(
            event_type,
            "trigger_ws_price_enqueued"
                | "trigger_once_fired"
                | "trigger_once_blocked"
                | "telegram_notify"
                | "step_completed"
        )
    }

    pub async fn notify_trade_flow_realtime(&self, payload_json: &Value) -> Result<()> {
        let payload = serde_json::to_string(payload_json)?;
        sqlx::query("SELECT pg_notify($1, $2)")
            .bind("trade_flow_realtime")
            .bind(payload)
            .execute(self.pool())
            .await?;
        Ok(())
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
        .fetch_all(self.pool())
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
        .fetch_one(self.pool())
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
        .fetch_optional(self.pool())
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
        .fetch_optional(self.pool())
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
        .execute(self.pool())
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
        .fetch_optional(self.pool())
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
        .fetch_one(self.pool())
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
        .fetch_optional(self.pool())
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
        .fetch_all(self.pool())
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
            .execute(self.pool())
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
        .execute(self.pool())
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
        let created_at = Utc::now();
        let event_id: i64 = sqlx::query_scalar(
            "INSERT INTO trade_flow_events \
              (run_id, definition_id, version_id, event_type, payload_json, created_at) \
             VALUES \
              ($1, $2, $3, $4, $5, $6) \
             RETURNING id",
        )
        .bind(run_id)
        .bind(definition_id)
        .bind(version_id)
        .bind(event_type)
        .bind(payload_json)
        .bind(created_at)
        .fetch_one(self.pool())
        .await?;

        if Self::should_broadcast_trade_flow_event(event_type) {
            let realtime_payload = serde_json::json!({
                "id": event_id,
                "kind": "flow_event",
                "run_id": run_id,
                "definition_id": definition_id,
                "version_id": version_id,
                "event_type": event_type,
                "payload_json": payload_json,
                "created_at": created_at.to_rfc3339(),
            });
            let _ = self.notify_trade_flow_realtime(&realtime_payload).await;
        }
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
        .fetch_optional(self.pool())
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
        .fetch_all(self.pool())
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
        .fetch_all(self.pool())
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
        .execute(self.pool())
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
        .execute(self.pool())
        .await?;
        self.sync_trade_flow_node_runtime_snapshot_for_step(step_id)
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
        .execute(self.pool())
        .await?;
        self.sync_trade_flow_node_runtime_snapshot_for_step(step_id)
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
        .execute(self.pool())
        .await?;
        self.sync_trade_flow_node_runtime_snapshot_for_step(step_id)
            .await?;
        Ok(())
    }

    pub async fn find_latest_completed_place_order_output(
        &self,
        run_id: i64,
    ) -> Result<Option<Value>> {
        let row = sqlx::query(
            "SELECT output_json FROM trade_flow_run_steps \
             WHERE run_id = $1 AND node_type = 'action.place_order' AND status = 'completed' \
             ORDER BY ended_at DESC NULLS LAST LIMIT 1",
        )
        .bind(run_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.and_then(|r| r.try_get::<Option<Value>, _>("output_json").ok().flatten()))
    }

    pub async fn find_latest_completed_place_order_output_for_node(
        &self,
        run_id: i64,
        node_key: &str,
    ) -> Result<Option<Value>> {
        let row = sqlx::query(
            "SELECT output_json FROM trade_flow_run_steps \
             WHERE run_id = $1 \
               AND node_key = $2 \
               AND node_type = 'action.place_order' \
               AND status = 'completed' \
               AND output_json IS NOT NULL \
               AND ((output_json ? 'builder_order_id') OR (output_json ? 'builderOrderId')) \
             ORDER BY ended_at DESC NULLS LAST \
             LIMIT 1",
        )
        .bind(run_id)
        .bind(node_key)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.and_then(|r| r.try_get::<Option<Value>, _>("output_json").ok().flatten()))
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
        .fetch_one(self.pool())
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
        .fetch_all(self.pool())
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
        .execute(self.pool())
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
        .execute(self.pool())
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
        .execute(self.pool())
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
        .fetch_one(self.pool())
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
        .fetch_optional(self.pool())
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
        .fetch_all(self.pool())
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
        .fetch_optional(self.pool())
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
        .execute(self.pool())
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
        .execute(self.pool())
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
        .fetch_all(self.pool())
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
        .fetch_all(self.pool())
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
        .execute(self.pool())
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
        .execute(self.pool())
        .await?;
        Ok(())
    }
}
