'use client';

import { useState } from 'react';
import { PageShell } from '@/components/layout/page-shell';
import { MarketCycleCard } from '@/components/market/market-cycle-card';
import { Button } from '@/components/ui/button';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Skeleton } from '@/components/ui/skeleton';
import { usePolling } from '@/hooks/use-polling';
import type { Market, PaginatedResponse } from '@/lib/types';

export default function MarketPage() {
  const [page, setPage] = useState(1);
  const [statusFilter, setStatusFilter] = useState<string>('');
  const params = new URLSearchParams({ page: String(page), limit: '20' });
  if (statusFilter) params.set('status', statusFilter);

  const { data, isLoading } = usePolling<PaginatedResponse<Market>>(`/api/markets?${params}`, 10000);

  return (
    <PageShell title="Market Cycles">
      <div className="space-y-4">
        <div className="flex items-center gap-4">
          <Select value={statusFilter} onValueChange={(v) => { setStatusFilter(v === 'all' ? '' : v); setPage(1); }}>
            <SelectTrigger className="w-[180px] border-zinc-700 bg-zinc-800 text-zinc-200">
              <SelectValue placeholder="Status" />
            </SelectTrigger>
            <SelectContent className="border-zinc-700 bg-zinc-800">
              <SelectItem value="all" className="text-zinc-200">All Status</SelectItem>
              {['open', 'closed', 'settled'].map((s) => (
                <SelectItem key={s} value={s} className="text-zinc-200">{s}</SelectItem>
              ))}
            </SelectContent>
          </Select>
          {data && <span className="text-sm text-zinc-500">{data.total} markets total</span>}
        </div>

        {isLoading ? (
          <Skeleton className="h-[300px] bg-zinc-800" />
        ) : (
          <MarketCycleCard markets={data?.data ?? []} />
        )}

        {data && data.totalPages > 1 && (
          <div className="flex items-center justify-center gap-2">
            <Button variant="outline" size="sm" onClick={() => setPage((p) => Math.max(1, p - 1))} disabled={page === 1} className="border-zinc-700 text-zinc-300">Previous</Button>
            <span className="text-sm text-zinc-400">Page {page} of {data.totalPages}</span>
            <Button variant="outline" size="sm" onClick={() => setPage((p) => Math.min(data.totalPages, p + 1))} disabled={page === data.totalPages} className="border-zinc-700 text-zinc-300">Next</Button>
          </div>
        )}
      </div>
    </PageShell>
  );
}
