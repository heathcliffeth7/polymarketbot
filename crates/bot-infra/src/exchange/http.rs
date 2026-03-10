use super::*;

pub(crate) fn build_http_client() -> Client {
    let mut builder = Client::builder();
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
