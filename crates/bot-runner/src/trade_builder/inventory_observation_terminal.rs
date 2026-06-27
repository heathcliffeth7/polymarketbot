use bot_infra::db::PendingTradeBuilderFirstVisibleInventoryObservation;
use chrono::{DateTime, Duration as ChronoDuration, Utc};

pub(crate) const ZERO_FILL_TERMINAL_REASON: &str = "zero_fill";
pub(crate) const STALE_NOT_VISIBLE_TERMINAL_REASON: &str = "stale_not_visible_timeout";

const FILL_QTY_EPSILON: f64 = 0.000001;

fn stale_not_visible_timeout() -> ChronoDuration {
    ChronoDuration::minutes(30)
}

fn terminal_fill_qty(observation: &PendingTradeBuilderFirstVisibleInventoryObservation) -> f64 {
    observation
        .resolved_fill_qty
        .unwrap_or(observation.parent_order_filled_qty)
}

pub(crate) fn zero_fill_terminal_reason(
    observation: &PendingTradeBuilderFirstVisibleInventoryObservation,
) -> Option<&'static str> {
    if observation.parent_order_status == "completed"
        && terminal_fill_qty(observation) <= FILL_QTY_EPSILON
    {
        Some(ZERO_FILL_TERMINAL_REASON)
    } else {
        None
    }
}

pub(crate) fn stale_not_visible_terminal_reason(
    observation: &PendingTradeBuilderFirstVisibleInventoryObservation,
    now: DateTime<Utc>,
) -> Option<&'static str> {
    let age = now.signed_duration_since(observation.fill_observed_at);
    if terminal_fill_qty(observation) > FILL_QTY_EPSILON && age >= stale_not_visible_timeout() {
        Some(STALE_NOT_VISIBLE_TERMINAL_REASON)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(
        status: &str,
        resolved_fill_qty: Option<f64>,
        parent_order_filled_qty: f64,
        fill_observed_at: DateTime<Utc>,
    ) -> PendingTradeBuilderFirstVisibleInventoryObservation {
        PendingTradeBuilderFirstVisibleInventoryObservation {
            parent_builder_order_id: 1,
            observer_builder_order_id: None,
            user_id: 7,
            market_slug: "btc-updown-5m".to_string(),
            token_id: "token-a".to_string(),
            outcome_label: "Up".to_string(),
            exchange_order_id: None,
            baseline_visible_qty: None,
            submitted_dynamic_qty: None,
            resolved_fill_qty,
            submit_reference_price: None,
            fill_reference_price: None,
            fill_qty_source: None,
            fee_rate_bps: 0,
            fill_observed_at,
            parent_order_status: status.to_string(),
            parent_order_filled_qty,
        }
    }

    #[test]
    fn completed_zero_fill_terminalizes_without_external_lookup() {
        let now = Utc::now();
        let pending = observation("completed", None, 0.0, now);

        assert_eq!(
            zero_fill_terminal_reason(&pending),
            Some(ZERO_FILL_TERMINAL_REASON)
        );
    }

    #[test]
    fn positive_fill_before_timeout_keeps_polling() {
        let now = Utc::now();
        let pending = observation(
            "completed",
            Some(1.25),
            1.25,
            now - ChronoDuration::minutes(29),
        );

        assert_eq!(stale_not_visible_terminal_reason(&pending, now), None);
    }

    #[test]
    fn positive_fill_after_timeout_terminalizes_when_not_visible() {
        let now = Utc::now();
        let pending = observation(
            "completed",
            Some(1.25),
            1.25,
            now - ChronoDuration::minutes(30),
        );

        assert_eq!(
            stale_not_visible_terminal_reason(&pending, now),
            Some(STALE_NOT_VISIBLE_TERMINAL_REASON)
        );
    }
}
