import type { TradeFlowGraph } from '@/lib/types';
import { isRecord } from './flow-engine-utils';

const DUAL_DCA_ALLOWED_ASSETS = new Set(['btc', 'eth', 'sol', 'xrp', 'doge', 'bnb', 'hype']);
const DUAL_DCA_ALLOWED_TIMEFRAMES = new Set(['5m', '15m']);

export function toPositiveNumber(value: unknown): number | null {
  const parsed =
    typeof value === 'number'
      ? value
      : typeof value === 'string'
        ? Number(value)
        : Number.NaN;
  if (!Number.isFinite(parsed) || parsed <= 0) return null;
  return parsed;
}

export function normalizeDualDcaAsset(
  config: Record<string, unknown>
): 'btc' | 'eth' | 'sol' | 'xrp' | 'doge' | 'bnb' | 'hype' | null {
  const raw = String(config.asset ?? config.coin ?? '')
    .trim()
    .toLowerCase();
  if (!DUAL_DCA_ALLOWED_ASSETS.has(raw)) return null;
  return raw as 'btc' | 'eth' | 'sol' | 'xrp' | 'doge' | 'bnb' | 'hype';
}

export function normalizeDualDcaTimeframe(
  config: Record<string, unknown>
): '5m' | '15m' | null {
  const raw = String(config.timeframe ?? config.marketPeriod ?? '')
    .trim()
    .toLowerCase();
  const normalized =
    raw === '5' || raw === '5min' || raw === '5 min'
      ? '5m'
      : raw === '15' || raw === '15min' || raw === '15 min'
        ? '15m'
        : raw;
  if (!DUAL_DCA_ALLOWED_TIMEFRAMES.has(normalized)) return null;
  return normalized as '5m' | '15m';
}

export function mergeGraphContextPatch(
  baseContext: unknown,
  patch: Record<string, unknown>
): Record<string, unknown> {
  const merged = { ...(isRecord(baseContext) ? baseContext : {}), ...patch };
  for (const [key, value] of Object.entries(patch)) {
    if (value === undefined) {
      delete merged[key];
    }
  }
  return merged;
}

export function resolveExistingDualDcaSourceTradeId(
  graph: TradeFlowGraph
): number | null {
  const contextSourceTradeId = toPositiveNumber(graph.context.sourceTradeId);
  if (contextSourceTradeId != null) {
    return contextSourceTradeId;
  }

  const nodeSourceTradeIds = graph.nodes
    .filter((node) => node.type === 'action.dual_dca')
    .map((node) => toPositiveNumber(isRecord(node.config) ? node.config.sourceTradeId : null))
    .filter((value): value is number => value != null);
  return nodeSourceTradeIds[0] ?? null;
}

export function applyDualDcaSourceTradeId(
  graph: TradeFlowGraph,
  sourceTradeId: number
): { graphJson: TradeFlowGraph; changed: boolean } {
  const contextSourceTradeId = toPositiveNumber(graph.context.sourceTradeId);
  let changed = contextSourceTradeId !== sourceTradeId;

  const nextContext =
    contextSourceTradeId === sourceTradeId
      ? graph.context
      : { ...graph.context, sourceTradeId };

  const nextNodes = graph.nodes.map((node) => {
    if (node.type !== 'action.dual_dca') return node;
    const currentNodeSourceTradeId = toPositiveNumber(
      isRecord(node.config) ? node.config.sourceTradeId : null
    );
    if (currentNodeSourceTradeId != null) return node;
    changed = true;
    return {
      ...node,
      config: {
        ...node.config,
        sourceTradeId,
      },
    };
  });

  return {
    graphJson: {
      ...graph,
      context: nextContext,
      nodes: nextNodes,
    },
    changed,
  };
}

export interface EnsureDualDcaSourceTradeInput {
  asset: 'btc' | 'eth' | 'sol' | 'xrp' | 'doge' | 'bnb' | 'hype';
  timeframe: '5m' | '15m';
  definitionId: number;
  nodeKey: string;
}

export interface EnsureDualDcaSourceTradeResponse {
  data: {
    sourceTradeId: number;
    created: boolean;
  };
}

export async function prepareDualDcaGraphForPublish(
  draftGraph: TradeFlowGraph,
  definitionId: number,
  ensureSourceTrade: (
    payload: EnsureDualDcaSourceTradeInput
  ) => Promise<EnsureDualDcaSourceTradeResponse>
): Promise<{ graphJson: TradeFlowGraph; sourceTradeId: number | null; created: boolean }> {
  const dualDcaNodes = draftGraph.nodes.filter((node) => node.type === 'action.dual_dca');
  if (dualDcaNodes.length === 0) {
    return { graphJson: draftGraph, sourceTradeId: null, created: false };
  }

  const existingSourceTradeId = resolveExistingDualDcaSourceTradeId(draftGraph);
  if (existingSourceTradeId != null) {
    const nextGraph = applyDualDcaSourceTradeId(draftGraph, existingSourceTradeId);
    return {
      graphJson: nextGraph.graphJson,
      sourceTradeId: existingSourceTradeId,
      created: false,
    };
  }

  const primaryDualNode = dualDcaNodes[0];
  const config = isRecord(primaryDualNode.config) ? primaryDualNode.config : {};
  const asset = normalizeDualDcaAsset(config);
  const timeframe = normalizeDualDcaTimeframe(config);
  if (!asset || !timeframe) {
    return { graphJson: draftGraph, sourceTradeId: null, created: false };
  }

  const ensured = await ensureSourceTrade({
    asset,
    timeframe,
    definitionId,
    nodeKey: primaryDualNode.key,
  });
  const ensuredSourceTradeId = toPositiveNumber(ensured.data.sourceTradeId);
  if (ensuredSourceTradeId == null) {
    throw new Error('Dual DCA sourceTradeId otomatik olusturulamadi.');
  }

  return {
    graphJson: applyDualDcaSourceTradeId(draftGraph, ensuredSourceTradeId).graphJson,
    sourceTradeId: ensuredSourceTradeId,
    created: Boolean(ensured.data.created),
  };
}
