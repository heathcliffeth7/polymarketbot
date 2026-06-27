use serde_json::{json, Map, Value};

const DEFAULT_HIGH_BID_CENT: f64 = 50.0;
const DEFAULT_LOW_BID_CENT: f64 = 20.0;
const DEFAULT_WINDOW_SEC: f64 = 30.0;
const DEFAULT_LOOKBACK_SEC: f64 = 60.0;

pub(crate) const TOKEN_CRASH_COOLDOWN_BLOCK_REASON: &str = "blocked_token_crash_cooldown";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceToBeatIvTokenCrashCooldownConfig {
    pub(crate) enabled: bool,
    pub(crate) high_bid_cent: f64,
    pub(crate) low_bid_cent: f64,
    pub(crate) window_sec: f64,
    pub(crate) lookback_sec: f64,
}

impl Default for PriceToBeatIvTokenCrashCooldownConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            high_bid_cent: DEFAULT_HIGH_BID_CENT,
            low_bid_cent: DEFAULT_LOW_BID_CENT,
            window_sec: DEFAULT_WINDOW_SEC,
            lookback_sec: DEFAULT_LOOKBACK_SEC,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvTokenCrashCooldownEvaluation {
    pub(crate) enabled: bool,
    pub(crate) triggered: bool,
    pub(crate) block_reason: Option<&'static str>,
    pub(crate) max_bid_in_lookback: Option<f64>,
    pub(crate) min_bid_in_window: Option<f64>,
    pub(crate) crash_ratio: Option<f64>,
}

impl Default for PriceToBeatIvTokenCrashCooldownEvaluation {
    fn default() -> Self {
        Self {
            enabled: false,
            triggered: false,
            block_reason: None,
            max_bid_in_lookback: None,
            min_bid_in_window: None,
            crash_ratio: None,
        }
    }
}

impl PriceToBeatIvTokenCrashCooldownEvaluation {
    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert(
            "token_crash_cooldown_enabled".to_string(),
            json!(self.enabled),
        );
        obj.insert(
            "token_crash_cooldown_triggered".to_string(),
            json!(self.triggered),
        );
        obj.insert(
            "token_crash_cooldown_block_reason".to_string(),
            json!(self.block_reason),
        );
        obj.insert(
            "token_crash_cooldown_max_bid_in_lookback".to_string(),
            json!(self.max_bid_in_lookback),
        );
        obj.insert(
            "token_crash_cooldown_min_bid_in_window".to_string(),
            json!(self.min_bid_in_window),
        );
        obj.insert(
            "token_crash_cooldown_crash_ratio".to_string(),
            json!(self.crash_ratio),
        );
    }
}

pub(crate) struct PriceToBeatIvTokenCrashCooldownInput<'a> {
    pub(crate) config: &'a PriceToBeatIvTokenCrashCooldownConfig,
    pub(crate) market_slug: &'a str,
    pub(crate) outcome_label: &'a str,
    pub(crate) now: chrono::DateTime<chrono::Utc>,
}

pub(crate) fn evaluate_price_to_beat_iv_token_crash_cooldown(
    input: PriceToBeatIvTokenCrashCooldownInput<'_>,
    snapshots: &[TokenCrashSnapshot],
) -> PriceToBeatIvTokenCrashCooldownEvaluation {
    if !input.config.enabled {
        return PriceToBeatIvTokenCrashCooldownEvaluation::default();
    }
    let high_threshold = input.config.high_bid_cent / 100.0;
    let low_threshold = input.config.low_bid_cent / 100.0;
    let lookback_start = input.now - chrono::Duration::seconds(input.config.lookback_sec as i64);
    let window_start = input.now - chrono::Duration::seconds(input.config.window_sec as i64);

    let mut max_bid_in_lookback: Option<f64> = None;
    let mut min_bid_in_window: Option<f64> = None;

    for snap in snapshots {
        if snap.market_slug != input.market_slug || snap.outcome_label != input.outcome_label {
            continue;
        }
        if snap.timestamp >= lookback_start && snap.timestamp <= input.now {
            if let Some(bid) = snap.best_bid {
                if bid >= high_threshold {
                    max_bid_in_lookback = Some(max_bid_in_lookback.map_or(bid, |m| m.max(bid)));
                }
            }
        }
        if snap.timestamp >= window_start && snap.timestamp <= input.now {
            if let Some(bid) = snap.best_bid {
                if bid <= low_threshold {
                    min_bid_in_window = Some(min_bid_in_window.map_or(bid, |m| m.min(bid)));
                }
            }
        }
    }

    let crash_detected = max_bid_in_lookback.is_some() && min_bid_in_window.is_some();
    let crash_ratio = match (max_bid_in_lookback, min_bid_in_window) {
        (Some(max_bid), Some(min_bid)) if max_bid > 0.0 => Some(min_bid / max_bid),
        _ => None,
    };

    let triggered = crash_detected;
    let block_reason = if triggered {
        Some(TOKEN_CRASH_COOLDOWN_BLOCK_REASON)
    } else {
        None
    };

    PriceToBeatIvTokenCrashCooldownEvaluation {
        enabled: true,
        triggered,
        block_reason,
        max_bid_in_lookback,
        min_bid_in_window,
        crash_ratio,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TokenCrashSnapshot {
    pub(crate) market_slug: String,
    pub(crate) outcome_label: String,
    pub(crate) timestamp: chrono::DateTime<chrono::Utc>,
    pub(crate) best_bid: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use chrono::Utc;

    fn snap(
        slug: &str,
        outcome: &str,
        ts: chrono::DateTime<chrono::Utc>,
        bid: f64,
    ) -> TokenCrashSnapshot {
        TokenCrashSnapshot {
            market_slug: slug.to_string(),
            outcome_label: outcome.to_string(),
            timestamp: ts,
            best_bid: Some(bid),
        }
    }

    fn config() -> PriceToBeatIvTokenCrashCooldownConfig {
        PriceToBeatIvTokenCrashCooldownConfig {
            enabled: true,
            high_bid_cent: 50.0,
            low_bid_cent: 20.0,
            window_sec: 30.0,
            lookback_sec: 60.0,
        }
    }

    #[test]
    fn token_crash_cooldown_blocks_when_high_then_crash() {
        let now = Utc.timestamp_opt(1782080400, 0).unwrap();
        let snapshots = vec![
            snap("btc-updown-5m-1782080400", "Up", now - chrono::Duration::seconds(40), 0.82),
            snap("btc-updown-5m-1782080400", "Up", now - chrono::Duration::seconds(15), 0.05),
        ];
        let input = PriceToBeatIvTokenCrashCooldownInput {
            config: &config(),
            market_slug: "btc-updown-5m-1782080400",
            outcome_label: "Up",
            now,
        };
        let eval = evaluate_price_to_beat_iv_token_crash_cooldown(input, &snapshots);
        assert!(eval.triggered);
        assert_eq!(eval.block_reason, Some(TOKEN_CRASH_COOLDOWN_BLOCK_REASON));
        assert_eq!(eval.max_bid_in_lookback, Some(0.82));
        assert_eq!(eval.min_bid_in_window, Some(0.05));
    }

    #[test]
    fn token_crash_cooldown_passes_when_no_crash() {
        let now = Utc.timestamp_opt(1782080400, 0).unwrap();
        let snapshots = vec![
            snap("btc-updown-5m-1782080400", "Up", now - chrono::Duration::seconds(40), 0.82),
            snap("btc-updown-5m-1782080400", "Up", now - chrono::Duration::seconds(15), 0.75),
        ];
        let input = PriceToBeatIvTokenCrashCooldownInput {
            config: &config(),
            market_slug: "btc-updown-5m-1782080400",
            outcome_label: "Up",
            now,
        };
        let eval = evaluate_price_to_beat_iv_token_crash_cooldown(input, &snapshots);
        assert!(!eval.triggered);
        assert_eq!(eval.block_reason, None);
    }

    #[test]
    fn token_crash_cooldown_passes_when_no_high_bid() {
        let now = Utc.timestamp_opt(1782080400, 0).unwrap();
        let snapshots = vec![
            snap("btc-updown-5m-1782080400", "Up", now - chrono::Duration::seconds(40), 0.30),
            snap("btc-updown-5m-1782080400", "Up", now - chrono::Duration::seconds(15), 0.05),
        ];
        let input = PriceToBeatIvTokenCrashCooldownInput {
            config: &config(),
            market_slug: "btc-updown-5m-1782080400",
            outcome_label: "Up",
            now,
        };
        let eval = evaluate_price_to_beat_iv_token_crash_cooldown(input, &snapshots);
        assert!(!eval.triggered);
    }

    #[test]
    fn token_crash_cooldown_disabled_by_default() {
        let now = Utc.timestamp_opt(1782080400, 0).unwrap();
        let input = PriceToBeatIvTokenCrashCooldownInput {
            config: &PriceToBeatIvTokenCrashCooldownConfig::default(),
            market_slug: "btc-updown-5m-1782080400",
            outcome_label: "Up",
            now,
        };
        let eval = evaluate_price_to_beat_iv_token_crash_cooldown(input, &[]);
        assert!(!eval.enabled);
        assert!(!eval.triggered);
    }
}
