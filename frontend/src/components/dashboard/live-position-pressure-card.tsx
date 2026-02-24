'use client';

import { useEffect, useMemo, useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Switch } from '@/components/ui/switch';
import type { DashboardData } from '@/lib/types';
import { Gauge } from 'lucide-react';

type RuleRow = {
  legSide: 'yes' | 'no';
  dropSellPct: number;
  enabled: boolean;
};

export function LivePositionPressureCard({
  activePosition,
  pressure,
  rules,
}: {
  activePosition: DashboardData['activePosition'];
  pressure: DashboardData['pressure'];
  rules: DashboardData['positionExitRules'];
}) {
  const [rows, setRows] = useState<RuleRow[]>([]);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const base = (rules && rules.length > 0
      ? rules
      : [
          { legSide: 'yes' as const, dropSellPct: 15, enabled: true },
          { legSide: 'no' as const, dropSellPct: 15, enabled: true },
        ]).map((r) => ({
      legSide: r.legSide,
      dropSellPct: r.dropSellPct,
      enabled: r.enabled,
    }));
    setRows(base);
  }, [rules]);

  const legs = useMemo(() => activePosition?.legs ?? [], [activePosition]);

  const saveRules = async () => {
    if (!activePosition) return;
    setSaving(true);
    setMessage(null);
    setError(null);

    try {
      const res = await fetch('/api/position-rules', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          tradeId: activePosition.tradeId,
          rules: rows,
        }),
      });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) {
        throw new Error(data?.error || `HTTP ${res.status}`);
      }
      setMessage('Rules updated');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save');
    } finally {
      setSaving(false);
    }
  };

  if (!activePosition) {
    return (
      <Card className="border-zinc-800 bg-zinc-900">
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <CardTitle className="text-sm font-medium text-zinc-400">Live Position & Pressure</CardTitle>
          <Gauge className="h-4 w-4 text-zinc-500" />
        </CardHeader>
        <CardContent>
          <p className="text-sm text-zinc-500">No active position</p>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-sm font-medium text-zinc-400">Live Position & Pressure</CardTitle>
        <Gauge className="h-4 w-4 text-zinc-500" />
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="rounded-lg border border-zinc-800 p-3 text-xs text-zinc-400">
          <p>
            Market: <span className="text-zinc-200">{activePosition.marketSlug}</span>
          </p>
          <p>
            Pressure: <span className={pressure?.triggered ? 'text-red-400' : 'text-zinc-200'}>{pressure?.pressureScore?.toFixed(3) ?? '0.000'}</span>
          </p>
          <p>
            Trigger: <span className="text-zinc-200">{pressure?.triggerReason || '-'}</span>
          </p>
        </div>

        <div className="space-y-3">
          {rows.map((row) => {
            const leg = legs.find((l) => l.legSide === row.legSide);
            return (
              <div key={row.legSide} className="rounded-lg border border-zinc-800 p-3">
                <div className="mb-2 flex items-center justify-between text-xs">
                  <p className="font-semibold uppercase text-zinc-300">{row.legSide}</p>
                  <p className="text-zinc-500">qty {leg?.qty?.toFixed(2) ?? '0.00'}</p>
                </div>

                <div className="grid grid-cols-3 items-center gap-3 text-xs">
                  <span className="text-zinc-500">Rule Enabled</span>
                  <Switch
                    checked={row.enabled}
                    onCheckedChange={(v) => {
                      setRows((prev) =>
                        prev.map((x) => (x.legSide === row.legSide ? { ...x, enabled: v } : x))
                      );
                    }}
                  />
                  <span className="text-zinc-500" />

                  <span className="text-zinc-500">Sell On Drop (%)</span>
                  <Input
                    type="number"
                    min={0.01}
                    max={100}
                    step={0.01}
                    value={row.dropSellPct}
                    className="border-zinc-700 bg-zinc-800 text-zinc-200"
                    onChange={(e) => {
                      const value = parseFloat(e.target.value);
                      setRows((prev) =>
                        prev.map((x) =>
                          x.legSide === row.legSide
                            ? { ...x, dropSellPct: Number.isFinite(value) ? value : 0 }
                            : x
                        )
                      );
                    }}
                  />
                  <span className="text-zinc-500">min 0.01</span>
                </div>
              </div>
            );
          })}
        </div>

        {error && <p className="text-xs text-red-400">{error}</p>}
        {message && <p className="text-xs text-emerald-400">{message}</p>}

        <Button size="sm" onClick={saveRules} disabled={saving}>
          {saving ? 'Saving...' : 'Save Position Rules'}
        </Button>
      </CardContent>
    </Card>
  );
}
