'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { mutate as swrMutate } from 'swr';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { FlowCanvasEditor } from '@/components/trade-builder/flow-canvas-editor';
import {
  archiveTradeFlowDefinition,
  createTradeFlowDefinition,
  ensureDualDcaSourceTrade,
  patchTradeFlowDefinitionDraft,
  publishTradeFlowDefinition,
  useTradeFlowDefinitionDetail,
  useTradeFlowDefinitions,
  useTradeFlowOpenPositions,
  useTradeFlowRunEvents,
  useTradeFlowRuns,
  useTradeFlowVersions,
  validateTradeFlowDefinition,
} from '@/hooks/use-trade-flow';
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
import type {
  TradeFlowDefinition,
  TradeFlowDefinitionDetail,
  TradeFlowGraph,
  TradeFlowValidationResult,
} from '@/lib/types';
import {
  buildDetailSnapshotKey,
  createSellBuyIfElseTemplate,
  deepCloneGraph,
  isRecord,
} from './flow-engine-utils';
import {
  CreateFlowSlot,
  FlowContextEditor,
  FlowRunEventsCard,
  FlowSummaryBar,
  FlowVersionsCard,
} from './flow-engine-sections';

type BusyAction = 'create' | 'save' | 'validate' | 'publish' | 'archive' | null;
type TemplateKind = 'starter' | 'sell_buy_if' | 'dca' | 'sl_tp' | 'position_monitor' | 'multi_leg_hedge';

const DUAL_DCA_ALLOWED_ASSETS = new Set(['btc', 'eth', 'sol', 'xrp']);
const DUAL_DCA_ALLOWED_TIMEFRAMES = new Set(['5m', '15m']);

function toPositiveNumber(value: unknown): number | null {
  const parsed =
    typeof value === 'number'
      ? value
      : typeof value === 'string'
        ? Number(value)
        : Number.NaN;
  if (!Number.isFinite(parsed) || parsed <= 0) return null;
  return parsed;
}

function normalizeDualDcaAsset(config: Record<string, unknown>): 'btc' | 'eth' | 'sol' | 'xrp' | null {
  const raw = String(config.asset ?? config.coin ?? '').trim().toLowerCase();
  if (!DUAL_DCA_ALLOWED_ASSETS.has(raw)) return null;
  return raw as 'btc' | 'eth' | 'sol' | 'xrp';
}

function normalizeDualDcaTimeframe(config: Record<string, unknown>): '5m' | '15m' | null {
  const raw = String(config.timeframe ?? config.marketPeriod ?? '').trim().toLowerCase();
  const normalized =
    raw === '5' || raw === '5min' || raw === '5 min'
      ? '5m'
      : raw === '15' || raw === '15min' || raw === '15 min'
        ? '15m'
        : raw;
  if (!DUAL_DCA_ALLOWED_TIMEFRAMES.has(normalized)) return null;
  return normalized as '5m' | '15m';
}

interface FlowEnginePanelProps {
  defaultMarketSlug: string | null;
  defaultOutcome: { token_id: string; label: string } | null;
  externalSelectDefinitionId?: number | null;
  externalCreatedDefinition?: TradeFlowDefinition | null;
}

export function FlowEnginePanel({
  defaultMarketSlug,
  defaultOutcome,
  externalSelectDefinitionId = null,
  externalCreatedDefinition = null,
}: FlowEnginePanelProps) {
  const [selectedDefinitionId, setSelectedDefinitionId] = useState<number | null>(null);
  const [draftName, setDraftName] = useState('');
  const [draftDescription, setDraftDescription] = useState('');
  const [createName, setCreateName] = useState('');
  const [createDescription, setCreateDescription] = useState('');
  const [createTemplateKind, setCreateTemplateKind] = useState<TemplateKind>('starter');
  const [isWorkflowListOpen, setIsWorkflowListOpen] = useState(false);
  const [workflowListQuery, setWorkflowListQuery] = useState('');
  const [archivingDefinitionId, setArchivingDefinitionId] = useState<number | null>(null);
  const [optimisticDefinitions, setOptimisticDefinitions] = useState<
    Array<TradeFlowDefinition & { _addedAt?: number }>
  >([]);
  const [graph, setGraph] = useState<TradeFlowGraph>({ context: {}, nodes: [], edges: [] });
  const [contextForm, setContextForm] = useState<ContextFormState>(parseContextToForm({}));
  const [contextTab, setContextTab] = useState<'basic' | 'advanced'>('basic');
  const [validation, setValidation] = useState<TradeFlowValidationResult | null>(null);
  const [selectedRunId, setSelectedRunId] = useState<number | null>(null);
  const [busyAction, setBusyAction] = useState<BusyAction>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [lastHydratedSnapshotKey, setLastHydratedSnapshotKey] = useState<string | null>(null);
  const [isGraphDirty, setIsGraphDirty] = useState(false);
  const [isSwitchingDefinition, setIsSwitchingDefinition] = useState(false);
  const [hasPendingCanvasNodeDraft, setHasPendingCanvasNodeDraft] = useState(false);

  const selectedDefinitionIdRef = useRef<number | null>(selectedDefinitionId);
  const switchLockRef = useRef(false);
  const graphRef = useRef(graph);
  const draftNameRef = useRef(draftName);
  const draftDescriptionRef = useRef(draftDescription);
  const isGraphDirtyRef = useRef(isGraphDirty);
  selectedDefinitionIdRef.current = selectedDefinitionId;
  graphRef.current = graph;
  draftNameRef.current = draftName;
  draftDescriptionRef.current = draftDescription;
  isGraphDirtyRef.current = isGraphDirty;

  const { data: definitionsData, mutate: mutateDefinitions, isLoading: definitionsLoading } =
    useTradeFlowDefinitions(1, 50, undefined, true);
  const definitions = useMemo(() => definitionsData?.data ?? [], [definitionsData?.data]);
  const mergedDefinitions = useMemo(() => {
    const serverVisible = definitions.filter((d) => d.status !== 'archived');
    const existingIds = new Set(serverVisible.map((d) => d.id));
    const optimisticMissing = optimisticDefinitions.filter((d) => d.status !== 'archived' && !existingIds.has(d.id));
    return [...optimisticMissing, ...serverVisible];
  }, [definitions, optimisticDefinitions]);

  useEffect(() => {
    if (optimisticDefinitions.length === 0) return;
    const serverIds = new Set(definitions.map((d) => d.id));
    const now = Date.now();
    setOptimisticDefinitions((prev) => {
      const next = prev.filter((d) => {
        const age = now - (d._addedAt ?? 0);
        return !(serverIds.has(d.id) && age > 30_000);
      });
      return next.length === prev.length ? prev : next;
    });
  }, [definitions, optimisticDefinitions.length]);

  const visibleDefinitions = useMemo(() => mergedDefinitions.filter((d) => d.status !== 'archived'), [mergedDefinitions]);
  const filteredDefinitions = useMemo(() => {
    const query = workflowListQuery.trim().toLowerCase();
    if (!query) return visibleDefinitions;
    return visibleDefinitions.filter((d) => `${d.id} ${d.name} ${d.status}`.toLowerCase().includes(query));
  }, [visibleDefinitions, workflowListQuery]);

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
          setMessage(`Workflow #${currentDefinitionId} icin draft otomatik kaydedildi.`);
        }
        setSelectedDefinitionId(nextDefinitionId);
        setIsGraphDirty(false);
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
    [saveCurrentDraftBeforeSwitch]
  );

  useEffect(() => {
    if (visibleDefinitions.length === 0) {
      setSelectedDefinitionId(null);
      setIsGraphDirty(false);
      setLastHydratedSnapshotKey(null);
      return;
    }
    if (externalSelectDefinitionId != null && externalSelectDefinitionId > 0) return;
    const stillExists = visibleDefinitions.some((d) => d.id === selectedDefinitionId);
    if (!selectedDefinitionId || !stillExists) {
      void requestDefinitionSwitch(visibleDefinitions[0].id);
    }
  }, [externalSelectDefinitionId, requestDefinitionSwitch, selectedDefinitionId, visibleDefinitions]);

  useEffect(() => {
    if (externalSelectDefinitionId == null || externalSelectDefinitionId <= 0) return;
    if (selectedDefinitionId === externalSelectDefinitionId) return;
    void requestDefinitionSwitch(externalSelectDefinitionId);
  }, [externalSelectDefinitionId, requestDefinitionSwitch, selectedDefinitionId]);

  useEffect(() => {
    if (!externalCreatedDefinition) return;
    setOptimisticDefinitions((prev) => {
      const next = [
        { ...externalCreatedDefinition, _addedAt: Date.now() },
        ...prev.filter((d) => d.id !== externalCreatedDefinition.id),
      ];
      return next.slice(0, 20);
    });
    setIsWorkflowListOpen(true);
    void requestDefinitionSwitch(externalCreatedDefinition.id);
  }, [externalCreatedDefinition, requestDefinitionSwitch]);

  const { data: detailData, mutate: mutateDetail } = useTradeFlowDefinitionDetail(selectedDefinitionId);
  const detail = useMemo(() => detailData?.data ?? null, [detailData?.data]);
  const incomingSnapshotKey = useMemo(() => buildDetailSnapshotKey(detail), [detail]);

  useEffect(() => {
    setIsGraphDirty(false);
    setLastHydratedSnapshotKey(null);
  }, [selectedDefinitionId]);

  useEffect(() => {
    if (!detail?.draftVersion || !incomingSnapshotKey) return;
    if (isGraphDirty) return;
    if (incomingSnapshotKey === lastHydratedSnapshotKey) return;
    const normalized = deepCloneGraph(detail.draftVersion.graph_json);
    setGraph(normalized);
    setContextForm(parseContextToForm(normalized.context || {}));
    setDraftName(detail.definition.name);
    setDraftDescription(detail.definition.description || '');
    setValidation(null);
    setIsGraphDirty(false);
    setLastHydratedSnapshotKey(incomingSnapshotKey);
  }, [detail, incomingSnapshotKey, isGraphDirty, lastHydratedSnapshotKey]);

  const { data: versionsData, isLoading: versionsLoading } = useTradeFlowVersions(selectedDefinitionId);
  const versions = versionsData?.data ?? [];
  const { data: runsData } = useTradeFlowRuns(1, 15, selectedDefinitionId || undefined);
  const runs = useMemo(() => runsData?.data ?? [], [runsData?.data]);

  useEffect(() => {
    if (runs.length === 0) { setSelectedRunId(null); return; }
    const latestRunning = runs.find((r) => r.status === 'running');
    if (latestRunning && selectedRunId !== latestRunning.id) {
      setSelectedRunId(latestRunning.id);
      return;
    }
    const exists = runs.some((r) => r.id === selectedRunId);
    if (!selectedRunId || !exists) setSelectedRunId(runs[0].id);
  }, [runs, selectedRunId]);

  const { data: runEventsData } = useTradeFlowRunEvents(selectedRunId, 1, 25, !!selectedRunId);
  const runEvents = runEventsData?.data ?? [];
  const { data: openPositionsData, isLoading: openPositionsLoading } = useTradeFlowOpenPositions();
  const openPositions = useMemo(() => openPositionsData?.data ?? [], [openPositionsData?.data]);
  const openPositionsMeta = useMemo(() => openPositionsData?.meta ?? null, [openPositionsData?.meta]);

  const hydrateEditorFromDetail = (d: TradeFlowDefinitionDetail | null) => {
    if (!d?.draftVersion) return;
    const normalized = deepCloneGraph(d.draftVersion.graph_json);
    setGraph(normalized);
    setContextForm(parseContextToForm(normalized.context || {}));
    setDraftName(d.definition.name);
    setDraftDescription(d.definition.description || '');
    setValidation(null);
    setIsGraphDirty(false);
    setLastHydratedSnapshotKey(buildDetailSnapshotKey(d));
  };

  const createFromTemplate = async (kind: TemplateKind) => {
    const name = createName.trim();
    if (!name) { setError('Yeni flow icin ad zorunlu.'); return; }
    setBusyAction('create'); setError(null); setMessage(null);
    try {
      const templateMap: Record<TemplateKind, () => import('@/lib/types').TradeFlowGraph> = {
        starter: () => createStarterTradeFlowGraph(defaultMarketSlug, defaultOutcome),
        sell_buy_if: () => createSellBuyIfElseTemplate(defaultMarketSlug, defaultOutcome),
        dca: () => createDcaTradeFlowGraph(defaultMarketSlug, defaultOutcome),
        sl_tp: () => createStopLossTakeProfitGraph(defaultMarketSlug, defaultOutcome),
        position_monitor: () => createPositionMonitorNotifyGraph(defaultMarketSlug, defaultOutcome),
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
      const created = await createTradeFlowDefinition({ name, description: createDescription.trim() || null, graphJson: template });
      const cd = created.data;
      setOptimisticDefinitions((prev) => {
        const next = [{ ...cd.definition, _addedAt: Date.now() }, ...prev.filter((d) => d.id !== cd.definition.id)];
        return next.slice(0, 20);
      });
      setIsWorkflowListOpen(true); setWorkflowListQuery(''); setCreateName(''); setCreateDescription('');
      const switched = await requestDefinitionSwitch(cd.definition.id);
      if (!switched) return;
      hydrateEditorFromDetail(cd);
      setValidation(null);
      setMessage(templateLabels[kind]);
      await mutateDefinitions();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Flow olusturulamadi.');
    } finally { setBusyAction(null); }
  };

  const applyContextFromForm = (): Record<string, unknown> => {
    const parsed = buildContextFromForm(contextForm);
    setGraph((prev) => ({ ...prev, context: parsed }));
    setContextForm(parseContextToForm(parsed));
    setIsGraphDirty(true); setError(null);
    return parsed;
  };

  const applyContextFromAdvanced = (): Record<string, unknown> | null => {
    try {
      const parsed = JSON.parse(contextForm.advancedJson) as unknown;
      if (!isRecord(parsed)) throw new Error('Context JSON nesne olmali.');
      setGraph((prev) => ({ ...prev, context: parsed }));
      setContextForm(parseContextToForm(parsed));
      setIsGraphDirty(true); setError(null);
      return parsed;
    } catch (err) {
      setError(err instanceof Error ? `Context JSON hatali: ${err.message}` : 'Context JSON hatali.');
      return null;
    }
  };

  const resolveContext = (): Record<string, unknown> | null =>
    contextTab === 'advanced' ? applyContextFromAdvanced() : applyContextFromForm();

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

  const ensureDualDcaSourceTradeForPublish = async (
    draftGraph: TradeFlowGraph,
    definitionId: number
  ): Promise<{ graphJson: TradeFlowGraph; sourceTradeId: number | null; created: boolean }> => {
    const dualDcaNodes = draftGraph.nodes.filter((node) => node.type === 'action.dual_dca');
    if (dualDcaNodes.length === 0) {
      return { graphJson: draftGraph, sourceTradeId: null, created: false };
    }

    const contextSourceTradeId = toPositiveNumber(draftGraph.context.sourceTradeId);
    const nodeSourceTradeIds = dualDcaNodes
      .map((node) => toPositiveNumber((isRecord(node.config) ? node.config.sourceTradeId : null)))
      .filter((value): value is number => value != null);
    const existingSourceTradeId = contextSourceTradeId ?? nodeSourceTradeIds[0] ?? null;

    if (existingSourceTradeId != null) {
      let changed = contextSourceTradeId !== existingSourceTradeId;
      const nextContext =
        contextSourceTradeId === existingSourceTradeId
          ? draftGraph.context
          : { ...draftGraph.context, sourceTradeId: existingSourceTradeId };
      const nextNodes = draftGraph.nodes.map((node) => {
        if (node.type !== 'action.dual_dca') return node;
        const currentNodeSourceTradeId = toPositiveNumber(
          isRecord(node.config) ? node.config.sourceTradeId : null
        );
        if (currentNodeSourceTradeId != null) return node;
        changed = true;
        return {
          ...node,
          config: {
            ...node.config,
            sourceTradeId: existingSourceTradeId,
          },
        };
      });
      if (!changed) return { graphJson: draftGraph, sourceTradeId: existingSourceTradeId, created: false };
      return {
        graphJson: {
          ...draftGraph,
          context: nextContext,
          nodes: nextNodes,
        },
        sourceTradeId: existingSourceTradeId,
        created: false,
      };
    }

    const primaryDualNode = dualDcaNodes[0];
    const config = isRecord(primaryDualNode.config) ? primaryDualNode.config : {};
    const asset = normalizeDualDcaAsset(config);
    const timeframe = normalizeDualDcaTimeframe(config);
    if (!asset || !timeframe) {
      return { graphJson: draftGraph, sourceTradeId: null, created: false };
    }

    const ensured = await ensureDualDcaSourceTrade({
      asset,
      timeframe,
      definitionId,
      nodeKey: primaryDualNode.key,
    });
    const ensuredSourceTradeId = toPositiveNumber(ensured.data.sourceTradeId);
    if (ensuredSourceTradeId == null) {
      throw new Error('Dual DCA sourceTradeId otomatik olusturulamadi.');
    }

    return {
      graphJson: {
        ...draftGraph,
        context: {
          ...draftGraph.context,
          sourceTradeId: ensuredSourceTradeId,
        },
        nodes: draftGraph.nodes.map((node) => {
          if (node.type !== 'action.dual_dca') return node;
          const existing = toPositiveNumber(isRecord(node.config) ? node.config.sourceTradeId : null);
          if (existing != null) return node;
          return {
            ...node,
            config: {
              ...node.config,
              sourceTradeId: ensuredSourceTradeId,
            },
          };
        }),
      },
      sourceTradeId: ensuredSourceTradeId,
      created: Boolean(ensured.data.created),
    };
  };

  const formatOperationError = (err: unknown, fallback: string) =>
    formatClientRequestError(err, fallback);

  const validateGraph = async () => {
    if (!selectedDefinitionId) { setError('Once bir flow secin.'); return; }
    const ctx = resolveContext(); if (!ctx) return;
    setBusyAction('validate'); setError(null); setMessage(null);
    try {
      const result = await validateTradeFlowDefinition(selectedDefinitionId, { graphJson: { ...graph, context: ctx } });
      setValidation(result.data);
      setMessage(result.data.valid ? 'Flow dogrulama basarili.' : 'Flow dogrulamada sorunlar bulundu.');
    } catch (err) { setError(formatOperationError(err, 'Dogrulama yapilamadi.')); }
    finally { setBusyAction(null); }
  };

  const saveDraft = async () => {
    if (!selectedDefinitionId) { setError('Once bir flow secin.'); return; }
    const ctx = resolveContext(); if (!ctx) return;
    setBusyAction('save'); setError(null); setMessage(null);
    try {
      const payload = buildDraftPersistPayload({ ...graph, context: ctx });
      setGraph(payload.graphJson);
      const updated = await patchTradeFlowDefinitionDraft(selectedDefinitionId, payload);
      hydrateEditorFromDetail(updated.data);
      setMessage('Draft flow kaydedildi.');
      await mutateDefinitions(); await mutateDetail();
    } catch (err) { setError(formatOperationError(err, 'Draft kaydedilemedi.')); }
    finally { setBusyAction(null); }
  };

  const publishFlow = async () => {
    if (!selectedDefinitionId) { setError('Once bir flow secin.'); return; }
    const publishDefinitionId = selectedDefinitionId;
    const selectedDefinition = visibleDefinitions.find((d) => d.id === publishDefinitionId) ?? null;
    const publishName = draftName.trim() || selectedDefinition?.name || `Workflow ${publishDefinitionId}`;
    const publishLabel = `#${publishDefinitionId} - ${publishName}`;
    if (hasPendingCanvasNodeDraft) {
      setError("Node formunda uygulanmamis degisiklik var. Once 'Node Guncelle' veya 'JSON ile Guncelle' kullan.");
      return;
    }
    const publishConfirmed = window.confirm(
      `${publishLabel} publish edilsin mi?\n\nNot: Sadece DCA akisi istiyorsan canvas'ta trigger node olmamali.`
    );
    if (!publishConfirmed) return;
    const ctx = resolveContext(); if (!ctx) return;
    setBusyAction('publish'); setError(null); setMessage(null);
    let draftSaved = false;
    let ensuredSourceTradeId: number | null = null;
    let ensuredSourceTradeCreated = false;
    try {
      const baseDraftGraph: TradeFlowGraph = { ...graph, context: ctx };
      const prepared = await ensureDualDcaSourceTradeForPublish(baseDraftGraph, publishDefinitionId);
      const payload = buildDraftPersistPayload(prepared.graphJson);
      ensuredSourceTradeId = prepared.sourceTradeId;
      ensuredSourceTradeCreated = prepared.created;
      setGraph(payload.graphJson);
      await patchTradeFlowDefinitionDraft(publishDefinitionId, payload);
      setIsGraphDirty(false);
      draftSaved = true;
      const published = await publishTradeFlowDefinition(publishDefinitionId);
      hydrateEditorFromDetail(published.data); setValidation(null);
      if (ensuredSourceTradeId != null && ensuredSourceTradeCreated) {
        setMessage(`${publishLabel} publish edildi. Source Trade otomatik olusturuldu: #${ensuredSourceTradeId}.`);
      } else if (ensuredSourceTradeId != null) {
        setMessage(`${publishLabel} publish edildi. Source Trade atandi: #${ensuredSourceTradeId}.`);
      } else {
        setMessage(`${publishLabel} publish edildi.`);
      }
      await mutateDefinitions(); await mutateDetail();
    } catch (err) {
      const reason = formatOperationError(err, 'Flow publish edilemedi.');
      setError(
        draftSaved
          ? `Draft kaydedildi ama publish basarisiz (${publishLabel}). Neden: ${reason}`
          : `Publish basarisiz (${publishLabel}). Neden: ${reason}`
      );
    }
    finally { setBusyAction(null); }
  };

  const archiveFlow = async () => {
    if (!selectedDefinitionId) { setError('Once bir flow secin.'); return; }
    const targetId = selectedDefinitionId;
    setBusyAction('archive'); setError(null); setMessage(null);
    try {
      const archived = await archiveTradeFlowDefinition(targetId);
      setOptimisticDefinitions((prev) => prev.filter((d) => d.id !== targetId));
      hydrateEditorFromDetail(archived.data); setValidation(null);
      setMessage('Flow arsive alindi. Aktif run varsa iptal edildi.');
      await mutateDefinitions(); await mutateDetail();
    } catch (err) { setError(formatOperationError(err, 'Flow arsive alinamadi.')); }
    finally { setBusyAction(null); }
  };

  const archiveFlowFromList = async (defId: number) => {
    const target = visibleDefinitions.find((d) => d.id === defId);
    const label = target ? `#${target.id} - ${target.name}` : `#${defId}`;
    if (!window.confirm(`${label} workflow silinsin mi? Bu islem arsivleme yapar.`)) return;
    setBusyAction('archive'); setArchivingDefinitionId(defId); setError(null); setMessage(null);
    try {
      await archiveTradeFlowDefinition(defId);
      setOptimisticDefinitions((prev) => prev.filter((d) => d.id !== defId));
      if (selectedDefinitionId === defId) setSelectedDefinitionId(null);
      setValidation(null); setMessage(`Workflow ${label} silindi (arsivlendi).`);
      await mutateDefinitions(); await mutateDetail();
    } catch (err) { setError(formatOperationError(err, 'Workflow silinemedi.')); }
    finally { setArchivingDefinitionId(null); setBusyAction(null); }
  };

  const confirmAndArchiveCurrentFlow = async () => {
    if (!selectedDefinitionId) {
      setError('Once bir flow secin.');
      return;
    }
    const target = visibleDefinitions.find((d) => d.id === selectedDefinitionId);
    const label = target ? `#${target.id} - ${target.name}` : `#${selectedDefinitionId}`;
    if (!window.confirm(`${label} workflow silinsin mi? Bu islem arsivleme yapar.`)) return;
    await archiveFlow();
  };

  const updateGraphFromCanvas = (nextGraph: TradeFlowGraph) => {
    setGraph(nextGraph); setIsGraphDirty(true); setValidation(null);
  };

  const isActionBusy = busyAction !== null || isSwitchingDefinition;

  const applyCanvasContextPatch = (patch: Record<string, unknown>) => {
    const merged = { ...(isRecord(graph.context) ? graph.context : {}), ...patch };
    setGraph((prev) => ({ ...prev, context: merged }));
    setIsGraphDirty(true);
    setContextForm(parseContextToForm(merged));
    setContextTab('basic'); setValidation(null); setError(null);
  };

  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader>
        <CardTitle className="text-sm font-medium text-zinc-300">
          Flow Engine Otomasyon (If / Else + Birlesik Satis/Alis)
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-5">
        <p className="text-xs text-zinc-500">
          Bu bolum n8n benzeri canvas akisiyla calisir: node surukleyin, edge baglayin, if/else
          mantigini tek akis icinde kurun.
        </p>

        <div className="rounded-lg border border-zinc-800 bg-zinc-950/40 p-3">
          <p className="mb-3 text-xs text-zinc-400">Flow Secimi ve Meta</p>
          <div className="grid gap-3 md:grid-cols-3">
            <div className="space-y-2">
              <p className="text-xs text-zinc-500">Flow Tanimi</p>
              <select
                value={selectedDefinitionId ?? ''}
                onChange={(e) => {
                  const nextDefinitionId = Number(e.target.value);
                  if (!Number.isFinite(nextDefinitionId) || nextDefinitionId <= 0) return;
                  void requestDefinitionSwitch(nextDefinitionId);
                }}
                disabled={isActionBusy}
                className="h-9 w-full rounded-md border border-zinc-700 bg-zinc-800 px-3 text-sm text-zinc-200"
              >
                {visibleDefinitions.length === 0 && <option value="">Flow yok</option>}
                {visibleDefinitions.map((d) => (
                  <option key={d.id} value={d.id}>#{d.id} - {d.name} ({d.status})</option>
                ))}
              </select>
              {definitionsLoading && <p className="text-[11px] text-zinc-500">Flow listesi yukleniyor...</p>}
            </div>
            <div className="space-y-2">
              <p className="text-xs text-zinc-500">Flow Adi (Draft)</p>
              <Input value={draftName} onChange={(e) => setDraftName(e.target.value)} className="border-zinc-700 bg-zinc-800 text-zinc-200" />
            </div>
            <div className="space-y-2">
              <p className="text-xs text-zinc-500">Aciklama (Draft)</p>
              <Input value={draftDescription} onChange={(e) => setDraftDescription(e.target.value)} className="border-zinc-700 bg-zinc-800 text-zinc-200" />
            </div>

            <FlowContextEditor
              contextForm={contextForm} contextTab={contextTab}
              onContextFormChange={setContextForm} onContextTabChange={setContextTab}
              onApplyFromForm={applyContextFromForm} onApplyFromAdvanced={() => { applyContextFromAdvanced(); }}
            />
          </div>

          <div className="mt-3 flex flex-wrap gap-2">
            <Button disabled={isActionBusy} onClick={saveDraft}>Draft Kaydet</Button>
            <Button variant="outline" className="border-zinc-700 text-zinc-300" disabled={isActionBusy} onClick={validateGraph}>Dogrula</Button>
            <Button variant="outline" className="border-zinc-700 text-zinc-300" disabled={isActionBusy} onClick={publishFlow}>Publish</Button>
            <Button
              variant="outline"
              className="border-zinc-700 text-zinc-300"
              disabled={isActionBusy}
              onClick={() => { void confirmAndArchiveCurrentFlow(); }}
            >
              Sil (Arsivle)
            </Button>
          </div>
        </div>

        <FlowCanvasEditor
          graph={graph} onGraphChange={updateGraphFromCanvas} onError={setError}
          openPositions={openPositions} openPositionsMeta={openPositionsMeta}
          openPositionsLoading={openPositionsLoading}
          onApplyContextPatch={applyCanvasContextPatch}
          onPendingNodeDraftChange={setHasPendingCanvasNodeDraft}
          leftPanelTopSlot={
            <CreateFlowSlot
              createName={createName} createDescription={createDescription}
              createTemplateKind={createTemplateKind} busyAction={isSwitchingDefinition ? 'save' : busyAction}
              isWorkflowListOpen={isWorkflowListOpen} workflowListQuery={workflowListQuery}
              definitionsLoading={definitionsLoading} filteredDefinitions={filteredDefinitions}
              selectedDefinitionId={selectedDefinitionId} archivingDefinitionId={archivingDefinitionId}
              onCreateNameChange={setCreateName} onCreateDescriptionChange={setCreateDescription}
              onTemplateKindChange={setCreateTemplateKind}
              onCreateFromTemplate={(kind) => { void createFromTemplate(kind); }}
              onToggleWorkflowList={() => setIsWorkflowListOpen((prev) => !prev)}
              onWorkflowListQueryChange={setWorkflowListQuery}
              onSelectDefinition={(id) => { void requestDefinitionSwitch(id); }}
              onArchiveFromList={(id) => { void archiveFlowFromList(id); }}
              showWorkflowActions
              workflowActionsDisabled={isActionBusy}
              onSaveDraft={() => { void saveDraft(); }}
              onValidate={() => { void validateGraph(); }}
              onPublish={() => { void publishFlow(); }}
              onArchiveFlow={() => { void confirmAndArchiveCurrentFlow(); }}
            />
          }
        />

        <FlowSummaryBar graph={graph} validation={validation} />

        <div className="grid gap-4 lg:grid-cols-2">
          <FlowVersionsCard versions={versions} versionsLoading={versionsLoading} />
          <FlowRunEventsCard
            runs={runs} runEvents={runEvents}
            selectedRunId={selectedRunId} onSelectedRunChange={setSelectedRunId}
          />
        </div>

        {error && <p className="text-sm text-red-400">{error}</p>}
        {message && <p className="text-sm text-emerald-400">{message}</p>}
      </CardContent>
    </Card>
  );
}
