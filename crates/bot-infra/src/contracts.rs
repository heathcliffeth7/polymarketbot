use crate::db::PostgresRepository;
use crate::exchange::{
    ClobRestClient, FillInfo, OrderAck, OrderInfo, PlaceOrderRequest, PriceSnapshot,
};
pub use crate::market_data::MarketDataProvider;
use anyhow::Result;
use async_trait::async_trait;
use bot_core::TradeState;

#[async_trait]
pub trait OrderExecutor: Send + Sync {
    async fn midpoint(&self, market: &str) -> Result<PriceSnapshot>;
    async fn place(&self, req: &PlaceOrderRequest) -> Result<OrderAck>;
    async fn cancel(&self, exchange_order_id: &str) -> Result<()>;
    async fn status(&self, exchange_order_id: &str) -> Result<OrderInfo>;
    async fn list_open(&self, market: Option<&str>) -> Result<Vec<OrderInfo>>;
    async fn list_fills(&self, next_cursor: Option<&str>) -> Result<Vec<FillInfo>>;
    async fn available_token_qty(&self, token_id: &str) -> Result<Option<f64>>;

    async fn replace(&self, exchange_order_id: &str, req: &PlaceOrderRequest) -> Result<OrderAck> {
        self.cancel(exchange_order_id).await?;
        self.place(req).await
    }
}

#[async_trait]
impl<T> OrderExecutor for T
where
    T: ClobRestClient + Send + Sync,
{
    async fn midpoint(&self, market: &str) -> Result<PriceSnapshot> {
        ClobRestClient::get_price_snapshot(self, market).await
    }

    async fn place(&self, req: &PlaceOrderRequest) -> Result<OrderAck> {
        ClobRestClient::place_order(self, req).await
    }

    async fn cancel(&self, exchange_order_id: &str) -> Result<()> {
        ClobRestClient::cancel_order(self, exchange_order_id).await
    }

    async fn status(&self, exchange_order_id: &str) -> Result<OrderInfo> {
        ClobRestClient::get_order(self, exchange_order_id).await
    }

    async fn list_open(&self, market: Option<&str>) -> Result<Vec<OrderInfo>> {
        ClobRestClient::list_open_orders(self, market).await
    }

    async fn list_fills(&self, next_cursor: Option<&str>) -> Result<Vec<FillInfo>> {
        ClobRestClient::list_fills(self, next_cursor).await
    }

    async fn available_token_qty(&self, token_id: &str) -> Result<Option<f64>> {
        ClobRestClient::get_token_inventory(self, token_id).await
    }
}

#[async_trait]
pub trait StateRepository: Send + Sync {
    async fn trade_state(&self, trade_id: i64) -> Result<TradeState>;
    async fn transition_trade_state(
        &self,
        trade_id: i64,
        from: TradeState,
        to: TradeState,
        reason: &str,
    ) -> Result<()>;
    async fn record_risk_event(
        &self,
        trade_id: Option<i64>,
        event_type: &str,
        decision: &str,
        details: &str,
    ) -> Result<()>;
    async fn mark_order_status(&self, exchange_order_id: &str, status: &str) -> Result<()>;
    async fn open_exchange_order_ids_for_trade(&self, trade_id: i64) -> Result<Vec<String>>;
}

#[async_trait]
impl StateRepository for PostgresRepository {
    async fn trade_state(&self, trade_id: i64) -> Result<TradeState> {
        PostgresRepository::trade_state(self, trade_id).await
    }

    async fn transition_trade_state(
        &self,
        trade_id: i64,
        from: TradeState,
        to: TradeState,
        reason: &str,
    ) -> Result<()> {
        PostgresRepository::transition_trade_state(self, trade_id, from, to, reason).await
    }

    async fn record_risk_event(
        &self,
        trade_id: Option<i64>,
        event_type: &str,
        decision: &str,
        details: &str,
    ) -> Result<()> {
        PostgresRepository::record_risk_event(self, trade_id, event_type, decision, details).await
    }

    async fn mark_order_status(&self, exchange_order_id: &str, status: &str) -> Result<()> {
        PostgresRepository::mark_order_status(self, exchange_order_id, status).await
    }

    async fn open_exchange_order_ids_for_trade(&self, trade_id: i64) -> Result<Vec<String>> {
        PostgresRepository::open_exchange_order_ids_for_trade(self, trade_id).await
    }
}
