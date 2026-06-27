#[derive(Debug, Clone, PartialEq)]
struct AvgReboundVwapLevel {
    price: rust_decimal::Decimal,
    size: rust_decimal::Decimal,
}

#[derive(Debug, Clone, PartialEq)]
struct AvgReboundVwapQuote {
    requested_qty: rust_decimal::Decimal,
    limit_price: rust_decimal::Decimal,
    executable_qty: rust_decimal::Decimal,
    vwap: rust_decimal::Decimal,
    notional: rust_decimal::Decimal,
    best_ask: Option<rust_decimal::Decimal>,
    levels: Vec<AvgReboundVwapLevel>,
}

#[derive(Debug, Clone, PartialEq)]
struct AvgReboundVwapRejection {
    reason: &'static str,
    requested_qty: rust_decimal::Decimal,
    limit_price: rust_decimal::Decimal,
    executable_qty: rust_decimal::Decimal,
    best_ask: Option<rust_decimal::Decimal>,
}

fn avg_rebound_order_book_best_ask(book: &OrderBookSnapshot) -> Option<rust_decimal::Decimal> {
    book.asks
        .iter()
        .filter(|level| level.price.is_finite() && level.price > 0.0)
        .map(|level| avg_rebound_decimal_from_f64(level.price))
        .min()
}

fn avg_rebound_vwap_for_fok_limit(
    book: &OrderBookSnapshot,
    requested_qty: rust_decimal::Decimal,
    configured_cap: rust_decimal::Decimal,
    safety_buffer: rust_decimal::Decimal,
) -> std::result::Result<AvgReboundVwapQuote, AvgReboundVwapRejection> {
    let limit_price = configured_cap - safety_buffer;
    let best_ask = avg_rebound_order_book_best_ask(book);
    if requested_qty <= rust_decimal::Decimal::ZERO {
        return Err(AvgReboundVwapRejection {
            reason: "invalid_requested_qty",
            requested_qty,
            limit_price,
            executable_qty: rust_decimal::Decimal::ZERO,
            best_ask,
        });
    }
    if limit_price <= rust_decimal::Decimal::ZERO {
        return Err(AvgReboundVwapRejection {
            reason: "non_positive_limit_price",
            requested_qty,
            limit_price,
            executable_qty: rust_decimal::Decimal::ZERO,
            best_ask,
        });
    }

    let mut ask_levels = book
        .asks
        .iter()
        .filter(|level| {
            level.price.is_finite()
                && level.size.is_finite()
                && level.price > 0.0
                && level.size > 0.0
        })
        .map(|level| AvgReboundVwapLevel {
            price: avg_rebound_decimal_from_f64(level.price),
            size: avg_rebound_decimal_from_f64(level.size),
        })
        .collect::<Vec<_>>();
    ask_levels.sort_by(|left, right| left.price.cmp(&right.price));

    let mut remaining = requested_qty;
    let mut executable_qty = rust_decimal::Decimal::ZERO;
    let mut notional = rust_decimal::Decimal::ZERO;
    let mut used_levels = Vec::new();
    for level in ask_levels {
        if level.price > limit_price || remaining <= rust_decimal::Decimal::ZERO {
            continue;
        }
        let take = avg_rebound_qty_min(level.size, remaining);
        if take <= rust_decimal::Decimal::ZERO {
            continue;
        }
        executable_qty += take;
        notional += take * level.price;
        remaining -= take;
        used_levels.push(AvgReboundVwapLevel {
            price: level.price,
            size: take,
        });
    }

    if executable_qty < requested_qty {
        return Err(AvgReboundVwapRejection {
            reason: "insufficient_depth_at_or_below_limit",
            requested_qty,
            limit_price,
            executable_qty,
            best_ask,
        });
    }
    let vwap = notional / requested_qty;
    if vwap > limit_price {
        return Err(AvgReboundVwapRejection {
            reason: "vwap_above_limit",
            requested_qty,
            limit_price,
            executable_qty,
            best_ask,
        });
    }
    Ok(AvgReboundVwapQuote {
        requested_qty,
        limit_price,
        executable_qty,
        vwap,
        notional,
        best_ask,
        levels: used_levels,
    })
}

fn avg_rebound_vwap_quote_json(quote: &AvgReboundVwapQuote) -> Value {
    json!({
        "requested_qty": quote.requested_qty.to_string(),
        "limit_price": quote.limit_price.to_string(),
        "executable_qty": quote.executable_qty.to_string(),
        "vwap": quote.vwap.to_string(),
        "notional": quote.notional.to_string(),
        "best_ask": quote.best_ask.map(|value| value.to_string()),
        "levels": quote.levels.iter().map(|level| {
            json!({
                "price": level.price.to_string(),
                "size": level.size.to_string(),
            })
        }).collect::<Vec<_>>(),
    })
}

fn avg_rebound_vwap_rejection_json(rejection: &AvgReboundVwapRejection) -> Value {
    json!({
        "reason": rejection.reason,
        "requested_qty": rejection.requested_qty.to_string(),
        "limit_price": rejection.limit_price.to_string(),
        "executable_qty": rejection.executable_qty.to_string(),
        "best_ask": rejection.best_ask.map(|value| value.to_string()),
    })
}
