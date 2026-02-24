'use client';

import { usePolling } from './use-polling';
import { requestJson } from '@/lib/http-client';
import type {
  PaginatedResponse,
  TradeFlowDefinition,
  TradeFlowDefinitionDetail,
  TradeFlowEnsureDualDcaSourceTradeRequest,
  TradeFlowEnsureDualDcaSourceTradeResult,
  TradeFlowEnsureSourceTradeRequest,
  TradeFlowEnsureSourceTradeResult,
  TradeFlowEvent,
  TradeFlowOpenPositionsResponse,
  TradeFlowRun,
  TradeFlowValidationResult,
  TradeFlowVersion,
} from '@/lib/types';

function buildSearchParams(params: Record<string, string | undefined>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value == null || value === '') continue;
    search.set(key, value);
  }
  return search.toString();
}

export function useTradeFlowDefinitions(
  page = 1,
  limit = 20,
  status?: string,
  autoMigrateLegacy = true
) {
  const query = buildSearchParams({
    page: String(page),
    limit: String(limit),
    status,
    autoMigrateLegacy: autoMigrateLegacy ? '1' : '0',
  });
  const endpoint = `/api/trade-flow/definitions?${query}`;
  return usePolling<PaginatedResponse<TradeFlowDefinition>>(endpoint, 4000);
}

export function useTradeFlowDefinitionDetail(definitionId: number | null) {
  const endpoint = definitionId ? `/api/trade-flow/definitions/${definitionId}` : null;
  return usePolling<{ data: TradeFlowDefinitionDetail }>(endpoint, 4000);
}

export function useTradeFlowVersions(definitionId: number | null) {
  const endpoint = definitionId ? `/api/trade-flow/definitions/${definitionId}/versions` : null;
  return usePolling<{ data: TradeFlowVersion[] }>(endpoint, 7000);
}

export function useTradeFlowOpenPositions() {
  return usePolling<TradeFlowOpenPositionsResponse>('/api/trade-flow/open-positions', 8000);
}

export async function ensureTradeFlowSourceTrade(payload: TradeFlowEnsureSourceTradeRequest) {
  return requestJson<{ data: TradeFlowEnsureSourceTradeResult }>(
    '/api/trade-flow/open-positions/ensure-source-trade',
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    },
    { timeoutMs: 15_000 }
  );
}

export async function ensureDualDcaSourceTrade(
  payload: TradeFlowEnsureDualDcaSourceTradeRequest
) {
  return requestJson<{ data: TradeFlowEnsureDualDcaSourceTradeResult }>(
    '/api/trade-flow/dual-dca/ensure-source-trade',
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    },
    { timeoutMs: 15_000 }
  );
}

export function useTradeFlowRuns(
  page = 1,
  limit = 20,
  definitionId?: number,
  status?: string
) {
  const query = buildSearchParams({
    page: String(page),
    limit: String(limit),
    definitionId: definitionId != null ? String(definitionId) : undefined,
    status,
  });
  const endpoint = `/api/trade-flow/runs?${query}`;
  return usePolling<PaginatedResponse<TradeFlowRun>>(endpoint, 3000);
}

export function useTradeFlowRunEvents(
  runId: number | null,
  page = 1,
  limit = 50,
  enabled = true
) {
  const query = buildSearchParams({
    page: String(page),
    limit: String(limit),
  });
  const endpoint = enabled && runId ? `/api/trade-flow/runs/${runId}/events?${query}` : null;
  return usePolling<PaginatedResponse<TradeFlowEvent>>(endpoint, 3000);
}

export async function createTradeFlowDefinition(payload: Record<string, unknown>) {
  return requestJson<{ data: TradeFlowDefinitionDetail }>(
    '/api/trade-flow/definitions',
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    },
    { timeoutMs: 15_000 }
  );
}

export async function patchTradeFlowDefinitionDraft(
  definitionId: number,
  payload: Record<string, unknown>
) {
  return requestJson<{ data: TradeFlowDefinitionDetail }>(
    `/api/trade-flow/definitions/${definitionId}`,
    {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    },
    { timeoutMs: 15_000 }
  );
}

export async function validateTradeFlowDefinition(
  definitionId: number,
  payload: Record<string, unknown>
) {
  return requestJson<{ data: TradeFlowValidationResult }>(
    `/api/trade-flow/definitions/${definitionId}/validate`,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    },
    { timeoutMs: 15_000 }
  );
}

export async function publishTradeFlowDefinition(definitionId: number) {
  return requestJson<{ data: TradeFlowDefinitionDetail }>(
    `/api/trade-flow/definitions/${definitionId}/publish`,
    { method: 'POST' },
    { timeoutMs: 15_000 }
  );
}

export async function archiveTradeFlowDefinition(definitionId: number) {
  return requestJson<{ data: TradeFlowDefinitionDetail }>(
    `/api/trade-flow/definitions/${definitionId}/archive`,
    { method: 'POST' },
    { timeoutMs: 15_000 }
  );
}
