use crate::signer::{
    sign_order_eip712, unix_now_secs, ApiCredentials, ClobHeaderSigner, HeaderSigner,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use ethers::{
    signers::LocalWallet,
    types::{Address, U256},
};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

fn build_http_client() -> Client {
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

const SUPPORTED_UPDOWN_SLUG_PREFIXES: [&str; 8] = [
    "btc-updown-5m-",
    "btc-updown-15m-",
    "eth-updown-5m-",
    "eth-updown-15m-",
    "sol-updown-5m-",
    "sol-updown-15m-",
    "xrp-updown-5m-",
    "xrp-updown-15m-",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GammaMarket {
    pub slug: String,
    pub end_date_iso: Option<String>,
    pub active: bool,
    pub closed: bool,
    pub yes_token_id: Option<String>,
    pub no_token_id: Option<String>,
    pub maker_base_fee: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceSnapshot {
    pub market: String,
    pub price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceOrderRequest {
    pub market: String,
    pub token_id: Option<String>,
    pub side: String,
    pub price: f64,
    pub size: f64,
    pub intent: String,
    #[serde(default = "default_order_type")]
    pub order_type: String,
    pub client_order_id: String,
    pub leg_side: Option<String>,
    #[serde(default)]
    pub fee_rate_bps: u64,
}

fn default_order_type() -> String {
    "GTC".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderAck {
    pub client_order_id: String,
    pub exchange_order_id: Option<String>,
    pub status: String,
    pub reject_reason: Option<String>,
    pub raw_status: Option<String>,
    pub exchange_ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderInfo {
    pub order_id: String,
    pub client_order_id: Option<String>,
    pub status: String,
    pub price: Option<f64>,
    pub size: Option<f64>,
    pub filled_size: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillInfo {
    pub fill_id: String,
    pub order_id: String,
    pub price: f64,
    pub size: f64,
    pub fee: Option<f64>,
    pub ts: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct DataApiInventoryPosition {
    asset: Option<String>,
    #[serde(rename = "tokenId")]
    token_id: Option<String>,
    #[serde(rename = "clobTokenId")]
    clob_token_id: Option<String>,
    size: Option<Value>,
    balance: Option<Value>,
}

#[async_trait]
pub trait GammaClient: Send + Sync {
    async fn list_active_updown_markets(&self) -> Result<Vec<GammaMarket>>;

    async fn list_btc_5m_markets(&self) -> Result<Vec<GammaMarket>>;
}

#[async_trait]
pub trait ClobRestClient: Send + Sync {
    async fn get_price_snapshot(&self, market: &str) -> Result<PriceSnapshot>;
    async fn get_fee_rate_bps(&self, token_id: &str) -> Result<Option<u64>>;
    async fn place_order(&self, req: &PlaceOrderRequest) -> Result<OrderAck>;
    async fn cancel_order(&self, exchange_order_id: &str) -> Result<()>;
    async fn get_order(&self, exchange_order_id: &str) -> Result<OrderInfo>;
    async fn list_open_orders(&self, market: Option<&str>) -> Result<Vec<OrderInfo>>;
    async fn list_fills(&self, next_cursor: Option<&str>) -> Result<Vec<FillInfo>>;
    async fn get_balance(&self) -> Result<f64>;
    async fn get_token_inventory(&self, _token_id: &str) -> Result<Option<f64>> {
        Ok(None)
    }
}

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

fn parse_string_array(v: &serde_json::Value) -> Vec<String> {
    if let Some(arr) = v.as_array() {
        return arr
            .iter()
            .filter_map(|x| x.as_str().map(ToString::to_string))
            .collect();
    }

    if let Some(raw) = v.as_str() {
        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(raw) {
            return parsed;
        }
    }

    Vec::new()
}

fn parse_json_f64(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(v)) => v.as_f64(),
        Some(Value::String(v)) => v.parse::<f64>().ok(),
        _ => None,
    }
}

fn data_api_position_matches_token(position: &DataApiInventoryPosition, token_id: &str) -> bool {
    let normalized_token_id = token_id.trim();
    if normalized_token_id.is_empty() {
        return false;
    }

    [
        position.asset.as_deref(),
        position.token_id.as_deref(),
        position.clob_token_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|candidate| candidate.trim() == normalized_token_id)
}

fn parse_gamma_market(item: &serde_json::Value) -> Option<GammaMarket> {
    let slug = item
        .get("slug")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if slug.is_empty() {
        return None;
    }
    if !SUPPORTED_UPDOWN_SLUG_PREFIXES
        .iter()
        .any(|prefix| slug.starts_with(prefix))
    {
        return None;
    }

    let (yes_token_id, no_token_id) = parse_yes_no_token_ids(item);
    let maker_base_fee = item
        .get("makerBaseFee")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Some(GammaMarket {
        slug,
        end_date_iso: item
            .get("endDate")
            .or_else(|| item.get("end_date_iso"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        active: item
            .get("active")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        closed: item
            .get("closed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        yes_token_id,
        no_token_id,
        maker_base_fee,
    })
}

fn parse_outcome_side(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "yes" | "up" => Some("yes"),
        "no" | "down" => Some("no"),
        _ => None,
    }
}

fn parse_yes_no_token_ids(item: &serde_json::Value) -> (Option<String>, Option<String>) {
    let direct_yes = item
        .get("yesTokenId")
        .or_else(|| item.get("yes_token_id"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    let direct_no = item
        .get("noTokenId")
        .or_else(|| item.get("no_token_id"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    if direct_yes.is_some() || direct_no.is_some() {
        return (direct_yes, direct_no);
    }

    let mut outcomes = Vec::new();
    if let Some(v) = item.get("outcomes") {
        outcomes = parse_string_array(v)
            .into_iter()
            .map(|s| s.to_lowercase())
            .collect();
    }

    let mut clob_token_ids = Vec::new();
    if let Some(v) = item
        .get("clobTokenIds")
        .or_else(|| item.get("clob_token_ids"))
    {
        clob_token_ids = parse_string_array(v);
    }

    if outcomes.len() >= 2 && clob_token_ids.len() >= 2 {
        let mut yes = None;
        let mut no = None;
        for (idx, outcome) in outcomes.iter().enumerate() {
            match parse_outcome_side(outcome) {
                Some("yes") => yes = clob_token_ids.get(idx).cloned(),
                Some("no") => no = clob_token_ids.get(idx).cloned(),
                _ => {}
            }
        }
        if yes.is_some() || no.is_some() {
            return (yes, no);
        }
    }

    if let Some(tokens) = item.get("tokens").and_then(|v| v.as_array()) {
        let mut yes = None;
        let mut no = None;
        for token in tokens {
            let outcome = token
                .get("outcome")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_lowercase();
            let token_id = token
                .get("token_id")
                .or_else(|| token.get("tokenId"))
                .or_else(|| token.get("clobTokenId"))
                .or_else(|| token.get("id"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            match parse_outcome_side(&outcome) {
                Some("yes") if yes.is_none() => yes = token_id.clone(),
                Some("no") if no.is_none() => no = token_id.clone(),
                _ => {}
            }
        }
        if yes.is_some() || no.is_some() {
            return (yes, no);
        }
    }

    if clob_token_ids.len() >= 2 {
        return (
            clob_token_ids.first().cloned(),
            clob_token_ids.get(1).cloned(),
        );
    }

    (None, None)
}

#[derive(Clone)]
pub struct ClobHttpClient {
    base_url: String,
    positions_base_url: Option<String>,
    positions_page_size: i64,
    positions_max_pages: i64,
    http: Client,
    signer: Arc<dyn HeaderSigner>,
    wallet: LocalWallet,
    exchange_address: Address,
    chain_id: u64,
    address: String,
    api_key: String,
    gnosis_safe: Option<Address>, // proxy maker address (signatureType=2)
}

impl ClobHttpClient {
    pub fn from_credentials(
        base_url: String,
        positions_base_url: Option<String>,
        positions_page_size: i64,
        positions_max_pages: i64,
        creds: ApiCredentials,
        wallet: LocalWallet,
        exchange_address: Address,
        chain_id: u64,
        gnosis_safe: Option<Address>,
    ) -> Self {
        let address = creds.address.clone();
        let api_key = creds.key.clone();
        Self {
            base_url,
            positions_base_url,
            positions_page_size,
            positions_max_pages,
            http: build_http_client(),
            signer: Arc::new(ClobHeaderSigner { creds }),
            wallet,
            exchange_address,
            chain_id,
            address,
            api_key,
            gnosis_safe,
        }
    }

    async fn signed_json(
        &self,
        method: Method,
        request_path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), request_path);
        let method_name = method.as_str().to_string();
        let body_str = body.as_ref().map(|v| v.to_string());
        let timestamp = unix_now_secs()?;

        let headers = self
            .signer
            .signed_headers(
                timestamp,
                method.as_str(),
                request_path,
                body_str.as_deref(),
            )
            .map_err(|err| anyhow::anyhow!("build signed headers: {err:#}"))?;

        let mut req = self.http.request(method, url);
        for (k, v) in headers {
            req = req.header(k, v);
        }

        req = req
            .header("User-Agent", "dextrabot")
            .header("Accept", "*/*")
            .header("Connection", "keep-alive")
            .header("Content-Type", "application/json");

        if let Some(b) = body {
            req = req.body(b.to_string());
        }
        let response = req.send().await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }

        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "<failed to read error body>".to_string());
        let trimmed = error_text.trim();
        let summarized = if trimmed.is_empty() {
            "<empty error body>"
        } else {
            trimmed
        };
        Err(anyhow::anyhow!(
            "HTTP status {} for {} {} | body: {}",
            status,
            method_name,
            request_path,
            summarized
        ))
    }

    fn inventory_lookup_address(&self) -> String {
        self.gnosis_safe
            .map(|addr| ethers::utils::to_checksum(&addr, None))
            .unwrap_or_else(|| self.address.clone())
    }
}

#[async_trait]
impl ClobRestClient for ClobHttpClient {
    async fn get_price_snapshot(&self, market: &str) -> Result<PriceSnapshot> {
        // Polymarket CLOB midpoint endpoint expects token_id.
        let request_path = format!("/midpoint?token_id={market}");
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), request_path);

        let raw: serde_json::Value = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let price = raw
            .get("price")
            .or_else(|| raw.get("mid"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        Ok(PriceSnapshot {
            market: market.to_string(),
            price,
        })
    }

    async fn get_fee_rate_bps(&self, token_id: &str) -> Result<Option<u64>> {
        if token_id.trim().is_empty() {
            return Ok(None);
        }
        let request_path = format!("/fee-rate?token_id={token_id}");
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), request_path);
        let raw: serde_json::Value = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let fee_rate_bps = raw
            .get("fee_rate_bps")
            .or_else(|| raw.get("feeRateBps"))
            .and_then(|value| match value {
                Value::Number(v) => v.as_u64(),
                Value::String(v) => v.parse::<u64>().ok(),
                _ => None,
            });
        Ok(fee_rate_bps)
    }

    async fn place_order(&self, req: &PlaceOrderRequest) -> Result<OrderAck> {
        let client_id = if req.client_order_id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            req.client_order_id.clone()
        };
        let normalized_order_type = match req.order_type.trim().to_ascii_uppercase().as_str() {
            "IOC" => "IOC",
            "FOK" => "FOK",
            _ => "GTC",
        };

        let token_id_str = req.token_id.as_deref().unwrap_or("");
        let token_id = U256::from_dec_str(token_id_str).unwrap_or(U256::zero());

        let is_buy = req.side.eq_ignore_ascii_case("buy");
        // CLOB precision rules:
        // BUY:  makerAmount (USDC)   → max 4 decimals → divisible by 100 (of 1e6)
        //       takerAmount (shares) → max 2 decimals → divisible by 10_000 (of 1e6)
        // SELL: makerAmount (shares) → divisible by 10_000
        //       takerAmount (USDC)   → divisible by 100
        // Round to the nearest valid unit instead of floor to avoid off-by-one errors.
        let cost_units = ((req.price * req.size * 10_000.0).round() as u64) * 100;
        let size_units = ((req.size * 100.0).round() as u64) * 10_000;

        let (maker_amount, taker_amount) = if is_buy {
            // BUY: spending USDC (maker), receiving shares (taker)
            (U256::from(cost_units), U256::from(size_units))
        } else {
            // SELL: spending shares (maker), receiving USDC (taker)
            (U256::from(size_units), U256::from(cost_units))
        };
        let side_u8: u8 = if is_buy { 0 } else { 1 };

        // salt: small random u32 that fits in a JS safe integer (< 2^53).
        // The Polymarket CLOB API parses salt as a JSON number; large integers lose
        // precision in JavaScript and cause EIP-712 signature mismatch → "Invalid order payload".
        let salt_u32 = u32::from_be_bytes(Uuid::new_v4().as_bytes()[0..4].try_into().unwrap());
        let salt = U256::from(salt_u32);

        let eoa: Address = self
            .address
            .parse()
            .context("parse EOA address for EIP-712")?;

        // Gnosis Safe: maker=proxy, signer=EOA, signatureType=2
        // EOA:         maker=EOA,   signer=EOA, signatureType=0
        let (maker, signer_addr, sig_type): (Address, Address, u64) = match self.gnosis_safe {
            Some(safe) => (safe, eoa, 2),
            None => (eoa, eoa, 0),
        };

        let fee_rate_bps = req.fee_rate_bps;
        let signature = sign_order_eip712(
            &self.wallet,
            self.chain_id,
            self.exchange_address,
            salt,
            maker,
            signer_addr,
            token_id,
            maker_amount,
            taker_amount,
            side_u8,
            fee_rate_bps,
            sig_type,
        )
        .context("EIP-712 order signing failed")?;

        let maker_str = ethers::utils::to_checksum(&maker, None);
        let signer_str = ethers::utils::to_checksum(&signer_addr, None);
        let side_str = if is_buy { "BUY" } else { "SELL" };
        // Both salt and signatureType MUST be JSON integers (not strings).
        // The CLOB server parses them as JS numbers; strings or large ints fail EIP-712 verification.
        let salt_u64 = salt.low_u64(); // safe: salt_u32 ≤ u32::MAX, fits in JS number
        let body = json!({
            "order": {
                "salt": salt_u64,
                "maker": maker_str,
                "signer": signer_str,
                "taker": "0x0000000000000000000000000000000000000000",
                "tokenId": token_id.to_string(),
                "makerAmount": maker_amount.to_string(),
                "takerAmount": taker_amount.to_string(),
                "side": side_str,
                "expiration": "0",
                "nonce": "0",
                "feeRateBps": fee_rate_bps.to_string(),
                "signatureType": sig_type as i64,
                "signature": signature,
            },
            "owner": self.api_key,
            "orderType": normalized_order_type,
        });

        tracing::warn!(
            side = side_u8,
            side_str,
            token_id = %token_id,
            maker_amount = %maker_amount,
            taker_amount = %taker_amount,
            fee_rate_bps,
            salt = %salt,
            maker = %maker_str,
            signer = %signer_str,
            sig_type,
            exchange = %self.exchange_address,
            chain_id = self.chain_id,
            order_type = normalized_order_type,
            sig_prefix = &signature[..20],
            body_json = %body,
            "PLACE_ORDER_EIP712_DEBUG"
        );

        let raw: serde_json::Value = self
            .signed_json(Method::POST, "/order", Some(body))
            .await?
            .json()
            .await?;

        Ok(OrderAck {
            client_order_id: client_id,
            exchange_order_id: raw
                .get("orderID")
                .or_else(|| raw.get("id"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            status: raw
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("ack")
                .to_string(),
            reject_reason: raw
                .get("errorMsg")
                .or_else(|| raw.get("rejectReason"))
                .and_then(|v| v.as_str())
                .map(ToString::to_string),
            raw_status: raw
                .get("status")
                .and_then(|v| v.as_str())
                .map(ToString::to_string),
            exchange_ts: raw.get("timestamp").and_then(|v| v.as_i64()),
        })
    }

    async fn cancel_order(&self, exchange_order_id: &str) -> Result<()> {
        let body = json!({ "orderID": exchange_order_id });
        let _ = self
            .signed_json(Method::DELETE, "/order", Some(body))
            .await?;
        Ok(())
    }

    async fn get_order(&self, exchange_order_id: &str) -> Result<OrderInfo> {
        let path = format!("/data/order/{exchange_order_id}");
        let raw: serde_json::Value = self
            .signed_json(Method::GET, &path, None)
            .await?
            .json()
            .await?;

        Ok(OrderInfo {
            order_id: raw
                .get("orderID")
                .or_else(|| raw.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            client_order_id: raw
                .get("clientOrderId")
                .and_then(|v| v.as_str())
                .map(ToString::to_string),
            status: raw
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            price: raw.get("price").and_then(|v| v.as_f64()),
            size: raw.get("size").and_then(|v| v.as_f64()),
            filled_size: raw
                .get("filledSize")
                .or_else(|| raw.get("filled_size"))
                .and_then(|v| v.as_f64()),
        })
    }

    async fn list_open_orders(&self, market: Option<&str>) -> Result<Vec<OrderInfo>> {
        let mut path = "/data/orders?next_cursor=MA==".to_string();
        if let Some(market) = market {
            path.push_str(&format!("&market={market}"));
        }

        let raw: serde_json::Value = self
            .signed_json(Method::GET, &path, None)
            .await?
            .json()
            .await?;

        let rows = raw
            .get("data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        Ok(rows
            .into_iter()
            .map(|r| OrderInfo {
                order_id: r
                    .get("orderID")
                    .or_else(|| r.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                client_order_id: r
                    .get("clientOrderId")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string),
                status: r
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                price: r.get("price").and_then(|v| v.as_f64()),
                size: r.get("size").and_then(|v| v.as_f64()),
                filled_size: r
                    .get("filledSize")
                    .or_else(|| r.get("filled_size"))
                    .and_then(|v| v.as_f64()),
            })
            .collect())
    }

    async fn list_fills(&self, next_cursor: Option<&str>) -> Result<Vec<FillInfo>> {
        let cursor = next_cursor.unwrap_or("MA==");
        let path = format!("/data/trades?next_cursor={cursor}");

        let raw: serde_json::Value = self
            .signed_json(Method::GET, &path, None)
            .await?
            .json()
            .await?;

        let rows = raw
            .get("data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        Ok(rows
            .into_iter()
            .map(|r| FillInfo {
                fill_id: r
                    .get("id")
                    .or_else(|| r.get("fillID"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                order_id: r
                    .get("orderID")
                    .or_else(|| r.get("order_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                price: r.get("price").and_then(|v| v.as_f64()).unwrap_or_default(),
                size: r.get("size").and_then(|v| v.as_f64()).unwrap_or_default(),
                fee: r.get("fee").and_then(|v| v.as_f64()),
                ts: r.get("timestamp").and_then(|v| v.as_i64()),
            })
            .collect())
    }

    async fn get_balance(&self) -> Result<f64> {
        let request_path = format!("/data/balances?user_id={}", self.address);
        let resp = self.signed_json(Method::GET, &request_path, None).await?;
        let raw: serde_json::Value = resp.json().await?;
        // Response: [{"asset":"USDC","available":"12.50",...}] or {"balance":"12.50"}
        let balance = raw
            .as_array()
            .and_then(|arr| {
                arr.iter()
                    .find(|v| v.get("asset").and_then(|a| a.as_str()) == Some("USDC"))
            })
            .and_then(|v| v.get("available").or_else(|| v.get("balance")))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| {
                raw.get("balance")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse().ok())
            })
            .or_else(|| raw.get("balance").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);
        Ok(balance)
    }

    async fn get_token_inventory(&self, token_id: &str) -> Result<Option<f64>> {
        let Some(base_url) = self.positions_base_url.as_deref() else {
            return Ok(None);
        };
        if base_url.trim().is_empty() || token_id.trim().is_empty() {
            return Ok(None);
        }

        let limit = self.positions_page_size.max(1);
        let max_pages = self.positions_max_pages.max(1);
        let user = self.inventory_lookup_address();
        let url = format!("{}/positions", base_url.trim_end_matches('/'));
        let limit_str = limit.to_string();

        let mut total_qty = 0.0_f64;
        let mut saw_any_page = false;

        for page in 0..max_pages {
            let offset = page * limit;
            let offset_str = offset.to_string();
            let rows = self
                .http
                .get(url.clone())
                .query(&[
                    ("user", user.as_str()),
                    ("sizeThreshold", "0"),
                    ("limit", limit_str.as_str()),
                    ("offset", offset_str.as_str()),
                ])
                .send()
                .await?
                .error_for_status()?
                .json::<Vec<DataApiInventoryPosition>>()
                .await?;

            if rows.is_empty() {
                break;
            }
            saw_any_page = true;

            for row in &rows {
                if !data_api_position_matches_token(row, token_id) {
                    continue;
                }
                total_qty += parse_json_f64(row.size.as_ref())
                    .or_else(|| parse_json_f64(row.balance.as_ref()))
                    .unwrap_or_default();
            }

            if rows.len() < limit as usize {
                break;
            }
        }

        if !saw_any_page {
            return Ok(Some(0.0));
        }
        Ok(Some(total_qty.max(0.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mock_exchange::spawn_mock_exchange;
    use serde_json::json;

    #[tokio::test]
    async fn place_and_reconcile_against_mock_exchange() -> Result<()> {
        let mock = spawn_mock_exchange().await?;
        let wallet = "0000000000000000000000000000000000000000000000000000000000000001"
            .parse::<LocalWallet>()
            .unwrap();
        let dummy_addr = Address::zero();
        let client = ClobHttpClient::from_credentials(
            mock.base_http(),
            None,
            0,
            0,
            ApiCredentials {
                address: "0x0000000000000000000000000000000000000000".to_string(),
                key: "k".to_string(),
                secret: "YWFhYQ==".to_string(),
                passphrase: "p".to_string(),
            },
            wallet,
            dummy_addr,
            137,
            None,
        );

        let ack = client
            .place_order(&PlaceOrderRequest {
                market: "btc-updown-5m-1".to_string(),
                token_id: Some("tok-yes".to_string()),
                side: "buy".to_string(),
                price: 0.60,
                size: 10.0,
                intent: "entry".to_string(),
                order_type: "GTC".to_string(),
                client_order_id: Uuid::new_v4().to_string(),
                leg_side: Some("yes".to_string()),
                fee_rate_bps: 0,
            })
            .await?;

        assert!(ack.exchange_order_id.is_some());
        let open = client.list_open_orders(Some("btc-updown-5m-1")).await?;
        let fills = client.list_fills(None).await?;

        assert!(open.len() <= 1);
        assert!(!fills.is_empty());
        mock.shutdown();
        Ok(())
    }

    #[test]
    fn parse_token_ids_supports_up_down_outcomes() {
        let market = json!({
            "outcomes": ["Up", "Down"],
            "clobTokenIds": ["tok-up", "tok-down"]
        });

        let (yes_token_id, no_token_id) = parse_yes_no_token_ids(&market);
        assert_eq!(yes_token_id.as_deref(), Some("tok-up"));
        assert_eq!(no_token_id.as_deref(), Some("tok-down"));
    }

    #[test]
    fn parse_token_ids_supports_up_down_tokens_array() {
        let market = json!({
            "tokens": [
                { "outcome": "UP", "token_id": "tok-up" },
                { "outcome": "down", "token_id": "tok-down" }
            ]
        });

        let (yes_token_id, no_token_id) = parse_yes_no_token_ids(&market);
        assert_eq!(yes_token_id.as_deref(), Some("tok-up"));
        assert_eq!(no_token_id.as_deref(), Some("tok-down"));
    }

    #[test]
    fn parse_token_ids_prefers_direct_yes_no_fields() {
        let market = json!({
            "yesTokenId": "tok-yes-direct",
            "noTokenId": "tok-no-direct",
            "outcomes": ["Up", "Down"],
            "clobTokenIds": ["tok-up", "tok-down"]
        });

        let (yes_token_id, no_token_id) = parse_yes_no_token_ids(&market);
        assert_eq!(yes_token_id.as_deref(), Some("tok-yes-direct"));
        assert_eq!(no_token_id.as_deref(), Some("tok-no-direct"));
    }
}
