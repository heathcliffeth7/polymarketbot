'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { mutate as swrMutate } from 'swr';
import { toast } from 'sonner';
import { formatClientRequestError } from '@/lib/http-client';
import {
  buildContextFromForm,
  parseContextToForm,
  type ContextFormState,
} from '@/lib/trade-flow-config-mappers';
import {
  createStarterTradeFlowGraph,
  createDcaTradeFlowGraph,
  createStopLossTakeProfitGraph,
  createPositionMonitorNotifyGraph,
  createMultiLegHedgeGraph,
} from '@/lib/trade-flow-templates';
import type { TradeFlowDefinition, TradeFlowDefinitionDetail, TradeFlowGraph } from '@/lib/types';
import { useBotStatus } from './use-bot-status';
import { useConfig } from './use-config';
import {
  archiveTradeFlowDefinition,
  createTradeFlowDefinition,
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
  createSellBuyIfElseTemplate,
  deepCloneGraph,
  isRecord,
} from '@/components/trade-builder/flow-engine-utils';
import {
  compareFlowDetailSnapshotMeta,
  getFlowDetailSnapshotMeta,
} from './flow-detail-snapshot';
import type {
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
  const [archivingDefinitionId, setArchivingDefinitionId] = useState<number | null>(null);
  const [selectedDefinitionIds, setSelectedDefinitionIds] = useState<Set<number>>(new Set());
  const [bulkArchiving, setBulkArchiving] = useState(false);
  const [optimisticDefinitions, setOptimisticDefinitions] = useState<
    Array<TradeFlowDefinition & { _addedAt?: number }>
  >([]);
  const [graph, setGraph] = useState<TradeFlowGraph>({ context: {}, nodes: [], edges: [] });
  const [contextForm, setContextForm] = useState<ContextFormState>(parseContextToForm({}));
  const [contextTab, setContextTab] = useState<'basic' | 'advanced'>('basic');
  const [validation, setValidation] = useState<FlowEngineController['state']['validation']>(null);
  const [busyAction, setBusyAction] = useState<FlowEngineController['state']['busyAction']>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [autoSaveError, setAutoSaveError] = useState<string | null>(null);
  const [lastHydratedSnapshotKey, setLastHydratedSnapshotKey] = useState<string | null>(null);
  const [isGraphDirty, setIsGraphDirty] = useState(false);
  const [isSwitchingDefinition, setIsSwitchingDefinition] = useState(false);
  const [hasPendingCanvasNodeDraft, setHasPendingCanvasNodeDraft] = useState(false);
  const [stoppingFlow, setStoppingFlow] = useState(false);

  const { data: botStatus } = useBotStatus();
  const { livePrices: realtimeLivePrices } = useTradeFlowRealtime();
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
  const canvasAutoSaveRevisionRef = useRef(0);
  const latestDetailSnapshotRef = useRef<ReturnType<typeof getFlowDetailSnapshotMeta>>(null);
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

  const { data: definitionsData, mutate: mutateDefinitions, isLoading: definitionsLoading } =
    useTradeFlowDefinitions(1, 50, undefined, true);
  const definitions = useMemo(() => definitionsData?.data ?? [], [definitionsData?.data]);
  const mergedDefinitions = useMemo(() => {
    const serverVisible = definitions.filter((definition) => definition.status !== 'archived');
    const existingIds = new Set(serverVisible.map((definition) => definition.id));
    const optimisticMissing = optimisticDefinitions.filter(
      (definition) => definition.status !== 'archived' && !existingIds.has(definition.id)
    );
    return [...optimisticMissing, ...serverVisible];
  }, [definitions, optimisticDefinitions]);

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

  const visibleDefinitions = useMemo(
    () => mergedDefinitions.filter((definition) => definition.status !== 'archived'),
    [mergedDefinitions]
  );
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

  const bulkArchiveDefinitions = useCallback(async () => {
    if (selectedDefinitionIds.size === 0) return;
    if (
      !window.confirm(
        `${selectedDefinitionIds.size} workflow'u silmek (arsivlemek) istediginize emin misiniz?`
      )
    ) {
      return;
    }
    setBulkArchiving(true);
    try {
      for (const id of selectedDefinitionIds) {
        try {
          await archiveTradeFlowDefinition(id);
        } catch (err) {
          console.error(`Failed to archive definition ${id}:`, err);
        }
      }
      setSelectedDefinitionIds(new Set());
      swrMutate((key: string) => typeof key === 'string' && key.includes('/api/trade-flow/definitions'));
    } finally {
      setBulkArchiving(false);
    }
  }, [selectedDefinitionIds]);

  const saveCurrentDraftBeforeSwitch = useCallback(
    async (definitionId: number) => {
      if (!isGraphDirtyRef.current) return;
      const graphSnapshot = graphRef.current;
      const contextSnapshot = isRecord(graphSnapshot.context) ? graphSnapshot.context : {};
      const name = draftNameRef.current.trim();
      if (!name) {
        throw new Error('Flow adi bos olamaz.');
      }
      await patchTradeFlowDefinitionDraft(definitionId, {
        name,
        description: draftDescriptionRef.current.trim() || null,
        graphJson: { ...graphSnapshot, context: contextSnapshot },
      });
      await Promise.all([
        mutateDefinitions(),
        swrMutate(`/api/trade-flow/definitions/${definitionId}`),
      ]);
    },
    [mutateDefinitions]
  );

  const requestDefinitionSwitch = useCallback(
    async (nextDefinitionId: number) => {
      if (!Number.isFinite(nextDefinitionId) || nextDefinitionId <= 0) return false;
      if (switchLockRef.current) return false;
      const currentDefinitionId = selectedDefinitionIdRef.current;
      if (currentDefinitionId === nextDefinitionId) return true;

      switchLockRef.current = true;
      setIsSwitchingDefinition(true);
      setError(null);
      setMessage(null);
      try {
        if (currentDefinitionId && isGraphDirtyRef.current) {
          await saveCurrentDraftBeforeSwitch(currentDefinitionId);
          setAutoSaveError(null);
          setMessage(`Workflow #${currentDefinitionId} icin draft otomatik kaydedildi.`);
        }
        setSelectedDefinitionId(nextDefinitionId);
        setGraphDirtyState(false);
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
    [saveCurrentDraftBeforeSwitch, setGraphDirtyState]
  );

  useEffect(() => {
    if (visibleDefinitions.length === 0) {
      if (definitionsLoading || busyAction) return;
      setSelectedDefinitionId(null);
      setGraphDirtyState(false);
      setLastHydratedSnapshotKey(null);
      return;
    }
    const stillExists = visibleDefinitions.some(
      (definition) => definition.id === selectedDefinitionId
    );
    if (!selectedDefinitionId || !stillExists) {
      void requestDefinitionSwitch(visibleDefinitions[0].id);
    }
  }, [
    busyAction,
    definitionsLoading,
    requestDefinitionSwitch,
    selectedDefinitionId,
    setGraphDirtyState,
    visibleDefinitions,
  ]);

  const { data: detailData, mutate: mutateDetail } =
    useTradeFlowDefinitionDetail(selectedDefinitionId);
  const detail = useMemo(() => detailData?.data ?? null, [detailData?.data]);
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
    setLastHydratedSnapshotKey(null);
    canvasAutoSaveRevisionRef.current = 0;
    latestDetailSnapshotRef.current = null;
  }, [selectedDefinitionId, setGraphDirtyState]);

  const { data: openPositionsData, isLoading: openPositionsLoading } =
    useTradeFlowOpenPositions();
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

  useEffect(() => {
    if (!detail?.draftVersion || !incomingSnapshotKey) return;
    if (isGraphDirtyRef.current) return;
    if (incomingSnapshotKey === lastHydratedSnapshotKey) return;
    hydrateEditorFromDetail(detail);
  }, [detail, hydrateEditorFromDetail, incomingSnapshotKey, lastHydratedSnapshotKey]);

  const resolveContextInput = useCallback(() => {
    if (contextTab === 'advanced') {
      try {
        const parsed = JSON.parse(contextForm.advancedJson) as unknown;
        if (!isRecord(parsed)) throw new Error('Context JSON nesne olmali.');
        return { context: parsed as Record<string, unknown>, errorMessage: null };
      } catch (err) {
        return {
          context: null,
          errorMessage:
            err instanceof Error ? `Context JSON hatali: ${err.message}` : 'Context JSON hatali.',
        };
      }
    }

    return {
      context: buildContextFromForm(contextForm),
      errorMessage: null,
    };
  }, [contextForm, contextTab]);

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

  const buildDraftPersistPayload = (graphJson: TradeFlowGraph) => {
    const name = draftName.trim();
    if (!name) {
      throw new Error('Flow adi bos olamaz.');
    }
    return {
      name,
      description: draftDescription.trim() || null,
      graphJson,
    };
  };

  const formatOperationError = (err: unknown, fallback: string) =>
    formatClientRequestError(err, fallback);

  const createFromTemplate = async (kind: TemplateKind) => {
    const name = createName.trim();
    if (!name) {
      setError('Yeni flow icin ad zorunlu.');
      return;
    }
    setBusyAction('create');
    setError(null);
    setMessage(null);
    try {
      const templateMap: Record<TemplateKind, () => TradeFlowGraph> = {
        starter: () => createStarterTradeFlowGraph(defaultMarketSlug, defaultOutcome),
        sell_buy_if: () => createSellBuyIfElseTemplate(defaultMarketSlug, defaultOutcome),
        dca: () => createDcaTradeFlowGraph(defaultMarketSlug, defaultOutcome),
        sl_tp: () => createStopLossTakeProfitGraph(defaultMarketSlug, defaultOutcome),
        position_monitor: () =>
          createPositionMonitorNotifyGraph(defaultMarketSlug, defaultOutcome),
        multi_leg_hedge: () => createMultiLegHedgeGraph(defaultMarketSlug, defaultOutcome),
      };
      const templateLabels: Record<TemplateKind, string> = {
        starter: 'Starter flow olusturuldu.',
        sell_buy_if: 'Satis + If/Else + Alis sablonu olusturuldu.',
        dca: 'DCA sablonu olusturuldu.',
        sl_tp: 'Stop Loss + Take Profit sablonu olusturuldu.',
        position_monitor: 'Pozisyon Izleme + Bildirim sablonu olusturuldu.',
        multi_leg_hedge: 'Multi-Leg Hedge sablonu olusturuldu.',
      };
      const template = templateMap[kind]();
      const created = await createTradeFlowDefinition({
        name,
        description: createDescription.trim() || null,
        graphJson: template,
      });
      const createdDetail = created.data;
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
      setMessage(templateLabels[kind]);
      await mutateDefinitions();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Flow olusturulamadi.');
    } finally {
      setBusyAction(null);
    }
  };

  const validateGraph = async () => {
    if (!selectedDefinitionId) {
      setError('Once bir flow secin.');
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
      setError(formatOperationError(err, 'Dogrulama yapilamadi.'));
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
      setError(formatOperationError(err, 'Draft sunucudan yuklenemedi.'));
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
    const { context: ctx, errorMessage } = resolveContextInput();
    if (!ctx) {
      setError(errorMessage ?? 'Context JSON hatali.');
      return;
    }

    setBusyAction('save');
    setError(null);
    setMessage(null);
    try {
      invalidateCanvasAutoSaveRevision();
      const payload = buildDraftPersistPayload({ ...graphRef.current, context: ctx });
      setGraphState(payload.graphJson);
      const updated = await patchTradeFlowDefinitionDraft(selectedDefinitionId, payload);
      hydrateEditorFromDetail(updated.data);
      setAutoSaveError(null);
      setMessage('Draft flow kaydedildi.');
      await mutateDefinitions();
      await mutateDetail();
    } catch (err) {
      const reason = formatOperationError(err, 'Draft kaydedilemedi.');
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
    if (autoSaveError) {
      setError(
        'Autosave/PATCH hatasi duzelmeden publish edilemez. Draft Kaydet veya Taslagi Sunucudan Yukle kullan.'
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
      invalidateCanvasAutoSaveRevision();
      const baseDraftGraph: TradeFlowGraph = { ...graphRef.current, context: ctx };
      const prepared = await prepareDualDcaGraphForPublish(
        baseDraftGraph,
        publishDefinitionId,
        ensureDualDcaSourceTrade
      );
      const payload = buildDraftPersistPayload(prepared.graphJson);
      ensuredSourceTradeId = prepared.sourceTradeId;
      ensuredSourceTradeCreated = prepared.created;
      setGraphState(payload.graphJson);
      await patchTradeFlowDefinitionDraft(publishDefinitionId, payload);
      setGraphDirtyState(false);
      setAutoSaveError(null);
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
      const reason = formatOperationError(err, 'Flow publish edilemedi.');
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

  const archiveFlow = async () => {
    if (!selectedDefinitionId) {
      setError('Once bir flow secin.');
      return;
    }
    const targetId = selectedDefinitionId;
    setBusyAction('archive');
    setError(null);
    setMessage(null);
    try {
      const archived = await archiveTradeFlowDefinition(targetId);
      setOptimisticDefinitions((previous) =>
        previous.filter((definition) => definition.id !== targetId)
      );
      hydrateEditorFromDetail(archived.data);
      setValidation(null);
      setMessage('Flow arsive alindi. Aktif run varsa iptal edildi.');
      await mutateDefinitions();
      await mutateDetail();
    } catch (err) {
      setError(formatOperationError(err, 'Flow arsive alinamadi.'));
    } finally {
      setBusyAction(null);
    }
  };

  const archiveFlowFromList = async (definitionId: number) => {
    const target = visibleDefinitions.find((definition) => definition.id === definitionId);
    const label = target ? `#${target.id} - ${target.name}` : `#${definitionId}`;
    if (!window.confirm(`${label} workflow silinsin mi? Bu islem arsivleme yapar.`)) return;

    setBusyAction('archive');
    setArchivingDefinitionId(definitionId);
    setError(null);
    setMessage(null);
    try {
      await archiveTradeFlowDefinition(definitionId);
      setOptimisticDefinitions((previous) =>
        previous.filter((definition) => definition.id !== definitionId)
      );
      if (selectedDefinitionId === definitionId) {
        setSelectedDefinitionId(null);
      }
      setValidation(null);
      setMessage(`Workflow ${label} silindi (arsivlendi).`);
      await mutateDefinitions();
      await mutateDetail();
    } catch (err) {
      setError(formatOperationError(err, 'Workflow silinemedi.'));
    } finally {
      setArchivingDefinitionId(null);
      setBusyAction(null);
    }
  };

  const confirmAndArchiveCurrentFlow = async () => {
    if (!selectedDefinitionId) {
      setError('Once bir flow secin.');
      return;
    }
    const target = visibleDefinitions.find((definition) => definition.id === selectedDefinitionId);
    const label = target ? `#${target.id} - ${target.name}` : `#${selectedDefinitionId}`;
    if (!window.confirm(`${label} workflow silinsin mi? Bu islem arsivleme yapar.`)) return;
    await archiveFlow();
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
      const reason = formatOperationError(err, 'Workflow durdurulamadi.');
      setError(reason);
      toast.error(reason);
    } finally {
      setStoppingFlow(false);
    }
  };

  const updateGraphFromCanvas = (
    nextGraph: TradeFlowGraph,
    options?: { allowGraphShrink?: boolean }
  ) => {
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

    const { context: ctx, errorMessage } = resolveContextInput();
    if (!ctx) {
      setError(errorMessage ?? 'Context JSON hatali.');
      return;
    }

    const optimisticGraph = { ...nextGraph, context: ctx };
    const definitionId = selectedDefinitionIdRef.current;
    const revision = invalidateCanvasAutoSaveRevision();
    setGraphState(optimisticGraph);
    setGraphDirtyState(true);
    setValidation(null);
    setError(null);
    if (!definitionId) return;

    const fallbackName = draftName.trim() || 'Untitled';
    const payload = {
      name: fallbackName,
      description: draftDescription.trim() || null,
      graphJson: optimisticGraph,
    };
    void patchTradeFlowDefinitionDraft(definitionId, payload)
      .then((updated) => {
        if (selectedDefinitionIdRef.current !== definitionId) return;
        if (canvasAutoSaveRevisionRef.current !== revision) return;
        hydrateEditorFromDetail(updated.data);
        setAutoSaveError(null);
      })
      .catch((err) => {
        if (selectedDefinitionIdRef.current !== definitionId) return;
        if (canvasAutoSaveRevisionRef.current !== revision) return;
        const reason = formatOperationError(err, 'Autosave basarisiz.');
        setAutoSaveError(reason);
        setError(reason);
        console.warn('[auto-save] PATCH failed:', err);
      });
  };

  const isActionBusy = busyAction !== null || isSwitchingDefinition;
  const publishDisabled = isActionBusy || Boolean(autoSaveError);

  const applyCanvasContextPatch = useCallback(
    async (patch: Record<string, unknown>, successMessage?: string) => {
      const definitionId = selectedDefinitionIdRef.current;
      if (!definitionId) {
        setError('Once bir flow secin.');
        return;
      }

      const previousGraph = graphRef.current;
      const previousContext = isRecord(previousGraph.context) ? previousGraph.context : {};
      const previousValidation = validation;
      const previousIsGraphDirty = isGraphDirtyRef.current;
      const mergedContext = mergeGraphContextPatch(previousGraph.context, patch);
      const nextGraph: TradeFlowGraph = { ...previousGraph, context: mergedContext };
      const fallbackName =
        visibleDefinitions.find((definition) => definition.id === definitionId)?.name ||
        'Untitled';
      const payload = {
        name: draftNameRef.current.trim() || fallbackName,
        description: draftDescriptionRef.current.trim() || null,
        graphJson: nextGraph,
      };

      invalidateCanvasAutoSaveRevision();
      setGraphState(nextGraph);
      setGraphDirtyState(true);
      setContextForm(parseContextToForm(mergedContext));
      setValidation(null);
      setError(null);
      setMessage(null);

      try {
        const updated = await patchTradeFlowDefinitionDraft(definitionId, payload);
        hydrateEditorFromDetail(updated.data);
        if (successMessage) {
          setMessage(successMessage);
        }
        await Promise.all([mutateDefinitions(), mutateDetail()]);
      } catch (err) {
        setGraphState(previousGraph);
        setContextForm(parseContextToForm(previousContext));
        setValidation(previousValidation);
        setGraphDirtyState(previousIsGraphDirty);
        setError(formatOperationError(err, 'Autoclaim degisikligi kaydedilemedi.'));
      }
    },
    [
      hydrateEditorFromDetail,
      invalidateCanvasAutoSaveRevision,
      mutateDefinitions,
      mutateDetail,
      setGraphDirtyState,
      setGraphState,
      validation,
      visibleDefinitions,
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
      archivingDefinitionId,
      selectedDefinitionIds,
      bulkArchiving,
      graph,
      contextForm,
      contextTab,
      validation,
      busyAction,
      message,
      error,
      autoSaveError,
      stoppingFlow,
      isActionBusy,
      publishDisabled,
    },
    data: {
      definitionsLoading,
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
      confirmAndArchiveCurrentFlow,
      archiveFlowFromList,
      handleStopFlow,
      updateGraphFromCanvas,
      applyContextFromForm,
      applyContextFromAdvanced,
      applyCanvasContextPatch,
      toggleDefinitionSelection,
      selectAllDefinitions,
      deselectAllDefinitions,
      bulkArchiveDefinitions,
    },
  };
}
