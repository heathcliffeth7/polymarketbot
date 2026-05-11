use super::*;

pub(crate) fn build_http_client() -> Client {
    let mut builder = Client::builder()
        .pool_max_idle_per_host(32)
        .tcp_nodelay(true)
        .tcp_keepalive(Some(std::time::Duration::from_secs(30)))
        .pool_idle_timeout(Some(std::time::Duration::from_secs(300)))
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(10));
    if let Ok(proxy_url) = std::env::var("SOCKS5_PROXY_URL") {
        match reqwest::Proxy::all(&proxy_url) {
            Ok(proxy) => {
                builder = builder.proxy(proxy);
            }
            Err(e) => {
                tracing::warn!("SOCKS5_PROXY_URL invalid, ignoring: {e}");
            }
        }
    }
    builder.build().expect("HTTP client build failed")
}
