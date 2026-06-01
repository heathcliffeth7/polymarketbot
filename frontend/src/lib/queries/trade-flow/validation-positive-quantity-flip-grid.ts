import type {
  TradeFlowGraph,
  TradeFlowNode,
  TradeFlowValidationIssue,
} from "@/lib/types";
import {
  isRecord,
  RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME,
  resolveConfiguredBinaryPrice,
  toBooleanish,
  toFiniteNumber,
  toTrimmedString,
} from "./shared";
import {
  parsePtbStopLossRules,
  validateActionPlaceOrderPtbStopLossConfig,
} from "./validation-action-place-order-ptb-stop-loss";
import { pushNodeError } from "./validation-core";

interface ParsedPositiveGridSlRule {
  priceCent: number;
  sizePct: number;
}

function parseOptionalJsonObject(
  raw: unknown,
): Record<string, unknown> | null | "invalid" {
  if (raw == null || raw === "") return null;
  if (isRecord(raw)) return raw;
  const text = toTrimmedString(raw);
  if (!text) return null;
  try {
    const parsed = JSON.parse(text);
    return isRecord(parsed) ? parsed : "invalid";
  } catch {
    return "invalid";
  }
}

function findUniqueUpstreamMarketPriceBindingMode(
  nodeKey: string,
  graph: TradeFlowGraph,
): string | null {
  const nodeMap = new Map(
    graph.nodes.map((candidate) => [candidate.key, candidate]),
  );
  const incomingByTarget = new Map<string, string[]>();
  for (const edge of graph.edges) {
    const incoming = incomingByTarget.get(edge.target) ?? [];
    incoming.push(edge.source);
    incomingByTarget.set(edge.target, incoming);
  }
  const visited = new Set<string>();
  const modes = new Set<string>();
  const queue = [nodeKey];
  while (queue.length > 0) {
    const current = queue.shift() as string;
    if (visited.has(current)) continue;
    visited.add(current);
    for (const sourceKey of incomingByTarget.get(current) ?? []) {
      const sourceNode = nodeMap.get(sourceKey);
      if (!sourceNode) continue;
      if (sourceNode.type === "trigger.market_price") {
        const sourceConfig = isRecord(sourceNode.config)
          ? sourceNode.config
          : {};
        modes.add(
          toTrimmedString(sourceConfig.bindingMode).toLowerCase() || "standard",
        );
      } else {
        queue.push(sourceKey);
      }
    }
  }
  return modes.size === 1 ? [...modes][0] : modes.size > 1 ? "multiple" : null;
}

function findFirstUpstreamMarketPriceConfig(
  nodeKey: string,
  graph: TradeFlowGraph,
): Record<string, unknown> | null {
  const nodeMap = new Map(
    graph.nodes.map((candidate) => [candidate.key, candidate]),
  );
  const incomingByTarget = new Map<string, string[]>();
  for (const edge of graph.edges) {
    const incoming = incomingByTarget.get(edge.target) ?? [];
    incoming.push(edge.source);
    incomingByTarget.set(edge.target, incoming);
  }
  const visited = new Set<string>();
  const queue = [nodeKey];
  while (queue.length > 0) {
    const current = queue.shift() as string;
    if (visited.has(current)) continue;
    visited.add(current);
    for (const sourceKey of incomingByTarget.get(current) ?? []) {
      const sourceNode = nodeMap.get(sourceKey);
      if (!sourceNode) continue;
      if (sourceNode.type === "trigger.market_price") {
        return isRecord(sourceNode.config) ? sourceNode.config : {};
      }
      queue.push(sourceKey);
    }
  }
  return null;
}

function inferPositiveGridMarketDurationSec(
  node: TradeFlowNode,
  graph: TradeFlowGraph,
  config: Record<string, unknown>,
): number {
  const triggerConfig = findFirstUpstreamMarketPriceConfig(node.key, graph);
  const marketScope = toTrimmedString(triggerConfig?.marketScope).toLowerCase();
  const timeframe =
    RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[marketScope]?.timeframe;
  if (timeframe === "15m") return 900;
  if (timeframe === "5m") return 300;

  const marketSlug = toTrimmedString(
    triggerConfig?.marketSlug ?? config.marketSlug ?? graph.context.marketSlug,
  ).toLowerCase();
  return marketSlug.includes("-15m-") ? 900 : 300;
}

function validateNoBuyRanges(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: unknown,
) {
  if (value == null) return;
  if (!Array.isArray(value)) {
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_no_buy_ranges",
      "positiveQuantityFlipGrid noBuyRanges must be an array.",
    );
    return;
  }
  for (const item of value) {
    if (!isRecord(item)) {
      pushNodeError(
        issues,
        node,
        "invalid_positive_grid_no_buy_range",
        "positiveQuantityFlipGrid noBuyRanges entries must be objects.",
      );
      return;
    }
    const minCent = toFiniteNumber(item.minCent);
    const maxCent = toFiniteNumber(item.maxCent);
    if (
      minCent == null ||
      maxCent == null ||
      !(minCent > 0 && minCent < maxCent && maxCent <= 100)
    ) {
      pushNodeError(
        issues,
        node,
        "invalid_positive_grid_no_buy_range",
        "positiveQuantityFlipGrid noBuyRanges entries must satisfy 0 < minCent < maxCent <= 100.",
      );
      return;
    }
  }
}

function parsePositiveGridSlRules(raw: unknown): {
  isArray: boolean;
  validRules: ParsedPositiveGridSlRule[];
  invalidItem: boolean;
} {
  if (!Array.isArray(raw)) {
    return { isArray: false, validRules: [], invalidItem: false };
  }

  const validRules: ParsedPositiveGridSlRule[] = [];
  let invalidItem = false;
  for (const item of raw) {
    if (!isRecord(item)) {
      invalidItem = true;
      continue;
    }
    const priceCent = toFiniteNumber(item.priceCent);
    const sizePct = toFiniteNumber(item.sizePct);
    if (
      priceCent == null ||
      sizePct == null ||
      priceCent <= 0 ||
      priceCent > 100 ||
      sizePct <= 0 ||
      sizePct > 100
    ) {
      invalidItem = true;
      continue;
    }
    validRules.push({ priceCent, sizePct });
  }

  if (raw.length > 0 && validRules.length === 0) {
    invalidItem = true;
  }

  return { isArray: true, validRules, invalidItem };
}

function validatePositiveGridStopLossConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  graph: TradeFlowGraph,
  config: Record<string, unknown>,
) {
  const slEnabled = toBooleanish(config.slEnabled);
  const ptbStopLossEnabled = toBooleanish(config.ptbStopLossEnabled);
  const parsedSlRules = parsePositiveGridSlRules(config.slRules);
  const parsedPtbStopLossRules = parsePtbStopLossRules(config.ptbStopLossRules);
  const hasSlRules = parsedSlRules.validRules.length > 0;
  const effectiveClassicSlEnabled = slEnabled === true || hasSlRules;
  const slPrice = resolveConfiguredBinaryPrice(config.slPriceCent, config.slPrice);

  if (config.slEnabled != null && slEnabled == null) {
    pushNodeError(
      issues,
      node,
      "invalid_sl_enabled",
      "action.place_order slEnabled must be boolean (true/false).",
    );
  }
  if (config.ptbStopLossEnabled != null && ptbStopLossEnabled == null) {
    pushNodeError(
      issues,
      node,
      "invalid_ptb_stop_loss_enabled",
      "action.place_order ptbStopLossEnabled must be boolean (true/false).",
    );
  }
  if (parsedSlRules.isArray && parsedSlRules.validRules.length > 5) {
    pushNodeError(
      issues,
      node,
      "invalid_sl_rules_length",
      "action.place_order slRules cannot contain more than 5 entries.",
    );
  }
  if (parsedSlRules.invalidItem) {
    pushNodeError(
      issues,
      node,
      "invalid_sl_rules",
      "action.place_order slRules entries must provide priceCent in (0, 100] and sizePct in (0, 100].",
    );
  }
  if (hasSlRules) {
    const slRulesSum = parsedSlRules.validRules.reduce(
      (sum, item) => sum + item.sizePct,
      0,
    );
    if (Math.abs(slRulesSum - 100) > 0.000001) {
      pushNodeError(
        issues,
        node,
        "invalid_sl_rules_sum",
        "action.place_order slRules total sizePct must equal 100.",
      );
    }
    for (let index = 1; index < parsedSlRules.validRules.length; index += 1) {
      if (
        parsedSlRules.validRules[index - 1].priceCent <=
        parsedSlRules.validRules[index].priceCent
      ) {
        pushNodeError(
          issues,
          node,
          "invalid_sl_rules_order",
          "action.place_order slRules priceCent values must be strictly decreasing.",
        );
        break;
      }
    }
  }
  if (effectiveClassicSlEnabled && slEnabled === true && !slPrice.provided && !hasSlRules) {
    pushNodeError(
      issues,
      node,
      "missing_sl_price",
      "action.place_order slEnabled requires slPriceCent (or legacy slPrice).",
    );
  } else if (
    effectiveClassicSlEnabled &&
    !hasSlRules &&
    slPrice.provided &&
    slPrice.value == null
  ) {
    pushNodeError(
      issues,
      node,
      "invalid_sl_price",
      "action.place_order slPriceCent must be in (0, 100] or legacy slPrice must be in (0, 1].",
    );
  }

  const slTriggerPriceMode =
    typeof config.slTriggerPriceMode === "string" ? config.slTriggerPriceMode : null;
  if (effectiveClassicSlEnabled && slTriggerPriceMode != null) {
    const validModes = [
      "best_bid",
      "composite",
      "composite_safe",
      "composite_fast",
      "last_trade",
    ];
    if (!validModes.includes(slTriggerPriceMode)) {
      pushNodeError(
        issues,
        node,
        "invalid_sl_trigger_price_mode",
        "action.place_order slTriggerPriceMode must be one of: best_bid, composite, composite_safe, composite_fast, last_trade.",
      );
    }
  }

  validateActionPlaceOrderPtbStopLossConfig(issues, node, config, {
    side: "buy",
    graphMarketSlug: String(graph.context.marketSlug ?? "").trim(),
    hasResolveMarketNode: graph.nodes.some(
      (candidate) => candidate.type === "action.resolve_market",
    ),
    hasUpstreamMarketPriceAutoScope:
      findFirstUpstreamMarketPriceConfig(node.key, graph)?.marketMode === "auto_scope",
    ptbStopLossEnabled,
    parsedPtbStopLossRules,
  });

  const ptbStopLossTimeDecayMode = toTrimmedString(
    config.ptbStopLossTimeDecayMode,
  ).toLowerCase();
  if (
    config.ptbStopLossTimeDecayMode != null &&
    ptbStopLossTimeDecayMode !== "none" &&
    ptbStopLossTimeDecayMode !== "tighten" &&
    ptbStopLossTimeDecayMode !== "relax"
  ) {
    pushNodeError(
      issues,
      node,
      "invalid_ptb_stop_loss_time_decay_mode",
      "action.place_order ptbStopLossTimeDecayMode must be none, tighten, or relax.",
    );
  }
  if (config.ptbStopLossTimeDecayMode != null && ptbStopLossEnabled !== true) {
    pushNodeError(
      issues,
      node,
      "ptb_stop_loss_time_decay_mode_requires_ptb_stop_loss",
      "action.place_order ptbStopLossTimeDecayMode requires ptbStopLossEnabled=true.",
    );
  }
}

function boolValue(
  grid: Record<string, unknown>,
  config: Record<string, unknown>,
  key: string,
  fallback: boolean,
): boolean {
  return toBooleanish(grid[key] ?? config[key]) ?? fallback;
}

function validatePositiveGridBoolean(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  grid: Record<string, unknown>,
  key: string,
  code: string,
) {
  if (grid[key] != null && toBooleanish(grid[key]) == null) {
    pushNodeError(
      issues,
      node,
      code,
      `positiveQuantityFlipGrid ${key} must be boolean.`,
    );
  }
}

function validatePositiveGridCycleWindow(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  graph: TradeFlowGraph,
  config: Record<string, unknown>,
  grid: Record<string, unknown>,
) {
  const mode = toTrimmedString(grid.cycleWindowMode).toLowerCase();
  if (!mode) return;
  if (
    mode !== "off" &&
    mode !== "first" &&
    mode !== "last" &&
    mode !== "custom_range"
  ) {
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_cycle_window_mode",
      "positiveQuantityFlipGrid cycleWindowMode must be off, first, last, or custom_range.",
    );
    return;
  }
  if (mode === "off") return;
  const marketDurationSec = inferPositiveGridMarketDurationSec(
    node,
    graph,
    config,
  );
  if (mode === "first" || mode === "last") {
    const secs = toFiniteNumber(grid.cycleWindowSecs);
    if (!(secs != null && Number.isInteger(secs) && secs > 0)) {
      pushNodeError(
        issues,
        node,
        "invalid_positive_grid_cycle_window_secs",
        "positiveQuantityFlipGrid cycleWindowSecs must be an integer > 0.",
      );
    }
    return;
  }
  const startSec = toFiniteNumber(grid.cycleWindowStartSec);
  const endSec = toFiniteNumber(grid.cycleWindowEndSec);
  if (!(startSec != null && Number.isInteger(startSec) && startSec >= 0)) {
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_cycle_window_start_sec",
      "positiveQuantityFlipGrid cycleWindowStartSec must be an integer >= 0.",
    );
  }
  if (!(endSec != null && Number.isInteger(endSec) && endSec > 0)) {
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_cycle_window_end_sec",
      "positiveQuantityFlipGrid cycleWindowEndSec must be an integer > 0.",
    );
  }
  if (startSec != null && endSec != null && startSec >= endSec) {
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_cycle_window_range",
      "positiveQuantityFlipGrid custom_range requires cycleWindowStartSec < cycleWindowEndSec.",
    );
  }
  if (endSec != null && endSec > marketDurationSec) {
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_cycle_window_duration",
      "positiveQuantityFlipGrid custom_range end must fit inside the market duration.",
    );
  }
}

export function validatePositiveQuantityFlipGridConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  graph: TradeFlowGraph,
  config: Record<string, unknown>,
  side: string,
  executionMode: string,
) {
  const modeRaw = toTrimmedString(config.mode).toLowerCase();
  const positiveGridModeLabel =
    modeRaw === "positive_flip_pairlock_compression_v1"
      ? "positive_flip_pairlock_compression_v1"
      : "positive_quantity_flip_grid_v1";
  if (side !== "buy")
    pushNodeError(
      issues,
      node,
      "positive_grid_requires_buy_side",
      `action.place_order ${positiveGridModeLabel} only supports side=buy.`,
    );
  if (executionMode !== "market" && executionMode !== "limit")
    pushNodeError(
      issues,
      node,
      "positive_grid_requires_supported_execution",
      `action.place_order ${positiveGridModeLabel} only supports executionMode=market or limit.`,
    );
  if (
    toTrimmedString(config.kind).toLowerCase() &&
    toTrimmedString(config.kind).toLowerCase() !== "immediate"
  )
    pushNodeError(
      issues,
      node,
      "positive_grid_requires_immediate_kind",
      `action.place_order ${positiveGridModeLabel} only supports kind=immediate.`,
    );

  const bindingMode = findUniqueUpstreamMarketPriceBindingMode(node.key, graph);
  if (bindingMode !== "positive_quantity_flip_grid_only") {
    pushNodeError(
      issues,
      node,
      "positive_grid_requires_positive_binding_trigger",
      `action.place_order ${positiveGridModeLabel} requires upstream trigger.market_price bindingMode=positive_quantity_flip_grid_only.`,
    );
  }

  const parsed = parseOptionalJsonObject(config.positiveQuantityFlipGrid);
  if (parsed === "invalid") {
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_json",
      "action.place_order positiveQuantityFlipGrid must be a JSON object.",
    );
    return;
  }
  const grid = parsed ?? {};
  const pairlockCompressionMode =
    modeRaw === "positive_flip_pairlock_compression_v1";
  const numberValue = (key: string, fallback: number) =>
    toFiniteNumber(grid[key] ?? config[key]) ?? fallback;
  const baseBuyUsdc = numberValue("baseBuyUsdc", pairlockCompressionMode ? 2 : 1);
  const minMarketableBuyUsdc = numberValue("minMarketableBuyUsdc", 1.05);
  const entryBandMinCent = numberValue("entryBandMinCent", pairlockCompressionMode ? 52 : 50);
  const entryBandMaxCent = numberValue("entryBandMaxCent", pairlockCompressionMode ? 58 : 60);
  const exitPriceForSizingCent = numberValue("exitPriceForSizingCent", 98);
  const sizingPriceBufferCent = numberValue("sizingPriceBufferCent", pairlockCompressionMode ? 1 : 3);
  const partialRecoveryMinLossReductionUsdc = numberValue(
    "partialRecoveryMinLossReductionUsdc",
    0.1,
  );
  const partialRecoveryBalanceReserveUsdc = numberValue(
    "partialRecoveryBalanceReserveUsdc",
    1,
  );
  const partialRecoveryMaxBuyRaw =
    grid.partialRecoveryMaxBuyUsdc ?? config.partialRecoveryMaxBuyUsdc;
  const partialRecoveryMaxBuyProvided =
    partialRecoveryMaxBuyRaw != null &&
    toTrimmedString(partialRecoveryMaxBuyRaw) !== "";
  const partialRecoveryMaxBuyUsdc = toFiniteNumber(partialRecoveryMaxBuyRaw);
  const quantitySizingMode =
    toTrimmedString(grid.quantitySizingMode).toLowerCase() || "profit_target";
  const inventoryBalanceLeadQty = numberValue("inventoryBalanceLeadQty", 0);
  const minPositiveProfitUsdc = numberValue("minPositiveProfitUsdc", pairlockCompressionMode ? 0.05 : 1);
  const minSellNetProfitUsdc = numberValue("minSellNetProfitUsdc", pairlockCompressionMode ? 0.05 : 1);
  const sellBidMinCent = numberValue("sellBidMinCent", pairlockCompressionMode ? 59 : 98);
  const maxSingleBuyRaw = grid.maxSingleBuyUsdc ?? config.maxSingleBuyUsdc;
  const maxSingleBuyProvided =
    maxSingleBuyRaw != null && toTrimmedString(maxSingleBuyRaw) !== "";
  const maxSingleBuyUsdc = maxSingleBuyProvided
    ? toFiniteNumber(maxSingleBuyRaw)
    : null;
  const maxTotalSpentRaw =
    grid.maxTotalSpentPerMarketUsdc ?? config.maxTotalSpentPerMarketUsdc;
  const maxTotalSpentProvided =
    maxTotalSpentRaw != null && toTrimmedString(maxTotalSpentRaw) !== "";
  const maxTotalSpentPerMarketUsdc = maxTotalSpentProvided
    ? toFiniteNumber(maxTotalSpentRaw)
    : null;
  const hardMaxPriceCent = numberValue("hardMaxPriceCent", entryBandMaxCent);
  const worstPriceCent = numberValue("worstPriceCent", hardMaxPriceCent);
  const rescueBuyMinPriceCent = numberValue(
    "rescueBuyMinPriceCent",
    hardMaxPriceCent,
  );
  const rescueBuyMaxPriceCent = numberValue("rescueBuyMaxPriceCent", 70);
  const startSec = numberValue("newGridBuyStartRemainingSec", 285);
  const gridEndSec = numberValue("newGridBuyEndRemainingSec", 90);
  const completionEndSec = numberValue(
    "positiveCompletionBuyEndRemainingSec",
    30,
  );
  const noNewBuyUnderSec = numberValue("noNewBuyUnderSec", 30);
  const cycleWindowMode = toTrimmedString(grid.cycleWindowMode).toLowerCase();
  const orderType =
    toTrimmedString(grid.orderType ?? config.orderType).toUpperCase() || "FAK";
  const pairlockCompressionEnabled = boolValue(
    grid,
    config,
    "pairlockCompressionEnabled",
    pairlockCompressionMode,
  );
  const targetPairlockProfitCent = numberValue("targetPairlockProfitCent", 5);
  const feeBufferCent = numberValue("feeBufferCent", 1);
  const maxPairCostCent = numberValue("maxPairCostCent", 94);
  const pairlockOrderType =
    toTrimmedString(grid.pairlockOrderType ?? config.pairlockOrderType).toUpperCase() || "FOK";
  const maxUnmergedExposureUsdc = numberValue("maxUnmergedExposureUsdc", 2);
  const minBasketProfitUsdc = numberValue("minBasketProfitUsdc", 0.06);
  const minDirectProfitUsdc = numberValue("minDirectProfitUsdc", 0.05);
  const executionFloorGuardEnabled = boolValue(
    grid,
    config,
    "executionFloorGuardEnabled",
    true,
  );
  const ptbGuardEnabled = boolValue(grid, config, "ptbGuardEnabled", false);
  const rescueBuyEnabled = boolValue(grid, config, "rescueBuyEnabled", false);

  if (baseBuyUsdc <= 0)
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_base_buy",
      "positiveQuantityFlipGrid baseBuyUsdc must be > 0.",
    );
  if (!(minMarketableBuyUsdc >= 1 && minMarketableBuyUsdc <= 100))
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_min_marketable_buy",
      "positiveQuantityFlipGrid minMarketableBuyUsdc must be between 1 and 100.",
    );
  if (
    !(
      entryBandMinCent > 0 &&
      entryBandMinCent < entryBandMaxCent &&
      entryBandMaxCent <= 100
    )
  )
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_entry_band",
      "positiveQuantityFlipGrid entry band must be in (0, 100] and min < max.",
    );
  if (!(hardMaxPriceCent <= entryBandMaxCent && hardMaxPriceCent <= 100))
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_hard_max",
      "positiveQuantityFlipGrid hardMaxPriceCent must be <= entryBandMaxCent and <= 100.",
    );
  if (!(worstPriceCent >= hardMaxPriceCent && worstPriceCent <= 100))
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_worst_price",
      "positiveQuantityFlipGrid worstPriceCent must be >= hardMaxPriceCent and <= 100.",
    );
  if (
    rescueBuyEnabled &&
    !(
      rescueBuyMinPriceCent >= entryBandMaxCent &&
      rescueBuyMinPriceCent < rescueBuyMaxPriceCent &&
      rescueBuyMaxPriceCent < exitPriceForSizingCent
    )
  )
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_rescue_max_price",
      "positiveQuantityFlipGrid rescue range must satisfy entryBandMaxCent <= rescueBuyMinPriceCent < rescueBuyMaxPriceCent < exitPriceForSizingCent.",
    );
  if (
    !(
      exitPriceForSizingCent > entryBandMaxCent && exitPriceForSizingCent <= 100
    )
  )
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_exit_price",
      "positiveQuantityFlipGrid exitPriceForSizingCent must be above entryBandMaxCent.",
    );
  if (!(sizingPriceBufferCent >= 0 && sizingPriceBufferCent <= 5))
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_sizing_price_buffer",
      "positiveQuantityFlipGrid sizingPriceBufferCent must be between 0 and 5.",
    );
  if (!(partialRecoveryMinLossReductionUsdc >= 0))
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_partial_recovery_min_loss",
      "positiveQuantityFlipGrid partialRecoveryMinLossReductionUsdc must be >= 0.",
    );
  if (!(partialRecoveryBalanceReserveUsdc >= 0))
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_partial_recovery_reserve",
      "positiveQuantityFlipGrid partialRecoveryBalanceReserveUsdc must be >= 0.",
    );
  if (
    partialRecoveryMaxBuyProvided &&
    !(partialRecoveryMaxBuyUsdc != null && partialRecoveryMaxBuyUsdc > 0)
  )
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_partial_recovery_max_buy",
      "positiveQuantityFlipGrid partialRecoveryMaxBuyUsdc must be > 0 when set.",
    );
  if (
    quantitySizingMode !== "profit_target" &&
    quantitySizingMode !== "inventory_balance" &&
    quantitySizingMode !== "fixed_usdc"
  )
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_quantity_sizing_mode",
      "positiveQuantityFlipGrid quantitySizingMode must be profit_target, inventory_balance, or fixed_usdc.",
    );
  if (
    quantitySizingMode === "fixed_usdc" &&
    modeRaw !== "positive_flip_pairlock_compression_v1"
  )
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_fixed_usdc_requires_pairlock",
      "positiveQuantityFlipGrid quantitySizingMode=fixed_usdc requires action.place_order mode=positive_flip_pairlock_compression_v1.",
    );
  if (!(inventoryBalanceLeadQty >= 0 && inventoryBalanceLeadQty <= 1000))
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_inventory_balance_lead_qty",
      "positiveQuantityFlipGrid inventoryBalanceLeadQty must be between 0 and 1000.",
    );
  if (!(minPositiveProfitUsdc > 0 && minSellNetProfitUsdc > 0))
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_profit_target",
      "positiveQuantityFlipGrid profit targets must be > 0.",
    );
  if (!(sellBidMinCent > entryBandMaxCent && sellBidMinCent <= 100))
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_sell_bid_min",
      "positiveQuantityFlipGrid sellBidMinCent must be above entryBandMaxCent and <= 100.",
    );
  if (
    maxSingleBuyProvided &&
    (maxSingleBuyUsdc == null || maxSingleBuyUsdc <= 0)
  )
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_max_single_buy",
      "positiveQuantityFlipGrid maxSingleBuyUsdc must be > 0 when set.",
    );
  if (
    maxSingleBuyProvided &&
    maxSingleBuyUsdc != null &&
    maxSingleBuyUsdc < baseBuyUsdc
  )
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_max_single_buy",
      "positiveQuantityFlipGrid maxSingleBuyUsdc must be >= baseBuyUsdc.",
    );
  if (
    maxTotalSpentProvided &&
    (maxTotalSpentPerMarketUsdc == null || maxTotalSpentPerMarketUsdc <= 0)
  )
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_max_total_spent",
      "positiveQuantityFlipGrid maxTotalSpentPerMarketUsdc must be > 0 when set.",
    );
  if (
    maxTotalSpentProvided &&
    maxTotalSpentPerMarketUsdc != null &&
    maxTotalSpentPerMarketUsdc < baseBuyUsdc
  )
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_max_total_spent",
      "positiveQuantityFlipGrid maxTotalSpentPerMarketUsdc must be >= baseBuyUsdc.",
    );
  if (cycleWindowMode) {
    if (
      !(
        gridEndSec > completionEndSec &&
        completionEndSec >= noNewBuyUnderSec &&
        noNewBuyUnderSec >= 0
      )
    )
      pushNodeError(
        issues,
        node,
        "invalid_positive_grid_timing",
        "positiveQuantityFlipGrid timing must satisfy gridEnd > completionEnd >= noNewBuy.",
      );
  } else if (
    !(
      startSec > gridEndSec &&
      gridEndSec > completionEndSec &&
      completionEndSec >= noNewBuyUnderSec
    )
  )
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_timing",
      "positiveQuantityFlipGrid timing must satisfy start > gridEnd > completionEnd >= noNewBuy.",
    );
  if (orderType !== "FAK" && orderType !== "IOC")
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_order_type",
      "positiveQuantityFlipGrid orderType must be FAK or IOC.",
    );
  if (pairlockCompressionEnabled) {
    if (!(targetPairlockProfitCent >= 0 && targetPairlockProfitCent < 100))
      pushNodeError(
        issues,
        node,
        "invalid_positive_grid_pairlock_target",
        "positiveQuantityFlipGrid targetPairlockProfitCent must be >= 0 and < 100.",
      );
    if (!(feeBufferCent >= 0 && feeBufferCent <= 10))
      pushNodeError(
        issues,
        node,
        "invalid_positive_grid_pairlock_fee_buffer",
        "positiveQuantityFlipGrid feeBufferCent must be between 0 and 10.",
      );
    if (!(maxPairCostCent > 0 && maxPairCostCent < 100))
      pushNodeError(
        issues,
        node,
        "invalid_positive_grid_pairlock_max_pair_cost",
        "positiveQuantityFlipGrid maxPairCostCent must be > 0 and < 100.",
      );
    if (
      pairlockOrderType !== "FOK" &&
      pairlockOrderType !== "FAK" &&
      pairlockOrderType !== "IOC"
    )
      pushNodeError(
        issues,
        node,
        "invalid_positive_grid_pairlock_order_type",
        "positiveQuantityFlipGrid pairlockOrderType must be FOK, FAK, or IOC.",
      );
    if (!(maxUnmergedExposureUsdc >= 0))
      pushNodeError(
        issues,
        node,
        "invalid_positive_grid_pairlock_unmerged_exposure",
        "positiveQuantityFlipGrid maxUnmergedExposureUsdc must be >= 0.",
      );
    if (!(minBasketProfitUsdc >= 0))
      pushNodeError(
        issues,
        node,
        "invalid_positive_grid_pairlock_basket_profit",
        "positiveQuantityFlipGrid minBasketProfitUsdc must be >= 0.",
      );
    if (!(minDirectProfitUsdc >= 0))
      pushNodeError(
        issues,
        node,
        "invalid_positive_grid_pairlock_direct_profit",
        "positiveQuantityFlipGrid minDirectProfitUsdc must be >= 0.",
      );
  }
  validatePositiveGridCycleWindow(issues, node, graph, config, grid);
  validateNoBuyRanges(issues, node, grid.noBuyRanges);
  validatePositiveGridBoolean(
    issues,
    node,
    grid,
    "pairlockCompressionEnabled",
    "invalid_positive_grid_pairlock_enabled",
  );
  validatePositiveGridBoolean(
    issues,
    node,
    grid,
    "depthGuardEnabled",
    "invalid_positive_grid_depth_guard",
  );
  validatePositiveGridBoolean(
    issues,
    node,
    grid,
    "executionFloorGuardEnabled",
    "invalid_positive_grid_execution_floor_guard",
  );
  validatePositiveGridBoolean(
    issues,
    node,
    grid,
    "triggerPriceGuardEnabled",
    "invalid_positive_grid_trigger_price_guard",
  );
  validatePositiveGridBoolean(
    issues,
    node,
    grid,
    "ptbGuardEnabled",
    "invalid_positive_grid_ptb_guard",
  );
  validatePositiveGridBoolean(
    issues,
    node,
    grid,
    "rescueBuyEnabled",
    "invalid_positive_grid_rescue_buy",
  );
  validatePositiveGridBoolean(
    issues,
    node,
    grid,
    "partialRecoveryEnabled",
    "invalid_positive_grid_partial_recovery_enabled",
  );
  validatePositiveGridBoolean(
    issues,
    node,
    grid,
    "partialRecoveryIgnoreMarketBudget",
    "invalid_positive_grid_partial_recovery_ignore_market_budget",
  );
  validatePositiveGridBoolean(
    issues,
    node,
    grid,
    "blockConsecutiveSameSideBuys",
    "invalid_positive_grid_block_consecutive_same_side_buys",
  );
  validatePositiveGridBoolean(
    issues,
    node,
    grid,
    "stopBuysAfterPairlockMerge",
    "invalid_positive_grid_stop_buys_after_pairlock_merge",
  );

  const executionFloorProvided =
    grid.executionFloorPriceCent != null &&
    toTrimmedString(grid.executionFloorPriceCent) !== "";
  const executionFloorPriceCent = toFiniteNumber(grid.executionFloorPriceCent);
  if (
    (executionFloorProvided || executionFloorGuardEnabled) &&
    executionFloorProvided &&
    !(
      executionFloorPriceCent != null &&
      executionFloorPriceCent > 0 &&
      executionFloorPriceCent <= hardMaxPriceCent
    )
  ) {
    pushNodeError(
      issues,
      node,
      "invalid_positive_grid_execution_floor_price",
      "positiveQuantityFlipGrid executionFloorPriceCent must be > 0 and <= hardMaxPriceCent.",
    );
  }

  if (ptbGuardEnabled) {
    const ptbMinDiff = toFiniteNumber(grid.ptbMinDiff);
    const ptbDiffUnit =
      toTrimmedString(grid.ptbDiffUnit).toLowerCase() || "usd";
    const ptbCurrentPriceSource =
      toTrimmedString(grid.ptbCurrentPriceSource).toLowerCase() || "chainlink";
    if (!(ptbMinDiff != null && ptbMinDiff > 0)) {
      pushNodeError(
        issues,
        node,
        "invalid_positive_grid_ptb_min_diff",
        "positiveQuantityFlipGrid ptbMinDiff must be > 0 when PTB guard is enabled.",
      );
    }
    const ptbRescueMinDiff = toFiniteNumber(grid.ptbRescueMinDiff);
    if (
      ptbRescueMinDiff != null &&
      !(ptbRescueMinDiff > 0)
    ) {
      pushNodeError(
        issues,
        node,
        "invalid_positive_grid_ptb_rescue_min_diff",
        "positiveQuantityFlipGrid ptbRescueMinDiff must be > 0 when set.",
      );
    }
    if (ptbDiffUnit !== "usd" && ptbDiffUnit !== "cent") {
      pushNodeError(
        issues,
        node,
        "invalid_positive_grid_ptb_unit",
        "positiveQuantityFlipGrid ptbDiffUnit must be usd or cent.",
      );
    }
    if (
      !["chainlink", "binance", "coinbase", "hyperliquid"].includes(
        ptbCurrentPriceSource,
      )
    ) {
      pushNodeError(
        issues,
        node,
        "invalid_positive_grid_ptb_current_source",
        "positiveQuantityFlipGrid ptbCurrentPriceSource must be chainlink, binance, coinbase, or hyperliquid.",
      );
    }
  }

  if (toBooleanish(config.buyFillLockEnabled) === true)
    pushNodeError(
      issues,
      node,
      "positive_grid_disallows_buy_fill_lock",
      `${positiveGridModeLabel} requires buyFillLockEnabled=false.`,
    );
  if (
    toBooleanish(config.tpEnabled) === true ||
    toBooleanish(config.autoSellOnWindowEnd) === true
  )
    pushNodeError(
      issues,
      node,
      "positive_grid_disallows_classic_exits",
      `${positiveGridModeLabel} requires TP and window-end sell to be disabled.`,
    );
  if (Array.isArray(config.tpRules) && config.tpRules.length > 0)
    pushNodeError(
      issues,
      node,
      "positive_grid_disallows_tp_rules",
      `${positiveGridModeLabel} does not use tpRules.`,
    );
  validatePositiveGridStopLossConfig(issues, node, graph, config);
  if (toBooleanish(config.postOnly) === true)
    pushNodeError(
      issues,
      node,
      "positive_grid_disallows_post_only",
      `${positiveGridModeLabel} requires postOnly=false.`,
    );
  if (toBooleanish(config.priceToBeatGuardEnabled) === true)
    pushNodeError(
      issues,
      node,
      "positive_grid_disallows_generic_ptb",
      `${positiveGridModeLabel} uses positiveQuantityFlipGrid.ptbGuardEnabled instead of generic priceToBeatGuardEnabled.`,
    );
}
