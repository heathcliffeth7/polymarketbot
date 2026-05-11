'use client';

import { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Switch } from '@/components/ui/switch';
import { Label } from '@/components/ui/label';
import { usePolling } from '@/hooks/use-polling';

interface KillSwitchData {
  kill_switch_mode: string;
  manual_kill_switch_active: boolean;
  control_available?: boolean;
  control_reason?: string | null;
}

export function KillSwitchToggle() {
  const { data, mutate } = usePolling<KillSwitchData>('/api/kill-switch', 5000);
  const [loading, setLoading] = useState(false);

  const handleToggle = async (checked: boolean) => {
    setLoading(true);
    try {
      const res = await fetch('/api/kill-switch', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ active: checked }),
      });
      const payload = await res.json();
      if (!res.ok) {
        alert(payload.error || 'Failed to toggle kill switch');
        return;
      }
      if (payload.restart_applied === false && payload.control_available === false) {
        alert(
          payload.restart_message ||
            'Kill switch updated. Service control unavailable; restart manually from Control page.'
        );
      }
      mutate();
    } finally {
      setLoading(false);
    }
  };

  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader>
        <CardTitle className="text-sm font-medium text-zinc-400">Kill Switch</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex items-center justify-between">
          <div>
            <Label className="text-zinc-200">Manual Kill Switch</Label>
            <p className="text-xs text-zinc-500">
              Mode: <span className="text-zinc-300">{data?.kill_switch_mode ?? '...'}</span>
            </p>
          </div>
          <Switch
            checked={data?.manual_kill_switch_active ?? false}
            onCheckedChange={handleToggle}
            disabled={loading || data?.kill_switch_mode === 'disabled'}
          />
        </div>
        {data?.manual_kill_switch_active && (
          <p className="mt-2 text-xs text-red-400">Kill switch is ACTIVE - bot will not place new trades</p>
        )}
        {data?.control_available === false && (
          <p className="mt-2 text-xs text-amber-400">
            Service control unavailable: {data.control_reason || 'manual restart required'}.
          </p>
        )}
      </CardContent>
    </Card>
  );
}
