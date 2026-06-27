use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Deserialize;

const GEOBLOCK_URL: &str = "https://polymarket.com/api/geoblock";

#[derive(Debug, Deserialize)]
struct GeoblockResponse {
    blocked: bool,
    ip: Option<String>,
    country: Option<String>,
    region: Option<String>,
}

pub async fn ensure_order_egress_allowed(http: &Client) -> Result<()> {
    let response = http
        .get(GEOBLOCK_URL)
        .header("User-Agent", "dextrabot")
        .send()
        .await
        .context("checking Polymarket order geoblock")?;
    let status = response.status();
    if !status.is_success() {
        return Err(anyhow!(
            "Polymarket geoblock check failed with HTTP {status}"
        ));
    }

    let body: GeoblockResponse = response
        .json()
        .await
        .context("decoding Polymarket geoblock response")?;
    if body.blocked {
        return Err(anyhow!(
            "order egress geoblock blocked ip={} country={} region={}",
            body.ip.as_deref().unwrap_or("unknown"),
            body.country.as_deref().unwrap_or("unknown"),
            body.region.as_deref().unwrap_or("unknown")
        ));
    }

    Ok(())
}
