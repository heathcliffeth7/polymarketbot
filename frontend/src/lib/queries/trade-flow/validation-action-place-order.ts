import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import {
  findUniqueUpstreamMarketPriceTrigger,
  hasUpstreamAutoScopeMarketTrigger,
  hasUpstreamTriggerWithTriggerPrice,
  hasUpstreamBuyPlaceOrderNode,
} from './graph';
import {
  isRecord,
  isSupportedMarketPriceTriggerCondition,
  resolveConfiguredBinaryPrice,
  toBooleanish,
  toFiniteNumber,
} from './shared';
import { pushNodeError } from './validation-core';

export function validateActionPlaceOrderConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  graph: TradeFlowGraph
) {
  const config = isRecord(node.config) ? node.config : {};
  const graphSourceTradeId = toFiniteNumber(graph.context.sourceTradeId);
  const graphMarketSlug = String(graph.context.marketSlug ?? '').trim();
  const graphTokenId = String(graph.context.tokenId ?? '').trim();
  const hasResolveMarketNode = graph.nodes.some((candidate) => candidate.type === 'action.resolve_market');
  const hasUpstreamMarketPriceAutoScope = hasUpstreamAutoScopeMarketTrigger(node.key, graph);

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
    !(side === 'buy' && hasUpstreamMarketPriceAutoScope)
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
    !(side === 'buy' && hasUpstreamMarketPriceAutoScope)
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
  if (sizeModeRaw && sizeModeRaw !== 'usdc' && sizeModeRaw !== 'pct') {
    pushNodeError(
      issues,
      node,
      'invalid_size_mode',
      'action.place_order sizeMode must be usdc or pct.'
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
  const hasTriggerSizes = triggerSizes.length > 0;
  const usesPctSizing = sizeModeRaw === 'pct' || (!hasTriggerSizes && sizeUsdc == null && sizePct != null);
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
    } else if (sizeUsdc == null || sizeUsdc <= 0) {
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
  const tpPrice = resolveConfiguredBinaryPrice(config.tpPriceCent, config.tpPrice);
  const slPrice = resolveConfiguredBinaryPrice(config.slPriceCent, config.slPrice);

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
  if (tpEnabled === true && !tpPrice.provided) {
    pushNodeError(
      issues,
      node,
      'missing_tp_price',
      'action.place_order tpEnabled requires tpPriceCent (or legacy tpPrice).'
    );
  } else if (tpPrice.provided && tpPrice.value == null) {
    pushNodeError(
      issues,
      node,
      'invalid_tp_price',
      'action.place_order tpPriceCent must be in (0, 100] or legacy tpPrice must be in (0, 1].'
    );
  }
  if (slEnabled === true && !slPrice.provided) {
    pushNodeError(
      issues,
      node,
      'missing_sl_price',
      'action.place_order slEnabled requires slPriceCent (or legacy slPrice).'
    );
  } else if (slPrice.provided && slPrice.value == null) {
    pushNodeError(
      issues,
      node,
      'invalid_sl_price',
      'action.place_order slPriceCent must be in (0, 100] or legacy slPrice must be in (0, 1].'
    );
  }
  const slTriggerPriceMode =
    typeof config.slTriggerPriceMode === 'string' ? config.slTriggerPriceMode : null;
  if (slEnabled === true && slTriggerPriceMode != null) {
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
    (reentryMaxAttempts == null || reentryMaxAttempts < 1 || reentryMaxAttempts > 10 || !Number.isInteger(reentryMaxAttempts))
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_reentry_max_attempts',
      'action.place_order reentryMaxAttempts must be an integer in [1, 10].'
    );
  }
  if (reenterOnSlHit === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_reenter_on_sl_hit_side',
      'action.place_order reenterOnSlHit is only valid for side=buy.'
    );
  }
  if (reenterOnSlHit === true && slEnabled !== true) {
    pushNodeError(
      issues,
      node,
      'reenter_on_sl_hit_requires_sl',
      'action.place_order reenterOnSlHit requires slEnabled=true.'
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
  }
  if (
    tpEnabled === true &&
    slEnabled === true &&
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
  if (retryOnTriggerPriceGuardBlock === true && triggerPriceGuardEnabled !== true) {
    pushNodeError(
      issues,
      node,
      'retry_on_trigger_guard_block_requires_guard',
      'retryOnTriggerPriceGuardBlock requires triggerPriceGuardEnabled=true.'
    );
  }

  const executionFloorGuardEnabled = toBooleanish(config.executionFloorGuardEnabled);
  if (config.executionFloorGuardEnabled != null && executionFloorGuardEnabled == null) {
    pushNodeError(
      issues,
      node,
      'invalid_execution_floor_guard_enabled',
      'action.place_order executionFloorGuardEnabled must be boolean (true/false).'
    );
  }
  if (executionFloorGuardEnabled === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_execution_floor_guard_side',
      'action.place_order executionFloorGuardEnabled is only valid for side=buy.'
    );
  }
  if (executionFloorGuardEnabled === true && !hasUpstreamTriggerWithTriggerPrice(node.key, graph)) {
    pushNodeError(
      issues,
      node,
      'missing_upstream_execution_floor_trigger_price',
      'executionFloorGuardEnabled requires an upstream trigger with configured triggerPrice.'
    );
  }
  const retryOnExecutionFloorGuardBlock = toBooleanish(config.retryOnExecutionFloorGuardBlock);
  if (
    config.retryOnExecutionFloorGuardBlock != null &&
    retryOnExecutionFloorGuardBlock == null
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_retry_on_execution_floor_guard_block',
      'action.place_order retryOnExecutionFloorGuardBlock must be boolean (true/false).'
    );
  }
  if (retryOnExecutionFloorGuardBlock === true && executionFloorGuardEnabled !== true) {
    pushNodeError(
      issues,
      node,
      'retry_on_execution_floor_guard_block_requires_guard',
      'retryOnExecutionFloorGuardBlock requires executionFloorGuardEnabled=true.'
    );
  }

  const priceToBeatGuardEnabled = toBooleanish(config.priceToBeatGuardEnabled);
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

    const effectiveMarketSlug = String(config.marketSlug ?? graphMarketSlug).trim().toLowerCase();
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
    triggerPriceGuardEnabled !== true
  ) {
    pushNodeError(
      issues,
      node,
      'notify_on_trigger_price_blocked_requires_guard',
      'notifyOnTriggerPriceBlocked requires triggerPriceGuardEnabled=true.'
    );
  }

  const notifyOnExecutionFloorBlocked = toBooleanish(config.notifyOnExecutionFloorBlocked);
  if (
    config.notifyOnExecutionFloorBlocked != null &&
    notifyOnExecutionFloorBlocked == null
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_execution_floor_blocked',
      'action.place_order notifyOnExecutionFloorBlocked must be boolean (true/false).'
    );
  }
  if (notifyOnExecutionFloorBlocked === true && side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'invalid_notify_on_execution_floor_blocked_side',
      'action.place_order notifyOnExecutionFloorBlocked is only valid for side=buy.'
    );
  }
  if (
    notifyOnExecutionFloorBlocked === true &&
    executionFloorGuardEnabled !== true
  ) {
    pushNodeError(
      issues,
      node,
      'notify_on_execution_floor_blocked_requires_guard',
      'notifyOnExecutionFloorBlocked requires executionFloorGuardEnabled=true.'
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
  if (notifyOnMaxPriceBlocked === true && maxPriceCent == null) {
    pushNodeError(
      issues,
      node,
      'notify_on_max_price_blocked_requires_max_price',
      'notifyOnMaxPriceBlocked requires maxPriceCent to be set.'
    );
  }
  if (retryOnMaxPriceBlock === true && maxPriceCent == null) {
    pushNodeError(
      issues,
      node,
      'retry_on_max_price_block_requires_max_price',
      'retryOnMaxPriceBlock requires maxPriceCent to be set.'
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
  if (notifyOnSlHit === true && slEnabled !== true) {
    pushNodeError(
      issues,
      node,
      'notify_on_sl_hit_requires_sl',
      'notifyOnSlHit requires slEnabled=true.'
    );
  }
}
