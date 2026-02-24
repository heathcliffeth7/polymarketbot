'use client';

import { usePolling } from './use-polling';
import type {
  PaginatedResponse,
  TradeBuilderMarketSearchItem,
  TradeBuilderOrder,
  TradeBuilderOrderEvent,
  TradeBuilderOutcome,
  TradeBuilderWorkflowDetail,
  TradeBuilderWorkflowEvent,
} from '@/lib/types';

export function useTradeBuilderOrders(page = 1, limit = 20, status?: string) {
  const params = new URLSearchParams({ page: String(page), limit: String(limit) });
  if (status) params.set('status', status);
  return usePolling<PaginatedResponse<TradeBuilderOrder>>(`/api/trade-builder/orders?${params}`, 3000);
}

export function useTradeBuilderMarketSearch(query: string) {
  const q = query.trim();
  const endpoint = q.length > 0 ? `/api/trade-builder/markets/search?q=${encodeURIComponent(q)}` : null;
  return usePolling<{ data: TradeBuilderMarketSearchItem[] }>(endpoint, 10000);
}

export function useTradeBuilderOutcomes(slug: string | null) {
  const endpoint = slug ? `/api/trade-builder/markets/${encodeURIComponent(slug)}/outcomes` : null;
  return usePolling<{ data: TradeBuilderOutcome[] }>(endpoint, 10000);
}

export function useTradeBuilderOrderEvents(
  orderId: number | null,
  page = 1,
  limit = 25,
  eventType?: string,
  enabled = true
) {
  const params = new URLSearchParams({ page: String(page), limit: String(limit) });
  if (eventType) params.set('eventType', eventType);
  const endpoint =
    enabled && orderId != null
      ? `/api/trade-builder/orders/${orderId}/events?${params}`
      : null;
  return usePolling<PaginatedResponse<TradeBuilderOrderEvent>>(endpoint, 3000);
}

export function useTradeBuilderWorkflows(page = 1, limit = 20, status?: string) {
  const params = new URLSearchParams({ page: String(page), limit: String(limit) });
  if (status) params.set('status', status);
  return usePolling<PaginatedResponse<TradeBuilderWorkflowDetail>>(
    `/api/trade-builder/workflows?${params}`,
    3000
  );
}

export function useTradeBuilderWorkflowEvents(
  workflowId: number | null,
  page = 1,
  limit = 25,
  eventType?: string,
  enabled = true
) {
  const params = new URLSearchParams({ page: String(page), limit: String(limit) });
  if (eventType) params.set('eventType', eventType);
  const endpoint =
    enabled && workflowId != null
      ? `/api/trade-builder/workflows/${workflowId}/events?${params}`
      : null;
  return usePolling<PaginatedResponse<TradeBuilderWorkflowEvent>>(endpoint, 3000);
}

export async function createTradeBuilderOrder(payload: Record<string, unknown>) {
  const res = await fetch('/api/trade-builder/orders', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data?.error || `HTTP ${res.status}`);
  return data;
}

export async function patchTradeBuilderOrder(id: number, payload: Record<string, unknown>) {
  const res = await fetch(`/api/trade-builder/orders/${id}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data?.error || `HTTP ${res.status}`);
  return data;
}

export async function cancelTradeBuilderOrder(id: number) {
  const res = await fetch(`/api/trade-builder/orders/${id}`, {
    method: 'DELETE',
  });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data?.error || `HTTP ${res.status}`);
  return data;
}

export async function createTradeBuilderWorkflow(payload: Record<string, unknown>) {
  const res = await fetch('/api/trade-builder/workflows', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data?.error || `HTTP ${res.status}`);
  return data;
}

export async function patchTradeBuilderWorkflow(id: number, payload: Record<string, unknown>) {
  const res = await fetch(`/api/trade-builder/workflows/${id}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data?.error || `HTTP ${res.status}`);
  return data;
}

export async function cancelTradeBuilderWorkflow(id: number) {
  const res = await fetch(`/api/trade-builder/workflows/${id}`, {
    method: 'DELETE',
  });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data?.error || `HTTP ${res.status}`);
  return data;
}
