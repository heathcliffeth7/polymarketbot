'use client';

import { usePolling } from './use-polling';
import type { Trade, PaginatedResponse } from '@/lib/types';

export function useTrades(page = 1, limit = 20, state?: string) {
  const params = new URLSearchParams({ page: String(page), limit: String(limit) });
  if (state) params.set('state', state);
  return usePolling<PaginatedResponse<Trade>>(`/api/trades?${params}`, 5000);
}
