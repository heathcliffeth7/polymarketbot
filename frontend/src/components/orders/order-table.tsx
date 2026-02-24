'use client';

import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Badge } from '@/components/ui/badge';
import type { Order } from '@/lib/types';

const statusColors: Record<string, string> = {
  open: 'bg-blue-600',
  filled: 'bg-emerald-600',
  partially_filled: 'bg-cyan-600',
  canceled: 'bg-zinc-600',
  rejected: 'bg-red-600',
  expired: 'bg-orange-600',
  pending: 'bg-yellow-600',
};

export function OrderTable({ orders }: { orders: Order[] }) {
  return (
    <Table>
      <TableHeader>
        <TableRow className="border-zinc-800 hover:bg-transparent">
          <TableHead className="text-zinc-500">ID</TableHead>
          <TableHead className="text-zinc-500">Trade</TableHead>
          <TableHead className="text-zinc-500">Intent</TableHead>
          <TableHead className="text-zinc-500">Side</TableHead>
          <TableHead className="text-zinc-500">Price</TableHead>
          <TableHead className="text-zinc-500">Size</TableHead>
          <TableHead className="text-zinc-500">Status</TableHead>
          <TableHead className="text-zinc-500">Created</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {orders.length === 0 ? (
          <TableRow className="border-zinc-800">
            <TableCell colSpan={8} className="text-center text-zinc-500">No orders found</TableCell>
          </TableRow>
        ) : (
          orders.map((order) => (
            <TableRow key={order.id} className="border-zinc-800 hover:bg-zinc-800/50">
              <TableCell className="text-zinc-300">#{order.id}</TableCell>
              <TableCell className="text-zinc-300">#{order.trade_id}</TableCell>
              <TableCell>
                <Badge variant="outline" className="border-zinc-700 text-zinc-300">{order.intent}</Badge>
              </TableCell>
              <TableCell className={order.side === 'buy' ? 'text-emerald-400' : 'text-red-400'}>{order.side}</TableCell>
              <TableCell className="font-mono text-zinc-300">{order.price.toFixed(4)}</TableCell>
              <TableCell className="font-mono text-zinc-300">{order.size.toFixed(2)}</TableCell>
              <TableCell>
                <Badge className={statusColors[order.status] || 'bg-zinc-600'}>{order.status}</Badge>
              </TableCell>
              <TableCell className="text-xs text-zinc-400">{new Date(order.created_at).toLocaleString()}</TableCell>
            </TableRow>
          ))
        )}
      </TableBody>
    </Table>
  );
}
