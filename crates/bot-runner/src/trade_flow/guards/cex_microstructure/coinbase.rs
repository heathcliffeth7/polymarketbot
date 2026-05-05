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
            "level2" => {
                if let Some(updates) = event.get("updates").and_then(Value::as_array) {
                    parsed.books.extend(
                        updates.iter().filter_map(|update| {
                            parse_coinbase_level2_update(update, asset, now_ms)
                        }),
                    );
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
                .or_else(|| payload.get("bid_size")),
        ),
        ask_size: parse_f64(
            payload
                .get("best_ask_size")
                .or_else(|| payload.get("ask_size")),
        ),
        source: "ticker",
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
    if price <= 0.0 || size < 0.0 {
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
