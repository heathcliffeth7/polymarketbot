use crate::trade_flow::guards::chainlink_price::{
    get_chainlink_price_samples, ChainlinkPriceSample,
};
use anyhow::{Error, Result};
use chrono::Utc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(test)]
const CHAINLINK_SAMPLE_WARMUP_RETRY_TIMEOUT_MS: u64 = 0;
#[cfg(not(test))]
const CHAINLINK_SAMPLE_WARMUP_RETRY_TIMEOUT_MS: u64 = 2_500;
const CHAINLINK_SAMPLE_WARMUP_RETRY_STEP_MS: u64 = 250;

pub(crate) struct ChainlinkSampleFetch {
    pub(crate) samples: Vec<ChainlinkPriceSample>,
    pub(crate) start_ms: i64,
    pub(crate) end_ms: i64,
}

pub(crate) fn get_chainlink_price_samples_with_warmup(
    asset: &str,
    sample_window_secs: i64,
    initial_end_ms: i64,
) -> Result<ChainlinkSampleFetch> {
    let window_ms = sample_window_secs.max(1) * 1_000;
    let mut end_ms = initial_end_ms;
    let mut start_ms = end_ms - window_ms;
    match get_chainlink_price_samples(asset, start_ms, end_ms) {
        Ok(samples) => {
            return Ok(ChainlinkSampleFetch {
                samples,
                start_ms,
                end_ms,
            });
        }
        Err(error) if should_retry_chainlink_sample_cache_miss(&error) => {
            if CHAINLINK_SAMPLE_WARMUP_RETRY_TIMEOUT_MS == 0 {
                return Err(error);
            }
            tracing::warn!(
                asset,
                retry_timeout_ms = CHAINLINK_SAMPLE_WARMUP_RETRY_TIMEOUT_MS,
                "CHAINLINK_SAMPLE_WARMUP_RETRY_REQUESTED"
            );
        }
        Err(error) => return Err(error),
    }

    let deadline = Instant::now() + Duration::from_millis(CHAINLINK_SAMPLE_WARMUP_RETRY_TIMEOUT_MS);
    let mut last_error = None;
    while Instant::now() < deadline {
        let now = Instant::now();
        let sleep_ms = CHAINLINK_SAMPLE_WARMUP_RETRY_STEP_MS
            .min(deadline.saturating_duration_since(now).as_millis() as u64);
        if sleep_ms == 0 {
            break;
        }
        thread::sleep(Duration::from_millis(sleep_ms));
        end_ms = Utc::now().timestamp_millis();
        start_ms = end_ms - window_ms;
        match get_chainlink_price_samples(asset, start_ms, end_ms) {
            Ok(samples) => {
                tracing::info!(
                    asset,
                    sample_count = samples.len(),
                    sample_window_start_ms = start_ms,
                    sample_window_end_ms = end_ms,
                    "CHAINLINK_SAMPLE_WARMUP_RETRY_READY"
                );
                return Ok(ChainlinkSampleFetch {
                    samples,
                    start_ms,
                    end_ms,
                });
            }
            Err(error) if should_retry_chainlink_sample_cache_miss(&error) => {
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }

    let error = last_error.unwrap_or_else(|| {
        anyhow::anyhow!(
            "chainlink sample warmup retry exhausted for {asset} after {}ms",
            CHAINLINK_SAMPLE_WARMUP_RETRY_TIMEOUT_MS
        )
    });
    tracing::warn!(
        asset,
        error = %error,
        retry_timeout_ms = CHAINLINK_SAMPLE_WARMUP_RETRY_TIMEOUT_MS,
        "CHAINLINK_SAMPLE_WARMUP_RETRY_EXHAUSTED"
    );
    Err(error)
}

fn should_retry_chainlink_sample_cache_miss(error: &Error) -> bool {
    let message = error.to_string();
    message.contains("no cached price for ") || message.contains("no cached chainlink samples for ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_miss_errors_are_retryable() {
        assert!(should_retry_chainlink_sample_cache_miss(&anyhow::anyhow!(
            "no cached price for eth/usd"
        )));
        assert!(should_retry_chainlink_sample_cache_miss(&anyhow::anyhow!(
            "no cached chainlink samples for sol/usd"
        )));
        assert!(!should_retry_chainlink_sample_cache_miss(&anyhow::anyhow!(
            "unsupported asset: doge"
        )));
    }
}
