'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Background,
  Controls,
  MarkerType,
  MiniMap,
  ReactFlow,
  ReactFlowProvider,
  addEdge,
  applyEdgeChanges,
  applyNodeChanges,
  useNodesInitialized,
  useReactFlow,
  type Connection,
  type EdgeChange,
  type NodeChange,
} from '@xyflow/react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { ensureTradeFlowSourceTrade } from '@/hooks/use-trade-flow';
import { useCanvasHistory } from '@/hooks/use-canvas-history';
import type { NodeExecutionState, TradeFlowOpenPositionOption } from '@/lib/types';
import {
  buildEdgeConditionFromForm,
  buildNodeConfigFromForm,
  createEmptyConditionDraft,
  createEmptyKeyValueDraft,
  parseEdgeConditionToForm,
  parseNodeConfigToForm,
  type ConditionDraft,
  type EdgeConditionFormState,
  type NodeConfigFormState,
  type PrimitiveValueType,
} from '@/lib/trade-flow-config-mappers';
import {
  EDGE_STROKE_COLOR,
  GROUP_COLORS,
  NODE_PALETTE_CATEGORIES,
  NODE_TYPE_OPTIONS,
  type FlowCanvasEditorProps,
  type FlowEdge,
  type FlowNode,
  type NodeGroup,
  type NodePaletteCategory,
  type PlaceOrderPresetKind,
  type PlaceOrderPresetSeed,
} from './flow-canvas-constants';
import {
  autoLayoutNodes,
  buildPlaceOrderPresetConfig,
  createEdgeKey,
  createGraphFingerprint,
  createNodeKey,
  hasRequiredPlaceOrderSeed,
  isRecord,
  minimapColor,
  nodePaletteCategoryOf,
  toCanvasEdge,
  toCanvasNode,
  toDomainEdge,
  toDomainNode,
  toFiniteNumberValue,
  toTrimmedStringValue,
} from './flow-canvas-utils';
import { NODE_TYPES } from './flow-canvas-node-card';
import {
  EdgeInspectorPanel,
  NodeInspectorPanel,
  type EdgeInspectorActions,
  type NodeInspectorActions,
} from './flow-canvas-inspector';
import { useCanvasKeyboard } from './flow-canvas-keyboard';
import { exportGraphAsJson, importGraphFromFile } from './flow-import-export';

function FlowCanvasEditorBody({
  graph,
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
}: FlowCanvasEditorProps) {
  const graphFingerprint = useMemo(
    () => createGraphFingerprint(graph.nodes, graph.edges),
    [graph.nodes, graph.edges]
  );
  const [canvasNodes, setCanvasNodes] = useState<FlowNode[]>(() => graph.nodes.map(toCanvasNode));
  const [canvasEdges, setCanvasEdges] = useState<FlowEdge[]>(() => graph.edges.map(toCanvasEdge));
  const lastAppliedGraphFingerprintRef = useRef<string>(graphFingerprint);
  const canvasWrapperRef = useRef<HTMLDivElement | null>(null);
  const editorRootRef = useRef<HTMLDivElement | null>(null);
  const [pendingFocusNodeId, setPendingFocusNodeId] = useState<string | null>(null);

  const reactFlow = useReactFlow<FlowNode, FlowEdge>();
  const nodesInitialized = useNodesInitialized();
  const history = useCanvasHistory();

  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);
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
  const [clipboard, setClipboard] = useState<FlowNode | null>(null);
  const [showNodeSearch, setShowNodeSearch] = useState(false);
  const [nodeSearchQuery, setNodeSearchQuery] = useState('');
  const nodeSearchInputRef = useRef<HTMLInputElement | null>(null);
  const [nodeGroups, setNodeGroups] = useState<NodeGroup[]>([]);
  const nextGroupColorIdx = useRef(0);

  const selectedNode = useMemo(
    () => (selectedNodeId ? canvasNodes.find((n) => n.id === selectedNodeId) || null : null),
    [canvasNodes, selectedNodeId]
  );
  const selectedEdge = useMemo(
    () => (selectedEdgeId ? canvasEdges.find((e) => e.id === selectedEdgeId) || null : null),
    [canvasEdges, selectedEdgeId]
  );
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

  useEffect(() => {
    if (!executionMap) return;
    setCanvasNodes((prev) =>
      prev.map((n) => {
        const exec = executionMap.get(n.id);
        const nextStatus = exec?.status ?? 'idle';
        if (n.data.executionStatus === nextStatus) return n;
        return { ...n, data: { ...n.data, executionStatus: nextStatus } };
      })
    );
  }, [executionMap]);

  useEffect(() => {
    if (!livePrices) return;
    setCanvasNodes((prev) =>
      prev.map((n) => {
        if (!n.data.nodeType.startsWith('trigger.')) return n;
        const tokenId = toTrimmedStringValue(n.data.config.tokenId);
        const price = tokenId ? (livePrices[tokenId] ?? null) : null;
        if (n.data.livePrice === price) return n;
        return { ...n, data: { ...n.data, livePrice: price } };
      })
    );
  }, [livePrices]);

  const searchMatchedNodes = useMemo(() => {
    if (!nodeSearchQuery.trim()) return canvasNodes;
    const q = nodeSearchQuery.trim().toLowerCase();
    return canvasNodes.filter(
      (n) => n.id.toLowerCase().includes(q) || n.data.nodeType.toLowerCase().includes(q)
    );
  }, [canvasNodes, nodeSearchQuery]);

  const commitGraph = useCallback(
    (nextNodes: FlowNode[], nextEdges: FlowEdge[], skipHistory = false) => {
      if (!skipHistory) history.push(canvasNodes, canvasEdges);
      setCanvasNodes(nextNodes);
      setCanvasEdges(nextEdges);
      const domainNodes = nextNodes.map(toDomainNode);
      const domainEdges = nextEdges.map(toDomainEdge);
      lastAppliedGraphFingerprintRef.current = createGraphFingerprint(domainNodes, domainEdges);
      onGraphChange({ context: graph.context, nodes: domainNodes, edges: domainEdges });
    },
    [canvasEdges, canvasNodes, graph.context, history, onGraphChange]
  );

  useEffect(() => {
    if (graphFingerprint === lastAppliedGraphFingerprintRef.current) return;
    lastAppliedGraphFingerprintRef.current = graphFingerprint;
    setCanvasNodes(graph.nodes.map(toCanvasNode));
    setCanvasEdges(graph.edges.map(toCanvasEdge));
  }, [graph.edges, graph.nodes, graphFingerprint]);

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

  const getInsertPosition = useCallback(() => {
    if (selectedNode) return { x: selectedNode.position.x + 260, y: selectedNode.position.y + 20 };
    const canvasRect = canvasWrapperRef.current?.getBoundingClientRect();
    if (!canvasRect) {
      const viewport = reactFlow.getViewport();
      const zoom = viewport.zoom || 1;
      return { x: Math.round((-viewport.x + 220) / zoom), y: Math.round((-viewport.y + 120) / zoom) };
    }
    const mapped = reactFlow.screenToFlowPosition({
      x: canvasRect.left + canvasRect.width * 0.46,
      y: canvasRect.top + canvasRect.height * 0.32,
    });
    return { x: Math.round(mapped.x), y: Math.round(mapped.y) };
  }, [reactFlow, selectedNode]);

  const clearSelection = useCallback(() => {
    setSelectedNodeId(null);
    setSelectedEdgeId(null);
    setNodeForm(null);
    setEdgeForm(null);
    setHasPendingNodeDraft(false);
  }, []);

  const hydrateNodeDraft = useCallback((node: FlowNode) => {
    setSelectedNodeId(node.id);
    setSelectedEdgeId(null);
    setNodeInspectorTab('basic');
    setNodeKeyDraft(node.id);
    setNodeTypeDraft(node.data.nodeType);
    setNodeForm(parseNodeConfigToForm(node.data.nodeType, node.data.config));
    setEdgeForm(null);
    setHasPendingNodeDraft(false);
  }, []);

  const hydrateEdgeDraft = useCallback((edge: FlowEdge) => {
    setSelectedEdgeId(edge.id);
    setSelectedNodeId(null);
    setEdgeInspectorTab('basic');
    setEdgeTypeDraft(edge.data?.edgeType || 'default');
    setEdgeForm(parseEdgeConditionToForm(edge.data?.condition ?? null));
    setNodeForm(null);
  }, []);

  const onSelectionChange = useCallback(
    ({ nodes: pn, edges: pe }: { nodes: FlowNode[]; edges: FlowEdge[] }) => {
      if (pn.length > 0) { hydrateNodeDraft(pn[0]); onError(null); return; }
      if (pe.length > 0) { hydrateEdgeDraft(pe[0]); onError(null); return; }
      clearSelection();
    },
    [clearSelection, hydrateEdgeDraft, hydrateNodeDraft, onError]
  );

  const handleNodesChange = useCallback(
    (changes: NodeChange<FlowNode>[]) => {
      const nextNodes = applyNodeChanges(changes, canvasNodes);
      const nodeIdSet = new Set(nextNodes.map((n) => n.id));
      const nextEdges = canvasEdges.filter((e) => nodeIdSet.has(e.source) && nodeIdSet.has(e.target));
      if (selectedNodeId && !nodeIdSet.has(selectedNodeId)) clearSelection();
      const hasRemoval = changes.some((c) => c.type === 'remove');
      commitGraph(nextNodes, nextEdges, !hasRemoval);
    },
    [canvasEdges, canvasNodes, clearSelection, commitGraph, selectedNodeId]
  );

  const handleEdgesChange = useCallback(
    (changes: EdgeChange<FlowEdge>[]) => {
      const nextEdges = applyEdgeChanges(changes, canvasEdges);
      if (selectedEdgeId && !nextEdges.some((e) => e.id === selectedEdgeId)) clearSelection();
      const hasRemoval = changes.some((c) => c.type === 'remove');
      commitGraph(canvasNodes, nextEdges, !hasRemoval);
    },
    [canvasEdges, canvasNodes, clearSelection, commitGraph, selectedEdgeId]
  );

  const handleConnect = useCallback(
    (connection: Connection) => {
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
    [canvasEdges, canvasNodes, commitGraph, onError]
  );

  const addNode = (nodeType: string) => {
    const nodeId = createNodeKey(nodeType, new Set(canvasNodes.map((n) => n.id)));
    const nextNode: FlowNode = {
      id: nodeId, type: 'flowNode', position: getInsertPosition(),
      data: { nodeType, config: {} },
    };
    commitGraph([...canvasNodes, nextNode], canvasEdges);
    hydrateNodeDraft(nextNode);
    queueNodeFocus(nextNode.id);
    onError(null);
  };

  const addPresetPlaceOrderNode = (kind: PlaceOrderPresetKind) => {
    const fromSel: PlaceOrderPresetSeed | null =
      selectedNode && selectedNode.data.nodeType === 'trigger.open_positions'
        ? {
            sourceTradeId: toFiniteNumberValue(selectedNode.data.config.sourceTradeId),
            marketSlug: toTrimmedStringValue(selectedNode.data.config.marketSlug),
            tokenId: toTrimmedStringValue(selectedNode.data.config.tokenId),
            outcomeLabel: toTrimmedStringValue(selectedNode.data.config.outcomeLabel),
          }
        : null;
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
    commitGraph([...canvasNodes, nextNode], canvasEdges);
    hydrateNodeDraft(nextNode);
    queueNodeFocus(nextNode.id);
    if (!hasRequiredPlaceOrderSeed(seed)) {
      onError(kind === 'sell_current_position'
        ? 'Mevcut pozisyon kaynak bilgisi eksik. Node eklendi, alanlari manuel doldurun.'
        : 'Alis preset node eklendi. Eksik alanlari manuel doldurun.');
      return;
    }
    onError(null);
  };

  const deleteSelectedNode = () => {
    if (!selectedNode) return;
    const nextNodes = canvasNodes.filter((n) => n.id !== selectedNode.id);
    const nextEdges = canvasEdges.filter((e) => e.source !== selectedNode.id && e.target !== selectedNode.id);
    commitGraph(nextNodes, nextEdges);
    clearSelection();
    onError(null);
  };

  const deleteSelectedEdge = () => {
    if (!selectedEdge) return;
    commitGraph(canvasNodes, canvasEdges.filter((e) => e.id !== selectedEdge.id));
    clearSelection();
    onError(null);
  };

  const updateNodeField = (key: string, value: string) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => prev ? { ...prev, fields: { ...prev.fields, [key]: value } } : prev);
  };

  const updateTriggerSizeRow = (index: number, value: string) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => {
      if (!prev) return prev;
      const nextRows = [...prev.triggerSizeRows];
      while (nextRows.length <= index) nextRows.push('');
      nextRows[index] = value;
      return { ...prev, triggerSizeRows: nextRows };
    });
  };

  useEffect(() => {
    if (nodeTypeDraft !== 'action.place_order') return;
    setNodeForm((prev) => {
      if (!prev) return prev;
      const parsedMax = Number(prev.fields.maxTriggers ?? '');
      const targetCount = Number.isFinite(parsedMax) && parsedMax > 1 ? Math.min(20, Math.floor(parsedMax)) : 0;
      const currentRows = prev.triggerSizeRows || [];
      const nextRows = targetCount > 0
        ? Array.from({ length: targetCount }, (_, i) => currentRows[i] ?? '')
        : [];
      const unchanged = nextRows.length === currentRows.length && nextRows.every((v, i) => v === currentRows[i]);
      if (unchanged) return prev;
      return { ...prev, triggerSizeRows: nextRows };
    });
  }, [nodeTypeDraft, nodeForm?.fields.maxTriggers]);

  const canApplyOpenPosition = (p: TradeFlowOpenPositionOption) =>
    p.matchedTradeId != null ? true : Boolean(p.marketSlug.trim() && p.tokenId.trim());

  const applyOpenPositionSelection = async (position: TradeFlowOpenPositionOption) => {
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
      if (selectedNode && selectedNode.data.nodeType === 'trigger.open_positions') {
        const nextNodes = canvasNodes.map((n) => n.id !== selectedNode.id ? n : {
          ...n, data: { ...n.data, config: { ...n.data.config, sourceTradeId: nid,
            marketSlug: position.marketSlug, tokenId: position.tokenId, outcomeLabel: position.outcomeLabel } },
        });
        commitGraph(nextNodes, canvasEdges);
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
    setNodeForm((prev) => prev ? {
      ...prev, expressionRows: prev.expressionRows.map((r) => r.id === rowId ? { ...r, ...patch } : r),
    } : prev);
  };

  const addExpressionRow = () => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => prev ? { ...prev, expressionRows: [...prev.expressionRows, createEmptyConditionDraft()] } : prev);
  };

  const removeExpressionRow = (rowId: string) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => {
      if (!prev) return prev;
      const next = prev.expressionRows.filter((r) => r.id !== rowId);
      return { ...prev, expressionRows: next.length > 0 ? next : [createEmptyConditionDraft()] };
    });
  };

  const updateStatePatchRow = (rowId: string, patch: Partial<{ key: string; value: string; valueType: PrimitiveValueType }>) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => prev ? {
      ...prev, statePatchRows: prev.statePatchRows.map((r) => r.id === rowId ? { ...r, ...patch } : r),
    } : prev);
  };

  const addStatePatchRow = () => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => prev ? { ...prev, statePatchRows: [...prev.statePatchRows, createEmptyKeyValueDraft()] } : prev);
  };

  const removeStatePatchRow = (rowId: string) => {
    setHasPendingNodeDraft(true);
    setNodeForm((prev) => {
      if (!prev) return prev;
      const next = prev.statePatchRows.filter((r) => r.id !== rowId);
      return { ...prev, statePatchRows: next.length > 0 ? next : [createEmptyKeyValueDraft()] };
    });
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
    const nextKey = nodeKeyDraft.trim();
    const nextType = nodeTypeDraft.trim();
    if (!nextKey || !nextType) { onError('Node key ve type bos olamaz.'); return; }

    let parsedConfig: Record<string, unknown>;
    if (source === 'advanced') {
      const adv = parseAdvancedConfig();
      if (!adv) return;
      parsedConfig = adv;
    } else {
      if (!nodeForm) return;
      parsedConfig = buildNodeConfigFromForm(nextType, nodeForm);
    }

    if (mode === 'create') {
      if (canvasNodes.some((n) => n.id === nextKey)) { onError(`Ayni key ile baska node var: ${nextKey}`); return; }
      const nextNode: FlowNode = { id: nextKey, type: 'flowNode', position: getInsertPosition(), data: { nodeType: nextType, config: parsedConfig } };
      commitGraph([...canvasNodes, nextNode], canvasEdges);
      hydrateNodeDraft(nextNode);
      queueNodeFocus(nextNode.id);
      setHasPendingNodeDraft(false);
    } else {
      if (!selectedNode) { onError('Guncellemek icin once bir node secin.'); return; }
      if (nextKey !== selectedNode.id && canvasNodes.some((n) => n.id === nextKey)) { onError(`Ayni key ile baska node var: ${nextKey}`); return; }
      const nextNodes = canvasNodes.map((n) => n.id !== selectedNode.id ? n : { ...n, id: nextKey, data: { ...n.data, nodeType: nextType, config: parsedConfig } });
      const nextEdges = canvasEdges.map((e) => ({ ...e, source: e.source === selectedNode.id ? nextKey : e.source, target: e.target === selectedNode.id ? nextKey : e.target }));
      commitGraph(nextNodes, nextEdges);
      setSelectedNodeId(nextKey);
      setNodeForm(parseNodeConfigToForm(nextType, parsedConfig));
      setHasPendingNodeDraft(false);
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
    const snapshot = history.undo(canvasNodes, canvasEdges);
    if (!snapshot) return;
    setCanvasNodes(snapshot.nodes);
    setCanvasEdges(snapshot.edges);
    const dn = snapshot.nodes.map(toDomainNode);
    const de = snapshot.edges.map(toDomainEdge);
    lastAppliedGraphFingerprintRef.current = createGraphFingerprint(dn, de);
    onGraphChange({ context: graph.context, nodes: dn, edges: de });
    clearSelection();
  }, [canvasEdges, canvasNodes, clearSelection, graph.context, history, onGraphChange]);

  const handleRedo = useCallback(() => {
    const snapshot = history.redo(canvasNodes, canvasEdges);
    if (!snapshot) return;
    setCanvasNodes(snapshot.nodes);
    setCanvasEdges(snapshot.edges);
    const dn = snapshot.nodes.map(toDomainNode);
    const de = snapshot.edges.map(toDomainEdge);
    lastAppliedGraphFingerprintRef.current = createGraphFingerprint(dn, de);
    onGraphChange({ context: graph.context, nodes: dn, edges: de });
    clearSelection();
  }, [canvasEdges, canvasNodes, clearSelection, graph.context, history, onGraphChange]);

  // Copy/Paste
  const handleCopy = useCallback(() => {
    if (selectedNode) setClipboard(structuredClone(selectedNode));
  }, [selectedNode]);

  const handlePaste = useCallback(() => {
    if (!clipboard) return;
    const newId = createNodeKey(clipboard.data.nodeType, new Set(canvasNodes.map((n) => n.id)));
    const pasted: FlowNode = {
      ...structuredClone(clipboard),
      id: newId,
      position: { x: clipboard.position.x + 40, y: clipboard.position.y + 40 },
    };
    commitGraph([...canvasNodes, pasted], canvasEdges);
    hydrateNodeDraft(pasted);
    queueNodeFocus(pasted.id);
  }, [canvasEdges, canvasNodes, clipboard, commitGraph, hydrateNodeDraft, queueNodeFocus]);

  // Auto-Layout
  const handleAutoLayout = useCallback(() => {
    const laid = autoLayoutNodes(canvasNodes, canvasEdges);
    commitGraph(laid, canvasEdges);
  }, [canvasEdges, canvasNodes, commitGraph]);

  // Import/Export
  const handleExport = useCallback(() => {
    exportGraphAsJson({ context: graph.context, nodes: canvasNodes.map(toDomainNode), edges: canvasEdges.map(toDomainEdge) });
  }, [canvasEdges, canvasNodes, graph.context]);

  const handleImport = useCallback(async () => {
    try {
      const imported = await importGraphFromFile();
      const nextNodes = imported.nodes.map(toCanvasNode);
      const nextEdges = imported.edges.map(toCanvasEdge);
      commitGraph(nextNodes, nextEdges);
      onError(null);
    } catch (err) {
      onError(err instanceof Error ? err.message : 'JSON yukleme hatasi.');
    }
  }, [commitGraph, onError]);

  // Group / Ungroup
  const handleGroupSelected = useCallback(() => {
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
  }, [canvasEdges, canvasNodes, commitGraph, nodeGroups.length, selectedNode]);

  const handleAssignToGroup = useCallback((groupId: string) => {
    if (!selectedNode) return;
    const group = nodeGroups.find((g) => g.id === groupId);
    if (!group) return;
    const nextNodes = canvasNodes.map((n) =>
      n.id === selectedNode.id ? { ...n, data: { ...n.data, groupId, groupColor: group.color } } : n
    );
    commitGraph(nextNodes, canvasEdges);
  }, [canvasEdges, canvasNodes, commitGraph, nodeGroups, selectedNode]);

  const handleUngroupSelected = useCallback(() => {
    if (!selectedNode) return;
    const nextNodes = canvasNodes.map((n) =>
      n.id === selectedNode.id ? { ...n, data: { ...n.data, groupId: undefined, groupColor: undefined } } : n
    );
    commitGraph(nextNodes, canvasEdges);
  }, [canvasEdges, canvasNodes, commitGraph, selectedNode]);

  // Keyboard
  useCanvasKeyboard({
    onSave: () => {},
    onUndo: handleUndo,
    onRedo: handleRedo,
    onCopy: handleCopy,
    onPaste: handlePaste,
    onSelectAll: () => {},
    onDeselect: clearSelection,
    onDelete: () => { if (selectedNode) deleteSelectedNode(); else if (selectedEdge) deleteSelectedEdge(); },
    onSearch: () => { setShowNodeSearch(true); setTimeout(() => nodeSearchInputRef.current?.focus(), 50); },
  }, editorRootRef);

  // Node search overlay focus
  useEffect(() => {
    if (showNodeSearch) nodeSearchInputRef.current?.focus();
  }, [showNodeSearch]);

  const triggerCount = canvasNodes.filter((n) => n.data.nodeType.startsWith('trigger.')).length;
  const logicCount = canvasNodes.filter((n) => n.data.nodeType.startsWith('logic.')).length;
  const actionCount = canvasNodes.filter((n) => n.data.nodeType.startsWith('action.')).length;

  const nodeInspectorActions: NodeInspectorActions = {
    onNodeKeyChange: (key) => {
      setNodeKeyDraft(key);
      setHasPendingNodeDraft(true);
    },
    onNodeTypeChange: handleNodeTypeChange,
    onTabChange: setNodeInspectorTab,
    onFormChange: (updater) => {
      setHasPendingNodeDraft(true);
      setNodeForm(updater);
    },
    onUpdateField: updateNodeField,
    onUpdateTriggerSizeRow: updateTriggerSizeRow,
    onCreateNode: () => createOrUpdateNode('create', 'basic'),
    onUpdateNode: () => createOrUpdateNode('update', 'basic'),
    onDeleteNode: deleteSelectedNode,
    onCreateFromAdvanced: () => createOrUpdateNode('create', 'advanced'),
    onUpdateFromAdvanced: () => createOrUpdateNode('update', 'advanced'),
    onApplyOpenPosition: (p) => { void applyOpenPositionSelection(p); },
    onUpdateExpressionRow: updateExpressionRow,
    onAddExpressionRow: addExpressionRow,
    onRemoveExpressionRow: removeExpressionRow,
    onUpdateStatePatchRow: updateStatePatchRow,
    onAddStatePatchRow: addStatePatchRow,
    onRemoveStatePatchRow: removeStatePatchRow,
  };

  const edgeInspectorActions: EdgeInspectorActions = {
    onEdgeTypeChange: setEdgeTypeDraft,
    onTabChange: setEdgeInspectorTab,
    onFormChange: setEdgeForm,
    onUpdateConditionRow: updateEdgeConditionRow,
    onApplyBasic: () => applyEdge('basic'),
    onApplyAdvanced: () => applyEdge('advanced'),
    onDeleteEdge: deleteSelectedEdge,
  };

  return (
    <div ref={editorRootRef} tabIndex={-1} className="rounded-2xl border border-slate-200 bg-[linear-gradient(180deg,#ffffff,#f8fafc)] p-4 shadow-sm outline-none">
      <div className="flex items-center justify-between">
        <div>
          <p className="text-xs font-medium tracking-wide text-slate-700">Canvas Editoru (Surukle &amp; Birak)</p>
          <p className="mt-1 text-[11px] text-slate-500">
            Sol panelden node ekleyin, baglanti noktalarindan edge cizerek akisi kurun.
          </p>
        </div>
        <div className="flex gap-1">
          <Button size="sm" variant="outline" className="h-7 border-slate-300 px-2 text-[11px] text-slate-600" onClick={handleUndo} disabled={!history.canUndo()} title="Geri Al (Ctrl+Z)">&#8617; Geri</Button>
          <Button size="sm" variant="outline" className="h-7 border-slate-300 px-2 text-[11px] text-slate-600" onClick={handleRedo} disabled={!history.canRedo()} title="Ileri Al (Ctrl+Shift+Z)">&#8618; Ileri</Button>
          <Button size="sm" variant="outline" className="h-7 border-slate-300 px-2 text-[11px] text-slate-600" onClick={handleAutoLayout} title="Otomatik Duzenleme">&#9638; Layout</Button>
          <Button size="sm" variant="outline" className="h-7 border-slate-300 px-2 text-[11px] text-slate-600" onClick={handleExport} title="JSON Aktar">&#8615; Export</Button>
          <Button size="sm" variant="outline" className="h-7 border-slate-300 px-2 text-[11px] text-slate-600" onClick={() => { void handleImport(); }} title="JSON Yukle">&#8613; Import</Button>
          <Button size="sm" variant="outline" className="h-7 border-slate-300 px-2 text-[11px] text-slate-600" onClick={() => { setShowNodeSearch(true); setTimeout(() => nodeSearchInputRef.current?.focus(), 50); }} title="Node Ara (Ctrl+K)">&#128269; Ara</Button>
        </div>
      </div>

      {/* Node Search Overlay */}
      {showNodeSearch && (
        <div className="relative z-20 mt-2">
          <div className="rounded-lg border border-slate-300 bg-white p-2 shadow-lg">
            <Input
              ref={nodeSearchInputRef}
              value={nodeSearchQuery}
              onChange={(e) => setNodeSearchQuery(e.target.value)}
              placeholder="Node key veya tip ile ara... (Esc kapat)"
              className="h-8 border-slate-300 bg-white text-xs text-slate-900"
              onKeyDown={(e) => {
                if (e.key === 'Escape') { setShowNodeSearch(false); setNodeSearchQuery(''); }
                if (e.key === 'Enter' && searchMatchedNodes.length > 0) {
                  const target = searchMatchedNodes[0];
                  hydrateNodeDraft(target);
                  queueNodeFocus(target.id);
                  setShowNodeSearch(false);
                  setNodeSearchQuery('');
                }
              }}
            />
            {nodeSearchQuery.trim() && (
              <div className="mt-1 max-h-40 space-y-1 overflow-auto">
                {searchMatchedNodes.length === 0 ? (
                  <p className="text-[11px] text-slate-500">Eslesen node yok.</p>
                ) : searchMatchedNodes.map((n) => (
                  <button
                    key={n.id}
                    type="button"
                    className="w-full rounded-md px-2 py-1 text-left text-[11px] text-slate-700 hover:bg-slate-100"
                    onClick={() => {
                      hydrateNodeDraft(n);
                      queueNodeFocus(n.id);
                      setShowNodeSearch(false);
                      setNodeSearchQuery('');
                    }}
                  >
                    <span className="font-medium">{n.id}</span>
                    <span className="ml-2 text-slate-500">{n.data.nodeType}</span>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      <div className="mt-4 grid gap-3 xl:grid-cols-[220px_minmax(0,1fr)_380px]">
        {/* Left Panel */}
        <div className="space-y-3 rounded-xl border border-slate-200 bg-slate-50 p-3">
          {leftPanelTopSlot}
          <p className="text-xs font-medium text-slate-700">Node Paleti</p>
          <Input value={nodePaletteSearch} onChange={(e) => setNodePaletteSearch(e.target.value)} placeholder="Node ara..." className="h-8 border-slate-300 bg-white text-xs text-slate-900" />
          <div className="grid grid-cols-2 gap-1">
            {NODE_PALETTE_CATEGORIES.map((item) => (
              <button key={item.value} type="button"
                className={`h-8 rounded-md border text-xs ${nodePaletteCategory === item.value ? 'border-sky-300 bg-sky-100 text-sky-700' : 'border-slate-300 bg-white text-slate-600 hover:bg-slate-100'}`}
                onClick={() => setNodePaletteCategory(item.value)}>{item.label}</button>
            ))}
          </div>
          <div className="max-h-[320px] space-y-2 overflow-auto pr-1">
            {filteredNodeOptions.length === 0 ? (
              <p className="text-[11px] text-slate-500">Aramaya uygun node bulunamadi.</p>
            ) : filteredNodeOptions.map((option) => (
              <Button key={option.value} type="button" size="sm" variant="outline"
                className="w-full justify-start border-slate-300 bg-white text-slate-700 hover:bg-slate-100"
                onClick={() => addNode(option.value)}>+ {option.label}</Button>
            ))}
          </div>

          <div className="space-y-2 overflow-hidden rounded-md border border-slate-200 bg-white p-2">
            <p className="text-[11px] font-medium text-slate-700">Hizli Presetler</p>
            <p className="text-[10px] text-slate-500">Presetler action.place_order node&apos;u uretir.</p>
            <Button type="button" size="sm" variant="outline"
              className="h-auto min-h-8 w-full justify-start whitespace-normal break-words border-slate-300 bg-white py-1.5 text-left leading-tight text-slate-700 hover:bg-slate-100"
              onClick={() => addPresetPlaceOrderNode('place_order')}>+ Preset: Al / Sat</Button>
          </div>

          <div className="rounded-md border border-slate-200 bg-white p-2 text-[11px] text-slate-500">
            <p>Node: {canvasNodes.length}</p>
            <p>Edge: {canvasEdges.length}</p>
            <p>Trigger: {triggerCount} | Logic: {logicCount} | Action: {actionCount}</p>
          </div>

          <Button size="sm" variant="outline" className="w-full border-slate-300 text-slate-700 hover:bg-slate-100"
            disabled={!selectedNode && !selectedEdge}
            onClick={() => { if (selectedNode) deleteSelectedNode(); else if (selectedEdge) deleteSelectedEdge(); }}>
            Secili Ogeyi Sil
          </Button>

          <div className="space-y-1.5 rounded-md border border-slate-200 bg-white p-2">
            <p className="text-[11px] font-medium text-slate-700">Node Gruplama</p>
            <Button size="sm" variant="outline" className="w-full border-slate-300 text-[11px] text-slate-700 hover:bg-slate-100"
              disabled={!selectedNode} onClick={handleGroupSelected}>
              + Yeni Grup Olustur
            </Button>
            {selectedNode?.data.groupId && (
              <Button size="sm" variant="outline" className="w-full border-slate-300 text-[11px] text-slate-700 hover:bg-slate-100"
                onClick={handleUngroupSelected}>
                Gruptan Cikar
              </Button>
            )}
            {nodeGroups.length > 0 && selectedNode && (
              <div className="space-y-1">
                <p className="text-[10px] text-slate-500">Gruba Ekle:</p>
                {nodeGroups.map((g) => (
                  <button key={g.id} type="button"
                    className="flex w-full items-center gap-1.5 rounded-md border border-slate-200 px-2 py-1 text-left text-[11px] text-slate-700 hover:bg-slate-100"
                    onClick={() => handleAssignToGroup(g.id)}>
                    <span className="inline-block h-3 w-3 rounded-full" style={{ backgroundColor: g.color }} />
                    {g.name}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* Canvas */}
        <div ref={canvasWrapperRef} className="flow-canvas h-[calc(100vh-12rem)] min-h-[500px] rounded-xl border border-slate-200 bg-white">
          <ReactFlow<FlowNode, FlowEdge>
            nodes={canvasNodes} edges={canvasEdges} nodeTypes={NODE_TYPES}
            onNodesChange={handleNodesChange} onEdgesChange={handleEdgesChange}
            onConnect={handleConnect} onSelectionChange={onSelectionChange}
            fitView minZoom={0.25} maxZoom={1.6}
            deleteKeyCode={['Backspace', 'Delete']}
            defaultEdgeOptions={{
              type: 'smoothstep',
              markerEnd: { type: MarkerType.ArrowClosed, color: EDGE_STROKE_COLOR, width: 16, height: 16 },
              style: { stroke: EDGE_STROKE_COLOR, strokeWidth: 1.6 },
            }}>
            <MiniMap pannable zoomable nodeColor={minimapColor} />
            <Controls />
            <Background gap={20} size={1.1} color="#cbd5e1" />
          </ReactFlow>
        </div>

        {/* Right Panel */}
        <div className="flex flex-col overflow-hidden rounded-xl border border-slate-200 bg-white/95 p-3">
          {selectedNode && nodeForm ? (
            <NodeInspectorPanel
              node={selectedNode} form={nodeForm} nodeKeyDraft={nodeKeyDraft}
              nodeTypeDraft={nodeTypeDraft} tab={nodeInspectorTab}
              openPositions={openPositions} openPositionsMeta={openPositionsMeta}
              openPositionsLoading={openPositionsLoading}
              openPositionApplyingKey={openPositionApplyingKey}
              canApplyOpenPosition={canApplyOpenPosition}
              actions={nodeInspectorActions}
            />
          ) : selectedEdge && edgeForm ? (
            <EdgeInspectorPanel
              edge={selectedEdge} form={edgeForm} edgeTypeDraft={edgeTypeDraft}
              tab={edgeInspectorTab} actions={edgeInspectorActions}
            />
          ) : (
            <div className="space-y-2 text-xs text-slate-500">
              <p>Bir node veya edge secin.</p>
              <p>Form sekmesinde dogrudan alan girerek duzenleyebilirsiniz.</p>
              <p>JSON yalniz Advanced sekmesinde tutulur.</p>
              <p className="text-[10px] text-slate-400">
                Ctrl+Z: Geri Al | Ctrl+Shift+Z: Ileri Al | Ctrl+C/V: Kopyala/Yapistir | Ctrl+K: Ara
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export function FlowCanvasEditor(props: FlowCanvasEditorProps) {
  return (
    <ReactFlowProvider>
      <FlowCanvasEditorBody {...props} />
    </ReactFlowProvider>
  );
}
