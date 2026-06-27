import type { DefinitionSwitchPhase, DefinitionSwitchState, DraftSaveStatus, DraftSwitchRecovery } from '@/components/trade-builder/flow-engine-types';
import type { TradeFlowDefinition, TradeFlowGraph } from '@/lib/types';
import { isRecord } from '@/components/trade-builder/flow-engine-utils';

export const SWITCH_SAVE_TIMEOUT_MS = 15_000;
export const SWITCH_DETAIL_TIMEOUT_MS = 20_000;

export function buildDefinitionSwitchState(
  targetId: number,
  phase: DefinitionSwitchPhase
): DefinitionSwitchState {
  return { targetId, phase, startedAt: Date.now() };
}

export function isLoadingDetailSwitch(
  state: DefinitionSwitchState | null,
  targetId: number | null
) {
  return state?.targetId === targetId && state.phase === 'loading_detail';
}

export function getDefinitionSwitchReadOnlyReason(state: DefinitionSwitchState | null) {
  if (!state) return null;
  return state.phase === 'saving_current'
    ? 'Mevcut workflow kaydediliyor. Gecis tamamlaninca duzenleyebilirsiniz.'
    : 'Workflow yukleniyor. Canvas hazir olunca duzenleyebilirsiniz.';
}

export function buildDraftSwitchSnapshot({
  currentDefinition,
  draftDescription,
  draftName,
  graphSnapshot,
  isGraphDirty,
  resolvedContext,
}: {
  currentDefinition: TradeFlowDefinition | null;
  draftDescription: string;
  draftName: string;
  graphSnapshot: TradeFlowGraph;
  isGraphDirty: boolean;
  resolvedContext: Record<string, unknown>;
}) {
  const currentContext = isRecord(graphSnapshot.context) ? graphSnapshot.context : {};
  const hasMetadataChanges =
    draftName.trim() !== (currentDefinition?.name.trim() ?? '') ||
    draftDescription.trim() !== (currentDefinition?.description ?? '').trim();
  const hasContextChanges = JSON.stringify(currentContext) !== JSON.stringify(resolvedContext);
  return {
    graphSnapshot,
    resolvedContext,
    shouldSave: isGraphDirty || hasMetadataChanges || hasContextChanges,
  };
}

export async function saveDraftBeforeDefinitionSwitch({
  buildPayload,
  clearScheduledAutosave,
  currentDefinition,
  definitionId,
  draftDescriptionRef,
  draftNameRef,
  graphRef,
  hydratedDraftSignatureRef,
  isGraphDirtyRef,
  patchDraft,
  revalidate,
  resolveContextInput,
  saveStatus,
  setAutoSaveError,
  setGraphDirtyState,
  setSaveStatus,
  waitForQueuedDraftSave,
}: {
  buildPayload: (
    graphJson: TradeFlowGraph,
    draftName: string,
    draftDescription: string
  ) => Record<string, unknown>;
  clearScheduledAutosave: () => void;
  currentDefinition: TradeFlowDefinition | null;
  definitionId: number;
  draftDescriptionRef: { current: string };
  draftNameRef: { current: string };
  graphRef: { current: TradeFlowGraph };
  hydratedDraftSignatureRef: { current: string | null };
  isGraphDirtyRef: { current: boolean };
  patchDraft: (
    definitionId: number,
    payload: Record<string, unknown>,
    options: { timeoutMs: number; retries: number }
  ) => Promise<unknown>;
  revalidate: () => Promise<void>;
  resolveContextInput: () => { context: Record<string, unknown> | null; errorMessage: string | null };
  saveStatus: DraftSaveStatus;
  setAutoSaveError: (value: string | null) => void;
  setGraphDirtyState: (value: boolean) => void;
  setSaveStatus: (value: DraftSaveStatus) => void;
  waitForQueuedDraftSave: () => Promise<void>;
}) {
  clearScheduledAutosave();
  const getDraftSnapshot = () => {
    const { context: resolvedContext, errorMessage } = resolveContextInput();
    if (!resolvedContext) throw new Error(errorMessage ?? 'Context JSON hatali.');
    return buildDraftSwitchSnapshot({
      currentDefinition,
      draftDescription: draftDescriptionRef.current,
      draftName: draftNameRef.current,
      graphSnapshot: graphRef.current,
      isGraphDirty: isGraphDirtyRef.current,
      resolvedContext,
    });
  };

  const initialSnapshot = getDraftSnapshot();
  if (!initialSnapshot.shouldSave && saveStatus !== 'pending') return;
  await withTimeout(
    waitForQueuedDraftSave(),
    SWITCH_SAVE_TIMEOUT_MS,
    'Mevcut workflow draft kaydi tamamlanamadi, gecis durdu.'
  );
  const nextSnapshot = getDraftSnapshot();
  if (!nextSnapshot.shouldSave) return;

  const payload = buildPayload(
    { ...nextSnapshot.graphSnapshot, context: nextSnapshot.resolvedContext },
    draftNameRef.current,
    draftDescriptionRef.current
  );
  const payloadSignature = buildDraftPersistSignature(payload);
  if (shouldSkipUnchangedDraftSwitchSave({
    hydratedSignature: hydratedDraftSignatureRef.current,
    payloadSignature,
    shouldSave: nextSnapshot.shouldSave,
  })) {
    setGraphDirtyState(false);
    setSaveStatus('idle');
    setAutoSaveError(null);
    return;
  }

  setSaveStatus('pending');
  setAutoSaveError(null);
  await patchDraft(definitionId, { ...payload, syncNormalizedTables: false }, {
    timeoutMs: SWITCH_SAVE_TIMEOUT_MS,
    retries: 1,
  });
  hydratedDraftSignatureRef.current = payloadSignature;
  setGraphDirtyState(false);
  setSaveStatus('idle');
  setAutoSaveError(null);
  await revalidate();
}

export function stableStringify(value: unknown): string {
  return JSON.stringify(normalizeStableJson(value));
}

export function buildDraftPersistSignature(payload: Record<string, unknown>): string {
  return stableStringify(payload);
}

export function shouldSkipUnchangedDraftSwitchSave({
  hydratedSignature,
  payloadSignature,
  shouldSave,
}: {
  hydratedSignature: string | null;
  payloadSignature: string;
  shouldSave: boolean;
}) {
  return shouldSave && hydratedSignature != null && payloadSignature === hydratedSignature;
}

export function buildDraftSwitchFailureMessage(reason: string) {
  return reason.startsWith('Mevcut workflow draft')
    ? reason
    : `Draft kaydedilemedi, workflow degisikligi iptal edildi. ${reason}`;
}

export function buildDraftSwitchRecovery(
  currentDefinitionId: number,
  targetDefinitionId: number,
  message: string
): DraftSwitchRecovery {
  return { currentDefinitionId, targetDefinitionId, message };
}

function normalizeStableJson(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map((item) => {
      if (item === undefined || typeof item === 'function' || typeof item === 'symbol') return null;
      return normalizeStableJson(item);
    });
  }
  if (!value || typeof value !== 'object') return value;
  const input = value as Record<string, unknown>;
  const output: Record<string, unknown> = {};
  for (const key of Object.keys(input).sort()) {
    const next = input[key];
    if (next === undefined || typeof next === 'function' || typeof next === 'symbol') continue;
    output[key] = normalizeStableJson(next);
  }
  return output;
}

export async function withTimeout<T>(
  task: Promise<T>,
  timeoutMs: number,
  message: string
): Promise<T> {
  let timeoutId: ReturnType<typeof setTimeout> | null = null;
  const timeout = new Promise<never>((_, reject) => {
    timeoutId = setTimeout(() => reject(new Error(message)), timeoutMs);
  });
  try {
    return await Promise.race([task, timeout]);
  } finally {
    if (timeoutId != null) {
      clearTimeout(timeoutId);
    }
  }
}
