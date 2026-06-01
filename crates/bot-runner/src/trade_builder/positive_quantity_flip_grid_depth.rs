#[derive(Debug, Clone)]
struct PositiveQuantityFlipGridDepthCheck {
    blocked: bool,
    estimated_avg_fill: Option<f64>,
    payload: Value,
}

fn positive_quantity_flip_grid_depth_payload(
    result: &str,
    reason: Option<&str>,
    enabled: bool,
    best_ask: f64,
    worst_price: f64,
    intended_qty: f64,
    filled_qty: f64,
    visible_ask_qty: Option<f64>,
    estimated_avg_fill: Option<f64>,
    depth_levels_used: Option<usize>,
) -> Value {
    json!({
        "result": result,
        "reason": reason,
        "depth_guard_enabled": enabled,
        "best_ask": best_ask,
        "worst_price": worst_price,
        "intended_qty": intended_qty,
        "filled_qty": filled_qty,
        "visible_ask_qty": visible_ask_qty,
        "estimated_avg_fill": estimated_avg_fill,
        "depth_levels_used": depth_levels_used,
    })
}

fn positive_quantity_flip_grid_evaluate_depth(
    enabled: bool,
    order_book: Option<&OrderBookSnapshot>,
    best_ask: f64,
    intended_qty: f64,
    worst_price: f64,
) -> PositiveQuantityFlipGridDepthCheck {
    if !enabled {
        return PositiveQuantityFlipGridDepthCheck {
            blocked: false,
            estimated_avg_fill: Some(best_ask),
            payload: positive_quantity_flip_grid_depth_payload(
                "pass",
                Some("disabled"),
                false,
                best_ask,
                worst_price,
                intended_qty,
                intended_qty,
                None,
                Some(best_ask),
                None,
            ),
        };
    }
    let Some(order_book) = order_book else {
        return PositiveQuantityFlipGridDepthCheck {
            blocked: true,
            estimated_avg_fill: None,
            payload: positive_quantity_flip_grid_depth_payload(
                "blocked",
                Some("depth_unavailable"),
                true,
                best_ask,
                worst_price,
                intended_qty,
                0.0,
                None,
                None,
                None,
            ),
        };
    };
    let mut levels = order_book
        .asks
        .iter()
        .filter(|level| {
            level.price.is_finite()
                && level.size.is_finite()
                && level.price > 0.0
                && level.size > 0.0
                && level.price <= worst_price + 0.000001
        })
        .collect::<Vec<_>>();
    levels.sort_by(|left, right| left.price.total_cmp(&right.price));
    let visible_ask_qty = levels.iter().map(|level| level.size.max(0.0)).sum::<f64>();
    let mut remaining_qty = intended_qty;
    let mut filled_qty = 0.0;
    let mut cost = 0.0;
    let mut levels_used = 0usize;
    for level in levels {
        if remaining_qty <= 0.000001 {
            break;
        }
        let take_qty = remaining_qty.min(level.size);
        filled_qty += take_qty;
        cost += take_qty * level.price;
        remaining_qty -= take_qty;
        levels_used += 1;
    }
    if filled_qty + 0.000001 < intended_qty {
        return PositiveQuantityFlipGridDepthCheck {
            blocked: true,
            estimated_avg_fill: None,
            payload: positive_quantity_flip_grid_depth_payload(
                "blocked",
                Some("insufficient_visible_ask_depth"),
                true,
                best_ask,
                worst_price,
                intended_qty,
                filled_qty,
                Some(visible_ask_qty),
                None,
                Some(levels_used),
            ),
        };
    }
    let avg_fill = cost / filled_qty;
    PositiveQuantityFlipGridDepthCheck {
        blocked: false,
        estimated_avg_fill: Some(avg_fill),
        payload: positive_quantity_flip_grid_depth_payload(
            "pass",
            None,
            true,
            best_ask,
            worst_price,
            intended_qty,
            filled_qty,
            Some(visible_ask_qty),
            Some(avg_fill),
            Some(levels_used),
        ),
    }
}
