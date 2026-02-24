'use client';

import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Badge } from '@/components/ui/badge';
import type { RiskEvent } from '@/lib/types';

const decisionColors: Record<string, string> = {
  allow: 'bg-emerald-600',
  block: 'bg-orange-600',
  halt: 'bg-red-600',
};

export function RiskEventTable({ events }: { events: RiskEvent[] }) {
  return (
    <Table>
      <TableHeader>
        <TableRow className="border-zinc-800 hover:bg-transparent">
          <TableHead className="text-zinc-500">ID</TableHead>
          <TableHead className="text-zinc-500">Trade</TableHead>
          <TableHead className="text-zinc-500">Event Type</TableHead>
          <TableHead className="text-zinc-500">Decision</TableHead>
          <TableHead className="text-zinc-500">Details</TableHead>
          <TableHead className="text-zinc-500">Time</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {events.length === 0 ? (
          <TableRow className="border-zinc-800">
            <TableCell colSpan={6} className="text-center text-zinc-500">No risk events found</TableCell>
          </TableRow>
        ) : (
          events.map((event) => (
            <TableRow key={event.id} className="border-zinc-800 hover:bg-zinc-800/50">
              <TableCell className="text-zinc-300">#{event.id}</TableCell>
              <TableCell className="text-zinc-300">{event.trade_id ? `#${event.trade_id}` : '-'}</TableCell>
              <TableCell>
                <Badge variant="outline" className="border-zinc-700 text-zinc-300">{event.event_type}</Badge>
              </TableCell>
              <TableCell>
                <Badge className={decisionColors[event.decision] || 'bg-zinc-600'}>{event.decision}</Badge>
              </TableCell>
              <TableCell className="max-w-[300px] truncate text-xs text-zinc-400">{event.details}</TableCell>
              <TableCell className="text-xs text-zinc-400">{new Date(event.created_at).toLocaleString()}</TableCell>
            </TableRow>
          ))
        )}
      </TableBody>
    </Table>
  );
}
