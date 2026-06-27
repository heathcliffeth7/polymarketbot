use anyhow::{anyhow, Context, Result};
use reqwest::Url;
use std::{
    sync::atomic::{AtomicI64, Ordering},
    time::Duration,
};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    client_async_tls_with_config,
    tungstenite::{
        client::IntoClientRequest,
        handshake::client::{Request, Response},
        http::{
            header::{CACHE_CONTROL, PRAGMA, USER_AGENT},
            HeaderValue,
        },
    },
    MaybeTlsStream, WebSocketStream,
};

const BACKOFF_INITIAL_MS: u64 = 2_000;
const BACKOFF_MAX_MS: u64 = 60_000;
const HEADERS_MODE_BROWSER_COMPATIBLE: &str = "browser_compatible";
const LIVE_DATA_WS_USER_AGENT_ENV: &str = "POLYMARKET_LIVE_DATA_WS_USER_AGENT";
const DEFAULT_LIVE_DATA_WS_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0 Safari/537.36";

static ACTIVE_LIVE_DATA_WS_CONNECTIONS: AtomicI64 = AtomicI64::new(0);

pub(crate) struct PolymarketLiveDataWsConnection {
    pub(crate) ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    pub(crate) info: PolymarketLiveDataWsConnectionInfo,
    pub(crate) active_guard: LiveDataWsActiveConnection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolymarketLiveDataWsConnectionInfo {
    pub(crate) proxy_mode: &'static str,
    pub(crate) proxy_configured: bool,
    pub(crate) headers_mode: &'static str,
    pub(crate) target_host: String,
    pub(crate) target_port: u16,
    pub(crate) active_connections: i64,
}

pub(crate) struct LiveDataWsActiveConnection;

impl Drop for LiveDataWsActiveConnection {
    fn drop(&mut self) {
        ACTIVE_LIVE_DATA_WS_CONNECTIONS.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveDataWsErrorInfo {
    pub(crate) error_class: &'static str,
    pub(crate) http_status: Option<u16>,
    pub(crate) proxy_mode: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub(crate) struct LiveDataWsBackoff {
    owner_salt: u64,
    attempt: u32,
}

impl LiveDataWsBackoff {
    pub(crate) fn new(owner: &str) -> Self {
        Self {
            owner_salt: stable_hash(owner.as_bytes()),
            attempt: 0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.attempt = 0;
    }

    pub(crate) fn next_delay(&mut self, session_id: u64) -> Duration {
        let shift = self.attempt.min(5);
        let base = BACKOFF_INITIAL_MS.saturating_mul(1_u64 << shift);
        let capped = base.min(BACKOFF_MAX_MS);
        let jitter_seed =
            self.owner_salt ^ session_id ^ ((self.attempt as u64).wrapping_mul(0x9e37_79b9));
        let jitter_percent = 80 + (stable_hash(&jitter_seed.to_le_bytes()) % 41);
        self.attempt = self.attempt.saturating_add(1);
        Duration::from_millis((capped * jitter_percent / 100).min(BACKOFF_MAX_MS))
    }
}

pub(crate) async fn connect_polymarket_live_data_ws(
    owner: &'static str,
    session_id: u64,
    ws_url: &str,
) -> Result<PolymarketLiveDataWsConnection> {
    let url = Url::parse(ws_url).with_context(|| format!("parsing live data websocket url"))?;
    let target_host = url
        .host_str()
        .ok_or_else(|| anyhow!("live data websocket url missing host"))?
        .to_string();
    let target_port = url
        .port_or_known_default()
        .ok_or_else(|| anyhow!("live data websocket url missing port"))?;
    let proxy_configured = bot_infra::proxy::socks5_proxy_configured();

    tracing::info!(
        session_id,
        connection_owner = owner,
        proxy_mode = if proxy_configured { "socks5" } else { "direct" },
        proxy_configured,
        headers_mode = HEADERS_MODE_BROWSER_COMPATIBLE,
        live_data_ws_proxy_supported = true,
        target_host = %target_host,
        target_port,
        "POLYMARKET_LIVE_DATA_WS_CONNECTING"
    );

    let (stream, proxy_info) = bot_infra::proxy::connect_tcp_with_optional_socks5_proxy(
        &target_host,
        target_port,
    )
    .await
    .with_context(|| {
        format!(
            "proxy_connect_failed proxy_mode={} target_host={target_host} target_port={target_port}",
            if proxy_configured { "socks5" } else { "direct" }
        )
    })?;
    stream
        .set_nodelay(true)
        .context("setting live data websocket TCP_NODELAY")?;
    let request = build_live_data_ws_request(ws_url)?;
    let (ws, _response): (WebSocketStream<MaybeTlsStream<TcpStream>>, Response) =
        client_async_tls_with_config(request, stream, None, None)
            .await
            .with_context(|| {
                format!(
                    "ws_handshake_failed proxy_mode={proxy_mode} headers_mode={HEADERS_MODE_BROWSER_COMPATIBLE} target_host={target_host} target_port={target_port}"
                    ,
                    proxy_mode = proxy_info.proxy_mode
                )
            })?;
    let active_connections = ACTIVE_LIVE_DATA_WS_CONNECTIONS.fetch_add(1, Ordering::SeqCst) + 1;

    Ok(PolymarketLiveDataWsConnection {
        ws,
        info: PolymarketLiveDataWsConnectionInfo {
            proxy_mode: proxy_info.proxy_mode,
            proxy_configured: proxy_info.proxy_configured,
            headers_mode: HEADERS_MODE_BROWSER_COMPATIBLE,
            target_host,
            target_port,
            active_connections,
        },
        active_guard: LiveDataWsActiveConnection,
    })
}

pub(crate) fn classify_live_data_ws_error_text(error_text: &str) -> LiveDataWsErrorInfo {
    let http_status = extract_http_status(error_text);
    let error_class = if http_status.is_some() {
        "http_status"
    } else if error_text.contains("watched_chainlink_symbol_no_tick_timeout") {
        "watched_chainlink_symbol_no_tick_timeout"
    } else if error_text.contains("subscription_no_chainlink_tick_timeout") {
        "subscription_no_chainlink_tick_timeout"
    } else if error_text.contains("subscription_no_tick_timeout") {
        "subscription_no_tick_timeout"
    } else if error_text.contains("proxy_parse_error") {
        "proxy_parse"
    } else if error_text.contains("proxy_connect_failed") || error_text.contains("socks5") {
        "proxy_connect"
    } else if error_text.contains("ws_handshake_failed") || error_text.contains("WebSocket") {
        "ws_handshake"
    } else if error_text.contains("TLS") || error_text.contains("tls") {
        "tls"
    } else if error_text.contains("parse") || error_text.contains("payload") {
        "parse"
    } else {
        "io"
    };
    LiveDataWsErrorInfo {
        error_class,
        http_status,
        proxy_mode: extract_proxy_mode(error_text),
    }
}

pub(crate) fn classify_live_data_ws_error(error: &anyhow::Error) -> LiveDataWsErrorInfo {
    classify_live_data_ws_error_text(&format!("{error:#}"))
}

pub(crate) fn live_data_ws_error_cache_summary(error: &anyhow::Error) -> String {
    let text = error.to_string();
    let info = classify_live_data_ws_error(error);
    format!(
        "{text}; error_class={}; http_status={}; proxy_mode={}",
        info.error_class,
        info.http_status
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        info.proxy_mode.unwrap_or("unknown")
    )
}

pub(crate) fn active_live_data_ws_connections() -> i64 {
    ACTIVE_LIVE_DATA_WS_CONNECTIONS.load(Ordering::SeqCst)
}

pub(crate) fn build_live_data_ws_request(ws_url: &str) -> Result<Request> {
    let user_agent = std::env::var(LIVE_DATA_WS_USER_AGENT_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_LIVE_DATA_WS_USER_AGENT.to_string());
    build_live_data_ws_request_with_user_agent(ws_url, &user_agent)
}

fn build_live_data_ws_request_with_user_agent(ws_url: &str, user_agent: &str) -> Result<Request> {
    let mut request = ws_url
        .into_client_request()
        .with_context(|| format!("building live data websocket request"))?;
    let headers = request.headers_mut();
    headers.insert("Origin", HeaderValue::from_static("https://polymarket.com"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(user_agent)
            .context("building live data websocket user-agent header")?,
    );
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(PRAGMA, HeaderValue::from_static("no-cache"));
    Ok(request)
}

fn extract_http_status(text: &str) -> Option<u16> {
    let marker = "HTTP error: ";
    let start = text.find(marker)? + marker.len();
    text[start..]
        .split_whitespace()
        .next()
        .and_then(|value| value.parse().ok())
}

fn extract_proxy_mode(text: &str) -> Option<&'static str> {
    if text.contains("proxy_mode=socks5") {
        Some("socks5")
    } else if text.contains("proxy_mode=direct") {
        Some("direct")
    } else {
        None
    }
}

fn stable_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x1000_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_http_status_and_proxy_mode_from_error_chain() {
        let info = classify_live_data_ws_error_text(
            "ws_handshake_failed proxy_mode=socks5: HTTP error: 429 Too Many Requests",
        );

        assert_eq!(info.error_class, "http_status");
        assert_eq!(info.http_status, Some(429));
        assert_eq!(info.proxy_mode, Some("socks5"));
    }

    #[test]
    fn browser_compatible_request_adds_expected_headers() {
        let request = build_live_data_ws_request_with_user_agent(
            "wss://ws-live-data.polymarket.com",
            "test-agent",
        )
        .expect("request");

        assert_eq!(
            request
                .headers()
                .get("Origin")
                .and_then(|value| value.to_str().ok()),
            Some("https://polymarket.com")
        );
        assert_eq!(
            request
                .headers()
                .get(USER_AGENT)
                .and_then(|value| value.to_str().ok()),
            Some("test-agent")
        );
        assert_eq!(
            request
                .headers()
                .get(CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-cache")
        );
        assert_eq!(
            request
                .headers()
                .get(PRAGMA)
                .and_then(|value| value.to_str().ok()),
            Some("no-cache")
        );
    }

    #[test]
    fn classifies_subscription_no_tick_timeout() {
        let info = classify_live_data_ws_error_text(
            "subscription_no_tick_timeout after 10s without valid live-data tick",
        );

        assert_eq!(info.error_class, "subscription_no_tick_timeout");
        assert_eq!(info.http_status, None);
    }

    #[test]
    fn classifies_subscription_no_chainlink_tick_timeout() {
        let info = classify_live_data_ws_error_text(
            "subscription_no_chainlink_tick_timeout after 10s without valid Chainlink live-data tick",
        );

        assert_eq!(info.error_class, "subscription_no_chainlink_tick_timeout");
        assert_eq!(info.http_status, None);
    }

    #[test]
    fn classifies_watched_chainlink_symbol_no_tick_timeout() {
        let info = classify_live_data_ws_error_text(
            "watched_chainlink_symbol_no_tick_timeout symbol=sol/usd after 10s without watched Chainlink tick",
        );

        assert_eq!(info.error_class, "watched_chainlink_symbol_no_tick_timeout");
        assert_eq!(info.http_status, None);
    }

    #[test]
    fn backoff_grows_caps_and_uses_owner_jitter() {
        let mut chainlink = LiveDataWsBackoff::new("chainlink");
        let mut binance = LiveDataWsBackoff::new("binance");

        let first = chainlink.next_delay(1).as_millis();
        let second = chainlink.next_delay(1).as_millis();
        let other_owner = binance.next_delay(1).as_millis();
        for _ in 0..10 {
            chainlink.next_delay(1);
        }
        let capped = chainlink.next_delay(1).as_millis();

        assert!(second > first);
        assert_ne!(first, other_owner);
        assert!(capped <= u128::from(BACKOFF_MAX_MS));
        chainlink.reset();
        assert!(chainlink.next_delay(1).as_millis() < second);
    }
}
