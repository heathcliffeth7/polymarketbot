import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import {
  isPtbCurrentPriceSource,
  isPtbMode,
  normalizePtbMode,
  type PtbMode,
} from '@/lib/trade-flow-config-mappers/ptb-modes';
import {
  findUniqueUpstreamMarketPriceTrigger,
  hasUpstreamAutoScopeMarketTrigger,
  hasUpstreamTriggerWithTriggerPrice,
  hasUpstreamBuyPlaceOrderNode,
  isGenericPlaceOrderPresetConfig,
  resolveUpstreamFixedTriggerMarket,
} from './graph';
import {
  isRecord,
  RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME,
  isSupportedMarketPriceTriggerCondition,
  resolveConfiguredBinaryPrice,
  toBooleanish,
  toFiniteNumber,
  toTrimmedString,
} from './shared';
import { validateActionPlaceOrderExecutionFloorConfig } from './validation-action-place-order-execution-floor';
import { validateActionPlaceOrderAutoTuneConfig } from './validation-action-place-order-auto-tune';
import { validateActionPlaceOrderBuyFillLockConfig } from './validation-action-place-order-buy-fill-lock';
import {
  isDcaLivePlaceOrderConfig,
  validateActionPlaceOrderDcaLiveConfig,
} from './validation-action-place-order-dca';
import { validateActionPlaceOrderPairLockConfig } from './validation-action-place-order-pair';
import { validateActionPlaceOrderPtbStopLossBumpConfig } from './validation-action-place-order-ptb-bump';
import { validateActionPlaceOrderPtbIvTimeRulesConfig } from './validation-action-place-order-ptb-iv-time-rules';
import { parsePtbStopLossRules, validateActionPlaceOrderPtbStopLossConfig } from './validation-action-place-order-ptb-stop-loss';
import { validateActionPlaceOrderPtbV2Config } from './validation-action-place-order-ptb-v2';
import { pushNodeError } from './validation-core';

interface ParsedExitLadderRule {
  priceCent: number;
  sizePct: number;
}

function collectUpstreamRuntimePtbAssets(nodeKey: string, graph: TradeFlowGraph): Set<string> {
  const nodeMap = new Map(graph.nodes.map((candidate) => [candidate.key, candidate]));
  const incomingByTarget = new Map<string, string[]>();
  for (const edge of graph.edges) {
    const incoming = incomingByTarget.get(edge.target) ?? [];
    incoming.push(edge.source);
    incomingByTarget.set(edge.target, incoming);
  }

  const assets = new Set<string>();
  const visited = new Set<string>();
  const queue = [nodeKey];
  while (queue.length > 0) {
    const current = queue.shift() as string;
    if (visited.has(current)) continue;
    visited.add(current);

    for (const sourceKey of incomingByTarget.get(current) ?? []) {
      const sourceNode = nodeMap.get(sourceKey);
      if (!sourceNode) continue;
      const config = isRecord(sourceNode.config) ? sourceNode.config : {};

      if (
        sourceNode.type === 'trigger.market_price' &&
        toTrimmedString(config.marketMode).toLowerCase() === 'auto_scope'
      ) {
        const marketScope = toTrimmedString(config.marketScope).toLowerCase();
        const resolvedScope =
          marketScope && RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[marketScope]
            ? RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[marketScope]
            : null;
        if (resolvedScope?.asset) assets.add(resolvedScope.asset);
      }

      if (sourceNode.type === 'action.resolve_market') {
        const marketScope = toTrimmedString(config.marketScope).toLowerCase();
        const resolvedScope =
          marketScope && RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[marketScope]
            ? RESOLVE_MARKET_SCOPE_TO_ASSET_TIMEFRAME[marketScope]
            : null;
        const asset = toTrimmedString(config.asset).toLowerCase() || resolvedScope?.asset || '';
        if (asset) assets.add(asset);
      }

      queue.push(sourceKey);
    }
  }

  return assets;
}

function parseExitLadderRules(raw: unknown): {
  isArray: boolean;
  validRules: ParsedExitLadderRule[];
  invalidItem: boolean;
} {
  if (!Array.isArray(raw)) {
    return { isArray: false, validRules: [], invalidItem: false };
  }

  const validRules: ParsedExitLadderRule[] = [];
  let invalidItem = false;
  for (const item of raw) {
    if (!isRecord(item)) {
      invalidItem = true;
      continue;
    }
    const resolvedPrice = resolveConfiguredBinaryPrice(item.priceCent, item.price);
    const sizePct = toFiniteNumber(item.sizePct);
    if (
      !resolvedPrice.provided ||
      resolvedPrice.value == null ||
      sizePct == null ||
      sizePct <= 0 ||
      sizePct > 100
    ) {
      invalidItem = true;
      continue;
    }
    validRules.push({ priceCent: resolvedPrice.value * 100, sizePct });
  }

  if (raw.length > 0 && validRules.length === 0) {
    invalidItem = true;
  }

  return { isArray: true, validRules, invalidItem };
}

function parseTimeExitRules(raw: unknown): {
  isArray: boolean;
  validRules: Array<{ elapsedMinutes: number; remainingPct: number }>;
  invalidItem: boolean;
} {
  if (!Array.isArray(raw)) {
    return { isArray: false, validRules: [], invalidItem: false };
  }

  const validRules: Array<{ elapsedMinutes: number; remainingPct: number }> = [];
  let invalidItem = false;
  for (const item of raw) {
    if (!isRecord(item)) {
      invalidItem = true;
      continue;
    }
    const elapsedMinutes = toFiniteNumber(item.elapsedMinutes);
    const remainingPct = toFiniteNumber(item.remainingPct);
    if (
      elapsedMinutes == null ||
      !Number.isInteger(elapsedMinutes) ||
      elapsedMinutes <= 0 ||
      remainingPct == null ||
      remainingPct <= 0 ||
      remainingPct > 100
    ) {
      invalidItem = true;
      continue;
    }
    validRules.push({ elapsedMinutes, remainingPct });
  }

  if (raw.length > 0 && validRules.length === 0) {
    invalidItem = true;
  }

  return { isArray: true, validRules, invalidItem };
}

export function validateActionPlaceOrderConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  graph: TradeFlowGraph
) {
  const config = isRecord(node.config) ? node.config : {};
  const mode = toTrimmedString(config.mode).toLowerCase();
  const isDcaLiveMode = isDcaLivePlaceOrderConfig(config);
  const pairLockStrategy = toTrimmedString(config.pairLockStrategy).toLowerCase();
  const allowsZeroReentryMaxAttempts =
    mode === 'pair_lock' && pairLockStrategy === 'biased_hedge_v1';
  const graphSourceTradeId = toFiniteNumber(graph.context.sourceTradeId);
  const graphMarketSlug = String(graph.context.marketSlug ?? '').trim();
  const graphTokenId = String(graph.context.tokenId ?? '').trim();
  const hasResolveMarketNode = graph.nodes.some((candidate) => candidate.type === 'action.resolve_market');
  const hasUpstreamMarketPriceAutoScope = hasUpstreamAutoScopeMarketTrigger(node.key, graph);
  const isGenericPresetPlaceOrder = isGenericPlaceOrderPresetConfig(config);
  if (isGenericPresetPlaceOrder) {
    const upstreamFixedMarket = resolveUpstreamFixedTriggerMarket(node.key, graph);
    const hasUniqueUpstreamFixedOutcome =
      upstreamFixedMarket.kind === 'single' &&
      upstreamFixedMarket.marketSlug != null &&
      upstreamFixedMarket.outcomeKind === 'single' &&
      upstreamFixedMarket.tokenId != null &&
      upstreamFixedMarket.outcomeLabel != null;
    if (!hasUniqueUpstreamFixedOutcome && !hasUpstreamMarketPriceAutoScope) {
      pushNodeError(
        issues,
        node,
        'missing_unique_upstream_fixed_trigger_seed',
        'Preset action.place_order requires either exactly one upstream fixed trigger.market_price outcome or an upstream auto_scope trigger.market_price runtime binding to resolve market/token/outcome.'
      );
    }
  }

  const sourceTradeId = toFiniteNumber(config.sourceTradeId);
  const side = String(config.side ?? '').trim().toLowerCase();
  const effectiveSourceTradeId = sourceTradeId ?? graphSourceTradeId ?? 0;
  const allowBuyAutoScopeSourceTrade = side === 'buy';
  const hasUpstreamBuyOrder = hasUpstreamBuyPlaceOrderNode(node.key, graph);
  if (effectiveSourceTradeId <= 0 && !allowBuyAutoScopeSourceTrade && !hasUpstreamBuyOrder) {
    pushNodeError(
      issues,
      node,
      'missing_source_trade_id',
      'action.place_order requires sourceTradeId in node config or graph context.'
    );
  }
  if (
    !String(config.marketSlug ?? graphMarketSlug).trim() &&
    !hasResolveMarketNode &&
    !(side === 'buy' && hasUpstreamMarketPriceAutoScope) &&
    !isDcaLiveMode
  ) {
    pushNodeError(
      issues,
      node,
      'missing_market_slug',
      'action.place_order requires marketSlug in node config/graph context. Buy auto_scope zincirinde runtime tetikten de cozulebilir.'
    );
  }
  if (
    !String(config.tokenId ?? graphTokenId).trim() &&
    !hasResolveMarketNode &&
    !(side === 'buy' && hasUpstreamMarketPriceAutoScope) &&
    !isDcaLiveMode
  ) {
    pushNodeError(
      issues,
      node,
      'missing_token_id',
      'action.place_order requires tokenId in node config/graph context. Buy auto_scope zincirinde runtime tetikten de cozulebilir.'
    );
  }
  if (!side) {
    pushNodeError(issues, node, 'missing_side', 'action.place_order side is required (buy or sell).');
  } else if (side !== 'buy' && side !== 'sell') {
    pushNodeError(issues, node, 'invalid_side', 'action.place_order side must be buy or sell.');
  }

  const executionMode = String(config.executionMode ?? '').trim().toLowerCase();
  if (!executionMode) {
    pushNodeError(
      issues,
      node,
      'missing_execution_mode',
      'action.place_order executionMode is required (market or limit).'
    );
  } else if (executionMode !== 'market' && executionMode !== 'limit') {
    pushNodeError(
      issues,
      node,
      'invalid_execution_mode',
      'action.place_order executionMode must be market or limit.'
    );
  }
  validateActionPlaceOrderPairLockConfig(issues, node, graph, config, side, executionMode);
  validateActionPlaceOrderDcaLiveConfig(issues, node, graph, config, side, executionMode);

  const maxTriggers = toFiniteNumber(config.maxTriggers);
  if (maxTriggers != null && (maxTriggers < 1 || maxTriggers > 20)) {
    pushNodeError(
      issues,
      node,
      'invalid_max_triggers',
      'action.place_order maxTriggers must be in [1, 20].'
    );
  }

  const sizeModeRaw = String(config.sizeMode ?? '').trim().toLowerCase();
  if (sizeModeRaw && sizeModeRaw !== 'usdc' && sizeModeRaw !== 'pct' && sizeModeRaw !== 'shares') {
    pushNodeError(
      issues,
      node,
      'invalid_size_mode',
      'action.place_order sizeMode must be usdc, pct, or shares.'
    );
  }

  const triggerSizesRaw = config.triggerSizes;
  const triggerSizes: number[] = [];
  let triggerSizesInvalid = false;
  if (triggerSizesRaw != null) {
    if (!Array.isArray(triggerSizesRaw)) {
      pushNodeError(
        issues,
        node,
        'invalid_trigger_sizes',
        'action.place_order triggerSizes must be an array.'
      );
    } else {
      for (const item of triggerSizesRaw) {
        const value = toFiniteNumber(item);
        if (value == null || value <= 0) {
          triggerSizesInvalid = true;
          continue;
        }
        triggerSizes.push(value);
      }
      if (triggerSizesRaw.length > 0 && triggerSizes.length === 0) {
        triggerSizesInvalid = true;
      }
      if (triggerSizesInvalid) {
        pushNodeError(
          issues,
          node,
          'invalid_trigger_sizes',
          'action.place_order triggerSizes entries must be finite numbers > 0.'
        );
      }
      if (
        maxTriggers != null &&
        maxTriggers >= 1 &&
        triggerSizes.length > 0 &&
        triggerSizes.length > Math.floor(maxTriggers)
      ) {
        pushNodeError(
          issues,
          node,
          'invalid_trigger_sizes_length',
          'action.place_order triggerSizes length cannot exceed maxTriggers.'
        );
      }
      if (sizeModeRaw === 'pct' && triggerSizes.length > 0) {
        const triggerSizesSum = triggerSizes.reduce((sum, value) => sum + value, 0);
        if (triggerSizesSum > 100.000001) {
          pushNodeError(
            issues,
            node,
            'invalid_trigger_sizes_sum_pct',
            'action.place_order pct triggerSizes total must be <= 100.'
          );
        }
      }
    }
  }

  const sizeUsdc = toFiniteNumber(config.sizeUsdc ?? config.targetNotionalUsdc);
  const sizePct = toFiniteNumber(config.sizePct ?? config.sizePercent);
  const targetQty = toFiniteNumber(config.targetQty ?? config.target_qty);
  const hasTriggerSizes = triggerSizes.length > 0;
  const usesPctSizing = sizeModeRaw === 'pct' || (!hasTriggerSizes && sizeUsdc == null && sizePct != null);
  const usesShareSizing = sizeModeRaw === 'shares' || (!hasTriggerSizes && sizeUsdc == null && targetQty != null);
  if (!hasTriggerSizes) {
    if (usesPctSizing) {
      if (sizePct == null || sizePct <= 0 || sizePct > 100) {
        pushNodeError(
          issues,
          node,
          'invalid_size_pct',
          'action.place_order sizePct must be in (0, 100].'
        );
      }
    } else if (usesShareSizing) {
      if (targetQty == null || targetQty <= 0) {
        pushNodeError(
          issues,
          node,
          'invalid_target_qty',
          'action.place_order targetQty must be > 0 when sizeMode is shares.'
        );
      }
    } else if (!isDcaLiveMode && (sizeUsdc == null || sizeUsdc <= 0)) {
      pushNodeError(
        issues,
        node,
        'invalid_size',
        'action.place_order requires sizeUsdc/targetNotionalUsdc > 0 (or sizePct in pct mode).'
      );
    }
  }
  if (side === 'buy' && usesPctSizing && effectiveSourceTradeId <= 0 && !hasUpstreamBuyOrder) {
    pushNodeError(
      issues,
      node,
      'pct_buy_requires_source_trade',
      'action.place_order buy + pct sizing icin sourceTradeId gerekir. Buy auto source trade yalnizca usdc sizing ile desteklenir.'
    );
  }

  const minDistance = toFiniteNumber(config.minPriceDistanceCent);
  if (minDistance != null && minDistance <= 0) {
    pushNodeError(
      issues,
      node,
      'invalid_min_price_distance',
      'action.place_order minPriceDistanceCent must be > 0.'
    );
  }

  const triggerCondition = config.triggerCondition;
  if (
    triggerCondition != null &&
    !isSupportedMarketPriceTriggerCondition(triggerCondition)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_trigger_condition',
      'action.place_order triggerCondition must be cross_above, cross_below, level_above, or level_below.'
    );
  }

  const tpEnabled = toBooleanish(config.tpEnabled);
  const slEnabled = toBooleanish(config.slEnabled);
  const ptbStopLossEnabled = toBooleanish(config.ptbStopLossEnabled);
  const tpPrice = resolveConfiguredBinaryPrice(config.tpPriceCent, config.tpPrice);
  const slPrice = resolveConfiguredBinaryPrice(config.slPriceCent, config.slPrice);
  const parsedTpRules = parseExitLadderRules(config.tpRules);
  const parsedSlRules = parseExitLadderRules(config.slRules);
  const parsedPtbStopLossRules = parsePtbStopLossRules(config.ptbStopLossRules);
  const parsedTimeExitRules = parseTimeExitRules(config.timeExitRules);
  const hasTpRules = parsedTpRules.validRules.length > 0;
  const hasSlRules = parsedSlRules.validRules.length > 0;
  const hasPtbStopLossRules = parsedPtbStopLossRules.validRules.length > 0;
  const effectiveTpEnabled = tpEnabled === true || hasTpRules;
  const effectiveSlEnabled = slEnabled === true || hasSlRules;
  const effectiveClassicSlEnabled =
    hasSlRules || (slEnabled === true && (slPrice.provided || ptbStopLossEnabled !== true));
  const effectiveAnyStopLossEnabled =
    effectiveSlEnabled || ptbStopLossEnabled === true || hasPtbStopLossRules;
  const stagedSlReentryOnlyAfterAllStages = toBooleanish(
    config.stagedSlReentryOnlyAfterAllStages
  );

  if (config.tpEnabled != null && tpEnabled == null) {
    pushNodeError(
      issues,
      node,
      'invalid_tp_enabled',
      'action.place_order tpEnabled must be boolean (true/false).'
    );
  }
  if (config.slEnabled != null && slEnabled == null) {
    pushNodeError(
      issues,
      node,
      'invalid_sl_enabled',
      'action.place_order slEnabled must be boolean (true/false).'
    );
  }
  if (config.ptbStopLossEnabled != null && ptbStopLossEnabled == null) {
    pushNodeError(
      issues,
      node,
      'invalid_ptb_stop_loss_enabled',
      'action.place_order ptbStopLossEnabled must be boolean (true/false).'
    );
  }
  if (
    config.stagedSlReentryOnlyAfterAllStages != null &&
    stagedSlReentryOnlyAfterAllStages == null
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_staged_sl_reentry_only_after_all_stages',
      'action.place_order stagedSlReentryOnlyAfterAllStages must be boolean (true/false).'
    );
  }
  if (tpEnabled === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_tp_side',
      'action.place_order tpEnabled is only valid for side=buy.'
    );
  }
  if (slEnabled === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_sl_side',
      'action.place_order slEnabled is only valid for side=buy.'
    );
  }
  if (parsedTpRules.isArray && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_tp_rules_side',
      'action.place_order tpRules is only valid for side=buy.'
    );
  }
  if (parsedSlRules.isArray && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_sl_rules_side',
      'action.place_order slRules is only valid for side=buy.'
    );
  }
  if (parsedTimeExitRules.isArray && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_time_exit_rules_side',
      'action.place_order timeExitRules is only valid for side=buy.'
    );
  }
  if (parsedTpRules.isArray && parsedTpRules.validRules.length > 5) {
    pushNodeError(
      issues,
      node,
      'invalid_tp_rules_length',
      'action.place_order tpRules cannot contain more than 5 entries.'
    );
  }
  if (parsedSlRules.isArray && parsedSlRules.validRules.length > 5) {
    pushNodeError(
      issues,
      node,
      'invalid_sl_rules_length',
      'action.place_order slRules cannot contain more than 5 entries.'
    );
  }
  if (parsedTimeExitRules.isArray && parsedTimeExitRules.validRules.length > 5) {
    pushNodeError(
      issues,
      node,
      'invalid_time_exit_rules_length',
      'action.place_order timeExitRules cannot contain more than 5 entries.'
    );
  }
  if (parsedTpRules.invalidItem) {
    pushNodeError(
      issues,
      node,
      'invalid_tp_rules',
      'action.place_order tpRules entries must provide priceCent in (0, 100] and sizePct in (0, 100].'
    );
  }
  if (parsedSlRules.invalidItem) {
    pushNodeError(
      issues,
      node,
      'invalid_sl_rules',
      'action.place_order slRules entries must provide priceCent in (0, 100] and sizePct in (0, 100].'
    );
  }
  if (parsedTimeExitRules.invalidItem) {
    pushNodeError(
      issues,
      node,
      'invalid_time_exit_rules',
      'action.place_order timeExitRules entries must provide integer elapsedMinutes > 0 and remainingPct in (0, 100].'
    );
  }
  if (hasTpRules) {
    const tpRulesSum = parsedTpRules.validRules.reduce((sum, item) => sum + item.sizePct, 0);
    if (Math.abs(tpRulesSum - 100) > 0.000001) {
      pushNodeError(
        issues,
        node,
        'invalid_tp_rules_sum',
        'action.place_order tpRules total sizePct must equal 100.'
      );
    }
    for (let index = 1; index < parsedTpRules.validRules.length; index += 1) {
      if (parsedTpRules.validRules[index - 1].priceCent >= parsedTpRules.validRules[index].priceCent) {
        pushNodeError(
          issues,
          node,
          'invalid_tp_rules_order',
          'action.place_order tpRules priceCent values must be strictly increasing.'
        );
        break;
      }
    }
  }
  if (hasSlRules) {
    const slRulesSum = parsedSlRules.validRules.reduce((sum, item) => sum + item.sizePct, 0);
    if (Math.abs(slRulesSum - 100) > 0.000001) {
      pushNodeError(
        issues,
        node,
        'invalid_sl_rules_sum',
        'action.place_order slRules total sizePct must equal 100.'
      );
    }
    for (let index = 1; index < parsedSlRules.validRules.length; index += 1) {
      if (parsedSlRules.validRules[index - 1].priceCent <= parsedSlRules.validRules[index].priceCent) {
        pushNodeError(
          issues,
          node,
          'invalid_sl_rules_order',
          'action.place_order slRules priceCent values must be strictly decreasing.'
        );
        break;
      }
    }
  }
  if (parsedTimeExitRules.validRules.length > 0) {
    for (let index = 1; index < parsedTimeExitRules.validRules.length; index += 1) {
      if (
        parsedTimeExitRules.validRules[index - 1].elapsedMinutes >=
        parsedTimeExitRules.validRules[index].elapsedMinutes
      ) {
        pushNodeError(
          issues,
          node,
          'invalid_time_exit_rules_order',
          'action.place_order timeExitRules elapsedMinutes values must be strictly increasing.'
        );
        break;
      }
    }
  }
  if (tpEnabled === true && !tpPrice.provided && !hasTpRules) {
    pushNodeError(
      issues,
      node,
      'missing_tp_price',
      'action.place_order tpEnabled requires tpPriceCent (or legacy tpPrice).'
    );
  } else if (!hasTpRules && tpPrice.provided && tpPrice.value == null) {
    pushNodeError(
      issues,
      node,
      'invalid_tp_price',
      'action.place_order tpPriceCent must be in (0, 100] or legacy tpPrice must be in (0, 1].'
    );
  }
  if (effectiveClassicSlEnabled && slEnabled === true && !slPrice.provided && !hasSlRules) {
    pushNodeError(
      issues,
      node,
      'missing_sl_price',
      'action.place_order slEnabled requires slPriceCent (or legacy slPrice).'
    );
  } else if (effectiveClassicSlEnabled && !hasSlRules && slPrice.provided && slPrice.value == null) {
    pushNodeError(
      issues,
      node,
      'invalid_sl_price',
      'action.place_order slPriceCent must be in (0, 100] or legacy slPrice must be in (0, 1].'
    );
  }
  validateActionPlaceOrderPtbStopLossConfig(issues, node, config, {
    side,
    graphMarketSlug,
    hasResolveMarketNode,
    hasUpstreamMarketPriceAutoScope,
    ptbStopLossEnabled,
    parsedPtbStopLossRules,
  });
  const slTriggerPriceMode =
    typeof config.slTriggerPriceMode === 'string' ? config.slTriggerPriceMode : null;
  if (effectiveClassicSlEnabled && slTriggerPriceMode != null) {
    const validModes = ['best_bid', 'composite', 'composite_safe', 'composite_fast', 'last_trade'];
    if (!validModes.includes(slTriggerPriceMode)) {
      pushNodeError(
        issues,
        node,
        'invalid_sl_trigger_price_mode',
        'action.place_order slTriggerPriceMode must be one of: best_bid, composite, composite_safe, composite_fast, last_trade.'
      );
    }
  }
  const reenterOnSlHit = toBooleanish(config.reenterOnSlHit);
  if (config.reenterOnSlHit != null && reenterOnSlHit == null) {
    pushNodeError(
      issues,
      node,
      'invalid_reenter_on_sl_hit',
      'action.place_order reenterOnSlHit must be boolean (true/false).'
    );
  }
  const reentryMaxAttempts = toFiniteNumber(config.reentryMaxAttempts);
  if (
    config.reentryMaxAttempts != null &&
    !(
      allowsZeroReentryMaxAttempts &&
      reentryMaxAttempts === 0
    ) &&
    (reentryMaxAttempts == null || reentryMaxAttempts < 1 || reentryMaxAttempts > 10 || !Number.isInteger(reentryMaxAttempts))
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_reentry_max_attempts',
      'action.place_order reentryMaxAttempts must be an integer in [1, 10].'
    );
  }
  const reentryMinPriceCent = toFiniteNumber(config.reentryMinPriceCent);
  if (
    config.reentryMinPriceCent != null &&
    (reentryMinPriceCent == null || reentryMinPriceCent <= 0 || reentryMinPriceCent > 100)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_reentry_min_price_cent',
      'action.place_order reentryMinPriceCent must be in (0, 100].'
    );
  }
  const reentryMaxPriceCent = toFiniteNumber(config.reentryMaxPriceCent);
  if (
    config.reentryMaxPriceCent != null &&
    (reentryMaxPriceCent == null || reentryMaxPriceCent <= 0 || reentryMaxPriceCent > 100)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_reentry_max_price_cent',
      'action.place_order reentryMaxPriceCent must be in (0, 100].'
    );
  }
  const reentryPriceToBeatMaxDiff = toFiniteNumber(config.reentryPriceToBeatMaxDiff);
  if (
    config.reentryPriceToBeatMaxDiff != null &&
    (reentryPriceToBeatMaxDiff == null || reentryPriceToBeatMaxDiff <= 0)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_reentry_price_to_beat_max_diff',
      'action.place_order reentryPriceToBeatMaxDiff must be > 0.'
    );
  }
  const reentryPriceToBeatMaxDiffUnitRaw = String(config.reentryPriceToBeatMaxDiffUnit ?? '')
    .trim()
    .toLowerCase();
  const hasReentryPriceToBeatMaxDiffUnit = reentryPriceToBeatMaxDiffUnitRaw.length > 0;
  const hasInvalidReentryPriceToBeatMaxDiffUnit =
    hasReentryPriceToBeatMaxDiffUnit &&
    reentryPriceToBeatMaxDiffUnitRaw !== 'usd' &&
    reentryPriceToBeatMaxDiffUnitRaw !== 'cent';
  if (reenterOnSlHit === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_reenter_on_sl_hit_side',
      'action.place_order reenterOnSlHit is only valid for side=buy.'
    );
  }
  if (reenterOnSlHit === true && !effectiveAnyStopLossEnabled) {
    pushNodeError(
      issues,
      node,
      'reenter_on_sl_hit_requires_sl',
      'action.place_order reenterOnSlHit requires slEnabled=true, slRules, ptbStopLossEnabled=true, or ptbStopLossRules.'
    );
  }
  if (stagedSlReentryOnlyAfterAllStages === true && reenterOnSlHit !== true) {
    pushNodeError(
      issues,
      node,
      'staged_sl_reentry_only_after_all_stages_requires_reentry',
      'action.place_order stagedSlReentryOnlyAfterAllStages requires reenterOnSlHit=true.'
    );
  }
  if (
    stagedSlReentryOnlyAfterAllStages === true &&
    !hasSlRules &&
    !hasPtbStopLossRules
  ) {
    pushNodeError(
      issues,
      node,
      'staged_sl_reentry_only_after_all_stages_requires_sl_rules',
      'action.place_order stagedSlReentryOnlyAfterAllStages requires staged slRules or ptbStopLossRules.'
    );
  }
  const explicitKind = String(config.kind ?? '').trim().toLowerCase();
  const resolvedOrderKind =
    explicitKind === 'conditional' || explicitKind === 'immediate'
      ? explicitKind
      : isSupportedMarketPriceTriggerCondition(triggerCondition) &&
          (config.triggerPriceCent != null || config.triggerPrice != null)
        ? 'conditional'
        : 'immediate';
  if (reenterOnSlHit === true && resolvedOrderKind !== 'immediate') {
    pushNodeError(
      issues,
      node,
      'invalid_reentry_kind',
      'action.place_order reenterOnSlHit only supports immediate buy nodes.'
    );
  }
  if (config.reentryMinPriceCent != null && reenterOnSlHit !== true) {
    pushNodeError(
      issues,
      node,
      'reentry_min_price_requires_reentry',
      'action.place_order reentryMinPriceCent requires reenterOnSlHit=true.'
    );
  }
  if (config.reentryMaxPriceCent != null && reenterOnSlHit !== true) {
    pushNodeError(
      issues,
      node,
      'reentry_max_price_requires_reentry',
      'action.place_order reentryMaxPriceCent requires reenterOnSlHit=true.'
    );
  }
  if (config.reentryPriceToBeatMaxDiff != null && reenterOnSlHit !== true) {
    pushNodeError(
      issues,
      node,
      'reentry_price_to_beat_max_diff_requires_reentry',
      'action.place_order reentryPriceToBeatMaxDiff requires reenterOnSlHit=true.'
    );
  }
  if (hasReentryPriceToBeatMaxDiffUnit && reenterOnSlHit !== true) {
    pushNodeError(
      issues,
      node,
      'reentry_price_to_beat_max_diff_unit_requires_reentry',
      'action.place_order reentryPriceToBeatMaxDiffUnit requires reenterOnSlHit=true.'
    );
  }
  if (hasInvalidReentryPriceToBeatMaxDiffUnit) {
    pushNodeError(
      issues,
      node,
      'invalid_reentry_price_to_beat_max_diff_unit',
      'action.place_order reentryPriceToBeatMaxDiffUnit must be usd or cent when provided.'
    );
  }
  if (reenterOnSlHit === true) {
    if (reentryMaxAttempts == null || reentryMaxAttempts < 1 || reentryMaxAttempts > 10) {
      pushNodeError(
        issues,
        node,
        'missing_reentry_max_attempts',
        'action.place_order reentryMaxAttempts must be set to an integer in [1, 10] when reenterOnSlHit=true.'
      );
    }
    const triggerKey = findUniqueUpstreamMarketPriceTrigger(node.key, graph);
    if (!triggerKey) {
      pushNodeError(
        issues,
        node,
        'missing_unique_upstream_reentry_trigger',
        'action.place_order reenterOnSlHit requires exactly one upstream trigger.market_price.'
      );
    } else {
      const triggerNode = graph.nodes.find((candidate) => candidate.key === triggerKey);
      const triggerConfig = isRecord(triggerNode?.config) ? triggerNode.config : {};
      const repeatMode = String(triggerConfig.repeatMode ?? '').trim().toLowerCase();
      if (repeatMode !== 'once') {
        pushNodeError(
          issues,
          node,
          'invalid_reentry_trigger_repeat_mode',
          'action.place_order reenterOnSlHit requires upstream trigger.market_price repeatMode=once.'
        );
      }
    }
    if (
      reentryMinPriceCent != null &&
      reentryMaxPriceCent != null &&
      reentryMinPriceCent >= reentryMaxPriceCent
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_reentry_price_band',
        'action.place_order reentryMinPriceCent must be lower than reentryMaxPriceCent.'
      );
    }
  }
  if (
    effectiveTpEnabled &&
    effectiveSlEnabled &&
    tpPrice.value != null &&
    slPrice.value != null &&
    slPrice.value >= tpPrice.value
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_sl_tp_band',
      'action.place_order requires slPrice < tpPrice when both stop loss and take profit are enabled.'
    );
  }

  const maxPriceCent = toFiniteNumber(config.maxPriceCent);
  if (maxPriceCent != null && (maxPriceCent <= 0 || maxPriceCent > 100)) {
    pushNodeError(
      issues,
      node,
      'invalid_max_price_cent',
      'action.place_order maxPriceCent must be in (0, 100].'
    );
  }

  const triggerPriceGuardEnabled = toBooleanish(config.triggerPriceGuardEnabled);
  if (config.triggerPriceGuardEnabled != null && triggerPriceGuardEnabled == null) {
    pushNodeError(
      issues,
      node,
      'invalid_trigger_price_guard_enabled',
      'action.place_order triggerPriceGuardEnabled must be boolean (true/false).'
    );
  }
  if (triggerPriceGuardEnabled === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_trigger_guard_side',
      'action.place_order triggerPriceGuardEnabled is only valid for side=buy.'
    );
  }
  if (triggerPriceGuardEnabled === true && !hasUpstreamTriggerWithTriggerPrice(node.key, graph)) {
    pushNodeError(
      issues,
      node,
      'missing_upstream_trigger_price',
      'triggerPriceGuardEnabled requires an upstream trigger with configured triggerPrice.'
    );
  }
  const retryOnTriggerPriceGuardBlock = toBooleanish(config.retryOnTriggerPriceGuardBlock);
  if (
    config.retryOnTriggerPriceGuardBlock != null &&
    retryOnTriggerPriceGuardBlock == null
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_retry_on_trigger_guard_block',
      'action.place_order retryOnTriggerPriceGuardBlock must be boolean (true/false).'
    );
  }
  if (
    retryOnTriggerPriceGuardBlock === true &&
    triggerPriceGuardEnabled !== true &&
    !(reenterOnSlHit === true && reentryMinPriceCent != null)
  ) {
    pushNodeError(
      issues,
      node,
      'retry_on_trigger_guard_block_requires_guard',
      'retryOnTriggerPriceGuardBlock requires triggerPriceGuardEnabled=true or reentryMinPriceCent with reenterOnSlHit=true.'
    );
  }

  validateActionPlaceOrderBuyFillLockConfig(issues, node, config, side);
  validateActionPlaceOrderAutoTuneConfig(issues, node, config);

  validateActionPlaceOrderExecutionFloorConfig(issues, node, graph, side, config);

  const priceToBeatGuardEnabled = toBooleanish(config.priceToBeatGuardEnabled);
  let normalizedPriceToBeatMode: PtbMode | null = null;
  if (config.priceToBeatGuardEnabled != null && priceToBeatGuardEnabled == null) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_guard_enabled',
      'action.place_order priceToBeatGuardEnabled must be boolean (true/false).'
    );
  }
  if (priceToBeatGuardEnabled === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_guard_side',
      'action.place_order priceToBeatGuardEnabled is only valid for side=buy.'
    );
  }
	  if (priceToBeatGuardEnabled === true) {
    const ptbMode = String(config.priceToBeatMode ?? '').trim().toLowerCase();
    if (!ptbMode || isPtbMode(ptbMode)) {
      normalizedPriceToBeatMode = normalizePtbMode(ptbMode);
    } else {
      normalizedPriceToBeatMode = null;
    }
    if (normalizedPriceToBeatMode == null) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_mode',
        'action.place_order priceToBeatMode must be manual, auto_last_3_avg_excursion, auto_vol_pct, signal_formula, or iv_mismatch_edge.'
      );
    }
    const effectiveMarketSlug = String(config.marketSlug ?? graphMarketSlug).trim().toLowerCase();
    const explicitAssetMatch =
      effectiveMarketSlug.length > 0
        ? /^(btc|eth|sol|xrp)-updown-(5m|15m)-/.exec(effectiveMarketSlug)
        : null;
    const explicitAsset = explicitAssetMatch?.[1] ?? '';
    const upstreamRuntimeAssets = collectUpstreamRuntimePtbAssets(node.key, graph);
    if (
      normalizedPriceToBeatMode === 'auto_vol_pct' &&
      (explicitAsset === 'xrp' || upstreamRuntimeAssets.has('xrp'))
    ) {
      pushNodeError(
        issues,
        node,
        'unsupported_price_to_beat_auto_vol_pct_asset',
        'action.place_order auto_vol_pct supports only BTC, ETH, and SOL markets.'
      );
    }
    if (normalizedPriceToBeatMode === 'manual') {
      const priceToBeatMaxDiff = toFiniteNumber(config.priceToBeatMaxDiff);
      if (priceToBeatMaxDiff == null || priceToBeatMaxDiff <= 0) {
        pushNodeError(
          issues,
          node,
          'invalid_price_to_beat_max_diff',
          'action.place_order priceToBeatMaxDiff must be > 0 when guard is enabled.'
        );
      }
      const priceToBeatMaxDiffUnit = String(config.priceToBeatMaxDiffUnit ?? '')
        .trim()
        .toLowerCase();
      if (priceToBeatMaxDiffUnit !== 'usd' && priceToBeatMaxDiffUnit !== 'cent') {
        pushNodeError(
          issues,
          node,
          'invalid_price_to_beat_max_diff_unit',
          'action.place_order priceToBeatMaxDiffUnit must be usd or cent when guard is enabled.'
        );
      }
    }

    const hasSupportedRuntimeMarket = hasResolveMarketNode || hasUpstreamMarketPriceAutoScope;
    const isSupportedExplicitMarket =
      effectiveMarketSlug.length > 0 &&
      /^(btc|eth|sol|xrp)-updown-(5m|15m)-/.test(effectiveMarketSlug);
    if (effectiveMarketSlug.length > 0 && !isSupportedExplicitMarket) {
      pushNodeError(
        issues,
        node,
        'invalid_price_to_beat_market',
        'priceToBeatGuardEnabled only supports 5m/15m updown market slugs.'
      );
    } else if (effectiveMarketSlug.length === 0 && !hasSupportedRuntimeMarket) {
      pushNodeError(
        issues,
        node,
        'missing_price_to_beat_market',
        'priceToBeatGuardEnabled requires a 5m/15m updown market slug or an upstream trigger.market_price/runtime market resolver.'
      );
	    }
	  }
  validateActionPlaceOrderPtbIvTimeRulesConfig(
    issues,
    node,
    config,
    priceToBeatGuardEnabled,
    normalizedPriceToBeatMode
  );
  const ptbCurrentPriceSourceRaw = String(config.priceToBeatCurrentPriceSource ?? '')
    .trim()
    .toLowerCase();
  const ptbCurrentPriceSourceActive =
    priceToBeatGuardEnabled === true || ptbStopLossEnabled === true || hasPtbStopLossRules;
  if (ptbCurrentPriceSourceRaw && !isPtbCurrentPriceSource(ptbCurrentPriceSourceRaw)) {
    pushNodeError(
      issues,
      node,
      'invalid_price_to_beat_current_price_source',
      'action.place_order priceToBeatCurrentPriceSource must be chainlink, binance, or coinbase.'
    );
  }
  if (config.priceToBeatCurrentPriceSource != null && !ptbCurrentPriceSourceActive) {
    pushNodeError(
      issues,
      node,
      'price_to_beat_current_price_source_requires_ptb',
      'action.place_order priceToBeatCurrentPriceSource requires priceToBeatGuardEnabled=true, ptbStopLossEnabled=true, or ptbStopLossRules.'
    );
  }
  if (config.reentryPriceToBeatMaxDiff != null && priceToBeatGuardEnabled !== true) {
    pushNodeError(
      issues,
      node,
      'reentry_price_to_beat_max_diff_requires_guard',
      'action.place_order reentryPriceToBeatMaxDiff requires priceToBeatGuardEnabled=true.'
    );
  }
  if (hasReentryPriceToBeatMaxDiffUnit && priceToBeatGuardEnabled !== true) {
    pushNodeError(
      issues,
      node,
      'reentry_price_to_beat_max_diff_unit_requires_guard',
      'action.place_order reentryPriceToBeatMaxDiffUnit requires priceToBeatGuardEnabled=true.'
    );
  }
  if (
    reenterOnSlHit === true &&
    priceToBeatGuardEnabled === true &&
    config.reentryPriceToBeatMaxDiff != null
  ) {
    if (normalizedPriceToBeatMode !== 'manual') {
      if (!hasReentryPriceToBeatMaxDiffUnit) {
        pushNodeError(
          issues,
          node,
          'missing_reentry_price_to_beat_max_diff_unit',
          'action.place_order reentryPriceToBeatMaxDiffUnit must be usd or cent when reentryPriceToBeatMaxDiff overrides an auto PTB mode.'
        );
      }
    }
  }
  const retryOnPriceToBeatGuardBlock = toBooleanish(config.retryOnPriceToBeatGuardBlock);
  if (
    config.retryOnPriceToBeatGuardBlock != null &&
    retryOnPriceToBeatGuardBlock == null
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_retry_on_price_to_beat_guard_block',
      'action.place_order retryOnPriceToBeatGuardBlock must be boolean (true/false).'
    );
  }
  if (retryOnPriceToBeatGuardBlock === true && priceToBeatGuardEnabled !== true) {
    pushNodeError(
      issues,
      node,
      'retry_on_price_to_beat_guard_block_requires_guard',
      'retryOnPriceToBeatGuardBlock requires priceToBeatGuardEnabled=true.'
    );
  }
  validateActionPlaceOrderPtbStopLossBumpConfig(
    issues,
    node,
    side,
    config,
    priceToBeatGuardEnabled
  );
  validateActionPlaceOrderPtbV2Config(
    issues,
    node,
    graph,
    side,
    config,
    priceToBeatGuardEnabled === true,
    reenterOnSlHit === true,
    ptbStopLossEnabled === true
  );

  const notifyOnOrderSubmitted = toBooleanish(config.notifyOnOrderSubmitted);
  if (config.notifyOnOrderSubmitted != null && notifyOnOrderSubmitted == null) {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_order_submitted',
      'action.place_order notifyOnOrderSubmitted must be boolean (true/false).'
    );
  }
  const notifyOnOrderPlaced = toBooleanish(config.notifyOnOrderPlaced);
  if (config.notifyOnOrderPlaced != null && notifyOnOrderPlaced == null) {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_order_placed',
      'action.place_order notifyOnOrderPlaced must be boolean (true/false).'
    );
  }
  const notifyOnOrderNotFilled = toBooleanish(config.notifyOnOrderNotFilled);
  if (config.notifyOnOrderNotFilled != null && notifyOnOrderNotFilled == null) {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_order_not_filled',
      'action.place_order notifyOnOrderNotFilled must be boolean (true/false).'
    );
  }
  const notifyOnLiveGapCollectorDecision = toBooleanish(config.notifyOnLiveGapCollectorDecision);
  if (
    config.notifyOnLiveGapCollectorDecision != null &&
    notifyOnLiveGapCollectorDecision == null
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_live_gap_collector_decision',
      'action.place_order notifyOnLiveGapCollectorDecision must be boolean (true/false).'
    );
  }

  const notifyOnTriggerPriceBlocked = toBooleanish(config.notifyOnTriggerPriceBlocked);
  if (
    config.notifyOnTriggerPriceBlocked != null &&
    notifyOnTriggerPriceBlocked == null
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_trigger_price_blocked',
      'action.place_order notifyOnTriggerPriceBlocked must be boolean (true/false).'
    );
  }
  if (notifyOnTriggerPriceBlocked === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_trigger_price_blocked_side',
      'action.place_order notifyOnTriggerPriceBlocked is only valid for side=buy.'
    );
  }
  if (
    notifyOnTriggerPriceBlocked === true &&
    triggerPriceGuardEnabled !== true &&
    !(reenterOnSlHit === true && reentryMinPriceCent != null)
  ) {
    pushNodeError(
      issues,
      node,
      'notify_on_trigger_price_blocked_requires_guard',
      'notifyOnTriggerPriceBlocked requires triggerPriceGuardEnabled=true or reentryMinPriceCent with reenterOnSlHit=true.'
    );
  }

  const notifyOnMaxPriceBlocked = toBooleanish(config.notifyOnMaxPriceBlocked);
  if (
    config.notifyOnMaxPriceBlocked != null &&
    notifyOnMaxPriceBlocked == null
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_max_price_blocked',
      'action.place_order notifyOnMaxPriceBlocked must be boolean (true/false).'
    );
  }
  if (notifyOnMaxPriceBlocked === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_max_price_blocked_side',
      'action.place_order notifyOnMaxPriceBlocked is only valid for side=buy.'
    );
  }

  const retryOnMaxPriceBlock = toBooleanish(config.retryOnMaxPriceBlock);
  if (
    config.retryOnMaxPriceBlock != null &&
    retryOnMaxPriceBlock == null
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_retry_on_max_price_block',
      'action.place_order retryOnMaxPriceBlock must be boolean (true/false).'
    );
  }
  if (retryOnMaxPriceBlock === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_retry_on_max_price_block_side',
      'action.place_order retryOnMaxPriceBlock is only valid for side=buy.'
    );
  }
  if (
    notifyOnMaxPriceBlocked === true &&
    maxPriceCent == null &&
    !(reenterOnSlHit === true && reentryMaxPriceCent != null)
  ) {
    pushNodeError(
      issues,
      node,
      'notify_on_max_price_blocked_requires_max_price',
      'notifyOnMaxPriceBlocked requires maxPriceCent or reentryMaxPriceCent to be set.'
    );
  }
  if (
    retryOnMaxPriceBlock === true &&
    maxPriceCent == null &&
    !(reenterOnSlHit === true && reentryMaxPriceCent != null)
  ) {
    pushNodeError(
      issues,
      node,
      'retry_on_max_price_block_requires_max_price',
      'retryOnMaxPriceBlock requires maxPriceCent or reentryMaxPriceCent to be set.'
    );
  }

  const notifyOnPriceToBeatGapBlocked = toBooleanish(config.notifyOnPriceToBeatGapBlocked);
  if (
    config.notifyOnPriceToBeatGapBlocked != null &&
    notifyOnPriceToBeatGapBlocked == null
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_price_to_beat_gap_blocked',
      'action.place_order notifyOnPriceToBeatGapBlocked must be boolean (true/false).'
    );
  }
  if (notifyOnPriceToBeatGapBlocked === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_price_to_beat_gap_blocked_side',
      'action.place_order notifyOnPriceToBeatGapBlocked is only valid for side=buy.'
    );
  }
  if (
    notifyOnPriceToBeatGapBlocked === true &&
    priceToBeatGuardEnabled !== true
  ) {
    pushNodeError(
      issues,
      node,
      'notify_on_price_to_beat_gap_blocked_requires_guard',
      'notifyOnPriceToBeatGapBlocked requires priceToBeatGuardEnabled=true.'
    );
  }

  const notifyOnTpHit = toBooleanish(config.notifyOnTpHit);
  if (config.notifyOnTpHit != null && notifyOnTpHit == null) {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_tp_hit',
      'action.place_order notifyOnTpHit must be boolean (true/false).'
    );
  }
  if (notifyOnTpHit === true && tpEnabled !== true) {
    pushNodeError(
      issues,
      node,
      'notify_on_tp_hit_requires_tp',
      'notifyOnTpHit requires tpEnabled=true.'
    );
  }

  const notifyOnSlHit = toBooleanish(config.notifyOnSlHit);
  if (config.notifyOnSlHit != null && notifyOnSlHit == null) {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_sl_hit',
      'action.place_order notifyOnSlHit must be boolean (true/false).'
    );
  }
  if (notifyOnSlHit === true && !effectiveAnyStopLossEnabled) {
    pushNodeError(
      issues,
      node,
      'notify_on_sl_hit_requires_sl',
      'notifyOnSlHit requires slEnabled=true, slRules, ptbStopLossEnabled=true, or ptbStopLossRules.'
    );
  }
}
