import type { AutoScopeTradeAnalysisSummary } from '@/lib/types';
import { formatPercent, formatPnl, formatUsdc, pnlClassName } from './auto-scope-diagnostics-utils';

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

export function AutoScopeSummaryCards({
  summary,
  pagePnl,
}: AutoScopeSummaryCardsProps) {
  const isActivityWindow = summary.pnlSource === 'official_activity_window';
  const officialCashIn =
    summary.officialSellUsdc == null && summary.officialRedeemUsdc == null
      ? null
      : (summary.officialSellUsdc ?? 0) + (summary.officialRedeemUsdc ?? 0);
  const pendingInventoryRedeem =
    summary.pendingInventoryValueUsdc == null && summary.pendingRedeemableValueUsdc == null
      ? null
      : (summary.pendingInventoryValueUsdc ?? 0) + (summary.pendingRedeemableValueUsdc ?? 0);
  return (
    <div className="space-y-3">
      <div className="grid gap-2 text-xs text-zinc-300 sm:grid-cols-2 lg:grid-cols-5">
        <MetricTile
          label="Polymarket Wallet PnL"
          value={formatPnl(summary.totalPnlUsdc)}
          className={pnlClassName(summary.totalPnlUsdc)}
        />
        <MetricTile
          label="Activity Cash PnL"
          value={formatOptionalPnl(summary.localCashFillPnlUsdc)}
          className={optionalPnlClassName(summary.localCashFillPnlUsdc)}
        />
        <MetricTile
          label="Diagnostic PnL"
          value={formatOptionalPnl(summary.diagnosticPnlUsdc ?? summary.rootRowsPnlUsdc)}
          className={optionalPnlClassName(summary.diagnosticPnlUsdc ?? summary.rootRowsPnlUsdc)}
        />
        <MetricTile
          label="Pending Inventory / Redeem"
          value={formatOptionalPnl(pendingInventoryRedeem)}
          className={optionalPnlClassName(pendingInventoryRedeem)}
        />
        <MetricTile label="Profit Factor" value={summary.profitFactor == null ? '-' : summary.profitFactor.toFixed(2)} />
        <MetricTile label="Win Rate" value={formatPercent(summary.winRatePct)} />
        <MetricTile label="Avg Win" value={formatUsdc(summary.avgWinUsdc)} className="text-emerald-300" />
        <MetricTile label="Avg Loss" value={formatUsdc(summary.avgLossUsdc)} className="text-red-300" />
        <MetricTile label="Largest Loss" value={formatUsdc(summary.largestLossUsdc)} className="text-red-300" />
        <MetricTile
          label={isActivityWindow ? 'Activity In/Out' : 'Fee Drag'}
          value={
            isActivityWindow
              ? `${formatUsdc(officialCashIn)} / ${formatUsdc(summary.officialBuyUsdc ?? null)}`
              : formatUsdc(summary.feeDragUsdc)
          }
        />
        <MetricTile label="Gorunen Satirlar Diagnostic" value={formatPnl(pagePnl)} className={pnlClassName(pagePnl)} />
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
