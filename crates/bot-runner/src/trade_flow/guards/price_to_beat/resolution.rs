use super::{PriceToBeatDiffUnit, PriceToBeatGuardEvaluation, PriceToBeatMode};
use anyhow::Result;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ActionPlaceOrderPriceToBeatGuardResolution {
    pub(crate) configured_mode: PriceToBeatMode,
    pub(crate) effective_mode: PriceToBeatMode,
    pub(crate) threshold_value: Option<f64>,
    pub(crate) threshold_unit: PriceToBeatDiffUnit,
    pub(crate) base_threshold_value: Option<f64>,
    pub(crate) base_threshold_unit: Option<PriceToBeatDiffUnit>,
    pub(crate) base_threshold_usd: Option<f64>,
    pub(crate) effective_threshold_usd: Option<f64>,
    pub(crate) current_effective_ptb_usd: Option<f64>,
    pub(crate) stop_loss_bump_count: i64,
    pub(crate) stop_loss_bump_applied_count: i64,
    pub(crate) stop_loss_bump_amount: Option<f64>,
    pub(crate) stop_loss_bump_max_value: Option<f64>,
    pub(crate) stop_loss_bump_unit: Option<PriceToBeatDiffUnit>,
    pub(crate) stop_loss_bump_raw_usd: f64,
    pub(crate) stop_loss_bump_usd: f64,
    pub(crate) stop_loss_bump_capped: bool,
    pub(crate) stop_loss_bump_max_reached: bool,
    pub(crate) stop_loss_bump_current_market_excluded: bool,
    pub(crate) stop_loss_bump_increment_usd: f64,
    pub(crate) reentry_generation: i64,
    pub(crate) reentry_override_active: bool,
    pub(crate) reentry_override_value: Option<f64>,
    pub(crate) reentry_override_unit: Option<PriceToBeatDiffUnit>,
}

impl ActionPlaceOrderPriceToBeatGuardResolution {
    pub(crate) fn apply_metadata(&self, evaluation: &mut PriceToBeatGuardEvaluation) {
        evaluation.configured_threshold_mode = Some(self.configured_mode.as_str().to_string());
        evaluation.base_threshold_value = self.base_threshold_value;
        evaluation.base_threshold_unit = self
            .base_threshold_unit
            .map(|unit| unit.as_str().to_string());
        evaluation.base_threshold_usd = self.base_threshold_usd;
        evaluation.current_effective_ptb_usd = self.current_effective_ptb_usd;
        evaluation.stop_loss_bump_count = self.stop_loss_bump_count;
        evaluation.stop_loss_bump_applied_count = self.stop_loss_bump_applied_count;
        evaluation.stop_loss_bump_amount = self.stop_loss_bump_amount;
        evaluation.stop_loss_bump_max_value = self.stop_loss_bump_max_value;
        evaluation.stop_loss_bump_unit = self
            .stop_loss_bump_unit
            .map(|unit| unit.as_str().to_string());
        evaluation.stop_loss_bump_raw_usd = self.stop_loss_bump_raw_usd;
        evaluation.stop_loss_bump_usd = self.stop_loss_bump_usd;
        evaluation.stop_loss_bump_capped = self.stop_loss_bump_capped;
        evaluation.stop_loss_bump_max_reached = self.stop_loss_bump_max_reached;
        evaluation.stop_loss_bump_current_market_excluded =
            self.stop_loss_bump_current_market_excluded;
        evaluation.stop_loss_bump_increment_usd = self.stop_loss_bump_increment_usd;
        evaluation.reentry_generation = self.reentry_generation;
        evaluation.reentry_override_active = self.reentry_override_active;
        evaluation.reentry_override_value = self.reentry_override_value;
        evaluation.reentry_override_unit = self
            .reentry_override_unit
            .map(|unit| unit.as_str().to_string());
    }
}

fn action_place_order_reentry_generation(context: &Value, node_key: &str) -> i64 {
    context
        .get("nodeState")
        .and_then(|value| value.get(node_key))
        .and_then(|value| value.get("reentry_generation"))
        .and_then(|value| value.as_i64())
        .unwrap_or(0)
        .max(0)
}

fn threshold_value_from_usd(threshold_usd: f64, unit: PriceToBeatDiffUnit) -> f64 {
    match unit {
        PriceToBeatDiffUnit::Usd => threshold_usd,
        PriceToBeatDiffUnit::Cent => threshold_usd * 100.0,
    }
}

pub(crate) fn resolve_action_place_order_price_to_beat_guard_resolution(
    node: &crate::TradeFlowNode,
    context: &Value,
    market_slug: &str,
    outcome_label: &str,
) -> Result<ActionPlaceOrderPriceToBeatGuardResolution> {
    let configured_mode =
        PriceToBeatMode::parse(crate::node_config_string(node, "priceToBeatMode").as_deref())
            .unwrap_or(PriceToBeatMode::Manual);
    let reentry_generation = action_place_order_reentry_generation(context, &node.key);
    let reentry_override_value = crate::node_config_f64(node, "reentryPriceToBeatMaxDiff")
        .filter(|value| value.is_finite() && *value > 0.0);
    let reentry_threshold_decay = crate::node_config_f64(node, "reentryThresholdDecay")
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= 1.0)
        .unwrap_or(1.0);
    let reentry_override_unit_raw =
        crate::node_config_string(node, "reentryPriceToBeatMaxDiffUnit")
            .filter(|value| !value.trim().is_empty());
    let reentry_decay_factor = reentry_threshold_decay.powi(reentry_generation as i32);

    let (default_threshold_value, default_threshold_unit) = match configured_mode {
        PriceToBeatMode::Manual => {
            let threshold_value = crate::node_config_f64(node, "priceToBeatMaxDiff").unwrap_or(0.0);
            anyhow::ensure!(
                threshold_value.is_finite() && threshold_value > 0.0,
                "action.place_order priceToBeatMaxDiff must be > 0 when guard is enabled"
            );
            let threshold_unit = PriceToBeatDiffUnit::parse(
                crate::node_config_string(node, "priceToBeatMaxDiffUnit").as_deref(),
            )
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "action.place_order priceToBeatMaxDiffUnit must be one of: usd, cent"
                )
            })?;
            (Some(threshold_value), threshold_unit)
        }
        PriceToBeatMode::AutoLast3AvgExcursion
        | PriceToBeatMode::AutoVolPct
        | PriceToBeatMode::SignalFormula
        | PriceToBeatMode::IvMismatchEdge => (None, PriceToBeatDiffUnit::Usd),
    };

    let (effective_mode, base_threshold_value, threshold_unit, reentry_override_active) =
        if reentry_generation > 0 {
            if let Some(override_value) = reentry_override_value {
                let adjusted_override_value = override_value * reentry_decay_factor;
                let override_unit = if configured_mode == PriceToBeatMode::Manual {
                    reentry_override_unit_raw
                        .as_deref()
                        .map(|raw| {
                            PriceToBeatDiffUnit::parse(Some(raw)).ok_or_else(|| {
                                anyhow::anyhow!(
                                    "action.place_order reentryPriceToBeatMaxDiffUnit must be one of: usd, cent when re-entry PTB override unit is provided"
                                )
                            })
                        })
                        .transpose()?
                        .unwrap_or(default_threshold_unit)
                } else {
                    PriceToBeatDiffUnit::parse(reentry_override_unit_raw.as_deref()).ok_or_else(
                        || {
                            anyhow::anyhow!(
                                "action.place_order reentryPriceToBeatMaxDiffUnit must be one of: usd, cent when re-entry PTB override is used with auto PTB modes"
                            )
                        },
                    )?
                };
                (
                    PriceToBeatMode::Manual,
                    Some(adjusted_override_value),
                    override_unit,
                    true,
                )
            } else {
                (
                    configured_mode,
                    if configured_mode == PriceToBeatMode::Manual {
                        default_threshold_value.map(|value| value * reentry_decay_factor)
                    } else {
                        default_threshold_value
                    },
                    default_threshold_unit,
                    false,
                )
            }
        } else {
            (
                configured_mode,
                default_threshold_value,
                default_threshold_unit,
                false,
            )
        };

    let base_threshold_usd = base_threshold_value
        .map(|value| super::normalize_price_to_beat_threshold_usd(value, threshold_unit));
    let stop_loss_bump_config =
        crate::resolve_action_place_order_ptb_stop_loss_bump_config(node, "buy")?;
    let stop_loss_bump_state = if stop_loss_bump_config.is_some() {
        crate::resolve_action_place_order_ptb_stop_loss_bump_state(
            context,
            node,
            &node.key,
            market_slug,
            outcome_label,
        )
    } else {
        crate::ActionPlaceOrderPtbStopLossBumpState::default()
    };
    let (
        stop_loss_bump_raw_usd,
        stop_loss_bump_usd,
        stop_loss_bump_capped,
        stop_loss_bump_max_reached,
    ) = stop_loss_bump_config
        .as_ref()
        .map(|config| {
            let raw_bump_usd = stop_loss_bump_state.accumulated_bump_usd.max(0.0);
            let (capped_bump_usd, capped, max_reached) =
                crate::resolve_action_place_order_ptb_stop_loss_bump_capped_usd(
                    raw_bump_usd,
                    Some(config),
                );
            (raw_bump_usd, capped_bump_usd, capped, max_reached)
        })
        .unwrap_or((0.0, 0.0, false, false));
    let current_effective_ptb =
        crate::resolve_action_place_order_ptb_current_effective_threshold_resolution(
            context,
            node,
            &node.key,
            market_slug,
            outcome_label,
            base_threshold_usd.map(|base| base + stop_loss_bump_usd),
            stop_loss_bump_usd,
        );
    let effective_threshold_usd = current_effective_ptb
        .as_ref()
        .map(|value| value.threshold_usd)
        .or_else(|| base_threshold_usd.map(|base| base + stop_loss_bump_usd));
    let threshold_value = effective_threshold_usd
        .map(|threshold_usd| threshold_value_from_usd(threshold_usd, threshold_unit));

    Ok(ActionPlaceOrderPriceToBeatGuardResolution {
        configured_mode,
        effective_mode,
        threshold_value,
        threshold_unit,
        base_threshold_value,
        base_threshold_unit: base_threshold_value.map(|_| threshold_unit),
        base_threshold_usd,
        effective_threshold_usd,
        current_effective_ptb_usd: effective_threshold_usd,
        stop_loss_bump_count: stop_loss_bump_state.count,
        stop_loss_bump_applied_count: stop_loss_bump_state.count,
        stop_loss_bump_amount: stop_loss_bump_config
            .as_ref()
            .and_then(|config| config.amount),
        stop_loss_bump_max_value: stop_loss_bump_config
            .as_ref()
            .and_then(|config| config.max_value),
        stop_loss_bump_unit: stop_loss_bump_config.as_ref().map(|config| config.unit),
        stop_loss_bump_raw_usd,
        stop_loss_bump_usd,
        stop_loss_bump_capped,
        stop_loss_bump_max_reached,
        stop_loss_bump_current_market_excluded: false,
        stop_loss_bump_increment_usd: stop_loss_bump_state.last_bump_increment_usd,
        reentry_generation,
        reentry_override_active,
        reentry_override_value: reentry_override_active
            .then_some(reentry_override_value)
            .flatten(),
        reentry_override_unit: reentry_override_active.then_some(threshold_unit),
    })
}
