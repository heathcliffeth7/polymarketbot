const ACTION_PLACE_ORDER_MODE_REVENGE_FLIP_V1: &str = "revenge_flip_v1";
const REVENGE_FLIP_BINDING_MODE: &str = "revenge_flip_only";
const REVENGE_FLIP_ORDER_MARKER_KEY: &str = "revengeFlipOrder";
const REVENGE_FLIP_ROOT_NODE_KEY: &str = "revengeFlipRootNodeKey";
const REVENGE_FLIP_SIDE_KEY: &str = "revengeFlipSide";
const REVENGE_FLIP_INTENT_KEY: &str = "revengeFlipIntent";
const REVENGE_FLIP_STOP_LOSS_ENABLED_KEY: &str = "revengeFlipClassicStopLossEnabled";
const REVENGE_FLIP_STOP_LOSS_PCT_KEY: &str = "revengeFlipStopLossPct";
const REVENGE_FLIP_RUNTIME_INTENT_KEY: &str = "revenge_flip_intent";
const REVENGE_FLIP_POSITION_EPSILON: f64 = 0.0001;
const REVENGE_FLIP_DUST_CLOSE_QTY: f64 = 0.2;

fn revenge_flip_position_is_dust(position_qty: f64) -> bool {
    position_qty > REVENGE_FLIP_POSITION_EPSILON && position_qty < REVENGE_FLIP_DUST_CLOSE_QTY
}

fn revenge_flip_position_is_open(position_qty: f64) -> bool {
    position_qty > REVENGE_FLIP_DUST_CLOSE_QTY
}

fn revenge_flip_position_blocks_reentry(state: &TradeBuilderRevengeFlipState) -> bool {
    revenge_flip_position_is_open(state.position_qty)
}

fn augment_runtime_snapshot_with_revenge_flip_intent(
    runtime_snapshot_json: &mut Value,
    node: &TradeFlowNode,
) {
    if !node_config_bool(node, REVENGE_FLIP_ORDER_MARKER_KEY).unwrap_or(false) {
        return;
    }
    let Some(intent) = node_config_string(node, REVENGE_FLIP_INTENT_KEY) else {
        return;
    };
    let intent = intent.trim();
    if intent.is_empty() {
        return;
    }
    let mut snapshot = if runtime_snapshot_json.is_object() {
        runtime_snapshot_json.clone()
    } else {
        json!({})
    };
    if let Some(object) = snapshot.as_object_mut() {
        object.insert(REVENGE_FLIP_RUNTIME_INTENT_KEY.to_string(), json!(intent));
        *runtime_snapshot_json = Value::Object(object.clone());
    }
}

#[derive(Debug, Clone, PartialEq)]
struct RevengeFlipConfig {
    initial_order_usdc: f64,
    profit_target_usdc: f64,
    classic_stop_loss_enabled: bool,
    stop_loss_pct: f64,
    token_stop_loss_enabled: bool,
    token_stop_loss_pct: f64,
    stop_loss_rules: Vec<RevengeFlipStopLossRule>,
    entry_ptb_rules: Vec<RevengeFlipEntryPtbRule>,
    reentry_side_mode: String,
    max_flip: i64,
    min_reentry_shares: f64,
    lot_limit_pct: f64,
    close_only_sec: i64,
    order_type: String,
    trigger_price: RevengeFlipTriggerPriceConfig,
    time_rules: Vec<RevengeFlipTimeRule>,
    ptb: RevengeFlipPtbConfig,
    ptb_stop_loss: RevengeFlipPtbStopLossConfig,
}

#[derive(Debug, Clone, PartialEq)]
struct RevengeFlipTriggerPriceConfig {
    enabled: bool,
    min_cent: f64,
    max_cent: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct RevengeFlipTimeRule {
    min_remaining_sec: i64,
    max_remaining_sec: i64,
    price_to_beat_max_diff: Option<f64>,
    price_to_beat_unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct RevengeFlipStopLossRule {
    min_flip: i64,
    max_flip: Option<i64>,
    stop_loss_pct: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct RevengeFlipEntryPtbRule {
    min_flip: i64,
    max_flip: Option<i64>,
    min_remaining_sec: Option<i64>,
    max_remaining_sec: Option<i64>,
    side_mode: String,
    price_to_beat_max_diff: f64,
    price_to_beat_unit: Option<String>,
    max_price_cent: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
struct RevengeFlipPtbConfig {
    enabled: bool,
    mode: String,
    max_diff: f64,
    unit: String,
    current_price_source: String,
    stop_loss_bump_enabled: bool,
    stop_loss_bump_amount: f64,
    stop_loss_bump_unit: String,
    stop_loss_bump_max: Option<f64>,
    stop_loss_bump_max_unit: String,
}

#[derive(Debug, Clone, PartialEq)]
struct RevengeFlipPtbStopLossConfig {
    enabled: bool,
    gap_usd: Option<f64>,
    gap_unit: String,
    current_price_source: String,
    time_decay_mode: String,
}

#[derive(Debug, Clone, PartialEq)]
struct RevengeFlipEffectivePtb {
    enabled: bool,
    mode: String,
    max_diff: f64,
    unit: String,
    current_price_source: String,
    base_source: String,
    matched_entry_rule: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
struct RevengeFlipEffectiveEntryPrice {
    max_cent: Option<f64>,
    max_source: String,
}

fn action_place_order_uses_revenge_flip(node: &TradeFlowNode) -> bool {
    action_place_order_mode(node) == ACTION_PLACE_ORDER_MODE_REVENGE_FLIP_V1
}

fn revenge_flip_object(node: &TradeFlowNode) -> Option<&serde_json::Map<String, Value>> {
    node.config.get("revengeFlip").and_then(Value::as_object)
}

fn revenge_flip_nested_value<'a>(
    node: &'a TradeFlowNode,
    object_key: &str,
    key: &str,
) -> Option<&'a Value> {
    node.config
        .get(object_key)
        .and_then(Value::as_object)
        .and_then(|object| object.get(key))
}

fn revenge_flip_value<'a>(
    node: &'a TradeFlowNode,
    nested_key: &str,
    top_level_keys: &[&str],
) -> Option<&'a Value> {
    revenge_flip_object(node)
        .and_then(|object| object.get(nested_key))
        .or_else(|| top_level_keys.iter().find_map(|key| node.config.get(*key)))
}

fn revenge_flip_value_as_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(*value),
        Value::Number(value) => value.as_i64().map(|number| number != 0),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn revenge_flip_f64(
    node: &TradeFlowNode,
    nested_key: &str,
    top_level_keys: &[&str],
) -> Option<f64> {
    revenge_flip_value(node, nested_key, top_level_keys).and_then(value_as_f64)
}

fn revenge_flip_i64(
    node: &TradeFlowNode,
    nested_key: &str,
    top_level_keys: &[&str],
) -> Option<i64> {
    revenge_flip_value(node, nested_key, top_level_keys).and_then(value_as_i64)
}

fn revenge_flip_bool(
    node: &TradeFlowNode,
    nested_key: &str,
    top_level_keys: &[&str],
) -> Option<bool> {
    revenge_flip_value(node, nested_key, top_level_keys).and_then(revenge_flip_value_as_bool)
}

fn revenge_flip_string(
    node: &TradeFlowNode,
    nested_key: &str,
    top_level_keys: &[&str],
) -> Option<String> {
    revenge_flip_value(node, nested_key, top_level_keys)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn revenge_flip_unit(value: Option<String>, default_unit: &str) -> String {
    match value
        .unwrap_or_else(|| default_unit.to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "cent" | "cents" => "cent".to_string(),
        _ => "usd".to_string(),
    }
}

fn revenge_flip_reentry_side_mode(value: Option<String>) -> Result<String> {
    match value
        .unwrap_or_else(|| "opposite".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "opposite" => Ok("opposite".to_string()),
        "rule_match" | "rule-match" | "rules" => Ok("rule_match".to_string()),
        _ => anyhow::bail!("revenge_flip_v1 reentrySideMode must be opposite or rule_match"),
    }
}

fn revenge_flip_entry_side_mode(value: Option<&Value>) -> Result<String> {
    match value
        .and_then(Value::as_str)
        .unwrap_or("any")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "any" => Ok("any".to_string()),
        "same" => Ok("same".to_string()),
        "opposite" => Ok("opposite".to_string()),
        "up" => Ok("up".to_string()),
        "down" => Ok("down".to_string()),
        _ => anyhow::bail!(
            "revenge_flip_v1 entryPtbRules sideMode must be any, same, opposite, up, or down"
        ),
    }
}

fn revenge_flip_ptb_stop_loss_current_price_source(value: Option<String>) -> Result<String> {
    match value
        .unwrap_or_else(|| "chainlink".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "chainlink" => Ok("chainlink".to_string()),
        "binance" => Ok("binance".to_string()),
        "coinbase" => Ok("coinbase".to_string()),
        "hyperliquid" => Ok("hyperliquid".to_string()),
        "binance_hyperliquid" => Ok("binance_hyperliquid".to_string()),
        "cex_consensus" => Ok("cex_consensus".to_string()),
        _ => anyhow::bail!(
            "revenge_flip_v1 ptbStopLossCurrentPriceSource must be chainlink, binance, coinbase, hyperliquid, binance_hyperliquid, or cex_consensus"
        ),
    }
}

fn revenge_flip_ptb_stop_loss_gap_unit(value: Option<String>) -> Result<String> {
    match value
        .unwrap_or_else(|| "usd".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "usd" => Ok("usd".to_string()),
        "cent" | "cents" => Ok("cent".to_string()),
        _ => anyhow::bail!("revenge_flip_v1 ptbStopLossGapUnit must be usd or cent"),
    }
}

fn revenge_flip_ptb_stop_loss_time_decay_mode(value: Option<String>) -> Result<String> {
    match value
        .unwrap_or_else(|| "tighten".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "tighten" => Ok("tighten".to_string()),
        "relax" => Ok("relax".to_string()),
        "none" => Ok("none".to_string()),
        _ => anyhow::bail!(
            "revenge_flip_v1 ptbStopLossTimeDecayMode must be tighten, relax, or none"
        ),
    }
}

fn revenge_flip_value_to_usd(value: f64, unit: &str) -> f64 {
    if unit == "cent" {
        value / 100.0
    } else {
        value
    }
}

fn revenge_flip_usd_to_unit(value: f64, unit: &str) -> f64 {
    if unit == "cent" {
        value * 100.0
    } else {
        value
    }
}

fn revenge_flip_time_rule_from_value(value: &Value) -> Option<RevengeFlipTimeRule> {
    let object = value.as_object()?;
    let min_remaining_sec = object
        .get("minRemainingSec")
        .or_else(|| object.get("remainingSecMin"))
        .or_else(|| object.get("fromRemainingSec"))
        .and_then(value_as_i64)
        .unwrap_or(0)
        .max(0);
    let max_remaining_sec = object
        .get("maxRemainingSec")
        .or_else(|| object.get("remainingSecMax"))
        .or_else(|| object.get("toRemainingSec"))
        .and_then(value_as_i64)
        .unwrap_or(i64::MAX)
        .max(min_remaining_sec);
    let price_to_beat_max_diff = object
        .get("priceToBeatMinDiff")
        .or_else(|| object.get("ptbMinDiff"))
        .or_else(|| object.get("priceToBeatMaxDiff"))
        .or_else(|| object.get("ptbMaxDiff"))
        .and_then(value_as_f64)
        .filter(|value| value.is_finite() && *value >= 0.0);
    let price_to_beat_unit = object
        .get("priceToBeatMinDiffUnit")
        .or_else(|| object.get("priceToBeatMaxDiffUnit"))
        .or_else(|| object.get("ptbDiffUnit"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .map(|unit| revenge_flip_unit(Some(unit), "usd"));
    Some(RevengeFlipTimeRule {
        min_remaining_sec,
        max_remaining_sec,
        price_to_beat_max_diff,
        price_to_beat_unit,
    })
}

fn revenge_flip_time_rules(node: &TradeFlowNode) -> Vec<RevengeFlipTimeRule> {
    let raw = revenge_flip_object(node)
        .and_then(|object| object.get("timeRules"))
        .or_else(|| node.config.get("timeRules"));
    raw.and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(revenge_flip_time_rule_from_value)
                .collect()
        })
        .unwrap_or_default()
}

fn revenge_flip_stop_loss_rules(node: &TradeFlowNode) -> Result<Vec<RevengeFlipStopLossRule>> {
    let raw = revenge_flip_object(node)
        .and_then(|object| object.get("stopLossRules"))
        .or_else(|| node.config.get("stopLossRules"));
    let Some(rows) = raw.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut rules = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(object) = row.as_object() else {
            anyhow::bail!("revenge_flip_v1 stopLossRules entries must be objects");
        };
        let min_flip = object.get("minFlip").and_then(value_as_i64).unwrap_or(0);
        let max_flip = object.get("maxFlip").and_then(value_as_i64);
        let stop_loss_pct = object
            .get("stopLossPct")
            .and_then(value_as_f64)
            .ok_or_else(|| {
                anyhow::anyhow!("revenge_flip_v1 stopLossRules stopLossPct is required")
            })?;
        anyhow::ensure!(
            min_flip >= 0 && max_flip.map_or(true, |max| max >= min_flip),
            "revenge_flip_v1 stopLossRules minFlip/maxFlip must be non-negative ranges"
        );
        anyhow::ensure!(
            stop_loss_pct.is_finite() && stop_loss_pct > 0.0 && stop_loss_pct < 1.0,
            "revenge_flip_v1 stopLossRules stopLossPct must be between 0 and 1"
        );
        rules.push(RevengeFlipStopLossRule {
            min_flip,
            max_flip,
            stop_loss_pct,
        });
    }
    Ok(rules)
}

fn revenge_flip_optional_i64(
    object: &serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<i64> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(value_as_i64))
}

fn revenge_flip_entry_ptb_rules(node: &TradeFlowNode) -> Result<Vec<RevengeFlipEntryPtbRule>> {
    let raw = revenge_flip_object(node)
        .and_then(|object| object.get("entryPtbRules"))
        .or_else(|| node.config.get("entryPtbRules"));
    let Some(rows) = raw.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut rules = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(object) = row.as_object() else {
            anyhow::bail!("revenge_flip_v1 entryPtbRules entries must be objects");
        };
        let min_flip = object.get("minFlip").and_then(value_as_i64).unwrap_or(0);
        let max_flip = object.get("maxFlip").and_then(value_as_i64);
        let min_remaining_sec = revenge_flip_optional_i64(
            object,
            &["minRemainingSec", "remainingSecMin", "fromRemainingSec"],
        );
        let max_remaining_sec = revenge_flip_optional_i64(
            object,
            &["maxRemainingSec", "remainingSecMax", "toRemainingSec"],
        );
        let side_mode = revenge_flip_entry_side_mode(
            object
                .get("sideMode")
                .or_else(|| object.get("entrySideMode")),
        )?;
        let price_to_beat_max_diff = object
            .get("priceToBeatMinDiff")
            .or_else(|| object.get("ptbMinDiff"))
            .or_else(|| object.get("priceToBeatMaxDiff"))
            .or_else(|| object.get("ptbMaxDiff"))
            .and_then(value_as_f64)
            .ok_or_else(|| {
                anyhow::anyhow!("revenge_flip_v1 entryPtbRules priceToBeatMinDiff is required")
            })?;
        let price_to_beat_unit = object
            .get("priceToBeatMinDiffUnit")
            .or_else(|| object.get("priceToBeatMaxDiffUnit"))
            .or_else(|| object.get("ptbDiffUnit"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .map(|unit| revenge_flip_unit(Some(unit), "usd"));
        let max_price_cent = object
            .get("maxPriceCent")
            .or_else(|| object.get("entryMaxPriceCent"))
            .and_then(value_as_f64);
        anyhow::ensure!(
            min_flip >= 0 && max_flip.map_or(true, |max| max >= min_flip),
            "revenge_flip_v1 entryPtbRules minFlip/maxFlip must be non-negative ranges"
        );
        anyhow::ensure!(
            min_remaining_sec.map_or(true, |value| value >= 0)
                && max_remaining_sec.map_or(true, |value| value >= min_remaining_sec.unwrap_or(0)),
            "revenge_flip_v1 entryPtbRules remaining seconds must be non-negative ranges"
        );
        anyhow::ensure!(
            price_to_beat_max_diff.is_finite() && price_to_beat_max_diff >= 0.0,
            "revenge_flip_v1 entryPtbRules priceToBeatMinDiff must be >= 0"
        );
        anyhow::ensure!(
            max_price_cent.map_or(true, |value| value.is_finite()
                && value > 0.0
                && value <= 100.0),
            "revenge_flip_v1 entryPtbRules maxPriceCent must be > 0 and <= 100"
        );
        rules.push(RevengeFlipEntryPtbRule {
            min_flip,
            max_flip,
            min_remaining_sec,
            max_remaining_sec,
            side_mode,
            price_to_beat_max_diff,
            price_to_beat_unit,
            max_price_cent,
        });
    }
    Ok(rules)
}

fn revenge_flip_matching_time_rule<'a>(
    config: &'a RevengeFlipConfig,
    remaining_sec: Option<i64>,
) -> Option<&'a RevengeFlipTimeRule> {
    let remaining_sec = remaining_sec?;
    config.time_rules.iter().find(|rule| {
        remaining_sec >= rule.min_remaining_sec && remaining_sec <= rule.max_remaining_sec
    })
}

fn revenge_flip_opposite_side(side: &str) -> Option<&'static str> {
    match side.trim().to_ascii_lowercase().as_str() {
        "up" => Some("down"),
        "down" => Some("up"),
        _ => None,
    }
}

fn revenge_flip_entry_side_mode_matches(
    side_mode: &str,
    entry_side: Option<&str>,
    last_stopped_side: Option<&str>,
) -> bool {
    let entry_side = entry_side.map(|side| side.trim().to_ascii_lowercase());
    let last_stopped_side = last_stopped_side.map(|side| side.trim().to_ascii_lowercase());
    match side_mode {
        "any" => true,
        "up" | "down" => entry_side.as_deref() == Some(side_mode),
        "same" => entry_side.is_some() && entry_side == last_stopped_side,
        "opposite" => {
            let Some(stopped_side) = last_stopped_side.as_deref() else {
                return false;
            };
            entry_side.as_deref() == revenge_flip_opposite_side(stopped_side)
        }
        _ => false,
    }
}

fn revenge_flip_entry_ptb_rule_matches(
    rule: &RevengeFlipEntryPtbRule,
    entry_flip_index: i64,
    remaining_sec: Option<i64>,
    entry_side: Option<&str>,
    last_stopped_side: Option<&str>,
) -> bool {
    if entry_flip_index < rule.min_flip
        || rule
            .max_flip
            .map_or(false, |max_flip| entry_flip_index > max_flip)
    {
        return false;
    }
    if rule.min_remaining_sec.is_none() && rule.max_remaining_sec.is_none() {
        return revenge_flip_entry_side_mode_matches(
            &rule.side_mode,
            entry_side,
            last_stopped_side,
        );
    }
    let Some(remaining_sec) = remaining_sec else {
        return false;
    };
    if remaining_sec < rule.min_remaining_sec.unwrap_or(0)
        || rule
            .max_remaining_sec
            .map_or(false, |max| remaining_sec > max)
    {
        return false;
    }
    revenge_flip_entry_side_mode_matches(&rule.side_mode, entry_side, last_stopped_side)
}

fn revenge_flip_matching_entry_ptb_rule<'a>(
    config: &'a RevengeFlipConfig,
    entry_flip_index: i64,
    remaining_sec: Option<i64>,
    entry_side: Option<&str>,
    last_stopped_side: Option<&str>,
) -> Option<&'a RevengeFlipEntryPtbRule> {
    config.entry_ptb_rules.iter().find(|rule| {
        revenge_flip_entry_ptb_rule_matches(
            rule,
            entry_flip_index,
            remaining_sec,
            entry_side,
            last_stopped_side,
        )
    })
}

fn revenge_flip_entry_ptb_rule_json(rule: &RevengeFlipEntryPtbRule) -> Value {
    json!({
        "min_flip": rule.min_flip,
        "max_flip": rule.max_flip,
        "min_remaining_sec": rule.min_remaining_sec,
        "max_remaining_sec": rule.max_remaining_sec,
        "side_mode": rule.side_mode,
        "price_to_beat_max_diff": rule.price_to_beat_max_diff,
        "unit": rule.price_to_beat_unit,
        "max_price_cent": rule.max_price_cent,
    })
}

fn resolve_revenge_flip_config(node: &TradeFlowNode) -> Result<RevengeFlipConfig> {
    let initial_order_usdc =
        revenge_flip_f64(node, "initialOrderUsdc", &["initialOrderUsdc"]).unwrap_or(5.0);
    let profit_target_usdc =
        revenge_flip_f64(node, "profitTargetUsdc", &["profitTargetUsdc"]).unwrap_or(0.25);
    let classic_stop_loss_enabled =
        revenge_flip_bool(node, "classicStopLossEnabled", &["classicStopLossEnabled"])
            .unwrap_or(true);
    let stop_loss_pct = revenge_flip_f64(node, "stopLossPct", &["stopLossPct"]).unwrap_or(0.20);
    let token_stop_loss_enabled =
        revenge_flip_bool(node, "tokenStopLossEnabled", &["tokenStopLossEnabled"])
            .unwrap_or(false);
    let token_stop_loss_pct =
        revenge_flip_f64(node, "tokenStopLossPct", &["tokenStopLossPct"]).unwrap_or(0.15);
    let max_flip = revenge_flip_i64(node, "maxFlip", &["maxFlip"])
        .unwrap_or(0)
        .max(0);
    let min_reentry_shares =
        revenge_flip_f64(node, "minReentryShares", &["minReentryShares"]).unwrap_or(0.0);
    let lot_limit_pct = revenge_flip_f64(node, "lotLimitPct", &["lotLimitPct"]).unwrap_or(0.98);
    let close_only_sec = revenge_flip_i64(node, "closeOnlySec", &["closeOnlySec"])
        .unwrap_or(10)
        .max(0);
    let order_type = node_config_string(node, "orderType").unwrap_or_else(|| "FAK".to_string());
    let reentry_side_mode = revenge_flip_reentry_side_mode(revenge_flip_string(
        node,
        "reentrySideMode",
        &["reentrySideMode"],
    ))?;

    anyhow::ensure!(
        initial_order_usdc.is_finite() && initial_order_usdc > 0.0,
        "revenge_flip_v1 initialOrderUsdc must be > 0"
    );
    anyhow::ensure!(
        profit_target_usdc.is_finite() && profit_target_usdc >= 0.0,
        "revenge_flip_v1 profitTargetUsdc must be >= 0"
    );
    anyhow::ensure!(
        min_reentry_shares.is_finite() && min_reentry_shares >= 0.0,
        "revenge_flip_v1 minReentryShares must be >= 0"
    );
    if classic_stop_loss_enabled {
        anyhow::ensure!(
            stop_loss_pct.is_finite() && stop_loss_pct > 0.0 && stop_loss_pct < 1.0,
            "revenge_flip_v1 stopLossPct must be between 0 and 1"
        );
    }
    if token_stop_loss_enabled {
        anyhow::ensure!(
            token_stop_loss_pct.is_finite()
                && token_stop_loss_pct > 0.0
                && token_stop_loss_pct < 1.0,
            "revenge_flip_v1 tokenStopLossPct must be between 0 and 1"
        );
    }
    anyhow::ensure!(
        lot_limit_pct.is_finite() && lot_limit_pct > 0.0 && lot_limit_pct <= 1.0,
        "revenge_flip_v1 lotLimitPct must be > 0 and <= 1"
    );

    let trigger_price = RevengeFlipTriggerPriceConfig {
        enabled: revenge_flip_nested_value(node, "triggerPrice", "enabled")
            .and_then(revenge_flip_value_as_bool)
            .or_else(|| revenge_flip_bool(node, "triggerPriceEnabled", &["triggerPriceEnabled"]))
            .unwrap_or(false),
        min_cent: revenge_flip_nested_value(node, "triggerPrice", "minCent")
            .and_then(value_as_f64)
            .or_else(|| revenge_flip_f64(node, "triggerPriceMinCent", &["triggerPriceMinCent"]))
            .unwrap_or(0.0),
        max_cent: revenge_flip_nested_value(node, "triggerPrice", "maxCent")
            .and_then(value_as_f64)
            .or_else(|| revenge_flip_f64(node, "triggerPriceMaxCent", &["triggerPriceMaxCent"]))
            .unwrap_or(100.0),
    };
    anyhow::ensure!(
        trigger_price.min_cent >= 0.0
            && trigger_price.max_cent <= 100.0
            && trigger_price.min_cent <= trigger_price.max_cent,
        "revenge_flip_v1 triggerPrice range must be 0..100 cent"
    );

    let ptb = RevengeFlipPtbConfig {
        enabled: node_config_bool(node, "priceToBeatGuardEnabled")
            .or_else(|| node_config_bool(node, "priceToBeatGuard"))
            .unwrap_or(true),
        mode: node_config_string(node, "priceToBeatMode").unwrap_or_else(|| "manual".to_string()),
        max_diff: node_config_f64(node, "priceToBeatMinDiff")
            .or_else(|| node_config_f64(node, "priceToBeatMaxDiff"))
            .unwrap_or(0.01)
            .max(0.0),
        unit: revenge_flip_unit(
            node_config_string(node, "priceToBeatMinDiffUnit")
                .or_else(|| node_config_string(node, "priceToBeatMaxDiffUnit")),
            "usd",
        ),
        current_price_source: node_config_string(node, "priceToBeatCurrentPriceSource")
            .unwrap_or_else(|| "chainlink".to_string()),
        stop_loss_bump_enabled: revenge_flip_bool(
            node,
            "ptbStopLossBumpEnabled",
            &["priceToBeatStopLossBumpEnabled", "ptbStopLossBumpEnabled"],
        )
        .unwrap_or(false),
        stop_loss_bump_amount: revenge_flip_f64(
            node,
            "ptbStopLossBumpAmount",
            &["priceToBeatStopLossBumpAmount", "ptbStopLossBumpAmount"],
        )
        .unwrap_or(0.0)
        .max(0.0),
        stop_loss_bump_unit: revenge_flip_unit(
            revenge_flip_string(
                node,
                "ptbStopLossBumpUnit",
                &["priceToBeatStopLossBumpUnit", "ptbStopLossBumpUnit"],
            ),
            "usd",
        ),
        stop_loss_bump_max: revenge_flip_f64(
            node,
            "ptbStopLossBumpMax",
            &["priceToBeatStopLossBumpMax", "ptbStopLossBumpMax"],
        )
        .filter(|value| value.is_finite() && *value >= 0.0),
        stop_loss_bump_max_unit: revenge_flip_unit(
            revenge_flip_string(
                node,
                "ptbStopLossBumpMaxUnit",
                &["priceToBeatStopLossBumpMaxUnit", "ptbStopLossBumpMaxUnit"],
            ),
            "usd",
        ),
    };
    let ptb_stop_loss_enabled =
        revenge_flip_bool(node, "ptbStopLossEnabled", &["ptbStopLossEnabled"]).unwrap_or(false);
    let ptb_stop_loss_gap_unit = revenge_flip_ptb_stop_loss_gap_unit(revenge_flip_string(
        node,
        "ptbStopLossGapUnit",
        &["ptbStopLossGapUnit"],
    ))?;
    let ptb_stop_loss_gap_usd = revenge_flip_f64(node, "ptbStopLossGapUsd", &["ptbStopLossGapUsd"])
        .map(|value| revenge_flip_value_to_usd(value, &ptb_stop_loss_gap_unit));
    anyhow::ensure!(
        ptb_stop_loss_gap_usd.map_or(true, f64::is_finite),
        "revenge_flip_v1 ptbStopLossGapUsd must be finite"
    );
    anyhow::ensure!(
        !ptb_stop_loss_enabled || ptb_stop_loss_gap_usd.is_some(),
        "revenge_flip_v1 ptbStopLossGapUsd must be set when ptbStopLossEnabled=true"
    );
    anyhow::ensure!(
        classic_stop_loss_enabled || ptb_stop_loss_enabled,
        "revenge_flip_v1 classicStopLossEnabled=false requires ptbStopLossEnabled=true"
    );
    let ptb_stop_loss_time_decay_mode =
        revenge_flip_ptb_stop_loss_time_decay_mode(revenge_flip_string(
            node,
            "ptbStopLossTimeDecayMode",
            &["ptbStopLossTimeDecayMode"],
        ))?;
    let ptb_stop_loss_current_price_source = revenge_flip_ptb_stop_loss_current_price_source(
        revenge_flip_string(
            node,
            "ptbStopLossCurrentPriceSource",
            &["ptbStopLossCurrentPriceSource"],
        )
        .or_else(|| node_config_string(node, "priceToBeatCurrentPriceSource")),
    )?;

    Ok(RevengeFlipConfig {
        initial_order_usdc,
        profit_target_usdc,
        classic_stop_loss_enabled,
        stop_loss_pct,
        token_stop_loss_enabled,
        token_stop_loss_pct,
        stop_loss_rules: if classic_stop_loss_enabled {
            revenge_flip_stop_loss_rules(node)?
        } else {
            Vec::new()
        },
        entry_ptb_rules: revenge_flip_entry_ptb_rules(node)?,
        reentry_side_mode,
        max_flip,
        min_reentry_shares,
        lot_limit_pct,
        close_only_sec,
        order_type,
        trigger_price,
        time_rules: revenge_flip_time_rules(node),
        ptb,
        ptb_stop_loss: RevengeFlipPtbStopLossConfig {
            enabled: ptb_stop_loss_enabled,
            gap_usd: ptb_stop_loss_gap_usd,
            gap_unit: ptb_stop_loss_gap_unit,
            current_price_source: ptb_stop_loss_current_price_source,
            time_decay_mode: ptb_stop_loss_time_decay_mode,
        },
    })
}

fn revenge_flip_ptb_bump_usd(
    config: &RevengeFlipConfig,
    state: &TradeBuilderRevengeFlipState,
) -> f64 {
    if config.ptb.stop_loss_bump_enabled && config.ptb.stop_loss_bump_amount > 0.0 {
        let raw = revenge_flip_value_to_usd(
            config.ptb.stop_loss_bump_amount * state.ptb_bump_count.max(0) as f64,
            &config.ptb.stop_loss_bump_unit,
        );
        config
            .ptb
            .stop_loss_bump_max
            .map(|max| {
                raw.min(revenge_flip_value_to_usd(
                    max,
                    &config.ptb.stop_loss_bump_max_unit,
                ))
            })
            .unwrap_or(raw)
    } else {
        0.0
    }
}

fn revenge_flip_effective_ptb_from_entry_rule(
    config: &RevengeFlipConfig,
    state: &TradeBuilderRevengeFlipState,
    rule: &RevengeFlipEntryPtbRule,
) -> RevengeFlipEffectivePtb {
    let unit = rule
        .price_to_beat_unit
        .clone()
        .unwrap_or_else(|| config.ptb.unit.clone());
    let base_usd = revenge_flip_value_to_usd(rule.price_to_beat_max_diff, &unit);
    let bump_usd = revenge_flip_ptb_bump_usd(config, state);
    RevengeFlipEffectivePtb {
        enabled: config.ptb.enabled,
        mode: config.ptb.mode.clone(),
        max_diff: revenge_flip_usd_to_unit(base_usd + bump_usd, &unit),
        unit,
        current_price_source: config.ptb.current_price_source.clone(),
        base_source: "entry_ptb_rule".to_string(),
        matched_entry_rule: Some(revenge_flip_entry_ptb_rule_json(rule)),
    }
}

fn revenge_flip_effective_ptb(
    config: &RevengeFlipConfig,
    state: &TradeBuilderRevengeFlipState,
    entry_flip_index: i64,
    remaining_sec: Option<i64>,
    entry_side: Option<&str>,
    last_stopped_side: Option<&str>,
) -> RevengeFlipEffectivePtb {
    let matched_entry_rule = revenge_flip_matching_entry_ptb_rule(
        config,
        entry_flip_index,
        remaining_sec,
        entry_side,
        last_stopped_side,
    );
    let matched_time_rule = if matched_entry_rule.is_none() {
        revenge_flip_matching_time_rule(config, remaining_sec)
    } else {
        None
    };
    let unit = matched_entry_rule
        .and_then(|rule| rule.price_to_beat_unit.clone())
        .or_else(|| matched_time_rule.and_then(|rule| rule.price_to_beat_unit.clone()))
        .unwrap_or_else(|| config.ptb.unit.clone());
    let base = matched_entry_rule
        .map(|rule| rule.price_to_beat_max_diff)
        .or_else(|| matched_time_rule.and_then(|rule| rule.price_to_beat_max_diff))
        .unwrap_or(config.ptb.max_diff);
    let base_usd = revenge_flip_value_to_usd(base, &unit);
    let bump_usd = revenge_flip_ptb_bump_usd(config, state);
    RevengeFlipEffectivePtb {
        enabled: config.ptb.enabled,
        mode: config.ptb.mode.clone(),
        max_diff: revenge_flip_usd_to_unit(base_usd + bump_usd, &unit),
        unit,
        current_price_source: config.ptb.current_price_source.clone(),
        base_source: if matched_entry_rule.is_some() {
            "entry_ptb_rule".to_string()
        } else if matched_time_rule.is_some() {
            "time_rule".to_string()
        } else {
            "global".to_string()
        },
        matched_entry_rule: matched_entry_rule.map(revenge_flip_entry_ptb_rule_json),
    }
}

fn revenge_flip_effective_entry_price_from_entry_rule(
    config: &RevengeFlipConfig,
    rule: &RevengeFlipEntryPtbRule,
) -> RevengeFlipEffectiveEntryPrice {
    if let Some(max_cent) = rule.max_price_cent {
        return RevengeFlipEffectiveEntryPrice {
            max_cent: Some(max_cent),
            max_source: "entry_ptb_rule".to_string(),
        };
    }
    if config.trigger_price.enabled {
        return RevengeFlipEffectiveEntryPrice {
            max_cent: Some(config.trigger_price.max_cent),
            max_source: "trigger_price".to_string(),
        };
    }
    RevengeFlipEffectiveEntryPrice {
        max_cent: None,
        max_source: "none".to_string(),
    }
}

fn revenge_flip_effective_entry_price(
    config: &RevengeFlipConfig,
    entry_flip_index: i64,
    remaining_sec: Option<i64>,
    entry_side: Option<&str>,
    last_stopped_side: Option<&str>,
) -> RevengeFlipEffectiveEntryPrice {
    if let Some(max_cent) = revenge_flip_matching_entry_ptb_rule(
        config,
        entry_flip_index,
        remaining_sec,
        entry_side,
        last_stopped_side,
    )
    .and_then(|rule| rule.max_price_cent)
    {
        return RevengeFlipEffectiveEntryPrice {
            max_cent: Some(max_cent),
            max_source: "entry_ptb_rule".to_string(),
        };
    }
    if config.trigger_price.enabled {
        return RevengeFlipEffectiveEntryPrice {
            max_cent: Some(config.trigger_price.max_cent),
            max_source: "trigger_price".to_string(),
        };
    }
    RevengeFlipEffectiveEntryPrice {
        max_cent: None,
        max_source: "none".to_string(),
    }
}

fn revenge_flip_entry_price_passes(
    config: &RevengeFlipConfig,
    effective_entry_price: &RevengeFlipEffectiveEntryPrice,
    ask_price: f64,
) -> bool {
    let ask_cent = ask_price * 100.0;
    let min_passes = !config.trigger_price.enabled || ask_cent >= config.trigger_price.min_cent;
    let max_passes = effective_entry_price
        .max_cent
        .map_or(true, |max_cent| ask_cent <= max_cent);
    min_passes && max_passes
}

fn revenge_flip_stop_loss_triggered(state: &TradeBuilderRevengeFlipState, best_bid: f64) -> bool {
    state.position_stop_loss_enabled
        && best_bid <= state.position_avg_cost * (1.0 - revenge_flip_position_stop_loss_pct(state))
}

fn revenge_flip_entry_flip_index(state: &TradeBuilderRevengeFlipState) -> i64 {
    state.flip_count.max(0)
}

fn revenge_flip_stop_loss_pct_for_entry(config: &RevengeFlipConfig, entry_flip_index: i64) -> f64 {
    config
        .stop_loss_rules
        .iter()
        .find(|rule| {
            entry_flip_index >= rule.min_flip
                && rule
                    .max_flip
                    .map_or(true, |max_flip| entry_flip_index <= max_flip)
        })
        .map(|rule| rule.stop_loss_pct)
        .unwrap_or(config.stop_loss_pct)
}

fn revenge_flip_position_stop_loss_pct(state: &TradeBuilderRevengeFlipState) -> f64 {
    if state.position_stop_loss_pct.is_finite()
        && state.position_stop_loss_pct > 0.0
        && state.position_stop_loss_pct < 1.0
    {
        state.position_stop_loss_pct
    } else {
        0.20
    }
}

fn revenge_flip_max_flip_allows(
    config: &RevengeFlipConfig,
    state: &TradeBuilderRevengeFlipState,
) -> bool {
    config.max_flip == 0 || state.flip_count < config.max_flip
}
