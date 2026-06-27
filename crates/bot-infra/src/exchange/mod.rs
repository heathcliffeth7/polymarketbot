use crate::signer::{
    domain_separator_for_exchange, sign_order_eip712_with_domain_separator, unix_now_millis,
    unix_now_secs, ApiCredentials, ClobHeaderSigner, HeaderSigner,
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
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

mod clob;
mod data_api;
mod gamma;
mod http;
mod inventory_snapshot;
mod models;
mod order_egress;
mod parse;
#[cfg(test)]
mod tests;

pub use clob::ClobHttpClient;
pub use data_api::PolymarketDataApiClient;
pub use gamma::GammaHttpClient;
pub(crate) use http::{build_http_client, build_order_http_client};
pub use models::{
    ClobMarketFeeDetails, ClobMarketInfo, ClobMarketToken, ClobRestClient, DataApiActivity,
    FillInfo, FillPage, GammaClient, GammaMarket, OrderAck, OrderBookFetchResult, OrderBookLevel,
    OrderBookSnapshot, OrderInfo, PlaceOrderRequest, PriceHistoryPoint, PriceSnapshot,
    TokenInventorySnapshot, TradeQuery,
};
pub(crate) use parse::{
    parse_f64_value, parse_gamma_market, parse_gamma_market_any, parse_json_f64,
    DataApiInventoryPosition,
};
