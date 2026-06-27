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
pub struct OrderBookFetchResult {
    pub snapshot: Option<OrderBookSnapshot>,
    pub error_kind: Option<String>,
    pub error_code: Option<u16>,
    pub error_reason: Option<String>,
}

impl OrderBookFetchResult {
    pub fn ok(snapshot: Option<OrderBookSnapshot>) -> Self {
        Self {
            snapshot,
            error_kind: None,
            error_code: None,
            error_reason: None,
        }
    }

    pub fn failed(
        error_kind: impl Into<String>,
        error_code: Option<u16>,
        error_reason: impl Into<String>,
    ) -> Self {
        Self {
            snapshot: None,
            error_kind: Some(error_kind.into()),
            error_code,
            error_reason: Some(error_reason.into()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TokenInventorySnapshot {
    qty_by_alias: std::collections::HashMap<String, f64>,
    row_count: usize,
}

impl TokenInventorySnapshot {
    pub fn token_qty(&self, token_id: &str) -> Option<f64> {
        let token_id = token_id.trim();
        if token_id.is_empty() {
            return None;
        }
        Some(
            self.qty_by_alias
                .get(token_id)
                .copied()
                .unwrap_or_default()
                .max(0.0),
        )
    }

    pub fn row_count(&self) -> usize {
        self.row_count
    }

    pub fn alias_count(&self) -> usize {
        self.qty_by_alias.len()
    }

    pub(crate) fn add_position_row<I, S>(&mut self, aliases: I, qty: f64)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.row_count = self.row_count.saturating_add(1);
        let mut row_aliases = std::collections::HashSet::new();
        for alias in aliases {
            let alias = alias.as_ref().trim();
            if alias.is_empty() || !row_aliases.insert(alias.to_string()) {
                continue;
            }
            *self.qty_by_alias.entry(alias.to_string()).or_default() += qty;
        }
    }
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
    #[serde(default)]
    pub post_only: bool,
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
    #[serde(default)]
    pub associated_trade_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillInfo {
    pub fill_id: String,
    pub order_id: String,
    pub price: f64,
    pub size: f64,
    pub fee: Option<f64>,
    pub ts: Option<i64>,
    #[serde(default)]
    pub raw_payload: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TradeQuery {
    pub id: Option<String>,
    pub maker_address: Option<String>,
    pub market: Option<String>,
    pub asset_id: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillPage {
    pub fills: Vec<FillInfo>,
    pub next_cursor: Option<String>,
    pub count: Option<usize>,
    pub raw_count: usize,
    #[serde(default)]
    pub first_row_keys: Vec<String>,
}

impl FillPage {
    pub fn from_fills(fills: Vec<FillInfo>) -> Self {
        let raw_count = fills.len();
        Self {
            fills,
            next_cursor: None,
            count: Some(raw_count),
            raw_count,
            first_row_keys: Vec::new(),
        }
    }
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

    async fn get_order_book_with_diagnostics(
        &self,
        token_id: &str,
    ) -> Result<OrderBookFetchResult> {
        self.get_order_book(token_id)
            .await
            .map(OrderBookFetchResult::ok)
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
    async fn list_fills_page(&self, query: TradeQuery) -> Result<FillPage> {
        Ok(FillPage::from_fills(
            self.list_fills(query.next_cursor.as_deref()).await?,
        ))
    }
    async fn get_balance(&self) -> Result<f64>;

    async fn warmup_order_connection(&self) -> Result<()> {
        let _ = self.list_open_orders(None).await?;
        Ok(())
    }

    async fn get_token_inventory(&self, _token_id: &str) -> Result<Option<f64>> {
        Ok(None)
    }

    async fn get_token_inventory_snapshot(&self) -> Result<Option<TokenInventorySnapshot>> {
        Ok(None)
    }
}

#[cfg(test)]
mod token_inventory_snapshot_tests {
    use super::*;

    #[test]
    fn same_row_duplicate_alias_is_not_double_counted() {
        let mut snapshot = TokenInventorySnapshot::default();
        snapshot.add_position_row(["T1", "T1", "T1"], 5.0);

        assert_eq!(snapshot.token_qty("T1"), Some(5.0));
        assert_eq!(snapshot.row_count(), 1);
        assert_eq!(snapshot.alias_count(), 1);
    }

    #[test]
    fn different_rows_same_token_are_summed() {
        let mut snapshot = TokenInventorySnapshot::default();
        snapshot.add_position_row(["T1"], 5.0);
        snapshot.add_position_row(["T1"], 2.0);

        assert_eq!(snapshot.token_qty("T1"), Some(7.0));
        assert_eq!(snapshot.row_count(), 2);
        assert_eq!(snapshot.alias_count(), 1);
    }

    #[test]
    fn aliases_from_one_row_share_qty_once_each() {
        let mut snapshot = TokenInventorySnapshot::default();
        snapshot.add_position_row(["asset-1", "token-1", "clob-1"], 3.0);

        assert_eq!(snapshot.token_qty("asset-1"), Some(3.0));
        assert_eq!(snapshot.token_qty("token-1"), Some(3.0));
        assert_eq!(snapshot.token_qty("clob-1"), Some(3.0));
        assert_eq!(snapshot.alias_count(), 3);
    }
}
