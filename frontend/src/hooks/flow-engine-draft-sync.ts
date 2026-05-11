'use client';

import { createGraphFingerprint } from '@/components/trade-builder/flow-canvas-utils';
import type { TradeFlowGraph } from '@/lib/types';
import {
  compareFlowDetailSnapshotMeta,
  type FlowDetailSnapshotMeta,
} from './flow-detail-snapshot';

export function isGraphContentEqual(local: TradeFlowGraph, remote: TradeFlowGraph): boolean {
  return (
    createGraphFingerprint(local.nodes, local.edges) ===
    createGraphFingerprint(remote.nodes, remote.edges)
  );
}

export function isStaleSnapshot(
  incoming: FlowDetailSnapshotMeta | null,
  latest: FlowDetailSnapshotMeta | null
): boolean {
  if (!incoming || !latest) return false;
  if (incoming.definitionId !== latest.definitionId) return false;
  return compareFlowDetailSnapshotMeta(incoming, latest) < 0;
}

export function isEditorOwned(
  selectedDefinitionId: number | null,
  ownerDefinitionId: number | null
): boolean {
  return selectedDefinitionId != null && ownerDefinitionId === selectedDefinitionId;
}
