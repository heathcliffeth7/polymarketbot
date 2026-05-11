'use client';

import { useState, useEffect } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { useConfig, saveConfig } from '@/hooks/use-config';

export function ModeSwitch() {
  const { data, mutate } = useConfig('bot');
  const [currentMode, setCurrentMode] = useState<string>('paper');
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (data?.data?.mode) {
      setCurrentMode(data.data.mode as string);
    }
  }, [data]);

  const switchMode = async () => {
    const newMode = currentMode === 'paper' ? 'live' : 'paper';
    const msg = `Switch to ${newMode.toUpperCase()} mode? ${newMode === 'live' ? 'This will use REAL funds!' : ''}`;
    if (!confirm(msg)) return;

    setLoading(true);
    try {
      const config = { ...data?.data, mode: newMode };
      await saveConfig('bot', config as Record<string, unknown>);
      const res = await fetch('/api/bot/control', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action: 'restart' }),
      });
      if (!res.ok) {
        let message = 'Mode changed but restart failed. Please restart manually.';
        try {
          const err = await res.json();
          if (res.status === 503 || err.controlAvailable === false) {
            message =
              'Mode changed. Service control is unavailable in this environment; restart manually.';
          } else if (err.error) {
            message = err.error;
          }
        } catch {
          // keep fallback message
        }
        alert(message);
      }
      mutate();
      setCurrentMode(newMode);
    } catch (err) {
      alert(err instanceof Error ? err.message : 'Failed to switch mode');
    } finally {
      setLoading(false);
    }
  };

  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader>
        <CardTitle className="text-sm font-medium text-zinc-400">Execution Mode</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex items-center gap-3">
          <span className="text-zinc-400">Current:</span>
          <Badge className={currentMode === 'live' ? 'bg-red-600' : 'bg-blue-600'}>
            {currentMode.toUpperCase()}
          </Badge>
        </div>
        <Button
          onClick={switchMode}
          disabled={loading}
          variant={currentMode === 'paper' ? 'destructive' : 'default'}
          className={currentMode !== 'paper' ? 'bg-blue-700 hover:bg-blue-600' : ''}
        >
          {loading ? 'Switching...' : `Switch to ${currentMode === 'paper' ? 'LIVE' : 'PAPER'}`}
        </Button>
        {currentMode === 'live' && (
          <p className="text-xs text-red-400">Bot is using REAL funds in live mode</p>
        )}
      </CardContent>
    </Card>
  );
}
