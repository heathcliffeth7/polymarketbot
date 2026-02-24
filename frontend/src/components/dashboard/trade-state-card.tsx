'use client';

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { TradeStateBadge } from '@/components/trades/trade-state-badge';
import type { Trade } from '@/lib/types';
import { TrendingUp } from 'lucide-react';

export function TradeStateCard({ trade }: { trade: Trade | null }) {
  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-sm font-medium text-zinc-400">Active Trade</CardTitle>
        <TrendingUp className="h-4 w-4 text-zinc-500" />
      </CardHeader>
      <CardContent>
        {trade ? (
          <div className="space-y-2">
            <div className="flex items-center gap-2">
              <TradeStateBadge state={trade.state} />
              <span className="text-xs text-zinc-500">#{trade.id}</span>
            </div>
            <div className="grid grid-cols-2 gap-2 text-xs">
              <div>
                <span className="text-zinc-500">Market</span>
                <p className="truncate text-zinc-300">{trade.market_slug || `#${trade.market_id}`}</p>
              </div>
              <div>
                <span className="text-zinc-500">Entry</span>
                <p className="text-zinc-300">{trade.entry_price?.toFixed(4) ?? '-'}</p>
              </div>
              <div>
                <span className="text-zinc-500">Notional</span>
                <p className="text-zinc-300">${trade.notional_usdc.toFixed(2)}</p>
              </div>
              <div>
                <span className="text-zinc-500">Opened</span>
                <p className="text-zinc-300">{trade.opened_at ? new Date(trade.opened_at).toLocaleTimeString() : '-'}</p>
              </div>
            </div>
          </div>
        ) : (
          <p className="text-sm text-zinc-500">No active trade</p>
        )}
      </CardContent>
    </Card>
  );
}
