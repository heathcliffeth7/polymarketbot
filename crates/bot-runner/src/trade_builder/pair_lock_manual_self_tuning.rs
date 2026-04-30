const DEFAULT_MANUAL_ADAPTIVE_SELF_TUNE_ENABLED: bool = false;
const DEFAULT_MANUAL_ADAPTIVE_MISS_RELAX_ENABLED: bool = true;
const DEFAULT_MANUAL_ADAPTIVE_MISS_RELAX_AFTER_NO_ORDER_MARKETS: usize = 3;
const DEFAULT_MANUAL_ADAPTIVE_PTB_RELAX_STEP_CENT: f64 = 5.0;
const DEFAULT_MANUAL_ADAPTIVE_PTB_RELAX_MAX_CENT: f64 = 20.0;
const DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_RELAX_STEP_CENT: f64 = 1.0;
const DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_RELAX_MAX_CENT: f64 = 5.0;
const DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_RELAX_HARD_CAP_CENT: f64 = 90.0;
const DEFAULT_MANUAL_ADAPTIVE_MISS_RELAX_SIZE_MULTIPLIER: f64 = 0.8;
const DEFAULT_MANUAL_ADAPTIVE_SL_TIGHTEN_ENABLED: bool = true;
const DEFAULT_MANUAL_ADAPTIVE_PTB_SL_BUMP_STEP_CENT: f64 = 15.0;
const DEFAULT_MANUAL_ADAPTIVE_PTB_SL_BUMP_MAX_CENT: f64 = 45.0;
const DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_SL_PENALTY_STEP_CENT: f64 = 5.0;
const DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_SL_PENALTY_MAX_CENT: f64 = 15.0;
const DEFAULT_MANUAL_ADAPTIVE_SL_DISABLE_REENTRY: bool = true;
const DEFAULT_MANUAL_ADAPTIVE_CONSECUTIVE_SL_LOCKDOWN_AFTER: usize = 3;
const DEFAULT_MANUAL_ADAPTIVE_LOCKDOWN_RELEASE_CLEAN_MARKETS: usize = 3;
const DEFAULT_MANUAL_ADAPTIVE_LOCKDOWN_MAX_MARKETS: usize = 5;
const DEFAULT_MANUAL_ADAPTIVE_CLEAN_MARKET_DECAY_ENABLED: bool = true;
const DEFAULT_MANUAL_ADAPTIVE_PTB_RELAX_DECAY_PER_MARKET_CENT: f64 = 5.0;
const DEFAULT_MANUAL_ADAPTIVE_PTB_SL_BUMP_DECAY_PER_CLEAN_MARKET_CENT: f64 = 5.0;
const DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_RELAX_DECAY_PER_MARKET_CENT: f64 = 1.0;
const DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_SL_PENALTY_DECAY_PER_CLEAN_MARKET_CENT: f64 = 2.0;
const MANUAL_ADAPTIVE_SELF_TUNE_STATE_KEY: &str = "manual_adaptive_self_tune_scope_map";
const MANUAL_ADAPTIVE_EVENT_SELF_RELAX: &str = "manual_adaptive_self_relax";
const MANUAL_ADAPTIVE_EVENT_SL_TIGHTEN: &str = "manual_adaptive_sl_tighten";
const MANUAL_ADAPTIVE_EVENT_STRICT_PLUS: &str = "manual_adaptive_strict_plus";
const MANUAL_ADAPTIVE_EVENT_LOCKDOWN: &str = "manual_adaptive_lockdown";
const MANUAL_ADAPTIVE_EVENT_DECAY: &str = "manual_adaptive_decay";

#[derive(Debug, Clone, Copy)]
struct PairLockManualSelfTuneConfig {
    enabled: bool,
    miss_relax_enabled: bool,
    miss_relax_after_no_order_markets: usize,
    ptb_relax_step_cent: f64,
    ptb_relax_max_cent: f64,
    max_price_relax_step_cent: f64,
    max_price_relax_max_cent: f64,
    max_price_relax_hard_cap_cent: f64,
    miss_relax_size_multiplier: f64,
    sl_tighten_enabled: bool,
    ptb_sl_bump_step_cent: f64,
    ptb_sl_bump_max_cent: f64,
    max_price_sl_penalty_step_cent: f64,
    max_price_sl_penalty_max_cent: f64,
    sl_disable_reentry: bool,
    consecutive_sl_lockdown_after: usize,
    lockdown_release_clean_markets: usize,
    lockdown_max_markets: usize,
    clean_market_decay_enabled: bool,
    ptb_relax_decay_per_market_cent: f64,
    ptb_sl_bump_decay_per_clean_market_cent: f64,
    max_price_relax_decay_per_market_cent: f64,
    max_price_sl_penalty_decay_per_clean_market_cent: f64,
}

impl Default for PairLockManualSelfTuneConfig {
    fn default() -> Self {
        Self {
            enabled: DEFAULT_MANUAL_ADAPTIVE_SELF_TUNE_ENABLED,
            miss_relax_enabled: DEFAULT_MANUAL_ADAPTIVE_MISS_RELAX_ENABLED,
            miss_relax_after_no_order_markets:
                DEFAULT_MANUAL_ADAPTIVE_MISS_RELAX_AFTER_NO_ORDER_MARKETS,
            ptb_relax_step_cent: DEFAULT_MANUAL_ADAPTIVE_PTB_RELAX_STEP_CENT,
            ptb_relax_max_cent: DEFAULT_MANUAL_ADAPTIVE_PTB_RELAX_MAX_CENT,
            max_price_relax_step_cent: DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_RELAX_STEP_CENT,
            max_price_relax_max_cent: DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_RELAX_MAX_CENT,
            max_price_relax_hard_cap_cent:
                DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_RELAX_HARD_CAP_CENT,
            miss_relax_size_multiplier: DEFAULT_MANUAL_ADAPTIVE_MISS_RELAX_SIZE_MULTIPLIER,
            sl_tighten_enabled: DEFAULT_MANUAL_ADAPTIVE_SL_TIGHTEN_ENABLED,
            ptb_sl_bump_step_cent: DEFAULT_MANUAL_ADAPTIVE_PTB_SL_BUMP_STEP_CENT,
            ptb_sl_bump_max_cent: DEFAULT_MANUAL_ADAPTIVE_PTB_SL_BUMP_MAX_CENT,
            max_price_sl_penalty_step_cent:
                DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_SL_PENALTY_STEP_CENT,
            max_price_sl_penalty_max_cent:
                DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_SL_PENALTY_MAX_CENT,
            sl_disable_reentry: DEFAULT_MANUAL_ADAPTIVE_SL_DISABLE_REENTRY,
            consecutive_sl_lockdown_after:
                DEFAULT_MANUAL_ADAPTIVE_CONSECUTIVE_SL_LOCKDOWN_AFTER,
            lockdown_release_clean_markets:
                DEFAULT_MANUAL_ADAPTIVE_LOCKDOWN_RELEASE_CLEAN_MARKETS,
            lockdown_max_markets: DEFAULT_MANUAL_ADAPTIVE_LOCKDOWN_MAX_MARKETS,
            clean_market_decay_enabled: DEFAULT_MANUAL_ADAPTIVE_CLEAN_MARKET_DECAY_ENABLED,
            ptb_relax_decay_per_market_cent:
                DEFAULT_MANUAL_ADAPTIVE_PTB_RELAX_DECAY_PER_MARKET_CENT,
            ptb_sl_bump_decay_per_clean_market_cent:
                DEFAULT_MANUAL_ADAPTIVE_PTB_SL_BUMP_DECAY_PER_CLEAN_MARKET_CENT,
            max_price_relax_decay_per_market_cent:
                DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_RELAX_DECAY_PER_MARKET_CENT,
            max_price_sl_penalty_decay_per_clean_market_cent:
                DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_SL_PENALTY_DECAY_PER_CLEAN_MARKET_CENT,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
struct PairLockManualSelfTuneState {
    miss_streak: i64,
    sl_streak: i64,
    ptb_relax_credit_cent: f64,
    ptb_sl_bump_cent: f64,
    max_price_relax_credit_cent: f64,
    max_price_sl_penalty_cent: f64,
    cooldown_markets_left: i64,
    lockdown_markets_left: i64,
    clean_markets_since_lockdown: i64,
    last_updated_market_id: Option<String>,
}

#[derive(Debug, Clone)]
struct PairLockManualSelfTuneRuntime {
    state: PairLockManualSelfTuneState,
    miss_relax_applies: bool,
    cooldown_active: bool,
    lockdown_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PairLockManualSelfTuneUpdateKind {
    SelfRelax,
    SlTighten,
    StrictPlus,
    Lockdown,
    Decay,
}

impl PairLockManualSelfTuneUpdateKind {
    fn event_type(self) -> &'static str {
        match self {
            Self::SelfRelax => MANUAL_ADAPTIVE_EVENT_SELF_RELAX,
            Self::SlTighten => MANUAL_ADAPTIVE_EVENT_SL_TIGHTEN,
            Self::StrictPlus => MANUAL_ADAPTIVE_EVENT_STRICT_PLUS,
            Self::Lockdown => MANUAL_ADAPTIVE_EVENT_LOCKDOWN,
            Self::Decay => MANUAL_ADAPTIVE_EVENT_DECAY,
        }
    }

    fn reason(self) -> &'static str {
        match self {
            Self::SelfRelax => "safe_miss_relax",
            Self::SlTighten => "sl_tighten",
            Self::StrictPlus => "consecutive_sl_tighten",
            Self::Lockdown => "consecutive_sl_lockdown",
            Self::Decay => "clean_market_decay",
        }
    }
}

#[derive(Debug, Clone)]
struct PairLockManualSelfTuneUpdate {
    kind: PairLockManualSelfTuneUpdateKind,
    scope_side: String,
    market_slug: String,
    outcome_label: String,
    previous: PairLockManualSelfTuneState,
    next: PairLockManualSelfTuneState,
    payload: Value,
}

fn action_place_order_uses_manual_adaptive_self_tune_strategy(node: &TradeFlowNode) -> bool {
    action_place_order_uses_manual_adaptive_risk_strategy(node)
        && node_config_bool(node, "manualAdaptiveSelfTuneEnabled")
            .unwrap_or(DEFAULT_MANUAL_ADAPTIVE_SELF_TUNE_ENABLED)
}

fn manual_self_tune_node_f64(node: &TradeFlowNode, key: &str, fallback: f64) -> f64 {
    node_config_f64(node, key).unwrap_or(fallback)
}

fn manual_self_tune_node_usize(node: &TradeFlowNode, key: &str, fallback: usize) -> usize {
    node_config_i64(node, key)
        .filter(|value| *value >= 0)
        .map(|value| value as usize)
        .unwrap_or(fallback)
}

fn resolve_pair_lock_manual_self_tune_config(
    node: &TradeFlowNode,
) -> Result<PairLockManualSelfTuneConfig> {
    let config = PairLockManualSelfTuneConfig {
        enabled: node_config_bool(node, "manualAdaptiveSelfTuneEnabled")
            .unwrap_or(DEFAULT_MANUAL_ADAPTIVE_SELF_TUNE_ENABLED),
        miss_relax_enabled: node_config_bool(node, "manualAdaptiveMissRelaxEnabled")
            .unwrap_or(DEFAULT_MANUAL_ADAPTIVE_MISS_RELAX_ENABLED),
        miss_relax_after_no_order_markets: manual_self_tune_node_usize(
            node,
            "manualAdaptiveMissRelaxAfterNoOrderMarkets",
            DEFAULT_MANUAL_ADAPTIVE_MISS_RELAX_AFTER_NO_ORDER_MARKETS,
        )
        .max(1),
        ptb_relax_step_cent: manual_self_tune_node_f64(
            node,
            "manualAdaptivePtbRelaxStepCent",
            DEFAULT_MANUAL_ADAPTIVE_PTB_RELAX_STEP_CENT,
        ),
        ptb_relax_max_cent: manual_self_tune_node_f64(
            node,
            "manualAdaptivePtbRelaxMaxCent",
            DEFAULT_MANUAL_ADAPTIVE_PTB_RELAX_MAX_CENT,
        ),
        max_price_relax_step_cent: manual_self_tune_node_f64(
            node,
            "manualAdaptiveMaxPriceRelaxStepCent",
            DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_RELAX_STEP_CENT,
        ),
        max_price_relax_max_cent: manual_self_tune_node_f64(
            node,
            "manualAdaptiveMaxPriceRelaxMaxCent",
            DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_RELAX_MAX_CENT,
        ),
        max_price_relax_hard_cap_cent: manual_self_tune_node_f64(
            node,
            "manualAdaptiveMaxPriceRelaxHardCapCent",
            DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_RELAX_HARD_CAP_CENT,
        ),
        miss_relax_size_multiplier: manual_self_tune_node_f64(
            node,
            "manualAdaptiveMissRelaxSizeMultiplier",
            DEFAULT_MANUAL_ADAPTIVE_MISS_RELAX_SIZE_MULTIPLIER,
        ),
        sl_tighten_enabled: node_config_bool(node, "manualAdaptiveSlTightenEnabled")
            .unwrap_or(DEFAULT_MANUAL_ADAPTIVE_SL_TIGHTEN_ENABLED),
        ptb_sl_bump_step_cent: manual_self_tune_node_f64(
            node,
            "manualAdaptivePtbSlBumpStepCent",
            DEFAULT_MANUAL_ADAPTIVE_PTB_SL_BUMP_STEP_CENT,
        ),
        ptb_sl_bump_max_cent: manual_self_tune_node_f64(
            node,
            "manualAdaptivePtbSlBumpMaxCent",
            DEFAULT_MANUAL_ADAPTIVE_PTB_SL_BUMP_MAX_CENT,
        ),
        max_price_sl_penalty_step_cent: manual_self_tune_node_f64(
            node,
            "manualAdaptiveMaxPriceSlPenaltyStepCent",
            DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_SL_PENALTY_STEP_CENT,
        ),
        max_price_sl_penalty_max_cent: manual_self_tune_node_f64(
            node,
            "manualAdaptiveMaxPriceSlPenaltyMaxCent",
            DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_SL_PENALTY_MAX_CENT,
        ),
        sl_disable_reentry: node_config_bool(node, "manualAdaptiveSlDisableReentry")
            .unwrap_or(DEFAULT_MANUAL_ADAPTIVE_SL_DISABLE_REENTRY),
        consecutive_sl_lockdown_after: manual_self_tune_node_usize(
            node,
            "manualAdaptiveConsecutiveSlLockdownAfter",
            DEFAULT_MANUAL_ADAPTIVE_CONSECUTIVE_SL_LOCKDOWN_AFTER,
        )
        .max(1),
        lockdown_release_clean_markets: manual_self_tune_node_usize(
            node,
            "manualAdaptiveLockdownReleaseCleanMarkets",
            DEFAULT_MANUAL_ADAPTIVE_LOCKDOWN_RELEASE_CLEAN_MARKETS,
        ),
        lockdown_max_markets: manual_self_tune_node_usize(
            node,
            "manualAdaptiveLockdownMaxMarkets",
            DEFAULT_MANUAL_ADAPTIVE_LOCKDOWN_MAX_MARKETS,
        ),
        clean_market_decay_enabled: node_config_bool(node, "manualAdaptiveCleanMarketDecayEnabled")
            .unwrap_or(DEFAULT_MANUAL_ADAPTIVE_CLEAN_MARKET_DECAY_ENABLED),
        ptb_relax_decay_per_market_cent: manual_self_tune_node_f64(
            node,
            "manualAdaptivePtbRelaxDecayPerMarketCent",
            DEFAULT_MANUAL_ADAPTIVE_PTB_RELAX_DECAY_PER_MARKET_CENT,
        ),
        ptb_sl_bump_decay_per_clean_market_cent: manual_self_tune_node_f64(
            node,
            "manualAdaptivePtbSlBumpDecayPerCleanMarketCent",
            DEFAULT_MANUAL_ADAPTIVE_PTB_SL_BUMP_DECAY_PER_CLEAN_MARKET_CENT,
        ),
        max_price_relax_decay_per_market_cent: manual_self_tune_node_f64(
            node,
            "manualAdaptiveMaxPriceRelaxDecayPerMarketCent",
            DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_RELAX_DECAY_PER_MARKET_CENT,
        ),
        max_price_sl_penalty_decay_per_clean_market_cent: manual_self_tune_node_f64(
            node,
            "manualAdaptiveMaxPriceSlPenaltyDecayPerCleanMarketCent",
            DEFAULT_MANUAL_ADAPTIVE_MAX_PRICE_SL_PENALTY_DECAY_PER_CLEAN_MARKET_CENT,
        ),
    };
    validate_pair_lock_manual_self_tune_config(config)?;
    validate_manual_adaptive_trend_delta_by_scope(node)?;
    Ok(config)
}

fn validate_pair_lock_manual_self_tune_config(
    config: PairLockManualSelfTuneConfig,
) -> Result<()> {
    for (name, value) in [
        ("manualAdaptivePtbRelaxStepCent", config.ptb_relax_step_cent),
        ("manualAdaptivePtbRelaxMaxCent", config.ptb_relax_max_cent),
        (
            "manualAdaptiveMaxPriceRelaxStepCent",
            config.max_price_relax_step_cent,
        ),
        (
            "manualAdaptiveMaxPriceRelaxMaxCent",
            config.max_price_relax_max_cent,
        ),
        (
            "manualAdaptivePtbSlBumpStepCent",
            config.ptb_sl_bump_step_cent,
        ),
        ("manualAdaptivePtbSlBumpMaxCent", config.ptb_sl_bump_max_cent),
        (
            "manualAdaptiveMaxPriceSlPenaltyStepCent",
            config.max_price_sl_penalty_step_cent,
        ),
        (
            "manualAdaptiveMaxPriceSlPenaltyMaxCent",
            config.max_price_sl_penalty_max_cent,
        ),
        (
            "manualAdaptivePtbRelaxDecayPerMarketCent",
            config.ptb_relax_decay_per_market_cent,
        ),
        (
            "manualAdaptivePtbSlBumpDecayPerCleanMarketCent",
            config.ptb_sl_bump_decay_per_clean_market_cent,
        ),
        (
            "manualAdaptiveMaxPriceRelaxDecayPerMarketCent",
            config.max_price_relax_decay_per_market_cent,
        ),
        (
            "manualAdaptiveMaxPriceSlPenaltyDecayPerCleanMarketCent",
            config.max_price_sl_penalty_decay_per_clean_market_cent,
        ),
    ] {
        anyhow::ensure!(
            value.is_finite() && value >= 0.0,
            "action.place_order {name} must be >= 0"
        );
    }
    anyhow::ensure!(
        config.max_price_relax_hard_cap_cent.is_finite()
            && config.max_price_relax_hard_cap_cent > 0.0
            && config.max_price_relax_hard_cap_cent < 100.0,
        "action.place_order manualAdaptiveMaxPriceRelaxHardCapCent must be in (0, 100)"
    );
    anyhow::ensure!(
        config.miss_relax_size_multiplier.is_finite()
            && config.miss_relax_size_multiplier > 0.0
            && config.miss_relax_size_multiplier <= 1.0,
        "action.place_order manualAdaptiveMissRelaxSizeMultiplier must be in (0, 1]"
    );
    Ok(())
}

fn validate_manual_adaptive_trend_delta_by_scope(node: &TradeFlowNode) -> Result<()> {
    let Some(value) = node.config.get("manualAdaptiveTrendDeltaUsdByScope") else {
        return Ok(());
    };
    let parsed = match value {
        Value::Null => return Ok(()),
        Value::Object(map) => Value::Object(map.clone()),
        Value::String(raw) if raw.trim().is_empty() => return Ok(()),
        Value::String(raw) => serde_json::from_str::<Value>(raw).map_err(|err| {
            anyhow::anyhow!(
                "action.place_order manualAdaptiveTrendDeltaUsdByScope must be a JSON object: {err}"
            )
        })?,
        _ => anyhow::bail!(
            "action.place_order manualAdaptiveTrendDeltaUsdByScope must be a JSON object"
        ),
    };
    let Some(map) = parsed.as_object() else {
        anyhow::bail!(
            "action.place_order manualAdaptiveTrendDeltaUsdByScope must be a JSON object"
        );
    };
    for (scope, value) in map {
        let value = value_as_f64(value).ok_or_else(|| {
            anyhow::anyhow!(
                "action.place_order manualAdaptiveTrendDeltaUsdByScope.{scope} must be > 0"
            )
        })?;
        anyhow::ensure!(
            value.is_finite() && value > 0.0,
            "action.place_order manualAdaptiveTrendDeltaUsdByScope.{scope} must be > 0"
        );
    }
    Ok(())
}

fn manual_adaptive_trend_delta_usd_for_scope(
    node: &TradeFlowNode,
    market_slug: &str,
    fallback: f64,
    self_tune_enabled: bool,
) -> f64 {
    let scope = find_updown_scope_by_slug(market_slug);
    let scope_key = scope.as_ref().map(|scope| scope.scope.to_ascii_lowercase());
    let asset_key = scope.as_ref().map(|scope| scope.asset.to_ascii_lowercase());
    if let Some(value) = node
        .config
        .get("manualAdaptiveTrendDeltaUsdByScope")
        .and_then(|raw| manual_adaptive_trend_delta_scope_lookup(raw, scope_key.as_deref(), asset_key.as_deref()))
    {
        return value;
    }
    if self_tune_enabled {
        if let Some(asset) = asset_key.as_deref() {
            return match asset {
                "eth" => 0.5,
                "btc" => 10.0,
                "sol" => 0.05,
                _ => fallback,
            };
        }
    }
    fallback
}

fn manual_adaptive_trend_delta_scope_lookup(
    raw: &Value,
    scope_key: Option<&str>,
    asset_key: Option<&str>,
) -> Option<f64> {
    let parsed;
    let map = match raw {
        Value::Object(map) => map,
        Value::String(raw) => {
            parsed = serde_json::from_str::<Value>(raw).ok()?;
            parsed.as_object()?
        }
        _ => return None,
    };
    [scope_key, asset_key]
        .into_iter()
        .flatten()
        .find_map(|key| {
            map.get(key)
                .or_else(|| map.get(&key.to_ascii_uppercase()))
                .and_then(value_as_f64)
                .filter(|value| value.is_finite() && *value > 0.0)
        })
}

fn pair_lock_manual_self_tune_state_from_value(value: Option<&Value>) -> PairLockManualSelfTuneState {
    let Some(value) = value.and_then(Value::as_object) else {
        return PairLockManualSelfTuneState::default();
    };
    PairLockManualSelfTuneState {
        miss_streak: value.get("miss_streak").and_then(value_as_i64).unwrap_or(0).max(0),
        sl_streak: value.get("sl_streak").and_then(value_as_i64).unwrap_or(0).max(0),
        ptb_relax_credit_cent: value
            .get("ptb_relax_credit_cent")
            .and_then(value_as_f64)
            .unwrap_or(0.0)
            .max(0.0),
        ptb_sl_bump_cent: value
            .get("ptb_sl_bump_cent")
            .and_then(value_as_f64)
            .unwrap_or(0.0)
            .max(0.0),
        max_price_relax_credit_cent: value
            .get("max_price_relax_credit_cent")
            .and_then(value_as_f64)
            .unwrap_or(0.0)
            .max(0.0),
        max_price_sl_penalty_cent: value
            .get("max_price_sl_penalty_cent")
            .and_then(value_as_f64)
            .unwrap_or(0.0)
            .max(0.0),
        cooldown_markets_left: value
            .get("cooldown_markets_left")
            .and_then(value_as_i64)
            .unwrap_or(0)
            .max(0),
        lockdown_markets_left: value
            .get("lockdown_markets_left")
            .and_then(value_as_i64)
            .unwrap_or(0)
            .max(0),
        clean_markets_since_lockdown: value
            .get("clean_markets_since_lockdown")
            .and_then(value_as_i64)
            .unwrap_or(0)
            .max(0),
        last_updated_market_id: value
            .get("last_updated_market_id")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

fn pair_lock_manual_self_tune_state_value(state: &PairLockManualSelfTuneState) -> Value {
    json!({
        "miss_streak": state.miss_streak,
        "sl_streak": state.sl_streak,
        "ptb_relax_credit_cent": pair_lock_manual_adaptive_round_payload_number(state.ptb_relax_credit_cent),
        "ptb_sl_bump_cent": pair_lock_manual_adaptive_round_payload_number(state.ptb_sl_bump_cent),
        "max_price_relax_credit_cent": pair_lock_manual_adaptive_round_payload_number(state.max_price_relax_credit_cent),
        "max_price_sl_penalty_cent": pair_lock_manual_adaptive_round_payload_number(state.max_price_sl_penalty_cent),
        "cooldown_markets_left": state.cooldown_markets_left,
        "lockdown_markets_left": state.lockdown_markets_left,
        "clean_markets_since_lockdown": state.clean_markets_since_lockdown,
        "last_updated_market_id": state.last_updated_market_id,
    })
}

fn pair_lock_manual_self_tune_state(
    context: &Value,
    node_key: &str,
    scope_side: &str,
) -> PairLockManualSelfTuneState {
    pair_lock_manual_self_tune_state_from_value(
        flow_node_state(context, node_key, MANUAL_ADAPTIVE_SELF_TUNE_STATE_KEY)
            .and_then(|map| map.get(scope_side)),
    )
}

fn set_pair_lock_manual_self_tune_state(
    context: &mut Value,
    node_key: &str,
    scope_side: &str,
    state: &PairLockManualSelfTuneState,
) {
    let node_state = ensure_nested_object(context, "nodeState");
    if !node_state
        .get(node_key)
        .map(Value::is_object)
        .unwrap_or(false)
    {
        node_state.insert(node_key.to_string(), json!({}));
    }
    let node_obj = node_state
        .get_mut(node_key)
        .and_then(Value::as_object_mut)
        .expect("node state object");
    if !node_obj
        .get(MANUAL_ADAPTIVE_SELF_TUNE_STATE_KEY)
        .map(Value::is_object)
        .unwrap_or(false)
    {
        node_obj.insert(MANUAL_ADAPTIVE_SELF_TUNE_STATE_KEY.to_string(), json!({}));
    }
    if let Some(map) = node_obj
        .get_mut(MANUAL_ADAPTIVE_SELF_TUNE_STATE_KEY)
        .and_then(Value::as_object_mut)
    {
        map.insert(
            scope_side.to_string(),
            pair_lock_manual_self_tune_state_value(state),
        );
    }
}

fn pair_lock_manual_self_tune_safe_regime(volume_regime: &str, ptb_trend: &str) -> bool {
    matches!(volume_regime, "normal" | "elevated") && matches!(ptb_trend, "expanding" | "flat")
}

fn pair_lock_manual_self_tune_runtime(
    context: &Value,
    node_key: &str,
    scope_side: &str,
    volume_regime: &str,
    ptb_trend: &str,
    config: PairLockManualSelfTuneConfig,
) -> PairLockManualSelfTuneRuntime {
    let state = pair_lock_manual_self_tune_state(context, node_key, scope_side);
    let cooldown_active = state.cooldown_markets_left > 0;
    let lockdown_active = state.lockdown_markets_left > 0;
    let miss_relax_applies = config.enabled
        && config.miss_relax_enabled
        && !cooldown_active
        && !lockdown_active
        && pair_lock_manual_self_tune_safe_regime(volume_regime, ptb_trend);
    PairLockManualSelfTuneRuntime {
        state,
        miss_relax_applies,
        cooldown_active,
        lockdown_active,
    }
}

fn pair_lock_manual_self_tune_state_payload(
    previous: &PairLockManualSelfTuneState,
    next: &PairLockManualSelfTuneState,
) -> Value {
    json!({
        "previous": pair_lock_manual_self_tune_state_value(previous),
        "next": pair_lock_manual_self_tune_state_value(next),
    })
}

fn pair_lock_manual_self_tune_base_ptb_cent(node: &TradeFlowNode) -> Option<f64> {
    let value = node_config_f64(node, "priceToBeatMaxDiff")?;
    let unit = node_config_string(node, "priceToBeatMaxDiffUnit")
        .unwrap_or_else(|| "usd".to_string())
        .to_ascii_lowercase();
    Some(if unit == "cent" { value } else { value * 100.0 })
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn pair_lock_manual_self_tune_base_max_cent(node: &TradeFlowNode) -> Option<f64> {
    node_config_f64(node, "maxPriceCent").filter(|value| value.is_finite() && *value > 0.0)
}

fn pair_lock_manual_self_tune_preview_payload(
    node: &TradeFlowNode,
    previous: &PairLockManualSelfTuneState,
    next: &PairLockManualSelfTuneState,
) -> Value {
    let base_ptb_cent = pair_lock_manual_self_tune_base_ptb_cent(node);
    let base_max_cent = pair_lock_manual_self_tune_base_max_cent(node);
    let ptb_before = base_ptb_cent.map(|base| {
        (base - previous.ptb_relax_credit_cent + previous.ptb_sl_bump_cent).max(0.000_001)
    });
    let ptb_after = base_ptb_cent.map(|base| {
        (base - next.ptb_relax_credit_cent + next.ptb_sl_bump_cent).max(0.000_001)
    });
    let max_before = base_max_cent.map(|base| {
        (base + previous.max_price_relax_credit_cent - previous.max_price_sl_penalty_cent)
            .max(1.0)
    });
    let max_after = base_max_cent.map(|base| {
        (base + next.max_price_relax_credit_cent - next.max_price_sl_penalty_cent).max(1.0)
    });
    json!({
        "base_required_ptb_cent": base_ptb_cent,
        "effective_required_ptb_cent_before": ptb_before,
        "effective_required_ptb_cent_after": ptb_after,
        "base_max_price_cent": base_max_cent,
        "effective_max_price_cent_before": max_before,
        "effective_max_price_cent_after": max_after,
    })
}

fn pair_lock_manual_self_tune_update_payload(
    kind: PairLockManualSelfTuneUpdateKind,
    node: &TradeFlowNode,
    scope_side: &str,
    market_slug: &str,
    outcome_label: &str,
    previous: &PairLockManualSelfTuneState,
    next: &PairLockManualSelfTuneState,
    details: Value,
) -> Value {
    json!({
        "enabled": true,
        "strategy": PAIR_LOCK_STRATEGY_MANUAL_ADAPTIVE_RISK_V1,
        "feature": "manual_self_tuning_v1",
        "event": kind.event_type(),
        "reason": kind.reason(),
        "scope_side": scope_side,
        "market_slug": market_slug,
        "outcome_label": outcome_label,
        "state": pair_lock_manual_self_tune_state_payload(previous, next),
        "preview": pair_lock_manual_self_tune_preview_payload(node, previous, next),
        "details": details,
    })
}

fn pair_lock_manual_self_tune_decay_value(value: f64, decay: f64) -> f64 {
    (value - decay.max(0.0)).max(0.0)
}

fn pair_lock_manual_self_tune_record_sl(
    context: &mut Value,
    node: &TradeFlowNode,
    market_slug: &str,
    outcome_label: &str,
    config: PairLockManualSelfTuneConfig,
    cooldown_markets: usize,
) -> Option<PairLockManualSelfTuneUpdate> {
    if !config.enabled || !config.sl_tighten_enabled {
        return None;
    }
    let scope_side = pair_lock_manual_adaptive_scope_side(market_slug, outcome_label);
    let mut state = pair_lock_manual_self_tune_state(context, &node.key, &scope_side);
    if state.last_updated_market_id.as_deref() == Some(market_slug) {
        return None;
    }
    let previous = state.clone();
    state.sl_streak = state.sl_streak.saturating_add(1);
    state.miss_streak = 0;
    state.ptb_relax_credit_cent = 0.0;
    state.max_price_relax_credit_cent = 0.0;
    state.ptb_sl_bump_cent = (state.ptb_sl_bump_cent + config.ptb_sl_bump_step_cent)
        .min(config.ptb_sl_bump_max_cent);
    state.max_price_sl_penalty_cent =
        (state.max_price_sl_penalty_cent + config.max_price_sl_penalty_step_cent)
            .min(config.max_price_sl_penalty_max_cent);
    state.cooldown_markets_left = cooldown_markets as i64;
    state.last_updated_market_id = Some(market_slug.to_string());
    let kind = if state.sl_streak as usize >= config.consecutive_sl_lockdown_after {
        state.lockdown_markets_left = config.lockdown_max_markets as i64;
        state.clean_markets_since_lockdown = 0;
        PairLockManualSelfTuneUpdateKind::Lockdown
    } else if state.sl_streak > 1 {
        PairLockManualSelfTuneUpdateKind::StrictPlus
    } else {
        PairLockManualSelfTuneUpdateKind::SlTighten
    };
    set_pair_lock_manual_self_tune_state(context, &node.key, &scope_side, &state);
    let payload = pair_lock_manual_self_tune_update_payload(
        kind,
        node,
        &scope_side,
        market_slug,
        outcome_label,
        &previous,
        &state,
        json!({
            "cooldown_markets": cooldown_markets,
            "reentry": if config.sl_disable_reentry { "OFF" } else { "UNCHANGED" },
        }),
    );
    Some(PairLockManualSelfTuneUpdate {
        kind,
        scope_side,
        market_slug: market_slug.to_string(),
        outcome_label: outcome_label.to_string(),
        previous,
        next: state,
        payload,
    })
}

fn pair_lock_manual_self_tune_market_update(
    context: &mut Value,
    node: &TradeFlowNode,
    config: PairLockManualSelfTuneConfig,
    summary: &bot_infra::db::TradeFlowAutoTuneMarketSummaryInput,
    outcome_label: &str,
) -> Option<PairLockManualSelfTuneUpdate> {
    if !config.enabled {
        return None;
    }
    let scope_side = pair_lock_manual_adaptive_scope_side(&summary.market_slug, outcome_label);
    let mut state = pair_lock_manual_self_tune_state(context, &node.key, &scope_side);
    if state.last_updated_market_id.as_deref() == Some(summary.market_slug.as_str()) {
        return None;
    }
    let previous = state.clone();
    let manual = summary
        .metrics_json
        .get("manual_adaptive_risk")
        .unwrap_or(&Value::Null);
    let volume_regime = manual
        .get("volume_regime")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let ptb_trend = manual
        .get("ptb_trend")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let safe_regime = pair_lock_manual_self_tune_safe_regime(volume_regime, ptb_trend);
    let unsafe_block = summary.execution_floor_block
        || summary.pair_total_block
        || summary.counter_max_block
        || summary.counter_floor_block
        || summary.risk_block
        || summary.data_problem_block;
    let no_order = !summary.builder_order_created && !summary.order_submitted && !summary.order_filled;
    let safe_block = summary.ptb_block || summary.max_price_block;
    if state.cooldown_markets_left > 0 {
        state.cooldown_markets_left -= 1;
    }
    if state.lockdown_markets_left > 0 {
        state.lockdown_markets_left -= 1;
    }
    let cooldown_or_lockdown = state.cooldown_markets_left > 0 || state.lockdown_markets_left > 0;
    let safe_no_order = config.miss_relax_enabled
        && no_order
        && safe_block
        && safe_regime
        && !unsafe_block
        && !summary.sl_hit
        && !cooldown_or_lockdown;
    let clean_market = config.clean_market_decay_enabled
        && safe_regime
        && !summary.sl_hit
        && !unsafe_block
        && !safe_no_order;
    let mut kind = None;
    if safe_no_order {
        state.miss_streak = state.miss_streak.saturating_add(1);
        if state.miss_streak as usize >= config.miss_relax_after_no_order_markets
            && state.miss_streak as usize % config.miss_relax_after_no_order_markets == 0
        {
            state.ptb_relax_credit_cent =
                (state.ptb_relax_credit_cent + config.ptb_relax_step_cent)
                    .min(config.ptb_relax_max_cent);
            state.max_price_relax_credit_cent =
                (state.max_price_relax_credit_cent + config.max_price_relax_step_cent)
                    .min(config.max_price_relax_max_cent);
            kind = Some(PairLockManualSelfTuneUpdateKind::SelfRelax);
        }
    } else if clean_market {
        state.miss_streak = 0;
        state.sl_streak = 0;
        if previous.lockdown_markets_left > 0 {
            state.clean_markets_since_lockdown = state.clean_markets_since_lockdown.saturating_add(1);
            if state.clean_markets_since_lockdown as usize >= config.lockdown_release_clean_markets {
                state.lockdown_markets_left = 0;
            }
        } else {
            state.clean_markets_since_lockdown = 0;
        }
        let before_decay = state.clone();
        state.ptb_relax_credit_cent = pair_lock_manual_self_tune_decay_value(
            state.ptb_relax_credit_cent,
            config.ptb_relax_decay_per_market_cent,
        );
        state.max_price_relax_credit_cent = pair_lock_manual_self_tune_decay_value(
            state.max_price_relax_credit_cent,
            config.max_price_relax_decay_per_market_cent,
        );
        state.ptb_sl_bump_cent = pair_lock_manual_self_tune_decay_value(
            state.ptb_sl_bump_cent,
            config.ptb_sl_bump_decay_per_clean_market_cent,
        );
        state.max_price_sl_penalty_cent = pair_lock_manual_self_tune_decay_value(
            state.max_price_sl_penalty_cent,
            config.max_price_sl_penalty_decay_per_clean_market_cent,
        );
        if state != before_decay || state.lockdown_markets_left != previous.lockdown_markets_left {
            kind = Some(PairLockManualSelfTuneUpdateKind::Decay);
        }
    } else {
        state.clean_markets_since_lockdown = 0;
    }
    if previous == state {
        return None;
    }
    state.last_updated_market_id = Some(summary.market_slug.clone());
    set_pair_lock_manual_self_tune_state(context, &node.key, &scope_side, &state);
    let payload = pair_lock_manual_self_tune_update_payload(
        kind.unwrap_or(PairLockManualSelfTuneUpdateKind::Decay),
        node,
        &scope_side,
        &summary.market_slug,
        outcome_label,
        &previous,
        &state,
        json!({
            "safe_no_order": safe_no_order,
            "clean_market": clean_market,
            "volume_regime": volume_regime,
            "ptb_trend": ptb_trend,
            "safe_block": safe_block,
            "unsafe_block": unsafe_block,
            "no_order": no_order,
            "max_price_block": summary.max_price_block,
            "ptb_block": summary.ptb_block,
        }),
    );
    kind.map(|kind| PairLockManualSelfTuneUpdate {
        kind,
        scope_side,
        market_slug: summary.market_slug.clone(),
        outcome_label: outcome_label.to_string(),
        previous,
        next: state,
        payload,
    })
}

fn pair_lock_manual_self_tune_notify_enabled(
    cfg: PairLockManualAdaptiveNotifyConfig,
    kind: PairLockManualSelfTuneUpdateKind,
) -> bool {
    match kind {
        PairLockManualSelfTuneUpdateKind::SelfRelax => cfg.notify_strict,
        PairLockManualSelfTuneUpdateKind::SlTighten
        | PairLockManualSelfTuneUpdateKind::StrictPlus
        | PairLockManualSelfTuneUpdateKind::Lockdown => cfg.notify_sl_bump,
        PairLockManualSelfTuneUpdateKind::Decay => cfg.notify_summary,
    }
}

fn build_pair_lock_manual_self_tune_message(update: &PairLockManualSelfTuneUpdate) -> String {
    let preview = update.payload.get("preview").unwrap_or(&Value::Null);
    let title = match update.kind {
        PairLockManualSelfTuneUpdateKind::SelfRelax => "🟢 Manual Adaptive SELF-RELAX",
        PairLockManualSelfTuneUpdateKind::SlTighten => "🔴 Manual Adaptive SL TIGHTEN",
        PairLockManualSelfTuneUpdateKind::StrictPlus => "🔴 Manual Adaptive STRICT+",
        PairLockManualSelfTuneUpdateKind::Lockdown => "🧱 Manual Adaptive LOCKDOWN",
        PairLockManualSelfTuneUpdateKind::Decay => "🟡 Manual Adaptive DECAY",
    };
    format!(
        "{title} {}\nmiss={} -> {} | sl={} -> {}\nPTB {} -> {} | max {} -> {}\nreason={}",
        update.scope_side,
        update.previous.miss_streak,
        update.next.miss_streak,
        update.previous.sl_streak,
        update.next.sl_streak,
        pair_lock_manual_notify_cent(
            preview
                .get("effective_required_ptb_cent_before")
                .and_then(value_as_f64)
        ),
        pair_lock_manual_notify_cent(
            preview
                .get("effective_required_ptb_cent_after")
                .and_then(value_as_f64)
        ),
        pair_lock_manual_notify_cent(
            preview
                .get("effective_max_price_cent_before")
                .and_then(value_as_f64)
        ),
        pair_lock_manual_notify_cent(
            preview
                .get("effective_max_price_cent_after")
                .and_then(value_as_f64)
        ),
        update.kind.reason(),
    )
}

async fn maybe_notify_pair_lock_manual_self_tune_update(
    repo: &PostgresRepository,
    run: &TradeFlowRun,
    node: &TradeFlowNode,
    context: &mut Value,
    update: PairLockManualSelfTuneUpdate,
) -> Result<()> {
    let cfg = resolve_pair_lock_manual_adaptive_notify_config(node)?;
    if !pair_lock_manual_self_tune_notify_enabled(cfg, update.kind) {
        return Ok(());
    }
    emit_pair_lock_manual_adaptive_notification(
        repo,
        run,
        node,
        context,
        update.kind.event_type(),
        &update.market_slug,
        &update.outcome_label,
        update.kind.reason(),
        Some(&format!(
            "{}:{}:{}",
            update.kind.reason(),
            update.next.miss_streak,
            update.next.sl_streak
        )),
        update.payload.clone(),
        build_pair_lock_manual_self_tune_message(&update),
        matches!(update.kind, PairLockManualSelfTuneUpdateKind::Lockdown),
    )
    .await?;
    Ok(())
}

async fn maybe_record_pair_lock_manual_self_tune_market(
    repo: &PostgresRepository,
    run_spec: &mut WsOpenPositionPriceRunSpec,
    action_node: &TradeFlowNode,
    summary: &bot_infra::db::TradeFlowAutoTuneMarketSummaryInput,
    outcome_label: &str,
) -> Result<()> {
    let config = resolve_pair_lock_manual_self_tune_config(action_node)?;
    let previous_context = run_spec.context.clone();
    let update = pair_lock_manual_self_tune_market_update(
        &mut run_spec.context,
        action_node,
        config,
        summary,
        outcome_label,
    );
    if run_spec.context != previous_context {
        run_spec.context_dirty = true;
    }
    let Some(update) = update else {
        return Ok(());
    };
    if let Some(run) = repo.get_trade_flow_run(run_spec.run_id).await? {
        maybe_notify_pair_lock_manual_self_tune_update(
            repo,
            &run,
            action_node,
            &mut run_spec.context,
            update,
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod pair_lock_manual_self_tuning_tests {
    use super::*;

    fn test_node() -> TradeFlowNode {
        TradeFlowNode {
            key: "action".to_string(),
            node_type: "action.place_order".to_string(),
            config: json!({
                "mode": "pair_lock",
                "pairLockStrategy": "manual_adaptive_risk_v1",
                "priceToBeatGuardEnabled": true,
                "priceToBeatMode": "manual",
                "priceToBeatMaxDiff": 125,
                "priceToBeatMaxDiffUnit": "cent",
                "maxPriceCent": 85,
                "manualAdaptiveSelfTuneEnabled": true,
            }),
        }
    }

    fn test_config() -> PairLockManualSelfTuneConfig {
        PairLockManualSelfTuneConfig {
            enabled: true,
            ..PairLockManualSelfTuneConfig::default()
        }
    }

    fn summary(
        market_slug: &str,
        volume_regime: &str,
        ptb_trend: &str,
        max_price_block: bool,
        ptb_block: bool,
        builder_order_created: bool,
        sl_hit: bool,
    ) -> bot_infra::db::TradeFlowAutoTuneMarketSummaryInput {
        bot_infra::db::TradeFlowAutoTuneMarketSummaryInput {
            definition_id: 1,
            version_id: 1,
            flow_run_id: Some(1),
            node_key: "action".to_string(),
            market_scope: "eth_5m_updown".to_string(),
            market_slug: market_slug.to_string(),
            window_start: None,
            window_end: None,
            completed_at: Utc::now(),
            trigger_passed: true,
            action_started: builder_order_created || max_price_block || ptb_block,
            builder_order_created,
            order_submitted: builder_order_created,
            order_filled: builder_order_created,
            first_terminal_guard_scope: None,
            first_terminal_guard_code: None,
            first_terminal_guard_node: None,
            first_terminal_guard_at: None,
            last_guard_scope: None,
            last_guard_code: None,
            max_price_block,
            execution_floor_block: false,
            ptb_block,
            pair_total_block: false,
            counter_max_block: false,
            counter_floor_block: false,
            risk_block: false,
            data_problem_block: false,
            best_ask_at_block: None,
            max_price_effective: None,
            execution_floor_effective: None,
            pair_total_effective: None,
            counter_price_effective: None,
            iv_edge_margin: None,
            iv_dynamic_threshold: None,
            gap_strength: None,
            required_gap_strength: None,
            binance_stale_ms: None,
            binance_same_direction: None,
            depth_ok: Some(true),
            floor_recovered_once: false,
            max_best_ask_after_block: None,
            tradable_seconds_count: None,
            depth_ok_seconds_count: None,
            pair_session_id: None,
            pair_locked: false,
            locked_qty: None,
            unpaired_qty: None,
            locked_profit_per_share: None,
            orphan_detected: false,
            protective_unwind_triggered: false,
            sl_hit,
            tp_hit: false,
            realized_pnl_usdc: None,
            metrics_json: json!({
                "outcome_label": "Up",
                "manual_adaptive_risk": {
                    "volume_regime": volume_regime,
                    "ptb_trend": ptb_trend,
                }
            }),
        }
    }

    #[test]
    fn manual_self_tune_safe_no_order_relaxes_once_per_three_markets() {
        let node = test_node();
        let config = test_config();
        let mut context = json!({});

        for index in 1..=2 {
            let update = pair_lock_manual_self_tune_market_update(
                &mut context,
                &node,
                config,
                &summary(&format!("eth-updown-5m-{index}"), "normal", "expanding", true, false, false, false),
                "Up",
            );
            assert!(update.is_none());
        }
        let update = pair_lock_manual_self_tune_market_update(
            &mut context,
            &node,
            config,
            &summary("eth-updown-5m-3", "normal", "expanding", true, false, false, false),
            "Up",
        )
        .expect("third safe miss should relax");
        let scope_side = pair_lock_manual_adaptive_scope_side("eth-updown-5m-3", "Up");
        let state = pair_lock_manual_self_tune_state(&context, "action", &scope_side);

        assert_eq!(update.kind, PairLockManualSelfTuneUpdateKind::SelfRelax);
        assert_eq!(state.miss_streak, 3);
        assert_eq!(state.ptb_relax_credit_cent, 5.0);
        assert_eq!(state.max_price_relax_credit_cent, 1.0);
    }

    #[test]
    fn manual_self_tune_high_volume_no_order_does_not_relax() {
        let node = test_node();
        let config = test_config();
        let mut context = json!({});
        let update = pair_lock_manual_self_tune_market_update(
            &mut context,
            &node,
            config,
            &summary("eth-updown-5m-1", "high", "expanding", true, false, false, false),
            "Up",
        );
        let scope_side = pair_lock_manual_adaptive_scope_side("eth-updown-5m-1", "Up");
        let state = pair_lock_manual_self_tune_state(&context, "action", &scope_side);

        assert!(update.is_none());
        assert_eq!(state.miss_streak, 0);
        assert_eq!(state.ptb_relax_credit_cent, 0.0);
    }

    #[test]
    fn manual_self_tune_same_market_marker_prevents_double_count() {
        let node = test_node();
        let config = test_config();
        let mut context = json!({});
        let market = summary("eth-updown-5m-1", "normal", "flat", false, true, false, false);

        pair_lock_manual_self_tune_market_update(&mut context, &node, config, &market, "Up");
        pair_lock_manual_self_tune_market_update(&mut context, &node, config, &market, "Up");
        let scope_side = pair_lock_manual_adaptive_scope_side("eth-updown-5m-1", "Up");
        let state = pair_lock_manual_self_tune_state(&context, "action", &scope_side);

        assert_eq!(state.miss_streak, 1);
    }

    #[test]
    fn manual_self_tune_lockdown_releases_after_clean_markets() {
        let node = test_node();
        let config = test_config();
        let mut context = json!({});

        for index in 1..=3 {
            pair_lock_manual_self_tune_record_sl(
                &mut context,
                &node,
                &format!("eth-updown-5m-sl-{index}"),
                "Up",
                config,
                3,
            );
        }
        let scope_side = pair_lock_manual_adaptive_scope_side("eth-updown-5m-sl-3", "Up");
        let locked = pair_lock_manual_self_tune_state(&context, "action", &scope_side);
        assert!(locked.lockdown_markets_left > 0);

        for index in 1..=3 {
            pair_lock_manual_self_tune_market_update(
                &mut context,
                &node,
                config,
                &summary(&format!("eth-updown-5m-clean-{index}"), "normal", "expanding", false, false, true, false),
                "Up",
            );
        }
        let released = pair_lock_manual_self_tune_state(&context, "action", &scope_side);

        assert_eq!(released.lockdown_markets_left, 0);
        assert!(released.ptb_sl_bump_cent < locked.ptb_sl_bump_cent);
        assert!(released.max_price_sl_penalty_cent < locked.max_price_sl_penalty_cent);
    }
}
