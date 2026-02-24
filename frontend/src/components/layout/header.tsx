'use client';

import { useBotStatus } from '@/hooks/use-bot-status';
import { Badge } from '@/components/ui/badge';

export function Header({ title }: { title: string }) {
  const { data } = useBotStatus();

  return (
    <header className="flex h-14 items-center justify-between border-b border-zinc-800 px-6">
      <h2 className="text-lg font-semibold text-zinc-100">{title}</h2>
      <div className="flex items-center gap-3">
        {data && (
          <>
            <Badge variant={data.serviceActive ? 'default' : 'destructive'} className={data.serviceActive ? 'bg-emerald-600' : ''}>
              {data.serviceActive ? 'Bot Active' : 'Bot Stopped'}
            </Badge>
            {data.lastRun && (
              <span className="text-xs text-zinc-500">
                Mode: <span className="text-zinc-300">{data.lastRun.mode}</span>
              </span>
            )}
          </>
        )}
      </div>
    </header>
  );
}
