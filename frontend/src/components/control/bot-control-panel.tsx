'use client';

import { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { useBotStatus } from '@/hooks/use-bot-status';
import { Play, Square, RotateCcw } from 'lucide-react';

export function BotControlPanel() {
  const { data, mutate } = useBotStatus();
  const [loading, setLoading] = useState<string | null>(null);
  const controlUnavailable = data ? !data.controlAvailable : false;

  const handleAction = async (action: 'start' | 'stop' | 'restart') => {
    if (controlUnavailable) {
      alert(data?.controlReason || 'Service control unavailable. Please restart manually.');
      return;
    }
    if (action === 'stop' && !confirm('Are you sure you want to stop the bot?')) return;
    setLoading(action);
    try {
      const res = await fetch('/api/bot/control', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action }),
      });
      if (!res.ok) {
        const err = await res.json();
        alert(err.error || 'Action failed');
        return;
      }
      setTimeout(() => mutate(), 2000);
    } finally {
      setLoading(null);
    }
  };

  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader>
        <CardTitle className="text-sm font-medium text-zinc-400">Service Control</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex items-center gap-3">
          <span className="text-zinc-400">Status:</span>
          <Badge
            className={data?.serviceActive ? 'bg-emerald-600' : 'bg-red-600'}
          >
            {data?.serviceActive ? 'Active' : 'Stopped'}
          </Badge>
          {data?.lastRun && (
            <span className="text-xs text-zinc-500">v{data.lastRun.version} / {data.lastRun.mode}</span>
          )}
        </div>
        {controlUnavailable && (
          <p className="text-xs text-amber-400">
            Service control unavailable: {data?.controlReason || 'manual restart required'}
          </p>
        )}

        <div className="flex gap-3">
          <Button
            onClick={() => handleAction('start')}
            disabled={controlUnavailable || loading !== null || data?.serviceActive}
            className="bg-emerald-700 hover:bg-emerald-600"
          >
            <Play className="mr-2 h-4 w-4" />
            {loading === 'start' ? 'Starting...' : 'Start'}
          </Button>
          <Button
            variant="destructive"
            onClick={() => handleAction('stop')}
            disabled={controlUnavailable || loading !== null || !data?.serviceActive}
          >
            <Square className="mr-2 h-4 w-4" />
            {loading === 'stop' ? 'Stopping...' : 'Stop'}
          </Button>
          <Button
            variant="outline"
            onClick={() => handleAction('restart')}
            disabled={controlUnavailable || loading !== null}
            className="border-zinc-700 text-zinc-300 hover:bg-zinc-800"
          >
            <RotateCcw className="mr-2 h-4 w-4" />
            {loading === 'restart' ? 'Restarting...' : 'Restart'}
          </Button>
        </div>

        {data?.lastRun?.stopped_at && (
          <div className="text-xs text-zinc-500">
            Last stopped: {new Date(data.lastRun.stopped_at).toLocaleString()}
            {data.lastRun.reason && ` - ${data.lastRun.reason}`}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
