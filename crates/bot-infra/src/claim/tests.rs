use super::*;
use serde_json::json;

#[test]
fn positive_claim_value_requires_positive_current_value_or_price() {
    let winner = DataApiPosition {
        proxy_wallet: None,
        condition_id: None,
        market_slug: None,
        slug: None,
        current_value: Some(json!(5.32)),
        cur_price: Some(json!(1)),
        redeemable: Some(true),
        size: Some(json!(5.32)),
        balance: None,
    };
    assert!(has_positive_claim_value(&winner));

    let loser = DataApiPosition {
        current_value: Some(json!(0)),
        cur_price: Some(json!(0)),
        ..winner.clone()
    };
    assert!(!has_positive_claim_value(&loser));
}

#[test]
fn safe_prevalidated_signature_embeds_owner_and_marker() {
    let owner = Address::from_str("0x38562e48f0e8ce1c1c7931b482d6e2145937e452").unwrap();
    let signature = build_safe_prevalidated_signature(owner);
    assert_eq!(signature.len(), 65);
    assert_eq!(&signature[12..32], owner.as_bytes());
    assert_eq!(signature[64], 1u8);
}

#[test]
fn compact_error_includes_error_chain() {
    let err = anyhow::anyhow!("outer context").context("inner cause");
    let compact = compact_error(err);
    assert!(compact.contains("outer context"));
    assert!(compact.contains("inner cause"));
}

#[test]
fn receipt_timeout_uses_submitted_at_wall_clock() {
    let submitted_at = Utc::now() - ChronoDuration::seconds(RECEIPT_TIMEOUT_SEC as i64 + 1);
    assert!(has_receipt_timed_out(submitted_at, Utc::now()));
}

#[test]
fn gas_price_floor_and_buffer_applies_minimum() {
    let gas_price = U256::from(3_000_000_000u64);
    let effective = apply_gas_price_floor_and_buffer(gas_price);
    assert_eq!(gas_price_gwei(effective), 36);
}

#[test]
fn gas_price_floor_and_buffer_preserves_higher_node_quote() {
    let gas_price = U256::from(40_000_000_000u64);
    let effective = apply_gas_price_floor_and_buffer(gas_price);
    assert_eq!(gas_price_gwei(effective), 48);
}
