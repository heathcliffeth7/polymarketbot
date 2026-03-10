import type { TradeFlowEdge, TradeFlowGraph, TradeFlowNode } from '@/lib/types';
import { DEFAULT_GRAPH, isRecord, toTrimmedString } from './shared';

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
    config: isRecord(raw.config) ? raw.config : {},
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

  return {
    context: contextRaw,
    nodes,
    edges,
  };
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


export {
  toNode,
  toEdge,
  detectCycles,
  collectRootNodeKeys,
  collectReachableFromTriggers,
  hasUpstreamAutoScopeMarketTrigger,
};
