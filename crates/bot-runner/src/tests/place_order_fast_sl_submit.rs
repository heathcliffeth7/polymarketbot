use super::support::*;
use super::*;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

struct FreshSlSubmitExecutor {
    order_books: Mutex<VecDeque<OrderBookSnapshot>>,
    place_calls: Mutex<Vec<PlaceOrderRequest>>,
    place_errors_before_success: Mutex<usize>,
    order_book_calls: AtomicUsize,
}

#[async_trait::async_trait]
impl OrderExecutor for FreshSlSubmitExecutor {
    async fn midpoint(&self, _market: &str) -> Result<bot_infra::exchange::PriceSnapshot> {
        anyhow::bail!("unused")
    }

    async fn best_bid_ask(&self, _token_id: &str) -> Result<(Option<f64>, Option<f64>)> {
        anyhow::bail!("unused")
    }

    async fn order_book(&self, _token_id: &str) -> Result<Option<OrderBookSnapshot>> {
        self.order_book_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.order_books.lock().expect("order books").pop_front())
    }

    async fn last_trade_price(&self, _token_id: &str) -> Result<Option<f64>> {
        anyhow::bail!("unused")
    }

    async fn fee_rate_bps(&self, _token_id: &str) -> Result<Option<u64>> {
        Ok(Some(0))
    }

    async fn place(&self, req: &PlaceOrderRequest) -> Result<bot_infra::exchange::OrderAck> {
        self.place_calls
            .lock()
            .expect("place calls")
            .push(req.clone());
        {
            let mut errors_remaining = self
                .place_errors_before_success
                .lock()
                .expect("place errors");
            if *errors_remaining > 0 {
                *errors_remaining -= 1;
                anyhow::bail!("HTTP 400: no orders found to match with FAK order");
            }
        }

        Ok(bot_infra::exchange::OrderAck {
            client_order_id: req.client_order_id.clone(),
            exchange_order_id: Some("fresh-sl-order".to_string()),
            status: "matched".to_string(),
            reject_reason: None,
            raw_status: Some("matched".to_string()),
            exchange_ts: Some(Utc::now().timestamp_millis()),
            prepare_ms: None,
            sign_ms: None,
            header_sign_ms: None,
            http_ms: None,
            decode_ms: None,
            total_ms: None,
        })
    }

    async fn cancel(&self, _exchange_order_id: &str) -> Result<()> {
        anyhow::bail!("unused")
    }

    async fn status(&self, _exchange_order_id: &str) -> Result<OrderInfo> {
        anyhow::bail!("unused")
    }

    async fn list_open(&self, _market: Option<&str>) -> Result<Vec<OrderInfo>> {
        anyhow::bail!("unused")
    }

    async fn list_fills(&self, _next_cursor: Option<&str>) -> Result<Vec<FillInfo>> {
        anyhow::bail!("unused")
    }

    async fn available_token_qty(&self, _token_id: &str) -> Result<Option<f64>> {
        Ok(Some(10.0))
    }
}

fn sl_submit_executor(
    order_books: Vec<OrderBookSnapshot>,
    errors_before_success: usize,
) -> FreshSlSubmitExecutor {
    FreshSlSubmitExecutor {
        order_books: Mutex::new(VecDeque::from(order_books)),
        place_calls: Mutex::new(Vec::new()),
        place_errors_before_success: Mutex::new(errors_before_success),
        order_book_calls: AtomicUsize::new(0),
    }
}

fn fast_sl_order() -> TradeBuilderOrder {
    let mut order = test_builder_order("sell", Some(9));
    order.size_basis = TRADE_BUILDER_SIZE_BASIS_SHARES.to_string();
    order.target_qty = Some(5.0);
    order.remaining_qty = Some(5.0);
    order.trigger_condition = Some("cross_below".to_string());
    order.trigger_price = Some(0.70);
    order.trigger_latched = true;
    order.trigger_latched_reason = Some("stop_loss".to_string());
    order.trigger_latched_at = Some(Utc::now());
    order
}

fn fast_sl_order_book(best_bid: f64) -> OrderBookSnapshot {
    OrderBookSnapshot {
        bids: vec![bot_infra::exchange::OrderBookLevel {
            price: best_bid,
            size: 10.0,
        }],
        asks: vec![bot_infra::exchange::OrderBookLevel {
            price: best_bid + 0.02,
            size: 10.0,
        }],
    }
}

fn fast_sl_quote(order: &TradeBuilderOrder, best_bid: f64) -> ExitFastSubmitQuote {
    ExitFastSubmitQuote {
        token_id: order.token_id.clone(),
        captured_at: Utc::now(),
        market_updated_at_ms: 123,
        order_book: fast_sl_order_book(best_bid),
        best_bid: Some(best_bid),
        best_ask: Some(best_bid + 0.02),
        last_trade_price: Some(best_bid),
    }
}

fn fast_sl_request(order: &TradeBuilderOrder, price: f64) -> PlaceOrderRequest {
    PlaceOrderRequest {
        market: order.market_slug.clone(),
        token_id: Some(order.token_id.clone()),
        side: order.side.clone(),
        price,
        size: 5.0,
        intent: "exit".to_string(),
        order_type: "FAK".to_string(),
        client_order_id: "base-client-order".to_string(),
        leg_side: None,
        fee_rate_bps: 0,
        neg_risk: false,
    }
}

#[tokio::test]
async fn initial_stop_loss_submit_uses_fresh_exit_fast_quote_depth() {
    let executor = sl_submit_executor(Vec::new(), 0);
    let ws = ClobWsClient::new("ws://127.0.0.1:0".to_string());
    let order = fast_sl_order();
    let quote = fast_sl_quote(&order, 0.67);

    let price = resolve_trade_builder_fast_stop_loss_initial_submit_price(
        &executor,
        &ws,
        &order,
        0.56,
        Some(0.56),
        Some(0.56),
        Some(5.0),
        TRADE_BUILDER_SIZE_BASIS_SHARES,
        Some(&quote),
    )
    .await
    .expect("fresh initial sl submit price");

    assert_eq!(price.quote_source, "exit_fast_quote");
    assert_eq!(price.sell_submit_price.source, "orderbook_depth");
    assert!((price.sell_submit_price.desired_price - 0.66).abs() < 1e-9);
    assert_eq!(executor.order_book_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn initial_stop_loss_submit_fetches_fresh_book_instead_of_stale_loop_price() {
    let executor = sl_submit_executor(vec![fast_sl_order_book(0.67)], 0);
    let ws = ClobWsClient::new("ws://127.0.0.1:0".to_string());
    let order = fast_sl_order();

    let price = resolve_trade_builder_fast_stop_loss_initial_submit_price(
        &executor,
        &ws,
        &order,
        0.56,
        Some(0.56),
        Some(0.56),
        Some(5.0),
        TRADE_BUILDER_SIZE_BASIS_SHARES,
        None,
    )
    .await
    .expect("fresh initial sl submit price");

    assert_eq!(price.quote_source, "rest_orderbook");
    assert!((price.sell_submit_price.desired_price - 0.66).abs() < 1e-9);
    assert_ne!(price.sell_submit_price.desired_price, 0.56);
    assert_eq!(executor.order_book_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn initial_stop_loss_submit_does_not_reuse_stale_loop_price_without_fresh_bid() {
    let executor = sl_submit_executor(
        vec![OrderBookSnapshot {
            bids: Vec::new(),
            asks: Vec::new(),
        }],
        0,
    );
    let ws = ClobWsClient::new("ws://127.0.0.1:0".to_string());
    let order = fast_sl_order();

    let price = resolve_trade_builder_fast_stop_loss_initial_submit_price(
        &executor,
        &ws,
        &order,
        0.56,
        Some(0.56),
        Some(0.56),
        Some(5.0),
        TRADE_BUILDER_SIZE_BASIS_SHARES,
        None,
    )
    .await;

    assert!(price.is_none());
    assert_eq!(executor.order_book_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn fast_stop_loss_retry_can_use_five_fresh_attempts_in_same_pass() {
    let executor = sl_submit_executor(
        vec![
            fast_sl_order_book(0.61),
            fast_sl_order_book(0.60),
            fast_sl_order_book(0.59),
            fast_sl_order_book(0.58),
            fast_sl_order_book(0.57),
        ],
        4,
    );
    let ws = ClobWsClient::new("ws://127.0.0.1:0".to_string());
    let order = fast_sl_order();
    let base_req = fast_sl_request(&order, 0.63);

    let outcome = try_trade_builder_fast_stop_loss_retry(
        &executor,
        &ws,
        &order,
        &base_req,
        "FAK",
        TRADE_BUILDER_SIZE_BASIS_SHARES,
        "HTTP 400: no orders found to match with FAK order",
        0.63,
        Some(0.63),
        Some(0.63),
        Some(5.0),
    )
    .await;

    let TradeBuilderFastStopLossRetryOutcome::Success(success) = outcome else {
        panic!("expected fifth fast stop-loss retry success");
    };
    assert_eq!(success.attempt_payload["fast_sl_retry_attempt"], json!(5));

    let calls = executor.place_calls.lock().expect("place calls");
    assert_eq!(calls.len(), 5);
    assert!((calls[0].price - 0.60).abs() < 1e-9);
    assert!((calls[4].price - 0.56).abs() < 1e-9);
    assert_eq!(executor.order_book_calls.load(Ordering::SeqCst), 5);
}
