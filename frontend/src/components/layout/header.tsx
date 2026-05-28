'use client';

import { Bell } from 'lucide-react';
import { useBotStatus } from '@/hooks/use-bot-status';
import { useNotifications } from '@/contexts/notification-context';
import { useTradeFlowRealtime } from '@/contexts/trade-flow-realtime-context';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { useAuthState } from '@/lib/auth-client';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { ScrollArea } from '@/components/ui/scroll-area';

function formatTimeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const sec = Math.floor(diff / 1000);
  if (sec < 60) return `${sec}sn once`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}dk once`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}s once`;
  return `${Math.floor(hr / 24)}g once`;
}

export function Header({ title }: { title: string }) {
  const { data } = useBotStatus();
  const { data: auth } = useAuthState();
  const { notifications, unreadCount, markAllRead } = useNotifications();
  const { connectionState, lastEventLagMs } = useTradeFlowRealtime();
  const realtimeLabel =
    connectionState === 'open'
      ? `RT ${lastEventLagMs != null ? `${lastEventLagMs}ms` : 'live'}`
      : connectionState === 'connecting'
        ? 'RT baglaniyor'
        : 'RT kesik';
  const realtimeTone =
    connectionState === 'open'
      ? 'border-emerald-700 bg-emerald-500/10 text-emerald-300'
      : connectionState === 'connecting'
        ? 'border-amber-700 bg-amber-500/10 text-amber-300'
        : 'border-red-700 bg-red-500/10 text-red-300';

  return (
    <header className="flex h-14 shrink-0 items-center justify-between gap-3 border-b border-zinc-800 px-3 md:px-6">
      <h2 className="min-w-0 truncate text-base font-semibold text-zinc-100 md:text-lg">{title}</h2>
      <div className="flex shrink-0 items-center gap-2 md:gap-3">
        <DropdownMenu onOpenChange={(open) => { if (open) markAllRead(); }}>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="icon" className="relative h-8 w-8">
              <Bell className="h-4 w-4 text-zinc-400" />
              {unreadCount > 0 && (
                <span className="absolute -top-1 -right-1 flex h-4 w-4 items-center justify-center rounded-full bg-emerald-600 text-[10px] font-bold text-white">
                  {unreadCount > 9 ? '9+' : unreadCount}
                </span>
              )}
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent className="w-[calc(100vw-2rem)] sm:w-80" align="end">
            <DropdownMenuLabel>Bildirimler</DropdownMenuLabel>
            <DropdownMenuSeparator />
            <ScrollArea className="max-h-72">
              {notifications.length === 0 ? (
                <p className="p-4 text-center text-sm text-zinc-500">
                  Henuz bildirim yok
                </p>
              ) : (
                notifications.map((n) => (
                  <div
                    key={n.id}
                    className="flex flex-col gap-0.5 border-b border-zinc-800 px-3 py-2 last:border-0"
                  >
                    <span
                      className={`text-sm font-medium ${
                        n.tone === 'error'
                          ? 'text-red-400'
                          : n.tone === 'success'
                            ? 'text-emerald-400'
                            : 'text-zinc-100'
                      }`}
                    >
                      {n.title}
                    </span>
                    <span className="truncate text-xs text-zinc-500">
                      {n.detail}
                    </span>
                    {n.market && n.market !== n.detail && (
                      <span className="truncate text-xs text-zinc-600">
                        {n.market}
                      </span>
                    )}
                    <span className="text-xs text-zinc-600">
                      {formatTimeAgo(n.time)}
                    </span>
                  </div>
                ))
              )}
            </ScrollArea>
          </DropdownMenuContent>
        </DropdownMenu>

        {auth?.user?.username && (
          <span className="hidden rounded-full border border-zinc-800 bg-zinc-900 px-2 py-1 text-xs text-zinc-300 sm:inline-flex">
            @{auth.user.username}
          </span>
        )}
        <span className={`hidden rounded-full border px-2 py-1 text-xs sm:inline-flex ${realtimeTone}`}>
          {realtimeLabel}
        </span>

        {data && (
          <div className="hidden items-center gap-3 lg:flex">
            <Badge variant={data.serviceActive ? 'default' : 'destructive'} className={data.serviceActive ? 'bg-emerald-600' : ''}>
              {data.serviceActive ? 'Bot Active' : 'Bot Stopped'}
            </Badge>
            {data.lastRun && (
              <span className="text-xs text-zinc-500">
                Mode: <span className="text-zinc-300">{data.lastRun.mode}</span>
              </span>
            )}
          </div>
        )}
      </div>
    </header>
  );
}
