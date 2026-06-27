const SL_COMPOSITE_DIVERGENCE_KEEP_RATIO: f64 = 0.70;

#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderLocalInventoryFallback {
    submit_qty: f64,
    estimated_fee_qty: f64,
    execution_price: f64,
    fee_rate_bps: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TradeBuilderExitSubmitStage {
    DynamicGross,
    EstimatedVisible,
    VisibleInventory,
}

impl TradeBuilderExitSubmitStage {
    fn as_str(self) -> &'static str {
        match self {
            Self::DynamicGross => "dynamic_gross",
            Self::EstimatedVisible => "estimated_visible",
            Self::VisibleInventory => "visible_inventory",
        }
    }
}

fn trade_builder_exit_submit_stage_from_last_error(
    last_error: Option<&str>,
) -> Option<TradeBuilderExitSubmitStage> {
    let error_text = last_error?;
    if error_text.contains("[exit_submit_stage=visible_inventory]") {
        return Some(TradeBuilderExitSubmitStage::VisibleInventory);
    }
    if error_text.contains("[exit_submit_stage=estimated_visible]") {
        return Some(TradeBuilderExitSubmitStage::EstimatedVisible);
    }
    if error_text.contains("[exit_submit_stage=dynamic_gross]") {
        return Some(TradeBuilderExitSubmitStage::DynamicGross);
    }
    None
}

fn trade_builder_retry_error_text(
    error_text: &str,
    next_stage: Option<TradeBuilderExitSubmitStage>,
) -> String {
    let trimmed = error_text.trim();
    match next_stage {
        Some(stage) => format!(
            "{trimmed} {TRADE_BUILDER_EXIT_STAGE_MARKER_PREFIX}{}]",
            stage.as_str()
        ),
        None => trimmed.to_string(),
    }
}

fn trade_builder_current_exit_submit_stage(
    order: &TradeBuilderOrder,
) -> TradeBuilderExitSubmitStage {
    if !trade_builder_should_use_optimistic_exit_submit(order) {
        return TradeBuilderExitSubmitStage::DynamicGross;
    }
    trade_builder_exit_submit_stage_from_last_error(order.last_error.as_deref())
        .unwrap_or(TradeBuilderExitSubmitStage::DynamicGross)
}

fn trade_builder_exit_qty_buffer(gross_qty: f64) -> f64 {
    (gross_qty * TRADE_BUILDER_LOCAL_EXIT_QTY_BUFFER_RATE).max(TRADE_BUILDER_LOCAL_EXIT_QTY_BUFFER)
}

fn trade_builder_estimated_visible_exit_qty(
    order: &TradeBuilderOrder,
    requested_qty: f64,
) -> Option<TradeBuilderLocalInventoryFallback> {
    if !trade_builder_is_child_exit_sell(order)
        || normalize_trade_builder_size_basis(&order.size_basis) != TRADE_BUILDER_SIZE_BASIS_SHARES
    {
        return None;
    }

    let gross_qty = order.target_qty.filter(|qty| *qty > 0.0)?;
    let execution_price = (order.size_usdc / gross_qty).clamp(0.0, 1.0);
    if execution_price <= 0.0 {
        return None;
    }

    let fee_rate_bps = trade_builder_fee_rate_bps_or_default(order.fee_rate_bps);
    let estimated_fee_qty =
        estimate_trade_builder_buy_fee_shares(execution_price, gross_qty, fee_rate_bps);
    let estimated_visible_qty = round_trade_builder_share_qty(
        (gross_qty - estimated_fee_qty - trade_builder_exit_qty_buffer(gross_qty)).max(0.0),
    );
    let submit_qty = round_trade_builder_share_qty(estimated_visible_qty.min(requested_qty));
    (submit_qty > 0.0).then_some(TradeBuilderLocalInventoryFallback {
        submit_qty,
        estimated_fee_qty,
        execution_price,
        fee_rate_bps,
    })
}

fn trade_builder_local_inventory_fallback(
    order: &TradeBuilderOrder,
    requested_qty: f64,
) -> Option<TradeBuilderLocalInventoryFallback> {
    if !trade_builder_stop_loss_latched(order) {
        return None;
    }
    trade_builder_estimated_visible_exit_qty(order, requested_qty)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderExitInventoryResolution {
    submit_qty: f64,
    submit_partial_visible_inventory: bool,
    visible_qty: Option<f64>,
    local_fallback_qty: Option<f64>,
    local_fallback_fee_qty: Option<f64>,
    local_fallback_entry_price: Option<f64>,
    local_fallback_fee_rate_bps: Option<u64>,
}

fn resolve_trade_builder_exit_inventory(
    order: &TradeBuilderOrder,
    requested_qty: f64,
    visible_qty: Option<f64>,
) -> Option<TradeBuilderExitInventoryResolution> {
    let clamped_visible_qty = clamp_trade_builder_visible_share_qty(requested_qty, visible_qty);
    let local_fallback = trade_builder_local_inventory_fallback(order, requested_qty);
    let submit_qty =
        clamped_visible_qty.or_else(|| local_fallback.map(|value| value.submit_qty))?;

    Some(TradeBuilderExitInventoryResolution {
        submit_qty,
        submit_partial_visible_inventory: submit_qty + TRADE_BUILDER_EXIT_QTY_TOLERANCE
            < requested_qty,
        visible_qty,
        local_fallback_qty: local_fallback.map(|value| value.submit_qty),
        local_fallback_fee_qty: local_fallback.map(|value| value.estimated_fee_qty),
        local_fallback_entry_price: local_fallback.map(|value| value.execution_price),
        local_fallback_fee_rate_bps: local_fallback.map(|value| value.fee_rate_bps),
    })
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TradeBuilderExitRetryQtySource {
    EstimatedVisibleQty,
    ForcedTickQty,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderExitRetryResolution {
    next_qty: f64,
    source: TradeBuilderExitRetryQtySource,
    formula_qty: Option<f64>,
    forced_tick_qty: Option<f64>,
    estimated_fee_qty: Option<f64>,
    execution_price: Option<f64>,
    fee_rate_bps: Option<u64>,
}

fn trade_builder_retry_qty_is_lower(previous_qty: f64, next_qty: f64) -> bool {
    next_qty > 0.0
        && next_qty < previous_qty
        && round_trade_builder_share_qty(previous_qty - next_qty)
            >= TRADE_BUILDER_EXIT_RETRY_MIN_DECREMENT
}

fn trade_builder_forced_retry_tick_qty(requested_qty: f64) -> Option<f64> {
    let shaved = floor_trade_builder_share_qty(
        (requested_qty - TRADE_BUILDER_EXIT_RETRY_MIN_DECREMENT).max(0.0),
    );
    trade_builder_retry_qty_is_lower(requested_qty, shaved).then_some(shaved)
}

#[cfg(test)]
fn resolve_trade_builder_exit_retry_qty(
    order: &TradeBuilderOrder,
    attempted_qty: f64,
) -> Option<TradeBuilderExitRetryResolution> {
    let estimated_visible = trade_builder_estimated_visible_exit_qty(order, attempted_qty);
    if let Some(estimated_visible) = estimated_visible {
        if trade_builder_retry_qty_is_lower(attempted_qty, estimated_visible.submit_qty) {
            return Some(TradeBuilderExitRetryResolution {
                next_qty: estimated_visible.submit_qty,
                source: TradeBuilderExitRetryQtySource::EstimatedVisibleQty,
                formula_qty: Some(estimated_visible.submit_qty),
                forced_tick_qty: None,
                estimated_fee_qty: Some(estimated_visible.estimated_fee_qty),
                execution_price: Some(estimated_visible.execution_price),
                fee_rate_bps: Some(estimated_visible.fee_rate_bps),
            });
        }
    }

    let formula_qty = estimated_visible.map(|value| value.submit_qty);
    let forced_tick_qty = trade_builder_forced_retry_tick_qty(attempted_qty)?;
    Some(TradeBuilderExitRetryResolution {
        next_qty: forced_tick_qty,
        source: TradeBuilderExitRetryQtySource::ForcedTickQty,
        formula_qty,
        forced_tick_qty: Some(forced_tick_qty),
        estimated_fee_qty: estimated_visible.map(|value| value.estimated_fee_qty),
        execution_price: estimated_visible.map(|value| value.execution_price),
        fee_rate_bps: estimated_visible.map(|value| value.fee_rate_bps),
    })
}

fn trade_builder_next_retry_share_qty(requested_qty: f64) -> Option<f64> {
    trade_builder_forced_retry_tick_qty(requested_qty)
}

fn trade_builder_next_optimistic_exit_stage_after_balance_reject(
    _current_attempt_stage: TradeBuilderExitSubmitStage,
) -> TradeBuilderExitSubmitStage {
    TradeBuilderExitSubmitStage::VisibleInventory
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderVisibleInventorySubmitResolution {
    submit_qty: f64,
    submit_partial_visible_inventory: bool,
}

fn resolve_trade_builder_visible_inventory_submit(
    requested_qty: f64,
    available_qty: Option<f64>,
) -> Option<TradeBuilderVisibleInventorySubmitResolution> {
    let submit_qty = clamp_trade_builder_visible_share_qty(requested_qty, available_qty)?;
    Some(TradeBuilderVisibleInventorySubmitResolution {
        submit_qty,
        submit_partial_visible_inventory: submit_qty + TRADE_BUILDER_EXIT_QTY_TOLERANCE
            < requested_qty,
    })
}

fn cancel_error_indicates_terminal_match(error_text: &str) -> bool {
    let normalized = error_text.to_ascii_lowercase();
    normalized.contains("matched orders can't be canceled")
        || (normalized.contains("matched") && normalized.contains("cancel"))
        || (normalized.contains("filled") && normalized.contains("cancel"))
}

fn trade_builder_error_indicates_balance_or_allowance(error_text: &str) -> bool {
    error_text
        .to_ascii_lowercase()
        .contains("not enough balance / allowance")
}

fn trade_builder_error_indicates_midpoint_not_found(error_text: &str) -> bool {
    let normalized = error_text.to_ascii_lowercase();
    normalized.contains("/midpoint") && normalized.contains("404")
}

fn trade_builder_error_is_fatal_exchange_rejection(error_text: &str) -> bool {
    let normalized = error_text.to_ascii_lowercase();
    normalized.contains("invalid signature")
        || normalized.contains("signature verification failed")
        || normalized.contains("invalid api key")
        || normalized.contains("api key not found")
        || normalized.contains("unauthorized")
        || normalized.contains("forbidden")
        || normalized.contains("market not found")
        || normalized.contains("market closed")
        || normalized.contains("market resolved")
        || (normalized.contains("orderbook") && normalized.contains("does not exist"))
}

fn trade_builder_is_terminal_status(status: &str) -> bool {
    matches!(
        status,
        "completed" | "canceled" | "expired" | "filled" | "canceled_requested"
    )
}

fn trade_builder_is_child_exit_sell(order: &TradeBuilderOrder) -> bool {
    order.parent_order_id.is_some() && order.side == "sell" && order.kind == "conditional"
}

fn trade_builder_is_fast_entry_buy(order: &TradeBuilderOrder) -> bool {
    order.parent_order_id.is_none() && order.kind == "conditional" && order.side == "buy"
}

fn trade_builder_uses_fast_runtime_pricing(order: &TradeBuilderOrder) -> bool {
    trade_builder_is_child_exit_sell(order) || trade_builder_is_fast_entry_buy(order)
}

fn trade_builder_is_take_profit_child(order: &TradeBuilderOrder) -> bool {
    trade_builder_is_child_exit_sell(order)
        && matches!(order.trigger_condition.as_deref(), Some("cross_above"))
}

fn trade_builder_should_use_optimistic_exit_submit(order: &TradeBuilderOrder) -> bool {
    if normalize_trade_builder_size_basis(&order.size_basis) != TRADE_BUILDER_SIZE_BASIS_SHARES {
        return false;
    }
    if trade_builder_is_child_exit_sell(order) {
        return true;
    }
    order.side == "sell" && order.kind == "immediate" && order.origin_flow_run_id.is_some()
}

fn trade_builder_should_retry_exit_sell(order: &TradeBuilderOrder) -> bool {
    order.side == "sell"
        && normalize_trade_builder_size_basis(&order.size_basis) == TRADE_BUILDER_SIZE_BASIS_SHARES
        && !order
            .last_error
            .as_deref()
            .is_some_and(trade_builder_error_is_fatal_exchange_rejection)
}

fn trade_builder_should_retry_after_processing_error(order: &TradeBuilderOrder) -> bool {
    if order
        .last_error
        .as_deref()
        .is_some_and(trade_builder_error_is_fatal_exchange_rejection)
    {
        return false;
    }
    trade_builder_should_retry_exit_sell(order)
        && (trade_builder_stop_loss_latched(order)
            || trade_builder_order_is_revenge_flip_stop_loss_sell(order)
            || order
                .last_error
                .as_deref()
                .is_some_and(trade_builder_error_indicates_midpoint_not_found))
}

fn trade_builder_share_target_qty(order: &TradeBuilderOrder) -> Option<f64> {
    if normalize_trade_builder_size_basis(&order.size_basis) != TRADE_BUILDER_SIZE_BASIS_SHARES {
        return None;
    }
    order.target_qty.map(round_trade_builder_share_qty)
}

fn trade_builder_share_remaining_qty(order: &TradeBuilderOrder) -> Option<f64> {
    if normalize_trade_builder_size_basis(&order.size_basis) != TRADE_BUILDER_SIZE_BASIS_SHARES {
        return None;
    }
    order
        .remaining_qty
        .or(order.target_qty)
        .map(round_trade_builder_share_qty)
}

fn trade_builder_scaled_size_usdc(order: &TradeBuilderOrder, next_qty: f64) -> f64 {
    let next_qty = round_trade_builder_share_qty(next_qty).max(0.0);
    let current_target_qty = trade_builder_share_target_qty(order).unwrap_or_default();
    if current_target_qty > 0.0 && order.size_usdc > 0.0 {
        return ((order.size_usdc / current_target_qty) * next_qty).max(0.0);
    }
    let fallback_price = order
        .working_price
        .or(order.submitted_dynamic_price)
        .or(order.trigger_price)
        .unwrap_or_default();
    (next_qty * fallback_price).max(0.0)
}

fn trade_builder_exit_sell_price_floor(order: &TradeBuilderOrder) -> Option<f64> {
    if !trade_builder_is_child_exit_sell(order) {
        return None;
    }
    // SL icin floor uygulama - piyasa fiyatindan sat
    if matches!(order.trigger_condition.as_deref(), Some("cross_below")) {
        return None;
    }
    let trigger_price = order.trigger_price?;
    let floored =
        ((trigger_price - TRADE_BUILDER_EXIT_TRIGGER_BUFFER).max(0.0) * 100.0).round() / 100.0;
    Some(clamp_probability(floored))
}

fn trade_builder_cap_exit_sell_price(order: &TradeBuilderOrder, desired_price: f64) -> f64 {
    if order.side != "sell" {
        return desired_price;
    }
    trade_builder_exit_sell_price_floor(order)
        .map(|floor| desired_price.max(floor))
        .unwrap_or(desired_price)
}

fn trade_builder_is_market_buy(order: &TradeBuilderOrder) -> bool {
    order.side == "buy"
        && normalize_trade_builder_execution_mode(&order.execution_mode) == "market"
}

fn trade_builder_is_immediate_notional_buy(order: &TradeBuilderOrder) -> bool {
    trade_builder_is_market_buy(order)
        && order.kind == "immediate"
        && normalize_trade_builder_size_basis(&order.size_basis)
            == TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC
}

fn trade_builder_market_buy_anchor_price(order: &TradeBuilderOrder) -> Option<f64> {
    if !trade_builder_is_market_buy(order) {
        return None;
    }

    normalize_trade_builder_reference_price(order.trigger_price).map(clamp_probability)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderMarketBuyExecutionPrice {
    price: f64,
    source: &'static str,
    trigger_reference_price: Option<f64>,
}

const TRADE_BUILDER_MARKETABLE_BUY_MIN_NOTIONAL_USDC: f64 = 1.0;
const TRADE_BUILDER_MARKETABLE_BUY_MIN_NOTIONAL_CAP_TOLERANCE_USDC: f64 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderMarketableBuyMinNotionalTopUp {
    original_qty: f64,
    adjusted_qty: f64,
    original_notional_usdc: f64,
    adjusted_notional_usdc: f64,
    min_notional_usdc: f64,
    cap_usdc: f64,
    cap_tolerance_usdc: f64,
    blocked_by_cap: bool,
}

fn trade_builder_market_buy_execution_price(
    order: &TradeBuilderOrder,
    current_price: f64,
    best_ask: Option<f64>,
) -> Option<TradeBuilderMarketBuyExecutionPrice> {
    if !trade_builder_is_market_buy(order) {
        return None;
    }

    let trigger_reference_price = trade_builder_market_buy_anchor_price(order);
    if let Some(best_ask) = best_ask
        .and_then(|value| normalize_trade_builder_reference_price(Some(value)))
        .map(clamp_probability)
    {
        return Some(TradeBuilderMarketBuyExecutionPrice {
            price: best_ask,
            source: "best_ask",
            trigger_reference_price,
        });
    }

    if !trade_builder_is_immediate_notional_buy(order) {
        return None;
    }

    Some(TradeBuilderMarketBuyExecutionPrice {
        price: normalize_trade_builder_reference_price(Some(current_price))
            .map(clamp_probability)
            .unwrap_or_else(|| clamp_probability(current_price)),
        source: "current_price_fallback",
        trigger_reference_price,
    })
}

fn trade_builder_marketable_buy_min_notional_top_up(
    side: &str,
    order_type: &str,
    size_basis: &str,
    desired_price: f64,
    submit_qty: f64,
    cap_usdc: f64,
) -> Option<TradeBuilderMarketableBuyMinNotionalTopUp> {
    if side != "buy"
        || size_basis != TRADE_BUILDER_SIZE_BASIS_SHARES
        || !matches!(order_type, "FAK" | "FOK")
        || !desired_price.is_finite()
        || desired_price <= 0.0
        || !submit_qty.is_finite()
        || submit_qty <= 0.0
    {
        return None;
    }
    let original_notional_usdc = submit_qty * desired_price;
    if original_notional_usdc + 0.000001 >= TRADE_BUILDER_MARKETABLE_BUY_MIN_NOTIONAL_USDC {
        return None;
    }
    let adjusted_qty = round_trade_builder_share_qty(
        ((TRADE_BUILDER_MARKETABLE_BUY_MIN_NOTIONAL_USDC / desired_price) * 100.0).ceil()
            / 100.0,
    );
    let adjusted_notional_usdc = adjusted_qty * desired_price;
    Some(TradeBuilderMarketableBuyMinNotionalTopUp {
        original_qty: submit_qty,
        adjusted_qty,
        original_notional_usdc,
        adjusted_notional_usdc,
        min_notional_usdc: TRADE_BUILDER_MARKETABLE_BUY_MIN_NOTIONAL_USDC,
        cap_usdc,
        cap_tolerance_usdc: TRADE_BUILDER_MARKETABLE_BUY_MIN_NOTIONAL_CAP_TOLERANCE_USDC,
        blocked_by_cap: adjusted_notional_usdc
            > cap_usdc + TRADE_BUILDER_MARKETABLE_BUY_MIN_NOTIONAL_CAP_TOLERANCE_USDC,
    })
}

fn trade_builder_submit_desired_price(order: &TradeBuilderOrder, current_price: f64) -> f64 {
    let uncapped_desired_price =
        aggressive_price_for_side(&order.side, current_price, order.min_price_distance_cent);
    trade_builder_cap_exit_sell_price(order, uncapped_desired_price)
}

fn trade_builder_clamp_buy_limit_price(order: &TradeBuilderOrder, desired_price: f64) -> f64 {
    if order.side == "buy" {
        order
            .max_price
            .filter(|max_price| max_price.is_finite() && *max_price > 0.0 && *max_price <= 1.0)
            .map(|max_price| desired_price.min(max_price))
            .unwrap_or(desired_price)
    } else {
        desired_price
    }
}

fn trade_builder_price_exceeds_max_price(order: &TradeBuilderOrder, desired_price: f64) -> bool {
    order
        .max_price
        .map(|max_price| desired_price.is_finite() && desired_price > max_price)
        .unwrap_or(false)
}

fn trade_builder_resolve_max_price_reference(
    order: &TradeBuilderOrder,
    best_ask: Option<f64>,
    desired_price: f64,
) -> (f64, &'static str) {
    if order.side != "buy" {
        return (desired_price, "desired_price");
    }
    match best_ask.filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0) {
        Some(best_ask) => (best_ask, "best_ask"),
        None => (desired_price, "desired_price_fallback"),
    }
}

fn trade_builder_resolve_trigger_guard_reference_price(
    order: &TradeBuilderOrder,
    current_price: f64,
    best_ask: Option<f64>,
) -> (f64, &'static str) {
    if order.side != "buy" {
        return (current_price, "current_price");
    }
    match best_ask
        .and_then(|value| normalize_trade_builder_reference_price(Some(value)))
        .map(clamp_probability)
    {
        Some(best_ask) => (best_ask, "best_ask"),
        None => (
            normalize_trade_builder_reference_price(Some(current_price))
                .map(clamp_probability)
                .unwrap_or_else(|| clamp_probability(current_price)),
            "current_price_fallback",
        ),
    }
}

fn trade_builder_price_below_guard_trigger(
    order: &TradeBuilderOrder,
    reference_price: f64,
) -> bool {
    if order.side != "buy" {
        return false;
    }
    order
        .guard_trigger_price
        .map(|guard| reference_price.is_finite() && reference_price < guard)
        .unwrap_or(false)
}

fn trade_builder_execution_floor_block_reason(
    order: &TradeBuilderOrder,
    best_ask: Option<f64>,
) -> Option<&'static str> {
    if order.side != "buy" {
        return None;
    }
    let floor_price = order.best_ask_floor_price?;
    let best_ask = best_ask.filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)?;
    if best_ask < floor_price {
        Some("below_best_ask_floor")
    } else {
        None
    }
}

#[cfg(test)]
fn trade_builder_execution_floor_missing_best_ask(
    order: &TradeBuilderOrder,
    best_ask: Option<f64>,
) -> bool {
    if order.side != "buy" || order.best_ask_floor_price.is_none() {
        return false;
    }
    best_ask
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .is_none()
}

fn trade_builder_execution_floor_should_wait(order: &TradeBuilderOrder, reason_code: &str) -> bool {
    reason_code == "best_ask_unavailable" || order.retry_on_execution_floor_guard_block
}

fn normalize_trade_builder_size_basis(raw: &str) -> &'static str {
    if raw
        .trim()
        .eq_ignore_ascii_case(TRADE_BUILDER_SIZE_BASIS_SHARES)
    {
        return TRADE_BUILDER_SIZE_BASIS_SHARES;
    }
    TRADE_BUILDER_SIZE_BASIS_NOTIONAL_USDC
}

fn round_trade_builder_share_qty(value: f64) -> f64 {
    ((value.max(0.0)) * 100.0).round() / 100.0
}

fn floor_trade_builder_share_qty(value: f64) -> f64 {
    ((value.max(0.0)) * 100.0).floor() / 100.0
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderExitChildSizing {
    size_usdc: f64,
    target_qty: f64,
    remaining_qty: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct TradeBuilderRuntimePrice {
    price: f64,
    source: &'static str,
    runtime_warning: Option<String>,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
enum TradeBuilderRuntimePriceFetch {
    Resolved(TradeBuilderRuntimePrice),
    Retry { error_text: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TradeBuilderTriggerEvaluation {
    should_trigger: bool,
    first_tick_threshold_used: bool,
}

fn fast_trigger_eval_price(
    runtime_price: &TradeBuilderRuntimePrice,
    trigger_condition: Option<&str>,
) -> f64 {
    match (
        runtime_price.best_bid,
        runtime_price.last_trade_price,
        trigger_condition,
    ) {
        (Some(best_bid), Some(last_trade_price), Some("cross_above")) => {
            clamp_probability(best_bid.max(last_trade_price))
        }
        (Some(best_bid), Some(last_trade_price), Some("cross_below")) => {
            let filtered = if best_bid > 0.0
                && last_trade_price < best_bid * SL_COMPOSITE_DIVERGENCE_KEEP_RATIO
            {
                best_bid
            } else {
                best_bid.min(last_trade_price)
            };
            clamp_probability(filtered)
        }
        (Some(best_bid), None, _) => clamp_probability(best_bid),
        (None, Some(last_trade_price), _) => clamp_probability(last_trade_price),
        _ => runtime_price.price,
    }
}

fn sl_trigger_eval_price_for_mode(
    mode: &str,
    runtime_price: &TradeBuilderRuntimePrice,
) -> Option<f64> {
    match mode {
        "best_bid" => runtime_price.best_bid.map(clamp_probability),
        "last_trade" => runtime_price.last_trade_price.map(clamp_probability),
        "composite_fast" => Some(
            match (runtime_price.best_bid, runtime_price.last_trade_price) {
                (Some(best_bid), Some(last_trade_price)) => {
                    clamp_probability(best_bid.min(last_trade_price))
                }
                (Some(best_bid), None) => clamp_probability(best_bid),
                (None, Some(last_trade_price)) => clamp_probability(last_trade_price),
                _ => runtime_price.price,
            },
        ),
        _ => Some(fast_trigger_eval_price(runtime_price, Some("cross_below"))),
    }
}

fn fast_execution_price_for_sell(runtime_price: &TradeBuilderRuntimePrice) -> f64 {
    runtime_price
        .best_bid
        .or(runtime_price.last_trade_price)
        .or(Some(runtime_price.price))
        .map(clamp_probability)
        .unwrap_or(runtime_price.price)
}

fn fast_execution_price_for_buy(runtime_price: &TradeBuilderRuntimePrice) -> f64 {
    runtime_price
        .best_ask
        .or(runtime_price.last_trade_price)
        .or(runtime_price.best_bid)
        .or(Some(runtime_price.price))
        .map(clamp_probability)
        .unwrap_or(runtime_price.price)
}

fn trade_builder_trigger_eval_price_for_order(
    order: &TradeBuilderOrder,
    runtime_price: &TradeBuilderRuntimePrice,
) -> f64 {
    if !trade_builder_uses_fast_runtime_pricing(order) {
        return runtime_price.price;
    }
    if trade_builder_is_stop_loss_child(order) {
        if let Some(mode) = order.sl_trigger_price_mode.as_deref() {
            if let Some(price) = sl_trigger_eval_price_for_mode(mode, runtime_price) {
                return price;
            }
            return runtime_price.price;
        }
    }
    fast_trigger_eval_price(runtime_price, order.trigger_condition.as_deref())
}

fn trade_builder_execution_price_for_order(
    order: &TradeBuilderOrder,
    runtime_price: &TradeBuilderRuntimePrice,
) -> f64 {
    if !trade_builder_uses_fast_runtime_pricing(order) {
        return runtime_price.price;
    }
    if order.side == "sell" {
        return fast_execution_price_for_sell(runtime_price);
    }
    fast_execution_price_for_buy(runtime_price)
}

fn trade_builder_last_seen_price_for_order(
    order: &TradeBuilderOrder,
    trigger_eval_price: f64,
    execution_price: f64,
) -> f64 {
    if trade_builder_is_child_exit_sell(order) {
        return execution_price;
    }
    if trade_builder_is_fast_entry_buy(order) {
        return trigger_eval_price;
    }
    execution_price
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderTerminalFillQtyCandidates {
    order_info_filled_size: Option<f64>,
    synced_db_fill_qty: Option<f64>,
    order_info_size: Option<f64>,
    stored_order_size: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TradeBuilderTerminalFillQtySource {
    OrderInfoFilledSize,
    OrderInfoSize,
    StoredOrderSize,
}

impl TradeBuilderTerminalFillQtySource {
    fn as_str(self) -> &'static str {
        match self {
            Self::OrderInfoFilledSize => "order_info_filled_size",
            Self::OrderInfoSize => "order_info_size",
            Self::StoredOrderSize => "stored_order_size",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TradeBuilderResolvedTerminalFillQty {
    qty: f64,
    source: TradeBuilderTerminalFillQtySource,
    candidates: TradeBuilderTerminalFillQtyCandidates,
}

fn normalize_trade_builder_terminal_fill_qty_candidate(value: Option<f64>) -> Option<f64> {
    let value = value?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    let rounded = round_trade_builder_share_qty(value);
    (rounded.is_finite() && rounded > 0.0).then_some(rounded)
}

fn select_trade_builder_terminal_fill_qty(
    candidates: TradeBuilderTerminalFillQtyCandidates,
) -> Option<TradeBuilderResolvedTerminalFillQty> {
    let selected = [
        (
            TradeBuilderTerminalFillQtySource::OrderInfoFilledSize,
            candidates.order_info_filled_size,
        ),
        (
            TradeBuilderTerminalFillQtySource::OrderInfoSize,
            candidates.order_info_size,
        ),
        (
            TradeBuilderTerminalFillQtySource::StoredOrderSize,
            candidates.stored_order_size,
        ),
    ]
    .into_iter()
    .find_map(|(source, value)| {
        normalize_trade_builder_terminal_fill_qty_candidate(value).map(|qty| (source, qty))
    })?;

    Some(TradeBuilderResolvedTerminalFillQty {
        qty: selected.1,
        source: selected.0,
        candidates,
    })
}

#[cfg(test)]
fn trade_builder_exit_child_sizing(
    filled_size: f64,
    execution_price: f64,
) -> TradeBuilderExitChildSizing {
    let target_qty = round_trade_builder_share_qty(filled_size);
    let size_usdc = (target_qty * execution_price).max(0.0);
    TradeBuilderExitChildSizing {
        size_usdc,
        target_qty,
        remaining_qty: target_qty,
    }
}

fn trade_builder_share_request_qty(order: &TradeBuilderOrder) -> Option<f64> {
    let qty = order.remaining_qty.or(order.target_qty)?;
    let rounded = round_trade_builder_share_qty(qty);
    (rounded > 0.0).then_some(rounded)
}

fn clamp_trade_builder_visible_share_qty(
    requested_qty: f64,
    available_qty: Option<f64>,
) -> Option<f64> {
    let effective_qty = match available_qty {
        Some(quantity) => requested_qty.min(quantity),
        None => requested_qty,
    };
    let clamped = floor_trade_builder_share_qty(effective_qty);
    (clamped > 0.0).then_some(clamped)
}

fn trade_builder_inventory_pending_tp_trigger_price(order: &TradeBuilderOrder) -> Option<f64> {
    let trigger_price = order.trigger_price?;
    if order.status == "inventory_pending"
        && order.side == "sell"
        && order.parent_order_id.is_some()
        && matches!(order.trigger_condition.as_deref(), Some("cross_above"))
    {
        let adjusted =
            ((trigger_price - TRADE_BUILDER_EXIT_TP_SLACK).max(0.0) * 100.0).round() / 100.0;
        return Some(clamp_probability(adjusted));
    }
    Some(trigger_price)
}

fn estimate_remaining_trade_builder_sizing(
    order: &TradeBuilderOrder,
    order_info: &OrderInfo,
    fallback_price: f64,
) -> (Option<f64>, Option<f64>) {
    let size_basis = normalize_trade_builder_size_basis(&order.size_basis);
    if let (Some(order_size), Some(filled_size)) = (order_info.size, order_info.filled_size) {
        let remaining_qty = round_trade_builder_share_qty((order_size - filled_size).max(0.0));
        let price = order_info
            .price
            .or(order.working_price)
            .unwrap_or(fallback_price);
        let remaining_usdc = (remaining_qty * price).max(0.0);
        return (
            Some(remaining_usdc),
            if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
                Some(remaining_qty)
            } else {
                None
            },
        );
    }

    if size_basis == TRADE_BUILDER_SIZE_BASIS_SHARES {
        let remaining_qty = trade_builder_share_request_qty(order);
        let remaining_usdc = remaining_qty.map(|qty| qty * fallback_price);
        return (remaining_usdc, remaining_qty);
    }

    (
        Some(order.remaining_size.unwrap_or(order.size_usdc).max(0.0)),
        None,
    )
}
