'use client';

import { useMemo, useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { useTradeFlowAutoScopeAnalysis } from '@/hooks/use-trade-flow';
import type {
  AutoScopeTradeAnalysisRow,
  AutoScopeTradeAnalysisSortBy,
  AutoScopeTradeAnalysisSortDirection,
} from '@/lib/types';

const PAGE_SIZE = 50;
type SortOption = 'default' | 'pnl_asc' | 'pnl_desc';

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

function formatPrice(value: number | null): string {
  return value == null ? '-' : value.toFixed(4);
}

function formatQty(value: number): string {
  return value.toFixed(4);
}

function formatPnl(value: number): string {
  const sign = value > 0 ? '+' : '';
  return `${sign}${value.toFixed(4)}`;
}

function exitReasonLabel(row: AutoScopeTradeAnalysisRow): string {
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
  if (row.positionState === 'closed_market_ended') {
    return row.marketEndAt
      ? `Market bitti: ${formatDateTime(row.marketEndAt)}`
      : 'Market bitti';
  }
  if (row.positionState === 'open') {
    return 'Acik pozisyon devam ediyor';
  }
  return row.sellFilledAt ? formatDateTime(row.sellFilledAt) : 'Kapandi';
}

function pnlClassName(value: number): string {
  if (value > 0) return 'text-emerald-400';
  if (value < 0) return 'text-red-400';
  return 'text-zinc-300';
}

export function AutoScopeAnalysisTable() {
  const [page, setPage] = useState(1);
  const [sortOption, setSortOption] = useState<SortOption>('default');

  const sortBy: AutoScopeTradeAnalysisSortBy =
    sortOption === 'default' ? 'default' : 'pnl';
  const sortDirection: AutoScopeTradeAnalysisSortDirection =
    sortOption === 'pnl_asc' ? 'asc' : 'desc';

  const { data, error, isLoading } = useTradeFlowAutoScopeAnalysis(
    page,
    PAGE_SIZE,
    sortBy,
    sortDirection
  );

  const rows = useMemo(() => data?.data ?? [], [data?.data]);
  const total = data?.total ?? 0;
  const totalPages = data?.totalPages ?? 0;
  const refreshedAt = data?.refreshedAt ?? null;

  const marketCount = useMemo(
    () => new Set(rows.map((row) => `${row.marketSlug}:${row.rootOrderId}`)).size,
    [rows]
  );
  const pagePnl = useMemo(
    () => rows.reduce((sum, row) => sum + row.rowPnlUsdc, 0),
    [rows]
  );

  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader className="space-y-3">
        <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
          <div>
            <CardTitle className="text-sm font-medium text-zinc-200">
              Auto-Scope Trade Analizi
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              Her satir tek bir exit tranche veya kalan acik pozisyonu temsil eder.
            </p>
          </div>
          <div className="text-xs text-zinc-500">
            Son yenileme: {formatDateTime(refreshedAt)}
          </div>
        </div>
        <div className="flex flex-wrap gap-3 text-xs text-zinc-300">
          <span className="rounded-full border border-zinc-700 bg-zinc-950 px-3 py-1">
            Sayfadaki market: {marketCount}
          </span>
          <span className="rounded-full border border-zinc-700 bg-zinc-950 px-3 py-1">
            Toplam satir: {total}
          </span>
          <span className={`rounded-full border border-zinc-700 bg-zinc-950 px-3 py-1 ${pnlClassName(pagePnl)}`}>
            Sayfa PnL: {formatPnl(pagePnl)}
          </span>
        </div>
        <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
          <div className="text-xs text-zinc-500">
            Siralama secimi sadece mevcut sayfayi degil, tum pagination duzenini etkiler.
          </div>
          <div className="flex items-center gap-2">
            <label htmlFor="analysis-sort" className="text-xs text-zinc-400">
              Siralama
            </label>
            <select
              id="analysis-sort"
              value={sortOption}
              onChange={(event) => {
                setSortOption(event.target.value as SortOption);
                setPage(1);
              }}
              className="h-8 rounded-md border border-zinc-700 bg-zinc-950 px-3 text-xs text-zinc-200"
            >
              <option value="default">Tetik zamani (Varsayilan)</option>
              <option value="pnl_asc">PnL artan</option>
              <option value="pnl_desc">PnL azalan</option>
            </select>
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {error && (
          <div className="rounded-md border border-red-900 bg-red-950/30 p-3 text-sm text-red-300">
            Analiz verisi yuklenemedi.
          </div>
        )}

        <Table className="text-xs text-zinc-200">
          <TableHeader>
            <TableRow className="border-zinc-800 hover:bg-transparent">
              <TableHead className="text-zinc-400">Workflow</TableHead>
              <TableHead className="text-zinc-400">Market</TableHead>
              <TableHead className="text-zinc-400">Exit</TableHead>
              <TableHead className="text-zinc-400">Trigger</TableHead>
              <TableHead className="text-zinc-400">Open→Trigger</TableHead>
              <TableHead className="text-zinc-400">Trigger→Buy</TableHead>
              <TableHead className="text-right text-zinc-400">Buy</TableHead>
              <TableHead className="text-right text-zinc-400">Sell/Canli</TableHead>
              <TableHead className="text-right text-zinc-400">Qty</TableHead>
              <TableHead className="text-right text-zinc-400">Kalan Qty</TableHead>
              <TableHead className="text-right text-zinc-400">PnL</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading && rows.length === 0 ? (
              <TableRow className="border-zinc-800">
                <TableCell colSpan={11} className="py-10 text-center text-zinc-500">
                  Analiz verisi yukleniyor...
                </TableCell>
              </TableRow>
            ) : rows.length === 0 ? (
              <TableRow className="border-zinc-800">
                <TableCell colSpan={11} className="py-10 text-center text-zinc-500">
                  Gosterilecek auto-scope trade analizi bulunamadi.
                </TableCell>
              </TableRow>
            ) : (
              rows.map((row) => (
                <TableRow key={row.rowId} className="border-zinc-800 hover:bg-zinc-950/60">
                  <TableCell>
                    <div className="space-y-1">
                      <p className="font-medium text-zinc-100">
                        {row.definitionName || `Flow #${row.definitionId}`}
                      </p>
                      <p className="text-[11px] text-zinc-500">
                        Run #{row.runId} • Root #{row.rootOrderId}
                      </p>
                    </div>
                  </TableCell>
                  <TableCell>
                    <div className="space-y-1">
                      <p className="font-medium text-zinc-100">{row.marketSlug}</p>
                      <p className="text-[11px] text-zinc-500">
                        {row.outcomeLabel} • {row.tokenId.slice(0, 10)}...
                      </p>
                    </div>
                  </TableCell>
                  <TableCell>
                    <div className="space-y-1">
                      <span className="inline-flex rounded-full border border-zinc-700 bg-zinc-950 px-2 py-1 text-[11px] text-zinc-200">
                        {exitReasonLabel(row)}
                      </span>
                      <p className="text-[11px] text-zinc-500">
                        {exitMetaLabel(row)}
                      </p>
                    </div>
                  </TableCell>
                  <TableCell>{formatDateTime(row.triggeredAt)}</TableCell>
                  <TableCell>{formatDuration(row.openToTriggerMs)}</TableCell>
                  <TableCell>{formatDuration(row.triggerToBuyFillMs)}</TableCell>
                  <TableCell className="text-right font-mono">{formatPrice(row.buyAvgPrice)}</TableCell>
                  <TableCell className="text-right font-mono">{formatPrice(row.sellOrLivePrice)}</TableCell>
                  <TableCell className="text-right font-mono">{formatQty(row.rowQty)}</TableCell>
                  <TableCell className="text-right font-mono">{formatQty(row.remainingQtyAfterExit)}</TableCell>
                  <TableCell className={`text-right font-mono ${pnlClassName(row.rowPnlUsdc)}`}>
                    {formatPnl(row.rowPnlUsdc)}
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>

        <div className="flex items-center justify-between gap-3">
          <p className="text-xs text-zinc-500">
            Sayfa {data?.page ?? page} / {Math.max(totalPages, 1)}
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
