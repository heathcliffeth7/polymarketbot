use bot_infra::exchange::{OrderBookLevel, OrderBookSnapshot};
use serde_json::{json, Map, Value};

const DEPTH_QTY_TOLERANCE: f64 = 0.000001;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvDepthEvaluation {
    pub(crate) result: &'static str,
    pub(crate) block_reason: Option<&'static str>,
    pub(crate) best_ask: Option<f64>,
    pub(crate) estimated_avg_fill: Option<f64>,
    pub(crate) vwap_slippage: Option<f64>,
    pub(crate) intended_qty: Option<f64>,
    pub(crate) available_qty_at_best_ask: Option<f64>,
    pub(crate) visible_ask_qty: Option<f64>,
    pub(crate) depth_levels_used: Option<usize>,
}

impl PriceToBeatIvDepthEvaluation {
    pub(crate) fn off() -> Self {
        Self {
            result: "off",
            block_reason: None,
            best_ask: None,
            estimated_avg_fill: None,
            vwap_slippage: None,
            intended_qty: None,
            available_qty_at_best_ask: None,
            visible_ask_qty: None,
            depth_levels_used: None,
        }
    }

    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert("depth_guard_result".to_string(), json!(self.result));
        obj.insert("depth_guard_reason".to_string(), json!(self.block_reason));
        obj.insert("depth_best_ask".to_string(), json!(self.best_ask));
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

    let best_ask = normalize_price(best_ask);
    let intended_qty = intended_qty.filter(|qty| qty.is_finite() && *qty > 0.0);
    let Some(best_ask) = best_ask else {
        return depth_unavailable(intended_qty, None, None, None);
    };
    let Some(intended_qty) = intended_qty else {
        return depth_unavailable(None, Some(best_ask), None, None);
    };
    let Some(order_book) = order_book else {
        return depth_unavailable(Some(intended_qty), Some(best_ask), None, None);
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
            Some(intended_qty),
            Some(best_ask),
            available_qty_at_best_ask,
            visible_ask_qty,
        );
    }

    let estimated_avg_fill = total_cost / filled_qty;
    let vwap_slippage = estimated_avg_fill - best_ask;
    let insufficient_qty = filled_qty + DEPTH_QTY_TOLERANCE < intended_qty;
    let excessive_slippage = vwap_slippage > max_slippage.max(0.0) + 0.000000001;
    let block_reason =
        (insufficient_qty || excessive_slippage).then_some("blocked_insufficient_depth");

    PriceToBeatIvDepthEvaluation {
        result: if block_reason.is_some() {
            "block"
        } else {
            "pass"
        },
        block_reason,
        best_ask: Some(best_ask),
        estimated_avg_fill: Some(estimated_avg_fill),
        vwap_slippage: Some(vwap_slippage),
        intended_qty: Some(intended_qty),
        available_qty_at_best_ask,
        visible_ask_qty,
        depth_levels_used: Some(depth_levels_used),
    }
}

fn depth_unavailable(
    intended_qty: Option<f64>,
    best_ask: Option<f64>,
    available_qty_at_best_ask: Option<f64>,
    visible_ask_qty: Option<f64>,
) -> PriceToBeatIvDepthEvaluation {
    PriceToBeatIvDepthEvaluation {
        result: "unavailable",
        block_reason: Some("blocked_depth_guard_unavailable"),
        best_ask,
        estimated_avg_fill: None,
        vwap_slippage: None,
        intended_qty,
        available_qty_at_best_ask,
        visible_ask_qty,
        depth_levels_used: None,
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
