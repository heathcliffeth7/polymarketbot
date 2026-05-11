use super::*;

fn row_to_auto_tune_market_summary(
    row: sqlx::postgres::PgRow,
) -> TradeFlowAutoTuneMarketSummaryRecord {
    TradeFlowAutoTuneMarketSummaryRecord {
        id: row.get("id"),
        definition_id: row.get("definition_id"),
        version_id: row.get("version_id"),
        flow_run_id: row.get("flow_run_id"),
        node_key: row.get("node_key"),
        market_scope: row.get("market_scope"),
        market_slug: row.get("market_slug"),
        window_start: row.get("window_start"),
        window_end: row.get("window_end"),
        completed_at: row.get("completed_at"),
        trigger_passed: row.get("trigger_passed"),
        action_started: row.get("action_started"),
        builder_order_created: row.get("builder_order_created"),
        order_submitted: row.get("order_submitted"),
        order_filled: row.get("order_filled"),
        first_terminal_guard_scope: row.get("first_terminal_guard_scope"),
        first_terminal_guard_code: row.get("first_terminal_guard_code"),
        first_terminal_guard_node: row.get("first_terminal_guard_node"),
        first_terminal_guard_at: row.get("first_terminal_guard_at"),
        last_guard_scope: row.get("last_guard_scope"),
        last_guard_code: row.get("last_guard_code"),
        max_price_block: row.get("max_price_block"),
        execution_floor_block: row.get("execution_floor_block"),
        ptb_block: row.get("ptb_block"),
        pair_total_block: row.get("pair_total_block"),
        counter_max_block: row.get("counter_max_block"),
        counter_floor_block: row.get("counter_floor_block"),
        risk_block: row.get("risk_block"),
        data_problem_block: row.get("data_problem_block"),
        best_ask_at_block: row.get("best_ask_at_block"),
        max_price_effective: row.get("max_price_effective"),
        execution_floor_effective: row.get("execution_floor_effective"),
        pair_total_effective: row.get("pair_total_effective"),
        counter_price_effective: row.get("counter_price_effective"),
        iv_edge_margin: row.get("iv_edge_margin"),
        binance_stale_ms: row.get("binance_stale_ms"),
        binance_same_direction: row.get("binance_same_direction"),
        depth_ok: row.get("depth_ok"),
        floor_recovered_once: row.get("floor_recovered_once"),
        max_best_ask_after_block: row.get("max_best_ask_after_block"),
        tradable_seconds_count: row.get("tradable_seconds_count"),
        pair_session_id: row.get("pair_session_id"),
        pair_locked: row.get("pair_locked"),
        locked_qty: row.get("locked_qty"),
        unpaired_qty: row.get("unpaired_qty"),
        locked_profit_per_share: row.get("locked_profit_per_share"),
        orphan_detected: row.get("orphan_detected"),
        protective_unwind_triggered: row.get("protective_unwind_triggered"),
        sl_hit: row.get("sl_hit"),
        tp_hit: row.get("tp_hit"),
        realized_pnl_usdc: row.get("realized_pnl_usdc"),
        metrics_json: row.get("metrics_json"),
    }
}

impl PostgresRepository {
    pub async fn upsert_trade_flow_auto_tune_market_summary(
        &self,
        input: &TradeFlowAutoTuneMarketSummaryInput,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO trade_flow_auto_tune_market_summaries
              (definition_id, version_id, flow_run_id, node_key, market_scope, market_slug,
               window_start, window_end, completed_at, trigger_passed, action_started,
               builder_order_created, order_submitted, order_filled, first_terminal_guard_scope,
               first_terminal_guard_code, first_terminal_guard_node, first_terminal_guard_at,
               last_guard_scope, last_guard_code, max_price_block, execution_floor_block,
               ptb_block, pair_total_block, counter_max_block, counter_floor_block, risk_block,
               data_problem_block, best_ask_at_block, max_price_effective, execution_floor_effective,
               pair_total_effective, counter_price_effective, iv_edge_margin, iv_dynamic_threshold,
               gap_strength, required_gap_strength, binance_stale_ms, binance_same_direction,
               depth_ok, floor_recovered_once, max_best_ask_after_block, tradable_seconds_count,
               depth_ok_seconds_count, pair_session_id, pair_locked, locked_qty, unpaired_qty,
               locked_profit_per_share, orphan_detected, protective_unwind_triggered, sl_hit,
               tp_hit, realized_pnl_usdc, metrics_json, created_at, updated_at)
             VALUES
              ($1, $2, $3, $4, $5, $6,
               $7, $8, $9, $10, $11,
               $12, $13, $14, $15,
               $16, $17, $18,
               $19, $20, $21, $22,
               $23, $24, $25, $26, $27,
               $28, $29, $30, $31,
               $32, $33, $34, $35,
               $36, $37, $38, $39,
               $40, $41, $42, $43,
               $44, $45, $46, $47, $48,
               $49, $50, $51, $52,
               $53, $54, $55, NOW(), NOW())
             ON CONFLICT (definition_id, version_id, node_key, market_slug) DO UPDATE SET
               flow_run_id = COALESCE(EXCLUDED.flow_run_id, trade_flow_auto_tune_market_summaries.flow_run_id),
               market_scope = EXCLUDED.market_scope,
               window_start = COALESCE(EXCLUDED.window_start, trade_flow_auto_tune_market_summaries.window_start),
               window_end = COALESCE(EXCLUDED.window_end, trade_flow_auto_tune_market_summaries.window_end),
               completed_at = GREATEST(trade_flow_auto_tune_market_summaries.completed_at, EXCLUDED.completed_at),
               trigger_passed = trade_flow_auto_tune_market_summaries.trigger_passed OR EXCLUDED.trigger_passed,
               action_started = trade_flow_auto_tune_market_summaries.action_started OR EXCLUDED.action_started,
               builder_order_created = trade_flow_auto_tune_market_summaries.builder_order_created OR EXCLUDED.builder_order_created,
               order_submitted = trade_flow_auto_tune_market_summaries.order_submitted OR EXCLUDED.order_submitted,
               order_filled = trade_flow_auto_tune_market_summaries.order_filled OR EXCLUDED.order_filled,
               first_terminal_guard_scope =
                 CASE
                   WHEN trade_flow_auto_tune_market_summaries.first_terminal_guard_at IS NULL THEN EXCLUDED.first_terminal_guard_scope
                   WHEN EXCLUDED.first_terminal_guard_at IS NULL THEN trade_flow_auto_tune_market_summaries.first_terminal_guard_scope
                   WHEN EXCLUDED.first_terminal_guard_at < trade_flow_auto_tune_market_summaries.first_terminal_guard_at THEN EXCLUDED.first_terminal_guard_scope
                   ELSE trade_flow_auto_tune_market_summaries.first_terminal_guard_scope
                 END,
               first_terminal_guard_code =
                 CASE
                   WHEN trade_flow_auto_tune_market_summaries.first_terminal_guard_at IS NULL THEN EXCLUDED.first_terminal_guard_code
                   WHEN EXCLUDED.first_terminal_guard_at IS NULL THEN trade_flow_auto_tune_market_summaries.first_terminal_guard_code
                   WHEN EXCLUDED.first_terminal_guard_at < trade_flow_auto_tune_market_summaries.first_terminal_guard_at THEN EXCLUDED.first_terminal_guard_code
                   ELSE trade_flow_auto_tune_market_summaries.first_terminal_guard_code
                 END,
               first_terminal_guard_node =
                 CASE
                   WHEN trade_flow_auto_tune_market_summaries.first_terminal_guard_at IS NULL THEN EXCLUDED.first_terminal_guard_node
                   WHEN EXCLUDED.first_terminal_guard_at IS NULL THEN trade_flow_auto_tune_market_summaries.first_terminal_guard_node
                   WHEN EXCLUDED.first_terminal_guard_at < trade_flow_auto_tune_market_summaries.first_terminal_guard_at THEN EXCLUDED.first_terminal_guard_node
                   ELSE trade_flow_auto_tune_market_summaries.first_terminal_guard_node
                 END,
               first_terminal_guard_at =
                 CASE
                   WHEN trade_flow_auto_tune_market_summaries.first_terminal_guard_at IS NULL THEN EXCLUDED.first_terminal_guard_at
                   WHEN EXCLUDED.first_terminal_guard_at IS NULL THEN trade_flow_auto_tune_market_summaries.first_terminal_guard_at
                   ELSE LEAST(trade_flow_auto_tune_market_summaries.first_terminal_guard_at, EXCLUDED.first_terminal_guard_at)
                 END,
               last_guard_scope = COALESCE(EXCLUDED.last_guard_scope, trade_flow_auto_tune_market_summaries.last_guard_scope),
               last_guard_code = COALESCE(EXCLUDED.last_guard_code, trade_flow_auto_tune_market_summaries.last_guard_code),
               max_price_block = trade_flow_auto_tune_market_summaries.max_price_block OR EXCLUDED.max_price_block,
               execution_floor_block = trade_flow_auto_tune_market_summaries.execution_floor_block OR EXCLUDED.execution_floor_block,
               ptb_block = trade_flow_auto_tune_market_summaries.ptb_block OR EXCLUDED.ptb_block,
               pair_total_block = trade_flow_auto_tune_market_summaries.pair_total_block OR EXCLUDED.pair_total_block,
               counter_max_block = trade_flow_auto_tune_market_summaries.counter_max_block OR EXCLUDED.counter_max_block,
               counter_floor_block = trade_flow_auto_tune_market_summaries.counter_floor_block OR EXCLUDED.counter_floor_block,
               risk_block = trade_flow_auto_tune_market_summaries.risk_block OR EXCLUDED.risk_block,
               data_problem_block = trade_flow_auto_tune_market_summaries.data_problem_block OR EXCLUDED.data_problem_block,
               best_ask_at_block = COALESCE(EXCLUDED.best_ask_at_block, trade_flow_auto_tune_market_summaries.best_ask_at_block),
               max_price_effective = COALESCE(EXCLUDED.max_price_effective, trade_flow_auto_tune_market_summaries.max_price_effective),
               execution_floor_effective = COALESCE(EXCLUDED.execution_floor_effective, trade_flow_auto_tune_market_summaries.execution_floor_effective),
               pair_total_effective = COALESCE(EXCLUDED.pair_total_effective, trade_flow_auto_tune_market_summaries.pair_total_effective),
               counter_price_effective = COALESCE(EXCLUDED.counter_price_effective, trade_flow_auto_tune_market_summaries.counter_price_effective),
               iv_edge_margin = COALESCE(EXCLUDED.iv_edge_margin, trade_flow_auto_tune_market_summaries.iv_edge_margin),
               iv_dynamic_threshold = COALESCE(EXCLUDED.iv_dynamic_threshold, trade_flow_auto_tune_market_summaries.iv_dynamic_threshold),
               gap_strength = COALESCE(EXCLUDED.gap_strength, trade_flow_auto_tune_market_summaries.gap_strength),
               required_gap_strength = COALESCE(EXCLUDED.required_gap_strength, trade_flow_auto_tune_market_summaries.required_gap_strength),
               binance_stale_ms = COALESCE(EXCLUDED.binance_stale_ms, trade_flow_auto_tune_market_summaries.binance_stale_ms),
               binance_same_direction = COALESCE(EXCLUDED.binance_same_direction, trade_flow_auto_tune_market_summaries.binance_same_direction),
               depth_ok = COALESCE(EXCLUDED.depth_ok, trade_flow_auto_tune_market_summaries.depth_ok),
               floor_recovered_once = trade_flow_auto_tune_market_summaries.floor_recovered_once OR EXCLUDED.floor_recovered_once,
               max_best_ask_after_block = GREATEST(
                 COALESCE(trade_flow_auto_tune_market_summaries.max_best_ask_after_block, EXCLUDED.max_best_ask_after_block),
                 COALESCE(EXCLUDED.max_best_ask_after_block, trade_flow_auto_tune_market_summaries.max_best_ask_after_block)
               ),
               tradable_seconds_count = GREATEST(
                 COALESCE(trade_flow_auto_tune_market_summaries.tradable_seconds_count, EXCLUDED.tradable_seconds_count),
                 COALESCE(EXCLUDED.tradable_seconds_count, trade_flow_auto_tune_market_summaries.tradable_seconds_count)
               ),
               depth_ok_seconds_count = GREATEST(
                 COALESCE(trade_flow_auto_tune_market_summaries.depth_ok_seconds_count, EXCLUDED.depth_ok_seconds_count),
                 COALESCE(EXCLUDED.depth_ok_seconds_count, trade_flow_auto_tune_market_summaries.depth_ok_seconds_count)
               ),
               pair_session_id = COALESCE(EXCLUDED.pair_session_id, trade_flow_auto_tune_market_summaries.pair_session_id),
               pair_locked = trade_flow_auto_tune_market_summaries.pair_locked OR EXCLUDED.pair_locked,
               locked_qty = COALESCE(EXCLUDED.locked_qty, trade_flow_auto_tune_market_summaries.locked_qty),
               unpaired_qty = COALESCE(EXCLUDED.unpaired_qty, trade_flow_auto_tune_market_summaries.unpaired_qty),
               locked_profit_per_share = COALESCE(EXCLUDED.locked_profit_per_share, trade_flow_auto_tune_market_summaries.locked_profit_per_share),
               orphan_detected = trade_flow_auto_tune_market_summaries.orphan_detected OR EXCLUDED.orphan_detected,
               protective_unwind_triggered = trade_flow_auto_tune_market_summaries.protective_unwind_triggered OR EXCLUDED.protective_unwind_triggered,
               sl_hit = trade_flow_auto_tune_market_summaries.sl_hit OR EXCLUDED.sl_hit,
               tp_hit = trade_flow_auto_tune_market_summaries.tp_hit OR EXCLUDED.tp_hit,
               realized_pnl_usdc = COALESCE(EXCLUDED.realized_pnl_usdc, trade_flow_auto_tune_market_summaries.realized_pnl_usdc),
               metrics_json = trade_flow_auto_tune_market_summaries.metrics_json || EXCLUDED.metrics_json,
               updated_at = NOW()",
        )
        .bind(input.definition_id)
        .bind(input.version_id)
        .bind(input.flow_run_id)
        .bind(&input.node_key)
        .bind(&input.market_scope)
        .bind(&input.market_slug)
        .bind(input.window_start)
        .bind(input.window_end)
        .bind(input.completed_at)
        .bind(input.trigger_passed)
        .bind(input.action_started)
        .bind(input.builder_order_created)
        .bind(input.order_submitted)
        .bind(input.order_filled)
        .bind(&input.first_terminal_guard_scope)
        .bind(&input.first_terminal_guard_code)
        .bind(&input.first_terminal_guard_node)
        .bind(input.first_terminal_guard_at)
        .bind(&input.last_guard_scope)
        .bind(&input.last_guard_code)
        .bind(input.max_price_block)
        .bind(input.execution_floor_block)
        .bind(input.ptb_block)
        .bind(input.pair_total_block)
        .bind(input.counter_max_block)
        .bind(input.counter_floor_block)
        .bind(input.risk_block)
        .bind(input.data_problem_block)
        .bind(input.best_ask_at_block)
        .bind(input.max_price_effective)
        .bind(input.execution_floor_effective)
        .bind(input.pair_total_effective)
        .bind(input.counter_price_effective)
        .bind(input.iv_edge_margin)
        .bind(input.iv_dynamic_threshold)
        .bind(input.gap_strength)
        .bind(input.required_gap_strength)
        .bind(input.binance_stale_ms)
        .bind(input.binance_same_direction)
        .bind(input.depth_ok)
        .bind(input.floor_recovered_once)
        .bind(input.max_best_ask_after_block)
        .bind(input.tradable_seconds_count)
        .bind(input.depth_ok_seconds_count)
        .bind(input.pair_session_id)
        .bind(input.pair_locked)
        .bind(input.locked_qty)
        .bind(input.unpaired_qty)
        .bind(input.locked_profit_per_share)
        .bind(input.orphan_detected)
        .bind(input.protective_unwind_triggered)
        .bind(input.sl_hit)
        .bind(input.tp_hit)
        .bind(input.realized_pnl_usdc)
        .bind(&input.metrics_json)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn list_trade_flow_auto_tune_market_summaries(
        &self,
        definition_id: i64,
        version_id: i64,
        node_key: &str,
        market_scope: &str,
        limit: i64,
    ) -> Result<Vec<TradeFlowAutoTuneMarketSummaryRecord>> {
        let rows = sqlx::query(
            "SELECT id, definition_id, version_id, flow_run_id, node_key, market_scope,
                    market_slug, window_start, window_end, completed_at, trigger_passed,
                    action_started, builder_order_created, order_submitted, order_filled,
                    first_terminal_guard_scope, first_terminal_guard_code,
                    first_terminal_guard_node, first_terminal_guard_at, last_guard_scope,
                    last_guard_code, max_price_block, execution_floor_block, ptb_block,
                    pair_total_block, counter_max_block, counter_floor_block, risk_block,
                    data_problem_block, best_ask_at_block, max_price_effective,
                    execution_floor_effective, pair_total_effective, counter_price_effective,
                    iv_edge_margin, binance_stale_ms, binance_same_direction, depth_ok,
                    floor_recovered_once, max_best_ask_after_block, tradable_seconds_count,
                    pair_session_id, pair_locked, locked_qty, unpaired_qty,
                    locked_profit_per_share, orphan_detected, protective_unwind_triggered,
                    sl_hit, tp_hit, realized_pnl_usdc, metrics_json
             FROM trade_flow_auto_tune_market_summaries
             WHERE definition_id = $1
               AND version_id = $2
               AND node_key = $3
               AND market_scope = $4
             ORDER BY completed_at DESC, id DESC
             LIMIT $5",
        )
        .bind(definition_id)
        .bind(version_id)
        .bind(node_key)
        .bind(market_scope)
        .bind(limit)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(row_to_auto_tune_market_summary)
            .collect())
    }

    pub async fn has_recent_trade_flow_auto_tune_advice(
        &self,
        definition_id: i64,
        version_id: i64,
        node_key: &str,
        market_scope: &str,
        sample_end_market_slugs: &[String],
        advice_action: Option<&str>,
        target_key_path: Option<&str>,
        suggested_value_json: Option<&Value>,
    ) -> Result<bool> {
        if sample_end_market_slugs.is_empty() {
            return Ok(false);
        }
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
               SELECT 1
               FROM trade_flow_auto_tune_advice
               WHERE definition_id = $1
                 AND version_id = $2
                 AND node_key = $3
                 AND market_scope = $4
                 AND sample_end_market_slug = ANY($5::text[])
                 AND ($6::text IS NULL OR advice_action = $6)
                 AND ($7::text IS NULL OR target_key_path IS NOT DISTINCT FROM $7::text)
                 AND ($8::jsonb IS NULL OR suggested_value_json IS NOT DISTINCT FROM $8::jsonb)
             )",
        )
        .bind(definition_id)
        .bind(version_id)
        .bind(node_key)
        .bind(market_scope)
        .bind(sample_end_market_slugs)
        .bind(advice_action)
        .bind(target_key_path)
        .bind(suggested_value_json)
        .fetch_one(self.pool())
        .await?;
        Ok(exists)
    }

    pub async fn insert_trade_flow_auto_tune_advice(
        &self,
        input: &TradeFlowAutoTuneAdviceInput,
    ) -> Result<bool> {
        let row = sqlx::query_scalar::<_, i64>(
            "INSERT INTO trade_flow_auto_tune_advice
              (definition_id, version_id, node_key, market_scope, sample_start_market_slug,
               sample_end_market_slug, markets_seen, eligible_markets, order_created_count,
               filled_count, pair_locked_count, orphan_count, sl_count, advice_kind,
               advice_action, target_key_path, current_value_json, suggested_value_json,
               clamped, hard_cap_min_json, hard_cap_max_json, reason_code, reason_text,
               dominant_blocker, metrics_json, dedupe_key, created_at)
             VALUES
              ($1, $2, $3, $4, $5,
               $6, $7, $8, $9,
               $10, $11, $12, $13, $14,
               $15, $16, $17, $18,
               $19, $20, $21, $22, $23,
               $24, $25, $26, NOW())
             ON CONFLICT (dedupe_key) DO NOTHING
             RETURNING id",
        )
        .bind(input.definition_id)
        .bind(input.version_id)
        .bind(&input.node_key)
        .bind(&input.market_scope)
        .bind(&input.sample_start_market_slug)
        .bind(&input.sample_end_market_slug)
        .bind(input.markets_seen)
        .bind(input.eligible_markets)
        .bind(input.order_created_count)
        .bind(input.filled_count)
        .bind(input.pair_locked_count)
        .bind(input.orphan_count)
        .bind(input.sl_count)
        .bind(&input.advice_kind)
        .bind(&input.advice_action)
        .bind(&input.target_key_path)
        .bind(&input.current_value_json)
        .bind(&input.suggested_value_json)
        .bind(input.clamped)
        .bind(&input.hard_cap_min_json)
        .bind(&input.hard_cap_max_json)
        .bind(&input.reason_code)
        .bind(&input.reason_text)
        .bind(&input.dominant_blocker)
        .bind(&input.metrics_json)
        .bind(&input.dedupe_key)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.is_some())
    }

    pub async fn load_trade_flow_auto_tune_order_rollup(
        &self,
        run_id: i64,
        action_node_key: &str,
        market_slug: &str,
    ) -> Result<TradeFlowAutoTuneOrderRollup> {
        let row = sqlx::query(
            "WITH orders AS (
               SELECT *
               FROM trade_builder_orders
               WHERE origin_flow_run_id = $1
                 AND origin_flow_node_key = $2
                 AND market_slug = $3
             ),
             sessions AS (
               SELECT *
               FROM trade_builder_pair_sessions
               WHERE flow_run_id = $1
                 AND flow_node_key = $2
                 AND market_slug = $3
               ORDER BY updated_at DESC, id DESC
               LIMIT 1
             ),
             pnl AS (
               SELECT SUM(total_pnl_usdc)::double precision AS realized_pnl_usdc
               FROM trade_flow_auto_scope_trade_diagnostics
               WHERE run_id = $1
                 AND market_slug = $3
             )
             SELECT
               COALESCE((SELECT COUNT(*) > 0 FROM orders), false) AS builder_order_created,
               COALESCE((SELECT BOOL_OR(active_exchange_order_id IS NOT NULL
                 OR status = ANY(ARRAY['open','partially_filled','filled','completed','canceled','expired']))
                 FROM orders), false) AS order_submitted,
               COALESCE((SELECT BOOL_OR(COALESCE(filled_qty, 0) > 0
                 OR status = ANY(ARRAY['filled','completed']))
                 FROM orders), false) AS order_filled,
               (SELECT id FROM sessions) AS pair_session_id,
               COALESCE((SELECT status = ANY(ARRAY['locked','completed']) OR COALESCE(locked_qty, 0) > 0
                 FROM sessions), false) AS pair_locked,
               (SELECT locked_qty FROM sessions) AS locked_qty,
               (SELECT ABS(COALESCE(primary_net_qty, 0) - COALESCE(counter_net_qty, 0))
                 FROM sessions) AS unpaired_qty,
               (SELECT projected_net_profit_usdc / NULLIF(locked_qty, 0)
                 FROM sessions) AS locked_profit_per_share,
               COALESCE((SELECT status = ANY(ARRAY['unwinding','expired','error'])
                 AND ABS(COALESCE(primary_net_qty, 0) - COALESCE(counter_net_qty, 0)) > 0.000001
                 FROM sessions), false) AS orphan_detected,
               COALESCE((SELECT COUNT(*) > 0
                 FROM orders
                 WHERE pair_leg_role = 'orphan_unwind_sell'), false) AS protective_unwind_triggered,
               COALESCE((SELECT BOOL_OR(trigger_latched_reason = 'stop_loss'
                 OR exit_ladder_kind = 'sl') FROM orders), false) AS sl_hit,
               COALESCE((SELECT BOOL_OR(exit_ladder_kind = 'tp' AND COALESCE(filled_qty, 0) > 0)
                 FROM orders), false) AS tp_hit,
               (SELECT realized_pnl_usdc FROM pnl) AS realized_pnl_usdc",
        )
        .bind(run_id)
        .bind(action_node_key)
        .bind(market_slug)
        .fetch_one(self.pool())
        .await?;

        Ok(TradeFlowAutoTuneOrderRollup {
            builder_order_created: row.get("builder_order_created"),
            order_submitted: row.get("order_submitted"),
            order_filled: row.get("order_filled"),
            pair_session_id: row.get("pair_session_id"),
            pair_locked: row.get("pair_locked"),
            locked_qty: row.get("locked_qty"),
            unpaired_qty: row.get("unpaired_qty"),
            locked_profit_per_share: row.get("locked_profit_per_share"),
            orphan_detected: row.get("orphan_detected"),
            protective_unwind_triggered: row.get("protective_unwind_triggered"),
            sl_hit: row.get("sl_hit"),
            tp_hit: row.get("tp_hit"),
            realized_pnl_usdc: row.get("realized_pnl_usdc"),
        })
    }
}
