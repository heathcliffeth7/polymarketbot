use super::types::{parse_f64, CexBookSample, CexTradeSample, CexVenue, TakerSide};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

pub(crate) const COINBASE_DEFAULT_WS_URL: &str = "wss://advanced-trade-ws.coinbase.com";
pub(crate) const COINBASE_WS_URL_ENV: &str = "EARLY_STALE_COINBASE_WS_URL";

pub(crate) fn coinbase_ws_url() -> String {
    std::env::var(COINBASE_WS_URL_ENV).unwrap_or_else(|_| COINBASE_DEFAULT_WS_URL.to_string())
}

pub(crate) fn coinbase_product_id(asset: &str) -> Option<&'static str> {
    match asset.trim().to_ascii_lowercase().as_str() {
        "btc" => Some("BTC-USD"),
        "eth" => Some("ETH-USD"),
        "sol" => Some("SOL-USD"),
        "xrp" => Some("XRP-USD"),
        "doge" | "dogecoin" => Some("DOGE-USD"),
        "bnb" => Some("BNB-USD"),
        "hype" | "hyperliquid" => Some("HYPE-USD"),
        _ => None,
    }
}

pub(crate) fn coinbase_subscription_messages(asset: &str) -> Option<Vec<Value>> {
    let product_id = coinbase_product_id(asset)?;
    Some(vec![
        json!({"type": "subscribe", "product_ids": [product_id], "channel": "ticker"}),
        json!({"type": "subscribe", "product_ids": [product_id], "channel": "market_trades"}),
        json!({"type": "subscribe", "product_ids": [product_id], "channel": "level2"}),
        json!({"type": "subscribe", "product_ids": [product_id], "channel": "heartbeats"}),
    ])
}

pub(crate) fn parse_coinbase_ws_payload(
    payload: &Value,
    asset: &str,
    now_ms: i64,
) -> ParsedCoinbase {
    let channel = payload
        .get("channel")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut parsed = ParsedCoinbase::default();
    let Some(events) = payload.get("events").and_then(Value::as_array) else {
        return parsed;
    };

    for event in events {
        match channel {
            "market_trades" => {
                if let Some(trades) = event.get("trades").and_then(Value::as_array) {
                    parsed.trades.extend(
                        trades
                            .iter()
                            .filter_map(|trade| parse_coinbase_market_trade(trade, asset)),
                    );
                }
            }
            "ticker" => {
                if let Some(tickers) = event.get("tickers").and_then(Value::as_array) {
                    parsed.books.extend(
                        tickers
                            .iter()
                            .filter_map(|ticker| parse_coinbase_ticker(ticker, asset, now_ms)),
                    );
                }
            }
            "level2" | "l2_data" => {
                if let Some(updates) = event.get("updates").and_then(Value::as_array) {
                    if event.get("type").and_then(Value::as_str) == Some("snapshot") {
                        if let Some(book) = parse_coinbase_level2_snapshot(updates, asset, now_ms) {
                            parsed.books.push(book);
                        }
                    } else {
                        parsed.books.extend(updates.iter().filter_map(|update| {
                            parse_coinbase_level2_update(update, asset, now_ms)
                        }));
                    }
                }
            }
            _ => {}
        }
    }
    parsed
}

#[derive(Debug, Default)]
pub(crate) struct ParsedCoinbase {
    pub(crate) trades: Vec<CexTradeSample>,
    pub(crate) books: Vec<CexBookSample>,
}

pub(crate) fn parse_coinbase_market_trade(payload: &Value, asset: &str) -> Option<CexTradeSample> {
    let price = parse_f64(payload.get("price"))?;
    let size = parse_f64(payload.get("size"))?;
    let side = payload.get("side")?.as_str()?.trim().to_ascii_uppercase();
    let timestamp_ms = payload
        .get("time")
        .and_then(Value::as_str)
        .and_then(parse_rfc3339_ms)
        .or_else(|| {
            payload
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(parse_rfc3339_ms)
        })?;
    Some(CexTradeSample {
        venue: CexVenue::Coinbase,
        asset: asset.to_ascii_lowercase(),
        timestamp_ms,
        price,
        size,
        taker_side: match side.as_str() {
            "BUY" => TakerSide::Sell,
            "SELL" => TakerSide::Buy,
            _ => return None,
        },
    })
}

pub(crate) fn parse_coinbase_ticker(
    payload: &Value,
    asset: &str,
    now_ms: i64,
) -> Option<CexBookSample> {
    let bid = parse_f64(
        payload
            .get("best_bid")
            .or_else(|| payload.get("best_bid_price")),
    )?;
    let ask = parse_f64(
        payload
            .get("best_ask")
            .or_else(|| payload.get("best_ask_price")),
    )?;
    valid_bid_ask(bid, ask)?;
    Some(CexBookSample {
        venue: CexVenue::Coinbase,
        asset: asset.to_ascii_lowercase(),
        timestamp_ms: payload
            .get("time")
            .and_then(Value::as_str)
            .and_then(parse_rfc3339_ms)
            .unwrap_or(now_ms),
        bid,
        ask,
        bid_size: parse_f64(
            payload
                .get("best_bid_size")
                .or_else(|| payload.get("best_bid_quantity"))
                .or_else(|| payload.get("bid_size")),
        ),
        ask_size: parse_f64(
            payload
                .get("best_ask_size")
                .or_else(|| payload.get("best_ask_quantity"))
                .or_else(|| payload.get("ask_size")),
        ),
        source: "ticker",
    })
}

pub(crate) fn parse_coinbase_level2_snapshot(
    updates: &[Value],
    asset: &str,
    now_ms: i64,
) -> Option<CexBookSample> {
    let mut best_bid: Option<(f64, f64, i64)> = None;
    let mut best_ask: Option<(f64, f64, i64)> = None;

    for update in updates {
        let side = update.get("side")?.as_str()?.trim().to_ascii_lowercase();
        let price = parse_f64(update.get("price_level").or_else(|| update.get("price")))?;
        let size = parse_f64(update.get("new_quantity").or_else(|| update.get("size")))?;
        if price <= 0.0 || size <= 0.0 {
            continue;
        }
        let timestamp_ms = update
            .get("event_time")
            .or_else(|| update.get("time"))
            .and_then(Value::as_str)
            .and_then(parse_rfc3339_ms)
            .unwrap_or(now_ms);

        match side.as_str() {
            "bid" | "buy" if best_bid.is_none_or(|(best, _, _)| price > best) => {
                best_bid = Some((price, size, timestamp_ms));
            }
            "ask" | "offer" | "sell" if best_ask.is_none_or(|(best, _, _)| price < best) => {
                best_ask = Some((price, size, timestamp_ms));
            }
            _ => {}
        }
    }

    let (bid, bid_size, bid_timestamp_ms) = best_bid?;
    let (ask, ask_size, ask_timestamp_ms) = best_ask?;
    valid_bid_ask(bid, ask)?;
    Some(CexBookSample {
        venue: CexVenue::Coinbase,
        asset: asset.to_ascii_lowercase(),
        timestamp_ms: bid_timestamp_ms.max(ask_timestamp_ms),
        bid,
        ask,
        bid_size: Some(bid_size),
        ask_size: Some(ask_size),
        source: "level2",
    })
}

pub(crate) fn parse_coinbase_level2_update(
    payload: &Value,
    asset: &str,
    now_ms: i64,
) -> Option<CexBookSample> {
    let side = payload.get("side")?.as_str()?.trim().to_ascii_lowercase();
    let price = parse_f64(payload.get("price_level").or_else(|| payload.get("price")))?;
    let size = parse_f64(payload.get("new_quantity").or_else(|| payload.get("size")))?;
    if price <= 0.0 || size <= 0.0 {
        return None;
    }
    let timestamp_ms = payload
        .get("event_time")
        .or_else(|| payload.get("time"))
        .and_then(Value::as_str)
        .and_then(parse_rfc3339_ms)
        .unwrap_or(now_ms);

    match side.as_str() {
        "bid" | "buy" => Some(CexBookSample {
            venue: CexVenue::Coinbase,
            asset: asset.to_ascii_lowercase(),
            timestamp_ms,
            bid: price,
            ask: price,
            bid_size: Some(size),
            ask_size: None,
            source: "level2",
        }),
        "ask" | "offer" | "sell" => Some(CexBookSample {
            venue: CexVenue::Coinbase,
            asset: asset.to_ascii_lowercase(),
            timestamp_ms,
            bid: price,
            ask: price,
            bid_size: None,
            ask_size: Some(size),
            source: "level2",
        }),
        _ => None,
    }
}

fn parse_rfc3339_ms(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc).timestamp_millis())
}

fn valid_bid_ask(bid: f64, ask: f64) -> Option<()> {
    (bid.is_finite() && ask.is_finite() && bid > 0.0 && ask > 0.0 && bid <= ask).then_some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn coinbase_market_trade_side_buy_counts_as_taker_sell() {
        let trade = parse_coinbase_market_trade(
            &json!({"price":"67500.0","size":"0.1","side":"BUY","time":"2026-05-02T00:00:00Z"}),
            "btc",
        )
        .expect("trade");

        assert_eq!(trade.taker_side, TakerSide::Sell);
    }

    #[test]
    fn coinbase_product_supports_bnb_hype_and_doge() {
        assert_eq!(coinbase_product_id("doge"), Some("DOGE-USD"));
        assert_eq!(coinbase_product_id("dogecoin"), Some("DOGE-USD"));
        assert_eq!(coinbase_product_id("bnb"), Some("BNB-USD"));
        assert_eq!(coinbase_product_id("hype"), Some("HYPE-USD"));
    }

    #[test]
    fn coinbase_ticker_parses_quantity_aliases() {
        let book = parse_coinbase_ticker(
            &json!({
                "best_bid": "63570.29",
                "best_ask": "63570.30",
                "best_bid_quantity": "1.06010393",
                "best_ask_quantity": "0.0019254"
            }),
            "btc",
            1_000,
        )
        .expect("ticker book");

        assert_eq!(book.bid_size, Some(1.06010393));
        assert_eq!(book.ask_size, Some(0.0019254));
        assert_eq!(book.timestamp_ms, 1_000);
    }

    #[test]
    fn coinbase_l2_data_snapshot_builds_one_complete_book() {
        let parsed = parse_coinbase_ws_payload(
            &json!({
                "channel": "l2_data",
                "events": [{
                    "type": "snapshot",
                    "updates": [
                        {"side":"bid","event_time":"2026-06-11T23:45:12.78969Z","price_level":"63570.29","new_quantity":"1.0"},
                        {"side":"bid","event_time":"2026-06-11T23:45:12.78969Z","price_level":"63569.99","new_quantity":"3.0"},
                        {"side":"offer","event_time":"2026-06-11T23:45:12.78969Z","price_level":"63570.31","new_quantity":"2.0"},
                        {"side":"offer","event_time":"2026-06-11T23:45:12.78969Z","price_level":"63570.30","new_quantity":"4.0"}
                    ]
                }]
            }),
            "btc",
            1_000,
        );

        assert_eq!(parsed.books.len(), 1);
        let book = &parsed.books[0];
        assert_eq!(book.bid, 63570.29);
        assert_eq!(book.ask, 63570.30);
        assert_eq!(book.bid_size, Some(1.0));
        assert_eq!(book.ask_size, Some(4.0));
        assert_eq!(book.source, "level2");
    }

    #[test]
    fn coinbase_l2_data_update_keeps_partial_update_shape() {
        let parsed = parse_coinbase_ws_payload(
            &json!({
                "channel": "l2_data",
                "events": [{
                    "type": "update",
                    "updates": [
                        {"side":"bid","event_time":"2026-06-11T23:45:12.78969Z","price_level":"63570.29","new_quantity":"1.0"}
                    ]
                }]
            }),
            "btc",
            1_000,
        );

        assert_eq!(parsed.books.len(), 1);
        let book = &parsed.books[0];
        assert_eq!(book.bid, 63570.29);
        assert_eq!(book.ask, 63570.29);
        assert!(book.bid_size.is_some());
        assert!(book.ask_size.is_none());
    }

    #[test]
    fn coinbase_market_trade_side_sell_counts_as_taker_buy() {
        let trade = parse_coinbase_market_trade(
            &json!({"price":"67500.0","size":"0.1","side":"SELL","time":"2026-05-02T00:00:00Z"}),
            "btc",
        )
        .expect("trade");

        assert_eq!(trade.taker_side, TakerSide::Buy);
    }

    #[test]
    fn coinbase_subscription_includes_heartbeat() {
        let channels = coinbase_subscription_messages("btc")
            .expect("messages")
            .into_iter()
            .filter_map(|value| {
                value
                    .get("channel")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect::<Vec<_>>();

        assert!(channels.contains(&"heartbeats".to_string()));
        assert!(channels.contains(&"market_trades".to_string()));
        assert!(channels.contains(&"level2".to_string()));
    }
}
