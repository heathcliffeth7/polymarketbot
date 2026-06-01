use super::types::{parse_f64, parse_i64, CexBookSample, CexVenue};
use serde_json::{json, Value};

pub(crate) const BYBIT_DEFAULT_WS_URL: &str = "wss://stream.bybit.com/v5/public/spot";
pub(crate) const BYBIT_WS_URL_ENV: &str = "EARLY_STALE_BYBIT_WS_URL";

pub(crate) fn bybit_ws_url() -> String {
    std::env::var(BYBIT_WS_URL_ENV).unwrap_or_else(|_| BYBIT_DEFAULT_WS_URL.to_string())
}

pub(crate) fn bybit_symbol(asset: &str) -> Option<&'static str> {
    match asset.trim().to_ascii_lowercase().as_str() {
        "btc" => Some("BTCUSDT"),
        "eth" => Some("ETHUSDT"),
        "sol" => Some("SOLUSDT"),
        "xrp" => Some("XRPUSDT"),
        _ => None,
    }
}

pub(crate) fn bybit_subscription_message(asset: &str) -> Option<Value> {
    let symbol = bybit_symbol(asset)?;
    Some(json!({
        "op": "subscribe",
        "args": [format!("orderbook.1.{symbol}")],
    }))
}

pub(crate) fn parse_bybit_ws_payload(
    payload: &Value,
    asset: &str,
    now_ms: i64,
) -> Option<CexBookSample> {
    let topic = payload.get("topic").and_then(Value::as_str)?;
    if !topic.starts_with("orderbook.1.") {
        return None;
    }
    parse_bybit_orderbook(
        payload.get("data").unwrap_or(payload),
        payload,
        asset,
        now_ms,
    )
}

pub(crate) fn parse_bybit_orderbook(
    data: &Value,
    envelope: &Value,
    asset: &str,
    now_ms: i64,
) -> Option<CexBookSample> {
    let best_bid = data
        .get("b")?
        .as_array()?
        .iter()
        .filter_map(parse_level)
        .max_by(|left, right| left.0.total_cmp(&right.0))?;
    let best_ask = data
        .get("a")?
        .as_array()?
        .iter()
        .filter_map(parse_level)
        .min_by(|left, right| left.0.total_cmp(&right.0))?;
    valid_bid_ask(best_bid.0, best_ask.0)?;
    Some(CexBookSample {
        venue: CexVenue::Bybit,
        asset: asset.to_ascii_lowercase(),
        timestamp_ms: parse_i64(envelope.get("cts").or_else(|| envelope.get("ts")))
            .or_else(|| parse_i64(data.get("cts").or_else(|| data.get("ts"))))
            .unwrap_or(now_ms),
        bid: best_bid.0,
        ask: best_ask.0,
        bid_size: Some(best_bid.1),
        ask_size: Some(best_ask.1),
        source: "orderbook.1",
    })
}

fn parse_level(value: &Value) -> Option<(f64, f64)> {
    let row = value.as_array()?;
    let price = parse_f64(row.first())?;
    let size = parse_f64(row.get(1))?;
    (price.is_finite() && price > 0.0 && size.is_finite() && size >= 0.0).then_some((price, size))
}

fn valid_bid_ask(bid: f64, ask: f64) -> Option<()> {
    (bid.is_finite() && ask.is_finite() && bid > 0.0 && ask > 0.0 && bid <= ask).then_some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bybit_subscription_uses_spot_orderbook_level_1() {
        let message = bybit_subscription_message("btc").expect("subscription");

        assert_eq!(message["op"], "subscribe");
        assert_eq!(message["args"][0], "orderbook.1.BTCUSDT");
    }

    #[test]
    fn bybit_orderbook_extracts_top_of_book() {
        let book = parse_bybit_ws_payload(
            &json!({
                "topic": "orderbook.1.BTCUSDT",
                "ts": 1774013100123i64,
                "data": {
                    "s": "BTCUSDT",
                    "b": [["67499.0", "1.0"], ["67500.0", "0.5"]],
                    "a": [["67502.0", "0.4"], ["67501.0", "0.7"]]
                }
            }),
            "btc",
            999,
        )
        .expect("book");

        assert_eq!(book.venue, CexVenue::Bybit);
        assert_eq!(book.bid, 67500.0);
        assert_eq!(book.ask, 67501.0);
        assert_eq!(book.bid_size, Some(0.5));
        assert_eq!(book.ask_size, Some(0.7));
        assert_eq!(book.timestamp_ms, 1774013100123);
        assert_eq!(book.source, "orderbook.1");
    }
}
