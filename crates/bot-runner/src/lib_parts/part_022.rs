#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderVisibleInventoryExpectation {
    gross_qty: f64,
    gross_qty_source: &'static str,
    reference_price: f64,
    expected_fee_qty: f64,
    expected_net_qty: f64,
    expected_visible_qty: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderFirstVisibleInventorySnapshot {
    actual_visible_qty: f64,
    visible_delta_qty: Option<f64>,
    gap_vs_submit_qty: Option<f64>,
    gap_vs_fill_qty: Option<f64>,
    gap_vs_expected_qty: Option<f64>,
}

fn round_trade_builder_signed_qty(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn normalize_trade_builder_visible_inventory_qty(value: Option<f64>) -> Option<f64> {
    let value = value?;
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    Some(round_trade_builder_share_qty(value))
}

fn normalize_trade_builder_visible_inventory_read(value: Option<f64>) -> Option<f64> {
    match value {
        Some(raw) if raw.is_finite() && raw >= 0.0 => Some(round_trade_builder_share_qty(raw)),
        Some(_) => None,
        None => Some(0.0),
    }
}

fn normalize_trade_builder_reference_price(value: Option<f64>) -> Option<f64> {
    let value = value?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    Some(value)
}

fn trade_builder_should_track_buy_inventory_observation(order: &TradeBuilderOrder) -> bool {
    order.side == "buy"
        && order.parent_order_id.is_none()
        && normalize_trade_builder_size_basis(&order.size_basis)
            == TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC
        && (order.tp_enabled || order.sl_enabled)
}

fn trade_builder_submitted_dynamic_qty(order: &TradeBuilderOrder) -> Option<f64> {
    normalize_trade_builder_terminal_fill_qty_candidate(order.submitted_dynamic_qty)
}

fn trade_builder_submitted_dynamic_price(order: &TradeBuilderOrder) -> Option<f64> {
    normalize_trade_builder_reference_price(order.submitted_dynamic_price)
}

fn trade_builder_cumulative_fill_qty(
    order: &TradeBuilderOrder,
    latest_fill_qty: Option<f64>,
) -> Option<f64> {
    if !trade_builder_should_track_buy_inventory_observation(order) || order.filled_qty <= 0.0 {
        return None;
    }

    let latest_fill_qty = normalize_trade_builder_terminal_fill_qty_candidate(latest_fill_qty)?;
    let cumulative = round_trade_builder_share_qty(order.filled_qty + latest_fill_qty);
    (cumulative.is_finite() && cumulative > 0.0).then_some(cumulative)
}

fn trade_builder_observed_submit_qty(
    order: &TradeBuilderOrder,
    submitted_dynamic_qty: Option<f64>,
) -> Option<(f64, &'static str)> {
    if let Some(cumulative_fill_qty) =
        trade_builder_cumulative_fill_qty(order, submitted_dynamic_qty)
    {
        return Some((cumulative_fill_qty, "cumulative_fill_qty"));
    }

    normalize_trade_builder_terminal_fill_qty_candidate(submitted_dynamic_qty)
        .map(|qty| (qty, "submitted_dynamic_qty"))
}

fn trade_builder_observed_fill_qty(
    order: &TradeBuilderOrder,
    resolved_fill_qty: Option<f64>,
) -> Option<(f64, &'static str)> {
    if let Some(cumulative_fill_qty) = trade_builder_cumulative_fill_qty(order, resolved_fill_qty) {
        return Some((cumulative_fill_qty, "cumulative_fill_qty"));
    }

    normalize_trade_builder_terminal_fill_qty_candidate(resolved_fill_qty)
        .map(|qty| (qty, "resolved_fill_qty"))
}

fn trade_builder_canonical_entry_qty(
    order: &TradeBuilderOrder,
    fallback_qty: Option<f64>,
) -> Option<(f64, &'static str)> {
    if trade_builder_should_track_buy_inventory_observation(order) {
        if let Some(cumulative_fill_qty) = trade_builder_cumulative_fill_qty(order, fallback_qty) {
            return Some((cumulative_fill_qty, "cumulative_fill_qty"));
        }
        if let Some(submitted_dynamic_qty) = trade_builder_submitted_dynamic_qty(order) {
            return Some((submitted_dynamic_qty, "submitted_dynamic_qty"));
        }
    }

    normalize_trade_builder_terminal_fill_qty_candidate(fallback_qty)
        .map(|resolved_qty| (resolved_qty, "actual_fill_qty"))
}

fn trade_builder_child_execution_price(
    order: &TradeBuilderOrder,
    actual_execution_price: Option<f64>,
    working_price: Option<f64>,
    market_fallback_price: Option<f64>,
) -> Option<f64> {
    normalize_trade_builder_reference_price(actual_execution_price)
        .or_else(|| normalize_trade_builder_reference_price(working_price))
        .or_else(|| trade_builder_submitted_dynamic_price(order))
        .or_else(|| normalize_trade_builder_reference_price(market_fallback_price))
}

fn trade_builder_visible_inventory_expectation(
    resolved_fill_qty: Option<f64>,
    submitted_dynamic_qty: Option<f64>,
    fill_reference_price: Option<f64>,
    submit_reference_price: Option<f64>,
    fee_rate_bps: i64,
) -> Option<TradeBuilderVisibleInventoryExpectation> {
    let resolved_fill_qty = normalize_trade_builder_terminal_fill_qty_candidate(resolved_fill_qty);
    let submitted_dynamic_qty =
        normalize_trade_builder_terminal_fill_qty_candidate(submitted_dynamic_qty);
    let fill_reference_price = normalize_trade_builder_reference_price(fill_reference_price);
    let submit_reference_price = normalize_trade_builder_reference_price(submit_reference_price);

    let (gross_qty, gross_qty_source, reference_price) =
        if let Some(submitted_dynamic_qty) = submitted_dynamic_qty {
            (
                submitted_dynamic_qty,
                "submitted_dynamic_qty",
                submit_reference_price.or(fill_reference_price)?,
            )
        } else {
            (
                resolved_fill_qty?,
                "resolved_fill_qty",
                fill_reference_price.or(submit_reference_price)?,
            )
        };

    let expected_fee_qty = estimate_trade_builder_buy_fee_shares(
        reference_price,
        gross_qty,
        trade_builder_fee_rate_bps_or_default(fee_rate_bps),
    );
    let expected_net_qty = (gross_qty - expected_fee_qty).max(0.0);
    let expected_visible_qty = floor_trade_builder_share_qty(
        (expected_net_qty - trade_builder_exit_qty_buffer(gross_qty)).max(0.0),
    );

    Some(TradeBuilderVisibleInventoryExpectation {
        gross_qty,
        gross_qty_source,
        reference_price,
        expected_fee_qty,
        expected_net_qty,
        expected_visible_qty,
    })
}

async fn maybe_persist_trade_builder_submitted_dynamic(
    repo: &PostgresRepository,
    run_id: i64,
    order: &mut TradeBuilderOrder,
    submitted_dynamic_qty: f64,
    submitted_dynamic_price: f64,
) {
    if !trade_builder_should_track_buy_inventory_observation(order) {
        return;
    }

    let submitted_dynamic_qty =
        normalize_trade_builder_terminal_fill_qty_candidate(Some(submitted_dynamic_qty));
    let submitted_dynamic_price =
        normalize_trade_builder_reference_price(Some(submitted_dynamic_price));
    let Some(submitted_dynamic_qty) = submitted_dynamic_qty else {
        return;
    };

    if let Err(err) = repo
        .set_trade_builder_order_submitted_dynamic(
            order.id,
            Some(submitted_dynamic_qty),
            submitted_dynamic_price,
        )
        .await
    {
        warn!(
            run_id,
            builder_order_id = order.id,
            error = %err,
            "TRADE_BUILDER_SUBMITTED_DYNAMIC_PERSIST_FAILED"
        );
        return;
    }

    order.submitted_dynamic_qty = Some(submitted_dynamic_qty);
    order.submitted_dynamic_price = submitted_dynamic_price;
}

fn trade_builder_first_visible_inventory_snapshot(
    baseline_visible_qty: Option<f64>,
    actual_visible_qty: f64,
    submitted_dynamic_qty: Option<f64>,
    resolved_fill_qty: Option<f64>,
    expected_visible_qty: Option<f64>,
) -> TradeBuilderFirstVisibleInventorySnapshot {
    let baseline_visible_qty = normalize_trade_builder_visible_inventory_qty(baseline_visible_qty);
    let actual_visible_qty = round_trade_builder_share_qty(actual_visible_qty);
    let visible_delta_qty = baseline_visible_qty
        .map(|baseline_qty| round_trade_builder_signed_qty(actual_visible_qty - baseline_qty));
    let submitted_dynamic_qty =
        normalize_trade_builder_terminal_fill_qty_candidate(submitted_dynamic_qty);
    let resolved_fill_qty = normalize_trade_builder_terminal_fill_qty_candidate(resolved_fill_qty);
    let expected_visible_qty = normalize_trade_builder_visible_inventory_qty(expected_visible_qty);

    TradeBuilderFirstVisibleInventorySnapshot {
        actual_visible_qty,
        visible_delta_qty,
        gap_vs_submit_qty: visible_delta_qty
            .zip(submitted_dynamic_qty)
            .map(|(visible, submitted)| round_trade_builder_signed_qty(visible - submitted)),
        gap_vs_fill_qty: visible_delta_qty
            .zip(resolved_fill_qty)
            .map(|(visible, filled)| round_trade_builder_signed_qty(visible - filled)),
        gap_vs_expected_qty: visible_delta_qty
            .zip(expected_visible_qty)
            .map(|(visible, expected)| round_trade_builder_signed_qty(visible - expected)),
    }
}

async fn maybe_record_trade_builder_buy_inventory_baseline(
    repo: &PostgresRepository,
    run_id: i64,
    client: &dyn OrderExecutor,
    order: &TradeBuilderOrder,
    reference_price: f64,
    fee_rate_bps: u64,
) {
    if !trade_builder_should_track_buy_inventory_observation(order) {
        return;
    }

    let (baseline_visible_qty, payload_json) =
        match client.available_token_qty(&order.token_id).await {
            Ok(quantity) => (
                normalize_trade_builder_visible_inventory_read(quantity),
                json!({
                    "measurement_status": "ok",
                    "raw_visible_qty": quantity,
                }),
            ),
            Err(err) => {
                warn!(
                    run_id,
                    builder_order_id = order.id,
                    token_id = %order.token_id,
                    error = %err,
                    "TRADE_BUILDER_BUY_INVENTORY_BASELINE_FAILED"
                );
                (
                    None,
                    json!({
                        "measurement_status": "error",
                        "error": err.to_string(),
                    }),
                )
            }
        };

    let observation = TradeBuilderInventoryObservationInput {
        parent_builder_order_id: order.id,
        observer_builder_order_id: Some(order.id),
        user_id: order.user_id,
        market_slug: order.market_slug.clone(),
        token_id: order.token_id.clone(),
        outcome_label: order.outcome_label.clone(),
        exchange_order_id: order.active_exchange_order_id.clone(),
        observation_kind: TRADE_BUILDER_OBSERVATION_KIND_BASELINE.to_string(),
        qty_source: Some("available_token_qty".to_string()),
        baseline_visible_qty,
        submitted_dynamic_qty: None,
        resolved_fill_qty: None,
        expected_fee_qty: None,
        expected_net_qty: None,
        expected_visible_qty: None,
        actual_visible_qty: None,
        visible_delta_qty: None,
        gap_vs_submit_qty: None,
        gap_vs_fill_qty: None,
        gap_vs_expected_qty: None,
        reference_price: Some(reference_price),
        fee_rate_bps: Some(fee_rate_bps as i64),
        fill_to_inventory_ms: None,
        payload_json,
    };

    if let Err(err) = repo
        .insert_trade_builder_inventory_observation_if_absent(&observation)
        .await
    {
        warn!(
            run_id,
            builder_order_id = order.id,
            error = %err,
            "TRADE_BUILDER_BUY_INVENTORY_BASELINE_RECORD_FAILED"
        );
    }
}

async fn maybe_record_trade_builder_buy_submit_observation(
    repo: &PostgresRepository,
    run_id: i64,
    order: &TradeBuilderOrder,
    exchange_order_id: &str,
    submitted_dynamic_qty: f64,
    reference_price: f64,
    fee_rate_bps: u64,
    normalized_status: &str,
    payload_json: Value,
) {
    if !trade_builder_should_track_buy_inventory_observation(order) {
        return;
    }

    let Some((submitted_dynamic_qty, qty_source)) =
        trade_builder_observed_submit_qty(order, Some(submitted_dynamic_qty))
    else {
        return;
    };
    let expectation = trade_builder_visible_inventory_expectation(
        if qty_source == "cumulative_fill_qty" {
            Some(submitted_dynamic_qty)
        } else {
            None
        },
        if qty_source == "cumulative_fill_qty" {
            None
        } else {
            Some(submitted_dynamic_qty)
        },
        None,
        Some(reference_price),
        fee_rate_bps as i64,
    );

    let observation = TradeBuilderInventoryObservationInput {
        parent_builder_order_id: order.id,
        observer_builder_order_id: Some(order.id),
        user_id: order.user_id,
        market_slug: order.market_slug.clone(),
        token_id: order.token_id.clone(),
        outcome_label: order.outcome_label.clone(),
        exchange_order_id: Some(exchange_order_id.to_string()),
        observation_kind: TRADE_BUILDER_OBSERVATION_KIND_SUBMIT.to_string(),
        qty_source: Some(qty_source.to_string()),
        baseline_visible_qty: None,
        submitted_dynamic_qty: Some(submitted_dynamic_qty),
        resolved_fill_qty: None,
        expected_fee_qty: expectation.map(|value| value.expected_fee_qty),
        expected_net_qty: expectation.map(|value| value.expected_net_qty),
        expected_visible_qty: expectation.map(|value| value.expected_visible_qty),
        actual_visible_qty: None,
        visible_delta_qty: None,
        gap_vs_submit_qty: None,
        gap_vs_fill_qty: None,
        gap_vs_expected_qty: None,
        reference_price: expectation
            .map(|value| value.reference_price)
            .or_else(|| normalize_trade_builder_reference_price(Some(reference_price))),
        fee_rate_bps: Some(fee_rate_bps as i64),
        fill_to_inventory_ms: None,
        payload_json: json!({
            "normalized_status": normalized_status,
            "payload": payload_json,
        }),
    };

    if let Err(err) = repo
        .upsert_trade_builder_inventory_observation(&observation)
        .await
    {
        warn!(
            run_id,
            builder_order_id = order.id,
            exchange_order_id,
            error = %err,
            "TRADE_BUILDER_BUY_SUBMIT_OBSERVATION_RECORD_FAILED"
        );
    }
}

async fn maybe_record_trade_builder_buy_fill_observation(
    repo: &PostgresRepository,
    order: &TradeBuilderOrder,
    exchange_order_id: &str,
    resolved_fill_qty: Option<f64>,
    reference_price: f64,
    qty_source: Option<&str>,
    force_terminal: bool,
) {
    if !trade_builder_should_track_buy_inventory_observation(order) {
        return;
    }

    let observed_fill_qty =
        trade_builder_observed_fill_qty(order, resolved_fill_qty).map(|(qty, _)| qty);
    let observed_submit_qty =
        trade_builder_observed_submit_qty(order, trade_builder_submitted_dynamic_qty(order))
            .map(|(qty, _)| qty);
    let expectation = trade_builder_visible_inventory_expectation(
        observed_fill_qty,
        if order.filled_qty > 0.0 {
            None
        } else {
            observed_submit_qty
        },
        Some(reference_price),
        trade_builder_submitted_dynamic_price(order),
        order.fee_rate_bps,
    );

    let observation = TradeBuilderInventoryObservationInput {
        parent_builder_order_id: order.id,
        observer_builder_order_id: Some(order.id),
        user_id: order.user_id,
        market_slug: order.market_slug.clone(),
        token_id: order.token_id.clone(),
        outcome_label: order.outcome_label.clone(),
        exchange_order_id: Some(exchange_order_id.to_string()),
        observation_kind: TRADE_BUILDER_OBSERVATION_KIND_FILL.to_string(),
        qty_source: qty_source.map(ToOwned::to_owned),
        baseline_visible_qty: None,
        submitted_dynamic_qty: observed_submit_qty,
        resolved_fill_qty: observed_fill_qty,
        expected_fee_qty: expectation.map(|value| value.expected_fee_qty),
        expected_net_qty: expectation.map(|value| value.expected_net_qty),
        expected_visible_qty: expectation.map(|value| value.expected_visible_qty),
        actual_visible_qty: None,
        visible_delta_qty: None,
        gap_vs_submit_qty: None,
        gap_vs_fill_qty: None,
        gap_vs_expected_qty: None,
        reference_price: expectation
            .map(|value| value.reference_price)
            .or_else(|| normalize_trade_builder_reference_price(Some(reference_price))),
        fee_rate_bps: Some(order.fee_rate_bps),
        fill_to_inventory_ms: None,
        payload_json: json!({
            "force_terminal": force_terminal,
            "actual_fill_qty_unresolved": observed_fill_qty.is_none(),
            "submitted_dynamic_qty": observed_submit_qty,
            "submitted_dynamic_price": trade_builder_submitted_dynamic_price(order),
        }),
    };

    if let Err(err) = repo
        .upsert_trade_builder_inventory_observation(&observation)
        .await
    {
        warn!(
            builder_order_id = order.id,
            exchange_order_id,
            error = %err,
            "TRADE_BUILDER_BUY_FILL_OBSERVATION_RECORD_FAILED"
        );
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum TradeBuilderFirstVisibleInventoryApplyOutcome {
    Applied,
    NotVisible,
}

async fn read_trade_builder_first_visible_inventory(
    client: &dyn OrderExecutor,
    observation: &PendingTradeBuilderFirstVisibleInventoryObservation,
) -> Result<crate::trade_builder_inventory_observation_timing::InventoryObservationReadResult> {
    let quantity = client.available_token_qty(&observation.token_id).await?;
    Ok(trade_builder_first_visible_inventory_read_result(quantity))
}

fn trade_builder_first_visible_inventory_read_result(
    quantity: Option<f64>,
) -> crate::trade_builder_inventory_observation_timing::InventoryObservationReadResult {
    let actual_visible_qty = normalize_trade_builder_visible_inventory_read(quantity);
    match actual_visible_qty {
        Some(qty) if qty > 0.0 => {
            crate::trade_builder_inventory_observation_timing::InventoryObservationReadResult::Visible {
                qty,
            }
        }
        Some(_) => {
            crate::trade_builder_inventory_observation_timing::InventoryObservationReadResult::NotVisible
        }
        None => {
            crate::trade_builder_inventory_observation_timing::InventoryObservationReadResult::ReadError
        }
    }
}

async fn apply_trade_builder_first_visible_inventory(
    repo: &PostgresRepository,
    observation: &PendingTradeBuilderFirstVisibleInventoryObservation,
    actual_visible_qty: f64,
    timing: &mut crate::trade_builder_inventory_observation_timing::InventoryObservationTimingStats,
) -> Result<TradeBuilderFirstVisibleInventoryApplyOutcome> {
    use crate::trade_builder_inventory_observation_timing::{
        InventoryObservationPhase, InventoryObservationTimer,
    };

    let apply_prepare_timer = InventoryObservationTimer::start();
    let baseline_visible_qty =
        normalize_trade_builder_visible_inventory_qty(observation.baseline_visible_qty);
    let visible_delta_qty = baseline_visible_qty
        .map(|baseline_qty| round_trade_builder_signed_qty(actual_visible_qty - baseline_qty));
    let is_ready = if baseline_visible_qty.is_some() {
        visible_delta_qty.unwrap_or_default() > 0.0
    } else {
        actual_visible_qty > 0.0
    };
    if !is_ready {
        timing.record_apply_prepare_ms(apply_prepare_timer.elapsed_ms());
        return Ok(TradeBuilderFirstVisibleInventoryApplyOutcome::NotVisible);
    }

    let expectation = trade_builder_visible_inventory_expectation(
        observation.resolved_fill_qty,
        observation.submitted_dynamic_qty,
        observation.fill_reference_price,
        observation.submit_reference_price,
        observation.fee_rate_bps,
    );
    let snapshot = trade_builder_first_visible_inventory_snapshot(
        observation.baseline_visible_qty,
        actual_visible_qty,
        observation.submitted_dynamic_qty,
        observation.resolved_fill_qty,
        expectation.map(|value| value.expected_visible_qty),
    );
    let fill_to_inventory_ms = Utc::now()
        .signed_duration_since(observation.fill_observed_at)
        .num_milliseconds()
        .max(0);

    let observation_row = TradeBuilderInventoryObservationInput {
        parent_builder_order_id: observation.parent_builder_order_id,
        observer_builder_order_id: observation.observer_builder_order_id,
        user_id: observation.user_id,
        market_slug: observation.market_slug.clone(),
        token_id: observation.token_id.clone(),
        outcome_label: observation.outcome_label.clone(),
        exchange_order_id: observation.exchange_order_id.clone(),
        observation_kind: TRADE_BUILDER_OBSERVATION_KIND_FIRST_VISIBLE.to_string(),
        qty_source: Some("available_token_qty".to_string()),
        baseline_visible_qty,
        submitted_dynamic_qty: normalize_trade_builder_terminal_fill_qty_candidate(
            observation.submitted_dynamic_qty,
        ),
        resolved_fill_qty: normalize_trade_builder_terminal_fill_qty_candidate(
            observation.resolved_fill_qty,
        ),
        expected_fee_qty: expectation.map(|value| value.expected_fee_qty),
        expected_net_qty: expectation.map(|value| value.expected_net_qty),
        expected_visible_qty: expectation.map(|value| value.expected_visible_qty),
        actual_visible_qty: Some(snapshot.actual_visible_qty),
        visible_delta_qty: snapshot.visible_delta_qty,
        gap_vs_submit_qty: snapshot.gap_vs_submit_qty,
        gap_vs_fill_qty: snapshot.gap_vs_fill_qty,
        gap_vs_expected_qty: snapshot.gap_vs_expected_qty,
        reference_price: expectation
            .map(|value| value.reference_price)
            .or_else(|| observation.fill_reference_price)
            .or_else(|| observation.submit_reference_price),
        fee_rate_bps: Some(observation.fee_rate_bps),
        fill_to_inventory_ms: Some(fill_to_inventory_ms),
        payload_json: json!({
            "observation_quality": if baseline_visible_qty.is_some() {
                "baseline_delta"
            } else {
                "no_baseline"
            },
            "gross_qty": expectation.map(|value| value.gross_qty),
            "gross_qty_source": expectation.map(|value| value.gross_qty_source),
            "fill_qty_source": observation.fill_qty_source.as_deref(),
            "submit_reference_price": observation.submit_reference_price,
            "fill_reference_price": observation.fill_reference_price,
        }),
    };
    timing.record_apply_prepare_ms(apply_prepare_timer.elapsed_ms());

    let insert_timer = InventoryObservationTimer::start();
    let insert_result = repo
        .insert_trade_builder_inventory_observation_if_absent(&observation_row)
        .await;
    timing.add_phase_ms(
        InventoryObservationPhase::DbObservationInsert,
        insert_timer.elapsed_ms(),
    );
    if let Err(err) = insert_result {
        timing.record_db_insert_error();
        return Err(err);
    }

    let rebase_timer = InventoryObservationTimer::start();
    let rebase_result = maybe_rebase_trade_builder_parent_position_from_first_visible_inventory(
        repo,
        observation.parent_builder_order_id,
    )
    .await;
    timing.add_phase_ms(
        InventoryObservationPhase::ParentRebase,
        rebase_timer.elapsed_ms(),
    );
    if rebase_result.is_err() {
        timing.record_parent_rebase_error();
    }
    Ok(TradeBuilderFirstVisibleInventoryApplyOutcome::Applied)
}

async fn record_trade_builder_terminal_not_visible_inventory(
    repo: &PostgresRepository,
    observation: &PendingTradeBuilderFirstVisibleInventoryObservation,
    terminal_reason: &str,
    actual_visible_qty: Option<f64>,
    timing: &mut crate::trade_builder_inventory_observation_timing::InventoryObservationTimingStats,
) -> Result<()> {
    use crate::trade_builder_inventory_observation_timing::{
        InventoryObservationPhase, InventoryObservationTimer,
    };

    let baseline_visible_qty =
        normalize_trade_builder_visible_inventory_qty(observation.baseline_visible_qty);
    let actual_visible_qty = actual_visible_qty.map(round_trade_builder_share_qty);
    let visible_delta_qty =
        baseline_visible_qty
            .zip(actual_visible_qty)
            .map(|(baseline_qty, actual_qty)| {
                round_trade_builder_signed_qty(actual_qty - baseline_qty)
            });
    let fill_to_inventory_ms = Utc::now()
        .signed_duration_since(observation.fill_observed_at)
        .num_milliseconds()
        .max(0);
    let observation_row = TradeBuilderInventoryObservationInput {
        parent_builder_order_id: observation.parent_builder_order_id,
        observer_builder_order_id: observation.observer_builder_order_id,
        user_id: observation.user_id,
        market_slug: observation.market_slug.clone(),
        token_id: observation.token_id.clone(),
        outcome_label: observation.outcome_label.clone(),
        exchange_order_id: observation.exchange_order_id.clone(),
        observation_kind: TRADE_BUILDER_OBSERVATION_KIND_FIRST_VISIBLE_TERMINAL_NOT_VISIBLE
            .to_string(),
        qty_source: Some("terminal_not_visible".to_string()),
        baseline_visible_qty,
        submitted_dynamic_qty: normalize_trade_builder_terminal_fill_qty_candidate(
            observation.submitted_dynamic_qty,
        ),
        resolved_fill_qty: normalize_trade_builder_terminal_fill_qty_candidate(
            observation.resolved_fill_qty,
        ),
        expected_fee_qty: None,
        expected_net_qty: None,
        expected_visible_qty: None,
        actual_visible_qty,
        visible_delta_qty,
        gap_vs_submit_qty: None,
        gap_vs_fill_qty: None,
        gap_vs_expected_qty: None,
        reference_price: observation
            .fill_reference_price
            .or(observation.submit_reference_price),
        fee_rate_bps: Some(observation.fee_rate_bps),
        fill_to_inventory_ms: Some(fill_to_inventory_ms),
        payload_json: json!({
            "terminal_reason": terminal_reason,
            "fill_observed_at": observation.fill_observed_at,
            "parent_order_status": observation.parent_order_status,
            "parent_order_filled_qty": observation.parent_order_filled_qty,
        }),
    };

    let insert_timer = InventoryObservationTimer::start();
    let insert_result = repo
        .insert_trade_builder_inventory_observation_if_absent(&observation_row)
        .await;
    timing.add_phase_ms(
        InventoryObservationPhase::DbObservationInsert,
        insert_timer.elapsed_ms(),
    );
    if let Err(err) = insert_result {
        timing.record_db_insert_error();
        return Err(err);
    }
    Ok(())
}

async fn record_trade_builder_terminal_not_visible_inventory_result(
    repo: &PostgresRepository,
    observation: &PendingTradeBuilderFirstVisibleInventoryObservation,
    terminal_reason: &str,
    actual_visible_qty: Option<f64>,
    timing: &mut crate::trade_builder_inventory_observation_timing::InventoryObservationTimingStats,
    record_timing: &mut crate::trade_builder_inventory_observation_timing::InventoryObservationRecordTiming,
) -> Result<crate::trade_builder_inventory_observation_due_gate::InventoryObservationDueResult> {
    use crate::trade_builder_inventory_observation_timing::{
        InventoryObservationPhase, InventoryObservationTimer,
    };

    let apply_timer = InventoryObservationTimer::start();
    record_trade_builder_terminal_not_visible_inventory(
        repo,
        observation,
        terminal_reason,
        actual_visible_qty,
        timing,
    )
    .await?;
    let apply_total_ms = apply_timer.elapsed_ms();
    timing.record_apply_total_ms(apply_total_ms);
    record_timing.add_phase_ms(InventoryObservationPhase::ApplyTotal, apply_total_ms);
    timing.record_not_visible();
    Ok(crate::trade_builder_inventory_observation_due_gate::InventoryObservationDueResult::NotVisible)
}

async fn maybe_sync_trade_builder_inventory_initial_fills(
    repo: &PostgresRepository,
    client: &dyn OrderExecutor,
    user_id: i64,
    builder_timing: &mut crate::trade_builder_order_housekeeping_timing::TradeBuilderOrderHousekeepingTimingStats,
    record_timing: &mut crate::trade_builder_inventory_observation_timing::InventoryObservationRecordTiming,
    synced_user_ids: &mut HashSet<i64>,
    final_fill_sync_required_user_ids: &mut HashSet<i64>,
    throttled_user_ids: &mut HashSet<i64>,
) -> Result<()> {
    use crate::trade_builder_inventory_observation_timing::{
        InventoryObservationPhase, InventoryObservationTimer,
    };

    if synced_user_ids.contains(&user_id) {
        return Ok(());
    }
    if !crate::trade_builder_fill_sync_throttle::inventory_initial_fill_sync_due(
        user_id,
        Instant::now(),
    ) {
        if throttled_user_ids.insert(user_id) {
            let mut skipped_stats =
                crate::trade_builder_fill_sync_timing::FinalFillSyncTimingStats::default();
            skipped_stats.record_skipped_fresh(user_id);
            builder_timing
                .inventory_observation
                .initial_fill_sync_detail
                .merge(skipped_stats);
        }
        return Ok(());
    }

    synced_user_ids.insert(user_id);
    builder_timing.fill_sync_user_count = builder_timing.fill_sync_user_count.saturating_add(1);
    builder_timing.fill_sync_call_count = builder_timing.fill_sync_call_count.saturating_add(1);
    let mut initial_fill_sync_stats =
        crate::trade_builder_fill_sync_timing::FinalFillSyncTimingStats::default();
    initial_fill_sync_stats.record_call(user_id);
    let initial_sync_timer = InventoryObservationTimer::start();
    let sync_result =
        sync_recent_trade_builder_fills_with_timing(repo, client, &mut initial_fill_sync_stats)
            .await;
    let initial_fill_sync_ms = initial_sync_timer.elapsed_ms();
    initial_fill_sync_stats.total_ms = initial_fill_sync_ms;
    initial_fill_sync_stats.record_user_ms(user_id, initial_fill_sync_ms);
    builder_timing
        .inventory_observation
        .record_initial_fill_sync(initial_fill_sync_ms);
    record_timing.add_phase_ms(
        InventoryObservationPhase::InitialFillSync,
        initial_fill_sync_ms,
    );
    if sync_result.is_err() {
        initial_fill_sync_stats.record_error();
        builder_timing.fill_sync_error_count =
            builder_timing.fill_sync_error_count.saturating_add(1);
        final_fill_sync_required_user_ids.insert(user_id);
    } else {
        initial_fill_sync_stats.record_success();
    }
    builder_timing
        .inventory_observation
        .initial_fill_sync_detail
        .merge(initial_fill_sync_stats);
    sync_result?;
    Ok(())
}
