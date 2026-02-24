'use client';

import { useState } from 'react';
import { PageShell } from '@/components/layout/page-shell';
import { TradeTable } from '@/components/trades/trade-table';
import { Card, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Skeleton } from '@/components/ui/skeleton';
import { useTrades } from '@/hooks/use-trades';
import type { TradeState } from '@/lib/types';

const STATES: TradeState[] = [
  'Idle', 'WaitingEntry', 'EntryPlaced', 'EntryPartiallyFilled',
  'EntryFilled', 'TpPlaced', 'SlArmed', 'ExitPartiallyFilled',
  'ExitFilled', 'Settled', 'Halted',
];

export default function TradesPage() {
  const [page, setPage] = useState(1);
  const [stateFilter, setStateFilter] = useState<string>('');
  const { data, isLoading } = useTrades(page, 20, stateFilter || undefined);

  return (
    <PageShell title="Trade History">
      <div className="space-y-4">
        <div className="flex items-center gap-4">
          <Select value={stateFilter} onValueChange={(v) => { setStateFilter(v === 'all' ? '' : v); setPage(1); }}>
            <SelectTrigger className="w-[200px] border-zinc-700 bg-zinc-800 text-zinc-200">
              <SelectValue placeholder="Filter by state" />
            </SelectTrigger>
            <SelectContent className="border-zinc-700 bg-zinc-800">
              <SelectItem value="all" className="text-zinc-200">All States</SelectItem>
              {STATES.map((s) => (
                <SelectItem key={s} value={s} className="text-zinc-200">{s}</SelectItem>
              ))}
            </SelectContent>
          </Select>
          {data && (
            <span className="text-sm text-zinc-500">
              {data.total} trades total
            </span>
          )}
        </div>

        <Card className="border-zinc-800 bg-zinc-900">
          <CardContent className="p-0">
            {isLoading ? (
              <div className="space-y-2 p-4">
                {Array.from({ length: 5 }).map((_, i) => (
                  <Skeleton key={i} className="h-10 bg-zinc-800" />
                ))}
              </div>
            ) : (
              <TradeTable trades={data?.data ?? []} />
            )}
          </CardContent>
        </Card>

        {data && data.totalPages > 1 && (
          <div className="flex items-center justify-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setPage((p) => Math.max(1, p - 1))}
              disabled={page === 1}
              className="border-zinc-700 text-zinc-300"
            >
              Previous
            </Button>
            <span className="text-sm text-zinc-400">
              Page {page} of {data.totalPages}
            </span>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setPage((p) => Math.min(data.totalPages, p + 1))}
              disabled={page === data.totalPages}
              className="border-zinc-700 text-zinc-300"
            >
              Next
            </Button>
          </div>
        )}
      </div>
    </PageShell>
  );
}
