use super::*;

#[derive(Clone)]
pub struct GammaHttpClient {
    base_url: String,
    http: Client,
}

impl GammaHttpClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            http: build_http_client(),
        }
    }

    pub async fn get_market_by_slug(&self, slug: &str) -> Result<Option<GammaMarket>> {
        let normalized_slug = slug.trim().to_ascii_lowercase();
        if normalized_slug.is_empty() {
            return Ok(None);
        }

        let url = format!(
            "{}/markets?slug={}",
            self.base_url.trim_end_matches('/'),
            normalized_slug
        );
        let raw: serde_json::Value = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(raw.as_array().and_then(|items| {
            items
                .iter()
                .find_map(parse_gamma_market)
                .filter(|market| market.slug == normalized_slug)
        }))
    }

    pub async fn get_market_spec_by_slug(&self, slug: &str) -> Result<Option<GammaMarket>> {
        let normalized_slug = slug.trim().to_ascii_lowercase();
        if normalized_slug.is_empty() {
            return Ok(None);
        }

        let url = format!(
            "{}/markets?slug={}",
            self.base_url.trim_end_matches('/'),
            normalized_slug
        );
        let raw: serde_json::Value = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(raw.as_array().and_then(|items| {
            items
                .iter()
                .find_map(parse_gamma_market_any)
                .filter(|market| market.slug == normalized_slug)
        }))
    }

    pub async fn get_market_spec_by_token_id(&self, token_id: &str) -> Result<Option<GammaMarket>> {
        let token_id = token_id.trim();
        if token_id.is_empty() {
            return Ok(None);
        }

        let url = format!(
            "{}/markets?clob_token_ids={}",
            self.base_url.trim_end_matches('/'),
            token_id
        );
        let raw: serde_json::Value = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(raw
            .as_array()
            .and_then(|items| items.iter().find_map(parse_gamma_market_any)))
    }
}

#[async_trait]
impl GammaClient for GammaHttpClient {
    async fn list_active_updown_markets(&self) -> Result<Vec<GammaMarket>> {
        let url = format!(
            "{}/markets?active=true&closed=false&limit=1000",
            self.base_url.trim_end_matches('/')
        );
        let raw: serde_json::Value = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut out = Vec::new();
        let items = raw.as_array().cloned().unwrap_or_default();
        for item in items {
            if let Some(parsed) = parse_gamma_market(&item) {
                out.push(parsed);
            }
        }
        Ok(out)
    }

    async fn list_btc_5m_markets(&self) -> Result<Vec<GammaMarket>> {
        let markets = self.list_active_updown_markets().await?;
        Ok(markets
            .into_iter()
            .filter(|market| market.slug.starts_with("btc-updown-5m-"))
            .collect())
    }
}
