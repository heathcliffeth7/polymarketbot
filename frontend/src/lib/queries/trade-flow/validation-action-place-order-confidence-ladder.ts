import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import {
  CONFIDENCE_LADDER_BINDING_MODE,
  CONFIDENCE_LADDER_MODE,
} from '@/lib/trade-flow-config-mappers/confidence-ladder';
import { isRecord, toBooleanish, toFiniteNumber, toTrimmedString } from './shared';
import { findUniqueUpstreamMarketPriceTrigger } from './graph';
import { pushNodeError } from './validation-core';

function configNumber(
  config: Record<string, unknown>,
  ladder: Record<string, unknown>,
  key: string,
): number | null {
  return toFiniteNumber(ladder[key] ?? config[key]);
}

function nestedNumber(
  parent: Record<string, unknown>,
  childKey: string,
  key: string,
): number | null {
  const child = isRecord(parent[childKey]) ? parent[childKey] : {};
  return toFiniteNumber(child[key]);
}

function requirePositive(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: number | null,
  code: string,
  label: string,
) {
  if (value != null && value <= 0) {
    pushNodeError(issues, node, code, `confidenceLadder.${label} must be > 0.`);
  }
}

function requireProbability(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  value: number | null,
  code: string,
  label: string,
) {
  if (value != null && (value <= 0 || value >= 1)) {
    pushNodeError(issues, node, code, `confidenceLadder.${label} must be in (0, 1).`);
  }
}

export function isConfidenceLadderPlaceOrderConfig(config: Record<string, unknown>): boolean {
  return toTrimmedString(config.mode).toLowerCase() === CONFIDENCE_LADDER_MODE;
}

export function validateActionPlaceOrderConfidenceLadderConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  graph: TradeFlowGraph,
  config: Record<string, unknown>,
  side: string,
  executionMode: string,
) {
  if (!isConfidenceLadderPlaceOrderConfig(config)) return;

  if (side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'confidence_ladder_requires_buy_side',
      'action.place_order confidence_ladder_hedge_lock_v1 only supports side=buy.'
    );
  }
  if (executionMode !== 'market' && executionMode !== 'limit') {
    pushNodeError(
      issues,
      node,
      'confidence_ladder_requires_supported_execution',
      'action.place_order confidence_ladder_hedge_lock_v1 supports executionMode=market or limit.'
    );
  }
  for (const key of ['tpEnabled', 'slEnabled', 'ptbStopLossEnabled']) {
    if (toBooleanish(config[key]) === true) {
      pushNodeError(
        issues,
        node,
        `confidence_ladder_${key}_disabled`,
        `action.place_order confidence_ladder_hedge_lock_v1 requires ${key}=false.`
      );
    }
  }

  const triggerKey = findUniqueUpstreamMarketPriceTrigger(node.key, graph);
  const triggerNode = triggerKey
    ? graph.nodes.find((candidate) => candidate.key === triggerKey)
    : null;
  const triggerConfig = isRecord(triggerNode?.config) ? triggerNode.config : {};
  if (!triggerNode || toTrimmedString(triggerConfig.bindingMode).toLowerCase() !== CONFIDENCE_LADDER_BINDING_MODE) {
    pushNodeError(
      issues,
      node,
      'confidence_ladder_requires_binding_trigger',
      'action.place_order confidence_ladder_hedge_lock_v1 requires exactly one upstream trigger.market_price bindingMode=confidence_ladder_only.'
    );
  }
  if (toTrimmedString(triggerConfig.marketMode).toLowerCase() !== 'auto_scope') {
    pushNodeError(
      issues,
      node,
      'confidence_ladder_requires_auto_scope',
      'confidence_ladder_only trigger must use marketMode=auto_scope.'
    );
  }
  if (toTrimmedString(triggerConfig.marketScope).toLowerCase() !== 'btc_5m_updown') {
    pushNodeError(
      issues,
      node,
      'confidence_ladder_requires_btc_5m',
      'confidence_ladder_hedge_lock_v1 MVP only supports marketScope=btc_5m_updown.'
    );
  }

  if (config.confidenceLadder != null && !isRecord(config.confidenceLadder)) {
    pushNodeError(
      issues,
      node,
      'invalid_confidence_ladder_config',
      'confidenceLadder must be a JSON object.'
    );
    return;
  }
  const ladder = isRecord(config.confidenceLadder) ? config.confidenceLadder : {};

  requirePositive(issues, node, configNumber(config, ladder, 'baseProbeShares'), 'invalid_confidence_ladder_base_probe', 'baseProbeShares');
  requirePositive(issues, node, configNumber(config, ladder, 'maxLossPerMarketUsdc'), 'invalid_confidence_ladder_max_loss', 'maxLossPerMarketUsdc');
  requirePositive(issues, node, configNumber(config, ladder, 'maxTotalCostPerMarketUsdc'), 'invalid_confidence_ladder_max_cost', 'maxTotalCostPerMarketUsdc');
  requireProbability(issues, node, configNumber(config, ladder, 'maxSpread'), 'invalid_confidence_ladder_max_spread', 'maxSpread');
  requireProbability(issues, node, configNumber(config, ladder, 'dominanceGap'), 'invalid_confidence_ladder_dominance_gap', 'dominanceGap');
  requireProbability(issues, node, configNumber(config, ladder, 'chopProbabilityMax'), 'invalid_confidence_ladder_chop', 'chopProbabilityMax');
  requireProbability(issues, node, configNumber(config, ladder, 'hardNoChaseAbove'), 'invalid_confidence_ladder_no_chase', 'hardNoChaseAbove');
  requireProbability(issues, node, configNumber(config, ladder, 'takerFeeRate'), 'invalid_confidence_ladder_taker_fee', 'takerFeeRate');
  requireProbability(issues, node, configNumber(config, ladder, 'slippageBuffer'), 'invalid_confidence_ladder_slippage', 'slippageBuffer');

  const startSec = configNumber(config, ladder, 'entryWindowStartSec') ?? nestedNumber(ladder, 'entryWindow', 'startSec');
  const endSec = configNumber(config, ladder, 'entryWindowEndSec') ?? nestedNumber(ladder, 'entryWindow', 'endSec');
  if (
    (startSec != null && (!Number.isInteger(startSec) || startSec < 0)) ||
    (endSec != null && (!Number.isInteger(endSec) || endSec <= 0)) ||
    (startSec != null && endSec != null && startSec >= endSec)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_confidence_ladder_entry_window',
      'confidenceLadder entry window must be integer seconds with start >= 0 and start < end.'
    );
  }

  const hedge = isRecord(ladder.hedge) ? ladder.hedge : {};
  requireProbability(issues, node, toFiniteNumber(hedge.oppositePriceMax), 'invalid_confidence_ladder_hedge_price', 'hedge.oppositePriceMax');
  requireProbability(issues, node, toFiniteNumber(hedge.minReversalEdge), 'invalid_confidence_ladder_reversal_edge', 'hedge.minReversalEdge');
  requireProbability(issues, node, toFiniteNumber(hedge.profitLockPairCostMax), 'invalid_confidence_ladder_pair_cost', 'hedge.profitLockPairCostMax');
  requireProbability(issues, node, toFiniteNumber(hedge.strongProfitLockPairCost), 'invalid_confidence_ladder_strong_pair_cost', 'hedge.strongProfitLockPairCost');
  const damageMin = toFiniteNumber(hedge.damageControlPriceMin);
  const damageMax = toFiniteNumber(hedge.damageControlPriceMax);
  if (
    (damageMin != null || damageMax != null) &&
    (damageMin == null || damageMax == null || damageMin <= 0 || damageMax >= 1 || damageMin >= damageMax)
  ) {
    pushNodeError(
      issues,
      node,
      'invalid_confidence_ladder_damage_control_range',
      'confidenceLadder.hedge damage control prices must satisfy 0 < min < max < 1.',
    );
  }

  const stop = isRecord(ladder.stop) ? ladder.stop : {};
  const maxDirectionFlips = toFiniteNumber(stop.maxDirectionFlips ?? ladder.maxDirectionFlips);
  if (maxDirectionFlips != null && (!Number.isInteger(maxDirectionFlips) || maxDirectionFlips < 0)) {
    pushNodeError(
      issues,
      node,
      'invalid_confidence_ladder_max_direction_flips',
      'confidenceLadder.stop.maxDirectionFlips must be an integer >= 0.'
    );
  }
}
