'use client';

import { useMemo } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { useTradeFlowNodeRuntime, useTradeFlowRuns } from '@/hooks/use-trade-flow';

interface FlowNodeRuntimePanelProps {
  definitionId: number;
}

function formatNumber(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return '-';
  return value.toFixed(Math.abs(value) >= 100 ? 2 : 4);
}

function formatJson(value: unknown): string {
  try {
    return JSON.stringify(value ?? {}, null, 2);
  } catch {
    return '{}';
  }
}

export function FlowNodeRuntimePanel({ definitionId }: FlowNodeRuntimePanelProps) {
  const { data: runsData, error: runsError } = useTradeFlowRuns(1, 20, definitionId);
  const selectedRun = useMemo(() => {
    const runs = runsData?.data ?? [];
    return runs.find((run) => run.status === 'running') ?? runs[0] ?? null;
  }, [runsData?.data]);

  const {
    data: runtimeData,
    error: runtimeError,
  } = useTradeFlowNodeRuntime(selectedRun?.id ?? null, 1, 20, undefined, undefined, Boolean(selectedRun));

  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader>
        <CardTitle className="text-sm font-medium text-zinc-300">Node Runtime Telemetry</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {!selectedRun && !runsError && (
          <p className="text-xs text-zinc-500">Bu flow icin henüz run yok.</p>
        )}
        {runsError && (
          <p className="text-xs text-red-400">Run listesi yuklenemedi.</p>
        )}
        {selectedRun && (
          <div className="rounded-md border border-zinc-800 bg-zinc-950/50 p-3 text-xs text-zinc-400">
            <p>
              Secili run: <span className="text-zinc-200">#{selectedRun.id}</span> ({selectedRun.status})
            </p>
            <p className="mt-1">
              Runtime snapshot sayisi:{' '}
              <span className="text-zinc-200">{runtimeData?.total ?? 0}</span>
            </p>
          </div>
        )}
        {selectedRun && !runtimeData && !runtimeError && (
          <p className="text-xs text-zinc-500">Node runtime verisi yukleniyor...</p>
        )}
        {runtimeError && (
          <p className="text-xs text-red-400">Node runtime analytics yuklenemedi.</p>
        )}
        {runtimeData && runtimeData.data.length === 0 && (
          <p className="text-xs text-zinc-500">Bu run icin henüz generic node runtime snapshot kaydı yok.</p>
        )}
        {runtimeData && runtimeData.data.length > 0 && (
          <div className="space-y-3">
            {runtimeData.data.map((row) => {
              const snapshot = row.snapshotJson;
              const output =
                snapshot && typeof snapshot === 'object' && 'output' in snapshot
                  ? (snapshot.output as Record<string, unknown> | null)
                  : null;
              const guard = output?.price_to_beat_guard as Record<string, unknown> | undefined;
              const runtimeMarketSlug =
                row.marketSlug ??
                (typeof output?.market_slug === 'string' ? output.market_slug : null);
              const runtimeTokenId =
                row.tokenId ??
                (typeof output?.token_id === 'string' ? output.token_id : null);
              const thresholdUsd =
                typeof guard?.threshold_usd === 'number' ? guard.threshold_usd : null;
              const baseThresholdUsd =
                typeof guard?.base_threshold_usd === 'number' ? guard.base_threshold_usd : null;
              const bumpUsd =
                typeof guard?.stop_loss_bump_usd === 'number' ? guard.stop_loss_bump_usd : null;
              const bumpIncrementUsd =
                typeof guard?.stop_loss_bump_increment_usd === 'number'
                  ? guard.stop_loss_bump_increment_usd
                  : null;
              const maxPriceRelax = output?.price_to_beat_guard &&
                typeof output.price_to_beat_guard === 'object'
                ? ((output.price_to_beat_guard as Record<string, unknown>).max_price_relax as Record<string, unknown> | undefined)
                : undefined;
              const missReason =
                typeof maxPriceRelax?.max_price_relax_miss_reason === 'string'
                  ? maxPriceRelax.max_price_relax_miss_reason
                  : null;
              const tradableSecondsCount =
                typeof maxPriceRelax?.max_price_relax_tradable_seconds_count === 'number'
                  ? maxPriceRelax.max_price_relax_tradable_seconds_count
                  : null;
              const qualityScore =
                typeof maxPriceRelax?.max_price_relax_quality_score === 'number'
                  ? maxPriceRelax.max_price_relax_quality_score
                  : null;

              return (
                <div
                  key={`${row.nodeKey}:${row.marketSlug ?? ''}:${row.tokenId ?? ''}`}
                  className="rounded-lg border border-zinc-800 bg-zinc-950/50 p-3"
                >
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="rounded border border-zinc-700 px-2 py-0.5 font-mono text-[11px] text-zinc-200">
                      {row.nodeKey}
                    </span>
                    <span className="rounded border border-zinc-700 px-2 py-0.5 text-[11px] text-zinc-400">
                      {row.nodeType}
                    </span>
                    <span className="rounded border border-zinc-700 px-2 py-0.5 text-[11px] text-zinc-400">
                      {row.stateKind}
                    </span>
                    <span className="text-[11px] text-zinc-500">{row.updatedAt}</span>
                  </div>

                  <div className="mt-3 grid gap-2 md:grid-cols-2 xl:grid-cols-4">
                    <div className="rounded-md border border-zinc-800 bg-zinc-900/60 p-2">
                      <p className="text-[11px] text-zinc-500">Market</p>
                      <p className="mt-1 text-[11px] text-zinc-200">{runtimeMarketSlug ?? '-'}</p>
                    </div>
                    <div className="rounded-md border border-zinc-800 bg-zinc-900/60 p-2">
                      <p className="text-[11px] text-zinc-500">Token</p>
                      <p className="mt-1 break-all text-[11px] text-zinc-200">{runtimeTokenId ?? '-'}</p>
                    </div>
                    <div className="rounded-md border border-zinc-800 bg-zinc-900/60 p-2">
                      <p className="text-[11px] text-zinc-500">Base / Bump</p>
                      <p className="mt-1 text-[11px] text-zinc-200">
                        {formatNumber(baseThresholdUsd)} / {formatNumber(bumpUsd)}
                      </p>
                    </div>
                    <div className="rounded-md border border-zinc-800 bg-zinc-900/60 p-2">
                      <p className="text-[11px] text-zinc-500">Effective Threshold</p>
                      <p className="mt-1 text-[11px] text-zinc-200">{formatNumber(thresholdUsd)}</p>
                    </div>
                    <div className="rounded-md border border-zinc-800 bg-zinc-900/60 p-2">
                      <p className="text-[11px] text-zinc-500">Miss Reason</p>
                      <p className="mt-1 text-[11px] text-zinc-200">{missReason ?? '-'}</p>
                    </div>
                    <div className="rounded-md border border-zinc-800 bg-zinc-900/60 p-2">
                      <p className="text-[11px] text-zinc-500">Tradable Sec / Quality</p>
                      <p className="mt-1 text-[11px] text-zinc-200">
                        {tradableSecondsCount ?? '-'} / {formatNumber(qualityScore)}
                      </p>
                    </div>
                    <div className="rounded-md border border-zinc-800 bg-zinc-900/60 p-2">
                      <p className="text-[11px] text-zinc-500">Bump Increment</p>
                      <p className="mt-1 text-[11px] text-zinc-200">
                        {formatNumber(bumpIncrementUsd)}
                      </p>
                    </div>
                  </div>

                  <details className="mt-3 rounded-md border border-zinc-800 bg-zinc-900/40 p-2">
                    <summary className="cursor-pointer text-[11px] text-zinc-300">
                      Raw Snapshot JSON
                    </summary>
                    <pre className="mt-2 overflow-x-auto text-[11px] text-zinc-400">
                      {formatJson(row.snapshotJson)}
                    </pre>
                  </details>
                </div>
              );
            })}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
