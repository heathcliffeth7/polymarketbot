import { useMemo } from 'react';
import type { FlowEdge, FlowNode } from '../flow-canvas-constants';
import {
  hasUpstreamAutoScopeTrigger,
  resolveUpstreamTriggerMaxPrice,
  type UpstreamMaxPriceResolution,
  hasUpstreamTriggerWithConfiguredPrice,
} from '../flow-canvas-utils';

const EMPTY_UPSTREAM_MAX_PRICE_RESOLUTION: UpstreamMaxPriceResolution = {
  kind: 'none',
  maxPriceCent: null,
  distinctMaxPriceCents: [],
};

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

  return { upstreamAutoScope, upstreamTriggerPrice, upstreamMaxPriceResolution };
}
