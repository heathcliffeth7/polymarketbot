'use client';

import { useMemo, useState } from 'react';
import { Download } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import {
  buildTradeFlowAutoScopeAnalysisQuery,
  useTradeFlowAutoScopeAnalysis,
} from '@/hooks/use-trade-flow';
import { AutoScopeDiagnosticsPanel } from './auto-scope-diagnostics-panel';
import { AutoScopeSummaryCards } from './auto-scope-summary-cards';
import type {
  AutoScopeTradeAnalysisPnlFilter,
  AutoScopeTradeAnalysisPositionFilter,
  AutoScopeTradeAnalysisRow,
  AutoScopeTradeAnalysisSortBy,
  AutoScopeTradeAnalysisSortDirection,
  AutoScopeTradeAnalysisSummary,
  AutoScopeTradeAnalysisTimeRange,
} from '@/lib/types';

const PAGE_SIZE = 50;
type SortOption = 'default' | 'pnl_asc' | 'pnl_desc';

const EMPTY_SUMMARY: AutoScopeTradeAnalysisSummary = {
  rowCount: 0,
  marketCount: 0,
  lossCount: 0,
  profitCount: 0,
  totalPnlUsdc: 0,
  realizedPnlUsdc: 0,
  openPnlUsdc: 0,
  lossUsdc: 0,
  profitUsdc: 0,
  buyFeeUsdc: 0,
  sellFeeUsdc: 0,
  totalFeeUsdc: 0,
  costBasisUsdc: 0,
  netValueUsdc: 0,
  profitFactor: null,
  winRatePct: null,
  avgWinUsdc: null,
  avgLossUsdc: null,
  largestLossUsdc: null,
  feeDragUsdc: 0,
  diagnosisBreakdown: [],
};

function formatDateTime(value: string | null): string {
  if (!value) return '-';
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return '-';
  return parsed.toLocaleString();
}

function formatDuration(ms: number | null): string {
  if (ms == null) return '-';
  if (ms < 1000) return `${ms} ms`;

  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) return `${hours}sa ${minutes}d ${seconds}sn`;
  if (minutes > 0) return `${minutes}d ${seconds}sn`;
  return `${seconds}sn`;
}

function dateStartIso(value: string): string | undefined {
  return value ? `${value}T00:00:00.000Z` : undefined;
}

function dateEndIso(value: string): string | undefined {
  return value ? `${value}T23:59:59.999Z` : undefined;
}

function formatPrice(value: number | null): string {
  return value == null ? '-' : value.toFixed(4);
}

function formatQty(value: number): string {
  return value.toFixed(4);
}

function formatUsdc(value: number | null): string {
  return value == null ? '-' : `${value.toFixed(2)} USDC`;
}

function formatPnl(value: number): string {
  const sign = value > 0 ? '+' : '';
  return `${sign}${value.toFixed(2)} USDC`;
}

function formatNullablePnl(value: number | null): string {
  return value == null ? '-' : formatPnl(value);
}

function nullablePnlClassName(value: number | null): string {
  return value == null ? 'text-zinc-300' : pnlClassName(value);
}

function formatPercent(value: number | null): string {
  if (value == null) return '-';
  const sign = value > 0 ? '+' : '';
  return `${sign}${value.toFixed(2)}%`;
}

function formatNullableQty(value: number | null): string {
  return value == null ? '-' : formatQty(value);
}

function cashStatusLabel(value: string | null): string {
  switch (value) {
    case 'closed_cash_observed':
      return 'Cash closed';
    case 'pending_inventory_or_redeem':
      return 'Pending inventory/redeem';
    case 'buy_without_sell_or_redeem':
      return 'Buy only';
    case 'redeem_ambiguous':
      return 'Redeem ambiguous';
    case 'no_fill_cash':
      return 'No fill cash';
    case 'lost_unclaimed_or_unredeemed':
      return 'Lost / no claim';
    case 'pending_analysis':
      return 'Pending analysis';
    default:
      return '-';
  }
}

function pnlSourceStatusLabel(
  value: AutoScopeTradeAnalysisRow['pnlSourceStatus']
): string | null {
  switch (value) {
    case 'activity_market':
      return 'Activity market';
    case 'local_fallback':
      return 'Local fallback';
    case 'local_fallback_no_activity_evidence':
      return 'No activity evidence';
    case 'pnl_source_mismatch':
      return 'PnL source mismatch';
    default:
      return null;
  }
}

function pnlSourceStatusClassName(
  value: AutoScopeTradeAnalysisRow['pnlSourceStatus']
): string {
  if (value === 'pnl_source_mismatch') return 'text-red-300';
  if (value === 'local_fallback_no_activity_evidence') return 'text-amber-300';
  if (value === 'activity_market') return 'text-emerald-300';
  return 'text-zinc-500';
}

function formatScore(value: number | null): string {
  return value == null ? '-' : `${value.toFixed(0)}/100`;
}

function exitReasonLabel(row: AutoScopeTradeAnalysisRow): string {
  if (row.positionState === 'pending_analysis') return 'Pending Analysis';
  if (row.rowType === 'settled_payout') return 'Settlement';
  if (row.positionState === 'closed_market_ended') return 'Kapali (Market End)';
  if (row.positionState === 'open') return 'Acik Pozisyon';
  switch (row.exitReason) {
    case 'tp':
      return 'TP';
    case 'sl':
      return 'SL';
    case 'window_end_auto_sell':
      return 'Window End';
    default:
      return 'Diger';
  }
}

function exitMetaLabel(row: AutoScopeTradeAnalysisRow): string {
  if (row.positionState === 'pending_analysis') {
    return row.buyFilledAt
      ? `Fill geldi: ${formatDateTime(row.buyFilledAt)}`
      : 'Forensic refresh bekleniyor';
  }
  if (row.rowType === 'settled_payout') {
    return row.sellFilledAt
      ? `Redeem: ${formatDateTime(row.sellFilledAt)}`
      : 'Polymarket redeem';
  }
  if (row.positionState === 'closed_market_ended') {
    return row.marketEndAt
      ? `Market bitti: ${formatDateTime(row.marketEndAt)}`
      : 'Market bitti';
  }
  if (row.positionState === 'open') {
    return row.markPriceCapturedAt
      ? `Mark: ${formatDateTime(row.markPriceCapturedAt)}`
      : 'Acik pozisyon devam ediyor';
  }
  return row.sellFilledAt ? formatDateTime(row.sellFilledAt) : 'Kapandi';
}

function pnlClassName(value: number): string {
  if (value > 0) return 'text-emerald-400';
  if (value < 0) return 'text-red-400';
  return 'text-zinc-300';
}

function diagnosisBadgeClassName(row: AutoScopeTradeAnalysisRow): string {
  const code = row.primaryDiagnosisCode;
  if (code === 'clean_win' || code === 'take_profit_success') {
    return 'border-emerald-800 bg-emerald-950/40 text-emerald-300';
  }
  if (code === 'unknown' || !code) {
    return 'border-zinc-700 bg-zinc-950 text-zinc-300';
  }
  if (row.rowPnlUsdc < 0) {
    return 'border-red-900 bg-red-950/40 text-red-300';
  }
  return 'border-amber-900 bg-amber-950/40 text-amber-300';
}

function entryPayloadValue(row: AutoScopeTradeAnalysisRow, path: string[]): unknown {
  let current: unknown = row.forensic?.entryDecision ?? null;
  for (const key of path) {
    if (!current || typeof current !== 'object' || Array.isArray(current)) return null;
    current = (current as Record<string, unknown>)[key];
  }
  return current ?? null;
}

function compactForensicValue(value: unknown): string {
  if (value == null) return '-';
  if (typeof value === 'number') return Number.isInteger(value) ? String(value) : value.toFixed(3);
  if (typeof value === 'boolean') return value ? 'YES' : 'NO';
  if (Array.isArray(value)) return value.length > 0 ? value.join(', ') : 'none';
  return String(value);
}

function shortConfigHash(value: string | null | undefined): string {
  if (!value) return '-';
  return value.startsWith('sha256:') ? value.slice(7, 19) : value.slice(0, 12);
}

function filterSelectClassName(): string {
  return 'h-8 rounded-md border border-zinc-700 bg-zinc-950 px-3 text-xs text-zinc-200';
}

export function AutoScopeAnalysisTable() {
  const [page, setPage] = useState(1);
  const [sortOption, setSortOption] = useState<SortOption>('default');
  const [pnlFilter, setPnlFilter] = useState<AutoScopeTradeAnalysisPnlFilter>('all');
  const [positionFilter, setPositionFilter] =
    useState<AutoScopeTradeAnalysisPositionFilter>('all');
  const [timeRange, setTimeRange] = useState<AutoScopeTradeAnalysisTimeRange>('all');
  const [fromDate, setFromDate] = useState('');
  const [toDate, setToDate] = useState('');
  const [selectedRootOrderId, setSelectedRootOrderId] = useState<number | null>(null);

  const sortBy: AutoScopeTradeAnalysisSortBy =
    sortOption === 'default' ? 'default' : 'pnl';
  const sortDirection: AutoScopeTradeAnalysisSortDirection =
    sortOption === 'pnl_asc' ? 'asc' : 'desc';
  const isCustomTimeRange = timeRange === 'custom';
  const from = isCustomTimeRange ? dateStartIso(fromDate) : undefined;
  const to = isCustomTimeRange ? dateEndIso(toDate) : undefined;

  const { data, error, isLoading } = useTradeFlowAutoScopeAnalysis({
    page,
    limit: PAGE_SIZE,
    sortBy,
    sortDirection,
    pnl: pnlFilter,
    position: positionFilter,
    timeRange,
    from,
    to,
  });

  const rows = useMemo(() => data?.data ?? [], [data?.data]);
  const total = data?.total ?? 0;
  const totalPages = data?.totalPages ?? 0;
  const refreshedAt = data?.refreshedAt ?? null;
  const summary = data?.summary ?? EMPTY_SUMMARY;
  const pagePnl = useMemo(
    () => rows.reduce((sum, row) => sum + row.rowPnlUsdc, 0),
    [rows]
  );
  const exportQuery = buildTradeFlowAutoScopeAnalysisQuery({
    sortBy,
    sortDirection,
    pnl: pnlFilter,
    position: positionFilter,
    timeRange,
    from,
    to,
  });

  function resetPage() {
    setPage(1);
  }

  function handleTimeRangeChange(nextTimeRange: AutoScopeTradeAnalysisTimeRange) {
    setTimeRange(nextTimeRange);
    if (nextTimeRange !== 'custom') {
      setFromDate('');
      setToDate('');
    }
    resetPage();
  }

  function exportCsv() {
    const a = document.createElement('a');
    a.href = `/api/trade-flow/analytics/auto-scope/export?${exportQuery}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
  }

  function exportForensicCsv() {
    const a = document.createElement('a');
    a.href = `/api/trade-flow/analytics/auto-scope/export/full?${exportQuery}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
  }

  function exportDecisionLogsCsv() {
    const a = document.createElement('a');
    a.href = `/api/trade-flow/analytics/auto-scope/decision-logs/export?${exportQuery}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
  }

  function exportNoOrderCsv() {
    const a = document.createElement('a');
    a.href = `/api/trade-flow/analytics/auto-scope/no-order/export?${exportQuery}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
  }

  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader className="space-y-4">
        <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
          <div>
            <CardTitle className="text-sm font-medium text-zinc-200">
              Auto-Scope Trade Analizi
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              Realized ve acik mark PnL kirilimi
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <div className="text-xs text-zinc-500">
              Son yenileme: {formatDateTime(refreshedAt)}
            </div>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="border-zinc-700 text-zinc-200"
              disabled={total === 0}
              onClick={exportCsv}
            >
              <Download className="size-4" />
              CSV
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="border-zinc-700 text-zinc-200"
              disabled={total === 0}
              onClick={exportForensicCsv}
            >
              <Download className="size-4" />
              Full CSV
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="border-zinc-700 text-zinc-200"
              onClick={exportDecisionLogsCsv}
            >
              <Download className="size-4" />
              Raw Logs
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="border-zinc-700 text-zinc-200"
              onClick={exportNoOrderCsv}
            >
              <Download className="size-4" />
              No-Order CSV
            </Button>
          </div>
        </div>

        <AutoScopeSummaryCards summary={summary} pagePnl={pagePnl} />

        <div className="grid gap-2 md:grid-cols-6">
          <label className="space-y-1 text-xs text-zinc-400">
            <span>Zaman</span>
            <select
              value={timeRange}
              onChange={(event) =>
                handleTimeRangeChange(event.target.value as AutoScopeTradeAnalysisTimeRange)
              }
              className={filterSelectClassName()}
            >
              <option value="all">Tum zamanlar</option>
              <option value="3h">Son 3 saat</option>
              <option value="6h">Son 6 saat</option>
              <option value="12h">Son 12 saat</option>
              <option value="24h">Son 24 saat</option>
              <option value="1w">Son 1 hafta</option>
              <option value="1m">Son 1 ay</option>
              <option value="custom">Ozel tarih</option>
            </select>
          </label>
          <label className="space-y-1 text-xs text-zinc-400">
            <span>PnL</span>
            <select
              value={pnlFilter}
              onChange={(event) => {
                setPnlFilter(event.target.value as AutoScopeTradeAnalysisPnlFilter);
                resetPage();
              }}
              className={filterSelectClassName()}
            >
              <option value="all">Tum satirlar</option>
              <option value="loss">Sadece zarar</option>
              <option value="profit">Sadece kar</option>
            </select>
          </label>
          <label className="space-y-1 text-xs text-zinc-400">
            <span>Pozisyon</span>
            <select
              value={positionFilter}
              onChange={(event) => {
                setPositionFilter(event.target.value as AutoScopeTradeAnalysisPositionFilter);
                resetPage();
              }}
              className={filterSelectClassName()}
            >
              <option value="all">Tum pozisyonlar</option>
              <option value="realized">Realized</option>
              <option value="open">Acik/Mark</option>
            </select>
          </label>
          <label className="space-y-1 text-xs text-zinc-400">
            <span>Baslangic</span>
            <Input
              type="date"
              value={fromDate}
              disabled={!isCustomTimeRange}
              onChange={(event) => {
                setFromDate(event.target.value);
                resetPage();
              }}
              className="h-8 border-zinc-700 bg-zinc-950 text-xs text-zinc-200"
            />
          </label>
          <label className="space-y-1 text-xs text-zinc-400">
            <span>Bitis</span>
            <Input
              type="date"
              value={toDate}
              disabled={!isCustomTimeRange}
              onChange={(event) => {
                setToDate(event.target.value);
                resetPage();
              }}
              className="h-8 border-zinc-700 bg-zinc-950 text-xs text-zinc-200"
            />
          </label>
          <label className="space-y-1 text-xs text-zinc-400">
            <span>Siralama</span>
            <select
              value={sortOption}
              onChange={(event) => {
                setSortOption(event.target.value as SortOption);
                resetPage();
              }}
              className={filterSelectClassName()}
            >
              <option value="default">Islem zamani</option>
              <option value="pnl_asc">PnL artan</option>
              <option value="pnl_desc">PnL azalan</option>
            </select>
          </label>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {error && (
          <div className="rounded-md border border-red-900 bg-red-950/30 p-3 text-sm text-red-300">
            Analiz verisi yuklenemedi.
          </div>
        )}

        <div className="overflow-x-auto">
          <Table className="min-w-[2320px] text-xs text-zinc-200">
            <TableHeader>
              <TableRow className="border-zinc-800 hover:bg-transparent">
                <TableHead className="text-zinc-400">Workflow</TableHead>
                <TableHead className="text-zinc-400">Node</TableHead>
                <TableHead className="text-zinc-400">Config Hash</TableHead>
                <TableHead className="text-zinc-400">PTB Trend</TableHead>
                <TableHead className="text-zinc-400">Volume</TableHead>
                <TableHead className="text-zinc-400">ShadowGuard</TableHead>
                <TableHead className="text-zinc-400">Risk Tags</TableHead>
                <TableHead className="text-zinc-400">Market</TableHead>
                <TableHead className="text-zinc-400">Exit</TableHead>
                <TableHead className="text-zinc-400">Teshis</TableHead>
                <TableHead className="text-zinc-400">Trigger</TableHead>
                <TableHead className="text-zinc-400">Gecikme</TableHead>
                <TableHead className="text-right text-zinc-400">Buy</TableHead>
                <TableHead className="text-right text-zinc-400">Sell/Canli</TableHead>
                <TableHead className="text-right text-zinc-400">Qty</TableHead>
                <TableHead className="text-right text-zinc-400">Maliyet</TableHead>
                <TableHead className="text-right text-zinc-400">Fee</TableHead>
                <TableHead className="text-right text-zinc-400">Net</TableHead>
                <TableHead className="text-right text-zinc-400">Activity Cash PnL</TableHead>
                <TableHead className="text-right text-zinc-400">Polymarket Position PnL</TableHead>
                <TableHead className="text-right text-zinc-400">Buy Cash</TableHead>
                <TableHead className="text-right text-zinc-400">Sell Cash</TableHead>
                <TableHead className="text-right text-zinc-400">Redeem</TableHead>
                <TableHead className="text-right text-zinc-400">Remaining Qty</TableHead>
                <TableHead className="text-zinc-400">Cash Status</TableHead>
                <TableHead className="text-right text-zinc-400">Diagnostic PnL</TableHead>
                <TableHead className="text-right text-zinc-400">PnL %</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {isLoading && rows.length === 0 ? (
                <TableRow className="border-zinc-800">
                  <TableCell colSpan={27} className="py-10 text-center text-zinc-500">
                    Analiz verisi yukleniyor...
                  </TableCell>
                </TableRow>
              ) : rows.length === 0 ? (
                <TableRow className="border-zinc-800">
                  <TableCell colSpan={27} className="py-10 text-center text-zinc-500">
                    Gosterilecek auto-scope trade analizi bulunamadi.
                  </TableCell>
                </TableRow>
              ) : (
                rows.map((row) => (
                  <TableRow
                    key={row.rowId}
                    className={`border-zinc-800 hover:bg-zinc-950/60 ${
                      selectedRootOrderId === row.rootOrderId ? 'bg-zinc-950/80' : ''
                    }`}
                  >
                    <TableCell>
                      <div className="space-y-1">
                        <p className="font-medium text-zinc-100">
                          {row.definitionName || `Flow #${row.definitionId}`}
                        </p>
                        <p className="text-[11px] text-zinc-500">
                          Run #{row.runId} / Root #{row.rootOrderId}
                        </p>
                      </div>
                    </TableCell>
                    <TableCell>
                      <div className="max-w-[150px] space-y-1">
                        <p className="truncate font-mono text-zinc-100">
                          {row.forensic?.entryNodeKey ?? '-'}
                        </p>
                        <p className="truncate text-[11px] text-zinc-500">
                          {row.forensic?.entryNodeType ?? '-'}
                        </p>
                      </div>
                    </TableCell>
                    <TableCell className="font-mono text-[11px] text-zinc-400">
                      {shortConfigHash(row.forensic?.entryNodeConfigHash)}
                    </TableCell>
                    <TableCell>
                      <div className="space-y-1">
                        <p className="text-zinc-100">
                          {compactForensicValue(entryPayloadValue(row, ['ptb', 'trend']))}
                        </p>
                        <p className="font-mono text-[11px] text-zinc-500">
                          slope {compactForensicValue(entryPayloadValue(row, ['ptb', 'slope_5s']))}
                        </p>
                      </div>
                    </TableCell>
                    <TableCell>
                      <div className="space-y-1">
                        <p className="text-zinc-100">
                          {compactForensicValue(
                            entryPayloadValue(row, ['volume', 'polymarket', 'regime'])
                          )}
                        </p>
                        <p className="font-mono text-[11px] text-zinc-500">
                          {compactForensicValue(
                            entryPayloadValue(row, ['volume', 'polymarket', 'ratio'])
                          )}
                          x
                        </p>
                      </div>
                    </TableCell>
                    <TableCell>
                      <div className="space-y-1">
                        <p
                          className={
                            entryPayloadValue(row, [
                              'guard_breakdown',
                              'shadow_volume_guard',
                              'would_block',
                            ]) === true
                              ? 'text-red-300'
                              : 'text-emerald-300'
                          }
                        >
                          {compactForensicValue(
                            entryPayloadValue(row, [
                              'guard_breakdown',
                              'shadow_volume_guard',
                              'would_block',
                            ])
                          )}
                        </p>
                        <p className="max-w-[140px] truncate text-[11px] text-zinc-500">
                          {compactForensicValue(
                            entryPayloadValue(row, [
                              'guard_breakdown',
                              'shadow_volume_guard',
                              'reason',
                            ])
                          )}
                        </p>
                      </div>
                    </TableCell>
                    <TableCell>
                      <p className="max-w-[180px] truncate text-[11px] text-zinc-400">
                        {compactForensicValue(entryPayloadValue(row, ['risk_tags']))}
                      </p>
                    </TableCell>
                    <TableCell>
                      <Popover
                        open={selectedRootOrderId === row.rootOrderId}
                        onOpenChange={(open) =>
                          setSelectedRootOrderId(open ? row.rootOrderId : null)
                        }
                      >
                        <PopoverTrigger asChild>
                          <button
                            type="button"
                            className="group -mx-2 block w-[300px] rounded-md px-2 py-1 text-left outline-none transition-colors hover:bg-zinc-900 focus-visible:ring-2 focus-visible:ring-sky-500/60"
                            aria-label={`Trade detayini ac: ${row.marketSlug}`}
                          >
                            <span className="block truncate font-medium text-zinc-100 group-hover:text-white">
                              {row.marketSlug}
                            </span>
                            <span className="block truncate text-[11px] text-zinc-500">
                              {row.outcomeLabel} / {row.tokenId.slice(0, 10)}...
                            </span>
                          </button>
                        </PopoverTrigger>
                        <PopoverContent
                          side="right"
                          align="start"
                          sideOffset={10}
                          collisionPadding={16}
                          className="max-h-[82vh] w-[min(1100px,calc(100vw-2rem))] overflow-y-auto border-zinc-800 bg-zinc-950 p-0 text-zinc-100 shadow-2xl shadow-black/40"
                        >
                          <AutoScopeDiagnosticsPanel
                            rootOrderId={row.rootOrderId}
                            onClose={() => setSelectedRootOrderId(null)}
                            className="border-0 bg-transparent p-4"
                          />
                        </PopoverContent>
                      </Popover>
                    </TableCell>
                    <TableCell>
                      <div className="space-y-1">
                        <span className="inline-flex rounded-full border border-zinc-700 bg-zinc-950 px-2 py-1 text-[11px] text-zinc-200">
                          {exitReasonLabel(row)}
                        </span>
                        <p className="text-[11px] text-zinc-500">{exitMetaLabel(row)}</p>
                      </div>
                    </TableCell>
                    <TableCell>
                      <div className="space-y-1">
                        <span
                          className={`inline-flex max-w-[170px] rounded-full border px-2 py-1 text-[11px] ${diagnosisBadgeClassName(row)}`}
                        >
                          <span className="truncate">
                            {row.diagnosisLabel ?? 'Teshis yok'}
                          </span>
                        </span>
                        <p className="text-[11px] text-zinc-500">
                          E {formatScore(row.entryQualityScore)} / X{' '}
                          {formatScore(row.exitQualityScore)}
                        </p>
                      </div>
                    </TableCell>
                    <TableCell>{formatDateTime(row.triggeredAt)}</TableCell>
                    <TableCell>
                      <div className="space-y-1">
                        <p>{formatDuration(row.openToTriggerMs)}</p>
                        <p className="text-[11px] text-zinc-500">
                          Buy: {formatDuration(row.triggerToBuyFillMs)}
                        </p>
                      </div>
                    </TableCell>
                    <TableCell className="text-right font-mono">
                      <div className="space-y-1">
                        <p>{formatPrice(row.buyAvgPrice)}</p>
                        <p className="text-[11px] text-zinc-500">
                          {formatUsdc(row.buyNotionalUsdc)}
                        </p>
                      </div>
                    </TableCell>
                    <TableCell className="text-right font-mono">
                      <div className="space-y-1">
                        <p>{formatPrice(row.sellOrLivePrice)}</p>
                        <p className="text-[11px] text-zinc-500">
                          {formatUsdc(row.sellNotionalUsdc ?? row.markValueUsdc)}
                        </p>
                      </div>
                    </TableCell>
                    <TableCell className="text-right font-mono">
                      <div className="space-y-1">
                        <p>{formatQty(row.rowQty)}</p>
                        <p className="text-[11px] text-zinc-500">{row.rowType}</p>
                      </div>
                    </TableCell>
                    <TableCell className="text-right font-mono">
                      {formatUsdc(row.costBasisUsdc)}
                    </TableCell>
                    <TableCell className="text-right font-mono">
                      <div className="space-y-1">
                        <p>{formatUsdc(row.buyFeeUsdc)}</p>
                        <p className="text-[11px] text-zinc-500">
                          Sell {formatUsdc(row.sellFeeUsdc)}
                        </p>
                      </div>
                    </TableCell>
                    <TableCell className="text-right font-mono">
                      {formatUsdc(row.netValueUsdc)}
                    </TableCell>
                    <TableCell className={`text-right font-mono ${nullablePnlClassName(row.cashFillPnlUsdc)}`}>
                      <div className="space-y-1">
                        <p>{formatNullablePnl(row.cashFillPnlUsdc)}</p>
                        {pnlSourceStatusLabel(row.pnlSourceStatus) ? (
                          <p className={`text-[11px] ${pnlSourceStatusClassName(row.pnlSourceStatus)}`}>
                            {pnlSourceStatusLabel(row.pnlSourceStatus)}
                          </p>
                        ) : null}
                      </div>
                    </TableCell>
                    <TableCell className={`text-right font-mono ${nullablePnlClassName(row.polymarketPositionPnlUsdc ?? null)}`}>
                      {formatNullablePnl(row.polymarketPositionPnlUsdc ?? null)}
                    </TableCell>
                    <TableCell className="text-right font-mono">
                      {formatUsdc(row.cashBuyUsdc)}
                    </TableCell>
                    <TableCell className="text-right font-mono">
                      {formatUsdc(row.cashSellUsdc)}
                    </TableCell>
                    <TableCell className="text-right font-mono">
                      {formatUsdc(row.cashRedeemUsdc)}
                    </TableCell>
                    <TableCell className="text-right font-mono">
                      <div className="space-y-1">
                        <p>{formatNullableQty(row.pendingInventoryQty)}</p>
                        <p className="text-[11px] text-zinc-500">
                          row {formatQty(row.remainingQtyAfterExit)}
                        </p>
                      </div>
                    </TableCell>
                    <TableCell>
                      <span className="inline-flex max-w-[170px] rounded-full border border-zinc-700 bg-zinc-950 px-2 py-1 text-[11px] text-zinc-300">
                        <span className="truncate">{cashStatusLabel(row.cashStatus)}</span>
                      </span>
                    </TableCell>
                    <TableCell className={`text-right font-mono ${pnlClassName(row.rowPnlUsdc)}`}>
                      {formatPnl(row.rowPnlUsdc)}
                    </TableCell>
                    <TableCell className={`text-right font-mono ${pnlClassName(row.rowPnlUsdc)}`}>
                      {formatPercent(row.pnlPct)}
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>

        <div className="flex items-center justify-between gap-3">
          <p className="text-xs text-zinc-500">
            Sayfa {data?.page ?? page} / {Math.max(totalPages, 1)} - Satir {total}
          </p>
          <div className="flex items-center gap-2">
            <Button
              type="button"
              variant="outline"
              className="border-zinc-700 text-zinc-200"
              disabled={page <= 1}
              onClick={() => setPage((current) => Math.max(1, current - 1))}
            >
              Onceki
            </Button>
            <Button
              type="button"
              variant="outline"
              className="border-zinc-700 text-zinc-200"
              disabled={totalPages === 0 || page >= totalPages}
              onClick={() => setPage((current) => current + 1)}
            >
              Sonraki
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
