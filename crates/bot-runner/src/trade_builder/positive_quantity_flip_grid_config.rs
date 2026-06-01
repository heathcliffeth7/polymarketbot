#[derive(Debug, Clone)]
struct PositiveQuantityFlipGridConfig {
    base_buy_usdc: f64,
    min_marketable_buy_usdc: f64,
    entry_band_min_cent: f64,
    entry_band_max_cent: f64,
    preferred_trigger_cent: f64,
    trigger_tolerance_cent: f64,
    exit_price_for_sizing: f64,
    sizing_price_buffer_cent: f64,
    partial_recovery_enabled: bool,
    partial_recovery_min_loss_reduction_usdc: f64,
    partial_recovery_balance_reserve_usdc: f64,
    partial_recovery_max_buy_usdc: Option<f64>,
    partial_recovery_ignore_market_budget: bool,
    quantity_sizing_mode: PositiveQuantityFlipGridQuantitySizingMode,
    inventory_balance_lead_qty: f64,
    min_positive_profit_usdc: f64,
    min_sell_net_profit_usdc: f64,
    max_single_buy_usdc: Option<f64>,
    max_total_spent_per_market_usdc: Option<f64>,
    max_active_markets: i64,
    max_open_grid_buys_per_market: i64,
    sell_bid_min: f64,
    hard_max_price_cent: f64,
    worst_price_cent: f64,
    rescue_buy_enabled: bool,
    rescue_buy_min_price_cent: f64,
    rescue_buy_max_price_cent: f64,
    block_consecutive_same_side_buys: bool,
    no_buy_ranges: Vec<PositiveQuantityFlipGridNoBuyRange>,
    cycle_window_mode: Option<String>,
    cycle_window_secs: Option<i64>,
    cycle_window_start_sec: Option<i64>,
    cycle_window_end_sec: Option<i64>,
    new_grid_buy_start_remaining_sec: i64,
    new_grid_buy_end_remaining_sec: i64,
    positive_completion_buy_end_remaining_sec: i64,
    no_new_buy_under_sec: i64,
    order_type: &'static str,
    pairlock_compression_enabled: bool,
    stop_buys_after_pairlock_merge: bool,
    target_pairlock_profit: f64,
    fee_buffer: f64,
    max_pair_cost: f64,
    pairlock_order_type: &'static str,
    max_unmerged_exposure_usdc: f64,
    min_basket_profit_usdc: f64,
    min_direct_profit_usdc: f64,
    basket_exit_enabled: bool,
    direct_exit_enabled: bool,
    execution_floor_guard_enabled: bool,
    execution_floor_price_cent: Option<f64>,
    trigger_price_guard_enabled: bool,
    ptb_guard_enabled: bool,
    ptb_min_diff: f64,
    ptb_rescue_min_diff: Option<f64>,
    ptb_diff_unit: crate::trade_flow::guards::price_to_beat::PriceToBeatDiffUnit,
    ptb_current_price_source:
        crate::trade_flow::guards::price_to_beat::PriceToBeatCurrentPriceSource,
    depth_guard_enabled: bool,
}

#[derive(Debug, Clone)]
struct PositiveQuantityFlipGridNoBuyRange {
    min_cent: f64,
    max_cent: f64,
}

fn positive_quantity_flip_grid_config_map(
    node: &TradeFlowNode,
) -> Result<serde_json::Map<String, Value>> {
    match node.config.get(POSITIVE_QUANTITY_FLIP_GRID_CONFIG_KEY) {
        Some(Value::Object(map)) => Ok(map.clone()),
        Some(Value::String(raw)) if !raw.trim().is_empty() => {
            let parsed: Value = serde_json::from_str(raw).map_err(|err| {
                anyhow::anyhow!("positiveQuantityFlipGrid must be a JSON object: {err}")
            })?;
            parsed
                .as_object()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("positiveQuantityFlipGrid must be a JSON object"))
        }
        Some(Value::Null) | None => Ok(serde_json::Map::new()),
        Some(_) => Err(anyhow::anyhow!(
            "positiveQuantityFlipGrid must be a JSON object"
        )),
    }
}

fn positive_quantity_flip_grid_value<'a>(
    map: &'a serde_json::Map<String, Value>,
    node: &'a TradeFlowNode,
    key: &str,
) -> Option<&'a Value> {
    map.get(key).or_else(|| node.config.get(key))
}

fn positive_quantity_flip_grid_f64(
    map: &serde_json::Map<String, Value>,
    node: &TradeFlowNode,
    key: &str,
    default: f64,
) -> f64 {
    positive_quantity_flip_grid_value(map, node, key)
        .and_then(value_as_f64)
        .unwrap_or(default)
}

fn positive_quantity_flip_grid_i64(
    map: &serde_json::Map<String, Value>,
    node: &TradeFlowNode,
    key: &str,
    default: i64,
) -> i64 {
    positive_quantity_flip_grid_value(map, node, key)
        .and_then(value_as_i64)
        .unwrap_or(default)
}

fn positive_quantity_flip_grid_bool(
    map: &serde_json::Map<String, Value>,
    node: &TradeFlowNode,
    key: &str,
    default: bool,
) -> bool {
    match positive_quantity_flip_grid_value(map, node, key) {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => true,
            "false" | "0" | "no" | "off" => false,
            _ => default,
        },
        _ => default,
    }
}

fn positive_quantity_flip_grid_string(
    map: &serde_json::Map<String, Value>,
    node: &TradeFlowNode,
    key: &str,
) -> Option<String> {
    positive_quantity_flip_grid_value(map, node, key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn positive_quantity_flip_grid_optional_i64(
    map: &serde_json::Map<String, Value>,
    node: &TradeFlowNode,
    key: &str,
) -> Option<i64> {
    positive_quantity_flip_grid_value(map, node, key).and_then(value_as_i64)
}

fn positive_quantity_flip_grid_no_buy_ranges(
    map: &serde_json::Map<String, Value>,
    node: &TradeFlowNode,
) -> Result<Vec<PositiveQuantityFlipGridNoBuyRange>> {
    let Some(raw) = positive_quantity_flip_grid_value(map, node, "noBuyRanges") else {
        return Ok(Vec::new());
    };
    if raw.is_null() {
        return Ok(Vec::new());
    }
    let Value::Array(items) = raw else {
        return Err(anyhow::anyhow!(
            "positiveQuantityFlipGrid noBuyRanges must be an array"
        ));
    };
    let mut ranges = Vec::with_capacity(items.len());
    for item in items {
        let Value::Object(range) = item else {
            return Err(anyhow::anyhow!(
                "positiveQuantityFlipGrid noBuyRanges entries must be objects"
            ));
        };
        let min_cent = range.get("minCent").and_then(value_as_f64).ok_or_else(|| {
            anyhow::anyhow!("positiveQuantityFlipGrid noBuyRanges entries require minCent")
        })?;
        let max_cent = range.get("maxCent").and_then(value_as_f64).ok_or_else(|| {
            anyhow::anyhow!("positiveQuantityFlipGrid noBuyRanges entries require maxCent")
        })?;
        anyhow::ensure!(
            min_cent > 0.0 && min_cent < max_cent && max_cent <= 100.0,
            "positiveQuantityFlipGrid noBuyRanges entries must satisfy 0 < minCent < maxCent <= 100"
        );
        ranges.push(PositiveQuantityFlipGridNoBuyRange { min_cent, max_cent });
    }
    Ok(ranges)
}

fn resolve_positive_quantity_flip_grid_config(
    node: &TradeFlowNode,
) -> Result<PositiveQuantityFlipGridConfig> {
    let map = positive_quantity_flip_grid_config_map(node)?;
    let pairlock_compression_mode = action_place_order_positive_grid_mode(node)
        == Some(ACTION_PLACE_ORDER_MODE_POSITIVE_FLIP_PAIRLOCK_COMPRESSION_V1);
    let base_buy_usdc = positive_quantity_flip_grid_f64(
        &map,
        node,
        "baseBuyUsdc",
        if pairlock_compression_mode { 2.0 } else { 1.0 },
    );
    let min_marketable_buy_usdc = positive_quantity_flip_grid_f64(
        &map,
        node,
        "minMarketableBuyUsdc",
        POSITIVE_QUANTITY_FLIP_GRID_MIN_MARKETABLE_BUY_USDC,
    );
    let entry_band_min_cent = positive_quantity_flip_grid_f64(
        &map,
        node,
        "entryBandMinCent",
        if pairlock_compression_mode {
            52.0
        } else {
            50.0
        },
    );
    let entry_band_max_cent = positive_quantity_flip_grid_f64(
        &map,
        node,
        "entryBandMaxCent",
        if pairlock_compression_mode {
            58.0
        } else {
            60.0
        },
    );
    let preferred_trigger_cent = positive_quantity_flip_grid_f64(
        &map,
        node,
        "preferredTriggerCent",
        if pairlock_compression_mode {
            55.0
        } else {
            53.0
        },
    );
    let trigger_tolerance_cent =
        positive_quantity_flip_grid_f64(&map, node, "triggerToleranceCent", 3.0);
    let exit_price_for_sizing =
        positive_quantity_flip_grid_f64(&map, node, "exitPriceForSizingCent", 98.0) / 100.0;
    let sizing_price_buffer_cent = positive_quantity_flip_grid_f64(
        &map,
        node,
        "sizingPriceBufferCent",
        if pairlock_compression_mode {
            1.0
        } else {
            POSITIVE_QUANTITY_FLIP_GRID_DEFAULT_SIZING_BUFFER_CENT
        },
    );
    let partial_recovery_enabled =
        positive_quantity_flip_grid_bool(&map, node, "partialRecoveryEnabled", false);
    let partial_recovery_min_loss_reduction_usdc = positive_quantity_flip_grid_f64(
        &map,
        node,
        "partialRecoveryMinLossReductionUsdc",
        POSITIVE_QUANTITY_FLIP_GRID_DEFAULT_PARTIAL_MIN_LOSS_REDUCTION_USDC,
    );
    let partial_recovery_balance_reserve_usdc = positive_quantity_flip_grid_f64(
        &map,
        node,
        "partialRecoveryBalanceReserveUsdc",
        POSITIVE_QUANTITY_FLIP_GRID_DEFAULT_PARTIAL_BALANCE_RESERVE_USDC,
    );
    let partial_recovery_max_buy_usdc =
        positive_quantity_flip_grid_optional_f64(&map, node, "partialRecoveryMaxBuyUsdc");
    let partial_recovery_ignore_market_budget = positive_quantity_flip_grid_bool(
        &map,
        node,
        "partialRecoveryIgnoreMarketBudget",
        partial_recovery_enabled,
    );
    let quantity_sizing_mode = positive_quantity_flip_grid_parse_quantity_sizing_mode(&map, node)?;
    if quantity_sizing_mode == PositiveQuantityFlipGridQuantitySizingMode::FixedUsdc
        && !pairlock_compression_mode
    {
        return Err(anyhow::anyhow!(
            "positiveQuantityFlipGrid quantitySizingMode=fixed_usdc requires action.place_order mode=positive_flip_pairlock_compression_v1"
        ));
    }
    let inventory_balance_lead_qty =
        positive_quantity_flip_grid_f64(&map, node, "inventoryBalanceLeadQty", 0.0);
    let min_positive_profit_usdc = positive_quantity_flip_grid_f64(
        &map,
        node,
        "minPositiveProfitUsdc",
        if pairlock_compression_mode {
            0.05
        } else {
            POSITIVE_QUANTITY_FLIP_GRID_DEFAULT_PROFIT_TARGET_USDC
        },
    );
    let min_sell_net_profit_usdc = positive_quantity_flip_grid_f64(
        &map,
        node,
        "minSellNetProfitUsdc",
        if pairlock_compression_mode {
            0.05
        } else {
            POSITIVE_QUANTITY_FLIP_GRID_DEFAULT_PROFIT_TARGET_USDC
        },
    );
    let max_single_buy_usdc =
        positive_quantity_flip_grid_optional_f64(&map, node, "maxSingleBuyUsdc").or(
            if pairlock_compression_mode {
                None
            } else {
                Some(2.2)
            },
        );
    let max_total_spent_per_market_usdc =
        positive_quantity_flip_grid_optional_f64(&map, node, "maxTotalSpentPerMarketUsdc").or(
            if pairlock_compression_mode {
                None
            } else {
                Some(9.5)
            },
        );
    let max_active_markets =
        positive_quantity_flip_grid_i64(&map, node, "maxActiveMarkets", 1).max(1);
    let max_open_grid_buys_per_market = positive_quantity_flip_grid_i64(
        &map,
        node,
        "maxOpenGridBuysPerMarket",
        if pairlock_compression_mode { 10 } else { 8 },
    )
    .max(1);
    let sell_bid_min = positive_quantity_flip_grid_f64(
        &map,
        node,
        "sellBidMinCent",
        if pairlock_compression_mode {
            59.0
        } else {
            98.0
        },
    ) / 100.0;
    let hard_max_price_cent =
        positive_quantity_flip_grid_f64(&map, node, "hardMaxPriceCent", entry_band_max_cent);
    let worst_price_cent =
        positive_quantity_flip_grid_f64(&map, node, "worstPriceCent", hard_max_price_cent);
    let rescue_buy_enabled =
        positive_quantity_flip_grid_bool(&map, node, "rescueBuyEnabled", false);
    let rescue_buy_min_price_cent =
        positive_quantity_flip_grid_f64(&map, node, "rescueBuyMinPriceCent", hard_max_price_cent);
    let rescue_buy_max_price_cent =
        positive_quantity_flip_grid_f64(&map, node, "rescueBuyMaxPriceCent", 70.0);
    let block_consecutive_same_side_buys =
        positive_quantity_flip_grid_bool(&map, node, "blockConsecutiveSameSideBuys", true);
    let no_buy_ranges = positive_quantity_flip_grid_no_buy_ranges(&map, node)?;
    let cycle_window_mode = positive_quantity_flip_grid_string(&map, node, "cycleWindowMode")
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let cycle_window_secs = positive_quantity_flip_grid_optional_i64(&map, node, "cycleWindowSecs");
    let cycle_window_start_sec =
        positive_quantity_flip_grid_optional_i64(&map, node, "cycleWindowStartSec");
    let cycle_window_end_sec =
        positive_quantity_flip_grid_optional_i64(&map, node, "cycleWindowEndSec");
    let new_grid_buy_start_remaining_sec =
        positive_quantity_flip_grid_i64(&map, node, "newGridBuyStartRemainingSec", 285);
    let new_grid_buy_end_remaining_sec =
        positive_quantity_flip_grid_i64(&map, node, "newGridBuyEndRemainingSec", 90);
    let positive_completion_buy_end_remaining_sec =
        positive_quantity_flip_grid_i64(&map, node, "positiveCompletionBuyEndRemainingSec", 30);
    let no_new_buy_under_sec = positive_quantity_flip_grid_i64(&map, node, "noNewBuyUnderSec", 30);
    let order_type = positive_quantity_flip_grid_string(&map, node, "orderType")
        .and_then(|value| normalize_trade_builder_clob_order_type(&value))
        .unwrap_or("FAK");
    let pairlock_compression_enabled = positive_quantity_flip_grid_bool(
        &map,
        node,
        "pairlockCompressionEnabled",
        pairlock_compression_mode,
    );
    let stop_buys_after_pairlock_merge = positive_quantity_flip_grid_bool(
        &map,
        node,
        "stopBuysAfterPairlockMerge",
        pairlock_compression_mode,
    );
    let target_pairlock_profit =
        positive_quantity_flip_grid_f64(&map, node, "targetPairlockProfitCent", 5.0) / 100.0;
    let fee_buffer = positive_quantity_flip_grid_f64(&map, node, "feeBufferCent", 1.0) / 100.0;
    let max_pair_cost =
        positive_quantity_flip_grid_f64(&map, node, "maxPairCostCent", 94.0) / 100.0;
    let pairlock_order_type = positive_quantity_flip_grid_string(&map, node, "pairlockOrderType")
        .and_then(|value| normalize_trade_builder_clob_order_type(&value))
        .unwrap_or("FOK");
    let max_unmerged_exposure_usdc =
        positive_quantity_flip_grid_f64(&map, node, "maxUnmergedExposureUsdc", 2.0);
    let min_basket_profit_usdc =
        positive_quantity_flip_grid_f64(&map, node, "minBasketProfitUsdc", 0.06);
    let min_direct_profit_usdc =
        positive_quantity_flip_grid_f64(&map, node, "minDirectProfitUsdc", 0.05);
    let basket_exit_enabled =
        positive_quantity_flip_grid_bool(&map, node, "basketExitEnabled", false);
    let direct_exit_enabled = positive_quantity_flip_grid_bool(
        &map,
        node,
        "directExitEnabled",
        !pairlock_compression_mode,
    );
    let execution_floor_guard_enabled =
        positive_quantity_flip_grid_bool(&map, node, "executionFloorGuardEnabled", true);
    let execution_floor_price_cent =
        positive_quantity_flip_grid_optional_f64(&map, node, "executionFloorPriceCent");
    let trigger_price_guard_enabled =
        positive_quantity_flip_grid_bool(&map, node, "triggerPriceGuardEnabled", false);
    let ptb_guard_enabled = positive_quantity_flip_grid_bool(&map, node, "ptbGuardEnabled", false);
    let ptb_min_diff = positive_quantity_flip_grid_f64(&map, node, "ptbMinDiff", 2.0);
    let ptb_rescue_min_diff =
        positive_quantity_flip_grid_optional_f64(&map, node, "ptbRescueMinDiff");
    let ptb_diff_unit =
        positive_quantity_flip_grid_parse_ptb_diff_unit(&map, node, ptb_guard_enabled)?;
    let ptb_current_price_source =
        positive_quantity_flip_grid_parse_ptb_current_price_source(&map, node, ptb_guard_enabled)?;

    anyhow::ensure!(
        base_buy_usdc > 0.0,
        "positiveQuantityFlipGrid baseBuyUsdc must be > 0"
    );
    anyhow::ensure!(
        min_marketable_buy_usdc >= 1.0 && min_marketable_buy_usdc <= 100.0,
        "positiveQuantityFlipGrid minMarketableBuyUsdc must be between 1 and 100"
    );
    anyhow::ensure!(
        entry_band_min_cent > 0.0
            && entry_band_min_cent < entry_band_max_cent
            && entry_band_max_cent <= 100.0,
        "positiveQuantityFlipGrid entry band must be in (0, 100]"
    );
    anyhow::ensure!(
        hard_max_price_cent <= entry_band_max_cent && hard_max_price_cent <= 100.0,
        "positiveQuantityFlipGrid hardMaxPriceCent must be <= entryBandMaxCent and <= 100"
    );
    anyhow::ensure!(
        worst_price_cent >= hard_max_price_cent && worst_price_cent <= 100.0,
        "positiveQuantityFlipGrid worstPriceCent must be >= hardMaxPriceCent and <= 100"
    );
    if rescue_buy_enabled {
        anyhow::ensure!(
            rescue_buy_min_price_cent >= entry_band_max_cent
                && rescue_buy_min_price_cent < rescue_buy_max_price_cent
                && rescue_buy_max_price_cent < exit_price_for_sizing * 100.0,
            "positiveQuantityFlipGrid rescue range must satisfy entryBandMaxCent <= rescueBuyMinPriceCent < rescueBuyMaxPriceCent < exitPriceForSizingCent"
        );
    }
    anyhow::ensure!(
        exit_price_for_sizing > entry_band_max_cent / 100.0 && exit_price_for_sizing <= 1.0,
        "positiveQuantityFlipGrid exitPriceForSizingCent must be above entryBandMaxCent"
    );
    anyhow::ensure!(
        min_positive_profit_usdc > 0.0 && min_sell_net_profit_usdc > 0.0,
        "positiveQuantityFlipGrid profit targets must be > 0"
    );
    anyhow::ensure!(
        sizing_price_buffer_cent >= 0.0 && sizing_price_buffer_cent <= 5.0,
        "positiveQuantityFlipGrid sizingPriceBufferCent must be between 0 and 5"
    );
    anyhow::ensure!(
        partial_recovery_min_loss_reduction_usdc >= 0.0,
        "positiveQuantityFlipGrid partialRecoveryMinLossReductionUsdc must be >= 0"
    );
    anyhow::ensure!(
        partial_recovery_balance_reserve_usdc >= 0.0,
        "positiveQuantityFlipGrid partialRecoveryBalanceReserveUsdc must be >= 0"
    );
    if let Some(max_buy_usdc) = partial_recovery_max_buy_usdc {
        anyhow::ensure!(
            max_buy_usdc > 0.0,
            "positiveQuantityFlipGrid partialRecoveryMaxBuyUsdc must be > 0 when set"
        );
    }
    anyhow::ensure!(
        inventory_balance_lead_qty >= 0.0 && inventory_balance_lead_qty <= 1000.0,
        "positiveQuantityFlipGrid inventoryBalanceLeadQty must be between 0 and 1000"
    );
    if let Some(max_single_buy_usdc) = max_single_buy_usdc {
        anyhow::ensure!(
            max_single_buy_usdc >= base_buy_usdc,
            "positiveQuantityFlipGrid maxSingleBuyUsdc must be >= baseBuyUsdc"
        );
    }
    if let Some(max_total_spent_per_market_usdc) = max_total_spent_per_market_usdc {
        anyhow::ensure!(
            max_total_spent_per_market_usdc >= base_buy_usdc,
            "positiveQuantityFlipGrid maxTotalSpentPerMarketUsdc must be >= baseBuyUsdc"
        );
    }
    anyhow::ensure!(
        new_grid_buy_end_remaining_sec > positive_completion_buy_end_remaining_sec
            && positive_completion_buy_end_remaining_sec >= no_new_buy_under_sec
            && no_new_buy_under_sec >= 0,
        "positiveQuantityFlipGrid timing must satisfy gridEnd > completionEnd >= noNewBuy"
    );
    if cycle_window_mode.is_none() {
        anyhow::ensure!(
            new_grid_buy_start_remaining_sec > new_grid_buy_end_remaining_sec,
            "positiveQuantityFlipGrid timing must satisfy start > gridEnd"
        );
    }
    if let Some(mode) = cycle_window_mode.as_deref() {
        anyhow::ensure!(
            mode == "off" || mode == "first" || mode == "last" || mode == "custom_range",
            "positiveQuantityFlipGrid cycleWindowMode must be off, first, last, or custom_range"
        );
        if mode == "first" || mode == "last" {
            anyhow::ensure!(
                cycle_window_secs.is_some_and(|value| value > 0),
                "positiveQuantityFlipGrid cycleWindowSecs must be > 0"
            );
        }
        if mode == "custom_range" {
            let start_sec = cycle_window_start_sec.ok_or_else(|| {
                anyhow::anyhow!("positiveQuantityFlipGrid cycleWindowStartSec is required")
            })?;
            let end_sec = cycle_window_end_sec.ok_or_else(|| {
                anyhow::anyhow!("positiveQuantityFlipGrid cycleWindowEndSec is required")
            })?;
            anyhow::ensure!(
                start_sec >= 0 && start_sec < end_sec,
                "positiveQuantityFlipGrid custom_range requires 0 <= start < end"
            );
        }
    }
    anyhow::ensure!(
        order_type == "FAK",
        "positiveQuantityFlipGrid orderType must be FAK or IOC"
    );
    anyhow::ensure!(
        target_pairlock_profit >= 0.0 && target_pairlock_profit < 1.0,
        "positiveQuantityFlipGrid targetPairlockProfitCent must be >= 0 and < 100"
    );
    anyhow::ensure!(
        fee_buffer >= 0.0 && fee_buffer <= 0.10,
        "positiveQuantityFlipGrid feeBufferCent must be between 0 and 10"
    );
    anyhow::ensure!(
        max_pair_cost > 0.0 && max_pair_cost < 1.0,
        "positiveQuantityFlipGrid maxPairCostCent must be > 0 and < 100"
    );
    anyhow::ensure!(
        pairlock_order_type == "FOK" || pairlock_order_type == "FAK",
        "positiveQuantityFlipGrid pairlockOrderType must be FOK, FAK, or IOC"
    );
    anyhow::ensure!(
        max_unmerged_exposure_usdc >= 0.0
            && min_basket_profit_usdc >= 0.0
            && min_direct_profit_usdc >= 0.0,
        "positiveQuantityFlipGrid pairlock exposure and profit fields must be >= 0"
    );
    if execution_floor_guard_enabled {
        let floor_cent = execution_floor_price_cent.unwrap_or(entry_band_min_cent);
        anyhow::ensure!(
            floor_cent > 0.0 && floor_cent <= hard_max_price_cent,
            "positiveQuantityFlipGrid executionFloorPriceCent must be > 0 and <= hardMaxPriceCent"
        );
    }
    if ptb_guard_enabled {
        anyhow::ensure!(
            ptb_min_diff.is_finite() && ptb_min_diff > 0.0,
            "positiveQuantityFlipGrid ptbMinDiff must be > 0 when ptbGuardEnabled=true"
        );
        if let Some(rescue_min_diff) = ptb_rescue_min_diff {
            anyhow::ensure!(
                rescue_min_diff.is_finite() && rescue_min_diff > 0.0,
                "positiveQuantityFlipGrid ptbRescueMinDiff must be > 0 when set"
            );
        }
    }

    Ok(PositiveQuantityFlipGridConfig {
        base_buy_usdc,
        min_marketable_buy_usdc,
        entry_band_min_cent,
        entry_band_max_cent,
        preferred_trigger_cent,
        trigger_tolerance_cent,
        exit_price_for_sizing,
        sizing_price_buffer_cent,
        partial_recovery_enabled,
        partial_recovery_min_loss_reduction_usdc,
        partial_recovery_balance_reserve_usdc,
        partial_recovery_max_buy_usdc,
        partial_recovery_ignore_market_budget,
        quantity_sizing_mode,
        inventory_balance_lead_qty,
        min_positive_profit_usdc,
        min_sell_net_profit_usdc,
        max_single_buy_usdc,
        max_total_spent_per_market_usdc,
        max_active_markets,
        max_open_grid_buys_per_market,
        sell_bid_min,
        hard_max_price_cent,
        worst_price_cent,
        rescue_buy_enabled,
        rescue_buy_min_price_cent,
        rescue_buy_max_price_cent,
        block_consecutive_same_side_buys,
        no_buy_ranges,
        cycle_window_mode,
        cycle_window_secs,
        cycle_window_start_sec,
        cycle_window_end_sec,
        new_grid_buy_start_remaining_sec,
        new_grid_buy_end_remaining_sec,
        positive_completion_buy_end_remaining_sec,
        no_new_buy_under_sec,
        order_type,
        pairlock_compression_enabled,
        stop_buys_after_pairlock_merge,
        target_pairlock_profit,
        fee_buffer,
        max_pair_cost,
        pairlock_order_type,
        max_unmerged_exposure_usdc,
        min_basket_profit_usdc,
        min_direct_profit_usdc,
        basket_exit_enabled,
        direct_exit_enabled,
        execution_floor_guard_enabled,
        execution_floor_price_cent,
        trigger_price_guard_enabled,
        ptb_guard_enabled,
        ptb_min_diff,
        ptb_rescue_min_diff,
        ptb_diff_unit,
        ptb_current_price_source,
        depth_guard_enabled: positive_quantity_flip_grid_bool(
            &map,
            node,
            "depthGuardEnabled",
            true,
        ),
    })
}
