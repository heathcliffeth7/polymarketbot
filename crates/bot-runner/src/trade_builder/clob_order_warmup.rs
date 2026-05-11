static CLOB_ORDER_WARMUP_TASKS: LazyLock<StdMutex<HashSet<String>>> =
    LazyLock::new(|| StdMutex::new(HashSet::new()));

fn clob_order_warmup_error_requires_cooldown(error_text: &str, consecutive_failures: u32) -> bool {
    let normalized = error_text.to_ascii_lowercase();
    consecutive_failures >= 3
        || normalized.contains("429")
        || normalized.contains("too many requests")
        || normalized.contains("rate limit")
        || normalized.contains("401")
        || normalized.contains("403")
        || normalized.contains("unauthorized")
        || normalized.contains("forbidden")
}

fn spawn_clob_order_warmup_if_enabled(
    run_id: i64,
    cfg: &AppConfig,
    client: SharedOrderExecutor,
    key: impl Into<String>,
) {
    if !cfg.exchange.clob_order_warmup_enabled {
        return;
    }
    let key = key.into();
    if key.trim().is_empty() {
        return;
    }
    if let Ok(mut started) = CLOB_ORDER_WARMUP_TASKS.lock() {
        if !started.insert(key.clone()) {
            return;
        }
    } else {
        return;
    }

    let interval_ms = cfg
        .exchange
        .clob_order_warmup_interval_ms
        .clamp(5_000, 300_000);
    let cooldown_ms = cfg
        .exchange
        .clob_order_warmup_cooldown_ms
        .max(interval_ms)
        .clamp(interval_ms, 900_000);

    tokio::spawn(async move {
        let interval = Duration::from_millis(interval_ms);
        let cooldown = Duration::from_millis(cooldown_ms);
        let mut consecutive_failures = 0u32;

        loop {
            sleep(interval).await;
            match client.warmup_order_connection().await {
                Ok(()) => {
                    if consecutive_failures > 0 {
                        info!(
                            run_id,
                            warmup_key = %key,
                            "CLOB_ORDER_WARMUP_RECOVERED"
                        );
                    }
                    consecutive_failures = 0;
                    debug!(
                        run_id,
                        warmup_key = %key,
                        interval_ms,
                        "CLOB_ORDER_WARMUP_OK"
                    );
                }
                Err(err) => {
                    consecutive_failures = consecutive_failures.saturating_add(1);
                    let error_text = err.to_string();
                    let cooldown_now = clob_order_warmup_error_requires_cooldown(
                        &error_text,
                        consecutive_failures,
                    );
                    warn!(
                        run_id,
                        warmup_key = %key,
                        consecutive_failures,
                        cooldown_now,
                        error = %error_text,
                        "CLOB_ORDER_WARMUP_FAILED"
                    );
                    if cooldown_now {
                        sleep(cooldown).await;
                        consecutive_failures = 0;
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod clob_order_warmup_tests {
    use super::*;

    #[test]
    fn clob_order_warmup_cools_down_on_rate_limit_or_repeated_failures() {
        assert!(clob_order_warmup_error_requires_cooldown(
            "HTTP status 429 Too Many Requests",
            1
        ));
        assert!(clob_order_warmup_error_requires_cooldown(
            "temporary timeout",
            3
        ));
        assert!(!clob_order_warmup_error_requires_cooldown(
            "temporary timeout",
            2
        ));
    }
}
