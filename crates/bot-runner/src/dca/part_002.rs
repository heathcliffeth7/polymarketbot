async fn cancel_dual_dca_active_orders(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    job: &TradeFlowDualDcaJob,
    market_slug: Option<&str>,
    reason: &str,
) -> Result<usize> {
    let legs = repo
        .cancel_dual_dca_active_legs(job.id, market_slug)
        .await?;
    let mut canceled_count = 0usize;

    for (leg_id, exchange_oid) in &legs {
        if let Some(oid) = exchange_oid {
            if let Err(err) = client.cancel(oid).await {
                warn!(
                    dual_dca_job_id = job.id,
                    leg_id,
                    exchange_order_id = %oid,
                    error = %err,
                    "DUAL_DCA_CANCEL_CLOB_ORDER_FAILED"
                );
            }
        }
        canceled_count += 1;
    }

    if canceled_count > 0 {
        repo.append_trade_flow_dual_dca_event(
            job.id,
            None,
            "legs_canceled",
            &json!({
                "reason": reason,
                "market_slug": market_slug,
                "canceled_count": canceled_count
            }),
        )
        .await?;
    }

    Ok(canceled_count)
}

// ---------------------------------------------------------------------------
// Helper functions (moved from main.rs, unchanged)
// ---------------------------------------------------------------------------

fn resolve_dual_dca_outcomes<'a>(
    side_mode: &str,
    yes_token_id: &'a str,
    yes_price: f64,
    no_token_id: &'a str,
    no_price: f64,
) -> Vec<(&'static str, String, f64)> {
    let mut outcomes: Vec<(&'static str, String, f64)> = Vec::new();
    if matches!(side_mode, "up" | "all") {
        outcomes.push(("Yes", yes_token_id.to_string(), yes_price));
    }
    if matches!(side_mode, "down" | "all") {
        outcomes.push(("No", no_token_id.to_string(), no_price));
    }
    outcomes
}

async fn resolve_dual_dca_outcome_prices(
    ws: &ClobWsClient,
    client: &dyn OrderExecutor,
    market_slug: &str,
    yes_token_id: &str,
    no_token_id: &str,
) -> Result<(f64, f64)> {
    let midpoint_price = match client.midpoint(yes_token_id).await {
        Ok(snapshot) => clamp_probability(snapshot.price),
        Err(err) => {
            let fallback = 0.5;
            warn!(
                market = market_slug,
                error = %err,
                fallback_yes = fallback,
                "TRADE_FLOW_DUAL_DCA_MIDPOINT_FAILED_USING_FALLBACK"
            );
            fallback
        }
    };
    let fallback_yes = midpoint_price;
    let fallback_no = clamp_probability(1.0 - midpoint_price);

    let yes_ws_price = fetch_price_from_market_ws(ws, yes_token_id)
        .await
        .map(clamp_probability);
    let no_ws_price = fetch_price_from_market_ws(ws, no_token_id)
        .await
        .map(clamp_probability);

    let yes_price = yes_ws_price
        .or_else(|| no_ws_price.map(|v| clamp_probability(1.0 - v)))
        .unwrap_or(fallback_yes);
    let no_price = no_ws_price
        .or_else(|| yes_ws_price.map(|v| clamp_probability(1.0 - v)))
        .unwrap_or(fallback_no);

    Ok((clamp_probability(yes_price), clamp_probability(no_price)))
}

async fn evaluate_dual_dca_unrealized_pnl_usdc(
    repo: &PostgresRepository,
    source_trade_id: i64,
    yes_token_id: &str,
    no_token_id: &str,
    yes_price: f64,
    no_price: f64,
) -> Result<Option<(f64, Value)>> {
    let token_ids = vec![yes_token_id.to_string(), no_token_id.to_string()];
    let aggregates = repo
        .aggregate_trade_fill_by_token(source_trade_id, &token_ids)
        .await?;
    if aggregates.is_empty() {
        return Ok(None);
    }

    let mut by_token: HashMap<String, (f64, f64, f64, f64)> = HashMap::new();
    for item in aggregates {
        by_token.insert(
            item.token_id,
            (
                item.buy_qty,
                item.buy_notional_usdc,
                item.sell_qty,
                item.sell_notional_usdc,
            ),
        );
    }

    let mut has_open_position = false;
    let mut total_unrealized_pnl_usdc = 0.0f64;
    let mut breakdown_rows = Vec::new();

    for (outcome_label, token_id, current_price) in [
        ("Yes", yes_token_id, yes_price),
        ("No", no_token_id, no_price),
    ] {
        let (buy_qty, buy_notional_usdc, sell_qty, sell_notional_usdc) =
            by_token.remove(token_id).unwrap_or((0.0, 0.0, 0.0, 0.0));
        let net_qty = (buy_qty - sell_qty).max(0.0);
        let net_cost_usdc = (buy_notional_usdc - sell_notional_usdc).max(0.0);
        let mark_value_usdc = net_qty * current_price;
        let unrealized_pnl_usdc = if net_qty > 0.0 {
            mark_value_usdc - net_cost_usdc
        } else {
            0.0
        };

        if net_qty > 0.0000001 {
            has_open_position = true;
            total_unrealized_pnl_usdc += unrealized_pnl_usdc;
        }

        breakdown_rows.push(json!({
            "outcome_label": outcome_label,
            "token_id": token_id,
            "buy_qty": buy_qty,
            "buy_notional_usdc": buy_notional_usdc,
            "sell_qty": sell_qty,
            "sell_notional_usdc": sell_notional_usdc,
            "net_qty": net_qty,
            "net_cost_usdc": net_cost_usdc,
            "current_price": current_price,
            "mark_value_usdc": mark_value_usdc,
            "unrealized_pnl_usdc": unrealized_pnl_usdc
        }));
    }

    if !has_open_position {
        return Ok(None);
    }

    Ok(Some((
        total_unrealized_pnl_usdc,
        Value::Array(breakdown_rows),
    )))
}

fn dual_dca_level_step_distance(near_step: f64, step_mult: f64, level_index: i32) -> f64 {
    if level_index <= 0 {
        return 0.0;
    }
    let n = level_index as f64;
    if (step_mult - 1.0).abs() < 1e-9 {
        return near_step * n;
    }
    near_step * (step_mult.powf(n) - 1.0) / (step_mult - 1.0)
}

fn dual_dca_active_next_check(
    now: DateTime<Utc>,
    market_ends_at: Option<DateTime<Utc>>,
) -> DateTime<Utc> {
    let heartbeat = now + ChronoDuration::seconds(FLOW_DUAL_DCA_ACTIVE_CHECK_SECONDS);
    let candidate = market_ends_at
        .map(|ends_at| std::cmp::min(heartbeat, ends_at + ChronoDuration::seconds(3)))
        .unwrap_or(heartbeat);
    std::cmp::max(candidate, now + ChronoDuration::seconds(3))
}

fn dual_dca_rollover_next_check(
    now: DateTime<Utc>,
    market_ends_at: Option<DateTime<Utc>>,
    timeframe: &str,
) -> DateTime<Utc> {
    let base = market_ends_at
        .map(|ends_at| ends_at + ChronoDuration::seconds(3))
        .unwrap_or(now + dual_dca_timeframe_duration(timeframe));
    std::cmp::max(base, now + ChronoDuration::seconds(5))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cumulative_step_distance_grows_as_expected() {
        let near_step = 0.1;
        let step_mult = 1.1;
        let l1 = dual_dca_level_step_distance(near_step, step_mult, 1);
        let l2 = dual_dca_level_step_distance(near_step, step_mult, 2);
        assert!((l1 - 0.1).abs() < 1e-9);
        assert!((l2 - 0.21).abs() < 1e-9);
    }

    #[test]
    fn cumulative_step_distance_handles_unit_multiplier() {
        let near_step = 0.1;
        let step_mult = 1.0;
        let l2 = dual_dca_level_step_distance(near_step, step_mult, 2);
        let l3 = dual_dca_level_step_distance(near_step, step_mult, 3);
        assert!((l2 - 0.2).abs() < 1e-9);
        assert!((l3 - 0.3).abs() < 1e-9);
    }

    #[test]
    fn active_next_check_is_clamped_to_market_end_buffer() {
        let now = DateTime::parse_from_rfc3339("2026-02-22T12:00:00Z")
            .expect("valid datetime")
            .with_timezone(&Utc);
        let market_ends_at = now + ChronoDuration::seconds(10);
        let next_check = dual_dca_active_next_check(now, Some(market_ends_at));
        assert_eq!(next_check, market_ends_at + ChronoDuration::seconds(3));
    }

    #[test]
    fn resolves_up_mode_to_yes_only() {
        let outcomes = resolve_dual_dca_outcomes("up", "yes-token", 0.41, "no-token", 0.59);
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].0, "Yes");
    }

    #[test]
    fn resolves_down_mode_to_no_only() {
        let outcomes = resolve_dual_dca_outcomes("down", "yes-token", 0.41, "no-token", 0.59);
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].0, "No");
    }

    #[test]
    fn resolves_all_mode_to_both_outcomes() {
        let outcomes = resolve_dual_dca_outcomes("all", "yes-token", 0.41, "no-token", 0.59);
        assert_eq!(outcomes.len(), 2);
    }

    #[test]
    fn cross_below_fires_at_or_below_threshold() {
        assert!(dual_dca_trigger_crossed_below_strict(0.40, Some(0.40)));
        assert!(dual_dca_trigger_crossed_below_strict(0.39, Some(0.40)));
        assert!(!dual_dca_trigger_crossed_below_strict(0.41, Some(0.40)));
    }
}
