'use client';

import { useCallback, useEffect, useState } from 'react';
import { usePolling } from './use-polling';
import type {
  TradeBuilderMarketSearchItem,
  TradeBuilderOutcome,
} from '@/lib/types';

export function useTradeBuilderMarketSearch(query: string) {
  const q = query.trim();
  const endpoint = q.length > 0 ? `/api/trade-builder/markets/search?q=${encodeURIComponent(q)}` : null;
  return usePolling<{ data: TradeBuilderMarketSearchItem[] }>(endpoint, 10000);
}

export function useTradeBuilderOutcomes(slug: string | null) {
  const endpoint = slug ? `/api/trade-builder/markets/${encodeURIComponent(slug)}/outcomes` : null;
  return usePolling<{ data: TradeBuilderOutcome[] }>(endpoint, 60000);
}

const LIVE_PRICE_INTERVAL = 3_000;

export function useCanvasLivePrices(slugs: string[]): Record<string, number> {
  const [prices, setPrices] = useState<Record<string, number>>({});
  const slugsKey = slugs.slice().sort().join(',');

  const fetchAll = useCallback(async () => {
    if (slugs.length === 0) return;
    try {
      const results = await Promise.all(
        slugs.map(async (slug) => {
          try {
            const res = await fetch(
              `/api/trade-builder/markets/${encodeURIComponent(slug)}/outcomes`,
              { cache: 'no-store' }
            );
            if (!res.ok) return [];
            const json = (await res.json()) as { data?: TradeBuilderOutcome[] };
            return json.data ?? [];
          } catch {
            return [];
          }
        })
      );
      const next: Record<string, number> = {};
      for (const outcomes of results) {
        for (const o of outcomes) {
          if (o.token_id && o.price != null) {
            next[o.token_id] = o.price;
          }
        }
      }
      setPrices(next);
    } catch {
      // keep existing prices on error
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [slugsKey]);

  useEffect(() => {
    fetchAll();
    if (slugs.length === 0) return;
    const id = setInterval(fetchAll, LIVE_PRICE_INTERVAL);
    return () => clearInterval(id);
  }, [fetchAll, slugs.length]);

  return prices;
}
