import type { AutoScopeTradeAnalysisTimeRange } from '@/lib/types';

const RELATIVE_TIME_RANGE_HOURS = {
  '3h': 3,
  '6h': 6,
  '12h': 12,
  '24h': 24,
  '1w': 168,
  '1m': 720,
} as const;

export interface AutoScopeTradeAnalysisDateFilters {
  timeRange: AutoScopeTradeAnalysisTimeRange;
  from: string | null;
  to: string | null;
  error: string | null;
}

function parseOptionalDate(value: string | null | undefined): string | null {
  const trimmed = value?.trim();
  if (!trimmed) return null;
  const parsed = new Date(trimmed);
  return Number.isNaN(parsed.getTime()) ? null : parsed.toISOString();
}

function normalizeTimeRange(
  value: string | null | undefined
): AutoScopeTradeAnalysisTimeRange | null {
  const trimmed = value?.trim();
  if (!trimmed) return null;
  if (
    trimmed === 'all' ||
    trimmed === 'custom' ||
    trimmed === '3h' ||
    trimmed === '6h' ||
    trimmed === '12h' ||
    trimmed === '24h' ||
    trimmed === '1w' ||
    trimmed === '1m'
  ) {
    return trimmed;
  }
  return null;
}

function hasDateValue(value: string | null | undefined): boolean {
  return Boolean(value?.trim());
}

export function resolveAutoScopeTradeAnalysisDateFilters({
  timeRangeRaw,
  fromRaw,
  toRaw,
  now = new Date(),
}: {
  timeRangeRaw?: string | null;
  fromRaw?: string | null;
  toRaw?: string | null;
  now?: Date;
}): AutoScopeTradeAnalysisDateFilters {
  const timeRange = normalizeTimeRange(timeRangeRaw);
  if (timeRangeRaw?.trim() && !timeRange) {
    return {
      timeRange: 'all',
      from: null,
      to: null,
      error: 'timeRange must be one of all, custom, 3h, 6h, 12h, 24h, 1w, 1m',
    };
  }

  if (timeRange && timeRange !== 'all' && timeRange !== 'custom') {
    const hours = RELATIVE_TIME_RANGE_HOURS[timeRange];
    const to = now.toISOString();
    const from = new Date(now.getTime() - hours * 60 * 60 * 1000).toISOString();
    return { timeRange, from, to, error: null };
  }

  const from = parseOptionalDate(fromRaw);
  const to = parseOptionalDate(toRaw);

  if (hasDateValue(fromRaw) && !from) {
    return { timeRange: 'custom', from: null, to: null, error: 'from must be a valid date' };
  }
  if (hasDateValue(toRaw) && !to) {
    return { timeRange: 'custom', from: null, to: null, error: 'to must be a valid date' };
  }

  return {
    timeRange: timeRange ?? (from || to ? 'custom' : 'all'),
    from,
    to,
    error: null,
  };
}
