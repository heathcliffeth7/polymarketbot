import assert from "node:assert/strict";
import test from "node:test";

import {
  normalizePositiveQuantityFlipGridBuildConfig,
  parsePositiveGridNoBuyRanges,
  POSITIVE_GRID_BASE_BUY_USDC_FIELD,
  POSITIVE_GRID_BLOCK_CONSECUTIVE_SAME_SIDE_BUYS_FIELD,
  POSITIVE_GRID_BASKET_EXIT_ENABLED_FIELD,
  POSITIVE_GRID_STOP_BUYS_AFTER_PAIRLOCK_MERGE_FIELD,
  POSITIVE_GRID_CYCLE_WINDOW_END_SEC_FIELD,
  POSITIVE_GRID_CYCLE_WINDOW_MODE_FIELD,
  POSITIVE_GRID_CYCLE_WINDOW_SECS_FIELD,
  POSITIVE_GRID_CYCLE_WINDOW_START_SEC_FIELD,
  POSITIVE_GRID_DIRECT_EXIT_ENABLED_FIELD,
  POSITIVE_GRID_DEPTH_GUARD_FIELD,
  POSITIVE_GRID_EXECUTION_FLOOR_ENABLED_FIELD,
  POSITIVE_GRID_EXECUTION_FLOOR_PRICE_FIELD,
  POSITIVE_GRID_INVENTORY_BALANCE_LEAD_QTY_FIELD,
  POSITIVE_GRID_MAX_OPEN_BUYS_FIELD,
  POSITIVE_GRID_MIN_MARKETABLE_BUY_USDC_FIELD,
  POSITIVE_GRID_NO_BUY_RANGES_FIELD,
  POSITIVE_GRID_NORMAL_BUY_MAX_FIELD,
  POSITIVE_GRID_NORMAL_BUY_MIN_FIELD,
  POSITIVE_GRID_PARTIAL_RECOVERY_BALANCE_RESERVE_FIELD,
  POSITIVE_GRID_PARTIAL_RECOVERY_ENABLED_FIELD,
  POSITIVE_GRID_PARTIAL_RECOVERY_IGNORE_MARKET_BUDGET_FIELD,
  POSITIVE_GRID_PARTIAL_RECOVERY_MAX_BUY_FIELD,
  POSITIVE_GRID_PARTIAL_RECOVERY_MIN_LOSS_REDUCTION_FIELD,
  POSITIVE_GRID_PTB_CURRENT_SOURCE_FIELD,
  POSITIVE_GRID_PTB_DIFF_UNIT_FIELD,
  POSITIVE_GRID_PTB_GUARD_FIELD,
  POSITIVE_GRID_PTB_MIN_DIFF_FIELD,
  POSITIVE_GRID_PTB_RESCUE_MIN_DIFF_FIELD,
  POSITIVE_GRID_PROFIT_TARGET_FIELD,
  POSITIVE_GRID_QUANTITY_SIZING_MODE_FIELD,
  POSITIVE_GRID_RESCUE_BUY_ENABLED_FIELD,
  POSITIVE_GRID_RESCUE_MIN_PRICE_FIELD,
  POSITIVE_GRID_RESCUE_MAX_PRICE_FIELD,
  POSITIVE_GRID_SIZING_PRICE_BUFFER_FIELD,
  POSITIVE_GRID_TAKE_PROFIT_SELL_BID_FIELD,
  POSITIVE_GRID_TRIGGER_PRICE_GUARD_FIELD,
  POSITIVE_FLIP_PAIRLOCK_COMPRESSION_MODE,
  POSITIVE_QUANTITY_FLIP_GRID_MODE,
} from "./positive-quantity-flip-grid";

test("positive grid normal buy range writes entry, hard, and worst price together", () => {
  const config: Record<string, unknown> = {
    mode: POSITIVE_QUANTITY_FLIP_GRID_MODE,
    positiveQuantityFlipGrid: {
      entryBandMinCent: 50,
      entryBandMaxCent: 60,
      hardMaxPriceCent: 59,
      worstPriceCent: 59,
    },
  };

  normalizePositiveQuantityFlipGridBuildConfig(config, {
    [POSITIVE_GRID_CYCLE_WINDOW_MODE_FIELD]: "last",
    [POSITIVE_GRID_CYCLE_WINDOW_SECS_FIELD]: "120",
    [POSITIVE_GRID_NORMAL_BUY_MIN_FIELD]: "50",
    [POSITIVE_GRID_NORMAL_BUY_MAX_FIELD]: "60",
    [POSITIVE_GRID_PROFIT_TARGET_FIELD]: "0.25",
    [POSITIVE_GRID_TAKE_PROFIT_SELL_BID_FIELD]: "98",
    [POSITIVE_GRID_SIZING_PRICE_BUFFER_FIELD]: "3",
    [POSITIVE_GRID_QUANTITY_SIZING_MODE_FIELD]: "inventory_balance",
    [POSITIVE_GRID_INVENTORY_BALANCE_LEAD_QTY_FIELD]: "1.5",
    [POSITIVE_GRID_NO_BUY_RANGES_FIELD]: "[]",
  });

  const grid = config.positiveQuantityFlipGrid as Record<string, unknown>;
  assert.equal(grid.entryBandMinCent, 50);
  assert.equal(grid.entryBandMaxCent, 60);
  assert.equal(grid.hardMaxPriceCent, 60);
  assert.equal(grid.worstPriceCent, 60);
  assert.equal(grid.minPositiveProfitUsdc, 0.25);
  assert.equal(grid.minSellNetProfitUsdc, 0.25);
  assert.equal(grid.sellBidMinCent, 98);
  assert.equal(grid.sizingPriceBufferCent, 3);
  assert.equal(grid.quantitySizingMode, "inventory_balance");
  assert.equal(grid.inventoryBalanceLeadQty, 1.5);
  assert.equal(grid.cycleWindowMode, "last");
  assert.equal(grid.cycleWindowSecs, 120);
  assert.deepEqual(grid.noBuyRanges, []);
});

test("positive grid no-buy range parser accepts valid arrays and rejects bad shapes", () => {
  assert.deepEqual(
    parsePositiveGridNoBuyRanges('[{"minCent":56,"maxCent":60}]'),
    [{ minCent: 56, maxCent: 60 }],
  );
  assert.equal(
    parsePositiveGridNoBuyRanges('{"minCent":56,"maxCent":60}'),
    "invalid",
  );
  assert.equal(
    parsePositiveGridNoBuyRanges('[{"minCent":60,"maxCent":56}]'),
    "invalid",
  );
  assert.equal(
    parsePositiveGridNoBuyRanges('[{"minCent":50,"maxCent":101}]'),
    "invalid",
  );
});

test("positive grid mapper writes normal buy and marketable floor controls", () => {
  const config: Record<string, unknown> = {
    mode: POSITIVE_QUANTITY_FLIP_GRID_MODE,
    positiveQuantityFlipGrid: {},
  };

  normalizePositiveQuantityFlipGridBuildConfig(config, {
    [POSITIVE_GRID_BASE_BUY_USDC_FIELD]: "1.50",
    [POSITIVE_GRID_MIN_MARKETABLE_BUY_USDC_FIELD]: "1.05",
  });

  const grid = config.positiveQuantityFlipGrid as Record<string, unknown>;
  assert.equal(grid.baseBuyUsdc, 1.5);
  assert.equal(grid.minMarketableBuyUsdc, 1.05);
});

test("positive flip pairlock compression mapper uses 2 base buy default", () => {
  const config: Record<string, unknown> = {
    mode: POSITIVE_FLIP_PAIRLOCK_COMPRESSION_MODE,
    positiveQuantityFlipGrid: {},
  };

  normalizePositiveQuantityFlipGridBuildConfig(config, {});

  const grid = config.positiveQuantityFlipGrid as Record<string, unknown>;
  assert.equal(grid.baseBuyUsdc, 2);
});

test("positive flip pairlock compression mapper preserves user base buy usdc", () => {
  const config: Record<string, unknown> = {
    mode: POSITIVE_FLIP_PAIRLOCK_COMPRESSION_MODE,
    positiveQuantityFlipGrid: {},
  };

  normalizePositiveQuantityFlipGridBuildConfig(config, {
    [POSITIVE_GRID_BASE_BUY_USDC_FIELD]: "2.00",
  });

  const grid = config.positiveQuantityFlipGrid as Record<string, unknown>;
  assert.equal(grid.baseBuyUsdc, 2);
});

test("positive flip pairlock compression mapper preserves separate mode", () => {
  const config: Record<string, unknown> = {
    mode: POSITIVE_FLIP_PAIRLOCK_COMPRESSION_MODE,
    positiveQuantityFlipGrid: {
      pairlockCompressionEnabled: true,
      maxPairCostCent: 94,
    },
  };

  const handled = normalizePositiveQuantityFlipGridBuildConfig(config, {
    [POSITIVE_GRID_NORMAL_BUY_MIN_FIELD]: "52",
    [POSITIVE_GRID_NORMAL_BUY_MAX_FIELD]: "58",
    [POSITIVE_GRID_PROFIT_TARGET_FIELD]: "0.05",
    [POSITIVE_GRID_TAKE_PROFIT_SELL_BID_FIELD]: "59",
    [POSITIVE_GRID_BASKET_EXIT_ENABLED_FIELD]: "false",
    [POSITIVE_GRID_STOP_BUYS_AFTER_PAIRLOCK_MERGE_FIELD]: "true",
    [POSITIVE_GRID_DIRECT_EXIT_ENABLED_FIELD]: "false",
    [POSITIVE_GRID_SIZING_PRICE_BUFFER_FIELD]: "1",
    [POSITIVE_GRID_QUANTITY_SIZING_MODE_FIELD]: "profit_target",
  });

  const grid = config.positiveQuantityFlipGrid as Record<string, unknown>;
  assert.equal(handled, true);
  assert.equal(config.mode, POSITIVE_FLIP_PAIRLOCK_COMPRESSION_MODE);
  assert.equal(grid.entryBandMinCent, 52);
  assert.equal(grid.entryBandMaxCent, 58);
  assert.equal(grid.minPositiveProfitUsdc, 0.05);
  assert.equal(grid.minSellNetProfitUsdc, 0.05);
  assert.equal(grid.sellBidMinCent, 59);
  assert.equal(grid.basketExitEnabled, false);
  assert.equal(grid.directExitEnabled, false);
  assert.equal(grid.stopBuysAfterPairlockMerge, true);
  assert.equal(grid.sizingPriceBufferCent, 1);
  assert.equal(grid.pairlockCompressionEnabled, true);
  assert.equal(grid.maxPairCostCent, 94);
});

test("positive flip pairlock compression mapper preserves fixed usdc sizing mode", () => {
  const config: Record<string, unknown> = {
    mode: POSITIVE_FLIP_PAIRLOCK_COMPRESSION_MODE,
    positiveQuantityFlipGrid: {},
  };

  normalizePositiveQuantityFlipGridBuildConfig(config, {
    [POSITIVE_GRID_QUANTITY_SIZING_MODE_FIELD]: "fixed_usdc",
  });

  const grid = config.positiveQuantityFlipGrid as Record<string, unknown>;
  assert.equal(grid.quantitySizingMode, "fixed_usdc");
});

test("positive grid mapper writes trade guard fields into nested config", () => {
  const config: Record<string, unknown> = {
    mode: POSITIVE_QUANTITY_FLIP_GRID_MODE,
    positiveQuantityFlipGrid: {},
  };

  normalizePositiveQuantityFlipGridBuildConfig(config, {
    [POSITIVE_GRID_CYCLE_WINDOW_MODE_FIELD]: "custom_range",
    [POSITIVE_GRID_CYCLE_WINDOW_START_SEC_FIELD]: "120",
    [POSITIVE_GRID_CYCLE_WINDOW_END_SEC_FIELD]: "300",
    [POSITIVE_GRID_NORMAL_BUY_MIN_FIELD]: "50",
    [POSITIVE_GRID_NORMAL_BUY_MAX_FIELD]: "60",
    [POSITIVE_GRID_PROFIT_TARGET_FIELD]: "0.25",
    [POSITIVE_GRID_TAKE_PROFIT_SELL_BID_FIELD]: "98",
    [POSITIVE_GRID_BASKET_EXIT_ENABLED_FIELD]: "true",
    [POSITIVE_GRID_DIRECT_EXIT_ENABLED_FIELD]: "true",
    [POSITIVE_GRID_SIZING_PRICE_BUFFER_FIELD]: "3",
    [POSITIVE_GRID_PARTIAL_RECOVERY_ENABLED_FIELD]: "true",
    [POSITIVE_GRID_PARTIAL_RECOVERY_MIN_LOSS_REDUCTION_FIELD]: "0.1",
    [POSITIVE_GRID_PARTIAL_RECOVERY_BALANCE_RESERVE_FIELD]: "1",
    [POSITIVE_GRID_PARTIAL_RECOVERY_MAX_BUY_FIELD]: "",
    [POSITIVE_GRID_PARTIAL_RECOVERY_IGNORE_MARKET_BUDGET_FIELD]: "true",
    [POSITIVE_GRID_QUANTITY_SIZING_MODE_FIELD]: "profit_target",
    [POSITIVE_GRID_INVENTORY_BALANCE_LEAD_QTY_FIELD]: "0",
    [POSITIVE_GRID_MAX_OPEN_BUYS_FIELD]: "7",
    [POSITIVE_GRID_NO_BUY_RANGES_FIELD]: '[{"minCent":56,"maxCent":60}]',
    [POSITIVE_GRID_DEPTH_GUARD_FIELD]: "true",
    [POSITIVE_GRID_EXECUTION_FLOOR_ENABLED_FIELD]: "true",
    [POSITIVE_GRID_EXECUTION_FLOOR_PRICE_FIELD]: "52",
    [POSITIVE_GRID_RESCUE_BUY_ENABLED_FIELD]: "true",
    [POSITIVE_GRID_RESCUE_MIN_PRICE_FIELD]: "63",
    [POSITIVE_GRID_RESCUE_MAX_PRICE_FIELD]: "70",
    [POSITIVE_GRID_BLOCK_CONSECUTIVE_SAME_SIDE_BUYS_FIELD]: "false",
    [POSITIVE_GRID_TRIGGER_PRICE_GUARD_FIELD]: "true",
    [POSITIVE_GRID_PTB_GUARD_FIELD]: "true",
    [POSITIVE_GRID_PTB_MIN_DIFF_FIELD]: "2",
    [POSITIVE_GRID_PTB_DIFF_UNIT_FIELD]: "usd",
    [POSITIVE_GRID_PTB_CURRENT_SOURCE_FIELD]: "binance",
  });

  const grid = config.positiveQuantityFlipGrid as Record<string, unknown>;
  assert.equal(grid.cycleWindowMode, "custom_range");
  assert.equal(grid.cycleWindowStartSec, 120);
  assert.equal(grid.cycleWindowEndSec, 300);
  assert.equal(grid.minPositiveProfitUsdc, 0.25);
  assert.equal(grid.minSellNetProfitUsdc, 0.25);
  assert.equal(grid.sellBidMinCent, 98);
  assert.equal(grid.basketExitEnabled, true);
  assert.equal(grid.directExitEnabled, true);
  assert.equal(grid.sizingPriceBufferCent, 3);
  assert.equal(grid.partialRecoveryEnabled, true);
  assert.equal(grid.partialRecoveryMinLossReductionUsdc, 0.1);
  assert.equal(grid.partialRecoveryBalanceReserveUsdc, 1);
  assert.equal(grid.partialRecoveryMaxBuyUsdc, undefined);
  assert.equal(grid.partialRecoveryIgnoreMarketBudget, true);
  assert.equal(grid.quantitySizingMode, "profit_target");
  assert.equal(grid.inventoryBalanceLeadQty, 0);
  assert.equal(grid.maxOpenGridBuysPerMarket, 7);
  assert.equal(grid.depthGuardEnabled, true);
  assert.equal(grid.executionFloorGuardEnabled, true);
  assert.equal(grid.executionFloorPriceCent, 52);
  assert.equal(grid.rescueBuyEnabled, true);
  assert.equal(grid.rescueBuyMinPriceCent, 63);
  assert.equal(grid.rescueBuyMaxPriceCent, 70);
  assert.equal(grid.blockConsecutiveSameSideBuys, false);
  assert.equal(grid.triggerPriceGuardEnabled, true);
  assert.equal(grid.ptbGuardEnabled, true);
  assert.equal(grid.ptbMinDiff, 2);
  assert.equal(grid.ptbDiffUnit, "usd");
  assert.equal(grid.ptbCurrentPriceSource, "binance");
});

test("positive grid ptb rescue min diff round-trips and clears when empty", () => {
  const config: Record<string, unknown> = {
    mode: POSITIVE_QUANTITY_FLIP_GRID_MODE,
    positiveQuantityFlipGrid: {
      ptbGuardEnabled: true,
      ptbMinDiff: 80,
      ptbRescueMinDiff: 40,
    },
  };

  normalizePositiveQuantityFlipGridBuildConfig(config, {
    [POSITIVE_GRID_CYCLE_WINDOW_MODE_FIELD]: "last",
    [POSITIVE_GRID_CYCLE_WINDOW_SECS_FIELD]: "60",
    [POSITIVE_GRID_NORMAL_BUY_MIN_FIELD]: "50",
    [POSITIVE_GRID_NORMAL_BUY_MAX_FIELD]: "60",
    [POSITIVE_GRID_PROFIT_TARGET_FIELD]: "0.05",
    [POSITIVE_GRID_TAKE_PROFIT_SELL_BID_FIELD]: "98",
    [POSITIVE_GRID_PTB_GUARD_FIELD]: "true",
    [POSITIVE_GRID_PTB_MIN_DIFF_FIELD]: "80",
    [POSITIVE_GRID_PTB_RESCUE_MIN_DIFF_FIELD]: "40",
    [POSITIVE_GRID_PTB_DIFF_UNIT_FIELD]: "usd",
    [POSITIVE_GRID_PTB_CURRENT_SOURCE_FIELD]: "hyperliquid",
    [POSITIVE_GRID_RESCUE_BUY_ENABLED_FIELD]: "true",
    [POSITIVE_GRID_RESCUE_MIN_PRICE_FIELD]: "60",
    [POSITIVE_GRID_RESCUE_MAX_PRICE_FIELD]: "75",
  });

  let grid = config.positiveQuantityFlipGrid as Record<string, unknown>;
  assert.equal(grid.ptbMinDiff, 80);
  assert.equal(grid.ptbRescueMinDiff, 40);

  normalizePositiveQuantityFlipGridBuildConfig(config, {
    [POSITIVE_GRID_CYCLE_WINDOW_MODE_FIELD]: "last",
    [POSITIVE_GRID_CYCLE_WINDOW_SECS_FIELD]: "60",
    [POSITIVE_GRID_NORMAL_BUY_MIN_FIELD]: "50",
    [POSITIVE_GRID_NORMAL_BUY_MAX_FIELD]: "60",
    [POSITIVE_GRID_PROFIT_TARGET_FIELD]: "0.05",
    [POSITIVE_GRID_TAKE_PROFIT_SELL_BID_FIELD]: "98",
    [POSITIVE_GRID_PTB_GUARD_FIELD]: "true",
    [POSITIVE_GRID_PTB_MIN_DIFF_FIELD]: "80",
    [POSITIVE_GRID_PTB_RESCUE_MIN_DIFF_FIELD]: "",
    [POSITIVE_GRID_PTB_DIFF_UNIT_FIELD]: "usd",
    [POSITIVE_GRID_PTB_CURRENT_SOURCE_FIELD]: "hyperliquid",
    [POSITIVE_GRID_RESCUE_BUY_ENABLED_FIELD]: "true",
    [POSITIVE_GRID_RESCUE_MIN_PRICE_FIELD]: "60",
    [POSITIVE_GRID_RESCUE_MAX_PRICE_FIELD]: "75",
  });

  grid = config.positiveQuantityFlipGrid as Record<string, unknown>;
  assert.equal(grid.ptbMinDiff, 80);
  assert.equal(grid.ptbRescueMinDiff, undefined);
});
