const EXIT_FAST_SUBMIT_QUOTE_TTL_MS: i64 = 500;

#[derive(Debug, Clone)]
struct ExitFastSubmitQuote {
    token_id: String,
    captured_at: DateTime<Utc>,
    market_updated_at_ms: i64,
    order_book: OrderBookSnapshot,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
}

static EXIT_FAST_SUBMIT_QUOTES: LazyLock<StdMutex<HashMap<i64, ExitFastSubmitQuote>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

fn exit_fast_submit_quote_age_ms(quote: &ExitFastSubmitQuote, now: DateTime<Utc>) -> i64 {
    now.signed_duration_since(quote.captured_at)
        .num_milliseconds()
        .max(0)
}

fn exit_fast_submit_quote_is_fresh(
    quote: &ExitFastSubmitQuote,
    token_id: &str,
    now: DateTime<Utc>,
) -> bool {
    quote.token_id == token_id
        && exit_fast_submit_quote_age_ms(quote, now) <= EXIT_FAST_SUBMIT_QUOTE_TTL_MS
}

fn put_exit_fast_submit_quote(
    order_id: i64,
    token_id: &str,
    market_updated_at_ms: i64,
    order_book: &OrderBookSnapshot,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
) {
    if order_id <= 0 || token_id.trim().is_empty() {
        return;
    }
    let quote = ExitFastSubmitQuote {
        token_id: token_id.to_string(),
        captured_at: Utc::now(),
        market_updated_at_ms,
        order_book: order_book.clone(),
        best_bid,
        best_ask,
        last_trade_price,
    };
    if let Ok(mut cache) = EXIT_FAST_SUBMIT_QUOTES.lock() {
        cache.insert(order_id, quote);
        if cache.len() > 512 {
            let now = Utc::now();
            cache.retain(|_, quote| exit_fast_submit_quote_is_fresh(quote, &quote.token_id, now));
        }
    }
}

fn get_exit_fast_submit_quote(
    order_id: i64,
    token_id: &str,
    now: DateTime<Utc>,
) -> Option<ExitFastSubmitQuote> {
    let mut cache = EXIT_FAST_SUBMIT_QUOTES.lock().ok()?;
    let Some(quote) = cache.get(&order_id).cloned() else {
        return None;
    };
    if exit_fast_submit_quote_is_fresh(&quote, token_id, now) {
        return Some(quote);
    }
    cache.remove(&order_id);
    None
}

fn get_exit_fast_submit_quote_for_order(
    order: &TradeBuilderOrder,
    now: DateTime<Utc>,
) -> Option<ExitFastSubmitQuote> {
    if !trade_builder_is_child_exit_sell(order) {
        return None;
    }
    get_exit_fast_submit_quote(order.id, &order.token_id, now)
}

fn trade_builder_runtime_price_from_exit_fast_quote(
    quote: &ExitFastSubmitQuote,
) -> Option<TradeBuilderRuntimePrice> {
    build_trade_builder_fast_runtime_price(
        "exit_fast_quote",
        None,
        quote.best_bid,
        quote.best_ask,
        quote.last_trade_price,
    )
}

#[cfg(test)]
fn put_exit_fast_submit_quote_for_test(order_id: i64, quote: ExitFastSubmitQuote) {
    if let Ok(mut cache) = EXIT_FAST_SUBMIT_QUOTES.lock() {
        cache.insert(order_id.max(1), quote);
    }
}

#[cfg(test)]
fn clear_exit_fast_submit_quotes_for_test() {
    if let Ok(mut cache) = EXIT_FAST_SUBMIT_QUOTES.lock() {
        cache.clear();
    }
}

#[cfg(test)]
mod exit_fast_submit_tests {
    use super::*;

    fn test_order_book() -> OrderBookSnapshot {
        OrderBookSnapshot {
            bids: vec![bot_infra::exchange::OrderBookLevel {
                price: 0.43,
                size: 10.0,
            }],
            asks: vec![bot_infra::exchange::OrderBookLevel {
                price: 0.45,
                size: 8.0,
            }],
        }
    }

    fn test_quote(captured_at: DateTime<Utc>) -> ExitFastSubmitQuote {
        ExitFastSubmitQuote {
            token_id: "tok-up".to_string(),
            captured_at,
            market_updated_at_ms: 123,
            order_book: test_order_book(),
            best_bid: Some(0.43),
            best_ask: Some(0.45),
            last_trade_price: Some(0.46),
        }
    }

    #[test]
    fn exit_fast_submit_quote_expires_after_ttl() {
        clear_exit_fast_submit_quotes_for_test();
        let now = Utc::now();
        put_exit_fast_submit_quote_for_test(
            42,
            test_quote(now - ChronoDuration::milliseconds(EXIT_FAST_SUBMIT_QUOTE_TTL_MS + 1)),
        );

        assert!(get_exit_fast_submit_quote(42, "tok-up", now).is_none());
    }

    #[test]
    fn exit_fast_submit_quote_builds_runtime_price_from_book_bid() {
        let quote = test_quote(Utc::now());
        let runtime_price = trade_builder_runtime_price_from_exit_fast_quote(&quote).unwrap();

        assert_eq!(runtime_price.source, "exit_fast_quote");
        assert_eq!(runtime_price.price, 0.43);
        assert_eq!(runtime_price.best_bid, Some(0.43));
        assert_eq!(runtime_price.best_ask, Some(0.45));
        assert_eq!(runtime_price.last_trade_price, Some(0.46));
    }
}
