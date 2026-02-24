import { useCallback, useRef } from 'react';
import type { FlowEdge, FlowNode } from '@/components/trade-builder/flow-canvas-constants';

interface HistorySnapshot {
  nodes: FlowNode[];
  edges: FlowEdge[];
}

const MAX_HISTORY = 50;

export function useCanvasHistory() {
  const undoStack = useRef<HistorySnapshot[]>([]);
  const redoStack = useRef<HistorySnapshot[]>([]);

  const push = useCallback((nodes: FlowNode[], edges: FlowEdge[]) => {
    undoStack.current = [
      ...undoStack.current.slice(-(MAX_HISTORY - 1)),
      { nodes: structuredClone(nodes), edges: structuredClone(edges) },
    ];
    redoStack.current = [];
  }, []);

  const undo = useCallback(
    (
      currentNodes: FlowNode[],
      currentEdges: FlowEdge[]
    ): HistorySnapshot | null => {
      if (undoStack.current.length === 0) return null;
      const snapshot = undoStack.current[undoStack.current.length - 1];
      undoStack.current = undoStack.current.slice(0, -1);
      redoStack.current = [
        ...redoStack.current,
        { nodes: structuredClone(currentNodes), edges: structuredClone(currentEdges) },
      ];
      return snapshot;
    },
    []
  );

  const redo = useCallback(
    (
      currentNodes: FlowNode[],
      currentEdges: FlowEdge[]
    ): HistorySnapshot | null => {
      if (redoStack.current.length === 0) return null;
      const snapshot = redoStack.current[redoStack.current.length - 1];
      redoStack.current = redoStack.current.slice(0, -1);
      undoStack.current = [
        ...undoStack.current,
        { nodes: structuredClone(currentNodes), edges: structuredClone(currentEdges) },
      ];
      return snapshot;
    },
    []
  );

  const canUndo = useCallback(() => undoStack.current.length > 0, []);
  const canRedo = useCallback(() => redoStack.current.length > 0, []);

  return { push, undo, redo, canUndo, canRedo };
}
