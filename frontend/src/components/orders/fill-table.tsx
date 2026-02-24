'use client';

import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import type { Fill } from '@/lib/types';

export function FillTable({ fills }: { fills: Fill[] }) {
  return (
    <Table>
      <TableHeader>
        <TableRow className="border-zinc-800 hover:bg-transparent">
          <TableHead className="text-zinc-500">ID</TableHead>
          <TableHead className="text-zinc-500">Order</TableHead>
          <TableHead className="text-zinc-500">Price</TableHead>
          <TableHead className="text-zinc-500">Size</TableHead>
          <TableHead className="text-zinc-500">Fee</TableHead>
          <TableHead className="text-zinc-500">Filled At</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {fills.length === 0 ? (
          <TableRow className="border-zinc-800">
            <TableCell colSpan={6} className="text-center text-zinc-500">No fills found</TableCell>
          </TableRow>
        ) : (
          fills.map((fill) => (
            <TableRow key={fill.id} className="border-zinc-800 hover:bg-zinc-800/50">
              <TableCell className="text-zinc-300">#{fill.id}</TableCell>
              <TableCell className="text-zinc-300">#{fill.order_id}</TableCell>
              <TableCell className="font-mono text-zinc-300">{fill.price.toFixed(4)}</TableCell>
              <TableCell className="font-mono text-zinc-300">{fill.size.toFixed(2)}</TableCell>
              <TableCell className="font-mono text-zinc-400">{fill.fee.toFixed(6)}</TableCell>
              <TableCell className="text-xs text-zinc-400">{new Date(fill.filled_at).toLocaleString()}</TableCell>
            </TableRow>
          ))
        )}
      </TableBody>
    </Table>
  );
}
