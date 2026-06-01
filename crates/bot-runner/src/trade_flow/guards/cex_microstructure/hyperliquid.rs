use super::types::{parse_f64, parse_i64, CexBookSample, CexVenue};
use serde_json::{json, Value};

pub(crate) const HYPERLIQUID_DEFAULT_WS_URL: &str = "wss://api.hyperliquid.xyz/ws";
pub(crate) const HYPERLIQUID_WS_URL_ENV: &str = "EARLY_STALE_HYPERLIQUID_WS_URL";

pub(crate) fn hyperliquid_ws_url() -> String {
    std::env::var(HYPERLIQUID_WS_URL_ENV).unwrap_or_else(|_| HYPERLIQUID_DEFAULT_WS_URL.to_string())
}

pub(crate) fn hyperliquid_coin(asset: &str) -> Option<&'static str> {
    match asset.trim().to_ascii_lowercase().as_str() {
        "btc" => Some("BTC"),
        "eth" => Some("ETH"),
        "sol" => Some("SOL"),
        _ => None,
    }
}

pub(crate) fn hyperliquid_subscription_message(asset: &str) -> Option<Value> {
    let coin = hyperliquid_coin(asset)?;
    Some(json!({
        "method": "subscribe",
        "subscription": {
            "type": "l2Book",
            "coin": coin,
        }
    }))
}

pub(crate) fn parse_hyperliquid_ws_payload(
    payload: &Value,
    asset: &str,
    now_ms: i64,
) -> Option<CexBookSample> {
    let channel = payload.get("channel").and_then(Value::as_str);
    if channel.is_some_and(|channel| channel != "l2Book") {
        return None;
    }
    parse_hyperliquid_l2_book(payload.get("data").unwrap_or(payload), asset, now_ms)
}

pub(crate) fn parse_hyperliquid_l2_book(
    payload: &Value,
    asset: &str,
    now_ms: i64,
) -> Option<CexBookSample> {
    let levels = payload.get("levels")?.as_array()?;
    let bids = levels.first()?.as_array()?;
    let asks = levels.get(1)?.as_array()?;
    let best_bid = bids
        .iter()
        .filter_map(parse_hyperliquid_level)
        .max_by(|left, right| left.0.total_cmp(&right.0))?;
    let best_ask = asks
        .iter()
        .filter_map(parse_hyperliquid_level)
        .min_by(|left, right| left.0.total_cmp(&right.0))?;
    valid_bid_ask(best_bid.0, best_ask.0)?;
    Some(CexBookSample {
        venue: CexVenue::Hyperliquid,
        asset: asset.to_ascii_lowercase(),
        timestamp_ms: parse_i64(payload.get("time")).unwrap_or(now_ms),
        bid: best_bid.0,
        ask: best_ask.0,
        bid_size: Some(best_bid.1),
        ask_size: Some(best_ask.1),
        source: "l2Book",
    })
}

fn parse_hyperliquid_level(value: &Value) -> Option<(f64, f64)> {
    if let Some(row) = value.as_array() {
        let price = parse_f64(row.first())?;
        let size = parse_f64(row.get(1))?;
        return valid_level(price, size);
    }

    let price = parse_f64(value.get("px").or_else(|| value.get("price")))?;
    let size = parse_f64(value.get("sz").or_else(|| value.get("size")))?;
    valid_level(price, size)
}

fn valid_level(price: f64, size: f64) -> Option<(f64, f64)> {
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
    fn hyperliquid_subscription_uses_l2_book() {
        let message = hyperliquid_subscription_message("btc").expect("subscription");

        assert_eq!(message["method"], "subscribe");
        assert_eq!(message["subscription"]["type"], "l2Book");
        assert_eq!(message["subscription"]["coin"], "BTC");
    }

    #[test]
    fn hyperliquid_l2_book_extracts_top_of_book() {
        let book = parse_hyperliquid_ws_payload(
            &json!({
                "channel": "l2Book",
                "data": {
                    "coin": "BTC",
                    "time": 1774013100123i64,
                    "levels": [
                        [
                            { "px": "67499.0", "sz": "1.0", "n": 1 },
                            { "px": "67500.0", "sz": "0.5", "n": 1 }
                        ],
                        [
                            { "px": "67502.0", "sz": "0.4", "n": 1 },
                            { "px": "67501.0", "sz": "0.7", "n": 1 }
                        ]
                    ]
                }
            }),
            "btc",
            999,
        )
        .expect("book");

        assert_eq!(book.venue, CexVenue::Hyperliquid);
        assert_eq!(book.bid, 67500.0);
        assert_eq!(book.ask, 67501.0);
        assert_eq!(book.bid_size, Some(0.5));
        assert_eq!(book.ask_size, Some(0.7));
        assert_eq!(book.timestamp_ms, 1774013100123);
        assert_eq!(book.source, "l2Book");
    }
}
