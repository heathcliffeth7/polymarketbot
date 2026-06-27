use anyhow::{anyhow, Context, Result};
use reqwest::{ClientBuilder, Proxy, Url};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

const PROXY_POOL_ENV: &str = "SOCKS5_PROXY_URLS";
const LEGACY_PROXY_ENV: &str = "SOCKS5_PROXY_URL";
const ORDER_PROXY_ENV: &str = "ORDER_SOCKS5_PROXY_URL";
const SOCKS_VERSION: u8 = 0x05;
const SOCKS_AUTH_VERSION: u8 = 0x01;
const SOCKS_METHOD_NO_AUTH: u8 = 0x00;
const SOCKS_METHOD_USER_PASS: u8 = 0x02;
const SOCKS_METHOD_NONE: u8 = 0xff;
const SOCKS_CMD_CONNECT: u8 = 0x01;
const SOCKS_ATYP_DOMAIN: u8 = 0x03;
const SOCKS_REPLY_OK: u8 = 0x00;

static PROXY_CURSOR: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
pub struct Socks5ProxyConfig {
    url: Url,
    host: String,
    port: u16,
    username: Option<String>,
    password: Option<String>,
    redacted: String,
}

impl std::fmt::Debug for Socks5ProxyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Socks5ProxyConfig")
            .field("redacted", &self.redacted)
            .finish()
    }
}

impl Socks5ProxyConfig {
    fn parse(value: &str) -> Result<Self> {
        let url = Url::parse(value).context("proxy_parse_error parsing SOCKS5 proxy url")?;
        anyhow::ensure!(
            matches!(url.scheme(), "socks5" | "socks5h"),
            "proxy_parse_error unsupported SOCKS5 proxy scheme"
        );
        let host = url
            .host_str()
            .ok_or_else(|| anyhow!("proxy_parse_error SOCKS5 proxy missing host"))?
            .to_string();
        let port = url.port().unwrap_or(1080);
        let username = (!url.username().is_empty()).then(|| url.username().to_string());
        let password = url.password().map(ToString::to_string);
        Ok(Self {
            redacted: format!("{}://{}:{port}", url.scheme(), host),
            url,
            host,
            port,
            username,
            password,
        })
    }

    fn has_credentials(&self) -> bool {
        self.username.is_some() || self.password.is_some()
    }

    pub fn redacted(&self) -> &str {
        &self.redacted
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyConnectionInfo {
    pub proxy_mode: &'static str,
    pub proxy_configured: bool,
    pub proxy_redacted: Option<String>,
}

pub fn add_rotating_reqwest_proxy(builder: ClientBuilder, context: &'static str) -> ClientBuilder {
    let proxies = configured_socks5_proxies()
        .unwrap_or_else(|err| panic!("{context} SOCKS5 proxy config invalid: {err:#}"));
    if proxies.is_empty() {
        return builder;
    }

    builder.proxy(Proxy::custom(move |_| {
        next_socks5_proxy_url().ok().flatten()
    }))
}

pub fn add_order_reqwest_proxy(builder: ClientBuilder, context: &'static str) -> ClientBuilder {
    match configured_order_socks5_proxy()
        .unwrap_or_else(|err| panic!("{context} order SOCKS5 proxy config invalid: {err:#}"))
    {
        Some(proxy) => builder.proxy(Proxy::all(proxy.url).expect("valid order SOCKS5 proxy")),
        None => builder,
    }
}

pub fn socks5_proxy_configured() -> bool {
    configured_socks5_proxies()
        .map(|proxies| !proxies.is_empty())
        .unwrap_or(true)
}

pub fn next_socks5_proxy() -> Result<Option<Socks5ProxyConfig>> {
    let proxies = configured_socks5_proxies()?;
    if proxies.is_empty() {
        return Ok(None);
    }
    let index = PROXY_CURSOR.fetch_add(1, Ordering::Relaxed) % proxies.len();
    Ok(Some(proxies[index].clone()))
}

pub async fn connect_tcp_with_optional_socks5_proxy(
    target_host: &str,
    target_port: u16,
) -> Result<(TcpStream, ProxyConnectionInfo)> {
    match next_socks5_proxy()? {
        Some(proxy) => {
            let redacted = proxy.redacted().to_string();
            let stream = connect_socks5(proxy, target_host, target_port).await?;
            Ok((
                stream,
                ProxyConnectionInfo {
                    proxy_mode: "socks5",
                    proxy_configured: true,
                    proxy_redacted: Some(redacted),
                },
            ))
        }
        None => {
            let stream = TcpStream::connect((target_host, target_port)).await?;
            Ok((
                stream,
                ProxyConnectionInfo {
                    proxy_mode: "direct",
                    proxy_configured: false,
                    proxy_redacted: None,
                },
            ))
        }
    }
}

fn next_socks5_proxy_url() -> Result<Option<Url>> {
    Ok(next_socks5_proxy()?.map(|proxy| proxy.url))
}

fn configured_order_socks5_proxy() -> Result<Option<Socks5ProxyConfig>> {
    match std::env::var(ORDER_PROXY_ENV) {
        Ok(value) if !value.trim().is_empty() => Socks5ProxyConfig::parse(value.trim())
            .map(Some)
            .with_context(|| format!("proxy_parse_error parsing {ORDER_PROXY_ENV}")),
        _ => Ok(None),
    }
}

fn configured_socks5_proxies() -> Result<Vec<Socks5ProxyConfig>> {
    if let Ok(value) = std::env::var(PROXY_POOL_ENV) {
        if !value.trim().is_empty() {
            let proxies = parse_proxy_list(&value)?;
            anyhow::ensure!(
                !proxies.is_empty(),
                "{PROXY_POOL_ENV} is set but contains no SOCKS5 proxies"
            );
            return Ok(proxies);
        }
    }

    match std::env::var(LEGACY_PROXY_ENV) {
        Ok(value) if !value.trim().is_empty() => Socks5ProxyConfig::parse(value.trim())
            .map(|proxy| vec![proxy])
            .with_context(|| format!("proxy_parse_error parsing {LEGACY_PROXY_ENV}")),
        _ => Ok(Vec::new()),
    }
}

fn parse_proxy_list(value: &str) -> Result<Vec<Socks5ProxyConfig>> {
    value
        .split(|ch: char| ch == ',' || ch == ';' || ch.is_whitespace())
        .filter(|part| !part.trim().is_empty())
        .map(|part| Socks5ProxyConfig::parse(part.trim()))
        .collect()
}

async fn connect_socks5(
    proxy: Socks5ProxyConfig,
    target_host: &str,
    target_port: u16,
) -> Result<TcpStream> {
    let mut stream = TcpStream::connect((proxy.host.as_str(), proxy.port))
        .await
        .with_context(|| format!("connecting to SOCKS5 proxy {}", proxy.redacted()))?;
    let methods = if proxy.has_credentials() {
        vec![SOCKS_METHOD_NO_AUTH, SOCKS_METHOD_USER_PASS]
    } else {
        vec![SOCKS_METHOD_NO_AUTH]
    };
    stream
        .write_all(&[SOCKS_VERSION, methods.len() as u8])
        .await
        .context("writing SOCKS5 greeting")?;
    stream
        .write_all(&methods)
        .await
        .context("writing SOCKS5 auth methods")?;

    let mut method_response = [0_u8; 2];
    stream
        .read_exact(&mut method_response)
        .await
        .context("reading SOCKS5 auth method")?;
    anyhow::ensure!(
        method_response[0] == SOCKS_VERSION,
        "invalid SOCKS5 auth version: {}",
        method_response[0]
    );
    match method_response[1] {
        SOCKS_METHOD_NO_AUTH => {}
        SOCKS_METHOD_USER_PASS => authenticate_socks5(&mut stream, &proxy).await?,
        SOCKS_METHOD_NONE => return Err(anyhow!("SOCKS5 proxy rejected all auth methods")),
        other => return Err(anyhow!("unsupported SOCKS5 auth method selected: {other}")),
    }

    anyhow::ensure!(
        target_host.len() <= u8::MAX as usize,
        "SOCKS5 target host too long"
    );
    let mut request = Vec::with_capacity(7 + target_host.len());
    request.extend_from_slice(&[
        SOCKS_VERSION,
        SOCKS_CMD_CONNECT,
        0x00,
        SOCKS_ATYP_DOMAIN,
        target_host.len() as u8,
    ]);
    request.extend_from_slice(target_host.as_bytes());
    request.extend_from_slice(&target_port.to_be_bytes());
    stream
        .write_all(&request)
        .await
        .context("writing SOCKS5 CONNECT")?;
    read_socks5_connect_response(&mut stream).await?;
    Ok(stream)
}

async fn authenticate_socks5(stream: &mut TcpStream, proxy: &Socks5ProxyConfig) -> Result<()> {
    let username = proxy.username.as_deref().unwrap_or("");
    let password = proxy.password.as_deref().unwrap_or("");
    anyhow::ensure!(
        username.len() <= u8::MAX as usize && password.len() <= u8::MAX as usize,
        "SOCKS5 username/password too long"
    );
    let mut request = Vec::with_capacity(3 + username.len() + password.len());
    request.push(SOCKS_AUTH_VERSION);
    request.push(username.len() as u8);
    request.extend_from_slice(username.as_bytes());
    request.push(password.len() as u8);
    request.extend_from_slice(password.as_bytes());
    stream
        .write_all(&request)
        .await
        .context("writing SOCKS5 username/password auth")?;

    let mut response = [0_u8; 2];
    stream
        .read_exact(&mut response)
        .await
        .context("reading SOCKS5 username/password auth")?;
    anyhow::ensure!(
        response[0] == SOCKS_AUTH_VERSION && response[1] == 0x00,
        "SOCKS5 username/password authentication failed"
    );
    Ok(())
}

async fn read_socks5_connect_response(stream: &mut TcpStream) -> Result<()> {
    let mut header = [0_u8; 4];
    stream
        .read_exact(&mut header)
        .await
        .context("reading SOCKS5 CONNECT response header")?;
    anyhow::ensure!(
        header[0] == SOCKS_VERSION,
        "invalid SOCKS5 CONNECT response version: {}",
        header[0]
    );
    anyhow::ensure!(
        header[1] == SOCKS_REPLY_OK,
        "SOCKS5 CONNECT failed with reply code {}",
        header[1]
    );
    let addr_len = match header[3] {
        0x01 => 4,
        0x03 => {
            let mut len = [0_u8; 1];
            stream
                .read_exact(&mut len)
                .await
                .context("reading SOCKS5 domain response length")?;
            len[0] as usize
        }
        0x04 => 16,
        other => return Err(anyhow!("unsupported SOCKS5 response address type {other}")),
    };
    let mut discard = vec![0_u8; addr_len + 2];
    stream
        .read_exact(&mut discard)
        .await
        .context("reading SOCKS5 CONNECT response address")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proxy_list_with_multiple_separators() {
        let proxies = parse_proxy_list(
            "socks5h://user:pass@31.58.24.104:6175, socks5h://u:p@104.252.136.205:7122; socks5h://u:p@104.143.224.110:5971",
        )
        .unwrap();

        assert_eq!(proxies.len(), 3);
        assert_eq!(proxies[0].redacted(), "socks5h://31.58.24.104:6175");
        assert_eq!(proxies[1].redacted(), "socks5h://104.252.136.205:7122");
        assert_eq!(proxies[2].redacted(), "socks5h://104.143.224.110:5971");
    }

    #[test]
    fn rejects_non_socks5_proxy() {
        let err = parse_proxy_list("http://user:pass@example.com:8080").unwrap_err();
        let text = format!("{err:#}");

        assert!(text.contains("unsupported SOCKS5 proxy scheme"));
        assert!(!text.contains("user:pass"));
    }

    #[test]
    fn debug_redacts_credentials() {
        let proxy = Socks5ProxyConfig::parse("socks5h://user:secret@example.com:1234").unwrap();
        let debug = format!("{proxy:?}");

        assert!(debug.contains("socks5h://example.com:1234"));
        assert!(!debug.contains("secret"));
    }

    #[test]
    fn parses_order_proxy_from_env() {
        std::env::set_var(
            ORDER_PROXY_ENV,
            "socks5h://user:secret@residential.example.com:6228",
        );

        let proxy = configured_order_socks5_proxy().unwrap().unwrap();

        assert_eq!(proxy.redacted(), "socks5h://residential.example.com:6228");
        std::env::remove_var(ORDER_PROXY_ENV);
    }
}
