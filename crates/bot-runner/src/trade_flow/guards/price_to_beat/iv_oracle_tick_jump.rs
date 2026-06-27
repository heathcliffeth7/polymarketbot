use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
};

use chrono::Utc;
use serde_json::{json, Map, Value};

const DEFAULT_JUMP_RATIO: f64 = 3.0;
const DEFAULT_JUMP_EM_MULT: f64 = 1.0;
const DEFAULT_CEX_CONFIRM_RATIO: f64 = 0.5;
pub(crate) const DEFAULT_COOLDOWN_MS: i64 = 20_000;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PriceToBeatIvOracleTickJumpConfig {
    pub(crate) enabled: bool,
    pub(crate) jump_ratio: f64,
    pub(crate) jump_em_mult: f64,
    pub(crate) cex_confirm_ratio: f64,
    pub(crate) cooldown_ms: i64,
}

impl Default for PriceToBeatIvOracleTickJumpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            jump_ratio: DEFAULT_JUMP_RATIO,
            jump_em_mult: DEFAULT_JUMP_EM_MULT,
            cex_confirm_ratio: DEFAULT_CEX_CONFIRM_RATIO,
            cooldown_ms: DEFAULT_COOLDOWN_MS,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OracleTickJumpInput {
    pub(crate) config: PriceToBeatIvOracleTickJumpConfig,
    pub(crate) x_now: f64,
    pub(crate) x_prev: f64,
    pub(crate) expected_move_eff: f64,
    pub(crate) conservative_cex_gap: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OracleTickJumpEvaluation {
    pub(crate) enabled: bool,
    pub(crate) jump_detected: bool,
    pub(crate) jump_ratio_observed: Option<f64>,
    pub(crate) jump_delta_usd: Option<f64>,
    pub(crate) cex_confirmed: Option<bool>,
    pub(crate) cooldown_active: bool,
    pub(crate) cooldown_remaining_ms: Option<i64>,
    pub(crate) block_reason: Option<&'static str>,
}

impl OracleTickJumpEvaluation {
    pub(crate) fn disabled() -> Self {
        Self {
            enabled: false,
            jump_detected: false,
            jump_ratio_observed: None,
            jump_delta_usd: None,
            cex_confirmed: None,
            cooldown_active: false,
            cooldown_remaining_ms: None,
            block_reason: None,
        }
    }

    pub(crate) fn append_to_json(&self, obj: &mut Map<String, Value>) {
        obj.insert("tick_jump_enabled".to_string(), json!(self.enabled));
        obj.insert("tick_jump_detected".to_string(), json!(self.jump_detected));
        obj.insert(
            "tick_jump_ratio_observed".to_string(),
            json!(self.jump_ratio_observed),
        );
        obj.insert(
            "tick_jump_delta_usd".to_string(),
            json!(self.jump_delta_usd),
        );
        obj.insert(
            "tick_jump_cex_confirmed".to_string(),
            json!(self.cex_confirmed),
        );
        obj.insert(
            "tick_jump_cooldown_active".to_string(),
            json!(self.cooldown_active),
        );
        obj.insert(
            "tick_jump_cooldown_remaining_ms".to_string(),
            json!(self.cooldown_remaining_ms),
        );
        obj.insert(
            "tick_jump_block_reason".to_string(),
            json!(self.block_reason),
        );
    }
}

static TICK_JUMP_COOLDOWNS: LazyLock<Mutex<HashMap<String, i64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Pure core: no I/O, injectable now_ms and prev_cooldown_until for testability.
/// Returns (evaluation, new_cooldown_until); Some(until) → caller should persist.
pub(crate) fn evaluate_oracle_tick_jump(
    input: &OracleTickJumpInput,
    now_ms: i64,
    prev_cooldown_until: Option<i64>,
) -> (OracleTickJumpEvaluation, Option<i64>) {
    if !input.config.enabled {
        return (OracleTickJumpEvaluation::disabled(), None);
    }

    // Active cooldown from a prior jump — check before detecting a new one.
    if let Some(until) = prev_cooldown_until {
        if now_ms < until {
            return (
                OracleTickJumpEvaluation {
                    enabled: true,
                    jump_detected: false,
                    jump_ratio_observed: None,
                    jump_delta_usd: None,
                    cex_confirmed: None,
                    cooldown_active: true,
                    cooldown_remaining_ms: Some(until - now_ms),
                    block_reason: Some("blocked_oracle_tick_jump_cooldown"),
                },
                None,
            );
        }
    }

    const EPS: f64 = 0.01;
    let x_now_abs = input.x_now.abs();
    let x_prev_abs = input.x_prev.abs();
    let ratio = x_now_abs / x_prev_abs.max(EPS);
    let delta = (input.x_now - input.x_prev).abs();
    let jump_by_ratio = ratio >= input.config.jump_ratio;
    let jump_by_em = input.expected_move_eff > 0.0
        && delta >= input.config.jump_em_mult * input.expected_move_eff;
    let jump_detected = jump_by_ratio || jump_by_em;

    if !jump_detected {
        return (
            OracleTickJumpEvaluation {
                enabled: true,
                jump_detected: false,
                jump_ratio_observed: Some(ratio),
                jump_delta_usd: Some(delta),
                cex_confirmed: None,
                cooldown_active: false,
                cooldown_remaining_ms: None,
                block_reason: None,
            },
            None,
        );
    }

    // Jump detected: CEX must confirm at least cex_confirm_ratio × x_now.
    let cex_confirmed = input
        .conservative_cex_gap
        .map(|cex| cex >= input.config.cex_confirm_ratio * input.x_now);
    let confirmed = cex_confirmed.unwrap_or(false);

    if confirmed {
        return (
            OracleTickJumpEvaluation {
                enabled: true,
                jump_detected: true,
                jump_ratio_observed: Some(ratio),
                jump_delta_usd: Some(delta),
                cex_confirmed: Some(true),
                cooldown_active: false,
                cooldown_remaining_ms: None,
                block_reason: None,
            },
            None,
        );
    }

    let new_cooldown_until = now_ms + input.config.cooldown_ms;
    (
        OracleTickJumpEvaluation {
            enabled: true,
            jump_detected: true,
            jump_ratio_observed: Some(ratio),
            jump_delta_usd: Some(delta),
            cex_confirmed: Some(false),
            cooldown_active: true,
            cooldown_remaining_ms: Some(input.config.cooldown_ms),
            block_reason: Some("blocked_oracle_tick_jump_cooldown"),
        },
        Some(new_cooldown_until),
    )
}

/// Stateful entry point: reads and writes per-market cooldown from a process-wide singleton.
pub(crate) fn evaluate_oracle_tick_jump_with_state(
    market_slug: &str,
    input: &OracleTickJumpInput,
) -> OracleTickJumpEvaluation {
    if !input.config.enabled {
        return OracleTickJumpEvaluation::disabled();
    }
    let now_ms = Utc::now().timestamp_millis();
    let prev = {
        let guard = TICK_JUMP_COOLDOWNS
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        guard.get(market_slug).copied()
    };
    let (eval, new_cooldown) = evaluate_oracle_tick_jump(input, now_ms, prev);
    if let Some(until) = new_cooldown {
        let mut guard = TICK_JUMP_COOLDOWNS
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        guard.insert(market_slug.to_string(), until);
        let cutoff = now_ms - 600_000;
        guard.retain(|_, v| *v > cutoff);
    }
    eval
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> PriceToBeatIvOracleTickJumpConfig {
        PriceToBeatIvOracleTickJumpConfig::default()
    }

    #[test]
    fn jump_ratio_triggers_cooldown() {
        let input = OracleTickJumpInput {
            config: cfg(),
            x_now: 90.0,
            x_prev: 10.0, // ratio = 9.0 >= 3.0
            expected_move_eff: 20.0,
            conservative_cex_gap: None,
        };
        let (eval, new_cooldown) = evaluate_oracle_tick_jump(&input, 0, None);
        assert!(eval.jump_detected);
        assert_eq!(eval.cex_confirmed, Some(false));
        assert!(eval.cooldown_active);
        assert_eq!(eval.block_reason, Some("blocked_oracle_tick_jump_cooldown"));
        assert_eq!(new_cooldown, Some(DEFAULT_COOLDOWN_MS));
    }

    #[test]
    fn em_mult_triggers_cooldown() {
        let input = OracleTickJumpInput {
            config: cfg(),
            x_now: 30.0,
            x_prev: 22.0, // ratio = 1.36 < 3.0, delta = 8 >= 1.0 * 8
            expected_move_eff: 8.0,
            conservative_cex_gap: None,
        };
        let (eval, new_cooldown) = evaluate_oracle_tick_jump(&input, 0, None);
        assert!(eval.jump_detected);
        assert!(eval.cooldown_active);
        assert!(new_cooldown.is_some());
    }

    #[test]
    fn cex_confirmed_skips_cooldown() {
        let input = OracleTickJumpInput {
            config: cfg(),
            x_now: 90.0,
            x_prev: 10.0, // ratio = 9.0, jump
            expected_move_eff: 20.0,
            conservative_cex_gap: Some(80.0), // 80 >= 0.5 * 90 = 45 → confirmed
        };
        let (eval, new_cooldown) = evaluate_oracle_tick_jump(&input, 0, None);
        assert!(eval.jump_detected);
        assert_eq!(eval.cex_confirmed, Some(true));
        assert!(!eval.cooldown_active);
        assert!(eval.block_reason.is_none());
        assert!(new_cooldown.is_none());
    }

    #[test]
    fn cooldown_blocks_until_expiry() {
        let input = OracleTickJumpInput {
            config: cfg(),
            x_now: 50.0,
            x_prev: 50.0,
            expected_move_eff: 20.0,
            conservative_cex_gap: None,
        };
        let (eval, _) = evaluate_oracle_tick_jump(&input, 500, Some(1_000));
        assert!(eval.cooldown_active);
        assert_eq!(eval.block_reason, Some("blocked_oracle_tick_jump_cooldown"));
        assert_eq!(eval.cooldown_remaining_ms, Some(500));
    }

    #[test]
    fn expired_cooldown_allows_trade() {
        let input = OracleTickJumpInput {
            config: cfg(),
            x_now: 50.0,
            x_prev: 50.0,
            expected_move_eff: 20.0,
            conservative_cex_gap: None,
        };
        let (eval, _) = evaluate_oracle_tick_jump(&input, 1_000, Some(500));
        assert!(!eval.cooldown_active);
        assert!(eval.block_reason.is_none());
    }

    #[test]
    fn new_jump_restarts_cooldown() {
        // Prior cooldown expired; new jump triggers a fresh one anchored to now_ms
        let input = OracleTickJumpInput {
            config: cfg(),
            x_now: 90.0,
            x_prev: 10.0,
            expected_move_eff: 20.0,
            conservative_cex_gap: None,
        };
        let (_, new_cooldown) = evaluate_oracle_tick_jump(&input, 500, Some(400));
        assert_eq!(new_cooldown, Some(500 + DEFAULT_COOLDOWN_MS));
    }

    #[test]
    fn disabled_is_noop() {
        let input = OracleTickJumpInput {
            config: PriceToBeatIvOracleTickJumpConfig {
                enabled: false,
                ..Default::default()
            },
            x_now: 90.0,
            x_prev: 10.0,
            expected_move_eff: 20.0,
            conservative_cex_gap: None,
        };
        let (eval, new_cooldown) = evaluate_oracle_tick_jump(&input, 0, None);
        assert!(!eval.enabled);
        assert!(!eval.jump_detected);
        assert!(eval.block_reason.is_none());
        assert!(new_cooldown.is_none());
    }
}
