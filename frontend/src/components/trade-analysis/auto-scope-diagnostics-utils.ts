import type { AutoScopeTradeAnalysisRow } from '@/lib/types';

export function formatDateTime(value: string | null): string {
  if (!value) return '-';
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return '-';
  return parsed.toLocaleString();
}

export function formatDuration(ms: number | null): string {
  if (ms == null) return '-';
  if (ms < 1000) return `${ms} ms`;

  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) return `${hours}sa ${minutes}d ${seconds}sn`;
  if (minutes > 0) return `${minutes}d ${seconds}sn`;
  return `${seconds}sn`;
}

export function formatPrice(value: number | null): string {
  return value == null ? '-' : value.toFixed(4);
}

export function formatQty(value: number): string {
  return value.toFixed(4);
}

export function formatUsdc(value: number | null): string {
  return value == null ? '-' : `${value.toFixed(2)} USDC`;
}

export function formatPnl(value: number): string {
  const sign = value > 0 ? '+' : '';
  return `${sign}${value.toFixed(2)} USDC`;
}

export function formatPercent(value: number | null): string {
  if (value == null) return '-';
  const sign = value > 0 ? '+' : '';
  return `${sign}${value.toFixed(2)}%`;
}

export function formatScore(value: number | null): string {
  return value == null ? '-' : `${value.toFixed(0)}/100`;
}

export function pnlClassName(value: number): string {
  if (value > 0) return 'text-emerald-400';
  if (value < 0) return 'text-red-400';
  return 'text-zinc-300';
}

export function diagnosisBadgeClassName(row: AutoScopeTradeAnalysisRow): string {
  const code = row.primaryDiagnosisCode;
  if (code === 'clean_win' || code === 'take_profit_success') {
    return 'border-emerald-800 bg-emerald-950/40 text-emerald-300';
  }
  if (code === 'unknown') {
    return 'border-zinc-700 bg-zinc-950 text-zinc-300';
  }
  if (row.rowPnlUsdc < 0) {
    return 'border-red-900 bg-red-950/40 text-red-300';
  }
  return 'border-amber-900 bg-amber-950/40 text-amber-300';
}

export function exitReasonLabel(row: AutoScopeTradeAnalysisRow): string {
  if (row.rowType === 'settled_payout') return 'Settlement';
  if (row.positionState === 'closed_market_ended') return 'Kapali (Market End)';
  if (row.positionState === 'open') return 'Acik Pozisyon';
  switch (row.exitReason) {
    case 'tp':
      return 'TP';
    case 'sl':
      return 'SL';
    case 'window_end_auto_sell':
      return 'Window End';
    default:
      return 'Diger';
  }
}

export function exitMetaLabel(row: AutoScopeTradeAnalysisRow): string {
  if (row.rowType === 'settled_payout') {
    return row.sellFilledAt
      ? `Redeem: ${formatDateTime(row.sellFilledAt)}`
      : 'Polymarket redeem';
  }
  if (row.positionState === 'closed_market_ended') {
    return row.marketEndAt
      ? `Market bitti: ${formatDateTime(row.marketEndAt)}`
      : 'Market bitti';
  }
  if (row.positionState === 'open') {
    return row.markPriceCapturedAt
      ? `Mark: ${formatDateTime(row.markPriceCapturedAt)}`
      : 'Acik pozisyon devam ediyor';
  }
  return row.sellFilledAt ? formatDateTime(row.sellFilledAt) : 'Kapandi';
}
