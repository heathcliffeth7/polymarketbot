'use client';

import { ReactFlowProvider } from '@xyflow/react';
import type { FlowCanvasEditorProps } from '../flow-canvas-constants';
import { FlowCanvasEditorBody } from './editor-body';

export function FlowCanvasEditor(props: FlowCanvasEditorProps) {
  return (
    <ReactFlowProvider>
      <FlowCanvasEditorBody {...props} />
    </ReactFlowProvider>
  );
}
