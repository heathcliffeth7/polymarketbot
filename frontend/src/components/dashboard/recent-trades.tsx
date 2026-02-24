'use client';

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { TradeStateBadge } from '@/components/trades/trade-state-badge';
import type { Trade } from '@/lib/types';

export function RecentTrades({ trades }: { trades: Trade[] }) {
  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader>
        <CardTitle className="text-sm font-medium text-zinc-400">Recent Trades</CardTitle>
      </CardHeader>
      <CardContent>
        <Table>
          <TableHeader>
            <TableRow className="border-zinc-800 hover:bg-transparent">
              <TableHead className="text-zinc-500">ID</TableHead>
              <TableHead className="text-zinc-500">Market</TableHead>
              <TableHead className="text-zinc-500">State</TableHead>
              <TableHead className="text-zinc-500">Entry</TableHead>
              <TableHead className="text-zinc-500">Exit</TableHead>
              <TableHead className="text-zinc-500 text-right">PnL</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {trades.length === 0 ? (
              <TableRow className="border-zinc-800">
                <TableCell colSpan={6} className="text-center text-zinc-500">No trades yet</TableCell>
              </TableRow>
            ) : (
              trades.map((trade) => (
                <TableRow key={trade.id} className="border-zinc-800 hover:bg-zinc-800/50">
                  <TableCell className="text-zinc-300">#{trade.id}</TableCell>
                  <TableCell className="max-w-[120px] truncate text-zinc-300">{trade.market_slug || '-'}</TableCell>
                  <TableCell><TradeStateBadge state={trade.state} /></TableCell>
                  <TableCell className="text-zinc-300">{trade.entry_price?.toFixed(4) ?? '-'}</TableCell>
                  <TableCell className="text-zinc-300">{trade.exit_price?.toFixed(4) ?? '-'}</TableCell>
                  <TableCell className={`text-right font-mono ${(trade.realized_pnl ?? 0) >= 0 ? 'text-emerald-400' : 'text-red-400'}`}>
                    {trade.realized_pnl != null ? `${trade.realized_pnl >= 0 ? '+' : ''}${trade.realized_pnl.toFixed(4)}` : '-'}
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}
