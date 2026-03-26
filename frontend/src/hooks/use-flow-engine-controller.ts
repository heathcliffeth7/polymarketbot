'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { mutate as swrMutate } from 'swr';
import { toast } from 'sonner';
import {
  buildContextFromForm,
  parseContextToForm,
  type ContextFormState,
} from '@/lib/trade-flow-config-mappers';
import type { TradeFlowDefinition, TradeFlowDefinitionDetail, TradeFlowGraph } from '@/lib/types';
import { useBotStatus } from './use-bot-status';
import { useConfig } from './use-config';
import {
  createTradeFlowDefinition,
  deleteTradeFlowDefinition,
  ensureDualDcaSourceTrade,
  patchTradeFlowDefinitionDraft,
  publishTradeFlowDefinition,
  stopTradeFlowDefinition,
  useTradeFlowDefinitionDetail,
  useTradeFlowDefinitions,
  useTradeFlowOpenPositions,
  validateTradeFlowDefinition,
} from './use-trade-flow';
import { useTradeFlowRealtime } from '@/contexts/trade-flow-realtime-context';
import {
  mergeGraphContextPatch,
  prepareDualDcaGraphForPublish,
} from '@/components/trade-builder/flow-engine-helpers';
import {
  buildDetailSnapshotKey,
  deepCloneGraph,
  isRecord,
} from '@/components/trade-builder/flow-engine-utils';
import {
  compareFlowDetailSnapshotMeta,
  getFlowDetailSnapshotMeta,
} from './flow-detail-snapshot';
import { isEditorOwned, isGraphContentEqual, isStaleSnapshot } from './flow-engine-draft-sync';
import {
  buildFlowDraftPersistPayload,
  createTemplateGraph,
  formatFlowOperationError,
  getTemplateCreatedMessage,
  isFlowDefinitionBusyMessage,
  resolveFlowContextInput,
} from './flow-engine-controller-helpers';
import { useDraftSaveQueue } from './flow-engine-draft-save-queue';
import type {
  DraftSaveStatus,
  FlowEngineController,
  FlowEnginePanelProps,
  TemplateKind,
} from '@/components/trade-builder/flow-engine-types';

export function useFlowEngineController({
  defaultMarketSlug,
  defaultOutcome,
}: FlowEnginePanelProps): FlowEngineController {
  const [selectedDefinitionId, setSelectedDefinitionId] = useState<number | null>(() => {
    if (typeof window === 'undefined') return null;
    const stored = localStorage.getItem('flow-engine-selected-definition');
    const num = stored ? Number(stored) : null;
    return num && Number.isFinite(num) && num > 0 ? num : null;
  });
  const [draftName, setDraftName] = useState('');
  const [draftDescription, setDraftDescription] = useState('');
  const [createName, setCreateName] = useState('');
  const [createDescription, setCreateDescription] = useState('');
  const [createTemplateKind, setCreateTemplateKind] = useState<TemplateKind>('starter');
  const [isWorkflowListOpen, setIsWorkflowListOpen] = useState(false);
  const [workflowListQuery, setWorkflowListQuery] = useState('');
  const [deletingDefinitionId, setDeletingDefinitionId] = useState<number | null>(null);
  const [selectedDefinitionIds, setSelectedDefinitionIds] = useState<Set<number>>(new Set());
  const [bulkDeleting, setBulkDeleting] = useState(false);
  const [optimisticDefinitions, setOptimisticDefinitions] = useState<
    Array<TradeFlowDefinition & { _addedAt?: number }>
  >([]);
  const [deletedDefinitionIds, setDeletedDefinitionIds] = useState<Set<number>>(new Set());
  const [graph, setGraph] = useState<TradeFlowGraph>({ context: {}, nodes: [], edges: [] });
  const [contextForm, setContextForm] = useState<ContextFormState>(parseContextToForm({}));
  const [contextTab, setContextTab] = useState<'basic' | 'advanced'>('basic');
  const [validation, setValidation] = useState<FlowEngineController['state']['validation']>(null);
  const [busyAction, setBusyAction] = useState<FlowEngineController['state']['busyAction']>(null);
  const [saveStatus, setSaveStatus] = useState<DraftSaveStatus>('idle');
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [autoSaveError, setAutoSaveError] = useState<string | null>(null);
  const [lastHydratedSnapshotKey, setLastHydratedSnapshotKey] = useState<string | null>(null);
  const [isGraphDirty, setIsGraphDirty] = useState(false);
  const [isSwitchingDefinition, setIsSwitchingDefinition] = useState(false);
  const [hasPendingCanvasNodeDraft, setHasPendingCanvasNodeDraft] = useState(false);
  const [stoppingFlow, setStoppingFlow] = useState(false);

  const { data: botStatus } = useBotStatus();
  const { livePrices: realtimeLivePrices, setSavePaused, closeStream } = useTradeFlowRealtime();
  const { data: telegramConfig } = useConfig('telegram');

  const userTelegramBotTokenMasked = useMemo(() => {
    const value = String(telegramConfig?.data?.bot_token ?? '').trim();
    return value || null;
  }, [telegramConfig?.data?.bot_token]);
  const userTelegramDefaultChatId = useMemo(() => {
    const value = String(telegramConfig?.data?.chat_id ?? '').trim();
    return value || null;
  }, [telegramConfig?.data?.chat_id]);

  useEffect(() => {
    if (selectedDefinitionId != null) {
      localStorage.setItem('flow-engine-selected-definition', String(selectedDefinitionId));
    } else {
      localStorage.removeItem('flow-engine-selected-definition');
    }
  }, [selectedDefinitionId]);

  const selectedDefinitionIdRef = useRef<number | null>(selectedDefinitionId);
  const switchLockRef = useRef(false);
  const graphRef = useRef(graph);
  const draftNameRef = useRef(draftName);
  const draftDescriptionRef = useRef(draftDescription);
  const isGraphDirtyRef = useRef(isGraphDirty);
  const selectionPreferenceRef = useRef<number | 'blank' | null>(null);
  const blankStateAppliedRef = useRef(false);
  const graphOwnerDefinitionIdRef = useRef<number | null>(null);
  const autosaveTimeoutRef = useRef<number | null>(null);
  const canvasAutoSaveRevisionRef = useRef(0);
  const latestDetailSnapshotRef = useRef<ReturnType<typeof getFlowDetailSnapshotMeta>>(null);
  const acknowledgeSuccessRef = useRef<(detail: TradeFlowDefinitionDetail) => void>(
    () => {}
  );
  const resolveContextInputRef = useRef<() => { context: Record<string, unknown> | null; errorMessage: string | null }>(() => ({ context: {}, errorMessage: null }));
  selectedDefinitionIdRef.current = selectedDefinitionId;
  draftNameRef.current = draftName;
  draftDescriptionRef.current = draftDescription;

  const setGraphState = useCallback((nextGraph: TradeFlowGraph) => {
    graphRef.current = nextGraph;
    setGraph(nextGraph);
  }, []);
  const setGraphDirtyState = useCallback((nextDirty: boolean) => {
    isGraphDirtyRef.current = nextDirty;
    setIsGraphDirty(nextDirty);
  }, []);
  const invalidateCanvasAutoSaveRevision = useCallback(() => {
    canvasAutoSaveRevisionRef.current += 1;
    return canvasAutoSaveRevisionRef.current;
  }, []);
  const clearScheduledAutosave = useCallback(() => {
    if (autosaveTimeoutRef.current != null) {
      window.clearTimeout(autosaveTimeoutRef.current);
      autosaveTimeoutRef.current = null;
    }
  }, []);
  const resetEditorToBlankState = useCallback(() => {
    const blankGraph: TradeFlowGraph = { context: {}, nodes: [], edges: [] };
    clearScheduledAutosave();
    graphOwnerDefinitionIdRef.current = null;
    setGraphState(blankGraph);
    setContextForm(parseContextToForm({}));
    setDraftName('');
    setDraftDescription('');
    setValidation(null);
    setGraphDirtyState(false);
    setAutoSaveError(null);
    setSaveStatus('idle');
    setLastHydratedSnapshotKey(null);
    canvasAutoSaveRevisionRef.current = 0;
    latestDetailSnapshotRef.current = null;
  }, [clearScheduledAutosave, setGraphDirtyState, setGraphState]);
  useEffect(() => {
    return () => {
      clearScheduledAutosave();
    };
  }, [clearScheduledAutosave]);
  const stablePatchDraft = useCallback(
    async (definitionId: number, payload: Record<string, unknown>) =>
      (
        await patchTradeFlowDefinitionDraft(definitionId, payload, {
          timeoutMs: 60_000,
        })
      ).data,
    []
  );
  const { queueDraftSave, waitForQueuedDraftSave } = useDraftSaveQueue({
    acknowledgeSuccessRef,
    patchDraft: stablePatchDraft,
    revisionRef: canvasAutoSaveRevisionRef,
    saveStatus,
    selectedDefinitionIdRef,
    setAutoSaveError,
    setError,
    setSaveStatus,
  });

  const {
    data: definitionsData,
    error: rawDefinitionsError,
    mutate: mutateDefinitions,
    isLoading: definitionsLoading,
  } = useTradeFlowDefinitions(
    1,
    50,
    undefined,
    false,
    isGraphDirty || saveStatus === 'pending' || isSwitchingDefinition
  );
  const definitionsError = useMemo(
    () =>
      rawDefinitionsError instanceof Error
        ? rawDefinitionsError
        : rawDefinitionsError
          ? new Error('Flow listesi yuklenemedi.')
          : null,
    [rawDefinitionsError]
  );
  const hasResolvedDefinitions = definitionsData !== undefined && definitionsError == null;
  const definitions = useMemo(() => definitionsData?.data ?? [], [definitionsData?.data]);
  const mergedDefinitions = useMemo(() => {
    const serverVisible = definitions.filter(
      (definition) =>
        definition.status !== 'archived' && !deletedDefinitionIds.has(definition.id)
    );
    const existingIds = new Set(serverVisible.map((definition) => definition.id));
    const optimisticMissing = optimisticDefinitions.filter(
      (definition) =>
        definition.status !== 'archived' &&
        !deletedDefinitionIds.has(definition.id) &&
        !existingIds.has(definition.id)
    );
    return [...optimisticMissing, ...serverVisible];
  }, [definitions, deletedDefinitionIds, optimisticDefinitions]);

  useEffect(() => {
    if (optimisticDefinitions.length === 0) return;
    const serverIds = new Set(definitions.map((definition) => definition.id));
    const now = Date.now();
    setOptimisticDefinitions((previous) => {
      const next = previous.filter((definition) => {
        const age = now - (definition._addedAt ?? 0);
        return !(serverIds.has(definition.id) && age > 30_000);
      });
      return next.length === previous.length ? previous : next;
    });
  }, [definitions, optimisticDefinitions.length]);
  useEffect(() => {
    if (deletedDefinitionIds.size === 0) return;
    const serverIds = new Set(definitions.map((definition) => definition.id));
    setDeletedDefinitionIds((previous) => {
      const next = new Set(Array.from(previous).filter((definitionId) => serverIds.has(definitionId)));
      return next.size === previous.size ? previous : next;
    });
  }, [definitions, deletedDefinitionIds.size]);

  const visibleDefinitions = useMemo(
    () => mergedDefinitions.filter((definition) => definition.status !== 'archived'),
    [mergedDefinitions]
  );
  useEffect(() => {
    const visibleDefinitionIds = new Set(visibleDefinitions.map((definition) => definition.id));
    setSelectedDefinitionIds((previous) => {
      const next = new Set(
        Array.from(previous).filter((definitionId) => visibleDefinitionIds.has(definitionId))
      );
      return next.size === previous.size ? previous : next;
    });
  }, [visibleDefinitions]);
  const filteredDefinitions = useMemo(() => {
    const query = workflowListQuery.trim().toLowerCase();
    if (!query) return visibleDefinitions;
    return visibleDefinitions.filter((definition) =>
      `${definition.id} ${definition.name} ${definition.status}`.toLowerCase().includes(query)
    );
  }, [visibleDefinitions, workflowListQuery]);

  const toggleDefinitionSelection = useCallback((id: number) => {
    setSelectedDefinitionIds((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const selectAllDefinitions = useCallback(() => {
    setSelectedDefinitionIds(new Set(filteredDefinitions.map((definition) => definition.id)));
  }, [filteredDefinitions]);

  const deselectAllDefinitions = useCallback(() => {
    setSelectedDefinitionIds(new Set());
  }, []);

  const markDefinitionDeleted = useCallback(
    (definitionId: number) => {
      setDeletedDefinitionIds((previous) => {
        const next = new Set(previous);
        next.add(definitionId);
        return next;
      });
      setOptimisticDefinitions((previous) =>
        previous.filter((definition) => definition.id !== definitionId)
      );
      setSelectedDefinitionIds((previous) => {
        if (!previous.has(definitionId)) return previous;
        const next = new Set(previous);
        next.delete(definitionId);
        return next;
      });
      if (selectedDefinitionIdRef.current === definitionId) {
        selectionPreferenceRef.current = 'blank';
        setSelectedDefinitionId(null);
        resetEditorToBlankState();
      }
      setValidation(null);
    },
    [resetEditorToBlankState]
  );

  const revalidateDeletedFlowCaches = useCallback(async () => {
    await Promise.all([
      mutateDefinitions(),
      swrMutate(
        (key: string) =>
          typeof key === 'string' &&
          (key.includes('/api/trade-flow/definitions') ||
            key.includes('/api/trade-flow/runs') ||
            key.includes('/api/trade-flow/events/recent'))
      ),
    ]);
  }, [mutateDefinitions]);

  const resolveContextInput = useCallback(
    () => resolveFlowContextInput(contextTab, contextForm),
    [contextForm, contextTab]
  );
  resolveContextInputRef.current = resolveContextInput;

  const bulkDeleteDefinitions = useCallback(async () => {
    if (selectedDefinitionIds.size === 0) return;
    if (
      !window.confirm(
        `${selectedDefinitionIds.size} workflow'u kalici olarak silmek istediginize emin misiniz?\n\nBu islem arsivleme yapmaz ve geri alinamaz.`
      )
    ) {
      return;
    }
    setBusyAction('delete');
    setBulkDeleting(true);
    setError(null);
    setMessage(null);
    let deletedCount = 0;
    const failedIds: number[] = [];
    try {
      for (const id of Array.from(selectedDefinitionIds)) {
        try {
          await deleteTradeFlowDefinition(id);
          markDefinitionDeleted(id);
          deletedCount += 1;
        } catch (err) {
          failedIds.push(id);
          console.error(`Failed to delete definition ${id}:`, err);
        }
      }
      await revalidateDeletedFlowCaches();
      if (deletedCount > 0) {
        setMessage(`${deletedCount} workflow kalici olarak silindi.`);
      }
      if (failedIds.length > 0) {
        setError(
          `Su workflow'ler silinemedi: ${failedIds.map((id) => `#${id}`).join(', ')}.`
        );
      }
    } finally {
      setBulkDeleting(false);
      setBusyAction(null);
    }
  }, [markDefinitionDeleted, revalidateDeletedFlowCaches, selectedDefinitionIds]);

  const saveCurrentDraftBeforeSwitch = useCallback(
    async (definitionId: number) => {
      clearScheduledAutosave();
      await waitForQueuedDraftSave();

      const currentDefinition =
        visibleDefinitions.find((definition) => definition.id === definitionId) ?? null;
      const graphSnapshot = graphRef.current;
      const currentContext = isRecord(graphSnapshot.context) ? graphSnapshot.context : {};
      const { context: resolvedContext, errorMessage } = resolveContextInput();
      if (!resolvedContext) {
        throw new Error(errorMessage ?? 'Context JSON hatali.');
      }
      const nextName = draftNameRef.current.trim();
      const nextDescription = draftDescriptionRef.current.trim();
      const currentName = currentDefinition?.name.trim() ?? '';
      const currentDescription = (currentDefinition?.description ?? '').trim();
      const hasMetadataChanges =
        nextName !== currentName || nextDescription !== currentDescription;
      const hasContextChanges =
        JSON.stringify(currentContext) !== JSON.stringify(resolvedContext);
      if (!isGraphDirtyRef.current && !hasMetadataChanges && !hasContextChanges) return;

      const payload = buildFlowDraftPersistPayload(
        { ...graphSnapshot, context: resolvedContext },
        draftNameRef.current,
        draftDescriptionRef.current
      );
      setSaveStatus('pending');
      setAutoSaveError(null);
      try {
        await patchTradeFlowDefinitionDraft(
          definitionId,
          {
            ...payload,
            syncNormalizedTables: false,
          },
          { timeoutMs: 60_000, retries: 0 }
        );
        if (selectedDefinitionIdRef.current === definitionId) {
          setSaveStatus('idle');
          setAutoSaveError(null);
        }
      } catch (err) {
        const reason = formatFlowOperationError(err, 'Draft kaydedilemedi.');
        if (selectedDefinitionIdRef.current === definitionId) {
          setSaveStatus('error');
          setAutoSaveError(reason);
        }
        throw new Error(reason);
      }
      await Promise.all([
        mutateDefinitions(),
        swrMutate(`/api/trade-flow/definitions/${definitionId}`),
      ]);
    },
    [
      mutateDefinitions,
      setAutoSaveError,
      setSaveStatus,
      clearScheduledAutosave,
      resolveContextInput,
      visibleDefinitions,
      waitForQueuedDraftSave,
    ]
  );

  const requestDefinitionSwitch = useCallback(
    async (nextDefinitionId: number) => {
      if (!Number.isFinite(nextDefinitionId) || nextDefinitionId <= 0) return false;
      if (switchLockRef.current) return false;
      if (hasPendingCanvasNodeDraft) {
        const reason = "Node formunda uygulanmamis degisiklik var. Once 'Node Guncelle' kullanin.";
        setError(reason);
        toast.error(reason);
        return false;
      }
      const currentDefinitionId = selectedDefinitionIdRef.current;
      if (currentDefinitionId === nextDefinitionId) return true;

      switchLockRef.current = true;
      selectionPreferenceRef.current = nextDefinitionId;
      setIsSwitchingDefinition(true);
      invalidateCanvasAutoSaveRevision();
      setError(null);
      setMessage(null);
      try {
        if (currentDefinitionId) {
          await saveCurrentDraftBeforeSwitch(currentDefinitionId);
          setAutoSaveError(null);
          setSaveStatus('idle');
          setMessage(`Workflow #${currentDefinitionId} icin draft otomatik kaydedildi.`);
        }
        resetEditorToBlankState();
        setSelectedDefinitionIds(new Set());
        setSelectedDefinitionId(nextDefinitionId);
        setAutoSaveError(null);
        setLastHydratedSnapshotKey(null);
        return true;
      } catch (err) {
        const reason = err instanceof Error ? err.message : 'Bilinmeyen hata.';
        setError(`Draft kaydedilemedi, workflow degisikligi iptal edildi. ${reason}`);
        return false;
      } finally {
        switchLockRef.current = false;
        setIsSwitchingDefinition(false);
      }
    },
    [
      hasPendingCanvasNodeDraft,
      invalidateCanvasAutoSaveRevision,
      resetEditorToBlankState,
      saveCurrentDraftBeforeSwitch,
    ]
  );

  const { data: detailData, error: detailError, mutate: mutateDetail } = useTradeFlowDefinitionDetail(
    selectedDefinitionId,
    isGraphDirty || saveStatus === 'pending'
  );
  const detail = useMemo(() => detailData?.data ?? null, [detailData?.data]);
  const detailFetchSettled = detailData !== undefined || detailError != null;

  useEffect(() => {
    if (visibleDefinitions.length === 0) {
      if (
        definitionsLoading ||
        busyAction ||
        definitionsError ||
        !hasResolvedDefinitions ||
        (detail?.definition?.id != null && detail.definition.id === selectedDefinitionId) ||
        (selectedDefinitionId != null && !detailFetchSettled)
      ) {
        return;
      }
      if (!blankStateAppliedRef.current) {
        blankStateAppliedRef.current = true;
        selectionPreferenceRef.current = 'blank';
        if (selectedDefinitionId !== null) {
          setSelectedDefinitionId(null);
        }
        setSelectedDefinitionIds((previous) => (previous.size === 0 ? previous : new Set()));
        resetEditorToBlankState();
      }
      return;
    }
    blankStateAppliedRef.current = false;

    const selectionPreference = selectionPreferenceRef.current;
    if (selectionPreference === 'blank' && !selectedDefinitionId) {
      return;
    }
    if (typeof selectionPreference === 'number') {
      const preferredVisible = visibleDefinitions.some(
        (definition) => definition.id === selectionPreference
      );
      if (selectedDefinitionId === selectionPreference && preferredVisible) {
        selectionPreferenceRef.current = null;
      } else if (preferredVisible) {
        void requestDefinitionSwitch(selectionPreference);
        return;
      } else if (busyAction === 'create') {
        return;
      } else {
        selectionPreferenceRef.current = null;
      }
    }

    const stillExists = visibleDefinitions.some(
      (definition) => definition.id === selectedDefinitionId
    );
    if ((!selectedDefinitionId || !stillExists) && !definitionsError && hasResolvedDefinitions) {
      void requestDefinitionSwitch(visibleDefinitions[0].id);
    }
  }, [
    busyAction,
    definitionsError,
    detail?.definition?.id,
    detailFetchSettled,
    hasResolvedDefinitions,
    definitionsLoading,
    requestDefinitionSwitch,
    resetEditorToBlankState,
    selectedDefinitionId,
    visibleDefinitions,
  ]);

  const selectedDefinition = useMemo(() => {
    if (detail?.definition && detail.definition.id === selectedDefinitionId) {
      return detail.definition;
    }
    return visibleDefinitions.find((definition) => definition.id === selectedDefinitionId) ?? null;
  }, [detail?.definition, selectedDefinitionId, visibleDefinitions]);
  const canStopSelectedFlow = selectedDefinition?.status === 'published';
  const incomingSnapshotKey = useMemo(() => buildDetailSnapshotKey(detail), [detail]);

  useEffect(() => {
    setGraphDirtyState(false);
    setAutoSaveError(null);
    setSaveStatus('idle');
    setLastHydratedSnapshotKey(null);
    canvasAutoSaveRevisionRef.current = 0;
    latestDetailSnapshotRef.current = null;
  }, [selectedDefinitionId, setGraphDirtyState]);

  useEffect(() => {
    setSavePaused(isGraphDirty || saveStatus === 'pending');
  }, [isGraphDirty, saveStatus, setSavePaused]);

  const { data: openPositionsData, isLoading: openPositionsLoading } =
    useTradeFlowOpenPositions(
      isGraphDirty || saveStatus === 'pending' || isSwitchingDefinition
    );
  const openPositions = useMemo(() => openPositionsData?.data ?? [], [openPositionsData?.data]);
  const openPositionsMeta = useMemo(
    () => openPositionsData?.meta ?? null,
    [openPositionsData?.meta]
  );

  const primeDetailCache = useCallback(
    (nextDetail: TradeFlowDefinitionDetail | null) => {
      if (!nextDetail?.definition) return;
      void mutateDetail({ data: nextDetail }, { revalidate: false });
    },
    [mutateDetail]
  );

  const hydrateEditorFromDetail = useCallback((nextDetail: TradeFlowDefinitionDetail | null) => {
    if (!nextDetail?.draftVersion) return false;
    const nextSnapshot = getFlowDetailSnapshotMeta(nextDetail);
    if (
      compareFlowDetailSnapshotMeta(nextSnapshot, latestDetailSnapshotRef.current) < 0 &&
      nextSnapshot?.definitionId === latestDetailSnapshotRef.current?.definitionId
    ) {
      return false;
    }
    latestDetailSnapshotRef.current = nextSnapshot;
    const normalized = deepCloneGraph(nextDetail.draftVersion.graph_json);
    graphOwnerDefinitionIdRef.current = nextDetail.definition.id;
    setGraphState(normalized);
    setContextForm(parseContextToForm(normalized.context || {}));
    setDraftName(nextDetail.definition.name);
    setDraftDescription(nextDetail.definition.description || '');
    setValidation(null);
    setGraphDirtyState(false);
    setAutoSaveError(null);
    setLastHydratedSnapshotKey(buildDetailSnapshotKey(nextDetail));
    primeDetailCache(nextDetail);
    return true;
  }, [primeDetailCache, setGraphDirtyState, setGraphState]);

  const acknowledgeSuccess = useCallback((updatedDetail: TradeFlowDefinitionDetail) => {
    primeDetailCache(updatedDetail);
    latestDetailSnapshotRef.current = getFlowDetailSnapshotMeta(updatedDetail);
    setLastHydratedSnapshotKey(buildDetailSnapshotKey(updatedDetail));

    if (!updatedDetail.draftVersion) {
      setGraphDirtyState(false);
      setAutoSaveError(null);
      return;
    }

    const serverGraph = deepCloneGraph(updatedDetail.draftVersion.graph_json);
    if (!isGraphContentEqual(graphRef.current, serverGraph)) {
      hydrateEditorFromDetail(updatedDetail);
      return;
    }

    setGraphDirtyState(false);
    setAutoSaveError(null);
  }, [hydrateEditorFromDetail, primeDetailCache, setGraphDirtyState]);
  acknowledgeSuccessRef.current = acknowledgeSuccess;

  useEffect(() => {
    if (!detail?.draftVersion || !incomingSnapshotKey) return;
    if (detail.definition.id !== selectedDefinitionId) return;
    if (isGraphDirtyRef.current) return;
    if (saveStatus === 'pending') return;
    if (incomingSnapshotKey === lastHydratedSnapshotKey) return;
    const incoming = getFlowDetailSnapshotMeta(detail);
    if (isStaleSnapshot(incoming, latestDetailSnapshotRef.current)) return;
    hydrateEditorFromDetail(detail);
  }, [detail, hydrateEditorFromDetail, incomingSnapshotKey, lastHydratedSnapshotKey, saveStatus, selectedDefinitionId]);

  const applyResolvedContext = useCallback(
    (parsed: Record<string, unknown>) => {
      invalidateCanvasAutoSaveRevision();
      const nextGraph = { ...graphRef.current, context: parsed };
      setGraphState(nextGraph);
      setContextForm(parseContextToForm(parsed));
      setGraphDirtyState(true);
      setError(null);
      return parsed;
    },
    [invalidateCanvasAutoSaveRevision, setGraphDirtyState, setGraphState]
  );

  const applyContextFromForm = () => applyResolvedContext(buildContextFromForm(contextForm));

  const applyContextFromAdvanced = () => {
    const { context, errorMessage } = resolveContextInput();
    if (!context) {
      setError(errorMessage ?? 'Context JSON hatali.');
      return null;
    }
    return applyResolvedContext(context);
  };

  const createFromTemplate = async (kind: TemplateKind) => {
    const name = createName.trim();
    if (!name) {
      setError('Yeni flow icin ad zorunlu.');
      return;
    }
    const currentDefinitionId = selectedDefinitionIdRef.current;
    setBusyAction('create');
    setError(null);
    setMessage(null);
    try {
      if (currentDefinitionId) {
        await saveCurrentDraftBeforeSwitch(currentDefinitionId);
        setAutoSaveError(null);
        setSaveStatus('idle');
      }
      const template = createTemplateGraph(kind, defaultMarketSlug, defaultOutcome);
      const created = await createTradeFlowDefinition({
        name,
        description: createDescription.trim() || null,
        graphJson: template,
      });
      const createdDetail = created.data;
      selectionPreferenceRef.current = createdDetail.definition.id;
      setOptimisticDefinitions((previous) => {
        const next = [
          { ...createdDetail.definition, _addedAt: Date.now() },
          ...previous.filter((definition) => definition.id !== createdDetail.definition.id),
        ];
        return next.slice(0, 20);
      });
      setIsWorkflowListOpen(true);
      setWorkflowListQuery('');
      setCreateName('');
      setCreateDescription('');
      const switched = await requestDefinitionSwitch(createdDetail.definition.id);
      if (!switched) return;
      hydrateEditorFromDetail(createdDetail);
      setValidation(null);
      setMessage(getTemplateCreatedMessage(kind));
      await mutateDefinitions();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : 'Flow olusturulamadi.'
      );
    } finally {
      setBusyAction(null);
    }
  };

  const validateGraph = async () => {
    if (!selectedDefinitionId) {
      setError('Once bir flow secin.');
      return;
    }
    if (!isEditorOwned(selectedDefinitionIdRef.current, graphOwnerDefinitionIdRef.current) || isSwitchingDefinition) {
      setError('Flow yuklenmeden duzenleme yapilamaz.');
      return;
    }
    const { context: ctx, errorMessage } = resolveContextInput();
    if (!ctx) {
      setError(errorMessage ?? 'Context JSON hatali.');
      return;
    }

    setBusyAction('validate');
    setError(null);
    setMessage(null);
    try {
      const result = await validateTradeFlowDefinition(selectedDefinitionId, {
        graphJson: { ...graphRef.current, context: ctx },
      });
      setValidation(result.data);
      setMessage(
        result.data.valid
          ? 'Flow dogrulama basarili.'
          : 'Flow dogrulamada sorunlar bulundu.'
      );
    } catch (err) {
      setError(formatFlowOperationError(err, 'Dogrulama yapilamadi.'));
    } finally {
      setBusyAction(null);
    }
  };

  const reloadDraftFromServer = useCallback(async () => {
    if (!selectedDefinitionId) {
      setError('Once bir flow secin.');
      return;
    }
    const shouldReload =
      (!isGraphDirty && !autoSaveError) ||
      window.confirm(
        'Kaydedilmemis local degisiklikler atilacak. Sunucudaki draft tekrar yuklensin mi?'
      );
    if (!shouldReload) return;

    setBusyAction('save');
    setError(null);
    setMessage(null);
    try {
      invalidateCanvasAutoSaveRevision();
      const refreshed = await mutateDetail();
      const nextDetail = refreshed?.data ?? detail;
      if (!nextDetail?.draftVersion) {
        throw new Error('Sunucuda draft bulunamadi.');
      }
      hydrateEditorFromDetail(nextDetail);
      setAutoSaveError(null);
      setMessage('Draft sunucudan yeniden yuklendi.');
    } catch (err) {
      setError(formatFlowOperationError(err, 'Draft sunucudan yuklenemedi.'));
    } finally {
      setBusyAction(null);
    }
  }, [
    autoSaveError,
    detail,
    hydrateEditorFromDetail,
    invalidateCanvasAutoSaveRevision,
    isGraphDirty,
    mutateDetail,
    selectedDefinitionId,
  ]);

  const saveDraft = async () => {
    if (!selectedDefinitionId) {
      setError('Once bir flow secin.');
      return;
    }
    if (!isEditorOwned(selectedDefinitionIdRef.current, graphOwnerDefinitionIdRef.current) || isSwitchingDefinition) {
      setError('Flow yuklenmeden duzenleme yapilamaz.');
      return;
    }
    const { context: ctx, errorMessage } = resolveContextInput();
    if (!ctx) {
      setError(errorMessage ?? 'Context JSON hatali.');
      return;
    }

    setBusyAction('save');
    setError(null);
    setMessage(null);
    try {
      clearScheduledAutosave();
      const revision = invalidateCanvasAutoSaveRevision();
      const payload = buildFlowDraftPersistPayload(
        { ...graphRef.current, context: ctx },
        draftName,
        draftDescription
      );
      setGraphState(payload.graphJson);
      await queueDraftSave(selectedDefinitionId, {
        ...payload,
        syncNormalizedTables: false,
      }, {
        errorMessage: 'Draft kaydedilemedi.',
        revision,
      });
      setMessage('Draft flow kaydedildi.');
      await mutateDefinitions();
      await mutateDetail();
    } catch (err) {
      const reason = formatFlowOperationError(err, 'Draft kaydedilemedi.');
      setAutoSaveError(reason);
      setError(reason);
    } finally {
      setBusyAction(null);
    }
  };

  const publishFlow = async () => {
    if (!selectedDefinitionId) {
      setError('Once bir flow secin.');
      return;
    }
    if (!isEditorOwned(selectedDefinitionIdRef.current, graphOwnerDefinitionIdRef.current) || isSwitchingDefinition) {
      setError('Flow yuklenmeden duzenleme yapilamaz.');
      return;
    }
    if (saveStatus === 'pending') {
      try {
        await waitForQueuedDraftSave();
      } catch {
        // Queue failure is handled by autoSaveError below.
      }
    }
    if (saveStatus === 'error' || autoSaveError) {
      setError(
        isFlowDefinitionBusyMessage(autoSaveError)
          ? 'Ayni flow uzerinde baska bir islem calisiyor. Birkac saniye bekleyip tekrar dene. Sorun surerse Draft Kaydet veya Taslagi Sunucudan Yukle kullan.'
          : 'Autosave/PATCH hatasi duzelmeden publish edilemez. Draft Kaydet veya Taslagi Sunucudan Yukle kullan.'
      );
      return;
    }

    const publishDefinitionId = selectedDefinitionId;
    const currentDefinition =
      visibleDefinitions.find((definition) => definition.id === publishDefinitionId) ?? null;
    const publishName =
      draftName.trim() || currentDefinition?.name || `Workflow ${publishDefinitionId}`;
    const publishLabel = `#${publishDefinitionId} - ${publishName}`;
    if (hasPendingCanvasNodeDraft) {
      toast.error("Node formunda uygulanmamis degisiklik var. Once 'Node Guncelle' kullanin.");
      return;
    }
    const publishConfirmed = window.confirm(
      `${publishLabel} publish edilsin mi?\n\nNot: Sadece DCA akisi istiyorsan canvas'ta trigger node olmamali.`
    );
    if (!publishConfirmed) return;

    const { context: ctx, errorMessage } = resolveContextInput();
    if (!ctx) {
      setError(errorMessage ?? 'Context JSON hatali.');
      return;
    }

    setBusyAction('publish');
    setError(null);
    setMessage(null);
    let draftSaved = false;
    let ensuredSourceTradeId: number | null = null;
    let ensuredSourceTradeCreated = false;
    try {
      clearScheduledAutosave();
      const revision = invalidateCanvasAutoSaveRevision();
      const baseDraftGraph: TradeFlowGraph = { ...graphRef.current, context: ctx };
      const prepared = await prepareDualDcaGraphForPublish(
        baseDraftGraph,
        publishDefinitionId,
        ensureDualDcaSourceTrade
      );
      const payload = buildFlowDraftPersistPayload(
        prepared.graphJson,
        draftName,
        draftDescription
      );
      ensuredSourceTradeId = prepared.sourceTradeId;
      ensuredSourceTradeCreated = prepared.created;
      setGraphState(payload.graphJson);
      await queueDraftSave(publishDefinitionId, {
        ...payload,
        syncNormalizedTables: false,
      }, {
        errorMessage: 'Draft kaydedilemedi.',
        revision,
      });
      draftSaved = true;
      const published = await publishTradeFlowDefinition(publishDefinitionId);
      hydrateEditorFromDetail(published.data);
      setValidation(null);
      const runnerSuffix = botStatus?.serviceActive
        ? ' Aktif singleton runner bu flowu mevcut proseste otomatik alacak.'
        : ' Runner aktif degilse tek bir runner prosesi baslat.';
      if (ensuredSourceTradeId != null && ensuredSourceTradeCreated) {
        setMessage(
          `${publishLabel} publish edildi. Source Trade otomatik olusturuldu: #${ensuredSourceTradeId}.${runnerSuffix}`
        );
      } else if (ensuredSourceTradeId != null) {
        setMessage(
          `${publishLabel} publish edildi. Source Trade atandi: #${ensuredSourceTradeId}.${runnerSuffix}`
        );
      } else {
        setMessage(`${publishLabel} publish edildi.${runnerSuffix}`);
      }
      toast.success(`${publishLabel} publish edildi.`);
      await mutateDefinitions();
      await mutateDetail();
    } catch (err) {
      const reason = formatFlowOperationError(err, 'Flow publish edilemedi.');
      const errMsg = draftSaved
        ? `Draft kaydedildi ama publish basarisiz (${publishLabel}). Neden: ${reason}`
        : `Publish basarisiz (${publishLabel}). Neden: ${reason}`;
      if (!draftSaved) {
        setAutoSaveError(reason);
      }
      setError(errMsg);
      toast.error(errMsg);
    } finally {
      setBusyAction(null);
    }
  };

  const deleteFlow = async () => {
    if (!selectedDefinitionId) {
      setError('Once bir flow secin.');
      return;
    }
    const targetId = selectedDefinitionId;
    const target =
      selectedDefinition ??
      visibleDefinitions.find((definition) => definition.id === targetId) ??
      null;
    const label = target ? `#${target.id} - ${target.name}` : `#${targetId}`;
    setBusyAction('delete');
    setError(null);
    setMessage(null);
    try {
      await deleteTradeFlowDefinition(targetId);
      markDefinitionDeleted(targetId);
      await revalidateDeletedFlowCaches();
      setMessage(`${label} kalici olarak silindi. Aktif run/order/job akisi varsa durduruldu.`);
      toast.success(`${label} kalici olarak silindi.`);
    } catch (err) {
      const reason = formatFlowOperationError(err, 'Flow kalici olarak silinemedi.');
      setError(reason);
      toast.error(reason);
    } finally {
      setBusyAction(null);
    }
  };

  const deleteFlowFromList = async (definitionId: number) => {
    const target = visibleDefinitions.find((definition) => definition.id === definitionId);
    const label = target ? `#${target.id} - ${target.name}` : `#${definitionId}`;
    if (
      !window.confirm(
        `${label} workflow kalici olarak silinsin mi?\n\nBu islem arsivleme yapmaz ve geri alinamaz.`
      )
    ) {
      return;
    }

    setBusyAction('delete');
    setDeletingDefinitionId(definitionId);
    setError(null);
    setMessage(null);
    try {
      await deleteTradeFlowDefinition(definitionId);
      markDefinitionDeleted(definitionId);
      await revalidateDeletedFlowCaches();
      setMessage(`Workflow ${label} kalici olarak silindi.`);
      toast.success(`Workflow ${label} kalici olarak silindi.`);
    } catch (err) {
      const reason = formatFlowOperationError(err, 'Workflow kalici olarak silinemedi.');
      setError(reason);
      toast.error(reason);
    } finally {
      setDeletingDefinitionId(null);
      setBusyAction(null);
    }
  };

  const confirmAndDeleteCurrentFlow = async () => {
    if (!selectedDefinitionId) {
      setError('Once bir flow secin.');
      return;
    }
    const target = visibleDefinitions.find((definition) => definition.id === selectedDefinitionId);
    const label = target ? `#${target.id} - ${target.name}` : `#${selectedDefinitionId}`;
    if (
      !window.confirm(
        `${label} workflow kalici olarak silinsin mi?\n\nBu islem arsivleme yapmaz, aktif run/order/job akisini durdurur ve geri alinamaz.`
      )
    ) {
      return;
    }
    await deleteFlow();
  };

  const handleStopFlow = async () => {
    if (!selectedDefinitionId) {
      setError('Once bir flow secin.');
      return;
    }
    const target =
      selectedDefinition ??
      visibleDefinitions.find((definition) => definition.id === selectedDefinitionId) ??
      null;
    if (!target || target.status !== 'published') {
      setError('Yalnizca publish edilmis workflow durdurulabilir.');
      return;
    }
    const label = `#${target.id} - ${target.name}`;
    if (
      !window.confirm(
        `${label} durdurulsun mu?\n\nBu islem sadece secili workflow'u ve ona bagli run/order/job akisini durdurur. Tekrar baslatmak icin yeniden Publish etmelisin.`
      )
    ) {
      return;
    }
    setStoppingFlow(true);
    try {
      const stopped = await stopTradeFlowDefinition(selectedDefinitionId);
      hydrateEditorFromDetail(stopped.data);
      setValidation(null);
      const runnerNote = botStatus?.serviceActive
        ? ' Aktif runner kalan cancel-request orderlarini mevcut proseste kapatacak.'
        : '';
      setMessage(
        `${label} durduruldu. Bu workflow'un run ve bagli child akislari kapatildi. Tekrar baslatmak icin yeniden Publish et.${runnerNote}`
      );
      toast.success(`${label} durduruldu.`);
      await Promise.all([
        mutateDefinitions(),
        mutateDetail(),
        swrMutate((key: string) => typeof key === 'string' && key.includes('/api/trade-flow/runs')),
        swrMutate(
          (key: string) =>
            typeof key === 'string' &&
            (key.includes('/api/trade-flow/events/recent') ||
              key.includes(`/api/trade-flow/definitions/${selectedDefinitionId}`))
        ),
      ]);
    } catch (err) {
      const reason = formatFlowOperationError(err, 'Workflow durdurulamadi.');
      setError(reason);
      toast.error(reason);
    } finally {
      setStoppingFlow(false);
    }
  };

  const updateGraphFromCanvas = useCallback(
    (
      nextGraph: TradeFlowGraph,
      options?: { allowGraphShrink?: boolean; persistImmediately?: boolean }
    ) => {
      const definitionId = selectedDefinitionIdRef.current;
      const graphOwnerDefinitionId = graphOwnerDefinitionIdRef.current;
      if (!definitionId || graphOwnerDefinitionId !== definitionId || isSwitchingDefinition) {
        setError('Flow yuklenmeden duzenleme yapilamaz.');
        return;
      }
      const currentGraph = graphRef.current;
      const nextNodeKeys = new Set(nextGraph.nodes.map((node) => node.key));
      const nextEdgeKeys = new Set(nextGraph.edges.map((edge) => edge.key));
      const droppedExistingNodeWithoutPermission =
        !options?.allowGraphShrink &&
        currentGraph.nodes.some((node) => !nextNodeKeys.has(node.key));
      const droppedExistingEdgeWithoutPermission =
        !options?.allowGraphShrink &&
        currentGraph.edges.some((edge) => !nextEdgeKeys.has(edge.key));
      if (droppedExistingNodeWithoutPermission || droppedExistingEdgeWithoutPermission) {
        return;
      }

      const { context: ctx, errorMessage } = resolveContextInputRef.current();
      if (!ctx) {
        setError(errorMessage ?? 'Context JSON hatali.');
        return;
      }

      const optimisticGraph = { ...nextGraph, context: ctx };
      const revision = invalidateCanvasAutoSaveRevision();
      setGraphState(optimisticGraph);
      setGraphDirtyState(true);
      setValidation(null);
      setError(null);
      if (!definitionId) return;

      clearScheduledAutosave();
      const persistDraft = (delayMs: number, errorMessage: string) => {
        const runPersist = async () => {
          if (delayMs > 0) {
            await new Promise((r) => setTimeout(r, delayMs));
          }
          if (
            selectedDefinitionIdRef.current !== definitionId ||
            graphOwnerDefinitionIdRef.current !== definitionId ||
            isSwitchingDefinition
          ) {
            return;
          }
          closeStream();
          await queueDraftSave(
            definitionId,
            {
              graphJson: optimisticGraph,
              syncNormalizedTables: false,
            },
            {
              errorMessage,
              revision,
              surfaceError: true,
            }
          );
        };

        void runPersist().catch((err) => {
          if (
            selectedDefinitionIdRef.current !== definitionId ||
            canvasAutoSaveRevisionRef.current !== revision
          ) {
            return;
          }
          console.warn('[auto-save] PATCH failed:', err);
        });
      };

      if (options?.persistImmediately) {
        persistDraft(0, 'Node degisikligi kaydedilemedi.');
        return;
      }

      autosaveTimeoutRef.current = window.setTimeout(() => {
        autosaveTimeoutRef.current = null;
        persistDraft(50, 'Autosave basarisiz.');
      }, 200);
    },
    [
      clearScheduledAutosave,
      closeStream,
      invalidateCanvasAutoSaveRevision,
      isSwitchingDefinition,
      queueDraftSave,
      setGraphDirtyState,
      setGraphState,
    ]
  );

  const isActionBusy = busyAction !== null || isSwitchingDefinition;
  const publishDisabled =
    isActionBusy || saveStatus === 'pending' || Boolean(autoSaveError);
  const editorOwned = isEditorOwned(
    selectedDefinitionId,
    graphOwnerDefinitionIdRef.current
  );
  const isEditorReadOnly = isSwitchingDefinition || !editorOwned;
  const readOnlyReason = !selectedDefinitionId
    ? 'Once bir flow secin.'
    : 'Flow yukleniyor. Yukleme bitmeden duzenleme yapilamaz.';

  const applyCanvasContextPatch = useCallback(
    async (patch: Record<string, unknown>, successMessage?: string) => {
      const definitionId = selectedDefinitionIdRef.current;
      const graphOwnerDefinitionId = graphOwnerDefinitionIdRef.current;
      if (!definitionId || graphOwnerDefinitionId !== definitionId || isSwitchingDefinition) {
        setError('Once bir flow secin.');
        return;
      }

      const previousGraph = graphRef.current;
      const previousContext = isRecord(previousGraph.context) ? previousGraph.context : {};
      const previousValidation = validation;
      const previousIsGraphDirty = isGraphDirtyRef.current;
      const mergedContext = mergeGraphContextPatch(previousGraph.context, patch);
      const nextGraph: TradeFlowGraph = { ...previousGraph, context: mergedContext };
      const revision = invalidateCanvasAutoSaveRevision();
      setGraphState(nextGraph);
      setGraphDirtyState(true);
      setContextForm(parseContextToForm(mergedContext));
      setValidation(null);
      setError(null);
      setMessage(null);

      try {
        await queueDraftSave(
          definitionId,
          { graphJson: nextGraph },
          {
            errorMessage: 'Autoclaim degisikligi kaydedilemedi.',
            revision,
          }
        );
        if (successMessage) {
          setMessage(successMessage);
        }
        await Promise.all([mutateDefinitions(), mutateDetail()]);
      } catch (err) {
        setGraphState(previousGraph);
        setContextForm(parseContextToForm(previousContext));
        setValidation(previousValidation);
        setGraphDirtyState(previousIsGraphDirty);
        setError(formatFlowOperationError(err, 'Autoclaim degisikligi kaydedilemedi.'));
      }
    },
    [
      invalidateCanvasAutoSaveRevision,
      mutateDefinitions,
      mutateDetail,
      isSwitchingDefinition,
      queueDraftSave,
      setGraphDirtyState,
      setGraphState,
      validation,
    ]
  );

  const livePrices = useMemo(() => {
    return Object.keys(realtimeLivePrices).length > 0 ? realtimeLivePrices : undefined;
  }, [realtimeLivePrices]);

  return {
    state: {
      selectedDefinitionId,
      draftName,
      draftDescription,
      createName,
      createDescription,
      createTemplateKind,
      isWorkflowListOpen,
      workflowListQuery,
      deletingDefinitionId,
      selectedDefinitionIds,
      bulkDeleting,
      graph,
      contextForm,
      contextTab,
      validation,
      busyAction,
      saveStatus,
      message,
      error,
      autoSaveError,
      stoppingFlow,
      isActionBusy,
      isEditorReadOnly,
      readOnlyReason,
      publishDisabled,
    },
    data: {
      definitionsLoading,
      definitionsError,
      visibleDefinitions,
      filteredDefinitions,
      detail,
      openPositions,
      openPositionsMeta,
      openPositionsLoading,
      livePrices,
      userTelegramBotTokenMasked,
      userTelegramDefaultChatId,
      canStopSelectedFlow,
    },
    actions: {
      setDraftName,
      setDraftDescription,
      setCreateName,
      setCreateDescription,
      setCreateTemplateKind,
      setIsWorkflowListOpen,
      setWorkflowListQuery,
      setContextForm,
      setContextTab,
      setHasPendingCanvasNodeDraft,
      setError,
      requestDefinitionSwitch,
      createFromTemplate,
      saveDraft,
      validateGraph,
      reloadDraftFromServer,
      publishFlow,
      confirmAndDeleteCurrentFlow,
      deleteFlowFromList,
      handleStopFlow,
      updateGraphFromCanvas,
      applyContextFromForm,
      applyContextFromAdvanced,
      applyCanvasContextPatch,
      toggleDefinitionSelection,
      selectAllDefinitions,
      deselectAllDefinitions,
      bulkDeleteDefinitions,
    },
  };
}
