use super::*;

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
    neg_risk_exchange_address: Option<Address>,
    chain_id: u64,
    domain_separator: [u8; 32],
    neg_risk_domain_separator: Option<[u8; 32]>,
    address: String,
    api_key: String,
    gnosis_safe: Option<Address>,
    builder_code_hex: String,
    market_info_by_condition: Arc<Mutex<HashMap<String, ClobMarketInfo>>>,
    market_condition_by_token: Arc<Mutex<HashMap<String, String>>>,
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
        neg_risk_exchange_address: Option<Address>,
        chain_id: u64,
        gnosis_safe: Option<Address>,
        builder_code: Option<String>,
    ) -> Self {
        let address = creds.address.clone();
        let api_key = creds.key.clone();
        let domain_separator = domain_separator_for_exchange(chain_id, exchange_address);
        let neg_risk_domain_separator = neg_risk_exchange_address
            .map(|address| domain_separator_for_exchange(chain_id, address));
        let builder_code_hex = bytes32_to_hex(
            parse_bytes32_hex(builder_code.as_deref().unwrap_or_default()).unwrap_or([0u8; 32]),
        );
        Self {
            base_url,
            positions_base_url,
            positions_page_size,
            positions_max_pages,
            http: build_http_client(),
            signer: Arc::new(ClobHeaderSigner { creds }),
            wallet,
            exchange_address,
            neg_risk_exchange_address,
            chain_id,
            domain_separator,
            neg_risk_domain_separator,
            address,
            api_key,
            gnosis_safe,
            builder_code_hex,
            market_info_by_condition: Arc::new(Mutex::new(HashMap::new())),
            market_condition_by_token: Arc::new(Mutex::new(HashMap::new())),
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

        if let Some(ref s) = body_str {
            req = req.body(s.clone());
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

    pub(crate) fn effective_exchange_address(&self, neg_risk: bool) -> Address {
        if neg_risk {
            self.neg_risk_exchange_address
                .unwrap_or(self.exchange_address)
        } else {
            self.exchange_address
        }
    }

    fn cached_domain_separator(&self, neg_risk: bool) -> [u8; 32] {
        if neg_risk {
            self.neg_risk_domain_separator
                .unwrap_or(self.domain_separator)
        } else {
            self.domain_separator
        }
    }

    fn cache_market_info(&self, info: &ClobMarketInfo) {
        if let Ok(mut by_condition) = self.market_info_by_condition.lock() {
            by_condition.insert(info.condition_id.clone(), info.clone());
        }
        if let Ok(mut by_token) = self.market_condition_by_token.lock() {
            for token in &info.tokens {
                by_token.insert(token.token_id.clone(), info.condition_id.clone());
            }
        }
    }

    fn cached_market_info_by_condition(&self, condition_id: &str) -> Option<ClobMarketInfo> {
        self.market_info_by_condition
            .lock()
            .ok()?
            .get(condition_id)
            .cloned()
    }

    fn cached_market_info_by_token(&self, token_id: &str) -> Option<ClobMarketInfo> {
        let condition_id = self
            .market_condition_by_token
            .lock()
            .ok()?
            .get(token_id)?
            .clone();
        self.cached_market_info_by_condition(&condition_id)
    }
}

fn parse_bytes32_hex(raw: &str) -> Option<[u8; 32]> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Some([0u8; 32]);
    }
    let hex = trimmed.strip_prefix("0x")?;
    if hex.len() != 64 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (idx, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[idx * 2..idx * 2 + 2], 16).ok()?;
    }
    Some(out)
}

fn bytes32_to_hex(bytes: [u8; 32]) -> String {
    bytes.iter().fold("0x".to_string(), |mut out, byte| {
        use std::fmt::Write;
        write!(out, "{byte:02x}").unwrap();
        out
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_place_order_body(
    salt_u64: u64,
    maker_str: &str,
    signer_str: &str,
    token_id: U256,
    maker_amount: U256,
    taker_amount: U256,
    side_str: &str,
    sig_type: u64,
    signature: &str,
    owner: &str,
    normalized_order_type: &str,
    fee_rate_bps: u64,
) -> serde_json::Value {
    json!({
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
        "owner": owner,
        "orderType": normalized_order_type,
        "deferExec": false,
    })
}

fn parse_price_history_point(raw: &Value) -> Option<PriceHistoryPoint> {
    let ts = raw
        .get("t")
        .or_else(|| raw.get("timestamp"))
        .or_else(|| raw.get("ts"))
        .and_then(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(text) => text.parse::<i64>().ok(),
            _ => None,
        })?;
    let price = raw
        .get("p")
        .or_else(|| raw.get("price"))
        .and_then(parse_f64_value)?;
    if !price.is_finite() || price <= 0.0 {
        return None;
    }
    Some(PriceHistoryPoint { ts, price })
}

fn parse_fee_rate_bps_value(value: &Value) -> Option<u64> {
    match value {
        Value::Number(v) => v.as_u64(),
        Value::String(v) => v.parse::<u64>().ok(),
        _ => None,
    }
}

pub(super) fn parse_fee_rate_bps_response(raw: &serde_json::Value) -> Option<u64> {
    raw.get("fee_rate_bps")
        .or_else(|| raw.get("feeRateBps"))
        .or_else(|| raw.get("base_fee"))
        .or_else(|| raw.get("baseFee"))
        .and_then(parse_fee_rate_bps_value)
}

fn parse_clob_token(raw: &Value) -> Option<ClobMarketToken> {
    let token_id = raw
        .get("t")
        .or_else(|| raw.get("token_id"))
        .or_else(|| raw.get("tokenId"))
        .and_then(Value::as_str)?
        .to_string();
    let outcome = raw
        .get("o")
        .or_else(|| raw.get("outcome"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Some(ClobMarketToken { token_id, outcome })
}

pub(super) fn parse_clob_market_info_response(
    raw: &Value,
    fallback_condition_id: &str,
) -> Option<ClobMarketInfo> {
    let condition_id = raw
        .get("condition_id")
        .or_else(|| raw.get("conditionId"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback_condition_id)
        .to_string();
    if condition_id.trim().is_empty() {
        return None;
    }
    let tokens = raw
        .get("t")
        .or_else(|| raw.get("tokens"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(parse_clob_token)
        .collect::<Vec<_>>();
    let fee_details =
        raw.get("fd")
            .or_else(|| raw.get("fee_details"))
            .map(|fd| ClobMarketFeeDetails {
                rate: fd
                    .get("r")
                    .or_else(|| fd.get("rate"))
                    .and_then(parse_f64_value)
                    .unwrap_or(0.0),
                exponent: fd
                    .get("e")
                    .or_else(|| fd.get("exponent"))
                    .and_then(parse_f64_value)
                    .unwrap_or(0.0),
                taker_only: fd
                    .get("to")
                    .or_else(|| fd.get("taker_only"))
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            });
    Some(ClobMarketInfo {
        condition_id,
        tokens,
        min_tick_size: raw
            .get("mts")
            .or_else(|| raw.get("min_tick_size"))
            .and_then(parse_f64_value),
        min_order_size: raw
            .get("mos")
            .or_else(|| raw.get("min_order_size"))
            .and_then(parse_f64_value),
        maker_base_fee_bps: raw
            .get("mbf")
            .or_else(|| raw.get("maker_base_fee_bps"))
            .and_then(parse_fee_rate_bps_value),
        taker_base_fee_bps: raw
            .get("tbf")
            .or_else(|| raw.get("taker_base_fee_bps"))
            .and_then(parse_fee_rate_bps_value),
        fee_details,
        neg_risk: raw
            .get("nr")
            .or_else(|| raw.get("neg_risk"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn parse_order_book_levels(raw: &serde_json::Value, side_key: &str) -> Vec<OrderBookLevel> {
    raw.get(side_key)
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|row| {
            let price = row.get("price").and_then(parse_f64_value)?;
            let size = row
                .get("size")
                .or_else(|| row.get("amount"))
                .or_else(|| row.get("shares"))
                .and_then(parse_f64_value)?;
            if !price.is_finite() || !size.is_finite() || price <= 0.0 || size <= 0.0 {
                return None;
            }
            Some(OrderBookLevel { price, size })
        })
        .collect()
}

pub(super) fn extract_order_book_from_book(raw: &serde_json::Value) -> OrderBookSnapshot {
    OrderBookSnapshot {
        bids: parse_order_book_levels(raw, "bids"),
        asks: parse_order_book_levels(raw, "asks"),
    }
}

pub(super) fn extract_best_bid_ask_from_book(
    raw: &serde_json::Value,
) -> (Option<f64>, Option<f64>) {
    // Polymarket /book endpoint returns bids ascending and asks descending.
    // The last item is therefore the best price on each side.
    let best_bid = raw
        .get("bids")
        .and_then(|value| value.as_array())
        .and_then(|rows| rows.last())
        .and_then(|row| row.get("price"))
        .and_then(parse_f64_value);
    let best_ask = raw
        .get("asks")
        .and_then(|value| value.as_array())
        .and_then(|rows| rows.last())
        .and_then(|row| row.get("price"))
        .and_then(parse_f64_value);
    (best_bid, best_ask)
}

#[async_trait]
impl ClobRestClient for ClobHttpClient {
    async fn get_price_snapshot(&self, market: &str) -> Result<PriceSnapshot> {
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

    async fn get_best_bid_ask(&self, token_id: &str) -> Result<(Option<f64>, Option<f64>)> {
        if token_id.trim().is_empty() {
            return Ok((None, None));
        }

        let request_path = format!("/book?token_id={token_id}");
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), request_path);
        let response = self.http.get(url).send().await?;
        if !response.status().is_success() {
            return Ok((None, None));
        }

        let raw: serde_json::Value = response.json().await?;
        Ok(extract_best_bid_ask_from_book(&raw))
    }

    async fn get_order_book(&self, token_id: &str) -> Result<Option<OrderBookSnapshot>> {
        if token_id.trim().is_empty() {
            return Ok(None);
        }

        let request_path = format!("/book?token_id={token_id}");
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), request_path);
        let response = self.http.get(url).send().await?;
        if !response.status().is_success() {
            return Ok(None);
        }

        let raw: serde_json::Value = response.json().await?;
        Ok(Some(extract_order_book_from_book(&raw)))
    }

    async fn get_last_trade_price(&self, token_id: &str) -> Result<Option<f64>> {
        if token_id.trim().is_empty() {
            return Ok(None);
        }

        let request_path = format!("/last-trade-price?token_id={token_id}");
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), request_path);
        let response = self.http.get(url).send().await?;
        if !response.status().is_success() {
            return Ok(None);
        }

        let raw: serde_json::Value = response.json().await?;
        Ok(raw
            .get("price")
            .or_else(|| raw.get("last"))
            .or_else(|| raw.get("last_trade_price"))
            .and_then(parse_f64_value))
    }

    async fn get_price_history(
        &self,
        token_id: &str,
        start_ts: i64,
        end_ts: i64,
        fidelity: i64,
    ) -> Result<Vec<PriceHistoryPoint>> {
        if token_id.trim().is_empty() || start_ts <= 0 || end_ts <= start_ts {
            return Ok(Vec::new());
        }

        let fidelity = fidelity.max(1);
        let request_path = format!(
            "/prices-history?market={token_id}&startTs={start_ts}&endTs={end_ts}&fidelity={fidelity}"
        );
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), request_path);
        let response = self.http.get(url).send().await?;
        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let raw: serde_json::Value = response.json().await?;
        Ok(raw
            .get("history")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(parse_price_history_point)
            .collect())
    }

    async fn get_clob_market_info(&self, condition_id: &str) -> Result<Option<ClobMarketInfo>> {
        let condition_id = condition_id.trim();
        if condition_id.is_empty() {
            return Ok(None);
        }
        if let Some(cached) = self.cached_market_info_by_condition(condition_id) {
            return Ok(Some(cached));
        }

        let request_path = format!("/clob-markets/{condition_id}");
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), request_path);
        let response = self.http.get(url).send().await?;
        if !response.status().is_success() {
            return Ok(None);
        }

        let raw: serde_json::Value = response.json().await?;
        let info = parse_clob_market_info_response(&raw, condition_id);
        if let Some(info) = info.as_ref() {
            self.cache_market_info(info);
        }
        Ok(info)
    }

    async fn get_clob_market_info_by_token(
        &self,
        token_id: &str,
    ) -> Result<Option<ClobMarketInfo>> {
        let token_id = token_id.trim();
        if token_id.is_empty() {
            return Ok(None);
        }
        if let Some(cached) = self.cached_market_info_by_token(token_id) {
            return Ok(Some(cached));
        }

        let request_path = format!("/markets-by-token/{token_id}");
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), request_path);
        let response = self.http.get(url).send().await?;
        if !response.status().is_success() {
            return Ok(None);
        }

        let raw: serde_json::Value = response.json().await?;
        let condition_id = raw
            .get("condition_id")
            .or_else(|| raw.get("conditionId"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        if condition_id.is_empty() {
            return Ok(None);
        }
        self.get_clob_market_info(&condition_id).await
    }

    async fn get_fee_rate_bps(&self, token_id: &str) -> Result<Option<u64>> {
        if token_id.trim().is_empty() {
            return Ok(None);
        }
        if let Some(info) = self.get_clob_market_info_by_token(token_id).await? {
            if info.has_token(token_id) {
                if let Some(taker_base_fee_bps) = info.taker_base_fee_bps {
                    return Ok(Some(taker_base_fee_bps));
                }
                if let Some(details) = info.fee_details {
                    return Ok(Some(details.rate.max(0.0).round() as u64));
                }
            }
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

        Ok(parse_fee_rate_bps_response(&raw))
    }

    async fn place_order(&self, req: &PlaceOrderRequest) -> Result<OrderAck> {
        let order_started = std::time::Instant::now();
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
        let cost_units = ((req.price * req.size * 10_000.0).round() as u64) * 100;
        let size_units = ((req.size * 100.0).round() as u64) * 10_000;

        let (maker_amount, taker_amount) = if is_buy {
            (U256::from(cost_units), U256::from(size_units))
        } else {
            (U256::from(size_units), U256::from(cost_units))
        };
        let side_u8: u8 = if is_buy { 0 } else { 1 };

        let salt_u32 = u32::from_be_bytes(Uuid::new_v4().as_bytes()[0..4].try_into().unwrap());
        let salt = U256::from(salt_u32);

        let eoa: Address = self
            .address
            .parse()
            .context("parse EOA address for EIP-712")?;

        let (maker, signer_addr, sig_type): (Address, Address, u64) = match self.gnosis_safe {
            Some(safe) => (safe, eoa, 2),
            None => (eoa, eoa, 0),
        };

        let effective_exchange_address = self.effective_exchange_address(req.neg_risk);
        let sign_started = std::time::Instant::now();
        let signature = match sign_order_eip712_with_domain_separator(
            &self.wallet,
            self.cached_domain_separator(req.neg_risk),
            salt,
            maker,
            signer_addr,
            token_id,
            maker_amount,
            taker_amount,
            side_u8,
            req.fee_rate_bps,
            sig_type,
        ) {
            Ok(signature) => signature,
            Err(err) => {
                let sign_ms = sign_started.elapsed().as_millis() as i64;
                tracing::info!(
                    market = %req.market,
                    token_id = token_id_str,
                    side = %req.side,
                    order_type = normalized_order_type,
                    neg_risk = req.neg_risk,
                    sign_ms,
                    http_ms = 0,
                    total_ms = order_started.elapsed().as_millis() as i64,
                    status = "sign_error",
                    client_order_id = %client_id,
                    exchange_order_id = tracing::field::Empty,
                    error = %err,
                    "ORDER_LATENCY_TRACE"
                );
                return Err(err).context("EIP-712 order signing failed");
            }
        };
        let sign_ms = sign_started.elapsed().as_millis() as i64;

        let maker_str = ethers::utils::to_checksum(&maker, None);
        let signer_str = ethers::utils::to_checksum(&signer_addr, None);
        let side_str = if is_buy { "BUY" } else { "SELL" };
        let salt_u64 = salt.low_u64();
        let body = build_place_order_body(
            salt_u64,
            &maker_str,
            &signer_str,
            token_id,
            maker_amount,
            taker_amount,
            side_str,
            sig_type,
            &signature,
            &self.api_key,
            normalized_order_type,
            req.fee_rate_bps,
        );

        tracing::warn!(
            side = side_u8,
            side_str,
            token_id = %token_id,
            maker_amount = %maker_amount,
            taker_amount = %taker_amount,
            salt = %salt,
            maker = %maker_str,
            signer = %signer_str,
            sig_type,
            fee_rate_bps = req.fee_rate_bps,
            builder = %self.builder_code_hex,
            exchange = %effective_exchange_address,
            chain_id = self.chain_id,
            order_type = normalized_order_type,
            neg_risk = req.neg_risk,
            sig_prefix = &signature[..20],
            body_json = %body,
            "PLACE_ORDER_EIP712_DEBUG"
        );

        let http_started = std::time::Instant::now();
        let response = match self.signed_json(Method::POST, "/order", Some(body)).await {
            Ok(response) => response,
            Err(err) => {
                let http_ms = http_started.elapsed().as_millis() as i64;
                tracing::info!(
                    market = %req.market,
                    token_id = token_id_str,
                    side = %req.side,
                    order_type = normalized_order_type,
                    neg_risk = req.neg_risk,
                    sign_ms,
                    http_ms,
                    total_ms = order_started.elapsed().as_millis() as i64,
                    status = "http_error",
                    client_order_id = %client_id,
                    exchange_order_id = tracing::field::Empty,
                    error = %err,
                    "ORDER_LATENCY_TRACE"
                );
                return Err(err);
            }
        };
        let raw: serde_json::Value = match response.json().await {
            Ok(raw) => raw,
            Err(err) => {
                let http_ms = http_started.elapsed().as_millis() as i64;
                tracing::info!(
                    market = %req.market,
                    token_id = token_id_str,
                    side = %req.side,
                    order_type = normalized_order_type,
                    neg_risk = req.neg_risk,
                    sign_ms,
                    http_ms,
                    total_ms = order_started.elapsed().as_millis() as i64,
                    status = "decode_error",
                    client_order_id = %client_id,
                    exchange_order_id = tracing::field::Empty,
                    error = %err,
                    "ORDER_LATENCY_TRACE"
                );
                return Err(err.into());
            }
        };
        let http_ms = http_started.elapsed().as_millis() as i64;

        let ack = OrderAck {
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
            sign_ms: Some(sign_ms),
            http_ms: Some(http_ms),
            total_ms: Some(order_started.elapsed().as_millis() as i64),
        };

        tracing::info!(
            market = %req.market,
            token_id = token_id_str,
            side = %req.side,
            order_type = normalized_order_type,
            neg_risk = req.neg_risk,
            sign_ms,
            http_ms,
            total_ms = order_started.elapsed().as_millis() as i64,
            status = %ack.status,
            client_order_id = %ack.client_order_id,
            exchange_order_id = ack.exchange_order_id.as_deref().unwrap_or(""),
            "ORDER_LATENCY_TRACE"
        );

        Ok(ack)
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
