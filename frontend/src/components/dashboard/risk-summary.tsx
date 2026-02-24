'use client';

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { ShieldAlert } from 'lucide-react';
import type { DashboardData } from '@/lib/types';

export function RiskSummary({ data }: { data: DashboardData['riskSummary'] }) {
  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-sm font-medium text-zinc-400">Risk Summary</CardTitle>
        <ShieldAlert className="h-4 w-4 text-zinc-500" />
      </CardHeader>
      <CardContent>
        <div className="space-y-2">
          <div className="flex items-center justify-between text-sm">
            <span className="text-zinc-500">Kill Switch</span>
            <Badge className={data.killSwitchActive ? 'bg-red-600' : 'bg-zinc-700'}>
              {data.killSwitchActive ? 'ACTIVE' : 'OFF'}
            </Badge>
          </div>
          <div className="grid grid-cols-3 gap-2 text-xs">
            <div className="rounded-md bg-zinc-800 p-2 text-center">
              <p className="text-zinc-500">Open Orders</p>
              <p className="text-lg font-semibold text-zinc-200">{data.openOrders}</p>
            </div>
            <div className="rounded-md bg-zinc-800 p-2 text-center">
              <p className="text-zinc-500">Consec. Losses</p>
              <p className={`text-lg font-semibold ${data.consecutiveLosses >= 2 ? 'text-red-400' : 'text-zinc-200'}`}>
                {data.consecutiveLosses}
              </p>
            </div>
            <div className="rounded-md bg-zinc-800 p-2 text-center">
              <p className="text-zinc-500">Halts Today</p>
              <p className={`text-lg font-semibold ${data.haltCount > 0 ? 'text-orange-400' : 'text-zinc-200'}`}>
                {data.haltCount}
              </p>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
