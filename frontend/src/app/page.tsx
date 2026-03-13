'use client';

import { PageShell } from '@/components/layout/page-shell';
import { BotStatusCard } from '@/components/dashboard/bot-status-card';
import { ClaimSweepCard } from '@/components/dashboard/claim-sweep-card';
import { DailyPnlCard } from '@/components/dashboard/daily-pnl-card';
import { LivePositionPressureCard } from '@/components/dashboard/live-position-pressure-card';
import { Skeleton } from '@/components/ui/skeleton';
import { useDashboard } from '@/hooks/use-dashboard';

export default function DashboardPage() {
  const { data, error, isLoading, mutate } = useDashboard();

  return (
    <PageShell title="Dashboard">
      {error && (
        <div className="mb-4 rounded-lg border border-red-800 bg-red-900/20 p-3 text-sm text-red-400">
          Failed to load dashboard data
        </div>
      )}
      {isLoading ? (
        <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <Skeleton key={i} className="h-[180px] bg-zinc-800" />
          ))}
        </div>
      ) : data ? (
        <div className="space-y-6">
          <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
            <BotStatusCard data={data.botStatus} />
            <DailyPnlCard data={data.dailyPnl} />
            <ClaimSweepCard data={data.claimSweep} onSweepComplete={() => void mutate()} />
            <LivePositionPressureCard
              activePosition={data.activePosition ?? null}
              pressure={data.pressure ?? null}
              rules={data.positionExitRules ?? []}
            />
          </div>
        </div>
      ) : null}
    </PageShell>
  );
}
