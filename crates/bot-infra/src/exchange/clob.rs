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
    ) -> Self {
        let address = creds.address.clone();
        let api_key = creds.key.clone();
        let domain_separator = domain_separator_for_exchange(chain_id, exchange_address);
        let neg_risk_domain_separator = neg_risk_exchange_address
            .map(|address| domain_separator_for_exchange(chain_id, address));
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

        let fee_rate_bps = req.fee_rate_bps;
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
            fee_rate_bps,
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
