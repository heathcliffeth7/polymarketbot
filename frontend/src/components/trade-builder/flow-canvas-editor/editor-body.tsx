'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  MarkerType,
  addEdge,
  applyEdgeChanges,
  applyNodeChanges,
  useNodesInitialized,
  useReactFlow,
  type Connection,
  type EdgeChange,
  type NodeChange,
} from '@xyflow/react';
import { toast } from 'sonner';
import { ensureTradeFlowSourceTrade } from '@/hooks/use-trade-flow';
import { useCanvasLivePrices } from '@/hooks/use-trade-builder';
import { useCanvasHistory } from '@/hooks/use-canvas-history';
import type { NodeExecutionState, TradeFlowOpenPositionOption } from '@/lib/types';
import {
  buildEdgeConditionFromForm,
  buildNodeConfigFromForm,
  isPresetPlaceOrderMarker,
  parseEdgeConditionToForm,
  parseNodeConfigToForm,
  validateOutcomeConditionRow,
  type ConditionDraft,
  type DrawdownRuleRow,
  type EdgeConditionFormState,
  type NodeConfigFormState,
  type OutcomeConditionRow,
  type PrimitiveValueType,
} from '@/lib/trade-flow-config-mappers';
import {
  EDGE_STROKE_COLOR,
  GROUP_COLORS,
  NODE_TYPE_OPTIONS,
  type FlowCanvasGraphChangeOptions,
  type FlowCanvasEditorProps,
  type FlowEdge,
  type FlowNode,
  type NodeGroup,
  type NodePaletteCategory,
  type PlaceOrderPresetKind,
  type PlaceOrderPresetSeed,
} from '../flow-canvas-constants';
import {
  autoLayoutNodes,
  buildPlaceOrderPresetConfig,
  createEdgeKey,
  createGraphFingerprint,
  createNodeKey,
  extractPositionSeedFromNode,
  hasRequiredPlaceOrderSeed,
  isRecord,
  nodePaletteCategoryOf,
  toCanvasEdge,
  toCanvasNode,
  toDomainEdge,
  toDomainNode,
  toFiniteNumberValue,
  toTrimmedStringValue,
} from '../flow-canvas-utils';
import { createEdgeInspectorActions, createNodeInspectorActions } from './actions';
import {
  addDrawdownRuleState,
  addExpressionRowState,
  addOutcomeConditionState,
  addStatePatchRowState,
  removeDrawdownRuleState,
  removeExpressionRowState,
  removeOutcomeConditionState,
  removeStatePatchRowState,
  updateDrawdownRuleState,
  updateExpressionRowState,
  updateNodeFieldState,
  updateOutcomeConditionState,
  updateStatePatchRowState,
  updateTriggerSizeRowState,
} from './form-state';
import { useCanvasKeyboard } from '../flow-canvas-keyboard';
import { exportGraphAsJson, importGraphFromFile } from '../flow-import-export';
import { applyInheritedPlaceOrderMaxPriceConfig, normalizePresetPlaceOrderConfig } from './helpers';
import { FlowCanvasEditorLayout } from './layout';
import { validateNodeFormBeforeSave } from './node-form-validation';
import { useCanvasSelection } from './use-canvas-selection';
import { useMarketOutcomes } from './use-market-outcomes';
import { useSelectedNodeUpstream } from './use-selected-node-upstream';
import { useSyncPlaceOrderFormState } from './use-sync-place-order-form-state';

export function FlowCanvasEditorBody({
  graph,
  readOnly = false,
  readOnlyReason = null,
  onGraphChange,
  onError,
  onPendingNodeDraftChange,
  openPositions,
  openPositionsMeta,
  openPositionsLoading,
  onApplyContextPatch,
  leftPanelTopSlot,
  executionStates,
  livePrices,
  userTelegramBotTokenMasked,
  userTelegramDefaultChatId,
}: FlowCanvasEditorProps) {
  const graphFingerprint = useMemo(() => createGraphFingerprint(graph.nodes, graph.edges), [graph.nodes, graph.edges]);
  const [canvasNodes, setCanvasNodes] = useState<FlowNode[]>(() => graph.nodes.map(toCanvasNode));
  const [canvasEdges, setCanvasEdges] = useState<FlowEdge[]>(() => graph.edges.map(toCanvasEdge));
  const canvasNodesRef = useRef<FlowNode[]>(canvasNodes);
  const canvasEdgesRef = useRef<FlowEdge[]>(canvasEdges);
  const graphContextRef = useRef(graph.context);
  const lastAppliedGraphFingerprintRef = useRef<string>(graphFingerprint);
  const lastSeenPropGraphFingerprintRef = useRef<string>(graphFingerprint);
  const graphSyncGuardRef = useRef<{ until: number; nodes: number; edges: number } | null>(null);
  const canvasWrapperRef = useRef<HTMLDivElement | null>(null);
  const editorRootRef = useRef<HTMLDivElement | null>(null);
  const lastCanvasPointerRef = useRef<{ x: number; y: number } | null>(null);
  const [pendingFocusNodeId, setPendingFocusNodeId] = useState<string | null>(null);
  const reactFlow = useReactFlow<FlowNode, FlowEdge>();
  const nodesInitialized = useNodesInitialized();
  const history = useCanvasHistory();
  const [nodeInspectorTab, setNodeInspectorTab] = useState<'basic' | 'advanced'>('basic');
  const [edgeInspectorTab, setEdgeInspectorTab] = useState<'basic' | 'advanced'>('basic');
  const [openPositionApplyingKey, setOpenPositionApplyingKey] = useState<string | null>(null);
  const [nodeKeyDraft, setNodeKeyDraft] = useState('');
  const [nodeTypeDraft, setNodeTypeDraft] = useState('action.notify');
  const [nodeForm, setNodeForm] = useState<NodeConfigFormState | null>(null);
  const [hasPendingNodeDraft, setHasPendingNodeDraft] = useState(false);
  const [edgeTypeDraft, setEdgeTypeDraft] = useState('default');
  const [edgeForm, setEdgeForm] = useState<EdgeConditionFormState | null>(null);
  const [nodePaletteCategory, setNodePaletteCategory] = useState<NodePaletteCategory>('all');
  const [nodePaletteSearch, setNodePaletteSearch] = useState('');
  const [showNodeSearch, setShowNodeSearch] = useState(false);
  const [nodeSearchQuery, setNodeSearchQuery] = useState('');
  const nodeSearchInputRef = useRef<HTMLInputElement | null>(null);
  const [nodeGroups, setNodeGroups] = useState<NodeGroup[]>([]);
  const nextGroupColorIdx = useRef(0);
  const filteredNodeOptions = useMemo(() => {
    const search = nodePaletteSearch.trim().toLowerCase();
    return NODE_TYPE_OPTIONS.filter((option) => {
      if (nodePaletteCategory !== 'all' && nodePaletteCategoryOf(option.value) !== nodePaletteCategory)
        return false;
      if (!search) return true;
      return option.label.toLowerCase().includes(search) || option.value.toLowerCase().includes(search);
    });
  }, [nodePaletteCategory, nodePaletteSearch]);
  const executionMap = useMemo(() => {
    if (!executionStates || executionStates.length === 0) return null;
    const map = new Map<string, NodeExecutionState>();
    for (const s of executionStates) map.set(s.nodeKey, s);
    return map;
  }, [executionStates]);
  graphContextRef.current = graph.context;
  const setCanvasGraphState = useCallback((nextNodes: FlowNode[], nextEdges: FlowEdge[]) => {
    canvasNodesRef.current = nextNodes;
    canvasEdgesRef.current = nextEdges;
    setCanvasNodes(nextNodes);
    setCanvasEdges(nextEdges);
  }, []);
  const updateCanvasNodesState = useCallback(
    (updater: FlowNode[] | ((prev: FlowNode[]) => FlowNode[])) => {
      const nextNodes =
        typeof updater === 'function' ? updater(canvasNodesRef.current) : updater;
      canvasNodesRef.current = nextNodes;
      setCanvasNodes(nextNodes);
    },
    []
  );
  useEffect(() => {
    if (!executionMap) return;
    updateCanvasNodesState((prev) =>
      prev.map((n) => {
        const exec = executionMap.get(n.id);
        const nextStatus = exec?.status ?? 'idle';
        if (n.data.executionStatus === nextStatus) return n;
        return { ...n, data: { ...n.data, executionStatus: nextStatus } };
      })
    );
  }, [executionMap, updateCanvasNodesState]);
  // Collect unique market slugs from all trigger nodes for live price polling
  const allTriggerSlugs = useMemo(() => {
    const slugs = new Set<string>();
    for (const n of canvasNodes) {
      if (!n.data.nodeType.startsWith('trigger.')) continue;
      const slug = toTrimmedStringValue(n.data.config.marketSlug);
      if (slug) slugs.add(slug);
    }
    return Array.from(slugs);
  }, [canvasNodes]);
  const canvasLivePrices = useCanvasLivePrices(allTriggerSlugs);
  useEffect(() => {
    const prices = livePrices ?? (Object.keys(canvasLivePrices).length > 0 ? canvasLivePrices : null);
    if (!prices) return;
    updateCanvasNodesState((prev) =>
      prev.map((n) => {
        if (!n.data.nodeType.startsWith('trigger.')) return n;
        const tokenId = toTrimmedStringValue(n.data.config.tokenId);
        const price = tokenId ? (prices[tokenId] ?? null) : null;
        if (n.data.livePrice === price) return n;
        return { ...n, data: { ...n.data, livePrice: price } };
      })
    );
  }, [canvasLivePrices, livePrices, updateCanvasNodesState]);
  const searchMatchedNodes = useMemo(() => {
    if (!nodeSearchQuery.trim()) return canvasNodes;
    const q = nodeSearchQuery.trim().toLowerCase();
    return canvasNodes.filter(
      (n) => n.id.toLowerCase().includes(q) || n.data.nodeType.toLowerCase().includes(q)
    );
  }, [canvasNodes, nodeSearchQuery]);
  const commitGraph = useCallback(
    (
      nextNodes: FlowNode[],
      nextEdges: FlowEdge[],
      skipHistory = false,
      allowGraphShrink = false,
      persistImmediately = false
    ) => {
      const currentNodes = canvasNodesRef.current;
      const currentEdges = canvasEdgesRef.current;
      if (
        !allowGraphShrink &&
        (nextNodes.length < currentNodes.length || nextEdges.length < currentEdges.length)
      ) {
        return;
      }
      if (!allowGraphShrink && (nextNodes.length > currentNodes.length || nextEdges.length > currentEdges.length))
        graphSyncGuardRef.current = { until: Date.now() + 4000, nodes: nextNodes.length, edges: nextEdges.length };
      else if (allowGraphShrink) graphSyncGuardRef.current = null;
      if (!skipHistory) history.push(currentNodes, currentEdges);
      setCanvasGraphState(nextNodes, nextEdges);
      const domainNodes = nextNodes.map(toDomainNode), domainEdges = nextEdges.map(toDomainEdge);
      lastAppliedGraphFingerprintRef.current = createGraphFingerprint(domainNodes, domainEdges);
      const changeOptions: FlowCanvasGraphChangeOptions | undefined =
        allowGraphShrink || persistImmediately
          ? {
              ...(allowGraphShrink ? { allowGraphShrink: true } : {}),
              ...(persistImmediately ? { persistImmediately: true } : {}),
            }
          : undefined;
      onGraphChange(
        { context: graphContextRef.current, nodes: domainNodes, edges: domainEdges },
        changeOptions
      );
    },
    [history, onGraphChange, setCanvasGraphState]
  );
  const getViewportCenterPosition = useCallback(() => {
    const canvasRect = canvasWrapperRef.current?.getBoundingClientRect();
    if (!canvasRect) {
      const viewport = reactFlow.getViewport();
      const zoom = viewport.zoom || 1;
      return { x: Math.round((-viewport.x + 220) / zoom), y: Math.round((-viewport.y + 120) / zoom) };
    }
    const mapped = reactFlow.screenToFlowPosition({
      x: canvasRect.left + canvasRect.width * 0.5,
      y: canvasRect.top + canvasRect.height * 0.5,
    });
    return { x: Math.round(mapped.x), y: Math.round(mapped.y) };
  }, [reactFlow]);
  const {
    clearSelection,
    deleteSelectedEdge,
    deleteSelectedNode,
    deleteSelection,
    handleCopy,
    handlePaste,
    hasActiveSelection,
    hydrateNodeDraft,
    inspectedNodeId,
    isMultiSelection,
    onSelectionChange,
    selectedEdge,
    selectedEdgeCount,
    selectedEdgeIds,
    selectedNode,
    selectedNodeCount,
    selectedNodeIds,
  } = useCanvasSelection({
    canvasEdges,
    canvasEdgesRef,
    canvasNodes,
    canvasNodesRef,
    commitGraph,
    getPasteAnchor: () => lastCanvasPointerRef.current ?? getViewportCenterPosition(),
    onError,
    queueNodeFocus: (nodeId: string) => setPendingFocusNodeId(nodeId),
    setCanvasGraphState,
    setEdgeForm,
    setEdgeInspectorTab,
    setEdgeTypeDraft,
    setHasPendingNodeDraft,
    setNodeForm,
    setNodeInspectorTab,
    setNodeKeyDraft,
    setNodeTypeDraft,
  });
  const {
    upstreamAutoScope: selectedNodeUpstreamAutoScope,
    upstreamTriggerPrice: selectedNodeUpstreamTriggerPrice,
    upstreamMaxPriceResolution: selectedNodeUpstreamMaxPriceResolution,
    upstreamFixedMarketResolution: selectedNodeUpstreamFixedMarketResolution,
    upstreamPairLockTrigger: selectedNodeUpstreamPairLockTrigger,
  } = useSelectedNodeUpstream({ selectedNodeId: inspectedNodeId, canvasNodes, canvasEdges });
  useEffect(() => {
    if (graphFingerprint === lastSeenPropGraphFingerprintRef.current) return;
    lastSeenPropGraphFingerprintRef.current = graphFingerprint;
    const graphSyncGuard = graphSyncGuardRef.current;
    if (graphSyncGuard && graphSyncGuard.until > Date.now() && (graph.nodes.length < graphSyncGuard.nodes || graph.edges.length < graphSyncGuard.edges)) return;
    if (graphSyncGuard && graphSyncGuard.until <= Date.now()) graphSyncGuardRef.current = null;
    if (graphFingerprint === lastAppliedGraphFingerprintRef.current) return;
    lastAppliedGraphFingerprintRef.current = graphFingerprint;
    setCanvasGraphState(graph.nodes.map(toCanvasNode), graph.edges.map(toCanvasEdge));
  }, [graph.edges, graph.nodes, graphFingerprint, setCanvasGraphState]);
  useEffect(() => {
    const nodeIdSet = new Set(canvasNodes.map((node) => node.id));
    const edgeIdSet = new Set(canvasEdges.map((edge) => edge.id));
    const hasInvalidSelection =
      selectedNodeIds.some((nodeId) => !nodeIdSet.has(nodeId)) ||
      selectedEdgeIds.some((edgeId) => !edgeIdSet.has(edgeId));
    if (hasInvalidSelection) clearSelection();
  }, [canvasEdges, canvasNodes, clearSelection, selectedEdgeIds, selectedNodeIds]);
  useEffect(() => {
    if (!pendingFocusNodeId) return;
    const timeout = window.setTimeout(() => {
      setPendingFocusNodeId((cur) => {
        if (cur && cur === pendingFocusNodeId) onError(`Node eklendi ancak canvas'ta gosterilemedi: ${cur}`);
        return null;
      });
    }, 1200);
    return () => window.clearTimeout(timeout);
  }, [onError, pendingFocusNodeId]);
  useEffect(() => {
    if (!pendingFocusNodeId || !nodesInitialized) return;
    const targetNode = canvasNodes.find((n) => n.id === pendingFocusNodeId);
    if (!targetNode) return;
    const currentZoom = reactFlow.getZoom();
    const nextZoom = Number.isFinite(currentZoom) ? Math.min(1.15, Math.max(0.8, currentZoom)) : 1;
    void reactFlow.setCenter(targetNode.position.x + 110, targetNode.position.y + 36, {
      zoom: nextZoom,
      duration: 220,
    });
    setPendingFocusNodeId(null);
  }, [canvasNodes, nodesInitialized, pendingFocusNodeId, reactFlow]);
  const queueNodeFocus = useCallback((nodeId: string) => setPendingFocusNodeId(nodeId), []);
  const focusEditor = useCallback(() => editorRootRef.current?.focus(), []);
  const ensureWritable = useCallback(() => {
    if (!readOnly) return true;
    onError(readOnlyReason ?? 'Flow yuklenirken duzenleme kilitli.');
    return false;
  }, [onError, readOnly, readOnlyReason]);
  const getInsertPosition = useCallback(() => {
    if (selectedNode) return { x: selectedNode.position.x + 260, y: selectedNode.position.y + 20 };
    return getViewportCenterPosition();
  }, [getViewportCenterPosition, selectedNode]);
  const updateCanvasPointer = useCallback(
    (clientX: number, clientY: number) => {
      const position = reactFlow.screenToFlowPosition({ x: clientX, y: clientY });
      lastCanvasPointerRef.current = { x: Math.round(position.x), y: Math.round(position.y) };
    },
    [reactFlow]
  );
  const handleNodesChange = useCallback(
    (changes: NodeChange<FlowNode>[]) => {
      if (!ensureWritable()) return;
      const currentNodes = canvasNodesRef.current;
      const currentEdges = canvasEdgesRef.current;
      const nextNodes = applyNodeChanges(changes, currentNodes);
      const nodeIdSet = new Set(nextNodes.map((n) => n.id));
      const nextEdges = currentEdges.filter(
        (e) => nodeIdSet.has(e.source) && nodeIdSet.has(e.target)
      );
      const hasRemoval = changes.some((c) => c.type === 'remove');
      const droppedExistingNodeWithoutRemoval = !hasRemoval &&
        currentNodes.some((node) => !nodeIdSet.has(node.id));
      if (droppedExistingNodeWithoutRemoval) return;
      const nextEdgeIdSet = new Set(nextEdges.map((edge) => edge.id));
      const selectionInvalidated =
        selectedNodeIds.some((nodeId) => !nodeIdSet.has(nodeId)) ||
        selectedEdgeIds.some((edgeId) => !nextEdgeIdSet.has(edgeId));
      if (selectionInvalidated) clearSelection();
      commitGraph(nextNodes, nextEdges, !hasRemoval, hasRemoval);
    },
    [clearSelection, commitGraph, ensureWritable, selectedEdgeIds, selectedNodeIds]
  );
  const handleEdgesChange = useCallback(
    (changes: EdgeChange<FlowEdge>[]) => {
      if (!ensureWritable()) return;
      const currentEdges = canvasEdgesRef.current;
      const nextEdges = applyEdgeChanges(changes, currentEdges);
      const hasRemoval = changes.some((c) => c.type === 'remove');
      const droppedExistingEdgeWithoutRemoval = !hasRemoval &&
        currentEdges.some((edge) => !nextEdges.some((nextEdge) => nextEdge.id === edge.id));
      if (droppedExistingEdgeWithoutRemoval) return;
      const nextEdgeIdSet = new Set(nextEdges.map((edge) => edge.id));
      if (selectedEdgeIds.some((edgeId) => !nextEdgeIdSet.has(edgeId))) clearSelection();
      commitGraph(canvasNodesRef.current, nextEdges, !hasRemoval, hasRemoval);
    },
    [clearSelection, commitGraph, ensureWritable, selectedEdgeIds]
  );
  const handleConnect = useCallback(
    (connection: Connection) => {
      if (!ensureWritable()) return;
      if (!connection.source || !connection.target) return;
      const edgeId = createEdgeKey(new Set(canvasEdges.map((e) => e.id)));
      const next = addEdge<FlowEdge>(
        {
          id: edgeId,
          source: connection.source,
          target: connection.target,
          type: 'smoothstep',
          markerEnd: { type: MarkerType.ArrowClosed, color: EDGE_STROKE_COLOR, width: 16, height: 16 },
          label: 'default',
          data: { edgeType: 'default', condition: null },
          style: { stroke: EDGE_STROKE_COLOR, strokeWidth: 1.6 },
          labelStyle: { fill: '#334155', fontSize: 10 },
          labelBgStyle: { fill: '#e2e8f0', fillOpacity: 1 },
          labelBgBorderRadius: 6,
        },
        canvasEdges
      );
      commitGraph(canvasNodes, next);
      onError(null);
    },
    [canvasEdges, canvasNodes, commitGraph, ensureWritable, onError]
  );
  const addNode = (nodeType: string) => {
    if (!ensureWritable()) return;
    const nodeId = createNodeKey(nodeType, new Set(canvasNodes.map((n) => n.id)));
    const nextNode: FlowNode = {
      id: nodeId, type: 'flowNode', position: getInsertPosition(),
      data: { nodeType, config: {} },
    };
    commitGraph([...canvasNodes, nextNode], canvasEdges, false, false, true);
    hydrateNodeDraft(nextNode, true);
    queueNodeFocus(nextNode.id);
    onError(null);
  };
  const addPresetPlaceOrderNode = (kind: PlaceOrderPresetKind) => {
    if (!ensureWritable()) return;
    const fromSel = selectedNode ? extractPositionSeedFromNode(selectedNode) : null;
    const fromCtx: PlaceOrderPresetSeed = {
      sourceTradeId: toFiniteNumberValue(graph.context.sourceTradeId),
      marketSlug: toTrimmedStringValue(graph.context.marketSlug),
      tokenId: toTrimmedStringValue(graph.context.tokenId),
      outcomeLabel: toTrimmedStringValue(graph.context.outcomeLabel),
    };
    const seed: PlaceOrderPresetSeed = {
      sourceTradeId: fromSel?.sourceTradeId ?? fromCtx.sourceTradeId,
      marketSlug: fromSel?.marketSlug || fromCtx.marketSlug,
      tokenId: fromSel?.tokenId || fromCtx.tokenId,
      outcomeLabel: fromSel?.outcomeLabel || fromCtx.outcomeLabel,
    };
    const nodeId = createNodeKey('action.place_order', new Set(canvasNodes.map((n) => n.id)));
    const nextNode: FlowNode = {
      id: nodeId, type: 'flowNode', position: getInsertPosition(),
      data: { nodeType: 'action.place_order', config: buildPlaceOrderPresetConfig(kind, seed) },
    };
    commitGraph([...canvasNodes, nextNode], canvasEdges, false, false, true);
    hydrateNodeDraft(nextNode, true);
    queueNodeFocus(nextNode.id);
    if (!hasRequiredPlaceOrderSeed(seed)) {
      onError(kind === 'sell_current_position'
        ? 'Mevcut pozisyon kaynak bilgisi eksik. Node eklendi; sag paneldeki acik pozisyon listesinden secim yapabilirsin.'
        : 'Alis preset node eklendi. Eksik alanlar icin sag paneldeki acik pozisyon listesinden secim yapabilirsin.');
      return;
    }
    onError(null);
  };
  const updateNodeField = (key: string, value: string) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => updateNodeFieldState(prev, nodeTypeDraft, key, value));
  };
  const updateTriggerSizeRow = (index: number, value: string) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => updateTriggerSizeRowState(prev, index, value));
  };
  useSyncPlaceOrderFormState({
    nodeTypeDraft,
    selectedNodeId: inspectedNodeId,
    placeOrderMaxTriggers: nodeForm?.fields.maxTriggers,
    upstreamFixedMarketResolution: selectedNodeUpstreamFixedMarketResolution,
    upstreamMaxPriceResolution: selectedNodeUpstreamMaxPriceResolution,
    setNodeForm,
  });
  const canApplyOpenPosition = (p: TradeFlowOpenPositionOption) =>
    p.matchedTradeId != null ? true : Boolean(p.marketSlug.trim() && p.tokenId.trim());
  const applyOpenPositionSelection = async (position: TradeFlowOpenPositionOption) => {
    if (!ensureWritable()) return;
    if (!canApplyOpenPosition(position)) {
      onError('Bu pozisyon icin marketSlug/tokenId eksik, sourceTradeId atanamadi.');
      return;
    }
    setOpenPositionApplyingKey(position.positionKey);
    onError(null);
    try {
      let sourceTradeId = position.matchedTradeId;
      if (sourceTradeId == null) {
        const ensured = await ensureTradeFlowSourceTrade({
          marketSlug: position.marketSlug, tokenId: position.tokenId,
          outcomeLabel: position.outcomeLabel, marketTitle: position.marketTitle,
          size: position.size, avgPrice: position.avgPrice, currentValue: position.currentValue,
        });
        sourceTradeId = ensured.data.sourceTradeId;
      }
      const nid = typeof sourceTradeId === 'number' ? sourceTradeId : Number.NaN;
      if (!Number.isFinite(nid) || nid <= 0) throw new Error('sourceTradeId uretilemedi.');
      setNodeForm((prev) => prev ? {
        ...prev,
        fields: { ...prev.fields, sourceTradeId: String(nid), marketSlug: position.marketSlug,
          tokenId: position.tokenId, outcomeLabel: position.outcomeLabel },
      } : prev);
      setHasPendingNodeDraft(true);
      if (
        selectedNode &&
        (selectedNode.data.nodeType === 'trigger.open_positions' ||
          selectedNode.data.nodeType === 'action.place_order')
      ) {
        const nextNodes = canvasNodes.map((n) => n.id !== selectedNode.id ? n : {
          ...n, data: { ...n.data, config: { ...n.data.config, sourceTradeId: nid,
            marketSlug: position.marketSlug, tokenId: position.tokenId, outcomeLabel: position.outcomeLabel } },
        });
        commitGraph(nextNodes, canvasEdges, false, false, true);
        setHasPendingNodeDraft(false);
      }
      onApplyContextPatch({ sourceTradeId: nid, marketSlug: position.marketSlug,
        tokenId: position.tokenId, outcomeLabel: position.outcomeLabel });
      onError(null);
    } catch (err) {
      onError(err instanceof Error ? err.message : 'Pozisyon secimi uygulanamadi.');
    } finally {
      setOpenPositionApplyingKey(null);
    }
  };
  const updateExpressionRow = (rowId: string, patch: Partial<ConditionDraft>) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => updateExpressionRowState(prev, rowId, patch));
  };
  const addExpressionRow = () => {
    setHasPendingNodeDraft(true);
    setNodeForm(addExpressionRowState);
  };
  const removeExpressionRow = (rowId: string) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => removeExpressionRowState(prev, rowId));
  };
  const updateStatePatchRow = (rowId: string, patch: Partial<{ key: string; value: string; valueType: PrimitiveValueType }>) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => updateStatePatchRowState(prev, rowId, patch));
  };
  const addStatePatchRow = () => {
    setHasPendingNodeDraft(true);
    setNodeForm(addStatePatchRowState);
  };
  const removeStatePatchRow = (rowId: string) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => removeStatePatchRowState(prev, rowId));
  };
  const { marketOutcomes, marketOutcomeTokenIdSet, outcomesLoading } = useMarketOutcomes({
    nodeTypeDraft,
    nodeForm,
    upstreamPairLockTrigger: selectedNodeUpstreamPairLockTrigger,
  });
  const addOutcomeCondition = (tokenId: string, outcomeLabel: string) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => addOutcomeConditionState(prev, tokenId, outcomeLabel));
  };
  const removeOutcomeCondition = (rowId: string) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => removeOutcomeConditionState(prev, rowId));
  };
  const updateOutcomeCondition = (rowId: string, patch: Partial<OutcomeConditionRow>) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => updateOutcomeConditionState(prev, rowId, patch));
  };
  const addDrawdownRule = () => {
    setHasPendingNodeDraft(true);
    setNodeForm(addDrawdownRuleState);
  };
  const removeDrawdownRule = (rowId: string) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => removeDrawdownRuleState(prev, rowId));
  };
  const updateDrawdownRule = (rowId: string, patch: Partial<DrawdownRuleRow>) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => updateDrawdownRuleState(prev, rowId, patch));
  };
  const handleNodeTypeChange = (nextType: string) => {
    const previousType = nodeTypeDraft;
    setNodeTypeDraft(nextType);
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => {
      if (!prev) return parseNodeConfigToForm(nextType, {});
      return parseNodeConfigToForm(nextType, buildNodeConfigFromForm(previousType, prev));
    });
  };
  const parseAdvancedConfig = (): Record<string, unknown> | null => {
    if (!nodeForm) return null;
    try {
      const parsed = JSON.parse(nodeForm.advancedJson) as unknown;
      if (!isRecord(parsed)) throw new Error('Config JSON nesne olmali.');
      return parsed;
    } catch (err) {
      onError(err instanceof Error ? `Node config JSON hatali: ${err.message}` : 'Node config JSON hatali.');
      return null;
    }
  };
  const createOrUpdateNode = (mode: 'create' | 'update', source: 'basic' | 'advanced') => {
    if (!ensureWritable()) return;
    const nextKey = nodeKeyDraft.trim();
    const nextType = nodeTypeDraft.trim();
    if (!nextKey || !nextType) { onError('Node key ve type bos olamaz.'); return; }
    const isAutoScope = nodeForm?.fields.marketMode === 'auto_scope';
    const outcomeRequired =
      (
        nextType === 'trigger.open_positions' ||
        nextType === 'trigger.market_price' ||
        nextType === 'trigger.position_drawdown'
      ) && !isAutoScope;
    if (outcomeRequired) {
      if (outcomesLoading) {
        onError('Outcome listesi yukleniyor. Lutfen birkac saniye sonra tekrar dene.');
        return;
      }
      if (marketOutcomes.length === 0) {
        onError('Outcome secimi zorunlu. Bu market icin outcome listesi bulunamadi.');
        return;
      }
    }

    let parsedConfig: Record<string, unknown>;
    if (source === 'advanced') {
      const adv = parseAdvancedConfig();
      if (!adv) return;
      parsedConfig = adv;
    } else {
      if (!nodeForm) return;
      const nodeFormError = validateNodeFormBeforeSave(nextType, nodeForm);
      if (nodeFormError) {
        onError(nodeFormError);
        return;
      }
      parsedConfig = buildNodeConfigFromForm(nextType, nodeForm);
    }
    applyInheritedPlaceOrderMaxPriceConfig(
      nextType,
      parsedConfig,
      selectedNodeUpstreamMaxPriceResolution
    );
    if (nextType === 'action.place_order' && isPresetPlaceOrderMarker(parsedConfig.presetKind, parsedConfig.refKey)) {
      normalizePresetPlaceOrderConfig(parsedConfig);
      const placeOrderSide = toTrimmedStringValue(parsedConfig.side).toLowerCase();
      if (
        selectedNodeUpstreamAutoScope &&
        isPresetPlaceOrderMarker(parsedConfig.presetKind, parsedConfig.refKey) &&
        placeOrderSide === 'buy'
      ) {
        delete parsedConfig.marketSlug;
        delete parsedConfig.tokenId;
        delete parsedConfig.outcomeLabel;
      }
    }
    if (nextType === 'action.telegram_notify') {
      delete parsedConfig.botToken;
    }
    if (outcomeRequired) {
      const ptbTriggerEnabled =
        nextType === 'trigger.market_price' && parsedConfig.priceToBeatTriggerEnabled === true;
      const outcomeConditions = Array.isArray(parsedConfig.outcomeConditions)
        ? parsedConfig.outcomeConditions.filter((item): item is Record<string, unknown> => isRecord(item))
        : [];
      const validatedOutcomeConditions = outcomeConditions.map((item) => ({
        item,
        validation: validateOutcomeConditionRow({
          nodeType: nextType,
          tokenId: item.tokenId,
          outcomeLabel: item.outcomeLabel,
          triggerCondition: item.triggerCondition,
          triggerPriceCent: item.triggerPriceCent,
          maxPriceCent: item.maxPriceCent,
          priceToBeatTriggerEnabled: ptbTriggerEnabled,
        }),
      }));
      const hasLevelTriggerInLoopMode =
        nextType === 'trigger.market_price' &&
        toTrimmedStringValue(parsedConfig.repeatMode).toLowerCase() !== 'once' &&
        validatedOutcomeConditions.some(({ validation }) => validation.requiresOnceRepeatMode);
      if (hasLevelTriggerInLoopMode) {
        onError('trigger.market_price level_above/level_below only support repeatMode=once.');
        return;
      }
      const validOutcomeConditions = validatedOutcomeConditions
        .filter(({ validation }) => validation.isValid)
        .map(({ item }) => item);

      if (validOutcomeConditions.length === 0) {
        onError('En az bir gecerli outcome kosulu secmelisin.');
        return;
      }

      const hasUnknownOutcome = validOutcomeConditions.some(
        (item) => !marketOutcomeTokenIdSet.has(toTrimmedStringValue(item.tokenId))
      );
      if (hasUnknownOutcome) {
        onError('Outcome secimi sadece marketten gelen outcome listesinden yapilabilir.');
        return;
      }
    }
    if (nextType === 'trigger.position_drawdown') {
      if (source === 'basic' && nodeForm) {
        try {
          const advancedParsed = JSON.parse(nodeForm.advancedJson) as unknown;
          if (isRecord(advancedParsed)) {
            const legacyAtRoot = Object.prototype.hasOwnProperty.call(advancedParsed, 'windowSec');
            const legacyInRules =
              Array.isArray(advancedParsed.lossRules) &&
              advancedParsed.lossRules.some(
                (item) => isRecord(item) && Object.prototype.hasOwnProperty.call(item, 'windowSec')
              );
            if (legacyAtRoot || legacyInRules) {
              onError(
                'Bu node eski windowSec kullaniyor. Advanced JSON ekraninda windowSec alanlarini kaldirip windowMs kullan.'
              );
              return;
            }
          }
        } catch {
          // advancedJson parse edilmezse standart save validasyonu asagida devam eder.
        }
      }
      const selectedOutcomeLabel = toTrimmedStringValue(parsedConfig.outcomeLabel);
      const selectedTokenId = toTrimmedStringValue(parsedConfig.tokenId);
      if (!selectedOutcomeLabel || !selectedTokenId) {
        onError('Drawdown node icin marketten bir outcome secmelisin.');
        return;
      }
      if (!marketOutcomeTokenIdSet.has(selectedTokenId)) {
        onError('Secilen outcome bu market listesinde bulunmuyor. Outcome secimini yenile.');
        return;
      }
      const entryPriceCent = toFiniteNumberValue(parsedConfig.entryPriceCent);
      if (entryPriceCent == null || entryPriceCent <= 0 || entryPriceCent > 100) {
        onError('Entry fiyati zorunlu. 0-100 arasinda cent degeri gir.');
        return;
      }
      const drawdownRules = Array.isArray(parsedConfig.lossRules)
        ? parsedConfig.lossRules.filter((item): item is Record<string, unknown> => isRecord(item))
        : [];
      const hasInvalidDirection = drawdownRules.some((item) => {
        const direction = toTrimmedStringValue(item.direction).toLowerCase();
        return direction !== '' && direction !== 'down' && direction !== 'up';
      });
      if (hasInvalidDirection) {
        onError('Drawdown kural yonu sadece down veya up olabilir.');
        return;
      }
      const hasDeprecatedWindowSec =
        drawdownRules.some((item) => Object.prototype.hasOwnProperty.call(item, 'windowSec')) ||
        Object.prototype.hasOwnProperty.call(parsedConfig, 'windowSec');
      if (hasDeprecatedWindowSec) {
        onError('windowSec artik desteklenmiyor. Lutfen windowMs kullan.');
        return;
      }
    }

    if (mode === 'create') {
      if (canvasNodes.some((n) => n.id === nextKey)) { onError(`Ayni key ile baska node var: ${nextKey}`); return; }
      const nextNode: FlowNode = { id: nextKey, type: 'flowNode', position: getInsertPosition(), data: { nodeType: nextType, config: parsedConfig } };
      commitGraph([...canvasNodes, nextNode], canvasEdges, false, false, true);
      hydrateNodeDraft(nextNode, true);
      queueNodeFocus(nextNode.id);
      setHasPendingNodeDraft(false);
      toast.success('Node eklendi');
    } else {
      if (!selectedNode) { onError('Guncellemek icin once bir node secin.'); return; }
      if (nextKey !== selectedNode.id && canvasNodes.some((n) => n.id === nextKey)) { onError(`Ayni key ile baska node var: ${nextKey}`); return; }
      const nextNodes = canvasNodes.map((n) => n.id !== selectedNode.id ? n : { ...n, id: nextKey, data: { ...n.data, nodeType: nextType, config: parsedConfig } });
      const nextEdges = canvasEdges.map((e) => ({ ...e, source: e.source === selectedNode.id ? nextKey : e.source, target: e.target === selectedNode.id ? nextKey : e.target }));
      const updatedNode = nextNodes.find((node) => node.id === nextKey);
      commitGraph(nextNodes, nextEdges, false, true, true);
      if (updatedNode) hydrateNodeDraft(updatedNode, true);
      setHasPendingNodeDraft(false);
      toast.success('Node guncellendi');
    }
    onError(null);
  };
  useEffect(() => {
    onPendingNodeDraftChange?.(hasPendingNodeDraft);
  }, [hasPendingNodeDraft, onPendingNodeDraftChange]);
  const updateEdgeConditionRow = (patch: Partial<ConditionDraft>) => {
    setEdgeForm((prev) => prev ? { ...prev, conditionRow: { ...prev.conditionRow, ...patch } } : prev);
  };
  const applyEdge = (source: 'basic' | 'advanced') => {
    if (!ensureWritable()) return;
    if (!selectedEdge || !edgeForm) return;
    const nextEdgeType = edgeTypeDraft.trim() || 'default';
    let nextCondition: Record<string, unknown> | null = null;

    if (source === 'basic') {
      nextCondition = buildEdgeConditionFromForm(edgeForm);
    } else {
      if (edgeForm.advancedJson.trim()) {
        try {
          const parsed = JSON.parse(edgeForm.advancedJson) as unknown;
          if (!isRecord(parsed)) throw new Error('Condition JSON nesne olmali.');
          nextCondition = parsed;
        } catch (err) {
          onError(err instanceof Error ? `Edge condition JSON hatali: ${err.message}` : 'Edge condition JSON hatali.');
          return;
        }
      }
    }

    const nextEdges = canvasEdges.map((e) => e.id !== selectedEdge.id ? e : {
      ...e, label: nextEdgeType, data: { ...e.data, edgeType: nextEdgeType, condition: nextCondition },
    });
    commitGraph(canvasNodes, nextEdges);
    setEdgeForm(parseEdgeConditionToForm(nextCondition));
    onError(null);
  };
  // Undo/Redo
  const handleUndo = useCallback(() => {
    if (!ensureWritable()) return;
    const snapshot = history.undo(canvasNodesRef.current, canvasEdgesRef.current);
    if (!snapshot) return;
    setCanvasGraphState(snapshot.nodes, snapshot.edges);
    const dn = snapshot.nodes.map(toDomainNode);
    const de = snapshot.edges.map(toDomainEdge);
    lastAppliedGraphFingerprintRef.current = createGraphFingerprint(dn, de);
    onGraphChange(
      { context: graphContextRef.current, nodes: dn, edges: de },
      { allowGraphShrink: true }
    );
    clearSelection(true);
  }, [clearSelection, ensureWritable, history, onGraphChange, setCanvasGraphState]);
  const handleRedo = useCallback(() => {
    if (!ensureWritable()) return;
    const snapshot = history.redo(canvasNodesRef.current, canvasEdgesRef.current);
    if (!snapshot) return;
    setCanvasGraphState(snapshot.nodes, snapshot.edges);
    const dn = snapshot.nodes.map(toDomainNode);
    const de = snapshot.edges.map(toDomainEdge);
    lastAppliedGraphFingerprintRef.current = createGraphFingerprint(dn, de);
    onGraphChange(
      { context: graphContextRef.current, nodes: dn, edges: de },
      { allowGraphShrink: true }
    );
    clearSelection(true);
  }, [clearSelection, ensureWritable, history, onGraphChange, setCanvasGraphState]);
  // Auto-Layout
  const handleAutoLayout = useCallback(() => {
    if (!ensureWritable()) return;
    const laid = autoLayoutNodes(canvasNodes, canvasEdges);
    commitGraph(laid, canvasEdges);
  }, [canvasEdges, canvasNodes, commitGraph, ensureWritable]);
  // Import/Export
  const handleExport = useCallback(() => {
    exportGraphAsJson({ context: graph.context, nodes: canvasNodes.map(toDomainNode), edges: canvasEdges.map(toDomainEdge) });
  }, [canvasEdges, canvasNodes, graph.context]);
  const handleImport = useCallback(async () => {
    if (!ensureWritable()) return;
    try {
      const imported = await importGraphFromFile();
      const nextNodes = imported.nodes.map(toCanvasNode);
      const nextEdges = imported.edges.map(toCanvasEdge);
      commitGraph(nextNodes, nextEdges);
      onError(null);
    } catch (err) {
      onError(err instanceof Error ? err.message : 'JSON yukleme hatasi.');
    }
  }, [commitGraph, ensureWritable, onError]);
  // Group / Ungroup
  const handleGroupSelected = useCallback(() => {
    if (!ensureWritable()) return;
    if (!selectedNode) return;
    const colorSet = GROUP_COLORS[nextGroupColorIdx.current % GROUP_COLORS.length];
    nextGroupColorIdx.current += 1;
    const groupId = `grp_${Math.random().toString(36).slice(2, 8)}`;
    const groupName = `Grup ${nodeGroups.length + 1}`;
    setNodeGroups((prev) => [...prev, { id: groupId, name: groupName, color: colorSet.border }]);
    const nextNodes = canvasNodes.map((n) =>
      n.id === selectedNode.id ? { ...n, data: { ...n.data, groupId, groupColor: colorSet.border } } : n
    );
    commitGraph(nextNodes, canvasEdges);
  }, [canvasEdges, canvasNodes, commitGraph, ensureWritable, nodeGroups.length, selectedNode]);
  const handleAssignToGroup = useCallback((groupId: string) => {
    if (!ensureWritable()) return;
    if (!selectedNode) return;
    const group = nodeGroups.find((g) => g.id === groupId);
    if (!group) return;
    const nextNodes = canvasNodes.map((n) =>
      n.id === selectedNode.id ? { ...n, data: { ...n.data, groupId, groupColor: group.color } } : n
    );
    commitGraph(nextNodes, canvasEdges);
  }, [canvasEdges, canvasNodes, commitGraph, ensureWritable, nodeGroups, selectedNode]);
  const handleUngroupSelected = useCallback(() => {
    if (!ensureWritable()) return;
    if (!selectedNode) return;
    const nextNodes = canvasNodes.map((n) =>
      n.id === selectedNode.id ? { ...n, data: { ...n.data, groupId: undefined, groupColor: undefined } } : n
    );
    commitGraph(nextNodes, canvasEdges);
  }, [canvasEdges, canvasNodes, commitGraph, ensureWritable, selectedNode]);
  const guardedDeleteSelection = useCallback(() => {
    if (!ensureWritable()) return;
    deleteSelection();
  }, [deleteSelection, ensureWritable]);
  const guardedHandlePaste = useCallback(() => {
    if (!ensureWritable()) return;
    handlePaste();
  }, [ensureWritable, handlePaste]);
  const guardedDeleteSelectedNode = useCallback(() => {
    if (!ensureWritable()) return;
    deleteSelectedNode();
  }, [deleteSelectedNode, ensureWritable]);
  const guardedDeleteSelectedEdge = useCallback(() => {
    if (!ensureWritable()) return;
    deleteSelectedEdge();
  }, [deleteSelectedEdge, ensureWritable]);
  // Keyboard
  useCanvasKeyboard({
    onSave: () => {},
    onUndo: handleUndo,
    onRedo: handleRedo,
    onCopy: handleCopy,
    onPaste: guardedHandlePaste,
    onSelectAll: () => {},
    onDeselect: () => clearSelection(true),
    onDelete: guardedDeleteSelection,
    onSearch: () => { setShowNodeSearch(true); setTimeout(() => nodeSearchInputRef.current?.focus(), 50); },
  }, editorRootRef);
  // Node search overlay focus
  useEffect(() => {
    if (showNodeSearch) nodeSearchInputRef.current?.focus();
  }, [showNodeSearch]);
  const triggerCount = canvasNodes.filter((n) => n.data.nodeType.startsWith('trigger.')).length;
  const logicCount = canvasNodes.filter((n) => n.data.nodeType.startsWith('logic.')).length;
  const actionCount = canvasNodes.filter((n) => n.data.nodeType.startsWith('action.')).length;
  const nodeInspectorActions = createNodeInspectorActions({
    setNodeKeyDraft,
    setHasPendingNodeDraft,
    handleNodeTypeChange,
    setNodeInspectorTab,
    setNodeForm,
    updateNodeField,
    updateTriggerSizeRow,
    createOrUpdateNode,
    deleteSelectedNode: guardedDeleteSelectedNode,
    applyOpenPositionSelection,
    updateExpressionRow,
    addExpressionRow,
    removeExpressionRow,
    updateStatePatchRow,
    addStatePatchRow,
    removeStatePatchRow,
    addOutcomeCondition,
    removeOutcomeCondition,
    updateOutcomeCondition,
    addDrawdownRule,
    removeDrawdownRule,
    updateDrawdownRule,
  });
  const edgeInspectorActions = createEdgeInspectorActions({
    setEdgeTypeDraft,
    setEdgeInspectorTab,
    setEdgeForm,
    updateEdgeConditionRow,
    applyEdge,
    deleteSelectedEdge: guardedDeleteSelectedEdge,
  });
  return (
    <FlowCanvasEditorLayout
      readOnly={readOnly}
      readOnlyReason={readOnlyReason}
      editorRootRef={editorRootRef}
      canvasWrapperRef={canvasWrapperRef}
      focusEditor={focusEditor}
      onCanvasPointerMove={updateCanvasPointer}
      leftPanelTopSlot={leftPanelTopSlot}
      showNodeSearch={showNodeSearch}
      setShowNodeSearch={setShowNodeSearch}
      nodeSearchQuery={nodeSearchQuery}
      setNodeSearchQuery={setNodeSearchQuery}
      nodeSearchInputRef={nodeSearchInputRef}
      searchMatchedNodes={searchMatchedNodes}
      hydrateNodeDraft={hydrateNodeDraft}
      queueNodeFocus={queueNodeFocus}
      nodePaletteSearch={nodePaletteSearch}
      setNodePaletteSearch={setNodePaletteSearch}
      nodePaletteCategory={nodePaletteCategory}
      setNodePaletteCategory={setNodePaletteCategory}
      filteredNodeOptions={filteredNodeOptions}
      addNode={addNode}
      addPresetPlaceOrderNode={addPresetPlaceOrderNode}
      canvasNodes={canvasNodes}
      canvasEdges={canvasEdges}
      triggerCount={triggerCount}
      logicCount={logicCount}
      actionCount={actionCount}
      selectedNode={selectedNode}
      selectedEdge={selectedEdge}
      selectedNodeCount={selectedNodeCount}
      selectedEdgeCount={selectedEdgeCount}
      hasActiveSelection={hasActiveSelection}
      isMultiSelection={isMultiSelection}
      deleteSelection={guardedDeleteSelection}
      handleGroupSelected={handleGroupSelected}
      handleUngroupSelected={handleUngroupSelected}
      nodeGroups={nodeGroups}
      handleAssignToGroup={handleAssignToGroup}
      handleUndo={handleUndo}
      handleRedo={handleRedo}
      canUndo={history.canUndo()}
      canRedo={history.canRedo()}
      handleAutoLayout={handleAutoLayout}
      handleExport={handleExport}
      handleImport={handleImport}
      onNodesChange={handleNodesChange}
      onEdgesChange={handleEdgesChange}
      onConnect={handleConnect}
      onSelectionChange={onSelectionChange}
      nodeForm={nodeForm}
      nodeKeyDraft={nodeKeyDraft}
      nodeTypeDraft={nodeTypeDraft}
      nodeInspectorTab={nodeInspectorTab}
      openPositions={openPositions}
      openPositionsMeta={openPositionsMeta}
      openPositionsLoading={openPositionsLoading}
      openPositionApplyingKey={openPositionApplyingKey}
      canApplyOpenPosition={canApplyOpenPosition}
      marketOutcomes={marketOutcomes}
      outcomesLoading={outcomesLoading}
      selectedNodeUpstreamAutoScope={selectedNodeUpstreamAutoScope}
      selectedNodeUpstreamTriggerPrice={selectedNodeUpstreamTriggerPrice}
      selectedNodeUpstreamMaxPriceResolution={selectedNodeUpstreamMaxPriceResolution}
      selectedNodeUpstreamPairLockTrigger={selectedNodeUpstreamPairLockTrigger}
      userTelegramBotTokenMasked={userTelegramBotTokenMasked ?? null}
      userTelegramDefaultChatId={userTelegramDefaultChatId ?? null}
      nodeInspectorActions={nodeInspectorActions}
      edgeForm={edgeForm}
      edgeTypeDraft={edgeTypeDraft}
      edgeInspectorTab={edgeInspectorTab}
      edgeInspectorActions={edgeInspectorActions}
    />
  );
}
