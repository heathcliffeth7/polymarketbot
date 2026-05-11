use crate::db::PostgresRepository;
use crate::exchange::{
    ClobMarketInfo, ClobRestClient, FillInfo, OrderAck, OrderBookSnapshot, OrderInfo,
    PlaceOrderRequest, PriceHistoryPoint, PriceSnapshot,
};
pub use crate::market_data::MarketDataProvider;
use anyhow::Result;
use async_trait::async_trait;
use bot_core::TradeState;

#[async_trait]
pub trait OrderExecutor: Send + Sync {
    async fn midpoint(&self, market: &str) -> Result<PriceSnapshot>;
    async fn order_book(&self, _token_id: &str) -> Result<Option<OrderBookSnapshot>> {
        Ok(None)
    }
    async fn best_bid_ask(&self, _token_id: &str) -> Result<(Option<f64>, Option<f64>)> {
        Ok((None, None))
    }
    async fn last_trade_price(&self, _token_id: &str) -> Result<Option<f64>> {
        Ok(None)
    }
    async fn price_history(
        &self,
        _token_id: &str,
        _start_ts: i64,
        _end_ts: i64,
        _fidelity: i64,
    ) -> Result<Vec<PriceHistoryPoint>> {
        Ok(Vec::new())
    }
    async fn fee_rate_bps(&self, token_id: &str) -> Result<Option<u64>>;
    async fn clob_market_info_by_token(&self, _token_id: &str) -> Result<Option<ClobMarketInfo>> {
        Ok(None)
    }
    async fn place(&self, req: &PlaceOrderRequest) -> Result<OrderAck>;
    async fn cancel(&self, exchange_order_id: &str) -> Result<()>;
    async fn status(&self, exchange_order_id: &str) -> Result<OrderInfo>;
    async fn list_open(&self, market: Option<&str>) -> Result<Vec<OrderInfo>>;
    async fn list_fills(&self, next_cursor: Option<&str>) -> Result<Vec<FillInfo>>;
    async fn available_token_qty(&self, token_id: &str) -> Result<Option<f64>>;
    async fn warmup_order_connection(&self) -> Result<()> {
        Ok(())
    }

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

    async fn best_bid_ask(&self, token_id: &str) -> Result<(Option<f64>, Option<f64>)> {
        ClobRestClient::get_best_bid_ask(self, token_id).await
    }

    async fn order_book(&self, token_id: &str) -> Result<Option<OrderBookSnapshot>> {
        ClobRestClient::get_order_book(self, token_id).await
    }

    async fn last_trade_price(&self, token_id: &str) -> Result<Option<f64>> {
        ClobRestClient::get_last_trade_price(self, token_id).await
    }

    async fn price_history(
        &self,
        token_id: &str,
        start_ts: i64,
        end_ts: i64,
        fidelity: i64,
    ) -> Result<Vec<PriceHistoryPoint>> {
        ClobRestClient::get_price_history(self, token_id, start_ts, end_ts, fidelity).await
    }

    async fn place(&self, req: &PlaceOrderRequest) -> Result<OrderAck> {
        ClobRestClient::place_order(self, req).await
    }

    async fn fee_rate_bps(&self, token_id: &str) -> Result<Option<u64>> {
        ClobRestClient::get_fee_rate_bps(self, token_id).await
    }

    async fn clob_market_info_by_token(&self, token_id: &str) -> Result<Option<ClobMarketInfo>> {
        ClobRestClient::get_clob_market_info_by_token(self, token_id).await
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

    async fn warmup_order_connection(&self) -> Result<()> {
        ClobRestClient::warmup_order_connection(self).await
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
