'use client';

import { useEffect, useState } from 'react';
import useSWR from 'swr';
import { requestJson } from '@/lib/http-client';

export function usePolling<T>(url: string | null, intervalMs: number, paused = false) {
  const [isPageVisible, setIsPageVisible] = useState(() =>
    typeof document === 'undefined' ? true : document.visibilityState === 'visible'
  );

  useEffect(() => {
    if (typeof document === 'undefined') return;

    const handleVisibilityChange = () => {
      setIsPageVisible(document.visibilityState === 'visible');
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  }, []);

  const isPaused = paused || !isPageVisible;
  const fetcher = (endpoint: string) =>
    requestJson<T>(
      endpoint,
      { cache: 'no-store' },
      { timeoutMs: 10_000, retries: 1, retryDelayMs: 350 }
    );

  return useSWR<T>(url, fetcher, {
    refreshInterval: isPaused ? 0 : intervalMs,
    revalidateOnFocus: false,
    dedupingInterval: intervalMs / 2,
    isPaused: () => isPaused,
  });
}
