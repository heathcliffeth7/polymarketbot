'use client';

import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { TradeStateBadge } from './trade-state-badge';
import type { Trade } from '@/lib/types';

export function TradeTable({ trades }: { trades: Trade[] }) {
  return (
    <Table>
      <TableHeader>
        <TableRow className="border-zinc-800 hover:bg-transparent">
          <TableHead className="text-zinc-500">ID</TableHead>
          <TableHead className="text-zinc-500">Market</TableHead>
          <TableHead className="text-zinc-500">State</TableHead>
          <TableHead className="text-zinc-500">Entry</TableHead>
          <TableHead className="text-zinc-500">Exit</TableHead>
          <TableHead className="text-zinc-500">Notional</TableHead>
          <TableHead className="text-zinc-500 text-right">PnL</TableHead>
          <TableHead className="text-zinc-500">Opened</TableHead>
          <TableHead className="text-zinc-500">Closed</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {trades.length === 0 ? (
          <TableRow className="border-zinc-800">
            <TableCell colSpan={9} className="text-center text-zinc-500">No trades found</TableCell>
          </TableRow>
        ) : (
          trades.map((trade) => (
            <TableRow key={trade.id} className="border-zinc-800 hover:bg-zinc-800/50">
              <TableCell className="text-zinc-300">#{trade.id}</TableCell>
              <TableCell className="max-w-[150px] truncate text-zinc-300">{trade.market_slug || '-'}</TableCell>
              <TableCell><TradeStateBadge state={trade.state} /></TableCell>
              <TableCell className="font-mono text-zinc-300">{trade.entry_price?.toFixed(4) ?? '-'}</TableCell>
              <TableCell className="font-mono text-zinc-300">{trade.exit_price?.toFixed(4) ?? '-'}</TableCell>
              <TableCell className="font-mono text-zinc-300">${trade.notional_usdc.toFixed(2)}</TableCell>
              <TableCell className={`text-right font-mono ${(trade.realized_pnl ?? 0) >= 0 ? 'text-emerald-400' : 'text-red-400'}`}>
                {trade.realized_pnl != null ? `${trade.realized_pnl >= 0 ? '+' : ''}${trade.realized_pnl.toFixed(4)}` : '-'}
              </TableCell>
              <TableCell className="text-xs text-zinc-400">{trade.opened_at ? new Date(trade.opened_at).toLocaleString() : '-'}</TableCell>
              <TableCell className="text-xs text-zinc-400">{trade.closed_at ? new Date(trade.closed_at).toLocaleString() : '-'}</TableCell>
            </TableRow>
          ))
        )}
      </TableBody>
    </Table>
  );
}
