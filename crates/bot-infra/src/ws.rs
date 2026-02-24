use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WsChannel {
    Market,
    User,
}

#[derive(Debug, Clone)]
pub struct WsEvent {
    pub channel: WsChannel,
    pub payload: Value,
    pub event_type: WsEventType,
    pub market: Option<String>,
    pub order_id: Option<String>,
    pub fill_id: Option<String>,
    pub status: Option<String>,
    pub price: Option<f64>,
    pub size: Option<f64>,
    pub ts: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WsEventType {
    Unknown,
    Subscribed,
    PriceChange,
    Order,
    Fill,
}

#[derive(Debug, Clone)]
pub struct ClobWsClient {
    pub ws_url: String,
    pub max_retries: u8,
    pub retry_backoff_ms: u64,
}

impl ClobWsClient {
    pub fn new(ws_url: String) -> Self {
        Self {
            ws_url,
            max_retries: 3,
            retry_backoff_ms: 300,
        }
    }

    pub async fn subscribe_once(&self, channel: WsChannel, ids: &[String]) -> Result<Vec<WsEvent>> {
        let sub_msg = match channel {
            WsChannel::Market => json!({
                "type": "market",
                "assets_ids": ids,
                "initial_dump": true
            }),
            WsChannel::User => json!({
                "type": "user",
                "markets": ids,
                "initial_dump": true
            }),
        };
        let connect_url = resolve_ws_channel_url(&self.ws_url, channel);

        for attempt in 0..=self.max_retries {
            let (mut socket, _) = match connect_async(&connect_url).await {
                Ok(v) => v,
                Err(e) => {
                    if attempt >= self.max_retries {
                        return Err(e.into());
                    }
                    sleep(Duration::from_millis(self.retry_backoff_ms)).await;
                    continue;
                }
            };

            socket
                .send(Message::Text(sub_msg.to_string().into()))
                .await?;

            let mut out = Vec::new();
            for _ in 0..8 {
                if let Some(msg) = socket.next().await {
                    let msg = msg?;
                    if let Message::Text(text) = msg {
                        if let Ok(v) = serde_json::from_str::<Value>(&text) {
                            out.push(decode_event(channel, v));
                        }
                    }
                } else {
                    break;
                }
            }

            let _ = socket.close(None).await;
            return Ok(out);
        }

        Ok(vec![])
    }
}

fn resolve_ws_channel_url(base_url: &str, channel: WsChannel) -> String {
    let channel_name = match channel {
        WsChannel::Market => "market",
        WsChannel::User => "user",
    };
    let trimmed = base_url.trim_end_matches('/');

    if let Some((prefix, suffix)) = trimmed.rsplit_once("/ws/") {
        if matches!(suffix, "market" | "user" | "rfq") {
            return format!("{prefix}/ws/{channel_name}");
        }
    }

    if trimmed.ends_with("/ws") {
        return format!("{trimmed}/{channel_name}");
    }

    trimmed.to_string()
}

fn decode_event(channel: WsChannel, payload: Value) -> WsEvent {
    let msg_type = payload
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let event_type = match msg_type {
        "subscribed" | "connected" => WsEventType::Subscribed,
        "price_change" | "book" => WsEventType::PriceChange,
        "order" | "order_update" => WsEventType::Order,
        "trade" | "fill" => WsEventType::Fill,
        _ => WsEventType::Unknown,
    };

    WsEvent {
        channel,
        market: payload
            .get("market")
            .or_else(|| payload.get("slug"))
            .or_else(|| payload.get("asset_id"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        order_id: payload
            .get("orderID")
            .or_else(|| payload.get("order_id"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        fill_id: payload
            .get("fillID")
            .or_else(|| payload.get("id"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        status: payload
            .get("status")
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        price: parse_number(payload.get("price")),
        size: parse_number(payload.get("size")),
        ts: payload
            .get("timestamp")
            .or_else(|| payload.get("ts"))
            .and_then(|v| v.as_i64()),
        payload,
        event_type,
    }
}

fn parse_number(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.parse::<f64>().ok(),
        _ => None,
    }
}
