'use client';

import { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Switch } from '@/components/ui/switch';
import { Label } from '@/components/ui/label';
import { usePolling } from '@/hooks/use-polling';

interface RelaxData {
  max_price_relax_enabled: boolean;
  control_available?: boolean;
  control_reason?: string | null;
}

export function RelaxToggle() {
  const { data, mutate } = usePolling<RelaxData>('/api/relax', 5000);
  const [loading, setLoading] = useState(false);

  const handleToggle = async (checked: boolean) => {
    setLoading(true);
    try {
      const res = await fetch('/api/relax', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled: checked }),
      });
      const payload = await res.json();
      if (!res.ok) {
        alert(payload.error || 'Failed to toggle relax');
        return;
      }
      if (payload.restart_applied === false && payload.control_available === false) {
        alert(
          payload.restart_message ||
            'Relax updated. Service control unavailable; restart manually from Control page.'
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
        <CardTitle className="text-sm font-medium text-zinc-400">PTB Relax</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex items-center justify-between">
          <div>
            <Label className="text-zinc-200">Max Price Relax</Label>
            <p className="text-xs text-zinc-500">
              Status:{' '}
              <span className="text-zinc-300">
                {data ? (data.max_price_relax_enabled ? 'on' : 'off') : '...'}
              </span>
            </p>
          </div>
          <Switch
            checked={data?.max_price_relax_enabled ?? true}
            onCheckedChange={handleToggle}
            disabled={loading}
          />
        </div>
        {data?.max_price_relax_enabled === false && (
          <p className="mt-2 text-xs text-amber-400">
            Relax is OFF - PTB max-price relax will not adjust thresholds.
          </p>
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
