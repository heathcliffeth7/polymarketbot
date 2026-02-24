'use client';

import { Badge } from '@/components/ui/badge';
import type { TradeState } from '@/lib/types';

const stateColors: Record<TradeState, string> = {
  Idle: 'bg-zinc-600 text-zinc-200',
  WaitingEntry: 'bg-blue-600 text-blue-100',
  EntryPlaced: 'bg-indigo-600 text-indigo-100',
  EntryPartiallyFilled: 'bg-indigo-500 text-indigo-100',
  EntryFilled: 'bg-cyan-600 text-cyan-100',
  TpPlaced: 'bg-emerald-600 text-emerald-100',
  SlArmed: 'bg-orange-600 text-orange-100',
  ExitPartiallyFilled: 'bg-yellow-600 text-yellow-100',
  ExitFilled: 'bg-green-600 text-green-100',
  Settled: 'bg-green-700 text-green-100',
  Halted: 'bg-red-600 text-red-100',
};

export function TradeStateBadge({ state }: { state: TradeState }) {
  return (
    <Badge className={stateColors[state] || 'bg-zinc-600 text-zinc-200'}>
      {state}
    </Badge>
  );
}
