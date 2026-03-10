use super::parse::parse_yes_no_token_ids;
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
