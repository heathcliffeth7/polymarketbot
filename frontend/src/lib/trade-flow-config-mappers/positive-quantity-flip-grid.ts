import { isRecord, safeJsonStringify, toStringValue } from "./utils";

export const POSITIVE_QUANTITY_FLIP_GRID_MODE =
  "positive_quantity_flip_grid_v1";
export const POSITIVE_FLIP_PAIRLOCK_COMPRESSION_MODE =
  "positive_flip_pairlock_compression_v1";
export const POSITIVE_GRID_CYCLE_WINDOW_MODE_FIELD =
  "positiveGridCycleWindowMode";
export const POSITIVE_GRID_CYCLE_WINDOW_SECS_FIELD =
  "positiveGridCycleWindowSecs";
export const POSITIVE_GRID_CYCLE_WINDOW_START_SEC_FIELD =
  "positiveGridCycleWindowStartSec";
export const POSITIVE_GRID_CYCLE_WINDOW_END_SEC_FIELD =
  "positiveGridCycleWindowEndSec";
export const POSITIVE_GRID_NORMAL_BUY_MIN_FIELD =
  "positiveGridNormalBuyMinCent";
export const POSITIVE_GRID_NORMAL_BUY_MAX_FIELD =
  "positiveGridNormalBuyMaxCent";
export const POSITIVE_GRID_BASE_BUY_USDC_FIELD = "positiveGridBaseBuyUsdc";
export const POSITIVE_GRID_MIN_MARKETABLE_BUY_USDC_FIELD =
  "positiveGridMinMarketableBuyUsdc";
export const POSITIVE_GRID_MAX_BUY_PRICE_FIELD =
  POSITIVE_GRID_NORMAL_BUY_MAX_FIELD;
export const POSITIVE_GRID_PROFIT_TARGET_FIELD = "positiveGridProfitTargetUsdc";
export const POSITIVE_GRID_TAKE_PROFIT_SELL_BID_FIELD =
  "positiveGridTakeProfitSellBidCent";
export const POSITIVE_GRID_BASKET_EXIT_ENABLED_FIELD =
  "positiveGridBasketExitEnabled";
export const POSITIVE_GRID_STOP_BUYS_AFTER_PAIRLOCK_MERGE_FIELD =
  "positiveGridStopBuysAfterPairlockMerge";
export const POSITIVE_GRID_DIRECT_EXIT_ENABLED_FIELD =
  "positiveGridDirectExitEnabled";
export const POSITIVE_GRID_SIZING_PRICE_BUFFER_FIELD =
  "positiveGridSizingPriceBufferCent";
export const POSITIVE_GRID_MAX_SINGLE_BUY_FIELD =
  "positiveGridMaxSingleBuyUsdc";
export const POSITIVE_GRID_MAX_TOTAL_SPENT_FIELD =
  "positiveGridMaxTotalSpentPerMarketUsdc";
export const POSITIVE_GRID_MAX_OPEN_BUYS_FIELD =
  "positiveGridMaxOpenBuysPerMarket";
export const POSITIVE_GRID_PARTIAL_RECOVERY_ENABLED_FIELD =
  "positiveGridPartialRecoveryEnabled";
export const POSITIVE_GRID_PARTIAL_RECOVERY_MIN_LOSS_REDUCTION_FIELD =
  "positiveGridPartialRecoveryMinLossReductionUsdc";
export const POSITIVE_GRID_PARTIAL_RECOVERY_BALANCE_RESERVE_FIELD =
  "positiveGridPartialRecoveryBalanceReserveUsdc";
export const POSITIVE_GRID_PARTIAL_RECOVERY_MAX_BUY_FIELD =
  "positiveGridPartialRecoveryMaxBuyUsdc";
export const POSITIVE_GRID_PARTIAL_RECOVERY_IGNORE_MARKET_BUDGET_FIELD =
  "positiveGridPartialRecoveryIgnoreMarketBudget";
export const POSITIVE_GRID_QUANTITY_SIZING_MODE_FIELD =
  "positiveGridQuantitySizingMode";
export const POSITIVE_GRID_INVENTORY_BALANCE_LEAD_QTY_FIELD =
  "positiveGridInventoryBalanceLeadQty";
export const POSITIVE_GRID_RESCUE_BUY_ENABLED_FIELD =
  "positiveGridRescueBuyEnabled";
export const POSITIVE_GRID_RESCUE_MIN_PRICE_FIELD =
  "positiveGridRescueMinPriceCent";
export const POSITIVE_GRID_RESCUE_MAX_PRICE_FIELD =
  "positiveGridRescueMaxPriceCent";
export const POSITIVE_GRID_BLOCK_CONSECUTIVE_SAME_SIDE_BUYS_FIELD =
  "positiveGridBlockConsecutiveSameSideBuys";
export const POSITIVE_GRID_NO_BUY_RANGES_FIELD = "positiveGridNoBuyRangesJson";
export const POSITIVE_GRID_DEPTH_GUARD_FIELD = "positiveGridDepthGuardEnabled";
export const POSITIVE_GRID_EXECUTION_FLOOR_ENABLED_FIELD =
  "positiveGridExecutionFloorGuardEnabled";
export const POSITIVE_GRID_EXECUTION_FLOOR_PRICE_FIELD =
  "positiveGridExecutionFloorPriceCent";
export const POSITIVE_GRID_TRIGGER_PRICE_GUARD_FIELD =
  "positiveGridTriggerPriceGuardEnabled";
export const POSITIVE_GRID_PTB_GUARD_FIELD = "positiveGridPtbGuardEnabled";
export const POSITIVE_GRID_PTB_MIN_DIFF_FIELD = "positiveGridPtbMinDiff";
export const POSITIVE_GRID_PTB_RESCUE_MIN_DIFF_FIELD =
  "positiveGridPtbRescueMinDiff";
export const POSITIVE_GRID_PTB_DIFF_UNIT_FIELD = "positiveGridPtbDiffUnit";
export const POSITIVE_GRID_PTB_CURRENT_SOURCE_FIELD =
  "positiveGridPtbCurrentPriceSource";

export interface PositiveQuantityFlipGridNoBuyRange {
  minCent: number;
  maxCent: number;
}

function toFiniteNumber(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return null;
}

function toIntegerOrRaw(raw: string): number | string {
  const parsed = Number(raw);
  return Number.isInteger(parsed) ? parsed : raw;
}

function toBooleanString(value: unknown, fallback: boolean): string {
  if (typeof value === "boolean") return value ? "true" : "false";
  const text = toStringValue(value).trim().toLowerCase();
  if (["true", "1", "yes", "on"].includes(text)) return "true";
  if (["false", "0", "no", "off"].includes(text)) return "false";
  return fallback ? "true" : "false";
}

function fieldBoolean(
  fields: Record<string, string>,
  key: string,
  current: unknown,
  fallback: boolean,
): boolean {
  return toBooleanString(fields[key] ?? current, fallback) === "true";
}

function normalizeCycleWindowMode(value: unknown): string {
  const mode = toStringValue(value).trim().toLowerCase();
  return mode === "first" ||
    mode === "last" ||
    mode === "custom_range" ||
    mode === "off"
    ? mode
    : "custom_range";
}

function normalizeQuantitySizingMode(value: unknown): string {
  const mode = toStringValue(value).trim().toLowerCase();
  if (mode === "inventory_balance") return "inventory_balance";
  if (mode === "fixed_usdc") return "fixed_usdc";
  return "profit_target";
}

function parsePositiveGridObject(raw: unknown): Record<string, unknown> {
  if (isRecord(raw)) return raw;
  const text = toStringValue(raw).trim();
  if (!text) return {};
  try {
    const parsed = JSON.parse(text);
    return isRecord(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

export function parsePositiveGridNoBuyRanges(
  raw: unknown,
): PositiveQuantityFlipGridNoBuyRange[] | "invalid" {
  if (raw == null || raw === "") return [];
  const parsed = typeof raw === "string" ? safeParseJson(raw) : raw;
  if (parsed == null) return "invalid";
  if (!Array.isArray(parsed)) return "invalid";
  const ranges: PositiveQuantityFlipGridNoBuyRange[] = [];
  for (const item of parsed) {
    if (!isRecord(item)) return "invalid";
    const minCent = toFiniteNumber(item.minCent);
    const maxCent = toFiniteNumber(item.maxCent);
    if (
      minCent == null ||
      maxCent == null ||
      !(minCent > 0 && minCent < maxCent && maxCent <= 100)
    ) {
      return "invalid";
    }
    ranges.push({ minCent, maxCent });
  }
  return ranges;
}

export function applyPositiveQuantityFlipGridFormDefaults(
  fields: Record<string, string>,
  config: Record<string, unknown>,
) {
  if (isRecord(config.positiveQuantityFlipGrid)) {
    fields.positiveQuantityFlipGrid = safeJsonStringify(
      config.positiveQuantityFlipGrid,
    );
  }
  const grid = parsePositiveGridObject(config.positiveQuantityFlipGrid);
  const pairlockCompressionMode =
    toStringValue(config.mode).trim().toLowerCase() ===
    POSITIVE_FLIP_PAIRLOCK_COMPRESSION_MODE;
  const hasCycleWindow = toStringValue(grid.cycleWindowMode).trim().length > 0;
  const cycleWindowMode = hasCycleWindow
    ? normalizeCycleWindowMode(grid.cycleWindowMode)
    : "custom_range";
  fields[POSITIVE_GRID_CYCLE_WINDOW_MODE_FIELD] = cycleWindowMode;
  fields[POSITIVE_GRID_CYCLE_WINDOW_SECS_FIELD] = String(
    toFiniteNumber(grid.cycleWindowSecs) ?? 120,
  );
  const legacyStartRemainingSec = toFiniteNumber(
    grid.newGridBuyStartRemainingSec,
  );
  const fallbackStartSec =
    legacyStartRemainingSec == null
      ? 0
      : Math.max(0, 300 - legacyStartRemainingSec);
  fields[POSITIVE_GRID_CYCLE_WINDOW_START_SEC_FIELD] = String(
    toFiniteNumber(grid.cycleWindowStartSec) ?? fallbackStartSec,
  );
  fields[POSITIVE_GRID_CYCLE_WINDOW_END_SEC_FIELD] = String(
    toFiniteNumber(grid.cycleWindowEndSec) ?? 300,
  );
  const normalBuyMaxCent =
    toFiniteNumber(grid.entryBandMaxCent) ??
    toFiniteNumber(grid.hardMaxPriceCent) ??
    toFiniteNumber(grid.worstPriceCent) ??
    60;
  fields[POSITIVE_GRID_NORMAL_BUY_MIN_FIELD] = String(
    toFiniteNumber(grid.entryBandMinCent) ?? 50,
  );
  fields[POSITIVE_GRID_NORMAL_BUY_MAX_FIELD] = String(normalBuyMaxCent);
  fields[POSITIVE_GRID_BASE_BUY_USDC_FIELD] = String(
    toFiniteNumber(grid.baseBuyUsdc) ?? (pairlockCompressionMode ? 2 : 1),
  );
  fields[POSITIVE_GRID_MIN_MARKETABLE_BUY_USDC_FIELD] = String(
    toFiniteNumber(grid.minMarketableBuyUsdc) ?? 1.05,
  );
  const profitTarget =
    toFiniteNumber(grid.minPositiveProfitUsdc) ??
    toFiniteNumber(grid.minSellNetProfitUsdc) ??
    1;
  fields[POSITIVE_GRID_PROFIT_TARGET_FIELD] = String(profitTarget);
  fields[POSITIVE_GRID_TAKE_PROFIT_SELL_BID_FIELD] = String(
    toFiniteNumber(grid.sellBidMinCent) ?? 98,
  );
  fields[POSITIVE_GRID_BASKET_EXIT_ENABLED_FIELD] = toBooleanString(
    grid.basketExitEnabled,
    !pairlockCompressionMode,
  );
  fields[POSITIVE_GRID_STOP_BUYS_AFTER_PAIRLOCK_MERGE_FIELD] = toBooleanString(
    grid.stopBuysAfterPairlockMerge,
    pairlockCompressionMode,
  );
  fields[POSITIVE_GRID_DIRECT_EXIT_ENABLED_FIELD] = toBooleanString(
    grid.directExitEnabled,
    !pairlockCompressionMode,
  );
  fields[POSITIVE_GRID_SIZING_PRICE_BUFFER_FIELD] = String(
    toFiniteNumber(grid.sizingPriceBufferCent) ?? 3,
  );
  fields[POSITIVE_GRID_MAX_SINGLE_BUY_FIELD] =
    toFiniteNumber(grid.maxSingleBuyUsdc) == null
      ? ""
      : String(toFiniteNumber(grid.maxSingleBuyUsdc));
  fields[POSITIVE_GRID_MAX_TOTAL_SPENT_FIELD] =
    toFiniteNumber(grid.maxTotalSpentPerMarketUsdc) == null
      ? ""
      : String(toFiniteNumber(grid.maxTotalSpentPerMarketUsdc));
  fields[POSITIVE_GRID_MAX_OPEN_BUYS_FIELD] = String(
    toFiniteNumber(grid.maxOpenGridBuysPerMarket) ??
      (pairlockCompressionMode ? 7 : 8),
  );
  fields[POSITIVE_GRID_PARTIAL_RECOVERY_ENABLED_FIELD] = toBooleanString(
    grid.partialRecoveryEnabled,
    false,
  );
  fields[POSITIVE_GRID_PARTIAL_RECOVERY_MIN_LOSS_REDUCTION_FIELD] = String(
    toFiniteNumber(grid.partialRecoveryMinLossReductionUsdc) ?? 0.1,
  );
  fields[POSITIVE_GRID_PARTIAL_RECOVERY_BALANCE_RESERVE_FIELD] = String(
    toFiniteNumber(grid.partialRecoveryBalanceReserveUsdc) ?? 1,
  );
  fields[POSITIVE_GRID_PARTIAL_RECOVERY_MAX_BUY_FIELD] =
    toFiniteNumber(grid.partialRecoveryMaxBuyUsdc) == null
      ? ""
      : String(toFiniteNumber(grid.partialRecoveryMaxBuyUsdc));
  fields[POSITIVE_GRID_PARTIAL_RECOVERY_IGNORE_MARKET_BUDGET_FIELD] =
    toBooleanString(grid.partialRecoveryIgnoreMarketBudget, true);
  fields[POSITIVE_GRID_QUANTITY_SIZING_MODE_FIELD] =
    normalizeQuantitySizingMode(grid.quantitySizingMode);
  fields[POSITIVE_GRID_INVENTORY_BALANCE_LEAD_QTY_FIELD] = String(
    toFiniteNumber(grid.inventoryBalanceLeadQty) ?? 0,
  );
  fields[POSITIVE_GRID_RESCUE_BUY_ENABLED_FIELD] = toBooleanString(
    grid.rescueBuyEnabled,
    false,
  );
  fields[POSITIVE_GRID_RESCUE_MIN_PRICE_FIELD] = String(
    toFiniteNumber(grid.rescueBuyMinPriceCent) ?? normalBuyMaxCent,
  );
  fields[POSITIVE_GRID_RESCUE_MAX_PRICE_FIELD] = String(
    toFiniteNumber(grid.rescueBuyMaxPriceCent) ?? 70,
  );
  fields[POSITIVE_GRID_BLOCK_CONSECUTIVE_SAME_SIDE_BUYS_FIELD] =
    toBooleanString(grid.blockConsecutiveSameSideBuys, true);
  const noBuyRanges = parsePositiveGridNoBuyRanges(grid.noBuyRanges);
  fields[POSITIVE_GRID_NO_BUY_RANGES_FIELD] = safeJsonStringify(
    noBuyRanges === "invalid" ? [] : noBuyRanges,
  );
  fields[POSITIVE_GRID_DEPTH_GUARD_FIELD] = toBooleanString(
    grid.depthGuardEnabled,
    true,
  );
  fields[POSITIVE_GRID_EXECUTION_FLOOR_ENABLED_FIELD] = toBooleanString(
    grid.executionFloorGuardEnabled,
    true,
  );
  fields[POSITIVE_GRID_EXECUTION_FLOOR_PRICE_FIELD] =
    toFiniteNumber(grid.executionFloorPriceCent) == null
      ? ""
      : String(toFiniteNumber(grid.executionFloorPriceCent));
  fields[POSITIVE_GRID_TRIGGER_PRICE_GUARD_FIELD] = toBooleanString(
    grid.triggerPriceGuardEnabled,
    false,
  );
  fields[POSITIVE_GRID_PTB_GUARD_FIELD] = toBooleanString(
    grid.ptbGuardEnabled,
    false,
  );
  fields[POSITIVE_GRID_PTB_MIN_DIFF_FIELD] = String(
    toFiniteNumber(grid.ptbMinDiff) ?? 2,
  );
  fields[POSITIVE_GRID_PTB_RESCUE_MIN_DIFF_FIELD] =
    toFiniteNumber(grid.ptbRescueMinDiff) == null
      ? ""
      : String(toFiniteNumber(grid.ptbRescueMinDiff));
  fields[POSITIVE_GRID_PTB_DIFF_UNIT_FIELD] =
    toStringValue(grid.ptbDiffUnit).trim() || "usd";
  fields[POSITIVE_GRID_PTB_CURRENT_SOURCE_FIELD] =
    toStringValue(grid.ptbCurrentPriceSource).trim() || "chainlink";
}

export function normalizePositiveQuantityFlipGridBuildConfig(
  config: Record<string, unknown>,
  fields: Record<string, string>,
): boolean {
  const mode = toStringValue(config.mode).trim().toLowerCase();
  const positiveGridMode =
    mode === POSITIVE_QUANTITY_FLIP_GRID_MODE ||
    mode === POSITIVE_FLIP_PAIRLOCK_COMPRESSION_MODE;
  if (!positiveGridMode) {
    delete config.positiveQuantityFlipGrid;
    return false;
  }

  config.mode = mode;
  const grid = parsePositiveGridObject(config.positiveQuantityFlipGrid);
  if (
    Object.prototype.hasOwnProperty.call(
      fields,
      POSITIVE_GRID_CYCLE_WINDOW_MODE_FIELD,
    )
  ) {
    const cycleWindowMode = normalizeCycleWindowMode(
      fields[POSITIVE_GRID_CYCLE_WINDOW_MODE_FIELD],
    );
    grid.cycleWindowMode = cycleWindowMode;
    if (cycleWindowMode === "first" || cycleWindowMode === "last") {
      const rawSecs =
        fields[POSITIVE_GRID_CYCLE_WINDOW_SECS_FIELD]?.trim() || "";
      grid.cycleWindowSecs = toIntegerOrRaw(rawSecs);
      delete grid.cycleWindowStartSec;
      delete grid.cycleWindowEndSec;
    } else if (cycleWindowMode === "custom_range") {
      const rawStart =
        fields[POSITIVE_GRID_CYCLE_WINDOW_START_SEC_FIELD]?.trim() || "";
      const rawEnd =
        fields[POSITIVE_GRID_CYCLE_WINDOW_END_SEC_FIELD]?.trim() || "";
      grid.cycleWindowStartSec = toIntegerOrRaw(rawStart);
      grid.cycleWindowEndSec = toIntegerOrRaw(rawEnd);
      delete grid.cycleWindowSecs;
    } else {
      delete grid.cycleWindowSecs;
      delete grid.cycleWindowStartSec;
      delete grid.cycleWindowEndSec;
    }
  }
  const normalMinRaw = fields[POSITIVE_GRID_NORMAL_BUY_MIN_FIELD]?.trim();
  const normalMaxRaw = fields[POSITIVE_GRID_NORMAL_BUY_MAX_FIELD]?.trim();
  const normalBuyMinCent =
    toFiniteNumber(normalMinRaw) ?? toFiniteNumber(grid.entryBandMinCent) ?? 50;
  const normalBuyMaxCent =
    toFiniteNumber(normalMaxRaw) ??
    toFiniteNumber(grid.entryBandMaxCent) ??
    toFiniteNumber(grid.hardMaxPriceCent) ??
    toFiniteNumber(grid.worstPriceCent) ??
    60;
  grid.entryBandMinCent = normalBuyMinCent;
  grid.entryBandMaxCent = normalBuyMaxCent;
  grid.hardMaxPriceCent = normalBuyMaxCent;
  grid.worstPriceCent = normalBuyMaxCent;
  grid.baseBuyUsdc =
    toFiniteNumber(fields[POSITIVE_GRID_BASE_BUY_USDC_FIELD]?.trim()) ??
    toFiniteNumber(grid.baseBuyUsdc) ??
    (config.mode === POSITIVE_FLIP_PAIRLOCK_COMPRESSION_MODE ? 2 : 1);
  grid.minMarketableBuyUsdc =
    toFiniteNumber(
      fields[POSITIVE_GRID_MIN_MARKETABLE_BUY_USDC_FIELD]?.trim(),
    ) ??
    toFiniteNumber(grid.minMarketableBuyUsdc) ??
    1.05;
  grid.sizingPriceBufferCent =
    toFiniteNumber(fields[POSITIVE_GRID_SIZING_PRICE_BUFFER_FIELD]?.trim()) ??
    toFiniteNumber(grid.sizingPriceBufferCent) ??
    3;
  if (
    Object.prototype.hasOwnProperty.call(
      fields,
      POSITIVE_GRID_MAX_SINGLE_BUY_FIELD,
    )
  ) {
    const rawMaxSingle = fields[POSITIVE_GRID_MAX_SINGLE_BUY_FIELD]?.trim() || "";
    const maxSingle = toFiniteNumber(rawMaxSingle);
    if (rawMaxSingle && maxSingle != null && maxSingle > 0) {
      grid.maxSingleBuyUsdc = maxSingle;
    } else {
      delete grid.maxSingleBuyUsdc;
    }
  }
  if (
    Object.prototype.hasOwnProperty.call(
      fields,
      POSITIVE_GRID_MAX_TOTAL_SPENT_FIELD,
    )
  ) {
    const rawMaxTotal =
      fields[POSITIVE_GRID_MAX_TOTAL_SPENT_FIELD]?.trim() || "";
    const maxTotal = toFiniteNumber(rawMaxTotal);
    if (rawMaxTotal && maxTotal != null && maxTotal > 0) {
      grid.maxTotalSpentPerMarketUsdc = maxTotal;
    } else {
      delete grid.maxTotalSpentPerMarketUsdc;
    }
  }
  if (
    Object.prototype.hasOwnProperty.call(
      fields,
      POSITIVE_GRID_MAX_OPEN_BUYS_FIELD,
    )
  ) {
    const rawMaxOpen =
      fields[POSITIVE_GRID_MAX_OPEN_BUYS_FIELD]?.trim() || "";
    const maxOpen = toFiniteNumber(rawMaxOpen);
    if (rawMaxOpen && maxOpen != null && maxOpen > 0) {
      grid.maxOpenGridBuysPerMarket = Math.trunc(maxOpen);
    }
  }
  grid.partialRecoveryEnabled = fieldBoolean(
    fields,
    POSITIVE_GRID_PARTIAL_RECOVERY_ENABLED_FIELD,
    grid.partialRecoveryEnabled,
    false,
  );
  grid.partialRecoveryMinLossReductionUsdc =
    toFiniteNumber(
      fields[POSITIVE_GRID_PARTIAL_RECOVERY_MIN_LOSS_REDUCTION_FIELD]?.trim(),
    ) ??
    toFiniteNumber(grid.partialRecoveryMinLossReductionUsdc) ??
    0.1;
  grid.partialRecoveryBalanceReserveUsdc =
    toFiniteNumber(
      fields[POSITIVE_GRID_PARTIAL_RECOVERY_BALANCE_RESERVE_FIELD]?.trim(),
    ) ??
    toFiniteNumber(grid.partialRecoveryBalanceReserveUsdc) ??
    1;
  if (
    Object.prototype.hasOwnProperty.call(
      fields,
      POSITIVE_GRID_PARTIAL_RECOVERY_MAX_BUY_FIELD,
    )
  ) {
    const rawMaxBuy =
      fields[POSITIVE_GRID_PARTIAL_RECOVERY_MAX_BUY_FIELD]?.trim() || "";
    const maxBuy = toFiniteNumber(rawMaxBuy);
    if (rawMaxBuy && maxBuy != null) {
      grid.partialRecoveryMaxBuyUsdc = maxBuy;
    } else {
      delete grid.partialRecoveryMaxBuyUsdc;
    }
  }
  grid.partialRecoveryIgnoreMarketBudget = fieldBoolean(
    fields,
    POSITIVE_GRID_PARTIAL_RECOVERY_IGNORE_MARKET_BUDGET_FIELD,
    grid.partialRecoveryIgnoreMarketBudget,
    true,
  );
  grid.quantitySizingMode = normalizeQuantitySizingMode(
    fields[POSITIVE_GRID_QUANTITY_SIZING_MODE_FIELD] ?? grid.quantitySizingMode,
  );
  grid.inventoryBalanceLeadQty =
    toFiniteNumber(
      fields[POSITIVE_GRID_INVENTORY_BALANCE_LEAD_QTY_FIELD]?.trim(),
    ) ??
    toFiniteNumber(grid.inventoryBalanceLeadQty) ??
    0;
  const profitTargetUsdc =
    toFiniteNumber(fields[POSITIVE_GRID_PROFIT_TARGET_FIELD]?.trim()) ??
    toFiniteNumber(grid.minPositiveProfitUsdc) ??
    toFiniteNumber(grid.minSellNetProfitUsdc) ??
    1;
  grid.minPositiveProfitUsdc = profitTargetUsdc;
  grid.minSellNetProfitUsdc = profitTargetUsdc;
  grid.sellBidMinCent =
    toFiniteNumber(fields[POSITIVE_GRID_TAKE_PROFIT_SELL_BID_FIELD]?.trim()) ??
    toFiniteNumber(grid.sellBidMinCent) ??
    98;
  grid.basketExitEnabled = fieldBoolean(
    fields,
    POSITIVE_GRID_BASKET_EXIT_ENABLED_FIELD,
    grid.basketExitEnabled,
    config.mode !== POSITIVE_FLIP_PAIRLOCK_COMPRESSION_MODE,
  );
  grid.stopBuysAfterPairlockMerge = fieldBoolean(
    fields,
    POSITIVE_GRID_STOP_BUYS_AFTER_PAIRLOCK_MERGE_FIELD,
    grid.stopBuysAfterPairlockMerge,
    config.mode === POSITIVE_FLIP_PAIRLOCK_COMPRESSION_MODE,
  );
  grid.directExitEnabled = fieldBoolean(
    fields,
    POSITIVE_GRID_DIRECT_EXIT_ENABLED_FIELD,
    grid.directExitEnabled,
    config.mode !== POSITIVE_FLIP_PAIRLOCK_COMPRESSION_MODE,
  );
  grid.rescueBuyEnabled = fieldBoolean(
    fields,
    POSITIVE_GRID_RESCUE_BUY_ENABLED_FIELD,
    grid.rescueBuyEnabled,
    false,
  );
  grid.rescueBuyMinPriceCent =
    toFiniteNumber(fields[POSITIVE_GRID_RESCUE_MIN_PRICE_FIELD]?.trim()) ??
    toFiniteNumber(grid.rescueBuyMinPriceCent) ??
    normalBuyMaxCent;
  grid.rescueBuyMaxPriceCent =
    toFiniteNumber(fields[POSITIVE_GRID_RESCUE_MAX_PRICE_FIELD]?.trim()) ??
    toFiniteNumber(grid.rescueBuyMaxPriceCent) ??
    70;
  grid.blockConsecutiveSameSideBuys = fieldBoolean(
    fields,
    POSITIVE_GRID_BLOCK_CONSECUTIVE_SAME_SIDE_BUYS_FIELD,
    grid.blockConsecutiveSameSideBuys,
    true,
  );

  if (
    Object.prototype.hasOwnProperty.call(
      fields,
      POSITIVE_GRID_NO_BUY_RANGES_FIELD,
    )
  ) {
    const raw = fields[POSITIVE_GRID_NO_BUY_RANGES_FIELD].trim();
    const ranges = parsePositiveGridNoBuyRanges(raw || []);
    grid.noBuyRanges = ranges === "invalid" ? raw : ranges;
  } else if (!Object.prototype.hasOwnProperty.call(grid, "noBuyRanges")) {
    grid.noBuyRanges = [];
  }

  grid.depthGuardEnabled = fieldBoolean(
    fields,
    POSITIVE_GRID_DEPTH_GUARD_FIELD,
    grid.depthGuardEnabled,
    true,
  );
  grid.executionFloorGuardEnabled = fieldBoolean(
    fields,
    POSITIVE_GRID_EXECUTION_FLOOR_ENABLED_FIELD,
    grid.executionFloorGuardEnabled,
    true,
  );
  if (
    Object.prototype.hasOwnProperty.call(
      fields,
      POSITIVE_GRID_EXECUTION_FLOOR_PRICE_FIELD,
    )
  ) {
    const executionFloorRaw =
      fields[POSITIVE_GRID_EXECUTION_FLOOR_PRICE_FIELD].trim();
    const executionFloorCent = toFiniteNumber(executionFloorRaw);
    if (executionFloorRaw && executionFloorCent != null) {
      grid.executionFloorPriceCent = executionFloorCent;
    } else {
      delete grid.executionFloorPriceCent;
    }
  }
  grid.triggerPriceGuardEnabled = fieldBoolean(
    fields,
    POSITIVE_GRID_TRIGGER_PRICE_GUARD_FIELD,
    grid.triggerPriceGuardEnabled,
    false,
  );
  grid.ptbGuardEnabled = fieldBoolean(
    fields,
    POSITIVE_GRID_PTB_GUARD_FIELD,
    grid.ptbGuardEnabled,
    false,
  );
  grid.ptbMinDiff =
    toFiniteNumber(fields[POSITIVE_GRID_PTB_MIN_DIFF_FIELD]?.trim()) ??
    toFiniteNumber(grid.ptbMinDiff) ??
    2;
  if (
    Object.prototype.hasOwnProperty.call(
      fields,
      POSITIVE_GRID_PTB_RESCUE_MIN_DIFF_FIELD,
    )
  ) {
    const ptbRescueMinDiffRaw =
      fields[POSITIVE_GRID_PTB_RESCUE_MIN_DIFF_FIELD]?.trim() ?? "";
    const ptbRescueMinDiff = toFiniteNumber(ptbRescueMinDiffRaw);
    if (ptbRescueMinDiffRaw && ptbRescueMinDiff != null) {
      grid.ptbRescueMinDiff = ptbRescueMinDiff;
    } else {
      delete grid.ptbRescueMinDiff;
    }
  }
  grid.ptbDiffUnit =
    fields[POSITIVE_GRID_PTB_DIFF_UNIT_FIELD]?.trim() ||
    toStringValue(grid.ptbDiffUnit).trim() ||
    "usd";
  grid.ptbCurrentPriceSource =
    fields[POSITIVE_GRID_PTB_CURRENT_SOURCE_FIELD]?.trim() ||
    toStringValue(grid.ptbCurrentPriceSource).trim() ||
    "chainlink";

  config.positiveQuantityFlipGrid = grid;
  return true;
}

function safeParseJson(raw: string): unknown {
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}
