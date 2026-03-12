use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GammaMarket {
    pub slug: String,
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

#[async_trait]
pub trait GammaClient: Send + Sync {
    async fn list_active_updown_markets(&self) -> Result<Vec<GammaMarket>>;

    async fn list_btc_5m_markets(&self) -> Result<Vec<GammaMarket>>;
}

#[async_trait]
pub trait ClobRestClient: Send + Sync {
    async fn get_price_snapshot(&self, market: &str) -> Result<PriceSnapshot>;

    async fn get_best_bid_ask(&self, _token_id: &str) -> Result<(Option<f64>, Option<f64>)> {
        Ok((None, None))
    }

    async fn get_last_trade_price(&self, _token_id: &str) -> Result<Option<f64>> {
        Ok(None)
    }

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
