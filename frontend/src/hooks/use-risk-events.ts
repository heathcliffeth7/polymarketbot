'use client';

import { usePolling } from './use-polling';
import type { RiskEvent, PaginatedResponse } from '@/lib/types';

export function useRiskEvents(page = 1, limit = 30, eventType?: string, decision?: string) {
  const params = new URLSearchParams({ page: String(page), limit: String(limit) });
  if (eventType) params.set('eventType', eventType);
  if (decision) params.set('decision', decision);
  return usePolling<PaginatedResponse<RiskEvent>>(`/api/risk-events?${params}`, 5000);
}
