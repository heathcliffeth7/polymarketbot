'use client';

import { usePolling } from './use-polling';
import type { Order, Fill, PaginatedResponse } from '@/lib/types';

export function useOrders(page = 1, limit = 20, filters?: { tradeId?: number; status?: string; intent?: string }) {
  const params = new URLSearchParams({ page: String(page), limit: String(limit) });
  if (filters?.tradeId) params.set('tradeId', String(filters.tradeId));
  if (filters?.status) params.set('status', filters.status);
  if (filters?.intent) params.set('intent', filters.intent);
  return usePolling<PaginatedResponse<Order>>(`/api/orders?${params}`, 3000);
}

export function useFills(page = 1, limit = 20, orderId?: number) {
  const params = new URLSearchParams({ page: String(page), limit: String(limit) });
  if (orderId) params.set('orderId', String(orderId));
  return usePolling<PaginatedResponse<Fill>>(`/api/fills?${params}`, 3000);
}
