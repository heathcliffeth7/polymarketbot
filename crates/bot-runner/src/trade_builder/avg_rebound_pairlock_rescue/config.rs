const ACTION_PLACE_ORDER_MODE_AVG_REBOUND_PAIRLOCK_RESCUE_V1: &str =
    "avg_rebound_pairlock_rescue_v1";
const AVG_REBOUND_PAIRLOCK_RESCUE_BINDING_MODE: &str = "avg_rebound_pairlock_rescue_only";
const AVG_REBOUND_PAIRLOCK_RESCUE_CONFIG_KEY: &str = "avgReboundPairlockRescue";
const AVG_REBOUND_PAIRLOCK_RESCUE_ORDER_MARKER_KEY: &str = "avgReboundPairlockRescueOrder";
const AVG_REBOUND_PAIRLOCK_RESCUE_ROOT_NODE_KEY: &str = "avgReboundPairlockRescueRootNodeKey";
const AVG_REBOUND_PAIRLOCK_RESCUE_SESSION_ID_KEY: &str = "avgReboundPairlockRescueSessionId";
const AVG_REBOUND_PAIRLOCK_RESCUE_ROLE_KEY: &str = "avgReboundPairlockRescueRole";
const AVG_REBOUND_PAIRLOCK_RESCUE_INTENT_KEY: &str = "avgReboundPairlockRescueIntent";
const AVG_REBOUND_PAIRLOCK_RESCUE_STAGE_ID_KEY: &str = "avgReboundPairlockRescueStageId";
const AVG_REBOUND_PAIRLOCK_RESCUE_TIER_OR_LEG_ID_KEY: &str = "avgReboundPairlockRescueTierOrLegId";
const AVG_REBOUND_PAIRLOCK_RESCUE_DECISION_ID_KEY: &str = "avgReboundPairlockRescueDecisionId";
const AVG_REBOUND_PAIRLOCK_RESCUE_REQUESTED_QTY_KEY: &str = "avgReboundPairlockRescueRequestedQty";

const AVG_REBOUND_INTENT_PRIMARY_LADDER: &str = "PRIMARY_LADDER";
const AVG_REBOUND_INTENT_PROFIT_PAIRLOCK: &str = "PROFIT_PAIRLOCK";
const AVG_REBOUND_INTENT_GIVEBACK_GUARD: &str = "GIVEBACK_GUARD";
const AVG_REBOUND_INTENT_NORMAL_RESCUE: &str = "NORMAL_RESCUE";
const AVG_REBOUND_INTENT_EMERGENCY_RESCUE: &str = "EMERGENCY_RESCUE";
const AVG_REBOUND_INTENT_HARD_RESCUE: &str = "HARD_RESCUE";
const AVG_REBOUND_INTENT_LAST_CHANCE_RESCUE: &str = "LAST_CHANCE_RESCUE";
const AVG_REBOUND_PRIMARY_SIDE_SELECTION_CHEAPEST_ELIGIBLE: &str = "cheapest_eligible";

const AVG_REBOUND_STATUS_BUILDING_PRIMARY: &str = "BUILDING_PRIMARY";
const AVG_REBOUND_STATUS_PROFIT_LOCKING: &str = "PROFIT_LOCKING";
const AVG_REBOUND_STATUS_GUARD_EXIT: &str = "GUARD_EXIT";
const AVG_REBOUND_STATUS_RESCUE_EXIT: &str = "RESCUE_EXIT";
const AVG_REBOUND_STATUS_LOCKED: &str = "LOCKED";
const AVG_REBOUND_QTY_EPSILON: f64 = 0.0001;

#[derive(Debug, Clone, PartialEq)]
struct AvgReboundPrimaryTierConfig {
    id: String,
    price_cap: rust_decimal::Decimal,
    qty: rust_decimal::Decimal,
}

#[derive(Debug, Clone, PartialEq)]
struct AvgReboundProfitLegConfig {
    id: String,
    opposite_vwap_cap: rust_decimal::Decimal,
    qty: rust_decimal::Decimal,
}

#[derive(Debug, Clone, PartialEq)]
struct AvgReboundGivebackGuardConfig {
    trigger: rust_decimal::Decimal,
    max_execution_vwap: rust_decimal::Decimal,
}

#[derive(Debug, Clone, PartialEq)]
struct AvgReboundStageConfig {
    id: String,
    required_primary_tier_ids: Vec<String>,
    profit_legs: Vec<AvgReboundProfitLegConfig>,
    giveback_guard: AvgReboundGivebackGuardConfig,
}

#[derive(Debug, Clone, PartialEq)]
struct AvgReboundRescueConfig {
    enabled_only_after_full_ladder: bool,
    normal_vwap_cap: rust_decimal::Decimal,
    emergency_vwap_cap: rust_decimal::Decimal,
    hard_max_vwap_cap: rust_decimal::Decimal,
    last_chance_vwap_cap: Option<rust_decimal::Decimal>,
}

#[derive(Debug, Clone, PartialEq)]
struct AvgReboundPairlockRescueConfig {
    session_budget_usdc: rust_decimal::Decimal,
    reserved_budget_buffer_usdc: rust_decimal::Decimal,
    primary_outcome_label: String,
    opposite_outcome_label: String,
    primary_side_selection: String,
    order_type: String,
    execution_mode: String,
    vwap_source: String,
    extra_vwap_safety_buffer: rust_decimal::Decimal,
    target_profit_usdc: Option<rust_decimal::Decimal>,
    allow_primary_after_partial_profit: bool,
    pre_full_giveback_guard_enabled: bool,
    full_giveback_guard_enabled: bool,
    primary_ladder: Vec<AvgReboundPrimaryTierConfig>,
    stages: Vec<AvgReboundStageConfig>,
    rescue: AvgReboundRescueConfig,
}

fn avg_rebound_dec(value: &str) -> rust_decimal::Decimal {
    value
        .parse::<rust_decimal::Decimal>()
        .expect("valid avg rebound decimal default")
}

fn avg_rebound_default_config() -> AvgReboundPairlockRescueConfig {
    AvgReboundPairlockRescueConfig {
        session_budget_usdc: avg_rebound_dec("50"),
        reserved_budget_buffer_usdc: avg_rebound_dec("0.75"),
        primary_outcome_label: "auto".to_string(),
        opposite_outcome_label: "opposite".to_string(),
        primary_side_selection: AVG_REBOUND_PRIMARY_SIDE_SELECTION_CHEAPEST_ELIGIBLE.to_string(),
        order_type: "FOK".to_string(),
        execution_mode: "limit".to_string(),
        vwap_source: "rest_book".to_string(),
        extra_vwap_safety_buffer: avg_rebound_dec("0.005"),
        target_profit_usdc: None,
        allow_primary_after_partial_profit: true,
        pre_full_giveback_guard_enabled: false,
        full_giveback_guard_enabled: true,
        primary_ladder: vec![
            AvgReboundPrimaryTierConfig {
                id: "p50".to_string(),
                price_cap: avg_rebound_dec("0.50"),
                qty: avg_rebound_dec("8"),
            },
            AvgReboundPrimaryTierConfig {
                id: "p30".to_string(),
                price_cap: avg_rebound_dec("0.30"),
                qty: avg_rebound_dec("15"),
            },
            AvgReboundPrimaryTierConfig {
                id: "p10".to_string(),
                price_cap: avg_rebound_dec("0.10"),
                qty: avg_rebound_dec("24"),
            },
        ],
        stages: vec![
            AvgReboundStageConfig {
                id: "stage_50".to_string(),
                required_primary_tier_ids: vec!["p50".to_string()],
                profit_legs: vec![
                    AvgReboundProfitLegConfig {
                        id: "s50_profit_45".to_string(),
                        opposite_vwap_cap: avg_rebound_dec("0.45"),
                        qty: avg_rebound_dec("4"),
                    },
                    AvgReboundProfitLegConfig {
                        id: "s50_profit_40".to_string(),
                        opposite_vwap_cap: avg_rebound_dec("0.40"),
                        qty: avg_rebound_dec("4"),
                    },
                ],
                giveback_guard: AvgReboundGivebackGuardConfig {
                    trigger: avg_rebound_dec("0.47"),
                    max_execution_vwap: avg_rebound_dec("0.47"),
                },
            },
            AvgReboundStageConfig {
                id: "stage_30".to_string(),
                required_primary_tier_ids: vec!["p50".to_string(), "p30".to_string()],
                profit_legs: vec![
                    AvgReboundProfitLegConfig {
                        id: "s30_profit_59".to_string(),
                        opposite_vwap_cap: avg_rebound_dec("0.59"),
                        qty: avg_rebound_dec("8"),
                    },
                    AvgReboundProfitLegConfig {
                        id: "s30_profit_52".to_string(),
                        opposite_vwap_cap: avg_rebound_dec("0.52"),
                        qty: avg_rebound_dec("8"),
                    },
                    AvgReboundProfitLegConfig {
                        id: "s30_profit_45".to_string(),
                        opposite_vwap_cap: avg_rebound_dec("0.45"),
                        qty: avg_rebound_dec("7"),
                    },
                ],
                giveback_guard: AvgReboundGivebackGuardConfig {
                    trigger: avg_rebound_dec("0.63"),
                    max_execution_vwap: avg_rebound_dec("0.63"),
                },
            },
            AvgReboundStageConfig {
                id: "stage_full".to_string(),
                required_primary_tier_ids: vec![
                    "p50".to_string(),
                    "p30".to_string(),
                    "p10".to_string(),
                ],
                profit_legs: vec![
                    AvgReboundProfitLegConfig {
                        id: "full_profit_72".to_string(),
                        opposite_vwap_cap: avg_rebound_dec("0.72"),
                        qty: avg_rebound_dec("15"),
                    },
                    AvgReboundProfitLegConfig {
                        id: "full_profit_64".to_string(),
                        opposite_vwap_cap: avg_rebound_dec("0.64"),
                        qty: avg_rebound_dec("16"),
                    },
                    AvgReboundProfitLegConfig {
                        id: "full_profit_54".to_string(),
                        opposite_vwap_cap: avg_rebound_dec("0.54"),
                        qty: avg_rebound_dec("16"),
                    },
                ],
                giveback_guard: AvgReboundGivebackGuardConfig {
                    trigger: avg_rebound_dec("0.76"),
                    max_execution_vwap: avg_rebound_dec("0.78"),
                },
            },
        ],
        rescue: AvgReboundRescueConfig {
            enabled_only_after_full_ladder: true,
            normal_vwap_cap: avg_rebound_dec("0.78"),
            emergency_vwap_cap: avg_rebound_dec("0.81"),
            hard_max_vwap_cap: avg_rebound_dec("0.81"),
            last_chance_vwap_cap: None,
        },
    }
}

fn avg_rebound_config_root(node: &TradeFlowNode) -> Option<&Value> {
    node.config.get(AVG_REBOUND_PAIRLOCK_RESCUE_CONFIG_KEY)
}

fn avg_rebound_config_value<'a>(node: &'a TradeFlowNode, key: &str) -> Option<&'a Value> {
    avg_rebound_config_root(node)
        .and_then(|value| value.get(key))
        .or_else(|| node.config.get(key))
}

fn avg_rebound_config_decimal(
    node: &TradeFlowNode,
    key: &str,
) -> Result<Option<rust_decimal::Decimal>> {
    let Some(value) = avg_rebound_config_value(node, key) else {
        return Ok(None);
    };
    avg_rebound_value_decimal(value)
        .map(Some)
        .with_context(|| format!("avgReboundPairlockRescue.{key} must be a decimal"))
}

fn avg_rebound_value_decimal(value: &Value) -> Result<rust_decimal::Decimal> {
    match value {
        Value::String(value) => value
            .trim()
            .parse::<rust_decimal::Decimal>()
            .with_context(|| format!("invalid decimal: {value}")),
        Value::Number(value) => value
            .to_string()
            .parse::<rust_decimal::Decimal>()
            .with_context(|| format!("invalid decimal: {value}")),
        _ => anyhow::bail!("expected decimal string or number"),
    }
}

fn avg_rebound_value_string(value: Option<&Value>, default: &str) -> String {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_string()
}

fn avg_rebound_value_bool(value: Option<&Value>, default: bool) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => true,
            "false" | "0" | "no" | "off" => false,
            _ => default,
        },
        _ => default,
    }
}

fn avg_rebound_parse_primary_tier(value: &Value) -> Result<AvgReboundPrimaryTierConfig> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("primaryLadder entries must be objects"))?;
    let id = avg_rebound_value_string(object.get("id"), "");
    anyhow::ensure!(!id.is_empty(), "primaryLadder entries require id");
    let price_cap = avg_rebound_value_decimal(
        object
            .get("priceCap")
            .ok_or_else(|| anyhow::anyhow!("primaryLadder.{id}.priceCap is required"))?,
    )?;
    let qty = avg_rebound_value_decimal(
        object
            .get("qty")
            .ok_or_else(|| anyhow::anyhow!("primaryLadder.{id}.qty is required"))?,
    )?;
    anyhow::ensure!(
        price_cap > rust_decimal::Decimal::ZERO && price_cap < rust_decimal::Decimal::ONE,
        "primaryLadder.{id}.priceCap must be in (0, 1)"
    );
    anyhow::ensure!(
        qty > rust_decimal::Decimal::ZERO,
        "primaryLadder.{id}.qty must be > 0"
    );
    Ok(AvgReboundPrimaryTierConfig { id, price_cap, qty })
}

fn avg_rebound_parse_profit_leg(value: &Value) -> Result<AvgReboundProfitLegConfig> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("profitLegs entries must be objects"))?;
    let id = avg_rebound_value_string(object.get("id"), "");
    anyhow::ensure!(!id.is_empty(), "profitLegs entries require id");
    let opposite_vwap_cap = avg_rebound_value_decimal(
        object
            .get("oppositeVwapCap")
            .ok_or_else(|| anyhow::anyhow!("profitLegs.{id}.oppositeVwapCap is required"))?,
    )?;
    let qty = avg_rebound_value_decimal(
        object
            .get("qty")
            .ok_or_else(|| anyhow::anyhow!("profitLegs.{id}.qty is required"))?,
    )?;
    anyhow::ensure!(
        opposite_vwap_cap > rust_decimal::Decimal::ZERO
            && opposite_vwap_cap < rust_decimal::Decimal::ONE,
        "profitLegs.{id}.oppositeVwapCap must be in (0, 1)"
    );
    anyhow::ensure!(
        qty > rust_decimal::Decimal::ZERO,
        "profitLegs.{id}.qty must be > 0"
    );
    Ok(AvgReboundProfitLegConfig {
        id,
        opposite_vwap_cap,
        qty,
    })
}

fn avg_rebound_parse_stage(value: &Value) -> Result<AvgReboundStageConfig> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("stages entries must be objects"))?;
    let id = avg_rebound_value_string(object.get("id"), "");
    anyhow::ensure!(!id.is_empty(), "stages entries require id");
    let required_primary_tier_ids = object
        .get("requiredPrimaryTierIds")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("stage {id} requires requiredPrimaryTierIds"))?
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    anyhow::ensure!(
        !required_primary_tier_ids.is_empty(),
        "stage {id} requires at least one primary tier id"
    );
    let profit_legs = object
        .get("profitLegs")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("stage {id} requires profitLegs"))?
        .iter()
        .map(avg_rebound_parse_profit_leg)
        .collect::<Result<Vec<_>>>()?;
    let guard_object = object
        .get("givebackGuard")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("stage {id} requires givebackGuard"))?;
    let trigger = avg_rebound_value_decimal(
        guard_object
            .get("trigger")
            .ok_or_else(|| anyhow::anyhow!("stage {id} givebackGuard.trigger is required"))?,
    )?;
    let max_execution_vwap =
        avg_rebound_value_decimal(guard_object.get("maxExecutionVwap").ok_or_else(|| {
            anyhow::anyhow!("stage {id} givebackGuard.maxExecutionVwap is required")
        })?)?;
    Ok(AvgReboundStageConfig {
        id,
        required_primary_tier_ids,
        profit_legs,
        giveback_guard: AvgReboundGivebackGuardConfig {
            trigger,
            max_execution_vwap,
        },
    })
}

fn resolve_avg_rebound_pairlock_rescue_config(
    node: &TradeFlowNode,
) -> Result<AvgReboundPairlockRescueConfig> {
    let mut config = avg_rebound_default_config();
    if let Some(value) = avg_rebound_config_decimal(node, "sessionBudgetUsdc")? {
        config.session_budget_usdc = value;
    }
    if let Some(value) = avg_rebound_config_decimal(node, "reservedBudgetBufferUsdc")? {
        config.reserved_budget_buffer_usdc = value;
    }
    if let Some(value) = avg_rebound_config_decimal(node, "extraVwapSafetyBuffer")? {
        config.extra_vwap_safety_buffer = value;
    }
    if let Some(value) = avg_rebound_config_decimal(node, "targetProfitUsdc")? {
        anyhow::ensure!(
            value >= rust_decimal::Decimal::ZERO,
            "targetProfitUsdc must be >= 0"
        );
        config.target_profit_usdc = Some(value);
    }
    config.allow_primary_after_partial_profit = avg_rebound_value_bool(
        avg_rebound_config_value(node, "allowPrimaryAfterPartialProfit"),
        config.allow_primary_after_partial_profit,
    );
    config.pre_full_giveback_guard_enabled = avg_rebound_value_bool(
        avg_rebound_config_value(node, "preFullGivebackGuardEnabled"),
        config.pre_full_giveback_guard_enabled,
    );
    config.full_giveback_guard_enabled = avg_rebound_value_bool(
        avg_rebound_config_value(node, "fullGivebackGuardEnabled"),
        config.full_giveback_guard_enabled,
    );
    config.primary_outcome_label = avg_rebound_value_string(
        avg_rebound_config_value(node, "primaryOutcomeLabel"),
        &config.primary_outcome_label,
    );
    config.opposite_outcome_label = avg_rebound_value_string(
        avg_rebound_config_value(node, "oppositeOutcomeLabel"),
        &config.opposite_outcome_label,
    );
    config.primary_side_selection = avg_rebound_value_string(
        avg_rebound_config_value(node, "primarySideSelection"),
        &config.primary_side_selection,
    )
    .to_ascii_lowercase();
    config.order_type = avg_rebound_value_string(
        avg_rebound_config_value(node, "orderType"),
        &config.order_type,
    )
    .to_ascii_uppercase();
    config.execution_mode = avg_rebound_value_string(
        avg_rebound_config_value(node, "executionMode"),
        &config.execution_mode,
    )
    .to_ascii_lowercase();
    config.vwap_source = avg_rebound_value_string(
        avg_rebound_config_value(node, "vwapSource"),
        &config.vwap_source,
    )
    .to_ascii_lowercase();

    if let Some(value) = avg_rebound_config_value(node, "primaryLadder").and_then(Value::as_array) {
        config.primary_ladder = value
            .iter()
            .map(avg_rebound_parse_primary_tier)
            .collect::<Result<Vec<_>>>()?;
    }
    if let Some(value) = avg_rebound_config_value(node, "stages").and_then(Value::as_array) {
        config.stages = value
            .iter()
            .map(avg_rebound_parse_stage)
            .collect::<Result<Vec<_>>>()?;
    }
    if let Some(rescue) = avg_rebound_config_value(node, "rescue").and_then(Value::as_object) {
        config.rescue.enabled_only_after_full_ladder = avg_rebound_value_bool(
            rescue.get("enabledOnlyAfterFullLadder"),
            config.rescue.enabled_only_after_full_ladder,
        );
        if let Some(value) = rescue.get("normalVwapCap") {
            config.rescue.normal_vwap_cap = avg_rebound_value_decimal(value)?;
        }
        if let Some(value) = rescue.get("emergencyVwapCap") {
            config.rescue.emergency_vwap_cap = avg_rebound_value_decimal(value)?;
        }
        if let Some(value) = rescue.get("hardMaxVwapCap") {
            config.rescue.hard_max_vwap_cap = avg_rebound_value_decimal(value)?;
        }
        if let Some(value) = rescue.get("lastChanceVwapCap") {
            config.rescue.last_chance_vwap_cap = Some(avg_rebound_value_decimal(value)?);
        }
    }

    anyhow::ensure!(
        config.order_type == "FOK",
        "avg_rebound_pairlock_rescue_v1 supports only orderType=FOK in v1"
    );
    anyhow::ensure!(
        config.execution_mode == "limit",
        "avg_rebound_pairlock_rescue_v1 supports only executionMode=limit"
    );
    anyhow::ensure!(
        config.vwap_source == "rest_book",
        "avg_rebound_pairlock_rescue_v1 supports only vwapSource=rest_book"
    );
    let primary_outcome_is_auto = config
        .primary_outcome_label
        .trim()
        .eq_ignore_ascii_case("auto");
    if primary_outcome_is_auto {
        anyhow::ensure!(
            config.primary_side_selection == AVG_REBOUND_PRIMARY_SIDE_SELECTION_CHEAPEST_ELIGIBLE,
            "avg_rebound_pairlock_rescue_v1 primaryOutcomeLabel=auto requires primarySideSelection=cheapest_eligible"
        );
    } else {
        let _ = avg_rebound_normalized_side(&config.primary_outcome_label)?;
    }
    anyhow::ensure!(
        config.primary_side_selection == AVG_REBOUND_PRIMARY_SIDE_SELECTION_CHEAPEST_ELIGIBLE,
        "avg_rebound_pairlock_rescue_v1 primarySideSelection must be cheapest_eligible"
    );
    anyhow::ensure!(
        config.session_budget_usdc > rust_decimal::Decimal::ZERO,
        "sessionBudgetUsdc must be > 0"
    );
    anyhow::ensure!(
        config.reserved_budget_buffer_usdc >= rust_decimal::Decimal::ZERO
            && config.reserved_budget_buffer_usdc < config.session_budget_usdc,
        "reservedBudgetBufferUsdc must be >= 0 and less than sessionBudgetUsdc"
    );
    anyhow::ensure!(
        !config.primary_ladder.is_empty(),
        "primaryLadder must not be empty"
    );
    anyhow::ensure!(!config.stages.is_empty(), "stages must not be empty");
    Ok(config)
}

fn action_place_order_uses_avg_rebound_pairlock_rescue(node: &TradeFlowNode) -> bool {
    action_place_order_mode(node) == ACTION_PLACE_ORDER_MODE_AVG_REBOUND_PAIRLOCK_RESCUE_V1
}
