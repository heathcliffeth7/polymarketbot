import type { TradeFlowEdge, TradeFlowGraph, TradeFlowNode } from '@/lib/types';
import { DEFAULT_GRAPH, isRecord, toTrimmedString } from './shared';
import { normalizeTriggerMarketPriceCycleWindowConfig } from '@/lib/trade-flow-config-mappers/cycle-window';

export interface FixedTriggerMarketResolution {
  kind: 'none' | 'single' | 'multiple';
  marketSlug: string | null;
  outcomeKind: 'none' | 'single' | 'multiple';
  tokenId: string | null;
  outcomeLabel: string | null;
  distinctMarketSlugs: string[];
  distinctOutcomeLabels: string[];
}

function isQuickPresetBuySellPlaceOrderConfig(config: Record<string, unknown>): boolean {
  const presetKindRaw = toTrimmedString(config.presetKind).toLowerCase();
  const refKeyRaw = toTrimmedString(config.refKey).toLowerCase();
  return (
    presetKindRaw === 'sell_current_position' ||
    presetKindRaw === 'buy_current_position' ||
    refKeyRaw === 'preset_sell_current_position' ||
    refKeyRaw === 'preset_buy_current_position'
  );
}

export function isGenericPlaceOrderPresetConfig(config: Record<string, unknown>): boolean {
  if (isQuickPresetBuySellPlaceOrderConfig(config)) {
    return false;
  }

  const presetKindRaw = toTrimmedString(config.presetKind).toLowerCase();
  const refKeyRaw = toTrimmedString(config.refKey).toLowerCase();
  return presetKindRaw === 'place_order' || refKeyRaw === 'preset_place_order';
}

function normalizePlaceOrderPresetRefKey(
  nodeKey: string,
  config: Record<string, unknown>,
  knownNodeKeys?: ReadonlySet<string>
): Record<string, unknown> {
  const refKeyRaw = toTrimmedString(config.refKey);
  if (isQuickPresetBuySellPlaceOrderConfig(config)) {
    return config;
  }

  if (!isGenericPlaceOrderPresetConfig(config)) {
    return config;
  }

  if (!refKeyRaw || refKeyRaw.toLowerCase() === 'preset_place_order') {
    return {
      ...config,
      refKey: nodeKey,
    };
  }

  if (knownNodeKeys && refKeyRaw !== nodeKey && knownNodeKeys.has(refKeyRaw)) {
    return {
      ...config,
      refKey: nodeKey,
    };
  }

  return config;
}

interface FixedTriggerSeed {
  marketSlug: string;
  outcomeKind: FixedTriggerMarketResolution['outcomeKind'];
  tokenId: string | null;
  outcomeLabel: string | null;
}

function extractFixedTriggerSeed(node: TradeFlowNode): FixedTriggerSeed | null {
  if (node.type !== 'trigger.market_price') {
    return null;
  }

  const config = isRecord(node.config) ? node.config : {};
  const marketMode = toTrimmedString(config.marketMode).toLowerCase();
  const marketSlug = toTrimmedString(config.marketSlug);
  if (marketMode === 'auto_scope' || !marketSlug) {
    return null;
  }

  const outcomeConditions = Array.isArray(config.outcomeConditions)
    ? config.outcomeConditions.filter((row): row is Record<string, unknown> => isRecord(row))
    : [];
  if (outcomeConditions.length > 1) {
    return {
      marketSlug,
      outcomeKind: 'multiple',
      tokenId: null,
      outcomeLabel: null,
    };
  }

  if (outcomeConditions.length === 1) {
    const tokenId = toTrimmedString(outcomeConditions[0].tokenId);
    const outcomeLabel = toTrimmedString(outcomeConditions[0].outcomeLabel);
    if (tokenId && outcomeLabel) {
      return {
        marketSlug,
        outcomeKind: 'single',
        tokenId,
        outcomeLabel,
      };
    }
    return {
      marketSlug,
      outcomeKind: 'multiple',
      tokenId: null,
      outcomeLabel: null,
    };
  }

  const tokenId = toTrimmedString(config.tokenId);
  const outcomeLabel = toTrimmedString(config.outcomeLabel);
  if (tokenId && outcomeLabel) {
    return {
      marketSlug,
      outcomeKind: 'single',
      tokenId,
      outcomeLabel,
    };
  }

  return {
    marketSlug,
    outcomeKind: 'none',
    tokenId: null,
    outcomeLabel: null,
  };
}

export function resolveUpstreamFixedTriggerMarket(
  nodeKey: string,
  graph: TradeFlowGraph
): FixedTriggerMarketResolution {
  const nodeMap = new Map(graph.nodes.map((node) => [node.key, node]));
  const incomingByTarget = new Map<string, string[]>();
  for (const edge of graph.edges) {
    const incoming = incomingByTarget.get(edge.target) ?? [];
    incoming.push(edge.source);
    incomingByTarget.set(edge.target, incoming);
  }

  const visited = new Set<string>();
  const distinctMarketSlugs = new Set<string>();
  const distinctOutcomePairs = new Set<string>();
  const queue = [nodeKey];
  let hasUnresolvedOutcome = false;

  while (queue.length > 0) {
    const current = queue.shift() as string;
    if (visited.has(current)) continue;
    visited.add(current);

    for (const sourceKey of incomingByTarget.get(current) ?? []) {
      const sourceNode = nodeMap.get(sourceKey);
      if (!sourceNode) continue;

      const seed = extractFixedTriggerSeed(sourceNode);
      if (seed) {
        distinctMarketSlugs.add(seed.marketSlug);
        if (
          seed.outcomeKind === 'single' &&
          seed.tokenId &&
          seed.outcomeLabel
        ) {
          distinctOutcomePairs.add(`${seed.tokenId}::${seed.outcomeLabel}`);
        } else {
          hasUnresolvedOutcome = true;
        }
      }

      queue.push(sourceKey);
    }
  }

  const resolvedMarketSlugs = Array.from(distinctMarketSlugs).sort();
  const distinctOutcomeLabels = Array.from(distinctOutcomePairs)
    .map((pair) => pair.split('::')[1] ?? '')
    .filter((label) => label.length > 0)
    .sort();

  if (resolvedMarketSlugs.length === 0) {
    return {
      kind: 'none',
      marketSlug: null,
      outcomeKind: 'none',
      tokenId: null,
      outcomeLabel: null,
      distinctMarketSlugs: resolvedMarketSlugs,
      distinctOutcomeLabels,
    };
  }

  if (resolvedMarketSlugs.length > 1) {
    return {
      kind: 'multiple',
      marketSlug: null,
      outcomeKind: 'multiple',
      tokenId: null,
      outcomeLabel: null,
      distinctMarketSlugs: resolvedMarketSlugs,
      distinctOutcomeLabels,
    };
  }

  if (!hasUnresolvedOutcome && distinctOutcomePairs.size === 1) {
    const [pair] = Array.from(distinctOutcomePairs);
    const [tokenId, outcomeLabel] = pair.split('::');
    return {
      kind: 'single',
      marketSlug: resolvedMarketSlugs[0],
      outcomeKind: 'single',
      tokenId: tokenId || null,
      outcomeLabel: outcomeLabel || null,
      distinctMarketSlugs: resolvedMarketSlugs,
      distinctOutcomeLabels,
    };
  }

  return {
    kind: 'single',
    marketSlug: resolvedMarketSlugs[0],
    outcomeKind:
      hasUnresolvedOutcome || distinctOutcomePairs.size > 1 ? 'multiple' : 'none',
    tokenId: null,
    outcomeLabel: null,
    distinctMarketSlugs: resolvedMarketSlugs,
    distinctOutcomeLabels,
  };
}

function normalizeNodeConfig(
  nodeKey: string,
  nodeType: string,
  config: Record<string, unknown>
): Record<string, unknown> {
  if (nodeType === 'action.place_order') {
    return normalizePlaceOrderPresetRefKey(nodeKey, config);
  }

  if (nodeType !== 'trigger.market_price') {
    return config;
  }

  const priceModeRaw = toTrimmedString(config.priceMode).toLowerCase();
  const validPriceModes = new Set([
    'composite',
    'midpoint',
    'raw',
    'last_trade',
    'site_display',
    'best_bid',
    'best_ask',
  ]);
  return {
    ...normalizeTriggerMarketPriceCycleWindowConfig(config),
    priceMode: validPriceModes.has(priceModeRaw) ? priceModeRaw : 'composite',
  };
}

function normalizePresetPlaceOrderNodes(graph: TradeFlowGraph): TradeFlowGraph {
  const knownNodeKeys = new Set(graph.nodes.map((node) => node.key));
  let changed = false;

  const nodes = graph.nodes.map((node) => {
    if (node.type !== 'action.place_order' || !isRecord(node.config)) {
      return node;
    }

    let nextConfig = normalizePlaceOrderPresetRefKey(node.key, node.config, knownNodeKeys);
    if (isGenericPlaceOrderPresetConfig(nextConfig)) {
      const resolution = resolveUpstreamFixedTriggerMarket(node.key, graph);
      if (resolution.kind === 'single' && resolution.marketSlug) {
        if (toTrimmedString(nextConfig.marketSlug) !== resolution.marketSlug) {
          nextConfig = { ...nextConfig, marketSlug: resolution.marketSlug };
        }

        if (
          resolution.outcomeKind === 'single' &&
          resolution.tokenId &&
          resolution.outcomeLabel
        ) {
          if (
            toTrimmedString(nextConfig.tokenId) !== resolution.tokenId ||
            toTrimmedString(nextConfig.outcomeLabel) !== resolution.outcomeLabel
          ) {
            nextConfig = {
              ...nextConfig,
              tokenId: resolution.tokenId,
              outcomeLabel: resolution.outcomeLabel,
            };
          }
        } else if ('tokenId' in nextConfig || 'outcomeLabel' in nextConfig) {
          nextConfig = { ...nextConfig };
          delete nextConfig.tokenId;
          delete nextConfig.outcomeLabel;
        }
      } else if (hasUpstreamAutoScopeMarketTrigger(node.key, graph)) {
        if (
          'marketSlug' in nextConfig ||
          'tokenId' in nextConfig ||
          'outcomeLabel' in nextConfig
        ) {
          nextConfig = { ...nextConfig };
          delete nextConfig.marketSlug;
          delete nextConfig.tokenId;
          delete nextConfig.outcomeLabel;
        }
      }
    }

    if (nextConfig === node.config) {
      return node;
    }

    changed = true;
    return {
      ...node,
      config: nextConfig,
    };
  });

  return changed ? { ...graph, nodes } : graph;
}

function toNode(raw: unknown, idx: number): TradeFlowNode | null {
  if (!isRecord(raw)) return null;
  const keyRaw = String(raw.key ?? '').trim();
  const typeRaw = String(raw.type ?? '').trim();
  if (!keyRaw || !typeRaw) return null;

  const positionX = Number(raw.positionX);
  const positionY = Number(raw.positionY);

  return {
    key: keyRaw,
    type: typeRaw,
    positionX: Number.isFinite(positionX) ? positionX : idx * 220,
    positionY: Number.isFinite(positionY) ? positionY : 80,
    config: normalizeNodeConfig(keyRaw, typeRaw, isRecord(raw.config) ? raw.config : {}),
  };
}

function toEdge(raw: unknown, idx: number): TradeFlowEdge | null {
  if (!isRecord(raw)) return null;
  const keyRaw = String(raw.key ?? '').trim() || `edge_${idx + 1}`;
  const sourceRaw = String(raw.source ?? '').trim();
  const targetRaw = String(raw.target ?? '').trim();
  if (!sourceRaw || !targetRaw) return null;

  return {
    key: keyRaw,
    source: sourceRaw,
    target: targetRaw,
    type: String(raw.type ?? 'default').trim() || 'default',
    condition: isRecord(raw.condition) ? raw.condition : null,
  };
}

export function normalizeTradeFlowGraph(graphJson: unknown): TradeFlowGraph {
  if (!isRecord(graphJson)) return DEFAULT_GRAPH;
  const contextRaw = isRecord(graphJson.context) ? graphJson.context : {};
  const nodesRaw = Array.isArray(graphJson.nodes) ? graphJson.nodes : [];
  const edgesRaw = Array.isArray(graphJson.edges) ? graphJson.edges : [];

  const nodes = nodesRaw
    .map((row, idx) => toNode(row, idx))
    .filter((row): row is TradeFlowNode => !!row);

  const edges = edgesRaw
    .map((row, idx) => toEdge(row, idx))
    .filter((row): row is TradeFlowEdge => !!row);

  const graph = {
    context: contextRaw,
    nodes,
    edges,
  };

  return normalizePresetPlaceOrderNodes(graph);
}

function detectCycles(nodes: TradeFlowNode[], edges: TradeFlowEdge[]): boolean {
  const adjacency = new Map<string, string[]>();
  for (const node of nodes) adjacency.set(node.key, []);
  for (const edge of edges) {
    const list = adjacency.get(edge.source);
    if (list) list.push(edge.target);
  }

  const visited = new Set<string>();
  const stack = new Set<string>();

  const dfs = (nodeKey: string): boolean => {
    if (stack.has(nodeKey)) return true;
    if (visited.has(nodeKey)) return false;
    visited.add(nodeKey);
    stack.add(nodeKey);

    for (const next of adjacency.get(nodeKey) || []) {
      if (dfs(next)) return true;
    }

    stack.delete(nodeKey);
    return false;
  };

  for (const node of nodes) {
    if (dfs(node.key)) return true;
  }

  return false;
}

function collectRootNodeKeys(nodes: TradeFlowNode[], edges: TradeFlowEdge[]): Set<string> {
  const incoming = new Set(edges.map((edge) => edge.target));
  return new Set(nodes.filter((node) => !incoming.has(node.key)).map((node) => node.key));
}

function collectReachableFromTriggers(nodes: TradeFlowNode[], edges: TradeFlowEdge[]): Set<string> {
  const adjacency = new Map<string, string[]>();
  for (const node of nodes) adjacency.set(node.key, []);
  for (const edge of edges) {
    const list = adjacency.get(edge.source);
    if (list) list.push(edge.target);
  }

  const triggerStarts = nodes
    .filter((node) => node.type.startsWith('trigger.'))
    .map((node) => node.key);
  const rootNodeKeys = collectRootNodeKeys(nodes, edges);
  const dualDcaRootStarts = nodes
    .filter((node) => node.type === 'action.dual_dca' && rootNodeKeys.has(node.key))
    .map((node) => node.key);
  const queue = triggerStarts.length > 0 ? triggerStarts : dualDcaRootStarts;
  const reachable = new Set<string>(queue);

  while (queue.length > 0) {
    const current = queue.shift() as string;
    for (const next of adjacency.get(current) || []) {
      if (reachable.has(next)) continue;
      reachable.add(next);
      queue.push(next);
    }
  }

  return reachable;
}

function hasUpstreamAutoScopeMarketTrigger(nodeKey: string, graph: TradeFlowGraph): boolean {
  const nodeMap = new Map(graph.nodes.map((node) => [node.key, node]));
  const incomingByTarget = new Map<string, string[]>();
  for (const edge of graph.edges) {
    const list = incomingByTarget.get(edge.target) ?? [];
    list.push(edge.source);
    incomingByTarget.set(edge.target, list);
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
      if (
        sourceNode.type === 'trigger.market_price' &&
        toTrimmedString((isRecord(sourceNode.config) ? sourceNode.config : {}).marketMode).toLowerCase() === 'auto_scope'
      ) {
        return true;
      }
      queue.push(sourceKey);
    }
  }

  return false;
}

function findUniqueUpstreamMarketPriceTrigger(
  nodeKey: string,
  graph: TradeFlowGraph
): string | null {
  const nodeMap = new Map(graph.nodes.map((node) => [node.key, node]));
  const incomingByTarget = new Map<string, string[]>();
  for (const edge of graph.edges) {
    const list = incomingByTarget.get(edge.target) ?? [];
    list.push(edge.source);
    incomingByTarget.set(edge.target, list);
  }

  const visited = new Set<string>();
  const queue = [nodeKey];
  let foundKey: string | null = null;
  while (queue.length > 0) {
    const current = queue.shift() as string;
    if (visited.has(current)) continue;
    visited.add(current);
    for (const sourceKey of incomingByTarget.get(current) ?? []) {
      const sourceNode = nodeMap.get(sourceKey);
      if (!sourceNode) continue;
      if (sourceNode.type === 'trigger.market_price') {
        if (foundKey && foundKey !== sourceKey) {
          return null;
        }
        foundKey = sourceKey;
      }
      queue.push(sourceKey);
    }
  }

  return foundKey;
}

function hasValidTriggerPriceConfig(config: Record<string, unknown>): boolean {
  const triggerPriceCent = Number(config.triggerPriceCent);
  if (Number.isFinite(triggerPriceCent) && triggerPriceCent > 0 && triggerPriceCent <= 100) {
    return true;
  }

  const triggerPrice = Number(config.triggerPrice);
  return Number.isFinite(triggerPrice) && triggerPrice > 0 && triggerPrice <= 1;
}

function hasValidOutcomeTriggerPrice(config: Record<string, unknown>): boolean {
  const rows = config.outcomeConditions;
  if (!Array.isArray(rows)) return false;

  return rows.some((row) => {
    if (!isRecord(row)) return false;
    const triggerCondition = toTrimmedString(row.triggerCondition).toLowerCase();
    if (
      triggerCondition !== 'cross_above' &&
      triggerCondition !== 'cross_below' &&
      triggerCondition !== 'level_above' &&
      triggerCondition !== 'level_below'
    ) {
      return false;
    }
    return hasValidTriggerPriceConfig(row);
  });
}

function hasUpstreamBuyPlaceOrderNode(nodeKey: string, graph: TradeFlowGraph): boolean {
  const nodeMap = new Map(graph.nodes.map((node) => [node.key, node]));
  const incomingByTarget = new Map<string, string[]>();
  for (const edge of graph.edges) {
    const list = incomingByTarget.get(edge.target) ?? [];
    list.push(edge.source);
    incomingByTarget.set(edge.target, list);
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
      if (
        sourceNode.type === 'action.place_order' &&
        toTrimmedString((isRecord(sourceNode.config) ? sourceNode.config : {}).side).toLowerCase() === 'buy'
      ) {
        return true;
      }
      queue.push(sourceKey);
    }
  }
  return false;
}

function hasUpstreamTriggerWithTriggerPrice(nodeKey: string, graph: TradeFlowGraph): boolean {
  const nodeMap = new Map(graph.nodes.map((node) => [node.key, node]));
  const incomingByTarget = new Map<string, string[]>();
  for (const edge of graph.edges) {
    const list = incomingByTarget.get(edge.target) ?? [];
    list.push(edge.source);
    incomingByTarget.set(edge.target, list);
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
      const sourceConfig = isRecord(sourceNode.config) ? sourceNode.config : {};
      if (
        (sourceNode.type === 'trigger.market_price' || sourceNode.type === 'trigger.open_positions') &&
        (hasValidTriggerPriceConfig(sourceConfig) || hasValidOutcomeTriggerPrice(sourceConfig))
      ) {
        return true;
      }
      queue.push(sourceKey);
    }
  }

  return false;
}


export {
  toNode,
  toEdge,
  detectCycles,
  collectRootNodeKeys,
  collectReachableFromTriggers,
  hasUpstreamAutoScopeMarketTrigger,
  hasUpstreamTriggerWithTriggerPrice,
  hasUpstreamBuyPlaceOrderNode,
  findUniqueUpstreamMarketPriceTrigger,
};
