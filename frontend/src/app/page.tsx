'use client';

import { PageShell } from '@/components/layout/page-shell';
import { BotStatusCard } from '@/components/dashboard/bot-status-card';
import { TradeStateCard } from '@/components/dashboard/trade-state-card';
import { DailyPnlCard } from '@/components/dashboard/daily-pnl-card';
import { RiskSummary } from '@/components/dashboard/risk-summary';
import { RecentTrades } from '@/components/dashboard/recent-trades';
import { LivePositionPressureCard } from '@/components/dashboard/live-position-pressure-card';
import { Skeleton } from '@/components/ui/skeleton';
import { useDashboard } from '@/hooks/use-dashboard';

export default function DashboardPage() {
  const { data, error, isLoading } = useDashboard();

  return (
    <PageShell title="Dashboard">
      {error && (
        <div className="mb-4 rounded-lg border border-red-800 bg-red-900/20 p-3 text-sm text-red-400">
          Failed to load dashboard data
        </div>
      )}
      {isLoading ? (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <Skeleton key={i} className="h-[180px] bg-zinc-800" />
          ))}
        </div>
      ) : data ? (
        <div className="space-y-6">
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
            <BotStatusCard data={data.botStatus} />
            <TradeStateCard trade={data.activeTrade} />
            <DailyPnlCard data={data.dailyPnl} />
            <RiskSummary data={data.riskSummary} />
          </div>
          <div className="grid gap-4 md:grid-cols-1 lg:grid-cols-2">
            <LivePositionPressureCard
              activePosition={data.activePosition ?? null}
              pressure={data.pressure ?? null}
              rules={data.positionExitRules ?? []}
            />
          </div>
          <RecentTrades trades={data.recentTrades} />
        </div>
      ) : null}
    </PageShell>
  );
}
