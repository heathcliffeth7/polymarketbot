import { useMemo } from 'react';
import type { FlowEdge, FlowNode } from '../flow-canvas-constants';
import {
  hasUpstreamAutoScopeTrigger,
  resolveDirectUpstreamPairLockTrigger,
  resolveUpstreamFixedTriggerMarket,
  resolveUpstreamTriggerMaxPrice,
  type PairLockUpstreamTriggerSummary,
  type UpstreamFixedMarketResolution,
  type UpstreamMaxPriceResolution,
  hasUpstreamTriggerWithConfiguredPrice,
} from '../flow-canvas-utils';

const EMPTY_UPSTREAM_MAX_PRICE_RESOLUTION: UpstreamMaxPriceResolution = {
  kind: 'none',
  maxPriceCent: null,
  distinctMaxPriceCents: [],
};

const EMPTY_UPSTREAM_FIXED_MARKET_RESOLUTION: UpstreamFixedMarketResolution = {
  kind: 'none',
  marketSlug: null,
  outcomeKind: 'none',
  tokenId: null,
  outcomeLabel: null,
  distinctMarketSlugs: [],
  distinctOutcomeLabels: [],
};

const EMPTY_UPSTREAM_PAIR_LOCK_TRIGGER: PairLockUpstreamTriggerSummary | null = null;

interface UseSelectedNodeUpstreamArgs {
  selectedNodeId: string | null;
  canvasNodes: FlowNode[];
  canvasEdges: FlowEdge[];
}

export function useSelectedNodeUpstream({
  selectedNodeId,
  canvasNodes,
  canvasEdges,
}: UseSelectedNodeUpstreamArgs) {
  const upstreamAutoScope = useMemo(() => {
    if (!selectedNodeId) return false;
    return hasUpstreamAutoScopeTrigger(selectedNodeId, canvasNodes, canvasEdges);
  }, [selectedNodeId, canvasNodes, canvasEdges]);

  const upstreamTriggerPrice = useMemo(() => {
    if (!selectedNodeId) return false;
    return hasUpstreamTriggerWithConfiguredPrice(selectedNodeId, canvasNodes, canvasEdges);
  }, [selectedNodeId, canvasNodes, canvasEdges]);

  const upstreamMaxPriceResolution = useMemo(() => {
    if (!selectedNodeId) return EMPTY_UPSTREAM_MAX_PRICE_RESOLUTION;
    return resolveUpstreamTriggerMaxPrice(selectedNodeId, canvasNodes, canvasEdges);
  }, [selectedNodeId, canvasNodes, canvasEdges]);

  const upstreamFixedMarketResolution = useMemo(() => {
    if (!selectedNodeId) return EMPTY_UPSTREAM_FIXED_MARKET_RESOLUTION;
    return resolveUpstreamFixedTriggerMarket(selectedNodeId, canvasNodes, canvasEdges);
  }, [selectedNodeId, canvasNodes, canvasEdges]);

  const upstreamPairLockTrigger = useMemo(() => {
    if (!selectedNodeId) return EMPTY_UPSTREAM_PAIR_LOCK_TRIGGER;
    return resolveDirectUpstreamPairLockTrigger(selectedNodeId, canvasNodes, canvasEdges);
  }, [selectedNodeId, canvasNodes, canvasEdges]);

  return {
    upstreamAutoScope,
    upstreamTriggerPrice,
    upstreamMaxPriceResolution,
    upstreamFixedMarketResolution,
    upstreamPairLockTrigger,
  };
}
