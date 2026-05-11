use super::*;
use ethers::types::Bytes;

const BUILDER_RELAYER_MAX_INFLIGHT_JOBS: i64 = 1;
const RELAYER_RATE_LIMIT_MIN_COOLDOWN_SEC: u64 = 30;

pub(super) fn parse_json_f64(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(v)) => v.as_f64(),
        Some(Value::String(v)) => v.parse::<f64>().ok(),
        _ => None,
    }
}

pub(super) fn normalize_position_owner_address(
    proxy_wallet: Option<&str>,
    fallback_address: &str,
) -> Result<String> {
    if let Some(proxy_wallet) = proxy_wallet {
        let trimmed = proxy_wallet.trim();
        if !trimmed.is_empty() {
            return normalize_address(trimmed);
        }
    }
    normalize_address(fallback_address)
}

pub(super) fn meets_min_claim_value(position: &DataApiPosition, min_usdc: f64) -> bool {
    let value = redeemable_position_value(position);
    if value <= 0.0 {
        return false;
    }
    let threshold = min_usdc.max(0.0);
    if threshold <= 0.0 {
        return true;
    }
    value >= threshold
}

fn redeemable_position_value(position: &DataApiPosition) -> f64 {
    if let Some(current_value) = parse_json_f64(position.current_value.as_ref()) {
        if current_value > 0.0 {
            return current_value;
        }
    }
    let size = parse_json_f64(position.size.as_ref())
        .or_else(|| parse_json_f64(position.balance.as_ref()))
        .unwrap_or(0.0);
    if size <= 0.0 {
        return 0.0;
    }
    if let Some(cur_price) = parse_json_f64(position.cur_price.as_ref()) {
        if cur_price > 0.0 {
            return cur_price * size;
        }
    }
    size
}

pub(super) fn build_safe_prevalidated_signature(owner: Address) -> Bytes {
    let mut signature = Vec::with_capacity(65);
    signature.extend_from_slice(&[0u8; 12]);
    signature.extend_from_slice(owner.as_bytes());
    signature.extend_from_slice(&[0u8; 32]);
    signature.push(1u8);
    signature.into()
}

pub(super) fn compact_error(err: anyhow::Error) -> String {
    let mut parts: Vec<String> = Vec::new();
    for cause in err.chain() {
        let message = cause.to_string();
        if !parts.iter().any(|part| part.contains(&message)) {
            parts.push(message);
        }
    }
    compact_error_text(&parts.join(" -> ").replace('\n', " "))
}

pub(super) fn compact_submit_failure(err: &ClaimSubmitFailure) -> String {
    compact_error_text(&err.message)
}

pub(super) fn apply_gas_price_floor_and_buffer(gas_price: U256) -> U256 {
    let floor = U256::from(MIN_GAS_PRICE_GWEI) * U256::from(1_000_000_000u64);
    gas_price.max(floor) * U256::from(120u64) / U256::from(100u64)
}

pub(super) fn gas_price_gwei(gas_price: U256) -> u64 {
    (gas_price / U256::from(1_000_000_000u64)).as_u64()
}

pub(super) fn elapsed_seconds_since(start: DateTime<Utc>, end: DateTime<Utc>) -> i64 {
    end.signed_duration_since(start).num_seconds().max(0)
}

pub(super) fn has_receipt_timed_out(submitted_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    elapsed_seconds_since(submitted_at, now) >= RECEIPT_TIMEOUT_SEC as i64
}

pub(super) fn max_inflight_claim_jobs(
    execution_mode: ClaimExecutionMode,
    process_batch_size: i64,
) -> i64 {
    if matches!(
        execution_mode,
        ClaimExecutionMode::BuilderRelayer | ClaimExecutionMode::RelayerApiKey
    ) {
        BUILDER_RELAYER_MAX_INFLIGHT_JOBS
    } else {
        process_batch_size.max(1)
    }
}

pub(super) fn submit_failure_indicates_rate_limit(err: &ClaimSubmitFailure) -> bool {
    if !err.retryable {
        return false;
    }
    let lower = err.message.to_ascii_lowercase();
    lower.contains("relayer_rate_limited")
        || lower.contains("http 429")
        || lower.contains("\"status\":429")
        || lower.contains("too many requests")
        || lower.contains("quota exceeded")
}

pub(super) fn relayer_rate_limit_cooldown_until(
    now: DateTime<Utc>,
    retry_backoff_ms: u64,
) -> DateTime<Utc> {
    let retry_backoff_sec = retry_backoff_ms.saturating_add(999) / 1000;
    let cooldown_sec = retry_backoff_sec.max(RELAYER_RATE_LIMIT_MIN_COOLDOWN_SEC);
    now + ChronoDuration::seconds(cooldown_sec as i64)
}

fn compact_error_text(raw: &str) -> String {
    let mut out = raw.trim().replace('\n', " ");
    if out.len() > AUTO_CLAIM_MAX_ERROR_LEN {
        out.truncate(AUTO_CLAIM_MAX_ERROR_LEN);
    }
    out
}
