import { MarkerType } from '@xyflow/react';
import type { TradeFlowEdge, TradeFlowNode } from '@/lib/types';
import {
  EDGE_LABEL_BG_COLOR,
  EDGE_LABEL_COLOR,
  EDGE_LABEL_COLORS,
  EDGE_STROKE_COLOR,
  type FlowEdge,
  type FlowNode,
  type NodePaletteCategory,
  type PlaceOrderPresetKind,
  type PlaceOrderPresetSeed,
} from './flow-canvas-constants';

export function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

export function cloneRecord(value: unknown): Record<string, unknown> {
  if (!isRecord(value)) return {};
  return JSON.parse(JSON.stringify(value)) as Record<string, unknown>;
}

export function nodeKindTone(nodeType: string): string {
  if (nodeType.startsWith('trigger.')) return 'border-sky-200 bg-sky-50';
  if (nodeType.startsWith('logic.')) return 'border-amber-200 bg-amber-50';
  return 'border-emerald-200 bg-emerald-50';
}

export function minimapColor(node: FlowNode): string {
  if (node.data.nodeType.startsWith('trigger.')) return '#38bdf8';
  if (node.data.nodeType.startsWith('logic.')) return '#f59e0b';
  return '#34d399';
}

export function nodePaletteCategoryOf(nodeType: string): Exclude<NodePaletteCategory, 'all'> {
  if (nodeType.startsWith('trigger.')) return 'trigger';
  if (nodeType.startsWith('logic.')) return 'logic';
  return 'action';
}

export function toCanvasNode(node: TradeFlowNode): FlowNode {
  const x = Number(node.positionX);
  const y = Number(node.positionY);

  return {
    id: node.key,
    type: 'flowNode',
    position: {
      x: Number.isFinite(x) ? x : 80,
      y: Number.isFinite(y) ? y : 80,
    },
    data: {
      nodeType: String(node.type || 'action.notify'),
      config: cloneRecord(node.config),
    },
  };
}

export function toCanvasEdge(edge: TradeFlowEdge): FlowEdge {
  const edgeType = String(edge.type || 'default');
  const colorSet = EDGE_LABEL_COLORS[edgeType];
  const strokeColor = colorSet?.stroke ?? EDGE_STROKE_COLOR;
  const labelBg = colorSet?.bg ?? EDGE_LABEL_BG_COLOR;
  const labelText = colorSet?.text ?? EDGE_LABEL_COLOR;

  return {
    id: edge.key,
    type: 'smoothstep',
    source: edge.source,
    target: edge.target,
    markerEnd: {
      type: MarkerType.ArrowClosed,
      color: strokeColor,
      width: 16,
      height: 16,
    },
    label: edgeType,
    data: {
      edgeType,
      condition: edge.condition && isRecord(edge.condition) ? cloneRecord(edge.condition) : null,
    },
    style: {
      stroke: strokeColor,
      strokeWidth: 1.6,
    },
    labelStyle: {
      fill: labelText,
      fontSize: 10,
      fontWeight: colorSet ? 600 : 400,
    },
    labelBgStyle: {
      fill: labelBg,
      fillOpacity: 1,
    },
    labelBgBorderRadius: 6,
  };
}

export function toDomainNode(node: FlowNode): TradeFlowNode {
  return {
    key: node.id,
    type: node.data.nodeType,
    positionX: Math.round(node.position.x),
    positionY: Math.round(node.position.y),
    config: cloneRecord(node.data.config),
  };
}

export function toDomainEdge(edge: FlowEdge): TradeFlowEdge {
  return {
    key: edge.id,
    source: edge.source,
    target: edge.target,
    type: edge.data?.edgeType || 'default',
    condition: edge.data?.condition ? cloneRecord(edge.data.condition) : null,
  };
}

function keyPrefixFromNodeType(type: string): string {
  if (type.startsWith('trigger.')) return 'trigger';
  if (type.startsWith('logic.')) return 'logic';
  return 'action';
}

export function createNodeKey(type: string, existing: Set<string>): string {
  const prefix = keyPrefixFromNodeType(type);
  for (let i = 0; i < 40; i += 1) {
    const candidate = `${prefix}_${Math.random().toString(36).slice(2, 8)}`;
    if (!existing.has(candidate)) return candidate;
  }
  let fallback = 1;
  while (existing.has(`${prefix}_${fallback}`)) fallback += 1;
  return `${prefix}_${fallback}`;
}

export function createEdgeKey(existing: Set<string>): string {
  for (let i = 0; i < 40; i += 1) {
    const candidate = `edge_${Math.random().toString(36).slice(2, 8)}`;
    if (!existing.has(candidate)) return candidate;
  }
  let fallback = 1;
  while (existing.has(`edge_${fallback}`)) fallback += 1;
  return `edge_${fallback}`;
}

function normalizedPosition(value: number | null | undefined): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return 0;
  return Math.round(parsed);
}

export function createGraphFingerprint(nodes: TradeFlowNode[], edges: TradeFlowEdge[]): string {
  const nodePart = nodes
    .map(
      (node) =>
        `${node.key}:${node.type}:${normalizedPosition(node.positionX)}:${normalizedPosition(
          node.positionY
        )}`
    )
    .sort()
    .join('|');

  const edgePart = edges
    .map((edge) => `${edge.key}:${edge.source}:${edge.target}:${edge.type}`)
    .sort()
    .join('|');

  return `${nodePart}__${edgePart}`;
}

export function toTrimmedStringValue(value: unknown): string {
  if (typeof value === 'string') return value.trim();
  if (typeof value === 'number' || typeof value === 'boolean') return String(value).trim();
  return '';
}

export function toFiniteNumberValue(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return null;
}

export function buildPlaceOrderPresetConfig(
  kind: PlaceOrderPresetKind,
  seed: PlaceOrderPresetSeed
): Record<string, unknown> {
  const isQuickPresetBuySell =
    kind === 'sell_current_position' || kind === 'buy_current_position';
  const config: Record<string, unknown> = {
    presetKind: kind,
    side: kind === 'sell_current_position' ? 'sell' : kind === 'buy_current_position' ? 'buy' : '',
    executionMode: isQuickPresetBuySell ? 'market' : kind === 'place_order' ? '' : 'limit',
    kind: 'immediate',
    sizeMode: 'pct',
    sizePct: 100,
    minPriceDistanceCent: 1,
    maxTriggers: 1,
    refKey:
      kind === 'sell_current_position'
        ? 'preset_sell_current_position'
        : kind === 'buy_current_position'
          ? 'preset_buy_current_position'
          : 'preset_place_order',
  };

  if (seed.sourceTradeId != null && seed.sourceTradeId > 0) {
    config.sourceTradeId = seed.sourceTradeId;
  }
  if (seed.marketSlug) config.marketSlug = seed.marketSlug;
  if (seed.tokenId) config.tokenId = seed.tokenId;
  if (seed.outcomeLabel) config.outcomeLabel = seed.outcomeLabel;

  return config;
}

export function hasRequiredPlaceOrderSeed(seed: PlaceOrderPresetSeed): boolean {
  return seed.sourceTradeId != null && seed.sourceTradeId > 0 && !!seed.marketSlug && !!seed.tokenId;
}

export function normalizeDateTimeInput(value: string): string {
  if (!value.trim()) return '';
  if (/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(value.trim())) return value.trim();
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return '';
  const year = parsed.getFullYear();
  const month = `${parsed.getMonth() + 1}`.padStart(2, '0');
  const day = `${parsed.getDate()}`.padStart(2, '0');
  const hour = `${parsed.getHours()}`.padStart(2, '0');
  const minute = `${parsed.getMinutes()}`.padStart(2, '0');
  return `${year}-${month}-${day}T${hour}:${minute}`;
}

export function toShortNodeLabel(value: string, max = 26): string {
  const trimmed = value.trim();
  if (!trimmed) return '-';
  if (trimmed.length <= max) return trimmed;
  return `${trimmed.slice(0, max)}...`;
}

export function openPositionNodeLabel(config: Record<string, unknown>): string | null {
  const outcome =
    typeof config.outcomeLabel === 'string' ? config.outcomeLabel.trim().toUpperCase() : '';
  const marketSlug = typeof config.marketSlug === 'string' ? config.marketSlug.trim() : '';
  const tokenId = typeof config.tokenId === 'string' ? config.tokenId.trim() : '';
  const marketPart = marketSlug || tokenId;

  if (!marketPart) return null;
  if (outcome) return `${outcome} | ${toShortNodeLabel(marketPart)}`;
  return toShortNodeLabel(marketPart);
}

export function placeOrderNodeLabel(config: Record<string, unknown>): string {
  const side = typeof config.side === 'string' ? config.side.trim().toLowerCase() : '';
  const executionMode =
    typeof config.executionMode === 'string' ? config.executionMode.trim().toLowerCase() : '';

  const sideLabel = side === 'sell' ? 'Sat' : side === 'buy' ? 'Al' : '';
  const modeLabel = executionMode === 'market' ? 'Market' : executionMode === 'limit' ? 'Limit' : '';

  if (modeLabel && sideLabel) return `${modeLabel} ${sideLabel}`;
  if (sideLabel) return `${sideLabel} (mod sec)`;
  if (modeLabel) return `${modeLabel} Al / Sat`;
  return 'Mevcut Pozisyonu Al / Sat';
}

export function dualDcaNodeLabel(config: Record<string, unknown>): string {
  const marketPeriod =
    typeof config.marketPeriod === 'string' ? config.marketPeriod.trim().toLowerCase() : '';
  const timeframeRaw =
    typeof config.timeframe === 'string' ? config.timeframe.trim().toLowerCase() : '';
  const timeframe = timeframeRaw || marketPeriod || '5m';

  const assetRaw = typeof config.asset === 'string' ? config.asset.trim().toUpperCase() : '';
  const coinRaw = typeof config.coin === 'string' ? config.coin.trim().toUpperCase() : '';
  const asset = assetRaw || coinRaw || 'BTC';

  const sideModeRaw =
    typeof config.sideMode === 'string' ? config.sideMode.trim().toLowerCase() : '';
  const sideRaw = typeof config.side === 'string' ? config.side.trim().toLowerCase() : '';
  const sideMode = sideModeRaw || sideRaw || 'all';
  const sideLabel =
    sideMode === 'up' ? 'UP' : sideMode === 'down' ? 'DOWN' : 'ALL';

  const dcaLevelsRaw =
    typeof config.dcaLevels === 'number'
      ? config.dcaLevels
      : typeof config.dcaLevels === 'string'
        ? Number(config.dcaLevels)
        : null;
  const dcaLevels =
    dcaLevelsRaw != null && Number.isFinite(dcaLevelsRaw) && dcaLevelsRaw > 0
      ? Math.floor(dcaLevelsRaw)
      : 1;
  const totalLevels = dcaLevels + 1;

  return `${asset} ${timeframe} | ${sideLabel} | ${totalLevels} lvl`;
}

export function resolveMarketNodeLabel(config: Record<string, unknown>): string {
  const outcome =
    typeof config.outcomeLabel === 'string' ? config.outcomeLabel.trim().toUpperCase() : '';
  const assetRaw = typeof config.asset === 'string' ? config.asset.trim().toUpperCase() : '';
  const timeframeRaw = typeof config.timeframe === 'string' ? config.timeframe.trim() : '';
  const marketScope = typeof config.marketScope === 'string' ? config.marketScope.trim().toLowerCase() : '';
  const legacyScopeMap: Record<string, { asset: string; timeframe: string }> = {
    btc_5m_updown: { asset: 'BTC', timeframe: '5m' },
    btc_15m_updown: { asset: 'BTC', timeframe: '15m' },
    eth_5m_updown: { asset: 'ETH', timeframe: '5m' },
    eth_15m_updown: { asset: 'ETH', timeframe: '15m' },
    sol_5m_updown: { asset: 'SOL', timeframe: '5m' },
    sol_15m_updown: { asset: 'SOL', timeframe: '15m' },
    xrp_5m_updown: { asset: 'XRP', timeframe: '5m' },
    xrp_15m_updown: { asset: 'XRP', timeframe: '15m' },
  };
  const legacy = marketScope ? legacyScopeMap[marketScope] : undefined;
  const asset = assetRaw || legacy?.asset || 'BTC';
  const timeframe = timeframeRaw || legacy?.timeframe || '5m';
  const source = `${asset} ${timeframe}`.trim();
  if (outcome) return `${source} | ${outcome}`;
  return source;
}

export function autoLayoutNodes(
  nodes: FlowNode[],
  edges: FlowEdge[]
): FlowNode[] {
  if (nodes.length === 0) return nodes;

  const adjacency = new Map<string, string[]>();
  const inDegree = new Map<string, number>();
  for (const n of nodes) {
    adjacency.set(n.id, []);
    inDegree.set(n.id, 0);
  }
  for (const e of edges) {
    adjacency.get(e.source)?.push(e.target);
    inDegree.set(e.target, (inDegree.get(e.target) ?? 0) + 1);
  }

  const layers: string[][] = [];
  const visited = new Set<string>();
  let queue = nodes.filter((n) => (inDegree.get(n.id) ?? 0) === 0).map((n) => n.id);
  if (queue.length === 0) queue = [nodes[0].id];

  while (queue.length > 0) {
    layers.push([...queue]);
    for (const id of queue) visited.add(id);
    const next: string[] = [];
    for (const id of queue) {
      for (const target of adjacency.get(id) ?? []) {
        if (!visited.has(target) && !next.includes(target)) {
          next.push(target);
        }
      }
    }
    queue = next;
  }

  for (const n of nodes) {
    if (!visited.has(n.id)) {
      layers.push([n.id]);
      visited.add(n.id);
    }
  }

  const LAYER_GAP_X = 300;
  const NODE_GAP_Y = 120;
  const START_X = 80;
  const START_Y = 80;

  const positionMap = new Map<string, { x: number; y: number }>();
  for (let layerIdx = 0; layerIdx < layers.length; layerIdx++) {
    const layer = layers[layerIdx];
    const totalHeight = (layer.length - 1) * NODE_GAP_Y;
    const startY = START_Y + Math.max(0, (3 * NODE_GAP_Y - totalHeight) / 2);
    for (let nodeIdx = 0; nodeIdx < layer.length; nodeIdx++) {
      positionMap.set(layer[nodeIdx], {
        x: START_X + layerIdx * LAYER_GAP_X,
        y: startY + nodeIdx * NODE_GAP_Y,
      });
    }
  }

  return nodes.map((node) => {
    const pos = positionMap.get(node.id);
    if (!pos) return node;
    return { ...node, position: pos };
  });
}
