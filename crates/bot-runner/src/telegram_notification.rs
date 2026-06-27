#[derive(Debug, Clone)]
struct TelegramSendResult {
    sent: bool,
    skipped_by_backoff: bool,
    http_status: Option<u16>,
    error_body: Option<String>,
    error_message: Option<String>,
    retry_after_sec: Option<i64>,
    backoff_until_ms: Option<i64>,
}

static TELEGRAM_SEND_BACKOFF_UNTIL_MS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, i64>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

fn telegram_send_backoff_key(user_id: i64, chat_id: &str) -> String {
    format!("{user_id}:{}", chat_id.trim())
}

fn telegram_retry_after_seconds(status: u16, body: &str) -> Option<i64> {
    if status != 429 {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(seconds) = value
            .get("parameters")
            .and_then(|parameters| parameters.get("retry_after"))
            .and_then(|retry_after| {
                retry_after
                    .as_i64()
                    .or_else(|| retry_after.as_f64().map(|value| value.ceil() as i64))
            })
            .filter(|seconds| *seconds > 0)
        {
            return Some(seconds);
        }
    }
    let lower = body.to_ascii_lowercase();
    let marker = "retry after ";
    let Some(offset) = lower.find(marker) else {
        return None;
    };
    let tail = &lower[offset + marker.len()..];
    let digits = tail
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    digits.parse::<i64>().ok().filter(|seconds| *seconds > 0)
}

fn telegram_retry_after_or_default(status: u16, body: &str) -> Option<i64> {
    if status != 429 {
        return None;
    }
    Some(telegram_retry_after_seconds(status, body).unwrap_or(30))
}

fn telegram_backoff_until_ms_at(user_id: i64, chat_id: &str, now_ms: i64) -> Option<i64> {
    let key = telegram_send_backoff_key(user_id, chat_id);
    let mut guard = TELEGRAM_SEND_BACKOFF_UNTIL_MS.lock().ok()?;
    let until_ms = guard.get(&key).copied()?;
    if until_ms > now_ms {
        return Some(until_ms);
    }
    guard.remove(&key);
    None
}

fn record_telegram_backoff_at(
    user_id: i64,
    chat_id: &str,
    retry_after_sec: i64,
    now_ms: i64,
) -> i64 {
    let until_ms = now_ms.saturating_add((retry_after_sec + 1).saturating_mul(1_000));
    if let Ok(mut guard) = TELEGRAM_SEND_BACKOFF_UNTIL_MS.lock() {
        guard.insert(telegram_send_backoff_key(user_id, chat_id), until_ms);
    }
    until_ms
}

fn clear_telegram_backoff(user_id: i64, chat_id: &str) {
    if let Ok(mut guard) = TELEGRAM_SEND_BACKOFF_UNTIL_MS.lock() {
        guard.remove(&telegram_send_backoff_key(user_id, chat_id));
    }
}

const TELEGRAM_MAX_TEXT_CHARS: usize = 4096;
const TELEGRAM_CONTINUATION_PREFIX: &str = "(devam)\n";
const TELEGRAM_CONTINUATION_SUFFIX: &str = "\n...(devam var)";

fn telegram_text_chunks(text: &str) -> Vec<String> {
    if text.chars().count() <= TELEGRAM_MAX_TEXT_CHARS {
        return vec![text.to_string()];
    }

    let first_body_limit = TELEGRAM_MAX_TEXT_CHARS - TELEGRAM_CONTINUATION_SUFFIX.chars().count();
    let continuation_body_limit = TELEGRAM_MAX_TEXT_CHARS
        - TELEGRAM_CONTINUATION_PREFIX.chars().count()
        - TELEGRAM_CONTINUATION_SUFFIX.chars().count();
    let mut chars = text.chars().peekable();
    let mut chunks = Vec::new();
    let mut first = true;

    while chars.peek().is_some() {
        let mut chunk = String::new();
        if !first {
            chunk.push_str(TELEGRAM_CONTINUATION_PREFIX);
        }

        let body_limit = if first {
            first_body_limit
        } else {
            continuation_body_limit
        };
        for _ in 0..body_limit {
            let Some(ch) = chars.next() else {
                break;
            };
            chunk.push(ch);
        }
        if chars.peek().is_some() {
            chunk.push_str(TELEGRAM_CONTINUATION_SUFFIX);
        }
        chunks.push(chunk);
        first = false;
    }

    chunks
}

async fn send_telegram_message(
    user_id: i64,
    bot_token: &str,
    chat_id: &str,
    text: &str,
    parse_mode: Option<&str>,
    purpose: &str,
) -> TelegramSendResult {
    let now_ms = Utc::now().timestamp_millis();
    if let Some(backoff_until_ms) = telegram_backoff_until_ms_at(user_id, chat_id, now_ms) {
        return TelegramSendResult {
            sent: false,
            skipped_by_backoff: true,
            http_status: None,
            error_body: None,
            error_message: Some("telegram_backoff_active".to_string()),
            retry_after_sec: None,
            backoff_until_ms: Some(backoff_until_ms),
        };
    }

    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    // Telegram sendMessage text limiti 4096 karakter; asimi 400 ile reddediliyor.
    let chunks = telegram_text_chunks(text);
    let parse_mode = parse_mode
        .filter(|_| chunks.len() == 1)
        .filter(|value| !value.trim().is_empty());
    for chunk in chunks {
        let mut body = serde_json::json!({
            "chat_id": chat_id,
            "text": chunk,
        });
        if let Some(parse_mode) = parse_mode {
            if let Some(obj) = body.as_object_mut() {
                obj.insert("parse_mode".to_string(), serde_json::json!(parse_mode));
            }
        }

        let result = TELEGRAM_HTTP_CLIENT
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await;

        match result {
            Ok(resp) if resp.status().is_success() => {}
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();
                let retry_after_sec = telegram_retry_after_or_default(status, &body);
                let backoff_until_ms = retry_after_sec
                    .map(|seconds| record_telegram_backoff_at(user_id, chat_id, seconds, now_ms));
                warn!(
                    user_id,
                    purpose,
                    http_status = status,
                    retry_after_sec,
                    backoff_until_ms,
                    "TELEGRAM_NOTIFICATION_FAILED"
                );
                return TelegramSendResult {
                    sent: false,
                    skipped_by_backoff: false,
                    http_status: Some(status),
                    error_body: Some(body),
                    error_message: None,
                    retry_after_sec,
                    backoff_until_ms,
                };
            }
            Err(err) => {
                warn!(user_id, purpose, error = %err, "TELEGRAM_NOTIFICATION_FAILED");
                return TelegramSendResult {
                    sent: false,
                    skipped_by_backoff: false,
                    http_status: None,
                    error_body: None,
                    error_message: Some(err.to_string()),
                    retry_after_sec: None,
                    backoff_until_ms: None,
                };
            }
        }
    }

    clear_telegram_backoff(user_id, chat_id);
    TelegramSendResult {
        sent: true,
        skipped_by_backoff: false,
        http_status: Some(200),
        error_body: None,
        error_message: None,
        retry_after_sec: None,
        backoff_until_ms: None,
    }
}

#[cfg(test)]
mod telegram_notification_tests {
    use super::*;

    #[test]
    fn telegram_text_chunks_keep_short_and_split_long() {
        let short = "kisa mesaj";
        assert_eq!(telegram_text_chunks(short), vec![short.to_string()]);
        let long: String = "x".repeat(5000);
        let chunks = telegram_text_chunks(&long);
        assert_eq!(chunks.len(), 2);
        assert!(chunks
            .iter()
            .all(|chunk| chunk.chars().count() <= TELEGRAM_MAX_TEXT_CHARS));
        assert!(chunks[0].ends_with(TELEGRAM_CONTINUATION_SUFFIX));
        assert!(chunks[1].starts_with(TELEGRAM_CONTINUATION_PREFIX));
        let reconstructed: String = chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| {
                let without_prefix = if index == 0 {
                    chunk.as_str()
                } else {
                    chunk.trim_start_matches(TELEGRAM_CONTINUATION_PREFIX)
                };
                without_prefix.trim_end_matches(TELEGRAM_CONTINUATION_SUFFIX)
            })
            .collect();
        assert_eq!(reconstructed, long);
    }

    #[test]
    fn telegram_retry_after_parses_json_body() {
        let body = r#"{"ok":false,"error_code":429,"description":"Too Many Requests: retry after 19","parameters":{"retry_after":19}}"#;
        assert_eq!(telegram_retry_after_seconds(429, body), Some(19));
    }

    #[test]
    fn telegram_retry_after_parses_description_fallback() {
        let body = r#"{"description":"Too Many Requests: retry after 12"}"#;
        assert_eq!(telegram_retry_after_seconds(429, body), Some(12));
    }

    #[tokio::test]
    async fn telegram_send_skips_without_http_when_backoff_active() {
        let user_id = 42;
        let chat_id = "-100test";
        record_telegram_backoff_at(user_id, chat_id, 19, Utc::now().timestamp_millis());

        let result =
            send_telegram_message(user_id, "unused-token", chat_id, "hello", None, "test").await;

        assert!(!result.sent);
        assert!(result.skipped_by_backoff);
        assert!(result.backoff_until_ms.is_some());
        clear_telegram_backoff(user_id, chat_id);
    }
}
