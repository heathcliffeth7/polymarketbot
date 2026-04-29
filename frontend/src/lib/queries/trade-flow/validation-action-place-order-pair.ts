import { PAIR_LOCK_UNSUPPORTED_EXIT_FIELD_KEYS } from '@/lib/trade-flow-config-mappers/pair-lock';
import { isPtbMode } from '@/lib/trade-flow-config-mappers/ptb-modes';
import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import { hasUpstreamAutoScopeMarketTrigger } from './graph';
import {
  isSupportedMarketPriceTriggerCondition,
  isRecord,
  resolveConfiguredBinaryPrice,
  toBooleanish,
  toFiniteNumber,
  toTrimmedString,
} from './shared';
import { normalizePtbStopLossGapUnitValue } from './validation-action-place-order-ptb-stop-loss';
import { pushNodeError } from './validation-core';

interface ParsedExitLadderRule {
  priceCent: number;
  sizePct: number;
}

interface ParsedRemainingWindow {
  startRemainingSec: number;
  endRemainingSec: number;
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

function isSupportedStopLossTriggerPriceMode(value: string): boolean {
  return [
    'best_bid',
    'composite',
    'composite_safe',
    'composite_fast',
    'last_trade',
  ].includes(value);
}

function nestedConfig(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {};
}

function parseBiasedTimeExitRules(raw: unknown): Array<{ elapsedSec: number; remainingPct: number }> | null {
  const value = typeof raw === 'string' && raw.trim() ? safeParseJson(raw) : raw;
  if (!Array.isArray(value)) return null;
  const parsed: Array<{ elapsedSec: number; remainingPct: number }> = [];
  for (const item of value) {
    if (!isRecord(item)) return null;
    const elapsedSec = toFiniteNumber(item.elapsedSec);
    const remainingPct = toFiniteNumber(item.remainingPct);
    if (
      elapsedSec == null ||
      elapsedSec <= 0 ||
      !Number.isInteger(elapsedSec) ||
      remainingPct == null ||
      remainingPct < 0 ||
      remainingPct > 100
    ) {
      return null;
    }
    parsed.push({ elapsedSec, remainingPct });
  }
  return parsed;
}

function safeParseJson(raw: string): unknown {
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

function parseValidIvTimeRuleWindows(raw: unknown): ParsedRemainingWindow[] {
  if (!Array.isArray(raw)) return [];
  return raw
    .map((item) => {
      if (!isRecord(item)) return null;
      const startRemainingSec = toFiniteNumber(item.startRemainingSec);
      const endRemainingSec = toFiniteNumber(item.endRemainingSec);
      if (
        startRemainingSec == null ||
        endRemainingSec == null ||
        startRemainingSec <= endRemainingSec
      ) {
        return null;
      }
      return { startRemainingSec, endRemainingSec };
    })
    .filter((item): item is ParsedRemainingWindow => item != null);
}

function remainingWindowsOverlap(
  left: ParsedRemainingWindow,
  right: ParsedRemainingWindow
): boolean {
  return Math.max(left.endRemainingSec, right.endRemainingSec) <
    Math.min(left.startRemainingSec, right.startRemainingSec);
}

function biasedHedgeEntryRemainingWindow(
  triggerConfig: Record<string, unknown>,
  biasedHedge: Record<string, unknown>
): ParsedRemainingWindow | null {
  const marketScope = toTrimmedString(triggerConfig.marketScope).toLowerCase();
  if (marketScope !== 'btc_5m_updown') return null;
  const cycleStartSec = toFiniteNumber(triggerConfig.cycleWindowStartSec) ?? 30;
  const cycleEndSec = toFiniteNumber(triggerConfig.cycleWindowEndSec);
  const disableNewPrimaryAfterSec = toFiniteNumber(biasedHedge.disableNewPrimaryAfterSec) ?? 180;
  const entryEndSec = Math.min(
    cycleEndSec ?? disableNewPrimaryAfterSec,
    disableNewPrimaryAfterSec
  );
  if (cycleStartSec < 0 || entryEndSec <= cycleStartSec) return null;
  const windowSec = 300;
  return {
    startRemainingSec: windowSec - cycleStartSec,
    endRemainingSec: windowSec - entryEndSec,
  };
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
  const configuredPairLockStrategy = toTrimmedString(config.pairLockStrategy).toLowerCase();
  if (!mode || mode === 'single') {
    if (configuredPairLockStrategy === 'adaptive_max_price_v1') {
      pushNodeError(
        issues,
        node,
        'adaptive_max_price_requires_pair_lock_mode',
        'action.place_order adaptive_max_price_v1 requires mode=pair_lock.'
      );
    }
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
  const directTriggerConfig = directIncomingNodes.length === 1 && directIncomingNodes[0].type === 'trigger.market_price'
    ? nestedConfig(directIncomingNodes[0].config)
    : {};
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
  const pairLockStrategy = toTrimmedString(config.pairLockStrategy).toLowerCase() || 'legacy';
  if (
    pairLockStrategy !== 'legacy' &&
    pairLockStrategy !== 'edge_pairlock_v1' &&
    pairLockStrategy !== 'biased_hedge_v1' &&
    pairLockStrategy !== 'adaptive_max_price_v1'
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_pair_lock_strategy',
      'action.place_order pairLockStrategy must be legacy, edge_pairlock_v1, biased_hedge_v1, or adaptive_max_price_v1.'
    );
  }
  const usesEdgePairLock = pairLockStrategy === 'edge_pairlock_v1';
  const usesBiasedHedge = pairLockStrategy === 'biased_hedge_v1';
  const usesAdaptiveMaxPrice = pairLockStrategy === 'adaptive_max_price_v1';
  if (usesEdgePairLock) {
    if (toBooleanish(config.priceToBeatGuardEnabled) !== true) {
      pushNodeError(
        issues,
        node,
        'edge_pairlock_requires_ptb_guard',
        'action.place_order edge_pairlock_v1 requires priceToBeatGuardEnabled=true.'
      );
    }
    if (toTrimmedString(config.priceToBeatMode).toLowerCase() !== 'iv_mismatch_edge') {
      pushNodeError(
        issues,
        node,
        'edge_pairlock_requires_iv_mismatch_edge',
        'action.place_order edge_pairlock_v1 requires priceToBeatMode=iv_mismatch_edge.'
      );
    }
    const pairLockDecisionQty = toFiniteNumber(config.pairLockDecisionQty);
    if (config.pairLockDecisionQty != null && (pairLockDecisionQty == null || pairLockDecisionQty <= 0)) {
      pushNodeError(
        issues,
        node,
        'invalid_pair_lock_decision_qty',
        'action.place_order pairLockDecisionQty must be > 0 for edge_pairlock_v1.'
      );
    }
    const pairLockSingleEdgeThreshold = toFiniteNumber(config.pairLockSingleEdgeThreshold);
    if (
      config.pairLockSingleEdgeThreshold != null &&
      (pairLockSingleEdgeThreshold == null || pairLockSingleEdgeThreshold < 0)
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_pair_lock_single_edge_threshold',
        'action.place_order pairLockSingleEdgeThreshold must be >= 0 for edge_pairlock_v1.'
      );
    }
    const pairLockCostBuffer = toFiniteNumber(config.pairLockCostBuffer);
    if (config.pairLockCostBuffer != null && (pairLockCostBuffer == null || pairLockCostBuffer < 0)) {
      pushNodeError(
        issues,
        node,
        'invalid_pair_lock_cost_buffer',
        'action.place_order pairLockCostBuffer must be >= 0 for edge_pairlock_v1.'
      );
    }
  }
  if (usesAdaptiveMaxPrice) {
    if (toBooleanish(config.priceToBeatGuardEnabled) !== true) {
      pushNodeError(
        issues,
        node,
        'adaptive_max_price_requires_ptb_guard',
        'action.place_order adaptive_max_price_v1 requires priceToBeatGuardEnabled=true.'
      );
    }
    if (toTrimmedString(config.priceToBeatMode).toLowerCase() !== 'iv_mismatch_edge') {
      pushNodeError(
        issues,
        node,
        'adaptive_max_price_requires_iv_mismatch_edge',
        'action.place_order adaptive_max_price_v1 requires priceToBeatMode=iv_mismatch_edge.'
      );
    }
    const missCount = toFiniteNumber(config.adaptiveMaxPriceMissCount);
    const requiredGoodMiss = toFiniteNumber(config.adaptiveMaxPriceRequiredGoodMissCount);
    const sizeMultiplier = toFiniteNumber(config.adaptiveMaxPriceSizeMultiplier);
    const extraBuffer = toFiniteNumber(config.adaptiveMaxPriceExtraBufferCent);
    if (missCount == null || missCount <= 0 || !Number.isInteger(missCount)) {
      pushNodeError(issues, node, 'invalid_adaptive_max_price_miss_count', 'adaptiveMaxPriceMissCount must be a positive integer.');
    }
    if (requiredGoodMiss == null || requiredGoodMiss <= 0 || !Number.isInteger(requiredGoodMiss)) {
      pushNodeError(issues, node, 'invalid_adaptive_max_price_good_miss_count', 'adaptiveMaxPriceRequiredGoodMissCount must be a positive integer.');
    }
    if (missCount != null && requiredGoodMiss != null && requiredGoodMiss > missCount) {
      pushNodeError(issues, node, 'adaptive_max_price_good_miss_above_miss_count', 'adaptiveMaxPriceRequiredGoodMissCount cannot exceed adaptiveMaxPriceMissCount.');
    }
    for (const [key, code] of [
      ['adaptiveMaxPriceRelaxCreditCent', 'invalid_adaptive_max_price_relax_credit'],
      ['adaptiveMaxPriceMaxRelaxCreditCent', 'invalid_adaptive_max_price_max_relax'],
      ['adaptiveMaxPriceHardCapCent', 'invalid_adaptive_max_price_hard_cap'],
      ['adaptiveMaxPriceLateRelaxCutoffS', 'invalid_adaptive_max_price_late_cutoff'],
    ] as const) {
      const value = toFiniteNumber(config[key]);
      if (value == null || value <= 0) {
        pushNodeError(issues, node, code, `action.place_order ${key} must be > 0.`);
      }
    }
    for (const [key, code] of [
      ['adaptiveMaxPricePairBufferCent', 'invalid_adaptive_max_price_pair_buffer'],
      ['adaptiveMaxPriceSlCooldownMarkets', 'invalid_adaptive_max_price_sl_cooldown'],
    ] as const) {
      const value = toFiniteNumber(config[key]);
      if (value == null || value < 0) {
        pushNodeError(issues, node, code, `action.place_order ${key} must be >= 0.`);
      }
    }
    if (extraBuffer == null || extraBuffer < 1) {
      pushNodeError(issues, node, 'invalid_adaptive_max_price_extra_buffer', 'adaptiveMaxPriceExtraBufferCent must be >= 1.');
    }
    if (sizeMultiplier == null || sizeMultiplier <= 0 || sizeMultiplier > 1) {
      pushNodeError(issues, node, 'invalid_adaptive_max_price_size_multiplier', 'adaptiveMaxPriceSizeMultiplier must be in (0, 1].');
    }
  }
  if (usesBiasedHedge) {
    if (toBooleanish(config.priceToBeatGuardEnabled) !== true) {
      pushNodeError(
        issues,
        node,
        'biased_hedge_requires_ptb_guard',
        'action.place_order biased_hedge_v1 requires priceToBeatGuardEnabled=true.'
      );
    }
    if (toTrimmedString(config.priceToBeatMode).toLowerCase() !== 'iv_mismatch_edge') {
      pushNodeError(
        issues,
        node,
        'biased_hedge_requires_iv_mismatch_edge',
        'action.place_order biased_hedge_v1 requires priceToBeatMode=iv_mismatch_edge.'
      );
    }
    if (toBooleanish(config.pairProtectiveUnwindEnabled) !== true) {
      pushNodeError(
        issues,
        node,
        'biased_hedge_requires_protective_unwind',
        'action.place_order biased_hedge_v1 smoke requires pairProtectiveUnwindEnabled=true.'
      );
    }
    const cycleWindowEndSec = toFiniteNumber(directTriggerConfig.cycleWindowEndSec);
    if (cycleWindowEndSec != null && cycleWindowEndSec > 240) {
      pushNodeError(
        issues,
        node,
        'biased_hedge_cycle_window_end_too_late',
        'action.place_order biased_hedge_v1 requires trigger cycleWindowEndSec <= 240.'
      );
    }
    const biasedHedge = nestedConfig(config.biasedHedge);
    const biasedHedgeStop = nestedConfig(config.biasedHedgeStop);
    const primaryBudgetUsdc = toFiniteNumber(biasedHedge.primaryBudgetUsdc);
    const hedgeBudgetUsdc = toFiniteNumber(biasedHedge.hedgeBudgetUsdc);
    const minDominantShare = toFiniteNumber(biasedHedge.minDominantShare);
    const maxHedgeSpendRatio = toFiniteNumber(biasedHedge.maxHedgeSpendRatio);
    const highPriceCent = toFiniteNumber(biasedHedge.highPriceCent);
    const maxPriceCent = toFiniteNumber(config.maxPriceCent);
    const highPriceMinFinalQ = toFiniteNumber(biasedHedge.highPriceMinFinalQ);
    const hedgeMaxPriceCent = toFiniteNumber(biasedHedge.hedgeMaxPriceCent);
    const maxSideSwitchCount = toFiniteNumber(biasedHedge.maxSideSwitchCount);
    if (primaryBudgetUsdc == null || primaryBudgetUsdc <= 0) {
      pushNodeError(issues, node, 'biased_hedge_invalid_primary_budget', 'biasedHedge.primaryBudgetUsdc must be > 0.');
    }
    if (hedgeBudgetUsdc == null || hedgeBudgetUsdc <= 0) {
      pushNodeError(issues, node, 'biased_hedge_invalid_hedge_budget', 'biasedHedge.hedgeBudgetUsdc must be > 0.');
    }
    if (primaryBudgetUsdc != null && hedgeBudgetUsdc != null && hedgeBudgetUsdc >= primaryBudgetUsdc) {
      pushNodeError(issues, node, 'biased_hedge_hedge_budget_must_be_smaller', 'biasedHedge.hedgeBudgetUsdc must be less than primaryBudgetUsdc.');
    }
    if (minDominantShare == null || minDominantShare < 0.70 || minDominantShare >= 1) {
      pushNodeError(issues, node, 'biased_hedge_invalid_min_dominant_share', 'biasedHedge.minDominantShare must be in [0.70, 1).');
    }
    if (maxHedgeSpendRatio == null || maxHedgeSpendRatio <= 0 || maxHedgeSpendRatio > 0.35) {
      pushNodeError(issues, node, 'biased_hedge_invalid_max_hedge_spend_ratio', 'biasedHedge.maxHedgeSpendRatio must be in (0, 0.35].');
    }
    if (
      maxHedgeSpendRatio != null &&
      minDominantShare != null &&
      maxHedgeSpendRatio > (1 - minDominantShare) / minDominantShare
    ) {
      pushNodeError(issues, node, 'biased_hedge_ratio_breaks_dominance', 'biasedHedge.maxHedgeSpendRatio cannot break minDominantShare.');
    }
    if (toBooleanish(biasedHedge.hedgeOnlyIfPrimaryFilled) !== true) {
      pushNodeError(issues, node, 'biased_hedge_requires_primary_fill_before_hedge', 'biasedHedge.hedgeOnlyIfPrimaryFilled must be true.');
    }
    if (highPriceMinFinalQ == null || highPriceMinFinalQ < 0.78) {
      pushNodeError(issues, node, 'biased_hedge_high_price_min_q_too_low', 'biasedHedge.highPriceMinFinalQ must be >= 0.78.');
    }
    if (highPriceCent != null && maxPriceCent != null && highPriceCent > maxPriceCent) {
      pushNodeError(issues, node, 'biased_hedge_high_price_above_max_price', 'biasedHedge.highPriceCent cannot be greater than maxPriceCent.');
    }
    if (hedgeMaxPriceCent != null && highPriceCent != null && hedgeMaxPriceCent >= highPriceCent) {
      pushNodeError(issues, node, 'biased_hedge_hedge_max_above_high_price', 'biasedHedge.hedgeMaxPriceCent must be below highPriceCent.');
    }
    if (maxSideSwitchCount != null && maxSideSwitchCount > 1) {
      pushNodeError(issues, node, 'biased_hedge_side_switch_limit_too_high', 'biasedHedge.maxSideSwitchCount must be <= 1.');
    }
    if (toBooleanish(biasedHedgeStop.biasInvalidationEnabled) !== true) {
      pushNodeError(issues, node, 'biased_hedge_stop_required', 'biasedHedgeStop.biasInvalidationEnabled must be true.');
    }
    const exitPctOnInvalidation = toFiniteNumber(biasedHedgeStop.exitPctOnInvalidation);
    if (exitPctOnInvalidation == null || exitPctOnInvalidation <= 0) {
      pushNodeError(issues, node, 'biased_hedge_invalid_exit_pct', 'biasedHedgeStop.exitPctOnInvalidation must be > 0.');
    }
    const timeExitRules = parseBiasedTimeExitRules(biasedHedgeStop.timeExitRules);
    if (!timeExitRules || timeExitRules.length === 0) {
      pushNodeError(issues, node, 'biased_hedge_time_exit_required', 'biasedHedgeStop.timeExitRules cannot be empty.');
    }
    const explicitIvWindows = parseValidIvTimeRuleWindows(config.priceToBeatIvTimeRules);
    const entryIvWindow = biasedHedgeEntryRemainingWindow(directTriggerConfig, biasedHedge);
    if (
      explicitIvWindows.length > 0 &&
      entryIvWindow != null &&
      !explicitIvWindows.some((rule) => remainingWindowsOverlap(rule, entryIvWindow))
    ) {
      pushNodeError(
        issues,
        node,
        'biased_hedge_iv_time_rules_no_entry_overlap',
        'action.place_order biased_hedge_v1 priceToBeatIvTimeRules must overlap the early primary entry window.'
      );
    }
    const reentryMaxAttempts = toFiniteNumber(config.reentryMaxAttempts);
    const reentryCooldownSec = toFiniteNumber(config.reentryCooldownSec);
    if (reentryMaxAttempts != null && reentryMaxAttempts > 0 && reentryCooldownSec === 0) {
      pushNodeError(issues, node, 'biased_hedge_reentry_requires_cooldown', 'reentryCooldownSec cannot be 0 when reentryMaxAttempts > 0.');
    }
  }
  const sizeMode = toTrimmedString(config.sizeMode).toLowerCase();
  if (sizeMode !== 'usdc' || config.sizePct != null || config.sizePercent != null) {
    pushNodeError(
      issues,
      node,
      'pair_lock_requires_usdc_sizing',
      'action.place_order pair_lock requires sizeMode=usdc and does not support pct/shares sizing.'
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
  if (!usesBiasedHedge && (pairMaxTotalCent == null || pairMaxTotalCent <= 0 || pairMaxTotalCent >= 100)) {
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
    ['pairProtectiveUnwindEnabled', 'invalid_pair_protective_unwind_enabled'],
    ['pairIgnoreStopLossAfterLocked', 'invalid_pair_ignore_stop_loss_after_locked'],
    ['notifyOnPairLocked', 'invalid_notify_on_pair_locked'],
    ['notifyOnPairUnwind', 'invalid_notify_on_pair_unwind'],
    ['counterLegTpEnabled', 'invalid_counter_leg_tp_enabled'],
    ['counterLegNotifyOnTpHit', 'invalid_counter_leg_notify_on_tp_hit'],
    ['counterLegPriceToBeatGuardEnabled', 'invalid_counter_leg_ptb_guard_enabled'],
    ['counterLegExecutionFloorGuardEnabled', 'invalid_counter_leg_execution_floor_enabled'],
    ['counterLegRetryOnPriceToBeatGuardBlock', 'invalid_counter_leg_retry_on_ptb'],
    ['counterLegRetryOnExecutionFloorGuardBlock', 'invalid_counter_leg_retry_on_floor'],
    ['counterLegRetryOnMaxPriceBlock', 'invalid_counter_leg_retry_on_max'],
    ['counterLegSlEnabled', 'invalid_counter_leg_sl_enabled'],
    ['counterLegPtbStopLossEnabled', 'invalid_counter_leg_ptb_stop_loss_enabled'],
    ['counterLegNotifyOnSlHit', 'invalid_counter_leg_notify_on_sl_hit'],
  ] as const) {
    if (config[key] != null && toBooleanish(config[key]) == null) {
      pushNodeError(issues, node, code, `action.place_order ${key} must be boolean (true/false).`);
    }
  }

  const counterLegTpEnabled = toBooleanish(config.counterLegTpEnabled);
  const counterLegNotifyOnTpHit = toBooleanish(config.counterLegNotifyOnTpHit);
  const counterLegTpPrice = resolveConfiguredBinaryPrice(config.counterLegTpPriceCent, null);
  const parsedCounterLegTpRules = parseExitLadderRules(config.counterLegTpRules);
  const hasCounterLegTpRules = parsedCounterLegTpRules.validRules.length > 0;

  if (
    config.counterLegTpPriceCent != null &&
    (!counterLegTpPrice.provided || counterLegTpPrice.value == null)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_counter_leg_tp_price_cent',
      'action.place_order counterLegTpPriceCent must be in (0, 100] when provided.'
    );
  }
  if (parsedCounterLegTpRules.isArray && parsedCounterLegTpRules.validRules.length > 5) {
    pushNodeError(
      issues,
      node,
      'invalid_counter_leg_tp_rules_length',
      'action.place_order counterLegTpRules cannot contain more than 5 entries.'
    );
  }
  if (parsedCounterLegTpRules.invalidItem) {
    pushNodeError(
      issues,
      node,
      'invalid_counter_leg_tp_rules',
      'action.place_order counterLegTpRules entries must provide priceCent in (0, 100] and sizePct in (0, 100].'
    );
  }
  if (hasCounterLegTpRules) {
    const counterLegTpRulesSum = parsedCounterLegTpRules.validRules.reduce(
      (sum, item) => sum + item.sizePct,
      0
    );
    if (Math.abs(counterLegTpRulesSum - 100) > 0.000001) {
      pushNodeError(
        issues,
        node,
        'invalid_counter_leg_tp_rules_sum',
        'action.place_order counterLegTpRules total sizePct must equal 100.'
      );
    }
    for (let index = 1; index < parsedCounterLegTpRules.validRules.length; index += 1) {
      if (
        parsedCounterLegTpRules.validRules[index - 1].priceCent >=
        parsedCounterLegTpRules.validRules[index].priceCent
      ) {
        pushNodeError(
          issues,
          node,
          'invalid_counter_leg_tp_rules_order',
          'action.place_order counterLegTpRules priceCent values must be strictly increasing.'
        );
        break;
      }
    }
  }
  if (counterLegTpEnabled === true && !counterLegTpPrice.provided && !hasCounterLegTpRules) {
    pushNodeError(
      issues,
      node,
      'counter_leg_tp_requires_price_or_rules',
      'action.place_order counterLegTpEnabled requires counterLegTpPriceCent or counterLegTpRules.'
    );
  }
  if (
    counterLegNotifyOnTpHit === true &&
    counterLegTpEnabled !== true &&
    !hasCounterLegTpRules
  ) {
    pushNodeError(
      issues,
      node,
      'counter_leg_notify_on_tp_hit_requires_take_profit',
      'action.place_order counterLegNotifyOnTpHit requires counterLegTpEnabled=true or counterLegTpRules.'
    );
  }

  const counterLegSlEnabled = toBooleanish(config.counterLegSlEnabled);
  const counterLegPtbStopLossEnabled = toBooleanish(config.counterLegPtbStopLossEnabled);
  const counterLegNotifyOnSlHit = toBooleanish(config.counterLegNotifyOnSlHit);
  if (counterLegSlEnabled === true) {
    const counterLegSlPriceCent = toFiniteNumber(config.counterLegSlPriceCent);
    if (
      counterLegSlPriceCent == null ||
      counterLegSlPriceCent <= 0 ||
      counterLegSlPriceCent > 100
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_counter_leg_sl_price_cent',
        'action.place_order counterLegSlPriceCent must be in (0, 100] when counter leg SL is enabled.'
      );
    }
    const counterLegSlTriggerPriceMode = toTrimmedString(
      config.counterLegSlTriggerPriceMode
    ).toLowerCase();
    if (
      counterLegSlTriggerPriceMode &&
      !isSupportedStopLossTriggerPriceMode(counterLegSlTriggerPriceMode)
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_counter_leg_sl_trigger_price_mode',
        'action.place_order counterLegSlTriggerPriceMode must be best_bid, composite, composite_safe, composite_fast, or last_trade.'
      );
    }
  }

  if (counterLegPtbStopLossEnabled === true) {
    const counterLegPtbStopLossGapUsd = toFiniteNumber(config.counterLegPtbStopLossGapUsd);
    if (counterLegPtbStopLossGapUsd == null) {
      pushNodeError(
        issues,
        node,
        'invalid_counter_leg_ptb_stop_loss_gap_usd',
        'action.place_order counterLegPtbStopLossGapUsd must be set when counter leg PTB stop-loss is enabled.'
      );
    }
    const counterLegPtbStopLossGapUnit = normalizePtbStopLossGapUnitValue(
      config.counterLegPtbStopLossGapUnit
    );
    if (
      config.counterLegPtbStopLossGapUnit != null &&
      counterLegPtbStopLossGapUnit == null
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_counter_leg_ptb_stop_loss_gap_unit',
        'action.place_order counterLegPtbStopLossGapUnit must be usd or cent when provided.'
      );
    }
    const counterLegPtbStopLossTimeDecayMode = toTrimmedString(
      config.counterLegPtbStopLossTimeDecayMode
    ).toLowerCase();
    if (
      counterLegPtbStopLossTimeDecayMode &&
      !['tighten', 'relax', 'none'].includes(counterLegPtbStopLossTimeDecayMode)
    ) {
      pushNodeError(
        issues,
        node,
        'invalid_counter_leg_ptb_stop_loss_time_decay_mode',
        'action.place_order counterLegPtbStopLossTimeDecayMode must be tighten, relax, or none.'
      );
    }
  }

  if (
    counterLegNotifyOnSlHit === true &&
    counterLegSlEnabled !== true &&
    counterLegPtbStopLossEnabled !== true
  ) {
    pushNodeError(
      issues,
      node,
      'counter_leg_notify_on_sl_hit_requires_stop_loss',
      'action.place_order counterLegNotifyOnSlHit requires counterLegSlEnabled=true or counterLegPtbStopLossEnabled=true.'
    );
  }

  const primaryLegSizeUsdc = toFiniteNumber(config.sizeUsdc ?? config.targetNotionalUsdc);
  if (primaryLegSizeUsdc == null || primaryLegSizeUsdc <= 0) {
    pushNodeError(
      issues,
      node,
      'pair_lock_requires_size_usdc',
      'action.place_order pair_lock requires sizeUsdc/targetNotionalUsdc > 0 for primary sizing.'
    );
  }
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
  } else if (!usesEdgePairLock && !usesBiasedHedge) {
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
    if (counterPtbMode && !isPtbMode(counterPtbMode)) {
      pushNodeError(
        issues,
        node,
        'invalid_counter_leg_price_to_beat_mode',
        'action.place_order counterLegPriceToBeatMode must be manual, auto_last_3_avg_excursion, auto_vol_pct, signal_formula, or iv_mismatch_edge.'
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
        'action.place_order pair_lock allows primary staged SL plus primary/counter take profit, hard SL/PTB stop-loss, and basic re-entry fields; counter staged exits, time exits, and advanced re-entry fields are not supported.'
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
