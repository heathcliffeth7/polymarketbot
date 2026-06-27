import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import {
  AVG_REBOUND_PAIRLOCK_RESCUE_BINDING_MODE,
  AVG_REBOUND_PAIRLOCK_RESCUE_MODE,
} from '@/lib/trade-flow-config-mappers/avg-rebound-pairlock-rescue';
import { findUniqueUpstreamMarketPriceTrigger } from './graph';
import { isRecord, toBooleanish, toFiniteNumber, toTrimmedString } from './shared';
import { pushNodeError } from './validation-core';

export function isAvgReboundPairlockRescuePlaceOrderConfig(
  config: Record<string, unknown>,
): boolean {
  return toTrimmedString(config.mode).toLowerCase() === AVG_REBOUND_PAIRLOCK_RESCUE_MODE;
}

function positiveDecimal(value: unknown): boolean {
  const numeric = toFiniteNumber(value);
  return numeric != null && numeric > 0;
}

function probabilityDecimal(value: unknown): boolean {
  const numeric = toFiniteNumber(value);
  return numeric != null && numeric > 0 && numeric < 1;
}

export function validateActionPlaceOrderAvgReboundPairlockRescueConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  graph: TradeFlowGraph,
  config: Record<string, unknown>,
  side: string,
  executionMode: string,
) {
  if (!isAvgReboundPairlockRescuePlaceOrderConfig(config)) return;

  if (side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'avg_rebound_pairlock_rescue_requires_buy_side',
      'action.place_order avg_rebound_pairlock_rescue_v1 only supports side=buy.'
    );
  }
  if (executionMode !== 'limit') {
    pushNodeError(
      issues,
      node,
      'avg_rebound_pairlock_rescue_requires_limit',
      'action.place_order avg_rebound_pairlock_rescue_v1 only supports executionMode=limit.'
    );
  }
  if (toTrimmedString(config.orderType).toUpperCase() !== 'FOK') {
    pushNodeError(
      issues,
      node,
      'avg_rebound_pairlock_rescue_requires_fok',
      'action.place_order avg_rebound_pairlock_rescue_v1 only supports orderType=FOK.'
    );
  }
  for (const key of ['tpEnabled', 'slEnabled', 'ptbStopLossEnabled']) {
    if (toBooleanish(config[key]) === true) {
      pushNodeError(
        issues,
        node,
        `avg_rebound_pairlock_rescue_${key}_disabled`,
        `action.place_order avg_rebound_pairlock_rescue_v1 requires ${key}=false.`
      );
    }
  }

  const triggerKey = findUniqueUpstreamMarketPriceTrigger(node.key, graph);
  const triggerNode = triggerKey
    ? graph.nodes.find((candidate) => candidate.key === triggerKey)
    : null;
  const triggerConfig = isRecord(triggerNode?.config) ? triggerNode.config : {};
  if (
    !triggerNode ||
    toTrimmedString(triggerConfig.bindingMode).toLowerCase() !==
      AVG_REBOUND_PAIRLOCK_RESCUE_BINDING_MODE
  ) {
    pushNodeError(
      issues,
      node,
      'avg_rebound_pairlock_rescue_requires_binding_trigger',
      'action.place_order avg_rebound_pairlock_rescue_v1 requires exactly one upstream trigger.market_price bindingMode=avg_rebound_pairlock_rescue_only.'
    );
  }
  if (toTrimmedString(triggerConfig.marketMode).toLowerCase() !== 'auto_scope') {
    pushNodeError(
      issues,
      node,
      'avg_rebound_pairlock_rescue_requires_auto_scope',
      'avg_rebound_pairlock_rescue_only trigger must use marketMode=auto_scope.'
    );
  }

  if (config.avgReboundPairlockRescue != null && !isRecord(config.avgReboundPairlockRescue)) {
    pushNodeError(
      issues,
      node,
      'invalid_avg_rebound_pairlock_rescue_config',
      'avgReboundPairlockRescue must be a JSON object.'
    );
    return;
  }
  const strategy = isRecord(config.avgReboundPairlockRescue)
    ? config.avgReboundPairlockRescue
    : {};
  if (
    strategy.sessionBudgetUsdc != null &&
    !positiveDecimal(strategy.sessionBudgetUsdc)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_avg_rebound_pairlock_rescue_budget',
      'avgReboundPairlockRescue.sessionBudgetUsdc must be > 0.'
    );
  }
  if (
    strategy.extraVwapSafetyBuffer != null &&
    toFiniteNumber(strategy.extraVwapSafetyBuffer) == null
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_avg_rebound_pairlock_rescue_vwap_buffer',
      'avgReboundPairlockRescue.extraVwapSafetyBuffer must be a decimal price unit.'
    );
  }
  if (Array.isArray(strategy.primaryLadder)) {
    for (const item of strategy.primaryLadder) {
      if (!isRecord(item) || !probabilityDecimal(item.priceCap) || !positiveDecimal(item.qty)) {
        pushNodeError(
          issues,
          node,
          'invalid_avg_rebound_pairlock_rescue_primary_ladder',
          'avgReboundPairlockRescue.primaryLadder entries require priceCap in (0,1) and qty > 0.'
        );
        break;
      }
    }
  }
}
