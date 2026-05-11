use super::*;

#[derive(Clone)]
pub struct PolymarketDataApiClient {
    base_url: String,
    http: Client,
}

#[derive(Debug, Deserialize)]
struct RawDataApiActivity {
    #[serde(default, rename = "type")]
    activity_type: String,
    #[serde(default)]
    side: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    asset: String,
    #[serde(default)]
    outcome: String,
    #[serde(default)]
    size: Value,
    #[serde(default, rename = "usdcSize")]
    usdc_size: Value,
    #[serde(default)]
    price: Value,
    #[serde(default)]
    timestamp: Option<i64>,
}

impl PolymarketDataApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            http: build_http_client(),
        }
    }

    pub async fn list_market_activity(
        &self,
        wallet_address: &str,
        market_slug: &str,
        page_size: i64,
        max_pages: i64,
    ) -> Result<Vec<DataApiActivity>> {
        let wallet_address = wallet_address.trim().to_ascii_lowercase();
        let market_slug = market_slug.trim().to_ascii_lowercase();
        if wallet_address.is_empty() || market_slug.is_empty() {
            return Ok(Vec::new());
        }

        let limit = page_size.clamp(1, 500);
        let max_pages = max_pages.clamp(1, 20);
        let limit_str = limit.to_string();
        let url = format!("{}/activity", self.base_url.trim_end_matches('/'));
        let mut activity = Vec::new();

        for page in 0..max_pages {
            let offset = page * limit;
            let offset_str = offset.to_string();
            let rows = self
                .http
                .get(url.clone())
                .query(&[
                    ("user", wallet_address.as_str()),
                    ("limit", limit_str.as_str()),
                    ("offset", offset_str.as_str()),
                ])
                .send()
                .await
                .context("data-api activity request failed")?
                .error_for_status()
                .context("data-api activity endpoint returned error status")?
                .json::<Vec<RawDataApiActivity>>()
                .await
                .context("failed to parse data-api activity response")?;

            if rows.is_empty() {
                break;
            }
            for row in rows
                .iter()
                .filter(|row| row.slug.eq_ignore_ascii_case(&market_slug))
            {
                activity.push(DataApiActivity {
                    activity_type: row.activity_type.trim().to_ascii_uppercase(),
                    side: non_empty_string(&row.side).map(|value| value.to_ascii_uppercase()),
                    slug: row.slug.clone(),
                    asset: non_empty_string(&row.asset),
                    outcome: non_empty_string(&row.outcome),
                    size: parse_json_f64(Some(&row.size)).unwrap_or_default(),
                    usdc_size: parse_json_f64(Some(&row.usdc_size)).unwrap_or_default(),
                    price: parse_json_f64(Some(&row.price)),
                    timestamp: row.timestamp,
                });
            }

            if rows.len() < limit as usize {
                break;
            }
        }

        Ok(activity)
    }
}

fn non_empty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
