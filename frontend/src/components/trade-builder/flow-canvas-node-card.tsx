import { Handle, Position, NodeResizer, useReactFlow, type NodeProps } from '@xyflow/react';
import { NODE_TYPE_LABEL, type FlowNode } from './flow-canvas-constants';
import {
  dualDcaNodeLabel,
  nodeKindTone,
  openPositionNodeLabel,
  placeOrderNodeLabel,
  resolveMarketNodeLabel,
} from './flow-canvas-utils';
import type { NodeExecutionStatus } from '@/lib/types';

const EXECUTION_RING: Record<NodeExecutionStatus, string> = {
  idle: '',
  running: 'ring-2 ring-blue-400/70 animate-pulse',
  completed: 'ring-2 ring-emerald-400/80',
  failed: 'ring-2 ring-red-400/80',
  skipped: 'ring-2 ring-slate-300/60',
};

const EXECUTION_BADGE: Record<NodeExecutionStatus, { label: string; cls: string } | null> = {
  idle: null,
  running: { label: 'Running', cls: 'bg-blue-100 text-blue-700' },
  completed: { label: 'Done', cls: 'bg-emerald-100 text-emerald-700' },
  failed: { label: 'Error', cls: 'bg-red-100 text-red-700' },
  skipped: { label: 'Skip', cls: 'bg-slate-100 text-slate-500' },
};

export function FlowCanvasNodeCard({ data, selected, id }: NodeProps<FlowNode>) {
  const { deleteElements } = useReactFlow();
  const typeLabel = NODE_TYPE_LABEL.get(data.nodeType) || data.nodeType;
  const nodeTitle =
    data.nodeType === 'trigger.open_positions'
      ? openPositionNodeLabel(data.config) || data.nodeType
      : data.nodeType === 'action.resolve_market'
        ? resolveMarketNodeLabel(data.config)
      : data.nodeType === 'action.dual_dca'
        ? dualDcaNodeLabel(data.config)
      : data.nodeType === 'action.place_order'
        ? placeOrderNodeLabel(data.config)
        : data.nodeType;

  const execStatus = data.executionStatus ?? 'idle';
  const execRing = EXECUTION_RING[execStatus];
  const badge = EXECUTION_BADGE[execStatus];

  const groupStyle = data.groupColor
    ? { borderLeftWidth: '4px', borderLeftColor: data.groupColor }
    : undefined;

  return (
    <div
      className={`relative min-w-[210px] rounded-xl border px-3 py-2 shadow-sm transition ${nodeKindTone(data.nodeType)} ${
        execRing || (selected ? 'ring-2 ring-sky-400/80 shadow-md' : 'ring-1 ring-transparent')
      }`}
      style={groupStyle}
    >
      <NodeResizer
        isVisible={selected}
        minWidth={180}
        minHeight={60}
        maxWidth={500}
        maxHeight={300}
        lineClassName="!border-sky-300"
        handleClassName="!h-2.5 !w-2.5 !rounded-full !border-2 !border-sky-400 !bg-white"
      />
      {selected && (
        <button
          onClick={(e) => { e.stopPropagation(); deleteElements({ nodes: [{ id }] }); }}
          className="absolute -right-2 -top-2 z-10 flex h-5 w-5 items-center justify-center rounded-full bg-red-500 text-white text-xs shadow hover:bg-red-600"
        >
          ×
        </button>
      )}
      <Handle
        type="target"
        position={Position.Left}
        className="!h-2.5 !w-2.5 !border !border-slate-200 !bg-slate-700"
      />
      <div className="flex items-center justify-between gap-1">
        <p className="text-[10px] uppercase tracking-wide text-slate-600">{typeLabel}</p>
        {badge && (
          <span className={`rounded-full px-1.5 py-0.5 text-[9px] font-medium ${badge.cls}`}>
            {badge.label}
          </span>
        )}
      </div>
      <p className="mt-1 text-sm font-medium text-slate-900">{nodeTitle}</p>
      {data.livePrice != null && (
        <p className="mt-0.5 text-[11px] font-semibold tabular-nums text-sky-700">
          {data.livePrice.toFixed(1)}&cent;
        </p>
      )}
      <Handle
        type="source"
        position={Position.Right}
        className="!h-2.5 !w-2.5 !border !border-slate-200 !bg-slate-700"
      />
    </div>
  );
}

export const NODE_TYPES = {
  flowNode: FlowCanvasNodeCard,
};
