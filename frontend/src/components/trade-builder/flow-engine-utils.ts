import type {
  TradeFlowDefinitionDetail,
  TradeFlowEdge,
  TradeFlowGraph,
  TradeFlowNode,
} from '@/lib/types';

export function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

function toFinitePosition(value: unknown, fallback: number): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function normalizeNode(raw: unknown, index: number): TradeFlowNode | null {
  if (!isRecord(raw)) return null;
  const key = String(raw.key ?? '').trim();
  const type = String(raw.type ?? '').trim();
  if (!key || !type) return null;

  return {
    key,
    type,
    positionX: toFinitePosition(raw.positionX, index * 220),
    positionY: toFinitePosition(raw.positionY, 80),
    config: isRecord(raw.config) ? JSON.parse(JSON.stringify(raw.config)) : {},
  };
}

function normalizeEdge(raw: unknown, index: number): TradeFlowEdge | null {
  if (!isRecord(raw)) return null;
  const source = String(raw.source ?? '').trim();
  const target = String(raw.target ?? '').trim();
  if (!source || !target) return null;

  return {
    key: String(raw.key ?? '').trim() || `edge_${index + 1}`,
    source,
    target,
    type: String(raw.type ?? 'default').trim() || 'default',
    condition:
      raw.condition && isRecord(raw.condition)
        ? (JSON.parse(JSON.stringify(raw.condition)) as Record<string, unknown>)
        : null,
  };
}

function normalizeGraph(graph: unknown): TradeFlowGraph {
  const source = isRecord(graph) ? graph : {};
  const nodesRaw = Array.isArray(source.nodes) ? source.nodes : [];
  const edgesRaw = Array.isArray(source.edges) ? source.edges : [];

  return {
    context: isRecord(source.context) ? JSON.parse(JSON.stringify(source.context)) : {},
    nodes: nodesRaw
      .map((node, index) => normalizeNode(node, index))
      .filter((node): node is TradeFlowNode => !!node),
    edges: edgesRaw
      .map((edge, index) => normalizeEdge(edge, index))
      .filter((edge): edge is TradeFlowEdge => !!edge),
  };
}

export function deepCloneGraph(graph: unknown): TradeFlowGraph {
  return normalizeGraph(graph);
}

function normalizeGraphForRead(graph: unknown): TradeFlowGraph | null {
  if (!graph) return null;
  return normalizeGraph(graph);
}

function stableSerialize(value: unknown): string {
  if (Array.isArray(value)) {
    return `[${value.map((item) => stableSerialize(item)).join(',')}]`;
  }
  if (isRecord(value)) {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableSerialize(value[key])}`)
      .join(',')}}`;
  }
  return JSON.stringify(value);
}

export function buildGraphFingerprint(graph: unknown): string | null {
  const normalized = normalizeGraphForRead(graph);
  if (!normalized) return null;
  return stableSerialize({
    context: isRecord(normalized.context) ? normalized.context : {},
    nodes: [...normalized.nodes]
      .map((node) => ({
        key: node.key,
        type: node.type,
        positionX: node.positionX,
        positionY: node.positionY,
        config: isRecord(node.config) ? node.config : {},
      }))
      .sort((a, b) => a.key.localeCompare(b.key)),
    edges: [...normalized.edges]
      .map((edge) => ({
        key: edge.key,
        source: edge.source,
        target: edge.target,
        type: edge.type,
        condition: edge.condition && isRecord(edge.condition) ? edge.condition : null,
      }))
      .sort((a, b) => a.key.localeCompare(b.key)),
  });
}

export interface TradeFlowGraphSummary {
  nodes: number;
  edges: number;
  triggers: number;
  actions: number;
  hasTelegramNotify: boolean;
}

export function summarizeTradeFlowGraph(
  graph: unknown
): TradeFlowGraphSummary | null {
  const normalized = normalizeGraphForRead(graph);
  if (!normalized) return null;
  return {
    nodes: normalized.nodes.length,
    edges: normalized.edges.length,
    triggers: normalized.nodes.filter((node) => node.type.startsWith('trigger.')).length,
    actions: normalized.nodes.filter((node) => node.type.startsWith('action.')).length,
    hasTelegramNotify: normalized.nodes.some((node) => node.type === 'action.telegram_notify'),
  };
}

export function createSellBuyIfElseTemplate(
  marketSlug: string | null,
  outcome: { token_id: string; label: string } | null
): TradeFlowGraph {
  return {
    context: {
      sourceTradeId: 0,
      marketSlug: marketSlug || '',
      tokenId: outcome?.token_id || '',
      outcomeLabel: outcome?.label || '',
    },
    nodes: [
      {
        key: 'trigger_market',
        type: 'trigger.market_price',
        positionX: 80,
        positionY: 180,
        config: {
          marketSlug: marketSlug || '',
          tokenId: outcome?.token_id || '',
          pollIntervalMs: 1000,
        },
      },
      {
        key: 'action_sell',
        type: 'action.place_order',
        positionX: 370,
        positionY: 120,
        config: {
          side: 'sell',
          executionMode: 'market',
          marketSlug: marketSlug || '',
          tokenId: outcome?.token_id || '',
          outcomeLabel: outcome?.label || '',
          sizeUsdc: 25,
          minPriceDistanceCent: 1,
          maxTriggers: 1,
        },
      },
      {
        key: 'logic_if_rebuy',
        type: 'logic.if',
        positionX: 660,
        positionY: 180,
        config: {
          expression: { '<=': [{ var: 'market_price' }, 45] },
          comment: 'If market_price <= 45 ise yeniden alisa gec, degilse bekleme koluna git.',
        },
      },
      {
        key: 'action_buy',
        type: 'action.place_order',
        positionX: 980,
        positionY: 110,
        config: {
          side: 'buy',
          executionMode: 'market',
          marketSlug: marketSlug || '',
          tokenId: outcome?.token_id || '',
          outcomeLabel: outcome?.label || '',
          sizeUsdc: 20,
          minPriceDistanceCent: 1,
          maxTriggers: 2,
        },
      },
      {
        key: 'action_wait',
        type: 'action.set_state',
        positionX: 980,
        positionY: 255,
        config: {
          statePatch: { state: 'waiting_reentry', reason: 'if_false_path' },
        },
      },
    ],
    edges: [
      { key: 'edge_1', source: 'trigger_market', target: 'action_sell', type: 'default', condition: null },
      { key: 'edge_2', source: 'action_sell', target: 'logic_if_rebuy', type: 'on_success', condition: null },
      { key: 'edge_3', source: 'logic_if_rebuy', target: 'action_buy', type: 'on_true', condition: null },
      { key: 'edge_4', source: 'logic_if_rebuy', target: 'action_wait', type: 'on_false', condition: null },
    ],
  };
}

export function buildDetailSnapshotKey(detail: TradeFlowDefinitionDetail | null): string | null {
  if (!detail?.draftVersion) return null;
  return `${detail.definition.id}:${detail.draftVersion.id}:${detail.definition.updated_at}`;
}
