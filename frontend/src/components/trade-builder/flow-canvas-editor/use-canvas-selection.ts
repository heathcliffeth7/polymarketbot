import {
  useCallback,
  useMemo,
  useState,
  type Dispatch,
  type MutableRefObject,
  type SetStateAction,
} from 'react';
import {
  parseEdgeConditionToForm,
  parseNodeConfigToForm,
  type EdgeConditionFormState,
  type NodeConfigFormState,
} from '@/lib/trade-flow-config-mappers';
import type { FlowEdge, FlowNode } from '../flow-canvas-constants';
import {
  applyCanvasElementSelection,
  buildSelectionClipboard,
  pasteSelectionClipboard,
  type FlowCanvasPasteAnchor,
  type FlowCanvasClipboard,
} from './selection-clipboard';

interface UseCanvasSelectionArgs {
  canvasEdges: FlowEdge[];
  canvasEdgesRef: MutableRefObject<FlowEdge[]>;
  canvasNodes: FlowNode[];
  canvasNodesRef: MutableRefObject<FlowNode[]>;
  commitGraph: (
    nextNodes: FlowNode[],
    nextEdges: FlowEdge[],
    skipHistory?: boolean,
    allowGraphShrink?: boolean,
    persistImmediately?: boolean
  ) => void;
  onError: (message: string | null) => void;
  queueNodeFocus: (nodeId: string) => void;
  getPasteAnchor: () => FlowCanvasPasteAnchor | null;
  setCanvasGraphState: (nextNodes: FlowNode[], nextEdges: FlowEdge[]) => void;
  setEdgeForm: Dispatch<SetStateAction<EdgeConditionFormState | null>>;
  setEdgeInspectorTab: Dispatch<SetStateAction<'basic' | 'advanced'>>;
  setEdgeTypeDraft: Dispatch<SetStateAction<string>>;
  setHasPendingNodeDraft: Dispatch<SetStateAction<boolean>>;
  setNodeForm: Dispatch<SetStateAction<NodeConfigFormState | null>>;
  setNodeInspectorTab: Dispatch<SetStateAction<'basic' | 'advanced'>>;
  setNodeKeyDraft: Dispatch<SetStateAction<string>>;
  setNodeTypeDraft: Dispatch<SetStateAction<string>>;
}

export function useCanvasSelection({
  canvasEdges,
  canvasEdgesRef,
  canvasNodes,
  canvasNodesRef,
  commitGraph,
  onError,
  queueNodeFocus,
  getPasteAnchor,
  setCanvasGraphState,
  setEdgeForm,
  setEdgeInspectorTab,
  setEdgeTypeDraft,
  setHasPendingNodeDraft,
  setNodeForm,
  setNodeInspectorTab,
  setNodeKeyDraft,
  setNodeTypeDraft,
}: UseCanvasSelectionArgs) {
  const [selectedNodeIds, setSelectedNodeIds] = useState<string[]>([]);
  const [selectedEdgeIds, setSelectedEdgeIds] = useState<string[]>([]);
  const [clipboard, setClipboard] = useState<FlowCanvasClipboard | null>(null);

  const inspectedNodeId =
    selectedNodeIds.length === 1 && selectedEdgeIds.length === 0 ? (selectedNodeIds[0] ?? null) : null;
  const inspectedEdgeId =
    selectedEdgeIds.length === 1 && selectedNodeIds.length === 0 ? (selectedEdgeIds[0] ?? null) : null;

  const selectedNode = useMemo(
    () => (inspectedNodeId ? canvasNodes.find((node) => node.id === inspectedNodeId) || null : null),
    [canvasNodes, inspectedNodeId]
  );
  const selectedEdge = useMemo(
    () => (inspectedEdgeId ? canvasEdges.find((edge) => edge.id === inspectedEdgeId) || null : null),
    [canvasEdges, inspectedEdgeId]
  );

  const syncCanvasSelection = useCallback(
    (nodeIds: string[], edgeIds: string[]) => {
      const next = applyCanvasElementSelection(
        canvasNodesRef.current,
        canvasEdgesRef.current,
        nodeIds,
        edgeIds
      );
      setCanvasGraphState(next.nodes, next.edges);
    },
    [canvasEdgesRef, canvasNodesRef, setCanvasGraphState]
  );

  const setMultiSelectionState = useCallback(
    (nodeIds: string[], edgeIds: string[], syncCanvas = false) => {
      setSelectedNodeIds(nodeIds);
      setSelectedEdgeIds(edgeIds);
      setNodeForm(null);
      setEdgeForm(null);
      setHasPendingNodeDraft(false);
      if (syncCanvas) syncCanvasSelection(nodeIds, edgeIds);
    },
    [setEdgeForm, setHasPendingNodeDraft, setNodeForm, syncCanvasSelection]
  );

  const clearSelection = useCallback(
    (syncCanvas = false) => {
      setSelectedNodeIds([]);
      setSelectedEdgeIds([]);
      setNodeForm(null);
      setEdgeForm(null);
      setHasPendingNodeDraft(false);
      if (syncCanvas) syncCanvasSelection([], []);
    },
    [setEdgeForm, setHasPendingNodeDraft, setNodeForm, syncCanvasSelection]
  );

  const hydrateNodeDraft = useCallback(
    (node: FlowNode, syncCanvas = false) => {
      setSelectedNodeIds([node.id]);
      setSelectedEdgeIds([]);
      setNodeInspectorTab('basic');
      setNodeKeyDraft(node.id);
      setNodeTypeDraft(node.data.nodeType);
      setNodeForm(parseNodeConfigToForm(node.data.nodeType, node.data.config));
      setEdgeForm(null);
      setHasPendingNodeDraft(false);
      if (syncCanvas) syncCanvasSelection([node.id], []);
    },
    [
      setEdgeForm,
      setHasPendingNodeDraft,
      setNodeForm,
      setNodeInspectorTab,
      setNodeKeyDraft,
      setNodeTypeDraft,
      syncCanvasSelection,
    ]
  );

  const hydrateEdgeDraft = useCallback(
    (edge: FlowEdge, syncCanvas = false) => {
      setSelectedEdgeIds([edge.id]);
      setSelectedNodeIds([]);
      setEdgeInspectorTab('basic');
      setEdgeTypeDraft(edge.data?.edgeType || 'default');
      setEdgeForm(parseEdgeConditionToForm(edge.data?.condition ?? null));
      setNodeForm(null);
      setHasPendingNodeDraft(false);
      if (syncCanvas) syncCanvasSelection([], [edge.id]);
    },
    [
      setEdgeForm,
      setEdgeInspectorTab,
      setEdgeTypeDraft,
      setHasPendingNodeDraft,
      setNodeForm,
      syncCanvasSelection,
    ]
  );

  const onSelectionChange = useCallback(
    ({ nodes, edges }: { nodes: FlowNode[]; edges: FlowEdge[] }) => {
      if (nodes.length === 1 && edges.length === 0) {
        hydrateNodeDraft(nodes[0]);
        onError(null);
        return;
      }
      if (edges.length === 1 && nodes.length === 0) {
        hydrateEdgeDraft(edges[0]);
        onError(null);
        return;
      }
      if (nodes.length > 0 || edges.length > 0) {
        setMultiSelectionState(
          nodes.map((node) => node.id),
          edges.map((edge) => edge.id)
        );
        onError(null);
        return;
      }
      clearSelection();
    },
    [clearSelection, hydrateEdgeDraft, hydrateNodeDraft, onError, setMultiSelectionState]
  );

  const deleteSelection = useCallback(() => {
    const nodeIdSet = new Set(selectedNodeIds);
    const edgeIdSet = new Set(selectedEdgeIds);
    if (nodeIdSet.size === 0 && edgeIdSet.size === 0) return;

    const nextNodes = canvasNodes.filter((node) => !nodeIdSet.has(node.id));
    const nextEdges = canvasEdges.filter(
      (edge) =>
        !edgeIdSet.has(edge.id) &&
        !nodeIdSet.has(edge.source) &&
        !nodeIdSet.has(edge.target)
    );

    commitGraph(nextNodes, nextEdges, false, true, true);
    clearSelection();
    onError(null);
  }, [canvasEdges, canvasNodes, clearSelection, commitGraph, onError, selectedEdgeIds, selectedNodeIds]);

  const deleteSelectedNode = useCallback(() => {
    if (!selectedNode) return;
    deleteSelection();
  }, [deleteSelection, selectedNode]);

  const deleteSelectedEdge = useCallback(() => {
    if (!selectedEdge) return;
    deleteSelection();
  }, [deleteSelection, selectedEdge]);

  const handleCopy = useCallback(() => {
    const nextClipboard = buildSelectionClipboard(
      canvasNodesRef.current,
      canvasEdgesRef.current,
      selectedNodeIds
    );
    if (nextClipboard) setClipboard(nextClipboard);
  }, [canvasEdgesRef, canvasNodesRef, selectedNodeIds]);

  const handlePaste = useCallback(() => {
    if (!clipboard) return;
    const result = pasteSelectionClipboard(
      clipboard,
      canvasNodesRef.current,
      canvasEdgesRef.current,
      { anchor: getPasteAnchor() }
    );
    if (!result) return;

    commitGraph(result.nodes, result.edges);
    setClipboard(result.clipboard);

    if (result.pastedNodes.length === 1 && result.pastedEdges.length === 0) {
      hydrateNodeDraft(result.pastedNodes[0], true);
    } else {
      setMultiSelectionState(
        result.pastedNodes.map((node) => node.id),
        result.pastedEdges.map((edge) => edge.id),
        true
      );
    }

    queueNodeFocus(result.pastedNodes[0].id);
    onError(null);
  }, [
    canvasEdgesRef,
    canvasNodesRef,
    clipboard,
    commitGraph,
    getPasteAnchor,
    hydrateNodeDraft,
    onError,
    queueNodeFocus,
    setMultiSelectionState,
  ]);

  const selectedNodeCount = selectedNodeIds.length;
  const selectedEdgeCount = selectedEdgeIds.length;
  const hasActiveSelection = selectedNodeCount > 0 || selectedEdgeCount > 0;
  const isMultiSelection =
    selectedNodeCount + selectedEdgeCount > 1 ||
    (selectedNodeCount > 0 && selectedEdgeCount > 0);

  return {
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
  };
}
