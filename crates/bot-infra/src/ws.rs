use anyhow::Result;
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};
use tokio::{
    sync::{Mutex, Notify, RwLock},
    task::JoinHandle,
    time::{sleep, timeout, Duration},
};
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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MarketDataSnapshot {
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
    pub last_trade_price: Option<f64>,
    pub updated_at_ms: i64,
    pub last_source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketSnapshotWsState {
    Seeded,
    Stale,
    SubscribedUnseeded,
    NotSubscribed,
}

impl MarketSnapshotWsState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Seeded => "live_ws_seeded",
            Self::Stale => "live_ws_stale",
            Self::SubscribedUnseeded => "live_ws_subscribed_unseeded",
            Self::NotSubscribed => "live_ws_not_subscribed",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarketSnapshotIntrospection {
    pub state: MarketSnapshotWsState,
    pub desired: bool,
    pub snapshot: Option<MarketDataSnapshot>,
}

pub type MarketTickCallback = Arc<dyn Fn(&str, &MarketDataSnapshot) + Send + Sync>;

struct ClobWsClientInner {
    ws_url: String,
    max_retries: u8,
    retry_backoff_ms: u64,
    cache: RwLock<HashMap<String, MarketDataSnapshot>>,
    dirty_tokens: Mutex<BTreeSet<String>>,
    desired_tokens: RwLock<BTreeSet<String>>,
    tick_callback: RwLock<Option<MarketTickCallback>>,
    market_update_notify: Notify,
    market_task: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Clone)]
pub struct ClobWsClient {
    inner: Arc<ClobWsClientInner>,
}

impl std::fmt::Debug for ClobWsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClobWsClient")
            .field("ws_url", &self.inner.ws_url)
            .field("max_retries", &self.inner.max_retries)
            .field("retry_backoff_ms", &self.inner.retry_backoff_ms)
            .finish_non_exhaustive()
    }
}

impl ClobWsClient {
    pub fn new(ws_url: String) -> Self {
        Self {
            inner: Arc::new(ClobWsClientInner {
                ws_url,
                max_retries: 3,
                retry_backoff_ms: 300,
                cache: RwLock::new(HashMap::new()),
                dirty_tokens: Mutex::new(BTreeSet::new()),
                desired_tokens: RwLock::new(BTreeSet::new()),
                tick_callback: RwLock::new(None),
                market_update_notify: Notify::new(),
                market_task: Mutex::new(None),
            }),
        }
    }

    pub async fn subscribe_once(&self, channel: WsChannel, ids: &[String]) -> Result<Vec<WsEvent>> {
        let overall_timeout = Duration::from_secs(30);
        match timeout(overall_timeout, self.subscribe_once_inner(channel, ids)).await {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!("WS subscribe_once overall timeout (30s)")),
        }
    }

    pub async fn ensure_market_stream(&self, token_ids: &[String]) -> Result<()> {
        let normalized = normalize_market_tokens(token_ids);
        let mut task_guard = self.inner.market_task.lock().await;

        if normalized.is_empty() {
            self.inner.desired_tokens.write().await.clear();
            self.inner.cache.write().await.clear();
            self.inner.dirty_tokens.lock().await.clear();
            if let Some(handle) = task_guard.take() {
                handle.abort();
            }
            return Ok(());
        }

        let tokens_changed = {
            let desired = self.inner.desired_tokens.read().await;
            *desired != normalized
        };
        let should_spawn = tokens_changed
            || task_guard
                .as_ref()
                .map(|handle| handle.is_finished())
                .unwrap_or(true);

        if !should_spawn {
            return Ok(());
        }

        self.inner
            .desired_tokens
            .write()
            .await
            .clone_from(&normalized);
        self.inner
            .cache
            .write()
            .await
            .retain(|token_id, _| normalized.contains(token_id));

        if let Some(handle) = task_guard.take() {
            handle.abort();
        }

        let client = self.clone();
        let subscribed_tokens: Vec<String> = normalized.into_iter().collect();
        *task_guard = Some(tokio::spawn(async move {
            client.market_stream_loop(subscribed_tokens).await;
        }));

        Ok(())
    }

    pub async fn get_market_snapshot(&self, token_id: &str) -> Option<MarketDataSnapshot> {
        self.inner.cache.read().await.get(token_id).cloned()
    }

    pub async fn get_market_snapshots(
        &self,
        token_ids: &[String],
    ) -> HashMap<String, MarketDataSnapshot> {
        let cache = self.inner.cache.read().await;
        token_ids
            .iter()
            .filter_map(|token_id| {
                cache
                    .get(token_id)
                    .cloned()
                    .map(|snapshot| (token_id.clone(), snapshot))
            })
            .collect()
    }

    pub async fn inspect_market_snapshot(
        &self,
        token_id: &str,
        stale_after_ms: i64,
    ) -> MarketSnapshotIntrospection {
        let desired = self.inner.desired_tokens.read().await.contains(token_id);
        let snapshot = self.inner.cache.read().await.get(token_id).cloned();
        let state = match snapshot.as_ref() {
            Some(snapshot)
                if Utc::now()
                    .timestamp_millis()
                    .saturating_sub(snapshot.updated_at_ms)
                    <= stale_after_ms =>
            {
                MarketSnapshotWsState::Seeded
            }
            Some(_) => MarketSnapshotWsState::Stale,
            None if desired => MarketSnapshotWsState::SubscribedUnseeded,
            None => MarketSnapshotWsState::NotSubscribed,
        };
        MarketSnapshotIntrospection {
            state,
            desired,
            snapshot,
        }
    }

    pub async fn wait_for_market_update(&self) {
        self.inner.market_update_notify.notified().await;
    }

    pub async fn set_tick_callback(&self, cb: MarketTickCallback) {
        *self.inner.tick_callback.write().await = Some(cb);
    }

    pub async fn take_dirty_market_tokens(&self) -> Vec<String> {
        self.inner
            .dirty_tokens
            .lock()
            .await
            .iter()
            .cloned()
            .collect()
    }

    pub async fn clear_dirty_market_tokens(&self, token_ids: &[String]) {
        if token_ids.is_empty() {
            return;
        }
        let token_set: BTreeSet<&str> = token_ids.iter().map(String::as_str).collect();
        let mut dirty_tokens = self.inner.dirty_tokens.lock().await;
        dirty_tokens.retain(|token_id| !token_set.contains(token_id.as_str()));
    }

    async fn subscribe_once_inner(
        &self,
        channel: WsChannel,
        ids: &[String],
    ) -> Result<Vec<WsEvent>> {
        let sub_msg = subscribe_message(channel, ids);
        let connect_url = resolve_ws_channel_url(&self.inner.ws_url, channel);
        let msg_timeout = Duration::from_secs(5);

        for attempt in 0..=self.inner.max_retries {
            let (mut socket, _) = match connect_async(&connect_url).await {
                Ok(v) => v,
                Err(e) => {
                    if attempt >= self.inner.max_retries {
                        return Err(e.into());
                    }
                    sleep(Duration::from_millis(self.inner.retry_backoff_ms)).await;
                    continue;
                }
            };

            socket
                .send(Message::Text(sub_msg.to_string().into()))
                .await?;

            let mut out = Vec::new();
            let max_msgs = (ids.len() * 3).max(16);
            for _ in 0..max_msgs {
                match timeout(msg_timeout, socket.next()).await {
                    Ok(Some(Ok(Message::Text(text)))) => {
                        if let Ok(v) = serde_json::from_str::<Value>(&text) {
                            out.push(decode_event(channel, v));
                        }
                    }
                    Ok(Some(Ok(_))) => {}
                    _ => break,
                }
            }

            let _ = socket.close(None).await;
            return Ok(out);
        }

        Ok(vec![])
    }

    async fn market_stream_loop(&self, subscribed_tokens: Vec<String>) {
        let expected_tokens: BTreeSet<String> = subscribed_tokens.iter().cloned().collect();
        let sub_msg = subscribe_message(WsChannel::Market, &subscribed_tokens);
        let connect_url = resolve_ws_channel_url(&self.inner.ws_url, WsChannel::Market);
        let inactivity_timeout = Duration::from_secs(30);

        loop {
            if self.current_desired_tokens().await != expected_tokens {
                return;
            }

            let (mut socket, _) = match connect_async(&connect_url).await {
                Ok(v) => v,
                Err(err) => {
                    tracing::warn!(error = %err, tokens = subscribed_tokens.len(), "MARKET_STREAM_CONNECT_FAILED");
                    sleep(Duration::from_millis(self.inner.retry_backoff_ms)).await;
                    continue;
                }
            };

            if let Err(err) = socket.send(Message::Text(sub_msg.to_string().into())).await {
                tracing::warn!(error = %err, tokens = subscribed_tokens.len(), "MARKET_STREAM_SUBSCRIBE_FAILED");
                let _ = socket.close(None).await;
                sleep(Duration::from_millis(self.inner.retry_backoff_ms)).await;
                continue;
            }

            loop {
                if self.current_desired_tokens().await != expected_tokens {
                    let _ = socket.close(None).await;
                    return;
                }

                match timeout(inactivity_timeout, socket.next()).await {
                    Ok(Some(Ok(Message::Text(text)))) => {
                        if let Ok(v) = serde_json::from_str::<Value>(&text) {
                            self.update_market_cache_from_payload(&v).await;
                        }
                    }
                    Ok(Some(Ok(Message::Ping(payload)))) => {
                        let _ = socket.send(Message::Pong(payload)).await;
                    }
                    Ok(Some(Ok(Message::Close(_)))) | Ok(None) => break,
                    Ok(Some(Err(err))) => {
                        tracing::warn!(error = %err, tokens = subscribed_tokens.len(), "MARKET_STREAM_READ_FAILED");
                        break;
                    }
                    Err(_) => {
                        tracing::warn!(
                            tokens = subscribed_tokens.len(),
                            "MARKET_STREAM_IDLE_TIMEOUT"
                        );
                        break;
                    }
                    _ => {}
                }
            }

            let _ = socket.close(None).await;
            if self.current_desired_tokens().await != expected_tokens {
                return;
            }
            sleep(Duration::from_millis(self.inner.retry_backoff_ms)).await;
        }
    }

    async fn current_desired_tokens(&self) -> BTreeSet<String> {
        self.inner.desired_tokens.read().await.clone()
    }

    async fn update_market_cache_from_payload(&self, payload: &Value) {
        let fallback_ts = payload
            .get("timestamp")
            .or_else(|| payload.get("ts"))
            .and_then(|value| value.as_i64())
            .unwrap_or_else(|| Utc::now().timestamp_millis());

        let mut updates = Vec::new();
        if let Some(update) =
            extract_snapshot_update(payload, fallback_ts, payload_event_name(payload))
        {
            updates.push(update);
        }

        if let Some(changes) = payload
            .get("price_changes")
            .and_then(|value| value.as_array())
        {
            for change in changes {
                if let Some(update) = extract_snapshot_update(change, fallback_ts, "price_changes")
                {
                    updates.push(update);
                }
            }
        }

        if updates.is_empty() {
            return;
        }

        let tick_cb = self.inner.tick_callback.read().await.clone();
        let mut updated_tokens = BTreeSet::new();
        let mut tick_snapshots = Vec::new();
        let mut cache = self.inner.cache.write().await;
        for update in updates {
            let token_id = update.token_id.clone();
            updated_tokens.insert(token_id.clone());
            let entry = cache.entry(token_id.clone()).or_default();
            if update.best_bid.is_some() {
                entry.best_bid = update.best_bid;
            }
            if update.best_ask.is_some() {
                entry.best_ask = update.best_ask;
            }
            if update.last_trade_price.is_some() {
                entry.last_trade_price = update.last_trade_price;
            }
            entry.updated_at_ms = update.updated_at_ms;
            entry.last_source = update.source.to_string();
            if tick_cb.is_some() {
                tick_snapshots.push((token_id, entry.clone()));
            }
        }
        drop(cache);

        if let Some(ref cb) = tick_cb {
            for (token_id, snapshot) in tick_snapshots {
                cb(&token_id, &snapshot);
            }
        }

        if updated_tokens.is_empty() {
            return;
        }

        let mut dirty_tokens = self.inner.dirty_tokens.lock().await;
        dirty_tokens.extend(updated_tokens);
        drop(dirty_tokens);
        self.inner.market_update_notify.notify_one();
    }
}

#[derive(Debug)]
struct MarketSnapshotUpdate {
    token_id: String,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    last_trade_price: Option<f64>,
    updated_at_ms: i64,
    source: String,
}

fn normalize_market_tokens(ids: &[String]) -> BTreeSet<String> {
    ids.iter()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn subscribe_message(channel: WsChannel, ids: &[String]) -> Value {
    match channel {
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
    let msg_type = payload_event_name(&payload);
    let event_type = match msg_type {
        "subscribed" | "connected" => WsEventType::Subscribed,
        "price_change" | "book" | "best_bid_ask" | "last_trade_price" => WsEventType::PriceChange,
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

fn payload_event_name(payload: &Value) -> &str {
    payload
        .get("event_type")
        .or_else(|| payload.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
}

fn extract_snapshot_update(
    payload: &Value,
    fallback_ts: i64,
    default_source: &str,
) -> Option<MarketSnapshotUpdate> {
    let token_id = payload
        .get("asset_id")
        .or_else(|| payload.get("assetId"))
        .or_else(|| payload.get("market"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.starts_with("0x"))?
        .to_string();

    let best_bid = parse_number(
        payload
            .get("best_bid")
            .or_else(|| payload.get("bestBid"))
            .or_else(|| payload.get("bid")),
    );
    let best_ask = parse_number(
        payload
            .get("best_ask")
            .or_else(|| payload.get("bestAsk"))
            .or_else(|| payload.get("ask")),
    );
    let raw_price = parse_number(payload.get("price"));
    let event_name = payload_event_name(payload);
    let last_trade_price = if event_name == "last_trade_price"
        || event_name == "trade"
        || event_name == "fill"
        || event_name == "price_changes"
    {
        raw_price
    } else {
        None
    };

    if best_bid.is_none() && best_ask.is_none() && last_trade_price.is_none() {
        return None;
    }

    let updated_at_ms = payload
        .get("timestamp")
        .or_else(|| payload.get("ts"))
        .and_then(|value| value.as_i64())
        .unwrap_or(fallback_ts);

    Some(MarketSnapshotUpdate {
        token_id,
        best_bid,
        best_ask,
        last_trade_price,
        updated_at_ms,
        source: if event_name == "unknown" {
            default_source.to_string()
        } else {
            event_name.to_string()
        },
    })
}

fn parse_number(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};

    #[tokio::test]
    async fn market_cache_updates_book_and_last_trade_fields() {
        let ws = ClobWsClient::new("wss://example.com/ws".to_string());
        ws.update_market_cache_from_payload(&json!({
            "event_type": "book",
            "asset_id": "tok-yes",
            "best_bid": "0.77",
            "best_ask": "0.81",
            "timestamp": 12345
        }))
        .await;
        ws.update_market_cache_from_payload(&json!({
            "event_type": "last_trade_price",
            "asset_id": "tok-yes",
            "price": "0.79",
            "timestamp": 12346
        }))
        .await;

        let snapshot = ws.get_market_snapshot("tok-yes").await.unwrap();
        assert_eq!(snapshot.best_bid, Some(0.77));
        assert_eq!(snapshot.best_ask, Some(0.81));
        assert_eq!(snapshot.last_trade_price, Some(0.79));
        assert_eq!(snapshot.updated_at_ms, 12346);
        assert_eq!(snapshot.last_source, "last_trade_price");
    }

    #[tokio::test]
    async fn dirty_tokens_are_deduped_and_clearable() {
        let ws = ClobWsClient::new("wss://example.com/ws".to_string());
        ws.update_market_cache_from_payload(&json!({
            "event_type": "book",
            "asset_id": "tok-yes",
            "best_bid": "0.77",
            "best_ask": "0.81",
            "timestamp": 12345
        }))
        .await;
        ws.update_market_cache_from_payload(&json!({
            "event_type": "last_trade_price",
            "asset_id": "tok-yes",
            "price": "0.79",
            "timestamp": 12346
        }))
        .await;
        ws.update_market_cache_from_payload(&json!({
            "event_type": "book",
            "asset_id": "tok-no",
            "best_bid": "0.21",
            "best_ask": "0.23",
            "timestamp": 12347
        }))
        .await;

        let dirty_before_clear = ws.take_dirty_market_tokens().await;
        assert_eq!(
            dirty_before_clear,
            vec!["tok-no".to_string(), "tok-yes".to_string()]
        );

        ws.clear_dirty_market_tokens(&["tok-yes".to_string()]).await;
        let dirty_after_clear = ws.take_dirty_market_tokens().await;
        assert_eq!(dirty_after_clear, vec!["tok-no".to_string()]);
    }

    #[tokio::test]
    async fn tick_callback_receives_intermediate_snapshots_after_cache_updates() {
        let ws = ClobWsClient::new("wss://example.com/ws".to_string());
        let seen = Arc::new(StdMutex::new(Vec::<(String, MarketDataSnapshot)>::new()));
        let seen_cb = Arc::clone(&seen);
        ws.set_tick_callback(Arc::new(move |token_id, snapshot| {
            seen_cb
                .lock()
                .expect("tick callback mutex")
                .push((token_id.to_string(), snapshot.clone()));
        }))
        .await;

        ws.update_market_cache_from_payload(&json!({
            "timestamp": 12345,
            "price_changes": [
                {
                    "asset_id": "tok-yes",
                    "best_bid": "0.70",
                    "best_ask": "0.72",
                    "timestamp": 12345
                },
                {
                    "asset_id": "tok-yes",
                    "event_type": "last_trade_price",
                    "price": "0.68",
                    "timestamp": 12346
                }
            ]
        }))
        .await;

        let seen = seen.lock().expect("tick callback mutex");
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].0, "tok-yes");
        assert_eq!(seen[0].1.best_bid, Some(0.70));
        assert_eq!(seen[0].1.last_trade_price, None);
        assert_eq!(seen[1].0, "tok-yes");
        assert_eq!(seen[1].1.best_bid, Some(0.70));
        assert_eq!(seen[1].1.last_trade_price, Some(0.68));
        assert_eq!(seen[1].1.updated_at_ms, 12346);
    }

    #[tokio::test]
    async fn inspect_market_snapshot_reports_subscribed_unseeded() {
        let ws = ClobWsClient::new("wss://example.com/ws".to_string());
        ws.inner
            .desired_tokens
            .write()
            .await
            .insert("tok-yes".to_string());

        let inspection = ws.inspect_market_snapshot("tok-yes", 250).await;
        assert_eq!(inspection.state, MarketSnapshotWsState::SubscribedUnseeded);
        assert!(inspection.desired);
        assert!(inspection.snapshot.is_none());
    }

    #[tokio::test]
    async fn inspect_market_snapshot_reports_seeded_and_stale() {
        let ws = ClobWsClient::new("wss://example.com/ws".to_string());
        ws.inner
            .desired_tokens
            .write()
            .await
            .insert("tok-yes".to_string());
        ws.update_market_cache_from_payload(&json!({
            "event_type": "book",
            "asset_id": "tok-yes",
            "best_bid": "0.77",
            "best_ask": "0.81",
            "timestamp": Utc::now().timestamp_millis()
        }))
        .await;

        let seeded = ws.inspect_market_snapshot("tok-yes", 250).await;
        assert_eq!(seeded.state, MarketSnapshotWsState::Seeded);
        assert!(seeded.snapshot.is_some());

        ws.update_market_cache_from_payload(&json!({
            "event_type": "book",
            "asset_id": "tok-yes",
            "best_bid": "0.77",
            "best_ask": "0.81",
            "timestamp": Utc::now().timestamp_millis() - 5_000
        }))
        .await;

        let stale = ws.inspect_market_snapshot("tok-yes", 250).await;
        assert_eq!(stale.state, MarketSnapshotWsState::Stale);
        assert!(stale.snapshot.is_some());
    }
}
