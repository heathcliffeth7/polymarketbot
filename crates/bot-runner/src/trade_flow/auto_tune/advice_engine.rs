const AUTO_TUNE_ADVICE_EVENT: &str = "auto_tune_advice_generated";

#[derive(Debug, Clone, Default)]
struct AutoTuneWindowMetrics {
    markets_seen: i64,
    eligible_markets: i64,
    order_created_count: i64,
    filled_count: i64,
    pair_locked_count: i64,
    orphan_count: i64,
    sl_count: i64,
    net_pnl_usdc: f64,
    max_price_blocks: i64,
    execution_floor_blocks: i64,
    quality_floor_misses: i64,
    pair_total_blocks: i64,
    counter_max_blocks: i64,
    data_problem_blocks: i64,
    locked_profit_samples: i64,
    locked_profit_sum: f64,
}

impl AutoTuneWindowMetrics {
    fn avg_locked_profit_per_share(&self) -> Option<f64> {
        (self.locked_profit_samples >= 2)
            .then_some(self.locked_profit_sum / self.locked_profit_samples as f64)
    }

    fn orphan_rate(&self) -> f64 {
        auto_tune_ratio(self.orphan_count, self.eligible_markets)
    }

    fn sl_rate(&self) -> f64 {
        auto_tune_ratio(self.sl_count, self.eligible_markets)
    }

    fn max_price_ratio(&self) -> f64 {
        auto_tune_ratio(self.max_price_blocks, self.eligible_markets)
    }

    fn execution_floor_ratio(&self) -> f64 {
        auto_tune_ratio(self.execution_floor_blocks, self.eligible_markets)
    }

    fn quality_floor_miss_ratio(&self) -> f64 {
        auto_tune_ratio(self.quality_floor_misses, self.eligible_markets)
    }

    fn pair_total_ratio(&self) -> f64 {
        auto_tune_ratio(self.pair_total_blocks, self.eligible_markets)
    }

    fn counter_max_ratio(&self) -> f64 {
        auto_tune_ratio(self.counter_max_blocks, self.eligible_markets)
    }

    fn data_problem_ratio(&self) -> f64 {
        auto_tune_ratio(self.data_problem_blocks, self.markets_seen)
    }
}

async fn maybe_emit_trade_flow_auto_tune_advice(
    repo: &PostgresRepository,
    run_spec: &WsOpenPositionPriceRunSpec,
    action_node: &TradeFlowNode,
    market_scope: &str,
    cfg: &AutoTuneConfig,
) -> Result<()> {
    let history_limit = cfg
        .sample_markets
        .saturating_add(cfg.cooldown_markets_after_advice)
        .saturating_add(cfg.dedupe_same_advice_for_markets)
        .saturating_add(4)
        .max(cfg.sample_markets);
    let summaries = repo
        .list_trade_flow_auto_tune_market_summaries(
            run_spec.definition_id,
            run_spec.version_id,
            &action_node.key,
            market_scope,
            history_limit as i64,
        )
        .await?;
    if summaries.is_empty() {
        return Ok(());
    }
    let cooldown_slugs = latest_market_slugs(&summaries, cfg.cooldown_markets_after_advice);
    if !cooldown_slugs.is_empty()
        && repo
            .has_recent_trade_flow_auto_tune_advice(
                run_spec.definition_id,
                run_spec.version_id,
                &action_node.key,
                market_scope,
                &cooldown_slugs,
                None,
                None,
                None,
            )
            .await?
    {
        return Ok(());
    }

    let window = summaries
        .iter()
        .take(cfg.sample_markets)
        .cloned()
        .collect::<Vec<_>>();
    let metrics = auto_tune_window_metrics(&window);
    let mut drafts = Vec::new();
    if let Some(safety) = safety_protective_unwind_advice(action_node, &metrics) {
        drafts.push(safety);
    }
    drafts.push(primary_auto_tune_advice(action_node, cfg, &window, &metrics));

    let dedupe_slugs = latest_market_slugs(&summaries, cfg.dedupe_same_advice_for_markets);
    for draft in drafts {
        if !dedupe_slugs.is_empty()
            && repo
                .has_recent_trade_flow_auto_tune_advice(
                    run_spec.definition_id,
                    run_spec.version_id,
                    &action_node.key,
                    market_scope,
                    &dedupe_slugs,
                    Some(&draft.advice_action),
                    draft.target_key_path.as_deref(),
                    draft.suggested_value_json.as_ref(),
                )
                .await?
        {
            continue;
        }
        let sample_start_market_slug = window.last().map(|row| row.market_slug.clone());
        let sample_end_market_slug = window.first().map(|row| row.market_slug.clone());
        let dedupe_key = format!(
            "{}:{}:{}:{}:{}:{}:{}",
            run_spec.definition_id,
            run_spec.version_id,
            action_node.key,
            market_scope,
            draft.advice_action,
            draft.target_key_path.as_deref().unwrap_or("none"),
            sample_end_market_slug.as_deref().unwrap_or("none")
        );
        let input = bot_infra::db::TradeFlowAutoTuneAdviceInput {
            definition_id: run_spec.definition_id,
            version_id: run_spec.version_id,
            node_key: action_node.key.clone(),
            market_scope: market_scope.to_string(),
            sample_start_market_slug,
            sample_end_market_slug,
            markets_seen: metrics.markets_seen,
            eligible_markets: metrics.eligible_markets,
            order_created_count: metrics.order_created_count,
            filled_count: metrics.filled_count,
            pair_locked_count: metrics.pair_locked_count,
            orphan_count: metrics.orphan_count,
            sl_count: metrics.sl_count,
            advice_kind: draft.advice_kind.clone(),
            advice_action: draft.advice_action.clone(),
            target_key_path: draft.target_key_path.clone(),
            current_value_json: draft.current_value_json.clone(),
            suggested_value_json: draft.suggested_value_json.clone(),
            clamped: draft.clamped,
            hard_cap_min_json: draft.hard_cap_min_json.clone(),
            hard_cap_max_json: draft.hard_cap_max_json.clone(),
            reason_code: draft.reason_code.clone(),
            reason_text: draft.reason_text.clone(),
            dominant_blocker: draft.dominant_blocker.clone(),
            metrics_json: draft.metrics_json.clone(),
            dedupe_key,
        };
        if repo.insert_trade_flow_auto_tune_advice(&input).await? {
            repo.append_trade_flow_event(
                Some(run_spec.run_id),
                run_spec.definition_id,
                Some(run_spec.version_id),
                AUTO_TUNE_ADVICE_EVENT,
                &json!({
                    "mode": "advice",
                    "runtime_override_applied": false,
                    "published_config_changed": false,
                    "node_key": action_node.key,
                    "market_scope": market_scope,
                    "advice_kind": input.advice_kind,
                    "advice_action": input.advice_action,
                    "target_key_path": input.target_key_path,
                    "current_value": input.current_value_json,
                    "suggested_value": input.suggested_value_json,
                    "reason_code": input.reason_code,
                    "reason_text": input.reason_text,
                    "dominant_blocker": input.dominant_blocker,
                    "metrics": input.metrics_json,
                }),
            )
            .await?;
        }
    }

    Ok(())
}

fn auto_tune_window_metrics(
    summaries: &[bot_infra::db::TradeFlowAutoTuneMarketSummaryRecord],
) -> AutoTuneWindowMetrics {
    let mut metrics = AutoTuneWindowMetrics::default();
    for summary in summaries {
        metrics.markets_seen += 1;
        let eligible = (summary.trigger_passed || summary.action_started) && !summary.data_problem_block;
        if eligible {
            metrics.eligible_markets += 1;
        }
        metrics.order_created_count += i64::from(summary.builder_order_created);
        metrics.filled_count += i64::from(summary.order_filled);
        metrics.pair_locked_count += i64::from(summary.pair_locked);
        metrics.orphan_count += i64::from(summary.orphan_detected);
        metrics.sl_count += i64::from(summary.sl_hit);
        metrics.net_pnl_usdc += summary.realized_pnl_usdc.unwrap_or(0.0);
        metrics.max_price_blocks += i64::from(eligible && summary.max_price_block);
        metrics.execution_floor_blocks += i64::from(eligible && summary.execution_floor_block);
        metrics.pair_total_blocks += i64::from(eligible && summary.pair_total_block);
        metrics.counter_max_blocks += i64::from(eligible && summary.counter_max_block);
        metrics.data_problem_blocks += i64::from(summary.data_problem_block);
        metrics.quality_floor_misses += i64::from(eligible && is_quality_floor_miss(summary));
        if summary.pair_locked {
            if let Some(value) = summary.locked_profit_per_share.filter(|value| value.is_finite()) {
                metrics.locked_profit_samples += 1;
                metrics.locked_profit_sum += value;
            }
        }
    }
    metrics
}

fn primary_auto_tune_advice(
    action_node: &TradeFlowNode,
    cfg: &AutoTuneConfig,
    summaries: &[bot_infra::db::TradeFlowAutoTuneMarketSummaryRecord],
    metrics: &AutoTuneWindowMetrics,
) -> AutoTuneAdviceDraft {
    let metrics_json = auto_tune_metrics_json(metrics);
    if metrics.data_problem_blocks > 0
        && metrics.data_problem_blocks
            >= [
                metrics.max_price_blocks,
                metrics.execution_floor_blocks,
                metrics.pair_total_blocks,
                metrics.counter_max_blocks,
            ]
            .into_iter()
            .max()
            .unwrap_or_default()
    {
        return hold_advice(
            "hold_data_quality",
            "data problem dominant; config relax skipped",
            Some("data_problem"),
            metrics_json,
        );
    }
    if metrics.orphan_rate() > 0.05
        || metrics.sl_rate() > 0.30
        || metrics.net_pnl_usdc < -2.0
        || metrics
            .avg_locked_profit_per_share()
            .map(|value| value < 0.03)
            .unwrap_or(false)
    {
        return tighten_after_loss_advice(action_node, cfg, metrics);
    }
    if metrics.markets_seen < cfg.sample_markets as i64
        || metrics.eligible_markets < cfg.min_eligible_markets as i64
    {
        return hold_advice(
            "hold_insufficient_sample",
            "sample window is not large enough for advice",
            None,
            metrics_json,
        );
    }
    if metrics.order_created_count <= 1
        && metrics.orphan_count == 0
        && metrics.sl_count == 0
        && metrics.net_pnl_usdc >= -2.0
    {
        if metrics.max_price_ratio() >= 0.40 {
            if let Some(advice) = relax_iv_time_rule_max_price_advice(action_node, cfg, summaries, metrics) {
                return advice;
            }
            return hold_advice(
                "hold_time_rule_target_unknown",
                "max price is dominant, but the effective IV time rule could not be resolved",
                Some("above_max_price"),
                metrics_json,
            );
        }
        if metrics.pair_total_ratio() >= 0.30 {
            return relax_pair_total_advice(action_node, cfg, metrics);
        }
        if metrics.counter_max_ratio() >= 0.30 {
            if let Some(advice) = relax_counter_max_advice(action_node, cfg, summaries, metrics) {
                return advice;
            }
            return hold_advice(
                "hold_counter_pair_total_not_proven",
                "counter max blocked, but pair total stayed safe was not proven",
                Some("counter_above_max_price"),
                metrics_json,
            );
        }
        if metrics.execution_floor_ratio() >= 0.30 && metrics.quality_floor_miss_ratio() >= 0.30 {
            return relax_execution_floor_advice(action_node, cfg, metrics);
        }
    }
    hold_advice(
        "hold_healthy_or_no_dominant_signal",
        "no safe auto-tune action found for the current advice-only window",
        None,
        metrics_json,
    )
}

fn safety_protective_unwind_advice(
    action_node: &TradeFlowNode,
    metrics: &AutoTuneWindowMetrics,
) -> Option<AutoTuneAdviceDraft> {
    if auto_tune_config_bool(action_node, "counterLegEnabled") != Some(true) {
        return None;
    }
    if auto_tune_config_bool(action_node, "pairProtectiveUnwindEnabled") != Some(false) {
        return None;
    }
    Some(AutoTuneAdviceDraft {
        advice_kind: "safety".to_string(),
        advice_action: "safety_enable_protective_unwind".to_string(),
        target_key_path: Some(format!("{}.pairProtectiveUnwindEnabled", action_node.key)),
        current_value_json: Some(json!(false)),
        suggested_value_json: Some(json!(true)),
        clamped: false,
        hard_cap_min_json: None,
        hard_cap_max_json: None,
        reason_code: "pair_lock_counter_enabled_unwind_disabled".to_string(),
        reason_text: "pair lock counter is enabled while protective unwind is disabled".to_string(),
        dominant_blocker: None,
        metrics_json: auto_tune_metrics_json(metrics),
    })
}

fn relax_pair_total_advice(
    action_node: &TradeFlowNode,
    cfg: &AutoTuneConfig,
    metrics: &AutoTuneWindowMetrics,
) -> AutoTuneAdviceDraft {
    let current = auto_tune_config_f64(action_node, "pairMaxTotalCent").unwrap_or(93.0);
    let suggested = (current + 1.0).min(cfg.caps.pair_max_total_max_cent);
    AutoTuneAdviceDraft {
        advice_kind: "relax".to_string(),
        advice_action: "relax_pair_total".to_string(),
        target_key_path: Some(format!("{}.pairMaxTotalCent", action_node.key)),
        current_value_json: Some(json!(current)),
        suggested_value_json: Some(json!(suggested)),
        clamped: suggested < current + 1.0,
        hard_cap_min_json: Some(json!(cfg.caps.pair_max_total_min_cent)),
        hard_cap_max_json: Some(json!(cfg.caps.pair_max_total_max_cent)),
        reason_code: "too_tight_pair_total".to_string(),
        reason_text: "pair total is the dominant first terminal blocker".to_string(),
        dominant_blocker: Some("pair_total_above_max".to_string()),
        metrics_json: auto_tune_metrics_json(metrics),
    }
}

fn relax_counter_max_advice(
    action_node: &TradeFlowNode,
    cfg: &AutoTuneConfig,
    summaries: &[bot_infra::db::TradeFlowAutoTuneMarketSummaryRecord],
    metrics: &AutoTuneWindowMetrics,
) -> Option<AutoTuneAdviceDraft> {
    let pair_cap = auto_tune_config_f64(action_node, "pairMaxTotalCent").unwrap_or(93.0) / 100.0;
    let safe_counter_blocks = summaries.iter().any(|summary| {
        summary.counter_max_block
            && summary
                .pair_total_effective
                .map(|value| value <= pair_cap)
                .unwrap_or(false)
    });
    if !safe_counter_blocks {
        return None;
    }
    let current = auto_tune_config_f64(action_node, "counterLegMaxPriceCent").unwrap_or(70.0);
    let suggested = (current + 5.0).min(cfg.caps.counter_leg_max_price_max_cent);
    Some(AutoTuneAdviceDraft {
        advice_kind: "relax".to_string(),
        advice_action: "relax_counter_max".to_string(),
        target_key_path: Some(format!("{}.counterLegMaxPriceCent", action_node.key)),
        current_value_json: Some(json!(current)),
        suggested_value_json: Some(json!(suggested)),
        clamped: suggested < current + 5.0,
        hard_cap_min_json: Some(json!(cfg.caps.counter_leg_max_price_min_cent)),
        hard_cap_max_json: Some(json!(cfg.caps.counter_leg_max_price_max_cent)),
        reason_code: "too_tight_counter_max".to_string(),
        reason_text: "counter leg max price blocked while pair total stayed inside cap".to_string(),
        dominant_blocker: Some("counter_above_max_price".to_string()),
        metrics_json: auto_tune_metrics_json(metrics),
    })
}

fn relax_execution_floor_advice(
    action_node: &TradeFlowNode,
    cfg: &AutoTuneConfig,
    metrics: &AutoTuneWindowMetrics,
) -> AutoTuneAdviceDraft {
    let current = auto_tune_config_f64(action_node, "executionFloorPriceCent").unwrap_or(50.0);
    let suggested = (current - 1.0).max(cfg.caps.execution_floor_min_cent);
    AutoTuneAdviceDraft {
        advice_kind: "relax".to_string(),
        advice_action: "relax_execution_floor".to_string(),
        target_key_path: Some(format!("{}.executionFloorPriceCent", action_node.key)),
        current_value_json: Some(json!(current)),
        suggested_value_json: Some(json!(suggested)),
        clamped: suggested > current - 1.0,
        hard_cap_min_json: Some(json!(cfg.caps.execution_floor_min_cent)),
        hard_cap_max_json: Some(json!(cfg.caps.execution_floor_max_cent)),
        reason_code: "quality_floor_miss".to_string(),
        reason_text: "execution floor blocked but strict quality recovery evidence was present".to_string(),
        dominant_blocker: Some("below_best_ask_floor".to_string()),
        metrics_json: auto_tune_metrics_json(metrics),
    }
}

fn relax_iv_time_rule_max_price_advice(
    action_node: &TradeFlowNode,
    cfg: &AutoTuneConfig,
    summaries: &[bot_infra::db::TradeFlowAutoTuneMarketSummaryRecord],
    metrics: &AutoTuneWindowMetrics,
) -> Option<AutoTuneAdviceDraft> {
    let remaining_sec = summaries
        .iter()
        .filter(|summary| summary.max_price_block)
        .find_map(|summary| {
            auto_tune_json_i64_path(
                &summary.metrics_json,
                &[&["remaining_sec_at_first_terminal_guard"]],
            )
        })?;
    let rules = action_node
        .config
        .get("priceToBeatIvTimeRules")
        .and_then(Value::as_array)?;
    for (index, rule) in rules.iter().enumerate() {
        let start = auto_tune_json_i64_path(rule, &[&["startRemainingSec"]]).unwrap_or(i64::MAX);
        let end = auto_tune_json_i64_path(rule, &[&["endRemainingSec"]]).unwrap_or(0);
        if remaining_sec > start || remaining_sec < end {
            continue;
        }
        let Some(current) = auto_tune_json_f64_path(rule, &[&["maxPriceCent"]]) else {
            continue;
        };
        let max_delta = cfg.caps.iv_rule_max_price_delta_max_cent.max(0.0);
        let cap_max = current + max_delta;
        let suggested = (current + 1.0).min(cap_max);
        return Some(AutoTuneAdviceDraft {
            advice_kind: "relax".to_string(),
            advice_action: "relax_iv_time_rule_max_price".to_string(),
            target_key_path: Some(format!(
                "{}.priceToBeatIvTimeRules[{index}].maxPriceCent",
                action_node.key
            )),
            current_value_json: Some(json!(current)),
            suggested_value_json: Some(json!(suggested)),
            clamped: suggested < current + 1.0,
            hard_cap_min_json: Some(json!(current + cfg.caps.iv_rule_max_price_delta_min_cent)),
            hard_cap_max_json: Some(json!(cap_max)),
            reason_code: "too_tight_max_price_time_rule".to_string(),
            reason_text: "above_max_price was dominant in this IV time rule bucket".to_string(),
            dominant_blocker: Some("above_max_price".to_string()),
            metrics_json: auto_tune_metrics_json(metrics),
        });
    }
    None
}

fn tighten_after_loss_advice(
    action_node: &TradeFlowNode,
    cfg: &AutoTuneConfig,
    metrics: &AutoTuneWindowMetrics,
) -> AutoTuneAdviceDraft {
    let current = auto_tune_config_f64(action_node, "pairMaxTotalCent").unwrap_or(93.0);
    let suggested = (current - 1.0).max(cfg.caps.pair_max_total_min_cent);
    AutoTuneAdviceDraft {
        advice_kind: "tighten".to_string(),
        advice_action: "tighten_after_loss_or_orphan".to_string(),
        target_key_path: Some(format!("{}.pairMaxTotalCent", action_node.key)),
        current_value_json: Some(json!(current)),
        suggested_value_json: Some(json!(suggested)),
        clamped: suggested > current - 1.0,
        hard_cap_min_json: Some(json!(cfg.caps.pair_max_total_min_cent)),
        hard_cap_max_json: Some(json!(cfg.caps.pair_max_total_max_cent)),
        reason_code: "loss_or_orphan_safety".to_string(),
        reason_text: "loss, SL, orphan, or weak locked profit signal requires tightening".to_string(),
        dominant_blocker: Some("loss_or_orphan".to_string()),
        metrics_json: auto_tune_metrics_json(metrics),
    }
}

fn hold_advice(
    reason_code: &str,
    reason_text: &str,
    dominant_blocker: Option<&str>,
    metrics_json: Value,
) -> AutoTuneAdviceDraft {
    AutoTuneAdviceDraft {
        advice_kind: "hold".to_string(),
        advice_action: "hold".to_string(),
        target_key_path: None,
        current_value_json: None,
        suggested_value_json: None,
        clamped: false,
        hard_cap_min_json: None,
        hard_cap_max_json: None,
        reason_code: reason_code.to_string(),
        reason_text: reason_text.to_string(),
        dominant_blocker: dominant_blocker.map(str::to_string),
        metrics_json,
    }
}

fn is_quality_floor_miss(summary: &bot_infra::db::TradeFlowAutoTuneMarketSummaryRecord) -> bool {
    if summary.first_terminal_guard_code.as_deref() != Some("below_best_ask_floor") {
        return false;
    }
    if summary.iv_edge_margin.map(|value| value > 0.02) != Some(true) {
        return false;
    }
    let binance_fresh = summary
        .binance_stale_ms
        .map(|value| value <= 3_000)
        .unwrap_or(!summary.data_problem_block);
    if !binance_fresh || summary.binance_same_direction != Some(true) {
        return false;
    }
    let depth_later_ok = auto_tune_json_i64_path(&summary.metrics_json, &[&["depth_ok_seconds_count"]])
        .map(|value| value > 0)
        .unwrap_or(false);
    if summary.depth_ok != Some(true) && !depth_later_ok {
        return false;
    }
    summary.floor_recovered_once
        && summary.tradable_seconds_count.unwrap_or_default() >= 2
        && summary
            .max_best_ask_after_block
            .map(|value| value >= 0.60)
            .unwrap_or(false)
}

fn latest_market_slugs(
    summaries: &[bot_infra::db::TradeFlowAutoTuneMarketSummaryRecord],
    limit: usize,
) -> Vec<String> {
    summaries
        .iter()
        .take(limit)
        .map(|summary| summary.market_slug.clone())
        .collect()
}

fn auto_tune_metrics_json(metrics: &AutoTuneWindowMetrics) -> Value {
    json!({
        "markets_seen": metrics.markets_seen,
        "eligible_markets": metrics.eligible_markets,
        "order_created": metrics.order_created_count,
        "filled": metrics.filled_count,
        "pair_locked": metrics.pair_locked_count,
        "orphan": metrics.orphan_count,
        "sl": metrics.sl_count,
        "net_pnl_usdc": metrics.net_pnl_usdc,
        "orphan_rate": metrics.orphan_rate(),
        "sl_rate": metrics.sl_rate(),
        "avg_locked_profit_per_share": metrics.avg_locked_profit_per_share(),
        "locked_profit_sample_count": metrics.locked_profit_samples,
        "max_price_block_ratio": metrics.max_price_ratio(),
        "execution_floor_block_ratio": metrics.execution_floor_ratio(),
        "quality_floor_miss_ratio": metrics.quality_floor_miss_ratio(),
        "pair_total_block_ratio": metrics.pair_total_ratio(),
        "counter_max_block_ratio": metrics.counter_max_ratio(),
        "data_problem_ratio": metrics.data_problem_ratio(),
    })
}

fn auto_tune_ratio(count: i64, denominator: i64) -> f64 {
    if denominator <= 0 {
        0.0
    } else {
        count as f64 / denominator as f64
    }
}
