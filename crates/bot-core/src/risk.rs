use crate::{KillSwitchMode, RiskDecision};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    pub max_daily_loss_usdc: f64,
    pub max_consecutive_losses: u32,
    pub max_notional_per_market_usdc: f64,
    pub max_open_orders: u32,
    pub max_stale_data_ms: u64,
    pub kill_switch_mode: KillSwitchMode,
}

#[derive(Debug, Clone)]
pub struct RiskInput {
    pub proposed_notional_usdc: f64,
    pub open_orders: u32,
    pub stale_data_ms: u64,
    pub daily_realized_pnl_usdc: f64,
    pub consecutive_losses: u32,
    pub manual_kill_switch_active: bool,
}

#[derive(Debug, Clone)]
pub struct RiskEvaluation {
    pub decision: RiskDecision,
    pub reason: &'static str,
}

pub trait RiskPolicy: Send + Sync {
    fn evaluate(&self, limits: &RiskLimits, input: &RiskInput) -> RiskEvaluation;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultRiskPolicy;

impl RiskPolicy for DefaultRiskPolicy {
    fn evaluate(&self, limits: &RiskLimits, input: &RiskInput) -> RiskEvaluation {
        evaluate_risk(limits, input)
    }
}

pub fn evaluate_risk(limits: &RiskLimits, input: &RiskInput) -> RiskEvaluation {
    let manual_kill_active = input.manual_kill_switch_active;
    if manual_kill_active
        && matches!(
            limits.kill_switch_mode,
            KillSwitchMode::ManualOnly | KillSwitchMode::ManualOrPolicy
        )
    {
        return RiskEvaluation {
            decision: RiskDecision::Halt,
            reason: "manual_kill_switch_active",
        };
    }

    if input.stale_data_ms > limits.max_stale_data_ms {
        return RiskEvaluation {
            decision: RiskDecision::Block,
            reason: "stale_data",
        };
    }

    if input.daily_realized_pnl_usdc <= -limits.max_daily_loss_usdc {
        if matches!(limits.kill_switch_mode, KillSwitchMode::ManualOrPolicy) {
            return RiskEvaluation {
                decision: RiskDecision::Halt,
                reason: "daily_loss_limit_breached",
            };
        }
        return RiskEvaluation {
            decision: RiskDecision::Block,
            reason: "daily_loss_limit_blocked",
        };
    }

    if input.consecutive_losses >= limits.max_consecutive_losses {
        if matches!(limits.kill_switch_mode, KillSwitchMode::ManualOrPolicy) {
            return RiskEvaluation {
                decision: RiskDecision::Halt,
                reason: "consecutive_loss_limit_breached",
            };
        }
        return RiskEvaluation {
            decision: RiskDecision::Block,
            reason: "consecutive_loss_limit_blocked",
        };
    }

    if input.proposed_notional_usdc > limits.max_notional_per_market_usdc {
        return RiskEvaluation {
            decision: RiskDecision::Block,
            reason: "market_notional_limit_breached",
        };
    }

    if input.open_orders >= limits.max_open_orders {
        return RiskEvaluation {
            decision: RiskDecision::Block,
            reason: "open_order_limit_breached",
        };
    }

    RiskEvaluation {
        decision: RiskDecision::Allow,
        reason: "allow",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> RiskLimits {
        RiskLimits {
            max_daily_loss_usdc: 30.0,
            max_consecutive_losses: 3,
            max_notional_per_market_usdc: 10.0,
            max_open_orders: 4,
            max_stale_data_ms: 3000,
            kill_switch_mode: KillSwitchMode::ManualOrPolicy,
        }
    }

    #[test]
    fn blocks_on_stale_data() {
        let res = evaluate_risk(
            &limits(),
            &RiskInput {
                proposed_notional_usdc: 5.0,
                open_orders: 1,
                stale_data_ms: 5000,
                daily_realized_pnl_usdc: 0.0,
                consecutive_losses: 0,
                manual_kill_switch_active: false,
            },
        );
        assert!(matches!(res.decision, RiskDecision::Block));
    }

    #[test]
    fn halts_on_daily_loss() {
        let res = evaluate_risk(
            &limits(),
            &RiskInput {
                proposed_notional_usdc: 5.0,
                open_orders: 1,
                stale_data_ms: 100,
                daily_realized_pnl_usdc: -35.0,
                consecutive_losses: 0,
                manual_kill_switch_active: false,
            },
        );
        assert!(matches!(res.decision, RiskDecision::Halt));
    }

    #[test]
    fn halts_on_manual_kill_switch() {
        let res = evaluate_risk(
            &limits(),
            &RiskInput {
                proposed_notional_usdc: 5.0,
                open_orders: 1,
                stale_data_ms: 100,
                daily_realized_pnl_usdc: 0.0,
                consecutive_losses: 0,
                manual_kill_switch_active: true,
            },
        );
        assert!(matches!(res.decision, RiskDecision::Halt));
    }
}
