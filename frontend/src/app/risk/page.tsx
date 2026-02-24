'use client';

import { useState } from 'react';
import { PageShell } from '@/components/layout/page-shell';
import { RiskEventTable } from '@/components/risk/risk-event-table';
import { KillSwitchToggle } from '@/components/risk/kill-switch-toggle';
import { Card, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Skeleton } from '@/components/ui/skeleton';
import { useRiskEvents } from '@/hooks/use-risk-events';

export default function RiskPage() {
  const [page, setPage] = useState(1);
  const [decisionFilter, setDecisionFilter] = useState<string>('');
  const { data, isLoading } = useRiskEvents(page, 30, undefined, decisionFilter || undefined);

  return (
    <PageShell title="Risk Events">
      <div className="space-y-4">
        <KillSwitchToggle />

        <div className="flex items-center gap-4">
          <Select value={decisionFilter} onValueChange={(v) => { setDecisionFilter(v === 'all' ? '' : v); setPage(1); }}>
            <SelectTrigger className="w-[180px] border-zinc-700 bg-zinc-800 text-zinc-200">
              <SelectValue placeholder="Decision" />
            </SelectTrigger>
            <SelectContent className="border-zinc-700 bg-zinc-800">
              <SelectItem value="all" className="text-zinc-200">All Decisions</SelectItem>
              {['allow', 'block', 'halt'].map((d) => (
                <SelectItem key={d} value={d} className="text-zinc-200">{d}</SelectItem>
              ))}
            </SelectContent>
          </Select>
          {data && <span className="text-sm text-zinc-500">{data.total} events total</span>}
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
              <RiskEventTable events={data?.data ?? []} />
            )}
          </CardContent>
        </Card>

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
