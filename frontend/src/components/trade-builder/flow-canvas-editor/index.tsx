'use client';

import { Component, useState, type ReactNode } from 'react';
import { ReactFlowProvider } from '@xyflow/react';
import { Button } from '@/components/ui/button';
import type { FlowCanvasEditorProps } from '../flow-canvas-constants';
import { FlowCanvasEditorBody } from './editor-body';

interface FlowCanvasErrorBoundaryProps {
  children: ReactNode;
  retryKey: string;
  onRetry: () => void;
}

interface FlowCanvasErrorBoundaryState {
  hasError: boolean;
}

class FlowCanvasErrorBoundary extends Component<
  FlowCanvasErrorBoundaryProps,
  FlowCanvasErrorBoundaryState
> {
  state: FlowCanvasErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError(): FlowCanvasErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: Error) {
    console.error('Trade Builder canvas crashed:', error);
  }

  componentDidUpdate(prevProps: FlowCanvasErrorBoundaryProps) {
    if (this.state.hasError && prevProps.retryKey !== this.props.retryKey) {
      this.setState({ hasError: false });
    }
  }

  render() {
    if (!this.state.hasError) {
      return this.props.children;
    }

    return (
      <div className="rounded-2xl border border-amber-300 bg-amber-50 p-4 text-sm text-amber-900">
        <p className="font-medium">Trade Builder canvas yuklenirken client-side hata olustu.</p>
        <p className="mt-1 text-xs text-amber-800">
          Flow secimi ve ust panel kullanilabilir durumda. Canvas icin tekrar deneyebilirsin.
        </p>
        <Button size="sm" className="mt-3" onClick={this.props.onRetry}>
          Canvas&apos;i Tekrar Dene
        </Button>
      </div>
    );
  }
}

export function FlowCanvasEditor(props: FlowCanvasEditorProps) {
  const [retrySeed, setRetrySeed] = useState(0);
  const retryKey = [
    props.graph.nodes.map((node) => node.key).join(','),
    props.graph.edges.map((edge) => edge.key).join(','),
    retrySeed,
  ].join('|');

  return (
    <FlowCanvasErrorBoundary
      retryKey={retryKey}
      onRetry={() => setRetrySeed((current) => current + 1)}
    >
      <ReactFlowProvider key={retryKey}>
        <FlowCanvasEditorBody {...props} />
      </ReactFlowProvider>
    </FlowCanvasErrorBoundary>
  );
}
