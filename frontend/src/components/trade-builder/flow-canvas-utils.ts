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

export function createGraphFingerprint(nodes: TradeFlowNode[], edges: TradeFlowEdge[]): string {
  const nodePart = nodes
    .map(
      (node) =>
        `${node.key}:${node.type}:${normalizedPosition(node.positionX)}:${normalizedPosition(
          node.positionY
        )}:${stableSerialize(cloneRecord(node.config))}`
    )
    .sort()
    .join('|');

  const edgePart = edges
    .map(
      (edge) =>
        `${edge.key}:${edge.source}:${edge.target}:${edge.type}:${stableSerialize(
          edge.condition ? cloneRecord(edge.condition) : null
        )}`
    )
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

export function extractPositionSeedFromNode(node: FlowNode): PlaceOrderPresetSeed | null {
  if (
    node.data.nodeType !== 'trigger.open_positions' &&
    node.data.nodeType !== 'trigger.position_drawdown'
  ) {
    return null;
  }

  return {
    sourceTradeId: toFiniteNumberValue(node.data.config.sourceTradeId),
    marketSlug: toTrimmedStringValue(node.data.config.marketSlug),
    tokenId: toTrimmedStringValue(node.data.config.tokenId),
    outcomeLabel: toTrimmedStringValue(node.data.config.outcomeLabel),
  };
}

export function extractMarketPriceSeedFromNode(node: FlowNode): PlaceOrderPresetSeed | null {
  if (node.data.nodeType !== 'trigger.market_price') return null;

  const marketMode = toTrimmedStringValue(node.data.config.marketMode).toLowerCase();
  const marketSlug = toTrimmedStringValue(node.data.config.marketSlug);
  if (marketMode !== 'fixed' || !marketSlug) return null;

  let tokenId = '';
  let outcomeLabel = '';
  const outcomeConditions = Array.isArray(node.data.config.outcomeConditions)
    ? node.data.config.outcomeConditions
    : [];
  if (outcomeConditions.length === 1 && isRecord(outcomeConditions[0])) {
    tokenId = toTrimmedStringValue(outcomeConditions[0].tokenId);
    outcomeLabel = toTrimmedStringValue(outcomeConditions[0].outcomeLabel);
  }

  return {
    sourceTradeId: null,
    marketSlug,
    tokenId,
    outcomeLabel,
  };
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
  };

  if (kind === 'sell_current_position') {
    config.refKey = 'preset_sell_current_position';
  } else if (kind === 'buy_current_position') {
    config.refKey = 'preset_buy_current_position';
  }

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

  let label: string;
  if (modeLabel && sideLabel) label = `${modeLabel} ${sideLabel}`;
  else if (sideLabel) label = `${sideLabel} (mod sec)`;
  else if (modeLabel) label = `${modeLabel} Al / Sat`;
  else label = 'Mevcut Pozisyonu Al / Sat';

  if (config.tpEnabled === true || config.tpEnabled === 'true') {
    const tpCent = typeof config.tpPriceCent === 'number' ? config.tpPriceCent : null;
    label += tpCent != null ? ` | TP@${tpCent}c` : ' | TP';
  }
  if (config.slEnabled === true || config.slEnabled === 'true') {
    const slCent = typeof config.slPriceCent === 'number' ? config.slPriceCent : null;
    label += slCent != null ? ` | SL@${slCent}c` : ' | SL';
  }
  if (
    (config.ptbStopLossEnabled === true || config.ptbStopLossEnabled === 'true') &&
    !Array.isArray(config.ptbStopLossRules)
  ) {
    const rawGap =
      typeof config.ptbStopLossGapUsd === 'number'
        ? config.ptbStopLossGapUsd
        : typeof config.ptbStopLossGapUsd === 'string'
          ? Number(config.ptbStopLossGapUsd)
          : null;
    label +=
      rawGap != null && Number.isFinite(rawGap)
        ? ` | PTB-SL<=${rawGap}`
        : ' | PTB-SL';
  }
  if (Array.isArray(config.ptbStopLossRules) && config.ptbStopLossRules.length > 0) {
    label += ` | PTB-Kademe x${config.ptbStopLossRules.length}`;
  }
  return label;
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

export function hasUpstreamAutoScopeTrigger(
  nodeId: string,
  nodes: FlowNode[],
  edges: FlowEdge[],
): boolean {
  const nodeMap = new Map(nodes.map((n) => [n.id, n]));
  const incomingByTarget = new Map<string, string[]>();
  for (const edge of edges) {
    const list = incomingByTarget.get(edge.target) ?? [];
    list.push(edge.source);
    incomingByTarget.set(edge.target, list);
  }
  const visited = new Set<string>();
  const queue = [nodeId];
  while (queue.length > 0) {
    const current = queue.shift()!;
    if (visited.has(current)) continue;
    visited.add(current);
    for (const sourceId of incomingByTarget.get(current) ?? []) {
      const sourceNode = nodeMap.get(sourceId);
      if (!sourceNode) continue;
      if (
        sourceNode.data.nodeType === 'trigger.market_price' &&
        String(sourceNode.data.config?.marketMode ?? '').trim().toLowerCase() === 'auto_scope'
      ) {
        return true;
      }
      queue.push(sourceId);
    }
  }
  return false;
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
  if (!Array.isArray(config.outcomeConditions)) return false;

  return config.outcomeConditions.some((row) => {
    if (!isRecord(row)) return false;
    const triggerCondition = toTrimmedStringValue(row.triggerCondition).toLowerCase();
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

function formatCentValue(value: number): string {
  if (Number.isInteger(value)) return String(value);
  return value.toFixed(4).replace(/\.?0+$/, '');
}

function resolveConfiguredMaxPriceCent(
  maxPriceCentValue: unknown,
  legacyMaxPriceValue: unknown,
): string | null {
  const maxPriceCent = Number(maxPriceCentValue);
  if (Number.isFinite(maxPriceCent) && maxPriceCent > 0 && maxPriceCent <= 100) {
    return formatCentValue(maxPriceCent);
  }

  const maxPrice = Number(legacyMaxPriceValue);
  if (Number.isFinite(maxPrice) && maxPrice > 0 && maxPrice <= 1) {
    return formatCentValue(maxPrice * 100);
  }

  return null;
}

function collectTriggerMarketPriceMaxPriceCandidates(
  config: Record<string, unknown>,
): { values: string[]; hasUnpricedPath: boolean } {
  const outcomeConditions = Array.isArray(config.outcomeConditions) ? config.outcomeConditions : [];
  if (outcomeConditions.length > 0) {
    const values = new Set<string>();
    let hasUnpricedPath = false;
    for (const row of outcomeConditions) {
      if (!isRecord(row)) {
        hasUnpricedPath = true;
        continue;
      }
      const resolved = resolveConfiguredMaxPriceCent(row.maxPriceCent, row.maxPrice);
      if (resolved) {
        values.add(resolved);
      } else {
        hasUnpricedPath = true;
      }
    }
    return { values: Array.from(values), hasUnpricedPath };
  }

  const resolved = resolveConfiguredMaxPriceCent(config.maxPriceCent, config.maxPrice);
  return resolved
    ? { values: [resolved], hasUnpricedPath: false }
    : { values: [], hasUnpricedPath: true };
}

export function hasUpstreamTriggerWithConfiguredPrice(
  nodeId: string,
  nodes: FlowNode[],
  edges: FlowEdge[],
): boolean {
  const nodeMap = new Map(nodes.map((n) => [n.id, n]));
  const incomingByTarget = new Map<string, string[]>();
  for (const edge of edges) {
    const list = incomingByTarget.get(edge.target) ?? [];
    list.push(edge.source);
    incomingByTarget.set(edge.target, list);
  }

  const visited = new Set<string>();
  const queue = [nodeId];
  while (queue.length > 0) {
    const current = queue.shift()!;
    if (visited.has(current)) continue;
    visited.add(current);
    for (const sourceId of incomingByTarget.get(current) ?? []) {
      const sourceNode = nodeMap.get(sourceId);
      if (!sourceNode) continue;
      if (
        (sourceNode.data.nodeType === 'trigger.market_price' ||
          sourceNode.data.nodeType === 'trigger.open_positions') &&
        (hasValidTriggerPriceConfig(sourceNode.data.config) ||
          hasValidOutcomeTriggerPrice(sourceNode.data.config))
      ) {
        return true;
      }
      queue.push(sourceId);
    }
  }

  return false;
}

function formatOutcomePairKey(tokenId: string, outcomeLabel: string): string {
  return `${tokenId}::${outcomeLabel}`;
}

function parseOutcomePairKey(key: string): { tokenId: string; outcomeLabel: string } {
  const [tokenId = '', outcomeLabel = ''] = key.split('::');
  return { tokenId, outcomeLabel };
}

export interface UpstreamFixedMarketResolution {
  kind: 'none' | 'single' | 'multiple';
  marketSlug: string | null;
  outcomeKind: 'none' | 'single' | 'multiple';
  tokenId: string | null;
  outcomeLabel: string | null;
  distinctMarketSlugs: string[];
  distinctOutcomeLabels: string[];
}

export function resolveUpstreamFixedTriggerMarket(
  nodeId: string,
  nodes: FlowNode[],
  edges: FlowEdge[],
): UpstreamFixedMarketResolution {
  const nodeMap = new Map(nodes.map((n) => [n.id, n]));
  const incomingByTarget = new Map<string, string[]>();
  for (const edge of edges) {
    const list = incomingByTarget.get(edge.target) ?? [];
    list.push(edge.source);
    incomingByTarget.set(edge.target, list);
  }

  const visited = new Set<string>();
  const distinctMarketSlugs = new Set<string>();
  const distinctOutcomePairs = new Set<string>();
  let hasUnresolvedOutcome = false;
  const queue = [nodeId];

  while (queue.length > 0) {
    const current = queue.shift()!;
    if (visited.has(current)) continue;
    visited.add(current);
    for (const sourceId of incomingByTarget.get(current) ?? []) {
      const sourceNode = nodeMap.get(sourceId);
      if (!sourceNode) continue;
      const candidate = extractMarketPriceSeedFromNode(sourceNode);
      if (candidate) {
        distinctMarketSlugs.add(candidate.marketSlug);
        if (candidate.tokenId && candidate.outcomeLabel) {
          distinctOutcomePairs.add(
            formatOutcomePairKey(candidate.tokenId, candidate.outcomeLabel)
          );
        } else {
          hasUnresolvedOutcome = true;
        }
      }
      queue.push(sourceId);
    }
  }

  const resolvedMarketSlugs = Array.from(distinctMarketSlugs).sort();
  const distinctOutcomeLabels = Array.from(distinctOutcomePairs)
    .map((pair) => parseOutcomePairKey(pair).outcomeLabel)
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
    const resolvedOutcome = parseOutcomePairKey(Array.from(distinctOutcomePairs)[0]);
    return {
      kind: 'single',
      marketSlug: resolvedMarketSlugs[0],
      outcomeKind: 'single',
      tokenId: resolvedOutcome.tokenId,
      outcomeLabel: resolvedOutcome.outcomeLabel,
      distinctMarketSlugs: resolvedMarketSlugs,
      distinctOutcomeLabels,
    };
  }

  return {
    kind: 'single',
    marketSlug: resolvedMarketSlugs[0],
    outcomeKind:
      hasUnresolvedOutcome || distinctOutcomePairs.size > 0 ? 'multiple' : 'none',
    tokenId: null,
    outcomeLabel: null,
    distinctMarketSlugs: resolvedMarketSlugs,
    distinctOutcomeLabels,
  };
}

export interface UpstreamMaxPriceResolution {
  kind: 'none' | 'single' | 'multiple';
  maxPriceCent: string | null;
  distinctMaxPriceCents: string[];
}

export interface PairLockUpstreamTriggerSummary {
  nodeKey: string;
  bindingMode: 'standard' | 'pair_lock_only';
  marketMode: 'fixed' | 'auto_scope';
  marketSource: string;
  cycleWindowMode: string;
  cycleWindowSecs: string;
  cycleWindowStartSec: string;
  cycleWindowEndSec: string;
}

export function resolveUpstreamTriggerMaxPrice(
  nodeId: string,
  nodes: FlowNode[],
  edges: FlowEdge[],
): UpstreamMaxPriceResolution {
  const nodeMap = new Map(nodes.map((n) => [n.id, n]));
  const incomingByTarget = new Map<string, string[]>();
  for (const edge of edges) {
    const list = incomingByTarget.get(edge.target) ?? [];
    list.push(edge.source);
    incomingByTarget.set(edge.target, list);
  }

  const visited = new Set<string>();
  const distinctValues = new Set<string>();
  let hasUnpricedPath = false;
  const queue = [nodeId];
  while (queue.length > 0) {
    const current = queue.shift()!;
    if (visited.has(current)) continue;
    visited.add(current);
    for (const sourceId of incomingByTarget.get(current) ?? []) {
      const sourceNode = nodeMap.get(sourceId);
      if (!sourceNode) continue;
      if (sourceNode.data.nodeType === 'trigger.market_price') {
        const candidate = collectTriggerMarketPriceMaxPriceCandidates(sourceNode.data.config);
        for (const value of candidate.values) {
          distinctValues.add(value);
        }
        hasUnpricedPath = hasUnpricedPath || candidate.hasUnpricedPath;
      }
      queue.push(sourceId);
    }
  }

  const distinctMaxPriceCents = Array.from(distinctValues).sort(
    (left, right) => Number(left) - Number(right)
  );
  if (distinctMaxPriceCents.length === 0) {
    return {
      kind: 'none',
      maxPriceCent: null,
      distinctMaxPriceCents,
    };
  }

  if (distinctMaxPriceCents.length === 1 && !hasUnpricedPath) {
    return {
      kind: 'single',
      maxPriceCent: distinctMaxPriceCents[0],
      distinctMaxPriceCents,
    };
  }

  return {
    kind: 'multiple',
    maxPriceCent: null,
    distinctMaxPriceCents,
  };
}

export function resolveDirectUpstreamPairLockTrigger(
  nodeId: string,
  nodes: FlowNode[],
  edges: FlowEdge[],
): PairLockUpstreamTriggerSummary | null {
  const nodeMap = new Map(nodes.map((node) => [node.id, node]));
  const incoming = edges.filter((edge) => edge.target === nodeId);
  if (incoming.length !== 1) return null;
  const sourceNode = nodeMap.get(incoming[0].source);
  if (!sourceNode || sourceNode.data.nodeType !== 'trigger.market_price') return null;

  const config = sourceNode.data.config;
  const bindingMode =
    toTrimmedStringValue(config.bindingMode).toLowerCase() === 'pair_lock_only'
      ? 'pair_lock_only'
      : 'standard';
  const marketMode =
    toTrimmedStringValue(config.marketMode).toLowerCase() === 'auto_scope'
      ? 'auto_scope'
      : 'fixed';
  const marketSource =
    marketMode === 'auto_scope'
      ? toTrimmedStringValue(config.marketScope)
      : toTrimmedStringValue(config.marketSlug);

  return {
    nodeKey: sourceNode.id,
    bindingMode,
    marketMode,
    marketSource,
    cycleWindowMode: toTrimmedStringValue(config.cycleWindowMode) || 'off',
    cycleWindowSecs: toTrimmedStringValue(config.cycleWindowSecs),
    cycleWindowStartSec: toTrimmedStringValue(config.cycleWindowStartSec),
    cycleWindowEndSec: toTrimmedStringValue(config.cycleWindowEndSec),
  };
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
