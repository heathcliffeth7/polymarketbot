use anyhow::Result;
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderRow {
    order_id: String,
    client_order_id: Option<String>,
    market: String,
    side: String,
    price: f64,
    size: f64,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FillRow {
    fill_id: String,
    order_id: String,
    price: f64,
    size: f64,
    fee: f64,
    timestamp: i64,
}

#[derive(Debug, Default)]
struct ExchangeState {
    orders: HashMap<String, OrderRow>,
    fills: Vec<FillRow>,
    midpoint: f64,
}

#[derive(Clone)]
struct AppState {
    inner: Arc<Mutex<ExchangeState>>,
}

#[derive(Debug)]
pub struct MockExchangeHandle {
    pub addr: SocketAddr,
    shutdown_tx: oneshot::Sender<()>,
}

impl MockExchangeHandle {
    pub fn base_http(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn base_ws(&self) -> String {
        format!("ws://{}/ws", self.addr)
    }

    pub fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
    }
}

pub async fn spawn_mock_exchange() -> Result<MockExchangeHandle> {
    let state = AppState {
        inner: Arc::new(Mutex::new(ExchangeState {
            midpoint: 0.60,
            ..ExchangeState::default()
        })),
    };

    let app = Router::new()
        .route("/midpoint", get(get_midpoint))
        .route("/fee-rate", get(get_fee_rate))
        .route("/clob-markets/:condition_id", get(get_clob_market))
        .route("/markets-by-token/:token_id", get(get_market_by_token))
        .route("/order", post(post_order).delete(delete_order))
        .route("/data/order/:id", get(get_order))
        .route("/data/orders", get(get_orders))
        .route("/data/trades", get(get_trades))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let (tx, rx) = oneshot::channel();

    tokio::spawn(async move {
        let server = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = rx.await;
        });
        if let Err(err) = server.await {
            eprintln!("mock-exchange server error: {err}");
        }
    });

    info!(%addr, "mock exchange started");
    Ok(MockExchangeHandle {
        addr,
        shutdown_tx: tx,
    })
}

async fn get_midpoint(
    Query(q): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let market = q
        .get("market")
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let price = state.inner.lock().await.midpoint;
    Json(json!({ "market": market, "price": price }))
}

async fn get_fee_rate() -> impl IntoResponse {
    Json(json!({ "fee_rate_bps": 1000 }))
}

async fn get_clob_market(Path(condition_id): Path<String>) -> impl IntoResponse {
    let yes_token_id = condition_id.clone();
    let no_token_id = format!("{condition_id}-no");
    Json(json!({
        "condition_id": condition_id,
        "t": [
            { "t": yes_token_id, "o": "YES" },
            { "t": no_token_id, "o": "NO" }
        ],
        "mts": 0.01,
        "mos": 5.0,
        "nr": false,
        "fd": { "r": 0.0, "e": 0.0, "to": true },
        "mbf": 0,
        "tbf": 0
    }))
}

async fn get_market_by_token(Path(token_id): Path<String>) -> impl IntoResponse {
    Json(json!({ "condition_id": token_id }))
}

async fn post_order(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Response {
    // Handle both old flat format and new EIP-712 nested format
    let (market, side, price, size, client_order_id) = if let Some(order) = body.get("order") {
        let required_fields = [
            "salt",
            "maker",
            "signer",
            "taker",
            "tokenId",
            "makerAmount",
            "takerAmount",
            "side",
            "expiration",
            "nonce",
            "feeRateBps",
            "signatureType",
            "signature",
        ];
        let missing_fields = required_fields
            .iter()
            .copied()
            .filter(|field| order.get(field).is_none())
            .collect::<Vec<_>>();
        if !missing_fields.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "missing CLOB V2 order fields",
                    "fields": missing_fields
                })),
            )
                .into_response();
        }
        let forbidden_fields = ["timestamp", "metadata", "builder"]
            .iter()
            .copied()
            .filter(|field| order.get(field).is_some())
            .collect::<Vec<_>>();
        if !forbidden_fields.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "unsupported CLOB order fields",
                    "fields": forbidden_fields
                })),
            )
                .into_response();
        }

        // EIP-712 format: { "order": { "tokenId", "side", "makerAmount", "takerAmount", ... }, "owner", "orderType" }
        let token_id = order
            .get("tokenId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let side_raw = order.get("side").and_then(|v| v.as_str()).unwrap_or("0");
        let is_buy = side_raw == "0" || side_raw.eq_ignore_ascii_case("buy");
        let side_str = if is_buy { "buy" } else { "sell" }.to_string();
        let maker_amount: f64 = order
            .get("makerAmount")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        let taker_amount: f64 = order
            .get("takerAmount")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0);
        // Decode price and size from amounts (both 6 decimals)
        let (price, size) = if is_buy {
            // BUY: makerAmount=USDC, takerAmount=shares
            let shares = taker_amount / 1_000_000.0;
            let usdc = maker_amount / 1_000_000.0;
            let p = if shares > 0.0 { usdc / shares } else { 0.0 };
            (p, shares)
        } else {
            // SELL: makerAmount=shares, takerAmount=USDC
            let shares = maker_amount / 1_000_000.0;
            let usdc = taker_amount / 1_000_000.0;
            let p = if shares > 0.0 { usdc / shares } else { 0.0 };
            (p, shares)
        };
        let coid = body
            .get("owner")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        (token_id, side_str, price, size, coid)
    } else {
        // Legacy flat format
        let token_id = body
            .get("tokenID")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let market = body
            .get("market")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let market = if token_id.is_empty() {
            market
        } else {
            token_id
        };
        let side_str = body
            .get("side")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let price = body.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let size = body.get("size").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let coid = body
            .get("clientOrderId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        (market, side_str, price, size, coid)
    };

    let mut g = state.inner.lock().await;
    let id = Uuid::new_v4().to_string();

    let row = OrderRow {
        order_id: id.clone(),
        client_order_id,
        market,
        side,
        price,
        size,
        status: "open".to_string(),
    };

    g.orders.insert(id.clone(), row.clone());

    if (row.price - g.midpoint).abs() <= 0.02 {
        let fill = FillRow {
            fill_id: Uuid::new_v4().to_string(),
            order_id: id.clone(),
            price: row.price,
            size: row.size,
            fee: 0.0,
            timestamp: chrono::Utc::now().timestamp(),
        };
        g.fills.push(fill);
        if let Some(existing) = g.orders.get_mut(&id) {
            existing.status = "filled".to_string();
        }
    }

    Json(json!({
        "orderID": id,
        "status": g.orders.get(&id).map(|x| x.status.clone()).unwrap_or_else(|| "open".to_string())
    }))
    .into_response()
}

#[derive(Debug, Deserialize)]
struct CancelBody {
    #[serde(rename = "orderID")]
    order_id: String,
}

async fn delete_order(
    State(state): State<AppState>,
    Json(body): Json<CancelBody>,
) -> impl IntoResponse {
    let mut g = state.inner.lock().await;
    if let Some(row) = g.orders.get_mut(&body.order_id) {
        row.status = "canceled".to_string();
    }
    Json(json!({ "ok": true }))
}

async fn get_order(Path(id): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    let g = state.inner.lock().await;
    if let Some(o) = g.orders.get(&id) {
        return Json(json!({
            "orderID": o.order_id,
            "clientOrderId": o.client_order_id,
            "status": o.status,
            "price": o.price,
            "size": o.size,
            "filledSize": if o.status == "filled" { o.size } else { 0.0 }
        }))
        .into_response();
    }
    (StatusCode::NOT_FOUND, Json(json!({ "error": "not found" }))).into_response()
}

async fn get_orders(
    Query(q): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let market = q.get("market").cloned();
    let g = state.inner.lock().await;
    let mut rows = Vec::new();
    for o in g.orders.values() {
        if o.status != "open" && o.status != "partially_filled" {
            continue;
        }
        if let Some(m) = market.as_ref() {
            if &o.market != m {
                continue;
            }
        }
        rows.push(json!({
            "orderID": o.order_id,
            "clientOrderId": o.client_order_id,
            "status": o.status,
            "price": o.price,
            "size": o.size,
            "filledSize": if o.status == "filled" { o.size } else { 0.0 }
        }));
    }
    Json(json!({ "data": rows, "next_cursor": "LTE=" }))
}

async fn get_trades(State(state): State<AppState>) -> impl IntoResponse {
    let g = state.inner.lock().await;
    let rows: Vec<_> = g
        .fills
        .iter()
        .map(|f| {
            json!({
                "id": f.fill_id,
                "orderID": f.order_id,
                "price": f.price,
                "size": f.size,
                "fee": f.fee,
                "timestamp": f.timestamp
            })
        })
        .collect();
    Json(json!({ "data": rows, "next_cursor": "LTE=" }))
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(async move |mut socket| {
        use axum::extract::ws::Message;
        let welcome = json!({"type":"connected"}).to_string();
        let _ = socket.send(Message::Text(welcome)).await;

        if let Some(Ok(Message::Text(input))) = socket.recv().await {
            let ack = json!({"type":"subscribed","raw":input}).to_string();
            let _ = socket.send(Message::Text(ack)).await;
            let evt = json!({"type":"price_change","price":"0.61"}).to_string();
            let _ = socket.send(Message::Text(evt)).await;
        }
        let _ = socket.close().await;
    })
}
