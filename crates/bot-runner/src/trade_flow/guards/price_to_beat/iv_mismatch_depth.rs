use bot_infra::exchange::{OrderBookLevel, OrderBookSnapshot};
use serde_json::{json, Map, Value};

const DEPTH_QTY_TOLERANCE: f64 = 0.000001;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvDepthEvaluation {
    pub(crate) result: &'static str,
    pub(crate) block_reason: Option<&'static str>,
    pub(crate) block_kind: Option<&'static str>,
    pub(crate) unavailable_reason: Option<&'static str>,
    pub(crate) slippage_hard_blocked: bool,
    pub(crate) slippage_deferred_to_execution_vwap: bool,
    pub(crate) best_ask: Option<f64>,
    pub(crate) book_best_ask: Option<f64>,
    pub(crate) estimated_avg_fill: Option<f64>,
    pub(crate) vwap_slippage: Option<f64>,
    pub(crate) intended_qty: Option<f64>,
    pub(crate) available_qty_at_best_ask: Option<f64>,
    pub(crate) visible_ask_qty: Option<f64>,
    pub(crate) depth_levels_used: Option<usize>,
    pub(crate) order_book_present: Option<bool>,
    pub(crate) order_book_asks_len: Option<usize>,
    pub(crate) order_book_bids_len: Option<usize>,
    pub(crate) valid_asks_len: Option<usize>,
}

impl PriceToBeatIvDepthEvaluation {
    pub(crate) fn off() -> Self {
        Self {
            result: "off",
            block_reason: None,
            block_kind: None,
            unavailable_reason: None,
            slippage_hard_blocked: false,
            slippage_deferred_to_execution_vwap: false,
            best_ask: None,
            book_best_ask: None,
            estimated_avg_fill: None,
            vwap_slippage: None,
            intended_qty: None,
            available_qty_at_best_ask: None,
            visible_ask_qty: None,
            depth_levels_used: None,
            order_book_present: None,
            order_book_asks_len: None,
            order_book_bids_len: None,
            valid_asks_len: None,
        }
    }

    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert("depth_guard_result".to_string(), json!(self.result));
        obj.insert("depth_guard_reason".to_string(), json!(self.block_reason));
        obj.insert("depth_block_kind".to_string(), json!(self.block_kind));
        obj.insert(
            "depth_unavailable_reason".to_string(),
            json!(self.unavailable_reason),
        );
        obj.insert(
            "depth_slippage_hard_blocked".to_string(),
            json!(self.slippage_hard_blocked),
        );
        obj.insert(
            "depth_slippage_deferred_to_execution_vwap".to_string(),
            json!(self.slippage_deferred_to_execution_vwap),
        );
        obj.insert("depth_best_ask".to_string(), json!(self.best_ask));
        obj.insert("depth_book_best_ask".to_string(), json!(self.book_best_ask));
        obj.insert(
            "estimated_avg_fill".to_string(),
            json!(self.estimated_avg_fill),
        );
        obj.insert("vwap_slippage".to_string(), json!(self.vwap_slippage));
        obj.insert("intended_qty".to_string(), json!(self.intended_qty));
        obj.insert(
            "available_qty_at_best_ask".to_string(),
            json!(self.available_qty_at_best_ask),
        );
        obj.insert("visible_ask_qty".to_string(), json!(self.visible_ask_qty));
        obj.insert(
            "depth_levels_used".to_string(),
            json!(self.depth_levels_used),
        );
        obj.insert(
            "depth_order_book_present".to_string(),
            json!(self.order_book_present),
        );
        obj.insert(
            "depth_order_book_asks_len".to_string(),
            json!(self.order_book_asks_len),
        );
        obj.insert(
            "depth_order_book_bids_len".to_string(),
            json!(self.order_book_bids_len),
        );
        obj.insert(
            "depth_valid_asks_len".to_string(),
            json!(self.valid_asks_len),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DepthOrderBookDiagnostics {
    present: Option<bool>,
    asks_len: Option<usize>,
    bids_len: Option<usize>,
    valid_asks_len: Option<usize>,
}

impl DepthOrderBookDiagnostics {
    fn from_order_book(order_book: Option<&OrderBookSnapshot>) -> Self {
        match order_book {
            Some(order_book) => Self {
                present: Some(true),
                asks_len: Some(order_book.asks.len()),
                bids_len: Some(order_book.bids.len()),
                valid_asks_len: Some(order_book.asks.iter().filter(valid_level).count()),
            },
            None => Self {
                present: Some(false),
                asks_len: None,
                bids_len: None,
                valid_asks_len: None,
            },
        }
    }
}

pub(crate) fn evaluate_price_to_beat_iv_depth(
    order_book: Option<&OrderBookSnapshot>,
    best_ask: f64,
    intended_qty: Option<f64>,
    max_slippage: f64,
    enabled: bool,
) -> PriceToBeatIvDepthEvaluation {
    if !enabled {
        return PriceToBeatIvDepthEvaluation::off();
    }

    let order_book_diagnostics = DepthOrderBookDiagnostics::from_order_book(order_book);
    let best_ask = normalize_price(best_ask);
    let intended_qty = intended_qty.filter(|qty| qty.is_finite() && *qty > 0.0);
    let Some(best_ask) = best_ask else {
        return depth_unavailable(
            "invalid_best_ask",
            intended_qty,
            None,
            None,
            None,
            None,
            order_book_diagnostics,
        );
    };
    let Some(intended_qty) = intended_qty else {
        return depth_unavailable(
            "intended_qty_unavailable",
            None,
            Some(best_ask),
            None,
            None,
            None,
            order_book_diagnostics,
        );
    };
    let Some(order_book) = order_book else {
        return depth_unavailable(
            "order_book_unavailable",
            Some(intended_qty),
            Some(best_ask),
            None,
            None,
            None,
            order_book_diagnostics,
        );
    };

    let mut asks = order_book
        .asks
        .iter()
        .filter(|level| valid_level(level))
        .collect::<Vec<_>>();
    asks.sort_by(|left, right| left.price.total_cmp(&right.price));

    let visible_ask_qty = normalize_qty(asks.iter().map(|level| level.size).sum());
    let book_best_ask = asks.first().and_then(|level| normalize_price(level.price));
    let available_qty_at_best_ask = book_best_ask.and_then(|book_best| {
        normalize_qty(
            asks.iter()
                .filter(|level| (level.price - book_best).abs() <= 0.000001)
                .map(|level| level.size)
                .sum(),
        )
    });

    let mut remaining_qty = intended_qty;
    let mut filled_qty = 0.0;
    let mut total_cost = 0.0;
    let mut depth_levels_used = 0usize;

    for level in asks {
        if remaining_qty <= DEPTH_QTY_TOLERANCE {
            break;
        }
        let take_qty = level.size.min(remaining_qty);
        filled_qty += take_qty;
        total_cost += take_qty * level.price;
        remaining_qty -= take_qty;
        depth_levels_used += 1;
    }

    if filled_qty <= DEPTH_QTY_TOLERANCE {
        return depth_unavailable(
            "no_valid_ask_depth",
            Some(intended_qty),
            Some(best_ask),
            book_best_ask,
            available_qty_at_best_ask,
            visible_ask_qty,
            order_book_diagnostics,
        );
    }

    let estimated_avg_fill = total_cost / filled_qty;
    let slippage_reference = book_best_ask.unwrap_or(best_ask);
    let vwap_slippage = estimated_avg_fill - slippage_reference;
    let insufficient_qty = filled_qty + DEPTH_QTY_TOLERANCE < intended_qty;
    let excessive_slippage = vwap_slippage > max_slippage.max(0.0) + 0.000000001;
    let (block_reason, block_kind) = if insufficient_qty {
        (
            Some("blocked_depth_qty_insufficient"),
            Some("qty_insufficient"),
        )
    } else if excessive_slippage {
        (
            Some("blocked_depth_slippage_too_high"),
            Some("slippage_too_high"),
        )
    } else {
        (None, None)
    };

    PriceToBeatIvDepthEvaluation {
        result: if block_reason.is_some() {
            "block"
        } else {
            "pass"
        },
        block_reason,
        block_kind,
        unavailable_reason: None,
        slippage_hard_blocked: false,
        slippage_deferred_to_execution_vwap: false,
        best_ask: Some(best_ask),
        book_best_ask,
        estimated_avg_fill: Some(estimated_avg_fill),
        vwap_slippage: Some(vwap_slippage),
        intended_qty: Some(intended_qty),
        available_qty_at_best_ask,
        visible_ask_qty,
        depth_levels_used: Some(depth_levels_used),
        order_book_present: order_book_diagnostics.present,
        order_book_asks_len: order_book_diagnostics.asks_len,
        order_book_bids_len: order_book_diagnostics.bids_len,
        valid_asks_len: order_book_diagnostics.valid_asks_len,
    }
}

fn depth_unavailable(
    unavailable_reason: &'static str,
    intended_qty: Option<f64>,
    best_ask: Option<f64>,
    book_best_ask: Option<f64>,
    available_qty_at_best_ask: Option<f64>,
    visible_ask_qty: Option<f64>,
    order_book_diagnostics: DepthOrderBookDiagnostics,
) -> PriceToBeatIvDepthEvaluation {
    PriceToBeatIvDepthEvaluation {
        result: "unavailable",
        block_reason: Some("blocked_depth_guard_unavailable"),
        block_kind: Some("unavailable"),
        unavailable_reason: Some(unavailable_reason),
        slippage_hard_blocked: false,
        slippage_deferred_to_execution_vwap: false,
        best_ask,
        book_best_ask,
        estimated_avg_fill: None,
        vwap_slippage: None,
        intended_qty,
        available_qty_at_best_ask,
        visible_ask_qty,
        depth_levels_used: None,
        order_book_present: order_book_diagnostics.present,
        order_book_asks_len: order_book_diagnostics.asks_len,
        order_book_bids_len: order_book_diagnostics.bids_len,
        valid_asks_len: order_book_diagnostics.valid_asks_len,
    }
}

fn valid_level(level: &&OrderBookLevel) -> bool {
    normalize_price(level.price).is_some() && level.size.is_finite() && level.size > 0.0
}

fn normalize_price(value: f64) -> Option<f64> {
    value
        .is_finite()
        .then_some(value)
        .filter(|value| *value > 0.0 && *value < 1.0)
}

fn normalize_qty(value: f64) -> Option<f64> {
    value
        .is_finite()
        .then_some(value)
        .filter(|value| *value > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_unavailable_explains_missing_intended_qty_with_book_counts() {
        let order_book = OrderBookSnapshot {
            bids: vec![OrderBookLevel {
                price: 0.55,
                size: 4.0,
            }],
            asks: vec![OrderBookLevel {
                price: 0.57,
                size: 8.0,
            }],
        };

        let evaluation = evaluate_price_to_beat_iv_depth(Some(&order_book), 0.57, None, 0.03, true);

        assert_eq!(evaluation.result, "unavailable");
        assert_eq!(
            evaluation.unavailable_reason,
            Some("intended_qty_unavailable")
        );
        assert_eq!(evaluation.order_book_present, Some(true));
        assert_eq!(evaluation.order_book_asks_len, Some(1));
        assert_eq!(evaluation.order_book_bids_len, Some(1));
        assert_eq!(evaluation.valid_asks_len, Some(1));
    }

    #[test]
    fn depth_unavailable_separates_missing_book_from_empty_ask_depth() {
        let missing_book = evaluate_price_to_beat_iv_depth(None, 0.57, Some(4.0), 0.03, true);
        assert_eq!(
            missing_book.unavailable_reason,
            Some("order_book_unavailable")
        );
        assert_eq!(missing_book.order_book_present, Some(false));

        let empty_asks = OrderBookSnapshot {
            bids: vec![OrderBookLevel {
                price: 0.55,
                size: 4.0,
            }],
            asks: Vec::new(),
        };
        let empty_ask_depth =
            evaluate_price_to_beat_iv_depth(Some(&empty_asks), 0.57, Some(4.0), 0.03, true);

        assert_eq!(
            empty_ask_depth.unavailable_reason,
            Some("no_valid_ask_depth")
        );
        assert_eq!(empty_ask_depth.order_book_present, Some(true));
        assert_eq!(empty_ask_depth.order_book_asks_len, Some(0));
        assert_eq!(empty_ask_depth.order_book_bids_len, Some(1));
        assert_eq!(empty_ask_depth.valid_asks_len, Some(0));
    }

    #[test]
    fn depth_counts_invalid_ask_rows_without_treating_them_as_depth() {
        let order_book = OrderBookSnapshot {
            bids: Vec::new(),
            asks: vec![
                OrderBookLevel {
                    price: f64::NAN,
                    size: 5.0,
                },
                OrderBookLevel {
                    price: 0.57,
                    size: 0.0,
                },
                OrderBookLevel {
                    price: 1.02,
                    size: 5.0,
                },
            ],
        };

        let evaluation =
            evaluate_price_to_beat_iv_depth(Some(&order_book), 0.57, Some(4.0), 0.03, true);

        assert_eq!(evaluation.result, "unavailable");
        assert_eq!(evaluation.unavailable_reason, Some("no_valid_ask_depth"));
        assert_eq!(evaluation.order_book_asks_len, Some(3));
        assert_eq!(evaluation.valid_asks_len, Some(0));
    }

    #[test]
    fn depth_pass_keeps_existing_behavior_and_adds_counts() {
        let order_book = OrderBookSnapshot {
            bids: Vec::new(),
            asks: vec![OrderBookLevel {
                price: 0.57,
                size: 8.0,
            }],
        };

        let evaluation =
            evaluate_price_to_beat_iv_depth(Some(&order_book), 0.57, Some(4.0), 0.03, true);

        assert_eq!(evaluation.result, "pass");
        assert_eq!(evaluation.unavailable_reason, None);
        assert_eq!(evaluation.available_qty_at_best_ask, Some(8.0));
        assert_eq!(evaluation.visible_ask_qty, Some(8.0));
        assert_eq!(evaluation.order_book_present, Some(true));
        assert_eq!(evaluation.depth_levels_used, Some(1));
    }

    #[test]
    fn depth_block_reason_separates_qty_from_slippage() {
        let thin_book = OrderBookSnapshot {
            bids: Vec::new(),
            asks: vec![OrderBookLevel {
                price: 0.50,
                size: 1.0,
            }],
        };
        let insufficient_qty =
            evaluate_price_to_beat_iv_depth(Some(&thin_book), 0.50, Some(5.0), 0.03, true);

        assert_eq!(insufficient_qty.result, "block");
        assert_eq!(
            insufficient_qty.block_reason,
            Some("blocked_depth_qty_insufficient")
        );
        assert_eq!(insufficient_qty.block_kind, Some("qty_insufficient"));

        let slippy_book = OrderBookSnapshot {
            bids: Vec::new(),
            asks: vec![
                OrderBookLevel {
                    price: 0.50,
                    size: 1.0,
                },
                OrderBookLevel {
                    price: 0.7125,
                    size: 4.0,
                },
            ],
        };
        let slippage =
            evaluate_price_to_beat_iv_depth(Some(&slippy_book), 0.50, Some(5.0), 0.03, true);

        assert_eq!(slippage.result, "block");
        assert_eq!(
            slippage.block_reason,
            Some("blocked_depth_slippage_too_high")
        );
        assert_eq!(slippage.block_kind, Some("slippage_too_high"));
        assert_eq!(slippage.estimated_avg_fill, Some(0.67));
    }
}
