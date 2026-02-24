'use client';

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { DollarSign } from 'lucide-react';
import type { DashboardData } from '@/lib/types';

export function DailyPnlCard({ data }: { data: DashboardData['dailyPnl'] }) {
  const isPositive = data.totalPnl >= 0;
  const winRate = data.tradeCount > 0 ? ((data.winCount / data.tradeCount) * 100).toFixed(0) : '0';

  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-sm font-medium text-zinc-400">Daily PnL</CardTitle>
        <DollarSign className="h-4 w-4 text-zinc-500" />
      </CardHeader>
      <CardContent>
        <div className={`text-2xl font-bold ${isPositive ? 'text-emerald-400' : 'text-red-400'}`}>
          {isPositive ? '+' : ''}{data.totalPnl.toFixed(2)} USDC
        </div>
        <div className="mt-2 flex gap-4 text-xs text-zinc-500">
          <span>Trades: <span className="text-zinc-300">{data.tradeCount}</span></span>
          <span>W/L: <span className="text-emerald-400">{data.winCount}</span>/<span className="text-red-400">{data.lossCount}</span></span>
          <span>WR: <span className="text-zinc-300">{winRate}%</span></span>
        </div>
      </CardContent>
    </Card>
  );
}
