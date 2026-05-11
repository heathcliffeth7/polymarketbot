import type { TradeFlowGraph } from '@/lib/types';

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

export function exportGraphAsJson(graph: TradeFlowGraph, filename?: string): void {
  const json = JSON.stringify(graph, null, 2);
  const blob = new Blob([json], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename ?? `trade-flow-${Date.now()}.json`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

export function importGraphFromFile(): Promise<TradeFlowGraph> {
  return new Promise((resolve, reject) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json,application/json';
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) {
        reject(new Error('Dosya secilmedi.'));
        return;
      }
      try {
        const text = await file.text();
        const parsed = JSON.parse(text) as unknown;
        if (!isRecord(parsed)) throw new Error('JSON nesne olmali.');
        if (!Array.isArray(parsed.nodes)) throw new Error('nodes dizisi eksik.');
        if (!Array.isArray(parsed.edges)) throw new Error('edges dizisi eksik.');
        resolve(parsed as unknown as TradeFlowGraph);
      } catch (err) {
        reject(err instanceof Error ? err : new Error('JSON parse hatasi.'));
      }
    };
    input.click();
  });
}
