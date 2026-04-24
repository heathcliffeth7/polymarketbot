'use client';

import { X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useTradeFlowAutoScopeTradeDiagnostic } from '@/hooks/use-trade-flow';
import {
  formatDuration,
  formatPercent,
  formatPnl,
  formatPrice,
  formatQty,
  formatScore,
  formatUsdc,
  pnlClassName,
} from './auto-scope-diagnostics-utils';

interface AutoScopeDiagnosticsPanelProps {
  rootOrderId: number | null;
  onClose: () => void;
}

function DetailMetric({
  label,
  value,
  className = 'text-zinc-100',
}: {
  label: string;
  value: string;
  className?: string;
}) {
  return (
    <div className="rounded-md border border-zinc-800 bg-zinc-950 px-3 py-2">
      <p className="text-[11px] uppercase tracking-normal text-zinc-500">{label}</p>
      <p className={`mt-1 font-mono text-xs ${className}`}>{value}</p>
    </div>
  );
}

function compactMetricValue(value: unknown): string {
  if (value == null) return '-';
  if (typeof value === 'number') return Number.isInteger(value) ? String(value) : value.toFixed(4);
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  return String(value);
}

export function AutoScopeDiagnosticsPanel({
  rootOrderId,
  onClose,
}: AutoScopeDiagnosticsPanelProps) {
  const { data, error, isLoading } = useTradeFlowAutoScopeTradeDiagnostic(rootOrderId);
  if (!rootOrderId) return null;

  const diagnostic = data?.diagnostic ?? null;
  const rows = data?.rows ?? [];

  return (
    <div className="rounded-md border border-zinc-800 bg-zinc-950 p-4">
      <div className="mb-4 flex items-start justify-between gap-3">
        <div>
          <p className="text-sm font-medium text-zinc-100">
            Trade #{rootOrderId} detayli teshis
          </p>
          <p className="mt-1 text-xs text-zinc-500">
            {diagnostic?.marketSlug ?? rows[0]?.marketSlug ?? '-'}
          </p>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="h-8 w-8 text-zinc-400 hover:text-zinc-100"
          onClick={onClose}
        >
          <X className="size-4" />
        </Button>
      </div>

      {error && (
        <div className="rounded-md border border-red-900 bg-red-950/30 p-3 text-sm text-red-300">
          Teshis detayi yuklenemedi.
        </div>
      )}

      {isLoading && !diagnostic ? (
        <div className="rounded-md border border-zinc-800 bg-zinc-900/60 p-3 text-sm text-zinc-500">
          Teshis yukleniyor...
        </div>
      ) : diagnostic ? (
        <div className="space-y-4">
          <div className="rounded-md border border-zinc-800 bg-zinc-900/60 p-3">
            <div className="flex flex-col gap-2 md:flex-row md:items-start md:justify-between">
              <div>
                <p className="text-xs font-medium text-zinc-100">
                  {diagnostic.diagnosisLabel}
                </p>
                <p className="mt-1 text-xs text-zinc-500">{diagnostic.diagnosisDetail}</p>
              </div>
              <div className={`font-mono text-sm ${pnlClassName(diagnostic.totalPnlUsdc)}`}>
                {formatPnl(diagnostic.totalPnlUsdc)}
              </div>
            </div>
            {diagnostic.secondaryDiagnosisCode && (
              <p className="mt-2 text-[11px] text-zinc-500">
                Ikincil sinyal: {diagnostic.secondaryDiagnosisCode}
              </p>
            )}
          </div>

          <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
            <DetailMetric label="Maliyet" value={formatUsdc(diagnostic.costBasisUsdc)} />
            <DetailMetric label="Net Deger" value={formatUsdc(diagnostic.netValueUsdc)} />
            <DetailMetric label="Fee Drag" value={formatUsdc(diagnostic.feeDragUsdc)} />
            <DetailMetric label="PnL %" value={formatPercent(diagnostic.pnlPct)} />
          </div>

          <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
            <DetailMetric label="Entry Ref" value={formatPrice(diagnostic.entryReferencePrice)} />
            <DetailMetric label="Entry Fill" value={formatPrice(diagnostic.entryFillPrice)} />
            <DetailMetric label="Entry Slippage" value={formatUsdc(diagnostic.entrySlippageUsdc)} />
            <DetailMetric label="Entry Score" value={formatScore(diagnostic.entryQualityScore)} />
            <DetailMetric label="Exit Price" value={formatPrice(diagnostic.exitPrice)} />
            <DetailMetric label="Best Hold" value={formatPrice(diagnostic.bestPriceDuringHold)} />
            <DetailMetric label="Worst Hold" value={formatPrice(diagnostic.worstPriceDuringHold)} />
            <DetailMetric label="Exit Score" value={formatScore(diagnostic.exitQualityScore)} />
          </div>

          <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-5">
            <DetailMetric label="Open > Trigger" value={formatDuration(diagnostic.openToTriggerMs)} />
            <DetailMetric label="Trigger > Submit" value={formatDuration(diagnostic.triggerToSubmitMs)} />
            <DetailMetric label="Submit > Fill" value={formatDuration(diagnostic.submitToFillMs)} />
            <DetailMetric label="Trigger > Fill" value={formatDuration(diagnostic.triggerToBuyFillMs)} />
            <DetailMetric label="Hold" value={formatDuration(diagnostic.holdMs)} />
          </div>

          <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
            <DetailMetric label="Max Favorable" value={formatUsdc(diagnostic.maxFavorableUsdc)} className="text-emerald-300" />
            <DetailMetric label="Max Adverse" value={formatUsdc(diagnostic.maxAdverseUsdc)} className="text-red-300" />
            <DetailMetric label="Gave Back" value={formatUsdc(diagnostic.gaveBackUsdc)} className="text-amber-300" />
            <DetailMetric label="Snapshot Age" value={formatDuration(diagnostic.snapshotAgeMs)} />
          </div>

          <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
            <DetailMetric label="Guard Eval" value={formatDuration(diagnostic.guardEvalMs)} />
            <DetailMetric label="Runtime Price" value={formatDuration(diagnostic.runtimePriceFetchMs)} />
            <DetailMetric label="Place HTTP" value={formatDuration(diagnostic.placeHttpMs)} />
            <DetailMetric
              label="Path Samples"
              value={compactMetricValue(diagnostic.compactMetrics.path_sample_count)}
            />
          </div>

          <div className="rounded-md border border-zinc-800 bg-zinc-900/60 p-3">
            <p className="mb-2 text-xs font-medium text-zinc-200">Satirlar</p>
            <div className="space-y-2">
              {rows.map((row) => (
                <div
                  key={row.rowId}
                  className="grid gap-2 rounded border border-zinc-800 bg-zinc-950 px-3 py-2 text-xs md:grid-cols-5"
                >
                  <span className="text-zinc-300">{row.rowType}</span>
                  <span className="font-mono text-zinc-400">{formatQty(row.rowQty)}</span>
                  <span className="font-mono text-zinc-400">{formatUsdc(row.costBasisUsdc)}</span>
                  <span className="font-mono text-zinc-400">{formatUsdc(row.netValueUsdc)}</span>
                  <span className={`font-mono ${pnlClassName(row.rowPnlUsdc)}`}>
                    {formatPnl(row.rowPnlUsdc)}
                  </span>
                </div>
              ))}
            </div>
          </div>

          {diagnostic.dataQualityFlags.length > 0 && (
            <div className="flex flex-wrap gap-2">
              {diagnostic.dataQualityFlags.map((flag) => (
                <span
                  key={flag}
                  className="rounded-full border border-zinc-700 bg-zinc-900 px-2 py-1 text-[11px] text-zinc-400"
                >
                  {flag}
                </span>
              ))}
            </div>
          )}
        </div>
      ) : (
        <div className="rounded-md border border-zinc-800 bg-zinc-900/60 p-3 text-sm text-zinc-500">
          Teshis kaydi bulunamadi.
        </div>
      )}
    </div>
  );
}
