import type { FlowEdge, FlowNode } from '../flow-canvas-constants';
import { createEdgeKey, createNodeKey } from '../flow-canvas-utils';

export interface FlowCanvasClipboard {
  nodes: FlowNode[];
  edges: FlowEdge[];
  pasteCount: number;
}

export interface FlowCanvasPasteAnchor {
  x: number;
  y: number;
}

interface MutableFlowNode extends FlowNode {
  dragging?: boolean;
  positionAbsolute?: { x: number; y: number };
  resizing?: boolean;
  selected?: boolean;
}

interface MutableFlowEdge extends FlowEdge {
  selected?: boolean;
}

export interface FlowCanvasPasteResult {
  clipboard: FlowCanvasClipboard;
  edges: FlowEdge[];
  nodes: FlowNode[];
  pastedEdges: FlowEdge[];
  pastedNodes: FlowNode[];
}

interface FlowCanvasPasteOptions {
  anchor?: FlowCanvasPasteAnchor | null;
  offsetStep?: number;
}

function toTrimmedString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

function isQuickPresetBuySellPlaceOrderConfig(config: Record<string, unknown>): boolean {
  const presetKindRaw = toTrimmedString(config.presetKind).toLowerCase();
  const refKeyRaw = toTrimmedString(config.refKey).toLowerCase();
  return (
    presetKindRaw === 'sell_current_position' ||
    presetKindRaw === 'buy_current_position' ||
    refKeyRaw === 'preset_sell_current_position' ||
    refKeyRaw === 'preset_buy_current_position'
  );
}

function isGenericPlaceOrderPresetConfig(config: Record<string, unknown>): boolean {
  if (isQuickPresetBuySellPlaceOrderConfig(config)) {
    return false;
  }

  const presetKindRaw = toTrimmedString(config.presetKind).toLowerCase();
  const refKeyRaw = toTrimmedString(config.refKey).toLowerCase();
  return presetKindRaw === 'place_order' || refKeyRaw === 'preset_place_order';
}

function normalizePastedPlaceOrderPresetRefKey(
  node: FlowNode,
  nextNodeId: string,
  knownNodeIds: ReadonlySet<string>
): void {
  if (node.data.nodeType !== 'action.place_order') return;
  const config = node.data.config;
  if (!config || typeof config !== 'object' || Array.isArray(config)) return;
  if (!isGenericPlaceOrderPresetConfig(config)) return;

  const refKeyRaw = toTrimmedString(config.refKey);
  if (
    !refKeyRaw ||
    refKeyRaw.toLowerCase() === 'preset_place_order' ||
    (refKeyRaw !== nextNodeId && knownNodeIds.has(refKeyRaw))
  ) {
    config.refKey = nextNodeId;
  }
}

function cloneClipboardNode(node: FlowNode): FlowNode {
  const cloned = structuredClone(node) as MutableFlowNode;
  const nextData = { ...cloned.data };

  delete nextData.executionStatus;
  delete nextData.livePrice;
  delete cloned.dragging;
  delete cloned.positionAbsolute;
  delete cloned.resizing;
  delete cloned.selected;

  cloned.data = nextData;
  return cloned;
}

function cloneClipboardEdge(edge: FlowEdge): FlowEdge {
  const cloned = structuredClone(edge) as MutableFlowEdge;
  delete cloned.selected;
  return cloned;
}

function nodeDimension(node: FlowNode): { height: number; width: number } {
  const width = node.measured?.width ?? node.width ?? node.initialWidth ?? 0;
  const height = node.measured?.height ?? node.height ?? node.initialHeight ?? 0;
  return { width, height };
}

function clipboardCenter(nodes: FlowNode[]): FlowCanvasPasteAnchor | null {
  if (nodes.length === 0) return null;

  let minX = Number.POSITIVE_INFINITY;
  let minY = Number.POSITIVE_INFINITY;
  let maxX = Number.NEGATIVE_INFINITY;
  let maxY = Number.NEGATIVE_INFINITY;

  for (const node of nodes) {
    const { width, height } = nodeDimension(node);
    minX = Math.min(minX, node.position.x);
    minY = Math.min(minY, node.position.y);
    maxX = Math.max(maxX, node.position.x + width);
    maxY = Math.max(maxY, node.position.y + height);
  }

  return {
    x: Math.round((minX + maxX) / 2),
    y: Math.round((minY + maxY) / 2),
  };
}

export function applyCanvasElementSelection(
  nodes: FlowNode[],
  edges: FlowEdge[],
  selectedNodeIds: string[],
  selectedEdgeIds: string[]
): { edges: FlowEdge[]; nodes: FlowNode[] } {
  const selectedNodeIdSet = new Set(selectedNodeIds);
  const selectedEdgeIdSet = new Set(selectedEdgeIds);

  const nextNodes = nodes.map((node) => {
    const isSelected = selectedNodeIdSet.has(node.id);
    return node.selected === isSelected ? node : { ...node, selected: isSelected };
  });

  const nextEdges = edges.map((edge) => {
    const isSelected = selectedEdgeIdSet.has(edge.id);
    return edge.selected === isSelected ? edge : { ...edge, selected: isSelected };
  });

  return { nodes: nextNodes, edges: nextEdges };
}

export function buildSelectionClipboard(
  nodes: FlowNode[],
  edges: FlowEdge[],
  selectedNodeIds: string[]
): FlowCanvasClipboard | null {
  if (selectedNodeIds.length === 0) return null;

  const selectedNodeIdSet = new Set(selectedNodeIds);
  const copiedNodes = nodes
    .filter((node) => selectedNodeIdSet.has(node.id))
    .map((node) => cloneClipboardNode(node));

  if (copiedNodes.length === 0) return null;

  const copiedEdges = edges
    .filter((edge) => selectedNodeIdSet.has(edge.source) && selectedNodeIdSet.has(edge.target))
    .map((edge) => cloneClipboardEdge(edge));

  return {
    nodes: copiedNodes,
    edges: copiedEdges,
    pasteCount: 0,
  };
}

export function pasteSelectionClipboard(
  clipboard: FlowCanvasClipboard,
  existingNodes: FlowNode[],
  existingEdges: FlowEdge[],
  options: FlowCanvasPasteOptions = {}
): FlowCanvasPasteResult | null {
  if (clipboard.nodes.length === 0) return null;

  const nextPasteCount = clipboard.pasteCount + 1;
  const offsetStep = options.offsetStep ?? 40;
  const offset = offsetStep * nextPasteCount;
  const sourceCenter = clipboardCenter(clipboard.nodes);
  const anchor = options.anchor;
  const nodeIdMap = new Map<string, string>();
  const knownNodeIds = new Set([
    ...existingNodes.map((node) => node.id),
    ...clipboard.nodes.map((node) => node.id),
  ]);
  const existingNodeIds = new Set(existingNodes.map((node) => node.id));

  const pastedNodes = clipboard.nodes.map((node) => {
    const newId = createNodeKey(node.data.nodeType, existingNodeIds);
    existingNodeIds.add(newId);
    nodeIdMap.set(node.id, newId);

    const nextNode = cloneClipboardNode(node);
    nextNode.id = newId;
    normalizePastedPlaceOrderPresetRefKey(nextNode, newId, knownNodeIds);
    if (anchor && sourceCenter) {
      let deltaX = anchor.x - sourceCenter.x;
      let deltaY = anchor.y - sourceCenter.y;
      if (Math.abs(deltaX) < offsetStep && Math.abs(deltaY) < offsetStep) {
        deltaX += offset;
        deltaY += offset;
      }
      nextNode.position = {
        x: Math.round(nextNode.position.x + deltaX),
        y: Math.round(nextNode.position.y + deltaY),
      };
    } else {
      nextNode.position = {
        x: nextNode.position.x + offset,
        y: nextNode.position.y + offset,
      };
    }
    return nextNode;
  });

  const existingEdgeIds = new Set(existingEdges.map((edge) => edge.id));
  const pastedEdges = clipboard.edges.reduce<FlowEdge[]>((acc, edge) => {
    const source = nodeIdMap.get(edge.source);
    const target = nodeIdMap.get(edge.target);
    if (!source || !target) return acc;

    const newId = createEdgeKey(existingEdgeIds);
    existingEdgeIds.add(newId);

    const nextEdge = cloneClipboardEdge(edge);
    nextEdge.id = newId;
    nextEdge.source = source;
    nextEdge.target = target;
    acc.push(nextEdge);
    return acc;
  }, []);

  return {
    clipboard: {
      ...clipboard,
      pasteCount: nextPasteCount,
    },
    nodes: [...existingNodes, ...pastedNodes],
    edges: [...existingEdges, ...pastedEdges],
    pastedNodes,
    pastedEdges,
  };
}
