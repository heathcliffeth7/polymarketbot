use super::*;

fn base_http_client_builder() -> reqwest::ClientBuilder {
    Client::builder()
        .pool_max_idle_per_host(32)
        .tcp_nodelay(true)
        .tcp_keepalive(Some(std::time::Duration::from_secs(30)))
        .pool_idle_timeout(Some(std::time::Duration::from_secs(300)))
        .connect_timeout(std::time::Duration::from_secs(3))
        .timeout(std::time::Duration::from_secs(10))
}

pub(crate) fn build_http_client() -> Client {
    let builder = base_http_client_builder();
    let builder = crate::proxy::add_rotating_reqwest_proxy(builder, "exchange_http");
    builder.build().expect("HTTP client build failed")
}

pub(crate) fn build_order_http_client() -> Client {
    let builder = base_http_client_builder();
    let builder = crate::proxy::add_order_reqwest_proxy(builder, "exchange_order_http");
    builder.build().expect("order HTTP client build failed")
}
