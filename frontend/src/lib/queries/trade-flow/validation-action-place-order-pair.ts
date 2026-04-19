import { PAIR_LOCK_UNSUPPORTED_EXIT_FIELD_KEYS } from '@/lib/trade-flow-config-mappers/pair-lock';
import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { hasUpstreamAutoScopeMarketTrigger } from './graph';
import {
  isSupportedMarketPriceTriggerCondition,
  toBooleanish,
  toFiniteNumber,
  toTrimmedString,
} from './shared';
import { pushNodeError } from './validation-core';

function normalizeBinaryOutcome(value: string): 'yes' | 'no' | null {
  switch (value.trim().toLowerCase()) {
    case 'yes':
    case 'up':
    case 'true':
    case '1':
      return 'yes';
    case 'no':
    case 'down':
    case 'false':
    case '0':
      return 'no';
    default:
      return null;
  }
}

function isIgnoredPairLockZeroValue(key: string, value: unknown): boolean {
  if (key !== 'reentryCooldownSec' && key !== 'reentryMaxPriceTightenBps') {
    return false;
  }
  return toFiniteNumber(value) === 0;
}

export function validateActionPlaceOrderPairLockConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  graph: TradeFlowGraph,
  config: Record<string, unknown>,
  side: string,
  executionMode: string
) {
  const mode = toTrimmedString(config.mode).toLowerCase();
  if (!mode || mode === 'single') {
    return;
  }

  if (mode !== 'pair_lock') {
    pushNodeError(
      issues,
      node,
      'invalid_place_order_mode',
      'action.place_order mode must be single or pair_lock.'
    );
    return;
  }

  const nodeMap = new Map(graph.nodes.map((candidate) => [candidate.key, candidate]));
  const directIncomingNodes = graph.edges
    .filter((edge) => edge.target === node.key)
    .map((edge) => nodeMap.get(edge.source))
    .filter((candidate): candidate is TradeFlowNode => !!candidate);
  if (directIncomingNodes.length !== 1) {
    pushNodeError(
      issues,
      node,
      'pair_lock_requires_single_direct_trigger',
      'action.place_order pair_lock requires exactly one direct upstream trigger.market_price.'
    );
  } else {
    const triggerNode = directIncomingNodes[0];
    const triggerConfig =
      typeof triggerNode.config === 'object' && triggerNode.config != null ? triggerNode.config : {};
    const bindingMode = toTrimmedString((triggerConfig as Record<string, unknown>).bindingMode).toLowerCase() || 'standard';
    if (triggerNode.type !== 'trigger.market_price') {
      pushNodeError(
        issues,
        node,
        'pair_lock_requires_market_price_trigger',
        'action.place_order pair_lock only supports a direct upstream trigger.market_price.'
      );
    } else if (bindingMode !== 'pair_lock_only') {
      pushNodeError(
        issues,
        node,
        'pair_lock_requires_pair_lock_only_trigger',
        'action.place_order pair_lock requires upstream trigger.market_price bindingMode=pair_lock_only.'
      );
    }
  }

  if (side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'pair_lock_requires_buy_side',
      'action.place_order pair_lock only supports side=buy.'
    );
  }
  if (executionMode !== 'limit' && executionMode !== 'market') {
    pushNodeError(
      issues,
      node,
      'pair_lock_requires_supported_execution',
      'action.place_order pair_lock only supports executionMode=limit or market.'
    );
  }
  const kind = toTrimmedString(config.kind).toLowerCase();
  if (kind && kind !== 'immediate') {
    pushNodeError(
      issues,
      node,
      'pair_lock_requires_immediate_kind',
      'action.place_order pair_lock only supports kind=immediate.'
    );
  }
  const sizeMode = toTrimmedString(config.sizeMode).toLowerCase();
  if (sizeMode === 'pct' || config.sizePct != null || config.sizePercent != null) {
    pushNodeError(
      issues,
      node,
      'pair_lock_requires_usdc_sizing',
      'action.place_order pair_lock only supports USDC sizing.'
    );
  }
  const maxTriggers = toFiniteNumber(config.maxTriggers);
  if (maxTriggers != null && maxTriggers !== 1) {
    pushNodeError(
      issues,
      node,
      'pair_lock_requires_single_trigger',
      'action.place_order pair_lock requires maxTriggers=1 when set.'
    );
  }

  const counterLegEnabled = toBooleanish(config.counterLegEnabled);
  if (config.counterLegEnabled != null && counterLegEnabled == null) {
    pushNodeError(
      issues,
      node,
      'invalid_counter_leg_enabled',
      'action.place_order counterLegEnabled must be boolean (true/false).'
    );
  }
  if (counterLegEnabled !== true) {
    pushNodeError(
      issues,
      node,
      'pair_lock_requires_counter_leg',
      'action.place_order pair_lock requires counterLegEnabled=true.'
    );
  }

  const pairMaxTotalCent = toFiniteNumber(config.pairMaxTotalCent ?? config.pairTargetTotalCent);
  if (pairMaxTotalCent == null || pairMaxTotalCent <= 0 || pairMaxTotalCent >= 100) {
    pushNodeError(
      issues,
      node,
      'invalid_pair_max_total_cent',
      'action.place_order pairMaxTotalCent must be in (0, 100).'
    );
  }
  const pairSizingMode = toTrimmedString(config.pairSizingMode).toLowerCase() || 'manual';
  if (pairSizingMode !== 'manual' && pairSizingMode !== 'auto_remaining_budget') {
    pushNodeError(
      issues,
      node,
      'invalid_pair_sizing_mode',
      'action.place_order pairSizingMode must be manual or auto_remaining_budget.'
    );
  }
  const pairOrphanGraceMs = toFiniteNumber(config.pairOrphanGraceMs);
  if (
    config.pairOrphanGraceMs != null &&
    (pairOrphanGraceMs == null || pairOrphanGraceMs < 0 || !Number.isInteger(pairOrphanGraceMs))
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_pair_orphan_grace_ms',
      'action.place_order pairOrphanGraceMs must be an integer >= 0.'
    );
  }

  for (const [key, code] of [
    ['notifyOnPairLocked', 'invalid_notify_on_pair_locked'],
    ['notifyOnPairUnwind', 'invalid_notify_on_pair_unwind'],
    ['counterLegPriceToBeatGuardEnabled', 'invalid_counter_leg_ptb_guard_enabled'],
    ['counterLegExecutionFloorGuardEnabled', 'invalid_counter_leg_execution_floor_enabled'],
    ['counterLegRetryOnPriceToBeatGuardBlock', 'invalid_counter_leg_retry_on_ptb'],
    ['counterLegRetryOnExecutionFloorGuardBlock', 'invalid_counter_leg_retry_on_floor'],
    ['counterLegRetryOnMaxPriceBlock', 'invalid_counter_leg_retry_on_max'],
  ] as const) {
    if (config[key] != null && toBooleanish(config[key]) == null) {
      pushNodeError(issues, node, code, `action.place_order ${key} must be boolean (true/false).`);
    }
  }

  const primaryLegSizeUsdc = toFiniteNumber(config.sizeUsdc ?? config.targetNotionalUsdc);
  if (pairSizingMode === 'auto_remaining_budget') {
    const pairTotalBudgetUsdc = toFiniteNumber(config.pairTotalBudgetUsdc);
    if (pairTotalBudgetUsdc == null || pairTotalBudgetUsdc <= 0) {
      pushNodeError(
        issues,
        node,
        'pair_lock_requires_total_budget_usdc',
        'action.place_order pairTotalBudgetUsdc must be > 0 in auto_remaining_budget mode.'
      );
    } else if (
      primaryLegSizeUsdc != null &&
      primaryLegSizeUsdc > 0 &&
      pairTotalBudgetUsdc <= primaryLegSizeUsdc
    ) {
      pushNodeError(
        issues,
        node,
        'pair_lock_total_budget_must_exceed_primary_size',
        'action.place_order pairTotalBudgetUsdc must be greater than the primary sizeUsdc.'
      );
    }
  } else {
    const counterLegSizeUsdc = toFiniteNumber(config.counterLegSizeUsdc);
    if (counterLegSizeUsdc == null || counterLegSizeUsdc <= 0) {
      pushNodeError(
        issues,
        node,
        'pair_lock_requires_counter_leg_size_usdc',
        'action.place_order pair_lock requires counterLegSizeUsdc > 0 in manual mode.'
      );
    }
  }

  const primaryOutcome = normalizeBinaryOutcome(toTrimmedString(config.outcomeLabel));
  const counterOutcomeRaw = toTrimmedString(config.counterLegOutcomeLabel).toLowerCase() || 'opposite';
  if (
    counterOutcomeRaw !== 'opposite' &&
    !['yes', 'no', 'up', 'down'].includes(counterOutcomeRaw)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_counter_leg_outcome_label',
      'action.place_order counterLegOutcomeLabel must be opposite, yes, no, up, or down.'
    );
  }
  const counterOutcome =
    counterOutcomeRaw === 'opposite'
      ? primaryOutcome === 'yes'
        ? 'no'
        : primaryOutcome === 'no'
          ? 'yes'
          : null
      : normalizeBinaryOutcome(counterOutcomeRaw);
  if (primaryOutcome != null && counterOutcome != null && primaryOutcome === counterOutcome) {
    pushNodeError(
      issues,
      node,
      'counter_leg_requires_opposite_outcome',
      'action.place_order pair_lock counter leg must resolve to the opposite binary outcome.'
    );
  }

  const counterTriggerCondition = toTrimmedString(config.counterLegTriggerCondition).toLowerCase();
  const counterTriggerPriceCent = toFiniteNumber(config.counterLegTriggerPriceCent);
  if (
    counterTriggerCondition &&
    !isSupportedMarketPriceTriggerCondition(counterTriggerCondition)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_counter_leg_trigger_condition',
      'action.place_order counterLegTriggerCondition must be cross_above, cross_below, level_above, or level_below.'
    );
  }
  if (!!counterTriggerCondition !== (config.counterLegTriggerPriceCent != null)) {
    pushNodeError(
      issues,
      node,
      'incomplete_counter_leg_trigger',
      'action.place_order counter leg trigger requires both counterLegTriggerCondition and counterLegTriggerPriceCent.'
    );
  }
  if (
    config.counterLegTriggerPriceCent != null &&
    (counterTriggerPriceCent == null || counterTriggerPriceCent <= 0 || counterTriggerPriceCent > 100)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_counter_leg_trigger_price_cent',
      'action.place_order counterLegTriggerPriceCent must be in (0, 100].'
    );
  }
  const counterLegMaxPriceCent = toFiniteNumber(config.counterLegMaxPriceCent);
  if (
    config.counterLegMaxPriceCent != null &&
    (counterLegMaxPriceCent == null || counterLegMaxPriceCent <= 0 || counterLegMaxPriceCent > 100)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_counter_leg_max_price_cent',
      'action.place_order counterLegMaxPriceCent must be in (0, 100].'
    );
  }

  const counterLegPtbGuardEnabled = toBooleanish(config.counterLegPriceToBeatGuardEnabled);
  if (counterLegPtbGuardEnabled === true) {
    const counterPtbMode = toTrimmedString(config.counterLegPriceToBeatMode).toLowerCase();
    if (
      counterPtbMode &&
      counterPtbMode !== 'manual' &&
      counterPtbMode !== 'auto_last_3_avg_excursion' &&
      counterPtbMode !== 'auto_vol_pct'
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_counter_leg_price_to_beat_mode',
        'action.place_order counterLegPriceToBeatMode must be manual, auto_last_3_avg_excursion, or auto_vol_pct.'
      );
    }
    if (!counterPtbMode || counterPtbMode === 'manual') {
      const counterPtbDiff = toFiniteNumber(config.counterLegPriceToBeatMaxDiff);
      const counterPtbUnit = toTrimmedString(config.counterLegPriceToBeatMaxDiffUnit).toLowerCase();
      if (counterPtbDiff == null || counterPtbDiff <= 0) {
        pushNodeError(
          issues,
          node,
          'invalid_counter_leg_price_to_beat_max_diff',
          'action.place_order counterLegPriceToBeatMaxDiff must be > 0 when counter leg PTB guard is manual.'
        );
      }
      if (counterPtbUnit !== 'usd' && counterPtbUnit !== 'cent') {
        pushNodeError(
          issues,
          node,
          'invalid_counter_leg_price_to_beat_max_diff_unit',
          'action.place_order counterLegPriceToBeatMaxDiffUnit must be usd or cent when counter leg PTB guard is manual.'
        );
      }
    }
  }

  const counterLegExecutionFloorPriceCent = toFiniteNumber(config.counterLegExecutionFloorPriceCent);
  if (
    config.counterLegExecutionFloorPriceCent != null &&
    (counterLegExecutionFloorPriceCent == null ||
      counterLegExecutionFloorPriceCent <= 0 ||
      counterLegExecutionFloorPriceCent > 100)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_counter_leg_execution_floor_price_cent',
      'action.place_order counterLegExecutionFloorPriceCent must be in (0, 100].'
    );
  }

  for (const key of [
    ...PAIR_LOCK_UNSUPPORTED_EXIT_FIELD_KEYS,
    'notifyOnTriggerPriceBlocked',
    'notifyOnExecutionFloorBlocked',
  ] as const) {
    if (config[key] != null && !isIgnoredPairLockZeroValue(key, config[key])) {
      pushNodeError(
        issues,
        node,
        'pair_lock_disallows_exit_features',
        'action.place_order pair_lock only allows hard SL/PTB-SL plus basic re-entry fields; TP, staged exits, time exits, and advanced re-entry fields are not supported.'
      );
      break;
    }
  }

  const marketSlug = toTrimmedString(config.marketSlug ?? graph.context.marketSlug).toLowerCase();
  const hasBinaryRuntimeMarket = hasUpstreamAutoScopeMarketTrigger(node.key, graph);
  const hasExplicitBinaryMarket = /^(btc|eth|sol|xrp)-updown-(5m|15m)-/.test(marketSlug);
  if (!hasExplicitBinaryMarket && !hasBinaryRuntimeMarket) {
    pushNodeError(
      issues,
      node,
      'pair_lock_requires_binary_market',
      'action.place_order pair_lock requires a binary up/down market slug or an upstream auto_scope trigger.market_price.'
    );
  }
}
