use super::clob::{extract_best_bid_ask_from_book, parse_fee_rate_bps_response};
use super::parse::{parse_gamma_market, parse_gamma_market_any, parse_yes_no_token_ids};
use super::{ClobHttpClient, ClobRestClient, PlaceOrderRequest};
use crate::signer::ApiCredentials;
use anyhow::Result;
use ethers::{signers::LocalWallet, types::Address};
use mock_exchange::spawn_mock_exchange;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn place_and_reconcile_against_mock_exchange() -> Result<()> {
    let mock = spawn_mock_exchange().await?;
    let wallet = "0000000000000000000000000000000000000000000000000000000000000001"
        .parse::<LocalWallet>()
        .unwrap();
    let dummy_addr = Address::zero();
    let client = ClobHttpClient::from_credentials(
        mock.base_http(),
        None,
        0,
        0,
        ApiCredentials {
            address: "0x0000000000000000000000000000000000000000".to_string(),
            key: "k".to_string(),
            secret: "YWFhYQ==".to_string(),
            passphrase: "p".to_string(),
        },
        wallet,
        dummy_addr,
        Some(Address::from_low_u64_be(7)),
        137,
        None,
    );

    let ack = client
        .place_order(&PlaceOrderRequest {
            market: "btc-updown-5m-1".to_string(),
            token_id: Some("tok-yes".to_string()),
            side: "buy".to_string(),
            price: 0.60,
            size: 10.0,
            intent: "entry".to_string(),
            order_type: "GTC".to_string(),
            client_order_id: Uuid::new_v4().to_string(),
            leg_side: Some("yes".to_string()),
            fee_rate_bps: 0,
            neg_risk: false,
        })
        .await?;

    assert!(ack.exchange_order_id.is_some());
    let open = client.list_open_orders(Some("btc-updown-5m-1")).await?;
    let fills = client.list_fills(None).await?;

    assert!(open.len() <= 1);
    assert!(!fills.is_empty());
    mock.shutdown();
    Ok(())
}

#[test]
fn parse_token_ids_supports_up_down_outcomes() {
    let market = json!({
        "outcomes": ["Up", "Down"],
        "clobTokenIds": ["tok-up", "tok-down"]
    });

    let (yes_token_id, no_token_id) = parse_yes_no_token_ids(&market);
    assert_eq!(yes_token_id.as_deref(), Some("tok-up"));
    assert_eq!(no_token_id.as_deref(), Some("tok-down"));
}

#[test]
fn parse_token_ids_supports_up_down_tokens_array() {
    let market = json!({
        "tokens": [
            { "outcome": "UP", "token_id": "tok-up" },
            { "outcome": "down", "token_id": "tok-down" }
        ]
    });

    let (yes_token_id, no_token_id) = parse_yes_no_token_ids(&market);
    assert_eq!(yes_token_id.as_deref(), Some("tok-up"));
    assert_eq!(no_token_id.as_deref(), Some("tok-down"));
}

#[test]
fn parse_token_ids_prefers_direct_yes_no_fields() {
    let market = json!({
        "yesTokenId": "tok-yes-direct",
        "noTokenId": "tok-no-direct",
        "outcomes": ["Up", "Down"],
        "clobTokenIds": ["tok-up", "tok-down"]
    });

    let (yes_token_id, no_token_id) = parse_yes_no_token_ids(&market);
    assert_eq!(yes_token_id.as_deref(), Some("tok-yes-direct"));
    assert_eq!(no_token_id.as_deref(), Some("tok-no-direct"));
}

#[test]
fn parse_gamma_market_any_supports_neg_risk_arbitrary_slug() {
    let market = json!({
        "slug": "lal-bil-bar-2026-03-08-bar",
        "active": true,
        "closed": true,
        "negRisk": true,
        "orderPriceMinTickSize": 0.001,
        "orderMinSize": 5,
        "makerBaseFee": 0,
        "clobTokenIds": [
            "tok-yes",
            "tok-no"
        ],
        "outcomes": ["Yes", "No"]
    });

    let parsed = parse_gamma_market_any(&market).expect("market");
    assert_eq!(parsed.slug, "lal-bil-bar-2026-03-08-bar");
    assert!(parsed.neg_risk);
    assert_eq!(parsed.order_price_min_tick_size, Some(0.001));
    assert_eq!(parsed.order_min_size, Some(5.0));
    assert_eq!(parsed.yes_token_id.as_deref(), Some("tok-yes"));
}

#[test]
fn parse_gamma_market_keeps_updown_filter() {
    let market = json!({
        "slug": "lal-bil-bar-2026-03-08-bar",
        "active": true,
        "closed": true
    });

    assert!(parse_gamma_market(&market).is_none());
}

#[test]
fn effective_exchange_address_uses_neg_risk_override() {
    let wallet = "0000000000000000000000000000000000000000000000000000000000000001"
        .parse::<LocalWallet>()
        .unwrap();
    let standard_addr = Address::from_low_u64_be(1);
    let neg_risk_addr = Address::from_low_u64_be(2);
    let client = ClobHttpClient::from_credentials(
        "https://clob.polymarket.com".to_string(),
        None,
        0,
        0,
        ApiCredentials {
            address: "0x0000000000000000000000000000000000000000".to_string(),
            key: "k".to_string(),
            secret: "YWFhYQ==".to_string(),
            passphrase: "p".to_string(),
        },
        wallet,
        standard_addr,
        Some(neg_risk_addr),
        137,
        None,
    );

    assert_eq!(client.effective_exchange_address(false), standard_addr);
    assert_eq!(client.effective_exchange_address(true), neg_risk_addr);
}

#[test]
fn book_parser_uses_last_bid_and_ask_entries_for_best_prices() {
    let raw = json!({
        "bids": [
            { "price": "0.01" },
            { "price": "0.50" },
            { "price": "0.93" }
        ],
        "asks": [
            { "price": "0.99" },
            { "price": "0.96" },
            { "price": "0.94" }
        ]
    });

    let (best_bid, best_ask) = extract_best_bid_ask_from_book(&raw);
    assert_eq!(best_bid, Some(0.93));
    assert_eq!(best_ask, Some(0.94));
}

#[test]
fn book_parser_supports_single_bid_and_ask_entries() {
    let raw = json!({
        "bids": [{ "price": "0.50" }],
        "asks": [{ "price": "0.60" }]
    });

    let (best_bid, best_ask) = extract_best_bid_ask_from_book(&raw);
    assert_eq!(best_bid, Some(0.50));
    assert_eq!(best_ask, Some(0.60));
}

#[test]
fn book_parser_returns_none_for_empty_bid_side() {
    let raw = json!({
        "bids": [],
        "asks": [{ "price": "0.94" }]
    });

    let (best_bid, best_ask) = extract_best_bid_ask_from_book(&raw);
    assert_eq!(best_bid, None);
    assert_eq!(best_ask, Some(0.94));
}

#[test]
fn book_parser_returns_none_for_empty_ask_side() {
    let raw = json!({
        "bids": [{ "price": "0.50" }],
        "asks": []
    });

    let (best_bid, best_ask) = extract_best_bid_ask_from_book(&raw);
    assert_eq!(best_bid, Some(0.50));
    assert_eq!(best_ask, None);
}

#[test]
fn book_parser_returns_none_when_both_sides_are_empty() {
    let raw = json!({
        "bids": [],
        "asks": []
    });

    let (best_bid, best_ask) = extract_best_bid_ask_from_book(&raw);
    assert_eq!(best_bid, None);
    assert_eq!(best_ask, None);
}

#[test]
fn fee_rate_parser_supports_base_fee_zero() {
    let raw = json!({
        "base_fee": 0
    });

    assert_eq!(parse_fee_rate_bps_response(&raw), Some(0));
}

#[test]
fn fee_rate_parser_preserves_existing_fee_rate_fields() {
    let snake = json!({
        "fee_rate_bps": 12
    });
    let camel = json!({
        "feeRateBps": "34"
    });

    assert_eq!(parse_fee_rate_bps_response(&snake), Some(12));
    assert_eq!(parse_fee_rate_bps_response(&camel), Some(34));
}
