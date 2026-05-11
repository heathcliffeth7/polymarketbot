use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GammaMarket {
    pub slug: String,
    pub condition_id: Option<String>,
    pub end_date_iso: Option<String>,
    pub active: bool,
    pub closed: bool,
    pub yes_token_id: Option<String>,
    pub no_token_id: Option<String>,
    pub maker_base_fee: u64,
    pub neg_risk: bool,
    pub order_price_min_tick_size: Option<f64>,
    pub order_min_size: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceSnapshot {
    pub market: String,
    pub price: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceHistoryPoint {
    pub ts: i64,
    pub price: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBookLevel {
    pub price: f64,
    pub size: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    pub bids: Vec<OrderBookLevel>,
    pub asks: Vec<OrderBookLevel>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClobMarketToken {
    pub token_id: String,
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClobMarketFeeDetails {
    pub rate: f64,
    pub exponent: f64,
    pub taker_only: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClobMarketInfo {
    pub condition_id: String,
    pub tokens: Vec<ClobMarketToken>,
    pub min_tick_size: Option<f64>,
    pub min_order_size: Option<f64>,
    pub maker_base_fee_bps: Option<u64>,
    pub taker_base_fee_bps: Option<u64>,
    pub fee_details: Option<ClobMarketFeeDetails>,
    pub neg_risk: bool,
}

impl ClobMarketInfo {
    pub fn has_token(&self, token_id: &str) -> bool {
        let token_id = token_id.trim();
        !token_id.is_empty() && self.tokens.iter().any(|token| token.token_id == token_id)
    }
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
    #[serde(default)]
    pub neg_risk: bool,
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
    pub prepare_ms: Option<i64>,
    pub sign_ms: Option<i64>,
    pub header_sign_ms: Option<i64>,
    pub http_ms: Option<i64>,
    pub decode_ms: Option<i64>,
    pub total_ms: Option<i64>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataApiActivity {
    pub activity_type: String,
    pub side: Option<String>,
    pub slug: String,
    pub asset: Option<String>,
    pub outcome: Option<String>,
    pub size: f64,
    pub usdc_size: f64,
    pub price: Option<f64>,
    pub timestamp: Option<i64>,
}

#[async_trait]
pub trait GammaClient: Send + Sync {
    async fn list_active_updown_markets(&self) -> Result<Vec<GammaMarket>>;

    async fn list_btc_5m_markets(&self) -> Result<Vec<GammaMarket>>;
}

#[async_trait]
pub trait ClobRestClient: Send + Sync {
    async fn get_price_snapshot(&self, market: &str) -> Result<PriceSnapshot>;

    async fn get_order_book(&self, _token_id: &str) -> Result<Option<OrderBookSnapshot>> {
        Ok(None)
    }

    async fn get_best_bid_ask(&self, _token_id: &str) -> Result<(Option<f64>, Option<f64>)> {
        Ok((None, None))
    }

    async fn get_last_trade_price(&self, _token_id: &str) -> Result<Option<f64>> {
        Ok(None)
    }

    async fn get_price_history(
        &self,
        _token_id: &str,
        _start_ts: i64,
        _end_ts: i64,
        _fidelity: i64,
    ) -> Result<Vec<PriceHistoryPoint>> {
        Ok(Vec::new())
    }

    async fn get_fee_rate_bps(&self, token_id: &str) -> Result<Option<u64>>;
    async fn get_clob_market_info(&self, _condition_id: &str) -> Result<Option<ClobMarketInfo>> {
        Ok(None)
    }
    async fn get_clob_market_info_by_token(
        &self,
        _token_id: &str,
    ) -> Result<Option<ClobMarketInfo>> {
        Ok(None)
    }
    async fn place_order(&self, req: &PlaceOrderRequest) -> Result<OrderAck>;
    async fn cancel_order(&self, exchange_order_id: &str) -> Result<()>;
    async fn get_order(&self, exchange_order_id: &str) -> Result<OrderInfo>;
    async fn list_open_orders(&self, market: Option<&str>) -> Result<Vec<OrderInfo>>;
    async fn list_fills(&self, next_cursor: Option<&str>) -> Result<Vec<FillInfo>>;
    async fn get_balance(&self) -> Result<f64>;

    async fn warmup_order_connection(&self) -> Result<()> {
        let _ = self.list_open_orders(None).await?;
        Ok(())
    }

    async fn get_token_inventory(&self, _token_id: &str) -> Result<Option<f64>> {
        Ok(None)
    }
}
