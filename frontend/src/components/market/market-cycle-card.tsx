'use client';

import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import type { Market } from '@/lib/types';

const statusColors: Record<string, string> = {
  open: 'bg-emerald-600',
  closed: 'bg-zinc-600',
  settled: 'bg-blue-600',
};

export function MarketCycleCard({ markets }: { markets: Market[] }) {
  return (
    <Card className="border-zinc-800 bg-zinc-900">
      <CardHeader>
        <CardTitle className="text-sm font-medium text-zinc-400">Market Cycles</CardTitle>
      </CardHeader>
      <CardContent>
        <Table>
          <TableHeader>
            <TableRow className="border-zinc-800 hover:bg-transparent">
              <TableHead className="text-zinc-500">ID</TableHead>
              <TableHead className="text-zinc-500">Slug</TableHead>
              <TableHead className="text-zinc-500">Status</TableHead>
              <TableHead className="text-zinc-500">Starts</TableHead>
              <TableHead className="text-zinc-500">Ends</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {markets.length === 0 ? (
              <TableRow className="border-zinc-800">
                <TableCell colSpan={5} className="text-center text-zinc-500">No markets found</TableCell>
              </TableRow>
            ) : (
              markets.map((m) => (
                <TableRow key={m.id} className="border-zinc-800 hover:bg-zinc-800/50">
                  <TableCell className="text-zinc-300">#{m.id}</TableCell>
                  <TableCell className="max-w-[250px] truncate text-zinc-300">{m.market_slug}</TableCell>
                  <TableCell>
                    <Badge className={statusColors[m.status] || 'bg-zinc-600'}>{m.status}</Badge>
                  </TableCell>
                  <TableCell className="text-xs text-zinc-400">{new Date(m.starts_at).toLocaleString()}</TableCell>
                  <TableCell className="text-xs text-zinc-400">{new Date(m.ends_at).toLocaleString()}</TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  );
}
