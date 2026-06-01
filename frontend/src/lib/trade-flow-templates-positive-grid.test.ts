import assert from "node:assert/strict";
import test from "node:test";

import { normalizeTradeFlowGraph } from "@/lib/queries/trade-flow/graph";
import { validateTradeFlowGraph } from "@/lib/queries/trade-flow/validation";
import {
  createPositiveFlipPairlockCompressionGraph,
  createPositiveQuantityFlipGrid1UsdcGraph,
  createPositiveQuantityFlipGridInventoryBalanceGraph,
  createRevengeFlip10_80Graph,
} from "@/lib/trade-flow-templates";
import type { TradeFlowGraph, TradeFlowNode } from "@/lib/types";

function positiveGridGraph(): TradeFlowGraph {
  return normalizeTradeFlowGraph(
    createPositiveQuantityFlipGrid1UsdcGraph(null, null),
  );
}

function cloneGraph(graph: TradeFlowGraph): TradeFlowGraph {
  return JSON.parse(JSON.stringify(graph)) as TradeFlowGraph;
}

function nodeConfig(node: TradeFlowNode): Record<string, unknown> {
  return node.config && typeof node.config === "object"
    ? (node.config as Record<string, unknown>)
    : {};
}

function findNode(graph: TradeFlowGraph, key: string): TradeFlowNode {
  const node = graph.nodes.find((candidate) => candidate.key === key);
  assert.ok(node, `missing node ${key}`);
  return node;
}

function errorCodes(graph: TradeFlowGraph): string[] {
  return validateTradeFlowGraph(graph)
    .issues.filter((issue) => issue.severity === "error")
    .map((issue) => issue.code);
}

test("positive quantity flip grid template validates with scoped trigger and downstream action", () => {
  const graph = positiveGridGraph();
  const trigger = nodeConfig(findNode(graph, "trigger_positive_grid"));
  const action = nodeConfig(findNode(graph, "action_positive_grid_buy"));
  const grid = action.positiveQuantityFlipGrid as Record<string, unknown>;

  assert.equal(trigger.bindingMode, "positive_quantity_flip_grid_only");
  assert.equal(trigger.marketScope, "btc_5m_updown");
  assert.equal(trigger.repeatMode, "loop");
  assert.equal(action.mode, "positive_quantity_flip_grid_v1");
  assert.equal(action.orderType, "FAK");
  assert.equal(grid.baseBuyUsdc, 1.05);
  assert.equal(grid.minMarketableBuyUsdc, 1.05);
  assert.equal(grid.maxSingleBuyUsdc, 2.2);
  assert.equal(grid.maxTotalSpentPerMarketUsdc, 9.5);
  assert.equal(grid.hardMaxPriceCent, 60);
  assert.equal(grid.worstPriceCent, 60);
  assert.equal(grid.minPositiveProfitUsdc, 1);
  assert.equal(grid.minSellNetProfitUsdc, 1);
  assert.equal(grid.sellBidMinCent, 98);
  assert.equal(grid.sizingPriceBufferCent, 3);
  assert.equal(grid.partialRecoveryEnabled ?? false, false);
  assert.equal(grid.quantitySizingMode, "profit_target");
  assert.equal(grid.inventoryBalanceLeadQty, 0);
  assert.equal(grid.cycleWindowMode, "custom_range");
  assert.equal(grid.cycleWindowStartSec, 0);
  assert.equal(grid.cycleWindowEndSec, 300);
  assert.equal(grid.newGridBuyStartRemainingSec, 300);
  assert.equal(grid.rescueBuyEnabled, false);
  assert.equal(grid.rescueBuyMinPriceCent, 60);
  assert.equal(grid.rescueBuyMaxPriceCent, 70);
  assert.equal(grid.blockConsecutiveSameSideBuys, true);
  assert.deepEqual(grid.noBuyRanges, []);
  assert.equal(grid.depthGuardEnabled, true);
  assert.equal(grid.executionFloorGuardEnabled, true);
  assert.equal(grid.triggerPriceGuardEnabled, false);
  assert.equal(grid.ptbGuardEnabled, false);
  assert.equal(grid.ptbMinDiff, 2);
  assert.equal(grid.ptbDiffUnit, "usd");
  assert.equal(grid.ptbCurrentPriceSource, "hyperliquid");
  assert.deepEqual(errorCodes(graph), []);
});

test("positive quantity flip grid inventory balance template uses capped sizing", () => {
  const graph = normalizeTradeFlowGraph(
    createPositiveQuantityFlipGridInventoryBalanceGraph(null, null),
  );
  const action = nodeConfig(findNode(graph, "action_positive_grid_buy"));
  const grid = action.positiveQuantityFlipGrid as Record<string, unknown>;

  assert.equal(grid.quantitySizingMode, "inventory_balance");
  assert.equal(grid.inventoryBalanceLeadQty, 0);
  assert.equal(grid.maxSingleBuyUsdc, 5);
  assert.equal(grid.maxTotalSpentPerMarketUsdc, 12);
  assert.equal(grid.maxOpenGridBuysPerMarket, 5);
  assert.equal(grid.minPositiveProfitUsdc, 0.02);
  assert.equal(grid.minSellNetProfitUsdc, 0.02);
  assert.deepEqual(errorCodes(graph), []);
});

test("positive flip pairlock compression template uses separate mode and pairlock defaults", () => {
  const graph = normalizeTradeFlowGraph(
    createPositiveFlipPairlockCompressionGraph(null, null),
  );
  const trigger = nodeConfig(findNode(graph, "trigger_positive_grid"));
  const action = nodeConfig(findNode(graph, "action_positive_grid_buy"));
  const grid = action.positiveQuantityFlipGrid as Record<string, unknown>;

  assert.equal(trigger.bindingMode, "positive_quantity_flip_grid_only");
  assert.equal(action.mode, "positive_flip_pairlock_compression_v1");
  assert.equal(grid.baseBuyUsdc, 2);
  assert.equal(grid.minMarketableBuyUsdc, 1.05);
  assert.equal(grid.entryBandMinCent, 52);
  assert.equal(grid.entryBandMaxCent, 58);
  assert.equal(grid.minPositiveProfitUsdc, 0.05);
  assert.equal(grid.maxTotalSpentPerMarketUsdc, undefined);
  assert.equal(grid.maxOpenGridBuysPerMarket, 10);
  assert.equal(grid.quantitySizingMode, "fixed_usdc");
  assert.equal(grid.pairlockCompressionEnabled, true);
  assert.equal(grid.stopBuysAfterPairlockMerge, true);
  assert.equal(grid.targetPairlockProfitCent, 5);
  assert.equal(grid.feeBufferCent, 1);
  assert.equal(grid.maxPairCostCent, 94);
  assert.equal(grid.pairlockOrderType, "FOK");
  assert.equal(grid.maxUnmergedExposureUsdc, 2);
  assert.equal(grid.minBasketProfitUsdc, 0.1);
  assert.equal(grid.minDirectProfitUsdc, 0.05);
  assert.deepEqual(errorCodes(graph), []);
});

test("revenge flip 10/80 template creates draft-ready scoped graph", () => {
  const graph = normalizeTradeFlowGraph(createRevengeFlip10_80Graph(null, null));
  const trigger = nodeConfig(findNode(graph, "trigger_revenge_flip"));
  const action = nodeConfig(findNode(graph, "action_revenge_flip"));
  const revenge = action.revengeFlip as Record<string, unknown>;
  const entryRules = revenge.entryPtbRules as Record<string, unknown>[];

  assert.equal(trigger.bindingMode, "revenge_flip_only");
  assert.equal(trigger.marketScope, "btc_5m_updown");
  assert.equal(trigger.repeatMode, "loop");
  assert.equal(action.mode, "revenge_flip_v1");
  assert.equal(action.priceToBeatMinDiff, 10);
  assert.equal(action.priceToBeatMinDiffUnit, "usd");
  assert.equal(action.priceToBeatCurrentPriceSource, "chainlink");
  assert.equal(revenge.reentrySideMode, "rule_match");
  assert.equal(revenge.minReentryShares, 5);
  assert.deepEqual(entryRules, [
    {
      minFlip: 0,
      maxFlip: 0,
      sideMode: "any",
      priceToBeatMinDiff: 10,
      priceToBeatMinDiffUnit: "usd",
      maxPriceCent: 80,
    },
    {
      minFlip: 1,
      sideMode: "any",
      priceToBeatMinDiff: 4,
      priceToBeatMinDiffUnit: "cent",
    },
  ]);
  assert.deepEqual(errorCodes(graph), []);
});

test("positive quantity flip grid rejects classic exits and buy fill lock", () => {
  const graph = cloneGraph(positiveGridGraph());
  const action = findNode(graph, "action_positive_grid_buy");
  action.config = {
    ...nodeConfig(action),
    buyFillLockEnabled: true,
    tpEnabled: true,
    slEnabled: true,
    autoSellOnWindowEnd: true,
  };

  const codes = errorCodes(graph);
  assert.ok(codes.includes("positive_grid_disallows_buy_fill_lock"));
  assert.ok(codes.includes("positive_grid_disallows_classic_exits"));
});

test("positive quantity flip grid requires positive-only market price binding", () => {
  const graph = cloneGraph(positiveGridGraph());
  const trigger = findNode(graph, "trigger_positive_grid");
  trigger.config = {
    ...nodeConfig(trigger),
    bindingMode: "pair_lock_only",
  };

  assert.ok(
    errorCodes(graph).includes(
      "positive_grid_requires_positive_binding_trigger",
    ),
  );
});

test("positive quantity flip grid accepts multiple no-buy ranges", () => {
  const graph = cloneGraph(positiveGridGraph());
  const action = findNode(graph, "action_positive_grid_buy");
  const config = nodeConfig(action);
  const grid = config.positiveQuantityFlipGrid as Record<string, unknown>;
  grid.noBuyRanges = [
    { minCent: 56, maxCent: 60 },
    { minCent: 50, maxCent: 51 },
  ];

  assert.deepEqual(errorCodes(graph), []);
});

test("positive quantity flip grid rejects invalid no-buy ranges", () => {
  for (const noBuyRanges of [
    "56-60",
    [{ minCent: 60, maxCent: 56 }],
    [{ minCent: 50, maxCent: 101 }],
  ]) {
    const graph = cloneGraph(positiveGridGraph());
    const action = findNode(graph, "action_positive_grid_buy");
    const config = nodeConfig(action);
    const grid = config.positiveQuantityFlipGrid as Record<string, unknown>;
    grid.noBuyRanges = noBuyRanges;

    const codes = errorCodes(graph);
    assert.ok(
      codes.includes("invalid_positive_grid_no_buy_ranges") ||
        codes.includes("invalid_positive_grid_no_buy_range"),
    );
  }
});

test("positive quantity flip grid accepts valid trade guard config", () => {
  const graph = cloneGraph(positiveGridGraph());
  const action = findNode(graph, "action_positive_grid_buy");
  const config = nodeConfig(action);
  const grid = config.positiveQuantityFlipGrid as Record<string, unknown>;
  grid.executionFloorGuardEnabled = true;
  grid.executionFloorPriceCent = 52;
  grid.rescueBuyEnabled = true;
  grid.rescueBuyMaxPriceCent = 70;
  grid.triggerPriceGuardEnabled = true;
  grid.ptbGuardEnabled = true;
  grid.ptbMinDiff = 2;
  grid.ptbDiffUnit = "usd";
  grid.ptbCurrentPriceSource = "hyperliquid";
  grid.cycleWindowMode = "last";
  grid.cycleWindowSecs = 120;
  delete grid.cycleWindowStartSec;
  delete grid.cycleWindowEndSec;

  assert.deepEqual(errorCodes(graph), []);
});

test("positive quantity flip grid accepts entry band up to 70 cent", () => {
  const graph = cloneGraph(positiveGridGraph());
  const action = findNode(graph, "action_positive_grid_buy");
  const config = nodeConfig(action);
  const grid = config.positiveQuantityFlipGrid as Record<string, unknown>;
  grid.entryBandMaxCent = 70;
  grid.hardMaxPriceCent = 70;
  grid.worstPriceCent = 70;
  grid.rescueBuyEnabled = true;
  grid.rescueBuyMinPriceCent = 70;
  grid.rescueBuyMaxPriceCent = 75;

  assert.deepEqual(errorCodes(graph), []);
});

test("positive flip pairlock compression accepts fixed usdc sizing mode", () => {
  const graph = normalizeTradeFlowGraph(
    createPositiveFlipPairlockCompressionGraph(null, null),
  );
  const action = nodeConfig(findNode(graph, "action_positive_grid_buy"));
  const grid = action.positiveQuantityFlipGrid as Record<string, unknown>;
  grid.quantitySizingMode = "fixed_usdc";

  assert.deepEqual(errorCodes(graph), []);
});

test("positive flip pairlock accepts fixed usdc with raised base buy usdc", () => {
  const graph = normalizeTradeFlowGraph(
    createPositiveFlipPairlockCompressionGraph(null, null),
  );
  const action = nodeConfig(findNode(graph, "action_positive_grid_buy"));
  const grid = action.positiveQuantityFlipGrid as Record<string, unknown>;
  grid.quantitySizingMode = "fixed_usdc";
  grid.baseBuyUsdc = 10;

  assert.deepEqual(errorCodes(graph), []);
});

test("positive quantity flip grid rejects fixed usdc outside pairlock mode", () => {
  const graph = cloneGraph(positiveGridGraph());
  const action = findNode(graph, "action_positive_grid_buy");
  const config = nodeConfig(action);
  const grid = config.positiveQuantityFlipGrid as Record<string, unknown>;
  grid.quantitySizingMode = "fixed_usdc";

  assert.ok(
    errorCodes(graph).includes("invalid_positive_grid_fixed_usdc_requires_pairlock"),
  );
});

test("positive quantity flip grid rejects invalid trade guard config", () => {
  const invalidCases: Array<[string, Partial<Record<string, unknown>>]> = [
    [
      "invalid_positive_grid_ptb_min_diff",
      { ptbGuardEnabled: true, ptbMinDiff: 0 },
    ],
    [
      "invalid_positive_grid_ptb_rescue_min_diff",
      { ptbGuardEnabled: true, ptbMinDiff: 80, ptbRescueMinDiff: 0 },
    ],
    [
      "invalid_positive_grid_ptb_unit",
      { ptbGuardEnabled: true, ptbDiffUnit: "ticks" },
    ],
    [
      "invalid_positive_grid_ptb_current_source",
      { ptbGuardEnabled: true, ptbCurrentPriceSource: "kraken" },
    ],
    [
      "invalid_positive_grid_execution_floor_price",
      { executionFloorGuardEnabled: true, executionFloorPriceCent: 61 },
    ],
    [
      "invalid_positive_grid_rescue_max_price",
      { rescueBuyEnabled: true, rescueBuyMaxPriceCent: 60 },
    ],
    [
      "invalid_positive_grid_rescue_max_price",
      { rescueBuyEnabled: true, rescueBuyMinPriceCent: 59 },
    ],
    [
      "invalid_positive_grid_rescue_max_price",
      { rescueBuyEnabled: true, rescueBuyMinPriceCent: 70 },
    ],
    [
      "invalid_positive_grid_sizing_price_buffer",
      { sizingPriceBufferCent: -1 },
    ],
    [
      "invalid_positive_grid_partial_recovery_min_loss",
      { partialRecoveryMinLossReductionUsdc: -0.01 },
    ],
    [
      "invalid_positive_grid_partial_recovery_reserve",
      { partialRecoveryBalanceReserveUsdc: -1 },
    ],
    [
      "invalid_positive_grid_partial_recovery_max_buy",
      { partialRecoveryMaxBuyUsdc: 0 },
    ],
    [
      "invalid_positive_grid_partial_recovery_enabled",
      { partialRecoveryEnabled: "soon" },
    ],
    [
      "invalid_positive_grid_quantity_sizing_mode",
      { quantitySizingMode: "martingale" },
    ],
    [
      "invalid_positive_grid_inventory_balance_lead_qty",
      { inventoryBalanceLeadQty: -1 },
    ],
    ["invalid_positive_grid_profit_target", { minPositiveProfitUsdc: 0 }],
    ["invalid_positive_grid_profit_target", { minSellNetProfitUsdc: 0 }],
    ["invalid_positive_grid_sell_bid_min", { sellBidMinCent: 60 }],
    ["invalid_positive_grid_sell_bid_min", { sellBidMinCent: 101 }],
    [
      "invalid_positive_grid_block_consecutive_same_side_buys",
      { blockConsecutiveSameSideBuys: "soon" },
    ],
    ["invalid_positive_grid_cycle_window_mode", { cycleWindowMode: "middle" }],
    [
      "invalid_positive_grid_cycle_window_secs",
      { cycleWindowMode: "last", cycleWindowSecs: 0 },
    ],
    [
      "invalid_positive_grid_cycle_window_range",
      {
        cycleWindowMode: "custom_range",
        cycleWindowStartSec: 180,
        cycleWindowEndSec: 120,
      },
    ],
    [
      "invalid_positive_grid_cycle_window_duration",
      {
        cycleWindowMode: "custom_range",
        cycleWindowStartSec: 0,
        cycleWindowEndSec: 301,
      },
    ],
    [
      "invalid_positive_grid_pairlock_target",
      { pairlockCompressionEnabled: true, targetPairlockProfitCent: -1 },
    ],
    [
      "invalid_positive_grid_pairlock_fee_buffer",
      { pairlockCompressionEnabled: true, feeBufferCent: -1 },
    ],
    [
      "invalid_positive_grid_pairlock_max_pair_cost",
      { pairlockCompressionEnabled: true, maxPairCostCent: 100 },
    ],
    [
      "invalid_positive_grid_pairlock_order_type",
      { pairlockCompressionEnabled: true, pairlockOrderType: "GTC" },
    ],
    [
      "invalid_positive_grid_pairlock_unmerged_exposure",
      { pairlockCompressionEnabled: true, maxUnmergedExposureUsdc: -1 },
    ],
    [
      "invalid_positive_grid_pairlock_basket_profit",
      { pairlockCompressionEnabled: true, minBasketProfitUsdc: -0.01 },
    ],
    [
      "invalid_positive_grid_pairlock_direct_profit",
      { pairlockCompressionEnabled: true, minDirectProfitUsdc: -0.01 },
    ],
  ];

  for (const [expectedCode, patch] of invalidCases) {
    const graph = cloneGraph(positiveGridGraph());
    const action = findNode(graph, "action_positive_grid_buy");
    const config = nodeConfig(action);
    const grid = config.positiveQuantityFlipGrid as Record<string, unknown>;
    Object.assign(grid, patch);

    assert.ok(errorCodes(graph).includes(expectedCode), expectedCode);
  }
});
