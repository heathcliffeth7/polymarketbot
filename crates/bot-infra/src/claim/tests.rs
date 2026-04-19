use super::*;
use serde_json::json;

#[test]
fn meets_min_claim_value_uses_current_value_threshold() {
    let winner = DataApiPosition {
        proxy_wallet: None,
        condition_id: None,
        market_slug: None,
        slug: None,
        current_value: Some(json!(1.0)),
        cur_price: Some(json!(1)),
        redeemable: Some(true),
        size: Some(json!(5.32)),
        balance: None,
    };
    assert!(meets_min_claim_value(&winner, 1.0));

    let loser = DataApiPosition {
        current_value: Some(json!(0.99)),
        ..winner.clone()
    };
    assert!(!meets_min_claim_value(&loser, 1.0));
}

#[test]
fn meets_min_claim_value_falls_back_to_cur_price_times_size() {
    let winner = DataApiPosition {
        proxy_wallet: None,
        condition_id: None,
        market_slug: None,
        slug: None,
        current_value: None,
        cur_price: Some(json!(0.25)),
        redeemable: Some(true),
        size: Some(json!(4.0)),
        balance: None,
    };
    assert!(meets_min_claim_value(&winner, 1.0));

    let loser = DataApiPosition {
        cur_price: Some(json!(0.24)),
        ..winner.clone()
    };
    assert!(!meets_min_claim_value(&loser, 1.0));
}

#[test]
fn meets_min_claim_value_is_fail_closed_without_enough_notional_data() {
    let missing_size = DataApiPosition {
        proxy_wallet: None,
        condition_id: None,
        market_slug: None,
        slug: None,
        current_value: None,
        cur_price: Some(json!(1.0)),
        redeemable: Some(true),
        size: None,
        balance: None,
    };
    assert!(!meets_min_claim_value(&missing_size, 1.0));

    let missing_value_and_price = DataApiPosition {
        cur_price: None,
        ..missing_size
    };
    assert!(!meets_min_claim_value(&missing_value_and_price, 1.0));
}

#[test]
fn meets_min_claim_value_requires_positive_value_when_threshold_is_non_positive() {
    let zero_value = DataApiPosition {
        proxy_wallet: None,
        condition_id: None,
        market_slug: None,
        slug: None,
        current_value: Some(json!(0.0)),
        cur_price: Some(json!(1.0)),
        redeemable: Some(true),
        size: Some(json!(1.0)),
        balance: None,
    };
    assert!(!meets_min_claim_value(&zero_value, 0.0));

    let positive_value = DataApiPosition {
        current_value: Some(json!(0.01)),
        ..zero_value
    };
    assert!(meets_min_claim_value(&positive_value, -5.0));
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

#[test]
fn max_inflight_claim_jobs_limits_builder_relayer_to_one() {
    assert_eq!(
        max_inflight_claim_jobs(ClaimExecutionMode::BuilderRelayer, 10),
        1
    );
    assert_eq!(max_inflight_claim_jobs(ClaimExecutionMode::Direct, 10), 10);
}

#[test]
fn submit_failure_indicates_rate_limit_matches_relayer_signals() {
    assert!(submit_failure_indicates_rate_limit(
        &ClaimSubmitFailure::retryable(
            r#"relayer_rate_limited: {"status":429,"statusText":"Too Many Requests"}"#
        )
    ));
    assert!(!submit_failure_indicates_rate_limit(
        &ClaimSubmitFailure::retryable("claim relayer adapter returned HTTP 500")
    ));
}

#[test]
fn relayer_rate_limit_cooldown_uses_minimum_and_backoff() {
    let now = Utc::now();
    let min_cooldown = relayer_rate_limit_cooldown_until(now, 10_000);
    let long_cooldown = relayer_rate_limit_cooldown_until(now, 45_000);

    assert_eq!(elapsed_seconds_since(now, min_cooldown), 30);
    assert_eq!(elapsed_seconds_since(now, long_cooldown), 45);
}

#[test]
fn claim_relayer_adapter_error_details_compacts_html_body() {
    let (retryable, code, message) = claim_relayer_adapter_error_details(
        StatusCode::BAD_GATEWAY,
        None,
        "<!DOCTYPE html><html><head><title>Oops</title></head><body>Broken</body></html>",
    );

    assert!(retryable);
    assert_eq!(code, "claim_relayer_adapter_invalid_html");
    assert_eq!(message, "HTTP 502 from internal adapter");
}

#[test]
fn claim_relayer_adapter_error_details_prefers_parsed_json_payload() {
    let parsed = ClaimRelayerAdapterErrorBody {
        code: "unauthorized".to_string(),
        retryable: Some(false),
        message: "Unauthorized".to_string(),
    };
    let (retryable, code, message) = claim_relayer_adapter_error_details(
        StatusCode::UNAUTHORIZED,
        Some(&parsed),
        r#"{"code":"unauthorized","message":"Unauthorized"}"#,
    );

    assert!(!retryable);
    assert_eq!(code, "unauthorized");
    assert_eq!(message, "Unauthorized");
}
