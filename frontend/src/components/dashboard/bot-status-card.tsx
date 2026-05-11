'use client';

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import type { DashboardData } from '@/lib/types';
import { Activity } from 'lucide-react';

export function BotStatusCard({ data }: { data: DashboardData['botStatus'] }) {
  const uptime = data.lastRun?.started_at
    ? getUptime(data.lastRun.started_at)
    : 'N/A';
  const waitingForMarket = data.serviceActive && data.marketDiscoveryState === 'waiting_for_market';
  const discoveryError = data.serviceActive && data.marketDiscoveryState === 'error';

  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-sm font-medium text-zinc-400">Bot Status</CardTitle>
        <Activity className="h-4 w-4 text-zinc-500" />
      </CardHeader>
      <CardContent>
        <div className="flex items-center gap-2">
          <Badge
            variant={data.serviceActive ? 'default' : 'destructive'}
            className={data.serviceActive ? 'bg-emerald-600' : ''}
          >
            {data.serviceActive ? 'Active' : 'Stopped'}
          </Badge>
          {data.lastRun && (
            <Badge variant="outline" className="border-zinc-700 text-zinc-400">
              {data.lastRun.mode}
            </Badge>
          )}
          {waitingForMarket && (
            <Badge variant="outline" className="border-amber-600 text-amber-400">
              Waiting Market
            </Badge>
          )}
        </div>
        <div className="mt-3 space-y-1 text-xs text-zinc-500">
          {data.controlAvailable === false && (
            <p className="text-amber-400">
              Control unavailable: {data.controlReason || 'manual restart required'}
            </p>
          )}
          {waitingForMarket && (
            <p className="text-amber-400">
              Active market bekleniyor. Bot çalışmaya devam ediyor.
            </p>
          )}
          {discoveryError && (
            <p className="text-red-400">
              Market discovery error: {data.marketDiscoveryMessage || 'unknown error'}
            </p>
          )}
          {data.selectedMarketSlug && (
            <p>
              Selected market: <span className="text-zinc-300">{data.selectedMarketSlug}</span>
            </p>
          )}
          {data.lastRun && (
            <>
              <p>Version: <span className="text-zinc-300">{data.lastRun.version}</span></p>
              <p>Uptime: <span className="text-zinc-300">{uptime}</span></p>
              {data.lastRun.reason && (
                <p>Last stop: <span className="text-zinc-300">{data.lastRun.reason}</span></p>
              )}
            </>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function getUptime(startedAt: string): string {
  const diff = Date.now() - new Date(startedAt).getTime();
  const hours = Math.floor(diff / 3600000);
  const minutes = Math.floor((diff % 3600000) / 60000);
  if (hours > 24) return `${Math.floor(hours / 24)}d ${hours % 24}h`;
  return `${hours}h ${minutes}m`;
}
