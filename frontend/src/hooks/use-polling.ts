'use client';

import useSWR from 'swr';
import { requestJson } from '@/lib/http-client';

export function usePolling<T>(url: string | null, intervalMs: number) {
  const fetcher = (endpoint: string) =>
    requestJson<T>(
      endpoint,
      { cache: 'no-store' },
      { timeoutMs: 10_000, retries: 1, retryDelayMs: 350 }
    );

  return useSWR<T>(url, fetcher, {
    refreshInterval: intervalMs,
    revalidateOnFocus: false,
    dedupingInterval: intervalMs / 2,
  });
}
