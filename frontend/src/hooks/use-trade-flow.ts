'use client';

import { usePolling } from './use-polling';
import { requestJson, type RequestJsonOptions } from '@/lib/http-client';
import type {
  AutoScopeTradeAnalysisPnlFilter,
  AutoScopeTradeAnalysisPositionFilter,
  AutoScopeTradeDiagnosticResponse,
  AutoScopeTradeAnalysisResponse,
  AutoScopeTradeAnalysisSortBy,
  AutoScopeTradeAnalysisSortDirection,
  AutoScopeTradeAnalysisTimeRange,
  PaginatedResponse,
  TradeFlowNodeRuntimeResponse,
  TradeFlowPtbStateResponse,
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

export interface TradeFlowAutoScopeAnalysisParams {
  page?: number;
  limit?: number;
  sortBy?: AutoScopeTradeAnalysisSortBy;
  sortDirection?: AutoScopeTradeAnalysisSortDirection;
  pnl?: AutoScopeTradeAnalysisPnlFilter;
  position?: AutoScopeTradeAnalysisPositionFilter;
  timeRange?: AutoScopeTradeAnalysisTimeRange;
  from?: string;
  to?: string;
  enabled?: boolean;
}

export function buildTradeFlowAutoScopeAnalysisQuery(
  params: TradeFlowAutoScopeAnalysisParams
): string {
  const relativeTimeRange =
    params.timeRange && params.timeRange !== 'all' && params.timeRange !== 'custom'
      ? params.timeRange
      : undefined;

  return buildSearchParams({
    page: params.page != null ? String(params.page) : undefined,
    limit: params.limit != null ? String(params.limit) : undefined,
    sortBy: params.sortBy,
    sortDirection: params.sortDirection,
    pnl: params.pnl,
    position: params.position,
    timeRange: relativeTimeRange,
    from: relativeTimeRange ? undefined : params.from,
    to: relativeTimeRange ? undefined : params.to,
  });
}

export function useTradeFlowDefinitions(
  page = 1,
  limit = 20,
  status?: string,
  autoMigrateLegacy = false,
  paused = false
) {
  const query = buildSearchParams({
    page: String(page),
    limit: String(limit),
    status,
    autoMigrateLegacy: autoMigrateLegacy ? '1' : '0',
  });
  const endpoint = `/api/trade-flow/definitions?${query}`;
  return usePolling<PaginatedResponse<TradeFlowDefinition>>(endpoint, 4000, paused);
}

export function useTradeFlowDefinitionDetail(definitionId: number | null, paused = false) {
  const endpoint = definitionId ? `/api/trade-flow/definitions/${definitionId}` : null;
  return usePolling<{ data: TradeFlowDefinitionDetail }>(endpoint, 4000, paused);
}

export function useTradeFlowVersions(definitionId: number | null) {
  const endpoint = definitionId ? `/api/trade-flow/definitions/${definitionId}/versions` : null;
  return usePolling<{ data: TradeFlowVersion[] }>(endpoint, 7000);
}

export function useTradeFlowOpenPositions(paused = false) {
  return usePolling<TradeFlowOpenPositionsResponse>(
    '/api/trade-flow/open-positions',
    8000,
    paused
  );
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

export function useTradeFlowRecentEvents(
  status: TradeFlowRun['status'] | undefined = 'running',
  limit = 100,
  enabled = true
) {
  const query = buildSearchParams({
    status,
    limit: String(limit),
  });
  const endpoint = enabled ? `/api/trade-flow/events/recent?${query}` : null;
  return usePolling<{ data: TradeFlowEvent[] }>(endpoint, 3000);
}

export function useTradeFlowAutoScopeAnalysis({
  page = 1,
  limit = 50,
  sortBy = 'default',
  sortDirection = 'desc',
  pnl = 'all',
  position = 'all',
  timeRange = 'all',
  from,
  to,
  enabled = true,
}: TradeFlowAutoScopeAnalysisParams = {}) {
  const query = buildTradeFlowAutoScopeAnalysisQuery({
    page,
    limit,
    sortBy,
    sortDirection,
    pnl,
    position,
    timeRange,
    from,
    to,
  });
  const endpoint = enabled ? `/api/trade-flow/analytics/auto-scope?${query}` : null;
  return usePolling<AutoScopeTradeAnalysisResponse>(endpoint, 10_000);
}

export function useTradeFlowAutoScopeTradeDiagnostic(
  rootOrderId: number | null,
  enabled = true
) {
  const endpoint =
    enabled && rootOrderId
      ? `/api/trade-flow/analytics/auto-scope/${rootOrderId}/diagnostics`
      : null;
  return usePolling<AutoScopeTradeDiagnosticResponse>(endpoint, 10_000);
}

export function useTradeFlowPtbState(
  runId: number | null,
  page = 1,
  limit = 50,
  enabled = true
) {
  const query = buildSearchParams({
    page: String(page),
    limit: String(limit),
    runId: runId != null ? String(runId) : undefined,
  });
  const endpoint = enabled ? `/api/trade-flow/analytics/ptb-state?${query}` : null;
  return usePolling<TradeFlowPtbStateResponse>(endpoint, 10_000);
}

export function useTradeFlowNodeRuntime(
  runId: number | null,
  page = 1,
  limit = 50,
  nodeKey?: string,
  nodeType?: string,
  enabled = true
) {
  const query = buildSearchParams({
    page: String(page),
    limit: String(limit),
    runId: runId != null ? String(runId) : undefined,
    nodeKey,
    nodeType,
  });
  const endpoint = enabled ? `/api/trade-flow/analytics/node-runtime?${query}` : null;
  return usePolling<TradeFlowNodeRuntimeResponse>(endpoint, 10_000);
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
  payload: Record<string, unknown>,
  options?: RequestJsonOptions
) {
  return requestJson<{ data: TradeFlowDefinitionDetail }>(
    `/api/trade-flow/definitions/${definitionId}`,
    {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    },
    { timeoutMs: 30_000, ...options }
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

export async function stopTradeFlowDefinition(definitionId: number) {
  return requestJson<{ data: TradeFlowDefinitionDetail }>(
    `/api/trade-flow/definitions/${definitionId}/stop`,
    { method: 'POST' },
    { timeoutMs: 15_000 }
  );
}

export async function deleteTradeFlowDefinition(definitionId: number) {
  return requestJson<{ success: boolean; data: null }>(
    `/api/trade-flow/definitions/${definitionId}`,
    { method: 'DELETE' },
    { timeoutMs: 60_000 }
  );
}
