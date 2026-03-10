import type { TradeFlowDefinitionDetail } from '@/lib/types';

export interface FlowDetailSnapshotMeta {
  definitionId: number;
  draftVersionId: number;
  updatedAtMs: number;
}

function toTimestamp(value: string): number {
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

export function getFlowDetailSnapshotMeta(
  detail: TradeFlowDefinitionDetail | null
): FlowDetailSnapshotMeta | null {
  if (!detail) return null;
  return {
    definitionId: detail.definition.id,
    draftVersionId: detail.draftVersion?.id ?? 0,
    updatedAtMs: toTimestamp(detail.definition.updated_at),
  };
}

export function compareFlowDetailSnapshotMeta(
  left: FlowDetailSnapshotMeta | null,
  right: FlowDetailSnapshotMeta | null
): number {
  if (!left && !right) return 0;
  if (!left) return -1;
  if (!right) return 1;
  if (left.definitionId !== right.definitionId) {
    return left.definitionId - right.definitionId;
  }
  if (left.updatedAtMs !== right.updatedAtMs) {
    return left.updatedAtMs - right.updatedAtMs;
  }
  return left.draftVersionId - right.draftVersionId;
}
