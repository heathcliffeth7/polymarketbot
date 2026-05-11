import type { AutoScopeTradeAnalysisSummary } from '@/lib/types';
import { formatPnl, formatUsdc, pnlClassName } from './auto-scope-diagnostics-utils';

interface AutoScopeSummaryCardsProps {
  summary: AutoScopeTradeAnalysisSummary;
  pagePnl: number;
}

function MetricTile({
  label,
  value,
  className = 'text-zinc-200',
}: {
  label: string;
  value: string;
  className?: string;
}) {
  return (
    <div className="rounded-md border border-zinc-800 bg-zinc-950 px-3 py-2">
      <p className="text-[11px] uppercase tracking-normal text-zinc-500">{label}</p>
      <p className={`mt-1 font-mono text-sm ${className}`}>{value}</p>
    </div>
  );
}

function formatOptionalPnl(value: number | null | undefined): string {
  return value == null ? '-' : formatPnl(value);
}

function optionalPnlClassName(value: number | null | undefined): string {
  return value == null ? 'text-zinc-300' : pnlClassName(value);
}

function formatUnsignedPercent(value: number | null): string {
  return value == null ? '-' : `${value.toFixed(2)}%`;
}

export function buildAutoScopeSummaryCardMetrics(
  summary: AutoScopeTradeAnalysisSummary,
  pagePnl: number
): Array<{ label: string; value: string; className?: string }> {
  const pendingInventoryRedeem =
    summary.pendingInventoryValueUsdc == null && summary.pendingRedeemableValueUsdc == null
      ? null
      : (summary.pendingInventoryValueUsdc ?? 0) + (summary.pendingRedeemableValueUsdc ?? 0);

  return [
    {
      label: 'Activity Cash PnL',
      value: formatPnl(summary.totalPnlUsdc),
      className: pnlClassName(summary.totalPnlUsdc),
    },
    {
      label: 'Polymarket Reference PnL',
      value: formatOptionalPnl(summary.referencePnlUsdc),
      className: optionalPnlClassName(summary.referencePnlUsdc),
    },
    {
      label: 'Diagnostic PnL',
      value: formatOptionalPnl(summary.diagnosticPnlUsdc ?? summary.rootRowsPnlUsdc),
      className: optionalPnlClassName(summary.diagnosticPnlUsdc ?? summary.rootRowsPnlUsdc),
    },
    {
      label: 'Pending Inventory / Redeem',
      value: formatOptionalPnl(pendingInventoryRedeem),
      className: optionalPnlClassName(pendingInventoryRedeem),
    },
    {
      label: 'Profit Factor',
      value: summary.profitFactor == null ? '-' : summary.profitFactor.toFixed(2),
    },
    { label: 'Win Rate', value: formatUnsignedPercent(summary.winRatePct) },
    { label: 'Avg Win', value: formatUsdc(summary.avgWinUsdc), className: 'text-emerald-300' },
    { label: 'Avg Loss', value: formatUsdc(summary.avgLossUsdc), className: 'text-red-300' },
    {
      label: 'Largest Loss',
      value: formatUsdc(summary.largestLossUsdc),
      className: 'text-red-300',
    },
    { label: 'Fee Drag', value: formatUsdc(summary.feeDragUsdc) },
    {
      label: 'Gorunen Satirlar Diagnostic',
      value: formatPnl(pagePnl),
      className: pnlClassName(pagePnl),
    },
  ];
}

export function AutoScopeSummaryCards({
  summary,
  pagePnl,
}: AutoScopeSummaryCardsProps) {
  const metrics = buildAutoScopeSummaryCardMetrics(summary, pagePnl);
  return (
    <div className="space-y-3">
      <div className="grid gap-2 text-xs text-zinc-300 sm:grid-cols-2 lg:grid-cols-5">
        {metrics.map((metric) => (
          <MetricTile
            key={metric.label}
            label={metric.label}
            value={metric.value}
            className={metric.className}
          />
        ))}
      </div>

      <div className="rounded-md border border-zinc-800 bg-zinc-950 p-3">
        <div className="mb-2 flex items-center justify-between gap-3">
          <p className="text-xs font-medium text-zinc-200">Zarar Nedenleri</p>
          <p className="text-[11px] text-zinc-500">{summary.marketCount} market</p>
        </div>
        {summary.diagnosisBreakdown.length === 0 ? (
          <p className="text-xs text-zinc-500">Teshis verisi henuz yok.</p>
        ) : (
          <div className="grid gap-2 md:grid-cols-2 xl:grid-cols-3">
            {summary.diagnosisBreakdown.map((item) => (
              <div
                key={item.code}
                className="flex items-center justify-between gap-3 rounded border border-zinc-800 bg-zinc-900/70 px-3 py-2"
              >
                <div className="min-w-0">
                  <p className="truncate text-xs font-medium text-zinc-200">{item.label}</p>
                  <p className="text-[11px] text-zinc-500">{item.count} trade</p>
                </div>
                <p className={`shrink-0 font-mono text-xs ${pnlClassName(item.pnlUsdc)}`}>
                  {formatPnl(item.pnlUsdc)}
                </p>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
