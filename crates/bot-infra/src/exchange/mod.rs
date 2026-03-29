use crate::signer::{
    domain_separator_for_exchange, sign_order_eip712_with_domain_separator, unix_now_secs,
    ApiCredentials, ClobHeaderSigner, HeaderSigner,
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
use std::sync::Arc;
use uuid::Uuid;

mod clob;
mod gamma;
mod http;
mod models;
mod parse;
#[cfg(test)]
mod tests;

pub use clob::ClobHttpClient;
pub use gamma::GammaHttpClient;
pub(crate) use http::build_http_client;
pub use models::{
    ClobRestClient, FillInfo, GammaClient, GammaMarket, OrderAck, OrderBookLevel,
    OrderBookSnapshot, OrderInfo, PlaceOrderRequest, PriceSnapshot,
};
pub(crate) use parse::{
    data_api_position_matches_token, parse_f64_value, parse_gamma_market, parse_gamma_market_any,
    parse_json_f64, DataApiInventoryPosition,
};
