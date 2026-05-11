import type { TradeFlowGraph, TradeFlowNode, TradeFlowValidationIssue } from '@/lib/types';
import {
  hasProvidedValue,
  isRecord,
  toFiniteNumber,
  toTrimmedString,
} from './shared';
import { pushNodeError, pushNodeWarning } from './validation-core';

const DCA_MARKET_SELECTION_MODES = new Set([
  'manual_slug',
  'manual_slug_list',
  'auto_group_top_n',
  'auto_scope',
]);

const DCA_SIDE_MODES = new Set([
  'one_sided',
  'two_sided_pair',
  'multi_outcome_basket',
]);

function directIncomingNodes(nodeKey: string, graph: TradeFlowGraph): TradeFlowNode[] {
  const nodeMap = new Map(graph.nodes.map((candidate) => [candidate.key, candidate]));
  return graph.edges
    .filter((edge) => edge.target === nodeKey)
    .map((edge) => nodeMap.get(edge.source))
    .filter((candidate): candidate is TradeFlowNode => !!candidate);
}

function upstreamMarketPriceTriggers(nodeKey: string, graph: TradeFlowGraph): TradeFlowNode[] {
  const nodeMap = new Map(graph.nodes.map((candidate) => [candidate.key, candidate]));
  const incomingByTarget = new Map<string, string[]>();
  for (const edge of graph.edges) {
    const incoming = incomingByTarget.get(edge.target) ?? [];
    incoming.push(edge.source);
    incomingByTarget.set(edge.target, incoming);
  }

  const visited = new Set<string>();
  const triggersByKey = new Map<string, TradeFlowNode>();
  const queue = [nodeKey];
  while (queue.length > 0) {
    const currentKey = queue.shift() as string;
    if (visited.has(currentKey)) continue;
    visited.add(currentKey);

    for (const sourceKey of incomingByTarget.get(currentKey) ?? []) {
      const sourceNode = nodeMap.get(sourceKey);
      if (!sourceNode) continue;
      if (sourceNode.type === 'trigger.market_price') {
        triggersByKey.set(sourceKey, sourceNode);
      }
      queue.push(sourceKey);
    }
  }

  return [...triggersByKey.values()];
}

function selectedOutcomeRows(config: Record<string, unknown>): Record<string, unknown>[] {
  return Array.isArray(config.selectedOutcomes)
    ? config.selectedOutcomes.filter((item): item is Record<string, unknown> => isRecord(item))
    : [];
}

function validSelectedOutcomeRows(config: Record<string, unknown>): Record<string, unknown>[] {
  return selectedOutcomeRows(config).filter((item) => {
    const slug = toTrimmedString(item.slug);
    const label = toTrimmedString(item.outcomeLabel || item.outcome || item.label);
    const tokenId = toTrimmedString(item.tokenId || item.token_id);
    return !!slug && !!label && !!tokenId;
  });
}

function pushRiskCapErrors(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  config: Record<string, unknown>
) {
  const perSlug = toFiniteNumber(config.maxTotalCostPerSlugUsdc);
  const allSlugs = toFiniteNumber(config.maxTotalCostAllSlugsUsdc);
  const maxActiveSlugs = toFiniteNumber(config.maxActiveSlugs) ?? 1;
  if (perSlug == null || perSlug <= 0) {
    pushNodeError(
      issues,
      node,
      'invalid_dca_max_total_cost_per_slug',
      'action.place_order dca_live_v1 requires maxTotalCostPerSlugUsdc > 0.'
    );
  }
  if (allSlugs == null || allSlugs <= 0) {
    pushNodeError(
      issues,
      node,
      'invalid_dca_max_total_cost_all_slugs',
      'action.place_order dca_live_v1 requires maxTotalCostAllSlugsUsdc > 0.'
    );
  }
  if (perSlug != null && allSlugs != null && allSlugs < perSlug) {
    pushNodeError(
      issues,
      node,
      'dca_all_slugs_cap_below_per_slug',
      'maxTotalCostAllSlugsUsdc must be >= maxTotalCostPerSlugUsdc.'
    );
  }
  if (
    perSlug != null &&
    allSlugs != null &&
    maxActiveSlugs != null &&
    maxActiveSlugs * perSlug > allSlugs
  ) {
    pushNodeWarning(
      issues,
      node,
      'dca_all_slugs_cap_below_active_slug_sum',
      'maxActiveSlugs * maxTotalCostPerSlugUsdc exceeds maxTotalCostAllSlugsUsdc; runtime cap will throttle active DCA legs.'
    );
  }
}

export function isDcaLivePlaceOrderConfig(config: Record<string, unknown>): boolean {
  return toTrimmedString(config.mode).toLowerCase() === 'dca_live_v1';
}

export function validateActionPlaceOrderDcaLiveConfig(
  issues: TradeFlowValidationIssue[],
  node: TradeFlowNode,
  graph: TradeFlowGraph,
  config: Record<string, unknown>,
  side: string,
  executionMode: string
) {
  if (!isDcaLivePlaceOrderConfig(config)) return;

  if (side !== 'buy') {
    pushNodeError(
      issues,
      node,
      'dca_live_requires_buy_side',
      'action.place_order dca_live_v1 only supports side=buy.'
    );
  }
  if (executionMode !== 'limit' && executionMode !== 'market') {
    pushNodeError(
      issues,
      node,
      'dca_live_requires_supported_execution',
      'action.place_order dca_live_v1 supports executionMode=limit or market.'
    );
  }

  const incomingNodes = directIncomingNodes(node.key, graph);
  if (incomingNodes.length > 0) {
    const marketPriceTriggers = upstreamMarketPriceTriggers(node.key, graph);
    if (marketPriceTriggers.length !== 1) {
      pushNodeError(
        issues,
        node,
        'dca_live_requires_single_market_price_binding',
        'action.place_order dca_live_v1 must have exactly one upstream trigger.market_price when it is trigger-bound.'
      );
    } else {
      const triggerConfig = isRecord(marketPriceTriggers[0].config) ? marketPriceTriggers[0].config : {};
      const bindingMode = toTrimmedString(triggerConfig.bindingMode).toLowerCase() || 'standard';
      if (bindingMode !== 'dca_live_only') {
        pushNodeError(
          issues,
          node,
          'dca_live_requires_dca_binding_mode',
          'action.place_order dca_live_v1 requires upstream trigger.market_price bindingMode=dca_live_only.'
        );
      }
    }
  }

  const marketSelectionMode = toTrimmedString(
    config.marketSelectionMode || config.dcaMarketSelectionMode
  ).toLowerCase();
  if (!DCA_MARKET_SELECTION_MODES.has(marketSelectionMode)) {
    pushNodeError(
      issues,
      node,
      'invalid_dca_market_selection_mode',
      'action.place_order dca_live_v1 marketSelectionMode must be manual_slug, manual_slug_list, auto_group_top_n, or auto_scope.'
    );
  }
  if (marketSelectionMode === 'manual_slug' && !toTrimmedString(config.manualSlug || config.marketSlug)) {
    pushNodeError(issues, node, 'missing_dca_manual_slug', 'manual_slug requires manualSlug or marketSlug.');
  }
  if (marketSelectionMode === 'manual_slug_list') {
    const manualSlugs = Array.isArray(config.manualSlugs) ? config.manualSlugs : [];
    const maxActiveSlugs = toFiniteNumber(config.maxActiveSlugs) ?? 1;
    if (manualSlugs.length < 1) {
      pushNodeError(issues, node, 'missing_dca_manual_slugs', 'manual_slug_list requires manualSlugs.');
    }
    if (maxActiveSlugs > manualSlugs.length) {
      pushNodeError(issues, node, 'invalid_dca_max_active_slugs', 'maxActiveSlugs cannot exceed manualSlugs length.');
    }
  }
  if (marketSelectionMode === 'auto_group_top_n') {
    const candidateLimit = toFiniteNumber(config.candidateSlugLimit) ?? 0;
    const maxActiveSlugs = toFiniteNumber(config.maxActiveSlugs) ?? 1;
    if (!toTrimmedString(config.marketGroup || config.autoGroup)) {
      pushNodeError(issues, node, 'missing_dca_market_group', 'auto_group_top_n requires marketGroup.');
    }
    if (candidateLimit < maxActiveSlugs) {
      pushNodeError(issues, node, 'invalid_dca_candidate_slug_limit', 'candidateSlugLimit must be >= maxActiveSlugs.');
    }
  }
  if (marketSelectionMode === 'auto_scope' && !toTrimmedString(config.marketScope)) {
    pushNodeError(issues, node, 'missing_dca_market_scope', 'auto_scope requires marketScope.');
  }

  const sideMode = toTrimmedString(config.sideMode || config.dcaSideMode).toLowerCase();
  const selectedOutcomes = validSelectedOutcomeRows(config);
  if (!DCA_SIDE_MODES.has(sideMode)) {
    pushNodeError(
      issues,
      node,
      'invalid_dca_side_mode',
      'action.place_order dca_live_v1 sideMode must be one_sided, two_sided_pair, or multi_outcome_basket.'
    );
  }
  if (sideMode === 'one_sided' && selectedOutcomes.length !== 1) {
    pushNodeError(issues, node, 'invalid_dca_one_sided_outcomes', 'one_sided DCA requires exactly 1 selected outcome.');
  }
  if (sideMode === 'two_sided_pair') {
    if (selectedOutcomes.length !== 2) {
      pushNodeError(issues, node, 'invalid_dca_pair_outcomes', 'two_sided_pair requires exactly 2 selected outcomes.');
    }
    const slugs = new Set(selectedOutcomes.map((row) => toTrimmedString(row.slug)));
    if (slugs.size > 1) {
      pushNodeError(issues, node, 'dca_pair_requires_same_slug', 'two_sided_pair requires both outcomes from the same slug.');
    }
    const targetPairCostCent = toFiniteNumber(config.targetPairCostCent);
    if (targetPairCostCent == null || targetPairCostCent <= 0 || targetPairCostCent >= 100) {
      pushNodeError(issues, node, 'invalid_dca_target_pair_cost', 'targetPairCostCent must be in (0, 100).');
    }
  }
  if (sideMode === 'multi_outcome_basket') {
    if (selectedOutcomes.length < 2) {
      pushNodeError(issues, node, 'invalid_dca_basket_outcomes', 'multi_outcome_basket requires at least 2 selected outcomes.');
    }
    if (
      hasProvidedValue(config.targetPairCostCent) ||
      hasProvidedValue(config.pairMaxTotalCent) ||
      hasProvidedValue(config.targetLockedProfitUsdc)
    ) {
      pushNodeError(
        issues,
        node,
        'dca_basket_disallows_pair_lock_fields',
        'multi_outcome_basket is not a binary pair; pair-cost and locked-profit fields must be disabled.'
      );
    }
  }

  const initialShares = toFiniteNumber(config.initialOrderShares ?? config.firstDcaShares ?? config.targetQty);
  if (initialShares == null || initialShares <= 0) {
    pushNodeError(
      issues,
      node,
      'invalid_dca_initial_shares',
      'action.place_order dca_live_v1 requires initialOrderShares, firstDcaShares, or targetQty > 0.'
    );
  }
  const levels = toFiniteNumber(config.dcaLevels);
  if (levels == null || !Number.isInteger(levels) || levels < 1 || levels > 20) {
    pushNodeError(issues, node, 'invalid_dca_levels', 'dcaLevels must be an integer in [1, 20].');
  }
  const maxOpenOrdersAllSlugs = toFiniteNumber(config.maxOpenOrdersAllSlugs);
  if (maxOpenOrdersAllSlugs == null || maxOpenOrdersAllSlugs <= 0) {
    pushNodeError(issues, node, 'invalid_dca_max_open_orders', 'maxOpenOrdersAllSlugs must be > 0.');
  }
  const hardStopLoss = toFiniteNumber(config.hardStopLossPriceCent);
  const stopLoss = toFiniteNumber(config.stopLossPriceCent);
  if (hardStopLoss != null && stopLoss != null && hardStopLoss > stopLoss) {
    pushNodeError(issues, node, 'invalid_dca_hard_stop_loss', 'hardStopLossPriceCent must be <= stopLossPriceCent.');
  }
  pushRiskCapErrors(issues, node, config);
}
